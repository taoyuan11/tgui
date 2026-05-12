use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use cosmic_text::fontdb::{Family, Query, Stretch, Style, Weight, ID};
use cosmic_text::{Attrs, AttrsOwned, Buffer, FontSystem, Metrics, Shaping, Wrap};
use unicode_segmentation::UnicodeSegmentation;

pub(crate) const ICON_FONT_FAMILY: &str = "tgui-icons";
const ICON_FONT_BYTES: &[u8] = include_bytes!("../assets/iconfont.ttf");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FontWeight {
    Thin,
    Light,
    Regular,
    Medium,
    SemiBold,
    Bold,
    ExtraBold,
}

impl FontWeight {
    pub const NORMAL: Self = Self::Regular;
    pub const SEMIBOLD: Self = Self::SemiBold;

    pub const fn to_raw(self) -> u16 {
        match self {
            Self::Thin => 100,
            Self::Light => 300,
            Self::Regular => 400,
            Self::Medium => 500,
            Self::SemiBold => 600,
            Self::Bold => 700,
            Self::ExtraBold => 800,
        }
    }
}

impl Default for FontWeight {
    fn default() -> Self {
        Self::Regular
    }
}

#[derive(Debug, Clone)]
pub(crate) struct FontCatalog {
    named_fonts: Vec<NamedFont>,
    default_font: Option<String>,
}

impl Default for FontCatalog {
    fn default() -> Self {
        let mut catalog = Self {
            named_fonts: Vec::new(),
            default_font: None,
        };
        catalog.register_font(ICON_FONT_FAMILY, ICON_FONT_BYTES);
        catalog
    }
}

impl FontCatalog {
    pub(crate) fn register_font(&mut self, name: impl Into<String>, bytes: &'static [u8]) {
        self.named_fonts.push(NamedFont {
            name: name.into(),
            source: FontSource::Binary(bytes),
        });
    }

    pub(crate) fn register_font_file(&mut self, name: impl Into<String>, path: impl Into<PathBuf>) {
        self.named_fonts.push(NamedFont {
            name: name.into(),
            source: FontSource::File(path.into()),
        });
    }

    pub(crate) fn set_default_font(&mut self, name: impl Into<String>) {
        self.default_font = Some(name.into());
    }

    pub(crate) fn configure_font_system(
        &self,
        font_system: &mut FontSystem,
    ) -> Vec<(String, String)> {
        #[cfg(any(target_os = "android", target_env = "ohos"))]
        load_mobile_system_fonts(font_system.db_mut());

        let mut aliases = Vec::with_capacity(self.named_fonts.len());
        for font in &self.named_fonts {
            let ids = font_system.db_mut().load_font_source(match &font.source {
                FontSource::Binary(bytes) => {
                    cosmic_text::fontdb::Source::Binary(Arc::new(bytes.to_vec()))
                }
                FontSource::File(path) => cosmic_text::fontdb::Source::File(path.clone().into()),
            });
            let actual_family = ids
                .iter()
                .find_map(|id| face_family_name(font_system.db(), *id))
                .unwrap_or_else(|| font.name.clone());
            aliases.push((font.name.clone(), actual_family));
        }

        aliases
    }
}

#[derive(Debug, Clone)]
struct NamedFont {
    name: String,
    source: FontSource,
}

#[derive(Debug, Clone)]
enum FontSource {
    Binary(&'static [u8]),
    File(PathBuf),
}

#[derive(Debug, Clone)]
pub struct ResolvedText {
    pub primary_font: String,
}

#[derive(Debug, Clone)]
pub(crate) struct TextLayoutInfo {
    pub width: f32,
    pub height: f32,
    lines: Vec<TextLineLayoutInfo>,
}

#[derive(Debug, Clone, Copy)]
struct TextBoundary {
    index: usize,
    x: f32,
}

#[derive(Debug, Clone)]
struct TextLineLayoutInfo {
    start_index: usize,
    end_index: usize,
    top: f32,
    height: f32,
    width: f32,
    boundaries: Vec<TextBoundary>,
}

impl TextLayoutInfo {
    pub(crate) fn x_for_index(&self, index: usize) -> f32 {
        self.line_for_index(index).x_for_index(index)
    }

    pub(crate) fn top_for_index(&self, index: usize) -> f32 {
        self.line_for_index(index).top
    }

    pub(crate) fn line_height_for_index(&self, index: usize) -> f32 {
        self.line_for_index(index).height
    }

    pub(crate) fn index_for_x(&self, x: f32) -> usize {
        self.lines
            .first()
            .map(|line| line.index_for_x(x))
            .unwrap_or(0)
    }

    pub(crate) fn index_for_point(&self, x: f32, y: f32) -> usize {
        self.line_for_y(y).index_for_x(x)
    }

    pub(crate) fn line_index_for_index(&self, index: usize) -> usize {
        self.find_line_index_for_index(index)
    }

    pub(crate) fn line_count(&self) -> usize {
        self.lines.len()
    }

    pub(crate) fn line_start(&self, line_index: usize) -> usize {
        self.lines
            .get(line_index)
            .or_else(|| self.lines.last())
            .map(|line| line.start_index)
            .unwrap_or(0)
    }

    pub(crate) fn line_end(&self, line_index: usize) -> usize {
        self.lines
            .get(line_index)
            .or_else(|| self.lines.last())
            .map(|line| line.end_index)
            .unwrap_or(0)
    }

    pub(crate) fn line_top(&self, line_index: usize) -> f32 {
        self.lines
            .get(line_index)
            .or_else(|| self.lines.last())
            .map(|line| line.top)
            .unwrap_or(0.0)
    }

    pub(crate) fn line_height(&self, line_index: usize) -> f32 {
        self.lines
            .get(line_index)
            .or_else(|| self.lines.last())
            .map(|line| line.height)
            .unwrap_or(0.0)
    }

    pub(crate) fn line_width(&self, line_index: usize) -> f32 {
        self.lines
            .get(line_index)
            .or_else(|| self.lines.last())
            .map(|line| line.width)
            .unwrap_or(0.0)
    }

    pub(crate) fn line_range_for_vertical_span(
        &self,
        top: f32,
        bottom: f32,
    ) -> std::ops::Range<usize> {
        if self.lines.is_empty() || bottom <= top {
            return 0..0;
        }

        let start = self.first_line_with_bottom_after(top);
        if start >= self.lines.len() {
            return self.lines.len()..self.lines.len();
        }

        let mut left = start;
        let mut right = self.lines.len();
        while left < right {
            let mid = (left + right) / 2;
            if self.lines[mid].top < bottom {
                left = mid + 1;
            } else {
                right = mid;
            }
        }

        start..left
    }

    fn find_line_index_for_index(&self, index: usize) -> usize {
        if self.lines.is_empty() {
            return 0;
        }

        for line_index in 0..self.lines.len() {
            let next_start = self
                .lines
                .get(line_index + 1)
                .map(|next| next.start_index)
                .unwrap_or(usize::MAX);
            if index < next_start {
                return line_index;
            }
        }

        self.lines.len() - 1
    }

    fn line_for_index(&self, index: usize) -> &TextLineLayoutInfo {
        let line_index = self.find_line_index_for_index(index);
        self.lines
            .get(line_index)
            .or_else(|| self.lines.first())
            .expect("text layout should always contain at least one line")
    }

    fn line_for_y(&self, y: f32) -> &TextLineLayoutInfo {
        if self.lines.is_empty() {
            panic!("text layout should always contain at least one line");
        }

        let local_y = y.max(0.0);
        self.lines
            .iter()
            .find(|line| local_y < line.top + line.height)
            .or_else(|| self.lines.last())
            .expect("text layout should always contain at least one line")
    }

    fn first_line_with_bottom_after(&self, y: f32) -> usize {
        let mut left = 0usize;
        let mut right = self.lines.len();
        while left < right {
            let mid = (left + right) / 2;
            if self.lines[mid].top + self.lines[mid].height <= y {
                left = mid + 1;
            } else {
                right = mid;
            }
        }
        left
    }
}

impl TextLineLayoutInfo {
    fn x_for_index(&self, index: usize) -> f32 {
        if self.boundaries.is_empty() {
            return 0.0;
        }

        let local_index = index.saturating_sub(self.start_index);
        let mut x = 0.0;
        for boundary in &self.boundaries {
            if boundary.index > local_index {
                break;
            }
            x = boundary.x;
        }
        x
    }

    fn index_for_x(&self, x: f32) -> usize {
        if self.boundaries.len() <= 1 {
            return self.start_index;
        }

        let local_x = x.max(0.0);
        for pair in self.boundaries.windows(2) {
            let start = pair[0];
            let end = pair[1];
            if local_x <= (start.x + end.x) * 0.5 {
                return self.start_index + start.index;
            }
        }

        self.boundaries
            .last()
            .map(|boundary| self.start_index + boundary.index)
            .unwrap_or(self.end_index)
    }
}

#[derive(Debug, Clone)]
pub struct TextFontRequest<'a> {
    pub preferred_font: Option<&'a str>,
    pub weight: FontWeight,
}

pub(crate) struct FontManager {
    font_system: RefCell<FontSystem>,
    aliases: Vec<(String, String)>,
    default_font: Option<String>,
    measure_cache: RefCell<HashMap<TextMeasureKey, (f32, f32)>>,
    layout_cache: RefCell<HashMap<TextLayoutKey, TextLayoutInfo>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct TextMeasureKey {
    text: String,
    preferred_font: Option<String>,
    weight: FontWeight,
    font_size_bits: u32,
    line_height_bits: u32,
    letter_spacing_bits: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct TextLayoutKey {
    text: String,
    preferred_font: Option<String>,
    weight: FontWeight,
    font_size_bits: u32,
    line_height_bits: u32,
    letter_spacing_bits: u32,
    wrap_width_bits: Option<u32>,
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

    pub(crate) fn measure_text(
        &self,
        text: &str,
        request: TextFontRequest<'_>,
        font_size: f32,
        line_height: f32,
        letter_spacing: f32,
    ) -> (f32, f32) {
        self.measure_text_raw(text, request, font_size, line_height, letter_spacing)
    }

    pub(crate) fn measure_text_raw(
        &self,
        text: &str,
        request: TextFontRequest<'_>,
        font_size: f32,
        line_height: f32,
        letter_spacing: f32,
    ) -> (f32, f32) {
        if text.is_empty() {
            return (0.0, line_height.ceil());
        }

        let cache_key = TextMeasureKey {
            text: text.to_string(),
            preferred_font: request.preferred_font.map(ToString::to_string),
            weight: request.weight,
            font_size_bits: font_size.to_bits(),
            line_height_bits: line_height.to_bits(),
            letter_spacing_bits: letter_spacing.to_bits(),
        };
        if let Some(cached) = self.measure_cache.borrow().get(&cache_key) {
            return *cached;
        }

        let layout =
            self.measure_text_layout(text, request, font_size, line_height, letter_spacing);
        let measured = (
            layout.width.max(0.0).ceil(),
            layout.height.max(line_height).ceil(),
        );
        let mut cache = self.measure_cache.borrow_mut();
        if cache.len() > 4096 {
            cache.clear();
        }
        cache.insert(cache_key, measured);
        measured
    }

    pub(crate) fn measure_text_layout(
        &self,
        text: &str,
        request: TextFontRequest<'_>,
        font_size: f32,
        line_height: f32,
        letter_spacing: f32,
    ) -> TextLayoutInfo {
        self.measure_text_layout_cached(text, request, font_size, line_height, letter_spacing, None)
    }

    pub(crate) fn measure_text_layout_wrapped(
        &self,
        text: &str,
        request: TextFontRequest<'_>,
        font_size: f32,
        line_height: f32,
        letter_spacing: f32,
        max_width: f32,
    ) -> TextLayoutInfo {
        if text.is_empty() {
            return TextLayoutInfo {
                width: 0.0,
                height: line_height,
                lines: vec![TextLineLayoutInfo {
                    start_index: 0,
                    end_index: 0,
                    top: 0.0,
                    height: line_height,
                    width: 0.0,
                    boundaries: vec![TextBoundary { index: 0, x: 0.0 }],
                }],
            };
        }

        let wrap_width = if max_width.is_finite() && max_width > 0.0 {
            Some(max_width)
        } else {
            None
        };
        if wrap_width.is_none() && !text.contains('\n') {
            return self.measure_text_layout_cached(
                text,
                request,
                font_size,
                line_height,
                letter_spacing,
                None,
            );
        }

        self.measure_text_layout_cached(
            text,
            request,
            font_size,
            line_height,
            letter_spacing,
            wrap_width,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn update_layout_after_edit(
        &self,
        previous: &mut TextLayoutInfo,
        old_text: &str,
        new_text: &str,
        request: TextFontRequest<'_>,
        font_size: f32,
        line_height: f32,
        letter_spacing: f32,
        wrap_width: Option<f32>,
        replacement: (usize, usize, usize, usize),
    ) -> bool {
        let (old_start, old_end, new_start, new_end) = replacement;
        let old_segment_start = logical_line_start(old_text, old_start);
        let old_segment_end = logical_line_end_exclusive(old_text, old_end);
        let new_segment_start = logical_line_start(new_text, new_start);
        let new_segment_end = logical_line_end_exclusive(new_text, new_end);
        let new_segment_measure_end =
            logical_line_measure_end_exclusive(new_text, new_segment_start, new_segment_end);

        if old_segment_start != new_segment_start {
            return false;
        }

        let start_line = previous.line_index_for_index(old_segment_start);
        let end_line_exclusive = if old_segment_end >= old_text.len() {
            previous.line_count()
        } else {
            previous.line_index_for_index(old_segment_end)
        };

        if start_line > end_line_exclusive || end_line_exclusive > previous.lines.len() {
            return false;
        }

        let base_top = previous.line_top(start_line);
        let removed_height = if end_line_exclusive < previous.line_count() {
            previous.line_top(end_line_exclusive) - base_top
        } else {
            previous.height - base_top
        };
        let next_layout = if let Some(wrap_width) = wrap_width {
            self.measure_text_layout_wrapped(
                &new_text[new_segment_start..new_segment_measure_end],
                request,
                font_size,
                line_height,
                letter_spacing,
                wrap_width,
            )
        } else {
            self.measure_text_layout(
                &new_text[new_segment_start..new_segment_measure_end],
                request,
                font_size,
                line_height,
                letter_spacing,
            )
        };
        let height_delta = next_layout.height - removed_height;
        let byte_delta = new_text.len() as isize - old_text.len() as isize;
        let inserted_lines: Vec<_> = next_layout
            .lines
            .into_iter()
            .map(|line| shift_line_layout(line, new_segment_start, base_top))
            .collect();
        let inserted_len = inserted_lines.len();
        previous
            .lines
            .splice(start_line..end_line_exclusive, inserted_lines);
        for line in previous.lines.iter_mut().skip(start_line + inserted_len) {
            shift_line_layout_tail_in_place(line, byte_delta, height_delta);
        }
        previous.width = previous
            .lines
            .iter()
            .map(|line| line.width)
            .fold(0.0, f32::max);
        previous.height = (previous.height + height_delta).max(line_height);
        true
    }

    fn measure_text_layout_cached(
        &self,
        text: &str,
        request: TextFontRequest<'_>,
        font_size: f32,
        line_height: f32,
        letter_spacing: f32,
        wrap_width: Option<f32>,
    ) -> TextLayoutInfo {
        let cache_key = TextLayoutKey {
            text: text.to_string(),
            preferred_font: request.preferred_font.map(ToString::to_string),
            weight: request.weight,
            font_size_bits: font_size.to_bits(),
            line_height_bits: line_height.to_bits(),
            letter_spacing_bits: letter_spacing.to_bits(),
            wrap_width_bits: wrap_width.map(f32::to_bits),
        };
        if let Some(cached) = self.layout_cache.borrow().get(&cache_key) {
            return cached.clone();
        }

        let layout = self.measure_text_layout_uncached(
            text,
            request,
            font_size,
            line_height,
            letter_spacing,
            wrap_width,
        );

        let mut cache = self.layout_cache.borrow_mut();
        if cache.len() > 256 {
            cache.clear();
        }
        cache.insert(cache_key, layout.clone());
        layout
    }

    fn measure_text_layout_uncached(
        &self,
        text: &str,
        request: TextFontRequest<'_>,
        font_size: f32,
        line_height: f32,
        letter_spacing: f32,
        wrap_width: Option<f32>,
    ) -> TextLayoutInfo {
        if text.is_empty() {
            return TextLayoutInfo {
                width: 0.0,
                height: line_height,
                lines: vec![TextLineLayoutInfo {
                    start_index: 0,
                    end_index: 0,
                    top: 0.0,
                    height: line_height,
                    width: 0.0,
                    boundaries: vec![TextBoundary { index: 0, x: 0.0 }],
                }],
            };
        }

        self.with_text_buffer(
            text,
            request,
            font_size,
            line_height,
            letter_spacing,
            wrap_width,
            |buffer| build_layout_info_from_buffer(buffer, text, line_height),
        )
    }

    fn with_text_buffer<T>(
        &self,
        text: &str,
        request: TextFontRequest<'_>,
        font_size: f32,
        line_height: f32,
        letter_spacing: f32,
        wrap_width: Option<f32>,
        compute: impl FnOnce(&Buffer) -> T,
    ) -> T {
        self.with_font_system(|font_system| {
            let mut buffer = Buffer::new(font_system, Metrics::new(font_size, line_height));
            self.configure_buffer(
                font_system,
                &mut buffer,
                text,
                request,
                font_size,
                line_height,
                letter_spacing,
                wrap_width,
                None,
                if wrap_width.is_some() {
                    Wrap::WordOrGlyph
                } else {
                    Wrap::None
                },
            );
            compute(&buffer)
        })
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

        let families = [Family::Name(name)];
        let query = Query {
            families: &families,
            weight: Weight(weight.to_raw()),
            stretch: Stretch::Normal,
            style: Style::Normal,
        };

        database
            .query(&query)
            .and_then(|id| face_family_name(database, id))
            .or_else(|| {
                database.faces().find_map(|face| {
                    face.families
                        .iter()
                        .find(|(family, _)| family.eq_ignore_ascii_case(name))
                        .map(|(family, _)| family.clone())
                })
            })
    }

    fn system_default_family_in_database(
        &self,
        database: &cosmic_text::fontdb::Database,
        weight: FontWeight,
    ) -> Option<String> {
        let families = [Family::SansSerif];
        let query = Query {
            families: &families,
            weight: Weight(weight.to_raw()),
            stretch: Stretch::Normal,
            style: Style::Normal,
        };

        database
            .query(&query)
            .and_then(|id| face_family_name(database, id))
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
}

fn push_boundary(boundaries: &mut Vec<TextBoundary>, index: usize, x: f32) {
    let x = x.max(0.0);
    if let Some(last) = boundaries.last_mut() {
        if last.index == index {
            // Keep the furthest-forward edge for duplicate boundaries so kerning
            // or glyph overlap doesn't pull the caret back into the previous glyph.
            last.x = last.x.max(x);
            return;
        }
    }

    boundaries.push(TextBoundary { index, x });
}

fn measured_glyph_line_height(
    buffer: &mut Buffer,
    font_system: &mut FontSystem,
    fallback_line_height: f32,
) -> f32 {
    let mut max_height = fallback_line_height;
    let mut line_index = 0usize;
    while let Some(layout_lines) = buffer.line_layout(font_system, line_index) {
        for layout_line in layout_lines {
            let glyph_height = layout_line.max_ascent + layout_line.max_descent;
            let requested_height = layout_line.line_height_opt.unwrap_or(fallback_line_height);
            max_height = max_height.max(glyph_height.max(requested_height));
        }
        line_index += 1;
    }
    max_height
}

pub(crate) fn build_layout_info_from_buffer(
    buffer: &Buffer,
    text: &str,
    line_height: f32,
) -> TextLayoutInfo {
    let line_offsets = logical_line_offsets(text);
    let mut width = 0.0f32;
    let mut height = 0.0f32;
    let mut lines = Vec::new();

    for run in buffer.layout_runs() {
        let line_offset = line_offsets.get(run.line_i).copied().unwrap_or(0);
        let start_index = line_offset
            + run
                .glyphs
                .iter()
                .map(|glyph| glyph.start)
                .min()
                .unwrap_or(0);
        let end_index = line_offset
            + run
                .glyphs
                .iter()
                .map(|glyph| glyph.end)
                .max()
                .unwrap_or(run.text.len());
        let start_relative = start_index.saturating_sub(line_offset);
        let mut boundaries = vec![TextBoundary { index: 0, x: 0.0 }];

        for glyph in run.glyphs {
            push_boundary(
                &mut boundaries,
                glyph.start.saturating_sub(start_relative),
                glyph.x.max(0.0),
            );

            let cluster = &run.text[glyph.start..glyph.end];
            let grapheme_count = cluster.graphemes(true).count().max(1);
            let grapheme_width = glyph.w / grapheme_count as f32;
            let mut grapheme_x = glyph.x;

            for (offset, grapheme) in cluster.grapheme_indices(true) {
                grapheme_x += grapheme_width;
                push_boundary(
                    &mut boundaries,
                    glyph.start + offset + grapheme.len() - start_relative,
                    grapheme_x.max(0.0),
                );
            }
        }

        push_boundary(
            &mut boundaries,
            end_index.saturating_sub(start_index),
            run.line_w.max(0.0),
        );

        width = width.max(run.line_w.max(0.0));
        height = height.max(run.line_top + run.line_height);
        lines.push(TextLineLayoutInfo {
            start_index,
            end_index,
            top: run.line_top,
            height: run.line_height.max(line_height),
            width: run.line_w.max(0.0),
            boundaries,
        });
    }

    if lines.is_empty() {
        lines.push(TextLineLayoutInfo {
            start_index: 0,
            end_index: 0,
            top: 0.0,
            height: line_height,
            width: 0.0,
            boundaries: vec![TextBoundary { index: 0, x: 0.0 }],
        });
        height = line_height;
    }

    TextLayoutInfo {
        width,
        height: height.max(line_height),
        lines,
    }
}

fn logical_line_offsets(text: &str) -> Vec<usize> {
    let mut offsets = vec![0];
    for (index, ch) in text.char_indices() {
        if ch == '\n' {
            offsets.push(index + ch.len_utf8());
        }
    }
    offsets
}

fn logical_line_start(text: &str, index: usize) -> usize {
    let target = index.min(text.len());
    text[..target].rfind('\n').map(|pos| pos + 1).unwrap_or(0)
}

fn logical_line_end_exclusive(text: &str, index: usize) -> usize {
    let target = index.min(text.len());
    text[target..]
        .find('\n')
        .map(|relative| target + relative + 1)
        .unwrap_or(text.len())
}

fn logical_line_measure_end_exclusive(text: &str, start: usize, end: usize) -> usize {
    if start < end && end < text.len() && text.as_bytes()[end - 1] == b'\n' {
        end - 1
    } else {
        end
    }
}

fn shift_line_layout(
    mut line: TextLineLayoutInfo,
    byte_offset: usize,
    top_offset: f32,
) -> TextLineLayoutInfo {
    line.start_index += byte_offset;
    line.end_index += byte_offset;
    line.top += top_offset;
    line
}

fn shift_line_layout_tail_in_place(
    line: &mut TextLineLayoutInfo,
    byte_delta: isize,
    top_delta: f32,
) {
    line.start_index = line.start_index.saturating_add_signed(byte_delta);
    line.end_index = line.end_index.saturating_add_signed(byte_delta);
    line.top += top_delta;
}

#[cfg(test)]
mod tests {
    use super::{
        push_boundary, FontCatalog, FontManager, FontWeight, TextBoundary, TextFontRequest,
    };
    use cosmic_text::{Attrs, Buffer, Family, Metrics, Shaping, Weight, Wrap};
    use unicode_segmentation::UnicodeSegmentation;

    #[test]
    fn duplicate_boundary_keeps_furthest_forward_position() {
        let mut boundaries = vec![TextBoundary { index: 0, x: 0.0 }];
        push_boundary(&mut boundaries, 1, 12.0);
        push_boundary(&mut boundaries, 1, 9.0);

        assert_eq!(boundaries.len(), 2);
        assert_eq!(boundaries[1].index, 1);
        assert_eq!(boundaries[1].x, 12.0);
    }

    #[test]
    fn mixed_text_layout_round_trips_cursor_boundaries() {
        let manager = FontManager::new(&FontCatalog::default());
        let text = "A中-文!B，c";
        let font_size = 16.0;
        let line_height = 24.0;
        let layout = manager.measure_text_layout(
            text,
            TextFontRequest {
                preferred_font: None,
                weight: FontWeight::NORMAL,
            },
            font_size,
            line_height,
            0.0,
        );

        let mut indices = vec![0];
        for (offset, grapheme) in text.grapheme_indices(true) {
            indices.push(offset + grapheme.len());
        }

        for pair in indices.windows(2) {
            let start = pair[0];
            let end = pair[1];
            let start_x = layout.x_for_index(start);
            let end_x = layout.x_for_index(end);
            assert!(end_x >= start_x, "cursor positions should be monotonic");

            let delta = end_x - start_x;
            if delta > 0.0 {
                assert_eq!(layout.index_for_x(start_x + delta * 0.25), start);
                assert_eq!(layout.index_for_x(start_x + delta * 0.75), end);
            }
        }
    }

    #[test]
    fn wrapped_text_layout_is_cached_between_calls() {
        let manager = FontManager::new(&FontCatalog::default());
        let text = "wrap this long line into multiple segments for caching\nand keep doing it";
        let request = TextFontRequest {
            preferred_font: None,
            weight: FontWeight::NORMAL,
        };

        let first =
            manager.measure_text_layout_wrapped(text, request.clone(), 16.0, 24.0, 0.0, 160.0);
        let cache_size_after_first = manager.layout_cache.borrow().len();
        let second = manager.measure_text_layout_wrapped(text, request, 16.0, 24.0, 0.0, 160.0);

        assert_eq!(cache_size_after_first, 1);
        assert_eq!(manager.layout_cache.borrow().len(), 1);
        assert_eq!(first.width, second.width);
        assert_eq!(first.height, second.height);
        assert_eq!(first.line_count(), second.line_count());
    }

    #[test]
    fn wrapped_text_layout_matches_cosmic_hit_positions() {
        let manager = FontManager::new(&FontCatalog::default());
        let text = "supercalifragilisticexpialidocious wrapped text\nwith another long visual line";
        let request = TextFontRequest {
            preferred_font: None,
            weight: FontWeight::NORMAL,
        };
        let font_size = 16.0;
        let line_height = 24.0;
        let wrap_width = 140.0;
        let layout = manager.measure_text_layout_wrapped(
            text,
            request.clone(),
            font_size,
            line_height,
            0.0,
            wrap_width,
        );

        let resolved = manager.resolve_text(text, request.clone());
        let mut font_system = manager.font_system.borrow_mut();
        let mut buffer = Buffer::new(&mut font_system, Metrics::new(font_size, line_height));
        buffer.set_size(Some(wrap_width), None);
        buffer.set_wrap(Wrap::WordOrGlyph);
        buffer.set_text(
            text,
            &Attrs::new()
                .family(Family::Name(&resolved.primary_font))
                .weight(Weight(request.weight.to_raw())),
            Shaping::Advanced,
            None,
        );
        buffer.shape_until_scroll(&mut font_system, false);

        let line_offsets = super::logical_line_offsets(text);
        for run in buffer.layout_runs() {
            let sample_y = run.line_top + (run.line_height * 0.5);
            for sample_x in [
                0.0,
                run.line_w * 0.25,
                run.line_w * 0.75,
                (run.line_w - 0.5).max(0.0),
            ] {
                let expected = buffer
                    .hit(sample_x, sample_y)
                    .map(|cursor| {
                        line_offsets.get(cursor.line).copied().unwrap_or(0) + cursor.index
                    })
                    .unwrap_or(0);
                let actual = layout.index_for_point(sample_x, sample_y);
                assert_eq!(actual, expected, "x={sample_x}, y={sample_y}");
            }
        }
    }

    #[test]
    fn incremental_layout_edit_on_nonterminal_line_matches_full_layout() {
        let manager = FontManager::new(&FontCatalog::default());
        let old_text = "hello\nworld";
        let new_text = "xhello\nworld";
        let request = TextFontRequest {
            preferred_font: None,
            weight: FontWeight::NORMAL,
        };
        let mut incremental =
            manager.measure_text_layout(old_text, request.clone(), 16.0, 24.0, 0.0);

        assert!(manager.update_layout_after_edit(
            &mut incremental,
            old_text,
            new_text,
            request.clone(),
            16.0,
            24.0,
            0.0,
            None,
            (0, 0, 0, 1),
        ));

        let fresh = manager.measure_text_layout(new_text, request, 16.0, 24.0, 0.0);
        assert_eq!(incremental.line_count(), fresh.line_count());
        assert_eq!(incremental.height, fresh.height);
        for line_index in 0..fresh.line_count() {
            assert_eq!(
                incremental.line_start(line_index),
                fresh.line_start(line_index)
            );
            assert_eq!(incremental.line_end(line_index), fresh.line_end(line_index));
            assert_eq!(incremental.line_top(line_index), fresh.line_top(line_index));
        }
    }

    #[test]
    fn line_range_for_vertical_span_tracks_visible_lines() {
        let manager = FontManager::new(&FontCatalog::default());
        let layout = manager.measure_text_layout(
            "line 0\nline 1\nline 2",
            TextFontRequest {
                preferred_font: None,
                weight: FontWeight::NORMAL,
            },
            16.0,
            24.0,
            0.0,
        );

        assert_eq!(layout.line_range_for_vertical_span(0.0, 1.0), 0..1);
        assert_eq!(layout.line_range_for_vertical_span(10.0, 30.0), 0..2);
        assert_eq!(layout.line_range_for_vertical_span(24.0, 48.0), 1..2);
        assert_eq!(layout.line_range_for_vertical_span(48.0, 72.0), 2..3);
        assert_eq!(layout.line_range_for_vertical_span(72.0, 96.0), 3..3);
    }

    #[test]
    fn chinese_text_resolves_to_single_primary_font() {
        let manager = FontManager::new(&FontCatalog::default());
        let resolved = manager.resolve_text(
            "中文测试ABC",
            TextFontRequest {
                preferred_font: None,
                weight: FontWeight::NORMAL,
            },
        );

        assert!(!resolved.primary_font.trim().is_empty());
    }

    #[test]
    fn mixed_cjk_text_keeps_same_primary_font_as_latin_text() {
        let manager = FontManager::new(&FontCatalog::default());
        let latin = manager.resolve_text(
            "abc123",
            TextFontRequest {
                preferred_font: None,
                weight: FontWeight::NORMAL,
            },
        );
        let mixed = manager.resolve_text(
            "abc123中文",
            TextFontRequest {
                preferred_font: None,
                weight: FontWeight::NORMAL,
            },
        );

        assert_eq!(latin.primary_font, mixed.primary_font);
    }
}

#[cfg(any(target_os = "android", target_env = "ohos"))]
fn load_mobile_system_fonts(database: &mut cosmic_text::fontdb::Database) {
    for path in mobile_font_dirs() {
        let path = std::path::Path::new(path);
        if path.exists() {
            database.load_fonts_dir(path);
        }
    }

    let sans_family = first_matching_family(database, mobile_sans_candidates())
        .or_else(|| first_loaded_family(database));

    let serif_family =
        first_matching_family(database, mobile_serif_candidates()).or_else(|| sans_family.clone());

    let monospace_family = first_matching_family(database, mobile_monospace_candidates())
        .or_else(|| sans_family.clone());

    if let Some(family) = sans_family {
        database.set_sans_serif_family(family.clone());
        database.set_cursive_family(family.clone());
        database.set_fantasy_family(family);
    }
    if let Some(family) = serif_family {
        database.set_serif_family(family);
    }
    if let Some(family) = monospace_family {
        database.set_monospace_family(family);
    }
}

fn contains_cjk(text: &str) -> bool {
    text.chars().any(is_cjk_character)
}

fn contains_non_cjk_alphanumeric(text: &str) -> bool {
    text.chars()
        .any(|ch| !is_cjk_character(ch) && ch.is_alphanumeric())
}

fn is_cjk_character(ch: char) -> bool {
    matches!(
        ch as u32,
        0x2E80..=0x2EFF
            | 0x2F00..=0x2FDF
            | 0x3000..=0x303F
            | 0x31C0..=0x31EF
            | 0x3400..=0x4DBF
            | 0x4E00..=0x9FFF
            | 0xF900..=0xFAFF
            | 0x20000..=0x2A6DF
            | 0x2A700..=0x2B73F
            | 0x2B740..=0x2B81F
            | 0x2B820..=0x2CEAF
            | 0x2CEB0..=0x2EBEF
            | 0x2F800..=0x2FA1F
    )
}

#[cfg(target_os = "android")]
fn mobile_font_dirs() -> &'static [&'static str] {
    &[
        "/system/fonts",
        "/system_ext/fonts",
        "/product/fonts",
        "/vendor/fonts",
    ]
}

#[cfg(target_env = "ohos")]
fn mobile_font_dirs() -> &'static [&'static str] {
    &[
        "/system/fonts",
        "/system/etc/fonts",
        "/system/fonts/visibility",
        "/data/service/el1/public/font",
    ]
}

#[cfg(target_os = "android")]
fn mobile_sans_candidates() -> &'static [&'static str] {
    &[
        "Roboto",
        "Roboto Static",
        "Roboto Flex",
        "Droid Sans",
        "Noto Sans CJK SC",
        "Noto Sans CJK TC",
        "Noto Sans CJK JP",
        "Noto Sans CJK KR",
        "Noto Sans",
    ]
}

#[cfg(target_env = "ohos")]
fn mobile_sans_candidates() -> &'static [&'static str] {
    &[
        "HarmonyOS Sans SC",
        "HarmonyOS Sans",
        "Noto Sans CJK SC",
        "Noto Sans SC",
        "Noto Sans",
    ]
}

#[cfg(target_os = "android")]
fn mobile_serif_candidates() -> &'static [&'static str] {
    &[
        "Noto Serif",
        "Noto Serif CJK SC",
        "Noto Serif CJK TC",
        "Noto Serif CJK JP",
        "Noto Serif CJK KR",
    ]
}

#[cfg(target_env = "ohos")]
fn mobile_serif_candidates() -> &'static [&'static str] {
    &[
        "Noto Serif CJK SC",
        "Noto Serif SC",
        "Noto Serif",
        "HarmonyOS Sans SC",
    ]
}

#[cfg(target_os = "android")]
fn mobile_monospace_candidates() -> &'static [&'static str] {
    &[
        "Droid Sans Mono",
        "Cutive Mono",
        "Roboto Mono",
        "Noto Sans Mono",
    ]
}

#[cfg(target_env = "ohos")]
fn mobile_monospace_candidates() -> &'static [&'static str] {
    &[
        "HarmonyOS Sans Mono",
        "Roboto Mono",
        "Noto Sans Mono",
        "HarmonyOS Sans SC",
    ]
}

fn first_matching_family(
    database: &cosmic_text::fontdb::Database,
    candidates: &[&str],
) -> Option<String> {
    candidates.iter().find_map(|candidate| {
        database.faces().find_map(|face| {
            face.families
                .iter()
                .find(|(family, _)| family.eq_ignore_ascii_case(candidate))
                .map(|(family, _)| family.clone())
        })
    })
}

#[cfg(any(target_os = "android", target_env = "ohos"))]
fn first_loaded_family(database: &cosmic_text::fontdb::Database) -> Option<String> {
    database
        .faces()
        .find_map(|face| face.families.first().map(|(family, _)| family.clone()))
}

#[cfg(target_os = "windows")]
fn desktop_cjk_sans_candidates() -> Option<&'static [&'static str]> {
    Some(&[
        "Noto Sans SC",
        "DengXian",
        "Microsoft YaHei",
        "Microsoft YaHei UI",
        "Microsoft JhengHei UI",
        "Microsoft JhengHei",
        "SimHei",
        "Yu Gothic UI",
        "Yu Gothic",
        "Malgun Gothic",
        "SimSun",
    ])
}

#[cfg(target_os = "macos")]
fn desktop_cjk_sans_candidates() -> Option<&'static [&'static str]> {
    Some(&[
        "PingFang SC",
        "Hiragino Sans GB",
        "Heiti SC",
        "Apple SD Gothic Neo",
    ])
}

#[cfg(all(target_os = "linux", not(target_env = "ohos")))]
fn desktop_cjk_sans_candidates() -> Option<&'static [&'static str]> {
    Some(&[
        "Noto Sans CJK SC",
        "Noto Sans SC",
        "Source Han Sans SC",
        "WenQuanYi Micro Hei",
        "Droid Sans Fallback",
    ])
}

#[cfg(any(target_os = "android", target_env = "ohos"))]
fn desktop_cjk_sans_candidates() -> Option<&'static [&'static str]> {
    None
}

fn face_family_name(database: &cosmic_text::fontdb::Database, id: ID) -> Option<String> {
    database
        .face(id)
        .and_then(|face| face.families.first().map(|(family, _)| family.clone()))
}
