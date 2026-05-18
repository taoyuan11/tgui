use std::cell::RefCell;
use std::borrow::Cow;
use std::collections::HashMap;

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

use keys::{TextLayoutKey, TextMeasureKey};

pub(crate) struct FontManager {
    pub(super) font_system: RefCell<FontSystem>,
    aliases: Vec<(String, String)>,
    default_font: Option<String>,
    measure_cache: RefCell<HashMap<TextMeasureKey, (f32, f32)>>,
    layout_cache: RefCell<HashMap<TextLayoutKey, TextLayoutInfo>>,
}

impl FontManager {
    pub(crate) fn new(catalog: &FontCatalog) -> Self {
        let mut font_system = FontSystem::new();
        let aliases = catalog.configure_font_system(&mut font_system);

        Self {
            font_system: RefCell::new(font_system),
            aliases,
            default_font: catalog.default_font.clone(),
            measure_cache: RefCell::new(HashMap::new()),
            layout_cache: RefCell::new(HashMap::new()),
        }
    }

    pub(super) fn text_key(text: &str) -> Cow<'static, str> {
        Cow::Owned(text.to_owned())
    }

    pub(super) fn font_key(font: Option<&str>) -> Option<Cow<'static, str>> {
        font.map(|name| Cow::Owned(name.to_owned()))
    }

    pub(crate) fn resolve_text(&self, text: &str, request: TextFontRequest<'_>) -> ResolvedText {
        let font_system = self.font_system.borrow();
        self.resolve_text_with_database(font_system.db(), text, request)
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

    pub(crate) fn buffer_attrs_owned(
        &self,
        font_system: &FontSystem,
        text: &str,
        request: TextFontRequest<'_>,
        font_size: f32,
        letter_spacing: f32,
    ) -> AttrsOwned {
        let resolved = self.resolve_text_with_database(font_system.db(), text, request.clone());
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
}
