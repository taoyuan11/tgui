use super::*;

use crate::platform::event::MouseButton;
use crate::ui::widget::{
    MenuItem, Tree, TreeCheckChange, TreeDropEvent, TreeDropPosition, TreeExpandChange, TreeNode,
    TreeNodeAction, TreeSelectionChange, TreeSelectionMode, WidgetKey,
};

fn sample_nodes() -> Vec<TreeNode<&'static str>> {
    vec![
        TreeNode::keyed("root", "Root").children([
            TreeNode::keyed("child-a", "Child A").child(TreeNode::keyed("leaf", "Leaf")),
            TreeNode::keyed("child-b", "Child B").disable(true),
        ]),
        TreeNode::keyed("sibling", "Sibling"),
    ]
}

fn tree_row_rect(
    handler: &mut BoundRuntimeHandler<TestVm>,
    key: impl Into<WidgetKey>,
) -> (WidgetId, Rect) {
    let key = key.into();
    handler
        .computed_scene()
        .hit_regions
        .iter()
        .find_map(|region| match &region.interaction {
            HitInteraction::TreeNode { id, state, .. } if state.key == key => {
                Some((*id, region.rect))
            }
            _ => None,
        })
        .expect("requested tree row should be visible")
}

fn tree_row_center(
    handler: &mut BoundRuntimeHandler<TestVm>,
    key: impl Into<WidgetKey>,
) -> (WidgetId, Point) {
    let (id, rect) = tree_row_rect(handler, key);
    (
        id,
        Point::new(rect.x + rect.width * 0.5, rect.y + rect.height * 0.5),
    )
}

fn disclosure_point(handler: &mut BoundRuntimeHandler<TestVm>, key: impl Into<WidgetKey>) -> Point {
    let (_, rect) = tree_row_rect(handler, key);
    Point::new(rect.x + dp(19.0), rect.y + rect.height * 0.5)
}

fn visible_disclosure_center(
    handler: &mut BoundRuntimeHandler<TestVm>,
    key: impl Into<WidgetKey>,
) -> Point {
    let key = key.into();
    handler
        .computed_scene()
        .hit_regions
        .iter()
        .find_map(|region| match &region.interaction {
            HitInteraction::TreeNode { state, .. } if state.key == key => Some(Point::new(
                region.rect.x
                    + state.item_padding.left
                    + state.indent_width * state.depth as f32
                    + state.disclosure_width * 0.5,
                region.rect.y + region.rect.height * 0.5,
            )),
            _ => None,
        })
        .expect("requested tree row should be visible")
}

fn checkbox_point(handler: &mut BoundRuntimeHandler<TestVm>, key: impl Into<WidgetKey>) -> Point {
    let (_, rect) = tree_row_rect(handler, key);
    Point::new(rect.x + dp(42.0), rect.y + rect.height * 0.5)
}

fn pointer_press(handler: &mut BoundRuntimeHandler<TestVm>, point: Point, button: MouseButton) {
    let event_loop = TestEventLoop;
    handler.handle_bound_window_event(
        &event_loop,
        WindowEvent::PointerButton {
            device_id: None,
            position: PhysicalPosition::new(f64::from(point.x.get()), f64::from(point.y.get())),
            state: ElementState::Pressed,
            button: ButtonSource::Mouse(button),
            primary: true,
        },
    );
}

fn pointer_release(handler: &mut BoundRuntimeHandler<TestVm>, point: Point) {
    let event_loop = TestEventLoop;
    handler.handle_bound_window_event(
        &event_loop,
        WindowEvent::PointerButton {
            device_id: None,
            position: PhysicalPosition::new(f64::from(point.x.get()), f64::from(point.y.get())),
            state: ElementState::Released,
            button: ButtonSource::Mouse(MouseButton::Left),
            primary: true,
        },
    );
}

fn top_tree_hit_at(
    handler: &mut BoundRuntimeHandler<TestVm>,
    point: Point,
) -> Option<HitInteraction<TestVm>> {
    WidgetTree::hit_path_from_computed(handler.computed_scene(), point).pop()
}

fn visible_tree_keys(handler: &mut BoundRuntimeHandler<TestVm>) -> Vec<WidgetKey> {
    handler
        .computed_scene()
        .hit_regions
        .iter()
        .filter_map(|region| match &region.interaction {
            HitInteraction::TreeNode { state, .. } => Some(state.key.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn tree_selected_keys_signal_updates_cached_scene_and_matches_full_recollect() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let selected = context.state(vec![WidgetKey::from("a")]);
    let selected_color = Color::hexa(0xD14B70FF);
    let row_color = Color::hexa(0x102030FF);
    let tree = WidgetTree::new(
        Tree::<&'static str, TestVm>::new(
            vec![TreeNode::keyed("a", "Alpha"), TreeNode::keyed("b", "Beta")],
            |ctx| Text::new(format!("{} selected={}", ctx.item, ctx.selected)).into(),
        )
        .selected_keys(selected.signal())
        .selection_mode(TreeSelectionMode::Multiple)
        .item_layout(crate::ui::widget::ItemLayout::Fixed {
            item_extent: dp(32.0),
            spacing: Dp::ZERO,
            overscan: 0,
        })
        .style(move |style, _| {
            style.item_background = crate::ui::layout::Value::Static(row_color);
            style.item_hover_background = crate::ui::layout::Value::Static(row_color);
            style.item_selected_background = crate::ui::layout::Value::Static(selected_color);
        })
        .size(dp(220.0), dp(120.0)),
    );
    let mut config = test_config_with_size(220.0, 120.0);
    config.reduced_motion = true;
    let mut handler = test_handler_with_config(TestVm, Some(tree), invalidation, config);

    let selected_rects = |scene: &crate::ui::widget::ComputedScene<TestVm>| {
        scene
            .scene
            .shapes
            .iter()
            .filter_map(|shape| (shape.color == selected_color).then_some(shape.rect))
            .collect::<Vec<_>>()
    };
    let text_contents = |scene: &crate::ui::widget::ComputedScene<TestVm>| {
        scene
            .scene
            .texts
            .iter()
            .map(|text| text.content.to_string())
            .collect::<Vec<_>>()
    };

    let initial = handler.computed_scene().clone();
    assert_eq!(selected_rects(&initial).len(), 1);
    assert!(text_contents(&initial)
        .iter()
        .any(|text| text == "Alpha selected=true"));
    assert!(text_contents(&initial)
        .iter()
        .any(|text| text == "Beta selected=false"));

    selected.set(vec![WidgetKey::from("b")]);
    handler.request_redraw_if_dirty(Instant::now());

    let (retained, retained_cache_dependencies) = {
        let cached = handler
            .cached_scene
            .as_ref()
            .expect("Tree selection invalidation should preserve the cache shell");
        assert!(cached.layout_valid && cached.computed_valid);
        (
            cached.computed.clone(),
            (
                cached.dependencies.dependency_count(),
                cached.dependencies.has_global_dependency(),
                cached.dependencies.all_owners(),
            ),
        )
    };
    assert_eq!(selected_rects(&retained).len(), 1);
    assert_ne!(selected_rects(&retained), selected_rects(&initial));
    let retained_text = text_contents(&retained);
    assert!(retained_text
        .iter()
        .any(|text| text == "Alpha selected=false"));
    assert!(retained_text
        .iter()
        .any(|text| text == "Beta selected=true"));

    let retained_b = retained
        .hit_regions
        .iter()
        .find_map(|region| match &region.interaction {
            HitInteraction::TreeNode { id, state, .. } if state.key == WidgetKey::from("b") => {
                Some(*id)
            }
            _ => None,
        })
        .expect("selected Tree row b should expose a hit region");
    let retained_accessibility = handler.accessibility_tree_update_for_test();
    let retained_b_node_id = crate::accessibility::node_id_from_widget(retained_b);
    let retained_b_node = retained_accessibility
        .nodes
        .iter()
        .find_map(|(id, node)| (*id == retained_b_node_id).then_some(node))
        .expect("selected Tree row b should expose an accessibility node");
    assert_eq!(retained_b_node.is_selected(), Some(true));

    handler.invalidate_scene_with_reason("tree_selection_equivalence_full_recollect");
    let full = handler.computed_scene().clone();
    let full_cache_dependencies = {
        let cached = handler
            .cached_scene
            .as_ref()
            .expect("full recollect should repopulate the cache");
        (
            cached.dependencies.dependency_count(),
            cached.dependencies.has_global_dependency(),
            cached.dependencies.all_owners(),
        )
    };
    let full_accessibility = handler.accessibility_tree_update_for_test();

    super::table_tests::assert_data_grid_scene_equivalent(&retained, &full);
    assert_eq!(retained_accessibility, full_accessibility);
    assert_eq!(retained_cache_dependencies, full_cache_dependencies);
    assert_eq!(
        (
            retained.dependencies.dependency_count(),
            retained.dependencies.has_global_dependency(),
            retained.dependencies.all_owners(),
        ),
        (
            full.dependencies.dependency_count(),
            full.dependencies.has_global_dependency(),
            full.dependencies.all_owners(),
        )
    );
}

#[test]
fn tree_checked_keys_signal_updates_cached_scene_and_matches_full_recollect() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let checked = context.state(vec![WidgetKey::from("a")]);
    let tree = WidgetTree::new(
        Tree::<&'static str, TestVm>::new(
            vec![TreeNode::keyed("a", "Alpha"), TreeNode::keyed("b", "Beta")],
            |ctx| Text::new(format!("{} check={:?}", ctx.item, ctx.check_state)).into(),
        )
        .checked_keys(checked.signal())
        .checkable(true)
        .item_layout(crate::ui::widget::ItemLayout::Fixed {
            item_extent: dp(32.0),
            spacing: Dp::ZERO,
            overscan: 0,
        })
        .size(dp(220.0), dp(120.0)),
    );
    let mut config = test_config_with_size(220.0, 120.0);
    config.reduced_motion = true;
    let mut handler = test_handler_with_config(TestVm, Some(tree), invalidation, config);

    let text_contents = |scene: &crate::ui::widget::ComputedScene<TestVm>| {
        scene
            .scene
            .texts
            .iter()
            .map(|text| text.content.to_string())
            .collect::<Vec<_>>()
    };
    let initial = handler.computed_scene().clone();
    let initial_text = text_contents(&initial);
    assert!(initial_text
        .iter()
        .any(|text| text == "Alpha check=Checked"));
    assert!(initial_text
        .iter()
        .any(|text| text == "Beta check=Unchecked"));

    checked.set(vec![WidgetKey::from("b")]);
    handler.request_redraw_if_dirty(Instant::now());

    let (retained, retained_cache_dependencies) = {
        let cached = handler
            .cached_scene
            .as_ref()
            .expect("Tree checked-keys invalidation should preserve the cache shell");
        assert!(cached.layout_valid && cached.computed_valid);
        (
            cached.computed.clone(),
            (
                cached.dependencies.dependency_count(),
                cached.dependencies.has_global_dependency(),
                cached.dependencies.all_owners(),
            ),
        )
    };
    let retained_text = text_contents(&retained);
    assert!(retained_text
        .iter()
        .any(|text| text == "Alpha check=Unchecked"));
    assert!(retained_text
        .iter()
        .any(|text| text == "Beta check=Checked"));
    let retained_b = retained
        .hit_regions
        .iter()
        .find_map(|region| match &region.interaction {
            HitInteraction::TreeNode { id, state, .. } if state.key == WidgetKey::from("b") => {
                assert_eq!(
                    state.check_state,
                    crate::ui::widget::TreeCheckState::Checked
                );
                Some(*id)
            }
            _ => None,
        })
        .expect("checked Tree row b should expose a hit region");
    let retained_accessibility = handler.accessibility_tree_update_for_test();
    let retained_b_node_id = crate::accessibility::node_id_from_widget(retained_b);
    let retained_b_node = retained_accessibility
        .nodes
        .iter()
        .find_map(|(id, node)| (*id == retained_b_node_id).then_some(node))
        .expect("checked Tree row b should expose an accessibility node");
    assert_eq!(retained_b_node.toggled(), Some(accesskit::Toggled::True));

    handler.invalidate_scene_with_reason("tree_checked_equivalence_full_recollect");
    let full = handler.computed_scene().clone();
    let full_cache_dependencies = {
        let cached = handler
            .cached_scene
            .as_ref()
            .expect("full recollect should repopulate the cache");
        (
            cached.dependencies.dependency_count(),
            cached.dependencies.has_global_dependency(),
            cached.dependencies.all_owners(),
        )
    };
    let full_accessibility = handler.accessibility_tree_update_for_test();

    super::table_tests::assert_data_grid_scene_equivalent(&retained, &full);
    assert_eq!(retained_accessibility, full_accessibility);
    assert_eq!(retained_cache_dependencies, full_cache_dependencies);
    assert_eq!(
        (
            retained.dependencies.dependency_count(),
            retained.dependencies.has_global_dependency(),
            retained.dependencies.all_owners(),
        ),
        (
            full.dependencies.dependency_count(),
            full.dependencies.has_global_dependency(),
            full.dependencies.all_owners(),
        )
    );
}

#[test]
fn tree_expanded_keys_signal_updates_cached_scene_and_matches_full_recollect() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let expanded = context.state(Vec::<WidgetKey>::new());
    let tree = WidgetTree::new(
        Tree::<&'static str, TestVm>::new(sample_nodes(), |ctx| Text::new(ctx.item).into())
            .expanded_keys(expanded.signal())
            .item_layout(crate::ui::widget::ItemLayout::Fixed {
                item_extent: dp(32.0),
                spacing: Dp::ZERO,
                overscan: 0,
            })
            .size(dp(240.0), dp(180.0)),
    );
    let mut config = test_config_with_size(240.0, 180.0);
    config.reduced_motion = true;
    let mut handler = test_handler_with_config(TestVm, Some(tree), invalidation, config);
    let initial = handler.computed_scene().clone();
    assert!(!initial.hit_regions.iter().any(|region| {
        matches!(
            &region.interaction,
            HitInteraction::TreeNode { state, .. }
                if state.key == WidgetKey::from("child-a")
        )
    }));

    expanded.set(vec![WidgetKey::from("root")]);
    handler.request_redraw_if_dirty(Instant::now());
    let retained = handler
        .cached_scene
        .as_ref()
        .expect("Tree expanded-keys invalidation should preserve the cache shell")
        .computed
        .clone();
    assert!(retained.hit_regions.iter().any(|region| {
        matches!(
            &region.interaction,
            HitInteraction::TreeNode { state, .. }
                if state.key == WidgetKey::from("child-a")
        )
    }));

    handler.invalidate_scene_with_reason("tree_expanded_equivalence_full_recollect");
    let full = handler.computed_scene().clone();
    super::table_tests::assert_data_grid_scene_equivalent(&retained, &full);
}

#[test]
fn tree_disclosure_click_dispatches_expand_change() {
    let invalidation = InvalidationSignal::new();
    let expanded = Arc::new(Mutex::new(Vec::<WidgetKey>::new()));
    let latest = Arc::new(Mutex::new(None::<TreeExpandChange>));
    let expanded_signal_source = Arc::clone(&expanded);
    let expanded_signal = Signal::new(
        move || expanded_signal_source.lock().unwrap().clone(),
        invalidation.clone(),
    );
    let expanded_for_cmd = Arc::clone(&expanded);
    let latest_for_cmd = Arc::clone(&latest);
    let tree = WidgetTree::new(
        Tree::<&'static str, TestVm>::new(sample_nodes(), |ctx| Text::new(ctx.item).into())
            .expanded_keys(expanded_signal)
            .on_expand_change(ValueCommand::new(
                move |_vm: &mut TestVm, change: TreeExpandChange| {
                    *expanded_for_cmd.lock().unwrap() = change.expanded_keys.clone();
                    *latest_for_cmd.lock().unwrap() = Some(change);
                },
            ))
            .size(dp(260.0), dp(180.0)),
    );
    let mut handler = test_handler(Some(tree), invalidation);
    let viewport = handler.viewport_rect();

    let point = disclosure_point(&mut handler, "root");
    handler.cursor_position = Some(point);
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);

    let change = latest.lock().unwrap().clone().expect("expand change");
    assert_eq!(change.key, WidgetKey::from("root"));
    assert!(change.expanded);
    assert_eq!(*expanded.lock().unwrap(), vec![WidgetKey::from("root")]);
}

#[test]
fn tree_disclosure_click_updates_visible_rows_for_controlled_expanded_keys() {
    let invalidation = InvalidationSignal::new();
    let expanded = Arc::new(Mutex::new(Vec::<WidgetKey>::new()));
    let expanded_signal_source = Arc::clone(&expanded);
    let expanded_signal = Signal::new(
        move || expanded_signal_source.lock().unwrap().clone(),
        invalidation.clone(),
    );
    let expanded_for_cmd = Arc::clone(&expanded);
    let tree = WidgetTree::new(
        Tree::<&'static str, TestVm>::new(sample_nodes(), |ctx| Text::new(ctx.item).into())
            .expanded_keys(expanded_signal)
            .on_expand_change(ValueCommand::new(
                move |_vm: &mut TestVm, change: TreeExpandChange| {
                    *expanded_for_cmd.lock().unwrap() = change.expanded_keys.clone();
                },
            ))
            .size(dp(260.0), dp(220.0)),
    );
    let mut handler = test_handler(Some(tree), invalidation);
    let viewport = handler.viewport_rect();

    assert_eq!(
        visible_tree_keys(&mut handler),
        vec![WidgetKey::from("root"), WidgetKey::from("sibling")]
    );

    let point = visible_disclosure_center(&mut handler, "root");
    handler.cursor_position = Some(point);
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);
    handler.invalidate_computed_scene();
    assert_eq!(
        visible_tree_keys(&mut handler),
        vec![
            WidgetKey::from("root"),
            WidgetKey::from("child-a"),
            WidgetKey::from("child-b"),
            WidgetKey::from("sibling")
        ]
    );

    let point = visible_disclosure_center(&mut handler, "root");
    handler.cursor_position = Some(point);
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);
    handler.invalidate_computed_scene();
    assert_eq!(
        visible_tree_keys(&mut handler),
        vec![WidgetKey::from("root"), WidgetKey::from("sibling")]
    );
}

#[test]
fn tree_disclosure_click_works_with_context_menu_and_dragging_enabled() {
    let invalidation = InvalidationSignal::new();
    let expanded = Arc::new(Mutex::new(Vec::<WidgetKey>::new()));
    let latest = Arc::new(Mutex::new(None::<TreeExpandChange>));
    let expanded_signal_source = Arc::clone(&expanded);
    let expanded_signal = Signal::new(
        move || expanded_signal_source.lock().unwrap().clone(),
        invalidation.clone(),
    );
    let expanded_for_cmd = Arc::clone(&expanded);
    let latest_for_cmd = Arc::clone(&latest);
    let tree = WidgetTree::new(
        Tree::<&'static str, TestVm>::new(sample_nodes(), |ctx| Text::new(ctx.item).into())
            .expanded_keys(expanded_signal)
            .context_menu(vec![MenuItem::new("Rename"), MenuItem::new("Delete")])
            .draggable(true)
            .on_drop(ValueCommand::new(|_: &mut TestVm, _: TreeDropEvent| {}))
            .on_expand_change(ValueCommand::new(
                move |_vm: &mut TestVm, change: TreeExpandChange| {
                    *expanded_for_cmd.lock().unwrap() = change.expanded_keys.clone();
                    *latest_for_cmd.lock().unwrap() = Some(change);
                },
            ))
            .size(dp(260.0), dp(180.0)),
    );
    let mut handler = test_handler(Some(tree), invalidation);
    let point = visible_disclosure_center(&mut handler, "root");
    assert!(matches!(
        top_tree_hit_at(&mut handler, point),
        Some(HitInteraction::TreeDisclosure { state, .. }) if state.key == WidgetKey::from("root")
    ));

    pointer_press(&mut handler, point, MouseButton::Left);

    let change = latest.lock().unwrap().clone().expect("expand change");
    assert_eq!(change.key, WidgetKey::from("root"));
    assert!(change.expanded);
    assert_eq!(*expanded.lock().unwrap(), vec![WidgetKey::from("root")]);
}

#[test]
fn tree_selection_and_keyboard_action_work_for_rows() {
    let invalidation = InvalidationSignal::new();
    let selected = Arc::new(Mutex::new(Vec::<WidgetKey>::new()));
    let selected_signal_source = Arc::clone(&selected);
    let selected_signal = Signal::new(
        move || selected_signal_source.lock().unwrap().clone(),
        invalidation.clone(),
    );
    let selected_for_cmd = Arc::clone(&selected);
    let action = Arc::new(Mutex::new(None::<TreeNodeAction>));
    let action_for_cmd = Arc::clone(&action);
    let tree = WidgetTree::new(
        Tree::<&'static str, TestVm>::new(sample_nodes(), |ctx| Text::new(ctx.item).into())
            .expanded_keys(vec![WidgetKey::from("root")])
            .selection_mode(TreeSelectionMode::Multiple)
            .selected_keys(selected_signal)
            .on_selection_change(ValueCommand::new(
                move |_vm: &mut TestVm, change: TreeSelectionChange| {
                    *selected_for_cmd.lock().unwrap() = change.selected_keys;
                },
            ))
            .on_node_action(ValueCommand::new(move |_vm: &mut TestVm, value| {
                *action_for_cmd.lock().unwrap() = Some(value);
            }))
            .size(dp(260.0), dp(220.0)),
    );
    let mut handler = test_handler(Some(tree), invalidation);
    let viewport = handler.viewport_rect();
    let (_, child) = tree_row_center(&mut handler, "child-a");

    handler.cursor_position = Some(child);
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);
    handler.invalidate_computed_scene();
    assert_eq!(*selected.lock().unwrap(), vec![WidgetKey::from("child-a")]);

    assert!(handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Enter))));
    assert_eq!(
        *action.lock().unwrap(),
        Some(TreeNodeAction {
            index: 1,
            key: WidgetKey::from("child-a"),
        })
    );
}

#[test]
fn tree_uses_viewport_as_its_only_default_tab_stop() {
    let invalidation = InvalidationSignal::new();
    let tree: Element<TestVm> = Tree::<&'static str, TestVm>::new(
        vec![
            TreeNode::keyed("a", "Alpha").disable(true),
            TreeNode::keyed("b", "Beta"),
            TreeNode::keyed("c", "Gamma"),
        ],
        |ctx| Text::new(ctx.item).into(),
    )
    .size(dp(240.0), dp(144.0))
    .into();
    let tree_id = tree.id;
    let mut handler = test_handler(Some(WidgetTree::new(tree)), invalidation);

    let tab_order = handler
        .focusable_widgets_in_tab_order()
        .into_iter()
        .map(|focused| focused.widget_id)
        .collect::<Vec<_>>();
    assert_eq!(tab_order, vec![tree_id]);

    let rows = handler
        .computed_scene()
        .hit_regions
        .iter()
        .filter_map(|region| match &region.interaction {
            HitInteraction::TreeNode { state, .. } => Some((
                state.key.clone(),
                region.focus.as_ref().map(|focus| focus.tab_index),
            )),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        rows,
        vec![
            (WidgetKey::from("a"), None),
            (WidgetKey::from("b"), Some(Some(-1))),
            (WidgetKey::from("c"), Some(Some(-1))),
        ]
    );

    assert!(handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Tab))));
    assert_eq!(handler.focused_widget_id(), Some(tree_id));
    assert!(
        handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::ArrowDown)))
    );
    let focused_id = handler.focused_widget_id();
    let focused_key = handler
        .computed_scene()
        .hit_regions
        .iter()
        .find_map(|region| match &region.interaction {
            HitInteraction::TreeNode { id, state, .. } if Some(*id) == focused_id => {
                Some(state.key.clone())
            }
            _ => None,
        });
    assert_eq!(focused_key, Some(WidgetKey::from("b")));
}

#[test]
fn tree_focus_moves_to_nearest_enabled_row_after_live_disable() {
    let invalidation = InvalidationSignal::new();
    let disabled = State::new(false, invalidation.clone());
    let tree = WidgetTree::new(
        Tree::<&'static str, TestVm>::new(
            vec![
                TreeNode::keyed("a", "Alpha"),
                TreeNode::keyed("b", "Beta").disable(disabled.signal()),
                TreeNode::keyed("c", "Gamma"),
            ],
            |ctx| Text::new(ctx.item).into(),
        )
        .size(dp(240.0), dp(144.0)),
    );
    let mut handler = test_handler(Some(tree), invalidation);
    let viewport = handler.viewport_rect();
    let (_, beta) = tree_row_center(&mut handler, "b");
    handler.cursor_position = Some(beta);
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);

    disabled.set(true);
    handler.request_redraw_if_dirty(Instant::now());
    let _ = handler.computed_scene();
    let focused_id = handler.focused_widget_id();
    let focused_key = handler
        .computed_scene()
        .hit_regions
        .iter()
        .find_map(|region| match &region.interaction {
            HitInteraction::TreeNode { id, state, .. } if Some(*id) == focused_id => {
                Some(state.key.clone())
            }
            _ => None,
        });
    assert_eq!(focused_key, Some(WidgetKey::from("c")));
}

#[test]
fn tree_focus_returns_to_root_when_all_rows_become_disabled() {
    let invalidation = InvalidationSignal::new();
    let disabled = State::new(false, invalidation.clone());
    let tree: Element<TestVm> = Tree::<&'static str, TestVm>::new(
        vec![
            TreeNode::keyed("a", "Alpha").disable(true),
            TreeNode::keyed("b", "Beta").disable(disabled.signal()),
            TreeNode::keyed("c", "Gamma").disable(true),
        ],
        |ctx| Text::new(ctx.item).into(),
    )
    .size(dp(240.0), dp(144.0))
    .into();
    let tree_id = tree.id;
    let mut handler = test_handler(Some(WidgetTree::new(tree)), invalidation);
    let viewport = handler.viewport_rect();
    let (_, beta) = tree_row_center(&mut handler, "b");
    handler.cursor_position = Some(beta);
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);

    disabled.set(true);
    handler.request_redraw_if_dirty(Instant::now());
    let _ = handler.computed_scene();
    assert_eq!(handler.focused_widget_id(), Some(tree_id));
    assert!(handler.tree_focus_state.is_none());
}

#[test]
fn tree_space_check_cascades_enabled_descendants() {
    let invalidation = InvalidationSignal::new();
    let checked = Arc::new(Mutex::new(Vec::<WidgetKey>::new()));
    let checked_signal_source = Arc::clone(&checked);
    let checked_signal = Signal::new(
        move || checked_signal_source.lock().unwrap().clone(),
        invalidation.clone(),
    );
    let latest = Arc::new(Mutex::new(None::<TreeCheckChange>));
    let checked_for_cmd = Arc::clone(&checked);
    let latest_for_cmd = Arc::clone(&latest);
    let tree = WidgetTree::new(
        Tree::<&'static str, TestVm>::new(sample_nodes(), |ctx| Text::new(ctx.item).into())
            .expanded_keys(vec![WidgetKey::from("root")])
            .checkable(true)
            .checked_keys(checked_signal)
            .on_check_change(ValueCommand::new(
                move |_vm: &mut TestVm, change: TreeCheckChange| {
                    *checked_for_cmd.lock().unwrap() = change.checked_keys.clone();
                    *latest_for_cmd.lock().unwrap() = Some(change);
                },
            ))
            .size(dp(260.0), dp(220.0)),
    );
    let mut handler = test_handler(Some(tree), invalidation);
    let viewport = handler.viewport_rect();
    let (_, root) = tree_row_center(&mut handler, "root");

    handler.cursor_position = Some(root);
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);
    assert!(handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Space))));

    assert_eq!(
        *checked.lock().unwrap(),
        vec![
            WidgetKey::from("root"),
            WidgetKey::from("child-a"),
            WidgetKey::from("leaf"),
        ]
    );
    assert_eq!(
        latest.lock().unwrap().as_ref().unwrap().affected_keys,
        vec![
            WidgetKey::from("root"),
            WidgetKey::from("child-a"),
            WidgetKey::from("leaf"),
        ]
    );
}

#[test]
fn tree_checkbox_click_dispatches_check_change() {
    let invalidation = InvalidationSignal::new();
    let latest = Arc::new(Mutex::new(None::<TreeCheckChange>));
    let latest_for_cmd = Arc::clone(&latest);
    let tree = WidgetTree::new(
        Tree::<&'static str, TestVm>::new(sample_nodes(), |ctx| Text::new(ctx.item).into())
            .expanded_keys(vec![WidgetKey::from("root")])
            .checkable(true)
            .on_check_change(ValueCommand::new(
                move |_vm: &mut TestVm, change: TreeCheckChange| {
                    *latest_for_cmd.lock().unwrap() = Some(change);
                },
            ))
            .size(dp(260.0), dp(220.0)),
    );
    let mut handler = test_handler(Some(tree), invalidation);
    let viewport = handler.viewport_rect();
    let point = checkbox_point(&mut handler, "root");
    assert!(matches!(
        top_tree_hit_at(&mut handler, point),
        Some(HitInteraction::TreeCheckbox { state, .. }) if state.key == WidgetKey::from("root")
    ));

    handler.cursor_position = Some(point);
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);

    let change = latest.lock().unwrap().clone().expect("check change");
    assert!(change.checked);
    assert!(change.checked_keys.contains(&WidgetKey::from("leaf")));
    assert!(!change.checked_keys.contains(&WidgetKey::from("child-b")));
}

#[test]
fn tree_right_click_opens_row_context_menu() {
    let invalidation = InvalidationSignal::new();
    let tree = WidgetTree::new(
        Tree::<&'static str, TestVm>::new(sample_nodes(), |ctx| Text::new(ctx.item).into())
            .expanded_keys(vec![WidgetKey::from("root")])
            .context_menu(vec![MenuItem::new("Rename"), MenuItem::new("Delete")])
            .size(dp(260.0), dp(220.0)),
    );
    let mut handler = test_handler(Some(tree), invalidation);
    let viewport = handler.viewport_rect();
    let (row_id, point) = tree_row_center(&mut handler, "child-a");

    handler.cursor_position = Some(point);
    let _ = handler.handle_hover(viewport);
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Right);

    assert!(handler.context_menu_anchor_states.contains_key(&row_id));
    handler.invalidate_computed_scene();
    let labels = handler
        .computed_scene()
        .scene
        .overlay_texts
        .iter()
        .map(|text| text.content.as_ref().to_string())
        .collect::<Vec<_>>();
    assert!(labels.iter().any(|label| label == "Rename"));
    assert!(labels.iter().any(|label| label == "Delete"));
}

#[test]
fn tree_live_disabled_row_blocks_context_menu_right_click_and_long_press() {
    let invalidation = InvalidationSignal::new();
    let disabled = Arc::new(Mutex::new(false));
    let disabled_for_signal = Arc::clone(&disabled);
    let disabled_signal = Signal::new(
        move || {
            *disabled_for_signal
                .lock()
                .expect("disabled lock should succeed")
        },
        invalidation.clone(),
    );
    let tree = WidgetTree::new(
        Tree::<&'static str, TestVm>::new(
            vec![TreeNode::keyed("row", "Row").disable(disabled_signal)],
            |ctx| Text::new(ctx.item).into(),
        )
        .context_menu(vec![MenuItem::new("Rename")])
        .size(dp(260.0), dp(48.0)),
    );
    let mut handler = test_handler(Some(tree), invalidation.clone());
    let viewport = handler.viewport_rect();
    let (row_id, point) = tree_row_center(&mut handler, "row");

    *disabled.lock().expect("disabled lock should succeed") = true;
    invalidation.mark_dirty();

    handler.cursor_position = Some(point);
    let _ = handler.handle_hover(viewport);
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Right);
    assert!(
        !handler.context_menu_anchor_states.contains_key(&row_id),
        "a tree row disabled after construction must reject a right-click context menu"
    );

    let long_press_started = Instant::now();
    handler.handle_bound_window_event(
        &TestEventLoop,
        WindowEvent::PointerButton {
            device_id: None,
            position: PhysicalPosition::new(f64::from(point.x.get()), f64::from(point.y.get())),
            state: ElementState::Pressed,
            button: ButtonSource::Touch {
                finger_id: FingerId::from_raw(1),
                force: None,
            },
            primary: true,
        },
    );
    let _ = handler.drive_animations(
        &TestEventLoop,
        long_press_started + crate::runtime::LONG_PRESS_THRESHOLD + Duration::from_millis(10),
    );
    assert!(
        !handler.context_menu_anchor_states.contains_key(&row_id),
        "a tree row disabled after construction must reject a long-press context menu"
    );
}

#[test]
fn tree_drop_dispatches_inside_event_and_rejects_self_drop() {
    let invalidation = InvalidationSignal::new();
    let latest = Arc::new(Mutex::new(None::<TreeDropEvent>));
    let latest_for_cmd = Arc::clone(&latest);
    let tree = WidgetTree::new(
        Tree::<&'static str, TestVm>::new(sample_nodes(), |ctx| Text::new(ctx.item).into())
            .expanded_keys(vec![WidgetKey::from("root")])
            .draggable(true)
            .on_drop(ValueCommand::new(move |_vm: &mut TestVm, event| {
                *latest_for_cmd.lock().unwrap() = Some(event);
            }))
            .size(dp(260.0), dp(220.0)),
    );
    let mut handler = test_handler(Some(tree), invalidation);
    let viewport = handler.viewport_rect();
    let (_, child) = tree_row_center(&mut handler, "child-a");
    let (_, sibling) = tree_row_center(&mut handler, "sibling");

    handler.cursor_position = Some(child);
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);
    pointer_release(&mut handler, sibling);
    assert_eq!(
        *latest.lock().unwrap(),
        Some(TreeDropEvent {
            dragged_key: WidgetKey::from("child-a"),
            target_key: WidgetKey::from("sibling"),
            position: TreeDropPosition::Inside,
        })
    );

    *latest.lock().unwrap() = None;
    handler.cursor_position = Some(child);
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);
    pointer_release(&mut handler, child);
    assert_eq!(*latest.lock().unwrap(), None);
}

#[test]
fn virtual_tree_arrow_down_crosses_materialized_window_boundary() {
    let invalidation = InvalidationSignal::new();
    let nodes = (0..64)
        .map(|index| TreeNode::keyed(format!("row-{index}"), index))
        .collect::<Vec<_>>();
    let tree = WidgetTree::new(
        Tree::<usize, TestVm>::new(nodes, |context| Text::new(context.item.to_string()).into())
            .item_layout(crate::ui::widget::ItemLayout::Fixed {
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
            HitInteraction::TreeNode { id, state, .. } => {
                Some((*id, state.clone(), region.focus.clone()?))
            }
            _ => None,
        })
        .max_by_key(|(_, state, _)| state.row_index)
        .expect("virtual Tree should materialize at least one row");
    let next_index = state.row_index + 1;
    let next_key = state
        .visible_keys
        .get(next_index)
        .cloned()
        .expect("test must stop before the visible source end");
    handler.focused_widget = Some(FocusedWidget {
        widget_id: id,
        scope_path: focus.scope_path,
        on_blur: focus.on_blur,
    });
    handler.tree_focus_state = Some((state.tree_id, state.key));
    handler.focus_visible = true;

    assert!(
        handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::ArrowDown))),
        "ArrowDown at the materialized boundary should schedule the next visible Tree row"
    );
    let _ = handler.computed_scene();
    assert_eq!(
        handler.tree_focus_state.as_ref().map(|(_, key)| key),
        Some(&next_key),
        "Tree focus should advance after the target row materializes"
    );

    let (id, state, focus) = handler
        .computed_scene()
        .hit_regions
        .iter()
        .filter_map(|region| match &region.interaction {
            HitInteraction::TreeNode { id, state, .. } => {
                Some((*id, state.clone(), region.focus.clone()?))
            }
            _ => None,
        })
        .min_by_key(|(_, state, _)| state.row_index)
        .expect("scrolled Tree should materialize a first boundary row");
    let previous_key = state
        .row_index
        .checked_sub(1)
        .and_then(|index| state.visible_keys.get(index))
        .cloned()
        .expect("test must stop after the visible source start");
    handler.focused_widget = Some(FocusedWidget {
        widget_id: id,
        scope_path: focus.scope_path,
        on_blur: focus.on_blur,
    });
    handler.tree_focus_state = Some((state.tree_id, state.key));

    let arrow_up = pressed_key_event(PhysicalKey::Code(KeyCode::ArrowUp));
    assert!(handler.handle_keyboard_input(&arrow_up));
    let focused_id = handler.focused_widget_id();
    let focused_key = handler
        .computed_scene()
        .hit_regions
        .iter()
        .find_map(|region| match &region.interaction {
            HitInteraction::TreeNode { id, state, .. } if Some(*id) == focused_id => {
                Some(state.key.clone())
            }
            _ => None,
        });
    assert_eq!(focused_key.as_ref(), Some(&previous_key));
}

#[test]
fn virtual_tree_home_end_and_page_use_full_visible_source() {
    let invalidation = InvalidationSignal::new();
    let nodes = (0..100)
        .map(|index| TreeNode::keyed(format!("row-{index}"), index))
        .collect::<Vec<_>>();
    let tree = WidgetTree::new(
        Tree::<usize, TestVm>::new(nodes, |context| Text::new(context.item.to_string()).into())
            .item_layout(crate::ui::widget::ItemLayout::Fixed {
                item_extent: dp(28.0),
                spacing: Dp::ZERO,
                overscan: 0,
            })
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
            HitInteraction::TreeNode { id, state, .. } => {
                Some((*id, state.clone(), region.focus.clone()?))
            }
            _ => None,
        })
        .min_by_key(|(_, state, _)| state.row_index)
        .expect("virtual Tree should materialize a first row");
    let tree_id = state.tree_id;
    handler.focused_widget = Some(FocusedWidget {
        widget_id: id,
        scope_path: focus.scope_path,
        on_blur: focus.on_blur,
    });
    handler.tree_focus_state = Some((tree_id, state.key));

    assert!(handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::End))));
    assert_eq!(
        handler.tree_focus_state.as_ref().map(|(_, key)| key),
        Some(&WidgetKey::from("row-99"))
    );
    assert!(handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Home))));
    assert_eq!(
        handler.tree_focus_state.as_ref().map(|(_, key)| key),
        Some(&WidgetKey::from("row-0"))
    );

    let (page, viewport_height) = handler
        .computed_scene()
        .scroll_regions
        .iter()
        .find(|region| region.id == tree_id)
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
        handler.tree_focus_state.as_ref().map(|(_, key)| key),
        Some(&WidgetKey::from(format!("row-{page}")))
    );
    let page_offset = handler
        .scroll_states
        .get(&tree_id)
        .map(|offset| offset.y)
        .unwrap_or(Dp::ZERO);
    let smooth = handler
        .smooth_scroll_states
        .get(&tree_id)
        .map(|state| (state.start, state.target));
    let focused_id = handler.focused_widget_id();
    let focused_state = handler
        .computed_scene()
        .hit_regions
        .iter()
        .find_map(|region| match &region.interaction {
            HitInteraction::TreeNode { id, state, .. } if Some(*id) == focused_id => {
                Some(state.clone())
            }
            _ => None,
        })
        .unwrap();
    let focused_bounds = handler.virtual_states[&tree_id].item_main_bounds(
        focused_state.row_index,
        focused_state.item_extent,
        focused_state.item_spacing,
    );
    let region_after = handler
        .computed_scene()
        .scroll_regions
        .iter()
        .find(|region| region.id == tree_id)
        .copied()
        .unwrap();
    assert!(
        (page_offset - viewport_height).abs() <= 0.01,
        "PageDown should advance one viewport ({viewport_height:?}), got {page_offset:?}; smooth={smooth:?}; row={} bounds={focused_bounds:?}; after_viewport={:?} max={:?} content={:?}",
        focused_state.row_index,
        region_after.content_viewport,
        region_after.max_offset(),
        region_after.content_bounds,
    );
    assert!(handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::PageUp,))));
    assert_eq!(
        handler.tree_focus_state.as_ref().map(|(_, key)| key),
        Some(&WidgetKey::from("row-0"))
    );
}

#[test]
fn virtual_tree_100k_materialization_bounds_disabled_signal_reads() {
    let invalidation = InvalidationSignal::new();
    let disabled_reads = Arc::new(AtomicUsize::new(0));
    let nodes = (0..100_000)
        .map(|index| {
            let reads_for_signal = Arc::clone(&disabled_reads);
            let disabled = Signal::new(
                move || {
                    reads_for_signal.fetch_add(1, Ordering::SeqCst);
                    false
                },
                invalidation.clone(),
            );
            TreeNode::keyed(format!("row-{index}"), index).disable(disabled)
        })
        .collect::<Vec<_>>();
    let tree = WidgetTree::new(
        Tree::<usize, TestVm>::new(nodes, |context| Text::new(context.item.to_string()).into())
            .item_layout(crate::ui::widget::ItemLayout::Fixed {
                item_extent: dp(28.0),
                spacing: Dp::ZERO,
                overscan: 0,
            })
            .size(dp(220.0), dp(84.0)),
    );
    let mut handler = test_handler_with_config(
        TestVm,
        Some(tree),
        invalidation,
        test_config_with_size(220.0, 84.0),
    );

    let (materialized, first_target) = {
        let computed = handler.computed_scene();
        let materialized = computed
            .hit_regions
            .iter()
            .filter(|region| matches!(region.interaction, HitInteraction::TreeNode { .. }))
            .count();
        let first_target =
            computed
                .hit_regions
                .iter()
                .find_map(|region| match &region.interaction {
                    HitInteraction::TreeNode { id, state, .. } => {
                        Some((*id, state.clone(), region.focus.clone()?))
                    }
                    _ => None,
                });
        (materialized, first_target)
    };
    assert!(
        materialized <= 4,
        "test viewport should stay tightly virtualized"
    );
    assert!(
        disabled_reads.load(Ordering::SeqCst) <= 64,
        "materializing {materialized} rows must not resolve disabled across the 100k source"
    );

    let (id, state, focus) = first_target.expect("first Tree row should be focusable");
    handler.focused_widget = Some(FocusedWidget {
        widget_id: id,
        scope_path: focus.scope_path,
        on_blur: focus.on_blur,
    });
    handler.tree_focus_state = Some((state.tree_id, state.key));
    disabled_reads.store(0, Ordering::SeqCst);
    assert!(handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::End))));
    assert_eq!(
        handler.tree_focus_state.as_ref().map(|(_, key)| key),
        Some(&WidgetKey::from("row-99999"))
    );
    assert!(
        disabled_reads.load(Ordering::SeqCst) <= 64,
        "100k End navigation must resolve only a bounded visible slice"
    );
}

#[test]
fn virtual_tree_page_long_disabled_run_is_conservative() {
    let invalidation = InvalidationSignal::new();
    let nodes = (0..32)
        .map(|index| {
            let node = TreeNode::keyed(format!("row-{index}"), index);
            if (1..=15).contains(&index) {
                node.disable(true)
            } else {
                node
            }
        })
        .collect::<Vec<_>>();
    let tree = WidgetTree::new(
        Tree::<usize, TestVm>::new(nodes, |context| Text::new(context.item.to_string()).into())
            .item_layout(crate::ui::widget::ItemLayout::Fixed {
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
            HitInteraction::TreeNode { id, state, .. } if state.key == WidgetKey::from("row-0") => {
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
    handler.tree_focus_state = Some((state.tree_id, state.key));

    assert!(
        handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::PageDown,)))
    );
    assert_eq!(
        handler.tree_focus_state.as_ref().map(|(_, key)| key),
        Some(&WidgetKey::from("row-0"))
    );
    assert!(!handler.scroll_states.contains_key(&state.tree_id));
}

#[test]
fn measured_virtual_tree_page_uses_sparse_prefix_bounds() {
    let invalidation = InvalidationSignal::new();
    let nodes = (0..64)
        .map(|index| TreeNode::keyed(format!("row-{index}"), index))
        .collect::<Vec<_>>();
    let tree = WidgetTree::new(
        Tree::<usize, TestVm>::new(nodes, |context| {
            Stack::new()
                .height(if context.item % 2 == 0 {
                    dp(32.0)
                } else {
                    dp(52.0)
                })
                .child(Text::new(context.item.to_string()))
                .into()
        })
        .item_layout(crate::ui::widget::ItemLayout::Measured {
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
            HitInteraction::TreeNode { id, state, .. } => {
                Some((*id, state.clone(), region.focus.clone()?))
            }
            _ => None,
        })
        .min_by_key(|(_, state, _)| state.row_index)
        .expect("measured Tree should materialize a first row");
    let tree_id = state.tree_id;
    let viewport_height = handler
        .computed_scene()
        .scroll_regions
        .iter()
        .find(|region| region.id == tree_id)
        .map(|region| region.content_viewport.height)
        .expect("measured Tree should scroll");
    let current_top = handler
        .virtual_states
        .get(&tree_id)
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
    handler.tree_focus_state = Some((tree_id, state.key));

    assert!(
        handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::PageDown,)))
    );
    let focused_id = handler.focused_widget_id();
    let focused_state = handler
        .computed_scene()
        .hit_regions
        .iter()
        .find_map(|region| match &region.interaction {
            HitInteraction::TreeNode { id, state, .. } if Some(*id) == focused_id => {
                Some(state.clone())
            }
            _ => None,
        })
        .expect("PageDown should focus a measured Tree row");
    let focused_top = handler
        .virtual_states
        .get(&tree_id)
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
    assert!(focused_state.row_index > state.row_index);
    assert!(focused_top + dp(0.01) >= current_top + viewport_height);
    let page_offset = handler.scroll_states[&tree_id].y;
    assert!(
        page_offset >= viewport_height - dp(52.0)
            && page_offset <= viewport_height + dp(52.0),
        "measured feedback may correct by at most one row: viewport={viewport_height:?}, offset={page_offset:?}"
    );
}
