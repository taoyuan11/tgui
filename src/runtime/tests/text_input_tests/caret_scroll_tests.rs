use super::*;

#[test]
fn focused_text_input_schedules_caret_blink_deadline() {
    let invalidation = InvalidationSignal::new();
    let tree: WidgetTree<TestVm> = WidgetTree::new(Input::<TestVm>::new("hello"));
    let mut handler = test_handler(Some(tree), invalidation);
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
        x: frame.x + (frame.width * 0.5),
        y: frame.y + (frame.height * 0.5),
    });
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);

    let deadline = handler.next_deadline(Instant::now());
    assert!(deadline.is_some());

    handler.update_focus(None, None, false);
    let deadline = handler.next_deadline(Instant::now());
    assert!(deadline.is_none());
}

#[test]
fn caret_blink_requests_redraw_when_visibility_flips() {
    let invalidation = InvalidationSignal::new();
    let tree: WidgetTree<TestVm> = WidgetTree::new(Input::<TestVm>::new("hello"));
    let mut handler = test_handler(Some(tree), invalidation);
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
        x: frame.x + (frame.width * 0.5),
        y: frame.y + (frame.height * 0.5),
    });
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);
    let _ = handler.computed_scene();

    handler
        .cached_scene
        .as_mut()
        .expect("cached scene should exist after rendering")
        .caret_visible = false;

    let now = handler.caret_blink_origin + std::time::Duration::from_millis(1);
    assert!(handler.caret_blink_needs_redraw(now));
}

#[test]
fn clicking_text_input_renders_caret_on_first_focused_frame() {
    let invalidation = InvalidationSignal::new();
    let tree: WidgetTree<TestVm> = WidgetTree::new(Input::<TestVm>::new("hello"));
    let mut handler = test_handler(Some(tree), invalidation);
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

    let computed = handler.computed_scene();
    assert!(
        computed.ime_cursor_area.is_some(),
        "focused input should expose a caret rect on the first focused frame"
    );
    assert!(
        !computed.scene.overlay_shapes.is_empty(),
        "focused input should render the caret immediately after click"
    );
}

#[test]
fn non_text_focus_does_not_schedule_caret_blink_deadline() {
    let invalidation = InvalidationSignal::new();
    let tree: WidgetTree<TestVm> = WidgetTree::new(Button::new("click"));
    let mut handler = test_handler(Some(tree), invalidation);

    handler.update_focus(
        Some(FocusedWidget {
            widget_id: WidgetId::from_raw(999),
            on_blur: None,
        }),
        None,
        false,
    );

    let deadline = handler.next_deadline(Instant::now());
    assert!(deadline.is_none());
}

#[test]
fn single_line_input_scrolls_horizontally_to_keep_caret_visible() {
    let invalidation = InvalidationSignal::new();
    let value = "0123456789abcdef0123456789";
    let tree = WidgetTree::new(Input::<TestVm>::new(value).size(dp(96.0), dp(40.0)));
    let mut handler = test_handler(Some(tree), invalidation);
    let viewport = handler.viewport_rect();
    let (frame, padding) = {
        let computed = handler.computed_scene();
        computed
            .hit_regions
            .iter()
            .find_map(|region| match &region.interaction {
                HitInteraction::TextInput { frame, padding, .. } => Some((*frame, *padding)),
                _ => None,
            })
            .expect("input hit region should exist")
    };

    handler.cursor_position = Some(Point {
        x: frame.x + dp(8.0),
        y: frame.y + (frame.height * 0.5),
    });
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);
    handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::End)));

    let text_id = handler
        .focused_widget_id()
        .expect("input should be focused after click");
    let state = handler
        .text_edit_states
        .get(&text_id)
        .expect("input edit state should exist");
    assert!(state.scroll_x > Dp::ZERO);

    let inner = frame.inset(padding);
    let caret = handler
        .computed_scene()
        .ime_cursor_area
        .expect("focused input should expose a caret rect");
    assert!(caret.x >= inner.x);
    assert!(caret.right() <= inner.right() + dp(1.0));
}

#[test]
fn clicking_scrolled_single_line_input_repositions_caret_within_visible_text() {
    let invalidation = InvalidationSignal::new();
    let value = "0123456789abcdef0123456789";
    let tree = WidgetTree::new(Input::<TestVm>::new(value).size(dp(96.0), dp(40.0)));
    let mut handler = test_handler(Some(tree), invalidation);
    let viewport = handler.viewport_rect();
    let (frame, padding, text_style) = {
        let computed = handler.computed_scene();
        computed
            .hit_regions
            .iter()
            .find_map(|region| match &region.interaction {
                HitInteraction::TextInput {
                    frame,
                    padding,
                    text_style,
                    ..
                } => Some((*frame, *padding, text_style.clone())),
                _ => None,
            })
            .expect("input hit region should exist")
    };

    handler.cursor_position = Some(Point {
        x: frame.x + dp(8.0),
        y: frame.y + (frame.height * 0.5),
    });
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);
    handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::End)));

    let text_id = handler
        .focused_widget_id()
        .expect("input should be focused after click");
    let scrolled_x = handler
        .text_edit_states
        .get(&text_id)
        .expect("input edit state should exist")
        .scroll_x;
    assert!(scrolled_x > Dp::ZERO);

    let inner = frame.inset(padding);
    let click_point = Point {
        x: inner.x + dp(12.0),
        y: inner.y + (inner.height * 0.5),
    };
    let expected_cursor = text_cursor_index_at_point(
        &handler.font_manager,
        &handler.theme,
        handler.unit_context(),
        frame,
        padding,
        &text_style,
        value,
        false,
        false,
        false,
        Point::new(scrolled_x, Dp::ZERO),
        click_point,
    );

    handler.cursor_position = Some(click_point);
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);

    let state = handler
        .text_edit_states
        .get(&text_id)
        .expect("input edit state should exist after click");
    assert_eq!(state.cursor, expected_cursor);
    assert_eq!(state.anchor, expected_cursor);
    assert!(state.cursor < value.len());
    assert!(
        (state.scroll_x - scrolled_x).abs() <= 0.01,
        "clicking within the visible span should preserve horizontal scroll"
    );

    let caret = handler
        .computed_scene()
        .ime_cursor_area
        .expect("focused input should expose a caret rect");
    assert!(caret.right() < inner.right() - dp(8.0));
}

#[test]
fn dragging_in_scrolled_single_line_input_tracks_pointer_in_visible_text() {
    let invalidation = InvalidationSignal::new();
    let value = "0123456789abcdef0123456789";
    let tree = WidgetTree::new(Input::<TestVm>::new(value).size(dp(96.0), dp(40.0)));
    let mut handler = test_handler(Some(tree), invalidation);
    let viewport = handler.viewport_rect();
    let (frame, padding, text_style) = {
        let computed = handler.computed_scene();
        computed
            .hit_regions
            .iter()
            .find_map(|region| match &region.interaction {
                HitInteraction::TextInput {
                    frame,
                    padding,
                    text_style,
                    ..
                } => Some((*frame, *padding, text_style.clone())),
                _ => None,
            })
            .expect("input hit region should exist")
    };

    handler.cursor_position = Some(Point {
        x: frame.x + dp(8.0),
        y: frame.y + (frame.height * 0.5),
    });
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);
    handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::End)));

    let text_id = handler
        .focused_widget_id()
        .expect("input should be focused after click");
    let scrolled_x = handler
        .text_edit_states
        .get(&text_id)
        .expect("input edit state should exist")
        .scroll_x;
    assert!(scrolled_x > Dp::ZERO);

    let inner = frame.inset(padding);
    let press_point = Point {
        x: inner.x + dp(10.0),
        y: inner.y + (inner.height * 0.5),
    };
    let press_cursor = text_cursor_index_at_point(
        &handler.font_manager,
        &handler.theme,
        handler.unit_context(),
        frame,
        padding,
        &text_style,
        value,
        false,
        false,
        false,
        Point::new(scrolled_x, Dp::ZERO),
        press_point,
    );

    handler.cursor_position = Some(press_point);
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);

    let pressed_state = handler
        .text_edit_states
        .get(&text_id)
        .expect("input edit state should exist after press");
    assert_eq!(pressed_state.cursor, press_cursor);
    assert_eq!(pressed_state.anchor, press_cursor);
    assert!(pressed_state.cursor < value.len());
    let pressed_scroll_x = pressed_state.scroll_x;
    assert!(
        (pressed_scroll_x - scrolled_x).abs() <= 0.01,
        "pressing within the visible span should not reset horizontal scroll"
    );

    let drag_point = Point {
        x: inner.x + (inner.width * 0.5),
        y: inner.y + (inner.height * 0.5),
    };
    let drag_cursor = text_cursor_index_at_point(
        &handler.font_manager,
        &handler.theme,
        handler.unit_context(),
        frame,
        padding,
        &text_style,
        value,
        false,
        false,
        false,
        Point::new(pressed_scroll_x, Dp::ZERO),
        drag_point,
    );

    handler.cursor_position = Some(drag_point);
    assert!(handler.handle_text_selection_drag());

    let state = handler
        .text_edit_states
        .get(&text_id)
        .expect("input edit state should exist after drag");
    assert_eq!(
        state.selection_range(),
        Some((press_cursor.min(drag_cursor), press_cursor.max(drag_cursor)))
    );
    assert!(state.cursor < value.len());
    assert!(state.anchor < value.len());
}

#[test]
fn ime_preedit_scrolls_single_line_input_to_keep_composition_caret_visible() {
    let invalidation = InvalidationSignal::new();
    let tree = WidgetTree::new(Input::<TestVm>::new("").size(dp(96.0), dp(40.0)));
    let mut handler = test_handler(Some(tree), invalidation);
    let viewport = handler.viewport_rect();
    let composition = "0123456789abcdef".repeat(3);
    let (frame, padding) = {
        let computed = handler.computed_scene();
        computed
            .hit_regions
            .iter()
            .find_map(|region| match &region.interaction {
                HitInteraction::TextInput { frame, padding, .. } => Some((*frame, *padding)),
                _ => None,
            })
            .expect("input hit region should exist")
    };

    handler.cursor_position = Some(Point {
        x: frame.x + dp(8.0),
        y: frame.y + (frame.height * 0.5),
    });
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);
    assert!(handler.handle_ime_event(&Ime::Preedit(
        composition.clone(),
        Some((0, composition.len())),
    )));

    let text_id = handler
        .focused_widget_id()
        .expect("input should be focused after click");
    let state = handler
        .text_edit_states
        .get(&text_id)
        .expect("input edit state should exist");
    assert!(state.scroll_x > Dp::ZERO);

    let inner = frame.inset(padding);
    let caret = handler
        .computed_scene()
        .ime_cursor_area
        .expect("focused input should expose a caret rect");
    assert!(caret.x >= inner.x);
    assert!(caret.right() <= inner.right() + dp(1.0));
}

#[test]
fn single_line_input_blur_resets_scroll_and_caret_to_start() {
    let invalidation = InvalidationSignal::new();
    let value = "0123456789abcdef0123456789";
    let tree = WidgetTree::new(Input::<TestVm>::new(value).size(dp(96.0), dp(40.0)));
    let mut handler = test_handler(Some(tree), invalidation);
    let viewport = handler.viewport_rect();
    let (frame, padding) = {
        let computed = handler.computed_scene();
        computed
            .hit_regions
            .iter()
            .find_map(|region| match &region.interaction {
                HitInteraction::TextInput { frame, padding, .. } => Some((*frame, *padding)),
                _ => None,
            })
            .expect("input hit region should exist")
    };

    handler.cursor_position = Some(Point {
        x: frame.x + dp(8.0),
        y: frame.y + (frame.height * 0.5),
    });
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);
    handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::End)));

    let text_id = handler
        .focused_widget_id()
        .expect("input should be focused after click");
    let scrolled_state = handler
        .text_edit_states
        .get(&text_id)
        .expect("input edit state should exist");
    assert!(scrolled_state.scroll_x > Dp::ZERO);
    assert!(scrolled_state.cursor > 0);

    handler.update_focus(None, None, false);

    let blurred_state = handler
        .text_edit_states
        .get(&text_id)
        .expect("input edit state should still exist after blur");
    assert_eq!(blurred_state.cursor, 0);
    assert_eq!(blurred_state.anchor, 0);
    assert_eq!(blurred_state.scroll_x, Dp::ZERO);
    assert_eq!(blurred_state.scroll_y, Dp::ZERO);
    assert!(!handler.scroll_states.contains_key(&text_id));

    let next_focus = handler
        .focusable_widgets_in_tab_order()
        .into_iter()
        .find(|candidate| candidate.widget_id == text_id)
        .expect("input should remain focusable");
    handler.update_focus(Some(next_focus), None, true);

    let inner = frame.inset(padding);
    let caret = handler
        .computed_scene()
        .ime_cursor_area
        .expect("refocused input should expose a caret rect");
    assert!(caret.x >= inner.x);
    assert!(caret.x <= inner.x + dp(1.0));
}

#[test]
fn single_line_input_blur_resets_scroll_even_without_cached_scene() {
    let invalidation = InvalidationSignal::new();
    let value = "0123456789abcdef0123456789";
    let tree = WidgetTree::new(Input::<TestVm>::new(value).size(dp(96.0), dp(40.0)));
    let mut handler = test_handler(Some(tree), invalidation);
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
    handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::End)));

    let text_id = handler
        .focused_widget_id()
        .expect("input should be focused after click");
    assert!(
        handler
            .text_edit_states
            .get(&text_id)
            .expect("input edit state should exist")
            .scroll_x
            > Dp::ZERO
    );

    handler.cached_scene = None;
    handler.update_focus(None, None, false);

    let blurred_state = handler
        .text_edit_states
        .get(&text_id)
        .expect("input edit state should still exist after blur");
    assert_eq!(blurred_state.cursor, 0);
    assert_eq!(blurred_state.anchor, 0);
    assert_eq!(blurred_state.scroll_x, Dp::ZERO);
    assert_eq!(blurred_state.scroll_y, Dp::ZERO);
    assert!(!handler.scroll_states.contains_key(&text_id));
}

