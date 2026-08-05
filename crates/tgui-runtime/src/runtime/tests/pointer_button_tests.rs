use super::*;

use std::sync::{Arc, Mutex};

use crate::foundation::view_model::ValueCommand;
use crate::ui::widget::{ContextMenu, MenuItem, Radio};

fn widget_center<VM: ViewModel>(handler: &mut BoundRuntimeHandler<VM>, id: WidgetId) -> Point {
    let computed = handler.computed_scene();
    computed
        .hit_regions
        .iter()
        .chain(computed.overlay_hit_regions.iter())
        .find(|region| region.interaction.target_id() == crate::ui::widget::HitTargetId::Widget(id))
        .map(|region| {
            Point::new(
                region.rect.x + region.rect.width * 0.5,
                region.rect.y + region.rect.height * 0.5,
            )
        })
        .expect("widget should expose a hit region")
}

fn right_press<VM: ViewModel>(handler: &mut BoundRuntimeHandler<VM>, id: WidgetId) {
    let viewport = handler.viewport_rect();
    let point = widget_center(handler, id);
    handler.cursor_position = Some(point);
    let _ = handler.handle_hover(viewport);
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Right);
}

#[test]
fn secondary_click_does_not_activate_standard_controls() {
    let invalidation = InvalidationSignal::new();
    let activations = Arc::new(Mutex::new(Vec::<&'static str>::new()));

    let button_activations = Arc::clone(&activations);
    let button: Element<TestVm> = Button::new("Button")
        .size(dp(160.0), dp(36.0))
        .on_click(Command::new(move |_: &mut TestVm| {
            button_activations.lock().unwrap().push("button");
        }))
        .into();
    let button_id = button.id;

    let checkbox_activations = Arc::clone(&activations);
    let checkbox: Element<TestVm> = Checkbox::new(false)
        .size(dp(160.0), dp(36.0))
        .on_change(ValueCommand::new(move |_: &mut TestVm, _| {
            checkbox_activations.lock().unwrap().push("checkbox");
        }))
        .into();
    let checkbox_id = checkbox.id;

    let radio_activations = Arc::clone(&activations);
    let radio: Element<TestVm> = Radio::new(false)
        .size(dp(160.0), dp(36.0))
        .on_change(ValueCommand::new(move |_: &mut TestVm, _| {
            radio_activations.lock().unwrap().push("radio");
        }))
        .into();
    let radio_id = radio.id;

    let switch_activations = Arc::clone(&activations);
    let switch: Element<TestVm> = Switch::new(false)
        .size(dp(160.0), dp(36.0))
        .on_change(ValueCommand::new(move |_: &mut TestVm, _| {
            switch_activations.lock().unwrap().push("switch");
        }))
        .into();
    let switch_id = switch.id;

    let slider_activations = Arc::clone(&activations);
    let slider: Element<TestVm> = Slider::new(0.0, 0.0, 1.0)
        .size(dp(160.0), dp(36.0))
        .on_change(ValueCommand::new(move |_: &mut TestVm, _| {
            slider_activations.lock().unwrap().push("slider");
        }))
        .into();
    let slider_id = slider.id;

    let select_activations = Arc::clone(&activations);
    let select: Element<TestVm> = Select::new(
        vec![SelectOption::new("value".to_string(), "Value".to_string())],
        None::<String>,
    )
    .size(dp(160.0), dp(36.0))
    .on_open_change(ValueCommand::new(move |_: &mut TestVm, _| {
        select_activations.lock().unwrap().push("select");
    }))
    .into();
    let select_id = select.id;

    let tree = WidgetTree::new(
        Flex::new(Axis::Vertical)
            .gap(dp(4.0))
            .child([button, checkbox, radio, switch, slider, select]),
    );
    let mut handler = test_handler_with_config(
        TestVm,
        Some(tree),
        invalidation,
        test_config_with_size(320.0, 320.0),
    );

    for id in [
        button_id,
        checkbox_id,
        radio_id,
        switch_id,
        slider_id,
        select_id,
    ] {
        right_press(&mut handler, id);
    }

    assert!(activations.lock().unwrap().is_empty());
    assert!(handler.active_slider_drag.is_none());
    assert_eq!(handler.resolved_select_open_state(select_id), Some(false));
}

#[test]
fn secondary_click_on_select_opens_context_menu_without_toggling_select() {
    let invalidation = InvalidationSignal::new();
    let select_changes = Arc::new(Mutex::new(Vec::<bool>::new()));
    let select_changes_for_command = Arc::clone(&select_changes);
    let select: Element<TestVm> = Select::new(
        vec![SelectOption::new("value".to_string(), "Value".to_string())],
        None::<String>,
    )
    .size(dp(180.0), dp(36.0))
    .on_open_change(ValueCommand::new(move |_: &mut TestVm, open| {
        select_changes_for_command.lock().unwrap().push(open);
    }))
    .into();
    let select_id = select.id;
    let tree = WidgetTree::new(ContextMenu::new(select).items(vec![MenuItem::new("Inspect")]));
    let mut handler = test_handler_with_config(
        TestVm,
        Some(tree),
        invalidation,
        test_config_with_size(320.0, 160.0),
    );

    right_press(&mut handler, select_id);

    assert_eq!(select_changes.lock().unwrap().as_slice(), &[]);
    assert_eq!(handler.resolved_select_open_state(select_id), Some(false));
    let computed = handler.computed_scene();
    assert!(computed
        .scene
        .overlay_texts
        .iter()
        .any(|text| text.content.as_ref() == "Inspect"));
}
