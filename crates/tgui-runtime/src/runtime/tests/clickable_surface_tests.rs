use super::*;

use crate::ui::widget::{Avatar, Card};

fn widget_center(handler: &mut BoundRuntimeHandler<TestVm>, id: WidgetId) -> Point {
    handler
        .computed_scene()
        .hit_regions
        .iter()
        .find(|region| region.interaction.target_id() == crate::ui::widget::HitTargetId::Widget(id))
        .map(|region| {
            Point::new(
                region.rect.x + region.rect.width * 0.5,
                region.rect.y + region.rect.height * 0.5,
            )
        })
        .expect("clickable surface should expose a widget hit region")
}

#[test]
fn clickable_avatar_and_card_dispatch_pointer_enter_and_space_activation() {
    let invalidation = InvalidationSignal::new();
    let activations = Arc::new(Mutex::new(Vec::new()));

    let avatar_activations = Arc::clone(&activations);
    let avatar: Element<TestVm> = Avatar::initials("TG")
        .on_click(Command::new(move |_: &mut TestVm| {
            avatar_activations.lock().unwrap().push("avatar");
        }))
        .size(dp(64.0), dp(64.0))
        .into();
    let avatar_id = avatar.id;

    let card_activations = Arc::clone(&activations);
    let card: Element<TestVm> = Card::new()
        .body(Text::new("Open details"))
        .on_click(Command::new(move |_: &mut TestVm| {
            card_activations.lock().unwrap().push("card");
        }))
        .size(dp(180.0), dp(64.0))
        .into();
    let card_id = card.id;

    let tree = WidgetTree::new(Flex::new(Axis::Vertical).child([avatar, card]));
    let mut handler = test_handler(Some(tree), invalidation);

    assert!(handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Tab))));
    assert_eq!(handler.focused_widget_id(), Some(avatar_id));
    assert!(handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Enter))));

    assert!(handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Tab))));
    assert_eq!(handler.focused_widget_id(), Some(card_id));
    assert!(handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Space))));

    let avatar_center = widget_center(&mut handler, avatar_id);
    handler.cursor_position = Some(avatar_center);
    let viewport = handler.viewport_rect();
    let _ = handler.handle_hover(viewport);
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);

    assert_eq!(
        activations.lock().unwrap().as_slice(),
        ["avatar", "card", "avatar"]
    );
}
