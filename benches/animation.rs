// 动画系统基准测试
// 覆盖动画引擎更新、时间线控制、值插值等热路径

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};

#[cfg(feature = "bench-support")]
use tgui::widgets::bench_support_ext::*;

#[cfg(feature = "bench-support")]
fn bench_animation_update(c: &mut Criterion) {
    let mut group = c.benchmark_group("animation_update");

    for count in [1, 10, 50, 100, 200].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(count),
            count,
            |b, &count| {
                let mut engine = create_animation_engine();
                let animations = create_n_animations(count);

                for anim in &animations {
                    add_animation_to_engine(&mut engine, anim.clone());
                }

                b.iter(|| {
                    update_animation_engine(&mut engine, black_box(16.0));
                });
            },
        );
    }
    group.finish();
}

#[cfg(feature = "bench-support")]
fn bench_animation_interpolation(c: &mut Criterion) {
    let mut group = c.benchmark_group("animation_interpolation");

    let interpolation_types = vec![
        ("linear", InterpolationType::Linear),
        ("ease_in", InterpolationType::EaseIn),
        ("ease_out", InterpolationType::EaseOut),
        ("ease_in_out", InterpolationType::EaseInOut),
        ("spring", InterpolationType::Spring),
    ];

    for (name, interp_type) in interpolation_types {
        group.bench_with_input(
            BenchmarkId::new("float", name),
            &interp_type,
            |b, &interp| {
                b.iter(|| {
                    let value = interpolate_float(0.0, 100.0, black_box(0.5), interp);
                    black_box(value);
                });
            },
        );
    }
    group.finish();
}

#[cfg(feature = "bench-support")]
fn bench_animation_color_interpolation(c: &mut Criterion) {
    c.bench_function("color_interpolation", |b| {
        let color1 = create_color(255, 0, 0, 255);
        let color2 = create_color(0, 0, 255, 255);

        b.iter(|| {
            let color = interpolate_color(color1, color2, black_box(0.5));
            black_box(color);
        });
    });
}

#[cfg(feature = "bench-support")]
fn bench_animation_transform_interpolation(c: &mut Criterion) {
    c.bench_function("transform_interpolation", |b| {
        let transform1 = create_transform(0.0, 0.0, 1.0, 0.0);
        let transform2 = create_transform(100.0, 100.0, 2.0, 45.0);

        b.iter(|| {
            let transform = interpolate_transform(transform1, transform2, black_box(0.5));
            black_box(transform);
        });
    });
}

#[cfg(feature = "bench-support")]
fn bench_timeline_playback(c: &mut Criterion) {
    let timeline = create_complex_timeline();

    c.bench_function("timeline_playback", |b| {
        b.iter(|| {
            let state = evaluate_timeline(&timeline, black_box(500.0));
            black_box(state);
        });
    });
}

#[cfg(feature = "bench-support")]
fn bench_animation_state_transition(c: &mut Criterion) {
    let mut group = c.benchmark_group("animation_state_transition");

    for transition_count in [1, 5, 10, 20].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(transition_count),
            transition_count,
            |b, &count| {
                let mut state_machine = create_animation_state_machine();

                b.iter(|| {
                    for _ in 0..count {
                        trigger_state_transition(&mut state_machine);
                    }
                    black_box(&state_machine);
                });
            },
        );
    }
    group.finish();
}

#[cfg(feature = "bench-support")]
fn bench_spring_physics(c: &mut Criterion) {
    c.bench_function("spring_physics", |b| {
        let mut spring = create_spring_animation(0.0, 100.0, 0.5, 0.8);

        b.iter(|| {
            let value = update_spring(&mut spring, black_box(16.0));
            black_box(value);
        });
    });
}

#[cfg(feature = "bench-support")]
fn bench_keyframe_evaluation(c: &mut Criterion) {
    let mut group = c.benchmark_group("keyframe_evaluation");

    for keyframe_count in [3, 10, 20, 50].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(keyframe_count),
            keyframe_count,
            |b, &count| {
                let keyframes = create_n_keyframes(count);

                b.iter(|| {
                    let value = evaluate_keyframes(&keyframes, black_box(0.5));
                    black_box(value);
                });
            },
        );
    }
    group.finish();
}

#[cfg(not(feature = "bench-support"))]
fn bench_animation_update(_c: &mut Criterion) {
    eprintln!("Skipping animation benchmarks: bench-support feature not enabled");
}

#[cfg(not(feature = "bench-support"))]
fn bench_animation_interpolation(_c: &mut Criterion) {}

#[cfg(not(feature = "bench-support"))]
fn bench_animation_color_interpolation(_c: &mut Criterion) {}

#[cfg(not(feature = "bench-support"))]
fn bench_animation_transform_interpolation(_c: &mut Criterion) {}

#[cfg(not(feature = "bench-support"))]
fn bench_timeline_playback(_c: &mut Criterion) {}

#[cfg(not(feature = "bench-support"))]
fn bench_animation_state_transition(_c: &mut Criterion) {}

#[cfg(not(feature = "bench-support"))]
fn bench_spring_physics(_c: &mut Criterion) {}

#[cfg(not(feature = "bench-support"))]
fn bench_keyframe_evaluation(_c: &mut Criterion) {}

criterion_group!(
    benches,
    bench_animation_update,
    bench_animation_interpolation,
    bench_animation_color_interpolation,
    bench_animation_transform_interpolation,
    bench_timeline_playback,
    bench_animation_state_transition,
    bench_spring_physics,
    bench_keyframe_evaluation,
);
criterion_main!(benches);
