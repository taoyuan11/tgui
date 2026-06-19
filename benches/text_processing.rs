use std::hint::black_box;

use criterion::{criterion_group, criterion_main, BatchSize, BenchmarkId, Criterion};

#[cfg(feature = "bench-support")]
use std::time::Instant;

#[cfg(feature = "bench-support")]
use tgui::core::{dp, Rect};
#[cfg(feature = "bench-support")]
use tgui::layout::{Axis, Insets};
#[cfg(feature = "bench-support")]
use tgui::mvvm::ViewModelContext;
#[cfg(feature = "bench-support")]
use tgui::widgets::bench_support_ext::{BenchRopeBuffer, BenchTextLayout};
#[cfg(feature = "bench-support")]
use tgui::widgets::{Flex, Text, Textarea, WidgetBenchmarkContext, WidgetTree};

#[cfg(feature = "bench-support")]
fn viewport() -> Rect {
    Rect::new(0.0, 0.0, 1000.0, 720.0)
}

#[cfg(feature = "bench-support")]
fn repeated_text(bytes_hint: usize) -> String {
    let sentence = "The quick brown fox jumps over the retained text layout path. ";
    let mut text = String::with_capacity(bytes_hint + sentence.len());
    while text.len() < bytes_hint {
        text.push_str(sentence);
    }
    text
}

#[cfg(feature = "bench-support")]
fn repeated_unicode_text(bytes_hint: usize) -> String {
    let sentence = "Hello 你好 cafe こんにちは text edit boundary path. ";
    let mut text = String::with_capacity(bytes_hint + sentence.len());
    while text.len() < bytes_hint {
        text.push_str(sentence);
    }
    text
}

#[cfg(feature = "bench-support")]
fn indexed_lines(line_count: usize) -> (String, Vec<usize>, Vec<f32>) {
    let mut text = String::new();
    let mut byte_indices = Vec::with_capacity(line_count);
    let mut ys = Vec::with_capacity(line_count);

    for line in 0..line_count {
        let line_start = text.len();
        let line_text = format!("{line:04}: retained text layout query benchmark line");
        byte_indices.push(line_start + line_text.len() / 2);
        ys.push(line as f32 * 24.0 + 12.0);
        text.push_str(&line_text);
        if line + 1 < line_count {
            text.push('\n');
        }
    }

    (text, byte_indices, ys)
}

#[cfg(feature = "bench-support")]
fn indexed_single_line(bytes_hint: usize) -> (String, Vec<usize>) {
    let text = repeated_text(bytes_hint);
    let byte_indices = (0..=text.len()).collect();
    (text, byte_indices)
}

#[cfg(feature = "bench-support")]
fn clamp_to_char_boundary(text: &str, index: usize) -> usize {
    let mut index = index.min(text.len());
    while index > 0 && !text.is_char_boundary(index) {
        index -= 1;
    }
    index
}

#[cfg(feature = "bench-support")]
fn build_text_tree(lines: usize, sample: &str) -> WidgetTree<()> {
    let mut body = Flex::new(Axis::Vertical)
        .width(dp(920.0))
        .gap(dp(4.0))
        .padding(Insets::all(dp(10.0)));

    for line in 0..lines {
        body = body.child(Text::new(format!("{line:04}: {sample}")));
    }

    WidgetTree::new(
        Flex::new(Axis::Vertical)
            .width(dp(960.0))
            .height(dp(720.0))
            .padding(Insets::all(dp(16.0)))
            .child(body),
    )
}

#[cfg(feature = "bench-support")]
fn build_textarea_tree(controller: tgui::mvvm::TextController) -> WidgetTree<()> {
    WidgetTree::new(
        Flex::new(Axis::Vertical)
            .width(dp(960.0))
            .height(dp(720.0))
            .padding(Insets::all(dp(16.0)))
            .child(Textarea::new(controller).size(dp(920.0), dp(520.0))),
    )
}

#[cfg(feature = "bench-support")]
fn bench_text_shaping(c: &mut Criterion) {
    let mut group = c.benchmark_group("text_widget_full_layout_and_scene");

    let samples = [
        ("short", "Hello World".to_string()),
        (
            "medium",
            "The quick brown fox jumps over the lazy dog".to_string(),
        ),
        ("long", repeated_text(256)),
        ("very_long", repeated_text(2048)),
    ];

    for (name, sample) in samples {
        let tree = build_text_tree(20, &sample);
        group.bench_with_input(BenchmarkId::new("sample", name), &name, |b, _| {
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
fn bench_text_layout(c: &mut Criterion) {
    let mut group = c.benchmark_group("text_widget_scene_recollect_cached_layout");
    let sample = repeated_text(192);

    for line_count in [1_usize, 5, 20, 50, 100] {
        let tree = build_text_tree(line_count, &sample);
        group.bench_with_input(
            BenchmarkId::from_parameter(line_count),
            &line_count,
            |b, _| {
                let mut ctx = WidgetBenchmarkContext::new().with_viewport(viewport());
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

#[cfg(feature = "bench-support")]
fn bench_text_measurement(c: &mut Criterion) {
    let mut group = c.benchmark_group("text_controller_snapshot");
    let bench_ctx = ViewModelContext::for_benchmarks();

    for size in [32_usize, 256, 1024, 4096] {
        let controller = bench_ctx.text_controller(repeated_text(size));
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, _| {
            b.iter(|| {
                let snapshot = controller.snapshot();
                black_box(snapshot);
            });
        });
    }

    group.finish();
}

#[cfg(feature = "bench-support")]
fn bench_text_hit_test(c: &mut Criterion) {
    let mut group = c.benchmark_group("textarea_controller_update_scene_recollect");

    for size in [64_usize, 1024, 4096] {
        let bench_ctx = ViewModelContext::for_benchmarks();
        let controller = bench_ctx.text_controller(repeated_text(size));
        let tree = build_textarea_tree(controller.clone());
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &size| {
            let mut ctx = WidgetBenchmarkContext::new().with_viewport(viewport());
            let _ = ctx.run_layout_and_scene(&tree, Instant::now());
            let mut revision = 0usize;
            b.iter(|| {
                revision = revision.wrapping_add(1);
                controller.set_text(black_box(format!(
                    "{}\nrevision={revision}",
                    repeated_text(size)
                )));
                let stats = ctx.recollect_scene_only(black_box(&tree), Instant::now());
                black_box(stats);
            });
        });
    }

    group.finish();
}

#[cfg(feature = "bench-support")]
fn bench_text_controller_insert(c: &mut Criterion) {
    let mut group = c.benchmark_group("text_controller_replace_all");
    let bench_ctx = ViewModelContext::for_benchmarks();

    for size in [32_usize, 256, 1024, 4096] {
        let controller = bench_ctx.text_controller(repeated_text(size));
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &size| {
            let mut revision = 0usize;
            b.iter(|| {
                revision = revision.wrapping_add(1);
                controller.replace_all(black_box(format!(
                    "{} revision={revision}",
                    repeated_text(size)
                )));
                black_box(controller.revision());
            });
        });
    }

    group.finish();
}

#[cfg(feature = "bench-support")]
fn bench_text_controller_delete(c: &mut Criterion) {
    let mut group = c.benchmark_group("text_controller_with_text");
    let bench_ctx = ViewModelContext::for_benchmarks();

    for size in [32_usize, 256, 1024, 4096] {
        let controller = bench_ctx.text_controller(repeated_text(size));
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, _| {
            b.iter(|| {
                let len = controller.with_text(|text| text.len());
                black_box(len);
            });
        });
    }

    group.finish();
}

#[cfg(feature = "bench-support")]
fn bench_text_selection(c: &mut Criterion) {
    let mut group = c.benchmark_group("text_unicode_scene_recollect_cached_layout");

    let samples = [
        ("ascii", "Hello World 123".to_string()),
        ("latin", "Héllö Wörld".to_string()),
        ("cjk", "你好世界 こんにちは".to_string()),
        ("emoji", "Hello 👋 World 🌍".to_string()),
        ("mixed", "Hello 你好 👋 Wörld こんにちは 🌍".to_string()),
    ];

    for (name, sample) in samples {
        let tree = build_text_tree(50, &sample);
        group.bench_with_input(BenchmarkId::new("sample", name), &name, |b, _| {
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
fn bench_unicode_handling(c: &mut Criterion) {
    let mut group = c.benchmark_group("text_widget_full_layout_by_line_count");
    let sample = "Hello 你好 👋 Wörld こんにちは 🌍";

    for lines in [1_usize, 10, 50, 100] {
        let tree = build_text_tree(lines, sample);
        group.bench_with_input(BenchmarkId::from_parameter(lines), &lines, |b, _| {
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
fn bench_text_wrapping(c: &mut Criterion) {
    let mut group = c.benchmark_group("textarea_wrapped_scene_recollect_cached_layout");

    for width in [240.0_f32, 480.0, 720.0, 920.0] {
        let bench_ctx = ViewModelContext::for_benchmarks();
        let controller = bench_ctx.text_controller(repeated_text(4096));
        let tree = WidgetTree::new(
            Flex::new(Axis::Vertical)
                .width(dp(960.0))
                .height(dp(720.0))
                .padding(Insets::all(dp(16.0)))
                .child(Textarea::new(controller).size(dp(width), dp(520.0))),
        );
        group.bench_with_input(BenchmarkId::from_parameter(width as i32), &width, |b, _| {
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
fn bench_rope_buffer_replace(c: &mut Criterion) {
    let mut group = c.benchmark_group("text_rope_buffer_replace_at_cursor");

    for (name, sample) in [
        ("ascii_1024", repeated_text(1024)),
        ("mixed_1024", repeated_unicode_text(1024)),
        ("ascii_4096", repeated_text(4096)),
        ("mixed_4096", repeated_unicode_text(4096)),
    ] {
        let cursor = clamp_to_char_boundary(&sample, sample.len() / 2);
        group.bench_with_input(BenchmarkId::from_parameter(name), &sample, |b, sample| {
            b.iter_batched(
                || BenchRopeBuffer::from_text(sample),
                |mut buffer| {
                    buffer.replace_byte_range(black_box(cursor), black_box(cursor), "x");
                    black_box(buffer.len_bytes());
                },
                BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

#[cfg(feature = "bench-support")]
fn bench_rope_buffer_boundary_walk(c: &mut Criterion) {
    let mut group = c.benchmark_group("text_rope_buffer_boundary_walk");

    for (name, sample) in [
        ("ascii_4096", repeated_text(4096)),
        ("mixed_4096", repeated_unicode_text(4096)),
    ] {
        let buffer = BenchRopeBuffer::from_text(&sample);
        let cursor = clamp_to_char_boundary(&sample, sample.len() / 2);
        group.bench_with_input(BenchmarkId::from_parameter(name), &sample, |b, _| {
            b.iter(|| {
                let previous = buffer.prev_char_boundary_byte(black_box(cursor));
                let next = buffer.next_char_boundary_byte(black_box(cursor));
                black_box((previous, next));
            });
        });
    }

    group.finish();
}

#[cfg(feature = "bench-support")]
fn bench_text_layout_line_queries(c: &mut Criterion) {
    let mut group = c.benchmark_group("text_layout_line_queries");

    for line_count in [32_usize, 256, 2048] {
        let (text, byte_indices, ys) = indexed_lines(line_count);
        let layout = BenchTextLayout::from_text(&text);

        group.bench_with_input(
            BenchmarkId::new("index", line_count),
            &line_count,
            |b, _| {
                b.iter(|| {
                    let mut sum = 0usize;
                    for index in &byte_indices {
                        sum = sum.wrapping_add(layout.line_index_for_index(black_box(*index)));
                    }
                    black_box(sum);
                });
            },
        );

        group.bench_with_input(BenchmarkId::new("y", line_count), &line_count, |b, _| {
            b.iter(|| {
                let mut sum = 0usize;
                for y in &ys {
                    sum = sum.wrapping_add(layout.line_index_for_y(black_box(*y)));
                }
                black_box(sum);
            });
        });
    }

    group.finish();
}

#[cfg(feature = "bench-support")]
fn bench_text_layout_boundary_queries(c: &mut Criterion) {
    let mut group = c.benchmark_group("text_layout_boundary_queries");

    for size in [64_usize, 512, 4096] {
        let (text, byte_indices) = indexed_single_line(size);
        let layout = BenchTextLayout::from_text(&text);

        group.bench_with_input(BenchmarkId::new("x_for_index", size), &size, |b, _| {
            b.iter(|| {
                let mut sum = 0.0f32;
                for index in &byte_indices {
                    sum += layout.x_for_index(black_box(*index));
                }
                black_box(sum);
            });
        });
    }

    group.finish();
}

#[cfg(feature = "bench-support")]
fn bench_text_layout_hit_queries(c: &mut Criterion) {
    let mut group = c.benchmark_group("text_layout_hit_queries");

    for size in [64_usize, 512, 4096] {
        let (text, byte_indices) = indexed_single_line(size);
        let layout = BenchTextLayout::from_text(&text);
        let xs: Vec<_> = byte_indices
            .iter()
            .map(|index| layout.x_for_index(*index) + 0.25)
            .collect();

        group.bench_with_input(BenchmarkId::new("index_for_x", size), &size, |b, _| {
            b.iter(|| {
                let mut sum = 0usize;
                for x in &xs {
                    sum = sum.wrapping_add(layout.index_for_x(black_box(*x)));
                }
                black_box(sum);
            });
        });
    }

    group.finish();
}

#[cfg(not(feature = "bench-support"))]
fn bench_text_shaping(_c: &mut Criterion) {
    eprintln!("Skipping text_processing benchmarks: bench-support feature not enabled");
}

#[cfg(not(feature = "bench-support"))]
fn bench_text_layout(_c: &mut Criterion) {}

#[cfg(not(feature = "bench-support"))]
fn bench_text_measurement(_c: &mut Criterion) {}

#[cfg(not(feature = "bench-support"))]
fn bench_text_hit_test(_c: &mut Criterion) {}

#[cfg(not(feature = "bench-support"))]
fn bench_text_controller_insert(_c: &mut Criterion) {}

#[cfg(not(feature = "bench-support"))]
fn bench_text_controller_delete(_c: &mut Criterion) {}

#[cfg(not(feature = "bench-support"))]
fn bench_text_selection(_c: &mut Criterion) {}

#[cfg(not(feature = "bench-support"))]
fn bench_unicode_handling(_c: &mut Criterion) {}

#[cfg(not(feature = "bench-support"))]
fn bench_text_wrapping(_c: &mut Criterion) {}

#[cfg(not(feature = "bench-support"))]
fn bench_rope_buffer_replace(_c: &mut Criterion) {}

#[cfg(not(feature = "bench-support"))]
fn bench_rope_buffer_boundary_walk(_c: &mut Criterion) {}

#[cfg(not(feature = "bench-support"))]
fn bench_text_layout_line_queries(_c: &mut Criterion) {}

#[cfg(not(feature = "bench-support"))]
fn bench_text_layout_boundary_queries(_c: &mut Criterion) {}

#[cfg(not(feature = "bench-support"))]
fn bench_text_layout_hit_queries(_c: &mut Criterion) {}

criterion_group!(
    benches,
    bench_text_shaping,
    bench_text_layout,
    bench_text_measurement,
    bench_text_hit_test,
    bench_text_controller_insert,
    bench_text_controller_delete,
    bench_text_selection,
    bench_unicode_handling,
    bench_text_wrapping,
    bench_rope_buffer_replace,
    bench_rope_buffer_boundary_walk,
    bench_text_layout_line_queries,
    bench_text_layout_boundary_queries,
    bench_text_layout_hit_queries,
);
criterion_main!(benches);
