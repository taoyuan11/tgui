#[cfg(all(target_os = "android", feature = "android"))]
mod android_text_input;
mod application_handler;
mod binding_sync;
mod bootstrap;
mod cache_support;
mod commands;
mod handler_meta;
mod handler_support;
mod helpers;
mod ime_runtime;
mod input;
mod lifecycle;
mod render_cycle;
mod scene_patch;
mod scene_patch_cleanup;
mod scene_patch_dependencies;
mod scene_patch_invalidation;
mod scene_patch_roots;
mod scene_runtime;
mod state;
mod theme;
mod timing;
mod windows;

#[cfg(all(target_os = "android", feature = "android"))]
use self::bootstrap::build_event_loop_with_android_app;
#[cfg(all(target_env = "ohos", feature = "ohos"))]
use self::bootstrap::build_event_loop_with_ohos_app;
#[cfg(test)]
pub(super) use self::bootstrap::centered_window_position_for_monitor;
pub(super) use self::bootstrap::window_sync_priority;
use self::bootstrap::{
    build_event_loop, configure_native_modal_window, default_window_position,
    dirty_dependency_set_label, prepare_notifications_for_runtime,
};
use self::helpers::{
    canvas_mouse_button, cursor_icon, input_text_layout, is_primary_shortcut_modifier,
    mouse_scroll_delta, resolved_input_text_metrics, text_cursor_index_at_point,
    text_cursor_index_from_layout_at_point,
};
#[cfg(feature = "audio")]
use self::state::audio_lifecycle_state;
use self::state::{collect_pending_lifecycle_events, collect_pending_media_event};
use self::state::{
    ActiveCanvasDrag, ActiveKeyRepeat, CachedScene, CanvasPointerContext, ClickHandler,
    ClipboardService, DispatchedLifecycleState, DispatchedMediaState, FocusedWidget,
    HoverMoveHandler, HoverTargetId, HoverTransitionHandler, HoveredWidget, PendingClick,
    PendingLifecycleEvent, PendingMediaEvent, ScrollbarDrag, SliderDrag, SmoothScrollState,
    TextInputBufferState, TextInputSessionConfig, TextSelectionDrag, TouchScrollDrag,
};
#[cfg(all(target_os = "android", feature = "android"))]
use self::theme::{
    android_font_scale, apply_android_system_bar_style, is_light_color, SystemBarStyle,
};
use self::theme::{resolve_theme, resolve_window_theme};
use self::windows::MultiWindowHandler;
use crate::animation::{
    default_theme_transition, AnimationCoordinator, AnimationEngine, AnimationKey, Transition,
    WindowProperty,
};
use crate::application::{
    ApplicationConfig, ThemeSelection, WindowClosePolicy, WindowRole, WindowSetFactory,
};
use crate::dialog::{async_dialog_channel, AsyncDialogDispatcher, AsyncDialogReceiver};
use crate::foundation::binding::{
    DependencyGraph, DependencyPhase, DirtyDependencySet, InvalidationSignal, Signal,
};
use crate::foundation::color::Color;
use crate::foundation::error::TguiError;
use crate::foundation::event::InputTrigger;
use crate::foundation::view_model::{Command, ViewModel};
use crate::foundation::window_control::WindowRequestQueue;
use crate::log::{log_startup_phase, log_text_profile, text_profile_enabled, Log};
use crate::media::MediaManager;
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
use crate::platform::dpi::PhysicalPosition;
use crate::platform::event::{ElementState, MouseButton, WindowEvent};
use crate::platform::keyboard::ModifiersState;
#[cfg(all(target_env = "ohos", feature = "ohos"))]
use crate::platform::ohos::{OhosApp, WindowExtOhos};
use crate::platform::window::{
    ImeCapabilities, ImeEnableRequest, ImeHint, ImePurpose, ImeRequest, ImeSurroundingText,
    Theme as WindowTheme, WindowAttributes, WindowId,
};
use crate::rendering::renderer::{RenderStatus, Renderer};
#[cfg(feature = "audio")]
use crate::runtime::state::AudioLifecycleState;
use crate::text::font::FontManager;
use crate::ui::theme::{Theme, ThemeMode, ThemeSet, ThemeStore};
use crate::ui::unit::{dp, UnitContext};
#[cfg(feature = "audio")]
use crate::ui::widget::LifecycleEventState;
use crate::ui::widget::{
    CollectedSceneCache, ComputedScene, Point, Rect, ResolvedSceneLayout, ScrollRegion,
    ScrollbarHandle, TextEditState, WidgetId, WidgetStateMap, WidgetTree,
};
use image::GenericImageView;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use winit_core::icon::{Icon, RgbaIcon};

const DOUBLE_CLICK_THRESHOLD: Duration = Duration::from_millis(300);
const CARET_BLINK_INTERVAL: Duration = Duration::from_millis(500);
const KEY_REPEAT_INITIAL_DELAY: Duration = Duration::from_millis(300);
const KEY_REPEAT_INTERVAL: Duration = Duration::from_millis(33);
const TOUCH_SCROLL_ACTIVATION_THRESHOLD: f32 = 8.0;
#[cfg(all(target_os = "android", feature = "android"))]
const ANDROID_SYSTEM_THEME_POLL_INTERVAL: Duration = Duration::from_millis(500);

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

#[derive(Clone, Default)]
pub struct WindowBindings {
    pub(crate) title: Option<Signal<String>>,
    pub(crate) clear_color: Option<Signal<Color>>,
    pub(crate) theme_set: Option<Signal<ThemeSet>>,
    pub(crate) theme_mode: Option<Signal<ThemeMode>>,
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
    last_lifecycle_dispatch_revision: u64,
    animations: AnimationCoordinator,
    animation_engine: AnimationEngine,
    animation_epoch: u64,
    layout_animation_epoch: u64,
    hover_epoch: u64,
    cursor_position: Option<Point>,
    modifiers: ModifiersState,
    hovered_widgets: Vec<HoveredWidget<VM>>,
    hovered_scrollbar: Option<ScrollbarHandle>,
    active_scrollbar_drag: Option<ScrollbarDrag>,
    active_touch_scroll: Option<TouchScrollDrag>,
    active_slider_drag: Option<SliderDrag<VM>>,
    active_canvas_drag: Option<ActiveCanvasDrag<VM>>,
    active_key_repeat: Option<ActiveKeyRepeat>,
    pending_click: Option<PendingClick<VM>>,
    pressed_widget: Option<WidgetId>,
    focused_widget: Option<FocusedWidget<VM>>,
    focus_visible: bool,
    selected_text: Option<WidgetId>,
    text_edit_states: HashMap<WidgetId, TextEditState>,
    text_input_buffers: HashMap<WidgetId, TextInputBufferState>,
    text_input_regions: HashMap<WidgetId, input::TextInputRegionData<VM>>,
    text_input_flush_data: HashMap<WidgetId, input::TextInputFlushData<VM>>,
    active_text_selection: Option<TextSelectionDrag>,
    caret_blink_origin: Instant,
    clipboard: ClipboardService,
    cached_scene: Option<CachedScene<VM>>,
    cursor_icon: Option<CursorIcon>,
    scroll_states: HashMap<WidgetId, Point>,
    smooth_scroll_states: HashMap<WidgetId, SmoothScrollState>,
    select_open_states: HashMap<WidgetId, bool>,
    scroll_epoch: u64,
    text_input_epoch: u64,
    media_event_states: HashMap<WidgetId, DispatchedMediaState>,
    lifecycle_event_states: HashMap<WidgetId, DispatchedLifecycleState<VM>>,
    media_manager: MediaManager,
    startup_started_at: Instant,
    first_frame_logged: bool,
    window_requests: WindowRequestQueue,
    window: Option<Arc<dyn Window>>,
    renderer: Option<Renderer>,
    last_synced_clear_color: Option<Color>,
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
        let resource_budget = config.resource_budget;

        #[cfg(all(target_os = "android", feature = "android"))]
        if let Some(app) = android_app.as_ref() {
            // 装载 dialog JNI 桥接（幂等）；失败时 Android dialog 调度返回 Backend 错误。
            let _ = crate::dialog::install_android_app(app);
            let _ = crate::notification::install_android_app(app);
            let _ = self::android_text_input::install_android_text_input_bridge(app, &invalidation);
        }

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
            last_lifecycle_dispatch_revision: 0,
            animations,
            animation_engine: AnimationEngine::default(),
            animation_epoch: 0,
            layout_animation_epoch: 0,
            hover_epoch: 0,
            cursor_position: None,
            modifiers: ModifiersState::default(),
            hovered_widgets: Vec::new(),
            hovered_scrollbar: None,
            active_scrollbar_drag: None,
            active_touch_scroll: None,
            active_slider_drag: None,
            active_canvas_drag: None,
            active_key_repeat: None,
            pending_click: None,
            pressed_widget: None,
            focused_widget: None,
            focus_visible: false,
            selected_text: None,
            text_edit_states: HashMap::new(),
            text_input_buffers: HashMap::new(),
            text_input_regions: HashMap::new(),
            text_input_flush_data: HashMap::new(),
            active_text_selection: None,
            caret_blink_origin: Instant::now(),
            clipboard: ClipboardService::default(),
            cached_scene: None,
            cursor_icon: None,
            scroll_states: HashMap::new(),
            smooth_scroll_states: HashMap::new(),
            select_open_states: HashMap::new(),
            scroll_epoch: 0,
            text_input_epoch: 0,
            media_event_states: HashMap::new(),
            lifecycle_event_states: HashMap::new(),
            media_manager: MediaManager::with_budget(invalidation.clone(), resource_budget),
            startup_started_at: Instant::now(),
            first_frame_logged: false,
            window_requests: WindowRequestQueue::default(),
            window: None,
            renderer: None,
            last_synced_clear_color: None,
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
}

#[cfg(test)]
mod tests;
