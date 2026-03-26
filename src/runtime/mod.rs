mod renderer;

use std::sync::Arc;

use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

use crate::ui::{ClickEvent, Node, Scene, View, build_scene};
use renderer::{GpuRenderer, RenderResult};

struct RuntimeApp {
    root: Node,
    window: Option<Arc<Window>>,
    window_id: Option<WindowId>,
    renderer: Option<GpuRenderer>,
    scene: Scene,
    cursor_pos: (f32, f32),
}

impl RuntimeApp {
    fn new(root: Node) -> Self {
        Self {
            root,
            window: None,
            window_id: None,
            renderer: None,
            scene: Scene {
                commands: Vec::new(),
                hits: Vec::new(),
            },
            cursor_pos: (0.0, 0.0),
        }
    }
}

impl ApplicationHandler for RuntimeApp {

    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let attrs = Window::default_attributes()
            .with_transparent(true)
            .with_visible(false)
            .with_title("tgui")
            .with_inner_size(LogicalSize::new(900.0, 600.0));
        let window = Arc::new(event_loop.create_window(attrs).expect("failed to create window"));
        self.window_id = Some(window.id());
        self.renderer = Some(GpuRenderer::new(window.clone()));
        self.window = Some(window.clone());
        window.request_redraw();
        window.set_visible(true)
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        if Some(id) != self.window_id {
            return;
        }
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let Some(renderer) = &mut self.renderer {
                    renderer.resize(size);
                }
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor_pos = (position.x as f32, position.y as f32);
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => {
                let (mx, my) = self.cursor_pos;
                for hit in self.scene.hits.iter().rev() {
                    if mx >= hit.x && mx <= hit.x + hit.w && my >= hit.y && my <= hit.y + hit.h {
                        (hit.on_click.borrow_mut())(ClickEvent { x: mx, y: my });
                        if let Some(window) = &self.window {
                            window.request_redraw();
                        }
                        break;
                    }
                }
            }
            WindowEvent::RedrawRequested => {
                if let Some(renderer) = &mut self.renderer {
                    self.scene = build_scene(&self.root, renderer.size.width, renderer.size.height);
                    match renderer.render(&self.scene) {
                        RenderResult::Ok | RenderResult::Skip => {}
                        RenderResult::Reconfigure => renderer.resize(renderer.size),
                    }
                }
            }
            _ => {}
        }
    }
}

pub fn run<V, F>(app: F)
where
    V: View + 'static,
    F: FnOnce() -> V,
{
    let root = app().into_node();
    let event_loop = EventLoop::new().expect("failed to create event loop");
    event_loop.set_control_flow(ControlFlow::Wait);
    let mut runtime = RuntimeApp::new(root);
    event_loop.run_app(&mut runtime).expect("event loop failure");
}
