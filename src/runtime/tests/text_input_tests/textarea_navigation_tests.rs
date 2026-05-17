use super::*;

#[test]
fn textarea_arrow_down_moves_caret_to_next_visual_line() {
    let invalidation = InvalidationSignal::new();
    let tree = WidgetTree::new(Textarea::<TestVm>::new("hello\nworld").height(dp(120.0)));
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
    handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::ArrowDown)));

    let text_id = handler
        .focused_widget_id()
        .expect("textarea should be focused after click");
    let state = handler
        .text_edit_states
        .get(&text_id)
        .expect("textarea edit state should exist");
    assert!(state.cursor > "hello\n".len() - 1);
}

#[test]
fn textarea_edit_does_not_create_phantom_blank_line_in_layout_snapshot() {
    let invalidation = InvalidationSignal::new();
    let tree = WidgetTree::new(Textarea::<TestVm>::new("hello\nworld").height(dp(120.0)));
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

    assert!(handler.handle_keyboard_input(&text_key_event("x")));

    let session = handler
        .text_input_buffers
        .get(&text_id)
        .expect("textarea text input session should exist");
    let layout = session
        .layout_snapshot
        .as_ref()
        .expect("textarea layout snapshot should exist after edit");

    assert_eq!(session.current_text, "xhello\nworld");
    assert_eq!(layout.line_count(), 2);
    assert_eq!(layout.line_start(1), "xhello\n".len());
}

#[test]
fn textarea_click_tracks_visual_wrap_for_overflowing_initial_content() {
    let invalidation = InvalidationSignal::new();
    let value = "supercalifragilisticexpialidocious wrapped text with another long visual line";
    let tree = WidgetTree::new(
        Textarea::<TestVm>::new(value)
            .width(dp(140.0))
            .height(dp(52.0))
            .auto_wrap(true),
    );
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
                    multiline: true,
                    ..
                } => Some((*frame, *padding, text_style.clone())),
                _ => None,
            })
            .expect("textarea hit region should exist")
    };

    let content_viewport = crate::ui::widget::text_input_content_viewport(
        frame,
        padding,
        true,
        true,
        &handler.theme,
        handler.unit_context(),
    );
    let (layout, _font_size, _line_height) = super::input_text_layout(
        &handler.font_manager,
        &handler.theme,
        handler.unit_context(),
        &text_style,
        value,
        true,
        true,
        crate::ui::widget::text_input_layout_width(
            content_viewport,
            true,
            true,
            super::input::INPUT_CARET_WIDTH,
        ),
    );
    assert!(
        layout.line_count() > 1,
        "test value should wrap to multiple visual lines"
    );
    let second_line = 1;
    let sample_x = (layout.x_for_index(layout.line_end(second_line)) - 0.5).max(0.0);
    let sample_y = layout.line_top(second_line) + (layout.line_height(second_line) * 0.5);
    let expected_cursor = layout.index_for_point(sample_x, sample_y);

    handler.cursor_position = Some(Point {
        x: content_viewport.x + Dp::new(sample_x),
        y: content_viewport.y + Dp::new(sample_y),
    });
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);

    let text_id = handler
        .focused_widget_id()
        .expect("textarea should be focused after click");
    let state = handler
        .text_edit_states
        .get(&text_id)
        .expect("textarea edit state should exist");
    assert_eq!(state.cursor, expected_cursor);
}

#[test]
fn textarea_click_reuses_live_session_layout_snapshot() {
    let invalidation = InvalidationSignal::new();
    let tree = WidgetTree::new(Textarea::<TestVm>::new("abcde").height(dp(120.0)));
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
                    multiline: true,
                    ..
                } => Some((*frame, *padding, text_style.clone())),
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
    let _ = handler.sync_text_input_buffer(text_id);
    let inner = frame.inset(padding);
    let alternate_text = "a\nb\nc";
    let (alternate_layout, _font_size, _line_height) = super::input_text_layout(
        &handler.font_manager,
        &handler.theme,
        handler.unit_context(),
        &text_style,
        alternate_text,
        true,
        false,
        inner.width.get(),
    );
    let sample_line = 2usize;
    let sample_x = 0.0;
    let sample_y =
        alternate_layout.line_top(sample_line) + (alternate_layout.line_height(sample_line) * 0.5);
    let expected_cursor = alternate_layout.index_for_point(sample_x, sample_y);

    let session = handler
        .text_input_buffers
        .get_mut(&text_id)
        .expect("textarea text input session should exist");
    session.display_text = session.current_text.clone();
    session.layout_snapshot = Some(alternate_layout);

    handler.cursor_position = Some(Point {
        x: inner.x + Dp::new(sample_x),
        y: inner.y + Dp::new(sample_y),
    });
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);

    let state = handler
        .text_edit_states
        .get(&text_id)
        .expect("textarea edit state should exist");
    assert_eq!(state.cursor, expected_cursor);
}

#[test]
fn textarea_height_change_does_not_change_layout_session_config() {
    let invalidation = InvalidationSignal::new();
    let tree = WidgetTree::new(Textarea::<TestVm>::new("hello\nworld").height(dp(120.0)));
    let mut handler = test_handler(Some(tree), invalidation);
    let (text_id, frame) = {
        let computed = handler.computed_scene();
        let frame = computed
            .hit_regions
            .iter()
            .find_map(|region| match &region.interaction {
                HitInteraction::TextInput { id, .. } => Some((id, region.rect)),
                _ => None,
            })
            .expect("textarea hit region should exist");
        (*frame.0, frame.1)
    };

    let (config_signature, height) = handler
        .text_input_session_config_signature_for_test(text_id, frame)
        .expect("textarea config should exist");
    let (taller_signature, taller_height) = handler
        .text_input_session_config_signature_for_test(
            text_id,
            Rect::new(frame.x, frame.y, frame.width, frame.height + dp(48.0)),
        )
        .expect("textarea config should exist");

    assert_eq!(config_signature, taller_signature);
    assert_ne!(height, taller_height);
}

#[test]
fn repeated_tab_does_not_advance_focus() {
    let invalidation = InvalidationSignal::new();
    let first: Element<TestVm> = Input::<TestVm>::new("first").into();
    let second: Element<TestVm> = Input::<TestVm>::new("second").into();
    let tree = WidgetTree::new(Flex::new(Axis::Vertical).child([first, second]));
    let mut handler = test_handler(Some(tree), invalidation);
    let initial_focus = handler.focused_widget_id();

    assert!(!handler
        .handle_keyboard_input(&repeated_pressed_key_event(PhysicalKey::Code(KeyCode::Tab),)));
    assert_eq!(handler.focused_widget_id(), initial_focus);
}

#[test]
fn repeated_arrow_right_moves_single_line_input_cursor() {
    let invalidation = InvalidationSignal::new();
    let tree = WidgetTree::new(Input::<TestVm>::new("hello"));
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

    handler.handle_keyboard_input(&repeated_pressed_key_event(PhysicalKey::Code(
        KeyCode::ArrowRight,
    )));

    let state = handler
        .text_edit_states
        .get(&text_id)
        .expect("input edit state should exist");
    assert_eq!(state.cursor, "h".len());
    assert_eq!(state.anchor, "h".len());
}

#[test]
fn textarea_arrow_down_scrolls_caret_into_vertical_view() {
    let invalidation = InvalidationSignal::new();
    let value = "line 0\nline 1\nline 2\nline 3\nline 4";
    let tree = WidgetTree::new(Textarea::<TestVm>::new(value).height(dp(52.0)));
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

    for _ in 0..4 {
        handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::ArrowDown)));
    }

    let text_id = handler
        .focused_widget_id()
        .expect("textarea should be focused after click");
    let state = handler
        .text_edit_states
        .get(&text_id)
        .expect("textarea edit state should exist");
    let scroll_y = handler
        .scroll_states
        .get(&text_id)
        .map(|offset| offset.y)
        .unwrap_or(Dp::ZERO);

    assert!(state.cursor >= "line 0\nline 1\nline 2\n".len());
    assert!(scroll_y > Dp::ZERO);
}

#[test]
fn textarea_without_auto_wrap_keeps_keyboard_moved_caret_in_view() {
    let invalidation = InvalidationSignal::new();
    let value = (0..6)
        .map(|index| format!("line {index} 0123456789abcdef0123456789abcdef0123456789abcdef"))
        .collect::<Vec<_>>()
        .join("\n");
    let tree = WidgetTree::new(
        Textarea::<TestVm>::new(value)
            .size(dp(140.0), dp(52.0))
            .auto_wrap(false),
    );
    let mut handler = test_handler(Some(tree), invalidation);
    let viewport = handler.viewport_rect();
    let (frame, padding) = {
        let computed = handler.computed_scene();
        computed
            .hit_regions
            .iter()
            .find_map(|region| match &region.interaction {
                HitInteraction::TextInput {
                    frame,
                    padding,
                    multiline: true,
                    auto_wrap: false,
                    ..
                } => Some((*frame, *padding)),
                _ => None,
            })
            .expect("textarea hit region should exist")
    };

    handler.cursor_position = Some(Point {
        x: frame.x + dp(8.0),
        y: frame.y + dp(8.0),
    });
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);
    assert!(handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::End))));
    for _ in 0..5 {
        assert!(handler
            .handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::ArrowDown,))));
    }

    let text_id = handler
        .focused_widget_id()
        .expect("textarea should be focused after click");
    let state = handler
        .text_edit_states
        .get(&text_id)
        .expect("textarea edit state should exist");
    assert!(state.scroll_x > Dp::ZERO);
    assert!(state.scroll_y > Dp::ZERO);

    let inner = frame.inset(padding);
    let caret = handler
        .computed_scene()
        .ime_cursor_area
        .expect("focused textarea should expose a caret rect");
    assert!(caret.x >= inner.x);
    assert!(caret.right() <= inner.right() + dp(1.0));
    assert!(caret.y >= inner.y);
    assert!(caret.bottom() <= inner.bottom() + dp(1.0));
}

