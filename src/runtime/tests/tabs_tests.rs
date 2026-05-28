use super::*;

use std::sync::{Arc, Mutex};

use crate::ui::widget::{TabItem, Tabs};

fn tabs_tree(log: Arc<Mutex<Vec<String>>>) -> WidgetTree<TestVm> {
    let log_for_cmd = log;
    WidgetTree::new(
        Tabs::new(
            vec![
                TabItem::new("one", "One", Text::new("Panel one")),
                TabItem::new("two", "Two", Text::new("Panel two")).disabled(true),
                TabItem::new("three", "Three", Text::new("Panel three")),
            ],
            "one".to_string(),
        )
        .on_change(ValueCommand::new(move |_: &mut TestVm, (key, _label)| {
            log_for_cmd.lock().unwrap().push(key);
        })),
    )
}

fn tab_center(handler: &mut BoundRuntimeHandler<TestVm>, key: &str) -> Point {
    let scene = handler.computed_scene().clone();
    let rect = scene
        .hit_regions
        .iter()
        .find_map(|region| match &region.interaction {
            HitInteraction::TabTrigger { key: candidate, .. } if candidate == key => {
                Some(region.rect)
            }
            _ => None,
        })
        .expect("tab trigger should exist");
    Point::new(rect.x + rect.width * 0.5, rect.y + rect.height * 0.5)
}

#[test]
fn clicking_tab_trigger_dispatches_on_change() {
    let invalidation = InvalidationSignal::new();
    let log = Arc::new(Mutex::new(Vec::new()));
    let tree = tabs_tree(log.clone());
    let mut handler = test_handler(Some(tree), invalidation);
    let viewport = handler.viewport_rect();
    let _ = handler.computed_scene();

    handler.cursor_position = Some(tab_center(&mut handler, "three"));
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);

    assert_eq!(log.lock().unwrap().as_slice(), ["three"]);
}

#[test]
fn arrow_keys_skip_disabled_tabs_and_enter_activates_focused_tab() {
    let invalidation = InvalidationSignal::new();
    let log = Arc::new(Mutex::new(Vec::new()));
    let tree = tabs_tree(log.clone());
    let mut handler = test_handler(Some(tree), invalidation);

    assert!(handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Tab))));
    assert!(
        handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::ArrowRight,)))
    );
    assert!(handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Enter))));

    assert_eq!(log.lock().unwrap().as_slice(), ["three"]);
}

#[test]
fn home_and_end_move_between_tab_edges() {
    let invalidation = InvalidationSignal::new();
    let log = Arc::new(Mutex::new(Vec::new()));
    let tree = tabs_tree(log.clone());
    let mut handler = test_handler(Some(tree), invalidation);

    assert!(handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Tab))));
    assert!(handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::End))));
    assert!(handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Space))));
    assert!(handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Home))));
    assert!(handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Space))));

    assert_eq!(log.lock().unwrap().as_slice(), ["three", "one"]);
}
