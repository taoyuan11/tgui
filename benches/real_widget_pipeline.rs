use std::hint::black_box;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};

#[cfg(feature = "bench-support")]
use std::time::Instant;

#[cfg(feature = "bench-support")]
use tgui::core::{dp, Rect};
#[cfg(feature = "bench-support")]
use tgui::layout::{Axis, Insets, Overflow};
#[cfg(feature = "bench-support")]
use tgui::widgets::{Flex, Text, WidgetBenchmarkContext, WidgetTree};

#[cfg(feature = "bench-support")]
fn bench_viewport() -> Rect {
    Rect::new(0.0, 0.0, 1280.0, 720.0)
}

#[cfg(feature = "bench-support")]
fn build_dashboard_tree(rows: usize) -> WidgetTree<()> {
    let mut list = Flex::new(Axis::Vertical)
        .width(dp(1240.0))
        .gap(dp(2.0))
        .padding(Insets::all(dp(8.0)));

    for row in 0..rows {
        let item = Flex::new(Axis::Horizontal)
            .width(dp(1224.0))
            .height(dp(32.0))
            .gap(dp(12.0))
            .padding(Insets::symmetric(dp(8.0), dp(4.0)))
            .child(Text::new(format!("row-{row:04}")))
            .child(Text::new(format!("status {}", row % 7)))
            .child(Text::new(format!("owner {}", row % 19)))
            .child(Text::new(format!(
                "The quick brown fox jumps over metric bucket {}.",
                row % 31
            )));
        list = list.child(item);
    }

    WidgetTree::new(
        Flex::new(Axis::Vertical)
            .width(dp(1280.0))
            .height(dp(720.0))
            .padding(Insets::all(dp(12.0)))
            .gap(dp(8.0))
            .child(
                Flex::new(Axis::Horizontal)
                    .width(dp(1240.0))
                    .height(dp(48.0))
                    .gap(dp(16.0))
                    .padding(Insets::symmetric(dp(12.0), dp(8.0)))
                    .child(Text::new("Dashboard"))
                    .child(Text::new("Throughput"))
                    .child(Text::new("Latency")),
            )
            .child(
                Flex::new(Axis::Vertical)
                    .width(dp(1240.0))
                    .height(dp(620.0))
                    .overflow_y(Overflow::Scroll)
                    .child(list),
            ),
    )
}

#[cfg(feature = "bench-support")]
fn build_text_heavy_tree(paragraphs: usize) -> WidgetTree<()> {
    let mut body = Flex::new(Axis::Vertical)
        .width(dp(960.0))
        .gap(dp(6.0))
        .padding(Insets::all(dp(10.0)));

    for index in 0..paragraphs {
        body = body.child(Text::new(format!(
            "Paragraph {index}: retained scene collection should shape, cache, and emit text \
             primitives without forcing a fresh layout pass."
        )));
    }

    WidgetTree::new(
        Flex::new(Axis::Vertical)
            .width(dp(1000.0))
            .height(dp(700.0))
            .padding(Insets::all(dp(16.0)))
            .child(body),
    )
}

#[cfg(feature = "bench-support")]
fn bench_full_layout_and_scene(c: &mut Criterion) {
    let mut group = c.benchmark_group("real_widget_full_layout_and_scene");

    for rows in [50_usize, 200, 500] {
        let tree = build_dashboard_tree(rows);
        group.bench_with_input(BenchmarkId::from_parameter(rows), &rows, |b, _| {
            let mut ctx = WidgetBenchmarkContext::new().with_viewport(bench_viewport());
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
fn bench_scene_recollect_only(c: &mut Criterion) {
    let mut group = c.benchmark_group("real_widget_scene_recollect_only");

    for rows in [50_usize, 200, 500] {
        let tree = build_dashboard_tree(rows);
        group.bench_with_input(BenchmarkId::from_parameter(rows), &rows, |b, _| {
            let mut ctx = WidgetBenchmarkContext::new().with_viewport(bench_viewport());
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
fn bench_text_heavy_scene_recollect(c: &mut Criterion) {
    let mut group = c.benchmark_group("real_widget_text_heavy_scene_recollect");

    for paragraphs in [25_usize, 100, 250] {
        let tree = build_text_heavy_tree(paragraphs);
        group.bench_with_input(
            BenchmarkId::from_parameter(paragraphs),
            &paragraphs,
            |b, _| {
                let mut ctx = WidgetBenchmarkContext::new().with_viewport(bench_viewport());
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

#[cfg(not(feature = "bench-support"))]
fn bench_full_layout_and_scene(_c: &mut Criterion) {
    eprintln!("Skipping real_widget_pipeline benchmarks: bench-support feature not enabled");
}

#[cfg(not(feature = "bench-support"))]
fn bench_scene_recollect_only(_c: &mut Criterion) {}

#[cfg(not(feature = "bench-support"))]
fn bench_text_heavy_scene_recollect(_c: &mut Criterion) {}

criterion_group!(
    benches,
    bench_full_layout_and_scene,
    bench_scene_recollect_only,
    bench_text_heavy_scene_recollect,
);
criterion_main!(benches);
