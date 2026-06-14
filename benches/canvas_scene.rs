use std::hint::black_box;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use tgui::canvas::{CanvasRecorder, CanvasScene, PathBuilder};
use tgui::core::{dp, Color, Point, Rect};

fn build_canvas_scene(items: usize) -> CanvasScene {
    CanvasRecorder::build(|canvas| {
        canvas.set_fill(Color::rgb(32, 96, 160));
        canvas.set_stroke(tgui::canvas::CanvasStroke::new(
            dp(1.0),
            Color::rgb(12, 32, 56),
        ));

        for index in 0..items {
            let col = (index % 20) as f32;
            let row = (index / 20) as f32;
            let x = 8.0 + col * 38.0;
            let y = 8.0 + row * 32.0;

            if index % 7 == 0 {
                canvas.fill_circle(x + 14.0, y + 14.0, 12.0);
            } else if index % 5 == 0 {
                canvas.draw_text(Rect::new(x, y, 84.0, 24.0), format!("node-{index:04}"));
            } else {
                canvas.fill_round_rect(x, y, 32.0, 24.0, 4.0);
            }
        }
    })
}

fn build_complex_path(segments: usize) -> PathBuilder {
    let mut path = PathBuilder::new().move_to(0.0, 0.0);
    for index in 0..segments {
        let x = index as f32 * 3.0;
        let y = (index % 17) as f32 * 2.0;
        path = path.cubic_to(x + 1.0, y + 4.0, x + 2.0, y - 3.0, x + 3.0, y);
    }
    path.close()
}

fn bench_canvas_scene_build(c: &mut Criterion) {
    let mut group = c.benchmark_group("canvas_scene_build");

    for items in [50_usize, 200, 1000] {
        group.bench_with_input(BenchmarkId::from_parameter(items), &items, |b, &items| {
            b.iter(|| {
                let scene = build_canvas_scene(black_box(items));
                black_box(scene);
            });
        });
    }

    group.finish();
}

fn bench_canvas_scene_query(c: &mut Criterion) {
    let mut group = c.benchmark_group("canvas_scene_query_point_all");

    for items in [50_usize, 200, 1000] {
        let scene = build_canvas_scene(items);
        group.bench_with_input(BenchmarkId::from_parameter(items), &items, |b, _| {
            b.iter(|| {
                let hits = scene.query_point_all(black_box(Point::new(160.0, 120.0)));
                black_box(hits);
            });
        });
    }

    group.finish();
}

fn bench_canvas_debug_export(c: &mut Criterion) {
    let mut group = c.benchmark_group("canvas_debug_export_json");

    for items in [50_usize, 200, 1000] {
        let scene = build_canvas_scene(items);
        group.bench_with_input(BenchmarkId::from_parameter(items), &items, |b, _| {
            b.iter(|| {
                let json = scene.export_debug_json();
                black_box(json);
            });
        });
    }

    group.finish();
}

fn bench_canvas_path_builder(c: &mut Criterion) {
    let mut group = c.benchmark_group("canvas_path_builder_cubic");

    for segments in [16_usize, 64, 256, 1024] {
        group.bench_with_input(
            BenchmarkId::from_parameter(segments),
            &segments,
            |b, &segments| {
                b.iter(|| {
                    let path = build_complex_path(black_box(segments));
                    black_box(path);
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_canvas_scene_build,
    bench_canvas_scene_query,
    bench_canvas_debug_export,
    bench_canvas_path_builder,
);
criterion_main!(benches);
