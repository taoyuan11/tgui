pub mod dpi {
    pub use dpi::*;
}

pub mod error {
    pub use winit_core::error::*;
}

pub mod event {
    pub use winit_core::event::*;
}

pub mod keyboard {
    pub use winit_core::keyboard::*;
}

pub mod cursor {
    pub use winit_core::cursor::*;
}

pub mod window {
    pub use winit_core::window::*;
}

#[cfg(all(target_os = "android", feature = "android"))]
pub mod android {
    pub mod activity {
        pub use winit_android::activity::*;
    }

    pub use winit_android::{
        ActiveEventLoopExtAndroid, EventLoopBuilderExtAndroid, EventLoopExtAndroid,
        WindowExtAndroid,
    };
}

#[cfg(all(target_env = "ohos", feature = "ohos"))]
pub mod ohos {
    pub use tgui_winit_ohos::{
        export_ohos_winit_app, ActiveEventLoopExtOhos, EventLoopBuilderExtOhos, OhosApp,
        WindowExtOhos,
    };
}

pub(crate) mod backend {
    pub use winit_core::application;
    pub use winit_core::event_loop;
    pub use winit_core::window;

    pub(crate) use winit_core::error::EventLoopError;
    #[cfg(all(target_env = "ohos", feature = "ohos"))]
    use winit_core::event_loop::run_on_demand::EventLoopExtRunOnDemand;
    pub(crate) use winit_core::event_loop::ControlFlow;

    pub(crate) enum EventLoop {
        #[cfg(target_os = "windows")]
        Windows(winit_win32::EventLoop),
        #[cfg(target_os = "macos")]
        MacOS(winit_appkit::EventLoop),
        #[cfg(all(target_os = "linux", not(target_env = "ohos")))]
        Wayland(winit_wayland::EventLoop),
        #[cfg(all(target_os = "linux", not(target_env = "ohos")))]
        X11(winit_x11::EventLoop),
        #[cfg(all(target_os = "android", feature = "android"))]
        Android(winit_android::EventLoop),
        #[cfg(all(target_env = "ohos", feature = "ohos"))]
        Ohos(tgui_winit_ohos::EventLoop),
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

            #[cfg(all(target_os = "linux", not(target_env = "ohos")))]
            {
                match winit_wayland::EventLoop::new() {
                    Ok(event_loop) => Ok(Self::Wayland(event_loop)),
                    Err(_) => winit_x11::EventLoop::new().map(Self::X11),
                }
            }

            #[cfg(all(target_os = "android", feature = "android"))]
            {
                unreachable!("Android requires EventLoop::with_android_app");
            }

            #[cfg(all(target_env = "ohos", feature = "ohos"))]
            {
                unreachable!("OHOS requires EventLoop::with_ohos_app");
            }
        }

        #[cfg(all(target_os = "android", feature = "android"))]
        pub(crate) fn with_android_app(
            app: crate::platform::android::activity::AndroidApp,
        ) -> Result<Self, EventLoopError> {
            let attributes = winit_android::PlatformSpecificEventLoopAttributes {
                android_app: Some(app),
                ..Default::default()
            };
            winit_android::EventLoop::new(&attributes).map(Self::Android)
        }

        #[cfg(all(target_env = "ohos", feature = "ohos"))]
        pub(crate) fn with_ohos_app(
            app: crate::platform::ohos::OhosApp,
        ) -> Result<Self, EventLoopError> {
            let attributes = tgui_winit_ohos::PlatformSpecificEventLoopAttributes {
                ohos_app: Some(app),
            };
            tgui_winit_ohos::EventLoop::new(&attributes).map(Self::Ohos)
        }

        pub(crate) fn set_control_flow(&self, control_flow: ControlFlow) {
            match self {
                #[cfg(target_os = "windows")]
                Self::Windows(event_loop) => {
                    event_loop.window_target().set_control_flow(control_flow)
                }
                #[cfg(target_os = "macos")]
                Self::MacOS(event_loop) => event_loop.set_control_flow(control_flow),
                #[cfg(all(target_os = "linux", not(target_env = "ohos")))]
                Self::Wayland(event_loop) => {
                    event_loop.window_target().set_control_flow(control_flow)
                }
                #[cfg(all(target_os = "linux", not(target_env = "ohos")))]
                Self::X11(event_loop) => event_loop.window_target().set_control_flow(control_flow),
                #[cfg(all(target_os = "android", feature = "android"))]
                Self::Android(event_loop) => {
                    event_loop.window_target().set_control_flow(control_flow)
                }
                #[cfg(all(target_env = "ohos", feature = "ohos"))]
                Self::Ohos(event_loop) => event_loop.window_target().set_control_flow(control_flow),
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
                #[cfg(all(target_os = "linux", not(target_env = "ohos")))]
                Self::Wayland(event_loop) => event_loop.run_app_on_demand(app),
                #[cfg(all(target_os = "linux", not(target_env = "ohos")))]
                Self::X11(event_loop) => event_loop.run_app_on_demand(app),
                #[cfg(all(target_os = "android", feature = "android"))]
                Self::Android(event_loop) => event_loop.run_app_on_demand(app),
                #[cfg(all(target_env = "ohos", feature = "ohos"))]
                Self::Ohos(event_loop) => event_loop.run_app_on_demand(app),
            }
        }
    }
}
