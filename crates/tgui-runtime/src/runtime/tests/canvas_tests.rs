use super::*;

#[test]
fn canvas_item_hover_dispatches_canvas_pointer_payload() {
    let invalidation = InvalidationSignal::new();
    let tree = WidgetTree::new(
        Canvas::new(CanvasRecorder::build(|canvas| {
            canvas
                .next_item_id(7_u64)
                .set_fill(Color::WHITE)
                .fill_rect(10.0, 10.0, 50.0, 30.0);
        }))
        .size(dp(100.0), dp(80.0))
        .on_item_mouse_move(ValueCommand::new(|vm: &mut CanvasEventVm, event| {
            vm.hover_events.push(event);
        })),
    );
    let mut handler = test_handler_with_vm(CanvasEventVm::default(), Some(tree), invalidation);
    handler.cursor_position = Some(Point::new(dp(25.0), dp(20.0)));

    handler.handle_hover(handler.viewport_rect());

    let view_model = handler
        .view_model
        .lock()
        .expect("view model lock should not be poisoned");
    assert_eq!(view_model.hover_events.len(), 1);
    assert_eq!(view_model.hover_events[0].item_id, 7_u64.into());
    assert_eq!(
        view_model.hover_events[0].canvas_position,
        Point::new(25.0, 20.0)
    );
    assert_eq!(
        view_model.hover_events[0].local_position,
        Point::new(15.0, 10.0)
    );
}

#[test]
fn canvas_item_hover_reports_text_hit_payload() {
    let invalidation = InvalidationSignal::new();
    let tree = WidgetTree::new(
        Canvas::new(CanvasRecorder::build(|canvas| {
            canvas
                .next_item_id(9_u64)
                .set_text_style(CanvasTextStyle {
                    color: Color::WHITE,
                    font_size: sp(16.0),
                    ..Default::default()
                })
                .draw_text(Rect::new(10.0, 10.0, 80.0, 24.0), "hello");
        }))
        .size(dp(120.0), dp(80.0))
        .on_item_mouse_move(ValueCommand::new(|vm: &mut CanvasEventVm, event| {
            vm.hover_events.push(event);
        })),
    );
    let mut handler = test_handler_with_vm(CanvasEventVm::default(), Some(tree), invalidation);
    handler.cursor_position = Some(Point::new(dp(20.0), dp(20.0)));

    handler.handle_hover(handler.viewport_rect());

    let view_model = handler
        .view_model
        .lock()
        .expect("view model lock should not be poisoned");
    let text_hit = view_model.hover_events[0]
        .text_hit
        .expect("text hit should exist");
    assert!(text_hit.utf8_start < "hello".len());
    assert!(text_hit.utf8_end > text_hit.utf8_start);
}

#[test]
fn canvas_text_hover_uses_actual_text_content_bounds() {
    let invalidation = InvalidationSignal::new();
    let tree = WidgetTree::new(
        Canvas::new(CanvasRecorder::build(|canvas| {
            canvas
                .next_item_id(1_u64)
                .set_fill(Color::hexa(0xDBEAFEFF))
                .fill_round_rect(10.0, 10.0, 220.0, 80.0, dp(16.0))
                .next_item_id(2_u64)
                .set_text_style(CanvasTextStyle {
                    color: Color::hexa(0x0F172AFF),
                    font_size: sp(16.0),
                    line_height: Some(sp(20.0)),
                    ..Default::default()
                })
                .set_paragraph_style(CanvasParagraphStyle {
                    wrap: CanvasTextWrap::Word,
                    vertical_align: CanvasTextVerticalAlign::Center,
                    ..Default::default()
                })
                .draw_text(Rect::new(30.0, 20.0, 180.0, 50.0), "Centered text block");
        }))
        .size(dp(260.0), dp(120.0))
        .on_item_mouse_move(ValueCommand::new(|vm: &mut CanvasEventVm, event| {
            vm.hover_events.push(event);
        })),
    );
    let mut handler = test_handler_with_vm(CanvasEventVm::default(), Some(tree), invalidation);

    handler.cursor_position = Some(Point::new(dp(40.0), dp(24.0)));
    handler.handle_hover(handler.viewport_rect());
    {
        let view_model = handler
            .view_model
            .lock()
            .expect("view model lock should not be poisoned");
        assert_eq!(view_model.hover_events.len(), 1);
        assert_eq!(view_model.hover_events[0].item_id, 1_u64.into());
    }

    handler
        .view_model
        .lock()
        .expect("view model lock should not be poisoned")
        .hover_events
        .clear();

    handler.cursor_position = Some(Point::new(dp(52.0), dp(45.0)));
    handler.handle_hover(handler.viewport_rect());

    let view_model = handler
        .view_model
        .lock()
        .expect("view model lock should not be poisoned");
    assert_eq!(view_model.hover_events.len(), 1);
    assert_eq!(view_model.hover_events[0].item_id, 2_u64.into());
    assert!(view_model.hover_events[0].text_hit.is_some());
}

#[test]
fn canvas_item_hover_only_dispatches_topmost_overlapping_item() {
    let invalidation = InvalidationSignal::new();
    let tree = WidgetTree::new(
        Canvas::new(CanvasRecorder::build(|canvas| {
            canvas
                .next_item_id(1_u64)
                .set_fill(Color::hexa(0x38BDF8FF))
                .fill_rect(10.0, 10.0, 80.0, 40.0)
                .next_item_id(2_u64)
                .set_fill(Color::hexa(0x0F172AFF))
                .fill_rect(30.0, 20.0, 80.0, 40.0);
        }))
        .size(dp(140.0), dp(90.0))
        .on_item_mouse_move(ValueCommand::new(|vm: &mut CanvasEventVm, event| {
            vm.hover_events.push(event);
        })),
    );
    let mut handler = test_handler_with_vm(CanvasEventVm::default(), Some(tree), invalidation);
    handler.cursor_position = Some(Point::new(dp(50.0), dp(30.0)));

    handler.handle_hover(handler.viewport_rect());

    let view_model = handler
        .view_model
        .lock()
        .expect("view model lock should not be poisoned");
    assert_eq!(view_model.hover_events.len(), 1);
    assert_eq!(view_model.hover_events[0].item_id, 2_u64.into());
}

#[test]
fn canvas_item_click_takes_priority_over_widget_click() {
    let invalidation = InvalidationSignal::new();
    let tree = WidgetTree::new(
        Canvas::new(CanvasRecorder::build(|canvas| {
            canvas
                .next_item_id(11_u64)
                .set_fill(Color::WHITE)
                .fill_rect(10.0, 10.0, 50.0, 30.0);
        }))
        .size(dp(100.0), dp(80.0))
        .on_click(Command::new(|vm: &mut CanvasEventVm| {
            vm.widget_clicks += 1;
        }))
        .on_item_click(ValueCommand::new(|vm: &mut CanvasEventVm, _event| {
            vm.clicks += 1;
        })),
    );
    let mut handler = test_handler_with_vm(CanvasEventVm::default(), Some(tree), invalidation);
    handler.cursor_position = Some(Point::new(dp(20.0), dp(20.0)));

    handler.handle_mouse_press(
        handler.viewport_rect(),
        Instant::now(),
        CanvasMouseButton::Left,
    );

    let view_model = handler
        .view_model
        .lock()
        .expect("view model lock should not be poisoned");
    assert_eq!(view_model.clicks, 1);
    assert_eq!(view_model.widget_clicks, 0);
}

#[test]
fn canvas_item_mouse_down_up_wheel_and_drag_dispatch() {
    let invalidation = InvalidationSignal::new();
    let tree = WidgetTree::new(
        Canvas::new(CanvasRecorder::build(|canvas| {
            canvas
                .next_item_id(12_u64)
                .set_fill(Color::WHITE)
                .fill_rect(10.0, 10.0, 50.0, 30.0);
        }))
        .size(dp(100.0), dp(80.0))
        .on_item_mouse_down(ValueCommand::new(|vm: &mut CanvasEventVm, _event| {
            vm.mouse_downs += 1;
        }))
        .on_item_mouse_up(ValueCommand::new(|vm: &mut CanvasEventVm, _event| {
            vm.mouse_ups += 1;
        }))
        .on_item_wheel(ValueCommand::new(|vm: &mut CanvasEventVm, _event| {
            vm.wheel_events += 1;
        }))
        .on_item_drag(ValueCommand::new(|vm: &mut CanvasEventVm, _event| {
            vm.drag_events += 1;
        }))
        .on_item_drag_end(ValueCommand::new(|vm: &mut CanvasEventVm, _event| {
            vm.drag_end_events += 1;
        })),
    );
    let mut handler = test_handler_with_vm(CanvasEventVm::default(), Some(tree), invalidation);
    let viewport = handler.viewport_rect();
    handler.cursor_position = Some(Point::new(dp(20.0), dp(20.0)));

    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);
    handler.cursor_position = Some(Point::new(dp(36.0), dp(28.0)));
    assert!(handler.handle_canvas_drag());
    assert!(handler.handle_mouse_wheel(MouseScrollDelta::LineDelta(0.0, -1.0)));
    handler.handle_canvas_mouse_release(CanvasMouseButton::Left);

    let view_model = handler
        .view_model
        .lock()
        .expect("view model lock should not be poisoned");
    assert_eq!(view_model.mouse_downs, 1);
    assert_eq!(view_model.mouse_ups, 1);
    assert_eq!(view_model.wheel_events, 1);
    assert_eq!(view_model.drag_events, 1);
    assert_eq!(view_model.drag_end_events, 1);
}

#[test]
fn canvas_drag_end_only_requires_pointer_movement() {
    let invalidation = InvalidationSignal::new();
    let tree = WidgetTree::new(
        Canvas::new(CanvasRecorder::build(|canvas| {
            canvas
                .next_item_id(13_u64)
                .set_fill(Color::WHITE)
                .fill_rect(10.0, 10.0, 50.0, 30.0);
        }))
        .size(dp(100.0), dp(80.0))
        .on_item_drag_end(ValueCommand::new(
            |vm: &mut CanvasEventVm, event: CanvasDragEvent| {
                vm.drag_sequence.push(("end", event.button));
            },
        )),
    );
    let mut handler = test_handler_with_vm(CanvasEventVm::default(), Some(tree), invalidation);
    let viewport = handler.viewport_rect();
    handler.cursor_position = Some(Point::new(dp(20.0), dp(20.0)));

    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);
    handler.handle_canvas_mouse_release(CanvasMouseButton::Left);
    assert!(handler.view_model.lock().unwrap().drag_sequence.is_empty());

    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);
    handler.cursor_position = Some(Point::new(dp(30.0), dp(25.0)));
    assert!(handler.handle_canvas_drag());
    handler.handle_canvas_mouse_release(CanvasMouseButton::Left);
    assert_eq!(
        handler.view_model.lock().unwrap().drag_sequence,
        [("end", CanvasMouseButton::Left)]
    );
}

#[test]
fn canvas_drag_start_only_fires_once_after_pointer_movement() {
    let invalidation = InvalidationSignal::new();
    let tree = WidgetTree::new(
        Canvas::new(CanvasRecorder::build(|canvas| {
            canvas
                .next_item_id(14_u64)
                .set_fill(Color::WHITE)
                .fill_rect(10.0, 10.0, 50.0, 30.0);
        }))
        .size(dp(100.0), dp(80.0))
        .on_item_drag_start(ValueCommand::new(
            |vm: &mut CanvasEventVm, event: CanvasDragEvent| {
                vm.drag_sequence.push(("start", event.button));
            },
        )),
    );
    let mut handler = test_handler_with_vm(CanvasEventVm::default(), Some(tree), invalidation);
    let viewport = handler.viewport_rect();
    handler.cursor_position = Some(Point::new(dp(20.0), dp(20.0)));

    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);
    handler.handle_canvas_mouse_release(CanvasMouseButton::Left);
    assert!(handler.view_model.lock().unwrap().drag_sequence.is_empty());

    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);
    handler.cursor_position = Some(Point::new(dp(30.0), dp(25.0)));
    assert!(handler.handle_canvas_drag());
    handler.cursor_position = Some(Point::new(dp(35.0), dp(28.0)));
    assert!(handler.handle_canvas_drag());
    handler.handle_canvas_mouse_release(CanvasMouseButton::Left);
    assert_eq!(
        handler.view_model.lock().unwrap().drag_sequence,
        [("start", CanvasMouseButton::Left)]
    );
}

#[test]
fn canvas_drag_sequence_preserves_initiating_button_until_matching_release() {
    let invalidation = InvalidationSignal::new();
    let tree = WidgetTree::new(
        Canvas::new(CanvasRecorder::build(|canvas| {
            canvas
                .next_item_id(15_u64)
                .set_fill(Color::WHITE)
                .fill_rect(10.0, 10.0, 50.0, 30.0);
        }))
        .size(dp(100.0), dp(80.0))
        .on_item_drag_start(ValueCommand::new(
            |vm: &mut CanvasEventVm, event: CanvasDragEvent| {
                vm.drag_sequence.push(("start", event.button));
            },
        ))
        .on_item_drag(ValueCommand::new(
            |vm: &mut CanvasEventVm, event: CanvasDragEvent| {
                vm.drag_sequence.push(("drag", event.button));
            },
        ))
        .on_item_drag_end(ValueCommand::new(
            |vm: &mut CanvasEventVm, event: CanvasDragEvent| {
                vm.drag_sequence.push(("end", event.button));
            },
        )),
    );
    let mut handler = test_handler_with_vm(CanvasEventVm::default(), Some(tree), invalidation);
    let viewport = handler.viewport_rect();
    handler.cursor_position = Some(Point::new(dp(20.0), dp(20.0)));

    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Right);
    handler.cursor_position = Some(Point::new(dp(30.0), dp(25.0)));
    assert!(handler.handle_canvas_drag());
    handler.handle_canvas_mouse_release(CanvasMouseButton::Left);
    assert!(handler.active_canvas_drag.is_some());
    handler.handle_canvas_mouse_release(CanvasMouseButton::Right);

    assert_eq!(
        handler.view_model.lock().unwrap().drag_sequence,
        [
            ("start", CanvasMouseButton::Right),
            ("drag", CanvasMouseButton::Right),
            ("end", CanvasMouseButton::Right),
        ]
    );
}

#[test]
fn canvas_recorder_items_preserve_item_interaction_dispatch() {
    let invalidation = InvalidationSignal::new();
    let items = CanvasRecorder::build(|canvas| {
        canvas
            .next_item_id(33_u64)
            .set_fill(Color::WHITE)
            .fill_rect(10.0, 10.0, 50.0, 30.0);
    });
    let tree = WidgetTree::new(Canvas::new(items).size(dp(100.0), dp(80.0)).on_item_click(
        ValueCommand::new(|vm: &mut CanvasEventVm, event| {
            vm.hover_events.push(event);
            vm.clicks += 1;
        }),
    ));
    let mut handler = test_handler_with_vm(CanvasEventVm::default(), Some(tree), invalidation);
    handler.cursor_position = Some(Point::new(dp(20.0), dp(20.0)));

    handler.handle_mouse_press(
        handler.viewport_rect(),
        Instant::now(),
        CanvasMouseButton::Left,
    );

    let view_model = handler
        .view_model
        .lock()
        .expect("view model lock should not be poisoned");
    assert_eq!(view_model.clicks, 1);
    assert_eq!(view_model.hover_events[0].item_id, 33_u64.into());
}

#[test]
fn dashed_canvas_item_hit_testing_skips_gaps() {
    let make_tree = || {
        WidgetTree::new(
            Canvas::new(CanvasRecorder::build(|canvas| {
                canvas
                    .next_item_id(21_u64)
                    .set_stroke(CanvasStroke::new(dp(6.0), Color::WHITE).dash([dp(10.0), dp(10.0)]))
                    .draw_line(10.0, 20.0, 90.0, 20.0);
            }))
            .size(dp(100.0), dp(60.0))
            .on_item_mouse_move(ValueCommand::new(
                |vm: &mut CanvasEventVm, event| {
                    vm.hover_events.push(event);
                },
            )),
        )
    };

    let mut hit_handler = test_handler_with_vm(
        CanvasEventVm::default(),
        Some(make_tree()),
        InvalidationSignal::new(),
    );
    hit_handler.cursor_position = Some(Point::new(dp(15.0), dp(20.0)));
    hit_handler.handle_hover(hit_handler.viewport_rect());
    let hit_vm = hit_handler
        .view_model
        .lock()
        .expect("view model lock should not be poisoned");
    assert_eq!(hit_vm.hover_events.len(), 1);
    drop(hit_vm);

    let mut gap_handler = test_handler_with_vm(
        CanvasEventVm::default(),
        Some(make_tree()),
        InvalidationSignal::new(),
    );
    gap_handler.cursor_position = Some(Point::new(dp(25.0), dp(20.0)));
    gap_handler.handle_hover(gap_handler.viewport_rect());
    let gap_vm = gap_handler
        .view_model
        .lock()
        .expect("view model lock should not be poisoned");
    assert!(gap_vm.hover_events.is_empty());
}

#[test]
fn canvas_shadow_does_not_extend_item_hit_region() {
    let invalidation = InvalidationSignal::new();
    let tree = WidgetTree::new(
        Canvas::new(CanvasRecorder::build(|canvas| {
            canvas
                .next_item_id(31_u64)
                .set_fill(Color::WHITE)
                .set_shadow(CanvasShadow::new(
                    Color::BLACK,
                    Point::new(18.0, 0.0),
                    dp(8.0),
                ))
                .fill_rect(10.0, 10.0, 30.0, 30.0);
        }))
        .size(dp(100.0), dp(80.0))
        .on_item_mouse_move(ValueCommand::new(|vm: &mut CanvasEventVm, event| {
            vm.hover_events.push(event);
        })),
    );
    let mut handler = test_handler_with_vm(CanvasEventVm::default(), Some(tree), invalidation);
    handler.cursor_position = Some(Point::new(dp(55.0), dp(25.0)));

    handler.handle_hover(handler.viewport_rect());

    let view_model = handler
        .view_model
        .lock()
        .expect("view model lock should not be poisoned");
    assert!(view_model.hover_events.is_empty());
}
