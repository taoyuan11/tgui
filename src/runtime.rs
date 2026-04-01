use std::sync::Arc;
use std::time::Instant;

use crate::animation::{
    default_theme_transition, AnimationEngine, AnimationKey, Transition, WindowProperty,
};
use winit::application::ApplicationHandler;
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Theme as WindowTheme, Window, WindowAttributes, WindowId};

use crate::application::{ApplicationConfig, ThemeSelection};
use crate::foundation::binding::{Binding, InvalidationSignal};
use crate::foundation::color::Color;
use crate::foundation::error::TguiError;
use crate::foundation::event::InputTrigger;
use crate::foundation::view_model::{Command, ViewModel};
use crate::rendering::renderer::{RenderStatus, Renderer};
use crate::text::font::FontManager;
use crate::ui::theme::{Theme, ThemeMode};
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
    commands: Vec<WindowCommand<VM>>,
    invalidation: InvalidationSignal,
}

impl<VM: ViewModel> BoundRuntime<VM> {
    pub fn new(
        config: ApplicationConfig,
        view_model: VM,
        window_bindings: WindowBindings,
        widget_tree: Option<WidgetTree<VM>>,
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
    pub(crate) clear_color: Option<Binding<Color>>,
    pub(crate) theme_mode: Option<Binding<ThemeMode>>,
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

    fn resolved_theme(&self, window: &Window) -> Theme {
        resolve_theme(&self.config.theme, window.theme())
    }

    fn render_hidden_frame(&mut self, event_loop: &ActiveEventLoop) -> bool {
        let Some(renderer) = self.renderer.as_mut() else {
            return true;
        };

        match renderer.render(&ScenePrimitives::default()) {
            Ok(RenderStatus::Rendered | RenderStatus::SkipFrame) => true,
            Ok(RenderStatus::ReconfigureSurface) => {
                renderer.reconfigure();
                match renderer.render(&ScenePrimitives::default()) {
                    Ok(RenderStatus::Rendered | RenderStatus::SkipFrame) => true,
                    Ok(RenderStatus::ReconfigureSurface) => true,
                    Err(error) => {
                        self.fail(event_loop, error);
                        false
                    }
                }
            }
            Err(error) => {
                self.fail(event_loop, error);
                false
            }
        }
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
    animation_engine: AnimationEngine,
    animation_epoch: u64,
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
    animation_epoch: u64,
    primitives: ScenePrimitives,
}

impl<VM> BoundRuntimeHandler<VM> {
    fn new(
        config: ApplicationConfig,
        view_model: VM,
        window_bindings: WindowBindings,
        widget_tree: Option<WidgetTree<VM>>,
        commands: Vec<WindowCommand<VM>>,
        invalidation: InvalidationSignal,
    ) -> Self {
        let font_manager = FontManager::new(&config.fonts);
        let theme = match &config.theme {
            ThemeSelection::Fixed(theme) => theme.clone(),
            ThemeSelection::System => Theme::default(),
        };
        Self {
            config,
            font_manager,
            theme,
            view_model,
            window_bindings,
            widget_tree,
            commands,
            invalidation,
            animation_engine: AnimationEngine::default(),
            animation_epoch: 0,
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

    fn uses_system_theme(&self) -> bool {
        matches!(self.active_theme_selection(), ThemeSelection::System)
    }

    fn apply_theme(&mut self, theme: Theme) {
        self.theme = theme;
        self.invalidate_scene();
    }

    fn apply_window_theme(&mut self, window_theme: Option<WindowTheme>) {
        if self.uses_system_theme() {
            self.apply_theme(resolve_theme(&self.active_theme_selection(), window_theme));
        }
    }

    fn active_theme_selection(&self) -> ThemeSelection {
        self.window_bindings
            .theme_mode
            .as_ref()
            .map(|binding| ThemeSelection::from_mode(binding.get()))
            .unwrap_or_else(|| self.config.theme.clone())
    }

    fn sync_theme_binding(&mut self) {
        let resolved_theme = resolve_theme(
            &self.active_theme_selection(),
            self.window.as_ref().and_then(|window| window.theme()),
        );
        if self.theme != resolved_theme {
            self.apply_theme(resolved_theme);
        }
    }

    fn sync_bindings(&mut self, now: Instant) {
        self.sync_theme_binding();

        if let Some(window) = self.window.as_ref() {
            if let Some(binding) = self.window_bindings.title.as_ref() {
                window.set_title(&binding.get());
            }
        }

        let theme = self.animated_theme(now);
        if let Some(renderer) = self.renderer.as_mut() {
            if let Some(binding) = self.window_bindings.clear_color.as_ref() {
                renderer.set_clear_color(self.animation_engine.resolve_color(
                    AnimationKey::Window(WindowProperty::ClearColor),
                    binding.get(),
                    binding.transition(),
                    now,
                ));
            } else if !self.config.clear_color_overridden {
                renderer.set_clear_color(theme.palette.window_background);
            }
        }
    }

    fn request_redraw_if_dirty(&mut self, now: Instant) {
        if self.invalidation.take_dirty() {
            self.invalidate_scene();
            self.sync_bindings(now);

            if let Some(window) = self.window.as_ref() {
                window.request_redraw();
            }
        }
    }

    fn render_primitives(&mut self) -> ScenePrimitives {
        let viewport = self.viewport_rect();
        if let Some(cached) = self.cached_scene.as_ref() {
            if cached.viewport == viewport
                && cached.focused_input == self.focused_input
                && cached.animation_epoch == self.animation_epoch
            {
                return cached.primitives.clone();
            }
        }

        let theme = self.animated_theme(Instant::now());
        let primitives = match self.widget_tree.as_ref() {
            Some(tree) => tree.render_primitives(
                &self.font_manager,
                &theme,
                &mut self.animation_engine,
                viewport,
                self.focused_input,
            ),
            None => ScenePrimitives::default(),
        };
        self.cached_scene = Some(CachedScene {
            viewport,
            focused_input: self.focused_input,
            animation_epoch: self.animation_epoch,
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

    fn render_current_frame(&mut self) -> Result<RenderStatus, TguiError> {
        self.sync_bindings(Instant::now());
        let primitives = self.render_primitives();
        let renderer = self
            .renderer
            .as_mut()
            .expect("renderer should exist before drawing");
        renderer.render(&primitives)
    }

    fn drive_animations(&mut self, event_loop: &ActiveEventLoop, now: Instant) {
        if self.animation_engine.refresh(now) {
            self.animation_epoch = self.animation_epoch.wrapping_add(1);
            self.invalidate_scene();
            if let Some(window) = self.window.as_ref() {
                window.request_redraw();
            }
        }

        if let Some(deadline) = self.animation_engine.next_frame_deadline(now) {
            event_loop.set_control_flow(ControlFlow::WaitUntil(deadline));
            if let Some(window) = self.window.as_ref() {
                window.request_redraw();
            }
        } else {
            event_loop.set_control_flow(ControlFlow::Wait);
        }
    }

    fn render_hidden_frame(&mut self, event_loop: &ActiveEventLoop) -> bool {
        let status = match self.render_current_frame() {
            Ok(status) => status,
            Err(error) => {
                self.fail(event_loop, error);
                return false;
            }
        };

        if matches!(status, RenderStatus::ReconfigureSurface) {
            if let Some(renderer) = self.renderer.as_mut() {
                renderer.reconfigure();
            }

            if let Err(error) = self.render_current_frame() {
                self.fail(event_loop, error);
                return false;
            }
        }

        true
    }

    fn animated_theme(&mut self, now: Instant) -> Theme {
        let transition = Some(default_theme_transition());
        let mut theme = self.theme.clone();
        theme.palette.window_background = self.resolve_theme_color(
            WindowProperty::ThemeWindowBackground,
            theme.palette.window_background,
            transition,
            now,
        );
        theme.palette.surface = self.resolve_theme_color(
            WindowProperty::ThemeSurface,
            theme.palette.surface,
            transition,
            now,
        );
        theme.palette.surface_muted = self.resolve_theme_color(
            WindowProperty::ThemeSurfaceMuted,
            theme.palette.surface_muted,
            transition,
            now,
        );
        theme.palette.accent = self.resolve_theme_color(
            WindowProperty::ThemeAccent,
            theme.palette.accent,
            transition,
            now,
        );
        theme.palette.text = self.resolve_theme_color(
            WindowProperty::ThemeText,
            theme.palette.text,
            transition,
            now,
        );
        theme.palette.text_muted = self.resolve_theme_color(
            WindowProperty::ThemeTextMuted,
            theme.palette.text_muted,
            transition,
            now,
        );
        theme.palette.input_background = self.resolve_theme_color(
            WindowProperty::ThemeInputBackground,
            theme.palette.input_background,
            transition,
            now,
        );
        theme
    }

    fn resolve_theme_color(
        &mut self,
        property: WindowProperty,
        target: Color,
        transition: Option<Transition>,
        now: Instant,
    ) -> Color {
        self.animation_engine
            .resolve_color(AnimationKey::Window(property), target, transition, now)
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

        let clear_color = if self.config.clear_color_overridden {
            self.config.clear_color
        } else {
            self.resolved_theme(&window).palette.window_background
        };

        let renderer = match Renderer::new(window.clone(), clear_color, &self.config.fonts) {
            Ok(renderer) => renderer,
            Err(error) => {
                self.fail(event_loop, error);
                return;
            }
        };

        self.window_id = Some(window.id());
        self.renderer = Some(renderer);
        self.window = Some(window);

        if !self.render_hidden_frame(event_loop) {
            return;
        }

        if let Some(window) = self.window.as_ref() {
            window.request_redraw();
            window.set_visible(true);
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
            WindowEvent::ThemeChanged(_) => {
                if !self.config.clear_color_overridden {
                    let clear_color = self
                        .window
                        .as_ref()
                        .map(|window| self.resolved_theme(window).palette.window_background)
                        .unwrap_or(self.config.clear_color);
                    if let Some(renderer) = self.renderer.as_mut() {
                        renderer.set_clear_color(clear_color);
                    }
                }
                if let Some(window) = self.window.as_ref() {
                    window.request_redraw();
                }
            }
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

        self.theme = resolve_theme(&self.active_theme_selection(), window.theme());
        let clear_color =
            if self.window_bindings.clear_color.is_some() || self.config.clear_color_overridden {
                self.config.clear_color
            } else {
                self.theme.palette.window_background
            };

        let renderer = match Renderer::new(window.clone(), clear_color, &self.config.fonts) {
            Ok(renderer) => renderer,
            Err(error) => {
                self.fail(event_loop, error);
                return;
            }
        };

        self.window_id = Some(window.id());
        self.renderer = Some(renderer);
        self.window = Some(window);

        if !self.render_hidden_frame(event_loop) {
            return;
        }

        if let Some(window) = self.window.as_ref() {
            window.request_redraw();
            window.set_visible(true);
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
                let viewport = self.viewport_rect();
                let widget_result = tree.handle_window_event(
                    &self.font_manager,
                    &self.theme,
                    &mut self.animation_engine,
                    viewport,
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
            WindowEvent::ThemeChanged(theme) => {
                self.apply_window_theme(Some(theme));
                self.sync_bindings(Instant::now());
                if let Some(window) = self.window.as_ref() {
                    window.request_redraw();
                }
            }
            WindowEvent::Resized(size) => {
                self.invalidate_scene();
                if let Some(renderer) = self.renderer.as_mut() {
                    renderer.resize(size);
                }

                if let Some(window) = self.window.as_ref() {
                    window.request_redraw();
                }
            }
            WindowEvent::RedrawRequested => match self.render_current_frame() {
                Ok(RenderStatus::Rendered | RenderStatus::SkipFrame) => {}
                Ok(RenderStatus::ReconfigureSurface) => {
                    if let Some(renderer) = self.renderer.as_mut() {
                        renderer.reconfigure();
                    }
                }
                Err(error) => self.fail(event_loop, error),
            },
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let now = Instant::now();
        self.request_redraw_if_dirty(now);
        self.drive_animations(event_loop, now);
    }
}

fn resolve_theme(selection: &ThemeSelection, window_theme: Option<WindowTheme>) -> Theme {
    match selection {
        ThemeSelection::System => Theme::from_window_theme(window_theme),
        ThemeSelection::Fixed(theme) => theme.clone(),
    }
}
