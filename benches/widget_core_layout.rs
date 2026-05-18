use std::hint::black_box;
use std::time::{Duration, Instant};

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use tgui::animation::Transition;
use tgui::core::dp;
use tgui::layout::{Axis, Insets};
use tgui::mvvm::{State, ViewModelContext};
use tgui::widgets::{Element, Flex, Stack, Text, Textarea, WidgetBenchmarkContext, WidgetTree};

fn repeated_line(index: usize) -> String {
    format!("Line {index}: benchmark text payload for widget-core layout evaluation.")
}

fn build_many_widgets_tree(node_count: usize) -> WidgetTree<()> {
    let mut root = Flex::new(Axis::Vertical)
        .width(dp(1280.0))
        .padding(Insets::all(dp(8.0)))
        .gap(dp(6.0));

    for row in 0..node_count {
        let card = Stack::new()
            .width(dp(1240.0))
            .padding(Insets::all(dp(6.0)))
            .child(Text::new(format!("Row {row}")))
            .child(Text::new(repeated_line(row)))
            .child(
                Flex::new(Axis::Horizontal)
                    .gap(dp(4.0))
                    .child(Text::new("left metric"))
                    .child(Text::new("center metric"))
                    .child(Text::new("right metric")),
            );
        root = root.child(card);
    }

    WidgetTree::new(root)
}

fn build_text_heavy_tree(short_lines: usize, long_blocks: usize) -> WidgetTree<()> {
    let mut root = Flex::new(Axis::Vertical)
        .width(dp(1280.0))
        .padding(Insets::all(dp(12.0)))
        .gap(dp(8.0));

    for index in 0..short_lines {
        root = root.child(Text::new(repeated_line(index)));
    }

    let paragraph = (0..48).map(repeated_line).collect::<Vec<_>>().join("\n");

    for _ in 0..long_blocks {
        root = root.child(
            Textarea::new(paragraph.clone())
                .width(dp(1200.0))
                .height(dp(220.0)),
        );
    }

    WidgetTree::new(root)
}

fn build_animated_tree(
    card_count: usize,
    layout_affecting: bool,
) -> (WidgetTree<()>, State<f32>, Instant) {
    let ctx = ViewModelContext::for_benchmarks();
    let phase = ctx.state(0.0_f32);
    let start = Instant::now();

    let mut root = Flex::new(Axis::Vertical)
        .width(dp(1280.0))
        .padding(Insets::all(dp(12.0)))
        .gap(dp(8.0));

    for index in 0..card_count {
        let phase_signal = phase.signal();
        let animated_width = phase_signal
            .map(move |value| dp(220.0 + ((index % 5) as f32 * 8.0) + value * 96.0))
            .animated(Transition::ease_in_out(Duration::from_millis(240)));
        let animated_padding = phase_signal
            .map(move |value| Insets::all(dp(6.0 + value * 10.0)))
            .animated(Transition::ease_in_out(Duration::from_millis(240)));

        let content: Element<()> = if layout_affecting {
            Stack::new()
                .width(animated_width)
                .padding(animated_padding)
                .child(Text::new(format!("Animated row {index}")))
                .child(Text::new(repeated_line(index)))
                .into()
        } else {
            Stack::new()
                .width(dp(320.0))
                .padding(Insets::all(dp(8.0)))
                .opacity(phase_signal.map(move |value| 0.35 + value * 0.65))
                .child(Text::new(format!("Animated row {index}")))
                .child(Text::new(repeated_line(index)))
                .into()
        };

        root = root.child(content);
    }

    (WidgetTree::new(root), phase, start)
}

fn bench_many_widgets_layout(c: &mut Criterion) {
    let mut group = c.benchmark_group("many_widgets_layout");

    for node_count in [200_usize, 1000_usize] {
        let tree = build_many_widgets_tree(node_count);

        group.bench_with_input(
            BenchmarkId::new("layout_only", node_count),
            &node_count,
            |b, _| {
                let mut bench = WidgetBenchmarkContext::default();
                b.iter(|| {
                    let stats = bench.run_layout(&tree, Instant::now());
                    black_box(stats.dependency_count)
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("layout_and_scene", node_count),
            &node_count,
            |b, _| {
                let mut bench = WidgetBenchmarkContext::default();
                b.iter(|| {
                    let stats = bench.run_layout_and_scene(&tree, Instant::now());
                    black_box((
                        stats.shape_count,
                        stats.text_count,
                        stats.overlay_shape_count,
                        stats.hit_region_count,
                    ))
                });
            },
        );
    }

    group.finish();
}

fn bench_text_heavy_layout(c: &mut Criterion) {
    let mut group = c.benchmark_group("text_heavy_layout");

    let scenarios = [
        ("many_short_lines", build_text_heavy_tree(400, 0)),
        ("few_long_blocks", build_text_heavy_tree(40, 8)),
    ];

    for (label, tree) in scenarios {
        group.bench_with_input(BenchmarkId::new("layout_only", label), &label, |b, _| {
            let mut bench = WidgetBenchmarkContext::default();
            b.iter(|| {
                let stats = bench.run_layout(&tree, Instant::now());
                black_box((stats.dependency_count, stats.has_global_dependency))
            });
        });

        group.bench_with_input(
            BenchmarkId::new("layout_and_scene", label),
            &label,
            |b, _| {
                let mut bench = WidgetBenchmarkContext::default();
                b.iter(|| {
                    let stats = bench.run_layout_and_scene(&tree, Instant::now());
                    black_box((
                        stats.text_count,
                        stats.scroll_region_count,
                        stats.hit_region_count,
                    ))
                });
            },
        );
    }

    group.finish();
}

fn bench_animated_scene_recompute(c: &mut Criterion) {
    let mut group = c.benchmark_group("animated_scene_recompute");

    for (label, layout_affecting) in [
        ("animated_visual_only", false),
        ("animated_layout_affecting", true),
    ] {
        let (tree, phase, start) = build_animated_tree(240, layout_affecting);
        let sample_offsets = [
            Duration::from_millis(24),
            Duration::from_millis(96),
            Duration::from_millis(168),
        ];

        group.bench_with_input(
            BenchmarkId::new("layout_and_scene", label),
            &label,
            |b, _| {
                let mut bench = WidgetBenchmarkContext::default();
                let _ = bench.run_layout_and_scene(&tree, start);
                phase.set(1.0);
                bench.invalidate_all();
                let mut tick = 0usize;
                b.iter(|| {
                    let offset = sample_offsets[tick % sample_offsets.len()];
                    tick = tick.wrapping_add(1);
                    let stats = bench.run_layout_and_scene(&tree, start + offset);
                    black_box((
                        stats.shape_count,
                        stats.text_count,
                        stats.overlay_shape_count,
                        stats.hit_region_count,
                    ))
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    widget_core_layout_benches,
    bench_many_widgets_layout,
    bench_text_heavy_layout,
    bench_animated_scene_recompute
);
criterion_main!(widget_core_layout_benches);
