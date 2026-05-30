use std::borrow::Cow;

use super::super::catalog::FontWeight;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) struct TextMeasureKey {
    pub(super) text: Cow<'static, str>,
    pub(super) preferred_font: Option<Cow<'static, str>>,
    pub(super) weight: FontWeight,
    pub(super) font_size_bits: u32,
    pub(super) line_height_bits: u32,
    pub(super) letter_spacing_bits: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) struct TextLayoutKey {
    pub(super) text: Cow<'static, str>,
    pub(super) preferred_font: Option<Cow<'static, str>>,
    pub(super) weight: FontWeight,
    pub(super) font_size_bits: u32,
    pub(super) line_height_bits: u32,
    pub(super) letter_spacing_bits: u32,
    pub(super) wrap_width_bits: Option<u32>,
}

/// `resolve_text` 的缓存键。解析结果由文本内容(脚本感知回退依赖它)、
/// 优先字体族与字重共同决定,因此三者都必须进键。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) struct TextResolveKey {
    pub(super) text: Cow<'static, str>,
    pub(super) preferred_font: Option<Cow<'static, str>>,
    pub(super) weight: FontWeight,
}
