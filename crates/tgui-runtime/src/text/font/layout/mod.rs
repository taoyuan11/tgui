mod lines;
mod text_layout;

pub(crate) use text_layout::{build_layout_info_from_buffer, TextLayoutInfo};
type TextLineLayoutInfo = text_layout::TextLineLayoutInfo;

#[cfg(test)]
pub(crate) type TextBoundary = text_layout::TextBoundary;

#[cfg(test)]
pub(crate) fn push_boundary(boundaries: &mut Vec<TextBoundary>, index: usize, x: f32) {
    text_layout::push_boundary(boundaries, index, x);
}

#[cfg(test)]
pub(crate) fn logical_line_offsets(text: &str) -> Vec<usize> {
    lines::logical_line_offsets(text)
}

pub(in crate::text::font) fn empty_text_layout(line_height: f32) -> TextLayoutInfo {
    lines::empty_text_layout(line_height)
}

pub(in crate::text::font) fn logical_line_start(text: &str, index: usize) -> usize {
    lines::logical_line_start(text, index)
}

pub(in crate::text::font) fn logical_line_end_exclusive(text: &str, index: usize) -> usize {
    lines::logical_line_end_exclusive(text, index)
}

pub(in crate::text::font) fn logical_line_measure_end_exclusive(
    text: &str,
    start: usize,
    end: usize,
) -> usize {
    lines::logical_line_measure_end_exclusive(text, start, end)
}

pub(in crate::text::font) fn shift_line_layout(
    line: TextLineLayoutInfo,
    byte_offset: usize,
    top_offset: f32,
) -> TextLineLayoutInfo {
    lines::shift_line_layout(line, byte_offset, top_offset)
}

pub(in crate::text::font) fn shift_line_layout_tail_in_place(
    line: &mut TextLineLayoutInfo,
    byte_delta: isize,
    top_delta: f32,
) {
    lines::shift_line_layout_tail_in_place(line, byte_delta, top_delta);
}

pub(in crate::text::font) fn measured_glyph_line_height(
    buffer: &mut cosmic_text::Buffer,
    font_system: &mut cosmic_text::FontSystem,
    fallback_line_height: f32,
) -> f32 {
    text_layout::measured_glyph_line_height(buffer, font_system, fallback_line_height)
}
