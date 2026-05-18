use std::hint::black_box;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use tgui::mvvm::ViewModelContext;

fn bench_scalar_state_signal(c: &mut Criterion) {
    let mut group = c.benchmark_group("state_signal_scalar");
    let ctx = ViewModelContext::for_benchmarks();
    let state = ctx.state(0_u32);
    let signal = state.signal().map(|value| value.wrapping_mul(2));

    group.bench_function("update_then_get", |b| {
        b.iter(|| {
            state.update(|value| *value = value.wrapping_add(1));
            black_box(signal.get())
        });
    });

    group.finish();
}

fn bench_string_state_signal(c: &mut Criterion) {
    let mut group = c.benchmark_group("state_signal_string");

    let short_text = "criterion-state-signal";
    let long_text = "abcdefghijklmnopqrstuvwxyz0123456789".repeat(256);

    for (label, text) in [("short", short_text.to_string()), ("long", long_text)] {
        let ctx = ViewModelContext::for_benchmarks();
        let state = ctx.state(text.clone());
        let signal = state.signal().map(|value| value.len());
        let replacement = text.clone();

        group.bench_with_input(BenchmarkId::new("set_then_get_len", label), &replacement, |b, input| {
            b.iter(|| {
                state.set(black_box(input.clone()));
                black_box(signal.get())
            });
        });
    }

    group.finish();
}

criterion_group!(state_signal_benches, bench_scalar_state_signal, bench_string_state_signal);
criterion_main!(state_signal_benches);
