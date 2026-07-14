use super::text_layout::{TextBoundary, TextLayoutInfo, TextLineLayoutInfo};
use std::sync::Arc;

pub(in crate::text::font) fn empty_text_layout(line_height: f32) -> TextLayoutInfo {
    TextLayoutInfo {
        width: 0.0,
        height: line_height,
        lines: Arc::new(vec![TextLineLayoutInfo {
            start_index: 0,
            end_index: 0,
            top: 0.0,
            height: line_height,
            width: 0.0,
            boundaries: vec![TextBoundary { index: 0, x: 0.0 }],
        }]),
    }
}

pub(crate) fn logical_line_offsets(text: &str) -> Vec<usize> {
    let mut offsets = vec![0];
    for (index, ch) in text.char_indices() {
        if ch == '\n' {
            offsets.push(index + ch.len_utf8());
        }
    }
    offsets
}

pub(in crate::text::font) fn logical_line_start(text: &str, index: usize) -> usize {
    let target = index.min(text.len());
    text[..target].rfind('\n').map(|pos| pos + 1).unwrap_or(0)
}

pub(in crate::text::font) fn logical_line_end_exclusive(text: &str, index: usize) -> usize {
    let target = index.min(text.len());
    text[target..]
        .find('\n')
        .map(|relative| target + relative + 1)
        .unwrap_or(text.len())
}

pub(in crate::text::font) fn logical_line_measure_end_exclusive(
    text: &str,
    start: usize,
    end: usize,
) -> usize {
    if start < end && end < text.len() && text.as_bytes()[end - 1] == b'\n' {
        end - 1
    } else {
        end
    }
}

pub(in crate::text::font) fn shift_line_layout(
    mut line: TextLineLayoutInfo,
    byte_offset: usize,
    top_offset: f32,
) -> TextLineLayoutInfo {
    line.start_index += byte_offset;
    line.end_index += byte_offset;
    line.top += top_offset;
    line
}

pub(in crate::text::font) fn shift_line_layout_tail_in_place(
    line: &mut TextLineLayoutInfo,
    byte_delta: isize,
    top_delta: f32,
) {
    line.start_index = line.start_index.saturating_add_signed(byte_delta);
    line.end_index = line.end_index.saturating_add_signed(byte_delta);
    line.top += top_delta;
}
