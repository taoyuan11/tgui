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
            scope_path: Vec::new(),
            on_blur: None,
        }),
        None,
        false,
    );

    let deadline = handler.next_deadline(Instant::now());
    assert!(deadline.is_none());
}
