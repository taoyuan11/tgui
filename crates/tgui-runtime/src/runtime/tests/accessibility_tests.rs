use super::*;
use crate::foundation::binding::{Toast, ToastQueue};
use crate::ui::widget::{
    Collapse, ColorPicker, ComputedScene, ContextMenu, DataGridCellAction, DataGridSelectionChange,
    DataGridSelectionMode, DataGridSelectionTrigger, DatePicker, Drawer, DrawerHost, DrawerMode,
    Image, List, ListItem, ListItemAction, ListSelectionChange, ListSelectionMode,
    ListSelectionTrigger, Menu, MenuBar, MenuItem, Modal, NumberInput, NumberInputChangeTrigger,
    OverlayFlipPolicy, OverlayLayer, Pagination, Pane, Portal, ProgressBar, Radio, Rating,
    ResolvedWidgetKind, RichText, ScrollRegion, Show, Spinner, Splitter, SplitterAxis, TabItem,
    Tabs, TimePicker, ToastHost, Tree, TreeExpandChange, TreeNode, TreeNodeAction,
    TreeSelectionChange, TreeSelectionMode, TreeSelectionTrigger, Upload, UploadFile, UploadFileId,
    UploadStatus,
};
use accesskit::{
    Action, ActionData, ActionRequest, AriaCurrent, AutoComplete, HasPopup, Node, Orientation,
    Role, SortDirection, Toggled, TreeId,
};
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

fn overlay_scope_scene_fingerprint<VM>(
    scene: &ComputedScene<VM>,
) -> (
    Vec<(WidgetId, Vec<WidgetId>, bool, bool, bool, bool)>,
    Vec<(
        Rect,
        Option<Rect>,
        Vec<WidgetId>,
        Option<(WidgetId, Option<i32>, Vec<WidgetId>)>,
        std::mem::Discriminant<HitInteraction<VM>>,
    )>,
    Vec<(
        Rect,
        Option<Rect>,
        Vec<WidgetId>,
        Option<(WidgetId, Option<i32>, Vec<WidgetId>)>,
        std::mem::Discriminant<HitInteraction<VM>>,
    )>,
    Vec<(
        crate::ui::widget::OverlayId,
        Rect,
        OverlayLayer,
        bool,
        Option<WidgetId>,
        bool,
        bool,
    )>,
    Vec<ScrollRegion>,
) {
    let scopes = scene
        .focus_scopes
        .iter()
        .map(|scope| {
            (
                scope.scope_id,
                scope.path.clone(),
                scope.active,
                scope.options.is_trap(),
                scope.options.is_auto_focus_first(),
                scope.options.hides_from_accessibility(false),
            )
        })
        .collect();
    let hits = |regions: &[HitRegion<VM>]| {
        regions
            .iter()
            .map(|region| {
                (
                    region.rect,
                    region.clip_rect,
                    region.scope_path.clone(),
                    region
                        .focus
                        .as_ref()
                        .map(|focus| (focus.widget_id, focus.tab_index, focus.scope_path.clone())),
                    std::mem::discriminant(&region.interaction),
                )
            })
            .collect::<Vec<_>>()
    };
    let close_handlers = scene
        .overlay_close_handlers
        .iter()
        .map(|handler| {
            (
                handler.overlay_id,
                handler.rect,
                handler.layer,
                handler.on_close.is_some(),
                handler.return_focus_to,
                handler.close_on_outside_click,
                handler.close_on_escape,
            )
        })
        .collect();
    (
        scopes,
        hits(&scene.hit_regions),
        hits(&scene.overlay_hit_regions),
        close_handlers,
        scene.scroll_regions.to_vec(),
    )
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

fn node_id_for_label(update: &accesskit::TreeUpdate, label: &str) -> accesskit::NodeId {
    update
        .nodes
        .iter()
        .find_map(|(id, node)| (node.label() == Some(label)).then_some(*id))
        .unwrap_or_else(|| panic!("accessible node labeled {label:?} should exist"))
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
    let button: Element<TestVm> = Button::new("Save")
        .size(dp(80.0), dp(30.0))
        .on_click(Command::new(|_: &mut TestVm| {}))
        .into();
    let button_id = button.id;
    let checkbox: Element<TestVm> = Checkbox::new(true)
        .label("Remember")
        .size(dp(120.0), dp(30.0))
        .on_change(ValueCommand::new(|_: &mut TestVm, _: bool| {}))
        .into();
    let checkbox_id = checkbox.id;
    let radio: Element<TestVm> = Radio::new(true)
        .label("Choice")
        .size(dp(120.0), dp(30.0))
        .on_change(ValueCommand::new(|_: &mut TestVm, _: bool| {}))
        .into();
    let radio_id = radio.id;
    let slider: Element<TestVm> = Slider::new(4.0, 0.0, 10.0)
        .step(2.0)
        .size(dp(160.0), dp(30.0))
        .on_change(ValueCommand::new(|_: &mut TestVm, _: f32| {}))
        .into();
    let slider_id = slider.id;
    let tree = WidgetTree::new(Flex::new(Axis::Vertical).child([button, checkbox, radio, slider]));
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

    let radio_node = node_for(&update, radio_id);
    assert_eq!(radio_node.role(), Role::RadioButton);
    assert_eq!(radio_node.label(), Some("Choice"));
    assert_eq!(radio_node.toggled(), Some(Toggled::True));

    let slider_node = node_for(&update, slider_id);
    assert_eq!(slider_node.role(), Role::Slider);
    assert_eq!(slider_node.numeric_value(), Some(4.0));
    assert_eq!(slider_node.min_numeric_value(), Some(0.0));
    assert_eq!(slider_node.max_numeric_value(), Some(10.0));
    assert!(slider_node.supports_action(Action::SetValue));
}

#[test]
fn accessibility_read_only_controls_do_not_advertise_mutating_actions() {
    let invalidation = InvalidationSignal::new();
    let button: Element<TestVm> = Button::new("Preview").into();
    let button_id = button.id;
    let checkbox: Element<TestVm> = Checkbox::new(true).label("Checked preview").into();
    let checkbox_id = checkbox.id;
    let radio: Element<TestVm> = Radio::new(true).label("Radio preview").into();
    let radio_id = radio.id;
    let switch: Element<TestVm> = Switch::new(true).label("Switch preview").into();
    let switch_id = switch.id;
    let slider: Element<TestVm> = Slider::new(4.0, 0.0, 10.0).into();
    let slider_id = slider.id;
    let tree =
        WidgetTree::new(Flex::new(Axis::Vertical).child([button, checkbox, radio, switch, slider]));
    let mut handler = test_handler(Some(tree), invalidation);
    let update = accessibility_update(&mut handler);

    assert!(!node_for(&update, button_id).supports_action(Action::Click));
    assert!(!node_for(&update, checkbox_id).supports_action(Action::Click));
    assert!(!node_for(&update, radio_id).supports_action(Action::Click));
    assert!(!node_for(&update, switch_id).supports_action(Action::Click));
    let slider = node_for(&update, slider_id);
    assert!(!slider.supports_action(Action::Increment));
    assert!(!slider.supports_action(Action::Decrement));
    assert!(!slider.supports_action(Action::SetValue));
}

#[test]
fn accessibility_modal_hides_closed_descendants_and_exposes_open_dialog() {
    let invalidation = InvalidationSignal::new();
    let open = State::new(false, invalidation.clone());
    let content: Element<TestVm> = Button::new("Modal content")
        .size(dp(140.0), dp(30.0))
        .into();
    let content_id = content.id;
    let tree = WidgetTree::new(Modal::new(open.signal()).title("Settings").content(content));
    let mut handler = test_handler(Some(tree), invalidation);
    handler.reduced_motion = true;

    let closed = accessibility_update(&mut handler);
    assert!(!has_node(&closed, content_id));
    assert!(closed
        .nodes
        .iter()
        .all(|(_, node)| node.role() != Role::Dialog));

    open.set(true);
    handler.request_redraw_if_dirty(Instant::now());
    let opened = accessibility_update(&mut handler);
    assert!(has_node(&opened, content_id));
    let dialog = opened
        .nodes
        .iter()
        .find_map(|(_, node)| (node.role() == Role::Dialog && node.is_modal()).then_some(node))
        .expect("open modal should publish a dialog node");
    assert_eq!(dialog.label(), Some("Settings"));

    let retained_open = overlay_scope_scene_fingerprint(handler.computed_scene());
    handler.invalidate_scene_with_reason("modal_accessibility_equivalence_full_recollect");
    let full_open = overlay_scope_scene_fingerprint(handler.computed_scene());
    let full_open_accessibility = accessibility_update(&mut handler);
    assert_eq!(retained_open, full_open);
    assert_eq!(opened, full_open_accessibility);

    open.set(false);
    handler.request_redraw_if_dirty(Instant::now());
    let closed_again = accessibility_update(&mut handler);
    assert!(!has_node(&closed_again, content_id));
    assert!(closed_again
        .nodes
        .iter()
        .all(|(_, node)| node.role() != Role::Dialog));
    let retained_closed = overlay_scope_scene_fingerprint(handler.computed_scene());
    handler.invalidate_scene_with_reason("modal_closed_equivalence_full_recollect");
    let full_closed = overlay_scope_scene_fingerprint(handler.computed_scene());
    let full_closed_accessibility = accessibility_update(&mut handler);
    assert_eq!(retained_closed, full_closed);
    assert_eq!(closed_again, full_closed_accessibility);
}

#[test]
fn accessibility_drawer_hides_closed_overlay_and_push_descendants() {
    for mode in [DrawerMode::Overlay, DrawerMode::Push] {
        let invalidation = InvalidationSignal::new();
        let open = State::new(false, invalidation.clone());
        let content: Element<TestVm> = Button::new("Drawer content")
            .size(dp(140.0), dp(30.0))
            .into();
        let content_id = content.id;
        let drawer = Drawer::new(open.signal()).mode(mode).content(content);
        let root: Element<TestVm> = match mode {
            DrawerMode::Overlay => drawer.into(),
            DrawerMode::Push => DrawerHost::new(Text::new("Main content"), drawer)
                .size(dp(480.0), dp(320.0))
                .into(),
        };
        let mut handler = test_handler(Some(WidgetTree::new(root)), invalidation);
        handler.reduced_motion = true;

        let closed = accessibility_update(&mut handler);
        assert!(!has_node(&closed, content_id), "closed {mode:?} drawer");
        assert!(closed
            .nodes
            .iter()
            .all(|(_, node)| node.role() != Role::Dialog));

        open.set(true);
        handler.request_redraw_if_dirty(Instant::now());
        let opened = accessibility_update(&mut handler);
        assert!(has_node(&opened, content_id), "open {mode:?} drawer");
        assert!(opened
            .nodes
            .iter()
            .any(|(_, node)| node.role() == Role::Dialog && node.is_modal()));

        let retained_open = overlay_scope_scene_fingerprint(handler.computed_scene());
        handler.invalidate_scene_with_reason("drawer_accessibility_equivalence_full_recollect");
        let full_open = overlay_scope_scene_fingerprint(handler.computed_scene());
        let full_open_accessibility = accessibility_update(&mut handler);
        assert_eq!(retained_open, full_open, "open {mode:?} drawer scene");
        assert_eq!(opened, full_open_accessibility, "open {mode:?} drawer a11y");

        open.set(false);
        handler.request_redraw_if_dirty(Instant::now());
        let closed_again = accessibility_update(&mut handler);
        assert!(
            !has_node(&closed_again, content_id),
            "reclosed {mode:?} drawer"
        );
        assert!(closed_again
            .nodes
            .iter()
            .all(|(_, node)| node.role() != Role::Dialog));
        let retained_closed = overlay_scope_scene_fingerprint(handler.computed_scene());
        handler.invalidate_scene_with_reason("drawer_closed_equivalence_full_recollect");
        let full_closed = overlay_scope_scene_fingerprint(handler.computed_scene());
        let full_closed_accessibility = accessibility_update(&mut handler);
        assert_eq!(retained_closed, full_closed, "closed {mode:?} drawer scene");
        assert_eq!(
            closed_again, full_closed_accessibility,
            "closed {mode:?} drawer a11y"
        );
    }
}

#[test]
fn accessibility_toast_close_button_is_reachable_and_clickable() {
    let context = ViewModelContext::for_benchmarks();
    let invalidation = context.invalidation().clone();
    let queue = ToastQueue::<TestVm>::new(&context);
    queue.push_at(
        Toast::new("Background job completed").persistent(true),
        Instant::now() - Duration::from_secs(1),
    );
    let tree = WidgetTree::new(
        Stack::new()
            .size(dp(640.0), dp(480.0))
            .child(ToastHost::new(queue.clone())),
    );
    let mut handler = test_handler(Some(tree), invalidation);

    let update = accessibility_update(&mut handler);
    let (close_node_id, close_node) = update
        .nodes
        .iter()
        .find(|(_, node)| node.label() == Some("Dismiss notification"))
        .expect("Toast close button should be published by its overlay accessibility fragment");
    assert_eq!(close_node.role(), Role::Button);
    assert!(close_node.supports_action(Action::Focus));
    assert!(close_node.supports_action(Action::Click));
    assert!(has_reachable_node(&update, *close_node_id));

    handler
        .accessibility_action_sender
        .send(action_request_for_node(*close_node_id, Action::Click, None))
        .unwrap();
    assert!(handler.drain_accessibility_actions());
    assert!(queue
        .snapshot()
        .first()
        .and_then(|entry| entry.deadline)
        .is_some_and(|deadline| deadline <= Instant::now()));
}

#[test]
fn accessibility_collapse_hides_inactive_content_and_tracks_header_state() {
    let invalidation = InvalidationSignal::new();
    let expanded = State::new(false, invalidation.clone());
    let disabled = State::new(false, invalidation.clone());
    let expanded_for_command = expanded.clone();
    let content: Element<TestVm> = Button::new("Collapse content")
        .size(dp(160.0), dp(30.0))
        .into();
    let content_id = content.id;
    let tree = WidgetTree::new(
        Collapse::new("Details", content)
            .expanded(expanded.signal())
            .disabled(disabled.signal())
            .on_change(ValueCommand::new(move |_vm: &mut TestVm, value| {
                expanded_for_command.set(value);
            }))
            .size(dp(220.0), dp(100.0)),
    );
    let mut handler = test_handler(Some(tree), invalidation);

    let collapsed = accessibility_update(&mut handler);
    let (header_id, header) = collapsed
        .nodes
        .iter()
        .find(|(_, node)| node.role() == Role::Button && node.label() == Some("Details"))
        .expect("Collapse header should be exposed as a named button");
    assert_eq!(header.is_expanded(), Some(false));
    assert!(!header.is_disabled());
    assert!(header.supports_action(Action::Click));
    assert!(!has_node(&collapsed, content_id));

    handler
        .accessibility_action_sender
        .send(action_request_for_node(*header_id, Action::Click, None))
        .unwrap();
    assert!(handler.drain_accessibility_actions());
    assert!(expanded.get());
    handler.request_redraw_if_dirty(Instant::now());

    let opened = accessibility_update(&mut handler);
    assert_eq!(node_for_id(&opened, *header_id).is_expanded(), Some(true));
    assert!(has_node(&opened, content_id));

    disabled.set(true);
    handler.request_redraw_if_dirty(Instant::now());
    let disabled_update = accessibility_update(&mut handler);
    let disabled_header = node_for_id(&disabled_update, *header_id);
    assert!(disabled_header.is_disabled());
    assert!(!disabled_header.supports_action(Action::Click));
    assert!(has_node(&disabled_update, content_id));
}

#[test]
fn accessibility_carousel_exposes_only_active_slide_and_describes_indicators() {
    let invalidation = InvalidationSignal::new();
    let selected = State::new(0usize, invalidation.clone());
    let disabled = State::new(false, invalidation.clone());
    let selected_for_command = selected.clone();
    let first: Element<TestVm> = Button::new("First slide").size(dp(140.0), dp(30.0)).into();
    let first_id = first.id;
    let second: Element<TestVm> = Button::new("Second slide").size(dp(140.0), dp(30.0)).into();
    let second_id = second.id;
    let tree = WidgetTree::new(
        Carousel::new(vec![first, second], selected.signal())
            .disabled(disabled.signal())
            .on_change(ValueCommand::new(move |_vm: &mut TestVm, index| {
                selected_for_command.set(index);
            }))
            .size(dp(260.0), dp(160.0)),
    );
    let mut handler = test_handler(Some(tree), invalidation);

    let initial = accessibility_update(&mut handler);
    assert!(has_node(&initial, first_id));
    assert!(!has_node(&initial, second_id));
    let (second_indicator_id, second_indicator) = initial
        .nodes
        .iter()
        .find(|(_, node)| node.label() == Some("Go to slide 2 of 2"))
        .expect("second Carousel indicator should be named");
    assert_eq!(second_indicator.role(), Role::Button);
    assert_eq!(second_indicator.is_selected(), Some(false));
    assert_eq!(second_indicator.aria_current(), None);
    assert_eq!(second_indicator.position_in_set(), Some(2));
    assert_eq!(second_indicator.size_of_set(), Some(2));
    assert!(second_indicator.supports_action(Action::Click));

    handler
        .accessibility_action_sender
        .send(action_request_for_node(
            *second_indicator_id,
            Action::Click,
            None,
        ))
        .unwrap();
    assert!(handler.drain_accessibility_actions());
    assert_eq!(selected.get(), 1);
    handler.request_redraw_if_dirty(Instant::now());

    let changed = accessibility_update(&mut handler);
    assert!(!has_node(&changed, first_id));
    assert!(has_node(&changed, second_id));
    let selected_indicator = node_for_id(&changed, *second_indicator_id);
    assert_eq!(selected_indicator.is_selected(), Some(true));
    assert_eq!(selected_indicator.aria_current(), Some(AriaCurrent::True));

    disabled.set(true);
    handler.request_redraw_if_dirty(Instant::now());
    let disabled_update = accessibility_update(&mut handler);
    let disabled_indicator = node_for_id(&disabled_update, *second_indicator_id);
    assert!(disabled_indicator.is_disabled());
    assert!(!disabled_indicator.supports_action(Action::Click));
}

#[test]
fn accessibility_tabs_hide_unselected_panels() {
    let invalidation = InvalidationSignal::new();
    let selected = State::new("one".to_string(), invalidation.clone());
    let first: Element<TestVm> = Button::new("First panel action")
        .size(dp(140.0), dp(30.0))
        .into();
    let first_id = first.id;
    let second: Element<TestVm> = Button::new("Second panel action")
        .size(dp(140.0), dp(30.0))
        .into();
    let second_id = second.id;
    let tree = WidgetTree::new(
        Tabs::new(
            vec![
                TabItem::new("one", "One", first),
                TabItem::new("two", "Two", second),
            ],
            selected.signal(),
        )
        .size(dp(260.0), dp(150.0)),
    );
    let mut handler = test_handler(Some(tree), invalidation);

    let first_update = accessibility_update(&mut handler);
    assert!(has_node(&first_update, first_id));
    assert!(!has_node(&first_update, second_id));

    selected.set("two".to_string());
    handler.request_redraw_if_dirty(Instant::now());
    let second_update = accessibility_update(&mut handler);
    let scopes = handler
        .cached_scene
        .as_ref()
        .map(|cached| {
            cached
                .computed
                .focus_scopes
                .iter()
                .map(|scope| (scope.scope_id, scope.active))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    assert!(
        !has_node(&second_update, first_id),
        "inactive first panel remained accessible; focus scopes: {scopes:?}"
    );
    assert!(has_node(&second_update, second_id));
}

#[test]
fn accessibility_pagination_data_grid_and_splitter_publish_structural_state() {
    let invalidation = InvalidationSignal::new();
    let pagination: Element<TestVm> = Pagination::new(2usize, 5usize)
        .size(dp(260.0), dp(40.0))
        .into();
    let mut pagination_handler =
        test_handler(Some(WidgetTree::new(pagination)), invalidation.clone());
    let pagination_update = accessibility_update(&mut pagination_handler);
    let current_pages = pagination_update
        .nodes
        .iter()
        .filter(|(_, node)| node.aria_current() == Some(AriaCurrent::Page))
        .collect::<Vec<_>>();
    assert_eq!(current_pages.len(), 1);
    assert_eq!(current_pages[0].1.label(), Some("2"));

    let columns = vec![DataGridColumn::new(
        "name",
        "Name".to_string(),
        |context: crate::ui::widget::DataGridCellContext<&'static str>| {
            Text::new(context.row).into()
        },
    )
    .width(dp(100.0))
    .min_width(dp(60.0))
    .max_width(dp(140.0))
    .sortable(true)
    .resizable(true)];
    let grid = WidgetTree::new(
        DataGrid::new(vec![DataGridRow::keyed("row", "Alpha")], columns)
            .sort(vec![crate::ui::widget::DataGridSort {
                column_key: WidgetKey::from("name"),
                direction: crate::ui::widget::DataGridSortDirection::Descending,
            }])
            .on_sort_change(ValueCommand::new(|_vm: &mut TestVm, _change| {}))
            .on_column_width_change(ValueCommand::new(|_vm: &mut TestVm, _change| {}))
            .size(dp(180.0), dp(100.0)),
    );
    let mut grid_handler = test_handler(Some(grid), invalidation.clone());
    let (header_id, resize_id) = {
        let computed = grid_handler.computed_scene();
        let header = computed
            .hit_regions
            .iter()
            .find_map(|region| match &region.interaction {
                HitInteraction::DataGridHeader { id, .. } => Some(*id),
                _ => None,
            })
            .expect("sortable DataGrid header should be materialized");
        let resize = computed
            .hit_regions
            .iter()
            .find_map(|region| match &region.interaction {
                HitInteraction::DataGridResizeHandle { id, .. } => Some(*id),
                _ => None,
            })
            .expect("resizable DataGrid handle should be materialized");
        (header, resize)
    };
    let grid_update = accessibility_update(&mut grid_handler);
    assert_eq!(
        node_for(&grid_update, header_id).sort_direction(),
        Some(SortDirection::Descending)
    );
    let resize = node_for(&grid_update, resize_id);
    assert_eq!(resize.orientation(), Some(Orientation::Horizontal));
    assert_eq!(resize.numeric_value(), Some(100.0));
    assert_eq!(resize.min_numeric_value(), Some(60.0));
    assert_eq!(resize.max_numeric_value(), Some(140.0));

    let splitter = WidgetTree::new(
        Splitter::new(
            vec![
                Pane::new(Text::new("Left")).min(0.2).max(0.9),
                Pane::new(Text::new("Right")).min(0.4).max(0.6),
            ],
            vec![0.5, 0.5],
        )
        .axis(SplitterAxis::Horizontal)
        .size(dp(240.0), dp(100.0)),
    );
    let mut splitter_handler = test_handler(Some(splitter), invalidation);
    let handle_id = splitter_handler
        .computed_scene()
        .hit_regions
        .iter()
        .find_map(|region| match &region.interaction {
            HitInteraction::SplitterHandle { id, .. } => Some(*id),
            _ => None,
        })
        .expect("Splitter handle should be materialized");
    let splitter_update = accessibility_update(&mut splitter_handler);
    let handle = node_for(&splitter_update, handle_id);
    assert_eq!(handle.orientation(), Some(Orientation::Horizontal));
    assert_eq!(handle.numeric_value(), Some(0.5));
    assert!((handle.min_numeric_value().unwrap() - 0.4).abs() <= 1e-6);
    assert!((handle.max_numeric_value().unwrap() - 0.6).abs() <= 1e-6);
}

#[test]
fn accessibility_tree_tracks_uncontrolled_select_and_option_semantics() {
    let invalidation = InvalidationSignal::new();
    let selections = Arc::new(Mutex::new(Vec::<String>::new()));
    let selections_for_command = Arc::clone(&selections);
    let select: Element<TestVm> = Select::new(
        vec![
            SelectOption::new("one".to_string(), "One".to_string()),
            SelectOption::new("disabled".to_string(), "Disabled".to_string()).disable(true),
            SelectOption::new("two".to_string(), "Two".to_string()),
        ],
        Some("one".to_string()),
    )
    .on_change(ValueCommand::new(move |_vm: &mut TestVm, (key, _label)| {
        selections_for_command.lock().unwrap().push(key);
    }))
    .size(dp(180.0), dp(32.0))
    .into();
    let select_id = select.id;
    let mut handler = test_handler(Some(WidgetTree::new(select)), invalidation);
    handler.reduced_motion = true;

    let closed = accessibility_update(&mut handler);
    assert_eq!(node_for(&closed, select_id).is_expanded(), Some(false));

    assert!(handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Tab))));
    assert!(handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Enter))));
    let opened = accessibility_update(&mut handler);
    assert_eq!(node_for(&opened, select_id).is_expanded(), Some(true));

    let options = opened
        .nodes
        .iter()
        .filter(|(_, node)| node.role() == Role::ListBoxOption)
        .collect::<Vec<_>>();
    assert_eq!(options.len(), 3);
    let selected = options
        .iter()
        .find(|(_, node)| node.label() == Some("One"))
        .expect("selected option should be exposed");
    assert_eq!(selected.1.is_selected(), Some(true));
    let disabled = options
        .iter()
        .find(|(_, node)| node.label() == Some("Disabled"))
        .expect("disabled option should be exposed");
    assert!(disabled.1.is_disabled());
    let two = options
        .iter()
        .find(|(_, node)| node.label() == Some("Two"))
        .expect("enabled option should be exposed");
    assert!(two.1.supports_action(Action::Click));
    let two_node_id = two.0;

    handler
        .accessibility_action_sender
        .send(action_request_for_node(two_node_id, Action::Click, None))
        .unwrap();
    assert!(handler.drain_accessibility_actions());
    assert_eq!(selections.lock().unwrap().as_slice(), ["two"]);
    assert_eq!(handler.resolved_select_open_state(select_id), Some(false));
}

#[test]
fn accessibility_tree_labels_and_disables_switch_select_and_combobox() {
    let invalidation = InvalidationSignal::new();
    let checkbox: Element<TestVm> = Checkbox::new(true)
        .label("Remember me")
        .disable(true)
        .size(dp(140.0), dp(28.0))
        .into();
    let checkbox_id = checkbox.id;
    let radio: Element<TestVm> = Radio::new(true)
        .label("Primary")
        .disable(true)
        .size(dp(140.0), dp(28.0))
        .into();
    let radio_id = radio.id;
    let switch: Element<TestVm> = Switch::new(true)
        .label("Airplane mode")
        .disable(true)
        .size(dp(48.0), dp(28.0))
        .into();
    let switch_id = switch.id;
    let select: Element<TestVm> = Select::new(
        vec![SelectOption::new("cn".to_string(), "China".to_string())],
        Some("cn".to_string()),
    )
    .label("Country")
    .disable(true)
    .size(dp(180.0), dp(32.0))
    .into();
    let select_id = select.id;
    let slider: Element<TestVm> = Slider::new(4.0, 0.0, 10.0)
        .disable(true)
        .size(dp(180.0), dp(28.0))
        .into();
    let slider_id = slider.id;
    let combobox: Element<TestVm> = Combobox::new(
        TextController::new_legacy("Shanghai"),
        vec![ComboboxOption::new("sh", "Shanghai")],
    )
    .label("City")
    .disable(true)
    .size(dp(180.0), dp(32.0))
    .into();
    let combobox_id = combobox.id;
    let tree = WidgetTree::new(
        Flex::new(Axis::Vertical).child([checkbox, radio, switch, select, slider, combobox]),
    );
    let mut handler = test_handler(Some(tree), invalidation);

    let update = accessibility_update(&mut handler);

    let checkbox = node_for(&update, checkbox_id);
    assert_eq!(checkbox.role(), Role::CheckBox);
    assert_eq!(checkbox.toggled(), Some(Toggled::True));
    assert!(checkbox.is_disabled());
    assert!(!checkbox.supports_action(Action::Click));

    let radio = node_for(&update, radio_id);
    assert_eq!(radio.role(), Role::RadioButton);
    assert_eq!(radio.toggled(), Some(Toggled::True));
    assert!(radio.is_disabled());
    assert!(!radio.supports_action(Action::Click));

    let switch = node_for(&update, switch_id);
    assert_eq!(switch.role(), Role::Switch);
    assert_eq!(switch.label(), Some("Airplane mode"));
    assert_eq!(switch.toggled(), Some(Toggled::True));
    assert!(switch.is_disabled());
    assert!(!switch.supports_action(Action::Click));

    let select = node_for(&update, select_id);
    assert_eq!(select.role(), Role::ComboBox);
    assert_eq!(select.label(), Some("Country"));
    assert_eq!(select.value(), Some("China"));
    assert_eq!(select.has_popup(), Some(HasPopup::Listbox));
    assert_eq!(select.is_expanded(), Some(false));
    assert!(select.is_disabled());
    assert!(!select.supports_action(Action::Click));

    let slider = node_for(&update, slider_id);
    assert_eq!(slider.role(), Role::Slider);
    assert_eq!(slider.numeric_value(), Some(4.0));
    assert!(slider.is_disabled());
    assert!(!slider.supports_action(Action::Increment));
    assert!(!slider.supports_action(Action::Decrement));
    assert!(!slider.supports_action(Action::SetValue));

    let combobox = node_for(&update, combobox_id);
    assert_eq!(combobox.role(), Role::ComboBox);
    assert_eq!(combobox.label(), Some("City"));
    assert_eq!(combobox.value(), Some("Shanghai"));
    assert_eq!(combobox.auto_complete(), Some(AutoComplete::List));
    assert_eq!(combobox.has_popup(), Some(HasPopup::Listbox));
    assert_eq!(combobox.is_expanded(), Some(false));
    assert!(combobox.is_disabled());
    assert!(!combobox.supports_action(Action::Click));
    assert!(!combobox.supports_action(Action::SetValue));
}

#[test]
fn accessibility_select_fixed_open_only_clicks_when_public_handler_exists() {
    let invalidation = InvalidationSignal::new();
    let inert: Element<TestVm> = Select::new(
        vec![SelectOption::new("one".to_string(), "One".to_string())],
        None::<String>,
    )
    .open(false)
    .into();
    let inert_id = inert.id;
    let mut inert_handler = test_handler(Some(WidgetTree::new(inert)), invalidation.clone());
    let inert_update = accessibility_update(&mut inert_handler);
    assert!(!node_for(&inert_update, inert_id).supports_action(Action::Click));
    let inert_hit = inert_handler
        .computed_scene()
        .hit_regions
        .iter()
        .find(|hit| matches!(hit.interaction, HitInteraction::SelectTrigger { .. }))
        .expect("fixed Select trigger hit");
    assert_eq!(inert_hit.interaction.keyboard_activation(), None);

    let clicks = Arc::new(AtomicUsize::new(0));
    let clicks_for_command = Arc::clone(&clicks);
    let clickable: Element<TestVm> = Select::new(
        vec![SelectOption::new("one".to_string(), "One".to_string())],
        None::<String>,
    )
    .open(false)
    .on_click(Command::new(move |_vm: &mut TestVm| {
        clicks_for_command.fetch_add(1, Ordering::SeqCst);
    }))
    .into();
    let clickable_id = clickable.id;
    let mut clickable_handler = test_handler(Some(WidgetTree::new(clickable)), invalidation);
    let clickable_update = accessibility_update(&mut clickable_handler);
    assert!(node_for(&clickable_update, clickable_id).supports_action(Action::Click));
    clickable_handler
        .accessibility_action_sender
        .send(action_request(clickable_id, Action::Click, None))
        .unwrap();
    assert!(clickable_handler.drain_accessibility_actions());
    assert_eq!(clicks.load(Ordering::SeqCst), 1);
    assert_eq!(
        clickable_handler.resolved_select_open_state(clickable_id),
        Some(false)
    );
}

#[test]
fn accessibility_read_only_select_options_do_not_publish_click() {
    let invalidation = InvalidationSignal::new();
    let select: Element<TestVm> = Select::new(
        vec![SelectOption::new("one".to_string(), "One".to_string())],
        None::<String>,
    )
    .open(true)
    .size(dp(180.0), dp(32.0))
    .into();
    let mut handler = test_handler(Some(WidgetTree::new(select)), invalidation);
    handler.reduced_motion = true;

    let update = accessibility_update(&mut handler);
    let option = update
        .nodes
        .iter()
        .find_map(|(_, node)| (node.role() == Role::ListBoxOption).then_some(node))
        .expect("an open Select should expose its option");
    assert!(!option.supports_action(Action::Click));
}

#[test]
fn accessibility_combobox_fixed_open_and_read_only_tabs_do_not_publish_click() {
    let invalidation = InvalidationSignal::new();
    let combobox: Element<TestVm> = Combobox::new(
        TextController::new_legacy(""),
        vec![ComboboxOption::new("one", "One")],
    )
    .open(false)
    .into();
    let combobox_id = combobox.id;
    let mut combobox_handler = test_handler(Some(WidgetTree::new(combobox)), invalidation.clone());
    let combobox_update = accessibility_update(&mut combobox_handler);
    let combobox_node = node_for(&combobox_update, combobox_id);
    assert_eq!(combobox_node.role(), Role::ComboBox);
    assert!(combobox_node.supports_action(Action::SetValue));
    assert!(!combobox_node.supports_action(Action::Click));

    let tabs = Tabs::new(
        vec![
            TabItem::new("one", "One", Text::new("First panel")),
            TabItem::new("two", "Two", Text::new("Second panel")),
        ],
        "one".to_string(),
    );
    let mut tabs_handler = test_handler(Some(WidgetTree::new(tabs)), invalidation);
    let tabs_update = accessibility_update(&mut tabs_handler);
    let tab_nodes = tabs_update
        .nodes
        .iter()
        .filter_map(|(_, node)| (node.role() == Role::Tab).then_some(node))
        .collect::<Vec<_>>();
    assert_eq!(tab_nodes.len(), 2);
    assert!(tab_nodes
        .iter()
        .all(|node| !node.supports_action(Action::Click)));
}

#[test]
fn accessibility_combobox_exposes_popup_and_option_semantics() {
    let invalidation = InvalidationSignal::new();
    let combobox: Element<TestVm> = Combobox::new(
        TextController::new_legacy(""),
        vec![
            ComboboxOption::new("apple", "Apple"),
            ComboboxOption::new("banana", "Banana"),
            ComboboxOption::new("unavailable", "Unavailable").disabled(true),
        ],
    )
    .selected_key(Some("banana".to_string()))
    .label("Fruit")
    .size(dp(180.0), dp(32.0))
    .into();
    let combobox_id = combobox.id;
    let mut handler = test_handler(Some(WidgetTree::new(combobox)), invalidation);
    handler.reduced_motion = true;

    let closed = accessibility_update(&mut handler);
    let trigger = node_for(&closed, combobox_id);
    assert_eq!(trigger.role(), Role::ComboBox);
    assert_eq!(trigger.label(), Some("Fruit"));
    assert_eq!(trigger.auto_complete(), Some(AutoComplete::List));
    assert_eq!(trigger.has_popup(), Some(HasPopup::Listbox));
    assert_eq!(trigger.is_expanded(), Some(false));
    assert!(trigger.supports_action(Action::Click));
    assert!(trigger.supports_action(Action::SetValue));

    handler
        .accessibility_action_sender
        .send(action_request(combobox_id, Action::Click, None))
        .unwrap();
    assert!(handler.drain_accessibility_actions());

    let _ = accessibility_update(&mut handler);
    let opened = accessibility_update(&mut handler);
    assert_eq!(node_for(&opened, combobox_id).is_expanded(), Some(true));
    assert!(opened
        .nodes
        .iter()
        .any(|(_, node)| node.role() == Role::ListBox));
    let options = opened
        .nodes
        .iter()
        .filter(|(_, node)| node.role() == Role::ListBoxOption)
        .map(|(_, node)| node)
        .collect::<Vec<_>>();
    assert_eq!(
        options
            .iter()
            .map(|node| node.label().unwrap_or_default())
            .collect::<Vec<_>>(),
        ["Apple", "Banana", "Unavailable"]
    );
    let banana = options
        .iter()
        .find(|node| node.label() == Some("Banana"))
        .expect("selected combobox option");
    assert_eq!(banana.is_selected(), Some(true));
    let unavailable = options
        .iter()
        .find(|node| node.label() == Some("Unavailable"))
        .expect("disabled combobox option");
    assert!(unavailable.is_disabled());
    assert!(!unavailable.supports_action(Action::Click));
}

#[test]
fn accessibility_stale_click_cannot_open_newly_disabled_combobox() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let disabled = context.state(false);
    let open_requests = Arc::new(Mutex::new(Vec::new()));
    let requests_for_command = Arc::clone(&open_requests);
    let combobox: Element<TestVm> = Combobox::new(
        TextController::new_legacy(""),
        vec![ComboboxOption::new("one", "One")],
    )
    .label("Choice")
    .disable(disabled.signal())
    .on_open_change(ValueCommand::new(move |_vm, open| {
        requests_for_command.lock().unwrap().push(open);
    }))
    .size(dp(180.0), dp(32.0))
    .into();
    let combobox_id = combobox.id;
    let mut handler = test_handler(Some(WidgetTree::new(combobox)), invalidation);

    let enabled = accessibility_update(&mut handler);
    assert!(node_for(&enabled, combobox_id).supports_action(Action::Click));

    disabled.set(true);
    handler
        .accessibility_action_sender
        .send(action_request(combobox_id, Action::Click, None))
        .unwrap();
    assert!(!handler.drain_accessibility_actions());
    assert!(open_requests.lock().unwrap().is_empty());

    let disabled = accessibility_update(&mut handler);
    let node = node_for(&disabled, combobox_id);
    assert!(node.is_disabled());
    assert_eq!(node.is_expanded(), Some(false));
    assert!(!node.supports_action(Action::Click));
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
        text_input_count >= 2,
        "DatePicker and TimePicker should expose editable text inputs"
    );
    let number_input = update
        .nodes
        .iter()
        .find_map(|(_, node)| (node.role() == Role::SpinButton).then_some(node))
        .expect("NumberInput should expose a spin button");
    assert_eq!(number_input.value(), Some("24"));
    assert_eq!(number_input.numeric_value(), Some(24.0));
    assert_eq!(number_input.numeric_value_step(), Some(1.0));
    assert!(number_input.supports_action(Action::Increment));
    assert!(number_input.supports_action(Action::Decrement));
    assert!(number_input.supports_action(Action::SetValue));
    let (color_button_id, color_button) = update
        .nodes
        .iter()
        .find_map(|(id, node)| {
            (node.role() == Role::Button && node.label() == Some("#3366CCFF"))
                .then_some((*id, node))
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

    handler
        .accessibility_action_sender
        .send(action_request_for_node(
            color_button_id,
            Action::Click,
            None,
        ))
        .unwrap();
    assert!(handler.drain_accessibility_actions());
    assert!(handler
        .computed_scene()
        .overlay_close_handlers
        .iter()
        .any(|handler| handler.layer == crate::runtime::overlay::OverlayLayer::Popover));
}

#[test]
fn progress_spinner_and_image_publish_normalized_named_semantics() {
    let invalidation = InvalidationSignal::new();
    let spinner_label = State::new(String::from("Loading results"), invalidation.clone());
    let non_finite: Element<TestVm> = ProgressBar::new(f32::NAN).label("Unknown progress").into();
    let non_finite_id = non_finite.id;
    let overflow: Element<TestVm> = ProgressBar::new(1.5).label("Completed progress").into();
    let overflow_id = overflow.id;
    let spinner: Element<TestVm> = Spinner::new().label(spinner_label.signal()).into();
    let spinner_id = spinner.id;
    let image: Element<TestVm> = Image::from_bytes(Vec::<u8>::new())
        .fit(crate::media::ContentFit::Cover)
        .alt("Architecture diagram")
        .size(dp(32.0), dp(32.0))
        .into();
    let image_id = image.id;
    let decorative: Element<TestVm> = Image::from_bytes(Vec::<u8>::new())
        .alt("")
        .size(dp(32.0), dp(32.0))
        .into();
    let decorative_id = decorative.id;
    let tree = WidgetTree::new(
        Flex::new(Axis::Vertical).child([non_finite, overflow, spinner, image, decorative]),
    );
    let mut handler = test_handler(Some(tree), invalidation);

    let update = accessibility_update(&mut handler);
    let non_finite = node_for(&update, non_finite_id);
    assert_eq!(non_finite.role(), Role::ProgressIndicator);
    assert_eq!(non_finite.numeric_value(), Some(0.0));
    assert_eq!(non_finite.label(), Some("Unknown progress"));
    assert_eq!(node_for(&update, overflow_id).numeric_value(), Some(1.0));
    assert_eq!(
        node_for(&update, spinner_id).label(),
        Some("Loading results")
    );
    assert_eq!(
        node_for(&update, image_id).label(),
        Some("Architecture diagram")
    );
    assert_eq!(node_for(&update, decorative_id).label(), None);

    spinner_label.set(String::from("Almost done"));
    let updated = accessibility_update(&mut handler);
    assert_eq!(node_for(&updated, spinner_id).label(), Some("Almost done"));

    let image_fit = handler
        .cached_scene
        .as_ref()
        .and_then(|cached| cached.layout.as_ref())
        .and_then(|layout| layout.resolved_widget(image_id))
        .and_then(|resolved| match &resolved.kind {
            ResolvedWidgetKind::Image { image, .. } => Some(image.fit),
            _ => None,
        });
    assert_eq!(image_fit, Some(crate::media::ContentFit::Cover));
}

#[test]
fn color_picker_exposes_named_rgba_channel_sliders() {
    let invalidation = InvalidationSignal::new();
    let tree = WidgetTree::new(ColorPicker::new(Color::hexa(0x3366CCFF)).open(true));
    let mut handler = test_handler_with_config(
        TestVm,
        Some(tree),
        invalidation,
        test_config_with_size(500.0, 500.0),
    );

    let update = accessibility_update(&mut handler);
    let channels = update
        .nodes
        .iter()
        .filter_map(|(id, node)| {
            (node.role() == Role::Slider).then_some((*id, node.label().unwrap_or_default()))
        })
        .collect::<Vec<_>>();
    let mut labels = channels.iter().map(|(_, label)| *label).collect::<Vec<_>>();
    labels.sort_unstable();

    assert_eq!(
        labels,
        vec![
            "Alpha channel",
            "Blue channel",
            "Green channel",
            "Red channel"
        ]
    );

    let red_channel = channels
        .iter()
        .find_map(|(id, label)| (*label == "Red channel").then_some(*id))
        .expect("red channel accessibility node");
    handler
        .accessibility_action_sender
        .send(action_request_for_node(
            red_channel,
            Action::SetValue,
            Some(ActionData::NumericValue(128.0)),
        ))
        .unwrap();
    assert!(handler.drain_accessibility_actions());
    assert!(handler
        .computed_scene()
        .scene
        .overlay_texts
        .iter()
        .any(|text| text.content.as_ref() == "#8066CCFF"));
}

#[test]
fn rating_exposes_a_named_slider_semantic() {
    let invalidation = InvalidationSignal::new();
    let tree =
        WidgetTree::new(Rating::new(3.0).on_change(ValueCommand::new(|_: &mut TestVm, _| {})));
    let mut handler = test_handler_with_config(
        TestVm,
        Some(tree),
        invalidation,
        test_config_with_size(300.0, 120.0),
    );

    let update = accessibility_update(&mut handler);
    let rating = update
        .nodes
        .iter()
        .find_map(|(_, node)| {
            (node.role() == Role::Slider && node.label() == Some("Rating")).then_some(node)
        })
        .expect("interactive Rating should expose its named slider");

    assert_eq!(rating.numeric_value(), Some(3.0));
    assert_eq!(rating.min_numeric_value(), Some(0.0));
    assert_eq!(rating.max_numeric_value(), Some(5.0));
    assert!(rating.supports_action(Action::SetValue));
}

#[test]
fn rich_text_image_alt_reaches_accesskit_image_label() {
    let invalidation = InvalidationSignal::new();
    let tree = WidgetTree::new(RichText::markdown(
        "![Architecture diagram](https://example.com/diagram.png)",
    ));
    let mut handler = test_handler_with_config(
        TestVm,
        Some(tree),
        invalidation,
        test_config_with_size(320.0, 220.0),
    );

    let update = accessibility_update(&mut handler);
    let image = update
        .nodes
        .iter()
        .find_map(|(_, node)| (node.role() == Role::Image).then_some(node))
        .expect("rich-text image should be accessible");
    assert_eq!(image.label(), Some("Architecture diagram"));
}

#[test]
fn number_input_accessibility_increment_and_decrement_use_its_step_commands() {
    let invalidation = InvalidationSignal::new();
    let controller = TextController::new_legacy("4");
    let changes = Arc::new(Mutex::new(Vec::new()));
    let changes_for_command = Arc::clone(&changes);
    let tree = WidgetTree::new(
        NumberInput::new(controller.clone(), Some(4.0))
            .range(0.0, 10.0)
            .step(2.0)
            .on_change(ValueCommand::new(move |_: &mut TestVm, change| {
                changes_for_command.lock().unwrap().push(change);
            })),
    );
    let mut handler = test_handler(Some(tree), invalidation);
    let update = accessibility_update(&mut handler);
    let number_node = update
        .nodes
        .iter()
        .find_map(|(id, node)| (node.role() == Role::SpinButton).then_some(*id))
        .expect("NumberInput spin button node");

    handler
        .accessibility_action_sender
        .send(action_request_for_node(
            number_node,
            Action::Increment,
            None,
        ))
        .unwrap();
    assert!(handler.drain_accessibility_actions());
    assert_eq!(controller.text(), "6");

    handler
        .accessibility_action_sender
        .send(action_request_for_node(
            number_node,
            Action::Decrement,
            None,
        ))
        .unwrap();
    assert!(handler.drain_accessibility_actions());
    assert_eq!(controller.text(), "4");
    assert_eq!(changes.lock().unwrap().len(), 2);
}

#[test]
fn number_input_accessibility_set_value_clamps_formats_and_rejects_invalid_data() {
    let invalidation = InvalidationSignal::new();
    let controller = TextController::new_legacy("11");
    let changes = Arc::new(Mutex::new(Vec::new()));
    let changes_for_command = Arc::clone(&changes);
    let tree = WidgetTree::new(
        NumberInput::new(controller.clone(), Some(11.0))
            .range(0.0, 10.0)
            .step(0.25)
            .on_change(ValueCommand::new(move |_: &mut TestVm, change| {
                changes_for_command.lock().unwrap().push(change);
            })),
    );
    let mut handler = test_handler(Some(tree), invalidation);
    let update = accessibility_update(&mut handler);
    let number_node = update
        .nodes
        .iter()
        .find_map(|(id, node)| (node.role() == Role::SpinButton).then_some(*id))
        .expect("NumberInput spin button node");

    handler
        .accessibility_action_sender
        .send(action_request_for_node(
            number_node,
            Action::Increment,
            None,
        ))
        .unwrap();
    assert!(handler.drain_accessibility_actions());
    assert_eq!(controller.text(), "10");

    handler
        .accessibility_action_sender
        .send(action_request_for_node(
            number_node,
            Action::SetValue,
            Some(ActionData::NumericValue(7.125)),
        ))
        .unwrap();
    assert!(handler.drain_accessibility_actions());
    assert_eq!(controller.text(), "7.125");

    handler
        .accessibility_action_sender
        .send(action_request_for_node(
            number_node,
            Action::SetValue,
            Some(ActionData::NumericValue(99.0)),
        ))
        .unwrap();
    assert!(handler.drain_accessibility_actions());
    assert_eq!(controller.text(), "10");

    handler
        .accessibility_action_sender
        .send(action_request_for_node(
            number_node,
            Action::SetValue,
            Some(ActionData::Value("not-a-number".into())),
        ))
        .unwrap();
    assert!(!handler.drain_accessibility_actions());
    assert_eq!(controller.text(), "10");

    let changes = changes.lock().unwrap();
    assert_eq!(changes.len(), 3);
    assert_eq!(changes[0].value, Some(10.0));
    assert_eq!(changes[0].trigger, NumberInputChangeTrigger::StepUp);
    assert_eq!(changes[1].value, Some(7.125));
    assert_eq!(changes[1].trigger, NumberInputChangeTrigger::Text);
    assert_eq!(changes[2].value, Some(10.0));
    assert_eq!(changes[2].trigger, NumberInputChangeTrigger::Text);
}

#[test]
fn number_input_arrow_keys_step_the_focused_field_without_repeating_at_bounds() {
    let invalidation = InvalidationSignal::new();
    let controller = TextController::new_legacy("10");
    let changes = Arc::new(Mutex::new(Vec::new()));
    let changes_for_command = Arc::clone(&changes);
    let tree = WidgetTree::new(
        NumberInput::new(controller.clone(), Some(10.0))
            .range(0.0, 10.0)
            .step(2.0)
            .on_change(ValueCommand::new(move |_: &mut TestVm, change| {
                changes_for_command.lock().unwrap().push(change);
            })),
    );
    let mut handler = test_handler(Some(tree), invalidation);

    assert!(handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Tab))));
    assert!(handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Tab))));
    assert!(handler.focused_text_input_id().is_some());

    assert!(handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::ArrowUp,))));
    assert_eq!(controller.text(), "10");
    assert!(changes.lock().unwrap().is_empty());

    assert!(
        handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::ArrowDown,)))
    );
    assert_eq!(controller.text(), "8");
    assert_eq!(changes.lock().unwrap().len(), 1);
}

#[test]
fn time_picker_done_closes_internal_popover_and_restores_input_focus() {
    let invalidation = InvalidationSignal::new();
    let controller = TextController::new_legacy("09:30");
    let tree = WidgetTree::new(
        TimePicker::new(controller, Some(NaiveTime::from_hms_opt(9, 30, 0).unwrap())).open(true),
    );
    let mut handler = test_handler_with_config(
        TestVm,
        Some(tree),
        invalidation,
        test_config_with_size(500.0, 500.0),
    );
    handler.reduced_motion = true;

    assert!(handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Tab))));
    let input_id = handler
        .focused_text_input_id()
        .expect("TimePicker input should focus first");
    let computed = handler.computed_scene();
    let done_frame = computed
        .scene
        .overlay_texts
        .iter()
        .find(|text| text.content.as_ref() == "Done")
        .map(|text| text.frame)
        .expect("TimePicker should render its Done label");
    let done_center = Point::new(
        done_frame.x + done_frame.width * 0.5,
        done_frame.y + done_frame.height * 0.5,
    );
    let (done_focus, done_interaction) = computed
        .overlay_hit_regions
        .iter()
        .filter_map(|region| {
            region.focus.as_ref()?;
            matches!(
                &region.interaction,
                HitInteraction::Widget { interactions, .. }
                    if interactions.on_click.is_some()
            )
            .then_some(region)
        })
        .find(|region| {
            region
                .hit_delta_if_contains(done_center, &computed.transform_records)
                .is_some()
        })
        .map(|region| {
            (
                region.focus.as_ref().unwrap().clone(),
                region.interaction.clone(),
            )
        })
        .expect("TimePicker Done button should own the Done label");
    handler.update_focus(
        Some(FocusedWidget {
            widget_id: done_focus.widget_id,
            scope_path: done_focus.scope_path.clone(),
            on_blur: done_focus.on_blur.clone(),
        }),
        done_focus.on_focus.clone(),
        true,
    );
    assert_ne!(handler.focused_widget_id(), Some(input_id));

    assert!(handler.dispatch_accessibility_click_interaction(done_interaction));
    let _ = handler.computed_scene();
    assert_eq!(handler.focused_widget_id(), Some(input_id));
    assert!(!handler
        .computed_scene()
        .overlay_close_handlers
        .iter()
        .any(|handle| handle.layer == crate::runtime::overlay::OverlayLayer::Popover));
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
            .selection_mode(DataGridSelectionMode::None)
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
    assert!(!cell_node.supports_action(Action::Click));
    assert!(cell_node.supports_action(Action::Focus));
}

#[test]
fn accessibility_data_grid_click_falls_back_to_real_action_or_edit_capability() {
    let action_count = Arc::new(AtomicUsize::new(0));
    let action_count_for_command = Arc::clone(&action_count);
    let action_columns = vec![DataGridColumn::new(
        "action",
        "Action".to_string(),
        |context: crate::ui::widget::DataGridCellContext<&'static str>| {
            Text::new(context.row).into()
        },
    )];
    let action_tree = WidgetTree::new(
        DataGrid::new(
            vec![DataGridRow::keyed("action-row", "Run")],
            action_columns,
        )
        .selection_mode(DataGridSelectionMode::None)
        .on_cell_action(ValueCommand::new(move |_vm, _action| {
            action_count_for_command.fetch_add(1, Ordering::SeqCst);
        }))
        .size(dp(180.0), dp(96.0)),
    );
    let mut action_handler = test_handler(Some(action_tree), InvalidationSignal::new());
    let action_id = action_handler
        .computed_scene()
        .hit_regions
        .iter()
        .find_map(|region| match region.interaction {
            HitInteraction::DataGridCell { id, .. } => Some(id),
            _ => None,
        })
        .expect("action-only DataGrid cell");
    let update = accessibility_update(&mut action_handler);
    let node = node_for(&update, action_id);
    assert!(node.supports_action(Action::Click));
    assert!(node.supports_action(Action::Focus));
    action_handler
        .accessibility_action_sender
        .send(action_request(action_id, Action::Click, None))
        .unwrap();
    assert!(action_handler.drain_accessibility_actions());
    assert_eq!(action_count.load(Ordering::SeqCst), 1);

    let edit_count = Arc::new(AtomicUsize::new(0));
    let edit_count_for_command = Arc::clone(&edit_count);
    let edit_columns = vec![DataGridColumn::new(
        "edit",
        "Edit".to_string(),
        |context: crate::ui::widget::DataGridCellContext<&'static str>| {
            Text::new(context.row).into()
        },
    )
    .text_value(|row| row.to_string())
    .editable(true)];
    let edit_tree = WidgetTree::new(
        DataGrid::new(vec![DataGridRow::keyed("edit-row", "Edit")], edit_columns)
            .selection_mode(DataGridSelectionMode::None)
            .on_cell_edit_commit(ValueCommand::new(move |_vm, _commit| {
                edit_count_for_command.fetch_add(1, Ordering::SeqCst);
            }))
            .size(dp(180.0), dp(96.0)),
    );
    let mut edit_handler = test_handler(Some(edit_tree), InvalidationSignal::new());
    let edit_id = edit_handler
        .computed_scene()
        .hit_regions
        .iter()
        .find_map(|region| match region.interaction {
            HitInteraction::DataGridCell { id, .. } => Some(id),
            _ => None,
        })
        .expect("edit-only DataGrid cell");
    let update = accessibility_update(&mut edit_handler);
    let node = node_for(&update, edit_id);
    assert!(node.supports_action(Action::Click));
    assert!(node.supports_action(Action::Focus));
    edit_handler
        .accessibility_action_sender
        .send(action_request(edit_id, Action::Click, None))
        .unwrap();
    assert!(edit_handler.drain_accessibility_actions());
    assert_eq!(edit_count.load(Ordering::SeqCst), 1);
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
    .on_expand_change(ValueCommand::new(|_: &mut TestVm, _: TreeExpandChange| {}))
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
    assert!(root_node.supports_action(Action::Collapse));
    assert!(!root_node.supports_action(Action::Expand));

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
    assert!(!disabled_node.supports_action(Action::Click));
}

#[test]
fn accessibility_tree_expand_and_collapse_actions_dispatch_controlled_changes() {
    let invalidation = InvalidationSignal::new();
    let expanded = State::new(Vec::<WidgetKey>::new(), invalidation.clone());
    let expanded_for_command = expanded.clone();
    let tree = WidgetTree::new(
        Tree::<&'static str, TestVm>::new(
            vec![TreeNode::keyed("root", "Root").child(TreeNode::keyed("child", "Child"))],
            |ctx| Text::new(ctx.item).into(),
        )
        .expanded_keys(expanded.signal())
        .on_expand_change(ValueCommand::new(
            move |_: &mut TestVm, change: TreeExpandChange| {
                expanded_for_command.set(change.expanded_keys);
            },
        ))
        .size(dp(260.0), dp(120.0)),
    );
    let mut handler = test_handler(Some(tree), invalidation);
    let root_id = handler
        .computed_scene()
        .hit_regions
        .iter()
        .find_map(|region| match &region.interaction {
            HitInteraction::TreeNode { id, state, .. } if state.key == WidgetKey::from("root") => {
                Some(*id)
            }
            _ => None,
        })
        .expect("root Tree row should be materialized");

    let collapsed = accessibility_update(&mut handler);
    let collapsed_node = node_for(&collapsed, root_id);
    assert!(collapsed_node.supports_action(Action::Expand));
    assert!(!collapsed_node.supports_action(Action::Collapse));
    handler
        .accessibility_action_sender
        .send(action_request(root_id, Action::Expand, None))
        .unwrap();
    assert!(handler.drain_accessibility_actions());
    assert_eq!(expanded.get(), vec![WidgetKey::from("root")]);

    handler.request_redraw_if_dirty(Instant::now());
    let expanded_update = accessibility_update(&mut handler);
    let expanded_node = node_for(&expanded_update, root_id);
    assert!(!expanded_node.supports_action(Action::Expand));
    assert!(expanded_node.supports_action(Action::Collapse));
    handler
        .accessibility_action_sender
        .send(action_request(root_id, Action::Collapse, None))
        .unwrap();
    assert!(handler.drain_accessibility_actions());
    assert!(expanded.get().is_empty());
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
fn accessibility_radio_activation_is_idempotent_and_keeps_click_callback() {
    let invalidation = InvalidationSignal::new();
    let changes = Arc::new(Mutex::new(Vec::new()));
    let clicks = Arc::new(AtomicUsize::new(0));
    let changes_ref = Arc::clone(&changes);
    let clicks_ref = Arc::clone(&clicks);
    let radio: Element<TestVm> = crate::ui::widget::Radio::new(true)
        .on_change(ValueCommand::new(move |_vm: &mut TestVm, value| {
            changes_ref.lock().unwrap().push(value);
        }))
        .on_click(Command::new(move |_vm: &mut TestVm| {
            clicks_ref.fetch_add(1, Ordering::SeqCst);
        }))
        .size(dp(120.0), dp(30.0))
        .into();
    let radio_id = radio.id;
    let tree = WidgetTree::new(radio);
    let mut handler = test_handler(Some(tree), invalidation);
    let _ = accessibility_update(&mut handler);

    handler
        .accessibility_action_sender
        .send(action_request(radio_id, Action::Click, None))
        .unwrap();
    assert!(handler.drain_accessibility_actions());

    assert!(changes.lock().unwrap().is_empty());
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
fn accessibility_slider_uses_completion_callbacks_fallback_step_and_boundary_idempotence() {
    let invalidation = InvalidationSignal::new();
    let changes = Arc::new(Mutex::new(Vec::new()));
    let change_ends = Arc::new(Mutex::new(Vec::new()));
    let changes_ref = Arc::clone(&changes);
    let change_ends_ref = Arc::clone(&change_ends);
    let slider: Element<TestVm> = Slider::new(50.0, 0.0, 100.0)
        .step(0.0)
        .size(dp(120.0), dp(30.0))
        .on_change(ValueCommand::new(move |_vm: &mut TestVm, value| {
            changes_ref.lock().unwrap().push(value);
        }))
        .on_change_end(ValueCommand::new(move |_vm: &mut TestVm, value| {
            change_ends_ref.lock().unwrap().push(value);
        }))
        .into();
    let slider_id = slider.id;
    let mut handler = test_handler(Some(WidgetTree::new(slider)), invalidation);
    let update = accessibility_update(&mut handler);
    assert_eq!(node_for(&update, slider_id).numeric_value_step(), Some(1.0));

    handler
        .accessibility_action_sender
        .send(action_request(slider_id, Action::Increment, None))
        .unwrap();
    assert!(handler.drain_accessibility_actions());
    assert_eq!(*changes.lock().unwrap(), vec![51.0]);
    assert_eq!(*change_ends.lock().unwrap(), vec![51.0]);

    changes.lock().unwrap().clear();
    change_ends.lock().unwrap().clear();
    handler
        .accessibility_action_sender
        .send(action_request(
            slider_id,
            Action::SetValue,
            Some(ActionData::NumericValue(50.0)),
        ))
        .unwrap();
    assert!(handler.drain_accessibility_actions());
    assert!(changes.lock().unwrap().is_empty());
    assert!(change_ends.lock().unwrap().is_empty());
}

#[test]
fn accessibility_slider_with_only_completion_callback_remains_adjustable() {
    let invalidation = InvalidationSignal::new();
    let completions = Arc::new(Mutex::new(Vec::new()));
    let completions_ref = Arc::clone(&completions);
    let slider: Element<TestVm> = Slider::new(4.0, 0.0, 10.0)
        .step(2.0)
        .size(dp(120.0), dp(30.0))
        .on_change_end(ValueCommand::new(move |_vm: &mut TestVm, value| {
            completions_ref.lock().unwrap().push(value);
        }))
        .into();
    let slider_id = slider.id;
    let mut handler = test_handler(Some(WidgetTree::new(slider)), invalidation);
    let update = accessibility_update(&mut handler);
    let node = node_for(&update, slider_id);
    assert!(node.supports_action(Action::Increment));
    assert!(node.supports_action(Action::Decrement));
    assert!(node.supports_action(Action::SetValue));

    handler
        .accessibility_action_sender
        .send(action_request(slider_id, Action::Increment, None))
        .unwrap();
    assert!(handler.drain_accessibility_actions());
    assert_eq!(*completions.lock().unwrap(), vec![6.0]);
}

#[test]
fn accessibility_set_value_normalizes_input_and_textarea_line_breaks() {
    let invalidation = InvalidationSignal::new();
    let input_controller = TextController::new_legacy("");
    let textarea_controller = TextController::new_legacy("");
    let input: Element<TestVm> = Input::new(input_controller.clone())
        .size(dp(160.0), dp(30.0))
        .into();
    let input_id = input.id;
    let textarea: Element<TestVm> = Textarea::new(textarea_controller.clone())
        .size(dp(160.0), dp(60.0))
        .into();
    let textarea_id = textarea.id;
    let tree = WidgetTree::new(Flex::new(Axis::Vertical).child([input, textarea]));
    let mut handler = test_handler(Some(tree), invalidation);
    let _ = accessibility_update(&mut handler);
    let value = "a\r\nb\nc\rd\u{0085}e\u{2028}f\u{2029}g";

    handler
        .accessibility_action_sender
        .send(action_request(
            input_id,
            Action::SetValue,
            Some(ActionData::Value(value.into())),
        ))
        .unwrap();
    handler
        .accessibility_action_sender
        .send(action_request(
            textarea_id,
            Action::SetValue,
            Some(ActionData::Value(value.into())),
        ))
        .unwrap();
    assert!(handler.drain_accessibility_actions());

    assert_eq!(input_controller.text(), "abcdefg");
    assert_eq!(textarea_controller.text(), "a\nb\nc\nd\ne\nf\ng");
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

    assert_eq!(selections.load(Ordering::SeqCst), 0);
    assert_eq!(expands.load(Ordering::SeqCst), 0);
    assert_eq!(checks.load(Ordering::SeqCst), 1);
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
        let disabled_node = node_for(&update, disabled_id);
        assert!(!disabled_node.supports_action(Action::Click));

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
fn accessibility_menu_exposes_items_disabled_and_checked_state_and_click_closes() {
    let invalidation = InvalidationSignal::new();
    let selections = Arc::new(AtomicUsize::new(0));
    let selections_for_command = Arc::clone(&selections);
    let menu: Element<TestVm> = Menu::new(Button::new("File").size(dp(80.0), dp(28.0)))
        .items(vec![
            MenuItem::new("New"),
            MenuItem::separator(),
            MenuItem::new("Unavailable").disable(true),
            MenuItem::submenu("Empty submenu", Vec::new()),
            MenuItem::checkable("Show toolbar")
                .checked(true)
                .on_select(Command::new(move |_| {
                    selections_for_command.fetch_add(1, Ordering::SeqCst);
                })),
        ])
        .into();
    let menu_id = menu.id;
    let mut handler = test_handler(Some(WidgetTree::new(menu)), invalidation);

    let closed = accessibility_update(&mut handler);
    let closed_trigger = node_for(&closed, menu_id);
    assert_eq!(closed_trigger.has_popup(), Some(HasPopup::Menu));
    assert_eq!(closed_trigger.is_expanded(), Some(false));
    assert!(closed_trigger.supports_action(Action::Click));
    assert!(closed
        .nodes
        .iter()
        .all(|(_, node)| node.role() != Role::Menu));

    assert!(handler.set_menu_open_state(menu_id, true));
    let opened = accessibility_update(&mut handler);
    let opened_trigger = node_for(&opened, menu_id);
    assert_eq!(opened_trigger.has_popup(), Some(HasPopup::Menu));
    assert_eq!(opened_trigger.is_expanded(), Some(true));
    assert!(opened
        .nodes
        .iter()
        .any(|(_, node)| node.role() == Role::Menu));
    assert_eq!(
        opened
            .nodes
            .iter()
            .filter(|(_, node)| { matches!(node.role(), Role::MenuItem | Role::MenuItemCheckBox) })
            .count(),
        4,
        "separator rows must not become accessible menu items"
    );

    let empty_submenu = node_for_id(&opened, node_id_for_label(&opened, "Empty submenu"));
    assert_eq!(empty_submenu.has_popup(), None);
    assert_eq!(empty_submenu.is_expanded(), None);
    assert!(!empty_submenu.supports_action(Action::Expand));
    assert!(!empty_submenu.supports_action(Action::Collapse));

    let unavailable = node_for_id(&opened, node_id_for_label(&opened, "Unavailable"));
    assert_eq!(unavailable.role(), Role::MenuItem);
    assert!(unavailable.is_disabled());
    assert!(!unavailable.supports_action(Action::Focus));
    assert!(!unavailable.supports_action(Action::Click));

    let check_id = node_id_for_label(&opened, "Show toolbar");
    let check = node_for_id(&opened, check_id);
    assert_eq!(check.role(), Role::MenuItemCheckBox);
    assert_eq!(check.toggled(), Some(Toggled::True));
    assert!(check.supports_action(Action::Focus));
    assert!(check.supports_action(Action::Click));

    handler
        .accessibility_action_sender
        .send(action_request_for_node(check_id, Action::Focus, None))
        .unwrap();
    assert!(handler.drain_accessibility_actions());
    assert_eq!(handler.accessibility_focused_node, Some(check_id));
    handler
        .accessibility_action_sender
        .send(action_request_for_node(check_id, Action::Click, None))
        .unwrap();
    assert!(handler.drain_accessibility_actions());
    assert_eq!(selections.load(Ordering::SeqCst), 1);
    let closed_again = accessibility_update(&mut handler);
    assert!(closed_again
        .nodes
        .iter()
        .all(|(_, node)| node.role() != Role::Menu));
}

#[test]
fn accessibility_menu_submenu_expands_and_nested_click_routes_through_unique_fragments() {
    let invalidation = InvalidationSignal::new();
    let selections = Arc::new(AtomicUsize::new(0));
    let selections_for_command = Arc::clone(&selections);
    let menu: Element<TestVm> = Menu::new(Button::new("File").size(dp(80.0), dp(28.0)))
        .items(vec![MenuItem::submenu(
            "Recent",
            vec![MenuItem::new("a.txt").on_select(Command::new(move |_| {
                selections_for_command.fetch_add(1, Ordering::SeqCst);
            }))],
        )])
        .into();
    let menu_id = menu.id;
    let mut handler = test_handler(Some(WidgetTree::new(menu)), invalidation);
    let _ = accessibility_update(&mut handler);
    assert!(handler.set_menu_open_state(menu_id, true));

    let root_open = accessibility_update(&mut handler);
    let recent_id = node_id_for_label(&root_open, "Recent");
    let recent = node_for_id(&root_open, recent_id);
    assert_eq!(recent.role(), Role::MenuItem);
    assert_eq!(recent.has_popup(), Some(HasPopup::Menu));
    assert_eq!(recent.is_expanded(), Some(false));
    assert!(recent.supports_action(Action::Expand));
    assert!(recent.supports_action(Action::Click));

    handler
        .accessibility_action_sender
        .send(action_request_for_node(recent_id, Action::Expand, None))
        .unwrap();
    assert!(handler.drain_accessibility_actions());
    let expanded = accessibility_update(&mut handler);
    let recent = node_for_id(&expanded, node_id_for_label(&expanded, "Recent"));
    assert_eq!(recent.is_expanded(), Some(true));
    assert!(recent.supports_action(Action::Collapse));
    assert_eq!(
        expanded
            .nodes
            .iter()
            .filter(|(_, node)| node.role() == Role::Menu)
            .count(),
        2,
        "root and nested menu fragments must both be reachable"
    );

    let expanded_recent_id = node_id_for_label(&expanded, "Recent");
    handler
        .accessibility_action_sender
        .send(action_request_for_node(
            expanded_recent_id,
            Action::Collapse,
            None,
        ))
        .unwrap();
    assert!(handler.drain_accessibility_actions());
    let collapsed = accessibility_update(&mut handler);
    let collapsed_recent_id = node_id_for_label(&collapsed, "Recent");
    assert_eq!(
        node_for_id(&collapsed, collapsed_recent_id).is_expanded(),
        Some(false)
    );
    assert!(collapsed
        .nodes
        .iter()
        .all(|(_, node)| node.label() != Some("a.txt")));

    handler
        .accessibility_action_sender
        .send(action_request_for_node(
            collapsed_recent_id,
            Action::Click,
            None,
        ))
        .unwrap();
    assert!(handler.drain_accessibility_actions());
    let expanded = accessibility_update(&mut handler);

    let child_id = node_id_for_label(&expanded, "a.txt");
    let child = node_for_id(&expanded, child_id);
    assert_eq!(child.role(), Role::MenuItem);
    assert!(child.supports_action(Action::Click));
    handler
        .accessibility_action_sender
        .send(action_request_for_node(child_id, Action::Click, None))
        .unwrap();
    assert!(handler.drain_accessibility_actions());
    assert_eq!(selections.load(Ordering::SeqCst), 1);
    let closed = accessibility_update(&mut handler);
    assert!(closed
        .nodes
        .iter()
        .all(|(_, node)| node.role() != Role::Menu));
}

#[test]
fn accessibility_context_menu_and_menubar_publish_menu_item_roles() {
    let invalidation = InvalidationSignal::new();
    let context_menu: Element<TestVm> =
        ContextMenu::new(Button::new("Photo").size(dp(90.0), dp(30.0)))
            .items(vec![MenuItem::new("Copy")])
            .into();
    let context_menu_id = context_menu.id;
    let mut context_handler =
        test_handler(Some(WidgetTree::new(context_menu)), invalidation.clone());
    let _ = accessibility_update(&mut context_handler);
    assert!(context_handler.open_context_menu_at(context_menu_id, Point::new(dp(24.0), dp(14.0))));
    let context_open = accessibility_update(&mut context_handler);
    let copy_id = node_id_for_label(&context_open, "Copy");
    assert_eq!(node_for_id(&context_open, copy_id).role(), Role::MenuItem);
    context_handler
        .accessibility_action_sender
        .send(action_request_for_node(copy_id, Action::Click, None))
        .unwrap();
    assert!(context_handler.drain_accessibility_actions());
    let context_closed = accessibility_update(&mut context_handler);
    assert!(context_closed
        .nodes
        .iter()
        .all(|(_, node)| node.role() != Role::Menu));

    let menubar = MenuBar::<TestVm>::uncontrolled().entry("File", vec![MenuItem::new("New")]);
    let mut menubar_handler = test_handler(Some(WidgetTree::new(menubar)), invalidation);
    let viewport = menubar_handler.viewport_rect();
    menubar_handler.cursor_position = Some(Point::new(dp(24.0), dp(14.0)));
    let _ = menubar_handler.handle_hover(viewport);
    menubar_handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);
    let menubar_open = accessibility_update(&mut menubar_handler);
    let new_id = node_id_for_label(&menubar_open, "New");
    assert_eq!(node_for_id(&menubar_open, new_id).role(), Role::MenuItem);
    assert!(menubar_open
        .nodes
        .iter()
        .any(|(_, node)| node.role() == Role::Menu));
}

#[test]
fn accessibility_show_context_menu_action_opens_enabled_owner_and_is_absent_when_disabled() {
    let invalidation = InvalidationSignal::new();
    let context_menu: Element<TestVm> =
        ContextMenu::new(Button::new("Photo").size(dp(90.0), dp(30.0)))
            .items(vec![MenuItem::new("Copy")])
            .into();
    let context_menu_id = context_menu.id;
    let mut handler = test_handler(Some(WidgetTree::new(context_menu)), invalidation.clone());

    let update = accessibility_update(&mut handler);
    let trigger = node_for(&update, context_menu_id);
    assert_eq!(trigger.has_popup(), Some(HasPopup::Menu));
    assert!(trigger.supports_action(Action::ShowContextMenu));
    handler
        .accessibility_action_sender
        .send(action_request(
            context_menu_id,
            Action::ShowContextMenu,
            None,
        ))
        .unwrap();
    assert!(handler.drain_accessibility_actions());
    assert!(handler
        .context_menu_anchor_states
        .contains_key(&context_menu_id));
    assert!(accessibility_update(&mut handler)
        .nodes
        .iter()
        .any(|(_, node)| node.role() == Role::Menu));

    let disabled: Element<TestVm> =
        ContextMenu::new(Button::new("Disabled photo").size(dp(90.0), dp(30.0)))
            .items(vec![MenuItem::new("Copy")])
            .disable(true)
            .into();
    let disabled_id = disabled.id;
    let mut disabled_handler = test_handler(Some(WidgetTree::new(disabled)), invalidation);
    let disabled_update = accessibility_update(&mut disabled_handler);
    assert!(!node_for(&disabled_update, disabled_id).supports_action(Action::ShowContextMenu));
    disabled_handler
        .accessibility_action_sender
        .send(action_request(disabled_id, Action::ShowContextMenu, None))
        .unwrap();
    assert!(!disabled_handler.drain_accessibility_actions());
    assert!(!disabled_handler
        .context_menu_anchor_states
        .contains_key(&disabled_id));
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

    open.set(false);
    handler.request_redraw_if_dirty(Instant::now());
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
fn accessibility_tree_exposes_rich_tooltip_root_and_nested_controls() {
    let invalidation = InvalidationSignal::new();
    let tree = WidgetTree::new(
        Button::<TestVm>::new("Inspect")
            .size(dp(120.0), dp(40.0))
            .tooltip(
                Tooltip::content(
                    Button::new("Tooltip action").on_click(Command::new(|_vm: &mut TestVm| {})),
                )
                .delay(Duration::ZERO),
            ),
    );
    let mut handler = test_handler(Some(tree), invalidation);
    handler.reduced_motion = true;
    handler.cursor_position = Some(Point::new(dp(40.0), dp(20.0)));
    let viewport = handler.viewport_rect();
    assert!(handler.handle_hover(viewport));

    let update = accessibility_update(&mut handler);
    let tooltip_root_id = update
        .nodes
        .iter()
        .find_map(|(id, node)| (node.role() == Role::Tooltip).then_some(*id))
        .expect("rich Tooltip should publish a Tooltip root");
    let nested_button_id = update
        .nodes
        .iter()
        .find_map(|(id, node)| {
            (node.role() == Role::Button && node.label() == Some("Tooltip action")).then_some(*id)
        })
        .expect("rich Tooltip should publish nested controls");

    assert!(has_reachable_node(&update, tooltip_root_id));
    assert!(has_reachable_node(&update, nested_button_id));
    assert!(node_for_id(&update, nested_button_id).supports_action(Action::Click));
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
    assert_eq!(selections.load(Ordering::SeqCst), 0);
    assert_eq!(expands.load(Ordering::SeqCst), 0);
    assert_eq!(checks.load(Ordering::SeqCst), 1);
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

#[test]
fn clickable_avatar_and_card_publish_button_semantics_only_when_interactive() {
    let invalidation = InvalidationSignal::new();
    let activations = Arc::new(AtomicUsize::new(0));

    let avatar_activations = Arc::clone(&activations);
    let avatar: Element<TestVm> = crate::ui::widget::Avatar::initials("TG")
        .on_click(Command::new(move |_: &mut TestVm| {
            avatar_activations.fetch_add(1, Ordering::SeqCst);
        }))
        .size(dp(64.0), dp(64.0))
        .into();
    let avatar_id = avatar.id;

    let card_activations = Arc::clone(&activations);
    let card: Element<TestVm> = crate::ui::widget::Card::new()
        .body(Text::new("Open details"))
        .on_click(Command::new(move |_: &mut TestVm| {
            card_activations.fetch_add(1, Ordering::SeqCst);
        }))
        .size(dp(180.0), dp(64.0))
        .into();
    let card_id = card.id;

    let static_avatar: Element<TestVm> = crate::ui::widget::Avatar::initials("ST")
        .size(dp(64.0), dp(64.0))
        .into();
    let static_avatar_id = static_avatar.id;
    let static_card: Element<TestVm> = crate::ui::widget::Card::new()
        .body(Text::new("Preview only"))
        .size(dp(180.0), dp(64.0))
        .into();
    let static_card_id = static_card.id;

    let tree = WidgetTree::new(Flex::new(Axis::Vertical).child([
        avatar,
        card,
        static_avatar,
        static_card,
    ]));
    let mut handler = test_handler_with_config(
        TestVm,
        Some(tree),
        invalidation,
        test_config_with_size(320.0, 320.0),
    );
    let update = accessibility_update(&mut handler);

    for id in [avatar_id, card_id] {
        let node = node_for(&update, id);
        assert_eq!(node.role(), Role::Button);
        assert!(node.supports_action(Action::Focus));
        assert!(node.supports_action(Action::Click));
    }
    for id in [static_avatar_id, static_card_id] {
        assert!(!node_for(&update, id).supports_action(Action::Click));
    }
    assert_ne!(node_for(&update, static_avatar_id).role(), Role::Button);
    assert_ne!(node_for(&update, static_card_id).role(), Role::Button);

    handler
        .accessibility_action_sender
        .send(action_request(card_id, Action::Click, None))
        .unwrap();
    assert!(handler.drain_accessibility_actions());
    assert_eq!(activations.load(Ordering::SeqCst), 1);
}
