use std::sync::Arc;

use cosmic_text::{Buffer, FontSystem};
use unicode_segmentation::UnicodeSegmentation;

use super::lines::logical_line_offsets;

/// 表示一段文本布局后的几何信息。
#[derive(Debug, Clone)]
pub(crate) struct TextLayoutInfo {
    pub width: f32,
    pub height: f32,
    // Cache hits retain the immutable line/caret geometry; incremental edits detach via COW.
    pub(in crate::text::font) lines: Arc<Vec<TextLineLayoutInfo>>,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct TextBoundary {
    pub(crate) index: usize,
    pub(crate) x: f32,
}

#[derive(Debug, Clone)]
pub(in crate::text::font) struct TextLineLayoutInfo {
    pub(in crate::text::font) start_index: usize,
    pub(in crate::text::font) end_index: usize,
    pub(in crate::text::font) top: f32,
    pub(in crate::text::font) height: f32,
    pub(in crate::text::font) width: f32,
    pub(in crate::text::font) boundaries: Vec<TextBoundary>,
}

impl TextLayoutInfo {
    #[inline]
    pub(crate) fn x_for_index(&self, index: usize) -> f32 {
        self.line_for_index(index).x_for_index(index)
    }

    #[inline]
    pub(crate) fn top_for_index(&self, index: usize) -> f32 {
        self.line_for_index(index).top
    }

    #[inline]
    pub(crate) fn line_height_for_index(&self, index: usize) -> f32 {
        self.line_for_index(index).height
    }

    #[inline]
    pub(crate) fn index_for_x(&self, x: f32) -> usize {
        self.lines
            .first()
            .map(|line| line.index_for_x(x))
            .unwrap_or(0)
    }

    #[inline]
    pub(crate) fn index_for_point(&self, x: f32, y: f32) -> usize {
        self.line_for_y(y).index_for_x(x)
    }

    pub(crate) fn line_index_for_index(&self, index: usize) -> usize {
        self.find_line_index_for_index(index)
    }

    pub(crate) fn line_index_for_y(&self, y: f32) -> usize {
        if self.lines.len() <= 1 {
            return 0;
        }

        let local_y = y.max(0.0);
        self.first_line_with_bottom_after(local_y)
            .min(self.lines.len() - 1)
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

        self.lines
            .partition_point(|line| line.start_index <= index)
            .saturating_sub(1)
            .min(self.lines.len() - 1)
    }

    #[inline]
    fn line_for_index(&self, index: usize) -> &TextLineLayoutInfo {
        if self.lines.len() == 1 {
            return &self.lines[0];
        }

        let line_index = self.find_line_index_for_index(index);
        self.lines
            .get(line_index)
            .or_else(|| self.lines.first())
            .expect("text layout should always contain at least one line")
    }

    #[inline]
    fn line_for_y(&self, y: f32) -> &TextLineLayoutInfo {
        if self.lines.is_empty() {
            panic!("text layout should always contain at least one line");
        }

        self.lines
            .get(self.line_index_for_y(y))
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
    #[inline]
    fn x_for_index(&self, index: usize) -> f32 {
        if self.boundaries.is_empty() {
            return 0.0;
        }

        let local_index = index.saturating_sub(self.start_index);
        if let Some(boundary) = self.boundaries.get(local_index) {
            if boundary.index == local_index {
                return boundary.x;
            }
        }

        let boundary_index = self
            .boundaries
            .partition_point(|boundary| boundary.index <= local_index);
        if boundary_index == 0 {
            0.0
        } else {
            self.boundaries[boundary_index - 1].x
        }
    }

    #[inline]
    fn index_for_x(&self, x: f32) -> usize {
        if self.boundaries.len() <= 1 {
            return self.start_index;
        }

        let local_x = x.max(0.0);
        let right = self
            .boundaries
            .partition_point(|boundary| boundary.x < local_x);
        if right == 0 {
            return self.start_index + self.boundaries[0].index;
        }
        if right >= self.boundaries.len() {
            return self
                .boundaries
                .last()
                .map(|boundary| self.start_index + boundary.index)
                .unwrap_or(self.end_index);
        }

        let left = right - 1;
        let midpoint = (self.boundaries[left].x + self.boundaries[right].x) * 0.5;
        if local_x <= midpoint {
            self.start_index + self.boundaries[left].index
        } else {
            self.start_index + self.boundaries[right].index
        }
    }
}

pub(crate) fn push_boundary(boundaries: &mut Vec<TextBoundary>, index: usize, x: f32) {
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

pub(in crate::text::font) fn measured_glyph_line_height(
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
            let mut grapheme_x = glyph.x;

            if cluster.is_ascii() {
                let byte_count = cluster.len();
                if byte_count > 0 {
                    let grapheme_width = glyph.w / byte_count as f32;
                    for offset in 1..=byte_count {
                        grapheme_x += grapheme_width;
                        push_boundary(
                            &mut boundaries,
                            glyph.start + offset - start_relative,
                            grapheme_x.max(0.0),
                        );
                    }
                }
            } else {
                let grapheme_count = cluster.graphemes(true).count().max(1);
                let grapheme_width = glyph.w / grapheme_count as f32;

                for (offset, grapheme) in cluster.grapheme_indices(true) {
                    grapheme_x += grapheme_width;
                    push_boundary(
                        &mut boundaries,
                        glyph.start + offset + grapheme.len() - start_relative,
                        grapheme_x.max(0.0),
                    );
                }
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
        lines: Arc::new(lines),
    }
}
