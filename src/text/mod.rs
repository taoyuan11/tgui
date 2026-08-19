//! Unicode text shaping, logical layout, and layout caching.
//!
//! This module deliberately stops at glyph identities and logical geometry.
//! Rasterized glyphs and atlas pages are resources owned by the glyph-atlas
//! layer, so evicting an atlas page never invalidates or repeats shaping.

mod glyph_atlas;

pub use glyph_atlas::{
    AtlasRect, GlyphAtlas, GlyphAtlasConfig, GlyphAtlasKey, GlyphAtlasStats,
    GlyphCompletionOutcome, GlyphContentType, GlyphInvalidation, GlyphInvalidationPhases, GlyphKey,
    GlyphLookup, GlyphPageDescriptor, GlyphPlacement, GlyphRaster, GlyphRasterCompletion,
    GlyphRasterRequest, GlyphVariant, PhysicalFontSize,
};

use crate::core::{DpiScale, Error, FontHandle, Point, Rect, Result, Size};
use std::cmp::Ordering;
use std::collections::HashMap;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::ops::Range;
use std::sync::Arc;

#[cfg(feature = "text")]
use cosmic_text::{
    Align as CosmicAlign, Attrs, Buffer, Family as CosmicFamily, FontSystem,
    LineIter as CosmicLineIter, Metrics, Shaping, Style as CosmicStyle,
    SubpixelBin as CosmicSubpixelBin, Weight as CosmicWeight, Wrap as CosmicWrap,
};

pub const BACKEND_ENABLED: bool = cfg!(feature = "text");

/// A CSS-compatible font weight in the inclusive range 1 through 1000.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FontWeight(u16);

impl FontWeight {
    pub const THIN: Self = Self(100);
    pub const LIGHT: Self = Self(300);
    pub const NORMAL: Self = Self(400);
    pub const MEDIUM: Self = Self(500);
    pub const SEMIBOLD: Self = Self(600);
    pub const BOLD: Self = Self(700);
    pub const BLACK: Self = Self(900);

    pub fn new(weight: u16) -> Result<Self> {
        if !(1..=1000).contains(&weight) {
            return Err(Error::invalid_input(
                Some("text.weight".to_owned()),
                "font weight must be between 1 and 1000",
            ));
        }
        Ok(Self(weight))
    }

    pub const fn get(self) -> u16 {
        self.0
    }
}

impl Default for FontWeight {
    fn default() -> Self {
        Self::NORMAL
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum FontSlant {
    #[default]
    Normal,
    Italic,
    Oblique,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub enum FontFamily {
    Named(Arc<str>),
    Serif,
    #[default]
    SansSerif,
    Cursive,
    Fantasy,
    Monospace,
}

impl FontFamily {
    pub fn named(name: impl Into<Arc<str>>) -> Self {
        Self::Named(name.into())
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum TextDirection {
    #[default]
    Auto,
    LeftToRight,
    RightToLeft,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum WrapStrategy {
    None,
    Glyph,
    Word,
    #[default]
    WordOrGlyph,
}

/// Base attributes for a text layout, expressed in logical pixels.
#[derive(Clone, Debug, PartialEq)]
pub struct TextStyle {
    pub family: FontFamily,
    pub font_size: f32,
    pub line_height: f32,
    pub weight: FontWeight,
    pub slant: FontSlant,
    pub language: Option<Arc<str>>,
    pub direction: TextDirection,
}

impl TextStyle {
    pub fn new(font_size: f32) -> Self {
        Self {
            font_size,
            line_height: font_size * 1.2,
            ..Self::default()
        }
    }

    pub fn with_family(mut self, family: FontFamily) -> Self {
        self.family = family;
        self
    }

    pub const fn with_weight(mut self, weight: FontWeight) -> Self {
        self.weight = weight;
        self
    }

    pub const fn with_slant(mut self, slant: FontSlant) -> Self {
        self.slant = slant;
        self
    }

    pub fn with_language(mut self, language: impl Into<Arc<str>>) -> Self {
        self.language = Some(language.into());
        self
    }

    pub const fn with_direction(mut self, direction: TextDirection) -> Self {
        self.direction = direction;
        self
    }

    pub const fn with_line_height(mut self, line_height: f32) -> Self {
        self.line_height = line_height;
        self
    }

    fn validate(&self) -> Result<()> {
        if !self.font_size.is_finite() || self.font_size <= 0.0 {
            return Err(Error::invalid_input(
                Some("text.font_size".to_owned()),
                "font size must be finite and positive",
            ));
        }
        if !self.line_height.is_finite() || self.line_height <= 0.0 {
            return Err(Error::invalid_input(
                Some("text.line_height".to_owned()),
                "line height must be finite and positive",
            ));
        }
        if let FontFamily::Named(name) = &self.family
            && name.trim().is_empty()
        {
            return Err(Error::invalid_input(
                Some("text.family".to_owned()),
                "named font family must not be empty",
            ));
        }
        if self
            .language
            .as_ref()
            .is_some_and(|value| value.trim().is_empty())
        {
            return Err(Error::invalid_input(
                Some("text.language".to_owned()),
                "language must not be empty",
            ));
        }
        Ok(())
    }
}

impl Default for TextStyle {
    fn default() -> Self {
        Self {
            family: FontFamily::SansSerif,
            font_size: 14.0,
            line_height: 18.0,
            weight: FontWeight::NORMAL,
            slant: FontSlant::Normal,
            language: None,
            direction: TextDirection::Auto,
        }
    }
}

/// Optional style overrides over a UTF-8 byte range.
#[derive(Clone, Debug, PartialEq)]
pub struct TextSpan {
    pub range: Range<usize>,
    pub family: Option<FontFamily>,
    pub font_size: Option<f32>,
    pub weight: Option<FontWeight>,
    pub slant: Option<FontSlant>,
    pub language: Option<Arc<str>>,
}

impl TextSpan {
    pub fn new(range: Range<usize>) -> Self {
        Self {
            range,
            family: None,
            font_size: None,
            weight: None,
            slant: None,
            language: None,
        }
    }

    pub fn with_family(mut self, family: FontFamily) -> Self {
        self.family = Some(family);
        self
    }

    pub const fn with_font_size(mut self, font_size: f32) -> Self {
        self.font_size = Some(font_size);
        self
    }

    pub const fn with_weight(mut self, weight: FontWeight) -> Self {
        self.weight = Some(weight);
        self
    }

    pub const fn with_slant(mut self, slant: FontSlant) -> Self {
        self.slant = Some(slant);
        self
    }

    pub fn with_language(mut self, language: impl Into<Arc<str>>) -> Self {
        self.language = Some(language.into());
        self
    }
}

/// Complete, immutable input to shaping and line layout.
#[derive(Clone, Debug, PartialEq)]
pub struct TextRequest {
    pub text: Arc<str>,
    pub spans: Arc<[TextSpan]>,
    pub style: TextStyle,
    pub content_generation: u64,
    pub span_generation: u64,
    pub width: Option<f32>,
    pub wrap: WrapStrategy,
    pub dpi: DpiScale,
}

impl TextRequest {
    pub fn new(text: impl Into<Arc<str>>, style: TextStyle) -> Self {
        Self {
            text: text.into(),
            spans: Arc::from([]),
            style,
            content_generation: 0,
            span_generation: 0,
            width: None,
            wrap: WrapStrategy::WordOrGlyph,
            dpi: DpiScale::ONE,
        }
    }

    pub fn with_spans(
        mut self,
        spans: impl IntoIterator<Item = TextSpan>,
        generation: u64,
    ) -> Self {
        self.spans = spans.into_iter().collect::<Vec<_>>().into();
        self.span_generation = generation;
        self
    }

    pub const fn with_content_generation(mut self, generation: u64) -> Self {
        self.content_generation = generation;
        self
    }

    pub const fn with_width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }

    pub const fn without_width(mut self) -> Self {
        self.width = None;
        self
    }

    pub const fn with_wrap(mut self, wrap: WrapStrategy) -> Self {
        self.wrap = wrap;
        self
    }

    pub const fn with_dpi(mut self, dpi: DpiScale) -> Self {
        self.dpi = dpi;
        self
    }

    fn validate(&self) -> Result<()> {
        self.style.validate()?;
        if let Some(width) = self.width
            && (!width.is_finite() || width < 0.0)
        {
            return Err(Error::invalid_input(
                Some("text.width".to_owned()),
                "width must be finite and non-negative",
            ));
        }
        let physical_size = f64::from(self.style.font_size) * self.dpi.get();
        if !physical_size.is_finite() || physical_size > f64::from(f32::MAX) {
            return Err(Error::invalid_input(
                Some("text.dpi".to_owned()),
                "DPI produces an unsupported physical font size",
            ));
        }
        let mut previous_end = 0;
        for span in self.spans.iter() {
            if span.range.is_empty()
                || span.range.end > self.text.len()
                || !self.text.is_char_boundary(span.range.start)
                || !self.text.is_char_boundary(span.range.end)
            {
                return Err(Error::invalid_input(
                    Some("text.spans".to_owned()),
                    "span ranges must be non-empty UTF-8 boundaries inside the text",
                ));
            }
            if span.range.start < previous_end {
                return Err(Error::invalid_input(
                    Some("text.spans".to_owned()),
                    "span ranges must be sorted and non-overlapping",
                ));
            }
            if span
                .font_size
                .is_some_and(|size| !size.is_finite() || size <= 0.0)
            {
                return Err(Error::invalid_input(
                    Some("text.spans.font_size".to_owned()),
                    "span font size must be finite and positive",
                ));
            }
            if span
                .language
                .as_ref()
                .is_some_and(|value| value.trim().is_empty())
            {
                return Err(Error::invalid_input(
                    Some("text.spans.language".to_owned()),
                    "span language must not be empty",
                ));
            }
            previous_end = span.range.end;
        }
        Ok(())
    }
}

/// The complete stable identity of a cached layout.
#[derive(Clone, Debug)]
pub struct TextLayoutKey {
    text: Arc<str>,
    spans: Arc<[TextSpan]>,
    style: TextStyle,
    content_generation: u64,
    span_generation: u64,
    font_generation: u64,
    width_bits: Option<u32>,
    wrap: WrapStrategy,
    dpi_bits: u64,
}

impl TextLayoutKey {
    #[cfg(feature = "text")]
    fn new(request: &TextRequest, font_generation: u64) -> Self {
        Self {
            text: Arc::clone(&request.text),
            spans: Arc::clone(&request.spans),
            style: request.style.clone(),
            content_generation: request.content_generation,
            span_generation: request.span_generation,
            font_generation,
            width_bits: request.width.map(f32::to_bits),
            wrap: request.wrap,
            dpi_bits: request.dpi.get().to_bits(),
        }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn spans(&self) -> &[TextSpan] {
        &self.spans
    }

    pub const fn style(&self) -> &TextStyle {
        &self.style
    }

    pub const fn content_generation(&self) -> u64 {
        self.content_generation
    }

    pub const fn span_generation(&self) -> u64 {
        self.span_generation
    }

    pub const fn font_generation(&self) -> u64 {
        self.font_generation
    }

    pub fn width(&self) -> Option<f32> {
        self.width_bits.map(f32::from_bits)
    }

    pub const fn wrap(&self) -> WrapStrategy {
        self.wrap
    }

    pub fn dpi(&self) -> DpiScale {
        DpiScale::new(f64::from_bits(self.dpi_bits)).expect("validated layout DPI")
    }
}

impl PartialEq for TextLayoutKey {
    fn eq(&self, other: &Self) -> bool {
        self.text == other.text
            && spans_equal(&self.spans, &other.spans)
            && styles_equal(&self.style, &other.style)
            && self.content_generation == other.content_generation
            && self.span_generation == other.span_generation
            && self.font_generation == other.font_generation
            && self.width_bits == other.width_bits
            && self.wrap == other.wrap
            && self.dpi_bits == other.dpi_bits
    }
}

impl Eq for TextLayoutKey {}

impl Hash for TextLayoutKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.text.hash(state);
        self.content_generation.hash(state);
        self.span_generation.hash(state);
        self.font_generation.hash(state);
        self.style.family.hash(state);
        self.style.font_size.to_bits().hash(state);
        self.style.line_height.to_bits().hash(state);
        self.style.weight.hash(state);
        self.style.slant.hash(state);
        self.style.language.hash(state);
        self.style.direction.hash(state);
        for span in self.spans.iter() {
            span.range.hash(state);
            span.family.hash(state);
            span.font_size.map(f32::to_bits).hash(state);
            span.weight.hash(state);
            span.slant.hash(state);
            span.language.hash(state);
        }
        self.width_bits.hash(state);
        self.wrap.hash(state);
        self.dpi_bits.hash(state);
    }
}

fn styles_equal(left: &TextStyle, right: &TextStyle) -> bool {
    left.family == right.family
        && left.font_size.to_bits() == right.font_size.to_bits()
        && left.line_height.to_bits() == right.line_height.to_bits()
        && left.weight == right.weight
        && left.slant == right.slant
        && left.language == right.language
        && left.direction == right.direction
}

fn spans_equal(left: &[TextSpan], right: &[TextSpan]) -> bool {
    left.len() == right.len()
        && left.iter().zip(right).all(|(left, right)| {
            left.range == right.range
                && left.family == right.family
                && left.font_size.map(f32::to_bits) == right.font_size.map(f32::to_bits)
                && left.weight == right.weight
                && left.slant == right.slant
                && left.language == right.language
        })
}

/// Opaque font-database face identity used in rasterization keys.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FontFaceId(Arc<str>);

impl FontFaceId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for FontFaceId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SubpixelBin {
    Zero,
    One,
    Two,
    Three,
}

/// Glyph identity at a physical size. Atlas location and page generation are
/// intentionally absent.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct GlyphRasterKey {
    pub font: FontHandle,
    pub font_face: FontFaceId,
    pub glyph_id: u16,
    pub physical_size_bits: u32,
    pub x_bin: SubpixelBin,
    pub y_bin: SubpixelBin,
    pub weight: FontWeight,
    pub flags: u32,
}

impl GlyphRasterKey {
    pub fn physical_size(&self) -> f32 {
        f32::from_bits(self.physical_size_bits)
    }

    /// Converts shaping output into the atlas identity used by P4.2.
    pub fn glyph_key(&self, content_type: GlyphContentType) -> Result<GlyphKey> {
        let physical_size = PhysicalFontSize::from_pixels(self.physical_size())?;
        let variant = (u64::from(self.weight.get()) << 48)
            | (u64::from(self.flags) << 16)
            | (subpixel_fingerprint(self.x_bin) << 8)
            | subpixel_fingerprint(self.y_bin);
        Ok(GlyphKey::new(
            GlyphAtlasKey::new(
                self.font,
                physical_size,
                GlyphVariant::new(variant),
                content_type,
            ),
            u32::from(self.glyph_id),
        ))
    }
}

const fn subpixel_fingerprint(bin: SubpixelBin) -> u64 {
    match bin {
        SubpixelBin::Zero => 0,
        SubpixelBin::One => 1,
        SubpixelBin::Two => 2,
        SubpixelBin::Three => 3,
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct LayoutGlyph {
    pub cluster: Range<usize>,
    pub font: FontHandle,
    pub font_face: FontFaceId,
    pub glyph_id: u16,
    pub font_size: f32,
    pub weight: FontWeight,
    pub bidi_level: u8,
    pub position: Point,
    pub advance: f32,
    pub metadata: usize,
    pub raster_key: GlyphRasterKey,
    pub physical_position: (i32, i32),
}

impl LayoutGlyph {
    pub const fn is_rtl(&self) -> bool {
        self.bidi_level % 2 == 1
    }

    pub fn bounds(&self, line_top: f32, line_height: f32) -> Rect {
        Rect::from_xywh(
            self.position.x,
            line_top,
            self.advance.max(0.0),
            line_height,
        )
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct TextRenderRun {
    pub line_index: usize,
    pub rtl: bool,
    pub font: FontHandle,
    pub font_face: FontFaceId,
    pub font_size: f32,
    pub weight: FontWeight,
    pub glyph_range: Range<usize>,
    pub bounds: Rect,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TextLine {
    pub source_line: usize,
    pub byte_range: Range<usize>,
    pub rtl: bool,
    pub top: f32,
    pub baseline: f32,
    pub height: f32,
    pub width: f32,
    pub glyph_range: Range<usize>,
    pub run_range: Range<usize>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum TextAffinity {
    #[default]
    Before,
    After,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextHit {
    pub byte_index: usize,
    pub affinity: TextAffinity,
    pub line_index: usize,
    pub is_inside: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TextMetrics {
    pub size: Size,
    pub first_baseline: Option<f32>,
    pub line_count: usize,
    pub glyph_count: usize,
}

/// Immutable output of shaping and line layout.
#[derive(Clone, Debug)]
pub struct TextLayout {
    id: u64,
    key: TextLayoutKey,
    metrics: TextMetrics,
    lines: Arc<[TextLine]>,
    runs: Arc<[TextRenderRun]>,
    glyphs: Arc<[LayoutGlyph]>,
}

impl TextLayout {
    pub const fn id(&self) -> u64 {
        self.id
    }

    pub const fn key(&self) -> &TextLayoutKey {
        &self.key
    }

    pub const fn measure(&self) -> TextMetrics {
        self.metrics
    }

    pub const fn size(&self) -> Size {
        self.metrics.size
    }

    pub fn lines(&self) -> &[TextLine] {
        &self.lines
    }

    pub fn render_runs(&self) -> &[TextRenderRun] {
        &self.runs
    }

    pub fn glyphs(&self) -> &[LayoutGlyph] {
        &self.glyphs
    }

    pub fn hit_test(&self, point: Point) -> TextHit {
        let line_index = closest_line(&self.lines, point.y);
        let Some(line) = self.lines.get(line_index) else {
            return TextHit {
                byte_index: 0,
                affinity: TextAffinity::Before,
                line_index: 0,
                is_inside: false,
            };
        };
        let inside_y = point.y >= line.top && point.y < line.top + line.height;
        let inside_x = point.x >= 0.0 && point.x < line.width;
        let mut candidates = Vec::new();
        for glyph in &self.glyphs[line.glyph_range.clone()] {
            let left_index = if glyph.is_rtl() {
                glyph.cluster.end
            } else {
                glyph.cluster.start
            };
            let right_index = if glyph.is_rtl() {
                glyph.cluster.start
            } else {
                glyph.cluster.end
            };
            candidates.push((glyph.position.x, left_index, TextAffinity::After));
            candidates.push((
                glyph.position.x + glyph.advance,
                right_index,
                TextAffinity::Before,
            ));
        }
        if candidates.is_empty() {
            candidates.push((0.0, line.byte_range.start, TextAffinity::Before));
        }
        let (_, byte_index, affinity) = candidates
            .into_iter()
            .min_by(|left, right| {
                (left.0 - point.x)
                    .abs()
                    .partial_cmp(&(right.0 - point.x).abs())
                    .unwrap_or(Ordering::Equal)
            })
            .expect("at least one caret candidate");
        TextHit {
            byte_index,
            affinity,
            line_index,
            is_inside: inside_x && inside_y,
        }
    }

    pub fn caret_geometry(&self, byte_index: usize, affinity: TextAffinity) -> Option<Rect> {
        if byte_index > self.key.text.len() || !self.key.text.is_char_boundary(byte_index) {
            return None;
        }
        let caret_width = (1.0 / self.key.dpi().get() as f32).max(f32::EPSILON);
        let mut matches = Vec::new();
        for line in self.lines.iter() {
            for glyph in &self.glyphs[line.glyph_range.clone()] {
                if byte_index == glyph.cluster.start {
                    let x = if glyph.is_rtl() {
                        glyph.position.x + glyph.advance
                    } else {
                        glyph.position.x
                    };
                    matches.push(Rect::from_xywh(x, line.top, caret_width, line.height));
                }
                if byte_index == glyph.cluster.end {
                    let x = if glyph.is_rtl() {
                        glyph.position.x
                    } else {
                        glyph.position.x + glyph.advance
                    };
                    matches.push(Rect::from_xywh(x, line.top, caret_width, line.height));
                }
            }
            if matches.is_empty()
                && line.byte_range.is_empty()
                && byte_index == line.byte_range.start
            {
                matches.push(Rect::from_xywh(0.0, line.top, caret_width, line.height));
            }
        }
        match affinity {
            TextAffinity::Before => matches.first().copied(),
            TextAffinity::After => matches.last().copied(),
        }
    }

    /// Returns visual rectangles for a logical UTF-8 byte range. Mixed BiDi
    /// selections can yield multiple rectangles on one line.
    pub fn selection_geometry(&self, range: Range<usize>) -> Vec<Rect> {
        if range.is_empty()
            || range.end > self.key.text.len()
            || !self.key.text.is_char_boundary(range.start)
            || !self.key.text.is_char_boundary(range.end)
        {
            return Vec::new();
        }
        let mut output: Vec<Rect> = Vec::new();
        for line in self.lines.iter() {
            let mut intervals = self.glyphs[line.glyph_range.clone()]
                .iter()
                .filter(|glyph| glyph.cluster.start < range.end && range.start < glyph.cluster.end)
                .map(|glyph| (glyph.position.x, glyph.position.x + glyph.advance))
                .collect::<Vec<_>>();
            intervals
                .sort_by(|left, right| left.0.partial_cmp(&right.0).unwrap_or(Ordering::Equal));
            for (start, end) in intervals {
                if let Some(last) = output.last_mut()
                    && last.origin.y == line.top
                    && start <= last.max_x() + 0.01
                {
                    last.size.width = last.size.width.max(end - last.origin.x);
                    continue;
                }
                output.push(Rect::from_xywh(
                    start,
                    line.top,
                    (end - start).max(0.0),
                    line.height,
                ));
            }
        }
        output
    }
}

fn closest_line(lines: &[TextLine], y: f32) -> usize {
    lines
        .iter()
        .enumerate()
        .min_by(|(_, left), (_, right)| {
            distance_to_interval(y, left.top, left.top + left.height)
                .partial_cmp(&distance_to_interval(
                    y,
                    right.top,
                    right.top + right.height,
                ))
                .unwrap_or(Ordering::Equal)
        })
        .map_or(0, |(index, _)| index)
}

fn distance_to_interval(value: f32, start: f32, end: f32) -> f32 {
    if value < start {
        start - value
    } else if value > end {
        value - end
    } else {
        0.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RegisteredFontFace {
    pub handle: FontHandle,
    pub id: FontFaceId,
    pub families: Arc<[Arc<str>]>,
    pub post_script_name: Arc<str>,
    pub weight: FontWeight,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FontRegistration {
    pub generation: u64,
    pub faces: Arc<[RegisteredFontFace]>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TextCacheStats {
    pub entries: usize,
    pub hits: u64,
    pub misses: u64,
    pub shapings: u64,
    pub evictions: u64,
    pub font_generation: u64,
}

#[cfg_attr(not(feature = "text"), allow(dead_code))]
#[derive(Clone, Debug)]
struct CacheEntry {
    layout: Arc<TextLayout>,
    last_used: u64,
}

/// Application-scoped font database and logical layout cache.
#[derive(Debug)]
pub struct TextSystem {
    #[cfg(feature = "text")]
    font_system: FontSystem,
    #[cfg(feature = "text")]
    font_handles: HashMap<FontFaceId, FontHandle>,
    #[cfg(feature = "text")]
    next_font_slot: u32,
    cache: HashMap<TextLayoutKey, CacheEntry>,
    #[cfg(feature = "text")]
    cache_capacity: usize,
    #[cfg(feature = "text")]
    clock: u64,
    #[cfg(feature = "text")]
    next_layout_id: u64,
    hits: u64,
    misses: u64,
    shapings: u64,
    evictions: u64,
    font_generation: u64,
}

impl TextSystem {
    pub fn new() -> Self {
        Self::with_cache_capacity(512)
    }

    pub fn with_cache_capacity(capacity: usize) -> Self {
        #[cfg(not(feature = "text"))]
        let _ = capacity;
        #[cfg(feature = "text")]
        let (font_system, font_handles, next_font_slot) = {
            let font_system = FontSystem::new();
            let font_handles = font_system
                .db()
                .faces()
                .enumerate()
                .filter_map(|(index, face)| {
                    u32::try_from(index).ok().map(|slot| {
                        (
                            FontFaceId(Arc::from(face.id.to_string())),
                            FontHandle::from_parts(slot, 1),
                        )
                    })
                })
                .collect::<HashMap<_, _>>();
            let next_font_slot = u32::try_from(font_handles.len()).unwrap_or(u32::MAX);
            (font_system, font_handles, next_font_slot)
        };
        Self {
            #[cfg(feature = "text")]
            font_system,
            #[cfg(feature = "text")]
            font_handles,
            #[cfg(feature = "text")]
            next_font_slot,
            cache: HashMap::new(),
            #[cfg(feature = "text")]
            cache_capacity: capacity.max(1),
            #[cfg(feature = "text")]
            clock: 0,
            #[cfg(feature = "text")]
            next_layout_id: 1,
            hits: 0,
            misses: 0,
            shapings: 0,
            evictions: 0,
            font_generation: 0,
        }
    }

    pub const fn backend_enabled(&self) -> bool {
        BACKEND_ENABLED
    }

    pub const fn font_generation(&self) -> u64 {
        self.font_generation
    }

    pub fn font_face_count(&self) -> usize {
        #[cfg(feature = "text")]
        {
            self.font_system.db().len()
        }
        #[cfg(not(feature = "text"))]
        {
            0
        }
    }

    pub fn register_font(&mut self, bytes: impl Into<Vec<u8>>) -> Result<FontRegistration> {
        #[cfg(feature = "text")]
        {
            let previous = self
                .font_system
                .db()
                .faces()
                .map(|face| face.id.to_string())
                .collect::<std::collections::HashSet<_>>();
            self.font_system.db_mut().load_font_data(bytes.into());
            let faces = self
                .font_system
                .db()
                .faces()
                .filter(|face| !previous.contains(&face.id.to_string()))
                .map(|face| {
                    (
                        FontFaceId(Arc::from(face.id.to_string())),
                        face.families
                            .iter()
                            .map(|(family, _)| Arc::from(family.as_str()))
                            .collect::<Vec<_>>()
                            .into(),
                        Arc::from(face.post_script_name.as_str()),
                        FontWeight(face.weight.0.clamp(1, 1000)),
                    )
                })
                .collect::<Vec<_>>();
            if faces.is_empty() {
                return Err(Error::resource(
                    None,
                    "font bytes contain no supported font faces",
                    false,
                ));
            }
            let faces = faces
                .into_iter()
                .map(|(id, families, post_script_name, weight)| {
                    let handle =
                        resolve_font_handle(&mut self.font_handles, &mut self.next_font_slot, &id)?;
                    Ok(RegisteredFontFace {
                        handle,
                        id,
                        families,
                        post_script_name,
                        weight,
                    })
                })
                .collect::<Result<Vec<_>>>()?;
            self.font_generation = self.font_generation.saturating_add(1);
            self.evictions = self.evictions.saturating_add(self.cache.len() as u64);
            self.cache.clear();
            Ok(FontRegistration {
                generation: self.font_generation,
                faces: faces.into(),
            })
        }
        #[cfg(not(feature = "text"))]
        {
            let _ = bytes.into();
            Err(Error::degraded(
                "text shaping",
                "disabled text feature",
                "font registration requires the `text` feature",
            ))
        }
    }

    pub fn layout(&mut self, request: &TextRequest) -> Result<Arc<TextLayout>> {
        request.validate()?;
        #[cfg(not(feature = "text"))]
        {
            Err(Error::degraded(
                "text shaping",
                "disabled text feature",
                "layout requires the `text` feature",
            ))
        }
        #[cfg(feature = "text")]
        {
            let key = TextLayoutKey::new(request, self.font_generation);
            self.clock = self.clock.saturating_add(1);
            if let Some(entry) = self.cache.get_mut(&key) {
                entry.last_used = self.clock;
                self.hits = self.hits.saturating_add(1);
                return Ok(Arc::clone(&entry.layout));
            }
            self.misses = self.misses.saturating_add(1);
            let layout = Arc::new(shape_layout(
                &mut self.font_system,
                &mut self.font_handles,
                &mut self.next_font_slot,
                request,
                key.clone(),
                self.next_layout_id,
            )?);
            self.next_layout_id = self.next_layout_id.saturating_add(1);
            self.shapings = self.shapings.saturating_add(1);
            if self.cache.len() >= self.cache_capacity
                && let Some(oldest) = self
                    .cache
                    .iter()
                    .min_by_key(|(_, entry)| entry.last_used)
                    .map(|(key, _)| key.clone())
            {
                self.cache.remove(&oldest);
                self.evictions = self.evictions.saturating_add(1);
            }
            self.cache.insert(
                layout.key.clone(),
                CacheEntry {
                    layout: Arc::clone(&layout),
                    last_used: self.clock,
                },
            );
            Ok(layout)
        }
    }

    pub fn clear_layout_cache(&mut self) {
        self.evictions = self.evictions.saturating_add(self.cache.len() as u64);
        self.cache.clear();
    }

    pub fn cache_stats(&self) -> TextCacheStats {
        TextCacheStats {
            entries: self.cache.len(),
            hits: self.hits,
            misses: self.misses,
            shapings: self.shapings,
            evictions: self.evictions,
            font_generation: self.font_generation,
        }
    }
}

impl Default for TextSystem {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "text")]
fn resolve_font_handle(
    handles: &mut HashMap<FontFaceId, FontHandle>,
    next_slot: &mut u32,
    face: &FontFaceId,
) -> Result<FontHandle> {
    if let Some(handle) = handles.get(face) {
        return Ok(*handle);
    }
    let slot = *next_slot;
    *next_slot = next_slot
        .checked_add(1)
        .ok_or_else(|| Error::resource(None, "font handle identity space exhausted", false))?;
    let handle = FontHandle::from_parts(slot, 1);
    handles.insert(face.clone(), handle);
    Ok(handle)
}

#[cfg(feature = "text")]
fn shape_layout(
    font_system: &mut FontSystem,
    font_handles: &mut HashMap<FontFaceId, FontHandle>,
    next_font_slot: &mut u32,
    request: &TextRequest,
    key: TextLayoutKey,
    id: u64,
) -> Result<TextLayout> {
    let metrics = Metrics::new(request.style.font_size, request.style.line_height);
    let mut buffer = Buffer::new(font_system, metrics);
    buffer.set_size(font_system, request.width, None);
    buffer.set_wrap(font_system, cosmic_wrap(request.wrap));
    let default_attrs = cosmic_attrs(&request.style, None, 0);
    if request.spans.is_empty() {
        buffer.set_text(
            font_system,
            &request.text,
            &default_attrs,
            Shaping::Advanced,
            cosmic_alignment(request.style.direction),
        );
    } else {
        let segments = span_segments(request);
        buffer.set_rich_text(
            font_system,
            segments.iter().map(|segment| {
                (
                    &request.text[segment.range.clone()],
                    cosmic_attrs(
                        &request.style,
                        segment.span.and_then(|index| request.spans.get(index)),
                        segment.span.map_or(0, |index| index + 1),
                    ),
                )
            }),
            &default_attrs,
            Shaping::Advanced,
            cosmic_alignment(request.style.direction),
        );
    }
    buffer.shape_until_scroll(font_system, true);

    let mut line_offsets = CosmicLineIter::new(&request.text)
        .map(|(range, _)| (range.start, range.end))
        .collect::<Vec<_>>();
    if line_offsets.is_empty() {
        line_offsets.push((0, 0));
    } else if request.text.ends_with(['\r', '\n']) {
        line_offsets.push((request.text.len(), request.text.len()));
    }
    let mut glyphs = Vec::new();
    let mut runs = Vec::new();
    let mut lines = Vec::new();
    let mut max_width = 0.0_f32;
    let mut max_height = 0.0_f32;
    let mut first_baseline = None;

    for layout_run in buffer.layout_runs() {
        let line_index = lines.len();
        let line_glyph_start = glyphs.len();
        let line_run_start = runs.len();
        let source_offset = line_offsets
            .get(layout_run.line_i)
            .map_or(0, |(start, _)| *start);
        for glyph in layout_run.glyphs {
            let physical = glyph.physical((0.0, layout_run.line_y), request.dpi.get() as f32);
            let face = FontFaceId(Arc::from(glyph.font_id.to_string()));
            let font = resolve_font_handle(font_handles, next_font_slot, &face)?;
            let weight = FontWeight(glyph.font_weight.0.clamp(1, 1000));
            let logical = LayoutGlyph {
                cluster: source_offset.saturating_add(glyph.start)
                    ..source_offset.saturating_add(glyph.end),
                font,
                font_face: face.clone(),
                glyph_id: glyph.glyph_id,
                font_size: glyph.font_size,
                weight,
                bidi_level: glyph.level.number(),
                position: Point::new(
                    glyph.x + glyph.font_size * glyph.x_offset,
                    layout_run.line_y - glyph.font_size * glyph.y_offset,
                ),
                advance: glyph.w,
                metadata: glyph.metadata,
                raster_key: GlyphRasterKey {
                    font,
                    font_face: face.clone(),
                    glyph_id: physical.cache_key.glyph_id,
                    physical_size_bits: physical.cache_key.font_size_bits,
                    x_bin: subpixel_bin(physical.cache_key.x_bin),
                    y_bin: subpixel_bin(physical.cache_key.y_bin),
                    weight,
                    flags: physical.cache_key.flags.bits(),
                },
                physical_position: (physical.x, physical.y),
            };
            let glyph_index = glyphs.len();
            let can_extend = runs.last().is_some_and(|run: &TextRenderRun| {
                run.line_index == line_index
                    && run.font_face == face
                    && run.font_size.to_bits() == glyph.font_size.to_bits()
                    && run.weight == weight
                    && run.glyph_range.end == glyph_index
            });
            if can_extend {
                let run = runs.last_mut().expect("checked render run");
                run.glyph_range.end += 1;
                run.bounds = run.bounds.union(Rect::from_xywh(
                    logical.position.x,
                    layout_run.line_top,
                    logical.advance.max(0.0),
                    layout_run.line_height,
                ));
            } else {
                runs.push(TextRenderRun {
                    line_index,
                    rtl: layout_run.rtl,
                    font,
                    font_face: face,
                    font_size: glyph.font_size,
                    weight,
                    glyph_range: glyph_index..glyph_index + 1,
                    bounds: Rect::from_xywh(
                        logical.position.x,
                        layout_run.line_top,
                        logical.advance.max(0.0),
                        layout_run.line_height,
                    ),
                });
            }
            glyphs.push(logical);
        }
        let (byte_start, byte_end) = line_offsets
            .get(layout_run.line_i)
            .cloned()
            .unwrap_or((request.text.len(), request.text.len()));
        lines.push(TextLine {
            source_line: layout_run.line_i,
            byte_range: byte_start..byte_end,
            rtl: layout_run.rtl,
            top: layout_run.line_top,
            baseline: layout_run.line_y,
            height: layout_run.line_height,
            width: layout_run.line_w,
            glyph_range: line_glyph_start..glyphs.len(),
            run_range: line_run_start..runs.len(),
        });
        max_width = max_width.max(layout_run.line_w);
        max_height = max_height.max(layout_run.line_top + layout_run.line_height);
        first_baseline.get_or_insert(layout_run.line_y);
    }

    Ok(TextLayout {
        id,
        key,
        metrics: TextMetrics {
            size: Size::new(max_width, max_height),
            first_baseline,
            line_count: lines.len(),
            glyph_count: glyphs.len(),
        },
        lines: lines.into(),
        runs: runs.into(),
        glyphs: glyphs.into(),
    })
}

#[cfg(feature = "text")]
#[derive(Clone, Debug)]
struct SpanSegment {
    range: Range<usize>,
    span: Option<usize>,
}

#[cfg(feature = "text")]
fn span_segments(request: &TextRequest) -> Vec<SpanSegment> {
    let mut output = Vec::new();
    let mut offset = 0;
    for (index, span) in request.spans.iter().enumerate() {
        if offset < span.range.start {
            output.push(SpanSegment {
                range: offset..span.range.start,
                span: None,
            });
        }
        output.push(SpanSegment {
            range: span.range.clone(),
            span: Some(index),
        });
        offset = span.range.end;
    }
    if offset < request.text.len() {
        output.push(SpanSegment {
            range: offset..request.text.len(),
            span: None,
        });
    }
    output
}

#[cfg(feature = "text")]
fn cosmic_attrs<'a>(
    style: &'a TextStyle,
    span: Option<&'a TextSpan>,
    metadata: usize,
) -> Attrs<'a> {
    let family = span
        .and_then(|span| span.family.as_ref())
        .unwrap_or(&style.family);
    let font_size = span
        .and_then(|span| span.font_size)
        .unwrap_or(style.font_size);
    let weight = span.and_then(|span| span.weight).unwrap_or(style.weight);
    let slant = span.and_then(|span| span.slant).unwrap_or(style.slant);
    Attrs::new()
        .family(cosmic_family(family))
        .weight(CosmicWeight(weight.get()))
        .style(match slant {
            FontSlant::Normal => CosmicStyle::Normal,
            FontSlant::Italic => CosmicStyle::Italic,
            FontSlant::Oblique => CosmicStyle::Oblique,
        })
        .metrics(Metrics::new(font_size, style.line_height))
        .metadata(metadata)
}

#[cfg(feature = "text")]
fn cosmic_family(family: &FontFamily) -> CosmicFamily<'_> {
    match family {
        FontFamily::Named(name) => CosmicFamily::Name(name),
        FontFamily::Serif => CosmicFamily::Serif,
        FontFamily::SansSerif => CosmicFamily::SansSerif,
        FontFamily::Cursive => CosmicFamily::Cursive,
        FontFamily::Fantasy => CosmicFamily::Fantasy,
        FontFamily::Monospace => CosmicFamily::Monospace,
    }
}

#[cfg(feature = "text")]
const fn cosmic_wrap(wrap: WrapStrategy) -> CosmicWrap {
    match wrap {
        WrapStrategy::None => CosmicWrap::None,
        WrapStrategy::Glyph => CosmicWrap::Glyph,
        WrapStrategy::Word => CosmicWrap::Word,
        WrapStrategy::WordOrGlyph => CosmicWrap::WordOrGlyph,
    }
}

#[cfg(feature = "text")]
const fn cosmic_alignment(direction: TextDirection) -> Option<CosmicAlign> {
    match direction {
        TextDirection::Auto => None,
        TextDirection::LeftToRight => Some(CosmicAlign::Left),
        TextDirection::RightToLeft => Some(CosmicAlign::Right),
    }
}

#[cfg(feature = "text")]
const fn subpixel_bin(bin: CosmicSubpixelBin) -> SubpixelBin {
    match bin {
        CosmicSubpixelBin::Zero => SubpixelBin::Zero,
        CosmicSubpixelBin::One => SubpixelBin::One,
        CosmicSubpixelBin::Two => SubpixelBin::Two,
        CosmicSubpixelBin::Three => SubpixelBin::Three,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_requests_fail_before_backend_work() {
        let mut system = TextSystem::new();
        let request = TextRequest::new("hello", TextStyle::new(0.0));
        assert!(system.layout(&request).is_err());
        assert_eq!(system.cache_stats().misses, 0);
    }

    #[cfg(feature = "text")]
    #[test]
    fn invalid_font_registration_is_rejected_without_advancing_generation() {
        let mut system = TextSystem::new();
        assert!(system.register_font(b"not a font".to_vec()).is_err());
        assert_eq!(system.font_generation(), 0);
    }
}
