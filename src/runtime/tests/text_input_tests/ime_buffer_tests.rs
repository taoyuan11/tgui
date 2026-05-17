use super::*;

#[test]
fn ime_commit_replaces_multibyte_selection_with_rope_buffer() {
    let invalidation = InvalidationSignal::new();
    let controller = TextController::from("你a好");
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
            cursor: "你a".len(),
            anchor: "你".len(),
            composition: None,
            scroll_x: Dp::ZERO,
            scroll_y: Dp::ZERO,
            preferred_column_x: None,
        },
    );

    assert!(handler.handle_ime_event(&Ime::Preedit("🙂".to_string(), Some((0, "🙂".len())))));
    let composition = handler
        .text_edit_states
        .get(&text_id)
        .and_then(|state| state.composition.as_ref())
        .expect("composition state should be stored");
    assert_eq!(composition.replace_range, ("你".len(), "你a".len()));

    assert!(handler.handle_ime_event(&Ime::Commit("🙂".to_string())));
    flush_text_input_commits(&mut handler);
    let value = handler.with_view_model(|vm| vm.value.clone());
    assert_eq!(value, "你🙂好");
}

#[test]
fn textarea_ime_composition_ignores_keyboard_text_until_commit() {
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

    assert!(handler.handle_ime_event(&Ime::Preedit("ni".to_string(), Some((0, "ni".len())),)));
    assert!(!handler.handle_keyboard_input(&text_key_event("n")));

    let text_id = handler
        .focused_widget_id()
        .expect("textarea should stay focused during composition");
    let state = handler
        .text_edit_states
        .get(&text_id)
        .expect("composition state should exist");
    assert_eq!(
        state
            .composition
            .as_ref()
            .map(|composition| composition.text.as_str()),
        Some("ni")
    );

    assert!(handler.handle_ime_event(&Ime::Commit("你".to_string())));
    flush_text_input_commits(&mut handler);

    let value = handler.with_view_model(|vm| vm.value.clone());
    assert_eq!(value, "你");
}

#[test]
fn textarea_ime_preedit_updates_session_display_layout_snapshot() {
    let invalidation = InvalidationSignal::new();
    let tree = WidgetTree::new(Textarea::<TestVm>::new("hello world").height(dp(120.0)));
    let mut handler = test_handler(Some(tree), invalidation);
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
        .expect("textarea should stay focused during composition");
    let _ = handler.sync_text_input_buffer(text_id);

    assert!(handler.handle_ime_event(&Ime::Preedit("ni".to_string(), Some((0, "ni".len())),)));

    let session = handler
        .text_input_buffers
        .get(&text_id)
        .expect("textarea text input session should exist");
    assert_eq!(session.current_text, "hello world");
    assert_eq!(session.display_text, "nihello world");
    let layout = session
        .layout_snapshot
        .as_ref()
        .expect("textarea layout snapshot should exist during composition");
    assert_eq!(layout.line_start(0), 0);
    assert_eq!(layout.line_end(0), session.display_text.len());
}

#[test]
fn textarea_ime_request_uses_normal_text_purpose() {
    let invalidation = InvalidationSignal::new();
    let tree = WidgetTree::new(Textarea::<TestVm>::new("你好").key("textarea"));
    let mut handler = test_handler(Some(tree), invalidation);
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

    let request_data = handler
        .ime_request_data_for_text_input()
        .expect("focused textarea should request ime data");
    assert_eq!(
        request_data.hint_and_purpose,
        Some((ImeHint::NONE, ImePurpose::Normal))
    );
}

#[test]
fn long_textarea_ime_request_still_enables_without_surrounding_text() {
    let invalidation = InvalidationSignal::new();
    let long_text = "0123456789abcdef\n".repeat(300);
    let tree = WidgetTree::new(Textarea::<TestVm>::new(long_text));
    let mut handler = test_handler(Some(tree), invalidation);
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

    let request_data = handler
        .ime_request_data_for_text_input()
        .expect("focused textarea should request ime data");
    assert_eq!(
        request_data.hint_and_purpose,
        Some((ImeHint::NONE, ImePurpose::Normal))
    );
    assert!(request_data.cursor_area.is_some());
    assert!(request_data.surrounding_text.is_none());

    let capabilities = ImeCapabilities::new()
        .with_hint_and_purpose()
        .with_cursor_area();
    assert!(
        ImeEnableRequest::new(capabilities, request_data).is_some(),
        "ime should still enable for long textareas"
    );
}

#[test]
fn external_bound_value_rebuilds_text_input_buffer_and_clamps_state() {
    let invalidation = InvalidationSignal::new();
    let controller = TextController::from("hello🙂world");
    let tree: WidgetTree<TestVm> = WidgetTree::new(Input::<TestVm>::new(controller.clone()));
    let mut handler = test_handler(Some(tree), invalidation.clone());
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

    let text_id = handler
        .focused_widget_id()
        .expect("input should be focused after click");
    handler.text_edit_states.insert(
        text_id,
        TextEditState {
            cursor: "hello🙂world".len(),
            anchor: "hello🙂world".len(),
            composition: None,
            scroll_x: Dp::ZERO,
            scroll_y: Dp::ZERO,
            preferred_column_x: None,
        },
    );

    handler.sync_text_input_buffer(text_id);
    assert_eq!(
        handler
            .text_input_buffers
            .get(&text_id)
            .expect("text input buffer should exist")
            .external_value,
        "hello🙂world"
    );

    controller.set_text("中");
    handler.request_redraw_if_dirty(Instant::now());
    let _ = handler.computed_scene();
    handler.sync_text_input_buffer(text_id);

    let buffer_state = handler
        .text_input_buffers
        .get_mut(&text_id)
        .expect("text input buffer should be rebuilt");
    assert_eq!(buffer_state.external_value, "中");
    assert_eq!(buffer_state.current_text, "中");

    let state = handler
        .text_edit_states
        .get(&text_id)
        .expect("text edit state should still exist");
    assert_eq!(state.cursor, "中".len());
    assert_eq!(state.anchor, "中".len());
}

#[test]
fn textarea_large_text_edit_smoke_uses_rope_buffer() {
    let invalidation = InvalidationSignal::new();
    let initial = "0123456789abcdef\n".repeat(2048);
    let controller = TextController::from(initial.clone());
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
            cursor: 0,
            anchor: 0,
            composition: None,
            scroll_x: Dp::ZERO,
            scroll_y: Dp::ZERO,
            preferred_column_x: None,
        },
    );

    handler.handle_keyboard_input(&text_key_event("中"));
    flush_text_input_commits(&mut handler);

    let value = handler.with_view_model(|vm| vm.value.clone());
    assert_eq!(value.len(), initial.len() + "中".len());
    assert!(value.starts_with("中0123456789abcdef"));
}
