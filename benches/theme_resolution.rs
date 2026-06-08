use std::hint::black_box;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use tgui::core::Color;
use tgui::theme::{StateValue, Theme, ThemeMode, ThemeSet, ThemeStore, WidgetState};

fn make_color_state_value() -> StateValue<Color> {
    StateValue::interactive(
        Color::rgb(20, 20, 20),
        Color::rgb(40, 40, 40),
        Color::rgb(60, 60, 60),
        Color::rgb(120, 120, 120),
    )
}

fn bench_state_value_resolve(c: &mut Criterion) {
    let mut group = c.benchmark_group("state_value_resolve");
    let state_value = make_color_state_value();

    let states = [
        ("normal", WidgetState::default()),
        (
            "hovered",
            WidgetState {
                hovered: true,
                ..WidgetState::default()
            },
        ),
        (
            "pressed",
            WidgetState {
                pressed: true,
                hovered: true,
                ..WidgetState::default()
            },
        ),
        (
            "disabled",
            WidgetState {
                disabled: true,
                ..WidgetState::default()
            },
        ),
    ];

    for (label, state) in states {
        group.bench_function(BenchmarkId::new("color", label), |b| {
            b.iter(|| black_box(state_value.resolve(black_box(state))));
        });
    }

    group.finish();
}

fn bench_theme_set_resolve(c: &mut Criterion) {
    let mut group = c.benchmark_group("theme_set_resolve");
    let theme_set = ThemeSet::default();

    let modes = [
        ("light", ThemeMode::Light),
        ("dark", ThemeMode::Dark),
        ("system", ThemeMode::System),
    ];

    for (label, mode) in modes {
        group.bench_function(BenchmarkId::new("resolve", label), |b| {
            b.iter(|| {
                let resolved = theme_set.resolve(black_box(mode), None);
                black_box(resolved)
            });
        });
    }

    group.finish();
}

fn bench_theme_store_set_mode(c: &mut Criterion) {
    let mut group = c.benchmark_group("theme_store");

    group.bench_function("alternate_set_mode", |b| {
        let mut store = ThemeStore::new(ThemeSet::default(), ThemeMode::Light, None);
        let modes = [ThemeMode::Light, ThemeMode::Dark];
        let mut tick = 0usize;
        b.iter(|| {
            let mode = modes[tick % modes.len()];
            tick = tick.wrapping_add(1);
            black_box(store.set_mode(black_box(mode)));
            black_box(store.version())
        });
    });

    group.bench_function("clone_current", |b| {
        let store = ThemeStore::new(ThemeSet::default(), ThemeMode::Dark, None);
        b.iter(|| {
            let current = store.current();
            black_box(current.colors.primary)
        });
    });

    group.finish();
}

fn bench_theme_construction(c: &mut Criterion) {
    let mut group = c.benchmark_group("theme_construction");

    group.bench_function("light", |b| {
        b.iter(|| black_box(Theme::light()));
    });

    group.bench_function("dark", |b| {
        b.iter(|| black_box(Theme::dark()));
    });

    group.bench_function("from_mode_system", |b| {
        b.iter(|| {
            let theme = Theme::from_mode(black_box(ThemeMode::System), None);
            black_box(theme.name.len())
        });
    });

    group.finish();
}

criterion_group!(
    theme_resolution_benches,
    bench_state_value_resolve,
    bench_theme_set_resolve,
    bench_theme_store_set_mode,
    bench_theme_construction,
);
criterion_main!(theme_resolution_benches);
