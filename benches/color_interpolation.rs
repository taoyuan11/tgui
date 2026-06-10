use std::hint::black_box;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use tgui::animation::Animatable;
use tgui::core::{dp, Color, Point};
use tgui::layout::Insets;

const PROGRESS_SAMPLES: [f32; 16] = [
    0.0, 0.05, 0.12, 0.2, 0.27, 0.33, 0.4, 0.5, 0.6, 0.67, 0.74, 0.8, 0.87, 0.93, 0.99, 1.0,
];

fn bench_color_interpolate(c: &mut Criterion) {
    let mut group = c.benchmark_group("color_interpolate");

    let cases = [
        (
            "opaque_to_opaque",
            Color::rgb(20, 50, 80),
            Color::rgb(220, 130, 60),
        ),
        (
            "transparent_to_opaque",
            Color::rgba(0, 0, 0, 0),
            Color::rgba(255, 64, 96, 255),
        ),
        (
            "premultiplied_blend",
            Color::rgba(40, 80, 200, 64),
            Color::rgba(240, 200, 40, 220),
        ),
    ];

    for (label, from, to) in cases {
        group.bench_with_input(BenchmarkId::new("interp_16", label), &label, |b, _| {
            b.iter(|| {
                let mut acc_r: u32 = 0;
                for progress in PROGRESS_SAMPLES {
                    let mixed = Color::interpolate(&from, &to, black_box(progress));
                    acc_r = acc_r.wrapping_add(mixed.r as u32);
                }
                black_box(acc_r)
            });
        });
    }

    group.finish();
}

fn bench_color_lighten_darken(c: &mut Criterion) {
    let mut group = c.benchmark_group("color_lighten_darken");
    let base = Color::rgb(72, 144, 200);

    group.bench_function("lighten_16_steps", |b| {
        b.iter(|| {
            let mut acc: u32 = 0;
            for progress in PROGRESS_SAMPLES {
                let value = base.lighten(black_box(progress));
                acc = acc.wrapping_add(value.r as u32);
            }
            black_box(acc)
        });
    });

    group.bench_function("darken_16_steps", |b| {
        b.iter(|| {
            let mut acc: u32 = 0;
            for progress in PROGRESS_SAMPLES {
                let value = base.darken(black_box(progress));
                acc = acc.wrapping_add(value.r as u32);
            }
            black_box(acc)
        });
    });

    group.finish();
}

fn bench_dp_and_point_interpolate(c: &mut Criterion) {
    let mut group = c.benchmark_group("geometry_interpolate");

    let from_dp = dp(-12.0);
    let to_dp = dp(220.0);

    group.bench_function("dp_16_steps", |b| {
        b.iter(|| {
            let mut acc = dp(0.0);
            for progress in PROGRESS_SAMPLES {
                acc +=
                    tgui::animation::Animatable::interpolate(&from_dp, &to_dp, black_box(progress));
            }
            black_box(acc)
        });
    });

    let from_point = Point::new(dp(-40.0), dp(-12.0));
    let to_point = Point::new(dp(180.0), dp(420.0));

    group.bench_function("point_16_steps", |b| {
        b.iter(|| {
            let mut sum_x = dp(0.0);
            for progress in PROGRESS_SAMPLES {
                let value = Point::interpolate(&from_point, &to_point, black_box(progress));
                sum_x += value.x;
            }
            black_box(sum_x)
        });
    });

    let from_insets = Insets::all(dp(0.0));
    let to_insets = Insets::all(dp(40.0));

    group.bench_function("insets_16_steps", |b| {
        b.iter(|| {
            let mut acc = dp(0.0);
            for progress in PROGRESS_SAMPLES {
                let value = Insets::interpolate(&from_insets, &to_insets, black_box(progress));
                acc += value.left;
            }
            black_box(acc)
        });
    });

    group.finish();
}

criterion_group!(
    color_interpolation_benches,
    bench_color_interpolate,
    bench_color_lighten_darken,
    bench_dp_and_point_interpolate,
);
criterion_main!(color_interpolation_benches);
