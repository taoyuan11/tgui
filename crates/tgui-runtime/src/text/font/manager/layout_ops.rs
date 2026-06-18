use cosmic_text::{Buffer, Metrics, Wrap};

use super::super::catalog::TextFontRequest;
use super::super::layout::{
    build_layout_info_from_buffer, empty_text_layout, logical_line_end_exclusive,
    logical_line_measure_end_exclusive, logical_line_start, shift_line_layout,
    shift_line_layout_tail_in_place, TextLayoutInfo,
};
use super::{FontManager, TextLayoutKey, TextMeasureKey};

impl FontManager {
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
            text: Self::text_key(text),
            preferred_font: Self::font_key(request.preferred_font),
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
            return empty_text_layout(line_height);
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
            text: Self::text_key(text),
            preferred_font: Self::font_key(request.preferred_font),
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
            return empty_text_layout(line_height);
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
}
