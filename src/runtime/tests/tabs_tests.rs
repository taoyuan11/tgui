use super::*;

use std::sync::{Arc, Mutex};

use crate::foundation::binding::State;
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

/// 返回携带激活指示边框（stroke_width > 0）的 tab key。
///
/// 激活 tab 通过 `indicator_thickness` 描边区别于其它 tab。tab bar 容器自身也带描边，
/// 但它会覆盖所有 tab；因此「恰好只包含一个 tab 中心点」的描边矩形才是激活指示器。
fn active_tab_key(handler: &mut BoundRuntimeHandler<TestVm>) -> Option<String> {
    let scene = handler.computed_scene().clone();
    let triggers: Vec<(String, Point)> = scene
        .hit_regions
        .iter()
        .filter_map(|region| match &region.interaction {
            HitInteraction::TabTrigger { key, .. } => {
                let rect = region.rect;
                Some((
                    key.clone(),
                    Point::new(rect.x + rect.width * 0.5, rect.y + rect.height * 0.5),
                ))
            }
            _ => None,
        })
        .collect();
    scene
        .scene
        .shapes
        .iter()
        .filter(|shape| shape.stroke_width > 0.0)
        .find_map(|shape| {
            let mut contained = triggers
                .iter()
                .filter(|(_, center)| point_in_rect(*center, shape.rect));
            let first = contained.next()?;
            // 只接受恰好包含一个 tab 中心的描边（排除覆盖整条 tab bar 的容器边框）。
            match contained.next() {
                Some(_) => None,
                None => Some(first.0.clone()),
            }
        })
}

fn point_in_rect(point: Point, rect: Rect) -> bool {
    point.x >= rect.x
        && point.x <= rect.x + rect.width
        && point.y >= rect.y
        && point.y <= rect.y + rect.height
}

#[test]
fn changing_selected_state_moves_active_tab_indicator() {
    let invalidation = InvalidationSignal::new();
    let selected = State::new("one".to_string(), invalidation.clone());
    let tree = WidgetTree::new(Tabs::new(
        vec![
            TabItem::new("one", "One", Text::new("Panel one")),
            TabItem::new("two", "Two", Text::new("Panel two")),
            TabItem::new("three", "Three", Text::new("Panel three")),
        ],
        selected.signal(),
    ));
    let mut handler = test_handler(Some(tree), invalidation);

    assert_eq!(active_tab_key(&mut handler).as_deref(), Some("one"));

    selected.set("three".to_string());
    handler.request_redraw_if_dirty(Instant::now());

    assert_eq!(active_tab_key(&mut handler).as_deref(), Some("three"));
}
