use super::*;

use crate::ui::widget::{
    List, ListItem, ListItemAction, ListSelectionChange, ListSelectionMode, MenuItem, WidgetKey,
};

fn list_row_center(
    handler: &mut BoundRuntimeHandler<TestVm>,
    key: impl Into<WidgetKey>,
) -> (WidgetId, Point) {
    let key = key.into();
    let computed = handler.computed_scene();
    computed
        .hit_regions
        .iter()
        .find_map(|region| match &region.interaction {
            HitInteraction::ListItem { id, state, .. } if state.key == key => Some((
                *id,
                Point::new(
                    region.rect.x + region.rect.width * 0.5,
                    region.rect.y + region.rect.height * 0.5,
                ),
            )),
            _ => None,
        })
        .expect("requested list row should be visible")
}

fn primary_shortcut_modifier() -> ModifiersState {
    #[cfg(target_os = "macos")]
    {
        ModifiersState::META
    }

    #[cfg(not(target_os = "macos"))]
    {
        ModifiersState::CONTROL
    }
}

#[test]
fn list_single_selection_click_dispatches_change() {
    let invalidation = InvalidationSignal::new();
    let latest = Arc::new(Mutex::new(None::<ListSelectionChange>));
    let latest_ref = Arc::clone(&latest);
    let tree = WidgetTree::new(
        List::<&'static str, TestVm>::new(
            vec![ListItem::keyed("a", "Alpha"), ListItem::keyed("b", "Beta")],
            |ctx| Text::new(ctx.item).into(),
        )
        .on_selection_change(ValueCommand::new(move |_vm: &mut TestVm, change| {
            *latest_ref.lock().expect("selection lock should succeed") = Some(change);
        }))
        .size(dp(240.0), dp(96.0)),
    );
    let mut handler = test_handler(Some(tree), invalidation);
    let viewport = handler.viewport_rect();
    let (row_id, point) = list_row_center(&mut handler, "b");

    handler.cursor_position = Some(point);
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);

    let change = latest
        .lock()
        .expect("selection lock should succeed")
        .clone()
        .expect("selection change should be dispatched");
    assert_eq!(handler.focused_widget_id(), Some(row_id));
    assert_eq!(change.selected_keys, vec![WidgetKey::from("b")]);
    assert_eq!(change.focused_key, Some(WidgetKey::from("b")));
    assert_eq!(change.anchor_key, Some(WidgetKey::from("b")));
}

#[test]
fn list_enter_dispatches_item_action_for_focused_row() {
    let invalidation = InvalidationSignal::new();
    let latest = Arc::new(Mutex::new(None::<ListItemAction>));
    let latest_ref = Arc::clone(&latest);
    let tree = WidgetTree::new(
        List::<&'static str, TestVm>::new(
            vec![ListItem::keyed("a", "Alpha"), ListItem::keyed("b", "Beta")],
            |ctx| Text::new(ctx.item).into(),
        )
        .on_item_action(ValueCommand::new(move |_vm: &mut TestVm, action| {
            *latest_ref.lock().expect("action lock should succeed") = Some(action);
        }))
        .size(dp(240.0), dp(96.0)),
    );
    let mut handler = test_handler(Some(tree), invalidation);
    let viewport = handler.viewport_rect();
    let (_, point) = list_row_center(&mut handler, "b");

    handler.cursor_position = Some(point);
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);
    assert!(handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Enter))));

    assert_eq!(
        *latest.lock().expect("action lock should succeed"),
        Some(ListItemAction {
            index: 1,
            key: WidgetKey::from("b"),
        })
    );
}

#[test]
fn list_multiple_selection_supports_toggle_and_shift_range() {
    let invalidation = InvalidationSignal::new();
    let selected = Arc::new(Mutex::new(Vec::<WidgetKey>::new()));
    let selected_for_signal = Arc::clone(&selected);
    let selected_for_cmd = Arc::clone(&selected);
    let signal_invalidation = invalidation.clone();
    let selected_signal = crate::foundation::binding::Signal::new(
        move || {
            selected_for_signal
                .lock()
                .expect("selected lock should succeed")
                .clone()
        },
        signal_invalidation,
    );
    let tree = WidgetTree::new(
        List::<&'static str, TestVm>::new(
            vec![
                ListItem::keyed("a", "Alpha"),
                ListItem::keyed("b", "Beta"),
                ListItem::keyed("c", "Gamma"),
                ListItem::keyed("d", "Delta"),
            ],
            |ctx| Text::new(ctx.item).into(),
        )
        .selection_mode(ListSelectionMode::Multiple)
        .selected_keys(selected_signal)
        .on_selection_change(ValueCommand::new(
            move |_vm: &mut TestVm, change: ListSelectionChange| {
                *selected_for_cmd
                    .lock()
                    .expect("selected lock should succeed") = change.selected_keys;
            },
        ))
        .size(dp(240.0), dp(180.0)),
    );
    let mut handler = test_handler(Some(tree), invalidation);
    let viewport = handler.viewport_rect();
    let (_, row_a) = list_row_center(&mut handler, "a");
    let (_, row_c) = list_row_center(&mut handler, "c");

    handler.cursor_position = Some(row_a);
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);
    handler.invalidate_computed_scene();
    assert_eq!(
        *selected.lock().expect("selected lock should succeed"),
        vec![WidgetKey::from("a")]
    );

    handler.modifiers = primary_shortcut_modifier();
    handler.cursor_position = Some(row_c);
    handler.handle_mouse_press(
        viewport,
        Instant::now() + Duration::from_millis(700),
        CanvasMouseButton::Left,
    );
    handler.modifiers = ModifiersState::empty();
    handler.invalidate_computed_scene();
    assert_eq!(
        *selected.lock().expect("selected lock should succeed"),
        vec![WidgetKey::from("a"), WidgetKey::from("c")]
    );

    handler.modifiers = ModifiersState::SHIFT;
    assert!(handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::ArrowUp))));
    handler.modifiers = ModifiersState::empty();

    assert_eq!(
        *selected.lock().expect("selected lock should succeed"),
        vec![
            WidgetKey::from("a"),
            WidgetKey::from("b"),
            WidgetKey::from("c"),
        ]
    );
}

#[test]
fn list_keyboard_navigation_skips_disabled_rows_from_root_focus() {
    let invalidation = InvalidationSignal::new();
    let latest = Arc::new(Mutex::new(None::<ListSelectionChange>));
    let latest_ref = Arc::clone(&latest);
    let list: Element<TestVm> = List::<&'static str, TestVm>::new(
        vec![
            ListItem::keyed("a", "Alpha").disable(true),
            ListItem::keyed("b", "Beta"),
        ],
        |ctx| Text::new(ctx.item).into(),
    )
    .focusable(true)
    .on_selection_change(ValueCommand::new(move |_vm: &mut TestVm, change| {
        *latest_ref.lock().expect("selection lock should succeed") = Some(change);
    }))
    .size(dp(240.0), dp(96.0))
    .into();
    let list_id = list.id;
    let mut handler = test_handler(Some(WidgetTree::new(list)), invalidation);
    handler.focused_widget = Some(FocusedWidget {
        widget_id: list_id,
        scope_path: Vec::new(),
        on_blur: None,
    });

    assert!(
        handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::ArrowDown)))
    );

    let change = latest
        .lock()
        .expect("selection lock should succeed")
        .clone()
        .expect("keyboard selection should dispatch");
    assert_eq!(change.selected_keys, vec![WidgetKey::from("b")]);
    assert_eq!(
        handler.focused_widget_id().map(|_| change.focused_key),
        Some(Some(WidgetKey::from("b")))
    );
}

#[test]
fn list_disabled_row_does_not_select_or_fire_action() {
    let invalidation = InvalidationSignal::new();
    let selection_count = Arc::new(AtomicUsize::new(0));
    let action_count = Arc::new(AtomicUsize::new(0));
    let selection_ref = Arc::clone(&selection_count);
    let action_ref = Arc::clone(&action_count);
    let tree = WidgetTree::new(
        List::<&'static str, TestVm>::new(
            vec![ListItem::keyed("a", "Alpha").disable(true)],
            |ctx| Text::new(ctx.item).into(),
        )
        .on_selection_change(ValueCommand::new(move |_vm: &mut TestVm, _change| {
            selection_ref.fetch_add(1, Ordering::SeqCst);
        }))
        .on_item_action(ValueCommand::new(move |_vm: &mut TestVm, _action| {
            action_ref.fetch_add(1, Ordering::SeqCst);
        }))
        .size(dp(240.0), dp(48.0)),
    );
    let mut handler = test_handler(Some(tree), invalidation);
    let viewport = handler.viewport_rect();
    let (_, point) = list_row_center(&mut handler, "a");

    handler.cursor_position = Some(point);
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);
    let _ = handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Enter)));

    assert_eq!(selection_count.load(Ordering::SeqCst), 0);
    assert_eq!(action_count.load(Ordering::SeqCst), 0);
}

#[test]
fn list_row_context_menu_opens_on_right_click() {
    let invalidation = InvalidationSignal::new();
    let tree = WidgetTree::new(
        List::<&'static str, TestVm>::new(vec![ListItem::keyed("a", "Alpha")], |ctx| {
            Text::new(ctx.item).into()
        })
        .context_menu(vec![MenuItem::new("Rename"), MenuItem::new("Delete")])
        .size(dp(240.0), dp(48.0)),
    );
    let mut handler = test_handler(Some(tree), invalidation);
    let viewport = handler.viewport_rect();
    let (row_id, point) = list_row_center(&mut handler, "a");
    let has_context_menu = handler
        .cached_scene
        .as_ref()
        .and_then(|cached| cached.layout.as_ref())
        .and_then(|layout| layout.resolved_widget(row_id))
        .and_then(|resolved| resolved.context_menu.as_ref())
        .is_some();
    assert!(has_context_menu, "list row should attach context menu");

    handler.cursor_position = Some(point);
    let _ = handler.handle_hover(viewport);
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Right);
    assert!(
        handler.context_menu_anchor_states.contains_key(&row_id),
        "right click should open context menu for row {row_id:?}; anchors: {:?}",
        handler
            .context_menu_anchor_states
            .keys()
            .collect::<Vec<_>>()
    );
    handler.invalidate_computed_scene();

    let (labels, row_after) = {
        let computed = handler.computed_scene();
        let labels = computed
            .scene
            .overlay_texts
            .iter()
            .map(|text| text.content.as_ref().to_string())
            .collect::<Vec<_>>();
        let row_after = computed
            .hit_regions
            .iter()
            .find_map(|region| match &region.interaction {
                HitInteraction::ListItem { id, state, .. } if state.key == WidgetKey::from("a") => {
                    Some(*id)
                }
                _ => None,
            });
        (labels, row_after)
    };
    assert_eq!(
        row_after,
        Some(row_id),
        "row id should stay stable while context menu is open; anchor keys: {:?}",
        handler
            .context_menu_anchor_states
            .keys()
            .collect::<Vec<_>>()
    );
    assert!(
        labels.iter().any(|label| label == "Rename"),
        "expected Rename in overlay labels: {labels:?}"
    );
    assert!(
        labels.iter().any(|label| label == "Delete"),
        "expected Delete in overlay labels: {labels:?}"
    );
}

#[test]
fn list_inline_child_button_click_does_not_select_row() {
    let invalidation = InvalidationSignal::new();
    let selection_count = Arc::new(AtomicUsize::new(0));
    let button_count = Arc::new(AtomicUsize::new(0));
    let selection_ref = Arc::clone(&selection_count);
    let button_ref = Arc::clone(&button_count);
    let tree = WidgetTree::new(
        List::<&'static str, TestVm>::new(vec![ListItem::keyed("a", "Alpha")], move |_ctx| {
            Button::new("Inline")
                .size(dp(84.0), dp(32.0))
                .on_click(Command::new({
                    let button_ref = Arc::clone(&button_ref);
                    move |_vm: &mut TestVm| {
                        button_ref.fetch_add(1, Ordering::SeqCst);
                    }
                }))
                .into()
        })
        .on_selection_change(ValueCommand::new(move |_vm: &mut TestVm, _change| {
            selection_ref.fetch_add(1, Ordering::SeqCst);
        }))
        .size(dp(240.0), dp(48.0)),
    );
    let mut handler = test_handler(Some(tree), invalidation);
    let viewport = handler.viewport_rect();
    let button_center = handler
        .computed_scene()
        .hit_regions
        .iter()
        .find_map(|region| match &region.interaction {
            HitInteraction::Widget { interactions, .. } if interactions.on_click.is_some() => {
                Some(Point::new(
                    region.rect.x + region.rect.width * 0.5,
                    region.rect.y + region.rect.height * 0.5,
                ))
            }
            _ => None,
        })
        .expect("inline button should have a hit region");

    handler.cursor_position = Some(button_center);
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);

    assert_eq!(button_count.load(Ordering::SeqCst), 1);
    assert_eq!(selection_count.load(Ordering::SeqCst), 0);
}
