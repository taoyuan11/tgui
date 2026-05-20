use super::*;

#[test]
fn textarea_mouse_wheel_scrolls_vertical_overflow() {
    let invalidation = InvalidationSignal::new();
    let value = "line 0\nline 1\nline 2\nline 3\nline 4\nline 5";
    let tree = WidgetTree::new(Textarea::<TestVm>::new(value).height(dp(52.0)));
    let mut handler = test_handler(Some(tree), invalidation);
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
    assert!(
        handler
            .scroll_states
            .get(&text_id)
            .map(|offset| offset.y)
            .unwrap_or(Dp::ZERO)
            > Dp::ZERO
            || handler.smooth_scroll_states.contains_key(&text_id)
    );
}

#[test]
fn mouse_wheel_starts_immediately_and_keeps_smooth_target() {
    let invalidation = InvalidationSignal::new();
    let scroller: Element<TestVm> = Stack::new()
        .size(dp(100.0), dp(100.0))
        .overflow_y(Overflow::Scroll)
        .child(Stack::new().size(dp(100.0), dp(320.0)))
        .into();
    let scroller_id = scroller.id;
    let tree = WidgetTree::new(scroller);
    let mut handler = test_handler(Some(tree), invalidation);
    let region = handler
        .computed_scene()
        .scroll_regions
        .iter()
        .find(|region| region.id == scroller_id)
        .copied()
        .expect("scroll region should exist");

    handler.cursor_position = Some(Point {
        x: region.visible_frame.x + dp(8.0),
        y: region.visible_frame.y + dp(8.0),
    });

    assert!(handler.handle_mouse_wheel(MouseScrollDelta::LineDelta(0.0, -2.0)));
    assert!(handler.handle_mouse_wheel(MouseScrollDelta::LineDelta(0.0, -2.0)));

    let offset = handler
        .scroll_states
        .get(&scroller_id)
        .map(|state| state.y)
        .expect("scroll offset should exist");
    assert!(offset > Dp::ZERO);
    assert!(offset < dp(160.0));

    let target = handler
        .smooth_scroll_states
        .get(&scroller_id)
        .map(|state| state.target.y)
        .expect("smooth scroll target should exist");
    assert_eq!(target, dp(160.0));
}

#[test]
fn mouse_wheel_scrolls_stack_wrapped_grid_of_canvas_cards() {
    let invalidation = InvalidationSignal::new();
    let card = || {
        Stack::new().height(dp(180.0)).child(
            Canvas::new(CanvasRecorder::build(|canvas| {
                canvas
                    .next_item_id(1_u64)
                    .set_fill(Color::hexa(0x1D4ED8FF))
                    .fill_rect(0.0, 0.0, 80.0, 80.0);
            }))
            .size(dp(120.0), dp(120.0)),
        )
    };
    let scroller: Element<TestVm> = Stack::new()
        .size(dp(320.0), dp(240.0))
        .overflow_y(Overflow::Scroll)
        .child(
            crate::ui::widget::Grid::columns([
                crate::ui::layout::fr(1.0),
                crate::ui::layout::fr(1.0),
            ])
            .height(dp(780.0))
            .gap(dp(12.0))
            .child(card())
            .child(card())
            .child(card())
            .child(card()),
        )
        .into();
    let scroller_id = scroller.id;
    let tree = WidgetTree::new(scroller);
    let mut handler = test_handler(Some(tree), invalidation);
    let region = handler
        .computed_scene()
        .scroll_regions
        .iter()
        .find(|region| region.id == scroller_id)
        .copied()
        .expect("scroll region should exist");

    handler.cursor_position = Some(Point {
        x: region.visible_frame.x + dp(24.0),
        y: region.visible_frame.y + dp(24.0),
    });

    assert!(handler.handle_mouse_wheel(MouseScrollDelta::LineDelta(0.0, -2.0)));
    assert!(
        handler
            .scroll_states
            .get(&scroller_id)
            .map(|offset| offset.y)
            .unwrap_or(Dp::ZERO)
            > Dp::ZERO
            || handler.smooth_scroll_states.contains_key(&scroller_id)
    );
}

#[test]
fn pointer_entered_restores_mouse_wheel_scrolling_after_pointer_left() {
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
    let event_loop = TestEventLoop;

    let target = {
        let computed = handler.computed_scene();
        let scroll_region = computed
            .scroll_regions
            .iter()
            .find(|region| region.id == scroller_id)
            .copied()
            .expect("parent scroll region should exist");
        assert!(scroll_region.max_offset().y > Dp::ZERO);
        PhysicalPosition::new(
            f64::from((scroll_region.visible_frame.x + dp(24.0)).get()),
            f64::from((scroll_region.visible_frame.bottom() - dp(24.0)).get()),
        )
    };

    let input_frame = {
        let computed = handler.computed_scene();
        computed
            .hit_regions
            .iter()
            .find_map(|region| match &region.interaction {
                HitInteraction::TextInput {
                    multiline: false, ..
                } => Some(region.rect),
                _ => None,
            })
            .expect("input hit region should exist")
    };

    handler.cursor_position = Some(Point {
        x: input_frame.x + dp(8.0),
        y: input_frame.y + dp(8.0),
    });
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);
    assert!(handler.focused_widget_id().is_some());

    handler.handle_bound_window_event(
        &event_loop,
        WindowEvent::PointerLeft {
            device_id: None,
            position: None,
            primary: true,
            kind: PointerKind::Mouse,
        },
    );
    assert!(handler.cursor_position.is_none());

    handler.handle_bound_window_event(
        &event_loop,
        WindowEvent::PointerEntered {
            device_id: None,
            position: target,
            primary: true,
            kind: PointerKind::Mouse,
        },
    );
    assert!(handler.cursor_position.is_some());

    handler.handle_bound_window_event(
        &event_loop,
        WindowEvent::MouseWheel {
            device_id: None,
            delta: MouseScrollDelta::LineDelta(0.0, -2.0),
            phase: TouchPhase::Moved,
        },
    );
    assert!(
        handler
            .scroll_states
            .get(&scroller_id)
            .map(|offset| offset.y > Dp::ZERO)
            .unwrap_or(false)
            || handler.smooth_scroll_states.contains_key(&scroller_id),
        "parent scroller should respond after pointer re-enters the window"
    );
}

#[derive(Default)]
struct TouchScrollVm {
    clicks: usize,
}

impl ViewModel for TouchScrollVm {
    fn new(_: &ViewModelContext) -> Self {
        Self::default()
    }

    fn view(&self) -> Element<Self> {
        Stack::new().into()
    }
}

impl TouchScrollVm {
    fn click(&mut self) {
        self.clicks += 1;
    }
}

#[test]
fn touch_drag_scrolls_clickable_content_without_firing_tap() {
    let invalidation = InvalidationSignal::new();
    let button: Element<TouchScrollVm> = Button::new("drag me")
        .height(dp(80.0))
        .on_click(Command::new(TouchScrollVm::click))
        .into();
    let scroller: Element<TouchScrollVm> = Stack::new()
        .size(dp(320.0), dp(240.0))
        .overflow_y(Overflow::Scroll)
        .child(
            Flex::vertical()
                .height(dp(860.0))
                .gap(dp(12.0))
                .child([button, Stack::new().height(dp(760.0)).into()]),
        )
        .into();
    let scroller_id = scroller.id;
    let mut handler = test_handler_with_vm(
        TouchScrollVm::default(),
        Some(WidgetTree::new(scroller)),
        invalidation,
    );
    let event_loop = TestEventLoop;

    handler.handle_bound_window_event(
        &event_loop,
        WindowEvent::PointerButton {
            device_id: None,
            position: PhysicalPosition::new(24.0, 40.0),
            state: ElementState::Pressed,
            button: ButtonSource::Touch {
                finger_id: FingerId::from_raw(1),
                force: None,
            },
            primary: true,
        },
    );
    handler.handle_bound_window_event(
        &event_loop,
        WindowEvent::PointerMoved {
            device_id: None,
            position: PhysicalPosition::new(24.0, 16.0),
            primary: true,
            source: PointerSource::Touch {
                finger_id: FingerId::from_raw(1),
                force: None,
            },
        },
    );
    handler.handle_bound_window_event(
        &event_loop,
        WindowEvent::PointerButton {
            device_id: None,
            position: PhysicalPosition::new(24.0, 16.0),
            state: ElementState::Released,
            button: ButtonSource::Touch {
                finger_id: FingerId::from_raw(1),
                force: None,
            },
            primary: true,
        },
    );

    assert!(
        handler
            .scroll_states
            .get(&scroller_id)
            .map(|offset| offset.y > Dp::ZERO)
            .unwrap_or(false),
        "touch drag should scroll the parent scroller"
    );
    assert_eq!(handler.view_model.lock().unwrap().clicks, 0);
}

#[test]
fn touch_tap_on_clickable_content_still_fires_click() {
    let invalidation = InvalidationSignal::new();
    let button: Element<TouchScrollVm> = Button::new("tap me")
        .height(dp(80.0))
        .on_click(Command::new(TouchScrollVm::click))
        .into();
    let scroller: Element<TouchScrollVm> = Stack::new()
        .size(dp(320.0), dp(240.0))
        .overflow_y(Overflow::Scroll)
        .child(
            Flex::vertical()
                .height(dp(860.0))
                .gap(dp(12.0))
                .child([button, Stack::new().height(dp(760.0)).into()]),
        )
        .into();
    let mut handler = test_handler_with_vm(
        TouchScrollVm::default(),
        Some(WidgetTree::new(scroller)),
        invalidation,
    );
    let event_loop = TestEventLoop;

    handler.handle_bound_window_event(
        &event_loop,
        WindowEvent::PointerButton {
            device_id: None,
            position: PhysicalPosition::new(24.0, 40.0),
            state: ElementState::Pressed,
            button: ButtonSource::Touch {
                finger_id: FingerId::from_raw(1),
                force: None,
            },
            primary: true,
        },
    );
    handler.handle_bound_window_event(
        &event_loop,
        WindowEvent::PointerButton {
            device_id: None,
            position: PhysicalPosition::new(24.0, 40.0),
            state: ElementState::Released,
            button: ButtonSource::Touch {
                finger_id: FingerId::from_raw(1),
                force: None,
            },
            primary: true,
        },
    );

    assert_eq!(handler.view_model.lock().unwrap().clicks, 1);
}
