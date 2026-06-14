// 事件处理基准测试
// 覆盖输入事件处理、命中测试、焦点管理、命令派发等热路径

use std::hint::black_box;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};

#[cfg(feature = "bench-support")]
use tgui::widgets::bench_support_ext::*;

#[cfg(feature = "bench-support")]
fn bench_hit_test_flat(c: &mut Criterion) {
    let mut group = c.benchmark_group("hit_test_flat");

    for widget_count in [10, 50, 100, 200, 500, 1000].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(widget_count),
            widget_count,
            |b, &count| {
                let tree = create_flat_element_tree(count);
                let layout = compute_layout(&tree, (1920.0, 1080.0));
                let hit_regions = build_hit_regions(&layout);

                b.iter(|| {
                    let result = hit_test(&hit_regions, black_box((500.0, 500.0)));
                    black_box(result);
                });
            },
        );
    }
    group.finish();
}

#[cfg(feature = "bench-support")]
fn bench_hit_test_nested(c: &mut Criterion) {
    let mut group = c.benchmark_group("hit_test_nested");

    for depth in [2, 4, 8, 12, 16].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(depth), depth, |b, &depth| {
            let tree = create_nested_element_tree(depth);
            let layout = compute_layout(&tree, (1920.0, 1080.0));
            let hit_regions = build_hit_regions(&layout);

            b.iter(|| {
                let result = hit_test(&hit_regions, black_box((500.0, 500.0)));
                black_box(result);
            });
        });
    }
    group.finish();
}

#[cfg(feature = "bench-support")]
fn bench_hover_state_update(c: &mut Criterion) {
    let tree = create_flat_element_tree(100);
    let mut runtime = create_runtime_state(&tree);

    c.bench_function("hover_state_update", |b| {
        b.iter(|| {
            update_hover_state(&mut runtime, black_box((600.0, 400.0)));
            black_box(&runtime);
        });
    });
}

#[cfg(feature = "bench-support")]
fn bench_focus_navigation(c: &mut Criterion) {
    let tree = create_focusable_tree(50);
    let mut runtime = create_runtime_state(&tree);

    c.bench_function("focus_navigation", |b| {
        b.iter(|| {
            navigate_focus(&mut runtime, black_box(FocusDirection::Next));
            black_box(&runtime);
        });
    });
}

#[cfg(feature = "bench-support")]
fn bench_mouse_event_dispatch(c: &mut Criterion) {
    let mut group = c.benchmark_group("mouse_event_dispatch");

    for widget_count in [50, 100, 200, 500].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(widget_count),
            widget_count,
            |b, &count| {
                let tree = create_flat_element_tree(count);
                let mut runtime = create_runtime_state(&tree);

                b.iter(|| {
                    let event = create_mouse_event(MouseEventType::Click, (500.0, 500.0));
                    dispatch_mouse_event(&mut runtime, black_box(event));
                    black_box(&runtime);
                });
            },
        );
    }
    group.finish();
}

#[cfg(feature = "bench-support")]
fn bench_keyboard_event_dispatch(c: &mut Criterion) {
    let tree = create_flat_element_tree(100);
    let mut runtime = create_runtime_state(&tree);
    set_focus(&mut runtime, 50);

    c.bench_function("keyboard_event_dispatch", |b| {
        b.iter(|| {
            let event = create_keyboard_event(KeyCode::A, Modifiers::NONE);
            dispatch_keyboard_event(&mut runtime, black_box(event));
            black_box(&runtime);
        });
    });
}

#[cfg(feature = "bench-support")]
fn bench_command_dispatch(c: &mut Criterion) {
    let mut group = c.benchmark_group("command_dispatch");

    for command_count in [1, 5, 10, 20, 50].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(command_count),
            command_count,
            |b, &count| {
                let mut runtime = create_runtime_state(&create_flat_element_tree(100));

                b.iter(|| {
                    for _ in 0..count {
                        dispatch_command(&mut runtime, black_box(create_test_command()));
                    }
                    black_box(&runtime);
                });
            },
        );
    }
    group.finish();
}

#[cfg(feature = "bench-support")]
fn bench_scroll_event_handling(c: &mut Criterion) {
    let tree = create_scrollable_tree();
    let mut runtime = create_runtime_state(&tree);

    c.bench_function("scroll_event_handling", |b| {
        b.iter(|| {
            let event = create_scroll_event(0.0, black_box(-50.0));
            handle_scroll_event(&mut runtime, event);
            black_box(&runtime);
        });
    });
}

#[cfg(feature = "bench-support")]
fn bench_drag_tracking(c: &mut Criterion) {
    let tree = create_flat_element_tree(100);
    let mut runtime = create_runtime_state(&tree);

    c.bench_function("drag_tracking", |b| {
        let mut x = 0.0;
        b.iter(|| {
            x += 1.0;
            update_drag_state(&mut runtime, black_box((x, 300.0)));
            black_box(&runtime);
        });
    });
}

#[cfg(feature = "bench-support")]
fn bench_gesture_recognition(c: &mut Criterion) {
    let mut gesture_state = create_gesture_recognizer();

    c.bench_function("gesture_recognition", |b| {
        b.iter(|| {
            let touch_event = create_touch_event(TouchPhase::Moved, (500.0, 500.0));
            recognize_gesture(&mut gesture_state, black_box(touch_event));
            black_box(&gesture_state);
        });
    });
}

#[cfg(feature = "bench-support")]
fn bench_event_bubbling(c: &mut Criterion) {
    let mut group = c.benchmark_group("event_bubbling");

    for depth in [2, 4, 8, 12].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(depth), depth, |b, &depth| {
            let tree = create_nested_element_tree(depth);
            let mut runtime = create_runtime_state(&tree);

            b.iter(|| {
                let event = create_mouse_event(MouseEventType::Click, (500.0, 500.0));
                bubble_event(&mut runtime, black_box(event));
                black_box(&runtime);
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
fn bench_scroll_event_handling(_c: &mut Criterion) {}

#[cfg(not(feature = "bench-support"))]
fn bench_drag_tracking(_c: &mut Criterion) {}

#[cfg(not(feature = "bench-support"))]
fn bench_gesture_recognition(_c: &mut Criterion) {}

#[cfg(not(feature = "bench-support"))]
fn bench_event_bubbling(_c: &mut Criterion) {}

criterion_group!(
    benches,
    bench_hit_test_flat,
    bench_hit_test_nested,
    bench_hover_state_update,
    bench_focus_navigation,
    bench_mouse_event_dispatch,
    bench_keyboard_event_dispatch,
    bench_command_dispatch,
    bench_scroll_event_handling,
    bench_drag_tracking,
    bench_gesture_recognition,
    bench_event_bubbling,
);
criterion_main!(benches);
