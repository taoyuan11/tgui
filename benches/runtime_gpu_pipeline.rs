use criterion::{criterion_group, criterion_main, Criterion};

#[cfg(feature = "bench-support")]
use criterion::Throughput;
#[cfg(feature = "bench-support")]
use std::hint::black_box;
#[cfg(feature = "bench-support")]
use std::time::{Duration, Instant};
#[cfg(feature = "bench-support")]
use tgui::canvas::{Canvas, CanvasRecorder};
#[cfg(feature = "bench-support")]
use tgui::core::{dp, Color, Point, Rect};
#[cfg(feature = "bench-support")]
use tgui::layout::{Align, Axis, Insets, Justify};
#[cfg(feature = "bench-support")]
use tgui::media::MediaSource;
#[cfg(feature = "bench-support")]
use tgui::mvvm::{State, ViewModelContext};
#[cfg(feature = "bench-support")]
use tgui::theme::Theme;
#[cfg(feature = "bench-support")]
use tgui::widgets::{
    BackgroundGradientStop, BackgroundLinearGradient, Element, Flex, Image,
    RuntimeScrollBenchmarkContext, RuntimeScrollBenchmarkVm, ScrollView, Text,
    WidgetBenchmarkContext, WidgetBenchmarkStats, WidgetTree,
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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TextAtlasStalePattern {
    All,
    Alternating,
}

#[cfg(feature = "bench-support")]
fn text_atlas_eviction_dashboard(
    text_count: usize,
    pattern: TextAtlasStalePattern,
) -> (State<bool>, WidgetTree<()>) {
    let view_model = ViewModelContext::for_benchmarks();
    let replacement = view_model.state(false);
    let replacement_signal = replacement.signal();
    let mut body = Flex::new(Axis::Vertical).width(dp(1240.0)).gap(dp(1.0));
    for row in 0..text_count.div_ceil(48) {
        let mut item = Flex::new(Axis::Horizontal)
            .width(dp(1240.0))
            .height(dp(26.0))
            .gap(dp(1.0));
        for column in 0..48 {
            let index = row * 48 + column;
            if index >= text_count {
                break;
            }
            let dynamic = pattern == TextAtlasStalePattern::All || index % 2 == 0;
            let text = if dynamic {
                let content = replacement_signal.clone().map(move |replacement| {
                    format!("{}{:03}", if replacement { 'B' } else { 'A' }, index)
                });
                Text::new(content)
            } else {
                Text::new(format!("S{index:03}"))
            };
            item = item.child(text.width(dp(24.0)));
        }
        body = body.child(item);
    }
    (
        replacement,
        WidgetTree::new(
            Flex::new(Axis::Vertical)
                .width(dp(1280.0))
                .height(dp(720.0))
                .padding(Insets::all(dp(12.0)))
                .child(body),
        ),
    )
}

#[cfg(feature = "bench-support")]
fn dedicated_text_atlas_eviction_tree() -> (State<bool>, WidgetTree<()>) {
    let view_model = ViewModelContext::for_benchmarks();
    let replacement = view_model.state(false);
    let content = replacement.signal().map(|replacement| {
        if replacement {
            "Dedicated replacement B"
        } else {
            "Dedicated replacement A"
        }
    });
    (
        replacement,
        WidgetTree::new(Text::new(content).size(dp(2100.0), dp(40.0))),
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
fn tooltip_effect_tree(background_blur: f32) -> WidgetTree<()> {
    let tooltip_surface = Flex::new(Axis::Horizontal)
        .size(dp(240.0), dp(44.0))
        .align(Align::Center)
        .padding(Insets::symmetric(dp(12.0), dp(8.0)))
        .style(move |style, _| {
            style.surface.background = Some(Color::rgba(24, 24, 27, 245).into());
            style.surface.background_blur = dp(background_blur).into();
            style.surface.border_color = Some(Color::rgba(255, 255, 255, 28).into());
            style.surface.border_width = Some(dp(1.0).into());
            style.surface.border_radius = Some(dp(4.0).into());
        })
        .child(Text::new("Keyboard shortcut: ⌘S"));

    WidgetTree::new(
        Flex::new(Axis::Vertical)
            .size(dp(1280.0), dp(720.0))
            .align(Align::Center)
            .justify(Justify::Center)
            .style(|style, _| {
                style.surface.background_brush = Some(
                    BackgroundLinearGradient::new(
                        Point::new(0.0, 0.0),
                        Point::new(1280.0, 720.0),
                        vec![
                            BackgroundGradientStop::new(0.0, Color::rgba(15, 23, 42, 255)),
                            BackgroundGradientStop::new(0.5, Color::rgba(30, 64, 175, 255)),
                            BackgroundGradientStop::new(1.0, Color::rgba(13, 148, 136, 255)),
                        ],
                    )
                    .into(),
                );
            })
            .child(tooltip_surface),
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
fn scroll_dashboard(rows: usize) -> WidgetTree<RuntimeScrollBenchmarkVm> {
    let mut summary = Flex::new(Axis::Vertical)
        .width(dp(280.0))
        .height(dp(680.0))
        .gap(dp(6.0));
    for card in 0..48 {
        summary = summary.child(
            Flex::new(Axis::Horizontal)
                .height(dp(24.0))
                .child(Text::new(format!("retained summary {card:02}"))),
        );
    }

    let mut content = Flex::new(Axis::Vertical).width(dp(900.0));
    for row in 0..rows {
        content = content.child(
            Flex::new(Axis::Horizontal)
                .height(dp(28.0))
                .gap(dp(12.0))
                .child(Text::new(format!("scroll row {row:04}")))
                .child(Text::new(format!("metric {}", row % 31))),
        );
    }
    let scroller: Element<RuntimeScrollBenchmarkVm> = ScrollView::new()
        .width(dp(920.0))
        .height(dp(680.0))
        .show_scrollbar(false)
        .child(content)
        .into();
    WidgetTree::new(
        Flex::new(Axis::Horizontal)
            .width(dp(1280.0))
            .height(dp(720.0))
            .gap(dp(12.0))
            .padding(Insets::all(dp(12.0)))
            .child(summary)
            .child(scroller),
    )
}

#[cfg(feature = "bench-support")]
fn ready_context(tree: &WidgetTree<()>) -> Option<WidgetBenchmarkContext> {
    let mut context = WidgetBenchmarkContext::new().with_viewport(viewport());
    let _ = context.run_layout_and_scene(tree, Instant::now());
    match context.initialize_headless_gpu() {
        Ok(adapter) => {
            let _ = context.set_headless_gpu_clean_frame_cache(
                std::env::var("TGUI_CLEAN_PREPARED_FRAME_CACHE").as_deref() != Ok("off"),
            );
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
#[derive(Clone, Copy, Debug)]
struct TextAtlasEvictionFrame {
    total: Duration,
    liveness: Duration,
    scene: WidgetBenchmarkStats,
}

#[cfg(feature = "bench-support")]
struct TextAtlasEvictionFixture {
    replacement: State<bool>,
    tree: WidgetTree<()>,
    context: WidgetBenchmarkContext,
    generation: bool,
}

#[cfg(feature = "bench-support")]
impl TextAtlasEvictionFixture {
    fn new(
        text_count: usize,
        pattern: TextAtlasStalePattern,
        whole_page_fast_path: bool,
        rgba: bool,
    ) -> Option<Self> {
        let (replacement, tree) = text_atlas_eviction_dashboard(text_count, pattern);
        let mut context = ready_text_context(&tree, true)?;
        assert!(context.set_headless_text_atlas_whole_page_stale_release(whole_page_fast_path));
        if rgba {
            assert!(context.set_headless_text_mask_tint(false));
            context
                .render_cached_scene_to_headless_gpu(&tree, Instant::now())
                .expect("RGBA text-atlas eviction warmup");
        }
        Some(Self {
            replacement,
            tree,
            context,
            generation: false,
        })
    }

    fn dedicated(whole_page_fast_path: bool) -> Option<Self> {
        let (replacement, tree) = dedicated_text_atlas_eviction_tree();
        let mut context = ready_text_context(&tree, true)?;
        assert!(context.set_headless_text_atlas_whole_page_stale_release(whole_page_fast_path));
        Some(Self {
            replacement,
            tree,
            context,
            generation: false,
        })
    }

    fn replacement_frame(&mut self) -> TextAtlasEvictionFrame {
        self.generation = !self.generation;
        self.replacement.set(self.generation);
        self.context
            .recollect_scene_only(&self.tree, Instant::now());
        let started = Instant::now();
        let scene = self
            .context
            .render_cached_scene_to_headless_gpu(&self.tree, Instant::now())
            .expect("headless text-atlas replacement frame");
        TextAtlasEvictionFrame {
            total: started.elapsed(),
            liveness: self
                .context
                .headless_gpu_liveness_duration()
                .expect("text-atlas replacement liveness profile"),
            scene,
        }
    }

    fn zero_stale_frame(&mut self) -> TextAtlasEvictionFrame {
        assert!(self.context.force_headless_gpu_cache_liveness_refresh());
        let started = Instant::now();
        let scene = self
            .context
            .render_cached_scene_to_headless_gpu(&self.tree, Instant::now())
            .expect("headless zero-stale text-atlas frame");
        TextAtlasEvictionFrame {
            total: started.elapsed(),
            liveness: self
                .context
                .headless_gpu_liveness_duration()
                .expect("zero-stale text-atlas liveness profile"),
            scene,
        }
    }
}

#[cfg(feature = "bench-support")]
fn duration_p50(mut samples: Vec<Duration>) -> Duration {
    samples.sort_unstable();
    samples[samples.len() / 2]
}

#[cfg(feature = "bench-support")]
fn validate_text_atlas_eviction_pair(
    label: &str,
    optimized: &mut TextAtlasEvictionFixture,
    legacy: &mut TextAtlasEvictionFixture,
    expected_stale: usize,
    expect_whole_pages: bool,
    expected_initial_pages: impl Fn(usize) -> bool,
) {
    let optimized_initial = optimized
        .context
        .headless_text_gpu_cache_stats()
        .expect("optimized initial text cache stats");
    let legacy_initial = legacy
        .context
        .headless_text_gpu_cache_stats()
        .expect("legacy initial text cache stats");
    assert_eq!(
        optimized_initial, legacy_initial,
        "{label} initial cache state"
    );
    assert!(
        expected_initial_pages(optimized_initial.atlas_pages),
        "{label} unexpected initial cache state: {optimized_initial:?}"
    );
    assert_eq!(
        optimized_initial.atlas_live_allocations,
        optimized_initial.r8_allocations + optimized_initial.rgba_allocations
    );
    assert!(optimized
        .context
        .reset_headless_text_gpu_cache_activity_stats());
    assert!(legacy
        .context
        .reset_headless_text_gpu_cache_activity_stats());

    let optimized_frame = optimized.replacement_frame();
    let legacy_frame = legacy.replacement_frame();
    assert_eq!(optimized_frame.scene, legacy_frame.scene, "{label} scene");
    assert_eq!(
        optimized.context.headless_text_gpu_cache_stats(),
        legacy.context.headless_text_gpu_cache_stats(),
        "{label} cache entries/pages/live allocations",
    );
    let optimized_activity = optimized
        .context
        .headless_text_gpu_cache_activity_stats()
        .expect("optimized text-atlas activity");
    let legacy_activity = legacy
        .context
        .headless_text_gpu_cache_activity_stats()
        .expect("legacy text-atlas activity");
    assert_eq!(optimized_activity.hits, legacy_activity.hits);
    assert_eq!(optimized_activity.misses, legacy_activity.misses);
    assert_eq!(optimized_activity.atlas_releases, expected_stale);
    assert_eq!(legacy_activity.atlas_releases, expected_stale);
    assert_eq!(
        optimized_activity.retained_prepare_cache_clears,
        legacy_activity.retained_prepare_cache_clears
    );
    assert_eq!(
        optimized_activity.retained_prepare_cache_clears,
        usize::from(expected_stale != 0)
    );
    if expect_whole_pages {
        assert!(optimized_activity.whole_pages_released > 0);
        assert_eq!(optimized_activity.whole_page_atlas_releases, expected_stale);
        assert_eq!(optimized_activity.individual_atlas_releases, 0);
    } else {
        assert_eq!(optimized_activity.whole_pages_released, 0);
        assert_eq!(optimized_activity.whole_page_atlas_releases, 0);
        assert_eq!(optimized_activity.individual_atlas_releases, expected_stale);
    }
    assert_eq!(legacy_activity.whole_pages_released, 0);
    assert_eq!(legacy_activity.whole_page_atlas_releases, 0);
    assert_eq!(legacy_activity.individual_atlas_releases, expected_stale);
    assert_eq!(
        optimized.context.headless_gpu_draw_stats(),
        legacy.context.headless_gpu_draw_stats(),
        "{label} draw topology",
    );
    assert_eq!(
        optimized
            .context
            .headless_output_rgba()
            .expect("optimized text-atlas pixels"),
        legacy
            .context
            .headless_output_rgba()
            .expect("legacy text-atlas pixels"),
        "{label} RGBA output",
    );
}

#[cfg(feature = "bench-support")]
fn bench_text_atlas_bulk_stale_release(c: &mut Criterion) {
    const PROFILE_PAIRS: usize = 96;
    const TEXT_COUNT: u64 = 960;

    let Some(mut optimized) =
        TextAtlasEvictionFixture::new(TEXT_COUNT as usize, TextAtlasStalePattern::All, true, false)
    else {
        return;
    };
    let Some(mut legacy) = TextAtlasEvictionFixture::new(
        TEXT_COUNT as usize,
        TextAtlasStalePattern::All,
        false,
        false,
    ) else {
        return;
    };
    validate_text_atlas_eviction_pair(
        "multi-page R8 full stale",
        &mut optimized,
        &mut legacy,
        TEXT_COUNT as usize,
        true,
        |pages| pages > 1,
    );

    let Some(mut single_optimized) =
        TextAtlasEvictionFixture::new(96, TextAtlasStalePattern::All, true, false)
    else {
        return;
    };
    let Some(mut single_legacy) =
        TextAtlasEvictionFixture::new(96, TextAtlasStalePattern::All, false, false)
    else {
        return;
    };
    validate_text_atlas_eviction_pair(
        "single-page R8 full stale",
        &mut single_optimized,
        &mut single_legacy,
        96,
        true,
        |pages| pages == 1,
    );

    let Some(mut rgba_optimized) =
        TextAtlasEvictionFixture::new(96, TextAtlasStalePattern::All, true, true)
    else {
        return;
    };
    let Some(mut rgba_legacy) =
        TextAtlasEvictionFixture::new(96, TextAtlasStalePattern::All, false, true)
    else {
        return;
    };
    validate_text_atlas_eviction_pair(
        "single-page RGBA full stale",
        &mut rgba_optimized,
        &mut rgba_legacy,
        96,
        true,
        |pages| pages == 1,
    );
    let rgba_stats = rgba_optimized
        .context
        .headless_text_gpu_cache_stats()
        .expect("RGBA cache stats");
    assert_eq!(rgba_stats.r8_allocations, 0);
    assert_eq!(rgba_stats.rgba_allocations, 96);

    let Some(mut mixed_optimized) = TextAtlasEvictionFixture::new(
        TEXT_COUNT as usize,
        TextAtlasStalePattern::Alternating,
        true,
        false,
    ) else {
        return;
    };
    let Some(mut mixed_legacy) = TextAtlasEvictionFixture::new(
        TEXT_COUNT as usize,
        TextAtlasStalePattern::Alternating,
        false,
        false,
    ) else {
        return;
    };
    validate_text_atlas_eviction_pair(
        "mixed-page R8 partial stale",
        &mut mixed_optimized,
        &mut mixed_legacy,
        TEXT_COUNT as usize / 2,
        false,
        |pages| pages > 1,
    );

    let Some(mut dedicated_optimized) = TextAtlasEvictionFixture::dedicated(true) else {
        return;
    };
    let Some(mut dedicated_legacy) = TextAtlasEvictionFixture::dedicated(false) else {
        return;
    };
    validate_text_atlas_eviction_pair(
        "dedicated texture replacement",
        &mut dedicated_optimized,
        &mut dedicated_legacy,
        0,
        false,
        |pages| pages == 0,
    );

    let mut optimized_liveness = Vec::with_capacity(PROFILE_PAIRS);
    let mut legacy_liveness = Vec::with_capacity(PROFILE_PAIRS);
    let mut optimized_total = Vec::with_capacity(PROFILE_PAIRS);
    let mut legacy_total = Vec::with_capacity(PROFILE_PAIRS);
    let mut mixed_optimized_liveness = Vec::with_capacity(PROFILE_PAIRS);
    let mut mixed_legacy_liveness = Vec::with_capacity(PROFILE_PAIRS);
    let mut zero_optimized_liveness = Vec::with_capacity(PROFILE_PAIRS);
    let mut zero_legacy_liveness = Vec::with_capacity(PROFILE_PAIRS);
    for pair in 0..PROFILE_PAIRS {
        let (optimized_frame, legacy_frame) = if pair % 2 == 0 {
            (optimized.replacement_frame(), legacy.replacement_frame())
        } else {
            let legacy_frame = legacy.replacement_frame();
            let optimized_frame = optimized.replacement_frame();
            (optimized_frame, legacy_frame)
        };
        optimized_liveness.push(optimized_frame.liveness);
        legacy_liveness.push(legacy_frame.liveness);
        optimized_total.push(optimized_frame.total);
        legacy_total.push(legacy_frame.total);

        let (mixed_optimized_frame, mixed_legacy_frame) = if pair % 2 == 0 {
            (
                mixed_optimized.replacement_frame(),
                mixed_legacy.replacement_frame(),
            )
        } else {
            let legacy_frame = mixed_legacy.replacement_frame();
            let optimized_frame = mixed_optimized.replacement_frame();
            (optimized_frame, legacy_frame)
        };
        mixed_optimized_liveness.push(mixed_optimized_frame.liveness);
        mixed_legacy_liveness.push(mixed_legacy_frame.liveness);

        let (zero_optimized_frame, zero_legacy_frame) = if pair % 2 == 0 {
            (optimized.zero_stale_frame(), legacy.zero_stale_frame())
        } else {
            let legacy_frame = legacy.zero_stale_frame();
            let optimized_frame = optimized.zero_stale_frame();
            (optimized_frame, legacy_frame)
        };
        zero_optimized_liveness.push(zero_optimized_frame.liveness);
        zero_legacy_liveness.push(zero_legacy_frame.liveness);
    }
    let optimized_liveness = duration_p50(optimized_liveness);
    let legacy_liveness = duration_p50(legacy_liveness);
    let optimized_total = duration_p50(optimized_total);
    let legacy_total = duration_p50(legacy_total);
    let mixed_optimized_liveness = duration_p50(mixed_optimized_liveness);
    let mixed_legacy_liveness = duration_p50(mixed_legacy_liveness);
    let zero_optimized_liveness = duration_p50(zero_optimized_liveness);
    let zero_legacy_liveness = duration_p50(zero_legacy_liveness);
    let replacement_ratio =
        optimized_liveness.as_secs_f64() / legacy_liveness.as_secs_f64().max(f64::EPSILON);
    let mixed_ratio = mixed_optimized_liveness.as_secs_f64()
        / mixed_legacy_liveness.as_secs_f64().max(f64::EPSILON);
    let zero_ratio = zero_optimized_liveness.as_secs_f64()
        / zero_legacy_liveness.as_secs_f64().max(f64::EPSILON);
    eprintln!(
        "runtime_text_atlas_bulk_stale_release_p50: pairs={PROFILE_PAIRS} bulk_liveness={optimized_liveness:?} legacy_liveness={legacy_liveness:?} replacement_delta_pct={:.1} bulk_total={optimized_total:?} legacy_total={legacy_total:?} total_delta_pct={:.1} mixed_bulk={mixed_optimized_liveness:?} mixed_legacy={mixed_legacy_liveness:?} mixed_delta_pct={:.1} zero_bulk={zero_optimized_liveness:?} zero_legacy={zero_legacy_liveness:?} zero_delta_pct={:.1}",
        (replacement_ratio - 1.0) * 100.0,
        (optimized_total.as_secs_f64() / legacy_total.as_secs_f64().max(f64::EPSILON) - 1.0)
            * 100.0,
        (mixed_ratio - 1.0) * 100.0,
        (zero_ratio - 1.0) * 100.0,
    );
    assert!(
        replacement_ratio <= 0.95,
        "whole-page text-atlas release missed the 5% replacement gate: ratio={replacement_ratio:.3}",
    );
    assert!(
        mixed_ratio <= 1.05,
        "mixed-page partial release regressed by more than 5%: ratio={mixed_ratio:.3}",
    );
    assert!(
        zero_ratio <= 1.05,
        "zero-stale liveness regressed by more than 5%: ratio={zero_ratio:.3}",
    );

    let mut liveness = c.benchmark_group("runtime_text_atlas_bulk_stale_release_liveness");
    liveness.sample_size(30);
    liveness.throughput(Throughput::Elements(TEXT_COUNT));
    liveness.bench_function("whole_page_bulk/960_replaced_texts", |b| {
        b.iter_custom(|iterations| {
            let mut measured = Duration::ZERO;
            for _ in 0..iterations {
                measured += optimized.replacement_frame().liveness;
            }
            measured
        });
    });
    liveness.bench_function("legacy_individual/960_replaced_texts", |b| {
        b.iter_custom(|iterations| {
            let mut measured = Duration::ZERO;
            for _ in 0..iterations {
                measured += legacy.replacement_frame().liveness;
            }
            measured
        });
    });
    liveness.finish();

    let mut total = c.benchmark_group("runtime_text_atlas_bulk_stale_release_gpu_submit");
    total.sample_size(30);
    total.throughput(Throughput::Elements(TEXT_COUNT));
    total.bench_function("whole_page_bulk/960_replaced_texts", |b| {
        b.iter_custom(|iterations| {
            let mut measured = Duration::ZERO;
            for _ in 0..iterations {
                measured += optimized.replacement_frame().total;
            }
            measured
        });
    });
    total.bench_function("legacy_individual/960_replaced_texts", |b| {
        b.iter_custom(|iterations| {
            let mut measured = Duration::ZERO;
            for _ in 0..iterations {
                measured += legacy.replacement_frame().total;
            }
            measured
        });
    });
    total.finish();
}

#[cfg(feature = "bench-support")]
fn bench_tooltip_effect_gpu(c: &mut Criterion) {
    let flat_tree = tooltip_effect_tree(0.0);
    let legacy_blur_tree = tooltip_effect_tree(6.0);
    let Some(mut flat_context) = ready_context(&flat_tree) else {
        return;
    };
    let Some(mut legacy_blur_context) = ready_context(&legacy_blur_tree) else {
        return;
    };

    let flat_scene = flat_context
        .render_cached_scene_to_headless_gpu(&flat_tree, Instant::now())
        .expect("flat tooltip-like surface GPU validation");
    let legacy_blur_scene = legacy_blur_context
        .render_cached_scene_to_headless_gpu(&legacy_blur_tree, Instant::now())
        .expect("legacy tooltip-like blur GPU validation");
    assert_eq!(
        flat_scene, legacy_blur_scene,
        "tooltip effect fixtures must expose identical geometry, colors, text, and ordinary scene primitives",
    );
    assert_eq!(
        flat_context.headless_gpu_draw_stats(),
        legacy_blur_context.headless_gpu_draw_stats(),
        "the backdrop effect must not change ordinary rect/brush/mesh/sprite draws",
    );
    let flat_prepare = flat_context
        .headless_gpu_prepare_stats()
        .expect("flat tooltip prepare telemetry");
    let legacy_blur_prepare = legacy_blur_context
        .headless_gpu_prepare_stats()
        .expect("legacy tooltip blur prepare telemetry");
    assert_eq!(
        legacy_blur_prepare.total_commands,
        flat_prepare.total_commands + 1,
        "the 6dp control must differ by exactly one BackdropBlur command",
    );
    eprintln!(
        "tooltip_effect_gpu: flat_commands={} legacy_blur_commands={} ordinary_scene={flat_scene:?} ordinary_draws={:?}",
        flat_prepare.total_commands,
        legacy_blur_prepare.total_commands,
        flat_context.headless_gpu_draw_stats(),
    );

    let mut group = c.benchmark_group("tooltip_effect_gpu");
    group.sample_size(30);
    group.throughput(Throughput::Elements(1));
    group.bench_function("flat", |b| {
        b.iter(|| {
            black_box(
                flat_context
                    .render_cached_scene_to_headless_gpu(black_box(&flat_tree), Instant::now())
                    .expect("headless flat tooltip-like frame"),
            );
        });
    });
    group.bench_function("legacy_blur_6dp", |b| {
        b.iter(|| {
            black_box(
                legacy_blur_context
                    .render_cached_scene_to_headless_gpu(
                        black_box(&legacy_blur_tree),
                        Instant::now(),
                    )
                    .expect("headless legacy tooltip-like blur frame"),
            );
        });
    });
    group.finish();
}

#[cfg(feature = "bench-support")]
fn bench_runtime_gpu_pipeline(c: &mut Criterion) {
    bench_text_atlas_bulk_stale_release(c);
    if std::env::var("TGUI_TEXT_ATLAS_BULK_RELEASE_ONLY").as_deref() == Ok("1") {
        return;
    }

    bench_tooltip_effect_gpu(c);
    if std::env::var("TGUI_TOOLTIP_EFFECT_GPU_ONLY").as_deref() == Ok("1") {
        return;
    }

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
    eprintln!(
        "runtime_text_clean_frame_cache: mode={} stats={:?}",
        if std::env::var("TGUI_CLEAN_PREPARED_FRAME_CACHE").as_deref() == Ok("off") {
            "legacy_prepare"
        } else {
            "clean_slot_cache"
        },
        text_context.headless_gpu_clean_frame_cache_stats(),
    );

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
    let optimized_cache = opacity_context
        .headless_text_gpu_cache_stats()
        .expect("optimized opacity cache stats");
    let legacy_cache = legacy_opacity_context
        .headless_text_gpu_cache_stats()
        .expect("legacy opacity cache stats");
    assert_eq!(optimized_cache.cache_entries, legacy_cache.cache_entries);
    assert_eq!(optimized_cache.atlas_pages, legacy_cache.atlas_pages);
    assert_eq!(
        optimized_cache.dedicated_textures,
        legacy_cache.dedicated_textures
    );
    assert_eq!(
        optimized_cache.unique_bind_groups,
        legacy_cache.unique_bind_groups
    );
    assert_eq!(optimized_cache.r8_atlas_pages, 1);
    assert_eq!(optimized_cache.rgba_atlas_pages, 0);
    assert_eq!(legacy_cache.r8_atlas_pages, 0);
    assert_eq!(legacy_cache.rgba_atlas_pages, 1);
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
        batched_mesh.mesh_commands, batched_mesh.mesh_draw_calls, unbatched_mesh.mesh_draw_calls,
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

    let mut scroll_context =
        match RuntimeScrollBenchmarkContext::new(scroll_dashboard(600), viewport()) {
            Ok(context) => context,
            Err(error) => {
                eprintln!("Skipping production scroll benchmark: {error}");
                return;
            }
        };
    let optimized_probe = scroll_context
        .render_scroll_frame(Point::new(0.0, 180.0), false)
        .expect("production pure-scroll probe");
    let optimized_pixels = scroll_context
        .read_output_rgba()
        .expect("production pure-scroll output readback");
    let full_probe = scroll_context
        .render_scroll_frame(Point::new(0.0, 180.0), true)
        .expect("forced full-recollect probe");
    let full_pixels = scroll_context
        .read_output_rgba()
        .expect("forced full-recollect output readback");
    if optimized_pixels != full_pixels {
        let mismatched_bytes = optimized_pixels
            .iter()
            .zip(&full_pixels)
            .filter(|(optimized, full)| optimized != full)
            .count();
        panic!(
            "production pure-scroll output must be pixel-identical to full recollection; mismatched_bytes={mismatched_bytes}"
        );
    }
    assert_eq!(
        optimized_probe.gpu_fast_path_hits + optimized_probe.subtree_patch_hits,
        1,
        "optimized scroll probe must hit exactly one production fast path"
    );
    assert_eq!(full_probe.full_recollects, 1);
    eprintln!(
        "runtime_scroll_renderer_gpu_submit: adapter='{}' backend={} immediates={} optimized_gpu_hits={} optimized_patch_hits={} full_recollects={}",
        scroll_context.adapter_name,
        scroll_context.backend,
        scroll_context.gpu_scroll_supported,
        optimized_probe.gpu_fast_path_hits,
        optimized_probe.subtree_patch_hits,
        full_probe.full_recollects,
    );
    let mut profile_scene = std::time::Duration::ZERO;
    let mut profile_liveness = std::time::Duration::ZERO;
    let mut profile_prepare = std::time::Duration::ZERO;
    let mut profile_encode = std::time::Duration::ZERO;
    let mut profile_submit = std::time::Duration::ZERO;
    let mut profile_wait = std::time::Duration::ZERO;
    let profile_frames = 240_u32;
    let mut profile_offset = 180.0_f32;
    for _ in 0..profile_frames {
        profile_offset = if profile_offset == 0.0 { 180.0 } else { 0.0 };
        let frame = scroll_context
            .render_scroll_frame(Point::new(0.0, profile_offset), false)
            .expect("production pure-scroll profile frame");
        profile_scene += frame.scene_update;
        profile_liveness += frame.renderer_liveness;
        profile_prepare += frame.renderer_prepare_upload;
        profile_encode += frame.renderer_encode;
        profile_submit += frame.queue_submit;
        profile_wait += frame.gpu_wait;
    }
    eprintln!(
        "runtime_scroll_profile_avg: frames={profile_frames} scene={:?} liveness={:?} prepare_upload={:?} encode={:?} submit={:?} gpu_wait={:?}",
        profile_scene / profile_frames,
        profile_liveness / profile_frames,
        profile_prepare / profile_frames,
        profile_encode / profile_frames,
        profile_submit / profile_frames,
        profile_wait / profile_frames,
    );
    let mut full_recollect_context =
        RuntimeScrollBenchmarkContext::new(scroll_dashboard(600), viewport())
            .expect("full-recollect scroll control should use the same headless adapter");
    let full_warmup = full_recollect_context
        .render_scroll_frame(Point::new(0.0, 180.0), true)
        .expect("forced full-recollect control warmup");
    assert_eq!(full_warmup.full_recollects, 1);
    let mut optimized_offset = 180.0_f32;
    let mut full_offset = 180.0_f32;
    let mut scroll = c.benchmark_group("runtime_scroll_renderer_gpu_submit");
    scroll.throughput(Throughput::Elements(600));
    scroll.bench_function("production_pure_scroll/600_rows", |b| {
        b.iter(|| {
            optimized_offset = if optimized_offset == 0.0 { 180.0 } else { 0.0 };
            let stats = scroll_context
                .render_scroll_frame(black_box(Point::new(0.0, optimized_offset)), false)
                .expect("production pure-scroll frame");
            assert_eq!(stats.gpu_fast_path_hits + stats.subtree_patch_hits, 1);
            black_box(stats);
        });
    });
    scroll.bench_function("forced_full_recollect_control/600_rows", |b| {
        b.iter(|| {
            full_offset = if full_offset == 0.0 { 180.0 } else { 0.0 };
            let stats = full_recollect_context
                .render_scroll_frame(black_box(Point::new(0.0, full_offset)), true)
                .expect("forced full-recollect scrolling frame");
            assert_eq!(stats.full_recollects, 1);
            black_box(stats);
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
