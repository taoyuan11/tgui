use super::{
    centered_window_position_for_monitor, input, input_text_layout, text_cursor_index_at_point,
    BoundRuntimeHandler, CachedScene, FocusedWidget, WindowBindings,
};
use crate::animation::AnimationCoordinator;
use crate::application::{ApplicationConfig, MsaaMode, ResourceBudget, ThemeSelection, WindowRole};
use crate::dialog::async_dialog_channel;
use crate::foundation::binding::ScrollViewController;
use crate::foundation::binding::{DependencyGraph, ViewModelContext};
use crate::foundation::binding::{InvalidationSignal, Signal, TextController};
use crate::foundation::color::Color;
use crate::foundation::view_model::{Command, ValueCommand, ViewModel};
use crate::platform::backend::event_loop::{
    ActiveEventLoop, ControlFlow, DeviceEvents, EventLoopProxy, OwnedDisplayHandle,
};
use crate::platform::cursor::{CustomCursor, CustomCursorSource};
use crate::platform::dpi::{LogicalSize, PhysicalPosition, PhysicalSize};
use crate::platform::error::RequestError;
use crate::platform::event::{
    ButtonSource, ElementState, FingerId, Ime, KeyEvent, MouseScrollDelta, PointerKind,
    PointerSource, TouchPhase, WindowEvent,
};
use crate::platform::keyboard::{Key, KeyCode, KeyLocation, ModifiersState, NamedKey, PhysicalKey};
use crate::platform::window::{ImeCapabilities, ImeEnableRequest, ImeHint, ImePurpose};
use crate::platform::window::{Theme as WindowTheme, Window, WindowAttributes};
use crate::text::font::FontCatalog;
use crate::ui::layout::{Axis, Insets, Overflow};
use crate::ui::theme::{Theme, ThemeMode, ThemeSet};
use crate::ui::unit::{dp, sp, Dp, UnitContext};
use crate::ui::widget::{
    Button, Canvas, CanvasMouseButton, CanvasParagraphStyle, CanvasPointerEvent, CanvasRecorder,
    CanvasShadow, CanvasStroke, CanvasTextStyle, CanvasTextVerticalAlign, CanvasTextWrap, Checkbox,
    ContainerStyle, CursorStyle, Flex, FocusScopeOptions, HitInteraction, Input, Point, Rect,
    ScrollView, Select, SelectOption, Switch, Text, TextEditState, Textarea, Tooltip,
    VirtualCacheState, WidgetTree,
};
use crate::ui::widget::{Element, Stack, WidgetId};
use raw_window_handle::{DisplayHandle, HandleError, HasDisplayHandle};
#[cfg(feature = "audio")]
use std::sync::atomic::AtomicBool;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use std::time::Instant;
use winit_core::monitor::MonitorHandle;

#[cfg(feature = "audio")]
use crate::audio::backend::{
    AudioBackend, BackendSharedState as AudioBackendSharedState,
    DEFAULT_AUDIO_BUFFER_MEMORY_LIMIT_BYTES,
};
#[cfg(feature = "audio")]
use crate::audio::{Audio, AudioController, AudioMetrics, AudioPlaybackState, AudioSource};
#[cfg(feature = "video")]
use crate::media::TextureFrame;
use crate::notification::async_notification_channel;
#[cfg(feature = "video")]
use crate::video::backend::{
    BackendSharedState, VideoBackend, DEFAULT_VIDEO_BUFFER_MEMORY_LIMIT_BYTES,
};
#[cfg(feature = "video")]
use crate::video::{
    VideoController, VideoMetrics, VideoPlaybackState, VideoSize, VideoSource, VideoSurface,
    VideoSurfaceSnapshot,
};
#[derive(Default)]
struct TestVm;

impl crate::foundation::view_model::ViewModel for TestVm {
    fn new(_context: &ViewModelContext) -> Self {
        Self
    }

    fn view(&self) -> Element<Self>
    where
        Self: Sized,
    {
        Stack::new().into()
    }
}

fn test_config() -> ApplicationConfig {
    ApplicationConfig {
        app_id: None,
        title: "test".to_string(),
        size: LogicalSize::new(200.0, 120.0),
        min_size: None,
        max_size: None,
        clear_color: Color::BLACK,
        clear_color_overridden: true,
        close_children_with_main: true,
        decorations: true,
        viewport_insets: Insets::ZERO,
        msaa: MsaaMode::Auto,
        fonts: FontCatalog::default(),
        theme: ThemeSelection::System,
        theme_set: ThemeSet::default(),
        window_icon: None,
        resource_budget: ResourceBudget::DEFAULT,
    }
}

fn test_config_with_theme(theme: ThemeSelection, theme_set: ThemeSet) -> ApplicationConfig {
    ApplicationConfig {
        app_id: None,
        title: "test".to_string(),
        size: LogicalSize::new(200.0, 120.0),
        min_size: None,
        max_size: None,
        clear_color: Color::BLACK,
        clear_color_overridden: true,
        close_children_with_main: true,
        decorations: true,
        viewport_insets: Insets::ZERO,
        msaa: MsaaMode::Auto,
        fonts: FontCatalog::default(),
        theme,
        theme_set,
        window_icon: None,
        resource_budget: ResourceBudget::DEFAULT,
    }
}

fn test_config_with_size(width: f64, height: f64) -> ApplicationConfig {
    ApplicationConfig {
        size: LogicalSize::new(width, height),
        ..test_config()
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
    let (notification_dispatcher, notification_receiver) = async_notification_channel();
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
        notification_dispatcher,
        Some(notification_receiver),
        #[cfg(all(target_os = "android", feature = "android"))]
        None,
    )
}

fn pressed_key_event(physical_key: PhysicalKey) -> KeyEvent {
    KeyEvent {
        physical_key,
        logical_key: match physical_key {
            PhysicalKey::Code(KeyCode::Tab) => Key::Named(NamedKey::Tab),
            PhysicalKey::Code(KeyCode::PageUp) => Key::Named(NamedKey::PageUp),
            PhysicalKey::Code(KeyCode::PageDown) => Key::Named(NamedKey::PageDown),
            PhysicalKey::Code(KeyCode::Home) => Key::Named(NamedKey::Home),
            PhysicalKey::Code(KeyCode::End) => Key::Named(NamedKey::End),
            _ => Key::Character(" ".into()),
        },
        text: None,
        location: KeyLocation::Standard,
        state: ElementState::Pressed,
        repeat: false,
        text_with_all_modifiers: None,
        key_without_modifiers: match physical_key {
            PhysicalKey::Code(KeyCode::Tab) => Key::Named(NamedKey::Tab),
            PhysicalKey::Code(KeyCode::PageUp) => Key::Named(NamedKey::PageUp),
            PhysicalKey::Code(KeyCode::PageDown) => Key::Named(NamedKey::PageDown),
            PhysicalKey::Code(KeyCode::Home) => Key::Named(NamedKey::Home),
            PhysicalKey::Code(KeyCode::End) => Key::Named(NamedKey::End),
            _ => Key::Character(" ".into()),
        },
    }
}

fn repeated_pressed_key_event(physical_key: PhysicalKey) -> KeyEvent {
    let mut event = pressed_key_event(physical_key);
    event.repeat = true;
    event
}

fn released_key_event(physical_key: PhysicalKey) -> KeyEvent {
    let mut event = pressed_key_event(physical_key);
    event.state = ElementState::Released;
    event
}

fn text_key_event(text: &str) -> KeyEvent {
    KeyEvent {
        physical_key: PhysicalKey::Code(KeyCode::KeyA),
        logical_key: Key::Character(text.into()),
        text: Some(text.into()),
        location: KeyLocation::Standard,
        state: ElementState::Pressed,
        repeat: false,
        text_with_all_modifiers: None,
        key_without_modifiers: Key::Character(text.into()),
    }
}

fn repeated_text_key_event(text: &str) -> KeyEvent {
    let mut event = text_key_event(text);
    event.repeat = true;
    event
}

fn flush_text_input_commits<VM: crate::foundation::view_model::ViewModel>(
    handler: &mut BoundRuntimeHandler<VM>,
) {
    let _ = handler.flush_pending_text_input_changes();
}

fn dispatch_lifecycle_if_dirty<VM: crate::foundation::view_model::ViewModel>(
    handler: &mut BoundRuntimeHandler<VM>,
) {
    handler.dispatch_lifecycle_events_if_needed();
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

#[derive(Debug)]
struct TestEventLoop;

#[derive(Debug)]
struct TestDisplayHandle;

impl HasDisplayHandle for TestDisplayHandle {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
        Ok(DisplayHandle::windows())
    }
}

impl ActiveEventLoop for TestEventLoop {
    fn create_proxy(&self) -> EventLoopProxy {
        panic!("not needed in runtime tests")
    }

    fn create_window(
        &self,
        _window_attributes: WindowAttributes,
    ) -> Result<Box<dyn Window>, RequestError> {
        panic!("not needed in runtime tests")
    }

    fn create_custom_cursor(
        &self,
        _custom_cursor: CustomCursorSource,
    ) -> Result<CustomCursor, RequestError> {
        panic!("not needed in runtime tests")
    }

    fn available_monitors(&self) -> Box<dyn Iterator<Item = MonitorHandle>> {
        Box::new(std::iter::empty())
    }

    fn primary_monitor(&self) -> Option<MonitorHandle> {
        None
    }

    fn listen_device_events(&self, _allowed: DeviceEvents) {}

    fn system_theme(&self) -> Option<WindowTheme> {
        None
    }

    fn set_control_flow(&self, _control_flow: ControlFlow) {}

    fn control_flow(&self) -> ControlFlow {
        ControlFlow::Wait
    }

    fn exit(&self) {}

    fn exiting(&self) -> bool {
        false
    }

    fn owned_display_handle(&self) -> OwnedDisplayHandle {
        OwnedDisplayHandle::new(Arc::new(TestDisplayHandle))
    }

    fn rwh_06_handle(&self) -> &dyn HasDisplayHandle {
        static DISPLAY: TestDisplayHandle = TestDisplayHandle;
        &DISPLAY
    }
}

fn cached_scene_shell<VM: crate::foundation::view_model::ViewModel>(
    handler: &BoundRuntimeHandler<VM>,
    viewport: Rect,
    units: UnitContext,
) -> CachedScene<VM> {
    CachedScene {
        viewport,
        units,
        focused_widget: None,
        focus_visible: false,
        pressed_widget: None,
        selected_text: None,
        caret_visible: false,
        theme_epoch: handler.theme_store.version(),
        animation_epoch: 0,
        layout_animation_epoch: 0,
        scroll_epoch: 0,
        hover_epoch: 0,
        text_input_epoch: 0,
        hovered_scrollbar: None,
        active_scrollbar: None,
        computed_valid: true,
        layout: None,
        computed: Default::default(),
        lifecycle_states: Default::default(),
        scene_chunks: Default::default(),
        scene_chunk_parts: Default::default(),
        visual_contexts: Default::default(),
        dependencies: DependencyGraph::default(),
    }
}

#[test]
fn prune_removed_widget_state_clears_virtual_state_cache() {
    let invalidation = InvalidationSignal::new();
    let mut handler = test_handler(None, invalidation);
    let widget_id = WidgetId::next();
    handler
        .virtual_states
        .insert(widget_id, VirtualCacheState::default());

    handler.prune_removed_widget_state(&std::collections::HashSet::from([widget_id]));

    assert!(!handler.virtual_states.contains_key(&widget_id));
}

#[derive(Default)]
struct LifecycleVm {
    mounts: usize,
    updates: usize,
    unmounts: usize,
}

#[derive(Default)]
struct SwitchVm {
    checked: bool,
}

#[derive(Default)]
struct TextInputVm {
    value: String,
}

#[derive(Default)]
struct CanvasEventVm {
    hover_events: Vec<CanvasPointerEvent>,
    clicks: usize,
    widget_clicks: usize,
    mouse_downs: usize,
    mouse_ups: usize,
    wheel_events: usize,
    drag_events: usize,
    drag_end_events: usize,
}

impl crate::foundation::view_model::ViewModel for LifecycleVm {
    fn new(_context: &ViewModelContext) -> Self {
        Self::default()
    }

    fn view(&self) -> Element<Self>
    where
        Self: Sized,
    {
        Stack::new().into()
    }
}

impl crate::foundation::view_model::ViewModel for SwitchVm {
    fn new(_context: &ViewModelContext) -> Self {
        Self::default()
    }

    fn view(&self) -> Element<Self>
    where
        Self: Sized,
    {
        Stack::new().into()
    }
}

impl crate::foundation::view_model::ViewModel for TextInputVm {
    fn new(_context: &ViewModelContext) -> Self {
        Self::default()
    }

    fn view(&self) -> Element<Self>
    where
        Self: Sized,
    {
        Stack::new().into()
    }
}

impl crate::foundation::view_model::ViewModel for CanvasEventVm {
    fn new(_context: &ViewModelContext) -> Self {
        Self::default()
    }

    fn view(&self) -> Element<Self>
    where
        Self: Sized,
    {
        Stack::new().into()
    }
}

mod audio_video_tests;
mod cache_lifecycle_tests;
mod canvas_tests;
mod focus_selection_tests;
mod gesture_tests;
mod scroll_tests;
mod text_input_tests;
mod tooltip_tests;
mod window_theme_tests;
