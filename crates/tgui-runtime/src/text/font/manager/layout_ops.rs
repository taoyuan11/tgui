use std::sync::Arc;

use cosmic_text::{Buffer, Metrics, Wrap};

use super::super::catalog::TextFontRequest;
use super::super::layout::{
    build_layout_info_from_buffer, empty_text_layout, logical_line_end_exclusive,
    logical_line_measure_end_exclusive, logical_line_start, shift_line_layout,
    shift_line_layout_tail_in_place, TextLayoutInfo,
};
use super::keys::{text_fingerprint, CachedText, TextLayoutLookup, TextMeasureLookup};
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

    pub(crate) fn measure_text_with_layout(
        &self,
        text: &str,
        request: TextFontRequest<'_>,
        font_size: f32,
        line_height: f32,
        letter_spacing: f32,
    ) -> (f32, f32) {
        // Preserve the original empty-text fast path: there is no geometry to
        // warm, and the scene/input path can synthesize its canonical empty line.
        if text.is_empty() {
            return (0.0, line_height.ceil());
        }

        #[cfg(any(test, feature = "bench-support"))]
        self.precise_measure_calls
            .set(self.precise_measure_calls.get().saturating_add(1));
        let layout =
            self.measure_text_layout(text, request, font_size, line_height, letter_spacing);
        (
            layout.width.max(0.0).ceil(),
            layout.height.max(line_height).ceil(),
        )
    }

    pub(crate) fn measure_text_raw(
        &self,
        text: &str,
        request: TextFontRequest<'_>,
        font_size: f32,
        line_height: f32,
        letter_spacing: f32,
    ) -> (f32, f32) {
        #[cfg(feature = "bench-support")]
        let force_precise = self.force_precise_measurement.get();

        #[cfg(feature = "bench-support")]
        if !force_precise {
            self.measure_only_calls
                .set(self.measure_only_calls.get().saturating_add(1));
        }
        #[cfg(all(test, not(feature = "bench-support")))]
        self.measure_only_calls
            .set(self.measure_only_calls.get().saturating_add(1));
        if text.is_empty() {
            return (0.0, line_height.ceil());
        }

        #[cfg(any(test, feature = "bench-support"))]
        self.text_key_scanned_bytes.set(
            self.text_key_scanned_bytes
                .get()
                .saturating_add(text.len() as u64),
        );
        let fingerprint = text_fingerprint(text);
        let cache_lookup = TextMeasureLookup {
            text_fingerprint: fingerprint,
            text,
            preferred_font: request.preferred_font,
            weight: request.weight,
            font_size_bits: font_size.to_bits(),
            line_height_bits: line_height.to_bits(),
            letter_spacing_bits: letter_spacing.to_bits(),
        };
        if let Some(cached) = self.measure_cache.borrow().get(&cache_lookup) {
            return *cached;
        }
        let preferred_font = request.preferred_font;
        let weight = request.weight;

        #[cfg(feature = "bench-support")]
        if !force_precise {
            self.measure_only_cache_misses
                .set(self.measure_only_cache_misses.get().saturating_add(1));
        }
        #[cfg(all(test, not(feature = "bench-support")))]
        self.measure_only_cache_misses
            .set(self.measure_only_cache_misses.get().saturating_add(1));

        #[cfg(feature = "bench-support")]
        let measured = if force_precise {
            self.measure_text_with_layout(text, request, font_size, line_height, letter_spacing)
        } else {
            let size = self.measure_text_size_uncached(
                text,
                request,
                font_size,
                line_height,
                letter_spacing,
                None,
            );
            (size.0.max(0.0).ceil(), size.1.max(line_height).ceil())
        };
        #[cfg(not(feature = "bench-support"))]
        let measured = {
            let size = self.measure_text_size_uncached(
                text,
                request,
                font_size,
                line_height,
                letter_spacing,
                None,
            );
            (size.0.max(0.0).ceil(), size.1.max(line_height).ceil())
        };
        let mut cache = self.measure_cache.borrow_mut();
        if cache.len() > 4096 {
            cache.clear();
        }
        #[cfg(any(test, feature = "bench-support"))]
        self.text_key_owned_allocations
            .set(self.text_key_owned_allocations.get().saturating_add(1));
        cache.insert(
            TextMeasureKey {
                text: CachedText::new(text, fingerprint),
                preferred_font: preferred_font.map(Arc::from),
                weight,
                font_size_bits: font_size.to_bits(),
                line_height_bits: line_height.to_bits(),
                letter_spacing_bits: letter_spacing.to_bits(),
            },
            measured,
        );
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
        let inserted_lines: Vec<_> = Arc::unwrap_or_clone(next_layout.lines)
            .into_iter()
            .map(|line| shift_line_layout(line, new_segment_start, base_top))
            .collect();
        let inserted_len = inserted_lines.len();
        let lines = Arc::make_mut(&mut previous.lines);
        lines.splice(start_line..end_line_exclusive, inserted_lines);
        for line in lines.iter_mut().skip(start_line + inserted_len) {
            shift_line_layout_tail_in_place(line, byte_delta, height_delta);
        }
        previous.width = lines.iter().map(|line| line.width).fold(0.0, f32::max);
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
        #[cfg(any(test, feature = "bench-support"))]
        self.text_key_scanned_bytes.set(
            self.text_key_scanned_bytes
                .get()
                .saturating_add(text.len() as u64),
        );
        let fingerprint = text_fingerprint(text);
        let cache_lookup = TextLayoutLookup {
            text_fingerprint: fingerprint,
            text,
            preferred_font: request.preferred_font,
            weight: request.weight,
            font_size_bits: font_size.to_bits(),
            line_height_bits: line_height.to_bits(),
            letter_spacing_bits: letter_spacing.to_bits(),
            wrap_width_bits: wrap_width.map(f32::to_bits),
        };
        if let Some(cached) = self.layout_cache.borrow().get(&cache_lookup) {
            return cached.clone();
        }
        let preferred_font = request.preferred_font;
        let weight = request.weight;

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
        #[cfg(any(test, feature = "bench-support"))]
        self.text_key_owned_allocations
            .set(self.text_key_owned_allocations.get().saturating_add(1));
        cache.insert(
            TextLayoutKey {
                text: CachedText::new(text, fingerprint),
                preferred_font: preferred_font.map(Arc::from),
                weight,
                font_size_bits: font_size.to_bits(),
                line_height_bits: line_height.to_bits(),
                letter_spacing_bits: letter_spacing.to_bits(),
                wrap_width_bits: wrap_width.map(f32::to_bits),
            },
            layout.clone(),
        );
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
        #[cfg(any(test, feature = "bench-support"))]
        self.precise_layout_builds
            .set(self.precise_layout_builds.get().saturating_add(1));
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

    /// Measures only the intrinsic width and height produced by cosmic-text.
    ///
    /// Taffy's leaf measure callback never needs byte offsets, caret boundaries,
    /// grapheme positions, or the reference-counted line table carried by
    /// `TextLayoutInfo`. Keeping this path beside the precise-layout path makes
    /// both use exactly the same font resolution, shaping, wrapping, and
    /// effective line-height setup while avoiding all of that edit geometry.
    pub(crate) fn measure_text_size_uncached(
        &self,
        text: &str,
        request: TextFontRequest<'_>,
        font_size: f32,
        line_height: f32,
        letter_spacing: f32,
        wrap_width: Option<f32>,
    ) -> (f32, f32) {
        if text.is_empty() {
            return (0.0, line_height);
        }

        self.with_text_buffer(
            text,
            request,
            font_size,
            line_height,
            letter_spacing,
            wrap_width,
            |buffer| measure_buffer_size(buffer, line_height),
        )
    }

    #[cfg(feature = "bench-support")]
    pub(crate) fn benchmark_precise_text_size_uncached(
        &self,
        text: &str,
        request: TextFontRequest<'_>,
        font_size: f32,
        line_height: f32,
        letter_spacing: f32,
        wrap_width: Option<f32>,
    ) -> (f32, f32) {
        let layout = self.measure_text_layout_uncached(
            text,
            request,
            font_size,
            line_height,
            letter_spacing,
            wrap_width,
        );
        (layout.width, layout.height)
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

#[inline]
fn measure_buffer_size(buffer: &Buffer, line_height: f32) -> (f32, f32) {
    let mut width = 0.0f32;
    let mut height = 0.0f32;

    for run in buffer.layout_runs() {
        width = width.max(run.line_w.max(0.0));
        height = height.max(run.line_top + run.line_height);
    }

    (width, height.max(line_height))
}
