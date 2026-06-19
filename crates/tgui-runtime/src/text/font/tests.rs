use cosmic_text::{Attrs, Buffer, Family, Metrics, Shaping, Weight, Wrap};
use unicode_segmentation::UnicodeSegmentation;

use super::catalog::{FontCatalog, FontWeight, TextFontRequest};
use super::layout::{logical_line_offsets, push_boundary, TextBoundary};
use super::manager::FontManager;

#[test]
fn duplicate_boundary_keeps_furthest_forward_position() {
    let mut boundaries = vec![TextBoundary { index: 0, x: 0.0 }];
    push_boundary(&mut boundaries, 1, 12.0);
    push_boundary(&mut boundaries, 1, 9.0);

    assert_eq!(boundaries.len(), 2);
    assert_eq!(boundaries[1].index, 1);
    assert_eq!(boundaries[1].x, 12.0);
}

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

    assert_eq!(
        layout.x_for_index(usize::MAX),
        layout.x_for_index(text.len())
    );
}

#[test]
fn wrapped_text_layout_is_cached_between_calls() {
    let manager = FontManager::new(&FontCatalog::default());
    let text = "wrap this long line into multiple segments for caching\nand keep doing it";
    let request = TextFontRequest {
        preferred_font: None,
        weight: FontWeight::NORMAL,
    };

    let first = manager.measure_text_layout_wrapped(text, request.clone(), 16.0, 24.0, 0.0, 160.0);
    let cache_size_after_first = manager.layout_cache_len();
    let second = manager.measure_text_layout_wrapped(text, request, 16.0, 24.0, 0.0, 160.0);

    assert_eq!(cache_size_after_first, 1);
    assert_eq!(manager.layout_cache_len(), 1);
    assert_eq!(first.width, second.width);
    assert_eq!(first.height, second.height);
    assert_eq!(first.line_count(), second.line_count());
}

#[test]
fn wrapped_text_layout_matches_cosmic_hit_positions() {
    let manager = FontManager::new(&FontCatalog::default());
    let text = "supercalifragilisticexpialidocious wrapped text\nwith another long visual line";
    let request = TextFontRequest {
        preferred_font: None,
        weight: FontWeight::NORMAL,
    };
    let font_size = 16.0;
    let line_height = 24.0;
    let wrap_width = 140.0;
    let layout = manager.measure_text_layout_wrapped(
        text,
        request.clone(),
        font_size,
        line_height,
        0.0,
        wrap_width,
    );

    let resolved = manager.resolve_text(text, request.clone());
    let mut font_system = manager.font_system.borrow_mut();
    let mut buffer = Buffer::new(&mut font_system, Metrics::new(font_size, line_height));
    buffer.set_size(Some(wrap_width), None);
    buffer.set_wrap(Wrap::WordOrGlyph);
    buffer.set_text(
        text,
        &Attrs::new()
            .family(Family::Name(&resolved.primary_font))
            .weight(Weight(request.weight.to_raw())),
        Shaping::Advanced,
        None,
    );
    buffer.shape_until_scroll(&mut font_system, false);

    let line_offsets = logical_line_offsets(text);
    for run in buffer.layout_runs() {
        let sample_y = run.line_top + (run.line_height * 0.5);
        for sample_x in [
            0.0,
            run.line_w * 0.25,
            run.line_w * 0.75,
            (run.line_w - 0.5).max(0.0),
        ] {
            let expected = buffer
                .hit(sample_x, sample_y)
                .map(|cursor| line_offsets.get(cursor.line).copied().unwrap_or(0) + cursor.index)
                .unwrap_or(0);
            let actual = layout.index_for_point(sample_x, sample_y);
            assert_eq!(actual, expected, "x={sample_x}, y={sample_y}");
        }
    }
}

#[test]
fn incremental_layout_edit_on_nonterminal_line_matches_full_layout() {
    let manager = FontManager::new(&FontCatalog::default());
    let old_text = "hello\nworld";
    let new_text = "xhello\nworld";
    let request = TextFontRequest {
        preferred_font: None,
        weight: FontWeight::NORMAL,
    };
    let mut incremental = manager.measure_text_layout(old_text, request.clone(), 16.0, 24.0, 0.0);

    assert!(manager.update_layout_after_edit(
        &mut incremental,
        old_text,
        new_text,
        request.clone(),
        16.0,
        24.0,
        0.0,
        None,
        (0, 0, 0, 1),
    ));

    let fresh = manager.measure_text_layout(new_text, request, 16.0, 24.0, 0.0);
    assert_eq!(incremental.line_count(), fresh.line_count());
    assert_eq!(incremental.height, fresh.height);
    for line_index in 0..fresh.line_count() {
        assert_eq!(
            incremental.line_start(line_index),
            fresh.line_start(line_index)
        );
        assert_eq!(incremental.line_end(line_index), fresh.line_end(line_index));
        assert_eq!(incremental.line_top(line_index), fresh.line_top(line_index));
    }
}

#[test]
fn line_range_for_vertical_span_tracks_visible_lines() {
    let manager = FontManager::new(&FontCatalog::default());
    let layout = manager.measure_text_layout(
        "line 0\nline 1\nline 2",
        TextFontRequest {
            preferred_font: None,
            weight: FontWeight::NORMAL,
        },
        16.0,
        24.0,
        0.0,
    );

    assert_eq!(layout.line_range_for_vertical_span(0.0, 1.0), 0..1);
    assert_eq!(layout.line_range_for_vertical_span(10.0, 30.0), 0..2);
    assert_eq!(layout.line_range_for_vertical_span(24.0, 48.0), 1..2);
    assert_eq!(layout.line_range_for_vertical_span(48.0, 72.0), 2..3);
    assert_eq!(layout.line_range_for_vertical_span(72.0, 96.0), 3..3);
}

#[test]
fn line_queries_track_newline_and_vertical_boundaries() {
    let manager = FontManager::new(&FontCatalog::default());
    let layout = manager.measure_text_layout(
        "aa\nbb\ncc",
        TextFontRequest {
            preferred_font: None,
            weight: FontWeight::NORMAL,
        },
        16.0,
        24.0,
        0.0,
    );

    assert_eq!(layout.line_index_for_index(0), 0);
    assert_eq!(layout.line_index_for_index(2), 0);
    assert_eq!(layout.line_index_for_index(3), 1);
    assert_eq!(layout.line_index_for_index(6), 2);
    assert_eq!(layout.line_index_for_index(usize::MAX), 2);

    assert_eq!(layout.line_index_for_y(-10.0), 0);
    assert_eq!(layout.line_index_for_y(0.0), 0);
    assert_eq!(layout.line_index_for_y(23.99), 0);
    assert_eq!(layout.line_index_for_y(24.0), 1);
    assert_eq!(layout.line_index_for_y(48.0), 2);
    assert_eq!(layout.line_index_for_y(10_000.0), 2);
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

#[test]
fn mixed_cjk_text_keeps_same_primary_font_as_latin_text() {
    let manager = FontManager::new(&FontCatalog::default());
    let latin = manager.resolve_text(
        "abc123",
        TextFontRequest {
            preferred_font: None,
            weight: FontWeight::NORMAL,
        },
    );
    let mixed = manager.resolve_text(
        "abc123中文",
        TextFontRequest {
            preferred_font: None,
            weight: FontWeight::NORMAL,
        },
    );

    assert_eq!(latin.primary_font, mixed.primary_font);
}
