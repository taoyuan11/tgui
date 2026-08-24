//! Optional real-window adapter for the component gallery.

use tgui::Point;
use tgui::event::{PointerButton, PointerButtons, PointerEvent, PointerId, PointerKind, UiEvent};
use tgui::platform::{WinitSurface, WinitSurfaceEvent};
use winit::application::ApplicationHandler;
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::WindowId;

use crate::app::Gallery;

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Wait);
    let mut app = DesktopGallery::new()?;
    event_loop.run_app(&mut app)?;
    Ok(())
}

struct DesktopGallery {
    gallery: Gallery,
    surface: Option<WinitSurface>,
    cursor: Point,
    primary_pressed: bool,
}

impl DesktopGallery {
    fn new() -> tgui::Result<Self> {
        Ok(Self {
            gallery: Gallery::new()?,
            surface: None,
            cursor: Point::ZERO,
            primary_pressed: false,
        })
    }

    fn request_redraw(&self) {
        if let Some(surface) = &self.surface {
            surface.request_redraw();
        }
    }

    fn dispatch_window(&mut self, event: UiEvent) -> tgui::Result<()> {
        self.gallery.dispatch(Point::ZERO, event)
    }
}

impl ApplicationHandler for DesktopGallery {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.surface.is_some() {
            return;
        }
        match pollster::block_on(WinitSurface::new(event_loop, &self.gallery.spec)) {
            Ok(surface) => {
                let logical_size = surface.logical_size();
                let dpi_scale = surface.dpi_scale();
                self.surface = Some(surface);
                let result = self
                    .dispatch_window(UiEvent::WindowResized(logical_size))
                    .and_then(|()| self.dispatch_window(UiEvent::WindowDpiChanged(dpi_scale)));
                if let Err(error) = result {
                    eprintln!("gallery window initialization failed: {error}");
                    event_loop.exit();
                } else {
                    self.request_redraw();
                }
            }
            Err(error) => {
                eprintln!("gallery desktop initialization failed: {error}");
                event_loop.exit();
            }
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
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
                eprintln!("gallery window event failed: {error}");
                return;
            }
        };

        let result = match translated {
            WinitSurfaceEvent::CloseRequested => {
                event_loop.exit();
                return;
            }
            WinitSurfaceEvent::RedrawRequested => {
                let scene = self.gallery.redraw();
                scene.and_then(|scene| {
                    self.surface
                        .as_mut()
                        .expect("the surface remains installed")
                        .render(scene.as_ref())
                })
            }
            WinitSurfaceEvent::Resized {
                logical_size,
                dpi_scale,
            } => self
                .dispatch_window(UiEvent::WindowResized(logical_size))
                .and_then(|()| self.dispatch_window(UiEvent::WindowDpiChanged(dpi_scale))),
            WinitSurfaceEvent::Ignored => match event {
                WindowEvent::CursorMoved { position, .. } => {
                    let scale = self
                        .surface
                        .as_ref()
                        .expect("the surface remains installed")
                        .dpi_scale()
                        .get();
                    let logical = position.to_logical::<f64>(scale);
                    self.cursor = Point::new(logical.x as f32, logical.y as f32);
                    let buttons = if self.primary_pressed {
                        PointerButtons::PRIMARY
                    } else {
                        PointerButtons::NONE
                    };
                    self.gallery.dispatch(
                        self.cursor,
                        UiEvent::PointerMove(
                            PointerEvent::new(PointerId::MOUSE, PointerKind::Mouse, self.cursor)
                                .with_buttons(buttons),
                        ),
                    )
                }
                WindowEvent::MouseInput {
                    state,
                    button: MouseButton::Left,
                    ..
                } => {
                    self.primary_pressed = state == ElementState::Pressed;
                    let buttons = if self.primary_pressed {
                        PointerButtons::PRIMARY
                    } else {
                        PointerButtons::NONE
                    };
                    let pointer =
                        PointerEvent::new(PointerId::MOUSE, PointerKind::Mouse, self.cursor)
                            .with_button(Some(PointerButton::Primary))
                            .with_buttons(buttons);
                    let event = if self.primary_pressed {
                        UiEvent::PointerDown(pointer)
                    } else {
                        UiEvent::PointerUp(pointer)
                    };
                    self.gallery.dispatch(self.cursor, event)
                }
                WindowEvent::Focused(active) => {
                    self.dispatch_window(UiEvent::WindowActivated(active))
                }
                _ => Ok(()),
            },
        };

        if let Err(error) = result {
            eprintln!("gallery frame failed: {error}");
        } else if translated != WinitSurfaceEvent::RedrawRequested {
            self.request_redraw();
        }
    }
}
