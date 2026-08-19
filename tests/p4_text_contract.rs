#![cfg(feature = "text")]

use std::sync::Arc;
use tgui::core::{DpiScale, Point};
use tgui::text::{
    FontFamily, FontWeight, GlyphContentType, TextAffinity, TextDirection, TextRequest, TextSpan,
    TextStyle, TextSystem, WrapStrategy,
};

fn close(left: f32, right: f32) {
    assert!((left - right).abs() < 0.01, "{left} != {right}");
}

#[test]
fn multilingual_shaping_wrap_and_bidi_are_available_without_a_gpu() {
    let mut system = TextSystem::new();
    let request = TextRequest::new(
        "Latin 中文 emoji 🙂 and enough words to wrap across lines",
        TextStyle::new(18.0).with_language("zh-CN"),
    )
    .with_width(140.0)
    .with_wrap(WrapStrategy::WordOrGlyph);
    let layout = system.layout(&request).unwrap();

    assert!(layout.measure().line_count > 1);
    assert!(layout.measure().glyph_count > 10);
    assert!(!layout.render_runs().is_empty());
    assert!(layout.size().width <= 140.01);
    assert!(
        layout
            .glyphs()
            .iter()
            .all(|glyph| request.text.is_char_boundary(glyph.cluster.start)
                && request.text.is_char_boundary(glyph.cluster.end))
    );

    let bidi = TextRequest::new("abc אבג 123", TextStyle::new(20.0));
    let bidi_layout = system.layout(&bidi).unwrap();
    assert!(bidi_layout.glyphs().iter().any(|glyph| glyph.is_rtl()));

    let rtl = TextRequest::new(
        "مرحبا بالعالم",
        TextStyle::new(20.0).with_direction(TextDirection::RightToLeft),
    );
    let rtl_layout = system.layout(&rtl).unwrap();
    assert!(rtl_layout.lines().iter().all(|line| line.rtl));
}

#[test]
fn measure_hit_caret_selection_and_styled_runs_share_one_layout() {
    let mut system = TextSystem::new();
    let text = "hello 世界";
    let request = TextRequest::new(text, TextStyle::new(20.0))
        .with_spans(
            [TextSpan::new(0..5)
                .with_weight(FontWeight::BOLD)
                .with_family(FontFamily::Serif)],
            7,
        )
        .with_width(300.0);
    let layout = system.layout(&request).unwrap();

    let metrics = layout.measure();
    assert!(metrics.size.width > 0.0);
    assert!(metrics.first_baseline.is_some());
    assert!(layout.glyphs().iter().any(|glyph| glyph.metadata == 1));

    let first = &layout.glyphs()[0];
    let hit = layout.hit_test(Point::new(
        first.position.x + first.advance * 0.25,
        layout.lines()[0].top + 1.0,
    ));
    assert!(hit.is_inside);
    assert!(text.is_char_boundary(hit.byte_index));

    let caret = layout.caret_geometry(hit.byte_index, hit.affinity).unwrap();
    assert!(caret.size.width > 0.0);
    assert_eq!(caret.size.height, layout.lines()[hit.line_index].height);

    let selection = layout.selection_geometry(0..text.len());
    assert!(!selection.is_empty());
    assert!(
        layout
            .caret_geometry(text.len(), TextAffinity::After)
            .is_some()
    );
    assert!(layout.selection_geometry(6..7).is_empty());

    let multiline = TextRequest::new("a\r\n世界", TextStyle::new(16.0))
        .with_spans([TextSpan::new(3..9).with_weight(FontWeight::BOLD)], 1);
    let multiline = system.layout(&multiline).unwrap();
    assert!(multiline.glyphs().iter().all(|glyph| {
        multiline.key().text().is_char_boundary(glyph.cluster.start)
            && multiline.key().text().is_char_boundary(glyph.cluster.end)
    }));

    let empty = system
        .layout(&TextRequest::new("", TextStyle::new(16.0)))
        .unwrap();
    assert_eq!(empty.measure().line_count, 1);
    assert!(empty.caret_geometry(0, TextAffinity::Before).is_some());
}

#[test]
fn cache_key_covers_generations_style_language_width_wrap_and_dpi() {
    let mut system = TextSystem::with_cache_capacity(32);
    let base = TextRequest::new(
        "cache identity",
        TextStyle::new(16.0).with_language("en-US"),
    )
    .with_spans([TextSpan::new(0..5)], 1)
    .with_content_generation(1)
    .with_width(200.0)
    .with_wrap(WrapStrategy::Word)
    .with_dpi(DpiScale::ONE);

    let first = system.layout(&base).unwrap();
    let cached = system.layout(&base).unwrap();
    assert!(Arc::ptr_eq(&first, &cached));
    assert_eq!(system.cache_stats().shapings, 1);
    assert_eq!(system.cache_stats().hits, 1);

    let mut variants = Vec::new();
    let mut content_generation = base.clone();
    content_generation.content_generation += 1;
    variants.push(content_generation);
    let mut span_generation = base.clone();
    span_generation.span_generation += 1;
    variants.push(span_generation);
    let mut family = base.clone();
    family.style.family = FontFamily::Monospace;
    variants.push(family);
    let mut size = base.clone();
    size.style.font_size += 1.0;
    variants.push(size);
    let mut weight = base.clone();
    weight.style.weight = FontWeight::BOLD;
    variants.push(weight);
    let mut language = base.clone();
    language.style.language = Some(Arc::from("fr-FR"));
    variants.push(language);
    let mut direction = base.clone();
    direction.style.direction = TextDirection::RightToLeft;
    variants.push(direction);
    let mut width = base.clone();
    width.width = Some(120.0);
    variants.push(width);
    let mut wrap = base.clone();
    wrap.wrap = WrapStrategy::Glyph;
    variants.push(wrap);
    let mut dpi = base.clone();
    dpi.dpi = DpiScale::new(2.0).unwrap();
    variants.push(dpi);

    for variant in &variants {
        assert_ne!(system.layout(variant).unwrap().id(), first.id());
    }
    assert_eq!(system.cache_stats().shapings, 1 + variants.len() as u64);
}

#[test]
fn dpi_changes_raster_keys_but_not_logical_layout_geometry() {
    let mut system = TextSystem::new();
    let one = TextRequest::new("DPI text", TextStyle::new(18.0));
    let two = one.clone().with_dpi(DpiScale::new(2.0).unwrap());
    let layout_one = system.layout(&one).unwrap();
    let layout_two = system.layout(&two).unwrap();

    close(layout_one.size().width, layout_two.size().width);
    close(layout_one.size().height, layout_two.size().height);
    assert_eq!(layout_one.glyphs().len(), layout_two.glyphs().len());
    for (left, right) in layout_one.glyphs().iter().zip(layout_two.glyphs()) {
        close(left.position.x, right.position.x);
        close(left.advance, right.advance);
        close(
            left.raster_key.physical_size() * 2.0,
            right.raster_key.physical_size(),
        );
    }
}

#[test]
fn logical_cache_survives_independent_glyph_resource_lifetimes() {
    let mut system = TextSystem::new();
    let request = TextRequest::new("atlas independent", TextStyle::new(16.0));
    let layout = system.layout(&request).unwrap();
    let raster_keys = layout
        .glyphs()
        .iter()
        .map(|glyph| glyph.raster_key.clone())
        .collect::<Vec<_>>();
    let atlas_key = raster_keys[0].glyph_key(GlyphContentType::Mask).unwrap();
    assert_eq!(atlas_key.atlas.font, layout.glyphs()[0].font);

    // Atlas implementations may drop these keys at any time. There is no
    // atlas/page input in TextLayoutKey, so the next request remains a hit.
    drop(raster_keys);
    let again = system.layout(&request).unwrap();
    assert!(Arc::ptr_eq(&layout, &again));
    assert_eq!(system.cache_stats().shapings, 1);
}
