pub(crate) use winit::error::EventLoopError;
pub(crate) use winit::event_loop::ControlFlow;
use winit::platform::run_on_demand::EventLoopExtRunOnDemand;

use raw_window_handle::{HasDisplayHandle, HasWindowHandle};

pub mod application {
    pub use winit::application::*;
}

pub mod event_loop_ext {
    pub use winit::event_loop::ControlFlow;
    pub type EventLoopProxy = super::EventLoopProxy;

    pub trait ActiveEventLoop {
        fn create_window(
            &self,
            attributes: winit::window::WindowAttributes,
        ) -> Result<Box<dyn crate::platform::backend::window::Window>, winit::error::ExternalError>;
        fn create_proxy(&self) -> EventLoopProxy;
        fn set_control_flow(&self, control_flow: ControlFlow);
        fn control_flow(&self) -> ControlFlow;
        fn exit(&self);
        fn primary_monitor(&self) -> Option<winit::monitor::MonitorHandle>;
        fn available_monitors(
            &self,
        ) -> Box<dyn Iterator<Item = winit::monitor::MonitorHandle> + '_>;
    }

    impl ActiveEventLoop for winit::event_loop::ActiveEventLoop {
        fn create_window(
            &self,
            attributes: winit::window::WindowAttributes,
        ) -> Result<Box<dyn crate::platform::backend::window::Window>, winit::error::ExternalError>
        {
            winit::event_loop::ActiveEventLoop::create_window(self, attributes)
                .map_err(winit::error::ExternalError::Os)
                .map(|window| Box::new(window) as Box<dyn crate::platform::backend::window::Window>)
        }

        fn create_proxy(&self) -> EventLoopProxy {
            EventLoopProxy::new(super::CURRENT_EVENT_LOOP.with(|slot| {
                slot.borrow()
                    .as_ref()
                    .expect("event loop proxy unavailable outside winit callback")
                    .clone()
            }))
        }

        fn set_control_flow(&self, control_flow: ControlFlow) {
            winit::event_loop::ActiveEventLoop::set_control_flow(self, control_flow);
        }

        fn control_flow(&self) -> ControlFlow {
            winit::event_loop::ActiveEventLoop::control_flow(self)
        }

        fn exit(&self) {
            winit::event_loop::ActiveEventLoop::exit(self);
        }

        fn primary_monitor(&self) -> Option<winit::monitor::MonitorHandle> {
            winit::event_loop::ActiveEventLoop::primary_monitor(self)
        }

        fn available_monitors(&self) -> Box<dyn Iterator<Item = winit::monitor::MonitorHandle>> {
            Box::new(winit::event_loop::ActiveEventLoop::available_monitors(self))
        }
    }
}

pub mod window_ext {
    use super::*;
    use crate::platform::window::ImeRequest;

    pub use winit::window::{Cursor, ResizeDirection, Theme, WindowId};

    pub trait Window: HasWindowHandle + HasDisplayHandle + Send + Sync {
        fn id(&self) -> WindowId;
        fn scale_factor(&self) -> f64;
        fn request_redraw(&self);
        fn pre_present_notify(&self);
        fn surface_size(&self) -> winit::dpi::PhysicalSize<u32>;
        fn current_monitor(&self) -> Option<winit::monitor::MonitorHandle> {
            None
        }
        fn set_visible(&self, visible: bool);
        fn set_title(&self, title: &str);
        fn is_decorated(&self) -> bool;
        fn set_decorations(&self, decorations: bool);
        fn has_focus(&self) -> bool;
        fn is_maximized(&self) -> bool;
        fn set_maximized(&self, maximized: bool);
        fn set_minimized(&self, minimized: bool);
        fn drag_window(&self) -> Result<(), winit::error::ExternalError>;
        fn drag_resize_window(
            &self,
            direction: ResizeDirection,
        ) -> Result<(), winit::error::ExternalError>;
        fn set_cursor(&self, cursor: Cursor);
        fn theme(&self) -> Option<Theme>;
        fn request_ime_update(
            &self,
            request: ImeRequest,
        ) -> Result<(), winit::error::ExternalError>;

        #[cfg(target_os = "windows")]
        fn set_enable(&self, enabled: bool);
    }

    impl Window for winit::window::Window {
        fn id(&self) -> window::WindowId {
            winit::window::Window::id(self)
        }

        fn scale_factor(&self) -> f64 {
            winit::window::Window::scale_factor(self)
        }

        fn request_redraw(&self) {
            winit::window::Window::request_redraw(self);
        }

        fn pre_present_notify(&self) {
            winit::window::Window::pre_present_notify(self);
        }

        fn surface_size(&self) -> winit::dpi::PhysicalSize<u32> {
            winit::window::Window::inner_size(self)
        }

        fn current_monitor(&self) -> Option<winit::monitor::MonitorHandle> {
            winit::window::Window::current_monitor(self)
        }

        fn set_visible(&self, visible: bool) {
            winit::window::Window::set_visible(self, visible);
        }

        fn set_title(&self, title: &str) {
            winit::window::Window::set_title(self, title);
        }

        fn is_decorated(&self) -> bool {
            winit::window::Window::is_decorated(self)
        }

        fn set_decorations(&self, decorations: bool) {
            winit::window::Window::set_decorations(self, decorations);
        }

        fn has_focus(&self) -> bool {
            winit::window::Window::has_focus(self)
        }

        fn is_maximized(&self) -> bool {
            winit::window::Window::is_maximized(self)
        }

        fn set_maximized(&self, maximized: bool) {
            winit::window::Window::set_maximized(self, maximized);
        }

        fn set_minimized(&self, minimized: bool) {
            winit::window::Window::set_minimized(self, minimized);
        }

        fn drag_window(&self) -> Result<(), winit::error::ExternalError> {
            winit::window::Window::drag_window(self)
        }

        fn drag_resize_window(
            &self,
            direction: ResizeDirection,
        ) -> Result<(), winit::error::ExternalError> {
            winit::window::Window::drag_resize_window(self, direction)
        }

        fn set_cursor(&self, cursor: Cursor) {
            winit::window::Window::set_cursor(self, cursor);
        }

        fn theme(&self) -> Option<Theme> {
            winit::window::Window::theme(self)
        }

        fn request_ime_update(
            &self,
            request: ImeRequest,
        ) -> Result<(), winit::error::ExternalError> {
            match request {
                ImeRequest::Enable(enable) => {
                    winit::window::Window::set_ime_allowed(self, true);
                    apply_ime_data(self, enable.data);
                }
                ImeRequest::Update(data) => apply_ime_data(self, data),
                ImeRequest::Disable => winit::window::Window::set_ime_allowed(self, false),
            }
            Ok(())
        }

        #[cfg(target_os = "windows")]
        fn set_enable(&self, enabled: bool) {
            use winit::platform::windows::WindowExtWindows;
            WindowExtWindows::set_enable(self, enabled);
        }
    }

    fn apply_ime_data(
        window: &winit::window::Window,
        data: crate::platform::window::ImeRequestData,
    ) {
        if let Some((position, size)) = data.cursor_area {
            window.set_ime_cursor_area(position, size);
        }
        if let Some((_, purpose)) = data.hint_and_purpose {
            let purpose = match purpose {
                crate::platform::window::ImePurpose::Normal => winit::window::ImePurpose::Normal,
            };
            window.set_ime_purpose(purpose);
        }
    }

    pub use Window as WindowCompat;
}

#[derive(Clone)]
pub struct EventLoopProxy {
    inner: winit::event_loop::EventLoopProxy<()>,
}

impl EventLoopProxy {
    fn new(inner: winit::event_loop::EventLoopProxy<()>) -> Self {
        Self { inner }
    }

    pub fn wake_up(&self) -> bool {
        self.inner.send_event(()).is_ok()
    }
}

thread_local! {
    static CURRENT_EVENT_LOOP: std::cell::RefCell<Option<winit::event_loop::EventLoopProxy<()>>> =
        const { std::cell::RefCell::new(None) };
}

pub(crate) enum EventLoop {
    Winit(winit::event_loop::EventLoop<()>),
}

impl EventLoop {
    pub(crate) fn new() -> Result<Self, EventLoopError> {
        winit::event_loop::EventLoop::new().map(Self::Winit)
    }

    pub(crate) fn set_control_flow(&self, control_flow: ControlFlow) {
        match self {
            Self::Winit(event_loop) => event_loop.set_control_flow(control_flow),
        }
    }

    pub(crate) fn run_app_on_demand<A: winit::application::ApplicationHandler<()>>(
        &mut self,
        app: &mut A,
    ) -> Result<(), EventLoopError> {
        match self {
            Self::Winit(event_loop) => {
                CURRENT_EVENT_LOOP.with(|slot| {
                    *slot.borrow_mut() = Some(event_loop.create_proxy());
                });
                let result = event_loop.run_app_on_demand(app);
                CURRENT_EVENT_LOOP.with(|slot| {
                    *slot.borrow_mut() = None;
                });
                result
            }
        }
    }
}

pub use event_loop_ext as event_loop;
pub use window_ext as window;
