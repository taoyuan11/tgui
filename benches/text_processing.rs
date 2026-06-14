// 文本处理基准测试
// 覆盖文本整形（cosmic-text）、文本渲染、文本输入控制器等热路径

use std::hint::black_box;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};

#[cfg(feature = "bench-support")]
use tgui::widgets::bench_support_ext::*;

#[cfg(feature = "bench-support")]
fn bench_text_shaping(c: &mut Criterion) {
    let mut group = c.benchmark_group("text_shaping");

    let very_long = "Lorem ipsum dolor sit amet. ".repeat(20);
    let texts = vec![
        ("short", "Hello World"),
        ("medium", "The quick brown fox jumps over the lazy dog"),
        ("long", "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris."),
        ("very_long", very_long.as_str()),
    ];

    for (name, text) in texts {
        group.bench_with_input(BenchmarkId::new("shape", name), &text, |b, &text| {
            b.iter(|| {
                let shaped = shape_text(black_box(text), 14.0);
                black_box(shaped);
            });
        });
    }
    group.finish();
}

#[cfg(feature = "bench-support")]
fn bench_text_layout(c: &mut Criterion) {
    let mut group = c.benchmark_group("text_layout");

    for line_count in [1, 5, 10, 20, 50, 100].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(line_count),
            line_count,
            |b, &count| {
                let text = format!("{}\n", "Lorem ipsum dolor sit amet").repeat(count);

                b.iter(|| {
                    let layout = layout_text(black_box(&text), 400.0, 14.0);
                    black_box(layout);
                });
            },
        );
    }
    group.finish();
}

#[cfg(feature = "bench-support")]
fn bench_text_measurement(c: &mut Criterion) {
    let text = "The quick brown fox jumps over the lazy dog";

    c.bench_function("text_measurement", |b| {
        b.iter(|| {
            let size = measure_text(black_box(text), 14.0);
            black_box(size);
        });
    });
}

#[cfg(feature = "bench-support")]
fn bench_text_hit_test(c: &mut Criterion) {
    let text = "The quick brown fox jumps over the lazy dog";
    let layout = layout_text(text, 400.0, 14.0);

    c.bench_function("text_hit_test", |b| {
        b.iter(|| {
            let index = text_hit_test(&layout, black_box((150.0, 10.0)));
            black_box(index);
        });
    });
}

#[cfg(feature = "bench-support")]
fn bench_text_controller_insert(c: &mut Criterion) {
    let mut group = c.benchmark_group("text_controller_insert");

    for text_size in [10, 100, 1000, 10000].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(text_size),
            text_size,
            |b, &size| {
                let initial_text = "a".repeat(size);

                b.iter(|| {
                    let mut controller = create_text_controller(&initial_text);
                    controller.insert_at(size / 2, black_box("X"));
                    black_box(controller);
                });
            },
        );
    }
    group.finish();
}

#[cfg(feature = "bench-support")]
fn bench_text_controller_delete(c: &mut Criterion) {
    let mut group = c.benchmark_group("text_controller_delete");

    for text_size in [10, 100, 1000, 10000].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(text_size),
            text_size,
            |b, &size| {
                let initial_text = "a".repeat(size);

                b.iter(|| {
                    let mut controller = create_text_controller(&initial_text);
                    controller.delete_range(size / 2, size / 2 + 1);
                    black_box(controller);
                });
            },
        );
    }
    group.finish();
}

#[cfg(feature = "bench-support")]
fn bench_text_selection(c: &mut Criterion) {
    let text = "Lorem ipsum dolor sit amet\n".repeat(50);
    let layout = layout_text(&text, 400.0, 14.0);

    c.bench_function("text_selection_range", |b| {
        b.iter(|| {
            let selection = select_text_range(&layout, black_box(0), black_box(100));
            black_box(selection);
        });
    });
}

#[cfg(feature = "bench-support")]
fn bench_unicode_handling(c: &mut Criterion) {
    let mut group = c.benchmark_group("unicode_handling");

    let texts = vec![
        ("ascii", "Hello World 123"),
        ("latin", "Héllö Wörld"),
        ("cjk", "你好世界 こんにちは"),
        ("emoji", "Hello 👋 World 🌍"),
        ("mixed", "Hello 你好 👋 Wörld こんにちは 🌍"),
    ];

    for (name, text) in texts {
        group.bench_with_input(BenchmarkId::new("shape", name), &text, |b, &text| {
            b.iter(|| {
                let shaped = shape_text(black_box(text), 14.0);
                black_box(shaped);
            });
        });
    }
    group.finish();
}

#[cfg(feature = "bench-support")]
fn bench_text_wrapping(c: &mut Criterion) {
    let mut group = c.benchmark_group("text_wrapping");

    let text = "Lorem ipsum dolor sit amet, consectetur adipiscing elit. ".repeat(10);

    for width in [100.0, 200.0, 400.0, 800.0].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(*width as i32),
            width,
            |b, &width| {
                b.iter(|| {
                    let layout = layout_text(black_box(&text), black_box(width), 14.0);
                    black_box(layout);
                });
            },
        );
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
);
criterion_main!(benches);
