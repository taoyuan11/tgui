use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::animation::{
    default_theme_transition, AnimationCoordinator, AnimationEngine, AnimationKey, Transition,
    WindowProperty,
};
use winit::application::ApplicationHandler;
use winit::event::{ElementState, Ime, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, ModifiersState, PhysicalKey};
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
    ScenePrimitives, WidgetId, WidgetTree,
};
use unicode_segmentation::UnicodeSegmentation;

const DOUBLE_CLICK_THRESHOLD: Duration = Duration::from_millis(300);
const CARET_BLINK_INTERVAL: Duration = Duration::from_millis(500);

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
    animations: AnimationCoordinator,
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
            animations,
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
    animations: AnimationCoordinator,
    animation_engine: AnimationEngine,
    animation_epoch: u64,
    caret_blink_started_at: Option<Instant>,
    cursor_position: Option<Point>,
    modifiers: ModifiersState,
    hovered_widgets: Vec<HoveredWidget<VM>>,
    pending_click: Option<PendingClick<VM>>,
    focused_widget: Option<FocusedWidget<VM>>,
    focused_input: Option<WidgetId>,
    input_states: HashMap<WidgetId, InputEditState>,
    clipboard: ClipboardService,
    cached_scene: Option<CachedScene>,
    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
    window_id: Option<WindowId>,
    error: Option<TguiError>,
}

struct CachedScene {
    viewport: Rect,
    focused_input: Option<WidgetId>,
    caret_visible: bool,
    animation_epoch: u64,
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
    prefers_text_cursor: bool,
    on_mouse_enter: Option<Command<VM>>,
    on_mouse_leave: Option<Command<VM>>,
    on_mouse_move: Option<crate::foundation::view_model::ValueCommand<VM, Point>>,
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

    fn set_text(&mut self, text: String) {
        #[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
        {
            if self.inner.is_none() {
                self.inner = arboard::Clipboard::new().ok();
            }
            if let Some(clipboard) = self.inner.as_mut() {
                let _ = clipboard.set_text(text);
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
            pending_click: None,
            focused_widget: None,
            focused_input: None,
            input_states: HashMap::new(),
            clipboard: ClipboardService::default(),
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
                | WindowEvent::MouseInput {
                    state: ElementState::Pressed,
                    button: MouseButton::Left,
                    ..
                }
                | WindowEvent::KeyboardInput { .. }
        )
    }

    fn render_current_frame(&mut self) -> Result<RenderStatus, TguiError> {
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

    fn drive_animations(&mut self, event_loop: &ActiveEventLoop, now: Instant) {
        self.flush_pending_click_if_due(now);

        if self.animations.refresh(now) {
            self.animation_epoch = self.animation_epoch.wrapping_add(1);
            self.invalidate_scene();
            if let Some(window) = self.window.as_ref() {
                window.request_redraw();
            }
        }

        if self.animation_engine.refresh(now) {
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
                    viewport,
                    self.cursor_position,
                    self.focused_input,
                )
                .into_iter()
                .map(|interaction| {
                    let prefers_text_cursor =
                        matches!(&interaction, HitInteraction::FocusInput { .. });
                    match interaction {
                        HitInteraction::Widget {
                            id, interactions, ..
                        }
                        | HitInteraction::FocusInput {
                            id, interactions, ..
                        } => HoveredWidget {
                            widget_id: id,
                            prefers_text_cursor,
                            on_mouse_enter: interactions.on_mouse_enter,
                            on_mouse_leave: interactions.on_mouse_leave,
                            on_mouse_move: interactions.on_mouse_move,
                        },
                    }
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

        if let Some(window) = self.window.as_ref() {
            let cursor = if self
                .hovered_widgets
                .last()
                .map(|hovered| hovered.prefers_text_cursor)
                .unwrap_or(false)
            {
                Cursor::Icon(CursorIcon::Text)
            } else {
                Cursor::Icon(CursorIcon::Default)
            };
            window.set_cursor(cursor);
        }
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

        if let WindowEvent::ModifiersChanged(modifiers) = &event {
            self.modifiers = modifiers.state();
        }

        if matches!(event, WindowEvent::CursorLeft { .. }) {
            self.cursor_position = None;
            for hovered in std::mem::take(&mut self.hovered_widgets).into_iter().rev() {
                if let Some(command) = hovered.on_mouse_leave {
                    command.execute(&mut self.view_model);
                    self.invalidate_scene();
                    self.invalidation.mark_dirty();
                }
            }
            if let Some(window) = self.window.as_ref() {
                window.set_cursor(Cursor::Icon(CursorIcon::Default));
            }
        }

        if Self::should_dispatch_widget_event(&event) {
            let viewport = self.viewport_rect();
            let previous_focus = self.focused_input;

            match &event {
                WindowEvent::CursorMoved { .. } => {
                    self.handle_hover(viewport);
                }
                WindowEvent::MouseInput {
                    state: ElementState::Pressed,
                    button: MouseButton::Left,
                    ..
                } => {
                    self.handle_mouse_press(viewport, Instant::now());
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
            WindowEvent::Ime(ime) => {
                self.handle_input_ime(ime);
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
