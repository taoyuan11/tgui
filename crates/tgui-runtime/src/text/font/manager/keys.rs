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

/// `resolve_text` 的缓存键。解析结果只依赖脚本类别(CJK-only 文本会优先走
/// CJK 回退)、优先字体族与字重,不需要把整段文本复制进缓存键。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) struct TextResolveKey {
    pub(super) script: TextResolveScript,
    pub(super) preferred_font: Option<Cow<'static, str>>,
    pub(super) weight: FontWeight,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum TextResolveScript {
    CjkOnly,
    Other,
}
