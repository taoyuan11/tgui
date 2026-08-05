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
        .any(|primitive| primitive.content.as_ref() == "abc"));
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

#[test]
fn physical_enter_inserts_newline_in_focused_textarea() {
    let invalidation = InvalidationSignal::new();
    let controller = TextController::from("first");
    let callback_controller = controller.clone();
    let tree = WidgetTree::new(Textarea::new(controller).on_change(Command::new(
        move |vm: &mut TextInputVm| {
            vm.value = callback_controller.text();
        },
    )));
    let mut handler = test_handler_with_vm(TextInputVm::default(), Some(tree), invalidation);
    let viewport = handler.viewport_rect();
    let frame = text_input_frame(&mut handler, true);

    handler.cursor_position = Some(Point {
        x: frame.x + dp(8.0),
        y: frame.y + dp(8.0),
    });
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);

    let text_id = handler
        .focused_widget_id()
        .expect("textarea should be focused after click");
    handler
        .text_edit_states
        .insert(text_id, TextEditState::caret_at("first"));

    assert!(handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Enter,))));
    flush_text_input_commits(&mut handler);

    assert_eq!(handler.with_view_model(|vm| vm.value.clone()), "first\n");
}

#[test]
fn single_line_input_removes_line_breaks_from_inserted_text() {
    let invalidation = InvalidationSignal::new();
    let controller = TextController::from("");
    let callback_controller = controller.clone();
    let tree = WidgetTree::new(Input::new(controller).on_change(Command::new(
        move |vm: &mut TextInputVm| {
            vm.value = callback_controller.text();
        },
    )));
    let mut handler = test_handler_with_vm(TextInputVm::default(), Some(tree), invalidation);
    let viewport = handler.viewport_rect();
    let frame = text_input_frame(&mut handler, false);

    handler.cursor_position = Some(Point {
        x: frame.x + dp(8.0),
        y: frame.y + (frame.height * 0.5),
    });
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);

    assert!(
        handler.handle_keyboard_input(&text_key_event("a\r\nb\nc\rd\u{0085}e\u{2028}f\u{2029}g",))
    );
    flush_text_input_commits(&mut handler);

    assert_eq!(handler.with_view_model(|vm| vm.value.clone()), "abcdefg");
}

#[test]
fn multiline_input_normalizes_inserted_line_breaks_to_lf() {
    let invalidation = InvalidationSignal::new();
    let controller = TextController::from("");
    let callback_controller = controller.clone();
    let tree = WidgetTree::new(Textarea::new(controller).on_change(Command::new(
        move |vm: &mut TextInputVm| {
            vm.value = callback_controller.text();
        },
    )));
    let mut handler = test_handler_with_vm(TextInputVm::default(), Some(tree), invalidation);
    let viewport = handler.viewport_rect();
    let frame = text_input_frame(&mut handler, true);

    handler.cursor_position = Some(Point {
        x: frame.x + dp(8.0),
        y: frame.y + dp(8.0),
    });
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);

    assert!(
        handler.handle_keyboard_input(&text_key_event("a\r\nb\nc\rd\u{0085}e\u{2028}f\u{2029}g",))
    );
    flush_text_input_commits(&mut handler);

    assert_eq!(
        handler.with_view_model(|vm| vm.value.clone()),
        "a\nb\nc\nd\ne\nf\ng"
    );
}

#[test]
fn input_backspace_deletes_zwj_emoji_as_one_grapheme() {
    let invalidation = InvalidationSignal::new();
    let family = "👨‍👩‍👧‍👦";
    let initial = format!("a{family}b");
    let controller = TextController::from(initial.as_str());
    let callback_controller = controller.clone();
    let tree = WidgetTree::new(Input::new(controller).on_change(Command::new(
        move |vm: &mut TextInputVm| {
            vm.value = callback_controller.text();
        },
    )));
    let mut handler = test_handler_with_vm(TextInputVm::default(), Some(tree), invalidation);
    let viewport = handler.viewport_rect();
    let frame = text_input_frame(&mut handler, false);

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
            cursor: "a".len() + family.len(),
            anchor: "a".len() + family.len(),
            composition: None,
            scroll_x: Dp::ZERO,
            scroll_y: Dp::ZERO,
            preferred_column_x: None,
        },
    );

    handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Backspace)));
    flush_text_input_commits(&mut handler);

    let value = handler.with_view_model(|vm| vm.value.clone());
    assert_eq!(value, "ab");
}

#[test]
fn input_delete_deletes_combining_cluster_as_one_grapheme() {
    let invalidation = InvalidationSignal::new();
    let initial = "a\u{0301}b";
    let first_grapheme = "a\u{0301}";
    let controller = TextController::from(initial);
    let callback_controller = controller.clone();
    let tree = WidgetTree::new(Input::new(controller).on_change(Command::new(
        move |vm: &mut TextInputVm| {
            vm.value = callback_controller.text();
        },
    )));
    let mut handler = test_handler_with_vm(TextInputVm::default(), Some(tree), invalidation);
    let viewport = handler.viewport_rect();
    let frame = text_input_frame(&mut handler, false);

    handler.cursor_position = Some(Point {
        x: frame.x + dp(8.0),
        y: frame.y + (frame.height * 0.5),
    });
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);

    let text_id = handler
        .focused_widget_id()
        .expect("input should be focused after click");
    handler.text_edit_states.insert(
        text_id,
        TextEditState {
            cursor: 0,
            anchor: 0,
            composition: None,
            scroll_x: Dp::ZERO,
            scroll_y: Dp::ZERO,
            preferred_column_x: None,
        },
    );

    handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Delete)));
    flush_text_input_commits(&mut handler);

    let value = handler.with_view_model(|vm| vm.value.clone());
    assert_eq!(value, &initial[first_grapheme.len()..]);
}

#[test]
fn input_arrow_right_moves_by_grapheme_cluster() {
    let invalidation = InvalidationSignal::new();
    let family = "👩‍💻";
    let initial = format!("{family}b");
    let tree = WidgetTree::new(Input::<TestVm>::new(initial.as_str()));
    let mut handler = test_handler(Some(tree), invalidation);
    let viewport = handler.viewport_rect();
    let frame = text_input_frame(&mut handler, false);

    handler.cursor_position = Some(Point {
        x: frame.x + dp(8.0),
        y: frame.y + (frame.height * 0.5),
    });
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);

    let text_id = handler
        .focused_widget_id()
        .expect("input should be focused after click");
    handler.text_edit_states.insert(
        text_id,
        TextEditState {
            cursor: 0,
            anchor: 0,
            composition: None,
            scroll_x: Dp::ZERO,
            scroll_y: Dp::ZERO,
            preferred_column_x: None,
        },
    );

    handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::ArrowRight)));

    let state = handler
        .text_edit_states
        .get(&text_id)
        .expect("input edit state should exist");
    assert_eq!(state.cursor, family.len());
    assert_eq!(state.anchor, family.len());
}

#[test]
fn long_textarea_shift_arrow_preserves_utf8_selection_after_ime_preedit() {
    let invalidation = InvalidationSignal::new();
    let prefix = "中a\u{0301}".repeat(512);
    let cluster = "👩🏽‍💻";
    let initial = format!("{prefix}{cluster}tail");
    let tree = WidgetTree::new(
        Textarea::<TestVm>::new(initial.as_str())
            .size(dp(140.0), dp(72.0))
            .auto_wrap(false),
    );
    let mut handler = test_handler(Some(tree), invalidation);
    let viewport = handler.viewport_rect();
    let frame = text_input_frame(&mut handler, true);

    handler.cursor_position = Some(Point {
        x: frame.x + dp(8.0),
        y: frame.y + dp(8.0),
    });
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);

    let text_id = handler
        .focused_widget_id()
        .expect("textarea should be focused after click");
    let cluster_start = prefix.len();
    let cluster_end = cluster_start + cluster.len();
    handler.text_edit_states.insert(
        text_id,
        TextEditState {
            cursor: cluster_start,
            anchor: cluster_start,
            composition: None,
            scroll_x: Dp::ZERO,
            scroll_y: Dp::ZERO,
            preferred_column_x: None,
        },
    );

    assert!(handler.handle_ime_event(&Ime::Preedit("候補".to_string(), Some((0, "候補".len())),)));
    assert!(
        !handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::ArrowRight,))),
        "the platform IME must retain navigation while preedit is active",
    );
    assert!(handler.handle_ime_event(&Ime::Disabled));
    handler.modifiers = ModifiersState::SHIFT;
    assert!(
        handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::ArrowRight,)))
    );
    handler.modifiers = ModifiersState::empty();

    let state = handler
        .text_edit_states
        .get(&text_id)
        .expect("textarea edit state should exist");
    assert_eq!(state.cursor, cluster_end);
    assert_eq!(state.anchor, cluster_start);
    assert_eq!(state.selection_range(), Some((cluster_start, cluster_end)));
    assert!(state.composition.is_none());
    assert_eq!(
        handler
            .text_input_buffers
            .get(&text_id)
            .expect("textarea session should remain live")
            .current_text,
        initial,
    );
    assert!(
        handler
            .scroll_states
            .get(&text_id)
            .map(|offset| offset.x > Dp::ZERO)
            .unwrap_or(false),
        "moving the long-line caret should keep it horizontally visible",
    );
}

#[test]
fn textarea_replaces_mixed_rtl_complex_selection() {
    let invalidation = InvalidationSignal::new();
    let initial = "Report שלום नमस्ते 👩‍💻 done";
    let replacement = "تم";
    let controller = TextController::from(initial);
    let callback_controller = controller.clone();
    let tree = WidgetTree::new(Textarea::new(controller).on_change(Command::new(
        move |vm: &mut TextInputVm| {
            vm.value = callback_controller.text();
        },
    )));
    let mut handler = test_handler_with_vm(TextInputVm::default(), Some(tree), invalidation);
    let viewport = handler.viewport_rect();
    let frame = text_input_frame(&mut handler, true);

    handler.cursor_position = Some(Point {
        x: frame.x + dp(8.0),
        y: frame.y + dp(8.0),
    });
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);

    let selected = "שלום नमस्ते 👩‍💻";
    let start = "Report ".len();
    let end = start + selected.len();
    let text_id = handler
        .focused_widget_id()
        .expect("textarea should be focused after click");
    handler.text_edit_states.insert(
        text_id,
        TextEditState {
            cursor: end,
            anchor: start,
            composition: None,
            scroll_x: Dp::ZERO,
            scroll_y: Dp::ZERO,
            preferred_column_x: None,
        },
    );

    handler.handle_keyboard_input(&text_key_event(replacement));
    flush_text_input_commits(&mut handler);

    let value = handler.with_view_model(|vm| vm.value.clone());
    assert_eq!(value, "Report تم done");
}

fn text_input_frame<VM: 'static>(handler: &mut BoundRuntimeHandler<VM>, multiline: bool) -> Rect {
    let computed = handler.computed_scene();
    computed
        .hit_regions
        .iter()
        .find_map(|region| match &region.interaction {
            HitInteraction::TextInput {
                multiline: region_multiline,
                ..
            } if *region_multiline == multiline => Some(region.rect),
            _ => None,
        })
        .expect("text input hit region should exist")
}
