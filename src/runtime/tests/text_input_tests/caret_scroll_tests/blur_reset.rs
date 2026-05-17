use super::*;

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
