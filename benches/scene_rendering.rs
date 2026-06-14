use std::hint::black_box;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};

#[cfg(feature = "bench-support")]
use std::time::Instant;

#[cfg(feature = "bench-support")]
use tgui::core::{dp, Point, Rect};
#[cfg(feature = "bench-support")]
use tgui::layout::{Axis, Insets};
#[cfg(feature = "bench-support")]
use tgui::widgets::{Button, Flex, Text, WidgetBenchmarkContext, WidgetTree};

#[cfg(feature = "bench-support")]
fn viewport() -> Rect {
    Rect::new(0.0, 0.0, 1280.0, 720.0)
}

#[cfg(feature = "bench-support")]
fn build_scene_tree(rows: usize) -> WidgetTree<()> {
    let mut body = Flex::new(Axis::Vertical)
        .width(dp(1180.0))
        .gap(dp(4.0))
        .padding(Insets::all(dp(10.0)));

    for row in 0..rows {
        body = body.child(
            Flex::new(Axis::Horizontal)
                .width(dp(1140.0))
                .height(dp(34.0))
                .gap(dp(10.0))
                .padding(Insets::symmetric(dp(8.0), dp(4.0)))
                .child(Text::new(format!("metric-{row:04}")))
                .child(Text::new(format!("p95={}ms", row % 97)))
                .child(Text::new(format!("owner {}", row % 23)))
                .child(Button::new("Drill in").size(dp(96.0), dp(26.0))),
        );
    }

    WidgetTree::new(
        Flex::new(Axis::Vertical)
            .width(dp(1280.0))
            .height(dp(720.0))
            .padding(Insets::all(dp(16.0)))
            .gap(dp(8.0))
            .child(
                Flex::new(Axis::Horizontal)
                    .width(dp(1180.0))
                    .height(dp(44.0))
                    .gap(dp(14.0))
                    .padding(Insets::symmetric(dp(10.0), dp(6.0)))
                    .child(Text::new("Scene metrics"))
                    .child(Button::new("Refresh").size(dp(92.0), dp(28.0))),
            )
            .child(body),
    )
}

#[cfg(feature = "bench-support")]
fn build_nested_scene_tree(depth: usize) -> WidgetTree<()> {
    let mut node = Flex::new(Axis::Horizontal)
        .width(dp(460.0))
        .height(dp(36.0))
        .gap(dp(8.0))
        .padding(Insets::symmetric(dp(8.0), dp(4.0)))
        .child(Text::new("leaf scene node"))
        .child(Button::new("Run").size(dp(72.0), dp(26.0)));

    for level in 0..depth {
        node = Flex::new(Axis::Vertical)
            .width(dp(520.0 + level as f32 * 8.0))
            .gap(dp(3.0))
            .padding(Insets::all(dp(3.0)))
            .child(Text::new(format!("ancestor-{level}")))
            .child(node);
    }

    WidgetTree::new(
        Flex::new(Axis::Vertical)
            .width(dp(900.0))
            .height(dp(720.0))
            .padding(Insets::all(dp(16.0)))
            .child(node),
    )
}

#[cfg(feature = "bench-support")]
fn build_text_heavy_tree(paragraphs: usize) -> WidgetTree<()> {
    let mut body = Flex::new(Axis::Vertical)
        .width(dp(1000.0))
        .gap(dp(6.0))
        .padding(Insets::all(dp(10.0)));

    for index in 0..paragraphs {
        body = body.child(Text::new(format!(
            "Paragraph {index}: real scene collection should resolve text style, shape glyphs, \
             and emit text primitives while reusing the retained layout tree."
        )));
    }

    WidgetTree::new(
        Flex::new(Axis::Vertical)
            .width(dp(1080.0))
            .height(dp(720.0))
            .padding(Insets::all(dp(16.0)))
            .child(body),
    )
}

#[cfg(feature = "bench-support")]
fn bench_scene_graph_build(c: &mut Criterion) {
    let mut group = c.benchmark_group("scene_full_layout_and_collect");

    for rows in [25_usize, 100, 250, 500] {
        let tree = build_scene_tree(rows);
        group.bench_with_input(BenchmarkId::from_parameter(rows), &rows, |b, _| {
            let mut ctx = WidgetBenchmarkContext::new().with_viewport(viewport());
            b.iter(|| {
                ctx.invalidate_all();
                let stats = ctx.run_layout_and_scene(black_box(&tree), Instant::now());
                black_box(stats);
            });
        });
    }

    group.finish();
}

#[cfg(feature = "bench-support")]
fn bench_scene_primitive_collection(c: &mut Criterion) {
    let mut group = c.benchmark_group("scene_recollect_cached_layout");

    for rows in [25_usize, 100, 250, 500] {
        let tree = build_scene_tree(rows);
        group.bench_with_input(BenchmarkId::from_parameter(rows), &rows, |b, _| {
            let mut ctx = WidgetBenchmarkContext::new().with_viewport(viewport());
            let _ = ctx.run_layout_and_scene(&tree, Instant::now());
            b.iter(|| {
                let stats = ctx.recollect_scene_only(black_box(&tree), Instant::now());
                black_box(stats);
            });
        });
    }

    group.finish();
}

#[cfg(feature = "bench-support")]
fn bench_nested_scene_recollect(c: &mut Criterion) {
    let mut group = c.benchmark_group("scene_nested_recollect_cached_layout");

    for depth in [2_usize, 4, 8, 12, 16] {
        let tree = build_nested_scene_tree(depth);
        group.bench_with_input(BenchmarkId::from_parameter(depth), &depth, |b, _| {
            let mut ctx = WidgetBenchmarkContext::new().with_viewport(viewport());
            let _ = ctx.run_layout_and_scene(&tree, Instant::now());
            b.iter(|| {
                let stats = ctx.recollect_scene_only(black_box(&tree), Instant::now());
                black_box(stats);
            });
        });
    }

    group.finish();
}

#[cfg(feature = "bench-support")]
fn bench_text_scene_recollect(c: &mut Criterion) {
    let mut group = c.benchmark_group("scene_text_heavy_recollect_cached_layout");

    for paragraphs in [25_usize, 100, 250] {
        let tree = build_text_heavy_tree(paragraphs);
        group.bench_with_input(
            BenchmarkId::from_parameter(paragraphs),
            &paragraphs,
            |b, _| {
                let mut ctx = WidgetBenchmarkContext::new().with_viewport(viewport());
                let _ = ctx.run_layout_and_scene(&tree, Instant::now());
                b.iter(|| {
                    let stats = ctx.recollect_scene_only(black_box(&tree), Instant::now());
                    black_box(stats);
                });
            },
        );
    }

    group.finish();
}

#[cfg(feature = "bench-support")]
fn bench_hit_metadata_scan(c: &mut Criterion) {
    let mut group = c.benchmark_group("scene_cached_hit_metadata_scan");

    for rows in [25_usize, 100, 250, 500] {
        let tree = build_scene_tree(rows);
        group.bench_with_input(BenchmarkId::from_parameter(rows), &rows, |b, _| {
            let mut ctx = WidgetBenchmarkContext::new().with_viewport(viewport());
            let _ = ctx.run_layout_and_scene(&tree, Instant::now());
            b.iter(|| {
                let hit_len = ctx.cached_hit_path_len(
                    black_box(&tree),
                    black_box(Point::new(720.0, 360.0)),
                    Instant::now(),
                );
                black_box(hit_len);
            });
        });
    }

    group.finish();
}

#[cfg(not(feature = "bench-support"))]
fn bench_scene_graph_build(_c: &mut Criterion) {
    eprintln!("Skipping scene_rendering benchmarks: bench-support feature not enabled");
}

#[cfg(not(feature = "bench-support"))]
fn bench_scene_primitive_collection(_c: &mut Criterion) {}

#[cfg(not(feature = "bench-support"))]
fn bench_nested_scene_recollect(_c: &mut Criterion) {}

#[cfg(not(feature = "bench-support"))]
fn bench_text_scene_recollect(_c: &mut Criterion) {}

#[cfg(not(feature = "bench-support"))]
fn bench_hit_metadata_scan(_c: &mut Criterion) {}

criterion_group!(
    benches,
    bench_scene_graph_build,
    bench_scene_primitive_collection,
    bench_nested_scene_recollect,
    bench_text_scene_recollect,
    bench_hit_metadata_scan,
);
criterion_main!(benches);
