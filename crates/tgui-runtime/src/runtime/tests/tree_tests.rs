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
