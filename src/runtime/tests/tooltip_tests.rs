use super::*;

use std::time::Duration;

#[test]
fn tooltip_emits_overlay_primitives_when_hovered() {
    let invalidation = InvalidationSignal::new();
    let tree = WidgetTree::new(
        Button::new("Save")
            .size(dp(120.0), dp(40.0))
            .tooltip(Tooltip::new("点击保存").delay(Duration::ZERO)),
    );
    let mut handler = test_handler(Some(tree), invalidation);
    handler.cursor_position = Some(Point::new(dp(40.0), dp(20.0)));

    let viewport = handler.viewport_rect();
    assert!(handler.handle_hover(viewport));

    let computed = handler.computed_scene();
    // Tooltip 至少推一个背景 shape 与一段文字到 overlay 层
    assert!(
        !computed.scene.overlay_shapes.is_empty(),
        "expected tooltip background in overlay layer"
    );
    assert!(
        !computed.scene.overlay_texts.is_empty(),
        "expected tooltip text in overlay layer"
    );
}

#[test]
fn tooltip_absent_without_hover() {
    let invalidation = InvalidationSignal::new();
    let tree = WidgetTree::new(
        Button::new("Save")
            .size(dp(120.0), dp(40.0))
            .tooltip(Tooltip::new("hint").delay(Duration::ZERO)),
    );
    let mut handler = test_handler(Some(tree), invalidation);
    // cursor far away from button
    handler.cursor_position = Some(Point::new(dp(500.0), dp(500.0)));

    let viewport = handler.viewport_rect();
    let _ = handler.handle_hover(viewport);

    let computed = handler.computed_scene();
    assert!(
        computed.scene.overlay_shapes.is_empty(),
        "no tooltip should render when widget is not hovered"
    );
    assert!(computed.scene.overlay_texts.is_empty());
}

#[test]
fn tooltip_disappears_when_hover_leaves() {
    let invalidation = InvalidationSignal::new();
    let tree = WidgetTree::new(
        Button::new("Save")
            .size(dp(120.0), dp(40.0))
            .tooltip(Tooltip::new("hint").delay(Duration::ZERO)),
    );
    let mut handler = test_handler(Some(tree), invalidation);

    // 先 hover 进入触发显示
    handler.cursor_position = Some(Point::new(dp(40.0), dp(20.0)));
    let viewport = handler.viewport_rect();
    assert!(handler.handle_hover(viewport));
    let shown = handler.computed_scene();
    assert!(!shown.scene.overlay_texts.is_empty());

    // 再 hover 离开
    handler.cursor_position = Some(Point::new(dp(500.0), dp(500.0)));
    let _ = handler.handle_hover(viewport);
    let hidden = handler.computed_scene();
    assert!(
        hidden.scene.overlay_texts.is_empty(),
        "tooltip should disappear after hover leaves"
    );
}

#[test]
fn tooltip_widget_without_descriptor_emits_nothing() {
    let invalidation = InvalidationSignal::new();
    let tree = WidgetTree::new(Button::new("Save").size(dp(120.0), dp(40.0)));
    let mut handler = test_handler(Some(tree), invalidation);
    handler.cursor_position = Some(Point::new(dp(40.0), dp(20.0)));

    let viewport = handler.viewport_rect();
    let _ = handler.handle_hover(viewport);

    let computed = handler.computed_scene();
    assert!(computed.scene.overlay_shapes.is_empty());
    assert!(computed.scene.overlay_texts.is_empty());
}

#[test]
fn tooltip_does_not_register_close_handler() {
    // Tooltip 不挂 close handler——靠 hover 离开自然消失，不应进入 overlay_close_handlers。
    let invalidation = InvalidationSignal::new();
    let tree = WidgetTree::new(
        Button::new("Save")
            .size(dp(120.0), dp(40.0))
            .tooltip(Tooltip::new("hint").delay(Duration::ZERO)),
    );
    let mut handler = test_handler(Some(tree), invalidation);
    handler.cursor_position = Some(Point::new(dp(40.0), dp(20.0)));
    let viewport = handler.viewport_rect();
    let _ = handler.handle_hover(viewport);
    let computed = handler.computed_scene();
    assert!(computed.overlay_close_handlers.is_empty());
}

#[test]
fn tooltip_with_default_delay_does_not_emit_immediately() {
    // 默认 500ms hover 延迟：hover 立即触发但不应该立刻渲染 tooltip。
    let invalidation = InvalidationSignal::new();
    let tree = WidgetTree::new(
        Button::new("Save")
            .size(dp(120.0), dp(40.0))
            .tooltip(Tooltip::new("hint")), // 默认 500ms delay
    );
    let mut handler = test_handler(Some(tree), invalidation);
    handler.cursor_position = Some(Point::new(dp(40.0), dp(20.0)));
    let viewport = handler.viewport_rect();
    let _ = handler.handle_hover(viewport);
    let computed = handler.computed_scene();
    assert!(
        computed.scene.overlay_texts.is_empty(),
        "tooltip must wait for delay before showing"
    );
    // hover 已记录，但未触发渲染。
}

#[test]
fn tooltip_emits_after_delay_elapses() {
    // 用一个短延迟，确保等待后能触发渲染。
    let invalidation = InvalidationSignal::new();
    let tree = WidgetTree::new(
        Button::new("Save")
            .size(dp(120.0), dp(40.0))
            .tooltip(Tooltip::new("hint").delay(Duration::from_millis(20))),
    );
    let mut handler = test_handler(Some(tree), invalidation);
    handler.cursor_position = Some(Point::new(dp(40.0), dp(20.0)));
    let viewport = handler.viewport_rect();
    let _ = handler.handle_hover(viewport);
    // 未到 delay：不渲染。
    assert!(handler.computed_scene().scene.overlay_texts.is_empty());

    std::thread::sleep(Duration::from_millis(45));
    // 触发 scene 重 collect。
    handler.invalidate_computed_scene();
    let computed = handler.computed_scene();
    assert!(
        !computed.scene.overlay_texts.is_empty(),
        "tooltip should appear once hover duration exceeds delay"
    );
}

#[test]
fn tooltip_pending_delay_schedules_next_wakeup() {
    // hover 持续未达 delay 时，runtime 应记录下一次 wakeup 时刻。
    let invalidation = InvalidationSignal::new();
    let tree = WidgetTree::new(
        Button::new("Save")
            .size(dp(120.0), dp(40.0))
            .tooltip(Tooltip::new("hint").delay(Duration::from_millis(200))),
    );
    let mut handler = test_handler(Some(tree), invalidation);
    handler.cursor_position = Some(Point::new(dp(40.0), dp(20.0)));
    let viewport = handler.viewport_rect();
    let _ = handler.handle_hover(viewport);
    let _ = handler.computed_scene();
    assert!(
        handler.next_tooltip_wakeup_deadline.is_some(),
        "expected runtime to schedule tooltip wakeup deadline"
    );
}
