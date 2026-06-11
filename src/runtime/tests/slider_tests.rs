use super::*;

use std::sync::{Arc, Mutex};

use crate::platform::event::MouseButton;

fn press_vertical_slider(initial_value: f32, y_ratio_from_top: f32) -> Vec<f32> {
    let invalidation = InvalidationSignal::new();
    let changes = Arc::new(Mutex::new(Vec::new()));
    let changes_for_command = changes.clone();
    let tree = WidgetTree::new(
        Slider::new(initial_value, 0.0, 100.0)
            .vertical()
            .size(dp(40.0), dp(96.0))
            .on_change(ValueCommand::new(move |_: &mut TestVm, value| {
                changes_for_command.lock().unwrap().push(value);
            })),
    );
    let mut handler = test_handler_with_config(
        TestVm,
        Some(tree),
        invalidation,
        test_config_with_size(200.0, 200.0),
    );
    handler.invalidate_computed_scene();
    let track_rect = handler
        .computed_scene()
        .hit_regions
        .iter()
        .find_map(|region| match &region.interaction {
            HitInteraction::Slider { track_rect, .. } => Some(*track_rect),
            _ => None,
        })
        .expect("slider hit region should be present");
    let point = Point::new(
        track_rect.x + track_rect.width * 0.5,
        track_rect.y + track_rect.height * y_ratio_from_top,
    );
    let viewport = handler.viewport_rect();

    handler.cursor_position = Some(point);
    let _ = handler.handle_hover(viewport);
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);

    let recorded = changes.lock().unwrap().clone();
    recorded
}

#[test]
fn vertical_slider_press_maps_top_to_max_and_bottom_to_min() {
    let top_changes = press_vertical_slider(0.0, 0.0);
    assert_eq!(top_changes.len(), 1);
    assert!((top_changes[0] - 100.0).abs() <= f32::EPSILON);

    let bottom_changes = press_vertical_slider(100.0, 1.0);
    assert_eq!(bottom_changes.len(), 1);
    assert!((bottom_changes[0] - 0.0).abs() <= f32::EPSILON);
}

#[test]
fn slider_with_change_end_defers_press_commit_until_release() {
    let invalidation = InvalidationSignal::new();
    let changes = Arc::new(Mutex::new(Vec::new()));
    let change_ends = Arc::new(Mutex::new(Vec::new()));
    let changes_for_command = changes.clone();
    let change_ends_for_command = change_ends.clone();
    let tree = WidgetTree::new(
        Slider::new(0.0, 0.0, 100.0)
            .width(dp(180.0))
            .on_change(ValueCommand::new(move |_: &mut TestVm, value| {
                changes_for_command.lock().unwrap().push(value);
            }))
            .on_change_end(ValueCommand::new(move |_: &mut TestVm, value| {
                change_ends_for_command.lock().unwrap().push(value);
            })),
    );
    let mut handler = test_handler(Some(tree), invalidation);
    handler.invalidate_computed_scene();
    let track_rect = handler
        .computed_scene()
        .hit_regions
        .iter()
        .find_map(|region| match &region.interaction {
            HitInteraction::Slider { track_rect, .. } => Some(*track_rect),
            _ => None,
        })
        .expect("slider hit region should be present");
    let point = Point::new(
        track_rect.x + track_rect.width * 0.75,
        track_rect.y + track_rect.height * 0.5,
    );
    let viewport = handler.viewport_rect();

    handler.cursor_position = Some(point);
    let _ = handler.handle_hover(viewport);
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);

    assert!(
        changes.lock().unwrap().is_empty(),
        "on_change_end sliders should not commit on pointer press"
    );
    assert!(
        change_ends.lock().unwrap().is_empty(),
        "release-only command should wait for pointer release"
    );

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

    assert!(
        changes.lock().unwrap().is_empty(),
        "drag-end sliders should keep on_change visual-only during pointer interaction"
    );
    let ends = change_ends.lock().unwrap();
    assert_eq!(ends.len(), 1);
    assert!(
        (ends[0] - 75.0).abs() <= f32::EPSILON,
        "on_change_end should receive the pressed slider value"
    );
}
