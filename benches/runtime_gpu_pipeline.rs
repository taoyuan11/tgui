use criterion::{criterion_group, criterion_main, Criterion};

#[cfg(feature = "bench-support")]
use criterion::Throughput;
#[cfg(feature = "bench-support")]
use std::hint::black_box;
#[cfg(feature = "bench-support")]
use std::time::Instant;
#[cfg(feature = "bench-support")]
use tgui::canvas::{Canvas, CanvasRecorder};
#[cfg(feature = "bench-support")]
use tgui::core::{dp, Color, Point, Rect};
#[cfg(feature = "bench-support")]
use tgui::layout::{Axis, Insets, Overflow};
#[cfg(feature = "bench-support")]
use tgui::media::MediaSource;
#[cfg(feature = "bench-support")]
use tgui::mvvm::{State, ViewModelContext};
#[cfg(feature = "bench-support")]
use tgui::theme::Theme;
#[cfg(feature = "bench-support")]
use tgui::widgets::{
    BackgroundGradientStop, BackgroundLinearGradient, Flex, Image, Text, WidgetBenchmarkContext,
    WidgetTree,
};

#[cfg(feature = "bench-support")]
const SIMPLE_SVG: &[u8] =
    br##"<svg xmlns="http://www.w3.org/2000/svg" width="35" height="20"><rect width="35" height="20" fill="#38bdf8"/></svg>"##;

#[cfg(feature = "bench-support")]
fn viewport() -> Rect {
    Rect::new(0.0, 0.0, 1280.0, 720.0)
}

#[cfg(feature = "bench-support")]
fn dashboard(rows: usize, text_columns: usize) -> WidgetTree<()> {
    let mut body = Flex::new(Axis::Vertical)
        .width(dp(1240.0))
        .gap(dp(2.0))
        .padding(Insets::all(dp(8.0)));
    for row in 0..rows {
        let mut item = Flex::new(Axis::Horizontal)
            .width(dp(1224.0))
            .height(dp(30.0))
            .gap(dp(10.0))
            .padding(Insets::symmetric(dp(8.0), dp(3.0)));
        for column in 0..text_columns {
            item = item.child(Text::new(format!(
                "row {row:04} · column {column} · retained GPU frame"
            )));
        }
        body = body.child(item);
    }
    WidgetTree::new(
        Flex::new(Axis::Vertical)
            .width(dp(1280.0))
            .height(dp(720.0))
            .padding(Insets::all(dp(12.0)))
            .child(body),
    )
}

#[cfg(feature = "bench-support")]
fn dense_text_dashboard(rows: usize, text_columns: usize) -> WidgetTree<()> {
    let mut body = Flex::new(Axis::Vertical).width(dp(1240.0)).gap(dp(1.0));
    for row in 0..rows {
        let mut item = Flex::new(Axis::Horizontal)
            .width(dp(1240.0))
            .height(dp(26.0))
            .gap(dp(1.0));
        for column in 0..text_columns {
            // Three digits fit all 48 columns without horizontal culling while remaining unique,
            // so atlas/cache statistics and Criterion throughput both describe 960 real draws.
            item = item
                .child(Text::new(format!("{:03}", row * text_columns + column)).width(dp(20.0)));
        }
        body = body.child(item);
    }
    WidgetTree::new(
        Flex::new(Axis::Vertical)
            .width(dp(1280.0))
            .height(dp(720.0))
            .padding(Insets::all(dp(12.0)))
            .child(body),
    )
}

#[cfg(feature = "bench-support")]
fn opacity_text_tree() -> (State<f32>, WidgetTree<()>) {
    let view_model = ViewModelContext::for_benchmarks();
    let opacity = view_model.state(1.0_f32);
    let tree = WidgetTree::new(
        Flex::new(Axis::Vertical)
            .size(dp(480.0), dp(120.0))
            .opacity(opacity.signal())
            .child(Text::new(
                "Opacity animation must reuse shaped atlas pixels",
            )),
    );
    (opacity, tree)
}

#[cfg(feature = "bench-support")]
fn dense_primitive_dashboard(rows: usize, columns: usize) -> WidgetTree<()> {
    let mut body = Flex::new(Axis::Vertical).width(dp(1240.0)).gap(dp(1.0));
    for row in 0..rows {
        let mut item = Flex::new(Axis::Horizontal)
            .width(dp(1240.0))
            .height(dp(20.0))
            .gap(dp(1.0));
        for column in 0..columns {
            let index = row * columns + column;
            let cell = Flex::new(Axis::Horizontal).width(dp(35.0)).height(dp(20.0));
            item = if row < rows / 2 {
                let color = Color::rgba(32 + (index % 48) as u8, 92, 176, 208);
                item.child(cell.style(move |style, _| {
                    style.surface.background = Some(color.into());
                }))
            } else {
                let gradient = BackgroundLinearGradient::new(
                    Point::new(0.0, 0.0),
                    Point::new(35.0, 20.0),
                    vec![
                        BackgroundGradientStop::new(0.0, Color::rgba(22, 101, 180, 216)),
                        BackgroundGradientStop::new(1.0, Color::rgba(14, 165, 164, 216)),
                    ],
                );
                item.child(cell.style(move |style, _| {
                    style.surface.background_brush = Some(gradient.clone().into());
                }))
            };
        }
        body = body.child(item);
    }
    WidgetTree::new(
        Flex::new(Axis::Vertical)
            .width(dp(1280.0))
            .height(dp(720.0))
            .padding(Insets::all(dp(12.0)))
            .child(body),
    )
}

#[cfg(feature = "bench-support")]
fn dense_image_dashboard(rows: usize, columns: usize) -> WidgetTree<()> {
    let source = MediaSource::bytes(SIMPLE_SVG);
    let mut body = Flex::new(Axis::Vertical).width(dp(1240.0)).gap(dp(1.0));
    for _ in 0..rows {
        let mut item = Flex::new(Axis::Horizontal)
            .width(dp(1240.0))
            .height(dp(20.0))
            .gap(dp(1.0));
        for _ in 0..columns {
            item = item.child(Image::new(source.clone()).size(dp(35.0), dp(20.0)));
        }
        body = body.child(item);
    }
    WidgetTree::new(
        Flex::new(Axis::Vertical)
            .width(dp(1280.0))
            .height(dp(720.0))
            .padding(Insets::all(dp(12.0)))
            .child(body),
    )
}

#[cfg(feature = "bench-support")]
fn dense_mesh_canvas(items: usize) -> WidgetTree<()> {
    let scene = CanvasRecorder::build(|canvas| {
        canvas.set_fill(Color::rgba(56, 189, 248, 224));
        for index in 0..items {
            let column = (index % 40) as f32;
            let row = (index / 40) as f32;
            canvas.fill_circle(16.0 + column * 30.0, 16.0 + row * 27.0, 10.0);
        }
    });
    WidgetTree::new(Canvas::new(scene).width(dp(1240.0)).height(dp(660.0)))
}

#[cfg(feature = "bench-support")]
fn deep_patch_tree(depth: usize) -> WidgetTree<()> {
    let mut node = Flex::new(Axis::Vertical)
        .width(dp(720.0))
        .height(dp(28.0))
        .padding(Insets::all(dp(1.0)));
    for _ in 0..depth {
        node = Flex::new(Axis::Vertical)
            .width(dp(720.0))
            .padding(Insets::all(dp(1.0)))
            .child(node);
    }
    WidgetTree::new(node)
}

#[cfg(feature = "bench-support")]
fn scroll_dashboard(rows: usize) -> WidgetTree<()> {
    let mut content = Flex::new(Axis::Vertical).width(dp(1200.0));
    for row in 0..rows {
        content = content.child(
            Flex::new(Axis::Horizontal)
                .height(dp(28.0))
                .gap(dp(12.0))
                .child(Text::new(format!("scroll row {row:04}")))
                .child(Text::new(format!("metric {}", row % 31))),
        );
    }
    let scroller: tgui::widgets::Element<()> = Flex::new(Axis::Vertical)
        .width(dp(1240.0))
        .height(dp(680.0))
        .overflow_y(Overflow::Scroll)
        .child(content)
        .into();
    WidgetTree::new(scroller)
}

#[cfg(feature = "bench-support")]
fn ready_context(tree: &WidgetTree<()>) -> Option<WidgetBenchmarkContext> {
    let mut context = WidgetBenchmarkContext::new().with_viewport(viewport());
    let _ = context.run_layout_and_scene(tree, Instant::now());
    match context.initialize_headless_gpu() {
        Ok(adapter) => {
            eprintln!(
                "runtime_gpu_pipeline: adapter='{}' backend={} viewport=1280x720 sync=GPU-wait",
                adapter.name, adapter.backend
            );
        }
        Err(error) => {
            eprintln!(
                "Skipping runtime_gpu_pipeline benchmarks: no usable headless GPU adapter ({error})"
            );
            return None;
        }
    }
    if let Err(error) = context.render_cached_scene_to_headless_gpu(tree, Instant::now()) {
        eprintln!("Skipping runtime_gpu_pipeline benchmarks: renderer warmup failed ({error})");
        return None;
    }
    Some(context)
}

#[cfg(feature = "bench-support")]
fn ready_text_context(
    tree: &WidgetTree<()>,
    sprite_draw_batching: bool,
) -> Option<WidgetBenchmarkContext> {
    let mut context = ready_context(tree)?;
    assert!(context.set_headless_sprite_draw_batching(sprite_draw_batching));
    context
        .render_cached_scene_to_headless_gpu(tree, Instant::now())
        .expect("headless text A/B warmup");
    Some(context)
}

#[cfg(feature = "bench-support")]
fn bench_runtime_gpu_pipeline(c: &mut Criterion) {
    let stable_tree = dashboard(120, 2);
    let Some(mut stable_context) = ready_context(&stable_tree) else {
        return;
    };
    let mut stable = c.benchmark_group("runtime_renderer_gpu_submit");
    stable.throughput(Throughput::Elements(240));
    stable.bench_function("stable_retained_frame/120_rows_240_texts", |b| {
        b.iter(|| {
            black_box(
                stable_context
                    .render_cached_scene_to_headless_gpu(black_box(&stable_tree), Instant::now())
                    .expect("headless stable frame"),
            );
        });
    });
    stable.finish();

    let patch_tree = deep_patch_tree(16);
    let Some(mut patch_context) = ready_context(&patch_tree) else {
        return;
    };
    let mut patch = c.benchmark_group("runtime_patch_renderer_gpu_submit");
    patch.throughput(Throughput::Elements(17));
    patch.bench_function("single_leaf_scene_patch/depth_16", |b| {
        b.iter(|| {
            assert!(patch_context.patch_single_deep_leaf_scene(&patch_tree, Instant::now()));
            black_box(
                patch_context
                    .render_cached_scene_to_headless_gpu(black_box(&patch_tree), Instant::now())
                    .expect("headless patched frame"),
            );
        });
    });
    patch.finish();

    // Keep every command inside the viewport. The previous 240x4 layout was vertically culled to
    // about 88 real sprite commands while its Criterion throughput still claimed 960 elements.
    let text_tree = dense_text_dashboard(20, 48);
    let Some(mut text_context) = ready_text_context(&text_tree, true) else {
        return;
    };
    let Some(mut unbatched_text_context) = ready_text_context(&text_tree, false) else {
        return;
    };
    let Some(mut legacy_glyph_text_context) = ready_text_context(&text_tree, true) else {
        return;
    };
    assert!(legacy_glyph_text_context.set_headless_glyph_raster_cache_retention(false));
    if let Some(stats) = text_context.headless_text_gpu_cache_stats() {
        eprintln!(
            "runtime_text_renderer_gpu_submit: cache_entries={} atlas_pages={} dedicated_textures={} unique_bind_groups={}",
            stats.cache_entries,
            stats.atlas_pages,
            stats.dedicated_textures,
            stats.unique_bind_groups
        );
    }
    if let (Some(batched), Some(unbatched)) = (
        text_context.headless_gpu_draw_stats(),
        unbatched_text_context.headless_gpu_draw_stats(),
    ) {
        eprintln!(
            "runtime_text_renderer_gpu_submit: sprite_commands={} batched_draw_calls={} unbatched_draw_calls={} reduction={:.1}x",
            batched.sprite_commands,
            batched.sprite_draw_calls,
            unbatched.sprite_draw_calls,
            unbatched.sprite_draw_calls as f64 / batched.sprite_draw_calls.max(1) as f64,
        );
        assert_eq!(batched.sprite_commands, unbatched.sprite_commands);
        assert_eq!(batched.sprite_commands, 960);
        assert_eq!(unbatched.sprite_commands, unbatched.sprite_draw_calls);
        assert!(batched.sprite_draw_calls < unbatched.sprite_draw_calls);
    }
    let mut text = c.benchmark_group("runtime_text_renderer_gpu_submit");
    text.throughput(Throughput::Elements(960));
    text.bench_function("stable_text_dense_batched/20_rows_960_texts", |b| {
        b.iter(|| {
            black_box(
                text_context
                    .render_cached_scene_to_headless_gpu(black_box(&text_tree), Instant::now())
                    .expect("headless text-dense frame"),
            );
        });
    });
    text.bench_function(
        "stable_text_dense_unbatched_control/20_rows_960_texts",
        |b| {
            b.iter(|| {
                black_box(
                    unbatched_text_context
                        .render_cached_scene_to_headless_gpu(black_box(&text_tree), Instant::now())
                        .expect("headless unbatched text-dense frame"),
                );
            });
        },
    );
    text.finish();

    let Some(mut deferred_atlas_context) = ready_text_context(&text_tree, true) else {
        return;
    };
    let Some(mut immediate_atlas_context) = ready_text_context(&text_tree, true) else {
        return;
    };
    assert!(immediate_atlas_context.set_headless_text_atlas_deferred_upload(false));
    assert!(deferred_atlas_context.clear_headless_text_gpu_cache());
    assert!(immediate_atlas_context.clear_headless_text_gpu_cache());
    assert!(deferred_atlas_context.reset_headless_text_atlas_upload_stats());
    assert!(immediate_atlas_context.reset_headless_text_atlas_upload_stats());
    deferred_atlas_context
        .render_cached_scene_to_headless_gpu(&text_tree, Instant::now())
        .expect("deferred atlas upload probe");
    immediate_atlas_context
        .render_cached_scene_to_headless_gpu(&text_tree, Instant::now())
        .expect("immediate atlas upload control probe");
    let deferred_uploads = deferred_atlas_context
        .headless_text_atlas_upload_stats()
        .expect("deferred atlas upload stats");
    let immediate_uploads = immediate_atlas_context
        .headless_text_atlas_upload_stats()
        .expect("immediate atlas upload stats");
    eprintln!(
        "runtime_text_atlas_upload: deferred_calls={} immediate_calls={} reduction={:.1}x deferred_bytes={} immediate_bytes={} shadow_bytes={} shadow_budget_bytes={}",
        deferred_uploads.write_calls,
        immediate_uploads.write_calls,
        immediate_uploads.write_calls as f64 / deferred_uploads.write_calls.max(1) as f64,
        deferred_uploads.uploaded_bytes,
        immediate_uploads.uploaded_bytes,
        deferred_uploads.shadow_bytes,
        deferred_uploads.shadow_budget_bytes,
    );
    assert_eq!(immediate_uploads.write_calls, 960);
    assert!(deferred_uploads.write_calls < immediate_uploads.write_calls);
    assert_eq!(
        deferred_atlas_context.headless_text_gpu_cache_stats(),
        immediate_atlas_context.headless_text_gpu_cache_stats()
    );
    assert_eq!(
        deferred_atlas_context.headless_gpu_draw_stats(),
        immediate_atlas_context.headless_gpu_draw_stats()
    );

    let mut atlas_upload = c.benchmark_group("runtime_text_atlas_upload_gpu_submit");
    atlas_upload.throughput(Throughput::Elements(960));
    atlas_upload.bench_function("page_shadow_dirty_upload/20_rows_960_texts", |b| {
        b.iter(|| {
            assert!(deferred_atlas_context.clear_headless_text_gpu_cache());
            black_box(
                deferred_atlas_context
                    .render_cached_scene_to_headless_gpu(black_box(&text_tree), Instant::now())
                    .expect("deferred atlas cache rebuild"),
            );
        });
    });
    atlas_upload.bench_function("legacy_immediate_upload/20_rows_960_texts", |b| {
        b.iter(|| {
            assert!(immediate_atlas_context.clear_headless_text_gpu_cache());
            black_box(
                immediate_atlas_context
                    .render_cached_scene_to_headless_gpu(black_box(&text_tree), Instant::now())
                    .expect("immediate atlas cache rebuild"),
            );
        });
    });
    atlas_upload.finish();

    let Some(mut r8_atlas_context) = ready_text_context(&text_tree, true) else {
        return;
    };
    let Some(mut rgba_mask_atlas_context) = ready_text_context(&text_tree, true) else {
        return;
    };
    assert!(rgba_mask_atlas_context.set_headless_text_r8_atlas(false));
    assert!(r8_atlas_context.clear_headless_text_gpu_cache());
    assert!(rgba_mask_atlas_context.clear_headless_text_gpu_cache());
    assert!(r8_atlas_context.reset_headless_text_atlas_upload_stats());
    assert!(rgba_mask_atlas_context.reset_headless_text_atlas_upload_stats());
    r8_atlas_context
        .render_cached_scene_to_headless_gpu(&text_tree, Instant::now())
        .expect("R8 paragraph atlas probe");
    rgba_mask_atlas_context
        .render_cached_scene_to_headless_gpu(&text_tree, Instant::now())
        .expect("RGBA paragraph atlas control probe");
    let r8_stats = r8_atlas_context
        .headless_text_atlas_upload_stats()
        .expect("R8 paragraph atlas stats");
    let rgba_stats = rgba_mask_atlas_context
        .headless_text_atlas_upload_stats()
        .expect("RGBA paragraph atlas stats");
    eprintln!(
        "runtime_text_r8_atlas: r8_calls={} rgba_calls={} r8_upload_bytes={} rgba_upload_bytes={} r8_shadow_bytes={} rgba_shadow_bytes={} upload_reduction={:.1}% shadow_reduction={:.1}%",
        r8_stats.write_calls,
        rgba_stats.write_calls,
        r8_stats.uploaded_bytes,
        rgba_stats.uploaded_bytes,
        r8_stats.shadow_bytes,
        rgba_stats.shadow_bytes,
        100.0 * (1.0 - r8_stats.uploaded_bytes as f64 / rgba_stats.uploaded_bytes.max(1) as f64),
        100.0 * (1.0 - r8_stats.shadow_bytes as f64 / rgba_stats.shadow_bytes.max(1) as f64),
    );
    assert_eq!(r8_stats.uploaded_bytes, r8_stats.r8_uploaded_bytes);
    assert_eq!(r8_stats.rgba_uploaded_bytes, 0);
    assert_eq!(r8_stats.shadow_bytes, r8_stats.r8_shadow_bytes);
    assert_eq!(r8_stats.rgba_shadow_bytes, 0);
    assert_eq!(rgba_stats.uploaded_bytes, rgba_stats.rgba_uploaded_bytes);
    assert_eq!(rgba_stats.r8_uploaded_bytes, 0);
    assert_eq!(rgba_stats.shadow_bytes, rgba_stats.rgba_shadow_bytes);
    assert_eq!(rgba_stats.r8_shadow_bytes, 0);
    assert!(r8_stats.uploaded_bytes < rgba_stats.uploaded_bytes);
    assert!(r8_stats.shadow_bytes < rgba_stats.shadow_bytes);
    assert_eq!(
        r8_atlas_context.headless_gpu_draw_stats(),
        rgba_mask_atlas_context.headless_gpu_draw_stats()
    );

    let mut r8_atlas = c.benchmark_group("runtime_text_r8_atlas_gpu_submit");
    r8_atlas.throughput(Throughput::Elements(960));
    r8_atlas.bench_function("r8_coverage_page/20_rows_960_texts", |b| {
        b.iter(|| {
            assert!(r8_atlas_context.clear_headless_text_gpu_cache());
            black_box(
                r8_atlas_context
                    .render_cached_scene_to_headless_gpu(black_box(&text_tree), Instant::now())
                    .expect("R8 paragraph atlas rebuild"),
            );
        });
    });
    r8_atlas.bench_function("rgba_mask_page_control/20_rows_960_texts", |b| {
        b.iter(|| {
            assert!(rgba_mask_atlas_context.clear_headless_text_gpu_cache());
            black_box(
                rgba_mask_atlas_context
                    .render_cached_scene_to_headless_gpu(black_box(&text_tree), Instant::now())
                    .expect("RGBA paragraph atlas rebuild"),
            );
        });
    });
    r8_atlas.finish();

    let Some(mut blend_context) = ready_text_context(&text_tree, true) else {
        return;
    };
    let Some(mut legacy_blend_context) = ready_text_context(&text_tree, true) else {
        return;
    };
    assert!(legacy_blend_context.set_headless_text_blend_fast_path(false));
    assert!(blend_context.set_headless_glyph_raster_cache_retention(false));
    assert!(legacy_blend_context.set_headless_glyph_raster_cache_retention(false));
    assert!(blend_context.clear_headless_text_gpu_cache());
    assert!(legacy_blend_context.clear_headless_text_gpu_cache());
    assert!(blend_context.reset_headless_text_blend_stats());
    assert!(legacy_blend_context.reset_headless_text_blend_stats());
    blend_context
        .render_cached_scene_to_headless_gpu(&text_tree, Instant::now())
        .expect("fast source-over cold raster probe");
    legacy_blend_context
        .render_cached_scene_to_headless_gpu(&text_tree, Instant::now())
        .expect("legacy source-over cold raster probe");
    let blend_stats = blend_context
        .headless_text_blend_stats()
        .expect("fast source-over stats");
    let legacy_blend_stats = legacy_blend_context
        .headless_text_blend_stats()
        .expect("legacy source-over stats");
    eprintln!(
        "runtime_text_pixel_blend: transparent_fast={} direct_copy_fast={} general_fast={} general_legacy={}",
        blend_stats.transparent_source_pixels,
        blend_stats.direct_copy_pixels,
        blend_stats.general_blend_pixels,
        legacy_blend_stats.general_blend_pixels,
    );
    assert!(blend_stats.direct_copy_pixels > 0);
    assert_eq!(legacy_blend_stats.transparent_source_pixels, 0);
    assert_eq!(legacy_blend_stats.direct_copy_pixels, 0);
    assert_eq!(
        legacy_blend_stats.general_blend_pixels,
        blend_stats.transparent_source_pixels
            + blend_stats.direct_copy_pixels
            + blend_stats.general_blend_pixels
    );

    let mut text_blend = c.benchmark_group("runtime_text_cold_raster_pixel_blend");
    text_blend.throughput(Throughput::Elements(960));
    text_blend.bench_function("fast_source_over/20_rows_960_texts", |b| {
        b.iter(|| {
            assert!(blend_context.clear_headless_text_gpu_cache());
            black_box(
                blend_context
                    .render_cached_scene_to_headless_gpu(black_box(&text_tree), Instant::now())
                    .expect("fast source-over cold raster"),
            );
        });
    });
    text_blend.bench_function("legacy_all_float_control/20_rows_960_texts", |b| {
        b.iter(|| {
            assert!(legacy_blend_context.clear_headless_text_gpu_cache());
            black_box(
                legacy_blend_context
                    .render_cached_scene_to_headless_gpu(black_box(&text_tree), Instant::now())
                    .expect("legacy source-over cold raster"),
            );
        });
    });
    text_blend.finish();

    let mut text_rebuild = c.benchmark_group("runtime_text_cache_rebuild_gpu_submit");
    text_rebuild.throughput(Throughput::Elements(960));
    // Start both paths from an empty whole-text and glyph-raster cache, then prove that the second
    // isomorphic rebuild reuses every retained cosmic-text key while the former per-frame reset
    // inserts the same keys again. Toggling off then back on is benchmark-only and provides a
    // deterministic cold-cache reset for the production policy.
    assert!(text_context.set_headless_glyph_raster_cache_retention(false));
    assert!(text_context.set_headless_glyph_raster_cache_retention(true));
    assert!(legacy_glyph_text_context.set_headless_glyph_raster_cache_retention(false));
    assert!(text_context.clear_headless_text_gpu_cache());
    assert!(legacy_glyph_text_context.clear_headless_text_gpu_cache());
    assert!(text_context.reset_headless_glyph_raster_cache_stats());
    assert!(legacy_glyph_text_context.reset_headless_glyph_raster_cache_stats());
    text_context
        .render_cached_scene_to_headless_gpu(&text_tree, Instant::now())
        .expect("retained glyph cold rebuild probe");
    legacy_glyph_text_context
        .render_cached_scene_to_headless_gpu(&text_tree, Instant::now())
        .expect("legacy glyph cold rebuild probe");
    let retained_cold = text_context
        .headless_glyph_raster_cache_stats()
        .expect("retained glyph cold stats");
    let legacy_cold = legacy_glyph_text_context
        .headless_glyph_raster_cache_stats()
        .expect("legacy glyph cold stats");
    assert_eq!(retained_cold.image_insertions, legacy_cold.image_insertions);
    assert!(retained_cold.image_insertions > 0);
    assert!(retained_cold.image_entries > 0);
    assert_eq!(retained_cold.frame_resets, 0);
    assert_eq!(legacy_cold.image_entries, 0);
    assert_eq!(legacy_cold.frame_resets, 1);
    assert_eq!(retained_cold.budget_evictions, 0);
    assert_eq!(legacy_cold.budget_evictions, 0);

    assert!(text_context.clear_headless_text_gpu_cache());
    assert!(legacy_glyph_text_context.clear_headless_text_gpu_cache());
    assert!(text_context.reset_headless_glyph_raster_cache_stats());
    assert!(legacy_glyph_text_context.reset_headless_glyph_raster_cache_stats());
    text_context
        .render_cached_scene_to_headless_gpu(&text_tree, Instant::now())
        .expect("retained glyph hot rebuild probe");
    legacy_glyph_text_context
        .render_cached_scene_to_headless_gpu(&text_tree, Instant::now())
        .expect("legacy glyph hot rebuild probe");
    let retained_hot = text_context
        .headless_glyph_raster_cache_stats()
        .expect("retained glyph hot stats");
    let legacy_hot = legacy_glyph_text_context
        .headless_glyph_raster_cache_stats()
        .expect("legacy glyph hot stats");
    eprintln!(
        "runtime_text_glyph_raster_cache: cold_insertions={} hot_retained_insertions={} hot_legacy_insertions={} retained_entries={} retained_bytes={} legacy_frame_resets={}",
        retained_cold.image_insertions,
        retained_hot.image_insertions,
        legacy_hot.image_insertions,
        retained_hot.image_entries,
        retained_hot.image_bytes,
        legacy_hot.frame_resets,
    );
    assert_eq!(retained_hot.image_insertions, 0);
    assert_eq!(legacy_hot.image_insertions, retained_cold.image_insertions);
    assert_eq!(retained_hot.image_entries, retained_cold.image_entries);
    assert_eq!(legacy_hot.image_entries, 0);
    assert_eq!(retained_hot.frame_resets, 0);
    assert_eq!(legacy_hot.frame_resets, 1);
    assert_eq!(
        text_context.headless_text_gpu_cache_stats(),
        legacy_glyph_text_context.headless_text_gpu_cache_stats()
    );
    assert_eq!(
        text_context.headless_gpu_draw_stats(),
        legacy_glyph_text_context.headless_gpu_draw_stats()
    );

    text_rebuild.bench_function("retained_glyph_rasters/20_rows_960_texts", |b| {
        b.iter(|| {
            assert!(text_context.clear_headless_text_gpu_cache());
            black_box(
                text_context
                    .render_cached_scene_to_headless_gpu(black_box(&text_tree), Instant::now())
                    .expect("headless text cache rebuild"),
            );
        });
    });
    text_rebuild.bench_function("legacy_frame_reset_control/20_rows_960_texts", |b| {
        b.iter(|| {
            assert!(legacy_glyph_text_context.clear_headless_text_gpu_cache());
            black_box(
                legacy_glyph_text_context
                    .render_cached_scene_to_headless_gpu(black_box(&text_tree), Instant::now())
                    .expect("headless legacy glyph-cache rebuild"),
            );
        });
    });
    text_rebuild.finish();

    let (opacity, opacity_tree) = opacity_text_tree();
    let (legacy_opacity, legacy_opacity_tree) = opacity_text_tree();
    let Some(mut opacity_context) = ready_context(&opacity_tree) else {
        return;
    };
    let Some(mut legacy_opacity_context) = ready_context(&legacy_opacity_tree) else {
        return;
    };
    assert!(legacy_opacity_context.set_headless_text_alpha_cache_normalization(false));
    assert!(legacy_opacity_context.set_headless_text_mask_tint(false));
    legacy_opacity_context
        .render_cached_scene_to_headless_gpu(&legacy_opacity_tree, Instant::now())
        .expect("legacy alpha-key warmup");
    assert!(opacity_context.reset_headless_text_gpu_cache_activity_stats());
    assert!(legacy_opacity_context.reset_headless_text_gpu_cache_activity_stats());
    opacity.set(0.35);
    legacy_opacity.set(0.35);
    opacity_context.recollect_scene_only(&opacity_tree, Instant::now());
    legacy_opacity_context.recollect_scene_only(&legacy_opacity_tree, Instant::now());
    opacity_context
        .render_cached_scene_to_headless_gpu(&opacity_tree, Instant::now())
        .expect("alpha-only text-cache probe");
    legacy_opacity_context
        .render_cached_scene_to_headless_gpu(&legacy_opacity_tree, Instant::now())
        .expect("legacy alpha-key control probe");
    let alpha_activity = opacity_context
        .headless_text_gpu_cache_activity_stats()
        .expect("alpha-only text-cache activity");
    let legacy_alpha_activity = legacy_opacity_context
        .headless_text_gpu_cache_activity_stats()
        .expect("legacy alpha-key activity");
    assert_eq!(alpha_activity.hits, 1);
    assert_eq!(alpha_activity.misses, 0);
    assert_eq!(alpha_activity.atlas_releases, 0);
    assert_eq!(alpha_activity.retained_prepare_cache_clears, 0);
    assert_eq!(legacy_alpha_activity.hits, 0);
    assert_eq!(legacy_alpha_activity.misses, 1);
    assert_eq!(legacy_alpha_activity.atlas_releases, 1);
    assert_eq!(legacy_alpha_activity.retained_prepare_cache_clears, 1);
    assert_eq!(
        opacity_context.headless_text_gpu_cache_stats(),
        legacy_opacity_context.headless_text_gpu_cache_stats()
    );
    assert_eq!(
        opacity_context.headless_gpu_draw_stats(),
        legacy_opacity_context.headless_gpu_draw_stats()
    );

    assert!(opacity_context.reset_headless_text_gpu_cache_activity_stats());
    opacity_context.set_benchmark_theme(Theme::light());
    opacity_context
        .render_cached_scene_to_headless_gpu(&opacity_tree, Instant::now())
        .expect("RGB text-cache control probe");
    let rgb_activity = opacity_context
        .headless_text_gpu_cache_activity_stats()
        .expect("RGB text-cache control activity");
    eprintln!(
        "runtime_text_mask_tint_cache: optimized_hits={} optimized_misses={} optimized_releases={} legacy_hits={} legacy_misses={} legacy_releases={} legacy_prepare_clears={} rgb_hits={} rgb_misses={} rgb_releases={} rgb_prepare_clears={}",
        alpha_activity.hits,
        alpha_activity.misses,
        alpha_activity.atlas_releases,
        legacy_alpha_activity.hits,
        legacy_alpha_activity.misses,
        legacy_alpha_activity.atlas_releases,
        legacy_alpha_activity.retained_prepare_cache_clears,
        rgb_activity.hits,
        rgb_activity.misses,
        rgb_activity.atlas_releases,
        rgb_activity.retained_prepare_cache_clears,
    );
    assert_eq!(rgb_activity.hits, 1);
    assert_eq!(rgb_activity.misses, 0);
    assert_eq!(rgb_activity.atlas_releases, 0);
    assert_eq!(rgb_activity.retained_prepare_cache_clears, 0);

    let mut opacity_bench = c.benchmark_group("runtime_text_opacity_animation_gpu_submit");
    let mut optimized_alpha = 0.35_f32;
    opacity_bench.bench_function("paragraph_mask_vertex_tint", |b| {
        b.iter(|| {
            optimized_alpha = if optimized_alpha < 0.5 { 0.85 } else { 0.35 };
            opacity.set(optimized_alpha);
            opacity_context.recollect_scene_only(&opacity_tree, Instant::now());
            black_box(
                opacity_context
                    .render_cached_scene_to_headless_gpu(&opacity_tree, Instant::now())
                    .expect("optimized text opacity frame"),
            );
        });
    });
    let mut legacy_alpha = 0.35_f32;
    opacity_bench.bench_function("baked_rgba_raster_control", |b| {
        b.iter(|| {
            legacy_alpha = if legacy_alpha < 0.5 { 0.85 } else { 0.35 };
            legacy_opacity.set(legacy_alpha);
            legacy_opacity_context.recollect_scene_only(&legacy_opacity_tree, Instant::now());
            black_box(
                legacy_opacity_context
                    .render_cached_scene_to_headless_gpu(&legacy_opacity_tree, Instant::now())
                    .expect("legacy text opacity frame"),
            );
        });
    });
    opacity_bench.finish();

    let primitive_tree = dense_primitive_dashboard(30, 32);
    let Some(mut primitive_context) = ready_context(&primitive_tree) else {
        return;
    };
    assert!(primitive_context.set_headless_primitive_draw_batching(true));
    primitive_context
        .render_cached_scene_to_headless_gpu(&primitive_tree, Instant::now())
        .expect("headless batched primitive warmup");
    let batched = primitive_context
        .headless_gpu_draw_stats()
        .expect("headless batched primitive stats");
    assert!(primitive_context.set_headless_primitive_draw_batching(false));
    primitive_context
        .render_cached_scene_to_headless_gpu(&primitive_tree, Instant::now())
        .expect("headless unbatched primitive warmup");
    if let Some(unbatched) = primitive_context.headless_gpu_draw_stats() {
        eprintln!(
            "runtime_primitive_renderer_gpu_submit: rect={}->{} brush={}->{} unbatched_rect={} unbatched_brush={} mesh={}->{}",
            batched.rect_commands,
            batched.rect_draw_calls,
            batched.brush_commands,
            batched.brush_draw_calls,
            unbatched.rect_draw_calls,
            unbatched.brush_draw_calls,
            batched.mesh_commands,
            batched.mesh_draw_calls,
        );
        assert_eq!(batched.rect_commands, 480);
        assert_eq!(batched.brush_commands, 480);
        assert_eq!(batched.rect_commands, unbatched.rect_commands);
        assert_eq!(batched.brush_commands, unbatched.brush_commands);
        assert_eq!(unbatched.rect_commands, unbatched.rect_draw_calls);
        assert_eq!(unbatched.brush_commands, unbatched.brush_draw_calls);
        assert_eq!(batched.rect_draw_calls, 15);
        assert_eq!(batched.brush_draw_calls, 15);
        assert_eq!(batched.mesh_commands, batched.mesh_draw_calls);
    }
    let mut primitive = c.benchmark_group("runtime_primitive_renderer_gpu_submit");
    primitive.throughput(Throughput::Elements(960));
    assert!(primitive_context.set_headless_primitive_draw_batching(true));
    primitive.bench_function("stable_rect_brush_batched/960_primitives", |b| {
        b.iter(|| {
            black_box(
                primitive_context
                    .render_cached_scene_to_headless_gpu(black_box(&primitive_tree), Instant::now())
                    .expect("headless primitive frame"),
            );
        });
    });
    assert!(primitive_context.set_headless_primitive_draw_batching(false));
    primitive.bench_function("stable_rect_brush_unbatched_control/960_primitives", |b| {
        b.iter(|| {
            black_box(
                primitive_context
                    .render_cached_scene_to_headless_gpu(black_box(&primitive_tree), Instant::now())
                    .expect("headless unbatched primitive frame"),
            );
        });
    });
    primitive.finish();

    let mesh_tree = dense_mesh_canvas(960);
    let Some(mut mesh_context) = ready_context(&mesh_tree) else {
        return;
    };
    assert!(mesh_context.set_headless_primitive_draw_batching(true));
    mesh_context
        .render_cached_scene_to_headless_gpu(&mesh_tree, Instant::now())
        .expect("headless batched mesh warmup");
    let batched_mesh = mesh_context
        .headless_gpu_draw_stats()
        .expect("headless batched mesh stats");
    assert!(mesh_context.set_headless_primitive_draw_batching(false));
    mesh_context
        .render_cached_scene_to_headless_gpu(&mesh_tree, Instant::now())
        .expect("headless unbatched mesh warmup");
    let unbatched_mesh = mesh_context
        .headless_gpu_draw_stats()
        .expect("headless unbatched mesh stats");
    eprintln!(
        "runtime_mesh_renderer_gpu_submit: mesh_commands={} batched_draw_calls={} unbatched_draw_calls={}",
        batched_mesh.mesh_commands,
        batched_mesh.mesh_draw_calls,
        unbatched_mesh.mesh_draw_calls,
    );
    assert_eq!(batched_mesh.mesh_commands, 960);
    assert_eq!(batched_mesh.mesh_commands, unbatched_mesh.mesh_commands);
    assert_eq!(batched_mesh.mesh_draw_calls, 1);
    assert_eq!(unbatched_mesh.mesh_draw_calls, 960);
    let mut mesh = c.benchmark_group("runtime_mesh_renderer_gpu_submit");
    mesh.throughput(Throughput::Elements(960));
    assert!(mesh_context.set_headless_primitive_draw_batching(true));
    mesh.bench_function("stable_shared_clip_batched/960_meshes", |b| {
        b.iter(|| {
            black_box(
                mesh_context
                    .render_cached_scene_to_headless_gpu(black_box(&mesh_tree), Instant::now())
                    .expect("headless mesh frame"),
            );
        });
    });
    assert!(mesh_context.set_headless_primitive_draw_batching(false));
    mesh.bench_function("stable_shared_clip_unbatched_control/960_meshes", |b| {
        b.iter(|| {
            black_box(
                mesh_context
                    .render_cached_scene_to_headless_gpu(black_box(&mesh_tree), Instant::now())
                    .expect("headless unbatched mesh frame"),
            );
        });
    });
    mesh.finish();

    let image_tree = dense_image_dashboard(30, 32);
    let Some(mut image_context) = ready_context(&image_tree) else {
        return;
    };
    assert!(image_context.set_headless_transparent_shape_skip(true));
    assert!(image_context.set_headless_sprite_draw_batching(true));
    image_context
        .render_cached_scene_to_headless_gpu(&image_tree, Instant::now())
        .expect("headless repeated image batching probe");
    let image_scene = image_context
        .cached_texture_scene_stats()
        .expect("headless image scene stats");
    let batched = image_context
        .headless_gpu_draw_stats()
        .expect("headless image draw stats");
    assert!(image_context.set_headless_transparent_shape_skip(false));
    assert!(image_context.set_headless_sprite_draw_batching(false));
    image_context
        .render_cached_scene_to_headless_gpu(&image_tree, Instant::now())
        .expect("headless repeated image fallback control");
    if let Some(unbatched) = image_context.headless_gpu_draw_stats() {
        eprintln!(
            "runtime_image_renderer_gpu_submit: textures={} unique_texture_ids={} unique_clips={} optimized_rects={} optimized_sprites={}->{} legacy_rects={} legacy_sprites={}->{}",
            image_scene.texture_commands,
            image_scene.unique_texture_ids,
            image_scene.unique_clip_rects,
            batched.rect_commands,
            batched.sprite_commands,
            batched.sprite_draw_calls,
            unbatched.rect_draw_calls,
            unbatched.sprite_commands,
            unbatched.sprite_draw_calls,
        );
        assert_eq!(batched.sprite_commands, 960);
        assert_eq!(batched.sprite_commands, unbatched.sprite_commands);
        assert_eq!(unbatched.sprite_commands, unbatched.sprite_draw_calls);
        assert_eq!(image_scene.unique_texture_ids, 1);
        assert_eq!(image_scene.unique_clip_rects, 30);
        assert_eq!(batched.rect_draw_calls, 0);
        assert_eq!(batched.sprite_draw_calls, 30);
        assert_eq!(unbatched.rect_draw_calls, 960);
        assert_eq!(unbatched.sprite_draw_calls, 960);
    }
    let mut image = c.benchmark_group("runtime_image_renderer_gpu_submit");
    image.throughput(Throughput::Elements(960));
    assert!(image_context.set_headless_transparent_shape_skip(true));
    assert!(image_context.set_headless_sprite_draw_batching(true));
    image_context
        .render_cached_scene_to_headless_gpu(&image_tree, Instant::now())
        .expect("headless optimized image benchmark warmup");
    image.bench_function("stable_same_source_optimized/960_images", |b| {
        b.iter(|| {
            black_box(
                image_context
                    .render_cached_scene_to_headless_gpu(black_box(&image_tree), Instant::now())
                    .expect("headless repeated image frame"),
            );
        });
    });
    assert!(image_context.set_headless_transparent_shape_skip(false));
    assert!(image_context.set_headless_sprite_draw_batching(false));
    image_context
        .render_cached_scene_to_headless_gpu(&image_tree, Instant::now())
        .expect("headless legacy image benchmark warmup");
    image.bench_function("stable_same_source_legacy_control/960_images", |b| {
        b.iter(|| {
            black_box(
                image_context
                    .render_cached_scene_to_headless_gpu(black_box(&image_tree), Instant::now())
                    .expect("headless unbatched repeated image frame"),
            );
        });
    });
    image.finish();

    let scroll_tree = scroll_dashboard(600);
    let Some(mut scroll_context) = ready_context(&scroll_tree) else {
        return;
    };
    let mut offset = 0.0_f32;
    let mut scroll = c.benchmark_group("runtime_scroll_renderer_gpu_submit");
    scroll.throughput(Throughput::Elements(600));
    scroll.bench_function("scroll_recollect_and_submit/600_rows", |b| {
        b.iter(|| {
            offset = if offset == 0.0 { 180.0 } else { 0.0 };
            assert!(scroll_context.set_first_scroll_offset(
                &scroll_tree,
                Point::new(0.0, offset),
                Instant::now(),
            ));
            black_box(
                scroll_context
                    .render_cached_scene_to_headless_gpu(black_box(&scroll_tree), Instant::now())
                    .expect("headless scrolling frame"),
            );
        });
    });
    scroll.finish();
}

#[cfg(not(feature = "bench-support"))]
fn bench_runtime_gpu_pipeline(_c: &mut Criterion) {
    eprintln!("Skipping runtime_gpu_pipeline benchmarks: bench-support feature not enabled");
}

criterion_group!(benches, bench_runtime_gpu_pipeline);
criterion_main!(benches);
