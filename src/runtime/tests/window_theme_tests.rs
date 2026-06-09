use super::*;

#[test]
fn centered_window_position_uses_monitor_center() {
    let position = centered_window_position_for_monitor(
        Some(PhysicalPosition::new(-1920, 0)),
        PhysicalSize::new(1920, 1080),
        1.0,
        LogicalSize::new(960.0, 540.0),
    );

    assert_eq!(position, Some(PhysicalPosition::new(-1440, 270)));
}

#[test]
fn centered_window_position_clamps_to_monitor_origin_for_oversized_window() {
    let position = centered_window_position_for_monitor(
        Some(PhysicalPosition::new(100, 200)),
        PhysicalSize::new(800, 600),
        1.0,
        LogicalSize::new(1200.0, 700.0),
    );

    assert_eq!(position, Some(PhysicalPosition::new(100, 200)));
}

#[test]
fn window_control_close_request_marks_handler_for_close() {
    let invalidation = InvalidationSignal::new();
    let mut handler = test_handler(None, invalidation);
    let context = handler.command_context();

    context.window().close();

    assert!(handler.drain_window_requests());
    assert!(!handler.drain_window_requests());
}

#[test]
fn bound_theme_modes_resolve_through_configured_theme_set() {
    let invalidation = InvalidationSignal::new();
    let (theme_set, light, dark) = custom_theme_set();
    let mode = Signal::new(|| ThemeMode::Light, invalidation.clone());
    let mut handler = test_handler_with_config(
        TestVm,
        None,
        invalidation.clone(),
        test_config_with_theme(ThemeSelection::System, theme_set),
    );
    handler.window_bindings.theme_mode = Some(mode);

    handler.sync_theme_binding();
    assert_eq!(handler.theme, light);

    handler.window_bindings.theme_mode = Some(Signal::new(|| ThemeMode::Dark, invalidation));
    handler.sync_theme_binding();
    assert_eq!(handler.theme, dark);
}

#[test]
fn bound_theme_set_updates_current_theme_without_mode_change() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let (theme_set, light, _dark) = custom_theme_set();
    let themes = context.state(theme_set);
    let theme_binding = themes.signal();
    let mut handler = test_handler_with_config(
        TestVm,
        None,
        invalidation.clone(),
        test_config_with_theme(ThemeSelection::System, ThemeSet::default()),
    );
    handler.window_bindings.theme_mode =
        Some(Signal::new(|| ThemeMode::Light, invalidation.clone()));
    handler.window_bindings.theme_set = Some(theme_binding);

    handler.sync_theme_binding();
    assert_eq!(handler.theme, light);

    let mut updated_light = Theme::light();
    updated_light.colors.background = Color::hexa(0xFFFFFFFF);
    updated_light.colors.primary = Color::hexa(0xFFAA00FF);
    themes.update(|themes| {
        themes.light = Arc::new(updated_light.clone());
    });

    handler.sync_theme_binding();
    assert_eq!(handler.theme, updated_light);
}

#[derive(Debug, Default)]
struct CapturingEventLoop {
    control_flow: Mutex<Option<ControlFlow>>,
}

impl ActiveEventLoop for CapturingEventLoop {
    fn create_proxy(&self) -> EventLoopProxy {
        panic!("not needed in runtime tests")
    }

    fn create_window(
        &self,
        _window_attributes: WindowAttributes,
    ) -> Result<Box<dyn Window>, RequestError> {
        panic!("not needed in runtime tests")
    }

    fn create_custom_cursor(
        &self,
        _custom_cursor: CustomCursorSource,
    ) -> Result<CustomCursor, RequestError> {
        panic!("not needed in runtime tests")
    }

    fn available_monitors(&self) -> Box<dyn Iterator<Item = MonitorHandle>> {
        Box::new(std::iter::empty())
    }

    fn primary_monitor(&self) -> Option<MonitorHandle> {
        None
    }

    fn listen_device_events(&self, _allowed: DeviceEvents) {}

    fn system_theme(&self) -> Option<WindowTheme> {
        None
    }

    fn set_control_flow(&self, control_flow: ControlFlow) {
        *self
            .control_flow
            .lock()
            .expect("control flow lock poisoned") = Some(control_flow);
    }

    fn control_flow(&self) -> ControlFlow {
        self.control_flow
            .lock()
            .expect("control flow lock poisoned")
            .unwrap_or(ControlFlow::Wait)
    }

    fn exit(&self) {}

    fn exiting(&self) -> bool {
        false
    }

    fn owned_display_handle(&self) -> OwnedDisplayHandle {
        OwnedDisplayHandle::new(Arc::new(TestDisplayHandle))
    }

    fn rwh_06_handle(&self) -> &dyn HasDisplayHandle {
        static DISPLAY: TestDisplayHandle = TestDisplayHandle;
        &DISPLAY
    }
}

#[test]
fn schedule_next_deadline_tracks_theme_animations_created_during_scene_collection() {
    let invalidation = InvalidationSignal::new();
    let mut handler = test_handler_with_config(
        TestVm,
        Some(WidgetTree::new(Text::new("theme"))),
        invalidation,
        test_config_with_theme(ThemeSelection::Mode(ThemeMode::Light), ThemeSet::default()),
    );

    let _ = handler.computed_scene();
    handler.apply_theme(Theme::dark());
    let _ = handler.computed_scene();

    let event_loop = CapturingEventLoop::default();
    let now = Instant::now();
    let deadline = handler
        .schedule_next_deadline(&event_loop, now)
        .expect("active theme animation should schedule another frame");

    assert!(deadline > now);
    match event_loop.control_flow() {
        ControlFlow::WaitUntil(scheduled) => assert_eq!(scheduled, deadline),
        other => panic!("expected WaitUntil control flow, got {other:?}"),
    }
}
