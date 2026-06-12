mod accessibility;
mod action_stats;
mod application_handler;
mod binding_sync;
mod bootstrap;
mod cache_support;
mod carousel;
mod commands;
mod handler_meta;
mod handler_support;
mod helpers;
mod ime_runtime;
mod input;
mod lifecycle;
mod menu;
pub(crate) mod overlay;
mod popover;
pub(crate) mod portal;
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
mod tooltip;
mod windows;

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
    ActiveCanvasDrag, ActiveDataGridColumnReorder, ActiveDataGridColumnResize,
    ActiveGestureSession, ActiveKeyRepeat, ActivePinchSession, ActiveSplitterResize,
    ActiveTabReorder, ActiveTreeDrag, CachedScene, CanvasPointerContext, ClickHandler,
    ClipboardService, DeferredMouseClick, DispatchedLifecycleState, DispatchedMediaState,
    FocusedWidget, HoverMoveHandler, HoverTargetId, HoverTransitionHandler, HoveredWidget,
    PendingClick, PendingLifecycleEvent, PendingMediaEvent, PendingSplitterClick, ScrollbarDrag,
    SliderDrag, SmoothScrollState, TextInputBufferState, TextInputSessionConfig, TextSelectionDrag,
    TooltipState, TouchScrollDrag, TouchScrollInertiaState,
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
use crate::foundation::task::{async_task_channel, AsyncTaskDispatcher, AsyncTaskReceiver};
use crate::foundation::view_model::{Command, ViewModel};
use crate::foundation::window_control::WindowRequestQueue;
use crate::log::{log_startup_phase, log_text_profile, text_profile_enabled, Log};
use crate::media::MediaManager;
use crate::notification::{
    async_notification_channel, AsyncNotificationDispatcher, AsyncNotificationReceiver,
};
use crate::platform::accessibility::PlatformAccessibilityAdapter;
use crate::platform::backend::application::ApplicationHandler;
use crate::platform::backend::event_loop::{ActiveEventLoop, ControlFlow};
use crate::platform::backend::window::Window;
use crate::platform::backend::EventLoop;
use crate::platform::cursor::CursorIcon;
use crate::platform::dpi::PhysicalPosition;
use crate::platform::event::{ElementState, WindowEvent};
use crate::platform::keyboard::ModifiersState;
use crate::platform::window::{
    ImeCapabilities, ImeEnableRequest, ImeHint, ImePurpose, ImeRequest, ImeSurroundingText,
    Theme as WindowTheme, WindowAttributes, WindowId,
};
use crate::rendering::renderer::{RenderStatus, Renderer};
use crate::runtime::portal::ExternalPortalRequest;
#[cfg(feature = "audio")]
use crate::runtime::state::AudioLifecycleState;
use crate::text::font::FontManager;
use crate::ui::theme::{Theme, ThemeMode, ThemeSet, ThemeStore};
use crate::ui::unit::{dp, UnitContext};
#[cfg(feature = "audio")]
use crate::ui::widget::LifecycleEventState;
use crate::ui::widget::{
    CollectedSceneCache, ComputedScene, Point, Rect, ResolvedSceneLayout, ScrollRegion,
    ScrollbarHandle, TextEditState, VirtualCacheState, WidgetId, WidgetStateMap, WidgetTree,
};
use crossbeam_channel::{Receiver, Sender};
use image::GenericImageView;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use winit_core::icon::{Icon, RgbaIcon};

pub(super) const DOUBLE_CLICK_THRESHOLD: Duration = Duration::from_millis(300);
const CARET_BLINK_INTERVAL: Duration = Duration::from_millis(500);
const KEY_REPEAT_INITIAL_DELAY: Duration = Duration::from_millis(300);
const KEY_REPEAT_INTERVAL: Duration = Duration::from_millis(33);
pub(super) const TOUCH_SCROLL_ACTIVATION_THRESHOLD: f32 = 8.0;
pub(super) const TOUCH_SCROLL_INERTIA_MIN_VELOCITY: f32 = 30.0;
pub(super) const TOUCH_SCROLL_INERTIA_MAX_VELOCITY: f32 = 3600.0;
pub(super) const TOUCH_SCROLL_INERTIA_DECAY_PER_SECOND: f32 = 8.0;
pub(super) const TOUCH_SCROLL_INERTIA_FRAME: Duration = Duration::from_millis(16);
pub(super) const LONG_PRESS_THRESHOLD: Duration = Duration::from_millis(500);
pub(super) const TOOLTIP_LONG_PRESS_HIDE_DELAY: Duration = Duration::from_millis(150);
pub(super) const LONG_PRESS_MOVE_TOLERANCE: f32 = 8.0;
pub(super) const SWIPE_ACTIVATION_THRESHOLD: f32 = 12.0;
pub(super) const SWIPE_AXIS_LOCK_THRESHOLD: f32 = 8.0;
pub(super) const EDGE_SWIPE_BAND: f32 = 24.0;
pub(super) const PINCH_ACTIVATION_THRESHOLD: f32 = 12.0;
pub struct BoundRuntime<VM> {
    event_loop: EventLoop,
    config: ApplicationConfig,
    view_model: Arc<Mutex<VM>>,
    windows: Option<WindowSetFactory<VM>>,
    single_window: Option<SingleWindowSetup<VM>>,
    invalidation: InvalidationSignal,
    animations: AnimationCoordinator,
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
        let (task_dispatcher, task_receiver) = async_task_channel();
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
            task_dispatcher,
            Some(task_receiver),
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
    pub(crate) reduced_motion: Option<Signal<bool>>,
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
    font_manager: Arc<FontManager>,
    theme: Theme,
    theme_store: ThemeStore,
    reduced_motion: bool,
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
    /// Widget 进入 hover 的时间戳（按 `WidgetId` 索引）。
    /// `handle_hover` 维护：路径中新出现的 widget 写入 `Instant::now()`；离开 hover 链时删除。
    /// collect 阶段读取它来判断 Tooltip 是否已等够 `delay`。
    tooltip_hover_started_at: HashMap<WidgetId, Instant>,
    /// 最近一次 collect 上报的 tooltip 唤醒时刻（hover_start + delay）。
    /// 由 `next_deadline` 汇入 winit ControlFlow，到点后 `drive_animations` 触发 invalidate。
    next_tooltip_wakeup_deadline: Option<Instant>,
    /// 最近一次 collect 上报的 toast 唤醒时刻（最早自动消失 deadline）。
    next_toast_wakeup_deadline: Option<Instant>,
    /// Carousel autoplay 的下一次唤醒时刻。
    next_carousel_wakeup_deadline: Option<Instant>,
    tooltip_state: TooltipState,
    /// 最近一次成功解析出的 hover 预览浮层锚点（Popover 触发 widget 的 id）。
    /// `resolve_active_hover_popover` 正常依赖上一帧 `cached_scene` 的 overlay rect 判断光标是否仍在
    /// 触发器或浮层内容内；但命令执行（点击浮层内交互元素）会 `invalidate_scene_with_reason` 把
    /// `cached_scene` 硬清空，导致本帧无法推断，浮层会误关闭。点击不移动光标，故这里缓存上一次结果，
    /// 在 cache 缺失的重建帧里复用，重建后新 cache 会恢复正常解析。光标离开窗口时清空。
    hover_popover_anchor: Option<WidgetId>,
    /// 未受控 Menu 的内部开闭状态。受控 Menu 仍以 descriptor.open 为准。
    menu_open_states: HashMap<WidgetId, bool>,
    /// 未受控 MenuBar 的内部 active entry。key 为 MenuBarGroupId 的 raw 值。
    menubar_active_states: HashMap<u64, Option<usize>>,
    /// ContextMenu 由右键/长按自动打开时记录触发锚点。
    context_menu_anchor_states: HashMap<WidgetId, Point>,
    /// 每个打开的 Menu overlay 的键盘 cursor 路径（每层一个 option_index）。
    /// 长度=1 表示 cursor 在最外层；>1 表示已进入嵌套 submenu。
    /// Up/Down 调整最末元素；Right 把当前 cursor（必须是 Submenu 项）的首项 push；
    /// Left 弹栈直到深度=1，再交给 MenuBar Left/Right。
    menu_keyboard_cursor: HashMap<WidgetId, Vec<usize>>,
    /// List / VirtualList 的范围选择锚点。真实 selected keys 仍由 ViewModel 受控持有；
    /// runtime 只保存 Shift+Click / Shift+Arrow 的临时 anchor。
    list_anchor_states: HashMap<WidgetId, crate::ui::widget::WidgetKey>,
    /// 最近一次聚焦的 List row key。受控 selection 触发重建后 row WidgetId 可能更新，
    /// 键盘导航用它把焦点恢复到当前虚拟窗口中的同一行。
    list_focus_state: Option<(WidgetId, crate::ui::widget::WidgetKey)>,
    tree_anchor_states: HashMap<WidgetId, crate::ui::widget::WidgetKey>,
    tree_focus_state: Option<(WidgetId, crate::ui::widget::WidgetKey)>,
    data_grid_anchor_states: HashMap<WidgetId, crate::ui::widget::WidgetKey>,
    data_grid_focus_state: Option<(
        WidgetId,
        crate::ui::widget::WidgetKey,
        crate::ui::widget::WidgetKey,
    )>,
    hovered_scrollbar: Option<ScrollbarHandle>,
    active_scrollbar_drag: Option<ScrollbarDrag>,
    active_touch_scroll: Option<TouchScrollDrag>,
    active_gesture: Option<ActiveGestureSession<VM>>,
    active_pinch: Option<ActivePinchSession<VM>>,
    active_slider_drag: Option<SliderDrag<VM>>,
    active_canvas_drag: Option<ActiveCanvasDrag<VM>>,
    active_tab_reorder: Option<ActiveTabReorder<VM>>,
    active_tree_drag: Option<ActiveTreeDrag<VM>>,
    active_data_grid_column_resize: Option<ActiveDataGridColumnResize<VM>>,
    active_splitter_resize: Option<ActiveSplitterResize<VM>>,
    active_data_grid_column_reorder: Option<ActiveDataGridColumnReorder<VM>>,
    carousel_auto_play_last: HashMap<WidgetId, Instant>,
    active_key_repeat: Option<ActiveKeyRepeat>,
    pending_click: Option<PendingClick<VM>>,
    deferred_mouse_click: Option<DeferredMouseClick<VM>>,
    pressed_widget: Option<WidgetId>,
    focused_widget: Option<FocusedWidget<VM>>,
    focus_visible: bool,
    active_auto_focus_scope: Option<Vec<WidgetId>>,
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
    touch_scroll_inertia_states: HashMap<WidgetId, TouchScrollInertiaState>,
    virtual_states: HashMap<WidgetId, VirtualCacheState>,
    select_open_states: HashMap<WidgetId, bool>,
    external_portal_requests: Vec<ExternalPortalRequest<VM>>,
    external_portal_revision: u64,
    scroll_epoch: u64,
    text_input_epoch: u64,
    media_event_states: HashMap<WidgetId, DispatchedMediaState>,
    lifecycle_event_states: HashMap<WidgetId, DispatchedLifecycleState<VM>>,
    media_manager: MediaManager,
    startup_started_at: Instant,
    first_frame_logged: bool,
    window_requests: WindowRequestQueue,
    window: Option<Arc<dyn Window>>,
    accessibility_adapter: Option<PlatformAccessibilityAdapter>,
    accessibility_action_sender: Sender<accesskit::ActionRequest>,
    accessibility_action_receiver: Receiver<accesskit::ActionRequest>,
    renderer: Option<Renderer>,
    last_synced_clear_color: Option<Color>,
    window_id: Option<WindowId>,
    error: Option<TguiError>,
    dialog_dispatcher: AsyncDialogDispatcher<VM>,
    dialog_receiver: Option<AsyncDialogReceiver<VM>>,
    notification_dispatcher: AsyncNotificationDispatcher<VM>,
    notification_receiver: Option<AsyncNotificationReceiver<VM>>,
    task_dispatcher: AsyncTaskDispatcher<VM>,
    task_receiver: Option<AsyncTaskReceiver<VM>>,
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
        task_dispatcher: AsyncTaskDispatcher<VM>,
        task_receiver: Option<AsyncTaskReceiver<VM>>,
    ) -> Self {
        let font_manager = Arc::new(FontManager::new(&config.fonts));
        let theme = match &config.theme {
            ThemeSelection::Mode(mode) => config.theme_set.resolve(*mode, None).as_ref().clone(),
            ThemeSelection::System => config.theme_set.resolve_window_theme(None).as_ref().clone(),
        };
        let theme_store = ThemeStore::new(config.theme_set.clone(), ThemeMode::System, None);
        let resource_budget = config.resource_budget;
        let reduced_motion = config.reduced_motion;
        let (accessibility_action_sender, accessibility_action_receiver) =
            crossbeam_channel::unbounded();

        Self {
            window_key,
            window_instance_id,
            role,
            config,
            font_manager,
            theme,
            theme_store,
            reduced_motion,
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
            tooltip_hover_started_at: HashMap::new(),
            next_tooltip_wakeup_deadline: None,
            next_toast_wakeup_deadline: None,
            next_carousel_wakeup_deadline: None,
            tooltip_state: TooltipState {
                active: None,
                hover_suppressed: None,
                focus_suppressed: None,
                long_press_suppressed: None,
                long_press_candidate: None,
                long_press_release_deadline: None,
            },
            hover_popover_anchor: None,
            menu_open_states: HashMap::new(),
            menubar_active_states: HashMap::new(),
            context_menu_anchor_states: HashMap::new(),
            menu_keyboard_cursor: HashMap::new(),
            list_anchor_states: HashMap::new(),
            list_focus_state: None,
            tree_anchor_states: HashMap::new(),
            tree_focus_state: None,
            data_grid_anchor_states: HashMap::new(),
            data_grid_focus_state: None,
            hovered_scrollbar: None,
            active_scrollbar_drag: None,
            active_touch_scroll: None,
            active_gesture: None,
            active_pinch: None,
            active_slider_drag: None,
            active_canvas_drag: None,
            active_tab_reorder: None,
            active_tree_drag: None,
            active_data_grid_column_resize: None,
            active_splitter_resize: None,
            active_data_grid_column_reorder: None,
            carousel_auto_play_last: HashMap::new(),
            active_key_repeat: None,
            pending_click: None,
            deferred_mouse_click: None,
            pressed_widget: None,
            focused_widget: None,
            focus_visible: false,
            active_auto_focus_scope: None,
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
            touch_scroll_inertia_states: HashMap::new(),
            virtual_states: HashMap::new(),
            select_open_states: HashMap::new(),
            external_portal_requests: Vec::new(),
            external_portal_revision: 0,
            scroll_epoch: 0,
            text_input_epoch: 0,
            media_event_states: HashMap::new(),
            lifecycle_event_states: HashMap::new(),
            media_manager: MediaManager::with_budget(invalidation.clone(), resource_budget),
            startup_started_at: Instant::now(),
            first_frame_logged: false,
            window_requests: WindowRequestQueue::default(),
            window: None,
            accessibility_adapter: None,
            accessibility_action_sender,
            accessibility_action_receiver,
            renderer: None,
            last_synced_clear_color: None,
            window_id: None,
            error: None,
            dialog_dispatcher,
            dialog_receiver,
            notification_dispatcher,
            notification_receiver,
            task_dispatcher,
            task_receiver,
        }
    }
}

#[cfg(test)]
mod tests;
