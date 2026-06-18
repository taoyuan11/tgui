use super::*;

#[test]
fn focused_text_input_page_scroll_updates_scene_without_blur() {
    let invalidation = InvalidationSignal::new();
    let scroller: Element<TestVm> = Stack::new()
        .size(dp(320.0), dp(240.0))
        .overflow_y(Overflow::Scroll)
        .child(
            Flex::vertical().height(dp(860.0)).gap(dp(12.0)).child([
                Element::<TestVm>::from(Input::new("hello world").height(dp(40.0))),
                Element::<TestVm>::from(
                    Textarea::new(
                        (0..10)
                            .map(|index| format!("line {index}"))
                            .collect::<Vec<_>>()
                            .join("\n"),
                    )
                    .height(dp(72.0)),
                ),
                Element::<TestVm>::from(Stack::new().height(dp(640.0))),
            ]),
        )
        .into();
    let scroller_id = scroller.id;
    let tree = WidgetTree::new(scroller);
    let mut handler = test_handler_with_config(
        TestVm,
        Some(tree),
        invalidation,
        test_config_with_size(320.0, 240.0),
    );
    let viewport = handler.viewport_rect();

    let (input_frame, scroll_region) = {
        let computed = handler.computed_scene();
        let input_frame = computed
            .hit_regions
            .iter()
            .find_map(|region| match &region.interaction {
                HitInteraction::TextInput {
                    multiline: false, ..
                } => Some(region.rect),
                _ => None,
            })
            .expect("input hit region should exist");
        let scroll_region = computed
            .scroll_regions
            .iter()
            .find(|region| region.id == scroller_id)
            .copied()
            .expect("parent scroll region should exist");
        (input_frame, scroll_region)
    };

    handler.cursor_position = Some(Point {
        x: input_frame.x + dp(8.0),
        y: input_frame.y + dp(8.0),
    });
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);
    assert!(handler.focused_widget_id().is_some());

    handler.cursor_position = Some(Point {
        x: scroll_region.visible_frame.x + dp(24.0),
        y: (scroll_region.visible_frame.bottom() - dp(24.0)).max(input_frame.bottom() + dp(24.0)),
    });

    assert!(handler.handle_mouse_wheel(MouseScrollDelta::LineDelta(0.0, -2.0)));

    let focused_scroll = handler
        .computed_scene()
        .scroll_regions
        .iter()
        .find(|region| region.id == scroller_id)
        .copied()
        .expect("parent scroll region should still exist while focused")
        .scroll_offset
        .y;
    assert!(
        focused_scroll > Dp::ZERO || handler.smooth_scroll_states.contains_key(&scroller_id),
        "focused page scroll should update the rendered scene immediately"
    );
}

#[test]
fn textarea_click_after_prefocus_scroll_keeps_scrolled_viewport() {
    let invalidation = InvalidationSignal::new();
    let value = (0..8)
        .map(|index| format!("line {index}"))
        .collect::<Vec<_>>()
        .join("\n");
    let tree = WidgetTree::new(Textarea::<TestVm>::new(value).height(dp(52.0)));
    let mut handler = test_handler(Some(tree), invalidation);
    let viewport = handler.viewport_rect();
    let (text_id, frame) = {
        let computed = handler.computed_scene();
        computed
            .hit_regions
            .iter()
            .find_map(|region| match &region.interaction {
                HitInteraction::TextInput {
                    id,
                    multiline: true,
                    ..
                } => Some((*id, region.rect)),
                _ => None,
            })
            .expect("textarea hit region should exist")
    };

    handler.cursor_position = Some(Point {
        x: frame.x + dp(8.0),
        y: frame.y + dp(8.0),
    });

    assert!(handler.handle_mouse_wheel(MouseScrollDelta::LineDelta(0.0, -2.0)));
    while handler.advance_smooth_scroll() {}

    let scrolled_y = handler
        .scroll_states
        .get(&text_id)
        .map(|offset| offset.y)
        .unwrap_or(Dp::ZERO);
    assert!(scrolled_y > Dp::ZERO);

    let line_zero_visible_before_focus = handler
        .computed_scene()
        .scene
        .texts
        .iter()
        .any(|primitive| primitive.content.contains("line 0"));
    assert!(
        !line_zero_visible_before_focus,
        "prefocus wheel scrolling should move the first line out of view"
    );

    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);

    let state = handler
        .text_edit_states
        .get(&text_id)
        .expect("textarea edit state should exist after click");
    assert!(
        (state.scroll_y - scrolled_y).abs() <= 0.01,
        "clicking after prefocus scroll should preserve the existing vertical offset"
    );

    let line_zero_visible_after_focus = handler
        .computed_scene()
        .scene
        .texts
        .iter()
        .any(|primitive| primitive.content.contains("line 0"));
    assert!(
        !line_zero_visible_after_focus,
        "focusing the textarea should not jump the viewport back to the top"
    );
}

#[test]
fn textarea_backspace_keeps_scrolled_viewport_and_scroll_range() {
    let invalidation = InvalidationSignal::new();
    let value = (0..24)
        .map(|index| format!("line {index}"))
        .collect::<Vec<_>>()
        .join("\n");
    let tree = WidgetTree::new(Textarea::<TestVm>::new(value).height(dp(52.0)));
    let mut handler = test_handler(Some(tree), invalidation);
    let viewport = handler.viewport_rect();
    let (text_id, frame) = {
        let computed = handler.computed_scene();
        computed
            .hit_regions
            .iter()
            .find_map(|region| match &region.interaction {
                HitInteraction::TextInput {
                    id,
                    multiline: true,
                    ..
                } => Some((*id, region.rect)),
                _ => None,
            })
            .expect("textarea hit region should exist")
    };

    handler.cursor_position = Some(Point {
        x: frame.x + dp(24.0),
        y: frame.y + dp(8.0),
    });

    assert!(handler.handle_mouse_wheel(MouseScrollDelta::LineDelta(0.0, -8.0)));
    while handler.advance_smooth_scroll() {}

    let scrolled_before_focus = handler
        .scroll_states
        .get(&text_id)
        .map(|offset| offset.y)
        .unwrap_or(Dp::ZERO);
    assert!(scrolled_before_focus > Dp::ZERO);

    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);
    assert!(
        handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Backspace,)))
    );

    let state = handler
        .text_edit_states
        .get(&text_id)
        .expect("textarea edit state should exist after backspace");
    assert!(
        state.scroll_y > Dp::ZERO,
        "editing while focused should keep the vertical scroll offset"
    );

    let scroll_region = handler
        .computed_scene()
        .scroll_regions
        .iter()
        .find(|region| region.id == text_id)
        .copied()
        .expect("textarea scroll region should exist");
    assert!(
        scroll_region.max_offset().y > Dp::ZERO,
        "focused textarea should keep a vertical scroll range after backspace"
    );

    let line_zero_visible_after_backspace = handler
        .computed_scene()
        .scene
        .texts
        .iter()
        .any(|primitive| primitive.content.contains("line 0"));
    assert!(
        !line_zero_visible_after_backspace,
        "focused textarea should not jump back to the first line after backspace"
    );
}

#[test]
fn textarea_without_auto_wrap_keeps_edited_caret_in_view() {
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
    let (text_id, frame, padding) = {
        let computed = handler.computed_scene();
        computed
            .hit_regions
            .iter()
            .find_map(|region| match &region.interaction {
                HitInteraction::TextInput {
                    id,
                    frame,
                    padding,
                    multiline: true,
                    auto_wrap: false,
                    ..
                } => Some((*id, *frame, *padding)),
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
    assert!(
        handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Backspace,)))
    );

    let state = handler
        .text_edit_states
        .get(&text_id)
        .expect("textarea edit state should exist after backspace");
    assert!(state.scroll_x > Dp::ZERO);
    assert!(state.scroll_y > Dp::ZERO);

    let inner = frame.inset(padding);
    let caret = handler
        .computed_scene()
        .ime_cursor_area
        .expect("focused textarea should expose a caret rect after edit");
    assert!(caret.x >= inner.x);
    assert!(caret.right() <= inner.right() + dp(1.0));
    assert!(caret.y >= inner.y);
    assert!(caret.bottom() <= inner.bottom() + dp(1.0));

    let scroll_region = handler
        .computed_scene()
        .scroll_regions
        .iter()
        .find(|region| region.id == text_id)
        .copied()
        .expect("textarea scroll region should exist");
    assert!(scroll_region.max_offset().x > Dp::ZERO);
    assert!(scroll_region.max_offset().y > Dp::ZERO);
}
