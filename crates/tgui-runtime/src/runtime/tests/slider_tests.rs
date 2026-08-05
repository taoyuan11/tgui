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

#[test]
fn page_keys_adjust_focused_slider_by_ten_percent() {
    let invalidation = InvalidationSignal::new();
    let changes = Arc::new(Mutex::new(Vec::new()));
    let changes_for_command = Arc::clone(&changes);
    let tree = WidgetTree::new(
        Slider::new(50.0, 0.0, 100.0)
            .step(1.0)
            .width(dp(180.0))
            .on_change(ValueCommand::new(move |_: &mut TestVm, value| {
                changes_for_command.lock().unwrap().push(value);
            })),
    );
    let mut handler = test_handler(Some(tree), invalidation);

    assert!(handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Tab))));
    assert!(handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::PageUp))));
    assert!(handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::PageDown))));

    assert_eq!(*changes.lock().unwrap(), vec![60.0, 40.0]);
}

#[test]
fn keyboard_slider_only_responds_to_its_orientation_arrows() {
    let horizontal_changes = Arc::new(Mutex::new(Vec::new()));
    let horizontal_for_command = Arc::clone(&horizontal_changes);
    let horizontal = WidgetTree::new(Slider::new(50.0, 0.0, 100.0).on_change(ValueCommand::new(
        move |_: &mut TestVm, value| {
            horizontal_for_command.lock().unwrap().push(value);
        },
    )));
    let mut horizontal_handler = test_handler(Some(horizontal), InvalidationSignal::new());
    assert!(horizontal_handler
        .handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Tab))));
    assert!(!horizontal_handler
        .handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::ArrowUp))));
    assert!(horizontal_handler
        .handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::ArrowRight))));
    assert_eq!(*horizontal_changes.lock().unwrap(), vec![51.0]);

    let vertical_changes = Arc::new(Mutex::new(Vec::new()));
    let vertical_for_command = Arc::clone(&vertical_changes);
    let vertical = WidgetTree::new(Slider::new(50.0, 0.0, 100.0).vertical().on_change(
        ValueCommand::new(move |_: &mut TestVm, value| {
            vertical_for_command.lock().unwrap().push(value);
        }),
    ));
    let mut vertical_handler = test_handler(Some(vertical), InvalidationSignal::new());
    assert!(
        vertical_handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Tab)))
    );
    assert!(!vertical_handler
        .handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::ArrowRight))));
    assert!(vertical_handler
        .handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::ArrowUp))));
    assert_eq!(*vertical_changes.lock().unwrap(), vec![51.0]);
}

#[test]
fn keyboard_slider_dispatches_change_end_with_fallback_step() {
    let invalidation = InvalidationSignal::new();
    let change_ends = Arc::new(Mutex::new(Vec::new()));
    let change_ends_for_command = Arc::clone(&change_ends);
    let tree = WidgetTree::new(
        Slider::new(50.0, 0.0, 100.0)
            .step(0.0)
            .width(dp(180.0))
            .on_change_end(ValueCommand::new(move |_: &mut TestVm, value| {
                change_ends_for_command.lock().unwrap().push(value);
            })),
    );
    let mut handler = test_handler(Some(tree), invalidation);

    assert!(handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Tab))));
    assert!(
        handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::ArrowRight,)))
    );

    assert_eq!(*change_ends.lock().unwrap(), vec![51.0]);
}

#[test]
fn keyboard_slider_dispatches_change_then_change_end_and_skips_unchanged_boundary() {
    let invalidation = InvalidationSignal::new();
    let changes = Arc::new(Mutex::new(Vec::new()));
    let change_ends = Arc::new(Mutex::new(Vec::new()));
    let changes_for_command = Arc::clone(&changes);
    let change_ends_for_command = Arc::clone(&change_ends);
    let tree = WidgetTree::new(
        Slider::new(100.0, 0.0, 100.0)
            .step(1.0)
            .width(dp(180.0))
            .on_change(ValueCommand::new(move |_: &mut TestVm, value| {
                changes_for_command.lock().unwrap().push(value);
            }))
            .on_change_end(ValueCommand::new(move |_: &mut TestVm, value| {
                change_ends_for_command.lock().unwrap().push(value);
            })),
    );
    let mut handler = test_handler(Some(tree), invalidation);

    assert!(handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Tab))));
    assert!(
        handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::ArrowRight,)))
    );
    assert!(changes.lock().unwrap().is_empty());
    assert!(change_ends.lock().unwrap().is_empty());

    assert!(
        handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::ArrowLeft,)))
    );
    assert_eq!(*changes.lock().unwrap(), vec![99.0]);
    assert_eq!(*change_ends.lock().unwrap(), vec![99.0]);
}

#[test]
fn slider_non_finite_bounds_degrade_to_a_finite_fixed_value() {
    for (min, max, expected) in [
        (f32::NAN, 10.0, 10.0),
        (2.0, f32::INFINITY, 2.0),
        (f32::NEG_INFINITY, f32::NAN, 0.0),
    ] {
        let invalidation = InvalidationSignal::new();
        let tree = WidgetTree::new(Slider::new(5.0, min, max).width(dp(180.0)));
        let mut handler = test_handler(Some(tree), invalidation);
        let (value, resolved_min, resolved_max) = handler
            .computed_scene()
            .hit_regions
            .iter()
            .find_map(|region| match &region.interaction {
                HitInteraction::Slider {
                    value, min, max, ..
                } => Some((*value, *min, *max)),
                _ => None,
            })
            .expect("slider hit region should exist");

        assert_eq!(
            (value, resolved_min, resolved_max),
            (expected, expected, expected)
        );
    }
}

#[test]
fn color_picker_drag_updates_sibling_preview_and_channel_text_immediately() {
    let invalidation = InvalidationSignal::new();
    let tree =
        WidgetTree::new(crate::ui::widget::ColorPicker::new(Color::hexa(0x3366CCFF)).open(true));
    let mut handler = test_handler_with_config(
        TestVm,
        Some(tree),
        invalidation,
        test_config_with_size(500.0, 500.0),
    );
    handler.invalidate_computed_scene();
    let track_rect = handler
        .computed_scene()
        .overlay_hit_regions
        .iter()
        .filter_map(|region| match &region.interaction {
            HitInteraction::Slider { track_rect, .. } => Some(*track_rect),
            _ => None,
        })
        .next()
        .expect("red channel slider should be hittable");
    let viewport = handler.viewport_rect();
    let point = Point::new(
        track_rect.x + track_rect.width * (128.0 / 255.0),
        track_rect.y + track_rect.height * 0.5,
    );

    handler.cursor_position = Some(point);
    let _ = handler.handle_hover(viewport);
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);

    let computed = handler.computed_scene();
    let overlay_text = computed
        .scene
        .overlay_texts
        .iter()
        .map(|text| text.content.as_ref())
        .collect::<Vec<_>>();
    assert!(
        overlay_text.contains(&"#8066CCFF"),
        "current color text should update during the drag: {overlay_text:?}"
    );
    assert!(
        overlay_text.contains(&"128"),
        "red channel text should update during the drag: {overlay_text:?}"
    );
    assert!(
        computed
            .scene
            .overlay_shapes
            .iter()
            .any(|rect| rect.color == Color::hexa(0x8066CCFF)),
        "preview fill should update during the drag"
    );
}

#[test]
fn slider_drag_patches_sibling_owners_of_the_same_value_dependency() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let value = context.state(20.0_f32);
    let value_for_change = value.clone();
    let value_text = value
        .signal()
        .map(|value| format!("Shared value: {value:.0}"));
    let tree = WidgetTree::new(
        Flex::vertical()
            .child(
                Slider::new(value.signal(), 0.0, 100.0)
                    .width(dp(180.0))
                    .on_change(ValueCommand::new(move |_: &mut TestVm, next| {
                        value_for_change.set(next);
                    })),
            )
            .child(Text::new(value_text).width(dp(180.0))),
    );
    let mut handler = test_handler_with_config(
        TestVm,
        Some(tree),
        invalidation,
        test_config_with_size(240.0, 160.0),
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
        .expect("slider should be hittable");
    let viewport = handler.viewport_rect();
    let point = Point::new(
        track_rect.x + track_rect.width * 0.75,
        track_rect.y + track_rect.height * 0.5,
    );

    handler.cursor_position = Some(point);
    let _ = handler.handle_hover(viewport);
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);

    let cached = handler
        .cached_scene
        .as_ref()
        .expect("slider drag should keep the scene cache");
    assert!(
        cached.computed_valid,
        "same-dependency siblings should use the retained scene patch"
    );
    assert!(cached
        .computed
        .scene
        .texts
        .iter()
        .any(|text| text.content.as_ref() == "Shared value: 75"));
}
