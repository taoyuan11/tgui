use super::*;

#[test]
fn input_backspace_preserves_multibyte_boundaries_with_rope_buffer() {
    let invalidation = InvalidationSignal::new();
    let controller = TextController::from("a中🙂b");
    let callback_controller = controller.clone();
    let tree = WidgetTree::new(Input::new(controller).on_change(Command::new(
        move |vm: &mut TextInputVm| {
            vm.value = callback_controller.text();
        },
    )));
    let mut handler = test_handler_with_vm(TextInputVm::default(), Some(tree), invalidation);
    let viewport = handler.viewport_rect();
    let frame = {
        let computed = handler.computed_scene();
        computed
            .hit_regions
            .iter()
            .find_map(|region| match &region.interaction {
                HitInteraction::TextInput {
                    multiline: false, ..
                } => Some(region.rect),
                _ => None,
            })
            .expect("input hit region should exist")
    };

    handler.cursor_position = Some(Point {
        x: frame.x + frame.width - dp(4.0),
        y: frame.y + (frame.height * 0.5),
    });
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);

    let text_id = handler
        .focused_widget_id()
        .expect("input should be focused after click");
    handler.text_edit_states.insert(
        text_id,
        TextEditState {
            cursor: "a中🙂".len(),
            anchor: "a中🙂".len(),
            composition: None,
            scroll_x: Dp::ZERO,
            scroll_y: Dp::ZERO,
            preferred_column_x: None,
        },
    );

    handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Backspace)));
    flush_text_input_commits(&mut handler);

    let value = handler.with_view_model(|vm| vm.value.clone());
    assert_eq!(value, "a中b");
}

#[test]
fn input_backspace_repeats_while_key_is_held() {
    let invalidation = InvalidationSignal::new();
    let controller = TextController::from("abcd");
    let callback_controller = controller.clone();
    let tree = WidgetTree::new(Input::new(controller).on_change(Command::new(
        move |vm: &mut TextInputVm| {
            vm.value = callback_controller.text();
        },
    )));
    let mut handler = test_handler_with_vm(TextInputVm::default(), Some(tree), invalidation);
    let viewport = handler.viewport_rect();
    let frame = {
        let computed = handler.computed_scene();
        computed
            .hit_regions
            .iter()
            .find_map(|region| match &region.interaction {
                HitInteraction::TextInput { .. } => Some(region.rect),
                _ => None,
            })
            .expect("input hit region should exist")
    };

    handler.cursor_position = Some(Point {
        x: frame.x + frame.width - dp(4.0),
        y: frame.y + (frame.height * 0.5),
    });
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);

    handler.handle_keyboard_input(&repeated_pressed_key_event(PhysicalKey::Code(
        KeyCode::Backspace,
    )));
    flush_text_input_commits(&mut handler);

    let value = handler.with_view_model(|vm| vm.value.clone());
    assert_eq!(value, "abc");
}

#[test]
fn platform_repeated_backspace_events_are_ignored_for_text_input() {
    let invalidation = InvalidationSignal::new();
    let controller = TextController::from("abcd");
    let callback_controller = controller.clone();
    let tree = WidgetTree::new(Input::new(controller).on_change(Command::new(
        move |vm: &mut TextInputVm| {
            vm.value = callback_controller.text();
        },
    )));
    let mut handler = test_handler_with_vm(TextInputVm::default(), Some(tree), invalidation);
    let viewport = handler.viewport_rect();
    let frame = {
        let computed = handler.computed_scene();
        computed
            .hit_regions
            .iter()
            .find_map(|region| match &region.interaction {
                HitInteraction::TextInput { .. } => Some(region.rect),
                _ => None,
            })
            .expect("input hit region should exist")
    };

    handler.cursor_position = Some(Point {
        x: frame.x + frame.width - dp(4.0),
        y: frame.y + (frame.height * 0.5),
    });
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);

    assert!(
        handler.handle_platform_keyboard_input(&pressed_key_event(PhysicalKey::Code(
            KeyCode::Backspace,
        )))
    );
    assert!(
        !handler.handle_platform_keyboard_input(&repeated_pressed_key_event(PhysicalKey::Code(
            KeyCode::Backspace,
        )))
    );
    flush_text_input_commits(&mut handler);

    let value = handler.with_view_model(|vm| vm.value.clone());
    assert_eq!(value, "abc");
}

#[test]
fn released_key_stops_runtime_managed_repeat_immediately() {
    let invalidation = InvalidationSignal::new();
    let controller = TextController::from("abcd");
    let callback_controller = controller.clone();
    let tree = WidgetTree::new(Input::new(controller).on_change(Command::new(
        move |vm: &mut TextInputVm| {
            vm.value = callback_controller.text();
        },
    )));
    let mut handler = test_handler_with_vm(TextInputVm::default(), Some(tree), invalidation);
    let viewport = handler.viewport_rect();
    let frame = {
        let computed = handler.computed_scene();
        computed
            .hit_regions
            .iter()
            .find_map(|region| match &region.interaction {
                HitInteraction::TextInput { .. } => Some(region.rect),
                _ => None,
            })
            .expect("input hit region should exist")
    };

    handler.cursor_position = Some(Point {
        x: frame.x + frame.width - dp(4.0),
        y: frame.y + (frame.height * 0.5),
    });
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);

    assert!(
        handler.handle_platform_keyboard_input(&pressed_key_event(PhysicalKey::Code(
            KeyCode::Backspace,
        )))
    );
    flush_text_input_commits(&mut handler);
    assert_eq!(handler.with_view_model(|vm| vm.value.clone()), "abc");

    assert!(handler.active_key_repeat.is_some());

    assert!(
        !handler.handle_platform_keyboard_input(&released_key_event(PhysicalKey::Code(
            KeyCode::Backspace,
        )))
    );
    assert!(handler.active_key_repeat.is_none());
    assert!(!handler.drive_key_repeat(Instant::now() + Duration::from_secs(1)));
    flush_text_input_commits(&mut handler);
    assert_eq!(handler.with_view_model(|vm| vm.value.clone()), "abc");
}

#[test]
fn repeated_backspace_keeps_deleting_when_widget_value_is_static() {
    let invalidation = InvalidationSignal::new();
    let controller = TextController::from("abcd");
    let callback_controller = controller.clone();
    let tree = WidgetTree::new(Input::new(controller).on_change(Command::new(
        move |vm: &mut TextInputVm| {
            vm.value = callback_controller.text();
        },
    )));
    let mut handler = test_handler_with_vm(TextInputVm::default(), Some(tree), invalidation);
    let viewport = handler.viewport_rect();
    let frame = {
        let computed = handler.computed_scene();
        computed
            .hit_regions
            .iter()
            .find_map(|region| match &region.interaction {
                HitInteraction::TextInput { .. } => Some(region.rect),
                _ => None,
            })
            .expect("input hit region should exist")
    };

    handler.cursor_position = Some(Point {
        x: frame.x + frame.width - dp(4.0),
        y: frame.y + (frame.height * 0.5),
    });
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);

    assert!(
        handler.handle_keyboard_input(&repeated_pressed_key_event(PhysicalKey::Code(
            KeyCode::Backspace,
        )))
    );
    assert!(
        handler.handle_keyboard_input(&repeated_pressed_key_event(PhysicalKey::Code(
            KeyCode::Backspace,
        )))
    );
    flush_text_input_commits(&mut handler);

    let value = handler.with_view_model(|vm| vm.value.clone());
    assert_eq!(value, "ab");
    assert_eq!(
        handler
            .text_input_buffers
            .get(
                &handler
                    .focused_widget_id()
                    .expect("input should remain focused after repeated backspace"),
            )
            .expect("text input buffer should exist")
            .current_text,
        "ab"
    );
}

#[test]
fn focused_input_renders_local_buffer_until_bound_value_catches_up() {
    let invalidation = InvalidationSignal::new();
    let controller = TextController::from("abcd");
    let callback_controller = controller.clone();
    let tree = WidgetTree::new(Input::new(controller).on_change(Command::new(
        move |vm: &mut TextInputVm| {
            vm.value = callback_controller.text();
        },
    )));
    let mut handler = test_handler_with_vm(TextInputVm::default(), Some(tree), invalidation);
    let viewport = handler.viewport_rect();
    let frame = {
        let computed = handler.computed_scene();
        computed
            .hit_regions
            .iter()
            .find_map(|region| match &region.interaction {
                HitInteraction::TextInput { .. } => Some(region.rect),
                _ => None,
            })
            .expect("input hit region should exist")
    };

    handler.cursor_position = Some(Point {
        x: frame.x + frame.width - dp(4.0),
        y: frame.y + (frame.height * 0.5),
    });
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);
    handler.handle_keyboard_input(&repeated_pressed_key_event(PhysicalKey::Code(
        KeyCode::Backspace,
    )));

    let computed = handler.computed_scene();
    assert!(computed
        .scene
        .texts
        .iter()
        .any(|primitive| primitive.content == "abc"));
}

#[test]
fn textarea_replaces_multibyte_selection_via_rope_buffer() {
    let invalidation = InvalidationSignal::new();
    let controller = TextController::from("ab中🙂cd");
    let callback_controller = controller.clone();
    let tree = WidgetTree::new(Textarea::new(controller).on_change(Command::new(
        move |vm: &mut TextInputVm| {
            vm.value = callback_controller.text();
        },
    )));
    let mut handler = test_handler_with_vm(TextInputVm::default(), Some(tree), invalidation);
    let viewport = handler.viewport_rect();
    let frame = {
        let computed = handler.computed_scene();
        computed
            .hit_regions
            .iter()
            .find_map(|region| match &region.interaction {
                HitInteraction::TextInput {
                    multiline: true, ..
                } => Some(region.rect),
                _ => None,
            })
            .expect("textarea hit region should exist")
    };

    handler.cursor_position = Some(Point {
        x: frame.x + dp(8.0),
        y: frame.y + dp(8.0),
    });
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);

    let text_id = handler
        .focused_widget_id()
        .expect("textarea should be focused after click");
    handler.text_edit_states.insert(
        text_id,
        TextEditState {
            cursor: "ab中🙂".len(),
            anchor: "ab".len(),
            composition: None,
            scroll_x: Dp::ZERO,
            scroll_y: Dp::ZERO,
            preferred_column_x: None,
        },
    );

    handler.handle_keyboard_input(&text_key_event("X"));
    flush_text_input_commits(&mut handler);

    let value = handler.with_view_model(|vm| vm.value.clone());
    assert_eq!(value, "abXcd");
}
