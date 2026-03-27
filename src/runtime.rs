use std::sync::Arc;

use winit::application::ApplicationHandler;
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowAttributes, WindowId};

use crate::application::ApplicationConfig;
use crate::foundation::binding::{Binding, InvalidationSignal};
use crate::foundation::error::TguiError;
use crate::foundation::event::InputTrigger;
use crate::foundation::view_model::{Command, ViewModel};
use crate::rendering::renderer::{RenderStatus, Renderer};
use crate::text::font::FontManager;
use crate::ui::theme::Theme;
use crate::ui::widget::{
    Point, Rect, ScenePrimitives, WidgetCommand as UiCommand, WidgetId, WidgetTree,
};

pub struct Runtime {
    event_loop: EventLoop<()>,
    config: ApplicationConfig,
}

impl Runtime {
    pub fn new(config: ApplicationConfig) -> Result<Self, TguiError> {
        let event_loop = EventLoop::new()?;
        event_loop.set_control_flow(ControlFlow::Poll);
        Ok(Self { event_loop, config })
    }

    pub fn run(self) -> Result<(), TguiError> {
        let mut handler = RuntimeHandler::new(self.config);
        self.event_loop.run_app(&mut handler)?;

        if let Some(error) = handler.error {
            return Err(error);
        }

        Ok(())
    }
}

pub struct BoundRuntime<VM> {
    event_loop: EventLoop<()>,
    config: ApplicationConfig,
    view_model: VM,
    window_bindings: WindowBindings,
    widget_tree: Option<WidgetTree<VM>>,
    theme: Theme,
    commands: Vec<WindowCommand<VM>>,
    invalidation: InvalidationSignal,
}

impl<VM: ViewModel> BoundRuntime<VM> {
    pub fn new(
        config: ApplicationConfig,
        view_model: VM,
        window_bindings: WindowBindings,
        widget_tree: Option<WidgetTree<VM>>,
        theme: Theme,
        commands: Vec<WindowCommand<VM>>,
        invalidation: InvalidationSignal,
    ) -> Result<Self, TguiError> {
        let event_loop = EventLoop::new()?;
        event_loop.set_control_flow(ControlFlow::Wait);
        Ok(Self {
            event_loop,
            config,
            view_model,
            window_bindings,
            widget_tree,
            theme,
            commands,
            invalidation,
        })
    }

    pub fn run(self) -> Result<(), TguiError> {
        let mut handler = BoundRuntimeHandler::new(
            self.config,
            self.view_model,
            self.window_bindings,
            self.widget_tree,
            self.theme,
            self.commands,
            self.invalidation,
        );
        self.event_loop.run_app(&mut handler)?;

        if let Some(error) = handler.error {
            return Err(error);
        }

        Ok(())
    }
}

#[derive(Default)]
pub struct WindowBindings {
    pub(crate) title: Option<Binding<String>>,
    pub(crate) clear_color: Option<Binding<wgpu::Color>>,
}

pub struct WindowCommand<VM> {
    pub(crate) trigger: InputTrigger,
    pub(crate) command: Command<VM>,
}

struct RuntimeHandler {
    config: ApplicationConfig,
    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
    window_id: Option<WindowId>,
    error: Option<TguiError>,
}

impl RuntimeHandler {
    fn new(config: ApplicationConfig) -> Self {
        Self {
            config,
            window: None,
            renderer: None,
            window_id: None,
            error: None,
        }
    }

    fn fail(&mut self, event_loop: &ActiveEventLoop, error: TguiError) {
        self.error = Some(error);
        event_loop.exit();
    }
}

struct BoundRuntimeHandler<VM> {
    config: ApplicationConfig,
    font_manager: FontManager,
    theme: Theme,
    view_model: VM,
    window_bindings: WindowBindings,
    widget_tree: Option<WidgetTree<VM>>,
    commands: Vec<WindowCommand<VM>>,
    invalidation: InvalidationSignal,
    cursor_position: Option<Point>,
    focused_input: Option<WidgetId>,
    cached_scene: Option<CachedScene>,
    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
    window_id: Option<WindowId>,
    error: Option<TguiError>,
}

struct CachedScene {
    viewport: Rect,
    focused_input: Option<WidgetId>,
    primitives: ScenePrimitives,
}

impl<VM> BoundRuntimeHandler<VM> {
    fn new(
        config: ApplicationConfig,
        view_model: VM,
        window_bindings: WindowBindings,
        widget_tree: Option<WidgetTree<VM>>,
        theme: Theme,
        commands: Vec<WindowCommand<VM>>,
        invalidation: InvalidationSignal,
    ) -> Self {
        let font_manager = FontManager::new(&config.fonts);
        Self {
            config,
            font_manager,
            theme,
            view_model,
            window_bindings,
            widget_tree,
            commands,
            invalidation,
            cursor_position: None,
            focused_input: None,
            cached_scene: None,
            window: None,
            renderer: None,
            window_id: None,
            error: None,
        }
    }

    fn fail(&mut self, event_loop: &ActiveEventLoop, error: TguiError) {
        self.error = Some(error);
        event_loop.exit();
    }

    fn sync_bindings(&mut self) {
        if let Some(window) = self.window.as_ref() {
            if let Some(binding) = self.window_bindings.title.as_ref() {
                window.set_title(&binding.get());
            }
        }

        if let Some(renderer) = self.renderer.as_mut() {
            if let Some(binding) = self.window_bindings.clear_color.as_ref() {
                renderer.set_clear_color(binding.get());
            }
        }
    }

    fn request_redraw_if_dirty(&mut self) {
        if self.invalidation.take_dirty() {
            self.invalidate_scene();
            self.sync_bindings();

            if let Some(window) = self.window.as_ref() {
                window.request_redraw();
            }
        }
    }

    fn render_primitives(&mut self) -> ScenePrimitives {
        let viewport = self.viewport_rect();
        if let Some(cached) = self.cached_scene.as_ref() {
            if cached.viewport == viewport && cached.focused_input == self.focused_input {
                return cached.primitives.clone();
            }
        }

        let primitives = self
            .widget_tree
            .as_ref()
            .map(|tree| {
                tree.render_primitives(
                    &self.font_manager,
                    &self.theme,
                    viewport,
                    self.focused_input,
                )
            })
            .unwrap_or_default();
        self.cached_scene = Some(CachedScene {
            viewport,
            focused_input: self.focused_input,
            primitives: primitives.clone(),
        });
        primitives
    }

    fn viewport_rect(&self) -> Rect {
        let size = self
            .window
            .as_ref()
            .map(|window| window.inner_size())
            .unwrap_or(self.config.size.to_physical::<u32>(1.0));
        Rect::new(0.0, 0.0, size.width as f32, size.height as f32)
    }

    fn invalidate_scene(&mut self) {
        self.cached_scene = None;
    }

    fn should_dispatch_widget_event(event: &WindowEvent) -> bool {
        matches!(
            event,
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } | WindowEvent::KeyboardInput { .. }
        )
    }
}

impl ApplicationHandler for RuntimeHandler {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        let attributes = WindowAttributes::default()
            .with_transparent(true)
            .with_title(self.config.title.clone())
            .with_inner_size(self.config.size)
            .with_visible(false);

        let window = match event_loop.create_window(attributes) {
            Ok(window) => Arc::new(window),
            Err(error) => {
                self.fail(event_loop, error.into());
                return;
            }
        };

        let renderer =
            match Renderer::new(window.clone(), self.config.clear_color, &self.config.fonts) {
                Ok(renderer) => renderer,
                Err(error) => {
                    self.fail(event_loop, error);
                    return;
                }
            };

        self.window_id = Some(window.id());
        self.renderer = Some(renderer);
        self.window = Some(window);

        if let Some(window) = self.window.as_ref() {
            window.set_visible(true)
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        if Some(window_id) != self.window_id {
            return;
        }

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let Some(renderer) = self.renderer.as_mut() {
                    renderer.resize(size);
                }
            }
            WindowEvent::RedrawRequested => {
                if let Some(renderer) = self.renderer.as_mut() {
                    match renderer.render(&ScenePrimitives::default()) {
                        Ok(RenderStatus::Rendered | RenderStatus::SkipFrame) => {}
                        Ok(RenderStatus::ReconfigureSurface) => renderer.reconfigure(),
                        Err(error) => self.fail(event_loop, error),
                    }
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(window) = self.window.as_ref() {
            window.request_redraw();
        }
    }
}

impl<VM: ViewModel> ApplicationHandler for BoundRuntimeHandler<VM> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        let attributes = WindowAttributes::default()
            .with_transparent(true)
            .with_title(self.config.title.clone())
            .with_inner_size(self.config.size)
            .with_visible(false);

        let window = match event_loop.create_window(attributes) {
            Ok(window) => Arc::new(window),
            Err(error) => {
                self.fail(event_loop, error.into());
                return;
            }
        };

        let renderer =
            match Renderer::new(window.clone(), self.config.clear_color, &self.config.fonts) {
                Ok(renderer) => renderer,
                Err(error) => {
                    self.fail(event_loop, error);
                    return;
                }
            };

        self.window_id = Some(window.id());
        self.renderer = Some(renderer);
        self.window = Some(window);
        self.sync_bindings();

        if let Some(window) = self.window.as_ref() {
            window.set_visible(true);
            window.request_redraw();
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        if Some(window_id) != self.window_id {
            return;
        }

        if let WindowEvent::CursorMoved { position, .. } = &event {
            self.cursor_position = Some(Point {
                x: position.x as f32,
                y: position.y as f32,
            });
        }

        if Self::should_dispatch_widget_event(&event) {
            if let Some(tree) = self.widget_tree.as_ref() {
                let previous_focus = self.focused_input;
                let widget_result = tree.handle_window_event(
                    &self.font_manager,
                    &self.theme,
                    self.viewport_rect(),
                    &event,
                    self.cursor_position,
                    self.focused_input,
                );

                self.focused_input = widget_result.focus;
                if self.focused_input != previous_focus {
                    self.invalidate_scene();
                }

                if let Some(command) = widget_result.command {
                    match command {
                        UiCommand::Command(command) => command.execute(&mut self.view_model),
                        UiCommand::Value(command, value) => {
                            command.execute(&mut self.view_model, value)
                        }
                    }
                    self.invalidate_scene();
                    self.invalidation.mark_dirty();
                }

                if widget_result.request_redraw {
                    if let Some(window) = self.window.as_ref() {
                        window.request_redraw();
                    }
                }
            }
        }

        if let Some(window_command) = self
            .commands
            .iter()
            .find(|entry| entry.trigger.matches(&event))
        {
            window_command.command.execute(&mut self.view_model);
            self.invalidate_scene();
            self.invalidation.mark_dirty();
        }

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                self.invalidate_scene();
                if let Some(renderer) = self.renderer.as_mut() {
                    renderer.resize(size);
                }

                if let Some(window) = self.window.as_ref() {
                    window.request_redraw();
                }
            }
            WindowEvent::RedrawRequested => {
                self.sync_bindings();
                let primitives = self.render_primitives();

                if let Some(renderer) = self.renderer.as_mut() {
                    match renderer.render(&primitives) {
                        Ok(RenderStatus::Rendered | RenderStatus::SkipFrame) => {}
                        Ok(RenderStatus::ReconfigureSurface) => renderer.reconfigure(),
                        Err(error) => self.fail(event_loop, error),
                    }
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        self.request_redraw_if_dirty();
    }
}
