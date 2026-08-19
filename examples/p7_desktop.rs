#[cfg(feature = "desktop")]
mod desktop {
    use std::error::Error;

    use tgui::event::UiEvent;
    use tgui::platform::{WinitSurface, WinitSurfaceEvent};
    use tgui::widget::{BuildContext, Widget};
    use tgui::widgets::{Button, Container, Text};
    use tgui::{Application, Point, Size, WindowId, WindowSpec};
    use winit::application::ApplicationHandler;
    use winit::event::WindowEvent;
    use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
    use winit::window::WindowId as PlatformWindowId;

    struct DesktopSmoke {
        application: Application,
        app_window: WindowId,
        spec: WindowSpec,
        surface: Option<WinitSurface>,
        exit_after_first_frame: bool,
        inject_device_loss: bool,
        device_loss_injected: bool,
        require_resize: bool,
        resize_seen: bool,
    }

    impl DesktopSmoke {
        fn new() -> tgui::Result<Self> {
            let spec = WindowSpec::new("tgui P7 desktop smoke")
                .with_inner_size(Size::new(720.0, 420.0))
                .with_transparent(true)
                .with_native_surface_support(true);
            let mut application = Application::new();
            let app_window = application.create_window(spec.clone())?;
            let mut build = BuildContext::new();
            let content = Container::new()
                .with_child(Text::new("tgui retained rendering").build(&mut build)?)
                .with_child(Button::new("Button uses Paint IR").build(&mut build)?)
                .build(&mut build)?;
            application.mount_widget(app_window, content)?;
            application.render_window(app_window)?;
            Ok(Self {
                application,
                app_window,
                spec,
                surface: None,
                exit_after_first_frame: std::env::var_os("TGUI_SMOKE_ONCE").is_some(),
                inject_device_loss: std::env::var_os("TGUI_SMOKE_DEVICE_LOSS").is_some(),
                device_loss_injected: false,
                require_resize: std::env::var_os("TGUI_SMOKE_RESIZE").is_some(),
                resize_seen: false,
            })
        }

        fn dispatch_window_event(&mut self, event: UiEvent) -> tgui::Result<()> {
            let hit = self.application.hit_test(self.app_window, Point::ZERO)?;
            self.application
                .dispatch_event(self.app_window, hit, &event)?;
            Ok(())
        }

        fn redraw(&mut self) -> tgui::Result<()> {
            self.application.render_window(self.app_window)?;
            let scene = self
                .application
                .compiled_scene(self.app_window)
                .expect("a successful frame commits a compiled scene");
            self.surface
                .as_mut()
                .expect("redraw follows resume")
                .render(&scene)
        }
    }

    impl ApplicationHandler for DesktopSmoke {
        fn resumed(&mut self, event_loop: &ActiveEventLoop) {
            if self.surface.is_some() {
                return;
            }
            match pollster::block_on(WinitSurface::new(event_loop, &self.spec)) {
                Ok(surface) => {
                    let logical_size = surface.logical_size();
                    let dpi_scale = surface.dpi_scale();
                    self.surface = Some(surface);
                    if self
                        .dispatch_window_event(UiEvent::WindowResized(logical_size))
                        .and_then(|()| {
                            self.dispatch_window_event(UiEvent::WindowDpiChanged(dpi_scale))
                        })
                        .is_err()
                    {
                        event_loop.exit();
                        return;
                    }
                    if self.require_resize {
                        let resized =
                            Size::new(logical_size.width + 32.0, logical_size.height + 16.0);
                        if self
                            .surface
                            .as_ref()
                            .expect("surface installed")
                            .request_resize(resized)
                            .is_err()
                        {
                            event_loop.exit();
                            return;
                        }
                    }
                    self.surface
                        .as_ref()
                        .expect("surface installed")
                        .request_redraw();
                }
                Err(error) => {
                    eprintln!("desktop initialization failed: {error}");
                    event_loop.exit();
                }
            }
        }

        fn window_event(
            &mut self,
            event_loop: &ActiveEventLoop,
            window_id: PlatformWindowId,
            event: WindowEvent,
        ) {
            let Some(surface) = self.surface.as_mut() else {
                return;
            };
            if surface.id() != window_id {
                return;
            }
            let translated = match surface.handle_event(&event) {
                Ok(event) => event,
                Err(error) => {
                    eprintln!("window event failed: {error}");
                    return;
                }
            };
            let result = match translated {
                WinitSurfaceEvent::CloseRequested => {
                    event_loop.exit();
                    Ok(())
                }
                WinitSurfaceEvent::RedrawRequested => {
                    if self.inject_device_loss && !self.device_loss_injected {
                        self.surface
                            .as_mut()
                            .expect("surface remains installed")
                            .renderer_mut()
                            .inject_device_loss();
                        self.device_loss_injected = true;
                    }
                    self.redraw()
                }
                WinitSurfaceEvent::Resized {
                    logical_size,
                    dpi_scale,
                } => {
                    self.resize_seen = true;
                    self.dispatch_window_event(UiEvent::WindowResized(logical_size))
                        .and_then(|()| {
                            self.dispatch_window_event(UiEvent::WindowDpiChanged(dpi_scale))
                        })
                        .map(|()| {
                            self.surface
                                .as_ref()
                                .expect("surface remains installed")
                                .request_redraw();
                        })
                }
                WinitSurfaceEvent::Ignored => Ok(()),
            };
            if let Err(error) = result {
                eprintln!("desktop frame failed: {error}");
                if let Some(surface) = self.surface.as_mut() {
                    if pollster::block_on(surface.recover_device()).is_ok() {
                        surface.request_redraw();
                    }
                }
            } else if translated == WinitSurfaceEvent::RedrawRequested
                && self.exit_after_first_frame
                && (!self.require_resize || self.resize_seen)
            {
                let surface = self.surface.as_ref().expect("surface remains installed");
                println!(
                    "desktop_smoke=ok size={:?} dpi={} resize={} device_recovery={}",
                    surface.logical_size(),
                    surface.dpi_scale().get(),
                    self.resize_seen,
                    self.device_loss_injected,
                );
                event_loop.exit();
            }
        }
    }

    pub fn run() -> Result<(), Box<dyn Error>> {
        let event_loop = EventLoop::new()?;
        event_loop.set_control_flow(ControlFlow::Wait);
        let mut application = DesktopSmoke::new()?;
        event_loop.run_app(&mut application)?;
        Ok(())
    }
}

#[cfg(feature = "desktop")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    desktop::run()
}

#[cfg(not(feature = "desktop"))]
fn main() {
    println!("p7_desktop requires --features desktop");
}
