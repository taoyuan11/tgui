use super::*;
use crate::ui::widget::{Tree, TreeNode, TreeSelectionMode};
use accesskit::{Action, ActionData, ActionRequest, Node, Role, Toggled, TreeId};

fn accessibility_update(handler: &mut BoundRuntimeHandler<TestVm>) -> accesskit::TreeUpdate {
    let _ = handler.computed_scene();
    let focused = handler.focused_widget_id();
    let cached = handler
        .cached_scene
        .as_ref()
        .expect("computed scene should be cached");
    crate::accessibility::build_tree_update(
        cached.layout.as_ref(),
        &cached.computed,
        focused,
        cached.viewport,
    )
}

fn node_for(update: &accesskit::TreeUpdate, widget_id: WidgetId) -> &Node {
    let node_id = crate::accessibility::node_id_from_widget(widget_id);
    update
        .nodes
        .iter()
        .find_map(|(id, node)| (*id == node_id).then_some(node))
        .expect("accessibility node should exist")
}

fn has_node(update: &accesskit::TreeUpdate, widget_id: WidgetId) -> bool {
    let node_id = crate::accessibility::node_id_from_widget(widget_id);
    update.nodes.iter().any(|(id, _)| *id == node_id)
}

fn action_request(widget_id: WidgetId, action: Action, data: Option<ActionData>) -> ActionRequest {
    ActionRequest {
        action,
        target_tree: TreeId::ROOT,
        target_node: crate::accessibility::node_id_from_widget(widget_id),
        data,
    }
}

#[test]
fn accessibility_tree_maps_basic_roles_labels_values_and_bounds() {
    let invalidation = InvalidationSignal::new();
    let button: Element<TestVm> = Button::new("Save").size(dp(80.0), dp(30.0)).into();
    let button_id = button.id;
    let checkbox: Element<TestVm> = Checkbox::new(true)
        .label("Remember")
        .size(dp(120.0), dp(30.0))
        .into();
    let checkbox_id = checkbox.id;
    let slider: Element<TestVm> = Slider::new(4.0, 0.0, 10.0)
        .step(2.0)
        .size(dp(160.0), dp(30.0))
        .into();
    let slider_id = slider.id;
    let tree = WidgetTree::new(Flex::new(Axis::Vertical).child([button, checkbox, slider]));
    let mut handler = test_handler(Some(tree), invalidation);

    let update = accessibility_update(&mut handler);

    let button_node = node_for(&update, button_id);
    assert_eq!(button_node.role(), Role::Button);
    assert_eq!(button_node.label(), Some("Save"));
    assert!(button_node.supports_action(Action::Click));
    assert!(button_node.bounds().is_some());

    let checkbox_node = node_for(&update, checkbox_id);
    assert_eq!(checkbox_node.role(), Role::CheckBox);
    assert_eq!(checkbox_node.label(), Some("Remember"));
    assert_eq!(checkbox_node.toggled(), Some(Toggled::True));

    let slider_node = node_for(&update, slider_id);
    assert_eq!(slider_node.role(), Role::Slider);
    assert_eq!(slider_node.numeric_value(), Some(4.0));
    assert_eq!(slider_node.min_numeric_value(), Some(0.0));
    assert_eq!(slider_node.max_numeric_value(), Some(10.0));
    assert!(slider_node.supports_action(Action::SetValue));
}

#[test]
fn accessibility_tree_maps_data_grid_roles_and_selection() {
    let invalidation = InvalidationSignal::new();
    let columns: Vec<DataGridColumn<&'static str, TestVm>> =
        vec![DataGridColumn::new("name", "Name".to_string(), |ctx| {
            Text::new(ctx.row).into()
        })];
    let grid: Element<TestVm> =
        DataGrid::<&'static str, TestVm>::new(vec![DataGridRow::keyed("a", "Alpha")], columns)
            .selected_keys(vec![WidgetKey::from("a")])
            .size(dp(220.0), dp(120.0))
            .into();
    let grid_id = grid.id;
    let tree = WidgetTree::new(grid);
    let mut handler = test_handler(Some(tree), invalidation);
    let (header_id, cell_id) = {
        let computed = handler.computed_scene();
        let header_id = computed
            .hit_regions
            .iter()
            .find_map(|region| match &region.interaction {
                HitInteraction::DataGridHeader { id, .. } => Some(*id),
                _ => None,
            })
            .expect("DataGrid header should have a hit region");
        let cell_id = computed
            .hit_regions
            .iter()
            .find_map(|region| match &region.interaction {
                HitInteraction::DataGridCell { id, .. } => Some(*id),
                _ => None,
            })
            .expect("DataGrid cell should have a hit region");
        (header_id, cell_id)
    };

    let update = accessibility_update(&mut handler);
    assert_eq!(node_for(&update, grid_id).role(), Role::Grid);
    assert_eq!(node_for(&update, header_id).role(), Role::ColumnHeader);
    assert_eq!(node_for(&update, header_id).label(), Some("Name"));
    let cell_node = node_for(&update, cell_id);
    assert_eq!(cell_node.role(), Role::GridCell);
    assert_eq!(cell_node.is_selected(), Some(true));
    assert!(cell_node.supports_action(Action::Click));
}

#[test]
fn accessibility_tree_maps_tree_roles_state_and_set_metadata() {
    let invalidation = InvalidationSignal::new();
    let tree_element: Element<TestVm> = Tree::<&'static str, TestVm>::new(
        vec![TreeNode::keyed("root", "Root").children([
            TreeNode::keyed("child-a", "Child A"),
            TreeNode::keyed("child-b", "Child B").disable(true),
        ])],
        |ctx| Text::new(ctx.item).into(),
    )
    .expanded_keys(vec![WidgetKey::from("root")])
    .selected_keys(vec![WidgetKey::from("child-a")])
    .selection_mode(TreeSelectionMode::Multiple)
    .checkable(true)
    .checked_keys(vec![WidgetKey::from("child-a")])
    .size(dp(260.0), dp(180.0))
    .into();
    let tree_id = tree_element.id;
    let tree = WidgetTree::new(tree_element);
    let mut handler = test_handler(Some(tree), invalidation);
    let (root_row_id, child_row_id, disabled_row_id) = {
        let computed = handler.computed_scene();
        let find_row = |key: &str| {
            computed
                .hit_regions
                .iter()
                .find_map(|region| match &region.interaction {
                    HitInteraction::TreeNode { id, state, .. }
                        if state.key == WidgetKey::from(key) =>
                    {
                        Some(*id)
                    }
                    _ => None,
                })
                .expect("Tree node should have a hit region")
        };
        (find_row("root"), find_row("child-a"), find_row("child-b"))
    };

    let update = accessibility_update(&mut handler);
    let tree_node = node_for(&update, tree_id);
    assert_eq!(tree_node.role(), Role::Tree);
    assert_eq!(tree_node.size_of_set(), Some(3));
    assert!(tree_node.is_multiselectable());

    let root_node = node_for(&update, root_row_id);
    assert_eq!(root_node.role(), Role::TreeItem);
    assert_eq!(root_node.is_expanded(), Some(true));
    assert_eq!(root_node.toggled(), Some(Toggled::Mixed));
    assert_eq!(root_node.level(), Some(1));
    assert_eq!(root_node.position_in_set(), Some(1));
    assert_eq!(root_node.size_of_set(), Some(1));
    assert!(root_node.supports_action(Action::Click));

    let child_node = node_for(&update, child_row_id);
    assert_eq!(child_node.role(), Role::TreeItem);
    assert_eq!(child_node.is_selected(), Some(true));
    assert_eq!(child_node.toggled(), Some(Toggled::True));
    assert_eq!(child_node.level(), Some(2));
    assert_eq!(child_node.position_in_set(), Some(1));
    assert_eq!(child_node.size_of_set(), Some(2));

    let disabled_node = node_for(&update, disabled_row_id);
    assert_eq!(disabled_node.role(), Role::TreeItem);
    assert!(disabled_node.is_disabled());
    assert_eq!(disabled_node.toggled(), Some(Toggled::False));
}

#[test]
fn accessibility_tree_focus_matches_runtime_focus() {
    let invalidation = InvalidationSignal::new();
    let button: Element<TestVm> = Button::new("Focus").size(dp(80.0), dp(30.0)).into();
    let button_id = button.id;
    let tree = WidgetTree::new(button);
    let mut handler = test_handler(Some(tree), invalidation);

    handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Tab)));
    let update = accessibility_update(&mut handler);

    assert_eq!(
        update.focus,
        crate::accessibility::node_id_from_widget(button_id)
    );
}

#[test]
fn accessibility_tree_hides_nodes_outside_active_focus_trap() {
    let invalidation = InvalidationSignal::new();
    let inside: Element<TestVm> = Button::new("Inside").size(dp(80.0), dp(30.0)).into();
    let inside_id = inside.id;
    let outside: Element<TestVm> = Button::new("Outside")
        .size(dp(80.0), dp(30.0))
        .position_absolute()
        .top(dp(60.0))
        .into();
    let outside_id = outside.id;
    let tree = WidgetTree::new(
        Stack::new().child([
            Flex::new(Axis::Vertical)
                .focus_scope(FocusScopeOptions::new().trap(true))
                .child(inside)
                .into(),
            outside,
        ]),
    );
    let mut handler = test_handler(Some(tree), invalidation);

    let update = accessibility_update(&mut handler);

    assert!(has_node(&update, inside_id));
    assert!(!has_node(&update, outside_id));
}

#[test]
fn accessibility_focus_and_click_actions_use_runtime_focus_and_commands() {
    let invalidation = InvalidationSignal::new();
    let clicks = Arc::new(AtomicUsize::new(0));
    let clicks_ref = Arc::clone(&clicks);
    let button: Element<TestVm> = Button::new("Run")
        .size(dp(80.0), dp(30.0))
        .on_click(Command::new(move |_vm: &mut TestVm| {
            clicks_ref.fetch_add(1, Ordering::SeqCst);
        }))
        .into();
    let button_id = button.id;
    let tree = WidgetTree::new(button);
    let mut handler = test_handler(Some(tree), invalidation);
    let _ = handler.computed_scene();

    handler
        .accessibility_action_sender
        .send(action_request(button_id, Action::Focus, None))
        .unwrap();
    assert!(handler.drain_accessibility_actions());
    assert_eq!(handler.focused_widget_id(), Some(button_id));

    handler
        .accessibility_action_sender
        .send(action_request(button_id, Action::Click, None))
        .unwrap();
    assert!(handler.drain_accessibility_actions());
    assert_eq!(clicks.load(Ordering::SeqCst), 1);
}

#[test]
fn accessibility_slider_and_text_set_value_use_existing_change_paths() {
    let invalidation = InvalidationSignal::new();
    let slider_value = Arc::new(Mutex::new(0.0));
    let slider_value_ref = Arc::clone(&slider_value);
    let slider: Element<TestVm> = Slider::new(0.0, 0.0, 10.0)
        .step(1.0)
        .size(dp(120.0), dp(30.0))
        .on_change(ValueCommand::new(move |_vm: &mut TestVm, value| {
            *slider_value_ref.lock().unwrap() = value;
        }))
        .into();
    let slider_id = slider.id;

    let controller = TextController::new_legacy("old");
    let text_changes = Arc::new(Mutex::new(Vec::<String>::new()));
    let text_changes_ref = Arc::clone(&text_changes);
    let input: Element<TestVm> = Input::new(controller.clone())
        .size(dp(160.0), dp(30.0))
        .on_change(Command::new(move |_vm: &mut TestVm| {
            text_changes_ref.lock().unwrap().push("changed".to_string());
        }))
        .into();
    let input_id = input.id;

    let tree = WidgetTree::new(Flex::new(Axis::Vertical).child([slider, input]));
    let mut handler = test_handler(Some(tree), invalidation);
    let _ = handler.computed_scene();

    handler
        .accessibility_action_sender
        .send(action_request(
            slider_id,
            Action::SetValue,
            Some(ActionData::NumericValue(7.0)),
        ))
        .unwrap();
    assert!(handler.drain_accessibility_actions());
    assert_eq!(*slider_value.lock().unwrap(), 7.0);

    handler
        .accessibility_action_sender
        .send(action_request(
            input_id,
            Action::SetValue,
            Some(ActionData::Value("new".into())),
        ))
        .unwrap();
    assert!(handler.drain_accessibility_actions());
    assert_eq!(controller.text(), "new");
    assert_eq!(text_changes.lock().unwrap().as_slice(), ["changed"]);
}
