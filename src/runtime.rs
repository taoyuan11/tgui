use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::animation::{
    default_theme_transition, AnimationCoordinator, AnimationEngine, AnimationKey, Transition,
    WindowProperty,
};
#[cfg(all(target_os = "android", feature = "android"))]
use jni::{
    JavaVM, JValue, jni_sig, jni_str,
    objects::JObject,
};
use winit::application::ApplicationHandler;
use winit::event::{ElementState, Ime, MouseButton, MouseScrollDelta, TouchPhase, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, ModifiersState, PhysicalKey};
#[cfg(all(target_os = "android", feature = "android"))]
use winit::platform::android::activity::AndroidApp;
#[cfg(all(target_os = "android", feature = "android"))]
use winit::platform::android::activity::ndk::configuration::UiModeNight;
#[cfg(all(target_os = "android", feature = "android"))]
use winit::platform::android::EventLoopBuilderExtAndroid;
use winit::window::{
    Cursor, CursorIcon, ImePurpose, Theme as WindowTheme, Window, WindowAttributes, WindowId,
};

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
    HitInteraction, InputEditState, InputSnapshot, Point, Rect, RenderedWidgetScene,
    ScenePrimitives, ScrollbarAxis, ScrollbarHandle, WidgetId, WidgetTree,
};
use unicode_segmentation::UnicodeSegmentation;

const DOUBLE_CLICK_THRESHOLD: Duration = Duration::from_millis(300);
const CARET_BLINK_INTERVAL: Duration = Duration::from_millis(500);
#[cfg(all(target_os = "android", feature = "android"))]
const ANDROID_SYSTEM_THEME_POLL_INTERVAL: Duration = Duration::from_millis(500);

#[cfg(all(target_os = "android", feature = "android"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SystemBarStyle {
    color: Color,
    use_dark_icons: bool,
}

#[cfg(all(target_os = "android", feature = "android"))]
impl SystemBarStyle {
    fn from_theme(theme: &Theme) -> Self {
        let color = theme.palette.window_background;
        Self {
            color,
            use_dark_icons: is_light_color(color),
        }
    }
}

pub struct Runtime {
    event_loop: EventLoop<()>,
    config: ApplicationConfig,
    #[cfg(all(target_os = "android", feature = "android"))]
    android_app: Option<AndroidApp>,
}

impl Runtime {
    pub fn new(config: ApplicationConfig) -> Result<Self, TguiError> {
        let event_loop = build_event_loop(ControlFlow::Poll)?;
        Ok(Self {
            event_loop,
            config,
            #[cfg(all(target_os = "android", feature = "android"))]
            android_app: None,
        })
    }

    #[cfg(all(target_os = "android", feature = "android"))]
    pub fn new_android(config: ApplicationConfig, app: AndroidApp) -> Result<Self, TguiError> {
        let event_loop = build_event_loop_with_android_app(ControlFlow::Poll, app.clone())?;
        Ok(Self {
            event_loop,
            config,
            android_app: Some(app),
        })
    }

    pub fn run(self) -> Result<(), TguiError> {
        let mut handler = RuntimeHandler::new(
            self.config,
            #[cfg(all(target_os = "android", feature = "android"))]
            self.android_app,
        );
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
    animations: AnimationCoordinator,
    #[cfg(all(target_os = "android", feature = "android"))]
    android_app: Option<AndroidApp>,
}

impl<VM: ViewModel> BoundRuntime<VM> {
    pub fn new(
        config: ApplicationConfig,
        view_model: VM,
        window_bindings: WindowBindings,
        widget_tree: Option<WidgetTree<VM>>,
        commands: Vec<WindowCommand<VM>>,
        invalidation: InvalidationSignal,
        animations: AnimationCoordinator,
    ) -> Result<Self, TguiError> {
        let event_loop = build_event_loop(ControlFlow::Wait)?;
        Ok(Self {
            event_loop,
            config,
            view_model,
            window_bindings,
            widget_tree,
            commands,
            invalidation,
            animations,
            #[cfg(all(target_os = "android", feature = "android"))]
            android_app: None,
        })
    }

    #[cfg(all(target_os = "android", feature = "android"))]
    pub fn new_android(
        config: ApplicationConfig,
        view_model: VM,
        window_bindings: WindowBindings,
        widget_tree: Option<WidgetTree<VM>>,
        commands: Vec<WindowCommand<VM>>,
        invalidation: InvalidationSignal,
        animations: AnimationCoordinator,
        app: AndroidApp,
    ) -> Result<Self, TguiError> {
        let event_loop = build_event_loop_with_android_app(ControlFlow::Wait, app.clone())?;
        Ok(Self {
            event_loop,
            config,
            view_model,
            window_bindings,
            widget_tree,
            commands,
            invalidation,
            animations,
            android_app: Some(app),
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
            self.animations,
            #[cfg(all(target_os = "android", feature = "android"))]
            self.android_app,
        );
        self.event_loop.run_app(&mut handler)?;

        if let Some(error) = handler.error {
            return Err(error);
        }

        Ok(())
    }
}

fn build_event_loop(control_flow: ControlFlow) -> Result<EventLoop<()>, TguiError> {
    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(control_flow);
    Ok(event_loop)
}

#[cfg(all(target_os = "android", feature = "android"))]
fn build_event_loop_with_android_app(
    control_flow: ControlFlow,
    app: AndroidApp,
) -> Result<EventLoop<()>, TguiError> {
    let mut builder = EventLoop::builder();
    builder.with_android_app(app);
    let event_loop = builder.build()?;
    event_loop.set_control_flow(control_flow);
    Ok(event_loop)
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
    #[cfg(all(target_os = "android", feature = "android"))]
    android_app: Option<AndroidApp>,
    #[cfg(all(target_os = "android", feature = "android"))]
    system_bar_style: Option<SystemBarStyle>,
}

impl RuntimeHandler {
    fn new(
        config: ApplicationConfig,
        #[cfg(all(target_os = "android", feature = "android"))] android_app: Option<AndroidApp>,
    ) -> Self {
        Self {
            config,
            window: None,
            renderer: None,
            window_id: None,
            error: None,
            #[cfg(all(target_os = "android", feature = "android"))]
            android_app,
            #[cfg(all(target_os = "android", feature = "android"))]
            system_bar_style: None,
        }
    }

    fn fail(&mut self, event_loop: &ActiveEventLoop, error: TguiError) {
        self.error = Some(error);
        event_loop.exit();
    }

    fn resolved_theme(&self, window: &Window) -> Theme {
        resolve_theme(
            &self.config.theme,
            resolve_window_theme(
                Some(window),
                #[cfg(all(target_os = "android", feature = "android"))]
                self.android_app.as_ref(),
            ),
        )
    }

    #[cfg(all(target_os = "android", feature = "android"))]
    fn sync_system_bar_style(&mut self, theme: &Theme) {
        let Some(app) = self.android_app.as_ref() else {
            return;
        };
        let style = SystemBarStyle::from_theme(theme);
        if self.system_bar_style == Some(style) {
            return;
        }

        if let Err(error) = apply_android_system_bar_style(app, style) {
            eprintln!("failed to sync Android system bar style: {error}");
            return;
        }

        self.system_bar_style = Some(style);
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

    fn resume_existing_window(&mut self, event_loop: &ActiveEventLoop) {
        let Some(window) = self.window.clone() else {
            return;
        };

        let clear_color = if self.config.clear_color_overridden {
            self.config.clear_color
        } else {
            self.resolved_theme(&window).palette.window_background
        };

        match Renderer::new(window.clone(), clear_color, &self.config.fonts) {
            Ok(renderer) => self.renderer = Some(renderer),
            Err(error) => {
                self.fail(event_loop, error);
                return;
            }
        }

        #[cfg(all(target_os = "android", feature = "android"))]
        {
            let theme = self.resolved_theme(&window);
            self.sync_system_bar_style(&theme);
        }

        if !self.render_hidden_frame(event_loop) {
            return;
        }

        window.request_redraw();
        window.set_visible(true);
    }

    fn suspend(&mut self) {
        self.renderer = None;
        #[cfg(all(target_os = "android", feature = "android"))]
        {
            self.system_bar_style = None;
        }
    }

    fn handle_runtime_redraw(&mut self, event_loop: &ActiveEventLoop) {
        let Some(renderer) = self.renderer.as_mut() else {
            return;
        };

        match renderer.render(&ScenePrimitives::default()) {
            Ok(RenderStatus::Rendered | RenderStatus::SkipFrame) => {}
            Ok(RenderStatus::ReconfigureSurface) => {
                renderer.reconfigure();
                match renderer.render(&ScenePrimitives::default()) {
                    Ok(RenderStatus::Rendered | RenderStatus::SkipFrame) => {}
                    Ok(RenderStatus::ReconfigureSurface) => {}
                    Err(error) => self.fail(event_loop, error),
                }
            }
            Err(error) => self.fail(event_loop, error),
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
    animations: AnimationCoordinator,
    animation_engine: AnimationEngine,
    animation_epoch: u64,
    caret_blink_started_at: Option<Instant>,
    cursor_position: Option<Point>,
    modifiers: ModifiersState,
    hovered_widgets: Vec<HoveredWidget<VM>>,
    hovered_scrollbar: Option<ScrollbarHandle>,
    active_scrollbar_drag: Option<ScrollbarDrag>,
    pending_click: Option<PendingClick<VM>>,
    focused_widget: Option<FocusedWidget<VM>>,
    focused_input: Option<WidgetId>,
    input_states: HashMap<WidgetId, InputEditState>,
    clipboard: ClipboardService,
    cached_scene: Option<CachedScene>,
    scroll_states: HashMap<WidgetId, Point>,
    scroll_epoch: u64,
    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
    window_id: Option<WindowId>,
    error: Option<TguiError>,
    #[cfg(all(target_os = "android", feature = "android"))]
    android_app: Option<AndroidApp>,
    #[cfg(all(target_os = "android", feature = "android"))]
    system_bar_style: Option<SystemBarStyle>,
}

struct CachedScene {
    viewport: Rect,
    focused_input: Option<WidgetId>,
    caret_visible: bool,
    animation_epoch: u64,
    scroll_epoch: u64,
    rendered: RenderedWidgetScene,
}

struct PendingClick<VM> {
    widget_id: WidgetId,
    deadline: Instant,
    command: Option<Command<VM>>,
}

struct FocusedWidget<VM> {
    widget_id: WidgetId,
    on_blur: Option<Command<VM>>,
}

struct HoveredWidget<VM> {
    widget_id: WidgetId,
    cursor_style: Option<crate::ui::widget::CursorStyle>,
    on_mouse_enter: Option<Command<VM>>,
    on_mouse_leave: Option<Command<VM>>,
    on_mouse_move: Option<crate::foundation::view_model::ValueCommand<VM, Point>>,
}

#[derive(Clone, Copy)]
struct ScrollbarDrag {
    handle: ScrollbarHandle,
    start_cursor: Point,
    start_scroll_offset: Point,
    track: Rect,
    thumb: Rect,
    max_offset: f32,
}

#[derive(Default)]
struct ClipboardService {
    #[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
    inner: Option<arboard::Clipboard>,
}

impl ClipboardService {
    fn get_text(&mut self) -> Option<String> {
        #[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
        {
            if self.inner.is_none() {
                self.inner = arboard::Clipboard::new().ok();
            }
            return self
                .inner
                .as_mut()
                .and_then(|clipboard| clipboard.get_text().ok());
        }

        #[allow(unreachable_code)]
        None
    }

    fn set_text(&mut self, _text: String) {
        #[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
        {
            if self.inner.is_none() {
                self.inner = arboard::Clipboard::new().ok();
            }
            if let Some(clipboard) = self.inner.as_mut() {
                let _ = clipboard.set_text(_text);
            }
        }
    }
}

impl<VM> BoundRuntimeHandler<VM> {
    fn new(
        config: ApplicationConfig,
        view_model: VM,
        window_bindings: WindowBindings,
        widget_tree: Option<WidgetTree<VM>>,
        commands: Vec<WindowCommand<VM>>,
        invalidation: InvalidationSignal,
        animations: AnimationCoordinator,
        #[cfg(all(target_os = "android", feature = "android"))] android_app: Option<AndroidApp>,
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
            animations,
            animation_engine: AnimationEngine::default(),
            animation_epoch: 0,
            caret_blink_started_at: None,
            cursor_position: None,
            modifiers: ModifiersState::default(),
            hovered_widgets: Vec::new(),
            hovered_scrollbar: None,
            active_scrollbar_drag: None,
            pending_click: None,
            focused_widget: None,
            focused_input: None,
            input_states: HashMap::new(),
            clipboard: ClipboardService::default(),
            cached_scene: None,
            scroll_states: HashMap::new(),
            scroll_epoch: 0,
            window: None,
            renderer: None,
            window_id: None,
            error: None,
            #[cfg(all(target_os = "android", feature = "android"))]
            android_app,
            #[cfg(all(target_os = "android", feature = "android"))]
            system_bar_style: None,
        }
    }

    fn fail(&mut self, event_loop: &ActiveEventLoop, error: TguiError) {
        self.error = Some(error);
        event_loop.exit();
    }

    #[cfg(all(target_os = "android", feature = "android"))]
    fn sync_system_bar_style(&mut self, theme: &Theme) {
        let Some(app) = self.android_app.as_ref() else {
            return;
        };
        let style = SystemBarStyle::from_theme(theme);
        if self.system_bar_style == Some(style) {
            return;
        }

        if let Err(error) = apply_android_system_bar_style(app, style) {
            eprintln!("failed to sync Android system bar style: {error}");
            return;
        }

        self.system_bar_style = Some(style);
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
            self.apply_theme(resolve_theme(
                &self.active_theme_selection(),
                resolve_window_theme(
                    self.window.as_deref(),
                    #[cfg(all(target_os = "android", feature = "android"))]
                    self.android_app.as_ref(),
                )
                .or(window_theme),
            ));
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
            resolve_window_theme(
                self.window.as_deref(),
                #[cfg(all(target_os = "android", feature = "android"))]
                self.android_app.as_ref(),
            ),
        );
        if self.theme != resolved_theme {
            self.apply_theme(resolved_theme);
        }
    }

    fn refresh_platform_theme(&mut self) -> bool {
        let previous_theme = self.theme.clone();
        self.sync_theme_binding();
        self.theme != previous_theme
    }

    fn sync_bindings(&mut self, now: Instant) {
        self.sync_theme_binding();
        #[cfg(all(target_os = "android", feature = "android"))]
        {
            let theme = self.theme.clone();
            self.sync_system_bar_style(&theme);
        }

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

    fn render_primitives(&mut self) -> RenderedWidgetScene {
        let viewport = self.viewport_rect();
        let caret_visible = self.input_caret_visible(Instant::now());
        let focused_input_state = self
            .focused_input
            .and_then(|id| self.focused_input_state(id))
            .cloned();
        if let Some(cached) = self.cached_scene.as_ref() {
            if cached.viewport == viewport
                && cached.focused_input == self.focused_input
                && cached.caret_visible == caret_visible
                && cached.animation_epoch == self.animation_epoch
                && cached.scroll_epoch == self.scroll_epoch
            {
                return cached.rendered.clone();
            }
        }

        let theme = self.animated_theme(Instant::now());
        let rendered = match self.widget_tree.as_ref() {
            Some(tree) => tree.render_output(
                &self.font_manager,
                &theme,
                &mut self.animation_engine,
                self.hovered_scrollbar,
                self.active_scrollbar_drag.map(|drag| drag.handle),
                &self.scroll_states,
                viewport,
                self.focused_input,
                focused_input_state.as_ref(),
                caret_visible,
            ),
            None => RenderedWidgetScene::default(),
        };
        self.cached_scene = Some(CachedScene {
            viewport,
            focused_input: self.focused_input,
            caret_visible,
            animation_epoch: self.animation_epoch,
            scroll_epoch: self.scroll_epoch,
            rendered: rendered.clone(),
        });
        rendered
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
            WindowEvent::CursorMoved { .. }
                | WindowEvent::MouseWheel { .. }
                | WindowEvent::Touch(_)
                | WindowEvent::MouseInput {
                    state: ElementState::Pressed,
                    button: MouseButton::Left,
                    ..
                }
                | WindowEvent::KeyboardInput { .. }
        )
    }

    fn render_current_frame(&mut self) -> Result<RenderStatus, TguiError> {
        // Android can deliver a redraw before a replacement surface is ready.
        // In that case we simply skip the frame and wait for the next resume/redraw.
        if self.renderer.is_none() {
            return Ok(RenderStatus::SkipFrame);
        }

        self.sync_bindings(Instant::now());
        let rendered = self.render_primitives();
        if let (Some(window), Some(caret_rect)) = (self.window.as_ref(), rendered.ime_cursor_area) {
            window.set_ime_cursor_area(
                winit::dpi::PhysicalPosition::new(caret_rect.x as i32, caret_rect.y as i32),
                winit::dpi::PhysicalSize::new(
                    caret_rect.width.ceil().max(1.0) as u32,
                    caret_rect.height.ceil().max(1.0) as u32,
                ),
            );
        }
        let renderer = self
            .renderer
            .as_mut()
            .expect("renderer should exist before drawing");
        renderer.render(&rendered.primitives)
    }

    #[cfg(all(target_os = "android", feature = "android"))]
    fn render_immediately(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() || self.renderer.is_none() {
            return;
        }

        match self.render_current_frame() {
            Ok(RenderStatus::Rendered | RenderStatus::SkipFrame) => {}
            Ok(RenderStatus::ReconfigureSurface) => {
                if let Some(renderer) = self.renderer.as_mut() {
                    renderer.reconfigure();
                }
                match self.render_current_frame() {
                    Ok(RenderStatus::Rendered | RenderStatus::SkipFrame) => {}
                    Ok(RenderStatus::ReconfigureSurface) => {}
                    Err(error) => self.fail(event_loop, error),
                }
            }
            Err(error) => self.fail(event_loop, error),
        }
    }

    fn set_pointer_position(&mut self, position: winit::dpi::PhysicalPosition<f64>) {
        self.cursor_position = Some(Point {
            x: position.x as f32,
            y: position.y as f32,
        });
    }

    fn clear_pointer_position(&mut self) {
        self.cursor_position = None;
        for hovered in std::mem::take(&mut self.hovered_widgets).into_iter().rev() {
            if let Some(command) = hovered.on_mouse_leave {
                command.execute(&mut self.view_model);
                self.invalidate_scene();
                self.invalidation.mark_dirty();
            }
        }
        self.hovered_scrollbar = self.active_scrollbar_drag.map(|drag| drag.handle);
        self.update_cursor_icon();
    }

    fn drive_animations(&mut self, event_loop: &ActiveEventLoop, now: Instant) -> bool {
        self.flush_pending_click_if_due(now);

        let mut frame_advanced = false;
        if self.animations.refresh(now) {
            frame_advanced = true;
            self.animation_epoch = self.animation_epoch.wrapping_add(1);
            self.invalidate_scene();
            if let Some(window) = self.window.as_ref() {
                window.request_redraw();
            }
        }

        if self.animation_engine.refresh(now) {
            frame_advanced = true;
            self.animation_epoch = self.animation_epoch.wrapping_add(1);
            self.invalidate_scene();
            if let Some(window) = self.window.as_ref() {
                window.request_redraw();
            }
        }

        if let Some(deadline) = self.next_deadline(now) {
            event_loop.set_control_flow(ControlFlow::WaitUntil(deadline));
            if let Some(window) = self.window.as_ref() {
                window.request_redraw();
            }
        } else {
            #[cfg(all(target_os = "android", feature = "android"))]
            if self.uses_system_theme() {
                event_loop.set_control_flow(ControlFlow::WaitUntil(
                    now + ANDROID_SYSTEM_THEME_POLL_INTERVAL,
                ));
                return frame_advanced;
            }

            event_loop.set_control_flow(ControlFlow::Wait);
        }

        frame_advanced
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

    fn resume_existing_window(&mut self, event_loop: &ActiveEventLoop) {
        let Some(window) = self.window.clone() else {
            return;
        };

        self.sync_theme_binding();
        self.invalidate_scene();
        let clear_color =
            if self.window_bindings.clear_color.is_some() || self.config.clear_color_overridden {
                self.config.clear_color
            } else {
                self.theme.palette.window_background
            };

        match Renderer::new(window.clone(), clear_color, &self.config.fonts) {
            Ok(renderer) => self.renderer = Some(renderer),
            Err(error) => {
                self.fail(event_loop, error);
                return;
            }
        }

        #[cfg(all(target_os = "android", feature = "android"))]
        {
            let theme = self.theme.clone();
            self.sync_system_bar_style(&theme);
        }

        if !self.render_hidden_frame(event_loop) {
            return;
        }

        window.request_redraw();
        window.set_visible(true);
    }

    fn suspend(&mut self) {
        self.renderer = None;
        self.cached_scene = None;
        #[cfg(all(target_os = "android", feature = "android"))]
        {
            self.system_bar_style = None;
        }
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

    fn next_deadline(&self, now: Instant) -> Option<Instant> {
        let animation_deadline = self.animation_engine.next_frame_deadline(now);
        let controller_deadline = self.animations.next_frame_deadline(now);
        let caret_deadline = self.next_caret_blink_deadline(now);
        let click_deadline = self.pending_click.as_ref().map(|pending| pending.deadline);
        [
            animation_deadline,
            controller_deadline,
            caret_deadline,
            click_deadline,
        ]
        .into_iter()
        .flatten()
        .min()
    }

    fn reset_caret_blink(&mut self, now: Instant) {
        if self.focused_input.is_some() {
            self.caret_blink_started_at = Some(now);
            self.invalidate_scene();
        }
    }

    fn input_caret_visible(&self, now: Instant) -> bool {
        let Some(_) = self.focused_input else {
            return false;
        };
        let Some(started_at) = self.caret_blink_started_at else {
            return true;
        };
        let phase = now.saturating_duration_since(started_at).as_millis()
            / CARET_BLINK_INTERVAL.as_millis().max(1);
        phase % 2 == 0
    }

    fn next_caret_blink_deadline(&self, now: Instant) -> Option<Instant> {
        let Some(_) = self.focused_input else {
            return None;
        };
        let Some(started_at) = self.caret_blink_started_at else {
            return Some(now + CARET_BLINK_INTERVAL);
        };
        let interval_ms = CARET_BLINK_INTERVAL.as_millis().max(1);
        let elapsed_ms = now.saturating_duration_since(started_at).as_millis();
        let next_boundary_ms = ((elapsed_ms / interval_ms) + 1) * interval_ms;
        Some(started_at + Duration::from_millis(next_boundary_ms as u64))
    }

    fn flush_pending_click_if_due(&mut self, now: Instant) {
        let should_flush = self
            .pending_click
            .as_ref()
            .map(|pending| pending.deadline <= now)
            .unwrap_or(false);
        if !should_flush {
            return;
        }

        if let Some(pending) = self.pending_click.take() {
            if let Some(command) = pending.command {
                command.execute(&mut self.view_model);
                self.invalidate_scene();
                self.invalidation.mark_dirty();
            }
        }
    }

    fn focused_input_state(&self, id: WidgetId) -> Option<&InputEditState> {
        self.input_states.get(&id)
    }

    fn focused_input_snapshot(&self) -> Option<InputSnapshot<VM>> {
        let id = self.focused_input?;
        self.widget_tree.as_ref()?.input_snapshot(id)
    }

    fn sync_ime_allowed(&self) {
        if let Some(window) = self.window.as_ref() {
            let ime_allowed = self.focused_input.is_some();
            window.set_ime_allowed(ime_allowed);
            if ime_allowed {
                window.set_ime_purpose(ImePurpose::Normal);
            }
        }
    }

    fn ensure_input_state(&mut self, widget_id: WidgetId, text: &str) -> &mut InputEditState {
        self.input_states
            .entry(widget_id)
            .and_modify(|state| *state = state.clone().clamped_to(text))
            .or_insert_with(|| InputEditState::caret_at(text))
    }

    fn update_input_state(
        &mut self,
        widget_id: WidgetId,
        text: &str,
        update: impl FnOnce(&mut InputEditState),
    ) {
        let state = self.ensure_input_state(widget_id, text);
        update(state);
        *state = state.clone().clamped_to(text);
        if self.focused_input == Some(widget_id) {
            self.reset_caret_blink(Instant::now());
        }
        self.invalidate_scene();
    }

    fn set_input_focus_state(&mut self, widget_id: WidgetId, text: &str) {
        let state = self.ensure_input_state(widget_id, text);
        *state = InputEditState::caret_at(text);
        self.caret_blink_started_at = Some(Instant::now());
        self.invalidate_scene();
    }

    fn clear_input_composition(&mut self, widget_id: WidgetId, text: &str) {
        if let Some(state) = self.input_states.get_mut(&widget_id) {
            state.composition = None;
            *state = state.clone().clamped_to(text);
            if self.focused_input == Some(widget_id) {
                self.reset_caret_blink(Instant::now());
            }
            self.invalidate_scene();
        }
    }

    fn apply_input_text_change(
        &mut self,
        snapshot: &InputSnapshot<VM>,
        new_text: String,
        new_cursor: usize,
    ) {
        {
            let state = self.ensure_input_state(snapshot.id, &new_text);
            state.cursor = new_cursor.min(new_text.len());
            state.anchor = state.cursor;
            state.composition = None;
        }
        self.reset_caret_blink(Instant::now());

        if let Some(command) = snapshot.on_change.clone() {
            command.execute(&mut self.view_model, new_text);
            self.invalidation.mark_dirty();
        }

        self.invalidate_scene();
        if let Some(window) = self.window.as_ref() {
            window.request_redraw();
        }
    }

    fn insert_input_text(&mut self, snapshot: &InputSnapshot<VM>, inserted: &str) {
        if snapshot.on_change.is_none() || inserted.is_empty() {
            return;
        }

        let sanitized = normalize_single_line_text(inserted);
        if sanitized.is_empty() {
            return;
        }

        let state = self
            .focused_input_state(snapshot.id)
            .cloned()
            .unwrap_or_else(|| InputEditState::caret_at(&snapshot.text))
            .clamped_to(&snapshot.text);
        let replace_range = state
            .selection_range()
            .unwrap_or((state.cursor, state.cursor));
        let mut next_text = snapshot.text.clone();
        next_text.replace_range(replace_range.0..replace_range.1, &sanitized);
        self.apply_input_text_change(snapshot, next_text, replace_range.0 + sanitized.len());
    }

    fn handle_input_ime(&mut self, ime: Ime) {
        let Some(snapshot) = self.focused_input_snapshot() else {
            return;
        };
        let current_text = snapshot.text.clone();
        let state = self
            .focused_input_state(snapshot.id)
            .cloned()
            .unwrap_or_else(|| InputEditState::caret_at(&current_text))
            .clamped_to(&current_text);

        match ime {
            Ime::Enabled => {
                self.sync_ime_allowed();
            }
            Ime::Preedit(text, cursor) => {
                let replace_range = state
                    .composition
                    .as_ref()
                    .map(|composition| composition.replace_range)
                    .unwrap_or_else(|| {
                        state
                            .selection_range()
                            .unwrap_or((state.cursor, state.cursor))
                    });
                self.update_input_state(snapshot.id, &current_text, |edit| {
                    if text.is_empty() {
                        edit.composition = None;
                    } else {
                        edit.composition = Some(crate::ui::widget::CompositionState {
                            replace_range,
                            text,
                            cursor,
                        });
                    }
                });
                if let Some(window) = self.window.as_ref() {
                    window.request_redraw();
                }
            }
            Ime::Commit(text) => {
                let replace_range = state
                    .composition
                    .as_ref()
                    .map(|composition| composition.replace_range)
                    .unwrap_or_else(|| {
                        state
                            .selection_range()
                            .unwrap_or((state.cursor, state.cursor))
                    });
                let sanitized = normalize_single_line_text(&text);
                let mut next_text = current_text.clone();
                next_text.replace_range(replace_range.0..replace_range.1, &sanitized);
                self.apply_input_text_change(
                    &snapshot,
                    next_text,
                    replace_range.0 + sanitized.len(),
                );
            }
            Ime::Disabled => {
                self.clear_input_composition(snapshot.id, &current_text);
            }
        }
    }

    fn handle_input_keyboard_event(&mut self, event: &winit::event::KeyEvent) {
        let Some(snapshot) = self.focused_input_snapshot() else {
            return;
        };

        if event.state != ElementState::Pressed {
            return;
        }

        self.reset_caret_blink(Instant::now());

        let text = snapshot.text.clone();
        let mut state = self
            .focused_input_state(snapshot.id)
            .cloned()
            .unwrap_or_else(|| InputEditState::caret_at(&text))
            .clamped_to(&text);
        let extend_selection = self.modifiers.shift_key();

        if is_primary_shortcut_modifier(self.modifiers) {
            match event.physical_key {
                PhysicalKey::Code(KeyCode::KeyA) => {
                    state.cursor = text.len();
                    state.anchor = 0;
                    self.input_states.insert(snapshot.id, state);
                    self.invalidate_scene();
                }
                PhysicalKey::Code(KeyCode::KeyC) => {
                    if let Some((start, end)) = state.selection_range() {
                        self.clipboard.set_text(text[start..end].to_string());
                    }
                }
                PhysicalKey::Code(KeyCode::KeyX) => {
                    if let Some((start, end)) = state.selection_range() {
                        self.clipboard.set_text(text[start..end].to_string());
                        if snapshot.on_change.is_some() {
                            let mut next_text = text.clone();
                            next_text.replace_range(start..end, "");
                            self.apply_input_text_change(&snapshot, next_text, start);
                        }
                    }
                }
                PhysicalKey::Code(KeyCode::KeyV) => {
                    if let Some(clipboard_text) = self.clipboard.get_text() {
                        self.insert_input_text(&snapshot, &clipboard_text);
                    }
                }
                _ => {}
            }
            return;
        }

        match event.physical_key {
            PhysicalKey::Code(KeyCode::ArrowLeft) => {
                let next = if !extend_selection {
                    state.selection_range().map(|(start, _)| start)
                } else {
                    None
                }
                .unwrap_or_else(|| previous_grapheme_boundary(&text, state.cursor));
                move_cursor(&mut state, next, extend_selection);
            }
            PhysicalKey::Code(KeyCode::ArrowRight) => {
                let next = if !extend_selection {
                    state.selection_range().map(|(_, end)| end)
                } else {
                    None
                }
                .unwrap_or_else(|| next_grapheme_boundary(&text, state.cursor));
                move_cursor(&mut state, next, extend_selection);
            }
            PhysicalKey::Code(KeyCode::Home) => move_cursor(&mut state, 0, extend_selection),
            PhysicalKey::Code(KeyCode::End) => {
                move_cursor(&mut state, text.len(), extend_selection)
            }
            PhysicalKey::Code(KeyCode::Backspace) => {
                if snapshot.on_change.is_none() {
                    return;
                }
                let (start, end) = state.selection_range().unwrap_or_else(|| {
                    let previous = previous_grapheme_boundary(&text, state.cursor);
                    (previous, state.cursor)
                });
                if start == end {
                    return;
                }
                let mut next_text = text.clone();
                next_text.replace_range(start..end, "");
                self.apply_input_text_change(&snapshot, next_text, start);
                return;
            }
            PhysicalKey::Code(KeyCode::Delete) => {
                if snapshot.on_change.is_none() {
                    return;
                }
                let (start, end) = state.selection_range().unwrap_or_else(|| {
                    let next = next_grapheme_boundary(&text, state.cursor);
                    (state.cursor, next)
                });
                if start == end {
                    return;
                }
                let mut next_text = text.clone();
                next_text.replace_range(start..end, "");
                self.apply_input_text_change(&snapshot, next_text, start);
                return;
            }
            _ => {
                if let Some(input) = event.text.as_ref() {
                    self.insert_input_text(&snapshot, input);
                    return;
                }
                return;
            }
        }

        self.input_states.insert(snapshot.id, state);
        self.invalidate_scene();
        if let Some(window) = self.window.as_ref() {
            window.request_redraw();
        }
    }

    fn hit_test(&mut self, viewport: Rect) -> Option<HitInteraction<VM>> {
        self.widget_tree.as_ref()?.hit_test(
            &self.font_manager,
            &self.theme,
            &mut self.animation_engine,
            self.hovered_scrollbar,
            self.active_scrollbar_drag.map(|drag| drag.handle),
            &self.scroll_states,
            viewport,
            self.cursor_position,
            self.focused_input,
        )
    }

    fn hover_path(&mut self, viewport: Rect) -> Vec<HoveredWidget<VM>> {
        self.widget_tree
            .as_ref()
            .map(|tree| {
                tree.hit_path(
                    &self.font_manager,
                    &self.theme,
                    &mut self.animation_engine,
                    self.hovered_scrollbar,
                    self.active_scrollbar_drag.map(|drag| drag.handle),
                    &self.scroll_states,
                    viewport,
                    self.cursor_position,
                    self.focused_input,
                )
                .into_iter()
                .map(|interaction| match interaction {
                    HitInteraction::Widget {
                        id, interactions, ..
                    } => HoveredWidget {
                        widget_id: id,
                        cursor_style: interactions.cursor_style,
                        on_mouse_enter: interactions.on_mouse_enter,
                        on_mouse_leave: interactions.on_mouse_leave,
                        on_mouse_move: interactions.on_mouse_move,
                    },
                    HitInteraction::FocusInput {
                        id, interactions, ..
                    } => HoveredWidget {
                        widget_id: id,
                        cursor_style: interactions
                            .cursor_style
                            .or(Some(crate::ui::widget::CursorStyle::Text)),
                        on_mouse_enter: interactions.on_mouse_enter,
                        on_mouse_leave: interactions.on_mouse_leave,
                        on_mouse_move: interactions.on_mouse_move,
                    },
                })
                .collect()
            })
            .unwrap_or_default()
    }

    fn handle_hover(&mut self, viewport: Rect) {
        let cursor_position = self.cursor_position;
        let next_hovered = self.hover_path(viewport);
        let mut prefix_len = 0usize;
        while prefix_len < self.hovered_widgets.len()
            && prefix_len < next_hovered.len()
            && self.hovered_widgets[prefix_len].widget_id == next_hovered[prefix_len].widget_id
        {
            prefix_len += 1;
        }

        let previous_hovered = std::mem::take(&mut self.hovered_widgets);
        for previous in previous_hovered[prefix_len..].iter().rev() {
            if let Some(command) = previous.on_mouse_leave.clone() {
                command.execute(&mut self.view_model);
                self.invalidate_scene();
                self.invalidation.mark_dirty();
            }
        }

        for hovered in next_hovered[prefix_len..].iter() {
            if let Some(command) = hovered.on_mouse_enter.clone() {
                command.execute(&mut self.view_model);
                self.invalidate_scene();
                self.invalidation.mark_dirty();
            }
        }

        if let Some(position) = cursor_position {
            for hovered in &next_hovered {
                if let Some(command) = hovered.on_mouse_move.clone() {
                    command.execute(&mut self.view_model, position);
                    self.invalidate_scene();
                    self.invalidation.mark_dirty();
                }
            }
        }

        self.hovered_widgets = next_hovered;
        self.sync_scrollbar_hover();
        self.update_cursor_icon();
    }

    fn handle_mouse_wheel(&mut self, delta: MouseScrollDelta) -> bool {
        let Some(cursor_position) = self.cursor_position else {
            return false;
        };

        let mut scroll_delta = mouse_scroll_delta(delta);
        if scroll_delta.x.abs() <= f32::EPSILON && self.modifiers.shift_key() {
            scroll_delta.x = scroll_delta.y;
            scroll_delta.y = 0.0;
        }
        if scroll_delta.x.abs() <= f32::EPSILON && scroll_delta.y.abs() <= f32::EPSILON {
            return false;
        }

        let rendered = self.render_primitives();
        for region in rendered.scroll_regions.iter().rev().copied() {
            if region.visible_frame.is_empty() || !region.visible_frame.contains(cursor_position) {
                continue;
            }

            let max_offset = region.max_offset();
            let mut next_offset = region.scroll_offset;
            if region.can_scroll_x() {
                next_offset.x = (next_offset.x - scroll_delta.x).clamp(0.0, max_offset.x);
            }
            if region.can_scroll_y() {
                next_offset.y = (next_offset.y - scroll_delta.y).clamp(0.0, max_offset.y);
            }

            if (next_offset.x - region.scroll_offset.x).abs() > 0.01
                || (next_offset.y - region.scroll_offset.y).abs() > 0.01
            {
                self.set_scroll_offset(region.id, next_offset);
                return true;
            }
        }

        false
    }

    fn sync_scrollbar_hover(&mut self) {
        let next_hovered = if let Some(drag) = self.active_scrollbar_drag {
            Some(drag.handle)
        } else {
            self.scrollbar_thumb_hit()
        };

        if self.hovered_scrollbar != next_hovered {
            self.hovered_scrollbar = next_hovered;
            self.invalidate_scene();
        }
    }

    fn scrollbar_thumb_hit(&mut self) -> Option<ScrollbarHandle> {
        let cursor_position = self.cursor_position?;
        let rendered = self.render_primitives();
        rendered.scroll_regions.iter().rev().find_map(|region| {
            if region.visible_frame.is_empty() || !region.visible_frame.contains(cursor_position) {
                return None;
            }
            if region
                .vertical_thumb
                .map(|thumb| thumb.contains(cursor_position))
                .unwrap_or(false)
            {
                return Some(ScrollbarHandle {
                    id: region.id,
                    axis: ScrollbarAxis::Vertical,
                });
            }
            if region
                .horizontal_thumb
                .map(|thumb| thumb.contains(cursor_position))
                .unwrap_or(false)
            {
                return Some(ScrollbarHandle {
                    id: region.id,
                    axis: ScrollbarAxis::Horizontal,
                });
            }
            None
        })
    }

    fn begin_scrollbar_drag(&mut self) -> bool {
        let Some(handle) = self.scrollbar_thumb_hit() else {
            return false;
        };
        let Some(cursor_position) = self.cursor_position else {
            return false;
        };
        let rendered = self.render_primitives();
        let Some(region) = rendered
            .scroll_regions
            .iter()
            .copied()
            .find(|region| region.id == handle.id)
        else {
            return false;
        };

        let (track, thumb, max_offset) = match handle.axis {
            ScrollbarAxis::Horizontal => (
                region.horizontal_track,
                region.horizontal_thumb,
                region.max_offset().x,
            ),
            ScrollbarAxis::Vertical => (
                region.vertical_track,
                region.vertical_thumb,
                region.max_offset().y,
            ),
        };
        let (Some(track), Some(thumb)) = (track, thumb) else {
            return false;
        };

        self.active_scrollbar_drag = Some(ScrollbarDrag {
            handle,
            start_cursor: cursor_position,
            start_scroll_offset: region.scroll_offset,
            track,
            thumb,
            max_offset,
        });
        self.hovered_scrollbar = Some(handle);
        self.invalidate_scene();
        true
    }

    fn handle_scrollbar_drag(&mut self) -> bool {
        let Some(drag) = self.active_scrollbar_drag else {
            return false;
        };
        let Some(cursor_position) = self.cursor_position else {
            return false;
        };

        let (travel, delta) = match drag.handle.axis {
            ScrollbarAxis::Horizontal => (
                (drag.track.width - drag.thumb.width).max(0.0),
                cursor_position.x - drag.start_cursor.x,
            ),
            ScrollbarAxis::Vertical => (
                (drag.track.height - drag.thumb.height).max(0.0),
                cursor_position.y - drag.start_cursor.y,
            ),
        };

        let mut next_offset = drag.start_scroll_offset;
        let axis_offset = if travel <= 0.0 || drag.max_offset <= 0.0 {
            0.0
        } else {
            (delta / travel) * drag.max_offset
        };

        match drag.handle.axis {
            ScrollbarAxis::Horizontal => {
                next_offset.x =
                    (drag.start_scroll_offset.x + axis_offset).clamp(0.0, drag.max_offset)
            }
            ScrollbarAxis::Vertical => {
                next_offset.y =
                    (drag.start_scroll_offset.y + axis_offset).clamp(0.0, drag.max_offset)
            }
        }

        let previous = self
            .scroll_states
            .get(&drag.handle.id)
            .copied()
            .unwrap_or(Point::ZERO);
        if (previous.x - next_offset.x).abs() > 0.01 || (previous.y - next_offset.y).abs() > 0.01 {
            self.set_scroll_offset(drag.handle.id, next_offset);
            return true;
        }

        false
    }

    fn end_scrollbar_drag(&mut self) -> bool {
        if self.active_scrollbar_drag.take().is_none() {
            return false;
        }
        self.sync_scrollbar_hover();
        self.invalidate_scene();
        true
    }

    fn update_cursor_icon(&self) {
        if let Some(window) = self.window.as_ref() {
            let cursor = if self.active_scrollbar_drag.is_some() {
                Cursor::Icon(CursorIcon::Pointer)
            } else if self.hovered_scrollbar.is_some() {
                Cursor::Icon(CursorIcon::Pointer)
            } else if let Some(cursor_style) = self
                .hovered_widgets
                .last()
                .and_then(|hovered| hovered.cursor_style)
            {
                Cursor::Icon(cursor_icon(cursor_style))
            } else {
                Cursor::Icon(CursorIcon::Default)
            };
            window.set_cursor(cursor);
        }
    }

    fn set_scroll_offset(&mut self, widget_id: WidgetId, offset: Point) {
        if offset.x.abs() <= 0.01 && offset.y.abs() <= 0.01 {
            self.scroll_states.remove(&widget_id);
        } else {
            self.scroll_states.insert(widget_id, offset);
        }
        self.scroll_epoch = self.scroll_epoch.wrapping_add(1);
        self.invalidate_scene();
        self.invalidation.mark_dirty();
    }

    fn update_focus(
        &mut self,
        next_widget: Option<FocusedWidget<VM>>,
        next_input: Option<WidgetId>,
        on_focus: Option<Command<VM>>,
        input_text: Option<&str>,
    ) {
        let current_id = self
            .focused_widget
            .as_ref()
            .map(|focused| focused.widget_id);
        let next_id = next_widget.as_ref().map(|focused| focused.widget_id);

        if current_id == next_id {
            self.focused_input = next_input;
            self.caret_blink_started_at = next_input.map(|_| Instant::now());
            self.sync_ime_allowed();
            return;
        }

        if let Some(previous_input) = self.focused_input {
            if let Some(snapshot) = self
                .widget_tree
                .as_ref()
                .and_then(|tree| tree.input_snapshot(previous_input))
            {
                self.clear_input_composition(previous_input, &snapshot.text);
            }
        }

        let mut fired_handler = false;
        if let Some(previous) = self.focused_widget.take() {
            if let Some(command) = previous.on_blur {
                command.execute(&mut self.view_model);
                fired_handler = true;
            }
        }

        self.focused_widget = next_widget;
        self.focused_input = next_input;
        self.caret_blink_started_at = next_input.map(|_| Instant::now());
        if let (Some(input_id), Some(text)) = (next_input, input_text) {
            self.set_input_focus_state(input_id, text);
        }
        self.sync_ime_allowed();

        if let Some(command) = on_focus {
            if next_id.is_some() {
                command.execute(&mut self.view_model);
                fired_handler = true;
            }
        }

        if fired_handler {
            self.invalidate_scene();
            self.invalidation.mark_dirty();
        }
    }

    fn handle_mouse_press(&mut self, viewport: Rect, now: Instant) {
        self.flush_pending_click_if_due(now);

        let hit = self.hit_test(viewport);
        let Some(hit) = hit else {
            self.update_focus(None, None, None, None);
            self.pending_click = None;
            return;
        };

        let (widget_id, interactions, focus_target, focus_input, focus_command, input_text) =
            match hit {
                HitInteraction::Widget {
                    id,
                    interactions,
                    focusable,
                } => (
                    id,
                    interactions.clone(),
                    focusable.then_some(id),
                    None,
                    focusable.then_some(interactions.on_focus.clone()).flatten(),
                    None,
                ),
                HitInteraction::FocusInput {
                    id,
                    interactions,
                    text,
                    ..
                } => (
                    id,
                    interactions.clone(),
                    Some(id),
                    Some(id),
                    interactions.on_focus.clone(),
                    Some(text),
                ),
            };

        self.update_focus(
            focus_target.map(|id| FocusedWidget {
                widget_id: id,
                on_blur: interactions.on_blur.clone(),
            }),
            focus_input,
            focus_command,
            input_text.as_deref(),
        );

        let is_double_click = self
            .pending_click
            .as_ref()
            .map(|pending| pending.widget_id == widget_id && pending.deadline > now)
            .unwrap_or(false);

        if is_double_click {
            self.pending_click = None;
            if let Some(command) = interactions.on_double_click.or(interactions.on_click) {
                command.execute(&mut self.view_model);
                self.invalidate_scene();
                self.invalidation.mark_dirty();
            }
            return;
        }

        if interactions.on_double_click.is_some() {
            self.pending_click = Some(PendingClick {
                widget_id,
                deadline: now + DOUBLE_CLICK_THRESHOLD,
                command: interactions.on_click,
            });
        } else if let Some(command) = interactions.on_click {
            command.execute(&mut self.view_model);
            self.invalidate_scene();
            self.invalidation.mark_dirty();
        } else {
            self.pending_click = None;
        }
    }
}

impl ApplicationHandler for RuntimeHandler {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            self.resume_existing_window(event_loop);
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
        #[cfg(all(target_os = "android", feature = "android"))]
        {
            let theme = self.resolved_theme(&window);
            self.sync_system_bar_style(&theme);
        }

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
                    let resolved_theme = self
                        .window
                        .as_ref()
                        .map(|window| self.resolved_theme(window));
                    let clear_color = resolved_theme
                        .as_ref()
                        .map(|theme| theme.palette.window_background)
                        .unwrap_or(self.config.clear_color);
                    if let Some(renderer) = self.renderer.as_mut() {
                        renderer.set_clear_color(clear_color);
                    }
                    #[cfg(all(target_os = "android", feature = "android"))]
                    if let Some(theme) = resolved_theme.as_ref() {
                        self.sync_system_bar_style(theme);
                    }
                }
                if let Some(window) = self.window.as_ref() {
                    window.request_redraw();
                }
            }
            WindowEvent::ScaleFactorChanged { .. } => {
                if let Some(window) = self.window.clone() {
                    if !self.config.clear_color_overridden {
                        let theme = self.resolved_theme(&window);
                        if let Some(renderer) = self.renderer.as_mut() {
                            renderer.set_clear_color(theme.palette.window_background);
                            renderer.resize(window.inner_size());
                        }
                        #[cfg(all(target_os = "android", feature = "android"))]
                        self.sync_system_bar_style(&theme);
                    } else if let Some(renderer) = self.renderer.as_mut() {
                        renderer.resize(window.inner_size());
                    }
                    window.request_redraw();
                }
            }
            WindowEvent::Resized(size) => {
                if let Some(renderer) = self.renderer.as_mut() {
                    renderer.resize(size);
                }
            }
            WindowEvent::RedrawRequested => self.handle_runtime_redraw(event_loop),
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(window) = self.window.as_ref() {
            window.request_redraw();
        }
    }

    fn suspended(&mut self, _event_loop: &ActiveEventLoop) {
        self.suspend();
    }
}

impl<VM: ViewModel> ApplicationHandler for BoundRuntimeHandler<VM> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            self.resume_existing_window(event_loop);
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

        self.theme = resolve_theme(
            &self.active_theme_selection(),
            resolve_window_theme(
                Some(&window),
                #[cfg(all(target_os = "android", feature = "android"))]
                self.android_app.as_ref(),
            ),
        );
        #[cfg(all(target_os = "android", feature = "android"))]
        {
            let theme = self.theme.clone();
            self.sync_system_bar_style(&theme);
        }
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
            self.set_pointer_position(*position);
        }

        if let WindowEvent::Touch(touch) = &event {
            self.set_pointer_position(touch.location);
        }

        if let WindowEvent::ModifiersChanged(modifiers) = &event {
            self.modifiers = modifiers.state();
        }

        if matches!(event, WindowEvent::CursorLeft { .. }) {
            self.clear_pointer_position();
        }

        if Self::should_dispatch_widget_event(&event) {
            let viewport = self.viewport_rect();
            let previous_focus = self.focused_input;

            match &event {
                WindowEvent::CursorMoved { .. } => {
                    if self.active_scrollbar_drag.is_some() {
                        self.handle_scrollbar_drag();
                        self.sync_scrollbar_hover();
                        self.update_cursor_icon();
                    } else {
                        self.handle_hover(viewport);
                    }
                }
                WindowEvent::MouseWheel { delta, .. } => {
                    self.handle_mouse_wheel(*delta);
                }
                WindowEvent::Touch(touch) => match touch.phase {
                    TouchPhase::Started => {
                        if !self.begin_scrollbar_drag() {
                            self.handle_mouse_press(viewport, Instant::now());
                        } else {
                            self.update_cursor_icon();
                        }
                    }
                    TouchPhase::Moved => {
                        if self.active_scrollbar_drag.is_some() {
                            self.handle_scrollbar_drag();
                            self.sync_scrollbar_hover();
                            self.update_cursor_icon();
                        } else {
                            self.handle_hover(viewport);
                        }
                    }
                    TouchPhase::Ended | TouchPhase::Cancelled => {}
                },
                WindowEvent::MouseInput {
                    state: ElementState::Pressed,
                    button: MouseButton::Left,
                    ..
                } => {
                    if !self.begin_scrollbar_drag() {
                        self.handle_mouse_press(viewport, Instant::now());
                    } else {
                        self.update_cursor_icon();
                    }
                }
                WindowEvent::KeyboardInput { event, .. } => {
                    self.handle_input_keyboard_event(event);
                }
                _ => {}
            }

            if self.focused_input != previous_focus {
                self.invalidate_scene();
            }

            if let Some(window) = self.window.as_ref() {
                window.request_redraw();
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
            WindowEvent::Focused(false) => {
                self.end_scrollbar_drag();
                self.update_focus(None, None, None, None);
                if let Some(window) = self.window.as_ref() {
                    window.request_redraw();
                }
            }
            WindowEvent::ThemeChanged(theme) => {
                self.apply_window_theme(Some(theme));
                self.sync_bindings(Instant::now());
                if let Some(window) = self.window.as_ref() {
                    window.request_redraw();
                }
            }
            WindowEvent::ScaleFactorChanged { .. } => {
                self.apply_window_theme(None);
                self.invalidate_scene();
                if let Some(window) = self.window.as_ref() {
                    if let Some(renderer) = self.renderer.as_mut() {
                        renderer.resize(window.inner_size());
                    }
                    window.request_redraw();
                }
            }
            WindowEvent::Ime(ime) => {
                self.handle_input_ime(ime);
            }
            WindowEvent::MouseInput {
                state: ElementState::Released,
                button: MouseButton::Left,
                ..
            } => {
                self.end_scrollbar_drag();
                if let Some(window) = self.window.as_ref() {
                    window.request_redraw();
                }
            }
            WindowEvent::Touch(touch)
                if matches!(touch.phase, TouchPhase::Ended | TouchPhase::Cancelled) =>
            {
                self.end_scrollbar_drag();
                self.clear_pointer_position();
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
            WindowEvent::RedrawRequested => {
                match self.render_current_frame() {
                Ok(RenderStatus::Rendered | RenderStatus::SkipFrame) => {}
                Ok(RenderStatus::ReconfigureSurface) => {
                    if let Some(renderer) = self.renderer.as_mut() {
                        renderer.reconfigure();
                    }
                    match self.render_current_frame() {
                        Ok(RenderStatus::Rendered | RenderStatus::SkipFrame) => {}
                        Ok(RenderStatus::ReconfigureSurface) => {}
                        Err(error) => self.fail(event_loop, error),
                    }
                }
                Err(error) => self.fail(event_loop, error),
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let now = Instant::now();
        let theme_changed = self.refresh_platform_theme();
        if theme_changed {
            self.sync_bindings(now);
            if let Some(window) = self.window.as_ref() {
                window.request_redraw();
            }
        }
        self.request_redraw_if_dirty(now);
        #[cfg(all(target_os = "android", feature = "android"))]
        let animation_frame_advanced = self.drive_animations(event_loop, now);
        #[cfg(not(all(target_os = "android", feature = "android")))]
        self.drive_animations(event_loop, now);
        #[cfg(all(target_os = "android", feature = "android"))]
        if theme_changed || animation_frame_advanced {
            self.render_immediately(event_loop);
        }
    }

    fn suspended(&mut self, _event_loop: &ActiveEventLoop) {
        self.suspend();
    }
}

fn is_primary_shortcut_modifier(modifiers: ModifiersState) -> bool {
    #[cfg(target_os = "macos")]
    {
        modifiers.super_key()
    }

    #[cfg(not(target_os = "macos"))]
    {
        modifiers.control_key()
    }
}

fn normalize_single_line_text(text: &str) -> String {
    text.chars()
        .map(|ch| match ch {
            '\r' | '\n' => ' ',
            other => other,
        })
        .filter(|ch| !ch.is_control() || *ch == ' ')
        .collect()
}

fn mouse_scroll_delta(delta: MouseScrollDelta) -> Point {
    const LINE_SCROLL_STEP: f32 = 40.0;

    match delta {
        MouseScrollDelta::LineDelta(x, y) => Point {
            x: x * LINE_SCROLL_STEP,
            y: y * LINE_SCROLL_STEP,
        },
        MouseScrollDelta::PixelDelta(position) => Point {
            x: position.x as f32,
            y: position.y as f32,
        },
    }
}

fn cursor_icon(cursor_style: crate::ui::widget::CursorStyle) -> CursorIcon {
    match cursor_style {
        crate::ui::widget::CursorStyle::Default => CursorIcon::Default,
        crate::ui::widget::CursorStyle::Pointer => CursorIcon::Pointer,
        crate::ui::widget::CursorStyle::Text => CursorIcon::Text,
        crate::ui::widget::CursorStyle::Crosshair => CursorIcon::Crosshair,
        crate::ui::widget::CursorStyle::Move => CursorIcon::Move,
        crate::ui::widget::CursorStyle::NotAllowed => CursorIcon::NotAllowed,
        crate::ui::widget::CursorStyle::Grab => CursorIcon::Grab,
        crate::ui::widget::CursorStyle::Grabbing => CursorIcon::Grabbing,
        crate::ui::widget::CursorStyle::EwResize => CursorIcon::EwResize,
        crate::ui::widget::CursorStyle::NsResize => CursorIcon::NsResize,
        crate::ui::widget::CursorStyle::NeswResize => CursorIcon::NeswResize,
        crate::ui::widget::CursorStyle::NwseResize => CursorIcon::NwseResize,
    }
}

fn move_cursor(state: &mut InputEditState, next: usize, extend_selection: bool) {
    state.cursor = next;
    if !extend_selection {
        state.anchor = next;
    }
    state.composition = None;
}

fn grapheme_boundaries(text: &str) -> Vec<usize> {
    let mut boundaries = vec![0];
    for (index, grapheme) in text.grapheme_indices(true) {
        boundaries.push(index + grapheme.len());
    }
    boundaries
}

fn previous_grapheme_boundary(text: &str, cursor: usize) -> usize {
    grapheme_boundaries(text)
        .into_iter()
        .take_while(|boundary| *boundary < cursor)
        .last()
        .unwrap_or(0)
}

fn next_grapheme_boundary(text: &str, cursor: usize) -> usize {
    grapheme_boundaries(text)
        .into_iter()
        .find(|boundary| *boundary > cursor)
        .unwrap_or(text.len())
}

fn resolve_theme(selection: &ThemeSelection, window_theme: Option<WindowTheme>) -> Theme {
    match selection {
        ThemeSelection::System => Theme::from_window_theme(window_theme),
        ThemeSelection::Fixed(theme) => theme.clone(),
    }
}

fn resolve_window_theme(
    window: Option<&Window>,
    #[cfg(all(target_os = "android", feature = "android"))] android_app: Option<&AndroidApp>,
) -> Option<WindowTheme> {
    #[cfg(all(target_os = "android", feature = "android"))]
    if let Some(app) = android_app {
        if let Some(theme) = resolve_android_window_theme(app) {
            return Some(theme);
        }
    }

    window.and_then(|window| window.theme())
}

#[cfg(all(target_os = "android", feature = "android"))]
fn resolve_android_window_theme(app: &AndroidApp) -> Option<WindowTheme> {
    resolve_android_window_theme_from_java(app).or_else(|| match app.config().ui_mode_night() {
        UiModeNight::No => Some(WindowTheme::Light),
        UiModeNight::Yes => Some(WindowTheme::Dark),
        UiModeNight::Any | UiModeNight::__Unknown(_) => None,
        _ => None,
    })
}

#[cfg(all(target_os = "android", feature = "android"))]
fn resolve_android_window_theme_from_java(app: &AndroidApp) -> Option<WindowTheme> {
    let vm = unsafe { JavaVM::from_raw(app.vm_as_ptr().cast()) };
    let activity_raw = app.activity_as_ptr() as jni::sys::jobject;

    vm.attach_current_thread(|env| -> jni::errors::Result<Option<WindowTheme>> {
        let activity = unsafe { env.as_cast_raw::<JObject>(&activity_raw)? };
        let ui_mode_service = env
            .get_static_field(
                jni_str!("android/content/Context"),
                jni_str!("UI_MODE_SERVICE"),
                jni_sig!("Ljava/lang/String;"),
            )?
            .l()?;
        let ui_mode_manager = env
            .call_method(
                &activity,
                jni_str!("getSystemService"),
                jni_sig!("(Ljava/lang/String;)Ljava/lang/Object;"),
                &[JValue::Object(&ui_mode_service)],
            )?
            .l()?;

        if !ui_mode_manager.is_null() {
            let night_mode = env
                .call_method(
                    &ui_mode_manager,
                    jni_str!("getNightMode"),
                    jni_sig!("()I"),
                    &[],
                )?
                .i()?;
            let light = env
                .get_static_field(
                    jni_str!("android/app/UiModeManager"),
                    jni_str!("MODE_NIGHT_NO"),
                    jni_sig!("I"),
                )?
                .i()?;
            let dark = env
                .get_static_field(
                    jni_str!("android/app/UiModeManager"),
                    jni_str!("MODE_NIGHT_YES"),
                    jni_sig!("I"),
                )?
                .i()?;

            match night_mode {
                mode if mode == light => return Ok(Some(WindowTheme::Light)),
                mode if mode == dark => return Ok(Some(WindowTheme::Dark)),
                _ => {}
            }
        }

        let resources = env
            .call_method(
                &activity,
                jni_str!("getResources"),
                jni_sig!("()Landroid/content/res/Resources;"),
                &[],
            )?
            .l()?;
        let configuration = env
            .call_method(
                &resources,
                jni_str!("getConfiguration"),
                jni_sig!("()Landroid/content/res/Configuration;"),
                &[],
            )?
            .l()?;

        let ui_mode = env
            .get_field(&configuration, jni_str!("uiMode"), jni_sig!("I"))?
            .i()?;
        let mask = env
            .get_static_field(
                jni_str!("android/content/res/Configuration"),
                jni_str!("UI_MODE_NIGHT_MASK"),
                jni_sig!("I"),
            )?
            .i()?;
        let light = env
            .get_static_field(
                jni_str!("android/content/res/Configuration"),
                jni_str!("UI_MODE_NIGHT_NO"),
                jni_sig!("I"),
            )?
            .i()?;
        let dark = env
            .get_static_field(
                jni_str!("android/content/res/Configuration"),
                jni_str!("UI_MODE_NIGHT_YES"),
                jni_sig!("I"),
            )?
            .i()?;

        Ok(match ui_mode & mask {
            mode if mode == light => Some(WindowTheme::Light),
            mode if mode == dark => Some(WindowTheme::Dark),
            _ => None,
        })
    })
    .ok()
    .flatten()
}

#[cfg(all(target_os = "android", feature = "android"))]
fn apply_android_system_bar_style(
    app: &AndroidApp,
    style: SystemBarStyle,
) -> Result<(), String> {
    let scheduler_app = app.clone();
    let callback_app = scheduler_app.clone();
    scheduler_app.run_on_java_main_thread(Box::new(move || {
        if let Err(error) = apply_android_system_bar_style_on_main_thread(&callback_app, style) {
            eprintln!("failed to sync Android system bars: {error}");
        }
    }));

    Ok(())
}

#[cfg(all(target_os = "android", feature = "android"))]
fn apply_android_system_bar_style_on_main_thread(
    app: &AndroidApp,
    style: SystemBarStyle,
) -> Result<(), String> {
    let vm = unsafe { JavaVM::from_raw(app.vm_as_ptr().cast()) };
    let activity_raw = app.activity_as_ptr() as jni::sys::jobject;

    vm.attach_current_thread(|env| -> jni::errors::Result<()> {
        let activity = unsafe { env.as_cast_raw::<JObject>(&activity_raw)? };
        let window = env
            .call_method(
                &activity,
                jni_str!("getWindow"),
                jni_sig!("()Landroid/view/Window;"),
                &[],
            )?
            .l()?;

        let bar_color = color_to_android_argb(style.color);
        env.call_method(
            &window,
            jni_str!("setStatusBarColor"),
            jni_sig!("(I)V"),
            &[JValue::Int(bar_color)],
        )?;
        env.call_method(
            &window,
            jni_str!("setNavigationBarColor"),
            jni_sig!("(I)V"),
            &[JValue::Int(bar_color)],
        )?;

        let sdk_int = env
            .get_static_field(
                jni_str!("android/os/Build$VERSION"),
                jni_str!("SDK_INT"),
                jni_sig!("I"),
            )?
            .i()?;

        if sdk_int >= 30 {
            let controller = env
                .call_method(
                    &window,
                    jni_str!("getInsetsController"),
                    jni_sig!("()Landroid/view/WindowInsetsController;"),
                    &[],
                )?
                .l()?;

            if !controller.is_null() {
                let light_status = env
                    .get_static_field(
                        jni_str!("android/view/WindowInsetsController"),
                        jni_str!("APPEARANCE_LIGHT_STATUS_BARS"),
                        jni_sig!("I"),
                    )?
                    .i()?;
                let light_navigation = env
                    .get_static_field(
                        jni_str!("android/view/WindowInsetsController"),
                        jni_str!("APPEARANCE_LIGHT_NAVIGATION_BARS"),
                        jni_sig!("I"),
                    )?
                    .i()?;
                let mask = light_status | light_navigation;
                let appearance = if style.use_dark_icons { mask } else { 0 };
                env.call_method(
                    &controller,
                    jni_str!("setSystemBarsAppearance"),
                    jni_sig!("(II)V"),
                    &[JValue::Int(appearance), JValue::Int(mask)],
                )?;
            }
        } else {
            let decor_view = env
                .call_method(
                    &window,
                    jni_str!("getDecorView"),
                    jni_sig!("()Landroid/view/View;"),
                    &[],
                )?
                .l()?;
            let mut visibility = env
                .call_method(
                    &decor_view,
                    jni_str!("getSystemUiVisibility"),
                    jni_sig!("()I"),
                    &[],
                )?
                .i()?;

            let light_status = if sdk_int >= 23 {
                env.get_static_field(
                    jni_str!("android/view/View"),
                    jni_str!("SYSTEM_UI_FLAG_LIGHT_STATUS_BAR"),
                    jni_sig!("I"),
                )?
                .i()?
            } else {
                0
            };
            let light_navigation = if sdk_int >= 26 {
                env.get_static_field(
                    jni_str!("android/view/View"),
                    jni_str!("SYSTEM_UI_FLAG_LIGHT_NAVIGATION_BAR"),
                    jni_sig!("I"),
                )?
                .i()?
            } else {
                0
            };

            let flags = light_status | light_navigation;
            if style.use_dark_icons {
                visibility |= flags;
            } else {
                visibility &= !flags;
            }

            env.call_method(
                &decor_view,
                jni_str!("setSystemUiVisibility"),
                jni_sig!("(I)V"),
                &[JValue::Int(visibility)],
            )?;
        }

        Ok(())
    })
    .map_err(|error| format!("failed to sync Android system bars: {error}"))?;

    Ok(())
}

#[cfg(all(target_os = "android", feature = "android"))]
fn color_to_android_argb(color: Color) -> i32 {
    ((color.a as i32) << 24) | ((color.r as i32) << 16) | ((color.g as i32) << 8) | color.b as i32
}

#[cfg(all(target_os = "android", feature = "android"))]
fn is_light_color(color: Color) -> bool {
    let to_linear = |channel: u8| {
        let value = channel as f32 / 255.0;
        if value <= 0.03928 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(2.4)
        }
    };

    let luminance =
        0.2126 * to_linear(color.r) + 0.7152 * to_linear(color.g) + 0.0722 * to_linear(color.b);
    luminance > 0.5
}

#[cfg(test)]
mod tests {
    use super::{next_grapheme_boundary, normalize_single_line_text, previous_grapheme_boundary};

    #[test]
    fn grapheme_navigation_keeps_emoji_cluster_intact() {
        let text = "A👨‍👩‍👧‍👦中";
        let emoji_start = previous_grapheme_boundary(text, text.len());
        let end = next_grapheme_boundary(text, 1);

        assert_eq!(&text[1..end], "👨‍👩‍👧‍👦");
        assert_eq!(emoji_start, end);
    }

    #[test]
    fn normalize_single_line_text_replaces_newlines_with_spaces() {
        assert_eq!(
            normalize_single_line_text("hello\r\nworld\t!"),
            "hello  world!"
        );
    }
}
