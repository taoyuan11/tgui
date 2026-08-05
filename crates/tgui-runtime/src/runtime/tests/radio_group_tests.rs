use super::*;

use crate::foundation::binding::State;
use crate::ui::widget::{RadioGroup, RadioOption};

#[derive(Default)]
struct RadioGroupRuntimeVm {
    changes: Vec<String>,
}

impl crate::foundation::view_model::ViewModel for RadioGroupRuntimeVm {
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

fn radio_targets(handler: &mut BoundRuntimeHandler<RadioGroupRuntimeVm>) -> Vec<(usize, WidgetId)> {
    let scene = handler.computed_scene().clone();
    let mut targets = scene
        .hit_regions
        .iter()
        .filter_map(|region| match &region.interaction {
            HitInteraction::Radio {
                id, interactions, ..
            } => interactions.radio_group.map(|group| (group.index, *id)),
            _ => None,
        })
        .collect::<Vec<_>>();
    targets.sort_by_key(|(index, _)| *index);
    targets
}

fn radio_group_tree(
    selected: State<String>,
    direction: Axis,
) -> (WidgetTree<RadioGroupRuntimeVm>, WidgetId) {
    let selected_for_change = selected.clone();
    let outside: Element<RadioGroupRuntimeVm> = Button::new("Outside").into();
    let outside_id = outside.id;
    let group = RadioGroup::new(
        vec![
            RadioOption::new("email".to_string(), "Email".to_string()),
            RadioOption::new("sms".to_string(), "SMS".to_string()).disable(true),
            RadioOption::new("push".to_string(), "Push".to_string()),
        ],
        selected.signal(),
    )
    .direction(direction)
    .on_change(ValueCommand::new(
        move |vm: &mut RadioGroupRuntimeVm, (key, _value): (String, String)| {
            selected_for_change.set(key.clone());
            vm.changes.push(key);
        },
    ));
    (
        WidgetTree::new(Flex::vertical().child(group).child(outside)),
        outside_id,
    )
}

#[test]
fn horizontal_radio_group_arrows_wrap_skip_disabled_and_select_immediately() {
    let invalidation = InvalidationSignal::new();
    let selected = State::new("email".to_string(), invalidation.clone());
    let (tree, outside_id) = radio_group_tree(selected, Axis::Horizontal);
    let mut handler =
        test_handler_with_vm(RadioGroupRuntimeVm::default(), Some(tree), invalidation);
    let targets = radio_targets(&mut handler);
    assert_eq!(targets.len(), 2, "disabled option must not be a target");
    assert_eq!(targets[0].0, 0);
    assert_eq!(targets[1].0, 2);

    assert!(handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Tab))));
    assert_eq!(handler.focused_widget_id(), Some(targets[0].1));

    assert!(
        handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::ArrowRight)))
    );
    assert_eq!(handler.focused_widget_id(), Some(targets[1].1));
    assert_eq!(
        handler.view_model.lock().unwrap().changes.as_slice(),
        ["push"]
    );

    assert!(
        handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::ArrowRight)))
    );
    assert_eq!(handler.focused_widget_id(), Some(targets[0].1));
    assert_eq!(
        handler.view_model.lock().unwrap().changes.as_slice(),
        ["push", "email"]
    );

    assert!(
        handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::ArrowLeft)))
    );
    assert_eq!(handler.focused_widget_id(), Some(targets[1].1));
    assert_eq!(
        handler.view_model.lock().unwrap().changes.as_slice(),
        ["push", "email", "push"]
    );

    assert!(handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Tab))));
    assert_eq!(handler.focused_widget_id(), Some(outside_id));
}

#[test]
fn vertical_radio_group_only_uses_vertical_arrows() {
    let invalidation = InvalidationSignal::new();
    let selected = State::new("email".to_string(), invalidation.clone());
    let (tree, _) = radio_group_tree(selected, Axis::Vertical);
    let mut handler =
        test_handler_with_vm(RadioGroupRuntimeVm::default(), Some(tree), invalidation);
    let targets = radio_targets(&mut handler);

    assert!(handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Tab))));
    assert_eq!(handler.focused_widget_id(), Some(targets[0].1));
    assert!(
        !handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::ArrowRight)))
    );
    assert_eq!(handler.focused_widget_id(), Some(targets[0].1));
    assert!(handler.view_model.lock().unwrap().changes.is_empty());

    assert!(
        handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::ArrowDown)))
    );
    assert_eq!(handler.focused_widget_id(), Some(targets[1].1));
    assert_eq!(
        handler.view_model.lock().unwrap().changes.as_slice(),
        ["push"]
    );
}
