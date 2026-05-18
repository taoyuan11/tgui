use std::hint::black_box;
use std::time::Duration;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use tgui::animation::{
    AnimatedValue, AnimationCurve, AnimationSpec, FillMode, Keyframes, Playback, PlaybackDirection,
    Transition,
};
use tgui::core::{dp, Color};
use tgui::layout::Insets;
use tgui::mvvm::ViewModelContext;

fn bench_animation_curve_sample(c: &mut Criterion) {
    let mut group = c.benchmark_group("animation_curve_sample");

    let curves = [
        ("linear", AnimationCurve::Linear),
        ("ease_in", AnimationCurve::EaseInCubic),
        ("ease_out", AnimationCurve::EaseOutCubic),
        ("ease_in_out", AnimationCurve::EaseInOutCubic),
    ];

    for (label, curve) in curves {
        group.bench_function(BenchmarkId::new("sample_64", label), |b| {
            b.iter(|| {
                let mut acc = 0.0f32;
                for index in 0..64u32 {
                    let progress = index as f32 / 63.0;
                    acc += curve.sample(black_box(progress));
                }
                black_box(acc)
            });
        });
    }

    group.finish();
}

fn bench_keyframes_sample_at(c: &mut Criterion) {
    let mut group = c.benchmark_group("keyframes_sample_at");

    let total = Duration::from_millis(800);

    let f32_keyframes = Keyframes::timed(total)
        .at(Duration::ZERO, 0.0_f32)
        .at(Duration::from_millis(200), 30.0)
        .at(Duration::from_millis(450), 60.0)
        .at(Duration::from_millis(650), 80.0)
        .at(total, 100.0)
        .curve(AnimationCurve::EaseInOutCubic);

    let color_keyframes = Keyframes::timed(total)
        .at(Duration::ZERO, Color::rgba(0, 0, 0, 0))
        .at(Duration::from_millis(200), Color::rgba(64, 96, 128, 200))
        .at(Duration::from_millis(450), Color::rgba(196, 128, 64, 220))
        .at(Duration::from_millis(650), Color::rgba(255, 255, 255, 240))
        .at(total, Color::rgba(255, 32, 64, 255))
        .curve(AnimationCurve::EaseOutCubic);

    let insets_keyframes = Keyframes::timed(total)
        .at(Duration::ZERO, Insets::all(dp(0.0)))
        .at(Duration::from_millis(200), Insets::all(dp(8.0)))
        .at(Duration::from_millis(450), Insets::all(dp(20.0)))
        .at(total, Insets::all(dp(32.0)))
        .curve(AnimationCurve::Linear);

    let sample_offsets: [Duration; 8] = [
        Duration::from_millis(0),
        Duration::from_millis(75),
        Duration::from_millis(150),
        Duration::from_millis(275),
        Duration::from_millis(400),
        Duration::from_millis(525),
        Duration::from_millis(700),
        Duration::from_millis(800),
    ];

    group.bench_function("f32_5frames", |b| {
        b.iter(|| {
            let mut last = None;
            for offset in sample_offsets {
                last = f32_keyframes.sample_at(black_box(offset));
            }
            black_box(last)
        });
    });

    group.bench_function("color_5frames", |b| {
        b.iter(|| {
            let mut last = None;
            for offset in sample_offsets {
                last = color_keyframes.sample_at(black_box(offset));
            }
            black_box(last)
        });
    });

    group.bench_function("insets_4frames", |b| {
        b.iter(|| {
            let mut last = None;
            for offset in sample_offsets {
                last = insets_keyframes.sample_at(black_box(offset));
            }
            black_box(last)
        });
    });

    group.finish();
}

fn bench_animation_controller_seek(c: &mut Criterion) {
    let mut group = c.benchmark_group("animation_controller_seek");

    for track_count in [1_usize, 8, 32] {
        group.bench_with_input(
            BenchmarkId::new("seek_percent", track_count),
            &track_count,
            |b, &track_count| {
                let ctx = ViewModelContext::for_benchmarks();
                let mut builder = ctx.timeline().playback(
                    Playback::new()
                        .repeat(2)
                        .direction(PlaybackDirection::Alternate)
                        .fill_mode(FillMode::Both),
                );
                for index in 0..track_count {
                    let value = ctx.animated_value(0.0_f32);
                    let spec = AnimationSpec::new(
                        Keyframes::timed(Duration::from_millis(400))
                            .at(Duration::ZERO, index as f32)
                            .at(Duration::from_millis(200), index as f32 + 50.0)
                            .at(Duration::from_millis(400), index as f32 + 100.0)
                            .curve(AnimationCurve::EaseInOutCubic),
                    );
                    builder = builder.track(value, spec);
                }
                let handle = builder.build();
                handle.play();

                let percents = [0.0_f32, 0.15, 0.4, 0.6, 0.85, 1.0];
                let mut tick = 0usize;
                b.iter(|| {
                    let percent = percents[tick % percents.len()];
                    tick = tick.wrapping_add(1);
                    handle.seek_percent(black_box(percent));
                    black_box(handle.progress())
                });
            },
        );
    }

    group.finish();
}

fn bench_transition_clone(c: &mut Criterion) {
    let mut group = c.benchmark_group("transition_construction");

    group.bench_function("ease_in_out", |b| {
        b.iter(|| {
            let transition =
                Transition::ease_in_out(Duration::from_millis(black_box(240))).repeat(black_box(3));
            black_box(transition)
        });
    });

    group.bench_function("animated_value_set", |b| {
        let ctx = ViewModelContext::for_benchmarks();
        let value: AnimatedValue<f32> = ctx.animated_value(0.0);
        let mut counter = 0.0_f32;
        b.iter(|| {
            counter += 1.0;
            value.set(black_box(counter));
            black_box(value.get())
        });
    });

    group.finish();
}

criterion_group!(
    animation_engine_benches,
    bench_animation_curve_sample,
    bench_keyframes_sample_at,
    bench_animation_controller_seek,
    bench_transition_clone,
);
criterion_main!(animation_engine_benches);
