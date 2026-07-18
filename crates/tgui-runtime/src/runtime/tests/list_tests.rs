use super::*;

use crate::ui::layout::pct;
use crate::ui::widget::{
    ItemLayout, List, ListItem, ListItemAction, ListSection, ListSelectionChange,
    ListSelectionMode, MenuItem, ResolvedWidgetKind, ScrollRegion, VirtualList, WidgetKey,
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
        crate::platform::keyboard::meta_modifier()
    }

    #[cfg(not(target_os = "macos"))]
    {
        ModifiersState::CONTROL
    }
}

fn visible_list_item_centers(handler: &mut BoundRuntimeHandler<TestVm>) -> Vec<(WidgetKey, Point)> {
    let viewport = handler.viewport_rect();
    handler
        .computed_scene()
        .hit_regions
        .iter()
        .filter_map(|region| match &region.interaction {
            HitInteraction::ListItem { state, .. } => {
                let center = Point::new(
                    region.rect.x + region.rect.width * 0.5,
                    region.rect.y + region.rect.height * 0.5,
                );
                viewport
                    .contains(center)
                    .then(|| (state.key.clone(), center))
            }
            _ => None,
        })
        .collect()
}

fn first_vertical_scroll_region(handler: &mut BoundRuntimeHandler<TestVm>) -> ScrollRegion {
    handler
        .computed_scene()
        .scroll_regions
        .iter()
        .find(|region| region.max_offset().y > Dp::ZERO)
        .copied()
        .expect("scrollable list region should exist")
}

fn grouped_multi_select_list(selected: State<Vec<WidgetKey>>) -> Element<TestVm> {
    let selected_for_command = selected.clone();
    let sections = (0..3)
        .map(|section| {
            let prefix = match section {
                0 => "product",
                1 => "engineering",
                _ => "operations",
            };
            let items = (0..8)
                .map(|index| {
                    ListItem::keyed(format!("{prefix}-{index}"), format!("{prefix} row {index}"))
                })
                .collect::<Vec<_>>();
            ListSection::new(Text::new(format!("Section {section}")), items)
        })
        .collect::<Vec<_>>();

    List::sections(sections, |ctx| Text::new(ctx.item).width(pct(100.0)).into())
        .key("grouped-multi-select-list")
        .width(dp(260.0))
        .height(dp(140.0))
        .item_layout(ItemLayout::Measured {
            estimate: dp(42.0),
            spacing: dp(3.0),
            overscan: 2,
        })
        .selection_mode(ListSelectionMode::Multiple)
        .selected_keys(selected.signal())
        .on_selection_change(ValueCommand::new(
            move |_vm: &mut TestVm, change: ListSelectionChange| {
                selected_for_command.set(change.selected_keys);
            },
        ))
        .into()
}

#[test]
fn measured_virtual_list_rebuilds_after_initial_measure_feedback_without_scroll() {
    let invalidation = InvalidationSignal::new();
    let rows = (0..3).collect::<Vec<_>>();
    let list: Element<TestVm> = VirtualList::new(rows, |index, _item| {
        Stack::new()
            .height(dp(30.0))
            .child(Text::new(format!("Measured row {index}")))
            .into()
    })
    .item_layout(ItemLayout::Measured {
        estimate: dp(120.0),
        spacing: Dp::ZERO,
        overscan: 1,
    })
    .size(dp(200.0), dp(180.0))
    .into();
    let list_id = list.id;
    let tree = WidgetTree::new(list);
    let mut handler = test_handler_with_config(
        TestVm,
        Some(tree),
        invalidation,
        test_config_with_size(200.0, 180.0),
    );

    let first = handler
        .computed_scene()
        .scroll_regions
        .iter()
        .find(|region| region.id == list_id)
        .copied()
        .expect("measured virtual list should create a scroll region");
    assert_eq!(first.content_bounds.height, dp(180.0));
    assert!(
        handler
            .cached_scene
            .as_ref()
            .is_some_and(|cached| cached.layout_valid && cached.computed_valid),
        "initial measured feedback should rebuild before returning the scene"
    );
}

#[test]
fn virtual_builder_structure_patch_preserves_full_materialized_window() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let revision = context.state(0_u32);
    let revision_for_rows = revision.clone();
    let list: Element<TestVm> =
        VirtualList::new((0..10_usize).collect::<Vec<_>>(), move |index, _item| {
            let revision = revision_for_rows.get();
            Stack::<TestVm>::new()
                .height(dp(40.0))
                .on_click(Command::new(|_vm: &mut TestVm| {}))
                .child(Text::new(format!("row {index} revision {revision}")))
                .into()
        })
        .item_layout(ItemLayout::Fixed {
            item_extent: dp(40.0),
            spacing: Dp::ZERO,
            overscan: 0,
        })
        .size(dp(320.0), dp(720.0))
        .into();
    let list_id = list.id;
    let tree = WidgetTree::new(list);
    let mut config = test_config_with_size(320.0, 720.0);
    config.reduced_motion = true;
    let mut handler = test_handler_with_config(TestVm, Some(tree), invalidation, config);

    let assert_rows = |scene: &crate::ui::widget::ComputedScene<TestVm>, expected_revision| {
        let clickable_rows = scene
            .hit_regions
            .iter()
            .filter(|region| {
                matches!(
                    &region.interaction,
                    HitInteraction::Widget { interactions, .. }
                        if interactions.on_click.is_some()
                )
            })
            .count();
        assert_eq!(
            clickable_rows, 10,
            "all ten virtual rows must remain interactive"
        );
        assert_eq!(scene.scene.texts.len(), 10);
        assert_eq!(
            scene
                .scene
                .commands
                .iter()
                .filter(|command| { matches!(command, crate::ui::widget::RenderCommand::Text(_)) })
                .count(),
            10
        );
        for index in 0..10 {
            let expected = format!("row {index} revision {expected_revision}");
            assert!(
                scene
                    .scene
                    .texts
                    .iter()
                    .any(|text| text.content.as_ref() == expected.as_str()),
                "missing virtual row label {expected:?}"
            );
        }
    };

    assert_rows(handler.computed_scene(), 0);
    revision.set(1);
    handler.request_redraw_if_dirty(Instant::now());

    let cached = handler
        .cached_scene
        .as_ref()
        .expect("structure patch should preserve the cached scene shell");
    assert!(cached.layout_valid && cached.computed_valid);
    let children = cached
        .layout
        .as_ref()
        .and_then(|layout| layout.resolved_widget(list_id))
        .and_then(|resolved| match &resolved.kind {
            ResolvedWidgetKind::Virtual { children, .. } => Some(children.len()),
            _ => None,
        })
        .expect("cached root should remain a Virtual widget");
    assert_eq!(
        children, 10,
        "the retained layout must not collapse to bootstrap rows"
    );
    let retained = cached.computed.clone();
    assert_rows(&retained, 1);

    handler.invalidate_scene_with_reason("virtual_builder_full_recollect_control");
    let full = handler.computed_scene().clone();
    assert_rows(&full, 1);
    super::table_tests::assert_data_grid_scene_equivalent(&retained, &full);
}

#[test]
fn list_grouped_multi_select_click_keeps_scroll_offset_after_dynamic_rebuild() {
    let invalidation = InvalidationSignal::new();
    let selected = State::new(Vec::<WidgetKey>::new(), invalidation.clone());
    let page_revision = State::new(0_u32, invalidation.clone());
    let selected_for_page = selected.clone();
    let dynamic_page = page_revision.signal().map_unchecked(move |_| {
        grouped_multi_select_list(selected_for_page.clone()).key("dynamic-list-page")
    });
    let tree = WidgetTree::new_legacy(Stack::new().dynamic_child(dynamic_page));
    let mut handler = test_handler_with_config(
        TestVm,
        Some(tree),
        invalidation.clone(),
        test_config_with_size(320.0, 180.0),
    );

    let before_region = first_vertical_scroll_region(&mut handler);
    handler.set_scroll_offset(before_region.id, Point::new(Dp::ZERO, dp(260.0)));
    handler.invalidate_computed_scene();

    let before_region = first_vertical_scroll_region(&mut handler);
    let before_offset = before_region.scroll_offset;
    assert!(
        before_offset.y > dp(100.0),
        "test should start from a scrolled list; offset was {:?}",
        before_offset
    );
    let (clicked_key, click_point) = visible_list_item_centers(&mut handler)
        .into_iter()
        .find(|(key, _)| *key != WidgetKey::from("product-0"))
        .expect("a visible list item should be available after scrolling");

    handler.cursor_position = Some(click_point);
    handler.handle_mouse_press(
        handler.viewport_rect(),
        Instant::now(),
        CanvasMouseButton::Left,
    );
    handler.request_redraw_if_dirty(Instant::now());

    assert_eq!(selected.get(), vec![clicked_key]);
    let after_region = first_vertical_scroll_region(&mut handler);
    let after_offset = after_region.scroll_offset;
    assert_eq!(
        after_region.id, before_region.id,
        "keyed List should preserve its scroll region identity across selection rebuild"
    );
    assert!(
        after_offset.y > Dp::ZERO,
        "clicking a selected row should not reset the List scroll offset; before={:?}, after={:?}",
        before_offset,
        after_offset
    );
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
fn list_shift_range_uses_live_disabled_signal() {
    for (initially_disabled, disabled_before_range, expected) in [
        (
            false,
            true,
            vec![WidgetKey::from("a"), WidgetKey::from("c")],
        ),
        (
            true,
            false,
            vec![
                WidgetKey::from("a"),
                WidgetKey::from("b"),
                WidgetKey::from("c"),
            ],
        ),
    ] {
        let invalidation = InvalidationSignal::new();
        let disabled = Arc::new(Mutex::new(initially_disabled));
        let disabled_for_signal = Arc::clone(&disabled);
        let disabled_signal = Signal::new(
            move || {
                *disabled_for_signal
                    .lock()
                    .expect("disabled lock should succeed")
            },
            invalidation.clone(),
        );
        let selected = Arc::new(Mutex::new(Vec::<WidgetKey>::new()));
        let selected_for_signal = Arc::clone(&selected);
        let selected_for_command = Arc::clone(&selected);
        let selected_signal = Signal::new(
            move || {
                selected_for_signal
                    .lock()
                    .expect("selected lock should succeed")
                    .clone()
            },
            invalidation.clone(),
        );
        let tree = WidgetTree::new(
            List::<&'static str, TestVm>::new(
                vec![
                    ListItem::keyed("a", "Alpha"),
                    ListItem::keyed("b", "Beta").disable(disabled_signal),
                    ListItem::keyed("c", "Gamma"),
                ],
                |context| Text::new(context.item).into(),
            )
            .selection_mode(ListSelectionMode::Multiple)
            .selected_keys(selected_signal)
            .on_selection_change(ValueCommand::new(
                move |_vm: &mut TestVm, change: ListSelectionChange| {
                    *selected_for_command
                        .lock()
                        .expect("selected lock should succeed") = change.selected_keys;
                },
            ))
            .size(dp(240.0), dp(140.0)),
        );
        let mut handler = test_handler(Some(tree), invalidation.clone());
        let viewport = handler.viewport_rect();

        *disabled.lock().expect("disabled lock should succeed") = disabled_before_range;
        invalidation.mark_dirty();
        handler.request_redraw_if_dirty(Instant::now());

        let (_, row_a) = list_row_center(&mut handler, "a");
        handler.cursor_position = Some(row_a);
        handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);

        let (_, row_c) = list_row_center(&mut handler, "c");
        handler.modifiers = ModifiersState::SHIFT;
        handler.cursor_position = Some(row_c);
        handler.handle_mouse_press(
            viewport,
            Instant::now() + Duration::from_millis(700),
            CanvasMouseButton::Left,
        );
        handler.modifiers = ModifiersState::empty();

        assert_eq!(
            *selected.lock().expect("selected lock should succeed"),
            expected,
            "range selection must observe disabled={disabled_before_range} after construction"
        );
    }
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

#[test]
fn virtual_list_arrow_down_crosses_materialized_window_boundary() {
    let invalidation = InvalidationSignal::new();
    let items = (0..64)
        .map(|index| ListItem::keyed(format!("row-{index}"), index))
        .collect::<Vec<_>>();
    let tree = WidgetTree::new(
        List::<usize, TestVm>::new(items, |context| Text::new(context.item.to_string()).into())
            .item_layout(ItemLayout::Fixed {
                item_extent: dp(28.0),
                spacing: Dp::ZERO,
                overscan: 0,
            })
            .size(dp(220.0), dp(84.0)),
    );
    let mut config = test_config_with_size(220.0, 84.0);
    config.reduced_motion = true;
    let mut handler = test_handler_with_config(TestVm, Some(tree), invalidation, config);

    let (id, state, focus) = handler
        .computed_scene()
        .hit_regions
        .iter()
        .filter_map(|region| match &region.interaction {
            HitInteraction::ListItem { id, state, .. } => {
                Some((*id, state.clone(), region.focus.clone()?))
            }
            _ => None,
        })
        .max_by_key(|(_, state, _)| state.row_index)
        .expect("virtual List should materialize at least one row");
    let next_index = state.row_index + 1;
    assert!(next_index < 64, "test must stop before the source end");
    let next_key = WidgetKey::from(format!("row-{next_index}"));
    handler.focused_widget = Some(FocusedWidget {
        widget_id: id,
        scope_path: focus.scope_path,
        on_blur: focus.on_blur,
    });
    handler.list_focus_state = Some((state.list_id, state.key));
    handler.focus_visible = true;

    assert!(
        handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::ArrowDown))),
        "ArrowDown at the materialized boundary should schedule the next source row"
    );
    let _ = handler.computed_scene();
    assert_eq!(
        handler.list_focus_state.as_ref().map(|(_, key)| key),
        Some(&next_key),
        "keyboard focus should advance to the next source row after materialization"
    );

    let (id, state, focus) = handler
        .computed_scene()
        .hit_regions
        .iter()
        .filter_map(|region| match &region.interaction {
            HitInteraction::ListItem { id, state, .. } => {
                Some((*id, state.clone(), region.focus.clone()?))
            }
            _ => None,
        })
        .min_by_key(|(_, state, _)| state.row_index)
        .expect("scrolled List should materialize a first boundary row");
    let previous_key = state
        .item_index
        .checked_sub(1)
        .and_then(|index| state.selection.sibling_keys.get(index))
        .cloned()
        .expect("test must stop after the source start");
    handler.focused_widget = Some(FocusedWidget {
        widget_id: id,
        scope_path: focus.scope_path,
        on_blur: focus.on_blur,
    });
    handler.list_focus_state = Some((state.list_id, state.key));

    let arrow_up = pressed_key_event(PhysicalKey::Code(KeyCode::ArrowUp));
    assert!(handler.handle_keyboard_input(&arrow_up));
    let focused_id = handler.focused_widget_id();
    let focused_key = handler
        .computed_scene()
        .hit_regions
        .iter()
        .find_map(|region| match &region.interaction {
            HitInteraction::ListItem { id, state, .. } if Some(*id) == focused_id => {
                Some(state.key.clone())
            }
            _ => None,
        });
    assert_eq!(focused_key.as_ref(), Some(&previous_key));
}

#[test]
fn grouped_virtual_list_keyboard_crosses_section_materialized_boundary() {
    let invalidation = InvalidationSignal::new();
    let sections = vec![
        ListSection::new(
            Text::new("Section A"),
            (0..3)
                .map(|index| ListItem::keyed(format!("a-{index}"), index))
                .collect(),
        ),
        ListSection::new(
            Text::new("Section B"),
            (0..3)
                .map(|index| ListItem::keyed(format!("b-{index}"), index + 3))
                .collect(),
        ),
    ];
    let tree = WidgetTree::new(
        List::<usize, TestVm>::sections(sections, |context| {
            Text::new(context.item.to_string()).into()
        })
        .item_layout(ItemLayout::Fixed {
            item_extent: dp(28.0),
            spacing: Dp::ZERO,
            overscan: 0,
        })
        .size(dp(220.0), dp(56.0)),
    );
    let mut handler = test_handler_with_config(
        TestVm,
        Some(tree),
        invalidation,
        test_config_with_size(220.0, 56.0),
    );
    let list_id = first_vertical_scroll_region(&mut handler).id;
    handler.set_scroll_offset(list_id, Point::new(Dp::ZERO, dp(56.0)));

    let (id, state, focus) = handler
        .computed_scene()
        .hit_regions
        .iter()
        .find_map(|region| match &region.interaction {
            HitInteraction::ListItem { id, state, .. } if state.key == WidgetKey::from("a-2") => {
                Some((*id, state.clone(), region.focus.clone()?))
            }
            _ => None,
        })
        .expect("last item before the section header should be materialized");
    handler.focused_widget = Some(FocusedWidget {
        widget_id: id,
        scope_path: focus.scope_path,
        on_blur: focus.on_blur,
    });
    handler.list_focus_state = Some((state.list_id, state.key));

    let arrow_down = pressed_key_event(PhysicalKey::Code(KeyCode::ArrowDown));
    assert!(handler.handle_keyboard_input(&arrow_down));
    assert_eq!(
        handler.list_focus_state.as_ref().map(|(_, key)| key),
        Some(&WidgetKey::from("b-0"))
    );
    let arrow_up = pressed_key_event(PhysicalKey::Code(KeyCode::ArrowUp));
    assert!(handler.handle_keyboard_input(&arrow_up));
    assert_eq!(
        handler.list_focus_state.as_ref().map(|(_, key)| key),
        Some(&WidgetKey::from("a-2"))
    );
}

#[test]
fn measured_virtual_list_keyboard_crosses_materialized_boundary() {
    let invalidation = InvalidationSignal::new();
    let items = (0..64)
        .map(|index| ListItem::keyed(format!("row-{index}"), index))
        .collect::<Vec<_>>();
    let tree = WidgetTree::new(
        List::<usize, TestVm>::new(items, |context| {
            Stack::new()
                .height(if context.item % 2 == 0 {
                    dp(32.0)
                } else {
                    dp(52.0)
                })
                .child(Text::new(context.item.to_string()))
                .into()
        })
        .item_layout(ItemLayout::Measured {
            estimate: dp(28.0),
            spacing: dp(3.0),
            overscan: 0,
        })
        .size(dp(220.0), dp(96.0)),
    );
    let mut handler = test_handler_with_config(
        TestVm,
        Some(tree),
        invalidation,
        test_config_with_size(220.0, 96.0),
    );
    let (id, state, focus) = handler
        .computed_scene()
        .hit_regions
        .iter()
        .filter_map(|region| match &region.interaction {
            HitInteraction::ListItem { id, state, .. } => {
                Some((*id, state.clone(), region.focus.clone()?))
            }
            _ => None,
        })
        .max_by_key(|(_, state, _)| state.row_index)
        .expect("measured List should materialize a boundary row");
    let next_key = state
        .selection
        .sibling_keys
        .get(state.item_index + 1)
        .cloned()
        .expect("test must stop before the source end");
    handler.focused_widget = Some(FocusedWidget {
        widget_id: id,
        scope_path: focus.scope_path,
        on_blur: focus.on_blur,
    });
    handler.list_focus_state = Some((state.list_id, state.key));

    let arrow_down = pressed_key_event(PhysicalKey::Code(KeyCode::ArrowDown));
    assert!(handler.handle_keyboard_input(&arrow_down));
    let focused_id = handler.focused_widget_id();
    let focused_key = handler
        .computed_scene()
        .hit_regions
        .iter()
        .find_map(|region| match &region.interaction {
            HitInteraction::ListItem { id, state, .. } if Some(*id) == focused_id => {
                Some(state.key.clone())
            }
            _ => None,
        });
    assert_eq!(focused_key.as_ref(), Some(&next_key));
}

#[test]
fn virtual_list_home_end_and_page_use_full_logical_source() {
    let invalidation = InvalidationSignal::new();
    let latest_action = Arc::new(Mutex::new(None::<ListItemAction>));
    let latest_for_command = Arc::clone(&latest_action);
    let items = (0..100)
        .map(|index| ListItem::keyed(format!("row-{index}"), index))
        .collect::<Vec<_>>();
    let tree = WidgetTree::new(
        List::<usize, TestVm>::new(items, |context| Text::new(context.item.to_string()).into())
            .item_layout(ItemLayout::Fixed {
                item_extent: dp(28.0),
                spacing: Dp::ZERO,
                overscan: 0,
            })
            .on_item_action(ValueCommand::new(move |_vm, action| {
                *latest_for_command.lock().unwrap() = Some(action);
            }))
            .size(dp(220.0), dp(84.0)),
    );
    let mut handler = test_handler_with_config(
        TestVm,
        Some(tree),
        invalidation,
        test_config_with_size(220.0, 84.0),
    );
    let (id, state, focus) = handler
        .computed_scene()
        .hit_regions
        .iter()
        .filter_map(|region| match &region.interaction {
            HitInteraction::ListItem { id, state, .. } => {
                Some((*id, state.clone(), region.focus.clone()?))
            }
            _ => None,
        })
        .min_by_key(|(_, state, _)| state.row_index)
        .expect("virtual List should materialize a first row");
    let list_id = state.list_id;
    handler.focused_widget = Some(FocusedWidget {
        widget_id: id,
        scope_path: focus.scope_path,
        on_blur: focus.on_blur,
    });
    handler.list_focus_state = Some((list_id, state.key));

    assert!(handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::End))));
    assert_eq!(
        handler.list_focus_state.as_ref().map(|(_, key)| key),
        Some(&WidgetKey::from("row-99"))
    );
    let focused_id = handler
        .focused_widget_id()
        .expect("End should focus the newly materialized row");
    let cached = handler
        .cached_scene
        .as_ref()
        .expect("keyboard materialization should retain a scene");
    let accessibility = crate::accessibility::build_tree_update(
        cached.layout.as_ref(),
        &cached.computed,
        Some(focused_id),
        cached.viewport,
    );
    assert_eq!(
        accessibility.focus,
        crate::accessibility::node_id_from_widget(focused_id),
        "AccessKit focus must follow the real off-window row widget"
    );
    assert!(handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Enter))));
    assert_eq!(
        *latest_action.lock().unwrap(),
        Some(ListItemAction {
            index: 99,
            key: WidgetKey::from("row-99"),
        })
    );
    assert!(handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Home))));
    assert_eq!(
        handler.list_focus_state.as_ref().map(|(_, key)| key),
        Some(&WidgetKey::from("row-0"))
    );

    let (page, viewport_height) = handler
        .computed_scene()
        .scroll_regions
        .iter()
        .find(|region| region.id == list_id)
        .map(|region| {
            (
                ((region.content_viewport.height / dp(28.0)).ceil() as usize).max(1),
                region.content_viewport.height,
            )
        })
        .unwrap_or((1, dp(84.0)));
    assert!(
        handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::PageDown,)))
    );
    assert_eq!(
        handler.list_focus_state.as_ref().map(|(_, key)| key),
        Some(&WidgetKey::from(format!("row-{page}")))
    );
    let page_offset = handler
        .scroll_states
        .get(&list_id)
        .map(|offset| offset.y)
        .unwrap_or(Dp::ZERO);
    assert!(
        (page_offset - viewport_height).abs() <= 0.01,
        "PageDown should advance one viewport, got {page_offset:?}"
    );
    assert!(handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::PageUp,))));
    assert_eq!(
        handler.list_focus_state.as_ref().map(|(_, key)| key),
        Some(&WidgetKey::from("row-0"))
    );
}

#[test]
fn virtual_list_page_long_disabled_run_is_conservative() {
    let invalidation = InvalidationSignal::new();
    let items = (0..32)
        .map(|index| {
            let item = ListItem::keyed(format!("row-{index}"), index);
            if (1..=15).contains(&index) {
                item.disable(true)
            } else {
                item
            }
        })
        .collect::<Vec<_>>();
    let tree = WidgetTree::new(
        List::<usize, TestVm>::new(items, |context| Text::new(context.item.to_string()).into())
            .item_layout(ItemLayout::Fixed {
                item_extent: dp(28.0),
                spacing: Dp::ZERO,
                overscan: 0,
            })
            .size(dp(200.0), dp(56.0)),
    );
    let mut handler = test_handler_with_config(
        TestVm,
        Some(tree),
        invalidation,
        test_config_with_size(200.0, 56.0),
    );
    let (id, state, focus) = handler
        .computed_scene()
        .hit_regions
        .iter()
        .find_map(|region| match &region.interaction {
            HitInteraction::ListItem { id, state, .. } if state.key == WidgetKey::from("row-0") => {
                Some((*id, state.clone(), region.focus.clone()?))
            }
            _ => None,
        })
        .expect("first row should be materialized");
    handler.focused_widget = Some(FocusedWidget {
        widget_id: id,
        scope_path: focus.scope_path,
        on_blur: focus.on_blur,
    });
    handler.list_focus_state = Some((state.list_id, state.key));

    assert!(
        handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::PageDown,)))
    );
    assert_eq!(
        handler.list_focus_state.as_ref().map(|(_, key)| key),
        Some(&WidgetKey::from("row-0")),
        "a disabled run beyond the visible scan budget must not jump to a distant edge"
    );
    assert!(!handler.scroll_states.contains_key(&state.list_id));
}

#[test]
fn grouped_measured_virtual_list_page_uses_prefix_and_skips_headers() {
    let invalidation = InvalidationSignal::new();
    let sections = (0..2)
        .map(|section| {
            ListSection::new(
                Text::new(format!("Section {section}")),
                (0..8)
                    .map(|index| ListItem::keyed(format!("{section}-{index}"), section * 8 + index))
                    .collect(),
            )
        })
        .collect();
    let tree = WidgetTree::new(
        List::<usize, TestVm>::sections(sections, |context| {
            Stack::new()
                .height(if context.item % 2 == 0 {
                    dp(32.0)
                } else {
                    dp(52.0)
                })
                .child(Text::new(context.item.to_string()))
                .into()
        })
        .item_layout(ItemLayout::Measured {
            estimate: dp(28.0),
            spacing: dp(3.0),
            overscan: 0,
        })
        .size(dp(220.0), dp(96.0)),
    );
    let mut handler = test_handler_with_config(
        TestVm,
        Some(tree),
        invalidation,
        test_config_with_size(220.0, 96.0),
    );
    let (id, state, focus) = handler
        .computed_scene()
        .hit_regions
        .iter()
        .find_map(|region| match &region.interaction {
            HitInteraction::ListItem { id, state, .. } if state.key == WidgetKey::from("0-0") => {
                Some((*id, state.clone(), region.focus.clone()?))
            }
            _ => None,
        })
        .expect("first grouped item should be materialized");
    let list_id = state.list_id;
    let viewport_height = handler
        .computed_scene()
        .scroll_regions
        .iter()
        .find(|region| region.id == list_id)
        .map(|region| region.content_viewport.height)
        .expect("grouped measured List should scroll");
    let current_top = handler
        .virtual_states
        .get(&list_id)
        .map(|cache| {
            cache
                .item_main_bounds(state.row_index, state.item_extent, state.item_spacing)
                .0
        })
        .expect("measured prefix cache should exist");
    handler.focused_widget = Some(FocusedWidget {
        widget_id: id,
        scope_path: focus.scope_path,
        on_blur: focus.on_blur,
    });
    handler.list_focus_state = Some((list_id, state.key));

    assert!(
        handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::PageDown,)))
    );
    let focused_id = handler.focused_widget_id();
    let focused_state = handler
        .computed_scene()
        .hit_regions
        .iter()
        .find_map(|region| match &region.interaction {
            HitInteraction::ListItem { id, state, .. } if Some(*id) == focused_id => {
                Some(state.clone())
            }
            _ => None,
        })
        .expect("PageDown should focus a newly materialized item, never a section header");
    let focused_top = handler
        .virtual_states
        .get(&list_id)
        .map(|cache| {
            cache
                .item_main_bounds(
                    focused_state.row_index,
                    focused_state.item_extent,
                    focused_state.item_spacing,
                )
                .0
        })
        .expect("measured prefix cache should remain available");
    assert!(focused_state.item_index > state.item_index);
    assert!(focused_top + dp(0.01) >= current_top + viewport_height);
    let page_offset = handler.scroll_states[&list_id].y;
    assert!(
        page_offset >= viewport_height - dp(52.0)
            && page_offset <= viewport_height + dp(52.0),
        "measured feedback may correct by at most one row: viewport={viewport_height:?}, offset={page_offset:?}"
    );

    assert!(handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::End))));
    assert_eq!(
        handler.list_focus_state.as_ref().map(|(_, key)| key),
        Some(&WidgetKey::from("1-7"))
    );
    assert!(handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Home))));
    assert_eq!(
        handler.list_focus_state.as_ref().map(|(_, key)| key),
        Some(&WidgetKey::from("0-0"))
    );
}
