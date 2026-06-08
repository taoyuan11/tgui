use super::*;

#[test]
fn textarea_mouse_wheel_reaches_last_line_with_tall_line_height() {
    let invalidation = InvalidationSignal::new();
    let value = "line 0\nline 1\nline 2\nline 3\nline 4\nline 5";
    let tree = WidgetTree::new(Textarea::<TestVm>::new(value).height(dp(120.0)).style_full(
        |ctx| {
            let mut style = crate::ui::widget::TextareaStyle::default_for_theme(ctx.theme);
            style.text_style.line_height = Some(crate::ui::unit::sp(40.0));
            style
        },
    ));
    let mut handler = test_handler(Some(tree), invalidation);
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

    while handler.handle_mouse_wheel(MouseScrollDelta::LineDelta(0.0, -3.0)) {}

    let computed = handler.computed_scene();
    let text_primitive = computed
        .scene
        .texts
        .iter()
        .find(|primitive| primitive.content.contains("line 5"))
        .expect("textarea text primitive should render");
    let inner_bottom = frame.bottom() - dp(8.0);

    assert!(text_primitive.frame.bottom() <= inner_bottom + dp(1.0));
}

#[test]
fn textarea_arrow_up_reduces_vertical_scroll_in_long_text() {
    let invalidation = InvalidationSignal::new();
    let value = "line 0\nline 1\nline 2\nline 3\nline 4\nline 5\nline 6";
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

    for _ in 0..6 {
        handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::ArrowDown)));
    }

    let text_id = handler
        .focused_widget_id()
        .expect("textarea should be focused after click");
    let scrolled_down = handler
        .scroll_states
        .get(&text_id)
        .map(|offset| offset.y)
        .unwrap_or(Dp::ZERO);
    assert!(scrolled_down > Dp::ZERO);

    for _ in 0..6 {
        handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::ArrowUp)));
    }

    let state = handler
        .text_edit_states
        .get(&text_id)
        .expect("textarea edit state should exist");
    let scrolled_up = handler
        .scroll_states
        .get(&text_id)
        .map(|offset| offset.y)
        .unwrap_or(Dp::ZERO);

    assert!(state.cursor < "line 0\n".len());
    assert!(scrolled_up < scrolled_down);
}
