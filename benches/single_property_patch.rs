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
fn build_wide_leaf_tree(rows: usize) -> WidgetTree<()> {
    let mut body = Flex::new(Axis::Vertical)
        .width(dp(760.0))
        .padding(Insets::all(dp(4.0)));

    for _ in 0..rows {
        body = body.child(
            Flex::new(Axis::Horizontal)
                .width(dp(720.0))
                .height(dp(20.0))
                .padding(Insets::all(dp(1.0))),
        );
    }

    WidgetTree::new(
        Flex::new(Axis::Vertical)
            .width(dp(800.0))
            .height(dp(720.0))
            .padding(Insets::all(dp(8.0)))
            .child(body),
    )
}

#[cfg(feature = "bench-support")]
fn build_multi_root_tree(branches: usize, depth: usize) -> WidgetTree<()> {
    let mut body = Flex::new(Axis::Vertical).width(dp(760.0));
    for _ in 0..branches {
        let mut branch = Flex::new(Axis::Vertical).height(dp(12.0));
        for _ in 0..depth {
            branch = Flex::new(Axis::Vertical).child(branch);
        }
        body = body.child(branch);
    }
    WidgetTree::new(body)
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
        group.bench_with_input(BenchmarkId::new("reused", depth), &depth, |b, _| {
            let mut ctx = WidgetBenchmarkContext::new().with_viewport(viewport());
            let _ = ctx.run_layout_and_scene(&tree, Instant::now());
            b.iter(|| {
                let patched = ctx.patch_single_deep_leaf_scene(black_box(&tree), Instant::now());
                black_box(patched);
            });
        });

        group.bench_with_input(BenchmarkId::new("legacy_clone", depth), &depth, |b, _| {
            let mut ctx = WidgetBenchmarkContext::new().with_viewport(viewport());
            let _ = ctx.run_layout_and_scene(&tree, Instant::now());
            b.iter(|| {
                let patched = ctx.patch_single_deep_leaf_scene_legacy_recompose(
                    black_box(&tree),
                    Instant::now(),
                );
                black_box(patched);
            });
        });
    }

    group.finish();
}

#[cfg(feature = "bench-support")]
fn bench_wide_leaf_update_paths(c: &mut Criterion) {
    let mut group = c.benchmark_group("single_property_wide_leaf_update_paths");

    for rows in [100_usize, 1_000, 5_000] {
        let tree = build_wide_leaf_tree(rows);

        group.bench_with_input(BenchmarkId::new("full_recollect", rows), &rows, |b, _| {
            let mut ctx = WidgetBenchmarkContext::new().with_viewport(viewport());
            let _ = ctx.run_layout_and_scene(&tree, Instant::now());
            b.iter(|| {
                let stats = ctx.recollect_scene_only(black_box(&tree), Instant::now());
                black_box(stats);
            });
        });

        group.bench_with_input(BenchmarkId::new("scene_patch", rows), &rows, |b, _| {
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

#[cfg(feature = "bench-support")]
fn bench_multi_root_scene_patch(c: &mut Criterion) {
    let mut group = c.benchmark_group("single_property_multi_root_scene_patch");
    for roots in [2_usize, 8, 32] {
        let tree = build_multi_root_tree(roots, 6);
        group.bench_with_input(BenchmarkId::new("reused", roots), &roots, |b, roots| {
            let mut ctx = WidgetBenchmarkContext::new().with_viewport(viewport());
            let _ = ctx.run_layout_and_scene(&tree, Instant::now());
            b.iter(|| {
                let patched =
                    ctx.patch_multiple_deep_leaf_scenes(black_box(&tree), *roots, Instant::now());
                assert!(
                    patched,
                    "multi-root reused patch must not benchmark fallback"
                );
                black_box(patched);
            });
        });
        group.bench_with_input(
            BenchmarkId::new("legacy_clone", roots),
            &roots,
            |b, roots| {
                let mut ctx = WidgetBenchmarkContext::new().with_viewport(viewport());
                let _ = ctx.run_layout_and_scene(&tree, Instant::now());
                b.iter(|| {
                    let patched = ctx.patch_multiple_deep_leaf_scenes_legacy_recompose(
                        black_box(&tree),
                        *roots,
                        Instant::now(),
                    );
                    assert!(
                        patched,
                        "multi-root legacy patch must not benchmark fallback"
                    );
                    black_box(patched);
                });
            },
        );
    }
    group.finish();
}

#[cfg(not(feature = "bench-support"))]
fn bench_deep_leaf_full_recollect(_c: &mut Criterion) {
    eprintln!("Skipping single_property_patch benchmarks: bench-support feature not enabled");
}

#[cfg(not(feature = "bench-support"))]
fn bench_deep_leaf_scene_patch(_c: &mut Criterion) {}

#[cfg(not(feature = "bench-support"))]
fn bench_wide_leaf_update_paths(_c: &mut Criterion) {}

#[cfg(not(feature = "bench-support"))]
fn bench_multi_root_scene_patch(_c: &mut Criterion) {}

criterion_group!(
    benches,
    bench_deep_leaf_full_recollect,
    bench_deep_leaf_scene_patch,
    bench_wide_leaf_update_paths,
    bench_multi_root_scene_patch,
);
criterion_main!(benches);
