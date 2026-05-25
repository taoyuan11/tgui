use super::*;
use crate::platform::event::MouseButton;
use crate::runtime::LONG_PRESS_THRESHOLD;
use crate::ui::widget::{
    DoubleTapEvent, EdgeSwipeEvent, GestureEdgeSet, GestureRecognizer, LongPressEvent,
    PinchGestureEvent, SwipeAxis, SwipeGestureEvent,
};

#[derive(Default)]
struct GestureVm {
    clicks: usize,
    long_presses: usize,
    double_taps: usize,
    pinch_phases: Vec<crate::ui::widget::GesturePhase>,
    swipe_phases: Vec<crate::ui::widget::GesturePhase>,
    edge_swipes: usize,
    edge_phases: Vec<crate::ui::widget::GesturePhase>,
}

impl ViewModel for GestureVm {
    fn new(_: &ViewModelContext) -> Self {
        Self::default()
    }

    fn view(&self) -> Element<Self> {
        Stack::new().into()
    }
}

impl GestureVm {
    fn click(&mut self) {
        self.clicks += 1;
    }

    fn long_press(&mut self, _: LongPressEvent) {
        self.long_presses += 1;
    }

    fn double_tap(&mut self, _: DoubleTapEvent) {
        self.double_taps += 1;
    }

    fn swipe(&mut self, event: SwipeGestureEvent) {
        self.swipe_phases.push(event.phase);
    }

    fn pinch(&mut self, event: PinchGestureEvent) {
        self.pinch_phases.push(event.phase);
    }

    fn edge_swipe(&mut self, _: EdgeSwipeEvent) {
        self.edge_swipes += 1;
    }

    fn edge_swipe_phase(&mut self, event: EdgeSwipeEvent) {
        self.edge_swipes += 1;
        self.edge_phases.push(event.phase);
    }
}

#[test]
fn touch_long_press_triggers_without_click() {
    let invalidation = InvalidationSignal::new();
    let button_base = Button::<GestureVm>::new("hold")
        .height(dp(80.0))
        .on_click(Command::new(GestureVm::click));
    let button: Element<GestureVm> = Element::from(button_base)
        .gesture(GestureRecognizer::new().on_long_press(ValueCommand::new(GestureVm::long_press)));
    let scroller: Element<GestureVm> = Stack::new()
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
        GestureVm::default(),
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
    let _ = handler.drive_animations(
        &event_loop,
        Instant::now() + LONG_PRESS_THRESHOLD + Duration::from_millis(10),
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

    let vm = handler.view_model.lock().unwrap();
    assert_eq!(vm.long_presses, 1);
    assert_eq!(vm.clicks, 0);
}

#[test]
fn mouse_double_click_triggers_double_tap_gesture() {
    let invalidation = InvalidationSignal::new();
    let button_base = Button::<GestureVm>::new("double")
        .height(dp(80.0))
        .on_click(Command::new(GestureVm::click));
    let button: Element<GestureVm> = Element::from(button_base)
        .gesture(GestureRecognizer::new().on_double_tap(ValueCommand::new(GestureVm::double_tap)));
    let mut handler = test_handler_with_vm(
        GestureVm::default(),
        Some(WidgetTree::new(button)),
        invalidation,
    );
    let event_loop = TestEventLoop;

    handler.handle_bound_window_event(
        &event_loop,
        WindowEvent::PointerButton {
            device_id: None,
            position: PhysicalPosition::new(24.0, 40.0),
            state: ElementState::Pressed,
            button: ButtonSource::Mouse(MouseButton::Left),
            primary: true,
        },
    );
    handler.handle_bound_window_event(
        &event_loop,
        WindowEvent::PointerButton {
            device_id: None,
            position: PhysicalPosition::new(24.0, 40.0),
            state: ElementState::Released,
            button: ButtonSource::Mouse(MouseButton::Left),
            primary: true,
        },
    );
    handler.handle_bound_window_event(
        &event_loop,
        WindowEvent::PointerButton {
            device_id: None,
            position: PhysicalPosition::new(24.0, 40.0),
            state: ElementState::Pressed,
            button: ButtonSource::Mouse(MouseButton::Left),
            primary: true,
        },
    );
    handler.handle_bound_window_event(
        &event_loop,
        WindowEvent::PointerButton {
            device_id: None,
            position: PhysicalPosition::new(24.0, 40.0),
            state: ElementState::Released,
            button: ButtonSource::Mouse(MouseButton::Left),
            primary: true,
        },
    );

    let vm = handler.view_model.lock().unwrap();
    assert_eq!(vm.double_taps, 1);
}

#[test]
fn horizontal_swipe_inside_vertical_scrollview_emits_swipe_phases() {
    let invalidation = InvalidationSignal::new();
    let content_base = Button::<GestureVm>::new("swipe").height(dp(80.0));
    let content: Element<GestureVm> = Element::from(content_base).gesture(
        GestureRecognizer::new()
            .on_swipe(SwipeAxis::Horizontal, ValueCommand::new(GestureVm::swipe)),
    );
    let scroller: Element<GestureVm> = Stack::new()
        .size(dp(320.0), dp(240.0))
        .overflow_y(Overflow::Scroll)
        .child(
            Flex::vertical()
                .height(dp(860.0))
                .gap(dp(12.0))
                .child([content, Stack::new().height(dp(760.0)).into()]),
        )
        .into();
    let mut handler = test_handler_with_vm(
        GestureVm::default(),
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
            position: PhysicalPosition::new(72.0, 42.0),
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
            position: PhysicalPosition::new(96.0, 42.0),
            state: ElementState::Released,
            button: ButtonSource::Touch {
                finger_id: FingerId::from_raw(1),
                force: None,
            },
            primary: true,
        },
    );

    let vm = handler.view_model.lock().unwrap();
    assert!(vm
        .swipe_phases
        .contains(&crate::ui::widget::GesturePhase::Start));
    assert!(vm
        .swipe_phases
        .contains(&crate::ui::widget::GesturePhase::End));
}

#[test]
fn pinch_gesture_emits_start_update_end_and_suppresses_click() {
    let invalidation = InvalidationSignal::new();
    let content_base = Button::<GestureVm>::new("pinch")
        .size(dp(200.0), dp(120.0))
        .on_click(Command::new(GestureVm::click));
    let content: Element<GestureVm> = Element::from(content_base)
        .gesture(GestureRecognizer::new().on_pinch(ValueCommand::new(GestureVm::pinch)));
    let mut handler = test_handler_with_vm(
        GestureVm::default(),
        Some(WidgetTree::new(content)),
        invalidation,
    );
    let event_loop = TestEventLoop;

    handler.handle_bound_window_event(
        &event_loop,
        WindowEvent::PointerButton {
            device_id: None,
            position: PhysicalPosition::new(40.0, 40.0),
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
            position: PhysicalPosition::new(80.0, 40.0),
            state: ElementState::Pressed,
            button: ButtonSource::Touch {
                finger_id: FingerId::from_raw(2),
                force: None,
            },
            primary: true,
        },
    );
    handler.handle_bound_window_event(
        &event_loop,
        WindowEvent::PointerMoved {
            device_id: None,
            position: PhysicalPosition::new(24.0, 40.0),
            primary: true,
            source: PointerSource::Touch {
                finger_id: FingerId::from_raw(1),
                force: None,
            },
        },
    );
    handler.handle_bound_window_event(
        &event_loop,
        WindowEvent::PointerMoved {
            device_id: None,
            position: PhysicalPosition::new(96.0, 40.0),
            primary: true,
            source: PointerSource::Touch {
                finger_id: FingerId::from_raw(2),
                force: None,
            },
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

    assert!(handler.active_pinch.is_none());
    let vm = handler.view_model.lock().unwrap();
    assert_eq!(vm.clicks, 0);
    assert!(vm
        .pinch_phases
        .contains(&crate::ui::widget::GesturePhase::Start));
    assert!(vm
        .pinch_phases
        .contains(&crate::ui::widget::GesturePhase::Update));
    assert!(vm
        .pinch_phases
        .contains(&crate::ui::widget::GesturePhase::End));
}

#[test]
fn pinch_gesture_cancels_when_pointer_leaves() {
    let invalidation = InvalidationSignal::new();
    let content_base = Stack::<GestureVm>::new().size(dp(200.0), dp(120.0));
    let content: Element<GestureVm> = Element::from(content_base)
        .gesture(GestureRecognizer::new().on_pinch(ValueCommand::new(GestureVm::pinch)));
    let mut handler = test_handler_with_vm(
        GestureVm::default(),
        Some(WidgetTree::new(content)),
        invalidation,
    );
    let event_loop = TestEventLoop;

    handler.handle_bound_window_event(
        &event_loop,
        WindowEvent::PointerButton {
            device_id: None,
            position: PhysicalPosition::new(40.0, 40.0),
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
            position: PhysicalPosition::new(80.0, 40.0),
            state: ElementState::Pressed,
            button: ButtonSource::Touch {
                finger_id: FingerId::from_raw(2),
                force: None,
            },
            primary: true,
        },
    );
    handler.handle_bound_window_event(
        &event_loop,
        WindowEvent::PointerMoved {
            device_id: None,
            position: PhysicalPosition::new(24.0, 40.0),
            primary: true,
            source: PointerSource::Touch {
                finger_id: FingerId::from_raw(1),
                force: None,
            },
        },
    );
    handler.handle_bound_window_event(
        &event_loop,
        WindowEvent::PointerLeft {
            device_id: None,
            position: None,
            primary: true,
            kind: PointerKind::Touch(FingerId::from_raw(1)),
        },
    );

    assert!(handler.active_pinch.is_none());
    let vm = handler.view_model.lock().unwrap();
    assert!(vm
        .pinch_phases
        .contains(&crate::ui::widget::GesturePhase::Start));
    assert!(vm
        .pinch_phases
        .contains(&crate::ui::widget::GesturePhase::Cancel));
}

#[test]
fn edge_swipe_only_triggers_from_edge_band() {
    let invalidation = InvalidationSignal::new();
    let content_base = Stack::<GestureVm>::new().size(dp(320.0), dp(240.0));
    let content: Element<GestureVm> =
        Element::from(content_base).gesture(GestureRecognizer::new().on_edge_swipe(
            GestureEdgeSet::horizontal(),
            ValueCommand::new(GestureVm::edge_swipe),
        ));
    let mut handler = test_handler_with_vm(
        GestureVm::default(),
        Some(WidgetTree::new(content)),
        invalidation,
    );
    let event_loop = TestEventLoop;

    handler.handle_bound_window_event(
        &event_loop,
        WindowEvent::PointerButton {
            device_id: None,
            position: PhysicalPosition::new(4.0, 40.0),
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
            position: PhysicalPosition::new(40.0, 40.0),
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
            position: PhysicalPosition::new(40.0, 40.0),
            state: ElementState::Released,
            button: ButtonSource::Touch {
                finger_id: FingerId::from_raw(1),
                force: None,
            },
            primary: true,
        },
    );

    assert!(handler.view_model.lock().unwrap().edge_swipes >= 1);
}

#[test]
fn single_click_still_fires_after_double_tap_timeout() {
    let invalidation = InvalidationSignal::new();
    let button_base = Button::<GestureVm>::new("single")
        .height(dp(80.0))
        .on_click(Command::new(GestureVm::click));
    let button: Element<GestureVm> = Element::from(button_base)
        .gesture(GestureRecognizer::new().on_double_tap(ValueCommand::new(GestureVm::double_tap)));
    let mut handler = test_handler_with_vm(
        GestureVm::default(),
        Some(WidgetTree::new(button)),
        invalidation,
    );
    let event_loop = TestEventLoop;

    handler.handle_bound_window_event(
        &event_loop,
        WindowEvent::PointerButton {
            device_id: None,
            position: PhysicalPosition::new(24.0, 40.0),
            state: ElementState::Pressed,
            button: ButtonSource::Mouse(MouseButton::Left),
            primary: true,
        },
    );
    handler.handle_bound_window_event(
        &event_loop,
        WindowEvent::PointerButton {
            device_id: None,
            position: PhysicalPosition::new(24.0, 40.0),
            state: ElementState::Released,
            button: ButtonSource::Mouse(MouseButton::Left),
            primary: true,
        },
    );
    let _ = handler.drive_animations(
        &event_loop,
        Instant::now() + crate::runtime::DOUBLE_CLICK_THRESHOLD + Duration::from_millis(10),
    );

    let vm = handler.view_model.lock().unwrap();
    assert_eq!(vm.double_taps, 0);
    assert_eq!(vm.clicks, 1);
}

#[test]
fn mouse_long_press_mappings_work() {
    let invalidation = InvalidationSignal::new();
    let button_base = Button::<GestureVm>::new("hold")
        .height(dp(80.0))
        .on_click(Command::new(GestureVm::click));
    let button: Element<GestureVm> = Element::from(button_base)
        .gesture(GestureRecognizer::new().on_long_press(ValueCommand::new(GestureVm::long_press)));
    let mut right_click_handler = test_handler_with_vm(
        GestureVm::default(),
        Some(WidgetTree::new(button.clone())),
        invalidation.clone(),
    );
    let event_loop = TestEventLoop;

    right_click_handler.handle_bound_window_event(
        &event_loop,
        WindowEvent::PointerButton {
            device_id: None,
            position: PhysicalPosition::new(24.0, 40.0),
            state: ElementState::Pressed,
            button: ButtonSource::Mouse(MouseButton::Right),
            primary: false,
        },
    );

    {
        let vm = right_click_handler.view_model.lock().unwrap();
        assert_eq!(vm.long_presses, 1);
        assert_eq!(vm.clicks, 0);
    }

    let mut left_hold_handler = test_handler_with_vm(
        GestureVm::default(),
        Some(WidgetTree::new(button)),
        invalidation,
    );
    left_hold_handler.handle_bound_window_event(
        &event_loop,
        WindowEvent::PointerButton {
            device_id: None,
            position: PhysicalPosition::new(24.0, 40.0),
            state: ElementState::Pressed,
            button: ButtonSource::Mouse(MouseButton::Left),
            primary: true,
        },
    );
    let _ = left_hold_handler.drive_animations(
        &event_loop,
        Instant::now() + LONG_PRESS_THRESHOLD + Duration::from_millis(10),
    );
    left_hold_handler.handle_bound_window_event(
        &event_loop,
        WindowEvent::PointerButton {
            device_id: None,
            position: PhysicalPosition::new(24.0, 40.0),
            state: ElementState::Released,
            button: ButtonSource::Mouse(MouseButton::Left),
            primary: true,
        },
    );

    let vm = left_hold_handler.view_model.lock().unwrap();
    assert_eq!(vm.long_presses, 1);
    assert_eq!(vm.clicks, 0);
}

#[test]
fn edge_swipe_cancels_when_pointer_leaves() {
    let invalidation = InvalidationSignal::new();
    let content_base = Stack::<GestureVm>::new().size(dp(320.0), dp(240.0));
    let content: Element<GestureVm> =
        Element::from(content_base).gesture(GestureRecognizer::new().on_edge_swipe(
            GestureEdgeSet::horizontal(),
            ValueCommand::new(GestureVm::edge_swipe_phase),
        ));
    let mut handler = test_handler_with_vm(
        GestureVm::default(),
        Some(WidgetTree::new(content)),
        invalidation,
    );
    let event_loop = TestEventLoop;

    handler.handle_bound_window_event(
        &event_loop,
        WindowEvent::PointerButton {
            device_id: None,
            position: PhysicalPosition::new(4.0, 40.0),
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
            position: PhysicalPosition::new(40.0, 40.0),
            primary: true,
            source: PointerSource::Touch {
                finger_id: FingerId::from_raw(1),
                force: None,
            },
        },
    );
    handler.handle_bound_window_event(
        &event_loop,
        WindowEvent::PointerLeft {
            device_id: None,
            position: None,
            primary: true,
            kind: PointerKind::Touch(FingerId::from_raw(1)),
        },
    );

    assert!(handler.active_gesture.is_none());
    let vm = handler.view_model.lock().unwrap();
    assert!(vm
        .edge_phases
        .contains(&crate::ui::widget::GesturePhase::Start));
    assert!(vm
        .edge_phases
        .contains(&crate::ui::widget::GesturePhase::Cancel));
}

#[test]
fn active_gesture_cancels_when_widget_is_removed() {
    let invalidation = InvalidationSignal::new();
    let content: Element<GestureVm> = Element::from(
        Stack::<GestureVm>::new().size(dp(320.0), dp(240.0)),
    )
    .gesture(GestureRecognizer::new().on_edge_swipe(
        GestureEdgeSet::horizontal(),
        ValueCommand::new(GestureVm::edge_swipe_phase),
    ));
    let mut handler = test_handler_with_vm(
        GestureVm::default(),
        Some(WidgetTree::new(content)),
        invalidation,
    );
    let event_loop = TestEventLoop;

    handler.handle_bound_window_event(
        &event_loop,
        WindowEvent::PointerButton {
            device_id: None,
            position: PhysicalPosition::new(4.0, 40.0),
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
            position: PhysicalPosition::new(40.0, 40.0),
            primary: true,
            source: PointerSource::Touch {
                finger_id: FingerId::from_raw(1),
                force: None,
            },
        },
    );

    let widget_id = handler
        .active_gesture
        .as_ref()
        .map(|session| session.widget_id)
        .expect("gesture session should exist");
    handler.prune_removed_widget_state(&std::collections::HashSet::from([widget_id]));

    assert!(handler.active_gesture.is_none());
    let vm = handler.view_model.lock().unwrap();
    assert!(vm
        .edge_phases
        .contains(&crate::ui::widget::GesturePhase::Start));
    assert!(vm
        .edge_phases
        .contains(&crate::ui::widget::GesturePhase::Cancel));
}

#[test]
fn vertical_swipe_does_not_steal_vertical_scroll_regions() {
    let invalidation = InvalidationSignal::new();
    let content_base = Button::<GestureVm>::new("swipe").height(dp(80.0));
    let content: Element<GestureVm> = Element::from(content_base).gesture(
        GestureRecognizer::new().on_swipe(SwipeAxis::Vertical, ValueCommand::new(GestureVm::swipe)),
    );
    let scroller: Element<GestureVm> = Stack::new()
        .size(dp(320.0), dp(240.0))
        .overflow_y(Overflow::Scroll)
        .child(
            Flex::vertical()
                .height(dp(860.0))
                .gap(dp(12.0))
                .child([content, Stack::new().height(dp(760.0)).into()]),
        )
        .into();
    let scroller_id = scroller.id;
    let mut handler = test_handler_with_vm(
        GestureVm::default(),
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
            position: PhysicalPosition::new(24.0, 8.0),
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
            position: PhysicalPosition::new(24.0, 8.0),
            state: ElementState::Released,
            button: ButtonSource::Touch {
                finger_id: FingerId::from_raw(1),
                force: None,
            },
            primary: true,
        },
    );

    let vm = handler.view_model.lock().unwrap();
    assert!(vm.swipe_phases.is_empty());
    assert!(handler
        .scroll_states
        .get(&scroller_id)
        .map(|offset| offset.y > Dp::ZERO)
        .unwrap_or(false));
}
