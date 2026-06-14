// 场景渲染管线基准测试
// 覆盖场景图构建、scene primitive 拼接、失效管理、顶点上传等热路径

use std::hint::black_box;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};

#[cfg(feature = "bench-support")]
use tgui::widgets::bench_support_ext::*;

#[cfg(feature = "bench-support")]
fn bench_scene_graph_build(c: &mut Criterion) {
    let mut group = c.benchmark_group("scene_graph_build");

    for node_count in [10, 50, 100, 200, 500].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(node_count),
            node_count,
            |b, &count| {
                b.iter(|| {
                    let scene = build_scene_graph(black_box(count));
                    black_box(scene);
                });
            },
        );
    }
    group.finish();
}

#[cfg(feature = "bench-support")]
fn bench_scene_primitive_collection(c: &mut Criterion) {
    let mut group = c.benchmark_group("scene_primitive_collection");

    for primitive_count in [10, 50, 100, 200, 500, 1000].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(primitive_count),
            primitive_count,
            |b, &count| {
                let tree = create_flat_element_tree(count);
                let layout = compute_layout(&tree, (1920.0, 1080.0));

                b.iter(|| {
                    let primitives = collect_scene_primitives(&layout);
                    black_box(primitives);
                });
            },
        );
    }
    group.finish();
}

#[cfg(feature = "bench-support")]
fn bench_scene_splice(c: &mut Criterion) {
    let mut group = c.benchmark_group("scene_splice");

    for chunk_size in [10, 50, 100, 200].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(chunk_size),
            chunk_size,
            |b, &size| {
                let mut scene = build_scene_graph(500);
                let new_chunk = create_scene_chunk(size);

                b.iter(|| {
                    splice_scene_chunk(&mut scene, black_box(250), &new_chunk);
                    black_box(&scene);
                });
            },
        );
    }
    group.finish();
}

#[cfg(feature = "bench-support")]
fn bench_invalidation_tracking(c: &mut Criterion) {
    let mut group = c.benchmark_group("invalidation_tracking");

    for widget_count in [50, 100, 200, 500].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(widget_count),
            widget_count,
            |b, &count| {
                let mut scene = build_scene_graph(count);

                b.iter(|| {
                    invalidate_widget(&mut scene, black_box(count / 2));
                    let invalid_set = collect_invalidated(&scene);
                    black_box(invalid_set);
                });
            },
        );
    }
    group.finish();
}

#[cfg(feature = "bench-support")]
fn bench_incremental_scene_update(c: &mut Criterion) {
    let tree = create_flat_element_tree(100);
    let mut scene = build_full_scene(&tree);

    c.bench_function("incremental_scene_update", |b| {
        b.iter(|| {
            update_single_widget_scene(&mut scene, black_box(50));
            black_box(&scene);
        });
    });
}

#[cfg(feature = "bench-support")]
fn bench_z_order_sorting(c: &mut Criterion) {
    let mut group = c.benchmark_group("z_order_sorting");

    for primitive_count in [50, 100, 200, 500, 1000].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(primitive_count),
            primitive_count,
            |b, &count| {
                let primitives = create_unordered_primitives(count);

                b.iter(|| {
                    let sorted = sort_by_z_order(black_box(&primitives));
                    black_box(sorted);
                });
            },
        );
    }
    group.finish();
}

#[cfg(feature = "bench-support")]
fn bench_scene_diffing(c: &mut Criterion) {
    let mut group = c.benchmark_group("scene_diffing");

    for scene_size in [50, 100, 200, 500].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(scene_size),
            scene_size,
            |b, &size| {
                let old_scene = build_scene_graph(size);
                let new_scene = build_modified_scene_graph(size, size / 10);

                b.iter(|| {
                    let diff = compute_scene_diff(&old_scene, &new_scene);
                    black_box(diff);
                });
            },
        );
    }
    group.finish();
}

#[cfg(feature = "bench-support")]
fn bench_vertex_generation(c: &mut Criterion) {
    let mut group = c.benchmark_group("vertex_generation");

    let primitives_vec = vec![
        ("rectangles_10", create_n_rectangles(10)),
        ("rectangles_50", create_n_rectangles(50)),
        ("rectangles_100", create_n_rectangles(100)),
        ("rounded_rects_50", create_n_rounded_rects(50)),
        ("circles_50", create_n_circles(50)),
        ("text_10", create_n_text_primitives(10)),
    ];

    for (name, primitives) in primitives_vec {
        group.bench_with_input(
            BenchmarkId::new("generate", name),
            &primitives,
            |b, prims| {
                b.iter(|| {
                    let vertices = generate_vertices(black_box(prims));
                    black_box(vertices);
                });
            },
        );
    }
    group.finish();
}

#[cfg(feature = "bench-support")]
fn bench_clipping(c: &mut Criterion) {
    let mut group = c.benchmark_group("clipping");

    for primitive_count in [50, 100, 200, 500].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(primitive_count),
            primitive_count,
            |b, &count| {
                let primitives = create_unordered_primitives(count);
                let clip_rect = create_rect(100.0, 100.0, 700.0, 500.0);

                b.iter(|| {
                    let clipped = apply_clipping(&primitives, black_box(&clip_rect));
                    black_box(clipped);
                });
            },
        );
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
fn bench_scene_splice(_c: &mut Criterion) {}

#[cfg(not(feature = "bench-support"))]
fn bench_invalidation_tracking(_c: &mut Criterion) {}

#[cfg(not(feature = "bench-support"))]
fn bench_incremental_scene_update(_c: &mut Criterion) {}

#[cfg(not(feature = "bench-support"))]
fn bench_z_order_sorting(_c: &mut Criterion) {}

#[cfg(not(feature = "bench-support"))]
fn bench_scene_diffing(_c: &mut Criterion) {}

#[cfg(not(feature = "bench-support"))]
fn bench_vertex_generation(_c: &mut Criterion) {}

#[cfg(not(feature = "bench-support"))]
fn bench_clipping(_c: &mut Criterion) {}

criterion_group!(
    benches,
    bench_scene_graph_build,
    bench_scene_primitive_collection,
    bench_scene_splice,
    bench_invalidation_tracking,
    bench_incremental_scene_update,
    bench_z_order_sorting,
    bench_scene_diffing,
    bench_vertex_generation,
    bench_clipping,
);
criterion_main!(benches);
