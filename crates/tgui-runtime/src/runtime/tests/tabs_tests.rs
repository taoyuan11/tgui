use super::*;

use std::sync::{Arc, Mutex};

use crate::foundation::binding::State;
use crate::platform::event::MouseButton;
use crate::ui::widget::{TabItem, Tabs, TabsReorderEvent};

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
            HitInteraction::TabTrigger { key: candidate, .. } if candidate == key => Some(
                region
                    .clip_rect
                    .and_then(|clip| region.rect.intersect(clip))
                    .unwrap_or(region.rect),
            ),
            _ => None,
        })
        .expect("tab trigger should exist");
    Point::new(rect.x + rect.width * 0.5, rect.y + rect.height * 0.5)
}

fn tab_id(handler: &mut BoundRuntimeHandler<TestVm>, key: &str) -> WidgetId {
    handler
        .computed_scene()
        .hit_regions
        .iter()
        .find_map(|region| match &region.interaction {
            HitInteraction::TabTrigger {
                id, key: candidate, ..
            } if candidate == key => Some(*id),
            _ => None,
        })
        .expect("tab trigger should exist")
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
fn scroll_tabs_keyboard_navigation_keeps_each_focused_trigger_visible() {
    let invalidation = InvalidationSignal::new();
    let tree = WidgetTree::new(
        Tabs::new(
            (0..8)
                .map(|index| {
                    TabItem::new(
                        format!("tab-{index}"),
                        format!("Tab {index}"),
                        Text::new(format!("Panel {index}")),
                    )
                })
                .collect(),
            "tab-0".to_string(),
        )
        .size(dp(180.0), dp(120.0))
        .style(|style, _| style.tab_min_width = dp(88.0)),
    );
    let mut handler = test_handler_with_config(
        TestVm,
        Some(tree),
        invalidation,
        test_config_with_size(220.0, 160.0),
    );

    assert!(handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Tab))));
    for expected_index in 1..7 {
        assert!(handler
            .handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::ArrowRight))));
        let focused_id = handler
            .focused_widget_id()
            .expect("a tab trigger should remain focused");
        let (key, rect, clip) = handler
            .computed_scene()
            .hit_regions
            .iter()
            .find_map(|region| match &region.interaction {
                HitInteraction::TabTrigger { id, key, .. } if *id == focused_id => {
                    Some((key.clone(), region.rect, region.clip_rect))
                }
                _ => None,
            })
            .expect("the focused tab trigger should remain materialized");
        assert_eq!(key, format!("tab-{expected_index}"));
        let clip = clip.expect("scrolling tab triggers should carry the strip clip");
        assert!(
            rect.x + dp(0.01) >= clip.x && rect.right() <= clip.right() + dp(0.01),
            "the focused trigger must be fully visible: rect={rect:?} clip={clip:?}"
        );
    }

    assert!(handler
        .scroll_states
        .values()
        .any(|offset| offset.x > Dp::ZERO));
}

#[test]
fn tab_navigation_enters_at_selected_enabled_tab_or_first_enabled_fallback() {
    let build = |selected: &str| {
        WidgetTree::new(Tabs::new(
            vec![
                TabItem::new("one", "One", Text::new("Panel one")),
                TabItem::new("two", "Two", Text::new("Panel two")).disabled(true),
                TabItem::new("three", "Three", Text::new("Panel three")),
            ],
            selected.to_string(),
        ))
    };

    let invalidation = InvalidationSignal::new();
    let mut selected_handler = test_handler(Some(build("three")), invalidation);
    let selected_id = tab_id(&mut selected_handler, "three");
    assert!(
        selected_handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Tab)))
    );
    assert_eq!(selected_handler.focused_widget_id(), Some(selected_id));

    let invalidation = InvalidationSignal::new();
    let mut fallback_handler = test_handler(Some(build("two")), invalidation);
    let fallback_id = tab_id(&mut fallback_handler, "one");
    assert!(
        fallback_handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Tab)))
    );
    assert_eq!(fallback_handler.focused_widget_id(), Some(fallback_id));
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
    active_tab_keys(handler).into_iter().next()
}

fn active_tab_keys(handler: &mut BoundRuntimeHandler<TestVm>) -> Vec<String> {
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
        .filter(|shape| shape.stroke_width > 0.0 && shape.color.a > 0)
        .filter_map(|shape| {
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
        .collect()
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

    #[cfg(feature = "bench-support")]
    crate::runtime::action_stats::reset();

    selected.set("three".to_string());
    let animation_start = Instant::now();
    handler.request_redraw_if_dirty(animation_start);
    // The selected signal updates the retained trigger structure and panel scene in
    // one invalidation batch. Both branches must be patched before this frame is read.
    let _ = handler.computed_scene();

    #[cfg(feature = "bench-support")]
    {
        let snapshot = crate::runtime::action_stats::snapshot();
        assert!(
            snapshot.iter().any(|(action, count)| {
                matches!(
                    *action,
                    "reactive_structure_slot_update" | "reactive_property_slot_write"
                ) && *count >= 1
            }),
            "tabs selected change should update every retained consumer: {snapshot:?}"
        );
    }

    let event_loop = TestEventLoop;
    handler.drive_animations(&event_loop, animation_start);
    let settled_at =
        animation_start + Duration::from_millis(handler.theme.motion.fast_ms.saturating_add(20));
    handler.drive_animations(&event_loop, settled_at);

    assert_eq!(active_tab_keys(&mut handler), ["three"]);
    handler.invalidate_computed_scene();
    assert_eq!(active_tab_keys(&mut handler), ["three"]);

    // A rapid reversal must settle on exactly one current indicator; the old
    // visual border never becomes a separate focus or hit target.
    selected.set("one".to_string());
    let reverse_start = settled_at + Duration::from_millis(1);
    handler.request_redraw_if_dirty(reverse_start);
    handler.invalidate_computed_scene();
    let _ = handler.computed_scene();
    handler.drive_animations(&event_loop, reverse_start);
    let reverse_mid =
        reverse_start + Duration::from_millis((handler.theme.motion.fast_ms / 3).max(20));
    handler.drive_animations(&event_loop, reverse_mid);

    selected.set("three".to_string());
    handler.request_redraw_if_dirty(reverse_mid);
    handler.invalidate_computed_scene();
    let _ = handler.computed_scene();
    handler.drive_animations(
        &event_loop,
        reverse_mid + Duration::from_millis(handler.theme.motion.fast_ms.saturating_add(20)),
    );
    assert_eq!(active_tab_keys(&mut handler), ["three"]);
}

#[test]
fn dragging_reorderable_tab_dispatches_reorder_event() {
    let invalidation = InvalidationSignal::new();
    let events = Arc::new(Mutex::new(Vec::<TabsReorderEvent>::new()));
    let events_for_cmd = events.clone();
    let tree = WidgetTree::new(
        Tabs::new(
            vec![
                TabItem::new("one", "One", Text::new("Panel one")),
                TabItem::new("two", "Two", Text::new("Panel two")),
                TabItem::new("three", "Three", Text::new("Panel three")),
            ],
            "one".to_string(),
        )
        .reorderable(true)
        .on_reorder(ValueCommand::new(move |_: &mut TestVm, event| {
            events_for_cmd.lock().unwrap().push(event);
        })),
    );
    let mut handler = test_handler(Some(tree), invalidation);
    let viewport = handler.viewport_rect();
    let from = tab_center(&mut handler, "one");
    let to = tab_center(&mut handler, "three");

    handler.cursor_position = Some(from);
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);
    handler.cursor_position = Some(to);
    let event = WindowEvent::PointerButton {
        device_id: None,
        position: PhysicalPosition::new(to.x.get() as f64, to.y.get() as f64),
        state: ElementState::Released,
        button: ButtonSource::Mouse(MouseButton::Left),
        primary: true,
    };
    let event_loop = TestEventLoop;
    let _ = handler.handle_bound_window_event(&event_loop, event);

    let recorded = events.lock().unwrap();
    assert_eq!(recorded.len(), 1);
    assert_eq!(recorded[0].from_index, 0);
    assert_eq!(recorded[0].to_index, 2);
    assert_eq!(recorded[0].key, "one");
    assert_eq!(recorded[0].target_key, "three");
}
