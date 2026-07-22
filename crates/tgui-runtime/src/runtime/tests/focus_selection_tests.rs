use super::*;

#[test]
fn tab_index_positive_priority_and_negative_values_are_respected() {
    let invalidation = InvalidationSignal::new();
    let default_first: Element<TestVm> =
        Button::new("Default First").size(dp(80.0), dp(30.0)).into();
    let default_first_id = default_first.id;
    let default_first = default_first.tab_index(0);
    let positive_second: Element<TestVm> = Button::new("Positive Second")
        .size(dp(80.0), dp(30.0))
        .into();
    let positive_second_id = positive_second.id;
    let positive_second = positive_second.tab_index(2);
    let positive_first: Element<TestVm> = Button::new("Positive First")
        .size(dp(80.0), dp(30.0))
        .into();
    let positive_first_id = positive_first.id;
    let positive_first = positive_first.tab_index(1);
    let skipped: Element<TestVm> = Button::new("Skipped").size(dp(80.0), dp(30.0)).into();
    let skipped = skipped.tab_index(-1);
    let default_second: Element<TestVm> = Button::new("Default Second")
        .size(dp(80.0), dp(30.0))
        .into();
    let default_second_id = default_second.id;
    let tree = WidgetTree::new(Flex::new(Axis::Vertical).child([
        default_first,
        positive_second,
        positive_first,
        skipped,
        default_second,
    ]));
    let mut handler = test_handler(Some(tree), invalidation);

    handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Tab)));
    assert_eq!(handler.focused_widget_id(), Some(positive_first_id));

    handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Tab)));
    assert_eq!(handler.focused_widget_id(), Some(positive_second_id));

    handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Tab)));
    assert_eq!(handler.focused_widget_id(), Some(default_first_id));

    handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Tab)));
    assert_eq!(handler.focused_widget_id(), Some(default_second_id));

    handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Tab)));
    assert_eq!(handler.focused_widget_id(), Some(positive_first_id));
}

#[test]
fn focus_trap_loops_tab_order_and_blocks_pointer_focus_escape() {
    let invalidation = InvalidationSignal::new();
    let inner_first: Element<TestVm> = Button::new("Inner First").size(dp(80.0), dp(30.0)).into();
    let inner_first_id = inner_first.id;
    let inner_second: Element<TestVm> = Button::new("Inner Second").size(dp(80.0), dp(30.0)).into();
    let inner_second_id = inner_second.id;
    let outside: Element<TestVm> = Button::new("Outside")
        .size(dp(80.0), dp(30.0))
        .position_absolute()
        .top(dp(90.0))
        .into();
    let tree = WidgetTree::new(
        Stack::new().child([
            Flex::new(Axis::Vertical)
                .focus_scope(FocusScopeOptions::new().trap(true))
                .child([inner_first, inner_second])
                .into(),
            outside,
        ]),
    );
    let mut handler = test_handler(Some(tree), invalidation);
    let viewport = handler.viewport_rect();

    handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Tab)));
    assert_eq!(handler.focused_widget_id(), Some(inner_first_id));

    handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Tab)));
    assert_eq!(handler.focused_widget_id(), Some(inner_second_id));

    handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Tab)));
    assert_eq!(handler.focused_widget_id(), Some(inner_first_id));

    handler.cursor_position = Some(Point::new(dp(10.0), dp(100.0)));
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);
    assert_eq!(handler.focused_widget_id(), Some(inner_first_id));
}

#[test]
fn auto_focus_first_focuses_first_candidate_in_tab_order_on_activation() {
    let invalidation = InvalidationSignal::new();
    let default_first: Element<TestVm> = Button::new("Default").size(dp(80.0), dp(30.0)).into();
    let positive: Element<TestVm> = Button::new("Positive").size(dp(80.0), dp(30.0)).into();
    let positive_id = positive.id;
    let tree = WidgetTree::new(
        Flex::new(Axis::Vertical)
            .auto_focus_first(true)
            .child([default_first, positive.tab_index(1)]),
    );
    let mut handler = test_handler(Some(tree), invalidation);

    let _ = handler.computed_scene();

    assert_eq!(handler.focused_widget_id(), Some(positive_id));
}

#[test]
fn auto_focus_first_does_not_steal_focus_already_inside_scope() {
    let invalidation = InvalidationSignal::new();
    let first: Element<TestVm> = Button::new("First").size(dp(80.0), dp(30.0)).into();
    let second: Element<TestVm> = Button::new("Second").size(dp(80.0), dp(30.0)).into();
    let second_id = second.id;
    let scope: Element<TestVm> = Flex::new(Axis::Vertical)
        .auto_focus_first(true)
        .child([first, second])
        .into();
    let scope_id = scope.id;
    let tree = WidgetTree::new(scope);
    let mut handler = test_handler(Some(tree), invalidation);
    handler.focused_widget = Some(FocusedWidget {
        widget_id: second_id,
        scope_path: vec![scope_id],
        on_blur: None,
    });

    let _ = handler.computed_scene();

    assert_eq!(handler.focused_widget_id(), Some(second_id));
}

#[test]
fn auto_focus_first_only_runs_when_topmost_scope_changes() {
    let invalidation = InvalidationSignal::new();
    let first: Element<TestVm> = Button::new("First").size(dp(80.0), dp(30.0)).into();
    let first_id = first.id;
    let outside: Element<TestVm> = Button::new("Outside")
        .size(dp(80.0), dp(30.0))
        .position_absolute()
        .top(dp(60.0))
        .into();
    let outside_id = outside.id;
    let tree = WidgetTree::new(
        Stack::new().child([
            Flex::new(Axis::Vertical)
                .auto_focus_first(true)
                .child(first)
                .into(),
            outside,
        ]),
    );
    let mut handler = test_handler(Some(tree), invalidation);

    let _ = handler.computed_scene();
    assert_eq!(handler.focused_widget_id(), Some(first_id));

    handler.focused_widget = Some(FocusedWidget {
        widget_id: outside_id,
        scope_path: Vec::new(),
        on_blur: None,
    });
    handler.invalidate_computed_scene();
    let _ = handler.computed_scene();

    assert_eq!(handler.focused_widget_id(), Some(outside_id));
}

#[test]
fn inactive_focus_scope_descendants_are_removed_from_tab_order() {
    let invalidation = InvalidationSignal::new();
    let hidden: Element<TestVm> = Button::new("Hidden").size(dp(80.0), dp(30.0)).into();
    let outside: Element<TestVm> = Button::new("Outside").size(dp(80.0), dp(30.0)).into();
    let outside_id = outside.id;
    let tree = WidgetTree::new(
        Flex::new(Axis::Vertical).child([
            Flex::new(Axis::Vertical)
                .focus_scope(
                    FocusScopeOptions::new()
                        .active(false)
                        .auto_focus_first(true),
                )
                .child(hidden)
                .into(),
            outside,
        ]),
    );
    let mut handler = test_handler(Some(tree), invalidation);

    handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Tab)));

    assert_eq!(handler.focused_widget_id(), Some(outside_id));
}

#[test]
fn enter_space_and_escape_drive_default_focus_actions() {
    let invalidation = InvalidationSignal::new();
    let button_clicks = Arc::new(AtomicUsize::new(0));
    let checkbox_value = Arc::new(Mutex::new(false));
    let switch_value = Arc::new(Mutex::new(false));

    let button_clicks_ref = Arc::clone(&button_clicks);
    let button: Element<TestVm> = Button::new("Button")
        .size(dp(80.0), dp(30.0))
        .on_click(Command::new(move |_vm: &mut TestVm| {
            button_clicks_ref.fetch_add(1, Ordering::SeqCst);
        }))
        .into();
    let button_id = button.id;

    let checkbox_value_ref = Arc::clone(&checkbox_value);
    let checkbox: Element<TestVm> = Checkbox::new(false)
        .size(dp(80.0), dp(30.0))
        .on_change(ValueCommand::new(move |_vm: &mut TestVm, value| {
            *checkbox_value_ref
                .lock()
                .expect("checkbox state lock should succeed") = value;
        }))
        .into();
    let checkbox_id = checkbox.id;

    let switch_value_ref = Arc::clone(&switch_value);
    let switch: Element<TestVm> = Switch::new(false)
        .size(dp(80.0), dp(30.0))
        .on_change(ValueCommand::new(move |_vm: &mut TestVm, value| {
            *switch_value_ref
                .lock()
                .expect("switch state lock should succeed") = value;
        }))
        .into();
    let switch_id = switch.id;

    let select: Element<TestVm> = Select::new(
        vec![SelectOption::new("email".to_string(), "Email".to_string())],
        None::<String>,
    )
    .size(dp(160.0), dp(32.0))
    .into();
    let select_id = select.id;

    let tree = WidgetTree::new(Flex::new(Axis::Vertical).child([button, checkbox, switch, select]));
    let mut handler = test_handler(Some(tree), invalidation);

    handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Tab)));
    assert_eq!(handler.focused_widget_id(), Some(button_id));
    assert!(handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Enter))));
    assert_eq!(button_clicks.load(Ordering::SeqCst), 1);

    handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Tab)));
    assert_eq!(handler.focused_widget_id(), Some(checkbox_id));
    assert!(handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Space))));
    assert!(*checkbox_value
        .lock()
        .expect("checkbox state lock should succeed"));

    handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Tab)));
    assert_eq!(handler.focused_widget_id(), Some(switch_id));
    assert!(handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Space))));
    assert!(*switch_value
        .lock()
        .expect("switch state lock should succeed"));

    handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Tab)));
    assert_eq!(handler.focused_widget_id(), Some(select_id));
    assert!(handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Enter))));
    assert_eq!(handler.resolved_select_open_state(select_id), Some(true));

    let event_loop = TestEventLoop;
    let _ = handler.computed_scene();
    handler.drive_animations(&event_loop, Instant::now() + Duration::from_millis(40));
    assert!(!handler.computed_scene().overlay_close_handlers.is_empty());
    assert!(handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Escape))));
    assert_eq!(handler.resolved_select_open_state(select_id), Some(false));
    assert_eq!(handler.focused_widget_id(), Some(select_id));
}

#[test]
fn scene_cache_invalidates_when_pressed_widget_changes() {
    let invalidation = InvalidationSignal::new();
    let mut handler = test_handler(None, invalidation);
    let viewport = handler.viewport_rect();
    let cached = cached_scene_shell(&handler, viewport, UnitContext::new(1.0, 1.0));

    handler.pressed_widget = Some(WidgetId::next());

    assert!(!handler.scene_cache_matches(
        &cached,
        viewport,
        UnitContext::new(1.0, 1.0),
        false,
        None,
    ));
}

#[test]
fn scene_cache_invalidates_when_focused_widget_changes() {
    let invalidation = InvalidationSignal::new();
    let mut handler = test_handler(None, invalidation);
    let viewport = handler.viewport_rect();
    let cached = cached_scene_shell(&handler, viewport, UnitContext::new(1.0, 1.0));

    handler.focused_widget = Some(super::FocusedWidget {
        widget_id: WidgetId::next(),
        scope_path: Vec::new(),
        on_blur: None,
    });

    assert!(!handler.scene_cache_matches(
        &cached,
        viewport,
        UnitContext::new(1.0, 1.0),
        false,
        None,
    ));
}

#[test]
fn user_select_text_defaults_to_text_cursor() {
    let invalidation = InvalidationSignal::new();
    let tree = WidgetTree::new(Text::new("hover").user_select(true));
    let mut handler = test_handler(Some(tree), invalidation);
    handler.cursor_position = Some(Point::new(dp(10.0), dp(10.0)));

    let viewport = handler.viewport_rect();
    let hovered = handler.hover_path(viewport);
    assert_eq!(
        hovered.last().and_then(|hovered| hovered.cursor_style),
        Some(CursorStyle::Text)
    );
}

#[test]
fn disabled_control_defaults_to_not_allowed_cursor() {
    let invalidation = InvalidationSignal::new();
    let tree = WidgetTree::new(Checkbox::new(false).disable(true).size(dp(120.0), dp(30.0)));
    let mut handler = test_handler(Some(tree), invalidation);
    handler.cursor_position = Some(Point::new(dp(10.0), dp(10.0)));

    let hovered = handler.hover_path(handler.viewport_rect());
    assert_eq!(
        hovered.last().and_then(|hovered| hovered.cursor_style),
        Some(CursorStyle::NotAllowed)
    );
}

#[test]
fn clicking_open_select_trigger_closes_dropdown() {
    let invalidation = InvalidationSignal::new();
    let select: Element<TestVm> = Select::new(
        vec![SelectOption::new("email".to_string(), "Email".to_string())],
        None::<String>,
    )
    .size(dp(160.0), dp(32.0))
    .into();
    let select_id = select.id;
    let tree = WidgetTree::new(select);
    let mut handler = test_handler(Some(tree), invalidation);
    let viewport = handler.viewport_rect();

    handler.cursor_position = Some(Point::new(dp(10.0), dp(10.0)));
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);
    assert_eq!(handler.focused_widget_id(), Some(select_id));
    assert_eq!(handler.resolved_select_open_state(select_id), Some(true));

    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);
    assert_eq!(handler.focused_widget_id(), Some(select_id));
    assert_eq!(handler.resolved_select_open_state(select_id), Some(false));
}

#[test]
fn clicking_outside_closes_open_select_dropdown() {
    let invalidation = InvalidationSignal::new();
    let select: Element<TestVm> = Select::new(
        vec![SelectOption::new("email".to_string(), "Email".to_string())],
        None::<String>,
    )
    .size(dp(160.0), dp(32.0))
    .into();
    let select_id = select.id;
    let filler: Element<TestVm> = Button::new("Other")
        .size(dp(160.0), dp(32.0))
        .top(dp(40.0))
        .position_absolute()
        .into();
    let tree = WidgetTree::new(Stack::new().child([select, filler]));
    let mut handler = test_handler(Some(tree), invalidation);
    let viewport = handler.viewport_rect();

    handler.cursor_position = Some(Point::new(dp(10.0), dp(10.0)));
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);
    assert_eq!(handler.resolved_select_open_state(select_id), Some(true));

    handler.cursor_position = Some(Point::new(dp(10.0), dp(60.0)));
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);
    assert_eq!(handler.resolved_select_open_state(select_id), Some(false));
}

#[test]
fn tab_focuses_first_widget_when_none_is_focused() {
    let invalidation = InvalidationSignal::new();
    let first: Element<TestVm> = Button::new("First").size(dp(80.0), dp(30.0)).into();
    let first_id = first.id;
    let second: Element<TestVm> = Button::new("Second").size(dp(80.0), dp(30.0)).into();
    let tree = WidgetTree::new(Flex::new(Axis::Vertical).child([first, second]));
    let mut handler = test_handler(Some(tree), invalidation);

    let changed =
        handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Tab)));

    assert!(changed);
    assert_eq!(handler.focused_widget_id(), Some(first_id));
}

#[test]
fn tab_advances_to_next_focusable_widget() {
    let invalidation = InvalidationSignal::new();
    let first: Element<TestVm> = Button::new("First").size(dp(80.0), dp(30.0)).into();
    let first_id = first.id;
    let second: Element<TestVm> = Button::new("Second").size(dp(80.0), dp(30.0)).into();
    let second_id = second.id;
    let tree = WidgetTree::new(Flex::new(Axis::Vertical).child([first, second]));
    let mut handler = test_handler(Some(tree), invalidation);
    handler.focused_widget = Some(super::FocusedWidget {
        widget_id: first_id,
        scope_path: Vec::new(),
        on_blur: None,
    });

    let changed =
        handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Tab)));

    assert!(changed);
    assert_eq!(handler.focused_widget_id(), Some(second_id));
}

#[test]
fn shift_tab_moves_focus_backward() {
    let invalidation = InvalidationSignal::new();
    let first: Element<TestVm> = Button::new("First").size(dp(80.0), dp(30.0)).into();
    let first_id = first.id;
    let second: Element<TestVm> = Button::new("Second").size(dp(80.0), dp(30.0)).into();
    let second_id = second.id;
    let tree = WidgetTree::new(Flex::new(Axis::Vertical).child([first, second]));
    let mut handler = test_handler(Some(tree), invalidation);
    handler.focused_widget = Some(super::FocusedWidget {
        widget_id: second_id,
        scope_path: Vec::new(),
        on_blur: None,
    });
    handler.modifiers = ModifiersState::SHIFT;

    let changed =
        handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Tab)));

    assert!(changed);
    assert_eq!(handler.focused_widget_id(), Some(first_id));
}

#[test]
fn mouse_focus_marks_focused_without_focus_visible() {
    let invalidation = InvalidationSignal::new();
    let button: Element<TestVm> = Button::new("First").size(dp(80.0), dp(30.0)).into();
    let button_id = button.id;
    let tree = WidgetTree::new(button);
    let mut handler = test_handler(Some(tree), invalidation);

    handler.cursor_position = Some(Point::new(dp(10.0), dp(10.0)));
    handler.handle_mouse_press(
        handler.viewport_rect(),
        Instant::now(),
        CanvasMouseButton::Left,
    );

    assert_eq!(handler.focused_widget_id(), Some(button_id));
    let state = handler.widget_state_map(None).get(button_id);
    assert!(state.focused);
    assert!(!state.focus_visible);
}

#[test]
fn tab_focus_marks_focused_and_focus_visible() {
    let invalidation = InvalidationSignal::new();
    let button: Element<TestVm> = Button::new("First").size(dp(80.0), dp(30.0)).into();
    let button_id = button.id;
    let tree = WidgetTree::new(button);
    let mut handler = test_handler(Some(tree), invalidation);

    handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Tab)));

    assert_eq!(handler.focused_widget_id(), Some(button_id));
    let state = handler.widget_state_map(None).get(button_id);
    assert!(state.focused);
    assert!(state.focus_visible);
}

#[test]
fn dragging_selectable_text_updates_selection_range() {
    let invalidation = InvalidationSignal::new();
    let tree = WidgetTree::new(Text::new("hello").user_select(true));
    let mut handler = test_handler(Some(tree), invalidation);
    let viewport = handler.viewport_rect();

    let (text_id, frame, padding, text_style, text) = {
        let computed = handler.computed_scene();
        computed
            .hit_regions
            .iter()
            .find_map(|region| match &region.interaction {
                HitInteraction::SelectableText {
                    id,
                    frame,
                    padding,
                    text_style,
                    text,
                    ..
                } => Some((*id, *frame, *padding, text_style.clone(), text.clone())),
                _ => None,
            })
            .expect("selectable text hit region should exist")
    };

    handler.cursor_position = Some(Point {
        x: frame.x + 1.0,
        y: frame.y + (frame.height * 0.5),
    });
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);

    handler.cursor_position = Some(Point {
        x: frame.x + frame.width - 1.0,
        y: frame.y + (frame.height * 0.5),
    });
    assert!(handler.handle_text_selection_drag());
    assert_eq!(handler.selected_text, Some(text_id));

    let state = handler
        .text_edit_states
        .get(&text_id)
        .expect("text selection state should be recorded");
    assert_eq!(state.selection_range(), Some((0, text.len())));
    assert_eq!(state.anchor, 0);
    assert_eq!(
        state.cursor,
        text_cursor_index_at_point(
            &handler.font_manager,
            &handler.theme,
            handler.unit_context(),
            input::TextInputContext {
                frame,
                padding,
                text_style: &text_style,
                text: &text,
                multiline: false,
                auto_wrap: false,
                show_scrollbar: false,
            },
            input::ScrollContext::ZERO,
            Point {
                x: frame.x + frame.width - 1.0,
                y: frame.y + (frame.height * 0.5),
            },
        )
    );
}

#[test]
fn selectable_text_can_provide_selected_content_for_copy() {
    let invalidation = InvalidationSignal::new();
    let tree = WidgetTree::new(Text::new("hello world").user_select(true));
    let mut handler = test_handler(Some(tree), invalidation);
    let text_id = {
        let computed = handler.computed_scene();
        computed
            .hit_regions
            .iter()
            .find_map(|region| match &region.interaction {
                HitInteraction::SelectableText { id, .. } => Some(*id),
                _ => None,
            })
            .expect("selectable text hit region should exist")
    };

    handler.selected_text = Some(text_id);
    handler.text_edit_states.insert(
        text_id,
        TextEditState {
            cursor: 11,
            anchor: 6,
            composition: None,
            scroll_x: Dp::ZERO,
            scroll_y: Dp::ZERO,
            preferred_column_x: None,
        },
    );

    assert_eq!(handler.selected_text_for_copy().as_deref(), Some("world"));
}

#[test]
fn dragging_multiline_selectable_text_can_copy_across_lines() {
    let invalidation = InvalidationSignal::new();
    let content = "alpha\nbeta\ngamma";
    let tree = WidgetTree::new(Text::new(content).user_select(true));
    let mut handler = test_handler(Some(tree), invalidation);
    let viewport = handler.viewport_rect();

    let text_id = {
        let computed = handler.computed_scene();
        computed
            .hit_regions
            .iter()
            .find_map(|region| match &region.interaction {
                HitInteraction::SelectableText { id, frame, .. } => Some((*id, *frame)),
                _ => None,
            })
            .expect("selectable text hit region should exist")
    };

    handler.cursor_position = Some(Point {
        x: text_id.1.x + 1.0,
        y: text_id.1.y + 1.0,
    });
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);

    handler.cursor_position = Some(Point {
        x: text_id.1.x + text_id.1.width - 1.0,
        y: text_id.1.y + text_id.1.height - 1.0,
    });
    assert!(handler.handle_text_selection_drag());

    assert_eq!(handler.selected_text, Some(text_id.0));
    assert_eq!(handler.selected_text_for_copy().as_deref(), Some(content));
}
