use super::*;

use std::sync::{Arc, Mutex};
use std::time::Instant;

use crate::ui::widget::{Button, Popover, PopoverTriggerMode};

#[derive(Default)]
struct PopoverVm {
    _open_changes: Vec<bool>,
    _checked: bool,
    _input: Option<TextController>,
}

impl crate::foundation::view_model::ViewModel for PopoverVm {
    fn new(context: &ViewModelContext) -> Self {
        Self {
            _open_changes: Vec::new(),
            _checked: false,
            _input: Some(context.text_controller("hello")),
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
