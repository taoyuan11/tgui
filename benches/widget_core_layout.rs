// Widget 核心布局基准测试
// 覆盖 Element 树构建、taffy 布局计算、scene primitive 收集等热路径

use std::hint::black_box;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};

#[cfg(feature = "bench-support")]
use tgui::widgets::bench_support_ext::*;

#[cfg(feature = "bench-support")]
fn bench_element_tree_build(c: &mut Criterion) {
    let mut group = c.benchmark_group("element_tree_build");

    for depth in [2, 4, 8, 16].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(depth), depth, |b, &depth| {
            b.iter(|| {
                let tree = create_nested_element_tree(black_box(depth));
                black_box(tree);
            });
        });
    }
    group.finish();
}

#[cfg(feature = "bench-support")]
fn bench_flat_layout(c: &mut Criterion) {
    let mut group = c.benchmark_group("flat_layout");

    for count in [10, 50, 100, 200, 500].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(count), count, |b, &count| {
            let tree = create_flat_element_tree(count);

            b.iter(|| {
                let layout_tree = compute_layout(&tree, (800.0, 600.0));
                black_box(layout_tree);
            });
        });
    }
    group.finish();
}

#[cfg(feature = "bench-support")]
fn bench_nested_layout(c: &mut Criterion) {
    let mut group = c.benchmark_group("nested_layout");

    for depth in [2, 4, 8, 12].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(depth), depth, |b, &depth| {
            let tree = create_nested_element_tree(depth);

            b.iter(|| {
                let layout_tree = compute_layout(&tree, (800.0, 600.0));
                black_box(layout_tree);
            });
        });
    }
    group.finish();
}

#[cfg(feature = "bench-support")]
fn bench_flex_layout(c: &mut Criterion) {
    let mut group = c.benchmark_group("flex_layout");

    for children in [5, 10, 20, 50].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(children),
            children,
            |b, &count| {
                let tree = create_flex_container(count);

                b.iter(|| {
                    let layout_tree = compute_layout(&tree, (800.0, 600.0));
                    black_box(layout_tree);
                });
            },
        );
    }
    group.finish();
}

#[cfg(feature = "bench-support")]
fn bench_scene_collection(c: &mut Criterion) {
    let mut group = c.benchmark_group("scene_collection");

    for widget_count in [10, 50, 100, 200, 500].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(widget_count),
            widget_count,
            |b, &count| {
                let tree = create_flat_element_tree(count);
                let layout_tree = compute_layout(&tree, (800.0, 600.0));

                b.iter(|| {
                    let scene = collect_scene_primitives(&layout_tree);
                    black_box(scene);
                });
            },
        );
    }
    group.finish();
}

#[cfg(feature = "bench-support")]
fn bench_hit_test(c: &mut Criterion) {
    let mut group = c.benchmark_group("hit_test");

    for widget_count in [10, 50, 100, 200, 500].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(widget_count),
            widget_count,
            |b, &count| {
                let tree = create_flat_element_tree(count);
                let layout_tree = compute_layout(&tree, (800.0, 600.0));

                b.iter(|| {
                    let hit = perform_hit_test(&layout_tree, (400.0, 300.0));
                    black_box(hit);
                });
            },
        );
    }
    group.finish();
}

#[cfg(feature = "bench-support")]
fn bench_incremental_layout(c: &mut Criterion) {
    let tree = create_flat_element_tree(100);
    let mut layout_tree = compute_layout(&tree, (800.0, 600.0));

    c.bench_function("incremental_layout_single_change", |b| {
        b.iter(|| {
            invalidate_single_widget(&mut layout_tree, 50);
            let result = recompute_layout(&layout_tree, (800.0, 600.0));
            black_box(result);
        });
    });
}

#[cfg(feature = "bench-support")]
fn bench_complex_grid(c: &mut Criterion) {
    let mut group = c.benchmark_group("complex_grid");

    for grid_size in [5, 10, 20, 30].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(grid_size),
            grid_size,
            |b, &size| {
                let tree = create_grid_layout(size, size);

                b.iter(|| {
                    let layout_tree = compute_layout(&tree, (800.0, 600.0));
                    black_box(layout_tree);
                });
            },
        );
    }
    group.finish();
}

#[cfg(feature = "bench-support")]
fn bench_mixed_layout(c: &mut Criterion) {
    // 混合布局：包含 flex、绝对定位、嵌套容器
    let tree = create_mixed_complex_layout();

    c.bench_function("mixed_complex_layout", |b| {
        b.iter(|| {
            let layout_tree = compute_layout(&tree, (1920.0, 1080.0));
            black_box(layout_tree);
        });
    });
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
fn bench_flex_layout(_c: &mut Criterion) {}

#[cfg(not(feature = "bench-support"))]
fn bench_scene_collection(_c: &mut Criterion) {}

#[cfg(not(feature = "bench-support"))]
fn bench_hit_test(_c: &mut Criterion) {}

#[cfg(not(feature = "bench-support"))]
fn bench_incremental_layout(_c: &mut Criterion) {}

#[cfg(not(feature = "bench-support"))]
fn bench_complex_grid(_c: &mut Criterion) {}

#[cfg(not(feature = "bench-support"))]
fn bench_mixed_layout(_c: &mut Criterion) {}

criterion_group!(
    benches,
    bench_element_tree_build,
    bench_flat_layout,
    bench_nested_layout,
    bench_flex_layout,
    bench_scene_collection,
    bench_hit_test,
    bench_incremental_layout,
    bench_complex_grid,
    bench_mixed_layout,
);
criterion_main!(benches);
