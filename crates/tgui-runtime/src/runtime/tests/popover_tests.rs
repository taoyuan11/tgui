use super::*;

use std::sync::{Arc, Mutex};
use std::time::Instant;

use crate::platform::event::MouseButton;
use crate::ui::widget::{Button, DatePicker, Input, Popover, PopoverTriggerMode};

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

fn press_popover_point(handler: &mut BoundRuntimeHandler<PopoverVm>, point: Point) {
    let viewport = handler.viewport_rect();
    handler.cursor_position = Some(point);
    let _ = handler.handle_hover(viewport);
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);
}

fn has_open_popover(handler: &mut BoundRuntimeHandler<PopoverVm>) -> bool {
    handler
        .computed_scene()
        .overlay_close_handlers
        .iter()
        .any(|handle| {
            handle.layer == crate::runtime::overlay::OverlayLayer::Popover
                && (handle.close_on_escape || handle.close_on_outside_click)
        })
}

#[test]
fn default_popover_opens_and_trigger_click_closes_without_reopening() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let tree = WidgetTree::new(
        Popover::new(Button::new("More").size(dp(90.0), dp(36.0)))
            .content(Text::new("Uncontrolled body")),
    );
    let mut handler = test_handler_with_vm(PopoverVm::new(&context), Some(tree), invalidation);

    assert!(!has_open_popover(&mut handler));
    press_popover_point(&mut handler, Point::new(dp(40.0), dp(18.0)));
    assert!(has_open_popover(&mut handler));
    assert!(popover_content_visible(&mut handler, "Uncontrolled body"));

    press_popover_point(&mut handler, Point::new(dp(40.0), dp(18.0)));
    assert!(
        !has_open_popover(&mut handler),
        "the trigger must count as part of the popover surface so its toggle can close it"
    );
}

#[test]
fn popover_toggle_finds_composite_trigger_from_child_hit() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let trigger = Flex::horizontal().child(Button::new("Nested trigger").size(dp(140.0), dp(36.0)));
    let tree = WidgetTree::new(Popover::new(trigger).content(Text::new("Composite trigger body")));
    let mut handler = test_handler_with_vm(PopoverVm::new(&context), Some(tree), invalidation);

    press_popover_point(&mut handler, Point::new(dp(70.0), dp(18.0)));
    assert!(
        has_open_popover(&mut handler),
        "a child hit should toggle the popover attached to its trigger ancestor"
    );
}

#[test]
fn uncontrolled_popover_closes_on_outside_click() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let tree = WidgetTree::new(
        Popover::new(Button::new("More").size(dp(90.0), dp(36.0)))
            .content(Text::new("Outside close body")),
    );
    let mut handler = test_handler_with_vm(PopoverVm::new(&context), Some(tree), invalidation);

    press_popover_point(&mut handler, Point::new(dp(40.0), dp(18.0)));
    assert!(has_open_popover(&mut handler));
    press_popover_point(&mut handler, Point::new(dp(380.0), dp(260.0)));
    assert!(!has_open_popover(&mut handler));
}

#[test]
fn controlled_popover_trigger_requests_latest_signal_toggle() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let open = context.state(false);
    let open_for_command = open.clone();
    let tree = WidgetTree::new(
        Popover::new(Button::new("More").size(dp(90.0), dp(36.0)))
            .content(Text::new("Controlled body"))
            .open(open.signal())
            .on_open_change(ValueCommand::new(move |_vm: &mut PopoverVm, next| {
                open_for_command.set(next);
            })),
    );
    let mut handler = test_handler_with_vm(PopoverVm::new(&context), Some(tree), invalidation);

    press_popover_point(&mut handler, Point::new(dp(40.0), dp(18.0)));
    assert!(open.get());
    assert!(has_open_popover(&mut handler));

    press_popover_point(&mut handler, Point::new(dp(40.0), dp(18.0)));
    assert!(!open.get());
    assert!(!has_open_popover(&mut handler));
}

#[test]
fn combobox_click_with_open_callback_toggles_only_once() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let requests = Arc::new(Mutex::new(Vec::new()));
    let requests_for_command = Arc::clone(&requests);
    let tree = WidgetTree::new(
        Combobox::new(
            TextController::new_legacy(""),
            vec![ComboboxOption::new("one", "One")],
        )
        .on_open_change(ValueCommand::new(move |_vm: &mut PopoverVm, open| {
            requests_for_command.lock().unwrap().push(open);
        })),
    );
    let mut handler = test_handler_with_vm(PopoverVm::new(&context), Some(tree), invalidation);

    press_popover_point(&mut handler, Point::new(dp(20.0), dp(18.0)));

    assert!(has_open_popover(&mut handler));
    assert_eq!(*requests.lock().unwrap(), vec![true]);
}

#[test]
fn combobox_arrow_key_opens_closed_popup_and_focuses_an_option() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let tree = WidgetTree::new(Combobox::new(
        TextController::new_legacy(""),
        vec![
            ComboboxOption::new("one", "One"),
            ComboboxOption::new("two", "Two"),
        ],
    ));
    let mut handler = test_handler_with_vm(PopoverVm::new(&context), Some(tree), invalidation);

    assert!(handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Tab))));
    let input_id = handler
        .focused_widget_id()
        .expect("combobox input should focus");
    assert!(
        handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::ArrowDown,)))
    );

    assert!(has_open_popover(&mut handler));
    assert_ne!(handler.focused_widget_id(), Some(input_id));
}

#[test]
fn combobox_enter_activates_the_current_keyboard_option() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let controller = TextController::new_legacy("");
    let selections = Arc::new(Mutex::new(Vec::new()));
    let selections_for_command = Arc::clone(&selections);
    let tree = WidgetTree::new(
        Combobox::new(
            controller.clone(),
            vec![
                ComboboxOption::new("one", "One"),
                ComboboxOption::new("two", "Two"),
            ],
        )
        .on_change(ValueCommand::new(move |_vm: &mut PopoverVm, change| {
            selections_for_command.lock().unwrap().push(change);
        })),
    );
    let mut handler = test_handler_with_vm(PopoverVm::new(&context), Some(tree), invalidation);
    handler.reduced_motion = true;

    assert!(handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Tab))));
    assert!(
        handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::ArrowDown,)))
    );
    assert!(
        handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::ArrowDown,)))
    );
    assert!(handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Enter))));

    assert_eq!(controller.text(), "Two");
    let selections = selections.lock().unwrap();
    assert_eq!(selections.len(), 1);
    assert_eq!(selections[0].selected_key.as_deref(), Some("two"));
    assert_eq!(selections[0].text, "Two");
}

#[test]
fn long_combobox_keyboard_navigation_scrolls_beyond_the_first_visible_window() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let controller = TextController::new_legacy("");
    let tree = WidgetTree::new(
        Combobox::new(
            controller.clone(),
            (0..20)
                .map(|index| {
                    let option =
                        ComboboxOption::new(format!("item-{index}"), format!("Option {index}"));
                    if index == 4 {
                        option.disabled(true)
                    } else {
                        option
                    }
                })
                .collect::<Vec<_>>(),
        )
        .style(|style, _| {
            style.option_height = dp(32.0);
            style.max_visible_options = 3;
        }),
    );
    let mut handler = test_handler_with_config(
        PopoverVm::new(&context),
        Some(tree),
        invalidation,
        test_config_with_size(240.0, 180.0),
    );
    handler.reduced_motion = true;

    assert!(handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Tab))));
    let input_id = handler
        .focused_widget_id()
        .expect("combobox input should focus");
    for _ in 0..8 {
        assert!(handler
            .handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::ArrowDown))));
    }
    let list_id = handler
        .cached_scene
        .as_ref()
        .and_then(|cached| cached.layout.as_ref())
        .and_then(|layout| layout.resolved_widget(input_id))
        .and_then(|resolved| resolved.popover.as_ref())
        .and_then(|popover| popover.virtual_list_navigation.as_ref())
        .map(|navigation| navigation.list_id)
        .expect("combobox virtual list navigation metadata should be retained");
    assert!(handler
        .scroll_states
        .get(&list_id)
        .is_some_and(|offset| offset.y > Dp::ZERO));
    assert!(handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Enter))));

    assert_eq!(controller.text(), "Option 8");
}

#[test]
fn long_combobox_arrow_up_focuses_the_last_logical_option() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let controller = TextController::new_legacy("");
    let tree = WidgetTree::new(
        Combobox::new(
            controller.clone(),
            (0..20)
                .map(|index| {
                    ComboboxOption::new(format!("item-{index}"), format!("Option {index}"))
                })
                .collect::<Vec<_>>(),
        )
        .style(|style, _| {
            style.option_height = dp(32.0);
            style.max_visible_options = 3;
        }),
    );
    let mut handler = test_handler_with_config(
        PopoverVm::new(&context),
        Some(tree),
        invalidation,
        test_config_with_size(240.0, 180.0),
    );
    handler.reduced_motion = true;

    assert!(handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Tab))));
    assert!(handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::ArrowUp))));
    assert!(handler
        .scroll_states
        .values()
        .any(|offset| offset.y > Dp::ZERO));
    assert!(handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Enter))));

    assert_eq!(controller.text(), "Option 19");
}

#[test]
fn typing_in_uncontrolled_combobox_opens_its_filtered_popup() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let controller = TextController::new_legacy("");
    let tree = WidgetTree::new(Combobox::new(
        controller.clone(),
        vec![
            ComboboxOption::new("alpha", "Alpha"),
            ComboboxOption::new("beta", "Beta"),
        ],
    ));
    let mut handler = test_handler_with_vm(PopoverVm::new(&context), Some(tree), invalidation);

    assert!(handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Tab))));
    let input_id = handler
        .focused_widget_id()
        .expect("combobox input should focus");
    assert!(handler.handle_ime_event(&Ime::Commit("a".to_string())));
    flush_text_input_commits(&mut handler);

    assert_eq!(controller.text(), "a");
    let _ = handler.computed_scene();
    assert!(handler
        .cached_scene
        .as_ref()
        .and_then(|cached| cached.layout.as_ref())
        .and_then(|layout| layout.resolved_widget(input_id))
        .and_then(|resolved| resolved.popover.as_ref())
        .is_some_and(|popover| popover.is_open()));
    assert!(has_open_popover(&mut handler));
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
fn date_picker_escape_restores_focus_to_its_input_descendant() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let tree = WidgetTree::new(
        DatePicker::new(
            "2026-06-06",
            Some(chrono::NaiveDate::from_ymd_opt(2026, 6, 6).unwrap()),
            chrono::NaiveDate::from_ymd_opt(2026, 6, 1).unwrap(),
        )
        .open(true),
    );
    let mut handler = test_handler_with_vm(PopoverVm::new(&context), Some(tree), invalidation);
    handler.reduced_motion = true;

    assert!(handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Tab))));
    let input_id = handler
        .focused_text_input_id()
        .expect("DatePicker input should be the first focus target");
    assert!(handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Tab))));
    assert!(handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Tab))));
    assert_ne!(handler.focused_widget_id(), Some(input_id));

    assert!(handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Escape))));
    assert_eq!(handler.focused_widget_id(), Some(input_id));
    assert!(!has_open_popover(&mut handler));
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
        .map(|text| text.content.as_ref())
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
    handler.cursor_position = Some(popover_widget_center(&mut handler));
    let _ = handler.handle_hover(viewport);
    handler.invalidate_computed_scene();
    let hovered_panel = handler.computed_scene();
    let labels: Vec<_> = hovered_panel
        .scene
        .overlay_texts
        .iter()
        .map(|text| text.content.as_ref())
        .collect();
    assert!(labels.iter().any(|text| *text == "Inside Action"));

    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);
    assert!(
        handler.focused_widget_id().is_some(),
        "interactive content inside popover should remain hittable"
    );
}

#[test]
fn hover_preview_remains_visible_while_crossing_anchor_gap() {
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
    let popover_rect = handler
        .computed_scene()
        .overlay_close_handlers
        .iter()
        .find(|handle| handle.layer == crate::runtime::overlay::OverlayLayer::Popover)
        .map(|handle| handle.rect)
        .expect("popover overlay rect should exist");

    handler.cursor_position = Some(Point::new(
        popover_rect.x + dp(12.0),
        dp(36.0) + ((popover_rect.y - dp(36.0)) * 0.5),
    ));
    let _ = handler.handle_hover(viewport);
    handler.invalidate_computed_scene();
    assert!(
        popover_content_visible(&mut handler, "Inside Action"),
        "hover preview should remain visible while crossing the offset gap"
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
fn popover_survives_text_input_scene_patch_from_trigger_focus() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let vm = PopoverVm::new(&context);
    let input_controller = vm.input.clone().unwrap();

    let trigger = Flex::horizontal()
        .gap(dp(8.0))
        .child(Input::new(input_controller.clone()).width(dp(180.0)))
        .child(Button::new("Open").size(dp(64.0), dp(36.0)));
    let tree = WidgetTree::new(
        Popover::new(trigger)
            .content(Text::new("Patched Popover"))
            .open(true),
    );

    let mut handler = test_handler_with_vm(vm, Some(tree), invalidation);
    handler.invalidate_computed_scene();
    let input_rect = handler
        .computed_scene()
        .hit_regions
        .iter()
        .find_map(|region| match region.interaction {
            HitInteraction::TextInput { .. } => Some(region.rect),
            _ => None,
        })
        .expect("trigger input should be hittable");

    let viewport = handler.viewport_rect();
    handler.cursor_position = Some(Point::new(input_rect.x + dp(12.0), input_rect.y + dp(12.0)));
    let _ = handler.handle_hover(viewport);
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);
    assert!(
        handler.focused_text_input_id().is_some(),
        "trigger input should be focused"
    );
    assert!(
        popover_content_visible(&mut handler, "Patched Popover"),
        "popover should start visible"
    );

    handler.text_input_epoch = handler.text_input_epoch.wrapping_add(1);
    let cached_before = handler
        .cached_scene
        .as_ref()
        .map(|cache| cache.text_input_epoch)
        .expect("scene should be cached before text input patch");
    assert_ne!(cached_before, handler.text_input_epoch);

    assert!(
        popover_content_visible(&mut handler, "Patched Popover"),
        "text-input scene patch should preserve popover content"
    );
}

fn popover_content_visible(handler: &mut BoundRuntimeHandler<PopoverVm>, label: &str) -> bool {
    let computed = handler.computed_scene();
    computed
        .scene
        .overlay_texts
        .iter()
        .any(|text| text.content.as_ref() == label)
}

fn popover_widget_center(handler: &mut BoundRuntimeHandler<PopoverVm>) -> Point {
    let computed = handler.computed_scene();
    let region = computed
        .overlay_hit_regions
        .iter()
        .find(|region| matches!(region.interaction, HitInteraction::Widget { .. }))
        .expect("popover widget should be hittable");
    let rect = region
        .clip_rect
        .and_then(|clip| region.rect.intersect(clip))
        .unwrap_or(region.rect);
    Point::new(rect.x + rect.width * 0.5, rect.y + rect.height * 0.5)
}

#[test]
fn hover_preview_survives_clicking_interactive_content() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let click_count = Arc::new(Mutex::new(0usize));
    let click_count_cmd = click_count.clone();

    let tree = WidgetTree::new(
        Popover::<PopoverVm>::new(Button::new("Trigger").size(dp(90.0), dp(36.0)))
            .content(
                Button::new("Inside Action")
                    .size(dp(140.0), dp(36.0))
                    .on_click(Command::new(move |_vm: &mut PopoverVm| {
                        *click_count_cmd.lock().unwrap() += 1;
                    })),
            )
            .trigger_mode(PopoverTriggerMode::ClickAndHoverPreview),
    );
    let mut handler = test_handler_with_vm(PopoverVm::new(&context), Some(tree), invalidation);
    let viewport = handler.viewport_rect();

    // Hover the trigger to open the preview.
    handler.cursor_position = Some(Point::new(dp(40.0), dp(20.0)));
    let _ = handler.handle_hover(viewport);
    handler.invalidate_computed_scene();
    // Move into the content.
    handler.cursor_position = Some(popover_widget_center(&mut handler));
    let _ = handler.handle_hover(viewport);
    handler.invalidate_computed_scene();
    assert!(
        popover_content_visible(&mut handler, "Inside Action"),
        "hover preview should be visible after moving into content"
    );

    // Click the interactive element inside the content (executes a command -> invalidates scene).
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);
    assert_eq!(*click_count.lock().unwrap(), 1, "inner click should fire");

    // Recollect: the hover preview must remain visible.
    handler.invalidate_computed_scene();
    assert!(
        popover_content_visible(&mut handler, "Inside Action"),
        "hover preview should remain visible after clicking interactive content"
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
        &[] as &[bool],
        "Popover should not close when clicking inside it"
    );
}

#[test]
fn slider_drag_inside_popover_recollects_without_hiding_content() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let value = context.state(0.0);
    let value_for_command = value.clone();
    let tree = WidgetTree::new(
        Popover::new(Button::new("Trigger").size(dp(90.0), dp(36.0)))
            .content(
                Flex::vertical()
                    .gap(dp(8.0))
                    .child(Text::new("Channel"))
                    .child(
                        Slider::new(value.signal(), 0.0, 100.0)
                            .width(dp(180.0))
                            .on_change(ValueCommand::new(move |_vm: &mut PopoverVm, next| {
                                value_for_command.set(next);
                            })),
                    ),
            )
            .open(true),
    );

    let mut handler = test_handler_with_vm(PopoverVm::new(&context), Some(tree), invalidation);
    handler.invalidate_computed_scene();
    let track_rect = handler
        .computed_scene()
        .overlay_hit_regions
        .iter()
        .find_map(|region| match &region.interaction {
            HitInteraction::Slider { track_rect, .. } => Some(*track_rect),
            _ => None,
        })
        .expect("popover slider should be hittable");
    let viewport = handler.viewport_rect();
    let press_point = Point::new(
        track_rect.x + track_rect.width * 0.75,
        track_rect.y + track_rect.height * 0.5,
    );

    handler.cursor_position = Some(press_point);
    let _ = handler.handle_hover(viewport);
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);

    assert!(
        !handler
            .cached_scene
            .as_ref()
            .expect("cache shell should remain after fallback")
            .computed_valid,
        "overlay slider changes should force a computed-scene recollect when local patching cannot find the detached widget"
    );
    assert!(
        popover_content_visible(&mut handler, "Channel"),
        "popover content should remain visible after the press updates the slider"
    );

    let drag_point = Point::new(
        track_rect.x + track_rect.width * 0.35,
        track_rect.y + track_rect.height * 0.5,
    );
    let event_loop = TestEventLoop;
    handler.handle_bound_window_event(
        &event_loop,
        WindowEvent::PointerMoved {
            device_id: None,
            position: PhysicalPosition::new(drag_point.x.get() as f64, drag_point.y.get() as f64),
            primary: true,
            source: PointerSource::Mouse,
        },
    );

    assert!(
        value.get() > 0.0,
        "slider drag should dispatch value changes"
    );
    assert!(
        popover_content_visible(&mut handler, "Channel"),
        "popover content should remain visible after a pointer-move drag update"
    );

    handler.handle_bound_window_event(
        &event_loop,
        WindowEvent::PointerButton {
            device_id: None,
            position: PhysicalPosition::new(drag_point.x.get() as f64, drag_point.y.get() as f64),
            state: ElementState::Released,
            button: ButtonSource::Mouse(MouseButton::Left),
            primary: true,
        },
    );
}
