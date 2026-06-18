pub use winit_core::application;
pub use winit_core::event_loop;
pub use winit_core::window;

pub(crate) use winit_core::error::EventLoopError;
pub(crate) use winit_core::event_loop::ControlFlow;

pub(crate) enum EventLoop {
    #[cfg(target_os = "windows")]
    Windows(winit_win32::EventLoop),
    #[cfg(target_os = "macos")]
    MacOS(winit_appkit::EventLoop),
    #[cfg(target_os = "linux")]
    Wayland(winit_wayland::EventLoop),
    #[cfg(target_os = "linux")]
    X11(winit_x11::EventLoop),
}

impl EventLoop {
    pub(crate) fn new() -> Result<Self, EventLoopError> {
        #[cfg(target_os = "windows")]
        {
            let mut attributes = winit_win32::PlatformSpecificEventLoopAttributes::default();
            return winit_win32::EventLoop::new(&mut attributes).map(Self::Windows);
        }

        #[cfg(target_os = "macos")]
        {
            let attributes = winit_appkit::PlatformSpecificEventLoopAttributes::default();
            return winit_appkit::EventLoop::new(&attributes).map(Self::MacOS);
        }

        #[cfg(target_os = "linux")]
        {
            match winit_wayland::EventLoop::new() {
                Ok(event_loop) => Ok(Self::Wayland(event_loop)),
                Err(_) => winit_x11::EventLoop::new().map(Self::X11),
            }
        }
    }

    pub(crate) fn set_control_flow(&self, control_flow: ControlFlow) {
        match self {
            #[cfg(target_os = "windows")]
            Self::Windows(event_loop) => event_loop.window_target().set_control_flow(control_flow),
            #[cfg(target_os = "macos")]
            Self::MacOS(event_loop) => event_loop.window_target().set_control_flow(control_flow),
            #[cfg(target_os = "linux")]
            Self::Wayland(event_loop) => event_loop.window_target().set_control_flow(control_flow),
            #[cfg(target_os = "linux")]
            Self::X11(event_loop) => event_loop.window_target().set_control_flow(control_flow),
        }
    }

    pub(crate) fn run_app_on_demand<A: application::ApplicationHandler>(
        &mut self,
        app: A,
    ) -> Result<(), EventLoopError> {
        match self {
            #[cfg(target_os = "windows")]
            Self::Windows(event_loop) => event_loop.run_app_on_demand(app),
            #[cfg(target_os = "macos")]
            Self::MacOS(event_loop) => event_loop.run_app_on_demand(app),
            #[cfg(target_os = "linux")]
            Self::Wayland(event_loop) => event_loop.run_app_on_demand(app),
            #[cfg(target_os = "linux")]
            Self::X11(event_loop) => event_loop.run_app_on_demand(app),
        }
    }
}
