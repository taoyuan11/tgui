use std::hint::black_box;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use tgui::canvas::{CanvasRecorder, CanvasScene, CanvasSceneQueryOptions, PathBuilder};
use tgui::core::{dp, Color, Point, Rect};
use tgui::media::MediaSource;

const IMAGE_BYTES: &[u8] = b"tgui-benchmark-image-bytes";

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

fn build_recorder_polyline(segments: usize) -> CanvasScene {
    CanvasRecorder::build(|canvas| {
        canvas.set_stroke(tgui::canvas::CanvasStroke::new(
            dp(1.0),
            Color::rgb(12, 32, 56),
        ));
        canvas.begin_path().move_to(0.0, 0.0);
        for index in 0..segments {
            let x = index as f32 * 2.0;
            let y = (index % 31) as f32 * 3.0;
            canvas.line_to(x, y);
        }
        canvas.stroke();
    })
}

fn build_canvas_image_scene(items: usize) -> CanvasScene {
    let source = MediaSource::bytes(IMAGE_BYTES);
    CanvasRecorder::build(|canvas| {
        for index in 0..items {
            let col = (index % 20) as f32;
            let row = (index / 20) as f32;
            let x = 8.0 + col * 40.0;
            let y = 8.0 + row * 34.0;
            canvas.draw_image(Rect::new(x, y, 32.0, 26.0), source.clone());
        }
    })
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

fn bench_canvas_scene_query_first_hit(c: &mut Criterion) {
    let mut group = c.benchmark_group("canvas_scene_query_first_hit");

    for items in [50_usize, 200, 1000] {
        let scene = build_canvas_scene(items);
        group.bench_with_input(BenchmarkId::from_parameter(items), &items, |b, _| {
            b.iter(|| {
                let hit = scene.query_point(black_box(Point::new(160.0, 120.0)));
                black_box(hit);
            });
        });
    }

    group.finish();
}

fn bench_canvas_scene_query_geometry_only(c: &mut Criterion) {
    let mut group = c.benchmark_group("canvas_scene_query_point_all_geometry_only");

    for items in [50_usize, 200, 1000] {
        let scene = build_canvas_scene(items);
        let options = CanvasSceneQueryOptions::new().without_text_hits();
        group.bench_with_input(BenchmarkId::from_parameter(items), &items, |b, _| {
            b.iter(|| {
                let hits =
                    scene.query_point_all_with(&options, black_box(Point::new(160.0, 120.0)));
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

fn bench_canvas_stable_export(c: &mut Criterion) {
    let mut group = c.benchmark_group("canvas_stable_export_json");

    for items in [50_usize, 200, 1000] {
        let scene = build_canvas_scene(items);
        group.bench_with_input(BenchmarkId::from_parameter(items), &items, |b, _| {
            b.iter(|| {
                let json = scene.export_json();
                black_box(json);
            });
        });
    }

    group.finish();
}

fn bench_canvas_image_stable_export(c: &mut Criterion) {
    let mut group = c.benchmark_group("canvas_image_stable_export_json");

    for items in [50_usize, 200, 1000] {
        let scene = build_canvas_image_scene(items);
        group.bench_with_input(BenchmarkId::from_parameter(items), &items, |b, _| {
            b.iter(|| {
                let json = scene.export_json();
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

fn bench_canvas_recorder_polyline(c: &mut Criterion) {
    let mut group = c.benchmark_group("canvas_recorder_polyline_path");

    for segments in [16_usize, 64, 256, 1024] {
        group.bench_with_input(
            BenchmarkId::from_parameter(segments),
            &segments,
            |b, &segments| {
                b.iter(|| {
                    let scene = build_recorder_polyline(black_box(segments));
                    black_box(scene);
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
    bench_canvas_scene_query_first_hit,
    bench_canvas_scene_query_geometry_only,
    bench_canvas_debug_export,
    bench_canvas_stable_export,
    bench_canvas_image_stable_export,
    bench_canvas_path_builder,
    bench_canvas_recorder_polyline,
);
criterion_main!(benches);
