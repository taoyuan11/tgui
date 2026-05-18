use std::hint::black_box;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use tgui::mvvm::{TextController, ViewModelContext};

fn make_payload(size: usize) -> String {
    let unit = "abcdefghijklmnopqrstuvwxyz0123456789-_";
    unit.repeat(size.div_ceil(unit.len()))
        .chars()
        .take(size)
        .collect()
}

fn bench_set_text(c: &mut Criterion) {
    let mut group = c.benchmark_group("text_controller_set_text");

    for size in [32_usize, 1024, 16 * 1024] {
        let payload_a = make_payload(size);
        let payload_b = make_payload(size + 7);

        group.bench_with_input(BenchmarkId::new("alternating", size), &size, |b, _| {
            let ctx = ViewModelContext::for_benchmarks();
            let controller = ctx.text_controller(payload_a.clone());
            let mut tick = 0usize;
            b.iter(|| {
                let payload = if tick % 2 == 0 {
                    payload_a.clone()
                } else {
                    payload_b.clone()
                };
                tick = tick.wrapping_add(1);
                controller.set_text(black_box(payload));
                black_box(controller.revision())
            });
        });
    }

    group.finish();
}

fn bench_snapshot_and_with_text(c: &mut Criterion) {
    let mut group = c.benchmark_group("text_controller_read");

    for size in [128_usize, 4096, 64 * 1024] {
        let payload = make_payload(size);
        let ctx = ViewModelContext::for_benchmarks();
        let controller = ctx.text_controller(payload.clone());

        group.bench_with_input(BenchmarkId::new("snapshot_clone", size), &size, |b, _| {
            b.iter(|| {
                let snapshot = controller.snapshot();
                black_box(snapshot.text.len())
            });
        });

        group.bench_with_input(BenchmarkId::new("with_text_borrow", size), &size, |b, _| {
            b.iter(|| {
                let len = controller.with_text(|text| text.len());
                black_box(len)
            });
        });
    }

    group.finish();
}

fn bench_replace_unchanged(c: &mut Criterion) {
    let mut group = c.benchmark_group("text_controller_replace_all");
    let payload = make_payload(2048);

    group.bench_function("identical_value", |b| {
        let ctx = ViewModelContext::for_benchmarks();
        let controller: TextController = ctx.text_controller(payload.clone());
        b.iter(|| {
            controller.replace_all(black_box(payload.clone()));
            black_box(controller.revision())
        });
    });

    group.bench_function("changing_value", |b| {
        let ctx = ViewModelContext::for_benchmarks();
        let controller: TextController = ctx.text_controller(payload.clone());
        let mut counter = 0u64;
        b.iter(|| {
            counter = counter.wrapping_add(1);
            controller.replace_all(format!("{counter}-{}", black_box(payload.as_str())));
            black_box(controller.revision())
        });
    });

    group.finish();
}

criterion_group!(
    text_controller_benches,
    bench_set_text,
    bench_snapshot_and_with_text,
    bench_replace_unchanged,
);
criterion_main!(text_controller_benches);
