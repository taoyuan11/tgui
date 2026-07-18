use super::*;
use raw_window_handle::{
    DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle, WindowHandle,
};

struct BindingProbeWindow {
    title_calls: AtomicUsize,
    title: Mutex<String>,
    theme_calls: AtomicUsize,
    theme: Mutex<Option<crate::platform::window::Theme>>,
}

impl BindingProbeWindow {
    fn new(theme: Option<crate::platform::window::Theme>) -> Self {
        Self {
            title_calls: AtomicUsize::new(0),
            title: Mutex::new(String::new()),
            theme_calls: AtomicUsize::new(0),
            theme: Mutex::new(theme),
        }
    }
}

impl HasDisplayHandle for BindingProbeWindow {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
        Err(HandleError::Unavailable)
    }
}

impl HasWindowHandle for BindingProbeWindow {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        Err(HandleError::Unavailable)
    }
}

impl Window for BindingProbeWindow {
    fn id(&self) -> crate::platform::window::WindowId {
        crate::platform::window::WindowId::dummy()
    }

    fn scale_factor(&self) -> f64 {
        1.0
    }

    fn request_redraw(&self) {}

    fn pre_present_notify(&self) {}

    fn surface_size(&self) -> PhysicalSize<u32> {
        PhysicalSize::new(200, 120)
    }

    fn set_visible(&self, _visible: bool) {}

    fn set_title(&self, title: &str) {
        self.title_calls.fetch_add(1, Ordering::Relaxed);
        *self.title.lock().expect("title lock poisoned") = title.to_owned();
    }

    fn is_decorated(&self) -> bool {
        true
    }

    fn set_decorations(&self, _decorations: bool) {}

    fn has_focus(&self) -> bool {
        true
    }

    fn is_maximized(&self) -> bool {
        false
    }

    fn set_maximized(&self, _maximized: bool) {}

    fn set_minimized(&self, _minimized: bool) {}

    fn drag_window(&self) -> Result<(), winit::error::ExternalError> {
        Ok(())
    }

    fn drag_resize_window(
        &self,
        _direction: winit::window::ResizeDirection,
    ) -> Result<(), winit::error::ExternalError> {
        Ok(())
    }

    fn set_cursor(&self, _cursor: winit::window::Cursor) {}

    fn theme(&self) -> Option<crate::platform::window::Theme> {
        self.theme_calls.fetch_add(1, Ordering::Relaxed);
        *self.theme.lock().expect("theme lock poisoned")
    }

    fn request_ime_update(
        &self,
        _request: crate::platform::window::ImeRequest,
    ) -> Result<(), winit::error::ExternalError> {
        Ok(())
    }

    #[cfg(target_os = "windows")]
    fn set_enable(&self, _enabled: bool) {}
}

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

#[test]
fn stable_title_binding_does_not_repeat_platform_set_title() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let title = context.state("Inbox".to_string());
    let unrelated = context.state(0usize);
    let window = Arc::new(BindingProbeWindow::new(None));
    let mut handler = test_handler(None, invalidation);
    handler.window_bindings.title = Some(title.signal());
    handler.window = Some(window.clone());

    handler.sync_bindings(Instant::now());
    for _ in 0..1024 {
        handler.sync_bindings(Instant::now());
    }
    unrelated.set(1);
    handler.sync_bindings(Instant::now());

    assert_eq!(window.title_calls.load(Ordering::Relaxed), 1);
    assert_eq!(&*window.title.lock().expect("title lock poisoned"), "Inbox");

    title.set("Archive".to_string());
    handler.sync_bindings(Instant::now());
    assert_eq!(window.title_calls.load(Ordering::Relaxed), 2);
    assert_eq!(
        &*window.title.lock().expect("title lock poisoned"),
        "Archive"
    );

    let replacement = Arc::new(BindingProbeWindow::new(None));
    handler.window = Some(replacement.clone());
    handler.sync_bindings(Instant::now());
    assert_eq!(replacement.title_calls.load(Ordering::Relaxed), 1);
    assert_eq!(
        &*replacement.title.lock().expect("title lock poisoned"),
        "Archive"
    );
}

#[test]
fn opaque_title_binding_uses_global_revision_but_skips_equal_platform_value() {
    let invalidation = InvalidationSignal::new();
    let value = Arc::new(Mutex::new("Stable".to_string()));
    let reads = Arc::new(AtomicUsize::new(0));
    let signal = Signal::new(
        {
            let value = value.clone();
            let reads = reads.clone();
            move || {
                reads.fetch_add(1, Ordering::Relaxed);
                value.lock().expect("value lock poisoned").clone()
            }
        },
        invalidation.clone(),
    );
    let window = Arc::new(BindingProbeWindow::new(None));
    let mut handler = test_handler(None, invalidation.clone());
    handler.window_bindings.title = Some(signal);
    handler.window = Some(window.clone());

    handler.sync_bindings(Instant::now());
    invalidation.mark_dirty();
    handler.sync_bindings(Instant::now());
    assert_eq!(reads.load(Ordering::Relaxed), 2);
    assert_eq!(window.title_calls.load(Ordering::Relaxed), 1);

    *value.lock().expect("value lock poisoned") = "Changed".to_string();
    invalidation.mark_dirty();
    handler.sync_bindings(Instant::now());
    assert_eq!(reads.load(Ordering::Relaxed), 3);
    assert_eq!(window.title_calls.load(Ordering::Relaxed), 2);
}

#[test]
fn stable_theme_binding_skips_resolve_and_platform_theme_polling() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let mode = context.state(ThemeMode::Light);
    let unrelated = context.state(0usize);
    let (theme_set, light, dark) = custom_theme_set();
    let window = Arc::new(BindingProbeWindow::new(Some(
        crate::platform::window::Theme::Dark,
    )));
    let mut handler = test_handler_with_config(
        TestVm,
        None,
        invalidation,
        test_config_with_theme(ThemeSelection::System, theme_set),
    );
    handler.window_bindings.theme_mode = Some(mode.signal());
    handler.window = Some(window.clone());

    handler.sync_theme_binding();
    let baseline_resolves = handler.binding_sync.theme_resolves;
    for _ in 0..1024 {
        assert!(!handler.refresh_platform_theme());
    }
    unrelated.set(1);
    handler.sync_theme_binding();

    assert_eq!(handler.theme, light);
    assert_eq!(handler.binding_sync.theme_resolves, baseline_resolves);
    assert_eq!(window.theme_calls.load(Ordering::Relaxed), 1);

    mode.set(ThemeMode::Dark);
    assert!(handler.sync_theme_binding());
    assert_eq!(handler.theme, dark);
    assert_eq!(handler.binding_sync.theme_resolves, baseline_resolves + 1);
}

#[test]
fn system_theme_events_are_window_local_and_do_not_poll() {
    let invalidation = InvalidationSignal::new();
    let (theme_set, light, dark) = custom_theme_set();
    let dark_window = Arc::new(BindingProbeWindow::new(Some(
        crate::platform::window::Theme::Dark,
    )));
    let light_window = Arc::new(BindingProbeWindow::new(Some(
        crate::platform::window::Theme::Light,
    )));
    let config = test_config_with_theme(ThemeSelection::System, theme_set);
    let mut dark_handler =
        test_handler_with_config(TestVm, None, invalidation.clone(), config.clone());
    let mut light_handler = test_handler_with_config(TestVm, None, invalidation, config);
    dark_handler.window = Some(dark_window.clone());
    light_handler.window = Some(light_window.clone());

    dark_handler.sync_theme_binding();
    light_handler.sync_theme_binding();
    assert_eq!(dark_handler.theme, dark);
    assert_eq!(light_handler.theme, light);
    assert_eq!(dark_window.theme_calls.load(Ordering::Relaxed), 1);
    assert_eq!(light_window.theme_calls.load(Ordering::Relaxed), 1);

    dark_handler.apply_window_theme(Some(crate::platform::window::Theme::Light));
    assert_eq!(dark_handler.theme, light);
    assert_eq!(dark_window.theme_calls.load(Ordering::Relaxed), 1);
    for _ in 0..1024 {
        assert!(!dark_handler.refresh_platform_theme());
        assert!(!light_handler.refresh_platform_theme());
    }
    assert_eq!(dark_window.theme_calls.load(Ordering::Relaxed), 1);
    assert_eq!(light_window.theme_calls.load(Ordering::Relaxed), 1);
}

#[derive(Debug, Default)]
struct CapturingEventLoop {
    control_flow: Mutex<Option<ControlFlow>>,
    set_control_flow_calls: AtomicUsize,
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

    fn available_monitors(&self) -> Box<dyn Iterator<Item = MonitorHandle>> {
        Box::new(std::iter::empty())
    }

    fn primary_monitor(&self) -> Option<MonitorHandle> {
        None
    }

    fn set_control_flow(&self, control_flow: ControlFlow) {
        self.set_control_flow_calls.fetch_add(1, Ordering::Relaxed);
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
}

#[test]
fn stable_wait_control_flow_is_not_reapplied() {
    let handler = test_handler_with_config(
        TestVm,
        None,
        InvalidationSignal::new(),
        test_config_with_theme(ThemeSelection::Mode(ThemeMode::Light), ThemeSet::default()),
    );
    let event_loop = CapturingEventLoop::default();
    let now = Instant::now();

    for _ in 0..1024 {
        handler.set_control_flow_for_deadline(&event_loop, None, now);
    }

    assert_eq!(event_loop.set_control_flow_calls.load(Ordering::Relaxed), 0);
    assert!(matches!(event_loop.control_flow(), ControlFlow::Wait));
}

#[test]
fn idle_window_does_not_erase_another_windows_future_deadline() {
    let handler = test_handler_with_config(
        TestVm,
        None,
        InvalidationSignal::new(),
        test_config_with_theme(ThemeSelection::Mode(ThemeMode::Light), ThemeSet::default()),
    );
    let event_loop = CapturingEventLoop::default();
    let now = Instant::now();
    let sibling_deadline = now + Duration::from_millis(8);
    event_loop.set_control_flow(ControlFlow::WaitUntil(sibling_deadline));

    handler.set_control_flow_for_deadline(&event_loop, None, now);

    assert_eq!(
        event_loop.control_flow(),
        ControlFlow::WaitUntil(sibling_deadline)
    );
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

#[test]
fn schedule_next_deadline_preserves_earlier_future_wakeup() {
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
    let computed_deadline = handler
        .next_deadline(now)
        .expect("active theme animation should arm the frame clock");
    let earlier_deadline = now + computed_deadline.saturating_duration_since(now) / 2;
    event_loop.set_control_flow(ControlFlow::WaitUntil(earlier_deadline));

    let scheduled_deadline = handler
        .schedule_next_deadline(&event_loop, now)
        .expect("active theme animation should schedule another frame");

    assert_eq!(scheduled_deadline, computed_deadline);
    assert!(scheduled_deadline > earlier_deadline);
    match event_loop.control_flow() {
        ControlFlow::WaitUntil(scheduled) => assert_eq!(scheduled, earlier_deadline),
        other => panic!("expected WaitUntil control flow, got {other:?}"),
    }
}

#[test]
fn schedule_next_deadline_preserves_elapsed_earlier_wakeup_after_redraw() {
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
    let earlier_deadline = now - Duration::from_millis(1);
    event_loop.set_control_flow(ControlFlow::WaitUntil(earlier_deadline));

    let _computed_deadline = handler
        .schedule_next_deadline(&event_loop, now)
        .expect("active theme animation should schedule another frame");

    match event_loop.control_flow() {
        ControlFlow::WaitUntil(scheduled) => assert_eq!(scheduled, earlier_deadline),
        other => panic!("expected WaitUntil control flow, got {other:?}"),
    }
}

#[test]
fn schedule_next_deadline_preserves_animation_frame_wakeup_after_redraw() {
    let invalidation = InvalidationSignal::new();
    let mut handler = test_handler_with_config(
        TestVm,
        Some(WidgetTree::new(
            Flex::new(Axis::Vertical)
                .child(ProgressBar::indeterminate(true))
                .child(Spinner::new()),
        )),
        invalidation,
        test_config_with_theme(ThemeSelection::Mode(ThemeMode::Light), ThemeSet::default()),
    );

    let _ = handler.computed_scene();

    let event_loop = CapturingEventLoop::default();
    let now = Instant::now();
    let pending_frame_deadline = now + Duration::from_millis(2);
    event_loop.set_control_flow(ControlFlow::WaitUntil(pending_frame_deadline));

    let computed_deadline = handler
        .schedule_next_deadline(&event_loop, now)
        .expect("feedback loading controls should schedule repeat animation frames");

    assert!(computed_deadline > pending_frame_deadline);
    match event_loop.control_flow() {
        ControlFlow::WaitUntil(scheduled) => assert_eq!(scheduled, pending_frame_deadline),
        other => panic!("expected WaitUntil control flow, got {other:?}"),
    }
}

#[test]
fn drive_animations_waits_for_scheduled_frame_clock_deadline() {
    let invalidation = InvalidationSignal::new();
    let mut handler = test_handler_with_config(
        TestVm,
        Some(WidgetTree::new(
            Flex::new(Axis::Vertical)
                .child(ProgressBar::indeterminate(true))
                .child(Spinner::new()),
        )),
        invalidation,
        test_config_with_theme(ThemeSelection::Mode(ThemeMode::Light), ThemeSet::default()),
    );

    let _ = handler.computed_scene();

    let event_loop = CapturingEventLoop::default();
    let now = Instant::now();
    let scheduled_deadline = handler
        .schedule_next_deadline(&event_loop, now)
        .expect("active animation should arm the per-window frame clock");
    let early = now + scheduled_deadline.saturating_duration_since(now) / 2;

    let before_epoch = handler.animation_epoch;
    assert!(!handler.drive_animations(&event_loop, early));
    assert_eq!(handler.animation_epoch, before_epoch);
    match event_loop.control_flow() {
        ControlFlow::WaitUntil(scheduled) => assert_eq!(scheduled, scheduled_deadline),
        other => panic!("expected WaitUntil control flow, got {other:?}"),
    }

    assert!(handler.drive_animations(&event_loop, scheduled_deadline + Duration::from_millis(1)));
    assert!(handler.animation_epoch > before_epoch);
}

#[test]
fn unrelated_earlier_wakeup_does_not_oversample_animation_clock() {
    let invalidation = InvalidationSignal::new();
    let mut handler = test_handler_with_config(
        TestVm,
        Some(WidgetTree::new(
            Flex::new(Axis::Vertical)
                .child(ProgressBar::indeterminate(true))
                .child(Spinner::new()),
        )),
        invalidation,
        test_config_with_theme(ThemeSelection::Mode(ThemeMode::Light), ThemeSet::default()),
    );
    let _ = handler.computed_scene();

    let event_loop = CapturingEventLoop::default();
    let now = Instant::now();
    handler.frame_clock = crate::animation::AdaptiveFrameClock::new(now);
    let frame_deadline = handler
        .next_deadline(now)
        .expect("feedback animation should arm a frame deadline");
    let unrelated_deadline = now + Duration::from_millis(4);
    assert!(unrelated_deadline < frame_deadline);
    event_loop.set_control_flow(ControlFlow::WaitUntil(unrelated_deadline));

    let before_epoch = handler.animation_epoch;
    let unrelated_wake = unrelated_deadline + Duration::from_millis(1);
    assert!(!handler.drive_animations(&event_loop, unrelated_wake));
    assert_eq!(handler.animation_epoch, before_epoch);
    match event_loop.control_flow() {
        ControlFlow::WaitUntil(scheduled) => assert_eq!(scheduled, frame_deadline),
        other => panic!("expected frame WaitUntil after unrelated wake, got {other:?}"),
    }
}

#[test]
fn set_control_flow_for_deadline_replaces_elapsed_wakeup_during_tick() {
    let invalidation = InvalidationSignal::new();
    let handler = test_handler_with_config(
        TestVm,
        None,
        invalidation,
        test_config_with_theme(ThemeSelection::Mode(ThemeMode::Light), ThemeSet::default()),
    );

    let event_loop = CapturingEventLoop::default();
    let now = Instant::now();
    let stale_deadline = now - Duration::from_millis(1);
    let next_deadline = now + Duration::from_millis(16);
    event_loop.set_control_flow(ControlFlow::WaitUntil(stale_deadline));

    handler.set_control_flow_for_deadline(&event_loop, Some(next_deadline), now);

    match event_loop.control_flow() {
        ControlFlow::WaitUntil(scheduled) => assert_eq!(scheduled, next_deadline),
        other => panic!("expected WaitUntil control flow, got {other:?}"),
    }
}
