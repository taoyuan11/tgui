mod commands;
mod input;
mod theme;

#[cfg(all(target_os = "android", feature = "android"))]
use self::theme::{
    android_font_scale, apply_android_system_bar_style, is_light_color, SystemBarStyle,
};
use self::theme::{resolve_theme, resolve_window_theme};
use crate::animation::{
    default_theme_transition, AnimationCoordinator, AnimationEngine, AnimationKey, Transition,
    WindowProperty,
};
use crate::application::{
    ApplicationConfig, ThemeSelection, WindowClosePolicy, WindowRole, WindowSetFactory,
};
use crate::dialog::{async_dialog_channel, AsyncDialogDispatcher, AsyncDialogReceiver};
use crate::foundation::binding::{Binding, InvalidationSignal};
use crate::foundation::color::Color;
use crate::foundation::error::TguiError;
use crate::foundation::event::InputTrigger;
use crate::foundation::view_model::{Command, ValueCommand, ViewModel};
use crate::foundation::window_control::WindowRequestQueue;
use crate::log::Log;
use crate::media::MediaManager;
#[cfg(target_os = "windows")]
use crate::notification::prepare_platform_notifications;
use crate::notification::{
    async_notification_channel, AsyncNotificationDispatcher, AsyncNotificationReceiver,
};
#[cfg(all(target_os = "android", feature = "android"))]
use crate::platform::android::activity::AndroidApp;
use crate::platform::backend::application::ApplicationHandler;
use crate::platform::backend::event_loop::{ActiveEventLoop, ControlFlow};
use crate::platform::backend::window::Window;
use crate::platform::backend::EventLoop;
use crate::platform::cursor::CursorIcon;
use crate::platform::dpi::{LogicalSize, PhysicalPosition, PhysicalSize};
use crate::platform::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use crate::platform::keyboard::ModifiersState;
#[cfg(all(target_env = "ohos", feature = "ohos"))]
use crate::platform::ohos::{OhosApp, WindowExtOhos};
use crate::platform::window::{
    ImeRequest, ResizeDirection, Theme as WindowTheme, WindowAttributes, WindowId,
};
use crate::rendering::renderer::{RenderStatus, Renderer};
use crate::text::font::{FontManager, TextFontRequest};
use crate::ui::theme::{Theme, ThemeMode, ThemeSet, ThemeStore};
use crate::ui::unit::{dp, sp, Dp, Sp, UnitContext};
use crate::ui::widget::{
    CanvasDragEvent, CanvasItemId, CanvasMouseButton, CanvasMouseEvent, CanvasPointerEvent,
    CanvasWheelEvent, ComputedScene, MediaEventPhase, MediaEventState, Point, Rect,
    ResolvedSceneLayout, ScrollRegion, ScrollbarHandle, Text, TextEditState, WidgetId,
    WidgetStateMap, WidgetTree,
};
use image::GenericImageView;
#[cfg(any(target_os = "windows", target_os = "macos"))]
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use winit_core::icon::{Icon, RgbaIcon};
#[cfg(target_os = "windows")]
use winit_win32::{WindowAttributesWindows, WindowExtWindows};

const DOUBLE_CLICK_THRESHOLD: Duration = Duration::from_millis(300);
#[cfg(all(target_os = "android", feature = "android"))]
const ANDROID_SYSTEM_THEME_POLL_INTERVAL: Duration = Duration::from_millis(500);

#[cfg(any(target_os = "windows", target_os = "macos"))]
#[derive(Clone, Copy)]
struct NativeModalParent {
    window: RawWindowHandle,
}

#[cfg(any(target_os = "windows", target_os = "macos"))]
impl NativeModalParent {
    fn from_window(window: &dyn Window) -> Option<Self> {
        Some(Self {
            window: window.window_handle().ok()?.as_raw(),
        })
    }
}

#[cfg(target_os = "windows")]
fn configure_native_modal_window(
    attributes: WindowAttributes,
    parent: &dyn Window,
) -> WindowAttributes {
    let Some(parent) = NativeModalParent::from_window(parent) else {
        return attributes;
    };

    match parent.window {
        RawWindowHandle::Win32(handle) => attributes.with_platform_attributes(Box::new(
            WindowAttributesWindows::default()
                .with_owner_window(handle.hwnd.get() as *mut core::ffi::c_void),
        )),
        _ => attributes,
    }
}

#[cfg(target_os = "macos")]
fn configure_native_modal_window(
    attributes: WindowAttributes,
    parent: &dyn Window,
) -> WindowAttributes {
    let Some(parent) = NativeModalParent::from_window(parent) else {
        return attributes;
    };

    unsafe { attributes.with_parent_window(Some(parent.window)) }
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
fn configure_native_modal_window(
    attributes: WindowAttributes,
    _parent: &dyn Window,
) -> WindowAttributes {
    attributes
}

fn window_sync_priority(role: WindowRole) -> u8 {
    match role {
        WindowRole::Main => 0,
        WindowRole::Child { .. } => 1,
    }
}

fn centered_window_position_for_monitor(
    monitor_position: Option<PhysicalPosition<i32>>,
    monitor_size: PhysicalSize<u32>,
    monitor_scale_factor: f64,
    window_size: LogicalSize<f64>,
) -> Option<PhysicalPosition<i32>> {
    let monitor_position = monitor_position?;
    let monitor_scale_factor = if monitor_scale_factor.is_finite() && monitor_scale_factor > 0.0 {
        monitor_scale_factor
    } else {
        1.0
    };

    let window_width = (window_size.width.max(1.0) * monitor_scale_factor).round() as i64;
    let window_height = (window_size.height.max(1.0) * monitor_scale_factor).round() as i64;
    let horizontal_gap = (i64::from(monitor_size.width) - window_width).max(0);
    let vertical_gap = (i64::from(monitor_size.height) - window_height).max(0);

    let x = i64::from(monitor_position.x) + horizontal_gap / 2;
    let y = i64::from(monitor_position.y) + vertical_gap / 2;

    Some(PhysicalPosition::new(
        x.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32,
        y.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32,
    ))
}

fn default_window_position(
    event_loop: &dyn ActiveEventLoop,
    window_size: LogicalSize<f64>,
) -> Option<PhysicalPosition<i32>> {
    let monitor = event_loop
        .primary_monitor()
        .or_else(|| event_loop.available_monitors().next())?;
    let monitor_size = monitor
        .current_video_mode()
        .map(|mode| mode.size())
        .or_else(|| monitor.video_modes().next().map(|mode| mode.size()))?;
    centered_window_position_for_monitor(
        monitor.position(),
        monitor_size,
        monitor.scale_factor(),
        window_size,
    )
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
        prepare_notifications_for_runtime(&self.config);
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
        let (notification_dispatcher, notification_receiver) = async_notification_channel();
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
            notification_dispatcher,
            Some(notification_receiver),
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
        let (notification_dispatcher, notification_receiver) = async_notification_channel();
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
            notification_dispatcher,
            Some(notification_receiver),
            #[cfg(all(target_os = "android", feature = "android"))]
            self.android_app,
        );
        (self.event_loop, handler)
    }
}

fn prepare_notifications_for_runtime(config: &ApplicationConfig) {
    #[cfg(target_os = "windows")]
    {
        if let Some(app_id) = config.app_id.as_deref() {
            if let Err(error) = prepare_platform_notifications(Some(app_id), &config.title) {
                Log::with_tag("tgui-runtime")
                    .warn(format!("failed to prepare Windows notifications: {error}"));
            }
        }
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
    cursor_position: Option<Point>,
    modifiers: ModifiersState,
    hovered_widgets: Vec<HoveredWidget<VM>>,
    hovered_scrollbar: Option<ScrollbarHandle>,
    active_scrollbar_drag: Option<ScrollbarDrag>,
    active_canvas_drag: Option<ActiveCanvasDrag<VM>>,
    pending_click: Option<PendingClick<VM>>,
    pressed_widget: Option<WidgetId>,
    focused_widget: Option<FocusedWidget<VM>>,
    focus_visible: bool,
    selected_text: Option<WidgetId>,
    text_edit_states: HashMap<WidgetId, TextEditState>,
    active_text_selection: Option<TextSelectionDrag>,
    clipboard: ClipboardService,
    cached_scene: Option<CachedScene<VM>>,
    cursor_icon: Option<CursorIcon>,
    scroll_states: HashMap<WidgetId, Point>,
    smooth_scroll_states: HashMap<WidgetId, SmoothScrollState>,
    select_open_states: HashMap<WidgetId, bool>,
    scroll_epoch: u64,
    media_event_states: HashMap<WidgetId, DispatchedMediaState>,
    media_manager: MediaManager,
    window_requests: WindowRequestQueue,
    window: Option<Arc<dyn Window>>,
    renderer: Option<Renderer>,
    window_id: Option<WindowId>,
    error: Option<TguiError>,
    dialog_dispatcher: AsyncDialogDispatcher<VM>,
    dialog_receiver: Option<AsyncDialogReceiver<VM>>,
    notification_dispatcher: AsyncNotificationDispatcher<VM>,
    notification_receiver: Option<AsyncNotificationReceiver<VM>>,
    #[cfg(all(target_os = "android", feature = "android"))]
    android_app: Option<AndroidApp>,
    #[cfg(all(target_os = "android", feature = "android"))]
    system_bar_style: Option<SystemBarStyle>,
}

struct CachedScene<VM> {
    viewport: Rect,
    units: UnitContext,
    focused_widget: Option<WidgetId>,
    focus_visible: bool,
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
    SelectOption {
        widget_id: WidgetId,
        option_index: usize,
    },
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
    fn mouse_event(self, position: Point, button: Option<CanvasMouseButton>) -> CanvasMouseEvent {
        CanvasMouseEvent {
            item_id: self.item_id,
            button,
            canvas_position: Point::new(
                position.x - self.canvas_origin.x,
                position.y - self.canvas_origin.y,
            ),
            scene_position: position,
            local_position: Point::new(
                position.x - self.item_origin.x,
                position.y - self.item_origin.y,
            ),
        }
    }

    fn pointer_event(self, position: Point) -> CanvasPointerEvent {
        self.mouse_event(position, None)
    }

    fn wheel_event(self, position: Point, delta: Point) -> CanvasWheelEvent {
        let mouse = self.mouse_event(position, None);
        CanvasWheelEvent {
            item_id: mouse.item_id,
            delta,
            canvas_position: mouse.canvas_position,
            scene_position: mouse.scene_position,
            local_position: mouse.local_position,
        }
    }

    fn drag_event(
        self,
        start_position: Point,
        position: Point,
        button: CanvasMouseButton,
    ) -> CanvasDragEvent {
        let start = self.mouse_event(start_position, Some(button));
        let current = self.mouse_event(position, Some(button));
        CanvasDragEvent {
            item_id: self.item_id,
            button,
            start_canvas_position: start.canvas_position,
            start_scene_position: start.scene_position,
            start_local_position: start.local_position,
            canvas_position: current.canvas_position,
            scene_position: current.scene_position,
            local_position: current.local_position,
            delta: Point::new(
                current.scene_position.x - start.scene_position.x,
                current.scene_position.y - start.scene_position.y,
            ),
        }
    }
}

#[derive(Clone)]
enum ClickHandler<VM> {
    Command(Command<VM>),
    Toggle(ValueCommand<VM, bool>, bool),
    SelectOption {
        widget_id: WidgetId,
        command: Option<Command<VM>>,
        on_open_change: Option<ValueCommand<VM, bool>>,
    },
    Canvas(
        ValueCommand<VM, CanvasMouseEvent>,
        CanvasPointerContext,
        Option<CanvasMouseButton>,
    ),
}

struct PendingClick<VM> {
    target_id: HoverTargetId,
    deadline: Instant,
    command: Option<ClickHandler<VM>>,
}

struct ActiveCanvasDrag<VM> {
    button: CanvasMouseButton,
    context: CanvasPointerContext,
    start_position: Point,
    started: bool,
    on_mouse_up: Option<ValueCommand<VM, CanvasMouseEvent>>,
    on_drag_start: Option<ValueCommand<VM, CanvasDragEvent>>,
    on_drag: Option<ValueCommand<VM, CanvasDragEvent>>,
    on_drag_end: Option<ValueCommand<VM, CanvasDragEvent>>,
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

#[derive(Clone, Copy)]
struct SmoothScrollState {
    target: Point,
}

const SMOOTH_SCROLL_EPSILON: f32 = 0.1;
const SMOOTH_SCROLL_LERP: f32 = 0.28;

#[derive(Clone)]
struct TextSelectionDrag {
    widget_id: WidgetId,
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
        notification_dispatcher: AsyncNotificationDispatcher<VM>,
        notification_receiver: Option<AsyncNotificationReceiver<VM>>,
        #[cfg(all(target_os = "android", feature = "android"))] android_app: Option<AndroidApp>,
    ) -> Self {
        let font_manager = FontManager::new(&config.fonts);
        let theme = match &config.theme {
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
            cursor_position: None,
            modifiers: ModifiersState::default(),
            hovered_widgets: Vec::new(),
            hovered_scrollbar: None,
            active_scrollbar_drag: None,
            active_canvas_drag: None,
            pending_click: None,
            pressed_widget: None,
            focused_widget: None,
            focus_visible: false,
            selected_text: None,
            text_edit_states: HashMap::new(),
            active_text_selection: None,
            clipboard: ClipboardService::default(),
            cached_scene: None,
            cursor_icon: None,
            scroll_states: HashMap::new(),
            smooth_scroll_states: HashMap::new(),
            select_open_states: HashMap::new(),
            scroll_epoch: 0,
            media_event_states: HashMap::new(),
            media_manager: MediaManager::new(invalidation.clone()),
            window_requests: WindowRequestQueue::default(),
            window: None,
            renderer: None,
            window_id: None,
            error: None,
            dialog_dispatcher,
            dialog_receiver,
            notification_dispatcher,
            notification_receiver,
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
        if let Some(window) = self.window.as_ref() {
            if window.is_decorated() != config.decorations {
                window.set_decorations(config.decorations);
            }
        }
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
            && cached.focused_widget == self.focused_widget_id()
            && cached.focus_visible == self.focus_visible
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
            && cached.focused_widget == self.focused_widget_id()
            && cached.focus_visible == self.focus_visible
            && cached.pressed_widget == self.pressed_widget
            && cached.selected_text == self.selected_text
            && cached.caret_visible == caret_visible
            && cached.animation_epoch == self.animation_epoch
            && cached.hover_epoch == self.hover_epoch
    }

    fn computed_scene(&mut self) -> &ComputedScene<VM> {
        let viewport = self.viewport_rect();
        let units = self.unit_context();
        let caret_visible = false;
        let active_scrollbar = self.active_scrollbar_drag.map(|drag| drag.handle);
        let selected_text_state = self
            .selected_text
            .and_then(|id| self.text_edit_state(id))
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
                                &self.select_open_states,
                                &self.scroll_states,
                                viewport,
                                None,
                                None,
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
                            &self.select_open_states,
                            &self.scroll_states,
                            viewport,
                            None,
                            None,
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
                focused_widget: self.focused_widget_id(),
                focus_visible: self.focus_visible,
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

    fn focused_widget_id(&self) -> Option<WidgetId> {
        self.focused_widget
            .as_ref()
            .map(|focused| focused.widget_id)
    }

    fn widget_state_map(&self, active_scrollbar: Option<ScrollbarHandle>) -> WidgetStateMap {
        let mut states = WidgetStateMap::default();
        for hovered in &self.hovered_widgets {
            match hovered.target_id {
                HoverTargetId::Widget(id) => {
                    let mut state = states.get(id);
                    state.hovered = true;
                    states.set(id, state);
                }
                HoverTargetId::SelectOption {
                    widget_id,
                    option_index,
                } => {
                    let mut state = states.get_select_option(widget_id, option_index);
                    state.hovered = true;
                    states.set_select_option(widget_id, option_index, state);
                }
                HoverTargetId::CanvasItem { .. } => {}
            }
        }
        if let Some(id) = self.pressed_widget {
            let mut state = states.get(id);
            state.pressed = true;
            states.set(id, state);
        }
        if self.focus_visible {
            if let Some(focused) = self.focused_widget.as_ref() {
                let mut state = states.get(focused.widget_id);
                state.focused = true;
                states.set(focused.widget_id, state);
            }
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

        let states = tree.media_event_states(&self.media_manager, &self.theme);
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
        let had_hovered_widgets = !self.hovered_widgets.is_empty();
        let previous_scrollbar = self.hovered_scrollbar;
        for hovered in std::mem::take(&mut self.hovered_widgets).into_iter().rev() {
            if let Some(command) = hovered.on_mouse_leave {
                self.execute_hover_transition_handler(&command, previous_position);
            }
        }
        self.hovered_scrollbar = self.active_scrollbar_drag.map(|drag| drag.handle);
        if had_hovered_widgets || self.hovered_scrollbar != previous_scrollbar {
            self.hover_epoch = self.hover_epoch.wrapping_add(1);
            self.invalidate_scene();
        }
        self.update_cursor_icon();
    }

    fn drive_animations(&mut self, event_loop: &dyn ActiveEventLoop, now: Instant) -> bool {
        self.flush_pending_click_if_due(now);

        let mut frame_advanced = false;
        if self.advance_smooth_scroll() {
            frame_advanced = true;
            if let Some(window) = self.window.as_ref() {
                window.request_redraw();
            }
        }
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
        let click_deadline = self.pending_click.as_ref().map(|pending| pending.deadline);
        let smooth_scroll_deadline =
            (!self.smooth_scroll_states.is_empty()).then_some(now + Duration::from_millis(16));
        [
            animation_deadline,
            controller_deadline,
            click_deadline,
            smooth_scroll_deadline,
        ]
        .into_iter()
        .flatten()
        .min()
    }

    fn text_edit_state(&self, id: WidgetId) -> Option<&TextEditState> {
        self.text_edit_states.get(&id)
    }

    fn update_text_edit_state(
        &mut self,
        widget_id: WidgetId,
        text: &str,
        update: impl FnOnce(&mut TextEditState),
    ) -> bool {
        let state = self
            .text_edit_states
            .entry(widget_id)
            .and_modify(|state| *state = state.clone().clamped_to(text))
            .or_insert_with(|| TextEditState::caret_at(text));
        let before = state.clone();
        update(state);
        *state = state.clone().clamped_to(text);
        if *state == before {
            return false;
        }
        self.invalidate_scene();
        true
    }

    fn window_id(&self) -> Option<WindowId> {
        self.window_id
    }

    fn create_or_resume_surface(
        &mut self,
        event_loop: &dyn ActiveEventLoop,
        modal_parent: Option<&Arc<dyn Window>>,
    ) {
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
            .with_decorations(self.config.decorations)
            .with_title(self.config.title.clone())
            .with_surface_size(self.config.size)
            .with_visible(false);

        if let Some(min_size) = self.config.min_size {
            attributes = attributes.with_min_surface_size(min_size);
        }

        if let Some(max_size) = self.config.max_size {
            attributes = attributes.with_max_surface_size(max_size);
        }

        if let Some(position) = default_window_position(event_loop, self.config.size) {
            attributes = attributes.with_position(position);
        }

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

        if self.blocks_main_window() {
            if let Some(parent) = modal_parent {
                attributes = configure_native_modal_window(attributes, parent.as_ref());
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
    notification_dispatcher: AsyncNotificationDispatcher<VM>,
    notification_receiver: AsyncNotificationReceiver<VM>,
    next_window_instance_id: u64,
    windows_by_key: HashMap<String, BoundRuntimeHandler<VM>>,
    window_keys_by_id: HashMap<WindowId, String>,
    closed_window_keys: HashSet<String>,
    last_window_sync_revision: u64,
    windows_need_sync: bool,
    shutting_down: bool,
    #[cfg(target_os = "windows")]
    main_window_disabled_for_modal: bool,
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
        let (notification_dispatcher, notification_receiver) = async_notification_channel();
        Self {
            config,
            view_model,
            windows,
            invalidation,
            animations,
            dialog_dispatcher,
            dialog_receiver,
            notification_dispatcher,
            notification_receiver,
            next_window_instance_id: 1,
            windows_by_key: HashMap::new(),
            window_keys_by_id: HashMap::new(),
            closed_window_keys: HashSet::new(),
            last_window_sync_revision: 0,
            windows_need_sync: true,
            shutting_down: false,
            #[cfg(target_os = "windows")]
            main_window_disabled_for_modal: false,
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

    fn main_window_key(&self) -> Option<&str> {
        self.windows_by_key.iter().find_map(|(key, window)| {
            if window.is_main_window() {
                Some(key.as_str())
            } else {
                None
            }
        })
    }

    fn main_window_ref(&self) -> Option<&Arc<dyn Window>> {
        let key = self.main_window_key()?;
        self.windows_by_key.get(key)?.window.as_ref()
    }

    #[cfg(target_os = "windows")]
    fn sync_native_modal_state(&mut self) {
        let should_disable_main = self.main_window_is_blocked();
        if self.main_window_disabled_for_modal == should_disable_main {
            return;
        }

        if let Some(window) = self.main_window_ref() {
            window.set_enable(!should_disable_main);
            self.main_window_disabled_for_modal = should_disable_main;
        }
    }

    #[cfg(not(target_os = "windows"))]
    fn sync_native_modal_state(&mut self) {}

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

        let mut resolved = match self.resolve_windows() {
            Ok(resolved) => resolved,
            Err(error) => {
                self.fail(event_loop, error);
                return;
            }
        };

        resolved.sort_by_key(|window| window_sync_priority(window.role));

        let desired_keys: HashSet<String> =
            resolved.iter().map(|window| window.key.clone()).collect();
        self.closed_window_keys
            .retain(|key| desired_keys.contains(key));

        for resolved_window in resolved {
            if self.closed_window_keys.contains(&resolved_window.key) {
                continue;
            }

            let key = resolved_window.key.clone();
            let modal_parent = if matches!(
                resolved_window.role,
                WindowRole::Child {
                    blocks_main_window: true
                }
            ) {
                self.main_window_ref().cloned()
            } else {
                None
            };
            if let Some(window) = self.windows_by_key.get_mut(&key) {
                window.set_definition(
                    resolved_window.role,
                    resolved_window.config,
                    resolved_window.window_bindings,
                    resolved_window.commands,
                    resolved_window.close_policy,
                );
                window.create_or_resume_surface(event_loop, modal_parent.as_ref());
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
                    self.notification_dispatcher.clone(),
                    None,
                    #[cfg(all(target_os = "android", feature = "android"))]
                    None,
                );
                window.close_policy = resolved_window.close_policy;
                window.create_or_resume_surface(event_loop, modal_parent.as_ref());
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

        self.sync_native_modal_state();

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
        self.drain_notification_completions();
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
            self.sync_native_modal_state();
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
        self.drain_notification_completions();
        self.sync_windows(event_loop, false);
        if self.error.is_some() {
            return;
        }

        let keys: Vec<String> = self.windows_by_key.keys().cloned().collect();
        for key in keys {
            let (close_requested, is_main_window) =
                if let Some(window) = self.windows_by_key.get_mut(&key) {
                    let close_requested = window.handle_bound_about_to_wait(event_loop);
                    let is_main_window = window.is_main_window();
                    if let Some(error) = window.error.take() {
                        self.fail(event_loop, error);
                        return;
                    }
                    self.window_keys_by_id
                        .retain(|_, existing_key| existing_key != &key);
                    if let Some(window_id) = window.window_id() {
                        self.window_keys_by_id.insert(window_id, key.clone());
                    }
                    (close_requested, is_main_window)
                } else {
                    (false, false)
                };

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
                self.sync_native_modal_state();
                if self.windows_by_key.is_empty() {
                    event_loop.exit();
                }
                return;
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
        self.create_or_resume_surface(event_loop, None);
    }

    fn proxy_wake_up(&mut self, event_loop: &dyn ActiveEventLoop) {
        self.drain_dialog_completions();
        self.drain_notification_completions();
        if self.drain_window_requests() {
            event_loop.exit();
        }
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
        self.drain_notification_completions();
        if self.handle_bound_about_to_wait(event_loop) {
            event_loop.exit();
        }
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

fn mouse_scroll_delta(delta: MouseScrollDelta) -> Point {
    const LINE_SCROLL_STEP: f32 = 40.0;

    match delta {
        MouseScrollDelta::LineDelta(x, y) => Point::new(x * LINE_SCROLL_STEP, y * LINE_SCROLL_STEP),
        MouseScrollDelta::PixelDelta(position) => Point::new(position.x as f32, position.y as f32),
    }
}

fn canvas_mouse_button(button: Option<MouseButton>) -> Option<CanvasMouseButton> {
    match button? {
        MouseButton::Left => Some(CanvasMouseButton::Left),
        MouseButton::Right => Some(CanvasMouseButton::Right),
        MouseButton::Middle => Some(CanvasMouseButton::Middle),
        MouseButton::Back => Some(CanvasMouseButton::Back),
        MouseButton::Forward => Some(CanvasMouseButton::Forward),
        other => Some(CanvasMouseButton::Other(other as u16)),
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

impl From<crate::foundation::window_control::WindowResizeDirection> for ResizeDirection {
    fn from(direction: crate::foundation::window_control::WindowResizeDirection) -> Self {
        match direction {
            crate::foundation::window_control::WindowResizeDirection::East => Self::East,
            crate::foundation::window_control::WindowResizeDirection::North => Self::North,
            crate::foundation::window_control::WindowResizeDirection::NorthEast => Self::NorthEast,
            crate::foundation::window_control::WindowResizeDirection::NorthWest => Self::NorthWest,
            crate::foundation::window_control::WindowResizeDirection::South => Self::South,
            crate::foundation::window_control::WindowResizeDirection::SouthEast => Self::SouthEast,
            crate::foundation::window_control::WindowResizeDirection::SouthWest => Self::SouthWest,
            crate::foundation::window_control::WindowResizeDirection::West => Self::West,
        }
    }
}

fn input_text_layout(
    font_manager: &FontManager,
    theme: &Theme,
    units: UnitContext,
    text_style: &Text,
    current_text: &str,
    multiline: bool,
    wrap_width: f32,
) -> (crate::text::font::TextLayoutInfo, f32, f32) {
    let default_style = &theme.typography.body;
    let font_size = units.resolve_sp(
        text_style
            .font_size
            .unwrap_or(default_style.size.max(sp(1.0))),
    );
    let line_height = (font_size * 1.25).max(font_size + 4.0);
    let letter_spacing = units.resolve_sp(
        text_style
            .letter_spacing
            .unwrap_or(default_style.letter_spacing.unwrap_or(Sp::ZERO)),
    );
    let text_request = TextFontRequest {
        preferred_font: text_style
            .font_family
            .as_deref()
            .or(default_style.font_family.as_deref()),
        weight: text_style.font_weight.unwrap_or(default_style.weight),
    };
    let layout = if multiline {
        font_manager.measure_text_layout_wrapped(
            current_text,
            text_request,
            font_size,
            line_height,
            letter_spacing,
            wrap_width,
        )
    } else {
        font_manager.measure_text_layout(
            current_text,
            text_request,
            font_size,
            line_height,
            letter_spacing,
        )
    };
    (layout, font_size, line_height)
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

    let inner = frame.inset(padding);
    let (layout, _font_size, line_height) = input_text_layout(
        font_manager,
        theme,
        units,
        text_style,
        current_text,
        false,
        inner.width.get(),
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

#[cfg(test)]
mod tests;
