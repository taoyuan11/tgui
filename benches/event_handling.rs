use std::hint::black_box;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};

#[cfg(feature = "bench-support")]
use std::sync::Arc;
#[cfg(feature = "bench-support")]
use std::time::Instant;

#[cfg(feature = "bench-support")]
use tgui::core::{dp, Point, Rect};
#[cfg(feature = "bench-support")]
use tgui::layout::{Axis, Insets, Overflow};
#[cfg(feature = "bench-support")]
use tgui::mvvm::{Command, ValueCommand, ViewModelContext};
#[cfg(feature = "bench-support")]
use tgui::widgets::bench_support_ext::{scope_bench_command, scope_bench_value_command};
#[cfg(feature = "bench-support")]
use tgui::widgets::{Button, Flex, Stack, Text, WidgetBenchmarkContext, WidgetTree};

#[cfg(feature = "bench-support")]
#[derive(Default)]
struct BenchVm {
    clicks: usize,
    last_x: f32,
}

#[cfg(feature = "bench-support")]
#[derive(Default)]
struct RootBenchVm {
    child: BenchVm,
}

#[cfg(feature = "bench-support")]
fn viewport() -> Rect {
    Rect::new(0.0, 0.0, 1280.0, 720.0)
}

#[cfg(feature = "bench-support")]
fn build_flat_interactive_tree(count: usize) -> WidgetTree<()> {
    let mut list = Flex::new(Axis::Vertical)
        .width(dp(960.0))
        .gap(dp(4.0))
        .padding(Insets::all(dp(8.0)));

    for index in 0..count {
        list = list.child(
            Flex::new(Axis::Horizontal)
                .width(dp(920.0))
                .height(dp(34.0))
                .gap(dp(8.0))
                .padding(Insets::symmetric(dp(8.0), dp(4.0)))
                .child(Text::new(format!("Target {index:04}")))
                .child(Button::new("Open").size(dp(84.0), dp(26.0))),
        );
    }

    WidgetTree::new(
        Flex::new(Axis::Vertical)
            .width(dp(1000.0))
            .height(dp(720.0))
            .padding(Insets::all(dp(16.0)))
            .child(list),
    )
}

#[cfg(feature = "bench-support")]
fn build_flat_interactive_tree_with_isolated_transform(count: usize) -> WidgetTree<()> {
    let view_model = ViewModelContext::for_benchmarks();
    let marker_offset = view_model.state(Point::new(dp(12.0), dp(12.0)));
    let mut list = Flex::new(Axis::Vertical)
        .width(dp(960.0))
        .gap(dp(4.0))
        .padding(Insets::all(dp(8.0)));

    for index in 0..count {
        list = list.child(
            Flex::new(Axis::Horizontal)
                .width(dp(920.0))
                .height(dp(34.0))
                .gap(dp(8.0))
                .padding(Insets::symmetric(dp(8.0), dp(4.0)))
                .child(Text::new(format!("Target {index:04}")))
                .child(Button::new("Open").size(dp(84.0), dp(26.0))),
        );
    }

    WidgetTree::new(
        Stack::new()
            .size(dp(1000.0), dp(720.0))
            .child(list)
            // This tiny reactive-offset subtree is intentionally independent from the dense
            // list. It models a badge/indicator animation that must not disable indexed hit
            // testing for every unrelated row in the same scene.
            .child(
                Flex::new(Axis::Horizontal)
                    .size(dp(80.0), dp(24.0))
                    .overflow(Overflow::Visible)
                    .offset(marker_offset.signal())
                    .child(Text::new("Live").size(dp(48.0), dp(20.0))),
            ),
    )
}

#[cfg(feature = "bench-support")]
fn build_nested_interactive_tree(depth: usize) -> WidgetTree<()> {
    let mut node = Flex::new(Axis::Horizontal)
        .width(dp(360.0))
        .height(dp(34.0))
        .gap(dp(8.0))
        .padding(Insets::symmetric(dp(8.0), dp(4.0)))
        .child(Text::new("Nested target"))
        .child(Button::new("Open").size(dp(84.0), dp(26.0)));

    for level in 0..depth {
        node = Flex::new(Axis::Vertical)
            .width(dp(420.0 + level as f32 * 10.0))
            .gap(dp(4.0))
            .padding(Insets::all(dp(4.0)))
            .child(Text::new(format!("Layer {level}")))
            .child(node);
    }

    WidgetTree::new(
        Flex::new(Axis::Vertical)
            .width(dp(900.0))
            .height(dp(720.0))
            .padding(Insets::all(dp(16.0)))
            .child(node),
    )
}

#[cfg(feature = "bench-support")]
fn build_scrollable_interactive_tree(count: usize) -> WidgetTree<()> {
    let mut content = Flex::new(Axis::Vertical)
        .width(dp(960.0))
        .gap(dp(4.0))
        .padding(Insets::all(dp(8.0)));

    for index in 0..count {
        content = content.child(
            Flex::new(Axis::Horizontal)
                .width(dp(920.0))
                .height(dp(32.0))
                .gap(dp(8.0))
                .padding(Insets::symmetric(dp(8.0), dp(3.0)))
                .child(Text::new(format!("Scrollable {index:04}")))
                .child(Button::new("Inspect").size(dp(92.0), dp(24.0))),
        );
    }

    WidgetTree::new(
        Flex::new(Axis::Vertical)
            .width(dp(1040.0))
            .height(dp(720.0))
            .padding(Insets::all(dp(16.0)))
            .child(
                Flex::new(Axis::Vertical)
                    .width(dp(1000.0))
                    .height(dp(540.0))
                    .overflow_y(Overflow::Scroll)
                    .child(content),
            ),
    )
}

#[cfg(feature = "bench-support")]
fn bench_hit_test_flat(c: &mut Criterion) {
    let mut group = c.benchmark_group("event_cached_hit_path_flat");

    for widget_count in [10_usize, 50, 100, 200, 500, 1_000, 10_000] {
        let tree = build_flat_interactive_tree(widget_count);
        group.bench_with_input(
            BenchmarkId::from_parameter(widget_count),
            &widget_count,
            |b, _| {
                let mut ctx = WidgetBenchmarkContext::new().with_viewport(viewport());
                let _ = ctx.run_layout_and_scene(&tree, Instant::now());
                b.iter(|| {
                    let result = ctx.cached_hit_path_len(
                        black_box(&tree),
                        black_box(Point::new(720.0, 360.0)),
                        Instant::now(),
                    );
                    black_box(result);
                });
            },
        );
    }

    group.finish();
}

#[cfg(feature = "bench-support")]
fn bench_hit_test_flat_full_scan(c: &mut Criterion) {
    let mut group = c.benchmark_group("event_cached_hit_path_flat_full_scan");

    for widget_count in [1_000_usize, 10_000] {
        let tree = build_flat_interactive_tree(widget_count);
        group.bench_with_input(
            BenchmarkId::from_parameter(widget_count),
            &widget_count,
            |b, _| {
                let mut ctx = WidgetBenchmarkContext::new().with_viewport(viewport());
                let _ = ctx.run_layout_and_scene(&tree, Instant::now());
                b.iter(|| {
                    let result = ctx.cached_hit_path_len_full_scan(
                        black_box(&tree),
                        black_box(Point::new(720.0, 360.0)),
                        Instant::now(),
                    );
                    black_box(result);
                });
            },
        );
    }

    group.finish();
}

#[cfg(feature = "bench-support")]
fn bench_hit_test_isolated_transform(c: &mut Criterion) {
    let mut indexed = c.benchmark_group("event_cached_hit_path_isolated_transform");
    for widget_count in [1_000_usize, 10_000] {
        let tree = build_flat_interactive_tree_with_isolated_transform(widget_count);
        indexed.bench_with_input(
            BenchmarkId::from_parameter(widget_count),
            &widget_count,
            |b, _| {
                let mut ctx = WidgetBenchmarkContext::new().with_viewport(viewport());
                let now = Instant::now();
                let _ = ctx.run_layout_and_scene(&tree, now);
                assert!(
                    ctx.cached_transform_record_count(&tree, now) > 0,
                    "fixture must contain a retained transform"
                );
                b.iter(|| {
                    black_box(ctx.cached_hit_path_len(
                        black_box(&tree),
                        black_box(Point::new(720.0, 360.0)),
                        Instant::now(),
                    ));
                });
            },
        );
    }
    indexed.finish();

    let mut full_scan = c.benchmark_group("event_cached_hit_path_isolated_transform_full_scan");
    for widget_count in [1_000_usize, 10_000] {
        let tree = build_flat_interactive_tree_with_isolated_transform(widget_count);
        full_scan.bench_with_input(
            BenchmarkId::from_parameter(widget_count),
            &widget_count,
            |b, _| {
                let mut ctx = WidgetBenchmarkContext::new().with_viewport(viewport());
                let now = Instant::now();
                let _ = ctx.run_layout_and_scene(&tree, now);
                assert!(ctx.cached_transform_record_count(&tree, now) > 0);
                b.iter(|| {
                    black_box(ctx.cached_hit_path_len_full_scan(
                        black_box(&tree),
                        black_box(Point::new(720.0, 360.0)),
                        Instant::now(),
                    ));
                });
            },
        );
    }
    full_scan.finish();
}

#[cfg(feature = "bench-support")]
fn bench_hit_test_nested(c: &mut Criterion) {
    let mut group = c.benchmark_group("event_cached_hit_path_nested");

    for depth in [2_usize, 4, 8, 12, 16] {
        let tree = build_nested_interactive_tree(depth);
        group.bench_with_input(BenchmarkId::from_parameter(depth), &depth, |b, _| {
            let mut ctx = WidgetBenchmarkContext::new().with_viewport(viewport());
            let _ = ctx.run_layout_and_scene(&tree, Instant::now());
            b.iter(|| {
                let result = ctx.cached_hit_path_len(
                    black_box(&tree),
                    black_box(Point::new(240.0, 180.0)),
                    Instant::now(),
                );
                black_box(result);
            });
        });
    }

    group.finish();
}

#[cfg(feature = "bench-support")]
fn bench_hover_state_update(c: &mut Criterion) {
    let tree = build_flat_interactive_tree(200);
    let mut ctx = WidgetBenchmarkContext::new().with_viewport(viewport());
    let _ = ctx.run_layout_and_scene(&tree, Instant::now());

    c.bench_function("event_pointer_move_hit_path_sweep", |b| {
        let mut x = 32.0_f32;
        b.iter(|| {
            x = if x > 920.0 { 32.0 } else { x + 17.0 };
            let hit_len = ctx.cached_hit_path_len(
                black_box(&tree),
                black_box(Point::new(x, 360.0)),
                Instant::now(),
            );
            black_box(hit_len);
        });
    });
}

#[cfg(feature = "bench-support")]
fn bench_focus_navigation(c: &mut Criterion) {
    let command = Command::new(|vm: &mut BenchVm| {
        vm.clicks = vm.clicks.wrapping_add(1);
    });
    let mut vm = BenchVm::default();

    c.bench_function("event_command_execute_plain", |b| {
        b.iter(|| {
            command.execute(black_box(&mut vm));
            black_box(&vm);
        });
    });
}

#[cfg(feature = "bench-support")]
fn bench_mouse_event_dispatch(c: &mut Criterion) {
    let mut group = c.benchmark_group("event_hit_path_then_command");
    let command = Command::new(|vm: &mut BenchVm| {
        vm.clicks = vm.clicks.wrapping_add(1);
    });

    for widget_count in [50_usize, 100, 200, 500] {
        let tree = build_flat_interactive_tree(widget_count);
        group.bench_with_input(
            BenchmarkId::from_parameter(widget_count),
            &widget_count,
            |b, _| {
                let mut ctx = WidgetBenchmarkContext::new().with_viewport(viewport());
                let _ = ctx.run_layout_and_scene(&tree, Instant::now());
                let mut vm = BenchVm::default();
                b.iter(|| {
                    let hit_len = ctx.cached_hit_path_len(
                        black_box(&tree),
                        black_box(Point::new(720.0, 360.0)),
                        Instant::now(),
                    );
                    if hit_len > 0 {
                        command.execute(&mut vm);
                    }
                    black_box((&vm, hit_len));
                });
            },
        );
    }

    group.finish();
}

#[cfg(feature = "bench-support")]
fn bench_keyboard_event_dispatch(c: &mut Criterion) {
    let command = ValueCommand::new(|vm: &mut BenchVm, point: Point| {
        vm.last_x = point.x.get();
        vm.clicks = vm.clicks.wrapping_add(1);
    });
    let mut vm = BenchVm::default();

    c.bench_function("event_value_command_execute_point", |b| {
        b.iter(|| {
            command.execute(black_box(&mut vm), black_box(Point::new(42.0, 12.0)));
            black_box(&vm);
        });
    });
}

#[cfg(feature = "bench-support")]
fn bench_command_dispatch(c: &mut Criterion) {
    let mut group = c.benchmark_group("event_command_execute_batch");
    let command = Command::new(|vm: &mut BenchVm| {
        vm.clicks = vm.clicks.wrapping_add(1);
    });

    for command_count in [1_usize, 5, 10, 20, 50] {
        group.bench_with_input(
            BenchmarkId::from_parameter(command_count),
            &command_count,
            |b, &count| {
                let mut vm = BenchVm::default();
                b.iter(|| {
                    for _ in 0..count {
                        command.execute(&mut vm);
                    }
                    black_box(&vm);
                });
            },
        );
    }

    group.finish();
}

#[cfg(feature = "bench-support")]
fn bench_scoped_command_dispatch(c: &mut Criterion) {
    let command = scope_bench_command(
        Command::new(|vm: &mut BenchVm| {
            vm.clicks = vm.clicks.wrapping_add(1);
        }),
        Arc::new(|vm: &mut RootBenchVm| &mut vm.child),
    );
    let mut vm = RootBenchVm::default();

    c.bench_function("event_scoped_command_execute_plain", |b| {
        b.iter(|| {
            command.execute(black_box(&mut vm));
            black_box(&vm);
        });
    });
}

#[cfg(feature = "bench-support")]
fn bench_scoped_value_command_dispatch(c: &mut Criterion) {
    let command = scope_bench_value_command(
        ValueCommand::new(|vm: &mut BenchVm, point: Point| {
            vm.last_x = point.x.get();
            vm.clicks = vm.clicks.wrapping_add(1);
        }),
        Arc::new(|vm: &mut RootBenchVm| &mut vm.child),
    );
    let mut vm = RootBenchVm::default();

    c.bench_function("event_scoped_value_command_execute_point", |b| {
        b.iter(|| {
            command.execute(black_box(&mut vm), black_box(Point::new(42.0, 12.0)));
            black_box(&vm);
        });
    });
}

#[cfg(feature = "bench-support")]
fn bench_scroll_event_handling(c: &mut Criterion) {
    let tree = build_scrollable_interactive_tree(500);
    let mut ctx = WidgetBenchmarkContext::new().with_viewport(viewport());
    let _ = ctx.run_layout_and_scene(&tree, Instant::now());

    c.bench_function("event_scroll_container_scene_recollect", |b| {
        b.iter(|| {
            let stats = ctx.recollect_scene_only(black_box(&tree), Instant::now());
            black_box(stats);
        });
    });
}

#[cfg(feature = "bench-support")]
fn bench_scroll_region_target_lookup(c: &mut Criterion) {
    let mut indexed = c.benchmark_group("event_scroll_region_target_indexed");
    for widget_count in [1_000_usize, 10_000] {
        let tree = build_scrollable_interactive_tree(widget_count);
        indexed.bench_with_input(
            BenchmarkId::from_parameter(widget_count),
            &widget_count,
            |b, _| {
                let mut ctx = WidgetBenchmarkContext::new().with_viewport(viewport());
                let scene_stats = ctx.run_layout_and_scene(&tree, Instant::now());
                let point = Point::new(40.0, 40.0);
                let delta = Point::new(0.0, -48.0);
                let lookup_stats = ctx.prepared_scroll_region_lookup_stats();
                let indexed_probe = ctx.prepared_scroll_target_stats(point, delta, true);
                let full_scan_probe = ctx.prepared_scroll_target_stats(point, delta, false);
                assert_eq!(lookup_stats.region_count, scene_stats.scroll_region_count);
                assert!(lookup_stats.uses_index);
                assert_eq!(lookup_stats.scrollable_candidate_count, 1);
                assert_eq!(indexed_probe.candidate_visits, 1);
                assert!(indexed_probe.found_target);
                assert!(full_scan_probe.candidate_visits > indexed_probe.candidate_visits);
                assert_eq!(
                    ctx.prepared_scroll_target(point, delta),
                    ctx.prepared_scroll_target_full_scan(point, delta)
                );
                b.iter(|| {
                    black_box(ctx.prepared_scroll_target(black_box(point), black_box(delta)));
                });
            },
        );
    }
    indexed.finish();

    let mut full_scan = c.benchmark_group("event_scroll_region_target_full_scan");
    for widget_count in [1_000_usize, 10_000] {
        let tree = build_scrollable_interactive_tree(widget_count);
        full_scan.bench_with_input(
            BenchmarkId::from_parameter(widget_count),
            &widget_count,
            |b, _| {
                let mut ctx = WidgetBenchmarkContext::new().with_viewport(viewport());
                let scene_stats = ctx.run_layout_and_scene(&tree, Instant::now());
                let point = Point::new(40.0, 40.0);
                let delta = Point::new(0.0, -48.0);
                let lookup_stats = ctx.prepared_scroll_region_lookup_stats();
                assert_eq!(lookup_stats.region_count, scene_stats.scroll_region_count);
                assert!(lookup_stats.uses_index);
                b.iter(|| {
                    black_box(
                        ctx.prepared_scroll_target_full_scan(black_box(point), black_box(delta)),
                    );
                });
            },
        );
    }
    full_scan.finish();

    // Diagnostic control: this intentionally includes WidgetBenchmarkContext cache synchronization
    // and animation-cache maintenance. Keeping it separate prevents the 8,192-slot animation GC
    // threshold from being attributed to the production scroll-region lookup itself.
    let mut cached_sync = c.benchmark_group("event_scroll_region_target_cached_sync");
    for widget_count in [1_000_usize, 10_000] {
        let tree = build_scrollable_interactive_tree(widget_count);
        cached_sync.bench_with_input(
            BenchmarkId::from_parameter(widget_count),
            &widget_count,
            |b, _| {
                let mut ctx = WidgetBenchmarkContext::new().with_viewport(viewport());
                let _ = ctx.run_layout_and_scene(&tree, Instant::now());
                let point = Point::new(40.0, 40.0);
                let delta = Point::new(0.0, -48.0);
                let _ = ctx.cached_scroll_target(&tree, point, delta, Instant::now());
                b.iter(|| {
                    black_box(ctx.cached_scroll_target(
                        black_box(&tree),
                        black_box(point),
                        black_box(delta),
                        Instant::now(),
                    ));
                });
            },
        );
    }
    cached_sync.finish();
}

#[cfg(feature = "bench-support")]
fn bench_drag_tracking(c: &mut Criterion) {
    let tree = build_flat_interactive_tree(200);
    let mut ctx = WidgetBenchmarkContext::new().with_viewport(viewport());
    let _ = ctx.run_layout_and_scene(&tree, Instant::now());

    c.bench_function("event_drag_pointer_hit_path_sweep", |b| {
        let mut x = 80.0_f32;
        let mut y = 80.0_f32;
        b.iter(|| {
            x = if x > 900.0 { 80.0 } else { x + 13.0 };
            y = if y > 620.0 { 80.0 } else { y + 7.0 };
            let hit_len = ctx.cached_hit_path_len(
                black_box(&tree),
                black_box(Point::new(x, y)),
                Instant::now(),
            );
            black_box(hit_len);
        });
    });
}

#[cfg(feature = "bench-support")]
fn bench_gesture_recognition(c: &mut Criterion) {
    let command = ValueCommand::new(|vm: &mut BenchVm, delta: (f32, f32)| {
        vm.last_x = delta.0;
        vm.clicks = vm.clicks.wrapping_add(delta.1 as usize);
    });
    let mut vm = BenchVm::default();

    c.bench_function("event_value_command_execute_tuple", |b| {
        b.iter(|| {
            command.execute(black_box(&mut vm), black_box((2.0, 1.0)));
            black_box(&vm);
        });
    });
}

#[cfg(feature = "bench-support")]
fn bench_event_bubbling(c: &mut Criterion) {
    let mut group = c.benchmark_group("event_nested_hit_path_as_bubble_chain");

    for depth in [2_usize, 4, 8, 12] {
        let tree = build_nested_interactive_tree(depth);
        group.bench_with_input(BenchmarkId::from_parameter(depth), &depth, |b, _| {
            let mut ctx = WidgetBenchmarkContext::new().with_viewport(viewport());
            let _ = ctx.run_layout_and_scene(&tree, Instant::now());
            b.iter(|| {
                let hit_len = ctx.cached_hit_path_len(
                    black_box(&tree),
                    black_box(Point::new(240.0, 180.0)),
                    Instant::now(),
                );
                black_box(hit_len);
            });
        });
    }

    group.finish();
}

#[cfg(not(feature = "bench-support"))]
fn bench_hit_test_flat(_c: &mut Criterion) {
    eprintln!("Skipping event_handling benchmarks: bench-support feature not enabled");
}

#[cfg(not(feature = "bench-support"))]
fn bench_hit_test_flat_full_scan(_c: &mut Criterion) {}

#[cfg(not(feature = "bench-support"))]
fn bench_hit_test_isolated_transform(_c: &mut Criterion) {}

#[cfg(not(feature = "bench-support"))]
fn bench_hit_test_nested(_c: &mut Criterion) {}

#[cfg(not(feature = "bench-support"))]
fn bench_hover_state_update(_c: &mut Criterion) {}

#[cfg(not(feature = "bench-support"))]
fn bench_focus_navigation(_c: &mut Criterion) {}

#[cfg(not(feature = "bench-support"))]
fn bench_mouse_event_dispatch(_c: &mut Criterion) {}

#[cfg(not(feature = "bench-support"))]
fn bench_keyboard_event_dispatch(_c: &mut Criterion) {}

#[cfg(not(feature = "bench-support"))]
fn bench_command_dispatch(_c: &mut Criterion) {}

#[cfg(not(feature = "bench-support"))]
fn bench_scoped_command_dispatch(_c: &mut Criterion) {}

#[cfg(not(feature = "bench-support"))]
fn bench_scoped_value_command_dispatch(_c: &mut Criterion) {}

#[cfg(not(feature = "bench-support"))]
fn bench_scroll_event_handling(_c: &mut Criterion) {}

#[cfg(not(feature = "bench-support"))]
fn bench_scroll_region_target_lookup(_c: &mut Criterion) {}

#[cfg(not(feature = "bench-support"))]
fn bench_drag_tracking(_c: &mut Criterion) {}

#[cfg(not(feature = "bench-support"))]
fn bench_gesture_recognition(_c: &mut Criterion) {}

#[cfg(not(feature = "bench-support"))]
fn bench_event_bubbling(_c: &mut Criterion) {}

criterion_group!(
    benches,
    bench_hit_test_flat,
    bench_hit_test_flat_full_scan,
    bench_hit_test_isolated_transform,
    bench_hit_test_nested,
    bench_hover_state_update,
    bench_focus_navigation,
    bench_mouse_event_dispatch,
    bench_keyboard_event_dispatch,
    bench_command_dispatch,
    bench_scoped_command_dispatch,
    bench_scoped_value_command_dispatch,
    bench_scroll_event_handling,
    bench_scroll_region_target_lookup,
    bench_drag_tracking,
    bench_gesture_recognition,
    bench_event_bubbling,
);
criterion_main!(benches);
