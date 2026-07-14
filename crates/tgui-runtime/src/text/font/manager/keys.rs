use std::borrow::Cow;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::Arc;

use hashbrown::Equivalent;

use super::super::catalog::FontWeight;

/// Owned text plus a content fingerprint computed once on cache insertion.
///
/// Equality still compares the complete string, so a fingerprint collision can
/// only cost an extra comparison; it can never produce a false cache hit. Hash
/// table rehashes and cloned keys do not rescan long text.
#[derive(Debug, Clone)]
pub(super) struct CachedText {
    content: Arc<str>,
    fingerprint: u64,
}

impl CachedText {
    pub(super) fn new(text: &str, fingerprint: u64) -> Self {
        Self {
            content: Arc::from(text),
            fingerprint,
        }
    }
}

impl PartialEq for CachedText {
    fn eq(&self, other: &Self) -> bool {
        self.fingerprint == other.fingerprint && self.content == other.content
    }
}

impl Eq for CachedText {}

impl Hash for CachedText {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.fingerprint.hash(state);
    }
}

pub(super) fn text_fingerprint(text: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    text.hash(&mut hasher);
    hasher.finish()
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) struct TextMeasureKey {
    pub(super) text: CachedText,
    pub(super) preferred_font: Option<Arc<str>>,
    pub(super) weight: FontWeight,
    pub(super) font_size_bits: u32,
    pub(super) line_height_bits: u32,
    pub(super) letter_spacing_bits: u32,
}

pub(super) struct TextMeasureLookup<'a> {
    pub(super) text_fingerprint: u64,
    pub(super) text: &'a str,
    pub(super) preferred_font: Option<&'a str>,
    pub(super) weight: FontWeight,
    pub(super) font_size_bits: u32,
    pub(super) line_height_bits: u32,
    pub(super) letter_spacing_bits: u32,
}

impl Hash for TextMeasureLookup<'_> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.text_fingerprint.hash(state);
        self.preferred_font.hash(state);
        self.weight.hash(state);
        self.font_size_bits.hash(state);
        self.line_height_bits.hash(state);
        self.letter_spacing_bits.hash(state);
    }
}

impl Equivalent<TextMeasureKey> for TextMeasureLookup<'_> {
    fn equivalent(&self, key: &TextMeasureKey) -> bool {
        self.text_fingerprint == key.text.fingerprint
            && self.text == key.text.content.as_ref()
            && self.preferred_font == key.preferred_font.as_deref()
            && self.weight == key.weight
            && self.font_size_bits == key.font_size_bits
            && self.line_height_bits == key.line_height_bits
            && self.letter_spacing_bits == key.letter_spacing_bits
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) struct TextLayoutKey {
    pub(super) text: CachedText,
    pub(super) preferred_font: Option<Arc<str>>,
    pub(super) weight: FontWeight,
    pub(super) font_size_bits: u32,
    pub(super) line_height_bits: u32,
    pub(super) letter_spacing_bits: u32,
    pub(super) wrap_width_bits: Option<u32>,
}

pub(super) struct TextLayoutLookup<'a> {
    pub(super) text_fingerprint: u64,
    pub(super) text: &'a str,
    pub(super) preferred_font: Option<&'a str>,
    pub(super) weight: FontWeight,
    pub(super) font_size_bits: u32,
    pub(super) line_height_bits: u32,
    pub(super) letter_spacing_bits: u32,
    pub(super) wrap_width_bits: Option<u32>,
}

impl Hash for TextLayoutLookup<'_> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.text_fingerprint.hash(state);
        self.preferred_font.hash(state);
        self.weight.hash(state);
        self.font_size_bits.hash(state);
        self.line_height_bits.hash(state);
        self.letter_spacing_bits.hash(state);
        self.wrap_width_bits.hash(state);
    }
}

impl Equivalent<TextLayoutKey> for TextLayoutLookup<'_> {
    fn equivalent(&self, key: &TextLayoutKey) -> bool {
        self.text_fingerprint == key.text.fingerprint
            && self.text == key.text.content.as_ref()
            && self.preferred_font == key.preferred_font.as_deref()
            && self.weight == key.weight
            && self.font_size_bits == key.font_size_bits
            && self.line_height_bits == key.line_height_bits
            && self.letter_spacing_bits == key.letter_spacing_bits
            && self.wrap_width_bits == key.wrap_width_bits
    }
}

/// `resolve_text` 的缓存键。解析结果只依赖脚本类别(CJK-only 文本会优先走
/// CJK 回退)、优先字体族与字重,不需要把整段文本复制进缓存键。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) struct TextResolveKey {
    pub(super) catalog_identity: u64,
    pub(super) database_identity: usize,
    pub(super) database_face_count: usize,
    pub(super) script: TextResolveScript,
    pub(super) preferred_font: Option<Cow<'static, str>>,
    pub(super) weight: FontWeight,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum TextResolveScript {
    CjkOnly,
    Other,
}
