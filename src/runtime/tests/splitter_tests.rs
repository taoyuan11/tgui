use super::*;

use crate::foundation::binding::State;
use crate::platform::event::MouseButton;
use crate::ui::widget::{Pane, ResizablePanels, SplitterResize};

fn splitter_tree(sizes: State<Vec<f32>>) -> WidgetTree<TestVm> {
    let sizes_for_resize = sizes.clone();
    WidgetTree::new(
        ResizablePanels::new(
            vec![
                Pane::new(Stack::<TestVm>::new()),
                Pane::new(Stack::<TestVm>::new()),
            ],
            sizes.signal(),
        )
        .size(dp(200.0), dp(100.0))
        .on_resize(ValueCommand::new(
            move |_: &mut TestVm, resize: SplitterResize| {
                sizes_for_resize.set(resize.sizes);
            },
        )),
    )
}

fn splitter_handle_center(handler: &mut BoundRuntimeHandler<TestVm>) -> Point {
    handler
        .computed_scene()
        .hit_regions
        .iter()
        .find_map(|region| match &region.interaction {
            HitInteraction::SplitterHandle { .. } => Some(Point::new(
                region.rect.x + region.rect.width * 0.5,
                region.rect.y + region.rect.height * 0.5,
            )),
            _ => None,
        })
        .expect("splitter handle should be visible")
}

fn pointer_press(handler: &mut BoundRuntimeHandler<TestVm>, point: Point) {
    let event_loop = TestEventLoop;
    handler.handle_bound_window_event(
        &event_loop,
        WindowEvent::PointerButton {
            device_id: None,
            position: PhysicalPosition::new(f64::from(point.x.get()), f64::from(point.y.get())),
            state: ElementState::Pressed,
            button: ButtonSource::Mouse(MouseButton::Left),
            primary: true,
        },
    );
}

fn pointer_release(handler: &mut BoundRuntimeHandler<TestVm>, point: Point) {
    let event_loop = TestEventLoop;
    handler.handle_bound_window_event(
        &event_loop,
        WindowEvent::PointerButton {
            device_id: None,
            position: PhysicalPosition::new(f64::from(point.x.get()), f64::from(point.y.get())),
            state: ElementState::Released,
            button: ButtonSource::Mouse(MouseButton::Left),
            primary: true,
        },
    );
}

fn pointer_move(handler: &mut BoundRuntimeHandler<TestVm>, point: Point) {
    let event_loop = TestEventLoop;
    handler.handle_bound_window_event(
        &event_loop,
        WindowEvent::PointerMoved {
            device_id: None,
            position: PhysicalPosition::new(f64::from(point.x.get()), f64::from(point.y.get())),
            primary: true,
            source: PointerSource::Mouse,
        },
    );
}

fn flush_double_click_window(handler: &mut BoundRuntimeHandler<TestVm>) {
    let event_loop = TestEventLoop;
    let _ = handler.drive_animations(
        &event_loop,
        Instant::now() + crate::runtime::DOUBLE_CLICK_THRESHOLD + Duration::from_millis(10),
    );
}

fn assert_sizes_close(actual: &[f32], expected: &[f32]) {
    assert_eq!(actual.len(), expected.len());
    for (actual, expected) in actual.iter().zip(expected.iter()) {
        assert!(
            (actual - expected).abs() <= 0.0005,
            "expected {expected}, got {actual}"
        );
    }
}

#[test]
fn splitter_single_click_steps_after_double_click_window() {
    let invalidation = InvalidationSignal::new();
    let sizes = State::new(vec![0.42, 0.58], invalidation.clone());
    let mut handler = test_handler(Some(splitter_tree(sizes.clone())), invalidation);
    let point = splitter_handle_center(&mut handler);

    pointer_press(&mut handler, point);
    pointer_release(&mut handler, point);
    assert_sizes_close(&sizes.get(), &[0.42, 0.58]);

    flush_double_click_window(&mut handler);
    assert_sizes_close(&sizes.get(), &[0.47, 0.53]);
}

#[test]
fn splitter_double_click_resets_without_single_click_step() {
    let invalidation = InvalidationSignal::new();
    let sizes = State::new(vec![0.42, 0.58], invalidation.clone());
    let mut handler = test_handler(Some(splitter_tree(sizes.clone())), invalidation);
    let point = splitter_handle_center(&mut handler);

    pointer_press(&mut handler, point);
    pointer_release(&mut handler, point);
    pointer_press(&mut handler, point);
    pointer_release(&mut handler, point);

    assert_sizes_close(&sizes.get(), &[0.5, 0.5]);
    flush_double_click_window(&mut handler);
    assert_sizes_close(&sizes.get(), &[0.5, 0.5]);
}

#[test]
fn splitter_single_click_waits_for_release_before_pending_step() {
    let invalidation = InvalidationSignal::new();
    let sizes = State::new(vec![0.42, 0.58], invalidation.clone());
    let mut handler = test_handler(Some(splitter_tree(sizes.clone())), invalidation);
    let point = splitter_handle_center(&mut handler);

    pointer_press(&mut handler, point);
    flush_double_click_window(&mut handler);
    assert_sizes_close(&sizes.get(), &[0.42, 0.58]);

    pointer_release(&mut handler, point);
    flush_double_click_window(&mut handler);
    assert_sizes_close(&sizes.get(), &[0.47, 0.53]);
}

#[test]
fn splitter_drag_clears_pending_single_click_step() {
    let invalidation = InvalidationSignal::new();
    let sizes = State::new(vec![0.42, 0.58], invalidation.clone());
    let mut handler = test_handler(Some(splitter_tree(sizes.clone())), invalidation);
    let start = splitter_handle_center(&mut handler);
    let end = Point::new(start.x + dp(30.0), start.y);

    pointer_press(&mut handler, start);
    pointer_move(&mut handler, end);
    pointer_release(&mut handler, end);
    let dragged_sizes = sizes.get();

    flush_double_click_window(&mut handler);
    assert_sizes_close(&sizes.get(), &dragged_sizes);
}
