use cosmic_text::{Attrs, Buffer, Family, Metrics, Shaping, Weight, Wrap};
use unicode_segmentation::UnicodeSegmentation;

use super::catalog::{FontCatalog, FontWeight, TextFontRequest};
use super::layout::{logical_line_offsets, push_boundary, TextBoundary};
use super::manager::FontManager;

#[test]
fn long_text_cache_hits_do_not_allocate_owned_keys() {
    let manager = FontManager::new(&FontCatalog::default());
    let text = "retained long text key ".repeat(800);
    let request = TextFontRequest {
        preferred_font: None,
        weight: FontWeight::NORMAL,
    };

    let first = manager.measure_text_layout(&text, request.clone(), 16.0, 24.0, 0.0);
    manager.reset_text_key_activity();
    for _ in 0..32 {
        let hit = manager.measure_text_layout(&text, request.clone(), 16.0, 24.0, 0.0);
        assert_eq!(hit.width, first.width);
        assert_eq!(hit.height, first.height);
    }

    let (owned_allocations, scanned_bytes) = manager.text_key_activity();
    assert_eq!(owned_allocations, 0);
    assert_eq!(scanned_bytes, (text.len() * 32) as u64);
}

#[test]
fn layout_cache_remains_bounded_and_distinguishes_changed_long_text() {
    let manager = FontManager::new(&FontCatalog::default());
    let request = TextFontRequest {
        preferred_font: None,
        weight: FontWeight::NORMAL,
    };

    for revision in 0..600 {
        let text = format!("revision {revision}: {}", "x".repeat(4096));
        let _ = manager.measure_text_layout(&text, request.clone(), 16.0, 24.0, 0.0);
        assert!(manager.layout_cache_len() <= 257);
    }

    let a = format!("{}a", "same-prefix".repeat(512));
    let b = format!("{}b", "same-prefix".repeat(512));
    let a_layout = manager.measure_text_layout(&a, request.clone(), 16.0, 24.0, 0.0);
    let b_layout = manager.measure_text_layout(&b, request, 16.0, 24.0, 0.0);
    assert_ne!(a_layout.width, b_layout.width);
}

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
fn measure_only_dimensions_match_precise_layout_across_text_modes() {
    let manager = FontManager::new(&FontCatalog::default());
    let request = TextFontRequest {
        preferred_font: None,
        weight: FontWeight::NORMAL,
    };
    let cases = [
        ("", None, 15.75, 23.625, 0.375),
        ("fractional scale label", None, 15.75, 23.625, 0.375),
        ("trailing newline\n", None, 16.0, 24.0, 0.0),
        (
            "wrap this long line over several visual rows\nand keep the final row",
            Some(137.5),
            16.25,
            24.75,
            0.125,
        ),
        ("中文排版尺寸与完整布局一致", Some(91.25), 17.5, 26.25, 0.25),
        (
            "مرحبا بالعالم هذا نص من اليمين",
            Some(121.75),
            16.5,
            25.0,
            0.0,
        ),
        (
            "emoji 👨‍👩‍👧‍👦 cafe\u{301} 🚀 stays intact",
            Some(109.5),
            15.25,
            23.875,
            0.2,
        ),
        // Canvas/Menu ellipsis overflow remains on the renderer's precise path;
        // an already-materialized ellipsis is still ordinary text for Taffy.
        ("A compact label…", None, 16.0, 24.0, 0.0),
    ];

    for (text, wrap_width, font_size, line_height, letter_spacing) in cases {
        let measured = manager.measure_text_size_uncached(
            text,
            request.clone(),
            font_size,
            line_height,
            letter_spacing,
            wrap_width,
        );
        let precise = match wrap_width {
            Some(width) => manager.measure_text_layout_wrapped(
                text,
                request.clone(),
                font_size,
                line_height,
                letter_spacing,
                width,
            ),
            None => manager.measure_text_layout(
                text,
                request.clone(),
                font_size,
                line_height,
                letter_spacing,
            ),
        };

        assert_eq!(
            measured,
            (precise.width, precise.height),
            "measure-only geometry diverged for {text:?} at wrap={wrap_width:?}"
        );
    }
}

#[test]
fn intrinsic_measurement_does_not_populate_precise_layout_cache() {
    let manager = FontManager::new(&FontCatalog::default());
    let request = TextFontRequest {
        preferred_font: None,
        weight: FontWeight::NORMAL,
    };
    let text = "普通 Text/Taffy only needs width and height 👨‍👩‍👧‍👦";

    let measured = manager.measure_text(text, request.clone(), 15.75, 23.625, 0.125);
    assert!(measured.0 > 0.0);
    assert!(measured.1 >= 24.0);
    assert_eq!(manager.layout_cache_len(), 0);

    // The size cache remains independent, so a stable Taffy measure cannot
    // accidentally clone or retain caret/grapheme line geometry.
    assert_eq!(
        manager.measure_text(text, request.clone(), 15.75, 23.625, 0.125),
        measured
    );
    assert_eq!(manager.layout_cache_len(), 0);

    let precise = manager.measure_text_layout(text, request, 15.75, 23.625, 0.125);
    assert_eq!(manager.layout_cache_len(), 1);
    assert_eq!(precise.x_for_index(0), 0.0);
    assert!(precise.x_for_index(text.len()) > 0.0);
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
    assert!(std::sync::Arc::ptr_eq(&first.lines, &second.lines));
}

#[test]
fn incremental_layout_edit_detaches_cached_geometry() {
    let manager = FontManager::new(&FontCatalog::default());
    let old_text = "hello\nworld";
    let new_text = "hello!\nworld";
    let request = TextFontRequest {
        preferred_font: None,
        weight: FontWeight::NORMAL,
    };
    let cached = manager.measure_text_layout(old_text, request.clone(), 16.0, 24.0, 0.0);
    let mut edited = manager.measure_text_layout(old_text, request.clone(), 16.0, 24.0, 0.0);

    assert!(std::sync::Arc::ptr_eq(&cached.lines, &edited.lines));
    assert!(manager.update_layout_after_edit(
        &mut edited,
        old_text,
        new_text,
        request,
        16.0,
        24.0,
        0.0,
        None,
        (5, 5, 5, 6),
    ));

    assert!(!std::sync::Arc::ptr_eq(&cached.lines, &edited.lines));
    assert_eq!(cached.line_end(0), 5);
    assert_eq!(edited.line_end(0), 6);
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

#[test]
fn buffer_attrs_reuses_family_resolution_across_stable_calls() {
    let manager = FontManager::new(&FontCatalog::default());
    let request = TextFontRequest {
        preferred_font: None,
        weight: FontWeight::NORMAL,
    };
    let font_system = manager.font_system.borrow();

    for _ in 0..1024 {
        let attrs = manager.buffer_attrs_owned(
            &font_system,
            "stable retained text",
            request.clone(),
            16.0,
            0.0,
        );
        assert!(!matches!(attrs.as_attrs().family, Family::Name("")));
    }

    assert_eq!(manager.resolve_query_count(), 1);
}

#[test]
fn family_resolution_cache_preserves_script_weight_and_preferred_font_boundaries() {
    let manager = FontManager::new(&FontCatalog::default());
    let samples = [
        ("Latin text", None, FontWeight::NORMAL),
        ("中文文本", None, FontWeight::NORMAL),
        ("مرحبا بالعالم", None, FontWeight::NORMAL),
        ("emoji 👨‍👩‍👧‍👦 🚀", None, FontWeight::NORMAL),
        ("Latin text", None, FontWeight::Bold),
        ("Latin text", Some("sans-serif"), FontWeight::NORMAL),
    ];

    for (text, preferred_font, weight) in samples {
        let first = manager.resolve_text(
            text,
            TextFontRequest {
                preferred_font,
                weight,
            },
        );
        let second = manager.resolve_text(
            text,
            TextFontRequest {
                preferred_font,
                weight,
            },
        );
        assert_eq!(first.primary_font, second.primary_font);
    }

    // Latin, RTL and emoji intentionally share the non-CJK primary-font
    // decision; CJK-only, weight and explicit family remain separate keys.
    assert_eq!(manager.resolve_query_count(), 4);
}

#[test]
fn family_resolution_cache_invalidates_when_font_database_catalog_changes() {
    let manager = FontManager::new(&FontCatalog::default());
    let request = TextFontRequest {
        preferred_font: None,
        weight: FontWeight::NORMAL,
    };

    let before = manager.resolve_text("catalog identity", request.clone());
    assert_eq!(manager.resolve_query_count(), 1);

    // A FontSystem can be extended by custom fonts. The face count is part of
    // the key, so catalog-local resolutions never survive that mutation.
    let face_ids = manager
        .font_system
        .borrow()
        .db()
        .faces()
        .map(|face| face.id)
        .collect::<Vec<_>>();
    {
        let mut font_system = manager.font_system.borrow_mut();
        for id in face_ids {
            font_system.db_mut().remove_face(id);
        }
    }
    let after = manager.resolve_text("catalog identity", request);

    assert_eq!(manager.resolve_query_count(), 2);
    assert!(!before.primary_font.is_empty());
    assert_eq!(after.primary_font, "sans-serif");
}
