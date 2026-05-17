use super::*;

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
