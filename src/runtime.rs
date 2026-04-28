use crate::animation::{
    default_theme_transition, AnimationCoordinator, AnimationEngine, AnimationKey, Transition,
    WindowProperty,
};
use crate::application::{
    ApplicationConfig, ThemeSelection, WindowClosePolicy, WindowRole, WindowSetFactory,
};
use crate::dialog::{async_dialog_channel, AsyncDialogDispatcher, AsyncDialogReceiver, Dialogs};
use crate::foundation::binding::{Binding, InvalidationSignal};
use crate::foundation::color::Color;
use crate::foundation::error::TguiError;
use crate::foundation::event::InputTrigger;
use crate::foundation::view_model::{Command, CommandContext, ValueCommand, ViewModel};
use crate::log::Log;
use crate::media::MediaManager;
#[cfg(all(target_os = "android", feature = "android"))]
use crate::platform::android::activity::ndk::configuration::UiModeNight;
#[cfg(all(target_os = "android", feature = "android"))]
use crate::platform::android::activity::AndroidApp;
use crate::platform::backend::application::ApplicationHandler;
use crate::platform::backend::event_loop::{ActiveEventLoop, ControlFlow};
use crate::platform::backend::window::Window;
use crate::platform::backend::EventLoop;
use crate::platform::cursor::{Cursor, CursorIcon};
use crate::platform::dpi::{PhysicalPosition, PhysicalSize};
use crate::platform::event::{
    ElementState, Ime, KeyEvent, MouseButton, MouseScrollDelta, WindowEvent,
};
use crate::platform::keyboard::{KeyCode, ModifiersState, PhysicalKey};
#[cfg(all(target_env = "ohos", feature = "ohos"))]
use crate::platform::ohos::{OhosApp, WindowExtOhos};
use crate::platform::window::{
    ImeCapabilities, ImeEnableRequest, ImeHint, ImePurpose, ImeRequest, ImeRequestData,
    Theme as WindowTheme, WindowAttributes, WindowId,
};
use crate::rendering::renderer::{RenderStatus, Renderer};
use crate::text::font::{FontManager, TextFontRequest};
use crate::ui::theme::{Theme, ThemeMode, ThemeSet, ThemeStore};
use crate::ui::unit::{dp, sp, Dp, Sp, UnitContext};
use crate::ui::widget::{
    input_scroll_offset, input_text_viewport, InputViewport, INPUT_CARET_EDGE_GAP,
};
use crate::ui::widget::{
    CanvasItemId, CanvasPointerEvent, ComputedScene, HitInteraction, InputEditState, InputSnapshot,
    InteractionHandlers, MediaEventPhase, MediaEventState, Point, Rect, ResolvedSceneLayout,
    ScrollRegion, ScrollbarAxis, ScrollbarHandle, Text, WidgetId, WidgetStateMap, WidgetTree,
};
use image::GenericImageView;
#[cfg(all(target_os = "android", feature = "android"))]
use jni::{jni_sig, jni_str, objects::JObject, JValue, JavaVM};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use unicode_segmentation::UnicodeSegmentation;
use winit_core::icon::{Icon, RgbaIcon};

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
        let color = theme.colors.background;
        Self {
            color,
            use_dark_icons: is_light_color(color),
        }
    }
}

pub struct BoundRuntime<VM> {
    event_loop: EventLoop,
    config: ApplicationConfig,
    view_model: Arc<Mutex<VM>>,
    windows: Option<WindowSetFactory<VM>>,
    single_window: Option<SingleWindowSetup<VM>>,
    invalidation: InvalidationSignal,
    animations: AnimationCoordinator,
    #[cfg(all(target_os = "android", feature = "android"))]
    android_app: Option<AndroidApp>,
}

struct SingleWindowSetup<VM> {
    key: String,
    window_bindings: WindowBindings,
    widget_tree: Option<WidgetTree<VM>>,
    commands: Vec<WindowCommand<VM>>,
}

impl<VM: ViewModel> BoundRuntime<VM> {
    pub fn new(
        config: ApplicationConfig,
        view_model: VM,
        windows: WindowSetFactory<VM>,
        invalidation: InvalidationSignal,
        animations: AnimationCoordinator,
    ) -> Result<Self, TguiError> {
        let event_loop = build_event_loop(ControlFlow::Wait)?;
        Ok(Self {
            event_loop,
            config,
            view_model: Arc::new(Mutex::new(view_model)),
            windows: Some(windows),
            single_window: None,
            invalidation: invalidation.clone(),
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
            view_model: Arc::new(Mutex::new(view_model)),
            windows: None,
            single_window: Some(SingleWindowSetup {
                key: "main".to_string(),
                window_bindings,
                widget_tree,
                commands,
            }),
            invalidation: invalidation.clone(),
            animations,
            android_app: Some(app),
        })
    }

    #[cfg(all(target_env = "ohos", feature = "ohos"))]
    pub fn new_ohos(
        config: ApplicationConfig,
        view_model: VM,
        window_bindings: WindowBindings,
        widget_tree: Option<WidgetTree<VM>>,
        commands: Vec<WindowCommand<VM>>,
        invalidation: InvalidationSignal,
        animations: AnimationCoordinator,
        app: OhosApp,
    ) -> Result<Self, TguiError> {
        let event_loop = build_event_loop_with_ohos_app(ControlFlow::Wait, app)?;
        Ok(Self {
            event_loop,
            config,
            view_model: Arc::new(Mutex::new(view_model)),
            windows: None,
            single_window: Some(SingleWindowSetup {
                key: "main".to_string(),
                window_bindings,
                widget_tree,
                commands,
            }),
            invalidation: invalidation.clone(),
            animations,
            #[cfg(all(target_os = "android", feature = "android"))]
            android_app: None,
        })
    }

    pub fn run(self) -> Result<(), TguiError> {
        if self.windows.is_some() {
            let (mut event_loop, mut handler) = self.into_parts();
            event_loop.run_app_on_demand(&mut handler)?;

            if let Some(error) = handler.error {
                return Err(error);
            }
        } else {
            let (mut event_loop, mut handler) = self.into_single_window_parts();
            event_loop.run_app_on_demand(&mut handler)?;

            if let Some(error) = handler.error {
                return Err(error);
            }
        }

        Ok(())
    }

    #[cfg(all(target_env = "ohos", feature = "ohos"))]
    pub(crate) fn handler(
        config: ApplicationConfig,
        view_model: VM,
        window_bindings: WindowBindings,
        widget_tree: Option<WidgetTree<VM>>,
        commands: Vec<WindowCommand<VM>>,
        invalidation: InvalidationSignal,
        animations: AnimationCoordinator,
    ) -> BoundRuntimeHandler<VM> {
        let (dialog_dispatcher, dialog_receiver) = async_dialog_channel();
        BoundRuntimeHandler::new(
            "main".to_string(),
            1,
            WindowRole::Main,
            config,
            Arc::new(Mutex::new(view_model)),
            window_bindings,
            widget_tree,
            commands,
            invalidation,
            animations,
            dialog_dispatcher,
            Some(dialog_receiver),
            #[cfg(all(target_os = "android", feature = "android"))]
            None,
        )
    }

    fn into_parts(self) -> (EventLoop, MultiWindowHandler<VM>) {
        let handler = MultiWindowHandler::new(
            self.config,
            self.view_model,
            self.windows
                .expect("desktop runtime requires a window factory"),
            self.invalidation,
            self.animations,
        );
        (self.event_loop, handler)
    }

    fn into_single_window_parts(self) -> (EventLoop, BoundRuntimeHandler<VM>) {
        let single_window = self
            .single_window
            .expect("single-window runtime requires a window definition");
        let (dialog_dispatcher, dialog_receiver) = async_dialog_channel();
        let handler = BoundRuntimeHandler::new(
            single_window.key,
            1,
            WindowRole::Main,
            self.config,
            self.view_model,
            single_window.window_bindings,
            single_window.widget_tree,
            single_window.commands,
            self.invalidation,
            self.animations,
            dialog_dispatcher,
            Some(dialog_receiver),
            #[cfg(all(target_os = "android", feature = "android"))]
            self.android_app,
        );
        (self.event_loop, handler)
    }
}

fn build_event_loop(control_flow: ControlFlow) -> Result<EventLoop, TguiError> {
    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(control_flow);
    Ok(event_loop)
}

#[cfg(all(target_os = "android", feature = "android"))]
fn build_event_loop_with_android_app(
    control_flow: ControlFlow,
    app: AndroidApp,
) -> Result<EventLoop, TguiError> {
    let event_loop = EventLoop::with_android_app(app)?;
    event_loop.set_control_flow(control_flow);
    Ok(event_loop)
}

#[cfg(all(target_env = "ohos", feature = "ohos"))]
fn build_event_loop_with_ohos_app(
    control_flow: ControlFlow,
    app: OhosApp,
) -> Result<EventLoop, TguiError> {
    let event_loop = EventLoop::with_ohos_app(app)?;
    event_loop.set_control_flow(control_flow);
    Ok(event_loop)
}

#[derive(Clone, Default)]
pub struct WindowBindings {
    pub(crate) title: Option<Binding<String>>,
    pub(crate) clear_color: Option<Binding<Color>>,
    pub(crate) theme_set: Option<Binding<ThemeSet>>,
    pub(crate) theme_mode: Option<Binding<ThemeMode>>,
}

pub struct WindowCommand<VM> {
    pub(crate) trigger: InputTrigger,
    pub(crate) command: Command<VM>,
}

impl<VM> Clone for WindowCommand<VM> {
    fn clone(&self) -> Self {
        Self {
            trigger: self.trigger,
            command: self.command.clone(),
        }
    }
}

#[doc(hidden)]
pub struct BoundRuntimeHandler<VM> {
    window_key: String,
    window_instance_id: u64,
    role: WindowRole,
    config: ApplicationConfig,
    font_manager: FontManager,
    theme: Theme,
    theme_store: ThemeStore,
    view_model: Arc<Mutex<VM>>,
    window_bindings: WindowBindings,
    widget_tree: Option<WidgetTree<VM>>,
    commands: Vec<WindowCommand<VM>>,
    close_policy: WindowClosePolicy,
    invalidation: InvalidationSignal,
    last_invalidation_revision: u64,
    animations: AnimationCoordinator,
    animation_engine: AnimationEngine,
    animation_epoch: u64,
    hover_epoch: u64,
    caret_blink_started_at: Option<Instant>,
    cursor_position: Option<Point>,
    modifiers: ModifiersState,
    hovered_widgets: Vec<HoveredWidget<VM>>,
    hovered_scrollbar: Option<ScrollbarHandle>,
    active_scrollbar_drag: Option<ScrollbarDrag>,
    pending_click: Option<PendingClick<VM>>,
    pressed_widget: Option<WidgetId>,
    focused_widget: Option<FocusedWidget<VM>>,
    focused_input: Option<WidgetId>,
    input_states: HashMap<WidgetId, InputEditState>,
    selected_text: Option<WidgetId>,
    selected_text_states: HashMap<WidgetId, InputEditState>,
    active_input_selection: Option<TextSelectionDrag>,
    active_text_selection: Option<TextSelectionDrag>,
    clipboard: ClipboardService,
    cached_scene: Option<CachedScene<VM>>,
    cursor_icon: Option<CursorIcon>,
    scroll_states: HashMap<WidgetId, Point>,
    scroll_epoch: u64,
    media_event_states: HashMap<WidgetId, DispatchedMediaState>,
    media_manager: MediaManager,
    window: Option<Arc<dyn Window>>,
    renderer: Option<Renderer>,
    window_id: Option<WindowId>,
    error: Option<TguiError>,
    dialog_dispatcher: AsyncDialogDispatcher<VM>,
    dialog_receiver: Option<AsyncDialogReceiver<VM>>,
    #[cfg(all(target_os = "android", feature = "android"))]
    android_app: Option<AndroidApp>,
    #[cfg(all(target_os = "android", feature = "android"))]
    system_bar_style: Option<SystemBarStyle>,
}

struct CachedScene<VM> {
    viewport: Rect,
    units: UnitContext,
    focused_input: Option<WidgetId>,
    pressed_widget: Option<WidgetId>,
    selected_text: Option<WidgetId>,
    caret_visible: bool,
    animation_epoch: u64,
    scroll_epoch: u64,
    hover_epoch: u64,
    hovered_scrollbar: Option<ScrollbarHandle>,
    active_scrollbar: Option<ScrollbarHandle>,
    layout: Option<ResolvedSceneLayout<VM>>,
    computed: ComputedScene<VM>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum HoverTargetId {
    Widget(WidgetId),
    CanvasItem {
        widget_id: WidgetId,
        item_id: CanvasItemId,
    },
}

#[derive(Clone, Copy)]
struct CanvasPointerContext {
    item_id: CanvasItemId,
    canvas_origin: Point,
    item_origin: Point,
}

impl CanvasPointerContext {
    fn pointer_event(self, position: Point) -> CanvasPointerEvent {
        CanvasPointerEvent {
            item_id: self.item_id,
            canvas_position: Point::new(
                position.x - self.canvas_origin.x,
                position.y - self.canvas_origin.y,
            ),
            local_position: Point::new(
                position.x - self.item_origin.x,
                position.y - self.item_origin.y,
            ),
        }
    }
}

#[derive(Clone)]
enum ClickHandler<VM> {
    Command(Command<VM>),
    Toggle(ValueCommand<VM, bool>, bool),
    Canvas(ValueCommand<VM, CanvasPointerEvent>, CanvasPointerContext),
}

struct PendingClick<VM> {
    target_id: HoverTargetId,
    deadline: Instant,
    command: Option<ClickHandler<VM>>,
}

struct FocusedWidget<VM> {
    widget_id: WidgetId,
    on_blur: Option<Command<VM>>,
}

#[derive(Clone)]
enum HoverTransitionHandler<VM> {
    Command(Command<VM>),
    Canvas(ValueCommand<VM, CanvasPointerEvent>, CanvasPointerContext),
}

#[derive(Clone)]
enum HoverMoveHandler<VM> {
    Point(ValueCommand<VM, Point>),
    Canvas(ValueCommand<VM, CanvasPointerEvent>, CanvasPointerContext),
}

struct HoveredWidget<VM> {
    target_id: HoverTargetId,
    cursor_style: Option<crate::ui::widget::CursorStyle>,
    on_mouse_enter: Option<HoverTransitionHandler<VM>>,
    on_mouse_leave: Option<HoverTransitionHandler<VM>>,
    on_mouse_move: Option<HoverMoveHandler<VM>>,
}

#[derive(Clone, Copy)]
struct ScrollbarDrag {
    handle: ScrollbarHandle,
    start_cursor: Point,
    start_scroll_offset: Point,
    track: Rect,
    thumb: Rect,
    max_offset: Dp,
}

#[derive(Clone)]
struct TextSelectionDrag {
    widget_id: WidgetId,
    frame: Rect,
    padding: crate::ui::layout::Insets,
    text_style: Text,
    text: String,
}

#[derive(Clone)]
struct InputLayoutMetrics {
    frame: Rect,
    padding: crate::ui::layout::Insets,
    text_style: Text,
    text: String,
}

enum PendingMediaEvent<VM> {
    Command(Command<VM>),
    Error(ValueCommand<VM, String>, String),
}

#[derive(Clone, Default)]
struct DispatchedMediaState {
    phase: Option<MediaEventPhase>,
}

fn collect_pending_media_event<VM>(
    state: &MediaEventState<VM>,
    previous: Option<&DispatchedMediaState>,
    pending: &mut Vec<PendingMediaEvent<VM>>,
) {
    if previous.and_then(|value| value.phase.as_ref()) != state.media_phase.as_ref() {
        match state.media_phase.as_ref() {
            Some(MediaEventPhase::Loading) => {
                if let Some(command) = state.handlers.on_loading.clone() {
                    pending.push(PendingMediaEvent::Command(command));
                }
            }
            Some(MediaEventPhase::Success) => {
                if let Some(command) = state.handlers.on_success.clone() {
                    pending.push(PendingMediaEvent::Command(command));
                }
            }
            Some(MediaEventPhase::Error(error)) => {
                if let Some(command) = state.handlers.on_error.clone() {
                    pending.push(PendingMediaEvent::Error(command, error.clone()));
                }
            }
            _ => {}
        }
    }
}

#[derive(Default)]
struct ClipboardService {
    #[cfg(any(
        target_os = "windows",
        target_os = "macos",
        all(target_os = "linux", not(target_env = "ohos"))
    ))]
    inner: Option<arboard::Clipboard>,
}

impl ClipboardService {
    fn get_text(&mut self) -> Option<String> {
        #[cfg(any(
            target_os = "windows",
            target_os = "macos",
            all(target_os = "linux", not(target_env = "ohos"))
        ))]
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
        #[cfg(any(
            target_os = "windows",
            target_os = "macos",
            all(target_os = "linux", not(target_env = "ohos"))
        ))]
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

impl<VM: 'static> BoundRuntimeHandler<VM> {
    fn new(
        window_key: String,
        window_instance_id: u64,
        role: WindowRole,
        config: ApplicationConfig,
        view_model: Arc<Mutex<VM>>,
        window_bindings: WindowBindings,
        widget_tree: Option<WidgetTree<VM>>,
        commands: Vec<WindowCommand<VM>>,
        invalidation: InvalidationSignal,
        animations: AnimationCoordinator,
        dialog_dispatcher: AsyncDialogDispatcher<VM>,
        dialog_receiver: Option<AsyncDialogReceiver<VM>>,
        #[cfg(all(target_os = "android", feature = "android"))] android_app: Option<AndroidApp>,
    ) -> Self {
        let font_manager = FontManager::new(&config.fonts);
        let theme = match &config.theme {
            ThemeSelection::Fixed(theme) => theme.clone(),
            ThemeSelection::Mode(mode) => config.theme_set.resolve(*mode, None).as_ref().clone(),
            ThemeSelection::System => config.theme_set.resolve_window_theme(None).as_ref().clone(),
        };
        let theme_store = ThemeStore::new(config.theme_set.clone(), ThemeMode::System, None);
        Self {
            window_key,
            window_instance_id,
            role,
            config,
            font_manager,
            theme,
            theme_store,
            view_model,
            window_bindings,
            widget_tree,
            commands,
            close_policy: WindowClosePolicy::Close,
            invalidation: invalidation.clone(),
            last_invalidation_revision: 0,
            animations,
            animation_engine: AnimationEngine::default(),
            animation_epoch: 0,
            hover_epoch: 0,
            caret_blink_started_at: None,
            cursor_position: None,
            modifiers: ModifiersState::default(),
            hovered_widgets: Vec::new(),
            hovered_scrollbar: None,
            active_scrollbar_drag: None,
            pending_click: None,
            pressed_widget: None,
            focused_widget: None,
            focused_input: None,
            input_states: HashMap::new(),
            selected_text: None,
            selected_text_states: HashMap::new(),
            active_input_selection: None,
            active_text_selection: None,
            clipboard: ClipboardService::default(),
            cached_scene: None,
            cursor_icon: None,
            scroll_states: HashMap::new(),
            scroll_epoch: 0,
            media_event_states: HashMap::new(),
            media_manager: MediaManager::new(invalidation.clone()),
            window: None,
            renderer: None,
            window_id: None,
            error: None,
            dialog_dispatcher,
            dialog_receiver,
            #[cfg(all(target_os = "android", feature = "android"))]
            android_app,
            #[cfg(all(target_os = "android", feature = "android"))]
            system_bar_style: None,
        }
    }

    fn with_view_model<R>(&self, f: impl FnOnce(&mut VM) -> R) -> R {
        let mut view_model = self.view_model.lock().expect("view model lock poisoned");
        f(&mut view_model)
    }

    fn command_context(&self) -> CommandContext<VM> {
        CommandContext::new(
            Dialogs::from_runtime(
                self.window_key.clone(),
                self.window_instance_id,
                self.window.as_ref(),
                self.dialog_dispatcher.clone(),
            ),
            Log::default(),
        )
    }

    fn set_dialog_proxy(&self, event_loop: &dyn ActiveEventLoop) {
        self.dialog_dispatcher.set_proxy(event_loop.create_proxy());
        self.invalidation.set_proxy(event_loop.create_proxy());
    }

    fn execute_command(&mut self, command: &Command<VM>) {
        let context = self.command_context();
        self.with_view_model(|view_model| command.execute_with_context(view_model, &context));
        self.invalidate_scene();
        self.invalidation.mark_dirty();
    }

    fn execute_value_command<V>(&mut self, command: &ValueCommand<VM, V>, value: V) {
        let context = self.command_context();
        self.with_view_model(|view_model| {
            command.execute_with_context(view_model, value, &context)
        });
        self.invalidate_scene();
        self.invalidation.mark_dirty();
    }

    fn execute_hover_transition_handler(
        &mut self,
        handler: &HoverTransitionHandler<VM>,
        position: Option<Point>,
    ) {
        match handler {
            HoverTransitionHandler::Command(command) => self.execute_command(command),
            HoverTransitionHandler::Canvas(command, context) => {
                if let Some(position) = position {
                    self.execute_value_command(command, context.pointer_event(position));
                }
            }
        }
    }

    fn execute_hover_move_handler(&mut self, handler: &HoverMoveHandler<VM>, position: Point) {
        match handler {
            HoverMoveHandler::Point(command) => self.execute_value_command(command, position),
            HoverMoveHandler::Canvas(command, context) => {
                self.execute_value_command(command, context.pointer_event(position));
            }
        }
    }

    fn execute_click_handler(&mut self, handler: &ClickHandler<VM>, position: Option<Point>) {
        match handler {
            ClickHandler::Command(command) => self.execute_command(command),
            ClickHandler::Toggle(command, next) => self.execute_value_command(command, *next),
            ClickHandler::Canvas(command, context) => {
                if let Some(position) = position {
                    self.execute_value_command(command, context.pointer_event(position));
                }
            }
        }
    }

    fn drain_dialog_completions(&mut self) -> bool {
        let completions: Vec<_> = self
            .dialog_receiver
            .as_ref()
            .map(|receiver| receiver.try_iter().collect())
            .unwrap_or_default();

        if completions.is_empty() {
            return false;
        }

        for completion in completions {
            if completion.window_instance_id != self.window_instance_id {
                continue;
            }
            let context = self.command_context();
            self.with_view_model(|view_model| (completion.callback)(view_model, &context));
            self.invalidate_scene();
            self.invalidation.mark_dirty();
        }

        if let Some(window) = self.window.as_ref() {
            window.request_redraw();
        }

        true
    }

    fn set_definition(
        &mut self,
        role: WindowRole,
        config: ApplicationConfig,
        window_bindings: WindowBindings,
        commands: Vec<WindowCommand<VM>>,
        close_policy: WindowClosePolicy,
    ) {
        self.role = role;
        let font_manager = FontManager::new(&config.fonts);
        self.config = config;
        self.font_manager = font_manager;
        self.window_bindings = window_bindings;
        self.commands = commands;
        self.close_policy = close_policy;
        self.media_event_states.clear();
        self.invalidate_scene();
    }

    fn close_policy(&self) -> WindowClosePolicy {
        self.close_policy
    }

    fn is_main_window(&self) -> bool {
        matches!(self.role, WindowRole::Main)
    }

    fn blocks_main_window(&self) -> bool {
        matches!(
            self.role,
            WindowRole::Child {
                blocks_main_window: true
            }
        )
    }

    fn fail(&mut self, event_loop: &dyn ActiveEventLoop, error: TguiError) {
        Log::with_tag("tgui-runtime").error(format!("bound runtime failed: {error}"));
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
            Log::with_tag("tgui-runtime")
                .warn(format!("failed to sync Android system bar style: {error}"));
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
                &self.active_theme_set(),
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

    fn active_theme_set(&self) -> ThemeSet {
        self.window_bindings
            .theme_set
            .as_ref()
            .map(Binding::get)
            .unwrap_or_else(|| self.config.theme_set.clone())
    }

    fn sync_theme_binding(&mut self) {
        let selection = self.active_theme_selection();
        let theme_set = self.active_theme_set();
        let system_theme = resolve_window_theme(
            self.window.as_deref(),
            #[cfg(all(target_os = "android", feature = "android"))]
            self.android_app.as_ref(),
        );
        self.theme_store.set_theme_set(theme_set.clone());
        self.theme_store.set_system_theme(system_theme);
        let resolved_theme = match selection {
            ThemeSelection::Fixed(theme) => theme,
            ThemeSelection::System => {
                self.theme_store.set_mode(ThemeMode::System);
                self.theme_store.current().as_ref().clone()
            }
            ThemeSelection::Mode(mode) => {
                self.theme_store.set_mode(mode);
                self.theme_store.current().as_ref().clone()
            }
        };
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
                renderer.set_clear_color(theme.colors.background);
            }
        }
    }

    fn request_redraw_if_dirty(&mut self, now: Instant) {
        let revision = self.invalidation.revision();
        if revision != self.last_invalidation_revision {
            self.last_invalidation_revision = revision;
            self.invalidate_scene();
            self.sync_bindings(now);

            if let Some(window) = self.window.as_ref() {
                window.request_redraw();
            }
        }
    }

    fn scene_cache_matches(
        &self,
        cached: &CachedScene<VM>,
        viewport: Rect,
        units: UnitContext,
        caret_visible: bool,
        active_scrollbar: Option<ScrollbarHandle>,
    ) -> bool {
        cached.viewport == viewport
            && cached.units == units
            && cached.focused_input == self.focused_input
            && cached.pressed_widget == self.pressed_widget
            && cached.selected_text == self.selected_text
            && cached.caret_visible == caret_visible
            && cached.animation_epoch == self.animation_epoch
            && cached.scroll_epoch == self.scroll_epoch
            && cached.hover_epoch == self.hover_epoch
            && cached.hovered_scrollbar == self.hovered_scrollbar
            && cached.active_scrollbar == active_scrollbar
    }

    fn scene_layout_cache_matches(
        &self,
        cached: &CachedScene<VM>,
        viewport: Rect,
        units: UnitContext,
        caret_visible: bool,
    ) -> bool {
        cached.viewport == viewport
            && cached.units == units
            && cached.focused_input == self.focused_input
            && cached.pressed_widget == self.pressed_widget
            && cached.selected_text == self.selected_text
            && cached.caret_visible == caret_visible
            && cached.animation_epoch == self.animation_epoch
            && cached.hover_epoch == self.hover_epoch
    }

    fn computed_scene(&mut self) -> &ComputedScene<VM> {
        let viewport = self.viewport_rect();
        let units = self.unit_context();
        let caret_visible = self.input_caret_visible(Instant::now());
        let active_scrollbar = self.active_scrollbar_drag.map(|drag| drag.handle);
        let focused_input_state = self
            .focused_input
            .and_then(|id| self.focused_input_state(id))
            .cloned();
        let selected_text_state = self
            .selected_text
            .and_then(|id| self.selected_text_state(id))
            .cloned();

        let cache_valid = self
            .cached_scene
            .as_ref()
            .map(|cached| {
                self.scene_cache_matches(cached, viewport, units, caret_visible, active_scrollbar)
            })
            .unwrap_or(false);
        let layout_cache_valid = self
            .cached_scene
            .as_ref()
            .map(|cached| self.scene_layout_cache_matches(cached, viewport, units, caret_visible))
            .unwrap_or(false);

        let widget_states = self.widget_state_map(active_scrollbar);
        if !cache_valid {
            let previous_cached = self.cached_scene.take();
            let theme = self.animated_theme(Instant::now());
            let (layout, computed) = match self.widget_tree.as_ref() {
                Some(tree) => {
                    if layout_cache_valid {
                        let computed = {
                            let cached = previous_cached
                                .as_ref()
                                .expect("layout cache should exist when layout cache is valid");
                            let layout = cached
                                .layout
                                .as_ref()
                                .expect("layout should exist when layout cache is valid");
                            tree.collect_scene_from_layout(
                                &self.font_manager,
                                layout,
                                &theme,
                                &self.media_manager,
                                &mut self.animation_engine,
                                self.hovered_scrollbar,
                                active_scrollbar,
                                &widget_states,
                                &self.scroll_states,
                                viewport,
                                self.focused_input,
                                focused_input_state.as_ref(),
                                self.selected_text,
                                selected_text_state.as_ref(),
                                caret_visible,
                            )
                        };
                        let layout = previous_cached.and_then(|cached| cached.layout);
                        (layout, computed)
                    } else {
                        let layout = tree.build_scene_layout(
                            &self.font_manager,
                            &theme,
                            &self.media_manager,
                            &mut self.animation_engine,
                            units,
                            viewport,
                        );
                        let computed = tree.collect_scene_from_layout(
                            &self.font_manager,
                            &layout,
                            &theme,
                            &self.media_manager,
                            &mut self.animation_engine,
                            self.hovered_scrollbar,
                            active_scrollbar,
                            &widget_states,
                            &self.scroll_states,
                            viewport,
                            self.focused_input,
                            focused_input_state.as_ref(),
                            self.selected_text,
                            selected_text_state.as_ref(),
                            caret_visible,
                        );
                        (Some(layout), computed)
                    }
                }
                None => (None, ComputedScene::default()),
            };
            self.cached_scene = Some(CachedScene {
                viewport,
                units,
                focused_input: self.focused_input,
                pressed_widget: self.pressed_widget,
                selected_text: self.selected_text,
                caret_visible,
                animation_epoch: self.animation_epoch,
                scroll_epoch: self.scroll_epoch,
                hover_epoch: self.hover_epoch,
                hovered_scrollbar: self.hovered_scrollbar,
                active_scrollbar,
                layout,
                computed,
            });
        }

        &self
            .cached_scene
            .as_ref()
            .expect("computed scene cache should exist")
            .computed
    }

    fn widget_state_map(&self, active_scrollbar: Option<ScrollbarHandle>) -> WidgetStateMap {
        let mut states = WidgetStateMap::default();
        for hovered in &self.hovered_widgets {
            if let HoverTargetId::Widget(id) = hovered.target_id {
                let mut state = states.get(id);
                state.hovered = true;
                states.set(id, state);
            }
        }
        if let Some(id) = self.pressed_widget {
            let mut state = states.get(id);
            state.pressed = true;
            states.set(id, state);
        }
        if let Some(id) = self.focused_input {
            let mut state = states.get(id);
            state.focused = true;
            states.set(id, state);
        }
        if let Some(focused) = self.focused_widget.as_ref() {
            let mut state = states.get(focused.widget_id);
            state.focused = true;
            states.set(focused.widget_id, state);
        }
        if let Some(handle) = self.hovered_scrollbar {
            let mut state = states.get(handle.id);
            state.hovered = true;
            states.set(handle.id, state);
        }
        if let Some(handle) = active_scrollbar {
            let mut state = states.get(handle.id);
            state.pressed = true;
            states.set(handle.id, state);
        }
        states
    }

    fn scroll_regions(&mut self) -> Vec<ScrollRegion> {
        self.computed_scene().scroll_regions.clone()
    }

    fn ime_cursor_area(&mut self) -> Option<Rect> {
        self.computed_scene().ime_cursor_area
    }

    fn dispatch_media_events(&mut self) {
        let Some(tree) = self.widget_tree.as_ref() else {
            self.media_event_states.clear();
            return;
        };

        let states = tree.media_event_states(&self.media_manager);
        let current_ids: HashSet<_> = states.iter().map(|state| state.widget_id).collect();
        self.media_event_states
            .retain(|widget_id, _| current_ids.contains(widget_id));

        let mut pending = Vec::new();
        for state in states {
            let previous = self.media_event_states.get(&state.widget_id);
            collect_pending_media_event(&state, previous, &mut pending);
            self.media_event_states.insert(
                state.widget_id,
                DispatchedMediaState {
                    phase: state.media_phase.clone(),
                },
            );
        }

        if pending.is_empty() {
            return;
        }

        for event in pending {
            match event {
                PendingMediaEvent::Command(command) => self.execute_command(&command),
                PendingMediaEvent::Error(command, error) => {
                    self.execute_value_command(&command, error);
                }
            }
        }

        if let Some(window) = self.window.as_ref() {
            window.request_redraw();
        }
    }

    fn viewport_rect(&self) -> Rect {
        let size = self
            .window
            .as_ref()
            .map(|window| {
                window
                    .surface_size()
                    .to_logical::<f32>(window.scale_factor())
            })
            .unwrap_or(crate::platform::dpi::LogicalSize::new(
                self.config.size.width as f32,
                self.config.size.height as f32,
            ));
        Rect::new(0.0, 0.0, size.width, size.height)
    }

    fn invalidate_scene(&mut self) {
        self.cached_scene = None;
    }

    fn should_dispatch_widget_event(event: &WindowEvent) -> bool {
        match event {
            WindowEvent::PointerMoved { .. } | WindowEvent::MouseWheel { .. } => true,
            WindowEvent::PointerButton {
                state: ElementState::Pressed,
                button,
                ..
            } => button.clone().mouse_button() == Some(MouseButton::Left),
            WindowEvent::KeyboardInput { .. } => true,
            _ => false,
        }
    }

    fn render_current_frame(&mut self) -> Result<RenderStatus, TguiError> {
        // Android can deliver a redraw before a replacement surface is ready.
        // In that case we simply skip the frame and wait for the next resume/redraw.
        if self.renderer.is_none() {
            return Ok(RenderStatus::SkipFrame);
        }

        self.sync_bindings(Instant::now());
        self.dispatch_media_events();
        let caret_rect = self.ime_cursor_area();
        if let (Some(window), Some(caret_rect)) = (self.window.as_ref(), caret_rect) {
            let _ = window.request_ime_update(ImeRequest::Update(Self::ime_cursor_request_data(
                caret_rect,
                self.unit_context(),
            )));
        }
        let mut renderer = self
            .renderer
            .take()
            .expect("renderer should exist before drawing");
        let status = {
            let computed = self.computed_scene();
            renderer.render(&computed.scene)
        };
        self.renderer = Some(renderer);
        status
    }

    #[cfg(all(target_os = "android", feature = "android"))]
    fn render_immediately(&mut self, event_loop: &dyn ActiveEventLoop) {
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

    fn set_pointer_position(&mut self, position: PhysicalPosition<f64>) {
        let logical = self
            .window
            .as_ref()
            .map(|window| position.to_logical::<f32>(window.scale_factor()))
            .unwrap_or_else(|| position.to_logical::<f32>(1.0));
        self.cursor_position = Some(Point {
            x: dp(logical.x),
            y: dp(logical.y),
        });
    }

    fn unit_context(&self) -> UnitContext {
        let scale_factor = self
            .window
            .as_ref()
            .map(|window| window.scale_factor() as f32)
            .unwrap_or(1.0);
        let font_scale = self.platform_font_scale();
        UnitContext::new(scale_factor, font_scale)
    }

    fn platform_font_scale(&self) -> f32 {
        #[cfg(all(target_env = "ohos", feature = "ohos"))]
        {
            if let Some(scale) = self
                .window
                .as_ref()
                .map(|window| window.font_scale() as f32)
                .filter(|scale| scale.is_finite() && *scale > 0.0)
            {
                return scale;
            }
        }
        #[cfg(all(target_os = "android", feature = "android"))]
        {
            if let Some(scale) = android_font_scale(self.android_app.as_ref()) {
                return scale;
            }
        }
        1.0
    }

    fn clear_pointer_position(&mut self) {
        let previous_position = self.cursor_position;
        self.cursor_position = None;
        for hovered in std::mem::take(&mut self.hovered_widgets).into_iter().rev() {
            if let Some(command) = hovered.on_mouse_leave {
                self.execute_hover_transition_handler(&command, previous_position);
            }
        }
        self.hovered_scrollbar = self.active_scrollbar_drag.map(|drag| drag.handle);
        self.update_cursor_icon();
    }

    fn drive_animations(&mut self, event_loop: &dyn ActiveEventLoop, now: Instant) -> bool {
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

    fn render_hidden_frame(&mut self, event_loop: &dyn ActiveEventLoop) -> bool {
        #[cfg(all(target_env = "ohos", feature = "ohos"))]
        {
            let _ = event_loop;
            return true;
        }

        #[cfg(not(all(target_env = "ohos", feature = "ohos")))]
        let status = match self.render_current_frame() {
            Ok(status) => status,
            Err(error) => {
                self.fail(event_loop, error);
                return false;
            }
        };

        #[cfg(not(all(target_env = "ohos", feature = "ohos")))]
        if matches!(status, RenderStatus::ReconfigureSurface) {
            if let Some(renderer) = self.renderer.as_mut() {
                renderer.reconfigure();
            }

            if let Err(error) = self.render_current_frame() {
                self.fail(event_loop, error);
                return false;
            }
        }

        #[cfg(not(all(target_env = "ohos", feature = "ohos")))]
        true
    }

    fn resume_existing_window(&mut self, event_loop: &dyn ActiveEventLoop) {
        let Some(window) = self.window.clone() else {
            return;
        };

        self.sync_theme_binding();
        self.invalidate_scene();
        let clear_color =
            if self.window_bindings.clear_color.is_some() || self.config.clear_color_overridden {
                self.config.clear_color
            } else {
                self.theme.colors.background
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
        self.media_event_states.clear();
        #[cfg(all(target_os = "android", feature = "android"))]
        {
            self.system_bar_style = None;
        }
    }

    fn animated_theme(&mut self, now: Instant) -> Theme {
        let transition = Some(default_theme_transition());
        let mut theme = self.theme.clone();
        theme.colors.background = self.resolve_theme_color(
            WindowProperty::ThemeBackground,
            theme.colors.background,
            transition,
            now,
        );
        theme.colors.surface = self.resolve_theme_color(
            WindowProperty::ThemeSurface,
            theme.colors.surface,
            transition,
            now,
        );
        theme.colors.surface_low = self.resolve_theme_color(
            WindowProperty::ThemeSurfaceLow,
            theme.colors.surface_low,
            transition,
            now,
        );
        theme.colors.surface_high = self.resolve_theme_color(
            WindowProperty::ThemeSurfaceHigh,
            theme.colors.surface_high,
            transition,
            now,
        );
        theme.colors.primary = self.resolve_theme_color(
            WindowProperty::ThemePrimary,
            theme.colors.primary,
            transition,
            now,
        );
        theme.colors.on_surface = self.resolve_theme_color(
            WindowProperty::ThemeOnSurface,
            theme.colors.on_surface,
            transition,
            now,
        );
        theme.colors.on_surface_muted = self.resolve_theme_color(
            WindowProperty::ThemeOnSurfaceMuted,
            theme.colors.on_surface_muted,
            transition,
            now,
        );
        theme.colors.primary_container = self.resolve_theme_color(
            WindowProperty::ThemePrimaryContainer,
            theme.colors.primary_container,
            transition,
            now,
        );
        theme.colors.focus_ring = self.resolve_theme_color(
            WindowProperty::ThemeFocusRing,
            theme.colors.focus_ring,
            transition,
            now,
        );
        theme.colors.selection = self.resolve_theme_color(
            WindowProperty::ThemeSelection,
            theme.colors.selection,
            transition,
            now,
        );
        let input_background = self.resolve_theme_color(
            WindowProperty::ThemeInputBackground,
            theme.components.input.background.normal,
            transition,
            now,
        );
        let input_border = self.resolve_theme_color(
            WindowProperty::ThemeInputBorder,
            theme.components.input.border.normal,
            transition,
            now,
        );
        let button_primary = self.resolve_theme_color(
            WindowProperty::ThemeButtonPrimary,
            theme.components.button.primary.container.normal,
            transition,
            now,
        );
        let button_secondary = self.resolve_theme_color(
            WindowProperty::ThemeButtonSecondary,
            theme.components.button.secondary.container.normal,
            transition,
            now,
        );
        let scrollbar_thumb = self.resolve_theme_color(
            WindowProperty::ThemeScrollbarThumb,
            theme.components.scrollbar.thumb.normal,
            transition,
            now,
        );
        theme.components = crate::ui::theme::ComponentTheme::from_tokens(
            &theme.colors,
            &theme.typography,
            &theme.spacing,
            &theme.radius,
            &theme.border,
            &theme.elevation,
            &theme.motion,
        );
        theme.components.input.background.normal = input_background;
        theme.components.input.border.normal = input_border;
        theme.components.button.primary.container.normal = button_primary;
        theme.components.button.secondary.container.normal = button_secondary;
        theme.components.scrollbar.thumb.normal = scrollbar_thumb;
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
                self.execute_click_handler(&command, self.cursor_position);
            }
        }
    }

    fn focused_input_state(&self, id: WidgetId) -> Option<&InputEditState> {
        self.input_states.get(&id)
    }

    fn selected_text_state(&self, id: WidgetId) -> Option<&InputEditState> {
        self.selected_text_states.get(&id)
    }

    fn focused_input_snapshot(&self) -> Option<InputSnapshot<VM>> {
        let id = self.focused_input?;
        self.widget_tree.as_ref()?.input_snapshot(id)
    }

    fn update_selected_text_state(
        &mut self,
        widget_id: WidgetId,
        text: &str,
        update: impl FnOnce(&mut InputEditState),
    ) -> bool {
        let state = self
            .selected_text_states
            .entry(widget_id)
            .and_modify(|state| *state = state.clone().clamped_to(text))
            .or_insert_with(|| InputEditState::caret_at(text));
        let before = state.clone();
        update(state);
        *state = state.clone().clamped_to(text);
        if *state == before {
            return false;
        }
        self.invalidate_scene();
        true
    }

    fn selected_text_content(&mut self, widget_id: WidgetId) -> Option<String> {
        self.computed_scene()
            .hit_regions
            .iter()
            .find_map(|region| match &region.interaction {
                HitInteraction::SelectableText { id, text, .. } if *id == widget_id => {
                    Some(text.clone())
                }
                _ => None,
            })
    }

    fn selected_text_for_copy(&mut self) -> Option<String> {
        let Some(widget_id) = self.selected_text else {
            return None;
        };
        let Some(text) = self.selected_text_content(widget_id) else {
            return None;
        };
        let Some((start, end)) = self
            .selected_text_state(widget_id)
            .cloned()
            .unwrap_or_else(|| InputEditState::caret_at(&text))
            .clamped_to(&text)
            .selection_range()
        else {
            return None;
        };
        Some(text[start..end].to_string())
    }

    fn copy_selected_text_to_clipboard(&mut self) -> bool {
        let Some(text) = self.selected_text_for_copy() else {
            return false;
        };
        self.clipboard.set_text(text);
        true
    }

    fn clear_selected_text(&mut self) -> bool {
        let had_selection = self.selected_text.take().is_some();
        let was_dragging = self.active_text_selection.take().is_some();
        if had_selection || was_dragging {
            self.invalidate_scene();
            return true;
        }
        false
    }

    fn begin_input_selection(
        &mut self,
        widget_id: WidgetId,
        frame: Rect,
        padding: crate::ui::layout::Insets,
        text_style: Text,
        text: String,
        cursor: usize,
    ) {
        self.active_input_selection = Some(TextSelectionDrag {
            widget_id,
            frame,
            padding,
            text_style,
            text: text.clone(),
        });
        self.update_input_state(widget_id, &text, |state| {
            state.cursor = cursor;
            state.anchor = cursor;
            state.composition = None;
        });
    }

    fn handle_input_selection_drag(&mut self) -> bool {
        let Some(drag) = self.active_input_selection.clone() else {
            return false;
        };
        let Some(point) = self.cursor_position else {
            return false;
        };

        let before = self.focused_input_state(drag.widget_id).cloned();
        let Some(mut next_state) = before.clone() else {
            return false;
        };
        let inner = drag.frame.inset(drag.padding);
        let overflow_left = (inner.x - point.x).max(0.0);
        let overflow_right = (point.x - inner.right()).max(0.0);
        if overflow_left > Dp::ZERO || overflow_right > Dp::ZERO {
            let delta = if overflow_left > Dp::ZERO {
                -overflow_left.get().max(1.0)
            } else {
                overflow_right.get().max(1.0)
            };
            next_state.scroll_x = (next_state.scroll_x + delta).max(Dp::ZERO);
        }
        let cursor = input_cursor_index_at_point_with_state(
            &self.font_manager,
            &self.theme,
            self.unit_context(),
            drag.frame,
            drag.padding,
            &drag.text_style,
            &drag.text,
            Some(&next_state),
            point,
        );
        next_state.cursor = cursor;
        next_state.composition = None;
        if before.as_ref() == Some(&next_state) {
            return false;
        }
        self.update_input_state(drag.widget_id, &drag.text, |state| {
            *state = next_state;
        });
        true
    }

    fn end_input_selection_drag(&mut self) -> bool {
        if self.active_input_selection.take().is_some() {
            self.invalidate_scene();
            return true;
        }
        false
    }

    fn begin_text_selection(
        &mut self,
        widget_id: WidgetId,
        frame: Rect,
        padding: crate::ui::layout::Insets,
        text_style: Text,
        text: String,
        cursor: usize,
    ) {
        self.selected_text = Some(widget_id);
        self.active_text_selection = Some(TextSelectionDrag {
            widget_id,
            frame,
            padding,
            text_style,
            text: text.clone(),
        });
        self.update_selected_text_state(widget_id, &text, |state| {
            state.cursor = cursor;
            state.anchor = cursor;
            state.composition = None;
        });
    }

    fn handle_text_selection_drag(&mut self) -> bool {
        let Some(drag) = self.active_text_selection.clone() else {
            return false;
        };
        let Some(point) = self.cursor_position else {
            return false;
        };

        let cursor = text_cursor_index_at_point(
            &self.font_manager,
            &self.theme,
            self.unit_context(),
            drag.frame,
            drag.padding,
            &drag.text_style,
            &drag.text,
            point,
        );
        self.selected_text = Some(drag.widget_id);
        self.update_selected_text_state(drag.widget_id, &drag.text, |state| {
            state.cursor = cursor;
            state.composition = None;
        })
    }

    fn end_text_selection_drag(&mut self) -> bool {
        if self.active_text_selection.take().is_some() {
            self.invalidate_scene();
            return true;
        }
        false
    }

    fn ime_cursor_request_data(caret_rect: Rect, units: UnitContext) -> ImeRequestData {
        ImeRequestData::default().with_cursor_area(
            PhysicalPosition::new(
                units.logical_to_physical(caret_rect.x.get()).round() as i32,
                units.logical_to_physical(caret_rect.y.get()).round() as i32,
            )
            .into(),
            PhysicalSize::new(
                units
                    .logical_to_physical(caret_rect.width.get())
                    .ceil()
                    .max(1.0) as u32,
                units
                    .logical_to_physical(caret_rect.height.get())
                    .ceil()
                    .max(1.0) as u32,
            )
            .into(),
        )
    }

    fn ime_enable_request(&mut self) -> Option<ImeEnableRequest> {
        let request_data = Self::ime_cursor_request_data(
            self.ime_cursor_area()?,
            self.unit_context(),
        )
        .with_hint_and_purpose(ImeHint::NONE, ImePurpose::Normal);
        ImeEnableRequest::new(
            ImeCapabilities::new()
                .with_cursor_area()
                .with_hint_and_purpose(),
            request_data,
        )
    }

    fn sync_ime_allowed(&mut self) {
        let request = if self.focused_input.is_some() {
            self.ime_enable_request().map(ImeRequest::Enable)
        } else {
            Some(ImeRequest::Disable)
        };

        if let (Some(window), Some(request)) = (self.window.as_ref(), request) {
            let _ = window.request_ime_update(request);
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

    fn focused_input_layout_metrics(&mut self, widget_id: WidgetId) -> Option<InputLayoutMetrics> {
        let computed = self.computed_scene();
        computed
            .hit_regions
            .iter()
            .find_map(|region| match &region.interaction {
                HitInteraction::FocusInput {
                    id,
                    frame,
                    padding,
                    text_style,
                    text,
                    ..
                } if *id == widget_id => Some(InputLayoutMetrics {
                    frame: *frame,
                    padding: *padding,
                    text_style: text_style.clone(),
                    text: text.clone(),
                }),
                _ => None,
            })
    }

    fn input_scroll_offset_for_state(
        &self,
        metrics: &InputLayoutMetrics,
        state: &InputEditState,
    ) -> Option<Dp> {
        if metrics.text.is_empty() {
            return Some(Dp::ZERO);
        }

        let units = self.unit_context();
        let theme_text = &self.theme.components.text.default;
        let font_size = units.resolve_sp(
            metrics
                .text_style
                .font_size
                .unwrap_or(theme_text.size.max(sp(1.0))),
        );
        let line_height =
            units.resolve_sp(theme_text.line_height.unwrap_or(theme_text.size * 1.25));
        let line_height = line_height.max(font_size + 4.0);
        let letter_spacing = units.resolve_sp(
            metrics
                .text_style
                .letter_spacing
                .unwrap_or(theme_text.letter_spacing.unwrap_or(Sp::ZERO)),
        );
        let request = TextFontRequest {
            preferred_font: metrics.text_style.font_family.as_deref().or(self
                .theme
                .components
                .text
                .default
                .font_family
                .as_deref()),
            weight: metrics.text_style.font_weight.unwrap_or(theme_text.weight),
        };
        let layout = self.font_manager.measure_text_layout(
            &metrics.text,
            request,
            font_size,
            line_height,
            letter_spacing,
        );
        let inner = metrics.frame.inset(metrics.padding);
        let caret_boundary = layout.x_for_index(state.cursor.min(metrics.text.len()));
        let caret_padding = if state.cursor >= metrics.text.len() {
            1.0
        } else {
            0.0
        };
        Some(Dp::new(input_scroll_offset(
            inner,
            layout.width,
            caret_boundary,
            caret_boundary + caret_padding + 2.0,
            state.scroll_x.get(),
        )))
    }

    fn keep_input_caret_visible(&mut self, widget_id: WidgetId, text: &str) {
        let Some(metrics) = self.focused_input_layout_metrics(widget_id) else {
            return;
        };
        let Some(mut state) = self.focused_input_state(widget_id).cloned() else {
            return;
        };
        state = state.clamped_to(text);
        let Some(scroll_x) = self.input_scroll_offset_for_state(&metrics, &state) else {
            return;
        };
        if state.scroll_x == scroll_x {
            return;
        }
        self.update_input_state(widget_id, text, |edit| {
            edit.scroll_x = scroll_x;
        });
    }

    fn set_input_focus_state(&mut self, widget_id: WidgetId, text: &str) {
        let state = self.ensure_input_state(widget_id, text);
        *state = state.clone().clamped_to(text);
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
        self.keep_input_caret_visible(snapshot.id, &new_text);
        self.reset_caret_blink(Instant::now());

        if let Some(command) = snapshot.on_change.clone() {
            self.execute_value_command(&command, new_text);
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
            Ime::DeleteSurrounding { .. } => {}
        }
    }

    fn handle_selected_text_keyboard_event(&mut self, event: &KeyEvent) -> bool {
        if event.state != ElementState::Pressed || !is_primary_shortcut_modifier(self.modifiers) {
            return false;
        }

        match event.physical_key {
            PhysicalKey::Code(KeyCode::KeyC) => self.copy_selected_text_to_clipboard(),
            _ => false,
        }
    }

    fn handle_input_keyboard_event(&mut self, event: &KeyEvent) {
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

        if matches!(
            event.physical_key,
            PhysicalKey::Code(KeyCode::ArrowLeft)
                | PhysicalKey::Code(KeyCode::ArrowRight)
                | PhysicalKey::Code(KeyCode::Home)
                | PhysicalKey::Code(KeyCode::End)
        ) {
            if let Some(scroll_x) = self
                .focused_input_layout_metrics(snapshot.id)
                .and_then(|metrics| self.input_scroll_offset_for_state(&metrics, &state))
            {
                state.scroll_x = scroll_x;
            }
        }

        self.input_states.insert(snapshot.id, state);
        self.invalidate_scene();
        if let Some(window) = self.window.as_ref() {
            window.request_redraw();
        }
    }

    fn hit_path(&mut self, _viewport: Rect) -> Vec<HitInteraction<VM>> {
        let Some(point) = self.cursor_position else {
            return Vec::new();
        };
        WidgetTree::hit_path_from_computed(self.computed_scene(), point)
    }

    fn hover_path(&mut self, _viewport: Rect) -> Vec<HoveredWidget<VM>> {
        self.hit_path(_viewport)
            .into_iter()
            .map(|interaction| match interaction {
                HitInteraction::Widget {
                    id, interactions, ..
                } => HoveredWidget {
                    target_id: HoverTargetId::Widget(id),
                    cursor_style: interactions.cursor_style.map(|c| c.resolve()),
                    on_mouse_enter: interactions
                        .on_mouse_enter
                        .map(HoverTransitionHandler::Command),
                    on_mouse_leave: interactions
                        .on_mouse_leave
                        .map(HoverTransitionHandler::Command),
                    on_mouse_move: interactions.on_mouse_move.map(HoverMoveHandler::Point),
                },
                HitInteraction::FocusInput {
                    id, interactions, ..
                } => HoveredWidget {
                    target_id: HoverTargetId::Widget(id),
                    cursor_style: interactions
                        .cursor_style
                        .map(|c| c.resolve())
                        .or(Some(crate::ui::widget::CursorStyle::Text)),
                    on_mouse_enter: interactions
                        .on_mouse_enter
                        .map(HoverTransitionHandler::Command),
                    on_mouse_leave: interactions
                        .on_mouse_leave
                        .map(HoverTransitionHandler::Command),
                    on_mouse_move: interactions.on_mouse_move.map(HoverMoveHandler::Point),
                },
                HitInteraction::SelectableText {
                    id, interactions, ..
                } => HoveredWidget {
                    target_id: HoverTargetId::Widget(id),
                    cursor_style: interactions
                        .cursor_style
                        .map(|c| c.resolve())
                        .or(Some(crate::ui::widget::CursorStyle::Text)),
                    on_mouse_enter: interactions
                        .on_mouse_enter
                        .map(HoverTransitionHandler::Command),
                    on_mouse_leave: interactions
                        .on_mouse_leave
                        .map(HoverTransitionHandler::Command),
                    on_mouse_move: interactions.on_mouse_move.map(HoverMoveHandler::Point),
                },
                HitInteraction::Switch {
                    id, interactions, ..
                } => HoveredWidget {
                    target_id: HoverTargetId::Widget(id),
                    cursor_style: interactions.cursor_style.map(|c| c.resolve()),
                    on_mouse_enter: interactions
                        .on_mouse_enter
                        .map(HoverTransitionHandler::Command),
                    on_mouse_leave: interactions
                        .on_mouse_leave
                        .map(HoverTransitionHandler::Command),
                    on_mouse_move: interactions.on_mouse_move.map(HoverMoveHandler::Point),
                },
                HitInteraction::CanvasItem {
                    id,
                    item_id,
                    item_interactions,
                    canvas_origin,
                    item_origin,
                } => {
                    let context = CanvasPointerContext {
                        item_id,
                        canvas_origin,
                        item_origin,
                    };
                    HoveredWidget {
                        target_id: HoverTargetId::CanvasItem {
                            widget_id: id,
                            item_id,
                        },
                        cursor_style: None,
                        on_mouse_enter: item_interactions
                            .on_mouse_enter
                            .map(|command| HoverTransitionHandler::Canvas(command, context)),
                        on_mouse_leave: item_interactions
                            .on_mouse_leave
                            .map(|command| HoverTransitionHandler::Canvas(command, context)),
                        on_mouse_move: item_interactions
                            .on_mouse_move
                            .map(|command| HoverMoveHandler::Canvas(command, context)),
                    }
                }
            })
            .collect()
    }

    fn handle_hover(&mut self, viewport: Rect) -> bool {
        let revision_before = self.invalidation.revision();
        let cursor_position = self.cursor_position;
        let next_hovered = self.hover_path(viewport);
        let hover_path_changed = self.hovered_widgets.len() != next_hovered.len()
            || self
                .hovered_widgets
                .iter()
                .zip(next_hovered.iter())
                .any(|(previous, next)| previous.target_id != next.target_id);
        let mut prefix_len = 0usize;
        while prefix_len < self.hovered_widgets.len()
            && prefix_len < next_hovered.len()
            && self.hovered_widgets[prefix_len].target_id == next_hovered[prefix_len].target_id
        {
            prefix_len += 1;
        }

        let previous_hovered = std::mem::take(&mut self.hovered_widgets);
        for previous in previous_hovered[prefix_len..].iter().rev() {
            if let Some(command) = previous.on_mouse_leave.as_ref() {
                self.execute_hover_transition_handler(command, cursor_position);
            }
        }

        for hovered in next_hovered[prefix_len..].iter().rev() {
            if let Some(command) = hovered.on_mouse_enter.as_ref() {
                self.execute_hover_transition_handler(command, cursor_position);
            }
        }

        if let Some(position) = cursor_position {
            for hovered in next_hovered.iter().rev() {
                if let Some(command) = hovered.on_mouse_move.as_ref() {
                    self.execute_hover_move_handler(command, position);
                }
            }
        }

        self.hovered_widgets = next_hovered;
        if hover_path_changed {
            self.hover_epoch = self.hover_epoch.wrapping_add(1);
        }
        let scrollbar_changed = self.sync_scrollbar_hover();
        let cursor_changed = self.update_cursor_icon();
        hover_path_changed
            || scrollbar_changed
            || cursor_changed
            || self.invalidation.revision() != revision_before
    }

    fn handle_mouse_wheel(&mut self, delta: MouseScrollDelta) -> bool {
        let Some(cursor_position) = self.cursor_position else {
            return false;
        };

        let mut scroll_delta = mouse_scroll_delta(delta);
        if scroll_delta.x.abs() <= f32::EPSILON && self.modifiers.shift_key() {
            scroll_delta.x = scroll_delta.y;
            scroll_delta.y = Dp::ZERO;
        }
        if scroll_delta.x.abs() <= f32::EPSILON && scroll_delta.y.abs() <= f32::EPSILON {
            return false;
        }

        let scroll_regions = self.scroll_regions();
        for region in scroll_regions.iter().rev().copied() {
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

    fn sync_scrollbar_hover(&mut self) -> bool {
        let next_hovered = if let Some(drag) = self.active_scrollbar_drag {
            Some(drag.handle)
        } else {
            self.scrollbar_thumb_hit()
        };

        if self.hovered_scrollbar != next_hovered {
            self.hovered_scrollbar = next_hovered;
            self.invalidate_scene();
            return true;
        }

        false
    }

    fn scrollbar_thumb_hit(&mut self) -> Option<ScrollbarHandle> {
        let cursor_position = self.cursor_position?;
        let scroll_regions = self.scroll_regions();
        scroll_regions.iter().rev().find_map(|region| {
            if region.visible_frame.is_empty() || !region.visible_frame.contains(cursor_position) {
                return None;
            }
            if region
                .vertical_thumb
                .map(|thumb: Rect| thumb.contains(cursor_position))
                .unwrap_or(false)
            {
                return Some(ScrollbarHandle {
                    id: region.id,
                    axis: ScrollbarAxis::Vertical,
                });
            }
            if region
                .horizontal_thumb
                .map(|thumb: Rect| thumb.contains(cursor_position))
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
        let scroll_regions = self.scroll_regions();
        let Some(region) = scroll_regions
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
            Dp::ZERO
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

    fn update_cursor_icon(&mut self) -> bool {
        let next_icon = if self.active_scrollbar_drag.is_some() || self.hovered_scrollbar.is_some()
        {
            CursorIcon::Pointer
        } else if self.active_input_selection.is_some() {
            CursorIcon::Text
        } else if self.active_text_selection.is_some() {
            CursorIcon::Text
        } else if let Some(cursor_style) = self
            .hovered_widgets
            .iter()
            .rev()
            .find_map(|hovered| hovered.cursor_style)
        {
            cursor_icon(cursor_style)
        } else {
            CursorIcon::Default
        };

        if self.cursor_icon == Some(next_icon) {
            return false;
        }

        self.cursor_icon = Some(next_icon);
        if let Some(window) = self.window.as_ref() {
            window.set_cursor(Cursor::Icon(next_icon));
        }
        true
    }

    fn set_scroll_offset(&mut self, widget_id: WidgetId, offset: Point) {
        if offset.x.abs() <= 0.01 && offset.y.abs() <= 0.01 {
            self.scroll_states.remove(&widget_id);
        } else {
            self.scroll_states.insert(widget_id, offset);
        }
        self.scroll_epoch = self.scroll_epoch.wrapping_add(1);
        self.invalidate_scene();
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
                self.execute_command(&command);
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
                self.execute_command(&command);
                fired_handler = true;
            }
        }

        if fired_handler {
            self.invalidate_scene();
        }
    }

    fn dispatch_widget_click(
        &mut self,
        target_id: HoverTargetId,
        interactions: InteractionHandlers<VM>,
        now: Instant,
    ) {
        let is_double_click = self
            .pending_click
            .as_ref()
            .map(|pending| pending.target_id == target_id && pending.deadline > now)
            .unwrap_or(false);

        if is_double_click {
            self.pending_click = None;
            if let Some(command) = interactions.on_double_click.or(interactions.on_click) {
                self.execute_command(&command);
            }
            return;
        }

        if interactions.on_double_click.is_some() {
            self.pending_click = Some(PendingClick {
                target_id,
                deadline: now + DOUBLE_CLICK_THRESHOLD,
                command: interactions.on_click.map(ClickHandler::Command),
            });
        } else if let Some(command) = interactions.on_click {
            self.execute_command(&command);
        } else {
            self.pending_click = None;
        }
    }

    fn handle_mouse_press(&mut self, viewport: Rect, now: Instant) {
        self.flush_pending_click_if_due(now);

        let hit_path = self.hit_path(viewport);
        let Some(hit) = hit_path.last().cloned() else {
            self.end_input_selection_drag();
            self.clear_selected_text();
            self.update_focus(None, None, None, None);
            self.pending_click = None;
            self.pressed_widget = None;
            return;
        };

        if matches!(hit, HitInteraction::CanvasItem { .. }) {
            self.end_input_selection_drag();
            self.clear_selected_text();
            self.update_focus(None, None, None, None);
            self.pending_click = None;
            self.pressed_widget = None;

            for interaction in hit_path.into_iter().rev() {
                match interaction {
                    HitInteraction::CanvasItem {
                        item_id,
                        item_interactions,
                        canvas_origin,
                        item_origin,
                        ..
                    } => {
                        if let Some(command) = item_interactions.on_click {
                            let context = CanvasPointerContext {
                                item_id,
                                canvas_origin,
                                item_origin,
                            };
                            self.execute_click_handler(
                                &ClickHandler::Canvas(command, context),
                                self.cursor_position,
                            );
                            return;
                        }
                    }
                    HitInteraction::Widget {
                        id, interactions, ..
                    } => {
                        self.dispatch_widget_click(HoverTargetId::Widget(id), interactions, now);
                        return;
                    }
                    _ => {}
                }
            }
            return;
        }

        let pointer_position = self.cursor_position;
        let (
            widget_id,
            interactions,
            focus_target,
            focus_input,
            focus_command,
            click_handler,
            input_text,
            input_cursor,
            input_selection,
            selectable_text,
        ) = match hit {
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
                interactions.on_click.clone().map(ClickHandler::Command),
                None,
                None,
                None,
                None,
            ),
            HitInteraction::FocusInput {
                id,
                frame,
                padding,
                interactions,
                text_style,
                text,
                ..
            } => {
                let cursor = pointer_position.map(|point| {
                    input_cursor_index_at_point_with_state(
                        &self.font_manager,
                        &self.theme,
                        self.unit_context(),
                        frame,
                        padding,
                        &text_style,
                        &text,
                        self.focused_input_state(id),
                        point,
                    )
                });
                (
                    id,
                    interactions.clone(),
                    Some(id),
                    Some(id),
                    interactions.on_focus.clone(),
                    interactions.on_click.clone().map(ClickHandler::Command),
                    Some(text.clone()),
                    cursor,
                    cursor.map(|cursor| (id, frame, padding, text_style, text, cursor)),
                    None,
                )
            }
            HitInteraction::SelectableText {
                id,
                frame,
                padding,
                interactions,
                text_style,
                text,
            } => {
                let cursor = pointer_position.map(|point| {
                    text_cursor_index_at_point(
                        &self.font_manager,
                        &self.theme,
                        self.unit_context(),
                        frame,
                        padding,
                        &text_style,
                        &text,
                        point,
                    )
                });
                (
                    id,
                    interactions.clone(),
                    None,
                    None,
                    None,
                    interactions.on_click.clone().map(ClickHandler::Command),
                    None,
                    None,
                    None,
                    cursor.map(|cursor| (id, frame, padding, text_style, text, cursor)),
                )
            }
            HitInteraction::Switch {
                id,
                interactions,
                on_change,
                current,
            } => (
                id,
                interactions.clone(),
                Some(id),
                None,
                interactions.on_focus.clone(),
                on_change
                    .clone()
                    .map(|command| ClickHandler::Toggle(command, !current))
                    .or_else(|| interactions.on_click.clone().map(ClickHandler::Command)),
                None,
                None,
                None,
                None,
            ),
            HitInteraction::CanvasItem { .. } => unreachable!("canvas item handled above"),
        };

        if input_selection.is_none() {
            self.end_input_selection_drag();
        }
        if selectable_text.is_none() {
            self.clear_selected_text();
        }

        self.update_focus(
            focus_target.map(|id| FocusedWidget {
                widget_id: id,
                on_blur: interactions.on_blur.clone(),
            }),
            focus_input,
            focus_command,
            input_text.as_deref(),
        );
        self.pressed_widget = Some(widget_id);

        if input_selection.is_none() {
            if let (Some(input_id), Some(text), Some(cursor)) =
                (focus_input, input_text.as_deref(), input_cursor)
            {
                self.update_input_state(input_id, text, |state| {
                    state.cursor = cursor;
                    state.anchor = cursor;
                    state.composition = None;
                });
            }
        }

        if let Some((widget_id, frame, padding, text_style, text, cursor)) = input_selection {
            self.begin_input_selection(widget_id, frame, padding, text_style, text, cursor);
        }

        if let Some((widget_id, frame, padding, text_style, text, cursor)) = selectable_text {
            self.begin_text_selection(widget_id, frame, padding, text_style, text, cursor);
        }

        if let Some(handler) = click_handler {
            if interactions.on_double_click.is_some() {
                let target_id = HoverTargetId::Widget(widget_id);
                let is_double_click = self
                    .pending_click
                    .as_ref()
                    .map(|pending| pending.target_id == target_id && pending.deadline > now)
                    .unwrap_or(false);

                if is_double_click {
                    self.pending_click = None;
                    if let Some(command) = interactions
                        .on_double_click
                        .clone()
                        .map(ClickHandler::Command)
                    {
                        self.execute_click_handler(&command, self.cursor_position);
                    } else {
                        self.execute_click_handler(&handler, self.cursor_position);
                    }
                } else {
                    self.pending_click = Some(PendingClick {
                        target_id,
                        deadline: now + DOUBLE_CLICK_THRESHOLD,
                        command: Some(handler),
                    });
                }
            } else {
                self.execute_click_handler(&handler, self.cursor_position);
            }
        } else {
            self.dispatch_widget_click(HoverTargetId::Widget(widget_id), interactions, now);
        }
    }

    fn window_id(&self) -> Option<WindowId> {
        self.window_id
    }

    fn create_or_resume_surface(&mut self, event_loop: &dyn ActiveEventLoop) {
        self.set_dialog_proxy(event_loop);

        if self.window.is_some() && self.renderer.is_some() {
            return;
        }

        if self.window.is_some() {
            self.resume_existing_window(event_loop);
            return;
        }

        let mut attributes = WindowAttributes::default()
            .with_transparent(!cfg!(all(target_env = "ohos", feature = "ohos")))
            .with_title(self.config.title.clone())
            .with_surface_size(self.config.size)
            .with_visible(false);

        if let Some(icon_bytes) = self.config.window_icon {
            match image::load_from_memory(icon_bytes) {
                Ok(image) => {
                    let (w, h) = image.dimensions();
                    let rgba = image.into_rgba8().into_raw();

                    match RgbaIcon::new(rgba, w, h) {
                        Ok(ok) => {
                            let icon = Icon::from(ok);
                            attributes = attributes.with_window_icon(Some(icon));
                        }
                        Err(err) => {
                            self.fail(event_loop, TguiError::Icon(err.to_string()));
                        }
                    }
                }
                Err(err) => {
                    self.fail(event_loop, TguiError::Icon(err.to_string()));
                }
            }
        }

        let window: Arc<dyn Window> = match event_loop.create_window(attributes) {
            Ok(window) => Arc::from(window),
            Err(error) => {
                self.fail(event_loop, error.into());
                return;
            }
        };

        self.theme = resolve_theme(
            &self.active_theme_selection(),
            &self.active_theme_set(),
            resolve_window_theme(
                Some(window.as_ref()),
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
                self.theme.colors.background
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

    fn handle_bound_window_event(
        &mut self,
        event_loop: &dyn ActiveEventLoop,
        event: WindowEvent,
    ) -> bool {
        if let WindowEvent::PointerMoved { position, .. } = &event {
            self.set_pointer_position(*position);
        }

        if let WindowEvent::ModifiersChanged(modifiers) = &event {
            self.modifiers = modifiers.state();
        }

        if matches!(event, WindowEvent::PointerLeft { .. }) {
            self.clear_pointer_position();
        }

        if Self::should_dispatch_widget_event(&event) {
            let viewport = self.viewport_rect();
            let previous_focus = self.focused_input;
            let revision_before = self.invalidation.revision();
            let mut needs_redraw = !matches!(event, WindowEvent::PointerMoved { .. });

            match &event {
                WindowEvent::PointerMoved { .. } => {
                    if self.active_scrollbar_drag.is_some() {
                        needs_redraw |= self.handle_scrollbar_drag();
                        needs_redraw |= self.sync_scrollbar_hover();
                        needs_redraw |= self.update_cursor_icon();
                    } else if self.active_input_selection.is_some() {
                        needs_redraw |= self.handle_input_selection_drag();
                        needs_redraw |= self.handle_hover(viewport);
                    } else if self.active_text_selection.is_some() {
                        needs_redraw |= self.handle_text_selection_drag();
                        needs_redraw |= self.handle_hover(viewport);
                    } else {
                        needs_redraw |= self.handle_hover(viewport);
                    }
                }
                WindowEvent::MouseWheel { delta, .. } => {
                    needs_redraw |= self.handle_mouse_wheel(*delta);
                }
                WindowEvent::PointerButton {
                    state: ElementState::Pressed,
                    position,
                    button,
                    ..
                } => {
                    if button.clone().mouse_button() == Some(MouseButton::Left) {
                        self.set_pointer_position(*position);
                        if !self.begin_scrollbar_drag() {
                            self.handle_mouse_press(viewport, Instant::now());
                        } else {
                            needs_redraw = true;
                            needs_redraw |= self.update_cursor_icon();
                        }
                    }
                }
                WindowEvent::KeyboardInput { event, .. } => {
                    if self.focused_input.is_some() {
                        self.handle_input_keyboard_event(event);
                    } else {
                        self.handle_selected_text_keyboard_event(event);
                    }
                }
                _ => {}
            }

            if self.focused_input != previous_focus {
                self.invalidate_scene();
                needs_redraw = true;
            }

            if self.invalidation.revision() != revision_before {
                needs_redraw = true;
            }

            if needs_redraw {
                if let Some(window) = self.window.as_ref() {
                    window.request_redraw();
                }
            }
        }

        if let Some(window_command) = self
            .commands
            .iter()
            .find(|entry| entry.trigger.matches(&event))
            .cloned()
        {
            self.execute_command(&window_command.command);
            if let Some(window) = self.window.as_ref() {
                window.request_redraw();
            }
        }

        match event {
            WindowEvent::CloseRequested => return self.close_policy() == WindowClosePolicy::Close,
            WindowEvent::Focused(false) => {
                self.end_scrollbar_drag();
                self.pressed_widget = None;
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
                        renderer.resize(window.surface_size(), window.scale_factor() as f32);
                    }
                    window.request_redraw();
                }
            }
            WindowEvent::Ime(ime) => {
                self.handle_input_ime(ime);
            }
            WindowEvent::PointerButton {
                state: ElementState::Released,
                position,
                button,
                ..
            } => {
                if button.clone().mouse_button() == Some(MouseButton::Left) {
                    self.set_pointer_position(position);
                    self.end_scrollbar_drag();
                    self.pressed_widget = None;
                    self.end_input_selection_drag();
                    self.end_text_selection_drag();
                    self.handle_hover(self.viewport_rect());
                    self.update_cursor_icon();
                    if let Some(window) = self.window.as_ref() {
                        window.request_redraw();
                    }
                }
            }
            WindowEvent::SurfaceResized(size) => {
                self.invalidate_scene();
                if let Some(renderer) = self.renderer.as_mut() {
                    let scale_factor = self
                        .window
                        .as_ref()
                        .map(|window| window.scale_factor() as f32)
                        .unwrap_or(1.0);
                    renderer.resize(size, scale_factor);
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
                    match self.render_current_frame() {
                        Ok(RenderStatus::Rendered | RenderStatus::SkipFrame) => {}
                        Ok(RenderStatus::ReconfigureSurface) => {}
                        Err(error) => self.fail(event_loop, error),
                    }
                }
                Err(error) => self.fail(event_loop, error),
            },
            _ => {}
        }

        false
    }

    fn handle_bound_about_to_wait(&mut self, event_loop: &dyn ActiveEventLoop) {
        let now = Instant::now();
        let theme_changed = self.refresh_platform_theme();
        let drag_scrolled = self.handle_input_selection_drag();
        if theme_changed {
            self.sync_bindings(now);
            if let Some(window) = self.window.as_ref() {
                window.request_redraw();
            }
        }
        if drag_scrolled {
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
}

struct ResolvedWindowSpec<VM> {
    key: String,
    role: WindowRole,
    config: ApplicationConfig,
    window_bindings: WindowBindings,
    widget_tree: Option<WidgetTree<VM>>,
    commands: Vec<WindowCommand<VM>>,
    close_policy: WindowClosePolicy,
}

struct MultiWindowHandler<VM> {
    config: ApplicationConfig,
    view_model: Arc<Mutex<VM>>,
    windows: WindowSetFactory<VM>,
    invalidation: InvalidationSignal,
    animations: AnimationCoordinator,
    dialog_dispatcher: AsyncDialogDispatcher<VM>,
    dialog_receiver: AsyncDialogReceiver<VM>,
    next_window_instance_id: u64,
    windows_by_key: HashMap<String, BoundRuntimeHandler<VM>>,
    window_keys_by_id: HashMap<WindowId, String>,
    closed_window_keys: HashSet<String>,
    last_window_sync_revision: u64,
    windows_need_sync: bool,
    shutting_down: bool,
    error: Option<TguiError>,
}

impl<VM: ViewModel> MultiWindowHandler<VM> {
    fn new(
        config: ApplicationConfig,
        view_model: Arc<Mutex<VM>>,
        windows: WindowSetFactory<VM>,
        invalidation: InvalidationSignal,
        animations: AnimationCoordinator,
    ) -> Self {
        let (dialog_dispatcher, dialog_receiver) = async_dialog_channel();
        Self {
            config,
            view_model,
            windows,
            invalidation,
            animations,
            dialog_dispatcher,
            dialog_receiver,
            next_window_instance_id: 1,
            windows_by_key: HashMap::new(),
            window_keys_by_id: HashMap::new(),
            closed_window_keys: HashSet::new(),
            last_window_sync_revision: 0,
            windows_need_sync: true,
            shutting_down: false,
            error: None,
        }
    }

    fn fail(&mut self, event_loop: &dyn ActiveEventLoop, error: TguiError) {
        Log::with_tag("tgui-runtime").error(format!("multi-window runtime failed: {error}"));
        self.error = Some(error);
        event_loop.exit();
    }

    fn next_window_instance_id(&mut self) -> u64 {
        let next = self.next_window_instance_id;
        self.next_window_instance_id = self.next_window_instance_id.wrapping_add(1);
        next
    }

    fn set_dialog_proxy(&self, event_loop: &dyn ActiveEventLoop) {
        self.dialog_dispatcher.set_proxy(event_loop.create_proxy());
        self.invalidation.set_proxy(event_loop.create_proxy());
    }

    fn drain_dialog_completions(&mut self) {
        let completions: Vec<_> = self.dialog_receiver.try_iter().collect();
        for completion in completions {
            let Some(window) = self.windows_by_key.get_mut(&completion.window_key) else {
                continue;
            };
            if completion.window_instance_id != window.window_instance_id {
                continue;
            }

            let context = window.command_context();
            window.with_view_model(|view_model| (completion.callback)(view_model, &context));
            window.invalidate_scene();
            self.invalidation.mark_dirty();
            if let Some(native_window) = window.window.as_ref() {
                native_window.request_redraw();
            }
        }
    }

    fn resolve_windows(&self) -> Result<Vec<ResolvedWindowSpec<VM>>, TguiError> {
        let view_model = self.view_model.lock().expect("view model lock poisoned");
        let specs = (self.windows.factory)(&view_model);
        let mut keys = HashSet::new();
        let mut main_window_count = 0usize;
        let mut resolved = Vec::with_capacity(specs.len());

        for spec in specs {
            let key = spec.key.clone();
            if !keys.insert(key.clone()) {
                return Err(TguiError::Unsupported(format!(
                    "window factory returned a duplicate window key: {key}"
                )));
            }

            if matches!(spec.role, WindowRole::Main) {
                main_window_count += 1;
            }

            let widget_tree = if self.windows_by_key.contains_key(&key) {
                None
            } else {
                spec.build_widget_tree(&view_model)
            };

            resolved.push(ResolvedWindowSpec {
                key,
                role: spec.role,
                config: spec.resolved_config(&self.config),
                window_bindings: spec.build_window_bindings(&view_model),
                widget_tree,
                commands: spec.commands,
                close_policy: spec.close_policy,
            });
        }

        if resolved.is_empty() {
            return Ok(resolved);
        }

        if main_window_count != 1 {
            return Err(TguiError::Unsupported(format!(
                "multi-window applications must declare exactly one main window, found {main_window_count}"
            )));
        }

        Ok(resolved)
    }

    fn main_window_is_blocked(&self) -> bool {
        self.windows_by_key
            .values()
            .any(BoundRuntimeHandler::blocks_main_window)
    }

    fn should_gate_main_window_event(event: &WindowEvent) -> bool {
        matches!(
            event,
            WindowEvent::PointerMoved { .. }
                | WindowEvent::PointerLeft { .. }
                | WindowEvent::PointerButton { .. }
                | WindowEvent::MouseWheel { .. }
                | WindowEvent::KeyboardInput { .. }
                | WindowEvent::Ime(_)
                | WindowEvent::ModifiersChanged(_)
        )
    }

    fn sync_windows(&mut self, event_loop: &dyn ActiveEventLoop, force: bool) {
        if self.shutting_down {
            return;
        }

        let revision = self.invalidation.revision();
        if !force
            && !self.windows_need_sync
            && !self.windows_by_key.is_empty()
            && revision == self.last_window_sync_revision
        {
            return;
        }

        let resolved = match self.resolve_windows() {
            Ok(resolved) => resolved,
            Err(error) => {
                self.fail(event_loop, error);
                return;
            }
        };

        let desired_keys: HashSet<String> =
            resolved.iter().map(|window| window.key.clone()).collect();
        self.closed_window_keys
            .retain(|key| desired_keys.contains(key));

        for resolved_window in resolved {
            if self.closed_window_keys.contains(&resolved_window.key) {
                continue;
            }

            let key = resolved_window.key.clone();
            if let Some(window) = self.windows_by_key.get_mut(&key) {
                window.set_definition(
                    resolved_window.role,
                    resolved_window.config,
                    resolved_window.window_bindings,
                    resolved_window.commands,
                    resolved_window.close_policy,
                );
                window.create_or_resume_surface(event_loop);
                if let Some(error) = window.error.take() {
                    self.fail(event_loop, error);
                    return;
                }
                self.window_keys_by_id
                    .retain(|_, existing_key| existing_key != &key);
                if let Some(window_id) = window.window_id() {
                    self.window_keys_by_id.insert(window_id, key);
                }
            } else {
                let mut window = BoundRuntimeHandler::new(
                    key.clone(),
                    self.next_window_instance_id(),
                    resolved_window.role,
                    resolved_window.config,
                    self.view_model.clone(),
                    resolved_window.window_bindings,
                    resolved_window.widget_tree,
                    resolved_window.commands,
                    self.invalidation.clone(),
                    self.animations.clone(),
                    self.dialog_dispatcher.clone(),
                    None,
                    #[cfg(all(target_os = "android", feature = "android"))]
                    None,
                );
                window.close_policy = resolved_window.close_policy;
                window.create_or_resume_surface(event_loop);
                if let Some(error) = window.error.take() {
                    self.fail(event_loop, error);
                    return;
                }
                if let Some(window_id) = window.window_id() {
                    self.window_keys_by_id.insert(window_id, key.clone());
                }
                self.windows_by_key.insert(key, window);
            }
        }

        let stale_keys: Vec<String> = self
            .windows_by_key
            .keys()
            .filter(|key| {
                !desired_keys.contains(*key) || self.closed_window_keys.contains(key.as_str())
            })
            .cloned()
            .collect();

        for key in stale_keys {
            self.remove_window(&key);
        }

        if self.windows_by_key.is_empty() {
            event_loop.exit();
        }

        self.last_window_sync_revision = revision;
        self.windows_need_sync = false;
    }

    fn remove_window(&mut self, key: &str) {
        if let Some(window) = self.windows_by_key.remove(key) {
            if let Some(window_id) = window.window_id() {
                self.window_keys_by_id.remove(&window_id);
            }
        }
    }
}

impl<VM: ViewModel> ApplicationHandler for MultiWindowHandler<VM> {
    fn can_create_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
        self.set_dialog_proxy(event_loop);
        self.sync_windows(event_loop, true);
    }

    fn proxy_wake_up(&mut self, _event_loop: &dyn ActiveEventLoop) {
        self.drain_dialog_completions();
    }

    fn window_event(
        &mut self,
        event_loop: &dyn ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        let Some(key) = self.window_keys_by_id.get(&window_id).cloned() else {
            return;
        };

        let is_main_window = self
            .windows_by_key
            .get(&key)
            .map(BoundRuntimeHandler::is_main_window)
            .unwrap_or(false);

        if is_main_window
            && self.main_window_is_blocked()
            && Self::should_gate_main_window_event(&event)
        {
            return;
        }

        let close_requested = self
            .windows_by_key
            .get_mut(&key)
            .map(|window| window.handle_bound_window_event(event_loop, event))
            .unwrap_or(false);

        if let Some(window) = self.windows_by_key.get_mut(&key) {
            if let Some(error) = window.error.take() {
                self.fail(event_loop, error);
                return;
            }
        }

        if close_requested {
            if is_main_window && self.config.close_children_with_main {
                self.shutting_down = true;
                self.windows_by_key.clear();
                self.window_keys_by_id.clear();
                event_loop.exit();
                return;
            }

            self.closed_window_keys.insert(key.clone());
            self.remove_window(&key);
            if self.windows_by_key.is_empty() {
                event_loop.exit();
            }
        }
    }

    fn about_to_wait(&mut self, event_loop: &dyn ActiveEventLoop) {
        if self.shutting_down {
            event_loop.exit();
            return;
        }

        self.drain_dialog_completions();
        self.sync_windows(event_loop, false);
        if self.error.is_some() {
            return;
        }

        let keys: Vec<String> = self.windows_by_key.keys().cloned().collect();
        for key in keys {
            if let Some(window) = self.windows_by_key.get_mut(&key) {
                window.handle_bound_about_to_wait(event_loop);
                if let Some(error) = window.error.take() {
                    self.fail(event_loop, error);
                    return;
                }
                self.window_keys_by_id
                    .retain(|_, existing_key| existing_key != &key);
                if let Some(window_id) = window.window_id() {
                    self.window_keys_by_id.insert(window_id, key.clone());
                }
            }
        }
    }

    fn suspended(&mut self, _event_loop: &dyn ActiveEventLoop) {
        for window in self.windows_by_key.values_mut() {
            window.suspend();
        }
    }
}

impl<VM: ViewModel> ApplicationHandler for BoundRuntimeHandler<VM> {
    fn can_create_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
        self.create_or_resume_surface(event_loop);
    }

    fn proxy_wake_up(&mut self, _event_loop: &dyn ActiveEventLoop) {
        self.drain_dialog_completions();
    }

    fn window_event(
        &mut self,
        event_loop: &dyn ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        if Some(window_id) != self.window_id {
            return;
        }

        if self.handle_bound_window_event(event_loop, event) {
            event_loop.exit();
        }
    }

    fn about_to_wait(&mut self, event_loop: &dyn ActiveEventLoop) {
        self.drain_dialog_completions();
        self.handle_bound_about_to_wait(event_loop);
    }

    fn suspended(&mut self, _event_loop: &dyn ActiveEventLoop) {
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
        MouseScrollDelta::LineDelta(x, y) => Point::new(x * LINE_SCROLL_STEP, y * LINE_SCROLL_STEP),
        MouseScrollDelta::PixelDelta(position) => Point::new(position.x as f32, position.y as f32),
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

fn text_cursor_index_at_point(
    font_manager: &FontManager,
    theme: &Theme,
    units: UnitContext,
    frame: Rect,
    padding: crate::ui::layout::Insets,
    text_style: &Text,
    current_text: &str,
    point: Point,
) -> usize {
    if current_text.is_empty() {
        return 0;
    }

    let default_style = &theme.components.text.default;
    let font_size = units.resolve_sp(
        text_style
            .font_size
            .unwrap_or(default_style.size.max(sp(1.0))),
    );
    let line_height = (font_size * 1.25).max(font_size + 4.0);
    let letter_spacing =
        units.resolve_sp(text_style.letter_spacing.unwrap_or(default_style.letter_spacing.unwrap_or(Sp::ZERO)));
    let text_request = TextFontRequest {
        preferred_font: text_style
            .font_family
            .as_deref()
            .or(default_style.font_family.as_deref()),
        weight: text_style.font_weight.unwrap_or(default_style.weight),
    };
    let inner = frame.inset(padding);
    let layout = font_manager.measure_text_layout(
        current_text,
        text_request,
        font_size,
        line_height,
        letter_spacing,
    );
    let content_height = inner
        .height
        .min(layout.height.max(line_height))
        .max(Dp::new(line_height));
    let content_frame = Rect::new(
        inner.x,
        inner.y + ((inner.height - content_height).max(0.0) * 0.5),
        inner.width.min(layout.width).max(0.0),
        content_height,
    );
    let local_x = (point.x - content_frame.x).max(0.0);
    layout.index_for_x(local_x.get())
}

fn input_cursor_index_at_point_with_state(
    font_manager: &FontManager,
    theme: &Theme,
    units: UnitContext,
    frame: Rect,
    padding: crate::ui::layout::Insets,
    text_style: &Text,
    current_text: &str,
    edit_state: Option<&InputEditState>,
    point: Point,
) -> usize {
    if current_text.is_empty() {
        return 0;
    }

    let default_style = &theme.components.text.default;
    let font_size = units.resolve_sp(
        text_style
            .font_size
            .unwrap_or(default_style.size.max(sp(1.0))),
    );
    let line_height = (font_size * 1.25).max(font_size + 4.0);
    let letter_spacing =
        units.resolve_sp(text_style.letter_spacing.unwrap_or(default_style.letter_spacing.unwrap_or(Sp::ZERO)));
    let text_request = TextFontRequest {
        preferred_font: text_style
            .font_family
            .as_deref()
            .or(default_style.font_family.as_deref()),
        weight: text_style.font_weight.unwrap_or(default_style.weight),
    };
    let inner = frame.inset(padding);
    let layout = font_manager.measure_text_layout(
        current_text,
        text_request,
        font_size,
        line_height,
        letter_spacing,
    );
    let state = edit_state
        .cloned()
        .unwrap_or_default()
        .clamped_to(current_text);
    let caret_boundary = layout.x_for_index(state.cursor.min(current_text.len()));
    let caret_padding = if state.cursor >= current_text.len() {
        1.0
    } else {
        0.0
    };
    let scrollable_width = layout
        .width
        .max(caret_boundary + caret_padding + 2.0 + INPUT_CARET_EDGE_GAP);
    let InputViewport {
        frame: content_frame,
        ..
    } = input_text_viewport(
        inner,
        layout.width,
        layout.height,
        line_height,
        state.scroll_x.get(),
        scrollable_width,
    );
    let local_x = (point.x - content_frame.x).max(0.0);
    layout.index_for_x(local_x.get())
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

fn resolve_theme(
    selection: &ThemeSelection,
    theme_set: &ThemeSet,
    window_theme: Option<WindowTheme>,
) -> Theme {
    match selection {
        ThemeSelection::System => theme_set
            .resolve_window_theme(window_theme)
            .as_ref()
            .clone(),
        ThemeSelection::Mode(mode) => theme_set.resolve(*mode, window_theme).as_ref().clone(),
        ThemeSelection::Fixed(theme) => theme.clone(),
    }
}

fn resolve_window_theme(
    window: Option<&dyn Window>,
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
fn android_font_scale(android_app: Option<&AndroidApp>) -> Option<f32> {
    let app = android_app?;
    let vm = unsafe { JavaVM::from_raw(app.vm_as_ptr().cast()) };
    let activity_raw = app.activity_as_ptr() as jni::sys::jobject;

    vm.attach_current_thread(|env| -> jni::errors::Result<Option<f32>> {
        let activity = unsafe { env.as_cast_raw::<JObject>(&activity_raw)? };
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
        let scale = env
            .get_field(&configuration, jni_str!("fontScale"), jni_sig!("F"))?
            .f()?;

        Ok((scale.is_finite() && scale > 0.0).then_some(scale))
    })
    .ok()
    .flatten()
}

#[cfg(all(target_os = "android", feature = "android"))]
fn apply_android_system_bar_style(app: &AndroidApp, style: SystemBarStyle) -> Result<(), String> {
    let scheduler_app = app.clone();
    let callback_app = scheduler_app.clone();
    scheduler_app.run_on_java_main_thread(Box::new(move || {
        if let Err(error) = apply_android_system_bar_style_on_main_thread(&callback_app, style) {
            Log::with_tag("tgui-runtime")
                .warn(format!("failed to sync Android system bars: {error}"));
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
    use crate::animation::AnimationCoordinator;
    use crate::application::{ApplicationConfig, ThemeSelection, WindowRole};
    use crate::dialog::async_dialog_channel;
    use crate::foundation::binding::{Binding, InvalidationSignal};
    use crate::foundation::color::Color;
    use crate::foundation::view_model::{Command, ValueCommand};
    use crate::platform::dpi::LogicalSize;
    use crate::platform::event::{ElementState, KeyEvent};
    use crate::platform::keyboard::{Key, KeyCode, KeyLocation, NamedKey, PhysicalKey};
    use crate::text::font::{FontCatalog, TextFontRequest};
    use crate::ui::layout::Axis;
    use crate::ui::theme::{Theme, ThemeMode, ThemeSet};
    use crate::ui::unit::{dp, sp, Dp, Sp, UnitContext};
    use crate::ui::widget::{
        Canvas, CanvasItem, CanvasPath, CanvasPointerEvent, CanvasShadow, CanvasStroke,
        CursorStyle, Flex, HitInteraction, Input, InputEditState, PathBuilder, Point, Text,
        WidgetTree, INPUT_CARET_EDGE_GAP,
    };
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};
    #[cfg(feature = "video")]
    use std::time::Duration;
    use std::time::Instant;
    use crate::{Element, Stack, ViewModelContext, WidgetId};
    use super::{
        input_cursor_index_at_point_with_state, next_grapheme_boundary, normalize_single_line_text,
        previous_grapheme_boundary, text_cursor_index_at_point, BoundRuntimeHandler, CachedScene,
        WindowBindings,
    };

    #[cfg(feature = "video")]
    use crate::media::TextureFrame;
    #[cfg(feature = "video")]
    use crate::video::backend::{
        BackendSharedState, VideoBackend, DEFAULT_VIDEO_BUFFER_MEMORY_LIMIT_BYTES,
    };
    #[cfg(feature = "video")]
    use crate::video::{
        PlaybackState, VideoController, VideoMetrics, VideoSize, VideoSource, VideoSurface,
        VideoSurfaceSnapshot,
    };
    #[cfg(feature = "video")]
    use crate::ViewModelContext;

    #[derive(Default)]
    struct TestVm;

    impl crate::foundation::view_model::ViewModel for TestVm {
        fn new(context: &ViewModelContext) -> Self {
            todo!()
        }

        fn view(&self) -> Element<Self>
        where
            Self: Sized
        {
            todo!()
        }
    }

    fn test_config() -> ApplicationConfig {
        ApplicationConfig {
            title: "test".to_string(),
            size: LogicalSize::new(200.0, 120.0),
            clear_color: Color::BLACK,
            clear_color_overridden: true,
            close_children_with_main: true,
            fonts: FontCatalog::default(),
            theme: ThemeSelection::System,
            theme_set: ThemeSet::default(),
            window_icon: None,
        }
    }

    fn test_config_with_theme(theme: ThemeSelection, theme_set: ThemeSet) -> ApplicationConfig {
        ApplicationConfig {
            title: "test".to_string(),
            size: LogicalSize::new(200.0, 120.0),
            clear_color: Color::BLACK,
            clear_color_overridden: true,
            close_children_with_main: true,
            fonts: FontCatalog::default(),
            theme,
            theme_set,
            window_icon: None,
        }
    }

    fn test_handler(
        widget_tree: Option<WidgetTree<TestVm>>,
        invalidation: InvalidationSignal,
    ) -> BoundRuntimeHandler<TestVm> {
        test_handler_with_vm(TestVm, widget_tree, invalidation)
    }

    fn test_handler_with_vm<VM: crate::foundation::view_model::ViewModel>(
        view_model: VM,
        widget_tree: Option<WidgetTree<VM>>,
        invalidation: InvalidationSignal,
    ) -> BoundRuntimeHandler<VM> {
        test_handler_with_config(view_model, widget_tree, invalidation, test_config())
    }

    fn test_handler_with_config<VM: crate::foundation::view_model::ViewModel>(
        view_model: VM,
        widget_tree: Option<WidgetTree<VM>>,
        invalidation: InvalidationSignal,
        config: ApplicationConfig,
    ) -> BoundRuntimeHandler<VM> {
        let (dialog_dispatcher, dialog_receiver) = async_dialog_channel();
        BoundRuntimeHandler::new(
            "test".to_string(),
            1,
            WindowRole::Main,
            config,
            Arc::new(Mutex::new(view_model)),
            WindowBindings::default(),
            widget_tree,
            Vec::new(),
            invalidation,
            AnimationCoordinator::default(),
            dialog_dispatcher,
            Some(dialog_receiver),
            #[cfg(all(target_os = "android", feature = "android"))]
            None,
        )
    }

    fn key_press_event(code: KeyCode, named: NamedKey) -> KeyEvent {
        KeyEvent {
            physical_key: PhysicalKey::Code(code),
            logical_key: Key::Named(named),
            text: None,
            location: KeyLocation::Standard,
            state: ElementState::Pressed,
            repeat: false,
            text_with_all_modifiers: None,
            key_without_modifiers: Key::Named(named),
        }
    }

    fn custom_theme_set() -> (ThemeSet, Theme, Theme) {
        let mut light = Theme::light();
        light.colors.background = Color::hexa(0xEAF4FFFF);
        light.colors.primary = Color::hexa(0x3366CCFF);
        let mut dark = Theme::dark();
        dark.colors.background = Color::hexa(0x06101DFF);
        dark.colors.primary = Color::hexa(0x66D9E8FF);
        (ThemeSet::new(light.clone(), dark.clone()), light, dark)
    }

    #[test]
    fn bound_theme_modes_resolve_through_configured_theme_set() {
        let invalidation = InvalidationSignal::new();
        let (theme_set, light, dark) = custom_theme_set();
        let mode = Binding::new(|| ThemeMode::Light);
        let mut handler = test_handler_with_config(
            TestVm,
            None,
            invalidation,
            test_config_with_theme(ThemeSelection::System, theme_set),
        );
        handler.window_bindings.theme_mode = Some(mode);

        handler.sync_theme_binding();
        assert_eq!(handler.theme, light);

        handler.window_bindings.theme_mode = Some(Binding::new(|| ThemeMode::Dark));
        handler.sync_theme_binding();
        assert_eq!(handler.theme, dark);
    }

    #[test]
    fn bound_theme_set_updates_current_theme_without_mode_change() {
        let invalidation = InvalidationSignal::new();
        let (theme_set, light, _dark) = custom_theme_set();
        let themes = Arc::new(Mutex::new(theme_set));
        let theme_binding = {
            let themes = themes.clone();
            Binding::new(move || themes.lock().expect("theme set lock poisoned").clone())
        };
        let mut handler = test_handler_with_config(
            TestVm,
            None,
            invalidation,
            test_config_with_theme(ThemeSelection::System, ThemeSet::default()),
        );
        handler.window_bindings.theme_mode = Some(Binding::new(|| ThemeMode::Light));
        handler.window_bindings.theme_set = Some(theme_binding);

        handler.sync_theme_binding();
        assert_eq!(handler.theme, light);

        let mut updated_light = Theme::light();
        updated_light.colors.background = Color::hexa(0xFFFFFFFF);
        updated_light.colors.primary = Color::hexa(0xFFAA00FF);
        themes.lock().expect("theme set lock poisoned").light = Arc::new(updated_light.clone());

        handler.sync_theme_binding();
        assert_eq!(handler.theme, updated_light);
    }

    #[test]
    fn fixed_theme_selection_ignores_configured_theme_set() {
        let invalidation = InvalidationSignal::new();
        let (theme_set, _light, _dark) = custom_theme_set();
        let mut fixed = Theme::dark();
        fixed.colors.primary = Color::hexa(0xFF3366FF);
        let mut handler = test_handler_with_config(
            TestVm,
            None,
            invalidation,
            test_config_with_theme(ThemeSelection::Fixed(fixed.clone()), theme_set),
        );

        handler.sync_theme_binding();
        assert_eq!(handler.theme, fixed);
    }

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

    #[test]
    fn hover_path_reuses_cached_computed_scene() {
        let invalidation = InvalidationSignal::new();
        let resolve_count = Arc::new(AtomicUsize::new(0));
        let child = {
            let resolve_count = resolve_count.clone();
            Binding::new(move || {
                resolve_count.fetch_add(1, Ordering::SeqCst);
                Text::new("hover").cursor(CursorStyle::Pointer)
            })
        };
        let tree = WidgetTree::new(Flex::new(Axis::Vertical).child(child));
        let mut handler = test_handler(Some(tree), invalidation);
        handler.cursor_position = Some(Point::new(dp(10.0), dp(10.0)));

        let viewport = handler.viewport_rect();
        assert_eq!(handler.hover_path(viewport).len(), 1);
        assert_eq!(resolve_count.load(Ordering::SeqCst), 1);

        assert_eq!(handler.hover_path(viewport).len(), 1);
        assert_eq!(resolve_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn scene_cache_invalidates_when_units_change() {
        let invalidation = InvalidationSignal::new();
        let handler = test_handler(None, invalidation);
        let viewport = handler.viewport_rect();
        let cached = CachedScene::<TestVm> {
            viewport,
            units: UnitContext::new(1.0, 1.0),
            focused_input: None,
            pressed_widget: None,
            selected_text: None,
            caret_visible: false,
            animation_epoch: 0,
            scroll_epoch: 0,
            hover_epoch: 0,
            hovered_scrollbar: None,
            active_scrollbar: None,
            layout: None,
            computed: Default::default(),
        };

        assert!(!handler.scene_cache_matches(
            &cached,
            viewport,
            UnitContext::new(1.0, 1.25),
            false,
            None,
        ));
    }

    #[test]
    fn scene_cache_invalidates_when_pressed_widget_changes() {
        let invalidation = InvalidationSignal::new();
        let mut handler = test_handler(None, invalidation);
        let viewport = handler.viewport_rect();
        let cached = CachedScene::<TestVm> {
            viewport,
            units: UnitContext::new(1.0, 1.0),
            focused_input: None,
            pressed_widget: None,
            selected_text: None,
            caret_visible: false,
            animation_epoch: 0,
            scroll_epoch: 0,
            hover_epoch: 0,
            hovered_scrollbar: None,
            active_scrollbar: None,
            layout: None,
            computed: Default::default(),
        };

        handler.pressed_widget = Some(WidgetId::next());

        assert!(!handler.scene_cache_matches(
            &cached,
            viewport,
            UnitContext::new(1.0, 1.0),
            false,
            None,
        ));
    }

    #[test]
    fn user_select_text_defaults_to_text_cursor() {
        let invalidation = InvalidationSignal::new();
        let tree = WidgetTree::new(Text::new("hover").user_select(true));
        let mut handler = test_handler(Some(tree), invalidation);
        handler.cursor_position = Some(Point::new(dp(10.0), dp(10.0)));

        let viewport = handler.viewport_rect();
        let hovered = handler.hover_path(viewport);
        assert_eq!(
            hovered.last().and_then(|hovered| hovered.cursor_style),
            Some(CursorStyle::Text)
        );
    }

    #[test]
    fn dragging_selectable_text_updates_selection_range() {
        let invalidation = InvalidationSignal::new();
        let tree = WidgetTree::new(Text::new("hello").user_select(true));
        let mut handler = test_handler(Some(tree), invalidation);
        let viewport = handler.viewport_rect();

        let (text_id, frame, padding, text_style, text) = {
            let computed = handler.computed_scene();
            computed
                .hit_regions
                .iter()
                .find_map(|region| match &region.interaction {
                    HitInteraction::SelectableText {
                        id,
                        frame,
                        padding,
                        text_style,
                        text,
                        ..
                    } => Some((*id, *frame, *padding, text_style.clone(), text.clone())),
                    _ => None,
                })
                .expect("selectable text hit region should exist")
        };

        handler.cursor_position = Some(Point {
            x: frame.x + 1.0,
            y: frame.y + (frame.height * 0.5),
        });
        handler.handle_mouse_press(viewport, Instant::now());

        handler.cursor_position = Some(Point {
            x: frame.x + frame.width - 1.0,
            y: frame.y + (frame.height * 0.5),
        });
        assert!(handler.handle_text_selection_drag());
        assert_eq!(handler.selected_text, Some(text_id));

        let state = handler
            .selected_text_states
            .get(&text_id)
            .expect("text selection state should be recorded");
        assert_eq!(state.selection_range(), Some((0, text.len())));
        assert_eq!(state.anchor, 0);
        assert_eq!(
            state.cursor,
            text_cursor_index_at_point(
                &handler.font_manager,
                &handler.theme,
                handler.unit_context(),
                frame,
                padding,
                &text_style,
                &text,
                Point {
                    x: frame.x + frame.width - 1.0,
                    y: frame.y + (frame.height * 0.5),
                },
            )
        );
    }

    #[test]
    fn selectable_text_can_provide_selected_content_for_copy() {
        let invalidation = InvalidationSignal::new();
        let tree = WidgetTree::new(Text::new("hello world").user_select(true));
        let mut handler = test_handler(Some(tree), invalidation);
        let text_id = {
            let computed = handler.computed_scene();
            computed
                .hit_regions
                .iter()
                .find_map(|region| match &region.interaction {
                    HitInteraction::SelectableText { id, .. } => Some(*id),
                    _ => None,
                })
                .expect("selectable text hit region should exist")
        };

        handler.selected_text = Some(text_id);
        handler.selected_text_states.insert(
            text_id,
            InputEditState {
                cursor: 11,
                anchor: 6,
                composition: None,
                scroll_x: Dp::ZERO,
            },
        );

        assert_eq!(handler.selected_text_for_copy().as_deref(), Some("world"));
    }

    #[test]
    fn clicking_input_moves_caret_to_pointer_position() {
        let invalidation = InvalidationSignal::new();
        let tree = WidgetTree::new(Input::new(Text::new("hello")).width(dp(220.0)));
        let mut handler = test_handler(Some(tree), invalidation);
        let viewport = handler.viewport_rect();

        let (frame, padding, text_style, content) = {
            let computed = handler.computed_scene();
            let region = computed
                .hit_regions
                .iter()
                .find_map(|region| match &region.interaction {
                    HitInteraction::FocusInput {
                        frame,
                        padding,
                        text_style,
                        text,
                        ..
                    } => Some((*frame, *padding, text_style.clone(), text.clone())),
                    _ => None,
                })
                .expect("input hit region should exist");
            region
        };
        let units = handler.unit_context();
        let font_size = units.resolve_sp(
            text_style
                .font_size
                .unwrap_or(handler.theme.components.text.default.size.max(sp(1.0))),
        );
        let line_height = (font_size * 1.25).max(font_size + 4.0);
        let letter_spacing = units.resolve_sp(
            text_style
                .letter_spacing
                .unwrap_or(handler.theme.components.text.default.letter_spacing.unwrap_or(Sp::ZERO)),
        );
        let request = TextFontRequest {
            preferred_font: text_style.font_family.as_deref().or(handler
                .theme
                .components
                .text
                .default
                .font_family
                .as_deref()),
            weight: text_style
                .font_weight
                .unwrap_or(handler.theme.components.text.default.weight),
        };
        let left = frame.x + padding.left.get();
        let before_target = handler
            .font_manager
            .measure_text_raw(
                &content[..2],
                request.clone(),
                font_size,
                line_height,
                letter_spacing,
            )
            .0;
        let after_target = handler
            .font_manager
            .measure_text_raw(
                &content[..3],
                request,
                font_size,
                line_height,
                letter_spacing,
            )
            .0;

        handler.cursor_position = Some(Point {
            x: left + before_target + ((after_target - before_target) * 0.25),
            y: frame.y + (frame.height * 0.5),
        });
        handler.handle_mouse_press(viewport, Instant::now());

        let input_id = handler.focused_input.expect("input should be focused");
        let state = handler
            .input_states
            .get(&input_id)
            .expect("input state should be recorded");
        assert_eq!(state.cursor, 2);
        assert_eq!(state.anchor, 2);
    }

    #[test]
    fn input_cursor_hit_testing_respects_font_scale() {
        let invalidation = InvalidationSignal::new();
        let tree = WidgetTree::new(Input::new(Text::new("hello")).width(dp(220.0)));
        let mut handler = test_handler(Some(tree), invalidation);

        let (frame, padding, text_style, content) = {
            let computed = handler.computed_scene();
            computed
                .hit_regions
                .iter()
                .find_map(|region| match &region.interaction {
                    HitInteraction::FocusInput {
                        frame,
                        padding,
                        text_style,
                        text,
                        ..
                    } => Some((*frame, *padding, text_style.clone(), text.clone())),
                    _ => None,
                })
                .expect("input hit region should exist")
        };
        let units = UnitContext::new(1.0, 1.5);
        let font_size = units.resolve_sp(
            text_style
                .font_size
                .unwrap_or(handler.theme.components.text.default.size.max(sp(1.0))),
        );
        let line_height = (font_size * 1.25).max(font_size + 4.0);
        let letter_spacing = units.resolve_sp(
            text_style
                .letter_spacing
                .unwrap_or(handler.theme.components.text.default.letter_spacing.unwrap_or(Sp::ZERO)),
        );
        let request = TextFontRequest {
            preferred_font: text_style.font_family.as_deref().or(handler
                .theme
                .components
                .text
                .default
                .font_family
                .as_deref()),
            weight: text_style
                .font_weight
                .unwrap_or(handler.theme.components.text.default.weight),
        };
        let left = frame.x + padding.left.get();
        let before_target = handler
            .font_manager
            .measure_text_raw(
                &content[..2],
                request.clone(),
                font_size,
                line_height,
                letter_spacing,
            )
            .0;
        let after_target = handler
            .font_manager
            .measure_text_raw(
                &content[..3],
                request,
                font_size,
                line_height,
                letter_spacing,
            )
            .0;
        let point = Point {
            x: left + before_target + ((after_target - before_target) * 0.25),
            y: frame.y + (frame.height * 0.5),
        };

        assert_eq!(
            input_cursor_index_at_point_with_state(
                &handler.font_manager,
                &handler.theme,
                units,
                frame,
                padding,
                &text_style,
                &content,
                None,
                point,
            ),
            2
        );
    }

    #[test]
    fn dragging_input_updates_selection_range() {
        let invalidation = InvalidationSignal::new();
        let tree = WidgetTree::new(Input::new(Text::new("hello")).width(dp(220.0)));
        let mut handler = test_handler(Some(tree), invalidation);
        let viewport = handler.viewport_rect();

        let (input_id, frame, padding, text_style, content) = {
            let computed = handler.computed_scene();
            computed
                .hit_regions
                .iter()
                .find_map(|region| match &region.interaction {
                    HitInteraction::FocusInput {
                        id,
                        frame,
                        padding,
                        text_style,
                        text,
                        ..
                    } => Some((*id, *frame, *padding, text_style.clone(), text.clone())),
                    _ => None,
                })
                .expect("input hit region should exist")
        };
        let units = handler.unit_context();
        let font_size = units.resolve_sp(
            text_style
                .font_size
                .unwrap_or(handler.theme.components.text.default.size.max(sp(1.0))),
        );
        let line_height = (font_size * 1.25).max(font_size + 4.0);
        let letter_spacing = units.resolve_sp(
            text_style
                .letter_spacing
                .unwrap_or(handler.theme.components.text.default.letter_spacing.unwrap_or(Sp::ZERO)),
        );
        let request = TextFontRequest {
            preferred_font: text_style.font_family.as_deref().or(handler
                .theme
                .components
                .text
                .default
                .font_family
                .as_deref()),
            weight: text_style
                .font_weight
                .unwrap_or(handler.theme.components.text.default.weight),
        };
        let left = frame.x + padding.left.get();
        let before_start = handler
            .font_manager
            .measure_text_raw(
                &content[..2],
                request.clone(),
                font_size,
                line_height,
                letter_spacing,
            )
            .0;
        let after_start = handler
            .font_manager
            .measure_text_raw(
                &content[..3],
                request.clone(),
                font_size,
                line_height,
                letter_spacing,
            )
            .0;
        let before_end = handler
            .font_manager
            .measure_text_raw(
                &content[..4],
                request.clone(),
                font_size,
                line_height,
                letter_spacing,
            )
            .0;
        let after_end = handler
            .font_manager
            .measure_text_raw(
                &content[..5],
                request,
                font_size,
                line_height,
                letter_spacing,
            )
            .0;

        handler.cursor_position = Some(Point {
            x: left + before_start + ((after_start - before_start) * 0.25),
            y: frame.y + (frame.height * 0.5),
        });
        handler.handle_mouse_press(viewport, Instant::now());

        handler.cursor_position = Some(Point {
            x: left + before_end + ((after_end - before_end) * 0.25),
            y: frame.y + (frame.height * 0.5),
        });
        assert!(handler.handle_input_selection_drag());
        assert_eq!(
            handler.focused_input,
            Some(input_id),
            "input should stay focused while selecting"
        );

        let state = handler
            .input_states
            .get(&input_id)
            .expect("input state should be recorded");
        assert_eq!(state.anchor, 2);
        assert_eq!(state.cursor, 4);
        assert_eq!(state.selection_range(), Some((2, 4)));
    }

    #[test]
    fn long_input_clicking_visible_text_uses_scrolled_viewport() {
        let invalidation = InvalidationSignal::new();
        let content = "https://example.com/a/very/long/input/value/that/needs/horizontal/scrolling";
        let tree = WidgetTree::new(Input::new(Text::new(content)).width(dp(180.0)));
        let mut handler = test_handler(Some(tree), invalidation);

        let (input_id, frame, padding, text_style, text) = {
            let computed = handler.computed_scene();
            computed
                .hit_regions
                .iter()
                .find_map(|region| match &region.interaction {
                    HitInteraction::FocusInput {
                        id,
                        frame,
                        padding,
                        text_style,
                        text,
                        ..
                    } => Some((*id, *frame, *padding, text_style.clone(), text.clone())),
                    _ => None,
                })
                .expect("input hit region should exist")
        };

        let units = handler.unit_context();
        let font_size = units.resolve_sp(
            text_style
                .font_size
                .unwrap_or(handler.theme.components.text.default.size.max(sp(1.0))),
        );
        let line_height = (font_size * 1.25).max(font_size + 4.0);
        let letter_spacing = units.resolve_sp(
            text_style
                .letter_spacing
                .unwrap_or(handler.theme.components.text.default.letter_spacing.unwrap_or(Sp::ZERO)),
        );
        let request = TextFontRequest {
            preferred_font: text_style.font_family.as_deref().or(handler
                .theme
                .components
                .text
                .default
                .font_family
                .as_deref()),
            weight: text_style
                .font_weight
                .unwrap_or(handler.theme.components.text.default.weight),
        };
        let layout = handler.font_manager.measure_text_layout(
            &text,
            request.clone(),
            font_size,
            line_height,
            letter_spacing,
        );
        let visible_cursor = text.len().saturating_sub(10);
        let target_cursor = text.len();
        let scroll_x = Dp::new(
            layout
                .x_for_index(visible_cursor)
                .max(layout.x_for_index(target_cursor) - frame.inset(padding).width.get() + 8.0),
        );
        handler.focused_input = Some(input_id);
        handler.input_states.insert(
            input_id,
            InputEditState {
                cursor: target_cursor,
                anchor: target_cursor,
                composition: None,
                scroll_x,
            },
        );

        let visible_right = Point {
            x: frame.right() - dp(8.0),
            y: frame.y + (frame.height * 0.5),
        };
        let target = input_cursor_index_at_point_with_state(
            &handler.font_manager,
            &handler.theme,
            handler.unit_context(),
            frame,
            padding,
            &text_style,
            &text,
            handler.input_states.get(&input_id),
            visible_right,
        );

        assert!(target >= visible_cursor);
    }

    #[test]
    fn clicking_scrolled_input_keeps_scroll_offset() {
        let invalidation = InvalidationSignal::new();
        let content = "https://example.com/a/very/long/input/value/that/needs/horizontal/scrolling";
        let tree = WidgetTree::new(Input::new(Text::new(content)).width(dp(180.0)));
        let mut handler = test_handler(Some(tree), invalidation);
        let viewport = handler.viewport_rect();

        let (input_id, frame, padding, text_style, text) = {
            let computed = handler.computed_scene();
            computed
                .hit_regions
                .iter()
                .find_map(|region| match &region.interaction {
                    HitInteraction::FocusInput {
                        id,
                        frame,
                        padding,
                        text_style,
                        text,
                        ..
                    } => Some((*id, *frame, *padding, text_style.clone(), text.clone())),
                    _ => None,
                })
                .expect("input hit region should exist")
        };
        let units = handler.unit_context();
        let font_size = units.resolve_sp(
            text_style
                .font_size
                .unwrap_or(handler.theme.components.text.default.size.max(sp(1.0))),
        );
        let line_height = (font_size * 1.25).max(font_size + 4.0);
        let letter_spacing = units.resolve_sp(
            text_style
                .letter_spacing
                .unwrap_or(handler.theme.components.text.default.letter_spacing.unwrap_or(Sp::ZERO)),
        );
        let request = TextFontRequest {
            preferred_font: text_style.font_family.as_deref().or(handler
                .theme
                .components
                .text
                .default
                .font_family
                .as_deref()),
            weight: text_style
                .font_weight
                .unwrap_or(handler.theme.components.text.default.weight),
        };
        let layout = handler.font_manager.measure_text_layout(
            &text,
            request.clone(),
            font_size,
            line_height,
            letter_spacing,
        );
        let target_cursor = text.len().saturating_sub(8);
        let next_cursor = text.len().saturating_sub(6);
        let scroll_x = Dp::new((layout.x_for_index(target_cursor) - 24.0).max(0.0));
        let visible_x = frame.inset(padding).x + layout.x_for_index(next_cursor) - scroll_x;

        handler.focused_input = Some(input_id);
        handler.input_states.insert(
            input_id,
            InputEditState {
                cursor: target_cursor,
                anchor: target_cursor,
                composition: None,
                scroll_x,
            },
        );
        handler.cursor_position = Some(Point {
            x: visible_x,
            y: frame.y + (frame.height * 0.5),
        });
        let expected_cursor = input_cursor_index_at_point_with_state(
            &handler.font_manager,
            &handler.theme,
            handler.unit_context(),
            frame,
            padding,
            &text_style,
            &text,
            handler.input_states.get(&input_id),
            handler
                .cursor_position
                .expect("cursor position should be set"),
        );

        handler.handle_mouse_press(viewport, Instant::now());

        let state = handler
            .input_states
            .get(&input_id)
            .expect("input state should be recorded");
        assert_eq!(state.cursor, expected_cursor);
        assert_eq!(state.scroll_x, scroll_x);
    }

    #[test]
    fn keyboard_navigation_scrolls_input_to_keep_caret_visible() {
        let invalidation = InvalidationSignal::new();
        let content = "https://example.com/a/very/long/input/value/that/needs/horizontal/scrolling";
        let tree = WidgetTree::new(Input::new(Text::new(content)).width(dp(180.0)));
        let mut handler = test_handler(Some(tree), invalidation);

        let input_id = {
            let computed = handler.computed_scene();
            computed
                .hit_regions
                .iter()
                .find_map(|region| match &region.interaction {
                    HitInteraction::FocusInput { id, .. } => Some(*id),
                    _ => None,
                })
                .expect("input hit region should exist")
        };
        handler.focused_input = Some(input_id);
        handler.input_states.insert(
            input_id,
            InputEditState {
                cursor: 0,
                anchor: 0,
                composition: None,
                scroll_x: Dp::ZERO,
            },
        );

        for _ in 0..content.len().min(48) {
            handler.handle_input_keyboard_event(&key_press_event(
                KeyCode::ArrowRight,
                NamedKey::ArrowRight,
            ));
        }

        let state = handler
            .input_states
            .get(&input_id)
            .expect("input state should be recorded");
        assert!(state.cursor > 0);
        assert!(state.scroll_x > Dp::ZERO);
    }

    #[test]
    fn keyboard_navigation_keeps_scroll_when_caret_stays_visible() {
        let invalidation = InvalidationSignal::new();
        let content = "https://example.com/a/very/long/input/value/that/needs/horizontal/scrolling";
        let tree = WidgetTree::new(Input::new(Text::new(content)).width(dp(180.0)));
        let mut handler = test_handler(Some(tree), invalidation);

        let (input_id, frame, padding, text_style, text) = {
            let computed = handler.computed_scene();
            computed
                .hit_regions
                .iter()
                .find_map(|region| match &region.interaction {
                    HitInteraction::FocusInput {
                        id,
                        frame,
                        padding,
                        text_style,
                        text,
                        ..
                    } => Some((*id, *frame, *padding, text_style.clone(), text.clone())),
                    _ => None,
                })
                .expect("input hit region should exist")
        };

        let units = handler.unit_context();
        let font_size = units.resolve_sp(
            text_style
                .font_size
                .unwrap_or(handler.theme.components.text.default.size.max(sp(1.0))),
        );
        let line_height = (font_size * 1.25).max(font_size + 4.0);
        let letter_spacing = units.resolve_sp(
            text_style
                .letter_spacing
                .unwrap_or(handler.theme.components.text.default.letter_spacing.unwrap_or(Sp::ZERO)),
        );
        let request = TextFontRequest {
            preferred_font: text_style.font_family.as_deref().or(handler
                .theme
                .components
                .text
                .default
                .font_family
                .as_deref()),
            weight: text_style
                .font_weight
                .unwrap_or(handler.theme.components.text.default.weight),
        };
        let layout = handler.font_manager.measure_text_layout(
            &text,
            request,
            font_size,
            line_height,
            letter_spacing,
        );
        let cursor = text
            .char_indices()
            .nth(36)
            .map(|(index, _)| index)
            .unwrap_or(text.len());
        let scroll_x =
            Dp::new((layout.x_for_index(cursor) - frame.inset(padding).width.get() * 0.5).max(0.0));

        handler.focused_input = Some(input_id);
        handler.input_states.insert(
            input_id,
            InputEditState {
                cursor,
                anchor: cursor,
                composition: None,
                scroll_x,
            },
        );

        handler
            .handle_input_keyboard_event(&key_press_event(KeyCode::ArrowLeft, NamedKey::ArrowLeft));
        let state = handler
            .input_states
            .get(&input_id)
            .expect("input state should be recorded");
        assert_eq!(state.scroll_x, scroll_x);

        handler.handle_input_keyboard_event(&key_press_event(
            KeyCode::ArrowRight,
            NamedKey::ArrowRight,
        ));
        let state = handler
            .input_states
            .get(&input_id)
            .expect("input state should be recorded");
        assert_eq!(state.scroll_x, scroll_x);
    }

    #[test]
    fn end_key_keeps_long_input_caret_visible_with_right_gap() {
        let invalidation = InvalidationSignal::new();
        let content = "https://example.com/a/very/long/input/value/that/needs/horizontal/scrolling";
        let tree = WidgetTree::new(Input::new(Text::new(content)).width(dp(180.0)));
        let mut handler = test_handler(Some(tree), invalidation);

        let (input_id, frame, padding) = {
            let computed = handler.computed_scene();
            computed
                .hit_regions
                .iter()
                .find_map(|region| match &region.interaction {
                    HitInteraction::FocusInput {
                        id, frame, padding, ..
                    } => Some((*id, *frame, *padding)),
                    _ => None,
                })
                .expect("input hit region should exist")
        };
        handler.focused_input = Some(input_id);
        handler.caret_blink_started_at = None;
        handler.input_states.insert(
            input_id,
            InputEditState {
                cursor: 0,
                anchor: 0,
                composition: None,
                scroll_x: Dp::ZERO,
            },
        );

        handler.handle_input_keyboard_event(&key_press_event(KeyCode::End, NamedKey::End));

        let caret = handler
            .computed_scene()
            .ime_cursor_area
            .expect("focused input should expose caret rect");
        let input_clip = frame.inset(padding);
        assert!(caret.right() <= input_clip.right() - dp(INPUT_CARET_EDGE_GAP));
        assert!(caret.x >= input_clip.x);
        let state = handler
            .input_states
            .get(&input_id)
            .expect("input state should be recorded");
        assert_eq!(state.cursor, content.len());
        assert!(state.scroll_x > Dp::ZERO);
    }

    #[test]
    fn home_key_returns_long_input_scroll_to_start() {
        let invalidation = InvalidationSignal::new();
        let content = "https://example.com/a/very/long/input/value/that/needs/horizontal/scrolling";
        let tree = WidgetTree::new(Input::new(Text::new(content)).width(dp(180.0)));
        let mut handler = test_handler(Some(tree), invalidation);

        let (input_id, frame, padding) = {
            let computed = handler.computed_scene();
            computed
                .hit_regions
                .iter()
                .find_map(|region| match &region.interaction {
                    HitInteraction::FocusInput {
                        id, frame, padding, ..
                    } => Some((*id, *frame, *padding)),
                    _ => None,
                })
                .expect("input hit region should exist")
        };
        handler.focused_input = Some(input_id);
        handler.caret_blink_started_at = None;
        handler.input_states.insert(
            input_id,
            InputEditState {
                cursor: content.len(),
                anchor: content.len(),
                composition: None,
                scroll_x: dp(240.0),
            },
        );

        handler.handle_input_keyboard_event(&key_press_event(KeyCode::Home, NamedKey::Home));

        let caret = handler
            .computed_scene()
            .ime_cursor_area
            .expect("focused input should expose caret rect");
        let input_clip = frame.inset(padding);
        assert!(caret.x >= input_clip.x);
        assert!(caret.x <= input_clip.x + dp(INPUT_CARET_EDGE_GAP));
        let state = handler
            .input_states
            .get(&input_id)
            .expect("input state should be recorded");
        assert_eq!(state.cursor, 0);
        assert_eq!(state.scroll_x, Dp::ZERO);
    }

    #[test]
    fn dragging_input_beyond_right_edge_scrolls_horizontally() {
        let invalidation = InvalidationSignal::new();
        let content = "https://example.com/a/very/long/input/value/that/needs/horizontal/scrolling";
        let tree = WidgetTree::new(Input::new(Text::new(content)).width(dp(180.0)));
        let mut handler = test_handler(Some(tree), invalidation);
        let viewport = handler.viewport_rect();

        let (input_id, frame, padding) = {
            let computed = handler.computed_scene();
            computed
                .hit_regions
                .iter()
                .find_map(|region| match &region.interaction {
                    HitInteraction::FocusInput {
                        id, frame, padding, ..
                    } => Some((*id, *frame, *padding)),
                    _ => None,
                })
                .expect("input hit region should exist")
        };

        handler.cursor_position = Some(Point {
            x: frame.inset(padding).x + dp(4.0),
            y: frame.y + (frame.height * 0.5),
        });
        handler.handle_mouse_press(viewport, Instant::now());

        handler.cursor_position = Some(Point {
            x: frame.right() + dp(40.0),
            y: frame.y + (frame.height * 0.5),
        });
        assert!(handler.handle_input_selection_drag());

        let state = handler
            .input_states
            .get(&input_id)
            .expect("input state should be recorded");
        assert!(state.scroll_x > Dp::ZERO);
        assert!(state.cursor > state.anchor);
    }

    #[derive(Default)]
    struct SwitchVm {
        checked: bool,
    }

    impl crate::foundation::view_model::ViewModel for SwitchVm {
        fn new(context: &ViewModelContext) -> Self {
            todo!()
        }

        fn view(&self) -> Element<Self>
        where
            Self: Sized
        {
            todo!()
        }
    }

    #[test]
    fn clicking_switch_dispatches_toggled_value() {
        let invalidation = InvalidationSignal::new();
        let tree = WidgetTree::new(
            crate::ui::widget::Switch::new(false)
                .on_change(ValueCommand::new(|vm: &mut SwitchVm, value| {
                    vm.checked = value
                }))
                .size(dp(52.0), dp(30.0)),
        );
        let mut handler = test_handler_with_vm(SwitchVm::default(), Some(tree), invalidation);
        let viewport = handler.viewport_rect();

        let frame = {
            let computed = handler.computed_scene();
            computed
                .hit_regions
                .iter()
                .find_map(|region| match &region.interaction {
                    HitInteraction::Switch { .. } => Some(region.rect),
                    _ => None,
                })
                .expect("switch hit region should exist")
        };

        handler.cursor_position = Some(Point {
            x: frame.x + (frame.width * 0.5),
            y: frame.y + (frame.height * 0.5),
        });
        handler.handle_mouse_press(viewport, Instant::now());

        let checked = handler.with_view_model(|vm| vm.checked);
        assert!(checked);
    }

    #[cfg(feature = "video")]
    struct MockVideoBackend;

    #[cfg(feature = "video")]
    impl VideoBackend for MockVideoBackend {
        fn load(&self, _source: VideoSource) -> Result<(), crate::TguiError> {
            Ok(())
        }

        fn play(&self) {}

        fn pause(&self) {}

        fn seek(&self, _position: Duration) {}

        fn set_volume(&self, _volume: f32) {}

        fn set_muted(&self, _muted: bool) {}

        fn set_buffer_memory_limit_bytes(&self, _bytes: u64) {}

        fn current_frame(&self) -> Option<Arc<TextureFrame>> {
            None
        }

        fn shutdown(&self) {}
    }

    #[cfg(feature = "video")]
    #[test]
    fn hover_path_keeps_video_surface_hit_testing_when_scene_is_cached() {
        let invalidation = InvalidationSignal::new();
        let animations = AnimationCoordinator::default();
        let ctx = ViewModelContext::new(invalidation.clone(), animations.clone());
        let shared = BackendSharedState {
            playback_state: ctx.observable(PlaybackState::Ready),
            metrics: ctx.observable(VideoMetrics::default()),
            volume: ctx.observable(1.0),
            muted: ctx.observable(false),
            buffer_memory_limit_bytes: ctx.observable(DEFAULT_VIDEO_BUFFER_MEMORY_LIMIT_BYTES),
            video_size: ctx.observable(VideoSize {
                width: 160,
                height: 90,
            }),
            error: ctx.observable(None),
            surface: ctx.observable(VideoSurfaceSnapshot::default()),
        };
        let controller = VideoController::from_parts(shared, Arc::new(MockVideoBackend));
        let tree = WidgetTree::new(
            VideoSurface::new(controller)
                .size(dp(160.0), dp(90.0))
                .cursor(CursorStyle::Pointer),
        );
        let mut handler = test_handler(Some(tree), invalidation);
        handler.cursor_position = Some(Point::new(dp(10.0), dp(10.0)));

        let viewport = handler.viewport_rect();
        assert_eq!(handler.hover_path(viewport).len(), 1);
        assert_eq!(handler.hover_path(viewport).len(), 1);
    }

    #[derive(Default)]
    struct CanvasEventVm {
        hover_events: Vec<CanvasPointerEvent>,
        clicks: usize,
        widget_clicks: usize,
    }

    impl crate::foundation::view_model::ViewModel for CanvasEventVm {
        fn new(context: &ViewModelContext) -> Self {
            Self {
                hover_events: vec![],
                clicks: 0,
                widget_clicks: 0,
            }
        }

        fn view(&self) -> Element<Self>
        where
            Self: Sized
        {
            Stack::new().into()
        }
    }

    #[test]
    fn canvas_item_hover_dispatches_canvas_pointer_payload() {
        let invalidation = InvalidationSignal::new();
        let tree = WidgetTree::new(
            Canvas::new(vec![CanvasItem::Path(
                CanvasPath::new(
                    7_u64,
                    PathBuilder::new()
                        .move_to(10.0, 10.0)
                        .line_to(60.0, 10.0)
                        .line_to(60.0, 40.0)
                        .line_to(10.0, 40.0)
                        .close(),
                )
                .fill(Color::WHITE),
            )])
            .size(dp(100.0), dp(80.0))
            .on_item_mouse_move(ValueCommand::new(
                |vm: &mut CanvasEventVm, event| {
                    vm.hover_events.push(event);
                },
            )),
        );
        let mut handler = test_handler_with_vm(CanvasEventVm::default(), Some(tree), invalidation);
        handler.cursor_position = Some(Point::new(dp(25.0), dp(20.0)));

        handler.handle_hover(handler.viewport_rect());

        let view_model = handler
            .view_model
            .lock()
            .expect("view model lock should not be poisoned");
        assert_eq!(view_model.hover_events.len(), 1);
        assert_eq!(view_model.hover_events[0].item_id, 7_u64.into());
        assert_eq!(
            view_model.hover_events[0].canvas_position,
            Point::new(25.0, 20.0)
        );
        assert_eq!(
            view_model.hover_events[0].local_position,
            Point::new(15.0, 10.0)
        );
    }

    #[test]
    fn canvas_item_click_takes_priority_over_widget_click() {
        let invalidation = InvalidationSignal::new();
        let tree = WidgetTree::new(
            Canvas::new(vec![CanvasItem::Path(
                CanvasPath::new(
                    11_u64,
                    PathBuilder::new()
                        .move_to(10.0, 10.0)
                        .line_to(60.0, 10.0)
                        .line_to(60.0, 40.0)
                        .line_to(10.0, 40.0)
                        .close(),
                )
                .fill(Color::WHITE),
            )])
            .size(dp(100.0), dp(80.0))
            .on_click(Command::new(|vm: &mut CanvasEventVm| {
                vm.widget_clicks += 1;
            }))
            .on_item_click(ValueCommand::new(|vm: &mut CanvasEventVm, _event| {
                vm.clicks += 1;
            })),
        );
        let mut handler = test_handler_with_vm(CanvasEventVm::default(), Some(tree), invalidation);
        handler.cursor_position = Some(Point::new(dp(20.0), dp(20.0)));

        handler.handle_mouse_press(handler.viewport_rect(), Instant::now());

        let view_model = handler
            .view_model
            .lock()
            .expect("view model lock should not be poisoned");
        assert_eq!(view_model.clicks, 1);
        assert_eq!(view_model.widget_clicks, 0);
    }

    #[test]
    fn dashed_canvas_item_hit_testing_skips_gaps() {
        let make_tree = || {
            WidgetTree::new(
                Canvas::new(vec![CanvasItem::Path(
                    CanvasPath::new(
                        21_u64,
                        PathBuilder::new().move_to(10.0, 20.0).line_to(90.0, 20.0),
                    )
                    .stroke(CanvasStroke::new(dp(6.0), Color::WHITE).dash([dp(10.0), dp(10.0)])),
                )])
                .size(dp(100.0), dp(60.0))
                .on_item_mouse_move(ValueCommand::new(
                    |vm: &mut CanvasEventVm, event| {
                        vm.hover_events.push(event);
                    },
                )),
            )
        };

        let mut hit_handler = test_handler_with_vm(
            CanvasEventVm::default(),
            Some(make_tree()),
            InvalidationSignal::new(),
        );
        hit_handler.cursor_position = Some(Point::new(dp(15.0), dp(20.0)));
        hit_handler.handle_hover(hit_handler.viewport_rect());
        let hit_vm = hit_handler
            .view_model
            .lock()
            .expect("view model lock should not be poisoned");
        assert_eq!(hit_vm.hover_events.len(), 1);
        drop(hit_vm);

        let mut gap_handler = test_handler_with_vm(
            CanvasEventVm::default(),
            Some(make_tree()),
            InvalidationSignal::new(),
        );
        gap_handler.cursor_position = Some(Point::new(dp(25.0), dp(20.0)));
        gap_handler.handle_hover(gap_handler.viewport_rect());
        let gap_vm = gap_handler
            .view_model
            .lock()
            .expect("view model lock should not be poisoned");
        assert!(gap_vm.hover_events.is_empty());
    }

    #[test]
    fn canvas_shadow_does_not_extend_item_hit_region() {
        let invalidation = InvalidationSignal::new();
        let tree = WidgetTree::new(
            Canvas::new(vec![CanvasItem::Path(
                CanvasPath::new(
                    31_u64,
                    PathBuilder::new()
                        .move_to(10.0, 10.0)
                        .line_to(40.0, 10.0)
                        .line_to(40.0, 40.0)
                        .line_to(10.0, 40.0)
                        .close(),
                )
                .fill(Color::WHITE)
                .shadow(CanvasShadow::new(
                    Color::BLACK,
                    Point::new(18.0, 0.0),
                    dp(8.0),
                )),
            )])
            .size(dp(100.0), dp(80.0))
            .on_item_mouse_move(ValueCommand::new(
                |vm: &mut CanvasEventVm, event| {
                    vm.hover_events.push(event);
                },
            )),
        );
        let mut handler = test_handler_with_vm(CanvasEventVm::default(), Some(tree), invalidation);
        handler.cursor_position = Some(Point::new(dp(55.0), dp(25.0)));

        handler.handle_hover(handler.viewport_rect());

        let view_model = handler
            .view_model
            .lock()
            .expect("view model lock should not be poisoned");
        assert!(view_model.hover_events.is_empty());
    }
}
