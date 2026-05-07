use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use cosmic_text::fontdb::{Family, Query, Stretch, Style, Weight, ID};
use cosmic_text::{Attrs, Buffer, FontSystem, Metrics, Shaping, Wrap};
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
    boundaries: Vec<TextBoundary>,
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

    pub(crate) fn index_for_x(&self, x: f32) -> usize {
        self.lines
            .first()
            .map(|line| line.index_for_x(x))
            .unwrap_or(0)
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
}

impl TextLineLayoutInfo {
    fn x_for_index(&self, index: usize) -> f32 {
        if self.boundaries.is_empty() {
            return 0.0;
        }

        let mut x = 0.0;
        for boundary in &self.boundaries {
            if boundary.index > index {
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
                return start.index;
            }
        }

        self.boundaries
            .last()
            .map(|boundary| boundary.index)
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

impl FontManager {
    pub(crate) fn new(catalog: &FontCatalog) -> Self {
        let mut font_system = FontSystem::new();
        let aliases = catalog.configure_font_system(&mut font_system);

        Self {
            font_system: RefCell::new(font_system),
            aliases,
            default_font: catalog.default_font.clone(),
            measure_cache: RefCell::new(HashMap::new()),
        }
    }

    pub(crate) fn resolve_text(&self, text: &str, request: TextFontRequest<'_>) -> ResolvedText {
        let preferred = request
            .preferred_font
            .and_then(|name| self.resolve_family_name(name, request.weight));
        let script_aware_default = self.script_aware_default_family(text, request.weight);

        ResolvedText {
            primary_font: preferred
                .or(script_aware_default)
                .or_else(|| {
                    self.default_font
                        .as_deref()
                        .and_then(|name| self.resolve_family_name(name, request.weight))
                })
                .or_else(|| self.system_default_family(request.weight))
                .unwrap_or_else(|| "sans-serif".to_string()),
        }
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
        self.measure_text_layout_unwrapped(text, request, font_size, line_height, letter_spacing)
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
                boundaries: vec![TextBoundary { index: 0, x: 0.0 }],
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
            return self.measure_text_layout_unwrapped(
                text,
                request,
                font_size,
                line_height,
                letter_spacing,
            );
        }

        let graphemes = text
            .grapheme_indices(true)
            .map(|(start, grapheme)| (start, start + grapheme.len(), grapheme))
            .collect::<Vec<_>>();
        let mut lines = Vec::new();
        let mut line_start = 0usize;
        let mut current_top = 0.0f32;

        while line_start < text.len() {
            let mut current_end = line_start;
            let mut last_break = None;
            let mut wrapped = false;
            let mut explicit_newline_end = None;

            for (start, end, grapheme) in graphemes.iter().copied() {
                if start < line_start {
                    continue;
                }
                if grapheme.contains('\n') {
                    explicit_newline_end = Some((start, end));
                    break;
                }

                let candidate_end = end;
                if let Some(limit) = wrap_width {
                    let candidate = self.measure_text_layout_unwrapped(
                        &text[line_start..candidate_end],
                        request.clone(),
                        font_size,
                        line_height,
                        letter_spacing,
                    );
                    if candidate.width > limit && current_end > line_start {
                        let break_at = last_break.unwrap_or(current_end);
                        let line = build_wrapped_line(
                            self,
                            text,
                            request.clone(),
                            font_size,
                            line_height,
                            letter_spacing,
                            line_start,
                            break_at,
                            current_top,
                        );
                        current_top += line.height;
                        lines.push(line);
                        line_start = break_at;
                        wrapped = true;
                        break;
                    }
                }

                current_end = candidate_end;
                if grapheme.chars().all(char::is_whitespace) {
                    last_break = Some(candidate_end);
                }
            }

            if wrapped {
                continue;
            }

            if let Some((newline_start, newline_end)) = explicit_newline_end {
                let line = build_wrapped_line(
                    self,
                    text,
                    request.clone(),
                    font_size,
                    line_height,
                    letter_spacing,
                    line_start,
                    newline_start,
                    current_top,
                );
                current_top += line.height;
                lines.push(line);
                line_start = newline_end;
                continue;
            }

            let line = build_wrapped_line(
                self,
                text,
                request.clone(),
                font_size,
                line_height,
                letter_spacing,
                line_start,
                current_end,
                current_top,
            );
            current_top += line.height;
            lines.push(line);
            line_start = current_end;
        }

        if text.ends_with('\n') {
            let line = build_wrapped_line(
                self,
                text,
                request,
                font_size,
                line_height,
                letter_spacing,
                text.len(),
                text.len(),
                current_top,
            );
            lines.push(line);
        }

        let width = lines.iter().map(|line| line.width).fold(0.0, f32::max);
        let height = lines
            .last()
            .map(|line| line.top + line.height)
            .unwrap_or(line_height);
        let boundaries = lines
            .first()
            .map(|line| line.boundaries.clone())
            .unwrap_or_else(|| vec![TextBoundary { index: 0, x: 0.0 }]);

        TextLayoutInfo {
            width,
            height,
            boundaries,
            lines,
        }
    }

    fn measure_text_layout_unwrapped(
        &self,
        text: &str,
        request: TextFontRequest<'_>,
        font_size: f32,
        line_height: f32,
        letter_spacing: f32,
    ) -> TextLayoutInfo {
        if text.is_empty() {
            return TextLayoutInfo {
                width: 0.0,
                height: line_height,
                boundaries: vec![TextBoundary { index: 0, x: 0.0 }],
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
            |buffer| {
                let mut width: f32 = 0.0;
                let mut height: f32 = 0.0;
                for run in buffer.layout_runs() {
                    width = width.max(run.line_w);
                    height = height.max(run.line_top + run.line_height);
                }

                let mut boundaries = vec![TextBoundary { index: 0, x: 0.0 }];
                if let Some(run) = buffer.layout_runs().next() {
                    for glyph in run.glyphs {
                        push_boundary(&mut boundaries, glyph.start, glyph.x.max(0.0));

                        let cluster = &run.text[glyph.start..glyph.end];
                        let grapheme_count = cluster.graphemes(true).count().max(1);
                        let grapheme_width = glyph.w / grapheme_count as f32;
                        let mut grapheme_x = glyph.x;

                        for (offset, grapheme) in cluster.grapheme_indices(true) {
                            grapheme_x += grapheme_width;
                            push_boundary(
                                &mut boundaries,
                                glyph.start + offset + grapheme.len(),
                                grapheme_x.max(0.0),
                            );
                        }
                    }
                }

                push_boundary(&mut boundaries, text.len(), width.max(0.0));

                TextLayoutInfo {
                    width: width.max(0.0),
                    height: height.max(line_height),
                    boundaries: boundaries.clone(),
                    lines: vec![TextLineLayoutInfo {
                        start_index: 0,
                        end_index: text.len(),
                        top: 0.0,
                        height: height.max(line_height),
                        width: width.max(0.0),
                        boundaries,
                    }],
                }
            },
        )
    }

    fn with_text_buffer<T>(
        &self,
        text: &str,
        request: TextFontRequest<'_>,
        font_size: f32,
        line_height: f32,
        letter_spacing: f32,
        compute: impl FnOnce(&Buffer) -> T,
    ) -> T {
        let resolved = self.resolve_text(text, request.clone());
        let mut font_system = self.font_system.borrow_mut();
        let mut buffer = Buffer::new(&mut font_system, Metrics::new(font_size, line_height));
        buffer.set_size(None, None);
        buffer.set_wrap(Wrap::None);
        let attrs = Attrs::new()
            .family(Family::Name(&resolved.primary_font))
            .weight(Weight(request.weight.to_raw()))
            .letter_spacing(letter_spacing / font_size.max(1.0));
        buffer.set_text(text, &attrs, Shaping::Advanced, None);
        buffer.shape_until_scroll(&mut font_system, false);
        let effective_line_height =
            measured_glyph_line_height(&mut buffer, &mut font_system, line_height).max(line_height);
        if effective_line_height > line_height + 0.01 {
            buffer.set_metrics(Metrics::new(font_size, effective_line_height));
            buffer.shape_until_scroll(&mut font_system, false);
        }
        compute(&buffer)
    }

    fn resolve_family_name(&self, name: &str, weight: FontWeight) -> Option<String> {
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

        self.font_system
            .borrow()
            .db()
            .query(&query)
            .and_then(|id| face_family_name(self.font_system.borrow().db(), id))
            .or_else(|| {
                self.font_system.borrow().db().faces().find_map(|face| {
                    face.families
                        .iter()
                        .find(|(family, _)| family.eq_ignore_ascii_case(name))
                        .map(|(family, _)| family.clone())
                })
            })
    }

    fn system_default_family(&self, weight: FontWeight) -> Option<String> {
        let families = [Family::SansSerif];
        let query = Query {
            families: &families,
            weight: Weight(weight.to_raw()),
            stretch: Stretch::Normal,
            style: Style::Normal,
        };

        self.font_system
            .borrow()
            .db()
            .query(&query)
            .and_then(|id| face_family_name(self.font_system.borrow().db(), id))
    }

    fn script_aware_default_family(&self, text: &str, weight: FontWeight) -> Option<String> {
        if !contains_cjk(text) {
            return None;
        }

        let database = self.font_system.borrow();
        let database = database.db();

        desktop_cjk_sans_candidates()
            .and_then(|candidates| first_matching_family(database, candidates))
            .or_else(|| self.system_default_family(weight))
    }
}

fn push_boundary(boundaries: &mut Vec<TextBoundary>, index: usize, x: f32) {
    let x = x.max(0.0);
    if let Some(last) = boundaries.last_mut() {
        if last.index == index {
            last.x = x;
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

fn build_wrapped_line(
    manager: &FontManager,
    text: &str,
    request: TextFontRequest<'_>,
    font_size: f32,
    line_height: f32,
    letter_spacing: f32,
    start_index: usize,
    end_index: usize,
    top: f32,
) -> TextLineLayoutInfo {
    let layout = manager.measure_text_layout_unwrapped(
        &text[start_index..end_index],
        request,
        font_size,
        line_height,
        letter_spacing,
    );
    let boundaries = layout
        .boundaries
        .into_iter()
        .map(|boundary| TextBoundary {
            index: boundary.index + start_index,
            x: boundary.x,
        })
        .collect::<Vec<_>>();

    TextLineLayoutInfo {
        start_index,
        end_index,
        top,
        height: layout.height.max(line_height),
        width: layout.width,
        boundaries,
    }
}

#[cfg(test)]
mod tests {
    use super::{FontCatalog, FontManager, FontWeight, TextFontRequest};
    use unicode_segmentation::UnicodeSegmentation;

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
