use std::collections::{HashMap, HashSet};

use accesskit::{
    Action, Node, NodeId, Rect as AccessRect, Role, Toggled, Tree, TreeId, TreeUpdate,
};

use crate::ui::widget::{
    ComputedScene, HitInteraction, HitRegion, Rect, ResolvedElement, ResolvedSceneLayout,
    ResolvedWidgetKind, WidgetId,
};

pub(crate) const ROOT_NODE_ID: NodeId = NodeId(0);
const WIDGET_NODE_OFFSET: u64 = 1;

pub(crate) fn node_id_from_widget(widget_id: WidgetId) -> NodeId {
    NodeId(widget_id.raw().saturating_add(WIDGET_NODE_OFFSET))
}

pub(crate) fn widget_id_from_node(node_id: NodeId) -> Option<WidgetId> {
    (node_id.0 >= WIDGET_NODE_OFFSET).then(|| WidgetId::from_raw(node_id.0 - WIDGET_NODE_OFFSET))
}

pub(crate) fn build_tree_update<VM: 'static>(
    layout: Option<&ResolvedSceneLayout<VM>>,
    computed: &ComputedScene<VM>,
    focused_widget: Option<WidgetId>,
    viewport: Rect,
) -> TreeUpdate {
    let mut root = Node::new(Role::Window);
    root.set_bounds(access_rect(viewport));
    root.set_label("tgui window");

    let Some(layout) = layout else {
        return TreeUpdate {
            nodes: vec![(ROOT_NODE_ID, root)],
            tree: Some(tree_metadata()),
            tree_id: TreeId::ROOT,
            focus: ROOT_NODE_ID,
        };
    };

    let hit_regions = hit_regions_by_widget(computed);
    let trap_scope = computed
        .focus_scopes
        .iter()
        .rev()
        .find(|scope| scope.active && scope.options.is_trap())
        .map(|scope| scope.path.clone());
    let trap_widget = trap_scope.as_ref().and_then(|path| path.last().copied());
    let trap_path = trap_widget.and_then(|id| layout.path_for(id).map(|path| path.to_vec()));

    let mut nodes = Vec::new();
    let mut included = HashSet::new();
    let root_children: Vec<NodeId> = collect_widget(
        layout,
        computed,
        &hit_regions,
        trap_scope.as_deref(),
        trap_path.as_deref(),
        layout.root_id(),
        &mut nodes,
        &mut included,
    )
    .into_iter()
    .collect();
    root.set_children(root_children);
    nodes.push((ROOT_NODE_ID, root));
    included.insert(ROOT_NODE_ID);

    let focus = focused_widget
        .map(node_id_from_widget)
        .filter(|node_id| included.contains(node_id))
        .unwrap_or(ROOT_NODE_ID);

    TreeUpdate {
        nodes,
        tree: Some(tree_metadata()),
        tree_id: TreeId::ROOT,
        focus,
    }
}

fn tree_metadata() -> Tree {
    let mut tree = Tree::new(ROOT_NODE_ID);
    tree.toolkit_name = Some("tgui".to_string());
    tree.toolkit_version = Some(env!("CARGO_PKG_VERSION").to_string());
    tree
}

fn collect_widget<VM: 'static>(
    layout: &ResolvedSceneLayout<VM>,
    computed: &ComputedScene<VM>,
    hit_regions: &HashMap<WidgetId, Vec<&HitRegion<VM>>>,
    trap_scope: Option<&[WidgetId]>,
    trap_path: Option<&[usize]>,
    widget_id: WidgetId,
    nodes: &mut Vec<(NodeId, Node)>,
    included: &mut HashSet<NodeId>,
) -> Option<NodeId> {
    if !is_visible_to_accessibility(layout, trap_scope, trap_path, widget_id) {
        return None;
    }
    let resolved = layout.resolved_widget(widget_id)?;
    let node_id = node_id_from_widget(widget_id);
    let mut node = node_for_widget(
        resolved,
        computed,
        hit_regions.get(&widget_id).map(Vec::as_slice),
    );
    if let Some(bounds) = widget_bounds(
        layout,
        hit_regions.get(&widget_id).map(Vec::as_slice),
        widget_id,
    ) {
        node.set_bounds(access_rect(bounds));
    }
    let children = children_of(&resolved.kind)
        .iter()
        .filter_map(|child| {
            collect_widget(
                layout,
                computed,
                hit_regions,
                trap_scope,
                trap_path,
                child.id,
                nodes,
                included,
            )
        })
        .collect::<Vec<_>>();
    if !children.is_empty() {
        node.set_children(children);
    }
    nodes.push((node_id, node));
    included.insert(node_id);
    Some(node_id)
}

fn is_visible_to_accessibility<VM: 'static>(
    layout: &ResolvedSceneLayout<VM>,
    trap_scope: Option<&[WidgetId]>,
    trap_path: Option<&[usize]>,
    widget_id: WidgetId,
) -> bool {
    let Some(scope) = trap_scope else {
        return true;
    };
    if scope.contains(&widget_id) {
        return true;
    }
    let Some(trap_path) = trap_path else {
        return true;
    };
    layout
        .path_for(widget_id)
        .map(|path| path.starts_with(trap_path) || trap_path.starts_with(path))
        .unwrap_or(false)
}

fn node_for_widget<VM: 'static>(
    resolved: &ResolvedElement<VM>,
    computed: &ComputedScene<VM>,
    regions: Option<&[&HitRegion<VM>]>,
) -> Node {
    let mut node = Node::new(role_for_widget(resolved));
    if let Some(key) = resolved.key.as_ref() {
        node.set_author_id(format!("{key:?}"));
    }
    if resolved
        .focus
        .focusable
        .unwrap_or_else(|| regions.is_some_and(|regions| regions.iter().any(|r| r.focus.is_some())))
    {
        node.add_action(Action::Focus);
    }
    apply_widget_semantics(&mut node, resolved, computed);
    apply_hit_actions(&mut node, regions);
    node
}

fn role_for_widget<VM>(resolved: &ResolvedElement<VM>) -> Role {
    if resolved.data_grid_header.is_some() {
        return Role::ColumnHeader;
    }
    if resolved.data_grid_cell.is_some() {
        return Role::GridCell;
    }
    if resolved.data_grid_root.is_some() {
        return Role::Grid;
    }
    if resolved.tree_node.is_some() {
        return Role::TreeItem;
    }
    if resolved.tree_root.is_some() {
        return Role::Tree;
    }
    if resolved.list_item.is_some() {
        return Role::ListBoxOption;
    }
    if resolved
        .modal
        .as_ref()
        .is_some_and(|modal| modal.open.resolve())
    {
        return Role::Dialog;
    }
    if resolved
        .drawer
        .as_ref()
        .is_some_and(|drawer| drawer.open.resolve())
    {
        return Role::Dialog;
    }
    if resolved.tab_trigger.is_some() {
        return Role::Tab;
    }
    match &resolved.kind {
        ResolvedWidgetKind::Container { layout, .. } if layout.scroll_view.is_some() => {
            Role::ScrollView
        }
        ResolvedWidgetKind::Virtual { children, .. }
            if children.iter().any(|child| child.list_item.is_some()) =>
        {
            Role::ListBox
        }
        ResolvedWidgetKind::Container { .. }
        | ResolvedWidgetKind::Virtual { .. }
        | ResolvedWidgetKind::Portal { .. } => Role::GenericContainer,
        ResolvedWidgetKind::Text { .. } => Role::TextRun,
        ResolvedWidgetKind::Image { .. } => Role::Image,
        ResolvedWidgetKind::Canvas { .. } => Role::Canvas,
        ResolvedWidgetKind::Button { .. } => Role::Button,
        ResolvedWidgetKind::Checkbox { .. } => Role::CheckBox,
        ResolvedWidgetKind::Radio { .. } => Role::RadioButton,
        ResolvedWidgetKind::Switch { .. } => Role::Switch,
        ResolvedWidgetKind::Select { .. } => Role::ComboBox,
        ResolvedWidgetKind::SelectOptionRow { .. } => Role::ListBoxOption,
        ResolvedWidgetKind::Slider { .. } => Role::Slider,
        ResolvedWidgetKind::ProgressBar { .. } => Role::ProgressIndicator,
        ResolvedWidgetKind::Spinner { .. } => Role::ProgressIndicator,
        ResolvedWidgetKind::Divider { .. } => Role::Splitter,
        ResolvedWidgetKind::TextEditor { multiline, .. } => {
            if *multiline {
                Role::MultilineTextInput
            } else {
                Role::TextInput
            }
        }
        ResolvedWidgetKind::ToastHost { .. } => Role::Status,
        #[cfg(feature = "audio")]
        ResolvedWidgetKind::Audio { .. } => Role::Audio,
        #[cfg(feature = "video")]
        ResolvedWidgetKind::VideoSurface { .. } => Role::Video,
    }
}

fn apply_widget_semantics<VM: 'static>(
    node: &mut Node,
    resolved: &ResolvedElement<VM>,
    computed: &ComputedScene<VM>,
) {
    if resolved
        .modal
        .as_ref()
        .is_some_and(|modal| modal.open.resolve())
        || resolved
            .drawer
            .as_ref()
            .is_some_and(|drawer| drawer.open.resolve())
    {
        node.set_modal();
    }

    if let Some(list_item) = resolved.list_item.as_ref() {
        node.set_selected(list_item.selected_keys.resolve().contains(&list_item.key));
        node.set_position_in_set(list_item.item_index + 1);
        node.set_size_of_set(list_item.sibling_keys.len());
        node.add_action(Action::Click);
        if list_item.disabled.resolve() {
            node.set_disabled();
        }
    }

    if let Some(tree_node) = resolved.tree_node.as_ref() {
        node.set_selected(tree_node.selected_keys.resolve().contains(&tree_node.key));
        node.set_level(tree_node.depth + 1);
        node.set_position_in_set(tree_node.position_in_set);
        node.set_size_of_set(tree_node.set_size);
        if tree_node.has_children {
            node.set_expanded(tree_node.expanded);
        }
        if tree_node.checkable.resolve() {
            node.set_toggled(match tree_node.check_state {
                crate::ui::widget::TreeCheckState::Unchecked => Toggled::False,
                crate::ui::widget::TreeCheckState::Checked => Toggled::True,
                crate::ui::widget::TreeCheckState::Indeterminate => Toggled::Mixed,
            });
        }
        node.add_action(Action::Click);
        if tree_node.disabled.resolve() {
            node.set_disabled();
        }
    }

    if let Some(root) = resolved.tree_root.as_ref() {
        node.set_size_of_set(root.node_count);
        if root.selection_mode == crate::ui::widget::TreeSelectionMode::Multiple {
            node.set_multiselectable();
        }
    }

    if let Some(root) = resolved.data_grid_root.as_ref() {
        node.set_row_count(root.row_count);
        node.set_column_count(root.column_count);
        if root.selection_mode == crate::ui::widget::DataGridSelectionMode::Multiple {
            node.set_multiselectable();
        }
    }

    if let Some(header) = resolved.data_grid_header.as_ref() {
        node.set_label(header.label.clone());
        node.set_column_index(header.column_index);
        node.set_column_span(1);
        node.add_action(Action::Click);
    }

    if let Some(cell) = resolved.data_grid_cell.as_ref() {
        node.set_row_index(cell.row_index);
        node.set_column_index(cell.column_index);
        node.set_selected(cell.selected_keys.resolve().contains(&cell.row_key));
        node.add_action(Action::Click);
        if cell.disabled.resolve() {
            node.set_disabled();
        }
    }

    match &resolved.kind {
        ResolvedWidgetKind::Container { layout, .. } if layout.scroll_view.is_some() => {
            apply_scroll_region(node, computed, resolved.id);
        }
        ResolvedWidgetKind::Virtual { children, .. } => {
            if let Some(tree_node) = children.iter().find_map(|child| child.tree_node.as_ref()) {
                node.set_size_of_set(tree_node.visible_keys.len());
                if tree_node.selection_mode == crate::ui::widget::TreeSelectionMode::Multiple {
                    node.set_multiselectable();
                }
            } else if let Some(list_item) =
                children.iter().find_map(|child| child.list_item.as_ref())
            {
                node.set_size_of_set(list_item.sibling_keys.len());
                if list_item.selection_mode == crate::ui::widget::ListSelectionMode::Multiple {
                    node.set_multiselectable();
                }
            }
        }
        ResolvedWidgetKind::Text { text } => {
            node.set_value(text.content.resolve());
        }
        ResolvedWidgetKind::Button {
            label, disabled, ..
        } => {
            node.set_label(label.resolve());
            node.add_action(Action::Click);
            if disabled.resolve() {
                node.set_disabled();
            }
        }
        ResolvedWidgetKind::Checkbox {
            checked,
            label,
            disabled,
            ..
        } => {
            if let Some(label) = label {
                node.set_label(label.resolve());
            }
            node.set_toggled(Toggled::from(checked.resolve()));
            node.add_action(Action::Click);
            if disabled.resolve() {
                node.set_disabled();
            }
        }
        ResolvedWidgetKind::Radio {
            checked,
            label,
            disabled,
            ..
        } => {
            if let Some(label) = label {
                node.set_label(label.resolve());
            }
            node.set_selected(checked.resolve());
            node.add_action(Action::Click);
            if disabled.resolve() {
                node.set_disabled();
            }
        }
        ResolvedWidgetKind::Switch {
            checked, disabled, ..
        } => {
            node.set_toggled(Toggled::from(checked.resolve()));
            node.add_action(Action::Click);
            if disabled.resolve() {
                node.set_disabled();
            }
        }
        ResolvedWidgetKind::Select {
            selected_label,
            placeholder,
            open,
            disabled,
            ..
        } => {
            match selected_label.resolve() {
                Some(label) if !label.is_empty() => node.set_value(label),
                _ => node.set_placeholder(placeholder.resolve()),
            }
            if let Some(open) = open {
                node.set_expanded(open.resolve());
            }
            node.add_action(Action::Click);
            if disabled.resolve() {
                node.set_disabled();
            }
        }
        ResolvedWidgetKind::Slider {
            value,
            min,
            max,
            step,
            disabled,
            ..
        } => {
            let current = value.resolve().clamp(*min, *max);
            node.set_numeric_value(current as f64);
            node.set_min_numeric_value(*min as f64);
            node.set_max_numeric_value(*max as f64);
            node.set_numeric_value_step(*step as f64);
            node.add_action(Action::Increment);
            node.add_action(Action::Decrement);
            node.add_action(Action::SetValue);
            if disabled.resolve() {
                node.set_disabled();
            }
        }
        ResolvedWidgetKind::ProgressBar {
            value,
            indeterminate,
            label,
            ..
        } => {
            if let Some(label) = label {
                node.set_label(label.resolve());
            }
            if !indeterminate.resolve() {
                node.set_numeric_value(value.resolve() as f64);
                node.set_min_numeric_value(0.0);
                node.set_max_numeric_value(1.0);
            }
        }
        ResolvedWidgetKind::Divider { label, .. } => {
            if let Some(label) = label {
                node.set_label(label.resolve());
            }
        }
        ResolvedWidgetKind::TextEditor {
            controller,
            placeholder,
            disabled,
            ..
        } => {
            node.set_value(controller.text());
            node.set_placeholder(placeholder.resolve());
            node.add_action(Action::SetValue);
            if disabled.resolve() {
                node.set_disabled();
            }
        }
        _ => {}
    }
}

fn apply_hit_actions<VM>(node: &mut Node, regions: Option<&[&HitRegion<VM>]>) {
    let Some(regions) = regions else { return };
    if regions.iter().any(|region| region.focus.is_some()) {
        node.add_action(Action::Focus);
    }
    for region in regions {
        match &region.interaction {
            HitInteraction::Widget { interactions, .. } => {
                if interactions.on_click.is_some() {
                    node.add_action(Action::Click);
                }
            }
            HitInteraction::Checkbox { .. }
            | HitInteraction::Radio { .. }
            | HitInteraction::Switch { .. }
            | HitInteraction::SelectTrigger { .. }
            | HitInteraction::TabTrigger { .. }
            | HitInteraction::ListItem { .. }
            | HitInteraction::TreeNode { .. }
            | HitInteraction::TreeDisclosure { .. }
            | HitInteraction::TreeCheckbox { .. } => {
                node.add_action(Action::Click);
            }
            HitInteraction::Slider { .. } => {
                node.add_action(Action::Increment);
                node.add_action(Action::Decrement);
                node.add_action(Action::SetValue);
            }
            HitInteraction::TextInput { .. } => {
                node.add_action(Action::SetValue);
            }
            _ => {}
        }
    }
}

fn apply_scroll_region<VM>(node: &mut Node, computed: &ComputedScene<VM>, widget_id: WidgetId) {
    let Some(region) = computed
        .scroll_regions
        .iter()
        .find(|region| region.id == widget_id)
    else {
        return;
    };
    let max = region.max_offset();
    if region.can_scroll_x() {
        node.set_scroll_x(region.scroll_offset.x.get() as f64);
        node.set_scroll_x_min(0.0);
        node.set_scroll_x_max(max.x.get() as f64);
    }
    if region.can_scroll_y() {
        node.set_scroll_y(region.scroll_offset.y.get() as f64);
        node.set_scroll_y_min(0.0);
        node.set_scroll_y_max(max.y.get() as f64);
    }
}

fn children_of<VM>(kind: &ResolvedWidgetKind<VM>) -> &[ResolvedElement<VM>] {
    match kind {
        ResolvedWidgetKind::Container { children, .. }
        | ResolvedWidgetKind::Virtual { children, .. } => children.as_slice(),
        _ => &[],
    }
}

fn widget_bounds<VM: 'static>(
    layout: &ResolvedSceneLayout<VM>,
    regions: Option<&[&HitRegion<VM>]>,
    widget_id: WidgetId,
) -> Option<Rect> {
    regions
        .and_then(|regions| regions.first().map(|region| region.rect))
        .or_else(|| layout.widget_bounds(widget_id))
}

fn hit_regions_by_widget<VM>(
    computed: &ComputedScene<VM>,
) -> HashMap<WidgetId, Vec<&HitRegion<VM>>> {
    let mut regions: HashMap<WidgetId, Vec<&HitRegion<VM>>> = HashMap::new();
    for region in computed
        .hit_regions
        .iter()
        .chain(computed.overlay_hit_regions.iter())
    {
        if let Some(widget_id) = hit_widget_id(&region.interaction) {
            regions.entry(widget_id).or_default().push(region);
        }
        if let Some(focus) = region.focus.as_ref() {
            regions.entry(focus.widget_id).or_default().push(region);
        }
    }
    regions
}

fn hit_widget_id<VM>(interaction: &HitInteraction<VM>) -> Option<WidgetId> {
    match interaction {
        HitInteraction::Occluder { id }
        | HitInteraction::Disabled { id }
        | HitInteraction::Widget { id, .. }
        | HitInteraction::SelectableText { id, .. }
        | HitInteraction::Switch { id, .. }
        | HitInteraction::Checkbox { id, .. }
        | HitInteraction::Radio { id, .. }
        | HitInteraction::SelectTrigger { id, .. }
        | HitInteraction::SelectOption { id, .. }
        | HitInteraction::TabTrigger { id, .. }
        | HitInteraction::ListItem { id, .. }
        | HitInteraction::TreeNode { id, .. }
        | HitInteraction::TreeDisclosure { id, .. }
        | HitInteraction::TreeCheckbox { id, .. }
        | HitInteraction::DataGridCell { id, .. }
        | HitInteraction::DataGridHeader { id, .. }
        | HitInteraction::DataGridResizeHandle { id, .. }
        | HitInteraction::Slider { id, .. }
        | HitInteraction::TextInput { id, .. }
        | HitInteraction::CanvasItem { id, .. } => Some(*id),
    }
}

fn access_rect(rect: Rect) -> AccessRect {
    AccessRect {
        x0: rect.x.get() as f64,
        y0: rect.y.get() as f64,
        x1: (rect.x + rect.width).get() as f64,
        y1: (rect.y + rect.height).get() as f64,
    }
}
