use std::hint::black_box;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};

#[cfg(feature = "bench-support")]
use std::time::Instant;

#[cfg(feature = "bench-support")]
use tgui::core::{dp, Point, Rect};
#[cfg(feature = "bench-support")]
use tgui::layout::{Axis, Insets, Overflow};
#[cfg(feature = "bench-support")]
use tgui::widgets::{Button, Flex, Text, WidgetBenchmarkContext, WidgetTree};

#[cfg(feature = "bench-support")]
fn viewport() -> Rect {
    Rect::new(0.0, 0.0, 1280.0, 720.0)
}

#[cfg(feature = "bench-support")]
fn build_flat_tree(rows: usize) -> WidgetTree<()> {
    let mut list = Flex::new(Axis::Vertical)
        .width(dp(900.0))
        .gap(dp(4.0))
        .padding(Insets::all(dp(8.0)));

    for row in 0..rows {
        list = list.child(
            Flex::new(Axis::Horizontal)
                .width(dp(860.0))
                .height(dp(36.0))
                .gap(dp(8.0))
                .padding(Insets::symmetric(dp(8.0), dp(4.0)))
                .child(Text::new(format!("Item {row:04}")))
                .child(Text::new(format!("status {}", row % 5)))
                .child(Button::new("Open").size(dp(80.0), dp(28.0))),
        );
    }

    WidgetTree::new(
        Flex::new(Axis::Vertical)
            .width(dp(960.0))
            .height(dp(720.0))
            .padding(Insets::all(dp(16.0)))
            .child(list),
    )
}

#[cfg(feature = "bench-support")]
fn build_nested_tree(depth: usize) -> WidgetTree<()> {
    let mut node = Flex::new(Axis::Vertical)
        .width(dp(120.0))
        .height(dp(32.0))
        .padding(Insets::all(dp(2.0)))
        .child(Button::new("Leaf").size(dp(96.0), dp(28.0)));

    for level in 0..depth {
        node = Flex::new(Axis::Vertical)
            .width(dp(160.0 + level as f32 * 12.0))
            .padding(Insets::all(dp(2.0)))
            .child(Text::new(format!("Level {level}")))
            .child(node);
    }

    WidgetTree::new(
        Flex::new(Axis::Vertical)
            .width(dp(640.0))
            .height(dp(720.0))
            .padding(Insets::all(dp(12.0)))
            .child(node),
    )
}

#[cfg(feature = "bench-support")]
fn build_scroll_tree(rows: usize) -> WidgetTree<()> {
    let mut content = Flex::new(Axis::Vertical)
        .width(dp(1100.0))
        .gap(dp(3.0))
        .padding(Insets::all(dp(8.0)));

    for row in 0..rows {
        content = content.child(
            Flex::new(Axis::Horizontal)
                .width(dp(1060.0))
                .height(dp(30.0))
                .gap(dp(12.0))
                .padding(Insets::symmetric(dp(8.0), dp(3.0)))
                .child(Text::new(format!("row-{row:04}")))
                .child(Text::new(format!("owner {}", row % 17)))
                .child(Text::new(format!(
                    "The quick brown fox jumps over bucket {}",
                    row % 31
                ))),
        );
    }

    WidgetTree::new(
        Flex::new(Axis::Vertical)
            .width(dp(1160.0))
            .height(dp(720.0))
            .padding(Insets::all(dp(12.0)))
            .child(
                Flex::new(Axis::Vertical)
                    .width(dp(1120.0))
                    .height(dp(640.0))
                    .overflow_y(Overflow::Scroll)
                    .child(content),
            ),
    )
}

#[cfg(feature = "bench-support")]
fn bench_element_tree_build(c: &mut Criterion) {
    let mut group = c.benchmark_group("widget_tree_build");

    for rows in [10_usize, 50, 100, 200] {
        group.bench_with_input(BenchmarkId::from_parameter(rows), &rows, |b, &rows| {
            b.iter(|| {
                let tree = build_flat_tree(black_box(rows));
                black_box(tree);
            });
        });
    }

    group.finish();
}

#[cfg(feature = "bench-support")]
fn bench_flat_layout(c: &mut Criterion) {
    let mut group = c.benchmark_group("widget_flat_full_layout");

    for rows in [10_usize, 50, 100, 200, 500] {
        let tree = build_flat_tree(rows);
        group.bench_with_input(BenchmarkId::from_parameter(rows), &rows, |b, _| {
            let mut ctx = WidgetBenchmarkContext::new().with_viewport(viewport());
            b.iter(|| {
                ctx.invalidate_all();
                let stats = ctx.run_layout(black_box(&tree), Instant::now());
                black_box(stats);
            });
        });
    }

    group.finish();
}

#[cfg(feature = "bench-support")]
fn bench_nested_layout(c: &mut Criterion) {
    let mut group = c.benchmark_group("widget_nested_full_layout");

    for depth in [2_usize, 4, 8, 12, 16] {
        let tree = build_nested_tree(depth);
        group.bench_with_input(BenchmarkId::from_parameter(depth), &depth, |b, _| {
            let mut ctx = WidgetBenchmarkContext::new().with_viewport(viewport());
            b.iter(|| {
                ctx.invalidate_all();
                let stats = ctx.run_layout(black_box(&tree), Instant::now());
                black_box(stats);
            });
        });
    }

    group.finish();
}

#[cfg(feature = "bench-support")]
fn bench_full_layout_and_scene(c: &mut Criterion) {
    let mut group = c.benchmark_group("widget_full_layout_and_scene");

    for rows in [50_usize, 200, 500] {
        let tree = build_flat_tree(rows);
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
fn bench_scene_recollect(c: &mut Criterion) {
    let mut group = c.benchmark_group("widget_scene_recollect_cached_layout");

    for rows in [50_usize, 200, 500] {
        let tree = build_flat_tree(rows);
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
fn bench_hit_test(c: &mut Criterion) {
    let mut group = c.benchmark_group("widget_cached_scene_hit_path");

    for rows in [50_usize, 200, 500] {
        let tree = build_flat_tree(rows);
        group.bench_with_input(BenchmarkId::from_parameter(rows), &rows, |b, _| {
            let mut ctx = WidgetBenchmarkContext::new().with_viewport(viewport());
            let _ = ctx.run_layout_and_scene(&tree, Instant::now());
            b.iter(|| {
                let hit_len = ctx.cached_hit_path_len(
                    black_box(&tree),
                    black_box(Point::new(640.0, 360.0)),
                    Instant::now(),
                );
                black_box(hit_len);
            });
        });
    }

    group.finish();
}

#[cfg(feature = "bench-support")]
fn bench_scroll_container_scene_recollect(c: &mut Criterion) {
    let mut group = c.benchmark_group("widget_scroll_scene_recollect_cached_layout");

    for rows in [50_usize, 200, 500] {
        let tree = build_scroll_tree(rows);
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

#[cfg(not(feature = "bench-support"))]
fn bench_element_tree_build(_c: &mut Criterion) {
    eprintln!("Skipping widget_core_layout benchmarks: bench-support feature not enabled");
}

#[cfg(not(feature = "bench-support"))]
fn bench_flat_layout(_c: &mut Criterion) {}

#[cfg(not(feature = "bench-support"))]
fn bench_nested_layout(_c: &mut Criterion) {}

#[cfg(not(feature = "bench-support"))]
fn bench_full_layout_and_scene(_c: &mut Criterion) {}

#[cfg(not(feature = "bench-support"))]
fn bench_scene_recollect(_c: &mut Criterion) {}

#[cfg(not(feature = "bench-support"))]
fn bench_hit_test(_c: &mut Criterion) {}

#[cfg(not(feature = "bench-support"))]
fn bench_scroll_container_scene_recollect(_c: &mut Criterion) {}

criterion_group!(
    benches,
    bench_element_tree_build,
    bench_flat_layout,
    bench_nested_layout,
    bench_full_layout_and_scene,
    bench_scene_recollect,
    bench_hit_test,
    bench_scroll_container_scene_recollect,
);
criterion_main!(benches);
