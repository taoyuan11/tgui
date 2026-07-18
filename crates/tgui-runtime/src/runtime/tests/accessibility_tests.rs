use super::*;
use crate::ui::widget::{
    ColorPicker, DataGridCellAction, DataGridSelectionChange, DataGridSelectionMode,
    DataGridSelectionTrigger, DatePicker, List, ListItem, ListItemAction, ListSelectionChange,
    ListSelectionMode, ListSelectionTrigger, NumberInput, OverlayFlipPolicy, OverlayLayer, Portal,
    Show, TimePicker, Tree, TreeNode, TreeNodeAction, TreeSelectionChange, TreeSelectionMode,
    TreeSelectionTrigger, Upload, UploadFile, UploadFileId, UploadStatus,
};
use accesskit::{Action, ActionData, ActionRequest, Node, Role, Toggled, TreeId};
use chrono::{NaiveDate, NaiveTime};

fn accessibility_update(handler: &mut BoundRuntimeHandler<TestVm>) -> accesskit::TreeUpdate {
    handler.accessibility_tree_update_for_test()
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

fn has_reachable_node(update: &accesskit::TreeUpdate, target: accesskit::NodeId) -> bool {
    let mut pending = vec![crate::accessibility::ROOT_NODE_ID];
    let mut visited = Vec::new();
    while let Some(node_id) = pending.pop() {
        if node_id == target {
            return true;
        }
        if visited.contains(&node_id) {
            continue;
        }
        visited.push(node_id);
        if let Some(node) = update
            .nodes
            .iter()
            .find_map(|(id, node)| (*id == node_id).then_some(node))
        {
            pending.extend(node.children().iter().copied());
        }
    }
    false
}

fn node_for_id(update: &accesskit::TreeUpdate, target: accesskit::NodeId) -> &Node {
    update
        .nodes
        .iter()
        .find_map(|(id, node)| (*id == target).then_some(node))
        .expect("accessibility node should exist")
}

fn has_node_id(update: &accesskit::TreeUpdate, target: accesskit::NodeId) -> bool {
    update.nodes.iter().any(|(id, _)| *id == target)
}

fn assert_node_bounds_match_hit(node: &Node, hit: &HitRegion<TestVm>) {
    let expected = hit
        .clip_rect
        .and_then(|clip| hit.rect.intersect(clip))
        .unwrap_or(hit.rect);
    let actual = node.bounds().expect("accessible node bounds");
    let epsilon = 0.01;
    assert!((actual.x0 - expected.x.get() as f64).abs() <= epsilon);
    assert!((actual.y0 - expected.y.get() as f64).abs() <= epsilon);
    assert!((actual.x1 - expected.right().get() as f64).abs() <= epsilon);
    assert!((actual.y1 - expected.bottom().get() as f64).abs() <= epsilon);
}

fn action_request(widget_id: WidgetId, action: Action, data: Option<ActionData>) -> ActionRequest {
    action_request_for_node(
        crate::accessibility::node_id_from_widget(widget_id),
        action,
        data,
    )
}

fn action_request_for_node(
    node_id: accesskit::NodeId,
    action: Action,
    data: Option<ActionData>,
) -> ActionRequest {
    ActionRequest {
        action,
        target_tree: TreeId::ROOT,
        target_node: node_id,
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
fn accessibility_tree_maps_input_controls_to_base_interactive_roles() {
    let invalidation = InvalidationSignal::new();
    let date_controller = TextController::new_legacy("2026-06-06");
    let time_controller = TextController::new_legacy("09:30");
    let number_controller = TextController::new_legacy("24");
    let upload_files = vec![UploadFile {
        id: UploadFileId::new("demo:report"),
        path: "report.pdf".into(),
        name: "report.pdf".to_string(),
        size_bytes: Some(512_000),
        status: UploadStatus::Uploading { progress: 0.5 },
    }];
    let date: Element<TestVm> = DatePicker::new(
        date_controller,
        Some(NaiveDate::from_ymd_opt(2026, 6, 6).unwrap()),
        NaiveDate::from_ymd_opt(2026, 6, 1).unwrap(),
    )
    .into();
    let time: Element<TestVm> =
        TimePicker::new(time_controller, NaiveTime::from_hms_opt(9, 30, 0)).into();
    let number: Element<TestVm> = NumberInput::new(number_controller, Some(24.0)).into();
    let color: Element<TestVm> = ColorPicker::new(Color::hexa(0x3366CCFF)).into();
    let upload: Element<TestVm> = Upload::new(upload_files).into();
    let tree =
        WidgetTree::new(Flex::new(Axis::Vertical).child([date, time, number, color, upload]));
    let mut handler = test_handler(Some(tree), invalidation);

    let update = accessibility_update(&mut handler);

    let text_input_count = update
        .nodes
        .iter()
        .filter(|(_, node)| node.role() == Role::TextInput)
        .count();
    assert!(
        text_input_count >= 3,
        "DatePicker, TimePicker and NumberInput should expose editable text inputs"
    );
    let color_button = update
        .nodes
        .iter()
        .find_map(|(_, node)| {
            (node.role() == Role::Button && node.label() == Some("#3366CCFF")).then_some(node)
        })
        .expect("ColorPicker trigger should be a labeled button");
    assert!(color_button.supports_action(Action::Click));
    let choose_button = update
        .nodes
        .iter()
        .find_map(|(_, node)| {
            (node.role() == Role::Button && node.label() == Some("Choose files")).then_some(node)
        })
        .expect("Upload should expose its file chooser button");
    assert!(choose_button.supports_action(Action::Click));
    assert!(update.nodes.iter().any(|(_, node)| {
        node.role() == Role::ProgressIndicator && node.numeric_value() == Some(0.5)
    }));
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
    let _ = accessibility_update(&mut handler);

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
    let _ = accessibility_update(&mut handler);

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

#[test]
fn accessibility_list_click_uses_pointer_selection_semantics_without_click_clock() {
    let invalidation = InvalidationSignal::new();
    let triggers = Arc::new(Mutex::new(Vec::<ListSelectionTrigger>::new()));
    let triggers_for_command = Arc::clone(&triggers);
    let actions = Arc::new(AtomicUsize::new(0));
    let actions_for_command = Arc::clone(&actions);
    let tree = WidgetTree::new(
        List::<usize, TestVm>::new(
            (0..32)
                .map(|index| ListItem::keyed(format!("row-{index}"), index))
                .collect(),
            |context| Text::new(context.item.to_string()).into(),
        )
        .selection_mode(ListSelectionMode::Single)
        .on_selection_change(ValueCommand::new(
            move |_vm, change: ListSelectionChange| {
                triggers_for_command.lock().unwrap().push(change.trigger);
            },
        ))
        .on_item_action(ValueCommand::new(move |_vm, _action: ListItemAction| {
            actions_for_command.fetch_add(1, Ordering::SeqCst);
        }))
        .item_layout(crate::ui::widget::ItemLayout::Fixed {
            item_extent: dp(28.0),
            spacing: Dp::ZERO,
            overscan: 0,
        })
        .size(dp(220.0), dp(84.0)),
    );
    let mut handler = test_handler(Some(tree), invalidation);
    let row_id = handler
        .computed_scene()
        .hit_regions
        .iter()
        .find_map(|region| match &region.interaction {
            HitInteraction::ListItem { id, state, .. } if state.key == WidgetKey::from("row-0") => {
                Some(*id)
            }
            _ => None,
        })
        .expect("materialized List row should expose a hit target");
    let update = accessibility_update(&mut handler);
    let row_node = node_for(&update, row_id);
    assert_eq!(row_node.role(), Role::ListBoxOption);
    assert!(row_node.supports_action(Action::Click));

    for expected_clicks in 1..=2 {
        handler
            .accessibility_action_sender
            .send(action_request(row_id, Action::Click, None))
            .unwrap();
        assert!(handler.drain_accessibility_actions());
        assert!(handler.pending_click.is_none());
        assert_eq!(triggers.lock().unwrap().len(), expected_clicks);
    }
    assert_eq!(
        triggers.lock().unwrap().as_slice(),
        [ListSelectionTrigger::Click, ListSelectionTrigger::Click]
    );
    assert_eq!(actions.load(Ordering::SeqCst), 0);
}

#[test]
fn accessibility_tree_click_ignores_stale_disclosure_cursor() {
    let invalidation = InvalidationSignal::new();
    let selections = Arc::new(AtomicUsize::new(0));
    let selections_for_command = Arc::clone(&selections);
    let expands = Arc::new(AtomicUsize::new(0));
    let expands_for_command = Arc::clone(&expands);
    let checks = Arc::new(AtomicUsize::new(0));
    let checks_for_command = Arc::clone(&checks);
    let tree = WidgetTree::new(
        Tree::<usize, TestVm>::new(
            vec![TreeNode::keyed("root", 0).child(TreeNode::keyed("child", 1))],
            |context| Text::new(context.item.to_string()).into(),
        )
        .expanded_keys(vec![WidgetKey::from("root")])
        .selection_mode(TreeSelectionMode::Single)
        .checkable(true)
        .on_selection_change(ValueCommand::new(
            move |_vm, change: TreeSelectionChange| {
                assert_eq!(change.trigger, TreeSelectionTrigger::Click);
                selections_for_command.fetch_add(1, Ordering::SeqCst);
            },
        ))
        .on_expand_change(ValueCommand::new(move |_vm, _change| {
            expands_for_command.fetch_add(1, Ordering::SeqCst);
        }))
        .on_check_change(ValueCommand::new(move |_vm, _change| {
            checks_for_command.fetch_add(1, Ordering::SeqCst);
        }))
        .size(dp(240.0), dp(120.0)),
    );
    let mut handler = test_handler(Some(tree), invalidation);
    let (row_id, stale_disclosure_cursor) = {
        let computed = handler.computed_scene();
        let row_id = computed
            .hit_regions
            .iter()
            .find_map(|region| match &region.interaction {
                HitInteraction::TreeNode { id, state, .. }
                    if state.key == WidgetKey::from("root") =>
                {
                    Some(*id)
                }
                _ => None,
            })
            .expect("root row should be materialized");
        let disclosure = computed
            .hit_regions
            .iter()
            .find_map(|region| match &region.interaction {
                HitInteraction::TreeDisclosure { id, .. } if *id == row_id => Some(Point::new(
                    region.rect.x + region.rect.width * 0.5,
                    region.rect.y + region.rect.height * 0.5,
                )),
                _ => None,
            })
            .expect("root row should expose a disclosure hit slot");
        (row_id, disclosure)
    };
    handler.cursor_position = Some(stale_disclosure_cursor);
    let _ = accessibility_update(&mut handler);
    handler
        .accessibility_action_sender
        .send(action_request(row_id, Action::Click, None))
        .unwrap();
    assert!(handler.drain_accessibility_actions());

    assert_eq!(selections.load(Ordering::SeqCst), 1);
    assert_eq!(expands.load(Ordering::SeqCst), 0);
    assert_eq!(checks.load(Ordering::SeqCst), 0);
    assert!(handler.pending_click.is_none());
}

#[test]
fn accessibility_tree_clicks_do_not_enter_pointer_double_click() {
    let invalidation = InvalidationSignal::new();
    let selections = Arc::new(AtomicUsize::new(0));
    let selections_for_command = Arc::clone(&selections);
    let actions = Arc::new(AtomicUsize::new(0));
    let actions_for_command = Arc::clone(&actions);
    let tree = WidgetTree::new(
        Tree::<usize, TestVm>::new(
            (0..32)
                .map(|index| TreeNode::keyed(format!("row-{index}"), index))
                .collect(),
            |context| Text::new(context.item.to_string()).into(),
        )
        .selection_mode(TreeSelectionMode::Single)
        .on_selection_change(ValueCommand::new(move |_vm, _change| {
            selections_for_command.fetch_add(1, Ordering::SeqCst);
        }))
        .on_node_action(ValueCommand::new(move |_vm, _action: TreeNodeAction| {
            actions_for_command.fetch_add(1, Ordering::SeqCst);
        }))
        .size(dp(220.0), dp(84.0)),
    );
    let mut handler = test_handler(Some(tree), invalidation);
    let row_id = handler
        .computed_scene()
        .hit_regions
        .iter()
        .find_map(|region| match &region.interaction {
            HitInteraction::TreeNode { id, .. } => Some(*id),
            _ => None,
        })
        .expect("materialized Tree row should expose a hit target");
    let update = accessibility_update(&mut handler);
    let row_node = node_for(&update, row_id);
    assert_eq!(row_node.role(), Role::TreeItem);
    assert!(row_node.supports_action(Action::Click));

    let pending_deadline = Instant::now() + Duration::from_secs(60);
    let pending_position = Point::new(dp(17.0), dp(19.0));
    let pending_target = crate::runtime::HoverTargetId::Widget(row_id);
    handler.cursor_position = Some(pending_position);
    handler.pending_click = Some(crate::runtime::PendingClick {
        target_id: pending_target,
        deadline: pending_deadline,
        position: pending_position,
        command: None,
        splitter: None,
    });

    for expected_clicks in 1..=2 {
        handler
            .accessibility_action_sender
            .send(action_request(row_id, Action::Click, None))
            .unwrap();
        assert!(handler.drain_accessibility_actions());
        let pending = handler
            .pending_click
            .as_ref()
            .expect("accessibility Click must preserve the pointer click clock");
        assert!(pending.target_id == pending_target);
        assert_eq!(pending.deadline, pending_deadline);
        assert_eq!(pending.position, pending_position);
        assert!(pending.command.is_none());
        assert!(pending.splitter.is_none());
        assert_eq!(selections.load(Ordering::SeqCst), expected_clicks);
    }
    assert_eq!(actions.load(Ordering::SeqCst), 0);
}

#[test]
fn accessibility_data_grid_clicks_ignore_cursor_and_pointer_double_click() {
    let invalidation = InvalidationSignal::new();
    let selections = Arc::new(Mutex::new(
        Vec::<(WidgetKey, DataGridSelectionTrigger)>::new(),
    ));
    let selections_for_command = Arc::clone(&selections);
    let actions = Arc::new(AtomicUsize::new(0));
    let actions_for_command = Arc::clone(&actions);
    let columns = vec![DataGridColumn::new(
        "name",
        "Name".to_string(),
        |context: crate::ui::widget::DataGridCellContext<usize>| {
            Text::new(context.row.to_string()).into()
        },
    )
    .width(dp(180.0))];
    let tree = WidgetTree::new(
        DataGrid::new(
            (0..32)
                .map(|index| DataGridRow::keyed(format!("row-{index}"), index))
                .collect(),
            columns,
        )
        .selection_mode(DataGridSelectionMode::Single)
        .on_selection_change(ValueCommand::new(
            move |_vm, change: DataGridSelectionChange| {
                selections_for_command
                    .lock()
                    .unwrap()
                    .push((change.changed_key.unwrap(), change.trigger));
            },
        ))
        .on_cell_action(ValueCommand::new(
            move |_vm, _action: DataGridCellAction| {
                actions_for_command.fetch_add(1, Ordering::SeqCst);
            },
        ))
        .row_height(dp(28.0))
        .overscan(0)
        .size(dp(200.0), dp(112.0)),
    );
    let mut handler = test_handler(Some(tree), invalidation);
    let (target_id, stale_other_row_cursor) = {
        let computed = handler.computed_scene();
        let mut rows = computed
            .hit_regions
            .iter()
            .filter_map(|region| match &region.interaction {
                HitInteraction::DataGridCell { id, state, .. } => Some((
                    *id,
                    state.row_key.clone(),
                    Point::new(
                        region.rect.x + region.rect.width * 0.5,
                        region.rect.y + region.rect.height * 0.5,
                    ),
                )),
                _ => None,
            });
        let target = rows.next().expect("first DataGrid row should materialize");
        let other = rows
            .find(|(_, key, _)| key != &target.1)
            .expect("another DataGrid row should materialize");
        (target.0, other.2)
    };
    handler.cursor_position = Some(stale_other_row_cursor);
    let update = accessibility_update(&mut handler);
    let cell_node = node_for(&update, target_id);
    assert_eq!(cell_node.role(), Role::GridCell);
    assert!(cell_node.supports_action(Action::Click));

    for expected_clicks in 1..=2 {
        handler
            .accessibility_action_sender
            .send(action_request(target_id, Action::Click, None))
            .unwrap();
        assert!(handler.drain_accessibility_actions());
        assert!(handler.pending_click.is_none());
        assert_eq!(selections.lock().unwrap().len(), expected_clicks);
    }
    assert!(selections
        .lock()
        .unwrap()
        .iter()
        .all(|(_, trigger)| *trigger == DataGridSelectionTrigger::Click));
    assert_eq!(actions.load(Ordering::SeqCst), 0);
}

#[test]
fn accessibility_click_does_not_activate_old_focus_for_disabled_target() {
    let invalidation = InvalidationSignal::new();
    let button_clicks = Arc::new(AtomicUsize::new(0));
    let clicks_for_button = Arc::clone(&button_clicks);
    let button: Element<TestVm> = Button::new("Active")
        .on_click(Command::new(move |_vm| {
            clicks_for_button.fetch_add(1, Ordering::SeqCst);
        }))
        .size(dp(120.0), dp(32.0))
        .into();
    let button_id = button.id;
    let disabled_tree: Element<TestVm> = Tree::<usize, TestVm>::new(
        vec![TreeNode::keyed("disabled", 0).disable(true)],
        |context| Text::new(context.item.to_string()).into(),
    )
    .size(dp(180.0), dp(48.0))
    .into();
    let tree = WidgetTree::new(Flex::new(Axis::Vertical).child([button, disabled_tree]));
    let mut handler = test_handler(Some(tree), invalidation);
    let update = accessibility_update(&mut handler);
    let disabled_id = update
        .nodes
        .iter()
        .find_map(|(node_id, node)| {
            (node.role() == Role::TreeItem && node.is_disabled())
                .then(|| crate::accessibility::widget_id_from_node(*node_id))
                .flatten()
        })
        .expect("disabled materialized Tree row should remain in the accessibility tree");

    handler
        .accessibility_action_sender
        .send(action_request(button_id, Action::Focus, None))
        .unwrap();
    assert!(handler.drain_accessibility_actions());
    assert_eq!(handler.focused_widget_id(), Some(button_id));

    handler
        .accessibility_action_sender
        .send(action_request(disabled_id, Action::Click, None))
        .unwrap();
    assert!(!handler.drain_accessibility_actions());
    assert_eq!(button_clicks.load(Ordering::SeqCst), 0);
    assert_eq!(handler.focused_widget_id(), Some(button_id));
    assert!(handler.pending_click.is_none());
}

#[test]
fn accessibility_click_rejects_disabled_virtual_list_and_data_grid_targets() {
    #[derive(Clone, Copy, Debug)]
    enum DisabledVirtualKind {
        List,
        DataGrid,
    }

    for kind in [DisabledVirtualKind::List, DisabledVirtualKind::DataGrid] {
        let invalidation = InvalidationSignal::new();
        let button_clicks = Arc::new(AtomicUsize::new(0));
        let clicks_for_button = Arc::clone(&button_clicks);
        let button: Element<TestVm> = Button::new("Active")
            .on_click(Command::new(move |_vm| {
                clicks_for_button.fetch_add(1, Ordering::SeqCst);
            }))
            .size(dp(120.0), dp(32.0))
            .into();
        let button_id = button.id;
        let selections = Arc::new(AtomicUsize::new(0));
        let actions = Arc::new(AtomicUsize::new(0));
        let disabled_virtual: Element<TestVm> = match kind {
            DisabledVirtualKind::List => {
                let selections_for_command = Arc::clone(&selections);
                let actions_for_command = Arc::clone(&actions);
                List::<usize, TestVm>::new(
                    vec![ListItem::keyed("disabled", 0).disable(true)],
                    |context| Text::new(context.item.to_string()).into(),
                )
                .on_selection_change(ValueCommand::new(move |_vm, _change| {
                    selections_for_command.fetch_add(1, Ordering::SeqCst);
                }))
                .on_item_action(ValueCommand::new(move |_vm, _action| {
                    actions_for_command.fetch_add(1, Ordering::SeqCst);
                }))
                .size(dp(180.0), dp(48.0))
                .into()
            }
            DisabledVirtualKind::DataGrid => {
                let selections_for_command = Arc::clone(&selections);
                let actions_for_command = Arc::clone(&actions);
                let columns = vec![DataGridColumn::new(
                    "name",
                    "Name".to_string(),
                    |context: crate::ui::widget::DataGridCellContext<usize>| {
                        Text::new(context.row.to_string()).into()
                    },
                )];
                DataGrid::new(
                    vec![DataGridRow::keyed("disabled", 0).disable(true)],
                    columns,
                )
                .on_selection_change(ValueCommand::new(move |_vm, _change| {
                    selections_for_command.fetch_add(1, Ordering::SeqCst);
                }))
                .on_cell_action(ValueCommand::new(move |_vm, _action| {
                    actions_for_command.fetch_add(1, Ordering::SeqCst);
                }))
                .size(dp(180.0), dp(88.0))
                .into()
            }
        };
        let tree = WidgetTree::new(Flex::new(Axis::Vertical).child([button, disabled_virtual]));
        let mut handler = test_handler_with_config(
            TestVm,
            Some(tree),
            invalidation,
            test_config_with_size(220.0, 180.0),
        );
        let expected_role = match kind {
            DisabledVirtualKind::List => Role::ListBoxOption,
            DisabledVirtualKind::DataGrid => Role::GridCell,
        };
        let update = accessibility_update(&mut handler);
        let disabled_id = update
            .nodes
            .iter()
            .find_map(|(node_id, node)| {
                (node.role() == expected_role && node.is_disabled())
                    .then(|| crate::accessibility::widget_id_from_node(*node_id))
                    .flatten()
            })
            .unwrap_or_else(|| panic!("disabled {kind:?} target should be materialized"));

        handler
            .accessibility_action_sender
            .send(action_request(button_id, Action::Focus, None))
            .unwrap();
        assert!(handler.drain_accessibility_actions());
        assert_eq!(handler.focused_widget_id(), Some(button_id));

        handler
            .accessibility_action_sender
            .send(action_request(disabled_id, Action::Click, None))
            .unwrap();
        assert!(!handler.drain_accessibility_actions());
        assert_eq!(button_clicks.load(Ordering::SeqCst), 0, "{kind:?}");
        assert_eq!(selections.load(Ordering::SeqCst), 0, "{kind:?}");
        assert_eq!(actions.load(Ordering::SeqCst), 0, "{kind:?}");
        assert_eq!(handler.focused_widget_id(), Some(button_id), "{kind:?}");
        assert!(handler.pending_click.is_none(), "{kind:?}");
    }
}

#[test]
fn accessibility_click_rejects_target_outside_active_focus_trap() {
    let invalidation = InvalidationSignal::new();
    let inside_clicks = Arc::new(AtomicUsize::new(0));
    let inside_for_command = Arc::clone(&inside_clicks);
    let inside: Element<TestVm> = Button::new("Inside")
        .on_click(Command::new(move |_vm| {
            inside_for_command.fetch_add(1, Ordering::SeqCst);
        }))
        .size(dp(80.0), dp(30.0))
        .into();
    let inside_id = inside.id;
    let outside_clicks = Arc::new(AtomicUsize::new(0));
    let outside_for_command = Arc::clone(&outside_clicks);
    let outside: Element<TestVm> = Button::new("Outside")
        .on_click(Command::new(move |_vm| {
            outside_for_command.fetch_add(1, Ordering::SeqCst);
        }))
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
    let _ = accessibility_update(&mut handler);

    handler
        .accessibility_action_sender
        .send(action_request(inside_id, Action::Focus, None))
        .unwrap();
    assert!(handler.drain_accessibility_actions());
    assert_eq!(handler.focused_widget_id(), Some(inside_id));

    handler
        .accessibility_action_sender
        .send(action_request(outside_id, Action::Click, None))
        .unwrap();
    assert!(!handler.drain_accessibility_actions());
    assert_eq!(outside_clicks.load(Ordering::SeqCst), 0);
    assert_eq!(inside_clicks.load(Ordering::SeqCst), 0);
    assert_eq!(handler.focused_widget_id(), Some(inside_id));
}

#[test]
fn accessibility_click_allows_target_inside_portal_focus_trap() {
    let invalidation = InvalidationSignal::new();
    let overlay_clicks = Arc::new(AtomicUsize::new(0));
    let clicks_for_overlay = Arc::clone(&overlay_clicks);
    let overlay_button: Element<TestVm> = Button::new("Overlay action")
        .on_click(Command::new(move |_vm| {
            clicks_for_overlay.fetch_add(1, Ordering::SeqCst);
        }))
        .size(dp(100.0), dp(30.0))
        .into();
    let overlay_button_id = overlay_button.id;
    let portal: Element<TestVm> = Portal::new(overlay_button)
        .anchor(Rect::new(dp(20.0), dp(20.0), dp(1.0), dp(1.0)))
        .focus_scope(FocusScopeOptions::new().trap(true))
        .into();
    let outside: Element<TestVm> = Button::new("Outside")
        .size(dp(80.0), dp(30.0))
        .position_absolute()
        .top(dp(70.0))
        .into();
    let outside_id = outside.id;
    let tree = WidgetTree::new(Stack::new().child([outside, portal]));
    let mut handler = test_handler(Some(tree), invalidation);

    {
        let computed = handler.computed_scene();
        let trap = computed
            .focus_scopes
            .iter()
            .rev()
            .find(|scope| scope.active && scope.options.is_trap())
            .expect("Portal should install an active focus trap");
        let overlay_hit = computed
            .overlay_hit_regions
            .iter()
            .find(|region| {
                matches!(
                    &region.interaction,
                    HitInteraction::Widget { id, .. } if *id == overlay_button_id
                )
            })
            .expect("Portal button should expose an overlay hit target");
        assert!(overlay_hit.scope_path.starts_with(&trap.path));
        assert!(overlay_hit.focus.as_ref().is_some_and(|focus| {
            focus.widget_id == overlay_button_id && focus.scope_path.starts_with(&trap.path)
        }));
    }

    let update = accessibility_update(&mut handler);
    let overlay_button_node_id = update
        .nodes
        .iter()
        .find_map(|(id, node)| {
            (node.role() == Role::Button && node.label() == Some("Overlay action")).then_some(*id)
        })
        .expect("Portal trap button should be published");
    assert!(has_reachable_node(&update, overlay_button_node_id));
    assert!(!has_node(&update, outside_id));

    handler
        .accessibility_action_sender
        .send(action_request(overlay_button_id, Action::Click, None))
        .unwrap();
    assert!(!handler.drain_accessibility_actions());
    assert_eq!(overlay_clicks.load(Ordering::SeqCst), 0);

    handler
        .accessibility_action_sender
        .send(action_request_for_node(
            overlay_button_node_id,
            Action::Click,
            None,
        ))
        .unwrap();
    assert!(handler.drain_accessibility_actions());
    assert_eq!(overlay_clicks.load(Ordering::SeqCst), 1);
    assert_eq!(handler.focused_widget_id(), Some(overlay_button_id));
    assert_eq!(
        accessibility_update(&mut handler).focus,
        overlay_button_node_id
    );
    assert!(handler.pending_click.is_none());
}

#[test]
fn accessibility_tree_exposes_same_window_portal_controls_and_removes_them_when_closed() {
    let context = ViewModelContext::for_benchmarks();
    let invalidation = context.invalidation().clone();
    let open = context.state(true);
    let portal_clicks = Arc::new(AtomicUsize::new(0));
    let clicks_for_command = Arc::clone(&portal_clicks);
    let portal_button: Element<TestVm> = Button::new("Portal action")
        .on_click(Command::new(move |_vm| {
            clicks_for_command.fetch_add(1, Ordering::SeqCst);
        }))
        .size(dp(120.0), dp(30.0))
        .into();
    let portal_button_id = portal_button.id;
    let controller = TextController::new_legacy("Portal value");
    let portal_input: Element<TestVm> = Input::new(controller.clone())
        .size(dp(160.0), dp(32.0))
        .into();
    let portal_input_id = portal_input.id;
    let portal: Element<TestVm> =
        Portal::new(Flex::new(Axis::Vertical).child([portal_button, portal_input]))
            .open(open.signal())
            .anchor(Rect::new(dp(24.0), dp(18.0), dp(1.0), dp(1.0)))
            .into();
    let mut handler = test_handler(
        Some(WidgetTree::new_legacy(Stack::new().child(portal))),
        invalidation,
    );

    let (portal_button_hit, portal_input_hit) = {
        let computed = handler.computed_scene();
        let button_hit = computed
            .overlay_hit_regions
            .iter()
            .find(|region| {
                matches!(
                    &region.interaction,
                    HitInteraction::Widget { id, .. } if *id == portal_button_id
                )
            })
            .expect("Portal button hit")
            .clone();
        let input_hit = computed
            .overlay_hit_regions
            .iter()
            .find(|region| {
                matches!(
                    &region.interaction,
                    HitInteraction::TextInput { id, .. } if *id == portal_input_id
                )
            })
            .expect("Portal input hit")
            .clone();
        (button_hit, input_hit)
    };

    let opened = accessibility_update(&mut handler);
    let portal_button_node_id = opened
        .nodes
        .iter()
        .find_map(|(id, node)| {
            (node.role() == Role::Button && node.label() == Some("Portal action")).then_some(*id)
        })
        .expect("same-window Portal button should be present");
    let portal_input_node_id = opened
        .nodes
        .iter()
        .find_map(|(id, node)| {
            (node.role() == Role::TextInput && node.value() == Some("Portal value")).then_some(*id)
        })
        .expect("same-window Portal input should be present");
    assert_ne!(
        portal_button_node_id,
        crate::accessibility::node_id_from_widget(portal_button_id)
    );
    assert_ne!(
        portal_input_node_id,
        crate::accessibility::node_id_from_widget(portal_input_id)
    );
    let portal_button_reachable = has_reachable_node(&opened, portal_button_node_id);
    let portal_input_reachable = has_reachable_node(&opened, portal_input_node_id);
    assert!(
        portal_button_reachable && portal_input_reachable,
        "same-window Portal controls are rendered and hittable but missing from the AccessKit tree: button={portal_button_reachable}, input={portal_input_reachable}"
    );
    let button_node = node_for_id(&opened, portal_button_node_id);
    assert_eq!(button_node.role(), Role::Button);
    assert_eq!(button_node.label(), Some("Portal action"));
    assert!(button_node.supports_action(Action::Click));
    assert!(button_node.bounds().is_some());
    assert_node_bounds_match_hit(button_node, &portal_button_hit);
    let input_node = node_for_id(&opened, portal_input_node_id);
    assert_eq!(input_node.role(), Role::TextInput);
    assert_eq!(input_node.value(), Some("Portal value"));
    assert!(input_node.supports_action(Action::SetValue));
    assert!(input_node.bounds().is_some());
    assert_node_bounds_match_hit(input_node, &portal_input_hit);

    handler
        .accessibility_action_sender
        .send(ActionRequest {
            action: Action::Click,
            target_tree: TreeId(accesskit::Uuid::from_u128(1)),
            target_node: portal_button_node_id,
            data: None,
        })
        .unwrap();
    assert!(!handler.drain_accessibility_actions());
    assert_eq!(portal_clicks.load(Ordering::SeqCst), 0);

    handler
        .accessibility_action_sender
        .send(action_request_for_node(
            portal_button_node_id,
            Action::Click,
            None,
        ))
        .unwrap();
    assert!(handler.drain_accessibility_actions());
    assert_eq!(portal_clicks.load(Ordering::SeqCst), 1);
    let focused = accessibility_update(&mut handler);
    assert_eq!(focused.focus, portal_button_node_id);

    handler
        .accessibility_action_sender
        .send(action_request_for_node(
            portal_input_node_id,
            Action::SetValue,
            Some(ActionData::Value("Updated Portal value".into())),
        ))
        .unwrap();
    assert!(handler.drain_accessibility_actions());
    assert_eq!(controller.text(), "Updated Portal value");

    #[cfg(feature = "bench-support")]
    crate::runtime::action_stats::reset();
    open.set(false);
    handler.request_redraw_if_dirty(Instant::now());
    #[cfg(feature = "bench-support")]
    let close_actions = crate::runtime::action_stats::snapshot();
    #[cfg(feature = "bench-support")]
    eprintln!("portal close invalidation actions: {close_actions:?}");
    let closed = accessibility_update(&mut handler);
    let closed_scene = &handler
        .cached_scene
        .as_ref()
        .expect("closed Portal should retain the cache shell")
        .computed;
    assert!(
        !has_node_id(&closed, portal_button_node_id),
        "closed Portal leaked button node: fragments={}, portal_fragments={}, entries={}",
        closed_scene.accessibility_fragments.len(),
        closed_scene.portal_overlay_counts.accessibility_fragments,
        closed_scene.portal_entries.len(),
    );
    assert!(!has_node_id(&closed, portal_input_node_id));
    assert!(!has_reachable_node(&closed, portal_button_node_id));
    assert!(!has_reachable_node(&closed, portal_input_node_id));

    handler
        .accessibility_action_sender
        .send(action_request_for_node(
            portal_button_node_id,
            Action::Click,
            None,
        ))
        .unwrap();
    assert!(!handler.drain_accessibility_actions());
    assert_eq!(portal_clicks.load(Ordering::SeqCst), 1);

    open.set(true);
    handler.request_redraw_if_dirty(Instant::now());
    let reopened = accessibility_update(&mut handler);
    let reopened_button_id = reopened
        .nodes
        .iter()
        .find_map(|(id, node)| {
            (node.role() == Role::Button && node.label() == Some("Portal action")).then_some(*id)
        })
        .expect("reopened Portal button should be present");
    assert_ne!(reopened_button_id, portal_button_node_id);
}

#[test]
fn accessibility_portal_occurrences_keep_stable_distinct_node_ids_and_retire_closed_routes() {
    let context = ViewModelContext::for_benchmarks();
    let invalidation = context.invalidation().clone();
    let first_open = context.state(true);
    let second_open = context.state(true);
    let clicks = Arc::new(AtomicUsize::new(0));
    let clicks_for_command = Arc::clone(&clicks);
    let blurs = Arc::new(AtomicUsize::new(0));
    let blurs_for_command = Arc::clone(&blurs);
    let shared_content: Element<TestVm> = Button::new("Shared Portal action")
        .on_click(Command::new(move |_vm| {
            clicks_for_command.fetch_add(1, Ordering::SeqCst);
        }))
        .on_blur(Command::new(move |_vm| {
            blurs_for_command.fetch_add(1, Ordering::SeqCst);
        }))
        .size(dp(140.0), dp(30.0))
        .into();
    let shared_content_id = shared_content.id;
    let portals: [Element<TestVm>; 2] = [
        Portal::new(shared_content.clone())
            .open(first_open.signal())
            .anchor(Rect::new(dp(16.0), dp(12.0), dp(1.0), dp(1.0)))
            .into(),
        Portal::new(shared_content)
            .open(second_open.signal())
            .anchor(Rect::new(dp(16.0), dp(64.0), dp(1.0), dp(1.0)))
            .into(),
    ];
    let mut handler = test_handler(
        Some(WidgetTree::new_legacy(Stack::new().child(portals))),
        invalidation,
    );

    let opened = accessibility_update(&mut handler);
    let mut occurrences = opened
        .nodes
        .iter()
        .filter_map(|(id, node)| {
            (node.role() == Role::Button && node.label() == Some("Shared Portal action"))
                .then(|| (*id, node.bounds().expect("Portal button bounds")))
        })
        .collect::<Vec<_>>();
    occurrences.sort_by(|left, right| left.1.y0.total_cmp(&right.1.y0));
    assert_eq!(occurrences.len(), 2);
    let first_node_id = occurrences[0].0;
    let second_node_id = occurrences[1].0;
    assert_ne!(first_node_id, second_node_id);

    handler
        .accessibility_action_sender
        .send(action_request_for_node(first_node_id, Action::Focus, None))
        .unwrap();
    assert!(handler.drain_accessibility_actions());
    assert_eq!(handler.focused_widget_id(), Some(shared_content_id));

    first_open.set(false);
    handler.request_redraw_if_dirty(Instant::now());
    let first_closed = accessibility_update(&mut handler);
    assert_eq!(handler.focused_widget_id(), None);
    assert_eq!(first_closed.focus, crate::accessibility::ROOT_NODE_ID);
    assert_eq!(blurs.load(Ordering::SeqCst), 1);
    assert!(!handler.activate_focused_widget(true, false));
    assert_eq!(clicks.load(Ordering::SeqCst), 0);
    let remaining = first_closed
        .nodes
        .iter()
        .find_map(|(id, node)| {
            (node.role() == Role::Button && node.label() == Some("Shared Portal action"))
                .then_some(*id)
        })
        .expect("second Portal occurrence should remain");
    assert_eq!(remaining, second_node_id);
    assert!(!has_node_id(&first_closed, first_node_id));

    handler
        .accessibility_action_sender
        .send(action_request_for_node(first_node_id, Action::Click, None))
        .unwrap();
    assert!(!handler.drain_accessibility_actions());
    assert_eq!(clicks.load(Ordering::SeqCst), 0);

    handler
        .accessibility_action_sender
        .send(action_request_for_node(second_node_id, Action::Click, None))
        .unwrap();
    assert!(handler.drain_accessibility_actions());
    assert_eq!(clicks.load(Ordering::SeqCst), 1);

    first_open.set(true);
    handler.request_redraw_if_dirty(Instant::now());
    let reopened = accessibility_update(&mut handler);
    let mut reopened_ids = reopened
        .nodes
        .iter()
        .filter_map(|(id, node)| {
            (node.role() == Role::Button && node.label() == Some("Shared Portal action"))
                .then_some(*id)
        })
        .collect::<Vec<_>>();
    reopened_ids.sort();
    assert_eq!(reopened_ids.len(), 2);
    assert!(reopened_ids.contains(&second_node_id));
    assert!(!reopened_ids.contains(&first_node_id));
}

#[test]
fn accessibility_portal_node_id_survives_unrelated_sibling_insertion() {
    let context = ViewModelContext::for_benchmarks();
    let invalidation = context.invalidation().clone();
    let prepend = context.state(false);
    let clicks = Arc::new(AtomicUsize::new(0));
    let clicks_for_command = Arc::clone(&clicks);
    let target: Element<TestVm> = Button::new("Stable Portal target")
        .on_click(Command::new(move |_vm| {
            clicks_for_command.fetch_add(1, Ordering::SeqCst);
        }))
        .size(dp(140.0), dp(30.0))
        .into();
    let content = Flex::new(Axis::Vertical)
        .child(Show::new(prepend.signal(), Text::new("prepended")))
        .child(target);
    let portal: Element<TestVm> = Portal::new(content)
        .anchor(Rect::new(dp(20.0), dp(20.0), dp(1.0), dp(1.0)))
        .into();
    let mut handler = test_handler(
        Some(WidgetTree::new_legacy(Stack::new().child(portal))),
        invalidation,
    );

    let before = accessibility_update(&mut handler);
    let stable_node_id = before
        .nodes
        .iter()
        .find_map(|(node_id, node)| {
            (node.role() == Role::Button && node.label() == Some("Stable Portal target"))
                .then_some(*node_id)
        })
        .expect("Portal target should be published");
    handler
        .accessibility_action_sender
        .send(action_request_for_node(stable_node_id, Action::Focus, None))
        .unwrap();
    assert!(handler.drain_accessibility_actions());

    prepend.set(true);
    handler.request_redraw_if_dirty(Instant::now());
    let after = accessibility_update(&mut handler);
    let moved_node_id = after
        .nodes
        .iter()
        .find_map(|(node_id, node)| {
            (node.role() == Role::Button && node.label() == Some("Stable Portal target"))
                .then_some(*node_id)
        })
        .expect("moved Portal target should remain published");
    assert_eq!(moved_node_id, stable_node_id);
    assert_eq!(after.focus, stable_node_id);

    handler
        .accessibility_action_sender
        .send(action_request_for_node(stable_node_id, Action::Click, None))
        .unwrap();
    assert!(handler.drain_accessibility_actions());
    assert_eq!(clicks.load(Ordering::SeqCst), 1);
}

#[test]
fn accessibility_portal_geometry_tracks_inner_scroll_clip_and_culling() {
    let invalidation = InvalidationSignal::new();
    let action: Element<TestVm> = Button::new("Scrolled Portal target")
        .size(dp(120.0), dp(40.0))
        .position_absolute()
        .top(dp(100.0))
        .into();
    let action_id = action.id;
    let content = Stack::new().size(dp(120.0), dp(140.0)).child(action);
    let scroller: Element<TestVm> = Stack::new()
        .size(dp(120.0), dp(60.0))
        .overflow_y(Overflow::Scroll)
        .child(content)
        .into();
    let scroller_id = scroller.id;
    let portal: Element<TestVm> = Portal::new(scroller)
        .anchor(Rect::new(dp(12.0), dp(12.0), dp(1.0), dp(1.0)))
        .viewport_padding(dp(0.0))
        .into();
    let mut handler = test_handler(
        Some(WidgetTree::new(Stack::new().child(portal))),
        invalidation,
    );

    let culled = accessibility_update(&mut handler);
    assert!(!culled
        .nodes
        .iter()
        .any(|(_, node)| node.label() == Some("Scrolled Portal target")));

    handler.set_scroll_offset(scroller_id, Point::new(Dp::ZERO, dp(60.0)));
    let hit = handler
        .computed_scene()
        .overlay_hit_regions
        .iter()
        .find(|hit| hit.interaction.widget_id() == action_id)
        .expect("scrolled target should become hittable")
        .clone();
    let visible = accessibility_update(&mut handler);
    let node = visible
        .nodes
        .iter()
        .find_map(|(_, node)| (node.label() == Some("Scrolled Portal target")).then_some(node))
        .expect("partially clipped target should be exposed");
    assert_node_bounds_match_hit(node, &hit);
    let bounds = node.bounds().expect("clipped bounds");
    assert!(bounds.y1 - bounds.y0 < hit.rect.height.get() as f64);
}

#[test]
fn accessibility_portal_geometry_recollects_reactive_offset_and_scale() {
    let context = ViewModelContext::for_benchmarks();
    let invalidation = context.invalidation().clone();
    let offset = context.state(Point::ZERO);
    let scale = context.state(1.0_f32);
    let action: Element<TestVm> = Button::new("Transformed Portal target")
        .size(dp(100.0), dp(32.0))
        .into();
    let action_id = action.id;
    let transformed: Element<TestVm> = Stack::new()
        .size(dp(100.0), dp(32.0))
        .offset(offset.signal())
        .scale(scale.signal())
        .child(action)
        .into();
    let transformed_id = transformed.id;
    let portal: Element<TestVm> = Portal::new(transformed)
        .anchor(Rect::new(dp(20.0), dp(20.0), dp(1.0), dp(1.0)))
        .viewport_padding(dp(0.0))
        .into();
    let mut handler = test_handler(
        Some(WidgetTree::new_legacy(Stack::new().child(portal))),
        invalidation,
    );

    let before = accessibility_update(&mut handler);
    let node_id = before
        .nodes
        .iter()
        .find_map(|(node_id, node)| {
            (node.label() == Some("Transformed Portal target")).then_some(*node_id)
        })
        .expect("transformed Portal target");
    let before_bounds = node_for_id(&before, node_id)
        .bounds()
        .expect("initial transformed bounds");
    assert!(!handler
        .computed_scene()
        .transform_records
        .contains_key(&transformed_id));

    offset.set(Point::new(dp(28.0), dp(14.0)));
    scale.set(1.25);
    handler.request_redraw_if_dirty(Instant::now());
    let hit = handler
        .computed_scene()
        .overlay_hit_regions
        .iter()
        .find(|hit| hit.interaction.widget_id() == action_id)
        .expect("transformed target hit")
        .clone();
    assert!(!handler
        .computed_scene()
        .transform_records
        .contains_key(&transformed_id));
    let after = accessibility_update(&mut handler);
    let node = node_for_id(&after, node_id);
    assert_node_bounds_match_hit(node, &hit);
    let after_bounds = node.bounds().expect("updated transformed bounds");
    assert!(
        after_bounds.x0 > before_bounds.x0 + 10.0,
        "reactive Portal x bounds should move: before={before_bounds:?}, after={after_bounds:?}, hit={:?}",
        hit.rect
    );
    assert!(
        after_bounds.y0 > before_bounds.y0 + 5.0,
        "reactive Portal y bounds should move: before={before_bounds:?}, after={after_bounds:?}, hit={:?}",
        hit.rect
    );
}

#[test]
fn accessibility_portal_geometry_tracks_non_hit_text_offset_and_scale() {
    let context = ViewModelContext::for_benchmarks();
    let invalidation = context.invalidation().clone();
    let offset = context.state(Point::ZERO);
    let scale = context.state(1.0_f32);
    let text: Element<TestVm> = Text::new("Geometry-only Portal text")
        .size(dp(100.0), dp(32.0))
        .into();
    let text_id = text.id;
    let transformed: Element<TestVm> = Stack::new()
        .size(dp(100.0), dp(32.0))
        .position_absolute()
        .left(dp(50.0))
        .top(dp(30.0))
        .offset(offset.signal())
        .scale(scale.signal())
        .child(text)
        .into();
    let transformed_id = transformed.id;
    let content = Stack::new().size(dp(200.0), dp(100.0)).child(transformed);
    let portal: Element<TestVm> = Portal::new(content)
        .anchor(Rect::new(dp(120.0), dp(60.0), dp(1.0), dp(1.0)))
        .viewport_padding(dp(0.0))
        .into();
    let mut handler = test_handler_with_config(
        TestVm,
        Some(WidgetTree::new_legacy(Stack::new().child(portal))),
        invalidation,
        test_config_with_size(420.0, 260.0),
    );

    assert!(!handler
        .computed_scene()
        .overlay_hit_regions
        .iter()
        .any(|hit| {
            hit.interaction.widget_id() == text_id || hit.interaction.widget_id() == transformed_id
        }));
    let before = accessibility_update(&mut handler);
    let text_node_id = before
        .nodes
        .iter()
        .find_map(|(node_id, node)| {
            (node.role() == Role::TextRun && node.value() == Some("Geometry-only Portal text"))
                .then_some(*node_id)
        })
        .expect("non-hit TextRun should be exposed");
    let wrapper_node_id = before
        .nodes
        .iter()
        .find_map(|(node_id, node)| node.children().contains(&text_node_id).then_some(*node_id))
        .expect("transformed wrapper node");
    let before_text = node_for_id(&before, text_node_id)
        .bounds()
        .expect("initial text bounds");
    let before_wrapper = node_for_id(&before, wrapper_node_id)
        .bounds()
        .expect("initial wrapper bounds");

    offset.set(Point::new(dp(20.0), dp(10.0)));
    scale.set(1.25);
    handler.request_redraw_if_dirty(Instant::now());
    let after = accessibility_update(&mut handler);
    let after_text = node_for_id(&after, text_node_id)
        .bounds()
        .expect("updated text bounds");
    let after_wrapper = node_for_id(&after, wrapper_node_id)
        .bounds()
        .expect("updated wrapper bounds");
    assert!(after_text.x0 > before_text.x0 + 5.0);
    assert!(after_text.y0 > before_text.y0 + 3.0);
    assert!(after_wrapper.x1 - after_wrapper.x0 > before_wrapper.x1 - before_wrapper.x0 + 20.0);
    assert!(after_wrapper.y1 - after_wrapper.y0 > before_wrapper.y1 - before_wrapper.y0 + 6.0);
    assert!(!handler
        .computed_scene()
        .transform_records
        .contains_key(&transformed_id));
}

#[test]
fn accessibility_portal_geometry_culls_and_clips_non_hit_text_in_scroll_view() {
    let invalidation = InvalidationSignal::new();
    let text: Element<TestVm> = Text::new("Scrolled geometry-only text")
        .size(dp(120.0), dp(40.0))
        .position_absolute()
        .top(dp(100.0))
        .into();
    let text_id = text.id;
    let content = Stack::new().size(dp(120.0), dp(140.0)).child(text);
    let scroller: Element<TestVm> = Stack::new()
        .size(dp(120.0), dp(60.0))
        .overflow_y(Overflow::Scroll)
        .child(content)
        .into();
    let scroller_id = scroller.id;
    let portal: Element<TestVm> = Portal::new(scroller)
        .anchor(Rect::new(dp(12.0), dp(12.0), dp(1.0), dp(1.0)))
        .viewport_padding(dp(0.0))
        .into();
    let mut handler = test_handler(
        Some(WidgetTree::new(Stack::new().child(portal))),
        invalidation,
    );

    assert!(!handler
        .computed_scene()
        .overlay_hit_regions
        .iter()
        .any(|hit| hit.interaction.widget_id() == text_id));
    let culled = accessibility_update(&mut handler);
    assert!(!culled.nodes.iter().any(|(_, node)| {
        node.role() == Role::TextRun && node.value() == Some("Scrolled geometry-only text")
    }));

    handler.set_scroll_offset(scroller_id, Point::new(Dp::ZERO, dp(60.0)));
    let visible = accessibility_update(&mut handler);
    let node = visible
        .nodes
        .iter()
        .find_map(|(_, node)| {
            (node.role() == Role::TextRun && node.value() == Some("Scrolled geometry-only text"))
                .then_some(node)
        })
        .expect("partially clipped non-hit TextRun should be exposed");
    let bounds = node.bounds().expect("clipped TextRun bounds");
    assert!(bounds.y1 - bounds.y0 > 0.0);
    assert!(bounds.y1 - bounds.y0 < 40.0);
}

#[test]
fn accessibility_portal_with_duplicate_widget_ids_fails_closed_without_affecting_hits() {
    let invalidation = InvalidationSignal::new();
    let clicks = Arc::new(AtomicUsize::new(0));
    let clicks_for_command = Arc::clone(&clicks);
    let shared: Element<TestVm> = Button::new("Ambiguous cloned target")
        .on_click(Command::new(move |_vm| {
            clicks_for_command.fetch_add(1, Ordering::SeqCst);
        }))
        .size(dp(150.0), dp(30.0))
        .into();
    let shared_id = shared.id;
    let content = Flex::new(Axis::Vertical).child([shared.clone(), shared]);
    let portal: Element<TestVm> = Portal::new(content)
        .anchor(Rect::new(dp(20.0), dp(20.0), dp(1.0), dp(1.0)))
        .into();
    let mut handler = test_handler(
        Some(WidgetTree::new(Stack::new().child(portal))),
        invalidation,
    );

    let computed = handler.computed_scene();
    assert!(computed
        .accessibility_fragments
        .iter()
        .any(|fragment| fragment.has_duplicate_widget_ids));
    assert!(
        computed
            .overlay_hit_regions
            .iter()
            .filter(|hit| hit.interaction.widget_id() == shared_id)
            .count()
            >= 2
    );
    let update = accessibility_update(&mut handler);
    assert!(!update
        .nodes
        .iter()
        .any(|(_, node)| node.label() == Some("Ambiguous cloned target")));

    handler
        .accessibility_action_sender
        .send(action_request(shared_id, Action::Click, None))
        .unwrap();
    assert!(!handler.drain_accessibility_actions());
    assert_eq!(clicks.load(Ordering::SeqCst), 0);
}

#[test]
fn accessibility_portal_duplicate_guard_is_stable_when_only_one_clone_is_materialized() {
    let invalidation = InvalidationSignal::new();
    let first: Element<TestVm> = Button::new("Culled duplicate target")
        .size(dp(120.0), dp(30.0))
        .position_absolute()
        .top(dp(0.0))
        .into();
    let shared_id = first.id;
    let mut second: Element<TestVm> = Button::new("Culled duplicate target")
        .size(dp(120.0), dp(30.0))
        .position_absolute()
        .top(dp(100.0))
        .into();
    second.id = shared_id;
    let tall = Stack::new()
        .size(dp(120.0), dp(140.0))
        .child([first, second]);
    let clipped = Stack::new()
        .size(dp(120.0), dp(40.0))
        .overflow(Overflow::Hidden)
        .child(tall);
    let portal: Element<TestVm> = Portal::new(clipped)
        .anchor(Rect::new(dp(10.0), dp(10.0), dp(1.0), dp(1.0)))
        .into();
    let mut handler = test_handler(
        Some(WidgetTree::new(Stack::new().child(portal))),
        invalidation,
    );

    let computed = handler.computed_scene();
    assert_eq!(
        computed
            .overlay_hit_regions
            .iter()
            .filter(|hit| hit.interaction.widget_id() == shared_id)
            .count(),
        1,
        "only the visible clone should materialize a hit"
    );
    assert!(computed
        .accessibility_fragments
        .iter()
        .any(|fragment| fragment.has_duplicate_widget_ids));
    let update = accessibility_update(&mut handler);
    assert!(!update
        .nodes
        .iter()
        .any(|(_, node)| node.label() == Some("Culled duplicate target")));
}

#[test]
fn accessibility_duplicate_portal_owner_retires_the_old_route_and_fails_closed() {
    let context = ViewModelContext::for_benchmarks();
    let invalidation = context.invalidation().clone();
    let show_clone = context.state(false);
    let clicks = Arc::new(AtomicUsize::new(0));
    let clicks_for_command = Arc::clone(&clicks);
    let button: Element<TestVm> = Button::new("Cloned Portal owner")
        .on_click(Command::new(move |_vm| {
            clicks_for_command.fetch_add(1, Ordering::SeqCst);
        }))
        .size(dp(150.0), dp(30.0))
        .into();
    let button_id = button.id;
    let portal: Element<TestVm> = Portal::new(button)
        .anchor(Rect::new(dp(20.0), dp(20.0), dp(1.0), dp(1.0)))
        .into();
    let root = Stack::new()
        .child(portal.clone())
        .child(Show::new(show_clone.signal(), portal));
    let mut handler = test_handler(Some(WidgetTree::new_legacy(root)), invalidation);

    let single = accessibility_update(&mut handler);
    let old_node_id = single
        .nodes
        .iter()
        .find_map(|(node_id, node)| {
            (node.role() == Role::Button && node.label() == Some("Cloned Portal owner"))
                .then_some(*node_id)
        })
        .expect("single Portal occurrence should be published");

    show_clone.set(true);
    handler.request_redraw_if_dirty(Instant::now());
    assert!(
        handler
            .computed_scene()
            .overlay_hit_regions
            .iter()
            .filter(|hit| hit.interaction.widget_id() == button_id)
            .count()
            >= 2
    );
    let duplicated = accessibility_update(&mut handler);
    assert!(!duplicated
        .nodes
        .iter()
        .any(|(_, node)| node.label() == Some("Cloned Portal owner")));
    assert!(!has_node_id(&duplicated, old_node_id));

    handler
        .accessibility_action_sender
        .send(action_request_for_node(old_node_id, Action::Click, None))
        .unwrap();
    assert!(!handler.drain_accessibility_actions());
    assert_eq!(clicks.load(Ordering::SeqCst), 0);
}

#[test]
fn accessibility_raw_local_action_cannot_fall_through_to_a_portal_clone() {
    let invalidation = InvalidationSignal::new();
    let clicks = Arc::new(AtomicUsize::new(0));
    let clicks_for_command = Arc::clone(&clicks);
    let portal_button: Element<TestVm> = Button::new("Local versus Portal clone")
        .on_click(Command::new(move |_vm| {
            clicks_for_command.fetch_add(1, Ordering::SeqCst);
        }))
        .size(dp(160.0), dp(30.0))
        .into();
    let shared_id = portal_button.id;
    let mut disabled_local: Element<TestVm> = Button::new("Local versus Portal clone")
        .disable(true)
        .size(dp(160.0), dp(30.0))
        .into();
    disabled_local.id = shared_id;
    let portal: Element<TestVm> = Portal::new(portal_button)
        .anchor(Rect::new(dp(20.0), dp(70.0), dp(1.0), dp(1.0)))
        .into();
    let mut handler = test_handler(
        Some(WidgetTree::new(
            Stack::new().child([disabled_local, portal]),
        )),
        invalidation,
    );

    let update = accessibility_update(&mut handler);
    let raw_node_id = crate::accessibility::node_id_from_widget(shared_id);
    assert!(node_for_id(&update, raw_node_id).is_disabled());
    let portal_node_id = update
        .nodes
        .iter()
        .find_map(|(node_id, node)| {
            (*node_id != raw_node_id
                && node.role() == Role::Button
                && node.label() == Some("Local versus Portal clone"))
            .then_some(*node_id)
        })
        .expect("Portal clone should use a synthetic node id");

    handler
        .accessibility_action_sender
        .send(action_request_for_node(raw_node_id, Action::Click, None))
        .unwrap();
    assert!(!handler.drain_accessibility_actions());
    assert_eq!(clicks.load(Ordering::SeqCst), 0);

    handler
        .accessibility_action_sender
        .send(action_request_for_node(portal_node_id, Action::Click, None))
        .unwrap();
    assert!(handler.drain_accessibility_actions());
    assert_eq!(clicks.load(Ordering::SeqCst), 1);
}

#[test]
fn accessibility_portal_fragments_follow_final_layer_order_and_skip_hidden_placement() {
    let invalidation = InvalidationSignal::new();
    let layer_portal = |label: &'static str, layer: OverlayLayer| -> Element<TestVm> {
        Portal::new(Button::new(label).size(dp(120.0), dp(28.0)))
            .layer(layer)
            .anchor(Rect::new(dp(20.0), dp(20.0), dp(1.0), dp(1.0)))
            .into()
    };
    let hidden_button: Element<TestVm> =
        Button::new("layer-hidden").size(dp(120.0), dp(28.0)).into();
    let hidden_button_id = hidden_button.id;
    let children: [Element<TestVm>; 6] = [
        layer_portal("layer-toast", OverlayLayer::Toast),
        layer_portal("layer-modal", OverlayLayer::Modal),
        layer_portal("layer-menu", OverlayLayer::Menu),
        layer_portal("layer-popover", OverlayLayer::Popover),
        layer_portal("layer-tooltip", OverlayLayer::Tooltip),
        Portal::new(hidden_button)
            .layer(OverlayLayer::Toast)
            .anchor(Rect::new(dp(500.0), dp(500.0), dp(1.0), dp(1.0)))
            .flip_policy(OverlayFlipPolicy::Hide)
            .viewport_padding(dp(0.0))
            .into(),
    ];
    let mut handler = test_handler(
        Some(WidgetTree::new(Stack::new().child(children))),
        invalidation,
    );
    assert!(!handler
        .computed_scene()
        .overlay_hit_regions
        .iter()
        .any(|hit| { hit.interaction.widget_id() == hidden_button_id }));

    let update = accessibility_update(&mut handler);
    assert!(!update
        .nodes
        .iter()
        .any(|(_, node)| node.label() == Some("layer-hidden")));
    let root = node_for_id(&update, crate::accessibility::ROOT_NODE_ID);
    let ordered_labels = root
        .children()
        .iter()
        .filter_map(|node_id| node_for_id(&update, *node_id).label())
        .filter(|label| label.starts_with("layer-"))
        .collect::<Vec<_>>();
    assert_eq!(
        ordered_labels,
        [
            "layer-tooltip",
            "layer-popover",
            "layer-menu",
            "layer-modal",
            "layer-toast",
        ]
    );
}

#[test]
fn accessibility_tree_click_ignores_stale_checkbox_cursor() {
    let invalidation = InvalidationSignal::new();
    let selections = Arc::new(AtomicUsize::new(0));
    let selections_for_command = Arc::clone(&selections);
    let expands = Arc::new(AtomicUsize::new(0));
    let expands_for_command = Arc::clone(&expands);
    let checks = Arc::new(AtomicUsize::new(0));
    let checks_for_command = Arc::clone(&checks);
    let tree = WidgetTree::new(
        Tree::<usize, TestVm>::new(
            vec![TreeNode::keyed("root", 0).child(TreeNode::keyed("child", 1))],
            |context| Text::new(context.item.to_string()).into(),
        )
        .expanded_keys(vec![WidgetKey::from("root")])
        .selection_mode(TreeSelectionMode::Single)
        .checkable(true)
        .on_selection_change(ValueCommand::new(
            move |_vm, change: TreeSelectionChange| {
                assert_eq!(change.trigger, TreeSelectionTrigger::Click);
                selections_for_command.fetch_add(1, Ordering::SeqCst);
            },
        ))
        .on_expand_change(ValueCommand::new(move |_vm, _change| {
            expands_for_command.fetch_add(1, Ordering::SeqCst);
        }))
        .on_check_change(ValueCommand::new(move |_vm, _change| {
            checks_for_command.fetch_add(1, Ordering::SeqCst);
        }))
        .size(dp(240.0), dp(120.0)),
    );
    let mut handler = test_handler(Some(tree), invalidation);
    let (row_id, checkbox_cursor) = {
        let computed = handler.computed_scene();
        let row_id = computed
            .hit_regions
            .iter()
            .find_map(|region| match &region.interaction {
                HitInteraction::TreeNode { id, state, .. }
                    if state.key == WidgetKey::from("root") =>
                {
                    Some(*id)
                }
                _ => None,
            })
            .expect("root row should be materialized");
        let checkbox = computed
            .hit_regions
            .iter()
            .find_map(|region| match &region.interaction {
                HitInteraction::TreeCheckbox { id, .. } if *id == row_id => Some(Point::new(
                    region.rect.x + region.rect.width * 0.5,
                    region.rect.y + region.rect.height * 0.5,
                )),
                _ => None,
            })
            .expect("root row should expose a checkbox hit slot");
        (row_id, checkbox)
    };
    let update = accessibility_update(&mut handler);
    assert!(node_for(&update, row_id).supports_action(Action::Click));

    handler.cursor_position = Some(checkbox_cursor);
    handler
        .accessibility_action_sender
        .send(action_request(row_id, Action::Click, None))
        .unwrap();
    assert!(handler.drain_accessibility_actions());

    assert_eq!(handler.focused_widget_id(), Some(row_id));
    assert_eq!(selections.load(Ordering::SeqCst), 1);
    assert_eq!(expands.load(Ordering::SeqCst), 0);
    assert_eq!(checks.load(Ordering::SeqCst), 0);
    assert!(handler.pending_click.is_none());
}

#[test]
fn accessibility_tree_click_ignores_pointer_selection_modifiers() {
    let invalidation = InvalidationSignal::new();
    let selected = State::new(Vec::<WidgetKey>::new(), invalidation.clone());
    let selected_for_command = selected.clone();
    let tree = WidgetTree::new(
        Tree::<usize, TestVm>::new(
            (0..3)
                .map(|index| TreeNode::keyed(format!("row-{index}"), index))
                .collect(),
            |context| Text::new(context.item.to_string()).into(),
        )
        .selection_mode(TreeSelectionMode::Multiple)
        .selected_keys(selected.signal())
        .on_selection_change(ValueCommand::new(
            move |_vm, change: TreeSelectionChange| {
                assert_eq!(change.trigger, TreeSelectionTrigger::Click);
                selected_for_command.set(change.selected_keys);
            },
        ))
        .size(dp(220.0), dp(120.0)),
    );
    let mut handler = test_handler(Some(tree), invalidation);
    let row_ids = handler
        .computed_scene()
        .hit_regions
        .iter()
        .filter_map(|region| match &region.interaction {
            HitInteraction::TreeNode { id, state, .. } => Some((state.key.clone(), *id)),
            _ => None,
        })
        .collect::<std::collections::HashMap<_, _>>();
    let row_a = row_ids[&WidgetKey::from("row-0")];
    let row_b = row_ids[&WidgetKey::from("row-1")];
    let row_c = row_ids[&WidgetKey::from("row-2")];
    let _ = accessibility_update(&mut handler);

    handler
        .accessibility_action_sender
        .send(action_request(row_a, Action::Click, None))
        .unwrap();
    assert!(handler.drain_accessibility_actions());
    assert_eq!(selected.get(), vec![WidgetKey::from("row-0")]);

    handler.modifiers = ModifiersState::SHIFT;
    handler
        .accessibility_action_sender
        .send(action_request(row_c, Action::Click, None))
        .unwrap();
    assert!(handler.drain_accessibility_actions());
    assert_eq!(selected.get(), vec![WidgetKey::from("row-2")]);

    handler.modifiers = ModifiersState::empty();
    handler
        .accessibility_action_sender
        .send(action_request(row_b, Action::Click, None))
        .unwrap();
    assert!(handler.drain_accessibility_actions());
    assert_eq!(selected.get(), vec![WidgetKey::from("row-1")]);

    handler.modifiers = accessibility_primary_modifier();
    handler
        .accessibility_action_sender
        .send(action_request(row_c, Action::Click, None))
        .unwrap();
    assert!(handler.drain_accessibility_actions());
    assert_eq!(selected.get(), vec![WidgetKey::from("row-2")]);
}

#[test]
fn accessibility_data_grid_click_ignores_pointer_selection_modifiers() {
    let invalidation = InvalidationSignal::new();
    let selected = State::new(Vec::<WidgetKey>::new(), invalidation.clone());
    let selected_for_command = selected.clone();
    let columns = vec![DataGridColumn::new(
        "name",
        "Name".to_string(),
        |context: crate::ui::widget::DataGridCellContext<usize>| {
            Text::new(context.row.to_string()).into()
        },
    )
    .width(dp(180.0))];
    let tree = WidgetTree::new(
        DataGrid::new(
            (0..3)
                .map(|index| DataGridRow::keyed(format!("row-{index}"), index))
                .collect(),
            columns,
        )
        .selection_mode(DataGridSelectionMode::Multiple)
        .selected_keys(selected.signal())
        .on_selection_change(ValueCommand::new(
            move |_vm, change: DataGridSelectionChange| {
                assert_eq!(change.trigger, DataGridSelectionTrigger::Click);
                selected_for_command.set(change.selected_keys);
            },
        ))
        .row_height(dp(28.0))
        .overscan(0)
        .size(dp(200.0), dp(140.0)),
    );
    let mut handler = test_handler(Some(tree), invalidation);
    let row_ids = handler
        .computed_scene()
        .hit_regions
        .iter()
        .filter_map(|region| match &region.interaction {
            HitInteraction::DataGridCell { id, state, .. } => Some((state.row_key.clone(), *id)),
            _ => None,
        })
        .collect::<std::collections::HashMap<_, _>>();
    let row_a = row_ids[&WidgetKey::from("row-0")];
    let row_b = row_ids[&WidgetKey::from("row-1")];
    let row_c = row_ids[&WidgetKey::from("row-2")];
    let _ = accessibility_update(&mut handler);

    handler
        .accessibility_action_sender
        .send(action_request(row_a, Action::Click, None))
        .unwrap();
    assert!(handler.drain_accessibility_actions());
    assert_eq!(selected.get(), vec![WidgetKey::from("row-0")]);

    handler.modifiers = ModifiersState::SHIFT;
    handler
        .accessibility_action_sender
        .send(action_request(row_c, Action::Click, None))
        .unwrap();
    assert!(handler.drain_accessibility_actions());
    assert_eq!(selected.get(), vec![WidgetKey::from("row-2")]);

    handler.modifiers = ModifiersState::empty();
    handler
        .accessibility_action_sender
        .send(action_request(row_b, Action::Click, None))
        .unwrap();
    assert!(handler.drain_accessibility_actions());
    assert_eq!(selected.get(), vec![WidgetKey::from("row-1")]);

    handler.modifiers = accessibility_primary_modifier();
    handler
        .accessibility_action_sender
        .send(action_request(row_c, Action::Click, None))
        .unwrap();
    assert!(handler.drain_accessibility_actions());
    assert_eq!(selected.get(), vec![WidgetKey::from("row-2")]);
}

#[test]
fn accessibility_list_click_never_falls_through_to_double_click_action() {
    for selection_mode in [ListSelectionMode::None, ListSelectionMode::Single] {
        let invalidation = InvalidationSignal::new();
        let actions = Arc::new(AtomicUsize::new(0));
        let actions_for_command = Arc::clone(&actions);
        let tree = WidgetTree::new(
            List::<usize, TestVm>::new(vec![ListItem::keyed("row", 0)], |context| {
                Text::new(context.item.to_string()).into()
            })
            .selection_mode(selection_mode)
            .on_item_action(ValueCommand::new(move |_vm, _action: ListItemAction| {
                actions_for_command.fetch_add(1, Ordering::SeqCst);
            }))
            .size(dp(180.0), dp(48.0)),
        );
        let mut handler = test_handler(Some(tree), invalidation);
        let row_id = handler
            .computed_scene()
            .hit_regions
            .iter()
            .find_map(|region| match &region.interaction {
                HitInteraction::ListItem { id, .. } => Some(*id),
                _ => None,
            })
            .expect("List row should materialize");
        let update = accessibility_update(&mut handler);
        assert!(node_for(&update, row_id).supports_action(Action::Click));

        for _ in 0..2 {
            handler
                .accessibility_action_sender
                .send(action_request(row_id, Action::Click, None))
                .unwrap();
            assert!(handler.drain_accessibility_actions());
            assert!(handler.pending_click.is_none());
        }
        assert_eq!(handler.focused_widget_id(), Some(row_id));
        assert_eq!(actions.load(Ordering::SeqCst), 0);
    }
}

#[test]
fn accessibility_editable_data_grid_click_never_commits_or_touches_pending_click() {
    let invalidation = InvalidationSignal::new();
    let selections = Arc::new(AtomicUsize::new(0));
    let selections_for_command = Arc::clone(&selections);
    let commits = Arc::new(AtomicUsize::new(0));
    let commits_for_command = Arc::clone(&commits);
    let columns = vec![DataGridColumn::new(
        "name",
        "Name".to_string(),
        |context: crate::ui::widget::DataGridCellContext<&'static str>| {
            Text::new(context.row).into()
        },
    )
    .width(dp(180.0))
    .text_value(|row| row.to_string())
    .editable(true)];
    let tree = WidgetTree::new(
        DataGrid::new(vec![DataGridRow::keyed("row", "Alpha")], columns)
            .selection_mode(DataGridSelectionMode::Single)
            .on_selection_change(ValueCommand::new(move |_vm, _change| {
                selections_for_command.fetch_add(1, Ordering::SeqCst);
            }))
            .on_cell_edit_commit(ValueCommand::new(move |_vm, _commit| {
                commits_for_command.fetch_add(1, Ordering::SeqCst);
            }))
            .size(dp(200.0), dp(96.0)),
    );
    let mut handler = test_handler(Some(tree), invalidation);
    let (cell_id, cell_center) = handler
        .computed_scene()
        .hit_regions
        .iter()
        .find_map(|region| match &region.interaction {
            HitInteraction::DataGridCell { id, .. } => Some((
                *id,
                Point::new(
                    region.rect.x + region.rect.width * 0.5,
                    region.rect.y + region.rect.height * 0.5,
                ),
            )),
            _ => None,
        })
        .expect("editable DataGrid cell should materialize");
    let update = accessibility_update(&mut handler);
    assert!(node_for(&update, cell_id).supports_action(Action::Click));
    handler.cursor_position = Some(cell_center);

    for expected_selections in 1..=2 {
        handler
            .accessibility_action_sender
            .send(action_request(cell_id, Action::Click, None))
            .unwrap();
        assert!(handler.drain_accessibility_actions());
        assert!(handler.pending_click.is_none());
        assert_eq!(selections.load(Ordering::SeqCst), expected_selections);
    }
    assert_eq!(commits.load(Ordering::SeqCst), 0);

    let sentinel_deadline = Instant::now() + Duration::from_secs(30);
    handler.pending_click = Some(crate::runtime::PendingClick {
        target_id: crate::runtime::HoverTargetId::Widget(cell_id),
        deadline: sentinel_deadline,
        position: cell_center,
        command: None,
        splitter: None,
    });
    handler
        .accessibility_action_sender
        .send(action_request(cell_id, Action::Click, None))
        .unwrap();
    assert!(handler.drain_accessibility_actions());
    let pending = handler
        .pending_click
        .as_ref()
        .expect("accessibility Click must preserve pointer pending state");
    assert!(pending.target_id == crate::runtime::HoverTargetId::Widget(cell_id));
    assert_eq!(pending.deadline, sentinel_deadline);
    assert_eq!(pending.position, cell_center);
    assert_eq!(selections.load(Ordering::SeqCst), 3);
    assert_eq!(commits.load(Ordering::SeqCst), 0);
}

#[test]
fn accessibility_data_grid_header_click_is_not_shift_additive() {
    let invalidation = InvalidationSignal::new();
    let initial_sort = vec![crate::ui::widget::DataGridSort {
        column_key: WidgetKey::from("name"),
        direction: crate::ui::widget::DataGridSortDirection::Ascending,
    }];
    let sort = Arc::new(Mutex::new(initial_sort));
    let sort_for_signal = Arc::clone(&sort);
    let sort_signal = Signal::new(
        move || sort_for_signal.lock().unwrap().clone(),
        invalidation.clone(),
    );
    let sort_for_command = Arc::clone(&sort);
    let columns = vec![
        DataGridColumn::new(
            "name",
            "Name".to_string(),
            |context: crate::ui::widget::DataGridCellContext<&'static str>| {
                Text::new(context.row).into()
            },
        )
        .width(dp(100.0))
        .sortable(true),
        DataGridColumn::new(
            "role",
            "Role".to_string(),
            |_context: crate::ui::widget::DataGridCellContext<&'static str>| {
                Text::new("Role").into()
            },
        )
        .width(dp(100.0))
        .sortable(true),
    ];
    let tree = WidgetTree::new(
        DataGrid::new(vec![DataGridRow::keyed("row", "Alpha")], columns)
            .sort(sort_signal)
            .on_sort_change(ValueCommand::new(
                move |_vm, change: crate::ui::widget::DataGridSortChange| {
                    *sort_for_command.lock().unwrap() = change.sort;
                },
            ))
            .size(dp(220.0), dp(96.0)),
    );
    let mut handler = test_handler(Some(tree), invalidation);
    let header_id = handler
        .computed_scene()
        .hit_regions
        .iter()
        .find_map(|region| match &region.interaction {
            HitInteraction::DataGridHeader { id, state, .. }
                if state.column_key == WidgetKey::from("role") =>
            {
                Some(*id)
            }
            _ => None,
        })
        .expect("sortable DataGrid header should materialize");
    let update = accessibility_update(&mut handler);
    assert!(node_for(&update, header_id).supports_action(Action::Click));

    handler.modifiers = ModifiersState::SHIFT;
    handler
        .accessibility_action_sender
        .send(action_request(header_id, Action::Click, None))
        .unwrap();
    assert!(handler.drain_accessibility_actions());
    assert_eq!(
        *sort.lock().unwrap(),
        vec![crate::ui::widget::DataGridSort {
            column_key: WidgetKey::from("role"),
            direction: crate::ui::widget::DataGridSortDirection::Ascending,
        }]
    );
}

fn accessibility_primary_modifier() -> ModifiersState {
    #[cfg(target_os = "macos")]
    {
        crate::platform::keyboard::meta_modifier()
    }

    #[cfg(not(target_os = "macos"))]
    {
        ModifiersState::CONTROL
    }
}
