use super::*;

#[test]
fn clicking_switch_dispatches_toggled_value() {
    let invalidation = InvalidationSignal::new();
    let tree = WidgetTree::new(
        crate::ui::widget::Switch::new(false)
            .on_change(ValueCommand::new(|vm: &mut SwitchVm, value| {
                vm.checked = value
            }))
            .size(dp(52.0), dp(30.0)),
    );
    let mut handler = test_handler_with_vm(SwitchVm::default(), Some(tree), invalidation);
    let viewport = handler.viewport_rect();

    let frame = {
        let computed = handler.computed_scene();
        computed
            .hit_regions
            .iter()
            .find_map(|region| match &region.interaction {
                HitInteraction::Switch { .. } => Some(region.rect),
                _ => None,
            })
            .expect("switch hit region should exist")
    };

    handler.cursor_position = Some(Point {
        x: frame.x + (frame.width * 0.5),
        y: frame.y + (frame.height * 0.5),
    });
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);

    let checked = handler.with_view_model(|vm| vm.checked);
    assert!(checked);
}

#[test]
fn clicking_checkbox_dispatches_toggled_value() {
    let invalidation = InvalidationSignal::new();
    let tree = WidgetTree::new(
        Checkbox::new(false)
            .label("Accept")
            .on_change(ValueCommand::new(|vm: &mut SwitchVm, value| {
                vm.checked = value
            }))
            .size(dp(120.0), dp(30.0)),
    );
    let mut handler = test_handler_with_vm(SwitchVm::default(), Some(tree), invalidation);
    let viewport = handler.viewport_rect();

    let frame = {
        let computed = handler.computed_scene();
        computed
            .hit_regions
            .iter()
            .find_map(|region| match &region.interaction {
                HitInteraction::Checkbox { .. } => Some(region.rect),
                _ => None,
            })
            .expect("checkbox hit region should exist")
    };

    handler.cursor_position = Some(Point {
        x: frame.x + (frame.width * 0.5),
        y: frame.y + (frame.height * 0.5),
    });
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);

    let checked = handler.with_view_model(|vm| vm.checked);
    assert!(checked);
}

#[test]
fn focused_input_receives_inserted_text_via_on_change() {
    let invalidation = InvalidationSignal::new();
    let controller = TextController::from("hi");
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
    handler.handle_keyboard_input(&text_key_event("a"));
    flush_text_input_commits(&mut handler);

    let value = handler.with_view_model(|vm| vm.value.clone());
    assert_eq!(value, "hia");
}

#[test]
fn focused_input_accepts_repeated_text_input() {
    let invalidation = InvalidationSignal::new();
    let controller = TextController::from("hi");
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
    handler.handle_keyboard_input(&repeated_text_key_event("a"));
    flush_text_input_commits(&mut handler);

    let value = handler.with_view_model(|vm| vm.value.clone());
    assert_eq!(value, "hia");
}

#[test]
fn focused_text_overrides_use_session_text_without_local_edits() {
    let invalidation = InvalidationSignal::new();
    let controller = TextController::from("hello world");
    let tree = WidgetTree::new(Input::new(controller));
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

    let focused_id = handler
        .focused_text_input_id()
        .expect("input should be focused after click");
    let (text, layout) = BoundRuntimeHandler::<TextInputVm>::focused_text_overrides(
        &handler.text_input_buffers,
        Some(focused_id),
        handler.text_edit_state(focused_id),
    );

    assert_eq!(text, Some("hello world"));
    assert!(layout.is_some());
}

#[test]
fn focused_input_batches_change_set_until_flush() {
    let invalidation = InvalidationSignal::new();
    let callback_count = Arc::new(AtomicUsize::new(0));
    let callback_count_capture = callback_count.clone();
    let controller = TextController::from("hi");
    let callback_controller = controller.clone();
    let tree = WidgetTree::new(Input::new(controller).on_change_set(ValueCommand::new(
        move |vm: &mut TextInputVm, _change_set: crate::mvvm::TextChangeSet| {
            callback_count_capture.fetch_add(1, Ordering::SeqCst);
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
    handler.handle_keyboard_input(&text_key_event("a"));
    handler.handle_keyboard_input(&text_key_event("b"));

    assert_eq!(handler.with_view_model(|vm| vm.value.clone()), "");
    assert_eq!(callback_count.load(Ordering::SeqCst), 0);

    flush_text_input_commits(&mut handler);

    assert_eq!(handler.with_view_model(|vm| vm.value.clone()), "hiab");
    assert_eq!(callback_count.load(Ordering::SeqCst), 1);
}

#[test]
fn textarea_on_change_does_not_force_global_invalidation() {
    let invalidation = InvalidationSignal::new();
    let callback_count = Arc::new(AtomicUsize::new(0));
    let callback_count_capture = callback_count.clone();
    let controller = TextController::from("hi");
    let tree = WidgetTree::new(Textarea::new(controller).on_change(Command::new(
        move |_vm: &mut TextInputVm| {
            callback_count_capture.fetch_add(1, Ordering::SeqCst);
        },
    )));
    let mut handler =
        test_handler_with_vm(TextInputVm::default(), Some(tree), invalidation.clone());
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

    let revision_before = invalidation.revision();

    handler.cursor_position = Some(Point {
        x: frame.x + frame.width - dp(4.0),
        y: frame.y + (frame.height * 0.5),
    });
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);
    handler.handle_keyboard_input(&text_key_event("a"));
    flush_text_input_commits(&mut handler);

    assert_eq!(callback_count.load(Ordering::SeqCst), 1);
    assert_eq!(invalidation.revision(), revision_before);
}

#[test]
fn textarea_edit_keeps_cached_scene_shell_for_text_input_patch() {
    let invalidation = InvalidationSignal::new();
    let controller = TextController::from("hi");
    let tree = WidgetTree::new(Textarea::new(controller));
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
    handler.last_invalidation_revision = handler.invalidation.revision();

    handler.cursor_position = Some(Point {
        x: frame.x + frame.width - dp(4.0),
        y: frame.y + (frame.height * 0.5),
    });
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);
    assert!(handler.focused_text_input_id().is_some());
    handler.handle_keyboard_input(&text_key_event("a"));
    flush_text_input_commits(&mut handler);
    handler.request_redraw_if_dirty(Instant::now());

    let cached = handler
        .cached_scene
        .as_ref()
        .expect("text edit should preserve the cached scene shell");
    assert!(cached.computed_valid);
    assert!(cached.layout.is_some());
    assert_eq!(cached.text_input_epoch, handler.text_input_epoch);

    let _ = handler.computed_scene();

    let cached = handler
        .cached_scene
        .as_ref()
        .expect("computed scene should remain cached after text patch");
    assert!(cached.computed_valid);
    assert_eq!(cached.text_input_epoch, handler.text_input_epoch);
}

