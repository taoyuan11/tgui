use super::*;

use std::sync::{Arc, Mutex};
use std::time::Instant;

use crate::ui::widget::{Button, Input, Popover, PopoverTriggerMode};

#[derive(Default)]
struct PopoverVm {
    _open_changes: Vec<bool>,
    _checked: bool,
    input: Option<TextController>,
}

impl crate::foundation::view_model::ViewModel for PopoverVm {
    fn new(context: &ViewModelContext) -> Self {
        Self {
            _open_changes: Vec::new(),
            _checked: false,
            input: Some(context.text_controller("hello")),
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
fn escape_closes_fixed_open_popover() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let close_calls = Arc::new(Mutex::new(Vec::new()));
    let close_calls_cmd = close_calls.clone();
    let tree = WidgetTree::new(
        Popover::new(Button::new("More").size(dp(90.0), dp(36.0)))
            .content(Text::new("Body"))
            .open(true)
            .on_open_change(ValueCommand::new(move |_vm: &mut PopoverVm, open| {
                close_calls_cmd.lock().unwrap().push(open);
            })),
    );
    let mut handler = test_handler_with_vm(PopoverVm::new(&context), Some(tree), invalidation);
    handler.invalidate_computed_scene();
    let _ = handler.computed_scene();

    assert!(handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Escape))));
    assert_eq!(close_calls.lock().unwrap().as_slice(), &[false]);
}

#[test]
fn outside_click_closes_fixed_open_popover() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let close_calls = Arc::new(Mutex::new(Vec::new()));
    let close_calls_cmd = close_calls.clone();
    let tree = WidgetTree::new(
        Popover::new(Button::new("More").size(dp(90.0), dp(36.0)))
            .content(Text::new("Body"))
            .open(true)
            .on_open_change(ValueCommand::new(move |_vm: &mut PopoverVm, open| {
                close_calls_cmd.lock().unwrap().push(open);
            })),
    );
    let mut handler = test_handler_with_vm(PopoverVm::new(&context), Some(tree), invalidation);
    handler.invalidate_computed_scene();
    let _ = handler.computed_scene();

    let _ = handler.consume_overlay_close_handlers_outside_click(Point::new(dp(380.0), dp(260.0)));
    assert_eq!(close_calls.lock().unwrap().as_slice(), &[false]);
}

#[test]
fn hover_preview_visible_when_trigger_hovered() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let tree = WidgetTree::new(
        Popover::<PopoverVm>::new(Button::new("More").size(dp(90.0), dp(36.0)))
            .content(Text::new("Preview body"))
            .trigger_mode(PopoverTriggerMode::ClickAndHoverPreview),
    );
    let mut handler = test_handler_with_vm(PopoverVm::new(&context), Some(tree), invalidation);
    handler.cursor_position = Some(Point::new(dp(40.0), dp(20.0)));
    let viewport = handler.viewport_rect();
    let _ = handler.handle_hover(viewport);
    handler.invalidate_computed_scene();
    let computed = handler.computed_scene();
    let labels: Vec<_> = computed
        .scene
        .overlay_texts
        .iter()
        .map(|text| text.content.as_str())
        .collect();
    assert!(labels.iter().any(|text| *text == "Preview body"));
}

#[test]
fn hover_preview_remains_visible_when_cursor_moves_into_popover_rect() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let tree = WidgetTree::new(
        Popover::<PopoverVm>::new(Button::new("More").size(dp(90.0), dp(36.0)))
            .content(Button::new("Inside Action").size(dp(140.0), dp(36.0)))
            .trigger_mode(PopoverTriggerMode::ClickAndHoverPreview),
    );
    let mut handler = test_handler_with_vm(PopoverVm::new(&context), Some(tree), invalidation);
    let viewport = handler.viewport_rect();

    handler.cursor_position = Some(Point::new(dp(40.0), dp(20.0)));
    let _ = handler.handle_hover(viewport);
    handler.invalidate_computed_scene();
    let shown = handler.computed_scene();
    let popover_rect = shown
        .overlay_close_handlers
        .iter()
        .find(|handle| handle.layer == crate::runtime::overlay::OverlayLayer::Popover)
        .map(|handle| handle.rect)
        .expect("popover overlay rect should exist");

    handler.cursor_position = Some(Point::new(
        popover_rect.x + dp(12.0),
        popover_rect.y + dp(12.0),
    ));
    let _ = handler.handle_hover(viewport);
    handler.invalidate_computed_scene();
    let hovered_panel = handler.computed_scene();
    let labels: Vec<_> = hovered_panel
        .scene
        .overlay_texts
        .iter()
        .map(|text| text.content.as_str())
        .collect();
    assert!(labels.iter().any(|text| *text == "Inside Action"));

    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);
    assert!(
        handler.focused_widget_id().is_some(),
        "interactive content inside popover should remain hittable"
    );
}

#[test]
fn input_inside_popover_can_receive_focus_and_show_caret() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let vm = PopoverVm::new(&context);
    let input_controller = vm.input.clone().unwrap();

    let tree = WidgetTree::new(
        Popover::new(Button::new("Open").size(dp(90.0), dp(36.0)))
            .content(Input::new(input_controller.clone()).width(dp(200.0)))
            .open(true),
    );

    let mut handler = test_handler_with_vm(vm, Some(tree), invalidation);
    handler.invalidate_computed_scene();
    let computed = handler.computed_scene();

    // 验证 Popover 内的 Input 生成了 TextInput hit region
    let text_input_regions: Vec<_> = computed
        .overlay_hit_regions
        .iter()
        .filter(|region| matches!(region.interaction, HitInteraction::TextInput { .. }))
        .collect();
    assert!(
        !text_input_regions.is_empty(),
        "Input inside popover should generate TextInput hit region"
    );

    // 点击 Input 使其获得焦点
    let input_region = text_input_regions[0];
    let click_point = Point::new(
        input_region.rect.x + dp(10.0),
        input_region.rect.y + dp(10.0),
    );
    handler.cursor_position = Some(click_point);
    let viewport = handler.viewport_rect();
    let _ = handler.handle_hover(viewport);
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);

    // 验证 Input 获得了焦点
    let focused_input = handler.focused_text_input_id();
    assert!(
        focused_input.is_some(),
        "Input inside popover should be focusable"
    );

    // 验证光标可见性（在闪烁周期的可见阶段）
    let now = Instant::now();
    let caret_visible = handler.caret_visible_at(now, focused_input);
    assert!(
        caret_visible,
        "Caret should be visible when input is focused"
    );

    // 重新渲染以获取光标状态
    handler.invalidate_computed_scene();
    let computed_with_focus = handler.computed_scene();

    // 验证 IME 光标区域被设置
    assert!(
        computed_with_focus.ime_cursor_area.is_some(),
        "IME cursor area should be set for focused input in popover"
    );
}

#[test]
fn clicking_inside_popover_should_not_close_it() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let close_calls = Arc::new(Mutex::new(Vec::new()));
    let close_calls_cmd = close_calls.clone();

    let tree = WidgetTree::new(
        Popover::new(Button::new("Trigger").size(dp(90.0), dp(36.0)))
            .content(Button::new("Inside Button").size(dp(120.0), dp(36.0)))
            .open(true)
            .on_open_change(ValueCommand::new(move |_vm: &mut PopoverVm, open| {
                close_calls_cmd.lock().unwrap().push(open);
            })),
    );

    let mut handler = test_handler_with_vm(PopoverVm::new(&context), Some(tree), invalidation);
    handler.invalidate_computed_scene();
    let computed = handler.computed_scene();

    // 找到 Popover 内的按钮
    let button_region = computed
        .overlay_hit_regions
        .iter()
        .find(|region| matches!(region.interaction, HitInteraction::Widget { .. }))
        .expect("Should have button hit region in popover");

    // 点击 Popover 内的按钮
    let click_point = Point::new(
        button_region.rect.x + dp(10.0),
        button_region.rect.y + dp(10.0),
    );

    let viewport = handler.viewport_rect();
    handler.cursor_position = Some(click_point);
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);

    // Popover 不应该被关闭
    assert_eq!(
        close_calls.lock().unwrap().as_slice(),
        &[],
        "Popover should not close when clicking inside it"
    );
}
