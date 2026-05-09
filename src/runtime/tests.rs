use super::{
    centered_window_position_for_monitor, text_cursor_index_at_point, BoundRuntimeHandler,
    CachedScene, WindowBindings,
};
use crate::animation::AnimationCoordinator;
use crate::application::{ApplicationConfig, ThemeSelection, WindowRole};
use crate::dialog::async_dialog_channel;
use crate::foundation::binding::ViewModelContext;
use crate::foundation::binding::{InvalidationSignal, Signal, TextController};
use crate::foundation::color::Color;
use crate::foundation::view_model::{Command, ValueCommand};
use crate::platform::dpi::{LogicalSize, PhysicalPosition, PhysicalSize};
use crate::platform::event::{ElementState, Ime, KeyEvent, MouseScrollDelta};
use crate::platform::keyboard::{Key, KeyCode, KeyLocation, ModifiersState, NamedKey, PhysicalKey};
use crate::text::font::FontCatalog;
use crate::ui::layout::{Axis, Overflow};
use crate::ui::theme::{Theme, ThemeMode, ThemeSet};
use crate::ui::unit::{dp, Dp, UnitContext};
use crate::ui::widget::{
    Button, Canvas, CanvasItem, CanvasMouseButton, CanvasPath, CanvasPointerEvent, CanvasShadow,
    CanvasStroke, Checkbox, CursorStyle, Flex, HitInteraction, Input, PathBuilder, Point, Select,
    SelectOption, Text, TextEditState, Textarea, WidgetTree,
};
use crate::ui::widget::{Element, Stack, WidgetId};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
#[cfg(feature = "video")]
use std::time::Duration;
use std::time::Instant;

#[cfg(feature = "video")]
use crate::media::TextureFrame;
use crate::notification::async_notification_channel;
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
    fn new(_context: &ViewModelContext) -> Self {
        todo!()
    }

    fn view(&self) -> Element<Self>
    where
        Self: Sized,
    {
        todo!()
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
        fonts: FontCatalog::default(),
        theme: ThemeSelection::System,
        theme_set: ThemeSet::default(),
        window_icon: None,
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
            _ => Key::Character(" ".into()),
        },
        text: None,
        location: KeyLocation::Standard,
        state: ElementState::Pressed,
        repeat: false,
        text_with_all_modifiers: None,
        key_without_modifiers: match physical_key {
            PhysicalKey::Code(KeyCode::Tab) => Key::Named(NamedKey::Tab),
            _ => Key::Character(" ".into()),
        },
    }
}

fn repeated_pressed_key_event(physical_key: PhysicalKey) -> KeyEvent {
    let mut event = pressed_key_event(physical_key);
    event.repeat = true;
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
fn centered_window_position_uses_monitor_center() {
    let position = centered_window_position_for_monitor(
        Some(PhysicalPosition::new(-1920, 0)),
        PhysicalSize::new(1920, 1080),
        1.0,
        LogicalSize::new(960.0, 540.0),
    );

    assert_eq!(position, Some(PhysicalPosition::new(-1440, 270)));
}

#[test]
fn centered_window_position_clamps_to_monitor_origin_for_oversized_window() {
    let position = centered_window_position_for_monitor(
        Some(PhysicalPosition::new(100, 200)),
        PhysicalSize::new(800, 600),
        1.0,
        LogicalSize::new(1200.0, 700.0),
    );

    assert_eq!(position, Some(PhysicalPosition::new(100, 200)));
}

#[test]
fn window_control_close_request_marks_handler_for_close() {
    let invalidation = InvalidationSignal::new();
    let mut handler = test_handler(None, invalidation);
    let context = handler.command_context();

    context.window().close();

    assert!(handler.drain_window_requests());
    assert!(!handler.drain_window_requests());
}

#[test]
fn bound_theme_modes_resolve_through_configured_theme_set() {
    let invalidation = InvalidationSignal::new();
    let (theme_set, light, dark) = custom_theme_set();
    let mode = Signal::new(|| ThemeMode::Light, invalidation.clone());
    let mut handler = test_handler_with_config(
        TestVm,
        None,
        invalidation.clone(),
        test_config_with_theme(ThemeSelection::System, theme_set),
    );
    handler.window_bindings.theme_mode = Some(mode);

    handler.sync_theme_binding();
    assert_eq!(handler.theme, light);

    handler.window_bindings.theme_mode = Some(Signal::new(|| ThemeMode::Dark, invalidation));
    handler.sync_theme_binding();
    assert_eq!(handler.theme, dark);
}

#[test]
fn bound_theme_set_updates_current_theme_without_mode_change() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let (theme_set, light, _dark) = custom_theme_set();
    let themes = context.state(theme_set);
    let theme_binding = themes.signal();
    let mut handler = test_handler_with_config(
        TestVm,
        None,
        invalidation.clone(),
        test_config_with_theme(ThemeSelection::System, ThemeSet::default()),
    );
    handler.window_bindings.theme_mode =
        Some(Signal::new(|| ThemeMode::Light, invalidation.clone()));
    handler.window_bindings.theme_set = Some(theme_binding);

    handler.sync_theme_binding();
    assert_eq!(handler.theme, light);

    let mut updated_light = Theme::light();
    updated_light.colors.background = Color::hexa(0xFFFFFFFF);
    updated_light.colors.primary = Color::hexa(0xFFAA00FF);
    themes.update(|themes| {
        themes.light = Arc::new(updated_light.clone());
    });

    handler.sync_theme_binding();
    assert_eq!(handler.theme, updated_light);
}

#[test]
fn hover_path_reuses_cached_computed_scene() {
    let invalidation = InvalidationSignal::new();
    let resolve_count = Arc::new(AtomicUsize::new(0));
    let child = {
        let resolve_count = resolve_count.clone();
        Signal::new(
            move || {
                resolve_count.fetch_add(1, Ordering::SeqCst);
                Text::new("hover").cursor(CursorStyle::Pointer)
            },
            invalidation.clone(),
        )
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
fn clearing_pointer_position_preserves_cached_layout_for_hover_recompute() {
    let invalidation = InvalidationSignal::new();
    let tree = WidgetTree::new(Text::new("hover").cursor(CursorStyle::Pointer));
    let mut handler = test_handler(Some(tree), invalidation);
    handler.cursor_position = Some(Point::new(dp(10.0), dp(10.0)));

    let viewport = handler.viewport_rect();
    assert!(handler.handle_hover(viewport));
    let hover_epoch = handler.hover_epoch;
    let _ = handler.computed_scene();
    assert!(handler.cached_scene.is_some());

    handler.clear_pointer_position();

    assert!(handler.hovered_widgets.is_empty());
    assert_eq!(handler.hover_epoch, hover_epoch.wrapping_add(1));
    assert!(handler.cached_scene.is_some());
}

#[test]
fn scrollbar_hover_preserves_cached_layout_for_hover_recompute() {
    let invalidation = InvalidationSignal::new();
    let tree = WidgetTree::new(
        Textarea::<TestVm>::new("line 0\nline 1\nline 2\nline 3\nline 4\nline 5").height(dp(52.0)),
    );
    let mut handler = test_handler(Some(tree), invalidation);
    let region = handler
        .computed_scene()
        .scroll_regions
        .iter()
        .find(|region| region.vertical_thumb.is_some())
        .copied()
        .expect("textarea scroll region with a vertical thumb should exist");
    let thumb = region
        .vertical_thumb
        .expect("vertical scrollbar thumb should exist");

    assert!(handler.cached_scene.is_some());

    handler.cursor_position = Some(Point {
        x: thumb.x + Dp::new(thumb.width.get() * 0.5),
        y: thumb.y + Dp::new(thumb.height.get() * 0.5),
    });

    assert!(handler.sync_scrollbar_hover());
    assert_eq!(
        handler.hovered_scrollbar.map(|handle| handle.id),
        Some(region.id)
    );
    assert!(handler.cached_scene.is_some());
}

#[test]
fn scene_cache_invalidates_when_units_change() {
    let invalidation = InvalidationSignal::new();
    let handler = test_handler(None, invalidation);
    let viewport = handler.viewport_rect();
    let cached = CachedScene::<TestVm> {
        viewport,
        units: UnitContext::new(1.0, 1.0),
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
fn scene_layout_cache_survives_visual_only_animation_epoch_change() {
    let invalidation = InvalidationSignal::new();
    let mut handler = test_handler(None, invalidation);
    let viewport = handler.viewport_rect();
    let cached = CachedScene::<TestVm> {
        viewport,
        units: UnitContext::new(1.0, 1.0),
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
        layout: None,
        computed: Default::default(),
    };

    handler.animation_epoch = 1;

    assert!(handler.scene_layout_cache_matches(&cached, viewport, UnitContext::new(1.0, 1.0),));
}

#[test]
fn theme_animation_invalidates_cached_layout() {
    let invalidation = InvalidationSignal::new();
    let mut handler = test_handler(None, invalidation);
    let viewport = handler.viewport_rect();
    let cached = CachedScene::<TestVm> {
        viewport,
        units: UnitContext::new(1.0, 1.0),
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
        layout: None,
        computed: Default::default(),
    };

    handler.layout_animation_epoch = 1;

    assert!(!handler.scene_layout_cache_matches(
        &cached,
        viewport,
        UnitContext::new(1.0, 1.0),
    ));
}

#[test]
fn theme_mode_change_invalidates_cached_layout_when_theme_changes() {
    let invalidation = InvalidationSignal::new();
    let mode = Signal::new(|| ThemeMode::Light, invalidation.clone());
    let (theme_set, _light, _dark) = custom_theme_set();
    let mut handler = test_handler_with_config(
        TestVm,
        None,
        invalidation.clone(),
        test_config_with_theme(ThemeSelection::System, theme_set),
    );
    handler.window_bindings.theme_mode = Some(mode);
    handler.sync_theme_binding();

    let viewport = handler.viewport_rect();
    let cached = CachedScene::<TestVm> {
        viewport,
        units: UnitContext::new(1.0, 1.0),
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
        layout: None,
        computed: Default::default(),
    };

    handler.window_bindings.theme_mode = Some(Signal::new(|| ThemeMode::Dark, invalidation));
    handler.sync_theme_binding();

    assert!(!handler.scene_layout_cache_matches(
        &cached,
        viewport,
        UnitContext::new(1.0, 1.0),
    ));
}

#[test]
fn animation_scene_invalidation_preserves_cached_layout() {
    let invalidation = InvalidationSignal::new();
    let tree = WidgetTree::new(
        Textarea::<TestVm>::new("line 0\nline 1\nline 2\nline 3\nline 4\nline 5").height(dp(52.0)),
    );
    let mut handler = test_handler(Some(tree), invalidation);
    let viewport = handler.viewport_rect();
    let units = handler.unit_context();

    let _ = handler.computed_scene();
    assert!(handler.cached_scene.is_some());
    assert!(!handler.text_input_regions.is_empty());

    handler.animation_epoch = handler.animation_epoch.wrapping_add(1);
    handler.invalidate_computed_scene();

    assert!(handler.cached_scene.is_some());
    assert!(handler.text_input_regions.is_empty());
    assert!(handler.scene_layout_cache_matches(
        handler
            .cached_scene
            .as_ref()
            .expect("cached scene should remain available"),
        viewport,
        units,
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
fn scene_cache_invalidates_when_focused_widget_changes() {
    let invalidation = InvalidationSignal::new();
    let mut handler = test_handler(None, invalidation);
    let viewport = handler.viewport_rect();
    let cached = CachedScene::<TestVm> {
        viewport,
        units: UnitContext::new(1.0, 1.0),
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
        layout: None,
        computed: Default::default(),
    };

    handler.focused_widget = Some(super::FocusedWidget {
        widget_id: WidgetId::next(),
        on_blur: None,
    });

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
fn disabled_control_defaults_to_not_allowed_cursor() {
    let invalidation = InvalidationSignal::new();
    let tree = WidgetTree::new(Checkbox::new(false).disable(true).size(dp(120.0), dp(30.0)));
    let mut handler = test_handler(Some(tree), invalidation);
    handler.cursor_position = Some(Point::new(dp(10.0), dp(10.0)));

    let hovered = handler.hover_path(handler.viewport_rect());
    assert_eq!(
        hovered.last().and_then(|hovered| hovered.cursor_style),
        Some(CursorStyle::NotAllowed)
    );
}

#[test]
fn clicking_open_select_trigger_closes_dropdown() {
    let invalidation = InvalidationSignal::new();
    let select: Element<TestVm> = Select::new(
        vec![SelectOption::new("email".to_string(), "Email".to_string())],
        None::<String>,
    )
    .size(dp(160.0), dp(32.0))
    .into();
    let select_id = select.id;
    let tree = WidgetTree::new(select);
    let mut handler = test_handler(Some(tree), invalidation);
    let viewport = handler.viewport_rect();

    handler.cursor_position = Some(Point::new(dp(10.0), dp(10.0)));
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);
    assert_eq!(handler.focused_widget_id(), Some(select_id));
    assert_eq!(handler.resolved_select_open_state(select_id), Some(true));

    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);
    assert_eq!(handler.focused_widget_id(), Some(select_id));
    assert_eq!(handler.resolved_select_open_state(select_id), Some(false));
}

#[test]
fn clicking_outside_closes_open_select_dropdown() {
    let invalidation = InvalidationSignal::new();
    let select: Element<TestVm> = Select::new(
        vec![SelectOption::new("email".to_string(), "Email".to_string())],
        None::<String>,
    )
    .size(dp(160.0), dp(32.0))
    .into();
    let select_id = select.id;
    let filler: Element<TestVm> = Button::new("Other")
        .size(dp(160.0), dp(32.0))
        .top(dp(40.0))
        .position_absolute()
        .into();
    let tree = WidgetTree::new(Stack::new().child([select, filler]));
    let mut handler = test_handler(Some(tree), invalidation);
    let viewport = handler.viewport_rect();

    handler.cursor_position = Some(Point::new(dp(10.0), dp(10.0)));
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);
    assert_eq!(handler.resolved_select_open_state(select_id), Some(true));

    handler.cursor_position = Some(Point::new(dp(10.0), dp(60.0)));
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);
    assert_eq!(handler.resolved_select_open_state(select_id), Some(false));
}

#[test]
fn tab_focuses_first_widget_when_none_is_focused() {
    let invalidation = InvalidationSignal::new();
    let first: Element<TestVm> = Button::new("First").size(dp(80.0), dp(30.0)).into();
    let first_id = first.id;
    let second: Element<TestVm> = Button::new("Second").size(dp(80.0), dp(30.0)).into();
    let tree = WidgetTree::new(Flex::new(Axis::Vertical).child([first, second]));
    let mut handler = test_handler(Some(tree), invalidation);

    let changed =
        handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Tab)));

    assert!(changed);
    assert_eq!(handler.focused_widget_id(), Some(first_id));
}

#[test]
fn tab_advances_to_next_focusable_widget() {
    let invalidation = InvalidationSignal::new();
    let first: Element<TestVm> = Button::new("First").size(dp(80.0), dp(30.0)).into();
    let first_id = first.id;
    let second: Element<TestVm> = Button::new("Second").size(dp(80.0), dp(30.0)).into();
    let second_id = second.id;
    let tree = WidgetTree::new(Flex::new(Axis::Vertical).child([first, second]));
    let mut handler = test_handler(Some(tree), invalidation);
    handler.focused_widget = Some(super::FocusedWidget {
        widget_id: first_id,
        on_blur: None,
    });

    let changed =
        handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Tab)));

    assert!(changed);
    assert_eq!(handler.focused_widget_id(), Some(second_id));
}

#[test]
fn shift_tab_moves_focus_backward() {
    let invalidation = InvalidationSignal::new();
    let first: Element<TestVm> = Button::new("First").size(dp(80.0), dp(30.0)).into();
    let first_id = first.id;
    let second: Element<TestVm> = Button::new("Second").size(dp(80.0), dp(30.0)).into();
    let second_id = second.id;
    let tree = WidgetTree::new(Flex::new(Axis::Vertical).child([first, second]));
    let mut handler = test_handler(Some(tree), invalidation);
    handler.focused_widget = Some(super::FocusedWidget {
        widget_id: second_id,
        on_blur: None,
    });
    handler.modifiers = ModifiersState::SHIFT;

    let changed =
        handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Tab)));

    assert!(changed);
    assert_eq!(handler.focused_widget_id(), Some(first_id));
}

#[test]
fn mouse_focus_does_not_mark_widget_as_focused_for_styling() {
    let invalidation = InvalidationSignal::new();
    let button: Element<TestVm> = Button::new("First").size(dp(80.0), dp(30.0)).into();
    let button_id = button.id;
    let tree = WidgetTree::new(button);
    let mut handler = test_handler(Some(tree), invalidation);

    handler.cursor_position = Some(Point::new(dp(10.0), dp(10.0)));
    handler.handle_mouse_press(
        handler.viewport_rect(),
        Instant::now(),
        CanvasMouseButton::Left,
    );

    assert_eq!(handler.focused_widget_id(), Some(button_id));
    assert!(!handler.widget_state_map(None).get(button_id).focused);
}

#[test]
fn tab_focus_marks_widget_as_focused_for_styling() {
    let invalidation = InvalidationSignal::new();
    let button: Element<TestVm> = Button::new("First").size(dp(80.0), dp(30.0)).into();
    let button_id = button.id;
    let tree = WidgetTree::new(button);
    let mut handler = test_handler(Some(tree), invalidation);

    handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Tab)));

    assert_eq!(handler.focused_widget_id(), Some(button_id));
    assert!(handler.widget_state_map(None).get(button_id).focused);
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
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);

    handler.cursor_position = Some(Point {
        x: frame.x + frame.width - 1.0,
        y: frame.y + (frame.height * 0.5),
    });
    assert!(handler.handle_text_selection_drag());
    assert_eq!(handler.selected_text, Some(text_id));

    let state = handler
        .text_edit_states
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
            false,
            false,
            false,
            Point::ZERO,
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
    handler.text_edit_states.insert(
        text_id,
        TextEditState {
            cursor: 11,
            anchor: 6,
            composition: None,
            scroll_x: Dp::ZERO,
            scroll_y: Dp::ZERO,
            preferred_column_x: None,
        },
    );

    assert_eq!(handler.selected_text_for_copy().as_deref(), Some("world"));
}

#[derive(Default)]
struct SwitchVm {
    checked: bool,
}

#[derive(Default)]
struct TextInputVm {
    value: String,
}

impl crate::foundation::view_model::ViewModel for TextInputVm {
    fn new(_context: &ViewModelContext) -> Self {
        todo!()
    }

    fn view(&self) -> Element<Self>
    where
        Self: Sized,
    {
        todo!()
    }
}

impl crate::foundation::view_model::ViewModel for SwitchVm {
    fn new(_context: &ViewModelContext) -> Self {
        todo!()
    }

    fn view(&self) -> Element<Self>
    where
        Self: Sized,
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
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);

    let checked = handler.with_view_model(|vm| vm.checked);
    assert!(checked);
}

#[test]
fn clicking_checkbox_dispatches_toggled_value() {
    let invalidation = InvalidationSignal::new();
    let tree = WidgetTree::new(
        Checkbox::new(false)
            .label("Accept")
            .on_change(ValueCommand::new(|vm: &mut SwitchVm, value| {
                vm.checked = value
            }))
            .size(dp(120.0), dp(30.0)),
    );
    let mut handler = test_handler_with_vm(SwitchVm::default(), Some(tree), invalidation);
    let viewport = handler.viewport_rect();

    let frame = {
        let computed = handler.computed_scene();
        computed
            .hit_regions
            .iter()
            .find_map(|region| match &region.interaction {
                HitInteraction::Checkbox { .. } => Some(region.rect),
                _ => None,
            })
            .expect("checkbox hit region should exist")
    };

    handler.cursor_position = Some(Point {
        x: frame.x + (frame.width * 0.5),
        y: frame.y + (frame.height * 0.5),
    });
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);

    let checked = handler.with_view_model(|vm| vm.checked);
    assert!(checked);
}

#[test]
fn focused_input_receives_inserted_text_via_on_change() {
    let invalidation = InvalidationSignal::new();
    let controller = TextController::from("hi");
    let callback_controller = controller.clone();
    let tree = WidgetTree::new(Input::new(controller).on_change(Command::new(
        move |vm: &mut TextInputVm| {
            vm.value = callback_controller.text();
        },
    )));
    let mut handler = test_handler_with_vm(TextInputVm::default(), Some(tree), invalidation);
    let viewport = handler.viewport_rect();
    let frame = {
        let computed = handler.computed_scene();
        computed
            .hit_regions
            .iter()
            .find_map(|region| match &region.interaction {
                HitInteraction::TextInput { .. } => Some(region.rect),
                _ => None,
            })
            .expect("input hit region should exist")
    };

    handler.cursor_position = Some(Point {
        x: frame.x + frame.width - dp(4.0),
        y: frame.y + (frame.height * 0.5),
    });
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);
    handler.handle_keyboard_input(&text_key_event("a"));
    flush_text_input_commits(&mut handler);

    let value = handler.with_view_model(|vm| vm.value.clone());
    assert_eq!(value, "hia");
}

#[test]
fn focused_input_accepts_repeated_text_input() {
    let invalidation = InvalidationSignal::new();
    let controller = TextController::from("hi");
    let callback_controller = controller.clone();
    let tree = WidgetTree::new(Input::new(controller).on_change(Command::new(
        move |vm: &mut TextInputVm| {
            vm.value = callback_controller.text();
        },
    )));
    let mut handler = test_handler_with_vm(TextInputVm::default(), Some(tree), invalidation);
    let viewport = handler.viewport_rect();
    let frame = {
        let computed = handler.computed_scene();
        computed
            .hit_regions
            .iter()
            .find_map(|region| match &region.interaction {
                HitInteraction::TextInput { .. } => Some(region.rect),
                _ => None,
            })
            .expect("input hit region should exist")
    };

    handler.cursor_position = Some(Point {
        x: frame.x + frame.width - dp(4.0),
        y: frame.y + (frame.height * 0.5),
    });
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);
    handler.handle_keyboard_input(&repeated_text_key_event("a"));
    flush_text_input_commits(&mut handler);

    let value = handler.with_view_model(|vm| vm.value.clone());
    assert_eq!(value, "hia");
}

#[test]
fn focused_input_batches_change_set_until_flush() {
    let invalidation = InvalidationSignal::new();
    let callback_count = Arc::new(AtomicUsize::new(0));
    let callback_count_capture = callback_count.clone();
    let controller = TextController::from("hi");
    let callback_controller = controller.clone();
    let tree = WidgetTree::new(Input::new(controller).on_change_set(ValueCommand::new(
        move |vm: &mut TextInputVm, _change_set: crate::mvvm::TextChangeSet| {
            callback_count_capture.fetch_add(1, Ordering::SeqCst);
            vm.value = callback_controller.text();
        },
    )));
    let mut handler = test_handler_with_vm(TextInputVm::default(), Some(tree), invalidation);
    let viewport = handler.viewport_rect();
    let frame = {
        let computed = handler.computed_scene();
        computed
            .hit_regions
            .iter()
            .find_map(|region| match &region.interaction {
                HitInteraction::TextInput { .. } => Some(region.rect),
                _ => None,
            })
            .expect("input hit region should exist")
    };

    handler.cursor_position = Some(Point {
        x: frame.x + frame.width - dp(4.0),
        y: frame.y + (frame.height * 0.5),
    });
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);
    handler.handle_keyboard_input(&text_key_event("a"));
    handler.handle_keyboard_input(&text_key_event("b"));

    assert_eq!(handler.with_view_model(|vm| vm.value.clone()), "");
    assert_eq!(callback_count.load(Ordering::SeqCst), 0);

    flush_text_input_commits(&mut handler);

    assert_eq!(handler.with_view_model(|vm| vm.value.clone()), "hiab");
    assert_eq!(callback_count.load(Ordering::SeqCst), 1);
}

#[test]
fn input_backspace_preserves_multibyte_boundaries_with_rope_buffer() {
    let invalidation = InvalidationSignal::new();
    let controller = TextController::from("a中🙂b");
    let callback_controller = controller.clone();
    let tree = WidgetTree::new(Input::new(controller).on_change(Command::new(
        move |vm: &mut TextInputVm| {
            vm.value = callback_controller.text();
        },
    )));
    let mut handler = test_handler_with_vm(TextInputVm::default(), Some(tree), invalidation);
    let viewport = handler.viewport_rect();
    let frame = {
        let computed = handler.computed_scene();
        computed
            .hit_regions
            .iter()
            .find_map(|region| match &region.interaction {
                HitInteraction::TextInput {
                    multiline: false, ..
                } => Some(region.rect),
                _ => None,
            })
            .expect("input hit region should exist")
    };

    handler.cursor_position = Some(Point {
        x: frame.x + frame.width - dp(4.0),
        y: frame.y + (frame.height * 0.5),
    });
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);

    let text_id = handler
        .focused_widget_id()
        .expect("input should be focused after click");
    handler.text_edit_states.insert(
        text_id,
        TextEditState {
            cursor: "a中🙂".len(),
            anchor: "a中🙂".len(),
            composition: None,
            scroll_x: Dp::ZERO,
            scroll_y: Dp::ZERO,
            preferred_column_x: None,
        },
    );

    handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Backspace)));
    flush_text_input_commits(&mut handler);

    let value = handler.with_view_model(|vm| vm.value.clone());
    assert_eq!(value, "a中b");
}

#[test]
fn input_backspace_repeats_while_key_is_held() {
    let invalidation = InvalidationSignal::new();
    let controller = TextController::from("abcd");
    let callback_controller = controller.clone();
    let tree = WidgetTree::new(Input::new(controller).on_change(Command::new(
        move |vm: &mut TextInputVm| {
            vm.value = callback_controller.text();
        },
    )));
    let mut handler = test_handler_with_vm(TextInputVm::default(), Some(tree), invalidation);
    let viewport = handler.viewport_rect();
    let frame = {
        let computed = handler.computed_scene();
        computed
            .hit_regions
            .iter()
            .find_map(|region| match &region.interaction {
                HitInteraction::TextInput { .. } => Some(region.rect),
                _ => None,
            })
            .expect("input hit region should exist")
    };

    handler.cursor_position = Some(Point {
        x: frame.x + frame.width - dp(4.0),
        y: frame.y + (frame.height * 0.5),
    });
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);

    handler.handle_keyboard_input(&repeated_pressed_key_event(PhysicalKey::Code(
        KeyCode::Backspace,
    )));
    flush_text_input_commits(&mut handler);

    let value = handler.with_view_model(|vm| vm.value.clone());
    assert_eq!(value, "abc");
}

#[test]
fn repeated_backspace_keeps_deleting_when_widget_value_is_static() {
    let invalidation = InvalidationSignal::new();
    let controller = TextController::from("abcd");
    let callback_controller = controller.clone();
    let tree = WidgetTree::new(Input::new(controller).on_change(Command::new(
        move |vm: &mut TextInputVm| {
            vm.value = callback_controller.text();
        },
    )));
    let mut handler = test_handler_with_vm(TextInputVm::default(), Some(tree), invalidation);
    let viewport = handler.viewport_rect();
    let frame = {
        let computed = handler.computed_scene();
        computed
            .hit_regions
            .iter()
            .find_map(|region| match &region.interaction {
                HitInteraction::TextInput { .. } => Some(region.rect),
                _ => None,
            })
            .expect("input hit region should exist")
    };

    handler.cursor_position = Some(Point {
        x: frame.x + frame.width - dp(4.0),
        y: frame.y + (frame.height * 0.5),
    });
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);

    assert!(
        handler.handle_keyboard_input(&repeated_pressed_key_event(PhysicalKey::Code(
            KeyCode::Backspace,
        )))
    );
    assert!(
        handler.handle_keyboard_input(&repeated_pressed_key_event(PhysicalKey::Code(
            KeyCode::Backspace,
        )))
    );
    flush_text_input_commits(&mut handler);

    let value = handler.with_view_model(|vm| vm.value.clone());
    assert_eq!(value, "ab");
    assert_eq!(
        handler
            .text_input_buffers
            .get(
                &handler
                    .focused_widget_id()
                    .expect("input should remain focused after repeated backspace"),
            )
            .expect("text input buffer should exist")
            .current_text,
        "ab"
    );
}

#[test]
fn focused_input_renders_local_buffer_until_bound_value_catches_up() {
    let invalidation = InvalidationSignal::new();
    let controller = TextController::from("abcd");
    let callback_controller = controller.clone();
    let tree = WidgetTree::new(Input::new(controller).on_change(Command::new(
        move |vm: &mut TextInputVm| {
            vm.value = callback_controller.text();
        },
    )));
    let mut handler = test_handler_with_vm(TextInputVm::default(), Some(tree), invalidation);
    let viewport = handler.viewport_rect();
    let frame = {
        let computed = handler.computed_scene();
        computed
            .hit_regions
            .iter()
            .find_map(|region| match &region.interaction {
                HitInteraction::TextInput { .. } => Some(region.rect),
                _ => None,
            })
            .expect("input hit region should exist")
    };

    handler.cursor_position = Some(Point {
        x: frame.x + frame.width - dp(4.0),
        y: frame.y + (frame.height * 0.5),
    });
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);
    handler.handle_keyboard_input(&repeated_pressed_key_event(PhysicalKey::Code(
        KeyCode::Backspace,
    )));

    let computed = handler.computed_scene();
    assert!(computed
        .scene
        .texts
        .iter()
        .any(|primitive| primitive.content == "abc"));
}

#[test]
fn textarea_replaces_multibyte_selection_via_rope_buffer() {
    let invalidation = InvalidationSignal::new();
    let controller = TextController::from("ab中🙂cd");
    let callback_controller = controller.clone();
    let tree = WidgetTree::new(Textarea::new(controller).on_change(Command::new(
        move |vm: &mut TextInputVm| {
            vm.value = callback_controller.text();
        },
    )));
    let mut handler = test_handler_with_vm(TextInputVm::default(), Some(tree), invalidation);
    let viewport = handler.viewport_rect();
    let frame = {
        let computed = handler.computed_scene();
        computed
            .hit_regions
            .iter()
            .find_map(|region| match &region.interaction {
                HitInteraction::TextInput {
                    multiline: true, ..
                } => Some(region.rect),
                _ => None,
            })
            .expect("textarea hit region should exist")
    };

    handler.cursor_position = Some(Point {
        x: frame.x + dp(8.0),
        y: frame.y + dp(8.0),
    });
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);

    let text_id = handler
        .focused_widget_id()
        .expect("textarea should be focused after click");
    handler.text_edit_states.insert(
        text_id,
        TextEditState {
            cursor: "ab中🙂".len(),
            anchor: "ab".len(),
            composition: None,
            scroll_x: Dp::ZERO,
            scroll_y: Dp::ZERO,
            preferred_column_x: None,
        },
    );

    handler.handle_keyboard_input(&text_key_event("X"));
    flush_text_input_commits(&mut handler);

    let value = handler.with_view_model(|vm| vm.value.clone());
    assert_eq!(value, "abXcd");
}

#[test]
fn ime_commit_replaces_multibyte_selection_with_rope_buffer() {
    let invalidation = InvalidationSignal::new();
    let controller = TextController::from("你a好");
    let callback_controller = controller.clone();
    let tree = WidgetTree::new(Input::new(controller).on_change(Command::new(
        move |vm: &mut TextInputVm| {
            vm.value = callback_controller.text();
        },
    )));
    let mut handler = test_handler_with_vm(TextInputVm::default(), Some(tree), invalidation);
    let viewport = handler.viewport_rect();
    let frame = {
        let computed = handler.computed_scene();
        computed
            .hit_regions
            .iter()
            .find_map(|region| match &region.interaction {
                HitInteraction::TextInput { .. } => Some(region.rect),
                _ => None,
            })
            .expect("input hit region should exist")
    };

    handler.cursor_position = Some(Point {
        x: frame.x + dp(8.0),
        y: frame.y + (frame.height * 0.5),
    });
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);

    let text_id = handler
        .focused_widget_id()
        .expect("input should be focused after click");
    handler.text_edit_states.insert(
        text_id,
        TextEditState {
            cursor: "你a".len(),
            anchor: "你".len(),
            composition: None,
            scroll_x: Dp::ZERO,
            scroll_y: Dp::ZERO,
            preferred_column_x: None,
        },
    );

    assert!(handler.handle_ime_event(&Ime::Preedit("🙂".to_string(), Some((0, "🙂".len())))));
    let composition = handler
        .text_edit_states
        .get(&text_id)
        .and_then(|state| state.composition.as_ref())
        .expect("composition state should be stored");
    assert_eq!(composition.replace_range, ("你".len(), "你a".len()));

    assert!(handler.handle_ime_event(&Ime::Commit("🙂".to_string())));
    flush_text_input_commits(&mut handler);
    let value = handler.with_view_model(|vm| vm.value.clone());
    assert_eq!(value, "你🙂好");
}

#[test]
fn external_bound_value_rebuilds_text_input_buffer_and_clamps_state() {
    let invalidation = InvalidationSignal::new();
    let controller = TextController::from("hello🙂world");
    let tree: WidgetTree<TestVm> = WidgetTree::new(Input::<TestVm>::new(controller.clone()));
    let mut handler = test_handler(Some(tree), invalidation.clone());
    let viewport = handler.viewport_rect();
    let frame = {
        let computed = handler.computed_scene();
        computed
            .hit_regions
            .iter()
            .find_map(|region| match &region.interaction {
                HitInteraction::TextInput { .. } => Some(region.rect),
                _ => None,
            })
            .expect("input hit region should exist")
    };

    handler.cursor_position = Some(Point {
        x: frame.x + frame.width - dp(4.0),
        y: frame.y + (frame.height * 0.5),
    });
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);

    let text_id = handler
        .focused_widget_id()
        .expect("input should be focused after click");
    handler.text_edit_states.insert(
        text_id,
        TextEditState {
            cursor: "hello🙂world".len(),
            anchor: "hello🙂world".len(),
            composition: None,
            scroll_x: Dp::ZERO,
            scroll_y: Dp::ZERO,
            preferred_column_x: None,
        },
    );

    handler.sync_text_input_buffer(text_id);
    assert_eq!(
        handler
            .text_input_buffers
            .get(&text_id)
            .expect("text input buffer should exist")
            .external_value,
        "hello🙂world"
    );

    controller.set_text("中");
    handler.request_redraw_if_dirty(Instant::now());
    let _ = handler.computed_scene();
    handler.sync_text_input_buffer(text_id);

    let buffer_state = handler
        .text_input_buffers
        .get_mut(&text_id)
        .expect("text input buffer should be rebuilt");
    assert_eq!(buffer_state.external_value, "中");
    assert_eq!(buffer_state.current_text, "中");

    let state = handler
        .text_edit_states
        .get(&text_id)
        .expect("text edit state should still exist");
    assert_eq!(state.cursor, "中".len());
    assert_eq!(state.anchor, "中".len());
}

#[test]
fn textarea_large_text_edit_smoke_uses_rope_buffer() {
    let invalidation = InvalidationSignal::new();
    let initial = "0123456789abcdef\n".repeat(2048);
    let controller = TextController::from(initial.clone());
    let callback_controller = controller.clone();
    let tree = WidgetTree::new(Textarea::new(controller).on_change(Command::new(
        move |vm: &mut TextInputVm| {
            vm.value = callback_controller.text();
        },
    )));
    let mut handler = test_handler_with_vm(TextInputVm::default(), Some(tree), invalidation);
    let viewport = handler.viewport_rect();
    let frame = {
        let computed = handler.computed_scene();
        computed
            .hit_regions
            .iter()
            .find_map(|region| match &region.interaction {
                HitInteraction::TextInput {
                    multiline: true, ..
                } => Some(region.rect),
                _ => None,
            })
            .expect("textarea hit region should exist")
    };

    handler.cursor_position = Some(Point {
        x: frame.x + dp(8.0),
        y: frame.y + dp(8.0),
    });
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);

    let text_id = handler
        .focused_widget_id()
        .expect("textarea should be focused after click");
    handler.text_edit_states.insert(
        text_id,
        TextEditState {
            cursor: 0,
            anchor: 0,
            composition: None,
            scroll_x: Dp::ZERO,
            scroll_y: Dp::ZERO,
            preferred_column_x: None,
        },
    );

    handler.handle_keyboard_input(&text_key_event("中"));
    flush_text_input_commits(&mut handler);

    let value = handler.with_view_model(|vm| vm.value.clone());
    assert_eq!(value.len(), initial.len() + "中".len());
    assert!(value.starts_with("中0123456789abcdef"));
}

#[test]
fn focused_text_input_schedules_caret_blink_deadline() {
    let invalidation = InvalidationSignal::new();
    let tree: WidgetTree<TestVm> = WidgetTree::new(Input::<TestVm>::new("hello"));
    let mut handler = test_handler(Some(tree), invalidation);
    let viewport = handler.viewport_rect();
    let frame = {
        let computed = handler.computed_scene();
        computed
            .hit_regions
            .iter()
            .find_map(|region| match &region.interaction {
                HitInteraction::TextInput { .. } => Some(region.rect),
                _ => None,
            })
            .expect("input hit region should exist")
    };

    handler.cursor_position = Some(Point {
        x: frame.x + (frame.width * 0.5),
        y: frame.y + (frame.height * 0.5),
    });
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);

    let deadline = handler.next_deadline(Instant::now());
    assert!(deadline.is_some());

    handler.update_focus(None, None, false);
    let deadline = handler.next_deadline(Instant::now());
    assert!(deadline.is_none());
}

#[test]
fn clicking_text_input_renders_caret_on_first_focused_frame() {
    let invalidation = InvalidationSignal::new();
    let tree: WidgetTree<TestVm> = WidgetTree::new(Input::<TestVm>::new("hello"));
    let mut handler = test_handler(Some(tree), invalidation);
    let viewport = handler.viewport_rect();
    let frame = {
        let computed = handler.computed_scene();
        computed
            .hit_regions
            .iter()
            .find_map(|region| match &region.interaction {
                HitInteraction::TextInput { .. } => Some(region.rect),
                _ => None,
            })
            .expect("input hit region should exist")
    };

    handler.cursor_position = Some(Point {
        x: frame.x + frame.width - dp(4.0),
        y: frame.y + (frame.height * 0.5),
    });
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);

    let computed = handler.computed_scene();
    assert!(
        computed.ime_cursor_area.is_some(),
        "focused input should expose a caret rect on the first focused frame"
    );
    assert!(
        !computed.scene.overlay_shapes.is_empty(),
        "focused input should render the caret immediately after click"
    );
}

#[test]
fn single_line_input_scrolls_horizontally_to_keep_caret_visible() {
    let invalidation = InvalidationSignal::new();
    let value = "0123456789abcdef0123456789";
    let tree = WidgetTree::new(Input::<TestVm>::new(value).size(dp(96.0), dp(40.0)));
    let mut handler = test_handler(Some(tree), invalidation);
    let viewport = handler.viewport_rect();
    let (frame, padding) = {
        let computed = handler.computed_scene();
        computed
            .hit_regions
            .iter()
            .find_map(|region| match &region.interaction {
                HitInteraction::TextInput { frame, padding, .. } => Some((*frame, *padding)),
                _ => None,
            })
            .expect("input hit region should exist")
    };

    handler.cursor_position = Some(Point {
        x: frame.x + dp(8.0),
        y: frame.y + (frame.height * 0.5),
    });
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);
    handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::End)));

    let text_id = handler
        .focused_widget_id()
        .expect("input should be focused after click");
    let state = handler
        .text_edit_states
        .get(&text_id)
        .expect("input edit state should exist");
    assert!(state.scroll_x > Dp::ZERO);

    let inner = frame.inset(padding);
    let caret = handler
        .computed_scene()
        .ime_cursor_area
        .expect("focused input should expose a caret rect");
    assert!(caret.x >= inner.x);
    assert!(caret.right() <= inner.right() + dp(1.0));
}

#[test]
fn clicking_scrolled_single_line_input_repositions_caret_within_visible_text() {
    let invalidation = InvalidationSignal::new();
    let value = "0123456789abcdef0123456789";
    let tree = WidgetTree::new(Input::<TestVm>::new(value).size(dp(96.0), dp(40.0)));
    let mut handler = test_handler(Some(tree), invalidation);
    let viewport = handler.viewport_rect();
    let (frame, padding, text_style) = {
        let computed = handler.computed_scene();
        computed
            .hit_regions
            .iter()
            .find_map(|region| match &region.interaction {
                HitInteraction::TextInput {
                    frame,
                    padding,
                    text_style,
                    ..
                } => Some((*frame, *padding, text_style.clone())),
                _ => None,
            })
            .expect("input hit region should exist")
    };

    handler.cursor_position = Some(Point {
        x: frame.x + dp(8.0),
        y: frame.y + (frame.height * 0.5),
    });
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);
    handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::End)));

    let text_id = handler
        .focused_widget_id()
        .expect("input should be focused after click");
    let scrolled_x = handler
        .text_edit_states
        .get(&text_id)
        .expect("input edit state should exist")
        .scroll_x;
    assert!(scrolled_x > Dp::ZERO);

    let inner = frame.inset(padding);
    let click_point = Point {
        x: inner.x + dp(12.0),
        y: inner.y + (inner.height * 0.5),
    };
    let expected_cursor = text_cursor_index_at_point(
        &handler.font_manager,
        &handler.theme,
        handler.unit_context(),
        frame,
        padding,
        &text_style,
        value,
        false,
        false,
        false,
        Point::new(scrolled_x, Dp::ZERO),
        click_point,
    );

    handler.cursor_position = Some(click_point);
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);

    let state = handler
        .text_edit_states
        .get(&text_id)
        .expect("input edit state should exist after click");
    assert_eq!(state.cursor, expected_cursor);
    assert_eq!(state.anchor, expected_cursor);
    assert!(state.cursor < value.len());
    assert!(
        (state.scroll_x - scrolled_x).abs() <= 0.01,
        "clicking within the visible span should preserve horizontal scroll"
    );

    let caret = handler
        .computed_scene()
        .ime_cursor_area
        .expect("focused input should expose a caret rect");
    assert!(caret.right() < inner.right() - dp(8.0));
}

#[test]
fn dragging_in_scrolled_single_line_input_tracks_pointer_in_visible_text() {
    let invalidation = InvalidationSignal::new();
    let value = "0123456789abcdef0123456789";
    let tree = WidgetTree::new(Input::<TestVm>::new(value).size(dp(96.0), dp(40.0)));
    let mut handler = test_handler(Some(tree), invalidation);
    let viewport = handler.viewport_rect();
    let (frame, padding, text_style) = {
        let computed = handler.computed_scene();
        computed
            .hit_regions
            .iter()
            .find_map(|region| match &region.interaction {
                HitInteraction::TextInput {
                    frame,
                    padding,
                    text_style,
                    ..
                } => Some((*frame, *padding, text_style.clone())),
                _ => None,
            })
            .expect("input hit region should exist")
    };

    handler.cursor_position = Some(Point {
        x: frame.x + dp(8.0),
        y: frame.y + (frame.height * 0.5),
    });
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);
    handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::End)));

    let text_id = handler
        .focused_widget_id()
        .expect("input should be focused after click");
    let scrolled_x = handler
        .text_edit_states
        .get(&text_id)
        .expect("input edit state should exist")
        .scroll_x;
    assert!(scrolled_x > Dp::ZERO);

    let inner = frame.inset(padding);
    let press_point = Point {
        x: inner.x + dp(10.0),
        y: inner.y + (inner.height * 0.5),
    };
    let press_cursor = text_cursor_index_at_point(
        &handler.font_manager,
        &handler.theme,
        handler.unit_context(),
        frame,
        padding,
        &text_style,
        value,
        false,
        false,
        false,
        Point::new(scrolled_x, Dp::ZERO),
        press_point,
    );

    handler.cursor_position = Some(press_point);
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);

    let pressed_state = handler
        .text_edit_states
        .get(&text_id)
        .expect("input edit state should exist after press");
    assert_eq!(pressed_state.cursor, press_cursor);
    assert_eq!(pressed_state.anchor, press_cursor);
    assert!(pressed_state.cursor < value.len());
    let pressed_scroll_x = pressed_state.scroll_x;
    assert!(
        (pressed_scroll_x - scrolled_x).abs() <= 0.01,
        "pressing within the visible span should not reset horizontal scroll"
    );

    let drag_point = Point {
        x: inner.x + (inner.width * 0.5),
        y: inner.y + (inner.height * 0.5),
    };
    let drag_cursor = text_cursor_index_at_point(
        &handler.font_manager,
        &handler.theme,
        handler.unit_context(),
        frame,
        padding,
        &text_style,
        value,
        false,
        false,
        false,
        Point::new(pressed_scroll_x, Dp::ZERO),
        drag_point,
    );

    handler.cursor_position = Some(drag_point);
    assert!(handler.handle_text_selection_drag());

    let state = handler
        .text_edit_states
        .get(&text_id)
        .expect("input edit state should exist after drag");
    assert_eq!(
        state.selection_range(),
        Some((press_cursor.min(drag_cursor), press_cursor.max(drag_cursor)))
    );
    assert!(state.cursor < value.len());
    assert!(state.anchor < value.len());
}

#[test]
fn ime_preedit_scrolls_single_line_input_to_keep_composition_caret_visible() {
    let invalidation = InvalidationSignal::new();
    let tree = WidgetTree::new(Input::<TestVm>::new("").size(dp(96.0), dp(40.0)));
    let mut handler = test_handler(Some(tree), invalidation);
    let viewport = handler.viewport_rect();
    let composition = "0123456789abcdef".repeat(3);
    let (frame, padding) = {
        let computed = handler.computed_scene();
        computed
            .hit_regions
            .iter()
            .find_map(|region| match &region.interaction {
                HitInteraction::TextInput { frame, padding, .. } => Some((*frame, *padding)),
                _ => None,
            })
            .expect("input hit region should exist")
    };

    handler.cursor_position = Some(Point {
        x: frame.x + dp(8.0),
        y: frame.y + (frame.height * 0.5),
    });
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);
    assert!(handler.handle_ime_event(&Ime::Preedit(
        composition.clone(),
        Some((0, composition.len())),
    )));

    let text_id = handler
        .focused_widget_id()
        .expect("input should be focused after click");
    let state = handler
        .text_edit_states
        .get(&text_id)
        .expect("input edit state should exist");
    assert!(state.scroll_x > Dp::ZERO);

    let inner = frame.inset(padding);
    let caret = handler
        .computed_scene()
        .ime_cursor_area
        .expect("focused input should expose a caret rect");
    assert!(caret.x >= inner.x);
    assert!(caret.right() <= inner.right() + dp(1.0));
}

#[test]
fn single_line_input_blur_resets_scroll_and_caret_to_start() {
    let invalidation = InvalidationSignal::new();
    let value = "0123456789abcdef0123456789";
    let tree = WidgetTree::new(Input::<TestVm>::new(value).size(dp(96.0), dp(40.0)));
    let mut handler = test_handler(Some(tree), invalidation);
    let viewport = handler.viewport_rect();
    let (frame, padding) = {
        let computed = handler.computed_scene();
        computed
            .hit_regions
            .iter()
            .find_map(|region| match &region.interaction {
                HitInteraction::TextInput { frame, padding, .. } => Some((*frame, *padding)),
                _ => None,
            })
            .expect("input hit region should exist")
    };

    handler.cursor_position = Some(Point {
        x: frame.x + dp(8.0),
        y: frame.y + (frame.height * 0.5),
    });
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);
    handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::End)));

    let text_id = handler
        .focused_widget_id()
        .expect("input should be focused after click");
    let scrolled_state = handler
        .text_edit_states
        .get(&text_id)
        .expect("input edit state should exist");
    assert!(scrolled_state.scroll_x > Dp::ZERO);
    assert!(scrolled_state.cursor > 0);

    handler.update_focus(None, None, false);

    let blurred_state = handler
        .text_edit_states
        .get(&text_id)
        .expect("input edit state should still exist after blur");
    assert_eq!(blurred_state.cursor, 0);
    assert_eq!(blurred_state.anchor, 0);
    assert_eq!(blurred_state.scroll_x, Dp::ZERO);
    assert_eq!(blurred_state.scroll_y, Dp::ZERO);
    assert!(!handler.scroll_states.contains_key(&text_id));

    let next_focus = handler
        .focusable_widgets_in_tab_order()
        .into_iter()
        .find(|candidate| candidate.widget_id == text_id)
        .expect("input should remain focusable");
    handler.update_focus(Some(next_focus), None, true);

    let inner = frame.inset(padding);
    let caret = handler
        .computed_scene()
        .ime_cursor_area
        .expect("refocused input should expose a caret rect");
    assert!(caret.x >= inner.x);
    assert!(caret.x <= inner.x + dp(1.0));
}

#[test]
fn single_line_input_blur_resets_scroll_even_without_cached_scene() {
    let invalidation = InvalidationSignal::new();
    let value = "0123456789abcdef0123456789";
    let tree = WidgetTree::new(Input::<TestVm>::new(value).size(dp(96.0), dp(40.0)));
    let mut handler = test_handler(Some(tree), invalidation);
    let viewport = handler.viewport_rect();
    let frame = {
        let computed = handler.computed_scene();
        computed
            .hit_regions
            .iter()
            .find_map(|region| match &region.interaction {
                HitInteraction::TextInput { .. } => Some(region.rect),
                _ => None,
            })
            .expect("input hit region should exist")
    };

    handler.cursor_position = Some(Point {
        x: frame.x + dp(8.0),
        y: frame.y + (frame.height * 0.5),
    });
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);
    handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::End)));

    let text_id = handler
        .focused_widget_id()
        .expect("input should be focused after click");
    assert!(
        handler
            .text_edit_states
            .get(&text_id)
            .expect("input edit state should exist")
            .scroll_x
            > Dp::ZERO
    );

    handler.cached_scene = None;
    handler.update_focus(None, None, false);

    let blurred_state = handler
        .text_edit_states
        .get(&text_id)
        .expect("input edit state should still exist after blur");
    assert_eq!(blurred_state.cursor, 0);
    assert_eq!(blurred_state.anchor, 0);
    assert_eq!(blurred_state.scroll_x, Dp::ZERO);
    assert_eq!(blurred_state.scroll_y, Dp::ZERO);
    assert!(!handler.scroll_states.contains_key(&text_id));
}

#[test]
fn textarea_arrow_down_moves_caret_to_next_visual_line() {
    let invalidation = InvalidationSignal::new();
    let tree = WidgetTree::new(Textarea::<TestVm>::new("hello\nworld").height(dp(120.0)));
    let mut handler = test_handler(Some(tree), invalidation);
    let viewport = handler.viewport_rect();
    let frame = {
        let computed = handler.computed_scene();
        computed
            .hit_regions
            .iter()
            .find_map(|region| match &region.interaction {
                HitInteraction::TextInput {
                    multiline: true, ..
                } => Some(region.rect),
                _ => None,
            })
            .expect("textarea hit region should exist")
    };

    handler.cursor_position = Some(Point {
        x: frame.x + dp(8.0),
        y: frame.y + dp(8.0),
    });
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);
    handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::ArrowDown)));

    let text_id = handler
        .focused_widget_id()
        .expect("textarea should be focused after click");
    let state = handler
        .text_edit_states
        .get(&text_id)
        .expect("textarea edit state should exist");
    assert!(state.cursor > "hello\n".len() - 1);
}

#[test]
fn textarea_edit_does_not_create_phantom_blank_line_in_layout_snapshot() {
    let invalidation = InvalidationSignal::new();
    let tree = WidgetTree::new(Textarea::<TestVm>::new("hello\nworld").height(dp(120.0)));
    let mut handler = test_handler(Some(tree), invalidation);
    let viewport = handler.viewport_rect();
    let frame = {
        let computed = handler.computed_scene();
        computed
            .hit_regions
            .iter()
            .find_map(|region| match &region.interaction {
                HitInteraction::TextInput {
                    multiline: true, ..
                } => Some(region.rect),
                _ => None,
            })
            .expect("textarea hit region should exist")
    };

    handler.cursor_position = Some(Point {
        x: frame.x + dp(8.0),
        y: frame.y + dp(8.0),
    });
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);

    let text_id = handler
        .focused_widget_id()
        .expect("textarea should be focused after click");
    handler.text_edit_states.insert(
        text_id,
        TextEditState {
            cursor: 0,
            anchor: 0,
            composition: None,
            scroll_x: Dp::ZERO,
            scroll_y: Dp::ZERO,
            preferred_column_x: None,
        },
    );

    assert!(handler.handle_keyboard_input(&text_key_event("x")));

    let session = handler
        .text_input_buffers
        .get(&text_id)
        .expect("textarea text input session should exist");
    let layout = session
        .layout_snapshot
        .as_ref()
        .expect("textarea layout snapshot should exist after edit");

    assert_eq!(session.current_text, "xhello\nworld");
    assert_eq!(layout.line_count(), 2);
    assert_eq!(layout.line_start(1), "xhello\n".len());
}

#[test]
fn textarea_click_tracks_visual_wrap_for_overflowing_initial_content() {
    let invalidation = InvalidationSignal::new();
    let value = "supercalifragilisticexpialidocious wrapped text with another long visual line";
    let tree = WidgetTree::new(
        Textarea::<TestVm>::new(value)
            .width(dp(140.0))
            .height(dp(52.0))
            .auto_wrap(true),
    );
    let mut handler = test_handler(Some(tree), invalidation);
    let viewport = handler.viewport_rect();
    let (frame, padding, text_style) = {
        let computed = handler.computed_scene();
        computed
            .hit_regions
            .iter()
            .find_map(|region| match &region.interaction {
                HitInteraction::TextInput {
                    frame,
                    padding,
                    text_style,
                    multiline: true,
                    ..
                } => Some((*frame, *padding, text_style.clone())),
                _ => None,
            })
            .expect("textarea hit region should exist")
    };

    let content_viewport = crate::ui::widget::text_input_content_viewport(
        frame,
        padding,
        true,
        true,
        &handler.theme,
        handler.unit_context(),
    );
    let (layout, _font_size, _line_height) = super::input_text_layout(
        &handler.font_manager,
        &handler.theme,
        handler.unit_context(),
        &text_style,
        value,
        true,
        true,
        crate::ui::widget::text_input_layout_width(
            content_viewport,
            true,
            true,
            super::input::INPUT_CARET_WIDTH,
        ),
    );
    assert!(
        layout.line_count() > 1,
        "test value should wrap to multiple visual lines"
    );
    let second_line = 1;
    let sample_x = (layout.x_for_index(layout.line_end(second_line)) - 0.5).max(0.0);
    let sample_y = layout.line_top(second_line) + (layout.line_height(second_line) * 0.5);
    let expected_cursor = layout.index_for_point(sample_x, sample_y);

    handler.cursor_position = Some(Point {
        x: content_viewport.x + Dp::new(sample_x),
        y: content_viewport.y + Dp::new(sample_y),
    });
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);

    let text_id = handler
        .focused_widget_id()
        .expect("textarea should be focused after click");
    let state = handler
        .text_edit_states
        .get(&text_id)
        .expect("textarea edit state should exist");
    assert_eq!(state.cursor, expected_cursor);
}

#[test]
fn textarea_click_reuses_live_session_layout_snapshot() {
    let invalidation = InvalidationSignal::new();
    let tree = WidgetTree::new(Textarea::<TestVm>::new("abcde").height(dp(120.0)));
    let mut handler = test_handler(Some(tree), invalidation);
    let viewport = handler.viewport_rect();
    let (frame, padding, text_style) = {
        let computed = handler.computed_scene();
        computed
            .hit_regions
            .iter()
            .find_map(|region| match &region.interaction {
                HitInteraction::TextInput {
                    frame,
                    padding,
                    text_style,
                    multiline: true,
                    ..
                } => Some((*frame, *padding, text_style.clone())),
                _ => None,
            })
            .expect("textarea hit region should exist")
    };

    handler.cursor_position = Some(Point {
        x: frame.x + dp(8.0),
        y: frame.y + dp(8.0),
    });
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);

    let text_id = handler
        .focused_widget_id()
        .expect("textarea should be focused after click");
    let _ = handler.sync_text_input_buffer(text_id);
    let inner = frame.inset(padding);
    let alternate_text = "a\nb\nc";
    let (alternate_layout, _font_size, _line_height) = super::input_text_layout(
        &handler.font_manager,
        &handler.theme,
        handler.unit_context(),
        &text_style,
        alternate_text,
        true,
        false,
        inner.width.get(),
    );
    let sample_line = 2usize;
    let sample_x = 0.0;
    let sample_y =
        alternate_layout.line_top(sample_line) + (alternate_layout.line_height(sample_line) * 0.5);
    let expected_cursor = alternate_layout.index_for_point(sample_x, sample_y);

    let session = handler
        .text_input_buffers
        .get_mut(&text_id)
        .expect("textarea text input session should exist");
    session.display_text = session.current_text.clone();
    session.layout_snapshot = Some(alternate_layout);

    handler.cursor_position = Some(Point {
        x: inner.x + Dp::new(sample_x),
        y: inner.y + Dp::new(sample_y),
    });
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);

    let state = handler
        .text_edit_states
        .get(&text_id)
        .expect("textarea edit state should exist");
    assert_eq!(state.cursor, expected_cursor);
}

#[test]
fn repeated_tab_does_not_advance_focus() {
    let invalidation = InvalidationSignal::new();
    let first: Element<TestVm> = Input::<TestVm>::new("first").into();
    let second: Element<TestVm> = Input::<TestVm>::new("second").into();
    let tree = WidgetTree::new(Flex::new(Axis::Vertical).child([first, second]));
    let mut handler = test_handler(Some(tree), invalidation);
    let initial_focus = handler.focused_widget_id();

    assert!(!handler
        .handle_keyboard_input(&repeated_pressed_key_event(PhysicalKey::Code(KeyCode::Tab),)));
    assert_eq!(handler.focused_widget_id(), initial_focus);
}

#[test]
fn repeated_arrow_right_moves_single_line_input_cursor() {
    let invalidation = InvalidationSignal::new();
    let tree = WidgetTree::new(Input::<TestVm>::new("hello"));
    let mut handler = test_handler(Some(tree), invalidation);
    let viewport = handler.viewport_rect();
    let frame = {
        let computed = handler.computed_scene();
        computed
            .hit_regions
            .iter()
            .find_map(|region| match &region.interaction {
                HitInteraction::TextInput { .. } => Some(region.rect),
                _ => None,
            })
            .expect("input hit region should exist")
    };

    handler.cursor_position = Some(Point {
        x: frame.x + dp(8.0),
        y: frame.y + (frame.height * 0.5),
    });
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);

    let text_id = handler
        .focused_widget_id()
        .expect("input should be focused after click");
    handler.text_edit_states.insert(
        text_id,
        TextEditState {
            cursor: 0,
            anchor: 0,
            composition: None,
            scroll_x: Dp::ZERO,
            scroll_y: Dp::ZERO,
            preferred_column_x: None,
        },
    );

    handler.handle_keyboard_input(&repeated_pressed_key_event(PhysicalKey::Code(
        KeyCode::ArrowRight,
    )));

    let state = handler
        .text_edit_states
        .get(&text_id)
        .expect("input edit state should exist");
    assert_eq!(state.cursor, "h".len());
    assert_eq!(state.anchor, "h".len());
}

#[test]
fn textarea_arrow_down_scrolls_caret_into_vertical_view() {
    let invalidation = InvalidationSignal::new();
    let value = "line 0\nline 1\nline 2\nline 3\nline 4";
    let tree = WidgetTree::new(Textarea::<TestVm>::new(value).height(dp(52.0)));
    let mut handler = test_handler(Some(tree), invalidation);
    let viewport = handler.viewport_rect();
    let frame = {
        let computed = handler.computed_scene();
        computed
            .hit_regions
            .iter()
            .find_map(|region| match &region.interaction {
                HitInteraction::TextInput {
                    multiline: true, ..
                } => Some(region.rect),
                _ => None,
            })
            .expect("textarea hit region should exist")
    };

    handler.cursor_position = Some(Point {
        x: frame.x + dp(8.0),
        y: frame.y + dp(8.0),
    });
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);

    for _ in 0..4 {
        handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::ArrowDown)));
    }

    let text_id = handler
        .focused_widget_id()
        .expect("textarea should be focused after click");
    let state = handler
        .text_edit_states
        .get(&text_id)
        .expect("textarea edit state should exist");
    let scroll_y = handler
        .scroll_states
        .get(&text_id)
        .map(|offset| offset.y)
        .unwrap_or(Dp::ZERO);

    assert!(state.cursor >= "line 0\nline 1\nline 2\n".len());
    assert!(scroll_y > Dp::ZERO);
}

#[test]
fn textarea_without_auto_wrap_keeps_keyboard_moved_caret_in_view() {
    let invalidation = InvalidationSignal::new();
    let value = (0..6)
        .map(|index| format!("line {index} 0123456789abcdef0123456789abcdef0123456789abcdef"))
        .collect::<Vec<_>>()
        .join("\n");
    let tree = WidgetTree::new(
        Textarea::<TestVm>::new(value)
            .size(dp(140.0), dp(52.0))
            .auto_wrap(false),
    );
    let mut handler = test_handler(Some(tree), invalidation);
    let viewport = handler.viewport_rect();
    let (frame, padding) = {
        let computed = handler.computed_scene();
        computed
            .hit_regions
            .iter()
            .find_map(|region| match &region.interaction {
                HitInteraction::TextInput {
                    frame,
                    padding,
                    multiline: true,
                    auto_wrap: false,
                    ..
                } => Some((*frame, *padding)),
                _ => None,
            })
            .expect("textarea hit region should exist")
    };

    handler.cursor_position = Some(Point {
        x: frame.x + dp(8.0),
        y: frame.y + dp(8.0),
    });
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);
    assert!(handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::End))));
    for _ in 0..5 {
        assert!(handler
            .handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::ArrowDown,))));
    }

    let text_id = handler
        .focused_widget_id()
        .expect("textarea should be focused after click");
    let state = handler
        .text_edit_states
        .get(&text_id)
        .expect("textarea edit state should exist");
    assert!(state.scroll_x > Dp::ZERO);
    assert!(state.scroll_y > Dp::ZERO);

    let inner = frame.inset(padding);
    let caret = handler
        .computed_scene()
        .ime_cursor_area
        .expect("focused textarea should expose a caret rect");
    assert!(caret.x >= inner.x);
    assert!(caret.right() <= inner.right() + dp(1.0));
    assert!(caret.y >= inner.y);
    assert!(caret.bottom() <= inner.bottom() + dp(1.0));
}

#[test]
fn textarea_mouse_wheel_scrolls_vertical_overflow() {
    let invalidation = InvalidationSignal::new();
    let value = "line 0\nline 1\nline 2\nline 3\nline 4\nline 5";
    let tree = WidgetTree::new(Textarea::<TestVm>::new(value).height(dp(52.0)));
    let mut handler = test_handler(Some(tree), invalidation);
    let (text_id, frame) = {
        let computed = handler.computed_scene();
        computed
            .hit_regions
            .iter()
            .find_map(|region| match &region.interaction {
                HitInteraction::TextInput {
                    id,
                    multiline: true,
                    ..
                } => Some((*id, region.rect)),
                _ => None,
            })
            .expect("textarea hit region should exist")
    };

    handler.cursor_position = Some(Point {
        x: frame.x + dp(8.0),
        y: frame.y + dp(8.0),
    });

    assert!(handler.handle_mouse_wheel(MouseScrollDelta::LineDelta(0.0, -2.0)));
    assert!(
        handler
            .scroll_states
            .get(&text_id)
            .map(|offset| offset.y)
            .unwrap_or(Dp::ZERO)
            > Dp::ZERO
            || handler.smooth_scroll_states.contains_key(&text_id)
    );
}

#[test]
fn mouse_wheel_starts_immediately_and_keeps_smooth_target() {
    let invalidation = InvalidationSignal::new();
    let scroller: Element<TestVm> = Stack::new()
        .size(dp(100.0), dp(100.0))
        .overflow_y(Overflow::Scroll)
        .child(Stack::new().size(dp(100.0), dp(320.0)))
        .into();
    let scroller_id = scroller.id;
    let tree = WidgetTree::new(scroller);
    let mut handler = test_handler(Some(tree), invalidation);
    let region = handler
        .computed_scene()
        .scroll_regions
        .iter()
        .find(|region| region.id == scroller_id)
        .copied()
        .expect("scroll region should exist");

    handler.cursor_position = Some(Point {
        x: region.visible_frame.x + dp(8.0),
        y: region.visible_frame.y + dp(8.0),
    });

    assert!(handler.handle_mouse_wheel(MouseScrollDelta::LineDelta(0.0, -2.0)));
    assert!(handler.handle_mouse_wheel(MouseScrollDelta::LineDelta(0.0, -2.0)));

    let offset = handler
        .scroll_states
        .get(&scroller_id)
        .map(|state| state.y)
        .expect("scroll offset should exist");
    assert!(offset > Dp::ZERO);
    assert!(offset < dp(160.0));

    let target = handler
        .smooth_scroll_states
        .get(&scroller_id)
        .map(|state| state.target.y)
        .expect("smooth scroll target should exist");
    assert_eq!(target, dp(160.0));
}

#[test]
fn textarea_click_after_prefocus_scroll_keeps_scrolled_viewport() {
    let invalidation = InvalidationSignal::new();
    let value = (0..8)
        .map(|index| format!("line {index}"))
        .collect::<Vec<_>>()
        .join("\n");
    let tree = WidgetTree::new(Textarea::<TestVm>::new(value).height(dp(52.0)));
    let mut handler = test_handler(Some(tree), invalidation);
    let viewport = handler.viewport_rect();
    let (text_id, frame) = {
        let computed = handler.computed_scene();
        computed
            .hit_regions
            .iter()
            .find_map(|region| match &region.interaction {
                HitInteraction::TextInput {
                    id,
                    multiline: true,
                    ..
                } => Some((*id, region.rect)),
                _ => None,
            })
            .expect("textarea hit region should exist")
    };

    handler.cursor_position = Some(Point {
        x: frame.x + dp(8.0),
        y: frame.y + dp(8.0),
    });

    assert!(handler.handle_mouse_wheel(MouseScrollDelta::LineDelta(0.0, -2.0)));
    while handler.advance_smooth_scroll() {}

    let scrolled_y = handler
        .scroll_states
        .get(&text_id)
        .map(|offset| offset.y)
        .unwrap_or(Dp::ZERO);
    assert!(scrolled_y > Dp::ZERO);

    let line_zero_visible_before_focus = handler
        .computed_scene()
        .scene
        .texts
        .iter()
        .any(|primitive| primitive.content.contains("line 0"));
    assert!(
        !line_zero_visible_before_focus,
        "prefocus wheel scrolling should move the first line out of view"
    );

    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);

    let state = handler
        .text_edit_states
        .get(&text_id)
        .expect("textarea edit state should exist after click");
    assert!(
        (state.scroll_y - scrolled_y).abs() <= 0.01,
        "clicking after prefocus scroll should preserve the existing vertical offset"
    );

    let line_zero_visible_after_focus = handler
        .computed_scene()
        .scene
        .texts
        .iter()
        .any(|primitive| primitive.content.contains("line 0"));
    assert!(
        !line_zero_visible_after_focus,
        "focusing the textarea should not jump the viewport back to the top"
    );
}

#[test]
fn textarea_backspace_keeps_scrolled_viewport_and_scroll_range() {
    let invalidation = InvalidationSignal::new();
    let value = (0..24)
        .map(|index| format!("line {index}"))
        .collect::<Vec<_>>()
        .join("\n");
    let tree = WidgetTree::new(Textarea::<TestVm>::new(value).height(dp(52.0)));
    let mut handler = test_handler(Some(tree), invalidation);
    let viewport = handler.viewport_rect();
    let (text_id, frame) = {
        let computed = handler.computed_scene();
        computed
            .hit_regions
            .iter()
            .find_map(|region| match &region.interaction {
                HitInteraction::TextInput {
                    id,
                    multiline: true,
                    ..
                } => Some((*id, region.rect)),
                _ => None,
            })
            .expect("textarea hit region should exist")
    };

    handler.cursor_position = Some(Point {
        x: frame.x + dp(24.0),
        y: frame.y + dp(8.0),
    });

    assert!(handler.handle_mouse_wheel(MouseScrollDelta::LineDelta(0.0, -8.0)));
    while handler.advance_smooth_scroll() {}

    let scrolled_before_focus = handler
        .scroll_states
        .get(&text_id)
        .map(|offset| offset.y)
        .unwrap_or(Dp::ZERO);
    assert!(scrolled_before_focus > Dp::ZERO);

    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);
    assert!(
        handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Backspace,)))
    );

    let state = handler
        .text_edit_states
        .get(&text_id)
        .expect("textarea edit state should exist after backspace");
    assert!(
        state.scroll_y > Dp::ZERO,
        "editing while focused should keep the vertical scroll offset"
    );

    let scroll_region = handler
        .computed_scene()
        .scroll_regions
        .iter()
        .find(|region| region.id == text_id)
        .copied()
        .expect("textarea scroll region should exist");
    assert!(
        scroll_region.max_offset().y > Dp::ZERO,
        "focused textarea should keep a vertical scroll range after backspace"
    );

    let line_zero_visible_after_backspace = handler
        .computed_scene()
        .scene
        .texts
        .iter()
        .any(|primitive| primitive.content.contains("line 0"));
    assert!(
        !line_zero_visible_after_backspace,
        "focused textarea should not jump back to the first line after backspace"
    );
}

#[test]
fn textarea_without_auto_wrap_keeps_edited_caret_in_view() {
    let invalidation = InvalidationSignal::new();
    let value = (0..6)
        .map(|index| format!("line {index} 0123456789abcdef0123456789abcdef0123456789abcdef"))
        .collect::<Vec<_>>()
        .join("\n");
    let tree = WidgetTree::new(
        Textarea::<TestVm>::new(value)
            .size(dp(140.0), dp(52.0))
            .auto_wrap(false),
    );
    let mut handler = test_handler(Some(tree), invalidation);
    let viewport = handler.viewport_rect();
    let (text_id, frame, padding) = {
        let computed = handler.computed_scene();
        computed
            .hit_regions
            .iter()
            .find_map(|region| match &region.interaction {
                HitInteraction::TextInput {
                    id,
                    frame,
                    padding,
                    multiline: true,
                    auto_wrap: false,
                    ..
                } => Some((*id, *frame, *padding)),
                _ => None,
            })
            .expect("textarea hit region should exist")
    };

    handler.cursor_position = Some(Point {
        x: frame.x + dp(8.0),
        y: frame.y + dp(8.0),
    });
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);
    assert!(handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::End))));
    for _ in 0..5 {
        assert!(handler
            .handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::ArrowDown,))));
    }
    assert!(
        handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Backspace,)))
    );

    let state = handler
        .text_edit_states
        .get(&text_id)
        .expect("textarea edit state should exist after backspace");
    assert!(state.scroll_x > Dp::ZERO);
    assert!(state.scroll_y > Dp::ZERO);

    let inner = frame.inset(padding);
    let caret = handler
        .computed_scene()
        .ime_cursor_area
        .expect("focused textarea should expose a caret rect after edit");
    assert!(caret.x >= inner.x);
    assert!(caret.right() <= inner.right() + dp(1.0));
    assert!(caret.y >= inner.y);
    assert!(caret.bottom() <= inner.bottom() + dp(1.0));

    let scroll_region = handler
        .computed_scene()
        .scroll_regions
        .iter()
        .find(|region| region.id == text_id)
        .copied()
        .expect("textarea scroll region should exist");
    assert!(scroll_region.max_offset().x > Dp::ZERO);
    assert!(scroll_region.max_offset().y > Dp::ZERO);
}

#[test]
fn textarea_mouse_wheel_reaches_last_line_with_tall_line_height() {
    let invalidation = InvalidationSignal::new();
    let value = "line 0\nline 1\nline 2\nline 3\nline 4\nline 5";
    let tree = WidgetTree::new(
        Textarea::<TestVm>::new(value)
            .height(dp(120.0))
            .style(|mode| {
                let mut style = crate::ui::widget::TextareaStyle::default_for(mode);
                style.text_style.line_height = Some(crate::ui::unit::sp(40.0));
                style
            }),
    );
    let mut handler = test_handler(Some(tree), invalidation);
    let frame = {
        let computed = handler.computed_scene();
        computed
            .hit_regions
            .iter()
            .find_map(|region| match &region.interaction {
                HitInteraction::TextInput {
                    multiline: true, ..
                } => Some(region.rect),
                _ => None,
            })
            .expect("textarea hit region should exist")
    };

    handler.cursor_position = Some(Point {
        x: frame.x + dp(8.0),
        y: frame.y + dp(8.0),
    });

    while handler.handle_mouse_wheel(MouseScrollDelta::LineDelta(0.0, -3.0)) {}

    let computed = handler.computed_scene();
    let text_primitive = computed
        .scene
        .texts
        .iter()
        .find(|primitive| primitive.content.contains("line 5"))
        .expect("textarea text primitive should render");
    let inner_bottom = frame.bottom() - dp(8.0);

    assert!(text_primitive.frame.bottom() <= inner_bottom + dp(1.0));
}

#[test]
fn textarea_arrow_up_reduces_vertical_scroll_in_long_text() {
    let invalidation = InvalidationSignal::new();
    let value = "line 0\nline 1\nline 2\nline 3\nline 4\nline 5\nline 6";
    let tree = WidgetTree::new(Textarea::<TestVm>::new(value).height(dp(52.0)));
    let mut handler = test_handler(Some(tree), invalidation);
    let viewport = handler.viewport_rect();
    let frame = {
        let computed = handler.computed_scene();
        computed
            .hit_regions
            .iter()
            .find_map(|region| match &region.interaction {
                HitInteraction::TextInput {
                    multiline: true, ..
                } => Some(region.rect),
                _ => None,
            })
            .expect("textarea hit region should exist")
    };

    handler.cursor_position = Some(Point {
        x: frame.x + dp(8.0),
        y: frame.y + dp(8.0),
    });
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);

    for _ in 0..6 {
        handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::ArrowDown)));
    }

    let text_id = handler
        .focused_widget_id()
        .expect("textarea should be focused after click");
    let scrolled_down = handler
        .scroll_states
        .get(&text_id)
        .map(|offset| offset.y)
        .unwrap_or(Dp::ZERO);
    assert!(scrolled_down > Dp::ZERO);

    for _ in 0..6 {
        handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::ArrowUp)));
    }

    let state = handler
        .text_edit_states
        .get(&text_id)
        .expect("textarea edit state should exist");
    let scrolled_up = handler
        .scroll_states
        .get(&text_id)
        .map(|offset| offset.y)
        .unwrap_or(Dp::ZERO);

    assert!(state.cursor < "line 0\n".len());
    assert!(scrolled_up < scrolled_down);
}

#[test]
fn clicking_disabled_checkbox_does_not_dispatch_toggled_value() {
    let invalidation = InvalidationSignal::new();
    let tree = WidgetTree::new(
        Checkbox::new(false)
            .disable(true)
            .on_change(ValueCommand::new(|vm: &mut SwitchVm, value| {
                vm.checked = value
            }))
            .size(dp(120.0), dp(30.0)),
    );
    let mut handler = test_handler_with_vm(SwitchVm::default(), Some(tree), invalidation);
    let viewport = handler.viewport_rect();

    let frame = {
        let computed = handler.computed_scene();
        computed
            .hit_regions
            .iter()
            .find_map(|region| match &region.interaction {
                HitInteraction::Disabled { .. } => Some(region.rect),
                _ => None,
            })
            .expect("disabled hit region should exist")
    };

    handler.cursor_position = Some(Point {
        x: frame.x + (frame.width * 0.5),
        y: frame.y + (frame.height * 0.5),
    });
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);

    let checked = handler.with_view_model(|vm| vm.checked);
    assert!(!checked);
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
        playback_state: ctx.state(PlaybackState::Ready),
        metrics: ctx.state(VideoMetrics::default()),
        volume: ctx.state(1.0),
        muted: ctx.state(false),
        buffer_memory_limit_bytes: ctx.state(DEFAULT_VIDEO_BUFFER_MEMORY_LIMIT_BYTES),
        video_size: ctx.state(VideoSize {
            width: 160,
            height: 90,
        }),
        error: ctx.state(None),
        surface: ctx.state(VideoSurfaceSnapshot::default()),
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
    mouse_downs: usize,
    mouse_ups: usize,
    wheel_events: usize,
    drag_events: usize,
    drag_end_events: usize,
}

impl crate::foundation::view_model::ViewModel for CanvasEventVm {
    fn new(_context: &ViewModelContext) -> Self {
        Self {
            hover_events: vec![],
            clicks: 0,
            widget_clicks: 0,
            mouse_downs: 0,
            mouse_ups: 0,
            wheel_events: 0,
            drag_events: 0,
            drag_end_events: 0,
        }
    }

    fn view(&self) -> Element<Self>
    where
        Self: Sized,
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
        .on_item_mouse_move(ValueCommand::new(|vm: &mut CanvasEventVm, event| {
            vm.hover_events.push(event);
        })),
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

    handler.handle_mouse_press(
        handler.viewport_rect(),
        Instant::now(),
        CanvasMouseButton::Left,
    );

    let view_model = handler
        .view_model
        .lock()
        .expect("view model lock should not be poisoned");
    assert_eq!(view_model.clicks, 1);
    assert_eq!(view_model.widget_clicks, 0);
}

#[test]
fn canvas_item_mouse_down_up_wheel_and_drag_dispatch() {
    let invalidation = InvalidationSignal::new();
    let tree = WidgetTree::new(
        Canvas::new(vec![CanvasItem::Path(
            CanvasPath::new(
                12_u64,
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
        .on_item_mouse_down(ValueCommand::new(|vm: &mut CanvasEventVm, _event| {
            vm.mouse_downs += 1;
        }))
        .on_item_mouse_up(ValueCommand::new(|vm: &mut CanvasEventVm, _event| {
            vm.mouse_ups += 1;
        }))
        .on_item_wheel(ValueCommand::new(|vm: &mut CanvasEventVm, _event| {
            vm.wheel_events += 1;
        }))
        .on_item_drag(ValueCommand::new(|vm: &mut CanvasEventVm, _event| {
            vm.drag_events += 1;
        }))
        .on_item_drag_end(ValueCommand::new(|vm: &mut CanvasEventVm, _event| {
            vm.drag_end_events += 1;
        })),
    );
    let mut handler = test_handler_with_vm(CanvasEventVm::default(), Some(tree), invalidation);
    let viewport = handler.viewport_rect();
    handler.cursor_position = Some(Point::new(dp(20.0), dp(20.0)));

    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);
    handler.cursor_position = Some(Point::new(dp(36.0), dp(28.0)));
    assert!(handler.handle_canvas_drag());
    assert!(handler.handle_mouse_wheel(MouseScrollDelta::LineDelta(0.0, -1.0)));
    handler.handle_canvas_mouse_release(CanvasMouseButton::Left);

    let view_model = handler
        .view_model
        .lock()
        .expect("view model lock should not be poisoned");
    assert_eq!(view_model.mouse_downs, 1);
    assert_eq!(view_model.mouse_ups, 1);
    assert_eq!(view_model.wheel_events, 1);
    assert_eq!(view_model.drag_events, 1);
    assert_eq!(view_model.drag_end_events, 1);
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
        .on_item_mouse_move(ValueCommand::new(|vm: &mut CanvasEventVm, event| {
            vm.hover_events.push(event);
        })),
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
