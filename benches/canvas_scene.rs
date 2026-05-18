use std::hint::black_box;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use tgui::canvas::{
    CanvasGroup, CanvasGroupMode, CanvasGroupShape, CanvasItem, CanvasPath, CanvasScene,
    CanvasSceneQueryOptions, CanvasText, PathBuilder,
};
use tgui::core::{dp, Point, Rect};

fn build_path_item(id: u64, x: f32, y: f32) -> CanvasItem {
    let path = PathBuilder::new()
        .rect(dp(x), dp(y), dp(64.0), dp(48.0))
        .move_to(dp(x + 8.0), dp(y + 8.0))
        .line_to(dp(x + 56.0), dp(y + 8.0))
        .line_to(dp(x + 56.0), dp(y + 40.0))
        .line_to(dp(x + 8.0), dp(y + 40.0))
        .close();
    CanvasPath::new(id, path).into()
}

fn build_text_item(id: u64, x: f32, y: f32, content: &str) -> CanvasItem {
    CanvasText::new(
        id,
        Rect::new(dp(x), dp(y), dp(120.0), dp(20.0)),
        content.to_string(),
    )
    .into()
}

fn build_grid_scene(rows: usize, cols: usize) -> CanvasScene {
    let mut items: Vec<CanvasItem> = Vec::with_capacity(rows * cols * 2);
    let cell = 80.0_f32;
    let mut id_counter = 1u64;
    for row in 0..rows {
        for col in 0..cols {
            let x = col as f32 * cell;
            let y = row as f32 * cell;
            items.push(build_path_item(id_counter, x, y));
            id_counter += 1;
            items.push(build_text_item(
                id_counter,
                x + 4.0,
                y + 52.0,
                "scene benchmark",
            ));
            id_counter += 1;
        }
    }
    CanvasScene::from_items(items)
}

fn build_grouped_scene(group_count: usize, items_per_group: usize) -> CanvasScene {
    let mut groups: Vec<CanvasItem> = Vec::with_capacity(group_count);
    let mut id_counter = 1u64;
    for group_index in 0..group_count {
        let mut children: Vec<CanvasItem> = Vec::with_capacity(items_per_group);
        for slot in 0..items_per_group {
            let x = (group_index as f32) * 60.0 + slot as f32 * 6.0;
            let y = (group_index as f32) * 8.0 + slot as f32 * 4.0;
            children.push(build_path_item(id_counter, x, y));
            id_counter += 1;
        }
        let clip_path = PathBuilder::new().rect(
            dp(group_index as f32 * 60.0),
            dp(group_index as f32 * 8.0),
            dp(220.0),
            dp(140.0),
        );
        let group = CanvasGroup::new(
            id_counter,
            CanvasGroupMode::Clip,
            CanvasGroupShape::path(clip_path),
            children,
        );
        id_counter += 1;
        groups.push(group.into());
    }
    CanvasScene::from_items(groups)
}

fn bench_scene_bounds(c: &mut Criterion) {
    let mut group = c.benchmark_group("canvas_scene_bounds");

    for (label, scene) in [
        ("grid_8x8", build_grid_scene(8, 8)),
        ("grid_24x24", build_grid_scene(24, 24)),
        ("grouped_16x32", build_grouped_scene(16, 32)),
    ] {
        group.bench_with_input(BenchmarkId::new("bounds", label), &label, |b, _| {
            b.iter(|| black_box(scene.bounds()));
        });
    }

    group.finish();
}

fn bench_scene_query_point(c: &mut Criterion) {
    let mut group = c.benchmark_group("canvas_scene_query");

    let scenes = [
        ("grid_8x8", build_grid_scene(8, 8)),
        ("grid_24x24", build_grid_scene(24, 24)),
        ("grouped_16x32", build_grouped_scene(16, 32)),
    ];

    let probe_points = [
        Point::new(dp(0.0), dp(0.0)),
        Point::new(dp(160.0), dp(120.0)),
        Point::new(dp(640.0), dp(440.0)),
        Point::new(dp(1100.0), dp(800.0)),
        Point::new(dp(-10.0), dp(-10.0)),
    ];

    for (label, scene) in &scenes {
        let options = CanvasSceneQueryOptions::default();

        group.bench_with_input(BenchmarkId::new("query_point", label), label, |b, _| {
            let mut tick = 0usize;
            b.iter(|| {
                let probe = probe_points[tick % probe_points.len()];
                tick = tick.wrapping_add(1);
                black_box(scene.query_point_with(&options, black_box(probe)))
            });
        });

        group.bench_with_input(BenchmarkId::new("query_point_all", label), label, |b, _| {
            let mut tick = 0usize;
            b.iter(|| {
                let probe = probe_points[tick % probe_points.len()];
                tick = tick.wrapping_add(1);
                let hits = scene.query_point_all_with(&options, black_box(probe));
                black_box(hits.len())
            });
        });
    }

    group.finish();
}

fn bench_scene_visit(c: &mut Criterion) {
    let mut group = c.benchmark_group("canvas_scene_visit");
    let scene = build_grouped_scene(16, 32);

    group.bench_function("visit_all", |b| {
        b.iter(|| {
            let mut count = 0usize;
            scene.visit(|visit| {
                count += 1 + visit.depth;
            });
            black_box(count)
        });
    });

    group.bench_function("contains_id_lookup", |b| {
        b.iter(|| {
            let mut found = 0usize;
            for id_value in [1u64, 32, 96, 200, 480, 960] {
                if scene.contains_id(id_value.into()) {
                    found += 1;
                }
            }
            black_box(found)
        });
    });

    group.finish();
}

criterion_group!(
    canvas_scene_benches,
    bench_scene_bounds,
    bench_scene_query_point,
    bench_scene_visit,
);
criterion_main!(canvas_scene_benches);
