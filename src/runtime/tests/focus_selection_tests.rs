use super::*;

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
        on_blur: None,
    });
    handler.modifiers = ModifiersState::SHIFT;

    let changed =
        handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Tab)));

    assert!(changed);
    assert_eq!(handler.focused_widget_id(), Some(first_id));
}

#[test]
fn mouse_focus_does_not_mark_widget_as_focused_for_styling() {
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
    assert!(!handler.widget_state_map(None).get(button_id).focused);
}

#[test]
fn tab_focus_marks_widget_as_focused_for_styling() {
    let invalidation = InvalidationSignal::new();
    let button: Element<TestVm> = Button::new("First").size(dp(80.0), dp(30.0)).into();
    let button_id = button.id;
    let tree = WidgetTree::new(button);
    let mut handler = test_handler(Some(tree), invalidation);

    handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Tab)));

    assert_eq!(handler.focused_widget_id(), Some(button_id));
    assert!(handler.widget_state_map(None).get(button_id).focused);
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
            frame,
            padding,
            &text_style,
            &text,
            false,
            false,
            false,
            Point::ZERO,
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

