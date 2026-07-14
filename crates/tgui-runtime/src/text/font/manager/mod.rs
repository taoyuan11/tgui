use hashbrown::HashMap;
use std::borrow::Cow;
#[cfg(any(test, feature = "bench-support"))]
use std::cell::Cell;
use std::cell::RefCell;
use std::sync::atomic::{AtomicU64, Ordering};

use cosmic_text::{Attrs, AttrsOwned, Buffer, Family, FontSystem, Metrics, Shaping, Weight, Wrap};

use super::catalog::{
    default_family_name, query_family_name, FontCatalog, FontWeight, ResolvedText, TextFontRequest,
};
use super::layout::{measured_glyph_line_height, TextLayoutInfo};
use super::platform::{
    contains_cjk, contains_non_cjk_alphanumeric, desktop_cjk_sans_candidates, first_matching_family,
};

mod keys;
mod layout_ops;

use keys::{TextLayoutKey, TextMeasureKey, TextResolveKey, TextResolveScript};

static NEXT_FONT_SYSTEM_CACHE_IDENTITY: AtomicU64 = AtomicU64::new(1);
const MAX_TEXT_RESOLVE_CACHE_ENTRIES: usize = 4096;

pub(crate) struct FontManager {
    pub(super) font_system: RefCell<FontSystem>,
    cache_identity: u64,
    aliases: Vec<(String, String)>,
    default_font: Option<String>,
    measure_cache: RefCell<HashMap<TextMeasureKey, (f32, f32)>>,
    layout_cache: RefCell<HashMap<TextLayoutKey, TextLayoutInfo>>,
    resolve_cache: RefCell<HashMap<TextResolveKey, ResolvedText>>,
    #[cfg(any(test, feature = "bench-support"))]
    resolve_query_count: Cell<u64>,
    #[cfg(any(test, feature = "bench-support"))]
    measure_only_calls: Cell<u64>,
    #[cfg(any(test, feature = "bench-support"))]
    measure_only_cache_misses: Cell<u64>,
    #[cfg(any(test, feature = "bench-support"))]
    precise_measure_calls: Cell<u64>,
    #[cfg(any(test, feature = "bench-support"))]
    precise_layout_builds: Cell<u64>,
    #[cfg(any(test, feature = "bench-support"))]
    text_key_owned_allocations: Cell<u64>,
    #[cfg(any(test, feature = "bench-support"))]
    text_key_scanned_bytes: Cell<u64>,
    #[cfg(feature = "bench-support")]
    force_precise_measurement: Cell<bool>,
}

impl FontManager {
    pub(crate) fn new(catalog: &FontCatalog) -> Self {
        let mut font_system = FontSystem::new();
        let aliases = catalog.configure_font_system(&mut font_system);

        Self {
            font_system: RefCell::new(font_system),
            cache_identity: NEXT_FONT_SYSTEM_CACHE_IDENTITY.fetch_add(1, Ordering::Relaxed),
            aliases,
            default_font: catalog.default_font.clone(),
            measure_cache: RefCell::new(HashMap::new()),
            layout_cache: RefCell::new(HashMap::new()),
            resolve_cache: RefCell::new(HashMap::new()),
            #[cfg(any(test, feature = "bench-support"))]
            resolve_query_count: Cell::new(0),
            #[cfg(any(test, feature = "bench-support"))]
            measure_only_calls: Cell::new(0),
            #[cfg(any(test, feature = "bench-support"))]
            measure_only_cache_misses: Cell::new(0),
            #[cfg(any(test, feature = "bench-support"))]
            precise_measure_calls: Cell::new(0),
            #[cfg(any(test, feature = "bench-support"))]
            precise_layout_builds: Cell::new(0),
            #[cfg(any(test, feature = "bench-support"))]
            text_key_owned_allocations: Cell::new(0),
            #[cfg(any(test, feature = "bench-support"))]
            text_key_scanned_bytes: Cell::new(0),
            #[cfg(feature = "bench-support")]
            force_precise_measurement: Cell::new(false),
        }
    }

    pub(crate) fn resolve_text(&self, text: &str, request: TextFontRequest<'_>) -> ResolvedText {
        let font_system = self.font_system.borrow();
        self.resolve_text_cached(font_system.db(), text, request)
    }

    fn resolve_text_cached(
        &self,
        database: &cosmic_text::fontdb::Database,
        text: &str,
        request: TextFontRequest<'_>,
    ) -> ResolvedText {
        // 解析结果依赖脚本类别(脚本感知 CJK 回退)、优先字体与字重。
        let cache_key = TextResolveKey {
            catalog_identity: self.cache_identity,
            database_identity: database as *const cosmic_text::fontdb::Database as usize,
            database_face_count: database.len(),
            script: Self::resolve_script_key(text),
            preferred_font: request
                .preferred_font
                .map(|name| Cow::Owned(name.to_owned())),
            weight: request.weight,
        };
        if let Some(cached) = self.resolve_cache.borrow().get(&cache_key) {
            return cached.clone();
        }

        #[cfg(any(test, feature = "bench-support"))]
        self.resolve_query_count
            .set(self.resolve_query_count.get().saturating_add(1));
        let resolved = self.resolve_text_with_database(database, text, request);

        let mut cache = self.resolve_cache.borrow_mut();
        if cache.len() >= MAX_TEXT_RESOLVE_CACHE_ENTRIES {
            cache.clear();
        }
        cache.insert(cache_key, resolved.clone());
        resolved
    }

    fn resolve_script_key(text: &str) -> TextResolveScript {
        if contains_cjk(text) && !contains_non_cjk_alphanumeric(text) {
            TextResolveScript::CjkOnly
        } else {
            TextResolveScript::Other
        }
    }

    fn resolve_text_with_database(
        &self,
        database: &cosmic_text::fontdb::Database,
        text: &str,
        request: TextFontRequest<'_>,
    ) -> ResolvedText {
        let preferred = request
            .preferred_font
            .and_then(|name| self.resolve_family_name_in_database(database, name, request.weight));
        let configured_default = self
            .default_font
            .as_deref()
            .and_then(|name| self.resolve_family_name_in_database(database, name, request.weight));
        let script_aware_default =
            self.script_aware_default_family_in_database(database, text, request.weight);

        ResolvedText {
            primary_font: preferred
                .or(configured_default)
                .or(script_aware_default)
                .or_else(|| self.system_default_family_in_database(database, request.weight))
                .unwrap_or_else(|| "sans-serif".to_string()),
        }
    }

    pub(crate) fn with_font_system<T>(&self, f: impl FnOnce(&mut FontSystem) -> T) -> T {
        let mut font_system = self.font_system.borrow_mut();
        f(&mut font_system)
    }

    /// Stable identity for caches whose cosmic-text font IDs are database-local.
    pub(crate) fn cache_identity(&self) -> u64 {
        self.cache_identity
    }

    pub(crate) fn buffer_attrs_owned(
        &self,
        font_system: &FontSystem,
        text: &str,
        request: TextFontRequest<'_>,
        font_size: f32,
        letter_spacing: f32,
    ) -> AttrsOwned {
        // This path used to bypass `resolve_cache`, causing every Buffer setup
        // to rescan fontdb families even when text, script, weight, and catalog
        // were unchanged. It cannot call `resolve_text` because the caller
        // already holds the FontSystem borrow, so use the database-aware cache
        // helper directly.
        let resolved = self.resolve_text_cached(font_system.db(), text, request.clone());
        let attrs = Attrs::new()
            .family(Family::Name(&resolved.primary_font))
            .weight(Weight(request.weight.to_raw()))
            .letter_spacing(letter_spacing / font_size.max(1.0));
        AttrsOwned::new(&attrs)
    }

    pub(crate) fn finish_buffer_layout(
        &self,
        font_system: &mut FontSystem,
        buffer: &mut Buffer,
        font_size: f32,
        line_height: f32,
    ) {
        buffer.shape_until_scroll(font_system, false);
        let effective_line_height =
            measured_glyph_line_height(buffer, font_system, line_height).max(line_height);
        let desired_metrics = Metrics::new(font_size, effective_line_height);
        if buffer.metrics() != desired_metrics {
            buffer.set_metrics(desired_metrics);
            buffer.shape_until_scroll(font_system, false);
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn configure_buffer(
        &self,
        font_system: &mut FontSystem,
        buffer: &mut Buffer,
        text: &str,
        request: TextFontRequest<'_>,
        font_size: f32,
        line_height: f32,
        letter_spacing: f32,
        width_opt: Option<f32>,
        height_opt: Option<f32>,
        wrap: Wrap,
    ) {
        let attrs = self.buffer_attrs_owned(font_system, text, request, font_size, letter_spacing);
        self.configure_buffer_with_attrs(
            font_system,
            buffer,
            text,
            &attrs,
            font_size,
            line_height,
            width_opt,
            height_opt,
            wrap,
        );
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn configure_buffer_with_attrs(
        &self,
        font_system: &mut FontSystem,
        buffer: &mut Buffer,
        text: &str,
        attrs: &AttrsOwned,
        font_size: f32,
        line_height: f32,
        width_opt: Option<f32>,
        height_opt: Option<f32>,
        wrap: Wrap,
    ) {
        buffer.set_metrics_and_size(Metrics::new(font_size, line_height), width_opt, height_opt);
        buffer.set_wrap(wrap);
        buffer.set_text(text, &attrs.as_attrs(), Shaping::Advanced, None);
        self.finish_buffer_layout(font_system, buffer, font_size, line_height);
    }

    fn resolve_family_name_in_database(
        &self,
        database: &cosmic_text::fontdb::Database,
        name: &str,
        weight: FontWeight,
    ) -> Option<String> {
        if let Some((_, family)) = self.aliases.iter().find(|(alias, _)| alias == name) {
            return Some(family.clone());
        }

        query_family_name(database, name, weight)
    }

    fn system_default_family_in_database(
        &self,
        database: &cosmic_text::fontdb::Database,
        weight: FontWeight,
    ) -> Option<String> {
        default_family_name(database, weight)
    }

    fn script_aware_default_family_in_database(
        &self,
        database: &cosmic_text::fontdb::Database,
        text: &str,
        weight: FontWeight,
    ) -> Option<String> {
        if !contains_cjk(text) || contains_non_cjk_alphanumeric(text) {
            return None;
        }

        desktop_cjk_sans_candidates()
            .and_then(|candidates| first_matching_family(database, candidates))
            .or_else(|| self.system_default_family_in_database(database, weight))
    }

    #[cfg(test)]
    pub(super) fn layout_cache_len(&self) -> usize {
        self.layout_cache.borrow().len()
    }

    #[cfg(any(test, feature = "bench-support"))]
    pub(crate) fn resolve_query_count(&self) -> u64 {
        self.resolve_query_count.get()
    }

    #[cfg(feature = "bench-support")]
    pub(crate) fn clear_resolve_cache_for_benchmark(&self) {
        self.resolve_cache.borrow_mut().clear();
    }

    #[cfg(any(test, feature = "bench-support"))]
    pub(crate) fn text_measure_activity(&self) -> (u64, u64, u64, u64) {
        (
            self.measure_only_calls.get(),
            self.measure_only_cache_misses.get(),
            self.precise_measure_calls.get(),
            self.precise_layout_builds.get(),
        )
    }

    #[cfg(any(test, feature = "bench-support"))]
    pub(crate) fn reset_text_measure_activity(&self) {
        self.measure_only_calls.set(0);
        self.measure_only_cache_misses.set(0);
        self.precise_measure_calls.set(0);
        self.precise_layout_builds.set(0);
    }

    #[cfg(feature = "bench-support")]
    pub(crate) fn clear_text_measure_caches_for_benchmark(&self) {
        self.measure_cache.borrow_mut().clear();
        self.layout_cache.borrow_mut().clear();
    }

    #[cfg(any(test, feature = "bench-support"))]
    pub(crate) fn text_key_activity(&self) -> (u64, u64) {
        (
            self.text_key_owned_allocations.get(),
            self.text_key_scanned_bytes.get(),
        )
    }

    #[cfg(any(test, feature = "bench-support"))]
    pub(crate) fn reset_text_key_activity(&self) {
        self.text_key_owned_allocations.set(0);
        self.text_key_scanned_bytes.set(0);
    }

    #[cfg(feature = "bench-support")]
    pub(crate) fn force_precise_measurement_for_benchmark(&self, force: bool) {
        self.force_precise_measurement.set(force);
    }
}
