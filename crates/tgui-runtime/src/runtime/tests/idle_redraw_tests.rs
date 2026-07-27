use super::*;

use raw_window_handle::{
    DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle, WindowHandle,
};

#[derive(Debug)]
struct RedrawProbeWindow {
    redraw_calls: AtomicUsize,
    size: PhysicalSize<u32>,
}

impl RedrawProbeWindow {
    fn new(size: PhysicalSize<u32>) -> Arc<Self> {
        Arc::new(Self {
            redraw_calls: AtomicUsize::new(0),
            size,
        })
    }

    fn redraw_calls(&self) -> usize {
        self.redraw_calls.load(Ordering::Relaxed)
    }
}

impl HasDisplayHandle for RedrawProbeWindow {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
        Err(HandleError::Unavailable)
    }
}

impl HasWindowHandle for RedrawProbeWindow {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        Err(HandleError::Unavailable)
    }
}

impl Window for RedrawProbeWindow {
    fn id(&self) -> crate::platform::window::WindowId {
        crate::platform::window::WindowId::dummy()
    }

    fn scale_factor(&self) -> f64 {
        1.0
    }

    fn request_redraw(&self) {
        self.redraw_calls.fetch_add(1, Ordering::Relaxed);
    }

    fn pre_present_notify(&self) {}

    fn surface_size(&self) -> PhysicalSize<u32> {
        self.size
    }

    fn set_visible(&self, _visible: bool) {}

    fn set_title(&self, _title: &str) {}

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
        None
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

#[derive(Debug, Default)]
struct DeadlineProbeEventLoop {
    control_flow: Mutex<Option<ControlFlow>>,
}

impl ActiveEventLoop for DeadlineProbeEventLoop {
    fn create_proxy(&self) -> EventLoopProxy {
        panic!("not needed in idle redraw tests")
    }

    fn create_window(
        &self,
        _window_attributes: WindowAttributes,
    ) -> Result<Box<dyn Window>, RequestError> {
        panic!("not needed in idle redraw tests")
    }

    fn available_monitors(&self) -> Box<dyn Iterator<Item = MonitorHandle>> {
        Box::new(std::iter::empty())
    }

    fn primary_monitor(&self) -> Option<MonitorHandle> {
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
}

fn handler_with_probe_window() -> (BoundRuntimeHandler<TestVm>, Arc<RedrawProbeWindow>) {
    let invalidation = InvalidationSignal::new();
    let window = RedrawProbeWindow::new(PhysicalSize::new(200, 120));
    let mut handler = test_handler(None, invalidation);
    handler.window = Some(window.clone());
    (handler, window)
}

#[test]
fn idle_redraw_does_not_request_before_deadline_and_requests_once_when_due() {
    let (mut handler, window) = handler_with_probe_window();
    let start = Instant::now();
    let deadline = start + Duration::from_secs(1);
    handler.next_idle_redraw_deadline = Some(deadline);

    assert!(!handler.drive_idle_redraw_inner(deadline - Duration::from_nanos(1), true));
    assert_eq!(window.redraw_calls(), 0);

    assert!(handler.drive_idle_redraw_inner(deadline, true));
    assert_eq!(window.redraw_calls(), 1);
    assert_eq!(
        handler.next_idle_redraw_deadline,
        Some(start + Duration::from_secs(2))
    );

    // A second event-loop wake at the same instant must not duplicate the request.
    assert!(!handler.drive_idle_redraw_inner(deadline, true));
    assert_eq!(window.redraw_calls(), 1);
}

#[test]
fn idle_redraw_catches_up_from_absolute_deadline_phase() {
    let (mut handler, window) = handler_with_probe_window();
    let start = Instant::now();
    handler.next_idle_redraw_deadline = Some(start);
    let late = start + Duration::from_millis(3_250);

    assert!(handler.drive_idle_redraw_inner(late, true));
    assert_eq!(window.redraw_calls(), 1);
    assert_eq!(
        handler.next_idle_redraw_deadline,
        Some(start + Duration::from_secs(4))
    );

    assert!(handler.drive_idle_redraw_inner(start + Duration::from_secs(4), true));
    assert_eq!(window.redraw_calls(), 2);
    assert_eq!(
        handler.next_idle_redraw_deadline,
        Some(start + Duration::from_secs(5))
    );
}

#[test]
fn idle_redraw_does_not_change_invalidation_or_animation_epochs() {
    let invalidation = InvalidationSignal::new();
    let window = RedrawProbeWindow::new(PhysicalSize::new(200, 120));
    let mut handler = test_handler(None, invalidation.clone());
    handler.window = Some(window.clone());
    let now = Instant::now();
    handler.next_idle_redraw_deadline = Some(now);

    let revision = invalidation.revision();
    let root_rebuild_revision = invalidation.root_rebuild_revision();
    let animation_epoch = handler.animation_epoch;
    let layout_animation_epoch = handler.layout_animation_epoch;
    let accessibility_animation_epoch = handler.accessibility_animation_epoch;

    assert!(handler.drive_idle_redraw_inner(now, true));
    assert_eq!(window.redraw_calls(), 1);
    assert_eq!(invalidation.revision(), revision);
    assert_eq!(invalidation.root_rebuild_revision(), root_rebuild_revision);
    assert_eq!(handler.animation_epoch, animation_epoch);
    assert_eq!(handler.layout_animation_epoch, layout_animation_epoch);
    assert_eq!(
        handler.accessibility_animation_epoch,
        accessibility_animation_epoch
    );
}

#[test]
fn ineligible_idle_redraw_disarms_without_requesting() {
    let (mut handler, window) = handler_with_probe_window();
    let now = Instant::now();
    handler.next_idle_redraw_deadline = Some(now);

    assert!(!handler.drive_idle_redraw_inner(now, false));
    assert_eq!(window.redraw_calls(), 0);
    assert_eq!(handler.next_idle_redraw_deadline, None);
}

#[test]
fn idle_deadline_keeps_an_earlier_event_loop_wakeup() {
    let (mut handler, _window) = handler_with_probe_window();
    let event_loop = DeadlineProbeEventLoop::default();
    let now = Instant::now();
    let earlier = now + Duration::from_millis(10);
    let idle = now + Duration::from_secs(1);
    handler.next_idle_redraw_deadline = Some(idle);
    event_loop.set_control_flow(ControlFlow::WaitUntil(earlier));

    assert_eq!(handler.schedule_next_deadline(&event_loop, now), Some(idle));
    assert_eq!(event_loop.control_flow(), ControlFlow::WaitUntil(earlier));
}
