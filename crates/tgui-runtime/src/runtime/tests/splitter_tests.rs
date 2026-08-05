use super::*;

use crate::animation::Transition;
use crate::foundation::binding::State;
use crate::platform::cursor::CursorIcon;
use crate::platform::event::MouseButton;
use crate::ui::widget::{Pane, ResizablePanels, SplitterResize};

fn splitter_tree(sizes: State<Vec<f32>>) -> WidgetTree<TestVm> {
    splitter_tree_with_sizes(sizes.clone(), sizes.signal())
}

fn splitter_tree_with_sizes(
    sizes: State<Vec<f32>>,
    sizes_value: impl Into<crate::ui::layout::Value<Vec<f32>>>,
) -> WidgetTree<TestVm> {
    let sizes_for_resize = sizes.clone();
    WidgetTree::new(
        ResizablePanels::new(
            vec![
                Pane::new(Stack::<TestVm>::new()),
                Pane::new(Stack::<TestVm>::new()),
            ],
            sizes_value,
        )
        .size(dp(200.0), dp(100.0))
        .on_resize(ValueCommand::new(
            move |_: &mut TestVm, resize: SplitterResize| {
                sizes_for_resize.set(resize.sizes);
            },
        )),
    )
}

fn uncontrolled_splitter_tree() -> WidgetTree<TestVm> {
    WidgetTree::new(
        ResizablePanels::new(
            vec![
                Pane::new(Stack::<TestVm>::new()),
                Pane::new(Stack::<TestVm>::new()),
            ],
            vec![0.42, 0.58],
        )
        .size(dp(200.0), dp(100.0)),
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

#[test]
fn splitter_drag_updates_handle_layout_from_sizes_signal() {
    let invalidation = InvalidationSignal::new();
    let sizes = State::new(vec![0.42, 0.58], invalidation.clone());
    let mut handler = test_handler(Some(splitter_tree(sizes)), invalidation);
    let start = splitter_handle_center(&mut handler);
    let end = Point::new(start.x + dp(30.0), start.y);

    pointer_press(&mut handler, start);
    pointer_move(&mut handler, end);

    let moved = splitter_handle_center(&mut handler);
    assert!(
        moved.x > start.x + dp(10.0),
        "splitter handle should move after dragging; start={start:?}, moved={moved:?}"
    );
}

#[test]
fn splitter_static_sizes_drag_without_callback_updates_layout() {
    let invalidation = InvalidationSignal::new();
    let mut handler = test_handler(Some(uncontrolled_splitter_tree()), invalidation);
    let start = splitter_handle_center(&mut handler);
    let end = Point::new(start.x + dp(30.0), start.y);

    pointer_press(&mut handler, start);
    pointer_move(&mut handler, end);

    let moved = splitter_handle_center(&mut handler);
    assert!(
        moved.x > start.x + dp(10.0),
        "an uncontrolled splitter should retain its dragged size; start={start:?}, moved={moved:?}"
    );
}

#[test]
fn splitter_static_sizes_keyboard_without_callback_updates_layout() {
    let invalidation = InvalidationSignal::new();
    let mut handler = test_handler(Some(uncontrolled_splitter_tree()), invalidation);
    let start = splitter_handle_center(&mut handler);

    assert!(handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Tab))));
    assert!(
        handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::ArrowRight)))
    );

    let moved = splitter_handle_center(&mut handler);
    assert!(
        moved.x > start.x,
        "an uncontrolled splitter should retain keyboard adjustments; start={start:?}, moved={moved:?}"
    );
}

#[test]
fn splitter_controlled_sizes_without_callback_are_inert() {
    let invalidation = InvalidationSignal::new();
    let sizes = State::new(vec![0.42, 0.58], invalidation.clone());
    let tree = WidgetTree::new(
        ResizablePanels::new(
            vec![
                Pane::new(Stack::<TestVm>::new()),
                Pane::new(Stack::<TestVm>::new()),
            ],
            sizes.signal(),
        )
        .size(dp(200.0), dp(100.0)),
    );
    let mut handler = test_handler(Some(tree), invalidation);

    assert!(!handler
        .computed_scene()
        .hit_regions
        .iter()
        .any(|region| matches!(&region.interaction, HitInteraction::SplitterHandle { .. })));

    let _ = handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Tab)));
    let _ =
        handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::ArrowRight)));
    assert_eq!(handler.focused_widget_id(), None);
    assert_sizes_close(&sizes.get(), &[0.42, 0.58]);

    let update = handler.accessibility_tree_update_for_test();
    assert!(!update
        .nodes
        .iter()
        .any(|(_, node)| node.role() == accesskit::Role::Splitter));
    assert!(update.nodes.iter().all(|(_, node)| {
        !node.supports_action(accesskit::Action::Increment)
            && !node.supports_action(accesskit::Action::Decrement)
            && !node.supports_action(accesskit::Action::SetValue)
    }));
}

#[test]
fn splitter_drag_keeps_resize_cursor_without_hover_rebuild() {
    let invalidation = InvalidationSignal::new();
    let sizes = State::new(vec![0.42, 0.58], invalidation.clone());
    let mut handler = test_handler(Some(splitter_tree(sizes)), invalidation);
    let start = splitter_handle_center(&mut handler);
    let end = Point::new(start.x + dp(30.0), start.y);

    pointer_press(&mut handler, start);
    handler.hovered_widgets.clear();
    handler.cursor_icon = None;
    pointer_move(&mut handler, end);

    assert_eq!(handler.cursor_icon, Some(CursorIcon::EwResize));
}

#[test]
fn splitter_drag_defers_scene_rebuild_until_redraw() {
    let invalidation = InvalidationSignal::new();
    let sizes = State::new(vec![0.42, 0.58], invalidation.clone());
    let mut handler = test_handler(Some(splitter_tree(sizes)), invalidation);
    let start = splitter_handle_center(&mut handler);
    let end = Point::new(start.x + dp(30.0), start.y);

    pointer_press(&mut handler, start);
    let _ = handler.computed_scene();
    assert!(
        handler
            .cached_scene
            .as_ref()
            .is_some_and(|cached| cached.layout_valid && cached.computed_valid),
        "cache should be valid before the drag move"
    );

    pointer_move(&mut handler, end);
    let cached = handler.cached_scene.as_ref().expect("cache shell");
    assert!(
        !cached.layout_valid && !cached.computed_valid,
        "splitter move should not force a scene rebuild during pointer event handling"
    );

    let moved = splitter_handle_center(&mut handler);
    assert!(
        moved.x > start.x + dp(10.0),
        "splitter handle should still move on the next scene read; start={start:?}, moved={moved:?}"
    );
}

#[test]
fn splitter_drag_uses_immediate_layout_when_sizes_signal_is_animated() {
    let invalidation = InvalidationSignal::new();
    let sizes = State::new(vec![0.42, 0.58], invalidation.clone());
    let transition = Transition::ease_in_out(Duration::from_millis(180));
    let mut handler = test_handler(
        Some(splitter_tree_with_sizes(
            sizes.clone(),
            sizes.signal().animated(transition),
        )),
        invalidation,
    );
    let start = splitter_handle_center(&mut handler);
    let end = Point::new(start.x + dp(30.0), start.y);

    pointer_press(&mut handler, start);
    pointer_move(&mut handler, end);

    let moved = splitter_handle_center(&mut handler);
    assert!(
        moved.x > start.x + dp(10.0),
        "animated size signals should not make splitter drags lag; start={start:?}, moved={moved:?}"
    );
}
