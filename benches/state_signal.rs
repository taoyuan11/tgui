// State 和 Signal 响应式系统基准测试
// 覆盖依赖跟踪、失效传播、派生信号计算等核心热路径

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use tgui::mvvm::ViewModelContext;

fn bench_state_creation(c: &mut Criterion) {
    c.bench_function("state_creation", |b| {
        let ctx = ViewModelContext::for_benchmarks();
        b.iter(|| {
            let state = ctx.state(black_box(42));
            black_box(state);
        });
    });
}

fn bench_state_read(c: &mut Criterion) {
    let ctx = ViewModelContext::for_benchmarks();
    let state = ctx.state(42);

    c.bench_function("state_read", |b| {
        b.iter(|| {
            let value = state.get();
            black_box(value);
        });
    });
}

fn bench_state_write(c: &mut Criterion) {
    let ctx = ViewModelContext::for_benchmarks();
    let state = ctx.state(0);

    c.bench_function("state_write", |b| {
        let mut counter = 0;
        b.iter(|| {
            counter += 1;
            state.set(black_box(counter));
        });
    });
}

fn bench_state_update(c: &mut Criterion) {
    let ctx = ViewModelContext::for_benchmarks();
    let state = ctx.state(0);

    c.bench_function("state_update", |b| {
        b.iter(|| {
            state.update(|v| *v = black_box(*v + 1));
        });
    });
}

fn bench_signal_creation(c: &mut Criterion) {
    let ctx = ViewModelContext::for_benchmarks();
    let state = ctx.state(42);

    c.bench_function("signal_creation", |b| {
        b.iter(|| {
            let signal = state.signal().map(|x| x * 2);
            black_box(signal);
        });
    });
}

fn bench_signal_read(c: &mut Criterion) {
    let ctx = ViewModelContext::for_benchmarks();
    let state = ctx.state(42);
    let signal = state.signal().map(|x| x * 2);

    c.bench_function("signal_read", |b| {
        b.iter(|| {
            let value = signal.get();
            black_box(value);
        });
    });
}

fn bench_signal_chain(c: &mut Criterion) {
    let mut group = c.benchmark_group("signal_chain");
    let ctx = ViewModelContext::for_benchmarks();

    for chain_length in [1, 2, 4, 8, 16].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(chain_length),
            chain_length,
            |b, &length| {
                let state = ctx.state(1);
                let mut signal = state.signal();

                for _ in 0..length {
                    signal = signal.map(|x| x + 1);
                }

                b.iter(|| {
                    let value = signal.get();
                    black_box(value);
                });
            },
        );
    }
    group.finish();
}

fn bench_dependency_tracking(c: &mut Criterion) {
    let mut group = c.benchmark_group("dependency_tracking");
    let ctx = ViewModelContext::for_benchmarks();

    for num_deps in [1, 5, 10, 20, 50].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(num_deps),
            num_deps,
            |b, &count| {
                let states: Vec<_> = (0..count).map(|i| ctx.state(i)).collect();

                b.iter(|| {
                    let mut sum = 0;
                    for state in &states {
                        sum += state.get();
                    }
                    black_box(sum);
                });
            },
        );
    }
    group.finish();
}

fn bench_invalidation_propagation(c: &mut Criterion) {
    let mut group = c.benchmark_group("invalidation_propagation");
    let ctx = ViewModelContext::for_benchmarks();

    for num_derived in [1, 5, 10, 20].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(num_derived),
            num_derived,
            |b, &count| {
                let base = ctx.state(0);
                let derived: Vec<_> = (0..count)
                    .map(|i| base.signal().map(move |x| x + i))
                    .collect();

                b.iter(|| {
                    base.set(black_box(42));
                    for signal in &derived {
                        let _ = black_box(signal.get());
                    }
                });
            },
        );
    }
    group.finish();
}

fn bench_complex_signal_graph(c: &mut Criterion) {
    let ctx = ViewModelContext::for_benchmarks();

    // 模拟复杂的信号依赖图：多个 State，多层派生，交叉依赖
    let state_a = ctx.state(10);
    let state_b = ctx.state(20);
    let state_c = ctx.state(30);

    let sig_ab = state_a.signal().map({
        let state_b = state_b.clone();
        move |a| a + state_b.get()
    });

    let sig_bc = state_b.signal().map({
        let state_c = state_c.clone();
        move |b| b + state_c.get()
    });

    let sig_abc = state_a.signal().map({
        let sig_bc = sig_bc.clone();
        move |a| a + sig_bc.get()
    });

    c.bench_function("complex_signal_graph", |b| {
        b.iter(|| {
            state_a.set(black_box(state_a.get() + 1));
            let result = sig_ab.get() + sig_bc.get() + sig_abc.get();
            black_box(result);
        });
    });
}

fn bench_state_with_large_data(c: &mut Criterion) {
    let mut group = c.benchmark_group("state_with_large_data");
    let ctx = ViewModelContext::for_benchmarks();

    for size in [100, 1000, 10000].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(size),
            size,
            |b, &size| {
                let data: Vec<i32> = (0..size).collect();
                let state = ctx.state(data.clone());

                b.iter(|| {
                    let mut new_data = state.get();
                    new_data[0] = black_box(new_data[0] + 1);
                    state.set(new_data);
                });
            },
        );
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_state_creation,
    bench_state_read,
    bench_state_write,
    bench_state_update,
    bench_signal_creation,
    bench_signal_read,
    bench_signal_chain,
    bench_dependency_tracking,
    bench_invalidation_propagation,
    bench_complex_signal_graph,
    bench_state_with_large_data,
);
criterion_main!(benches);
