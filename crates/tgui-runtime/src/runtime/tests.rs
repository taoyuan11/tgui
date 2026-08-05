use super::state::TouchScrollInertiaState;
use super::{
    centered_window_position_for_monitor, input, input_text_layout, text_cursor_index_at_point,
    BoundRuntimeHandler, CachedScene, FocusedWidget, WindowBindings,
};
use crate::animation::AnimationCoordinator;
use crate::application::{ApplicationConfig, MsaaMode, ResourceBudget, ThemeSelection, WindowRole};
use crate::dialog::async_dialog_channel;
use crate::foundation::binding::ScrollViewController;
use crate::foundation::binding::{DependencyGraph, State, ViewModelContext};
use crate::foundation::binding::{InvalidationSignal, Signal, TextController};
use crate::foundation::color::Color;
use crate::foundation::task::async_task_channel;
use crate::foundation::view_model::{Command, ValueCommand, ViewModel};
use crate::platform::backend::event_loop::{ActiveEventLoop, ControlFlow, EventLoopProxy};
use crate::platform::backend::window::Window;
use crate::platform::dpi::{LogicalSize, PhysicalPosition, PhysicalSize};
use crate::platform::error::RequestError;
use crate::platform::event::{
    ButtonSource, ElementState, FingerId, Ime, KeyEvent, MouseScrollDelta, PointerKind,
    PointerSource, TouchPhase, WindowEvent,
};
use crate::platform::keyboard::{Key, KeyCode, KeyLocation, ModifiersState, NamedKey, PhysicalKey};
use crate::platform::window::WindowAttributes;
use crate::platform::window::{ImeCapabilities, ImeEnableRequest, ImeHint, ImePurpose};
use crate::text::font::FontCatalog;
use crate::ui::layout::{Axis, Insets, Overflow};
use crate::ui::theme::{Theme, ThemeMode, ThemeSet};
use crate::ui::unit::{dp, sp, Dp, UnitContext};
use crate::ui::widget::{
    Button, Canvas, CanvasDragEvent, CanvasMouseButton, CanvasParagraphStyle, CanvasPointerEvent,
    CanvasRecorder, CanvasShadow, CanvasStroke, CanvasTextStyle, CanvasTextVerticalAlign,
    CanvasTextWrap, Carousel, Checkbox, Combobox, ComboboxOption, ContainerStyle, CursorStyle,
    DataGrid, DataGridColumn, DataGridColumnPin, DataGridRow, Flex, FocusScopeOptions,
    HitInteraction, HitRegion, Input, Point, ProgressBar, Rect, ScrollView, Select, SelectOption,
    Slider, Spinner, Switch, Text, TextEditState, Textarea, Tooltip, VirtualCacheState, WidgetKey,
    WidgetTree,
};
use crate::ui::widget::{Element, Stack, WidgetId};
#[cfg(feature = "audio")]
use std::sync::atomic::AtomicBool;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use std::time::Instant;
use winit::monitor::MonitorHandle;

#[cfg(feature = "audio")]
use crate::audio::backend::{
    AudioBackend, BackendSharedState as AudioBackendSharedState,
    DEFAULT_AUDIO_BUFFER_MEMORY_LIMIT_BYTES,
};
#[cfg(feature = "audio")]
use crate::audio::{Audio, AudioController, AudioMetrics, AudioPlaybackState, AudioSource};
use crate::notification::async_notification_channel;
#[cfg(feature = "video")]
use crate::video::backend::{
    BackendSharedState, VideoBackend, DEFAULT_VIDEO_BUFFER_MEMORY_LIMIT_BYTES,
};
#[cfg(feature = "video")]
use crate::video::{
    VideoAudioTrackSelection, VideoController, VideoMetrics, VideoPlaybackState, VideoSize,
    VideoSource, VideoSubtitleTrackSelection, VideoSurface, VideoSurfaceSnapshot,
};
#[derive(Default)]
struct TestVm;

#[derive(Default)]
struct CarouselRuntimeVm {
    selected: usize,
    changes: Vec<usize>,
}

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

impl crate::foundation::view_model::ViewModel for CarouselRuntimeVm {
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
        style_sheet: crate::ui::widget::StyleSheet::default(),
        reduced_motion: false,
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
        style_sheet: crate::ui::widget::StyleSheet::default(),
        reduced_motion: false,
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
    let (task_dispatcher, task_receiver) = async_task_channel();
    BoundRuntimeHandler::new(
        "test".to_string(),
        1,
        WindowRole::Main,
        config,
        Arc::new(Mutex::new(view_model)),
        WindowBindings::default(),
        widget_tree,
        None,
        Vec::new(),
        invalidation,
        AnimationCoordinator::default(),
        dialog_dispatcher,
        Some(dialog_receiver),
        notification_dispatcher,
        Some(notification_receiver),
        task_dispatcher,
        Some(task_receiver),
    )
}

#[test]
fn media_dispatch_skips_static_tree_without_handlers() {
    const CHILDREN: usize = 10_000;
    let children = (0..CHILDREN)
        .map(|index| -> Element<TestVm> { Text::new(index.to_string()).into() })
        .collect::<Vec<_>>();
    let tree = WidgetTree::new(Stack::<TestVm>::new().child(children));
    assert!(!tree.may_have_media_event_handlers());

    let invalidation = InvalidationSignal::new();
    let mut handler = test_handler(Some(tree), invalidation);

    crate::ui::widget::media_event_walk_probe::reset();
    let baseline = handler
        .widget_tree
        .as_ref()
        .expect("widget tree should exist")
        .media_event_states(&handler.media_manager, &handler.theme);
    assert!(baseline.is_empty());
    assert_eq!(
        crate::ui::widget::media_event_walk_probe::visits(),
        CHILDREN + 1,
        "the old path resolves and visits the root plus every child"
    );

    crate::ui::widget::media_event_walk_probe::reset();
    handler.dispatch_media_events();
    assert_eq!(
        crate::ui::widget::media_event_walk_probe::visits(),
        0,
        "the capability fast path must avoid the entire media-event walk"
    );
}

#[test]
fn media_dispatch_tracks_handlers_added_and_removed_by_dynamic_revision() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let visible = context.state(false);
    let tree = WidgetTree::new_legacy(Stack::<TestVm>::new().dynamic_child(
        visible.signal().map_unchecked(|visible| {
            let element: Element<TestVm> = if visible {
                crate::ui::widget::Image::new(crate::media::MediaSource::bytes(
                    br#"<svg xmlns="http://www.w3.org/2000/svg" width="1" height="1"/>"#,
                ))
                .on_loading(Command::new(|_vm: &mut TestVm| {}))
            } else {
                Stack::<TestVm>::new().into()
            };
            element
        }),
    ));
    assert!(
        tree.may_have_media_event_handlers(),
        "dynamic sources must never cache a negative handler capability"
    );
    let mut handler = test_handler(Some(tree), invalidation);

    handler.dispatch_media_events();
    assert!(handler.media_event_states.is_empty());

    visible.set(true);
    handler.dispatch_media_events();
    assert_eq!(handler.media_event_states.len(), 1);

    visible.set(false);
    handler.dispatch_media_events();
    assert!(handler.media_event_states.is_empty());
}

#[test]
fn media_event_walk_tracks_open_portal_and_active_rich_tooltip_content() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let portal_open = context.state(false);
    let source = crate::media::MediaSource::bytes(
        br#"<svg xmlns="http://www.w3.org/2000/svg" width="1" height="1"/>"#,
    );

    let portal_image: Element<TestVm> = crate::ui::widget::Image::new(source.clone())
        .on_loading(Command::new(|_vm: &mut TestVm| {}));
    let portal_image_id = portal_image.id;
    let portal: Element<TestVm> = crate::ui::widget::Portal::new(portal_image)
        .open(portal_open.signal())
        .anchor(Rect::new(dp(24.0), dp(18.0), dp(1.0), dp(1.0)))
        .into();

    let tooltip_image: Element<TestVm> =
        crate::ui::widget::Image::new(source).on_loading(Command::new(|_vm: &mut TestVm| {}));
    let tooltip_image_id = tooltip_image.id;
    let trigger: Element<TestVm> = Button::new("Inspect")
        .tooltip(Tooltip::content(tooltip_image).delay(Duration::ZERO))
        .into();
    let trigger_id = trigger.id;
    let tree = WidgetTree::new_legacy(Stack::new().child([trigger, portal]));
    let handler = test_handler(Some(tree), invalidation);

    let state_ids = |active_tooltip| {
        handler
            .widget_tree
            .as_ref()
            .expect("widget tree should exist")
            .media_event_states_with_active_tooltip(
                &handler.media_manager,
                &handler.theme,
                active_tooltip,
            )
            .into_iter()
            .map(|state| state.widget_id)
            .collect::<std::collections::HashSet<_>>()
    };

    assert!(state_ids(None).is_empty());

    portal_open.set(true);
    let portal_states = state_ids(None);
    assert!(portal_states.contains(&portal_image_id));
    assert!(!portal_states.contains(&tooltip_image_id));

    portal_open.set(false);
    let tooltip_states = state_ids(Some(trigger_id));
    assert!(!tooltip_states.contains(&portal_image_id));
    assert!(tooltip_states.contains(&tooltip_image_id));

    assert!(state_ids(None).is_empty());
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

    fn available_monitors(&self) -> Box<dyn Iterator<Item = MonitorHandle>> {
        Box::new(std::iter::empty())
    }

    fn primary_monitor(&self) -> Option<MonitorHandle> {
        None
    }

    fn set_control_flow(&self, _control_flow: ControlFlow) {}

    fn control_flow(&self) -> ControlFlow {
        ControlFlow::Wait
    }

    fn exit(&self) {}
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
        style_sheet_version: handler.config.style_sheet.version(),
        density: handler.theme.density,
        reduced_motion: handler.reduced_motion,
        text_scale_bits: units.font_scale().to_bits(),
        animation_epoch: 0,
        layout_animation_epoch: 0,
        accessibility_animation_epoch: 0,
        scroll_epoch: 0,
        hover_epoch: 0,
        text_input_epoch: 0,
        external_portal_revision: 0,
        hovered_scrollbar: None,
        active_scrollbar: None,
        layout_valid: true,
        computed_valid: true,
        gpu_scroll_deferred: false,
        layout: None,
        computed: Default::default(),
        lifecycle_states: Default::default(),
        scene_chunks: Default::default(),
        scene_chunk_parts: Default::default(),
        visual_contexts: Default::default(),
        layout_slot_bindings: Default::default(),
        reactive_slot_bindings: Default::default(),
        media_texture_bindings: Default::default(),
        media_texture_binding_index: Default::default(),
        caret_decoration: None,
        text_input_slot_bindings: Default::default(),
        scroll_view_controller_bindings: Default::default(),
        dependencies: DependencyGraph::default(),
        strict_capability_report: None,
    }
}

#[test]
fn carousel_auto_play_advances_after_interval() {
    let invalidation = InvalidationSignal::new();
    let items: Vec<Element<CarouselRuntimeVm>> =
        vec![Text::new("first").into(), Text::new("second").into()];
    let tree = WidgetTree::new(
        Carousel::new(items, 0usize)
            .auto_play(Duration::from_millis(10))
            .on_change(ValueCommand::new(|vm: &mut CarouselRuntimeVm, selected| {
                vm.selected = selected;
                vm.changes.push(selected);
            })),
    );
    let mut handler = test_handler_with_vm(CarouselRuntimeVm::default(), Some(tree), invalidation);
    let now = Instant::now();

    assert!(!handler.drive_carousel_auto_play(now));
    assert_eq!(
        handler.next_carousel_wakeup_deadline,
        Some(now + Duration::from_millis(10))
    );
    assert!(!handler.drive_carousel_auto_play(now + Duration::from_millis(5)));
    assert!(handler.drive_carousel_auto_play(now + Duration::from_millis(10)));

    {
        let view_model = handler.view_model.lock().unwrap();
        assert_eq!(view_model.selected, 1);
        assert_eq!(view_model.changes, vec![1]);
    }

    // A static initial index is uncontrolled. The wrapper updates the internal selection before
    // forwarding the callback, so the next tick wraps from 1 back to 0 instead of repeatedly
    // emitting 1.
    assert!(handler.drive_carousel_auto_play(now + Duration::from_millis(20)));
    let view_model = handler.view_model.lock().unwrap();
    assert_eq!(view_model.selected, 0);
    assert_eq!(view_model.changes, vec![1, 0]);
}

#[test]
fn uncontrolled_carousel_auto_play_advances_without_an_on_change_callback() {
    let invalidation = InvalidationSignal::new();
    let items: Vec<Element<CarouselRuntimeVm>> =
        vec![Text::new("first").into(), Text::new("second").into()];
    let tree = WidgetTree::new(Carousel::new(items, 0usize).auto_play(Duration::from_millis(10)));
    let mut handler = test_handler_with_vm(CarouselRuntimeVm::default(), Some(tree), invalidation);
    let now = Instant::now();

    assert!(!handler.drive_carousel_auto_play(now));
    assert!(handler.drive_carousel_auto_play(now + Duration::from_millis(10)));
    assert_eq!(
        handler.computed_scene().carousel_auto_play[0]
            .selected
            .resolve_untracked(),
        1
    );
    assert!(handler.drive_carousel_auto_play(now + Duration::from_millis(20)));
    assert_eq!(
        handler.computed_scene().carousel_auto_play[0]
            .selected
            .resolve_untracked(),
        0
    );
}

#[test]
fn carousel_auto_play_disabled_signal_pauses_and_restarts_a_full_interval() {
    let invalidation = InvalidationSignal::new();
    let disabled = State::new(false, invalidation.clone());
    let items: Vec<Element<CarouselRuntimeVm>> =
        vec![Text::new("first").into(), Text::new("second").into()];
    let tree = WidgetTree::new(
        Carousel::new(items, 0usize)
            .disabled(disabled.signal())
            .auto_play(Duration::from_millis(10)),
    );
    let mut handler = test_handler_with_vm(CarouselRuntimeVm::default(), Some(tree), invalidation);
    let now = Instant::now();

    assert!(!handler.drive_carousel_auto_play(now));
    disabled.set(true);
    assert!(!handler.drive_carousel_auto_play(now + Duration::from_millis(10)));
    assert_eq!(handler.next_carousel_wakeup_deadline, None);

    disabled.set(false);
    assert!(!handler.drive_carousel_auto_play(now + Duration::from_millis(100)));
    assert_eq!(
        handler.next_carousel_wakeup_deadline,
        Some(now + Duration::from_millis(110))
    );
    assert!(handler.drive_carousel_auto_play(now + Duration::from_millis(110)));
    assert_eq!(
        handler.computed_scene().carousel_auto_play[0]
            .selected
            .resolve_untracked(),
        1
    );
}

#[test]
fn carousel_auto_play_reads_live_selection_and_pauses_while_hovered() {
    let invalidation = InvalidationSignal::new();
    let selected = State::new(0usize, invalidation.clone());
    let selected_for_command = selected.clone();
    let items: Vec<Element<CarouselRuntimeVm>> = vec![
        Text::new("first").into(),
        Text::new("second").into(),
        Text::new("third").into(),
    ];
    let tree = WidgetTree::new(
        Carousel::new(items, selected.signal())
            .auto_play(Duration::from_millis(10))
            .on_change(ValueCommand::new(
                move |vm: &mut CarouselRuntimeVm, next| {
                    selected_for_command.set(next);
                    vm.selected = next;
                    vm.changes.push(next);
                },
            )),
    );
    let mut handler = test_handler_with_vm(CarouselRuntimeVm::default(), Some(tree), invalidation);
    let frame = handler
        .computed_scene()
        .carousel_auto_play
        .first()
        .expect("carousel autoplay metadata")
        .frame;
    let now = Instant::now();

    assert!(!handler.drive_carousel_auto_play(now));
    selected.set(1);
    handler.cursor_position = Some(Point::new(
        frame.x + frame.width * 0.5,
        frame.y + frame.height * 0.5,
    ));
    assert!(!handler.drive_carousel_auto_play(now + Duration::from_millis(10)));
    assert!(handler.view_model.lock().unwrap().changes.is_empty());

    handler.cursor_position = Some(Point::new(
        frame.right() + dp(20.0),
        frame.bottom() + dp(20.0),
    ));
    assert!(!handler.drive_carousel_auto_play(now + Duration::from_millis(19)));
    assert!(!handler.drive_carousel_auto_play(now + Duration::from_millis(20)));
    assert!(handler.drive_carousel_auto_play(now + Duration::from_millis(29)));
    assert_eq!(handler.view_model.lock().unwrap().changes.as_slice(), [2]);

    // The command updated the same State. A second tick must resolve that live
    // value and wrap from 2 to 0 even if no widget tree is rebuilt.
    assert!(handler.drive_carousel_auto_play(now + Duration::from_millis(39)));
    assert_eq!(
        handler.view_model.lock().unwrap().changes.as_slice(),
        [2, 0]
    );
}

#[test]
fn carousel_auto_play_pauses_while_focus_is_inside_and_restarts_a_full_interval() {
    let invalidation = InvalidationSignal::new();
    let first: Element<CarouselRuntimeVm> = Button::new("first").into();
    let first_id = first.id;
    let outside: Element<CarouselRuntimeVm> = Button::new("outside").into();
    let outside_id = outside.id;
    let tree = WidgetTree::new(
        Flex::new(Axis::Vertical).child([
            Carousel::new(vec![first, Button::new("second").into()], 0usize)
                .auto_play(Duration::from_millis(10))
                .into(),
            outside,
        ]),
    );
    let mut handler = test_handler_with_vm(CarouselRuntimeVm::default(), Some(tree), invalidation);
    let now = Instant::now();

    handler.focused_widget = Some(FocusedWidget {
        widget_id: first_id,
        scope_path: Vec::new(),
        on_blur: None,
    });
    assert!(!handler.drive_carousel_auto_play(now));
    assert!(!handler.drive_carousel_auto_play(now + Duration::from_millis(100)));
    assert_eq!(handler.next_carousel_wakeup_deadline, None);

    handler.focused_widget = Some(FocusedWidget {
        widget_id: outside_id,
        scope_path: Vec::new(),
        on_blur: None,
    });
    assert!(!handler.drive_carousel_auto_play(now + Duration::from_millis(100)));
    assert_eq!(
        handler.next_carousel_wakeup_deadline,
        Some(now + Duration::from_millis(110))
    );
    assert!(handler.drive_carousel_auto_play(now + Duration::from_millis(110)));
}

#[test]
fn carousel_previous_and_next_buttons_resolve_live_selection_on_activation() {
    let invalidation = InvalidationSignal::new();
    let selected = State::new(0usize, invalidation.clone());
    let selected_for_command = selected.clone();
    let items: Vec<Element<CarouselRuntimeVm>> = vec![
        Text::new("first").into(),
        Text::new("second").into(),
        Text::new("third").into(),
    ];
    let tree = WidgetTree::new(Carousel::new(items, selected.signal()).on_change(
        ValueCommand::new(move |vm: &mut CarouselRuntimeVm, next| {
            selected_for_command.set(next);
            vm.selected = next;
            vm.changes.push(next);
        }),
    ));
    let mut handler = test_handler_with_vm(CarouselRuntimeVm::default(), Some(tree), invalidation);
    let viewport = handler.viewport_rect();
    let scene = handler.computed_scene().clone();
    let button_center = |label: &str| {
        let text = scene
            .scene
            .texts
            .iter()
            .find(|text| text.content.as_ref() == label)
            .unwrap_or_else(|| panic!("missing {label} button"));
        Point::new(
            text.frame.x + text.frame.width * 0.5,
            text.frame.y + text.frame.height * 0.5,
        )
    };

    // Change selection after the tree and button commands already exist. A
    // build-time target snapshot would incorrectly choose 1 here.
    selected.set(1);
    handler.cursor_position = Some(button_center("Next"));
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);
    assert_eq!(handler.view_model.lock().unwrap().changes.as_slice(), [2]);

    handler.cursor_position = Some(button_center("Prev"));
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);
    assert_eq!(
        handler.view_model.lock().unwrap().changes.as_slice(),
        [2, 1]
    );
}

#[test]
fn reduced_motion_defaults_and_window_binding_override() {
    let invalidation = InvalidationSignal::new();
    let mut config = test_config();
    config.reduced_motion = false;
    let state = State::new(true, invalidation.clone());
    let bindings = WindowBindings {
        reduced_motion: Some(state.signal()),
        ..Default::default()
    };

    let (dialog_dispatcher, dialog_receiver) = async_dialog_channel();
    let (notification_dispatcher, notification_receiver) = async_notification_channel();
    let (task_dispatcher, task_receiver) = async_task_channel();
    let handler = BoundRuntimeHandler::new(
        "test".to_string(),
        1,
        WindowRole::Main,
        config.clone(),
        Arc::new(Mutex::new(TestVm)),
        bindings,
        None,
        None,
        Vec::new(),
        invalidation.clone(),
        AnimationCoordinator::default(),
        dialog_dispatcher,
        Some(dialog_receiver),
        notification_dispatcher,
        Some(notification_receiver),
        task_dispatcher,
        Some(task_receiver),
    );
    assert!(handler.active_reduced_motion());

    let default_handler = test_handler_with_config(TestVm, None, invalidation, config);
    assert!(!default_handler.active_reduced_motion());
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
    drag_sequence: Vec<(&'static str, CanvasMouseButton)>,
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

mod accessibility_tests;
mod audio_video_tests;
mod cache_lifecycle_tests;
mod calendar_tests;
mod canvas_tests;
mod clickable_surface_tests;
mod date_time_upload_tests;
mod drawer_tests;
mod file_drop_tests;
mod focus_navigation_cache_tests;
mod focus_selection_tests;
mod gesture_tests;
mod idle_redraw_tests;
mod list_tests;
mod menu_tests;
mod modal_tests;
mod pointer_button_tests;
mod popover_tests;
mod portal_tests;
mod radio_group_tests;
mod scroll_tests;
mod slider_tests;
mod splitter_tests;
mod table_tests;
mod tabs_tests;
mod text_input_tests;
mod toast_tests;
mod tooltip_tests;
mod tree_tests;
mod window_theme_tests;
