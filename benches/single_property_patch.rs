use std::hint::black_box;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};

#[cfg(feature = "bench-support")]
use std::time::Instant;

#[cfg(feature = "bench-support")]
use tgui::core::{dp, Rect};
#[cfg(feature = "bench-support")]
use tgui::layout::{Axis, Insets};
#[cfg(feature = "bench-support")]
use tgui::widgets::{Flex, WidgetBenchmarkContext, WidgetTree};

#[cfg(feature = "bench-support")]
fn viewport() -> Rect {
    Rect::new(0.0, 0.0, 1280.0, 720.0)
}

#[cfg(feature = "bench-support")]
fn build_deep_leaf_tree(depth: usize) -> WidgetTree<()> {
    let mut node = Flex::new(Axis::Vertical)
        .width(dp(720.0))
        .padding(Insets::all(dp(1.0)))
        .height(dp(28.0));

    for _ in 0..depth {
        node = Flex::new(Axis::Vertical)
            .width(dp(720.0))
            .padding(Insets::all(dp(1.0)))
            .child(node);
    }

    WidgetTree::new(
        Flex::new(Axis::Vertical)
            .width(dp(760.0))
            .height(dp(720.0))
            .padding(Insets::all(dp(8.0)))
            .child(node),
    )
}

#[cfg(feature = "bench-support")]
fn bench_deep_leaf_full_recollect(c: &mut Criterion) {
    let mut group = c.benchmark_group("single_property_deep_leaf_full_recollect");

    for depth in [4_usize, 8, 16] {
        let tree = build_deep_leaf_tree(depth);
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
fn bench_deep_leaf_scene_patch(c: &mut Criterion) {
    let mut group = c.benchmark_group("single_property_deep_leaf_scene_patch");

    for depth in [4_usize, 8, 16] {
        let tree = build_deep_leaf_tree(depth);
        group.bench_with_input(BenchmarkId::from_parameter(depth), &depth, |b, _| {
            let mut ctx = WidgetBenchmarkContext::new().with_viewport(viewport());
            let _ = ctx.run_layout_and_scene(&tree, Instant::now());
            b.iter(|| {
                let patched = ctx.patch_single_deep_leaf_scene(black_box(&tree), Instant::now());
                black_box(patched);
            });
        });
    }

    group.finish();
}

#[cfg(not(feature = "bench-support"))]
fn bench_deep_leaf_full_recollect(_c: &mut Criterion) {
    eprintln!("Skipping single_property_patch benchmarks: bench-support feature not enabled");
}

#[cfg(not(feature = "bench-support"))]
fn bench_deep_leaf_scene_patch(_c: &mut Criterion) {}

criterion_group!(
    benches,
    bench_deep_leaf_full_recollect,
    bench_deep_leaf_scene_patch,
);
criterion_main!(benches);
