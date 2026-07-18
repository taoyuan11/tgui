use std::collections::{HashMap, HashSet};

use accesskit::{
    Action, Node, NodeId, Rect as AccessRect, Role, Toggled, Tree, TreeId, TreeUpdate,
};

use crate::runtime::overlay::OverlayId;
use crate::ui::widget::{
    AccessibilityFragment, ComputedScene, HitInteraction, HitRegion, Rect, ResolvedElement,
    ResolvedSceneLayout, ResolvedWidgetKind, ScrollRegion, WidgetId,
};
use smallvec::SmallVec;

pub(crate) const ROOT_NODE_ID: NodeId = NodeId(0);
const WIDGET_NODE_OFFSET: u64 = 1;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct PortalAccessibilityNodeKey {
    target_window_instance_id: u64,
    source_window_instance_id: u64,
    source_publication_generation: Option<u64>,
    owner_path: Vec<u64>,
    widget_id: WidgetId,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PortalAccessibilityNodeRoute {
    pub(crate) source_window_instance_id: u64,
    pub(crate) source_publication_generation: Option<u64>,
    pub(crate) owner_path: SmallVec<[OverlayId; 2]>,
    pub(crate) resolved_path: SmallVec<[usize; 4]>,
    pub(crate) widget_id: WidgetId,
}

pub(crate) struct AccessibilityNodeRegistry {
    active_by_key: HashMap<PortalAccessibilityNodeKey, NodeId>,
    seen_keys: HashSet<PortalAccessibilityNodeKey>,
    live_routes: HashMap<NodeId, PortalAccessibilityNodeRoute>,
    live_local_node_ids: HashSet<NodeId>,
    next_node_id: u64,
}

impl Default for AccessibilityNodeRegistry {
    fn default() -> Self {
        Self {
            active_by_key: HashMap::new(),
            seen_keys: HashSet::new(),
            live_routes: HashMap::new(),
            live_local_node_ids: HashSet::new(),
            next_node_id: u64::MAX,
        }
    }
}

impl AccessibilityNodeRegistry {
    pub(crate) fn begin_update(&mut self) {
        self.seen_keys.clear();
        self.live_routes.clear();
    }

    pub(crate) fn finish_update(&mut self, included: &HashSet<NodeId>) {
        self.active_by_key
            .retain(|key, _| self.seen_keys.contains(key));
        self.live_local_node_ids.clear();
        self.live_local_node_ids.extend(
            included
                .iter()
                .copied()
                .filter(|node_id| !self.live_routes.contains_key(node_id)),
        );
    }

    pub(crate) fn live_route(&self, node_id: NodeId) -> Option<&PortalAccessibilityNodeRoute> {
        self.live_routes.get(&node_id)
    }

    pub(crate) fn live_routes(
        &self,
    ) -> impl Iterator<Item = (NodeId, &PortalAccessibilityNodeRoute)> {
        self.live_routes
            .iter()
            .map(|(node_id, route)| (*node_id, route))
    }

    pub(crate) fn is_live_local_node_id(&self, node_id: NodeId) -> bool {
        self.live_local_node_ids.contains(&node_id)
    }

    fn node_id_for(
        &mut self,
        target_window_instance_id: u64,
        route: PortalAccessibilityNodeRoute,
        reserved_ids: &HashSet<NodeId>,
    ) -> Option<NodeId> {
        let key = PortalAccessibilityNodeKey {
            target_window_instance_id,
            source_window_instance_id: route.source_window_instance_id,
            source_publication_generation: route.source_publication_generation,
            owner_path: route.owner_path.iter().map(|id| id.0).collect(),
            widget_id: route.widget_id,
        };
        if !self.seen_keys.insert(key.clone()) {
            return None;
        }
        let node_id = if let Some(node_id) = self
            .active_by_key
            .get(&key)
            .copied()
            .filter(|node_id| !reserved_ids.contains(node_id))
        {
            node_id
        } else {
            let node_id = loop {
                let candidate = NodeId(self.next_node_id);
                self.next_node_id = self
                    .next_node_id
                    .checked_sub(1)
                    .expect("Portal accessibility NodeId space exhausted");
                if candidate != ROOT_NODE_ID && !reserved_ids.contains(&candidate) {
                    break candidate;
                }
            };
            self.active_by_key.insert(key.clone(), node_id);
            node_id
        };
        self.live_routes.insert(node_id, route);
        Some(node_id)
    }
}

/// Runtime state that can change the contents or topology of the AccessKit tree.
///
/// A changed key always rebuilds the complete tree. Paint-only hover/animation state is excluded:
/// it cannot change any node emitted below. Scene-only offset/scale animation has its own geometry
/// epoch, while the scroll epoch participates only when the computed scene has an actually
/// scrollable region. Keeping the key value-based (rather than hashing it) also avoids correctness
/// depending on hash collision resistance.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct TreeUpdateKey {
    pub(crate) invalidation_revision: u64,
    pub(crate) scene_serial: u64,
    pub(crate) viewport: Rect,
    pub(crate) theme_epoch: u64,
    pub(crate) style_sheet_version: u64,
    pub(crate) density: crate::ui::theme::Density,
    pub(crate) reduced_motion: bool,
    pub(crate) text_scale_bits: u32,
    pub(crate) accessibility_animation_epoch: u64,
    pub(crate) scroll_epoch: Option<u64>,
    pub(crate) text_input_epoch: u64,
    pub(crate) external_portal_revision: u64,
}

pub(crate) fn node_id_from_widget(widget_id: WidgetId) -> NodeId {
    NodeId(widget_id.raw().saturating_add(WIDGET_NODE_OFFSET))
}

pub(crate) fn widget_id_from_node(node_id: NodeId) -> Option<WidgetId> {
    (node_id.0 >= WIDGET_NODE_OFFSET).then(|| WidgetId::from_raw(node_id.0 - WIDGET_NODE_OFFSET))
}

#[cfg(test)]
pub(crate) fn build_tree_update<VM: 'static>(
    layout: Option<&ResolvedSceneLayout<VM>>,
    computed: &ComputedScene<VM>,
    focused_widget: Option<WidgetId>,
    viewport: Rect,
) -> TreeUpdate {
    let mut registry = AccessibilityNodeRegistry::default();
    build_tree_update_with_registry(
        layout,
        computed,
        focused_widget.map(node_id_from_widget),
        viewport,
        0,
        &mut registry,
    )
}

pub(crate) fn build_tree_update_with_registry<VM: 'static>(
    layout: Option<&ResolvedSceneLayout<VM>>,
    computed: &ComputedScene<VM>,
    candidate_focus: Option<NodeId>,
    viewport: Rect,
    target_window_instance_id: u64,
    registry: &mut AccessibilityNodeRegistry,
) -> TreeUpdate {
    registry.begin_update();
    let mut root = Node::new(Role::Window);
    root.set_bounds(access_rect(viewport));
    root.set_label("tgui window");

    let hit_regions = hit_regions_by_widget(computed);
    let trap_scope = computed
        .focus_scopes
        .iter()
        .rev()
        .find(|scope| scope.active && scope.options.is_trap())
        .map(|scope| scope.path.clone());
    let trap_widget = trap_scope.as_ref().and_then(|path| path.last().copied());
    let trap_path = trap_widget
        .and_then(|id| layout.and_then(|layout| layout.path_for(id).map(|path| path.to_vec())));

    let mut nodes = Vec::new();
    let mut included = HashSet::new();
    let mut root_children = layout
        .and_then(|layout| {
            if trap_scope.is_some() && trap_path.is_none() {
                return None;
            }
            collect_widget(
                layout,
                computed,
                &hit_regions,
                trap_scope.as_deref(),
                trap_path.as_deref(),
                layout.root_id(),
                &mut nodes,
                &mut included,
            )
        })
        .into_iter()
        .collect::<Vec<_>>();
    let mut fragment_owner_counts = HashMap::<(u64, Vec<u64>), usize>::new();
    for fragment in &computed.accessibility_fragments {
        if !accessibility_fragment_source_is_open(fragment) {
            continue;
        }
        *fragment_owner_counts
            .entry((
                fragment
                    .source_window_instance_id
                    .unwrap_or(target_window_instance_id),
                fragment.owner_path.iter().map(|id| id.0).collect(),
            ))
            .or_default() += 1;
    }
    for fragment in &computed.accessibility_fragments {
        if !accessibility_fragment_source_is_open(fragment) {
            continue;
        }
        let owner_key = (
            fragment
                .source_window_instance_id
                .unwrap_or(target_window_instance_id),
            fragment
                .owner_path
                .iter()
                .map(|id| id.0)
                .collect::<Vec<_>>(),
        );
        if fragment_owner_counts.get(&owner_key).copied() != Some(1) {
            continue;
        }
        if let Some(node_id) = collect_accessibility_fragment(
            fragment,
            computed,
            trap_scope.as_deref(),
            target_window_instance_id,
            registry,
            &mut nodes,
            &mut included,
        ) {
            root_children.push(node_id);
        }
    }
    root.set_children(root_children);
    nodes.push((ROOT_NODE_ID, root));
    included.insert(ROOT_NODE_ID);

    let focus = candidate_focus
        .filter(|node_id| included.contains(node_id))
        .unwrap_or(ROOT_NODE_ID);

    let update = TreeUpdate {
        nodes,
        tree: Some(tree_metadata()),
        tree_id: TreeId::ROOT,
        focus,
    };
    registry.finish_update(&included);
    update
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
        None,
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

fn collect_accessibility_fragment<VM: 'static>(
    fragment: &AccessibilityFragment<VM>,
    computed: &ComputedScene<VM>,
    trap_scope: Option<&[WidgetId]>,
    target_window_instance_id: u64,
    registry: &mut AccessibilityNodeRegistry,
    nodes: &mut Vec<(NodeId, Node)>,
    included: &mut HashSet<NodeId>,
) -> Option<NodeId> {
    if !accessibility_fragment_source_is_open(fragment)
        || fragment.has_duplicate_widget_ids
        || fragment.clip_rect.is_some_and(Rect::is_empty)
        || trap_scope.is_some_and(|trap| !fragment.scope_path.starts_with(trap))
    {
        return None;
    }
    collect_accessibility_fragment_node(
        fragment,
        computed,
        0,
        target_window_instance_id,
        registry,
        nodes,
        included,
    )
}

fn accessibility_fragment_source_is_open<VM>(fragment: &AccessibilityFragment<VM>) -> bool {
    fragment
        .source_open
        .as_ref()
        .map(crate::ui::layout::Value::resolve_untracked)
        .unwrap_or(true)
}

fn collect_accessibility_fragment_node<VM: 'static>(
    fragment: &AccessibilityFragment<VM>,
    computed: &ComputedScene<VM>,
    node_index: usize,
    target_window_instance_id: u64,
    registry: &mut AccessibilityNodeRegistry,
    nodes: &mut Vec<(NodeId, Node)>,
    included: &mut HashSet<NodeId>,
) -> Option<NodeId> {
    let fragment_node = fragment.nodes.get(node_index)?;
    let resolved = resolved_at_fragment_path(
        fragment.resolved_root.as_ref(),
        &fragment_node.resolved_path,
    )?;
    let visible_bounds = accessibility_fragment_node_visible_bounds(fragment, node_index)?;
    let route = PortalAccessibilityNodeRoute {
        source_window_instance_id: fragment
            .source_window_instance_id
            .unwrap_or(target_window_instance_id),
        source_publication_generation: fragment.source_publication_generation,
        owner_path: fragment.owner_path.clone(),
        resolved_path: fragment_node.resolved_path.clone(),
        widget_id: fragment_node.widget_id,
    };
    let node_id = registry.node_id_for(target_window_instance_id, route, included)?;
    let hit_regions = fragment_node
        .hits
        .iter()
        .filter(|hit| {
            accessibility_fragment_hit_visible_bounds(fragment, node_index, hit).is_some()
        })
        .collect::<Vec<_>>();
    let mut node = node_for_widget(
        resolved,
        computed,
        Some(hit_regions.as_slice()),
        Some(fragment_node.scroll_regions.as_slice()),
    );
    node.set_bounds(access_rect(visible_bounds));
    let children = fragment_node
        .children
        .iter()
        .filter_map(|(_, child_index)| {
            collect_accessibility_fragment_node(
                fragment,
                computed,
                *child_index,
                target_window_instance_id,
                registry,
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

pub(crate) fn accessibility_fragment_node_visible_bounds<VM>(
    fragment: &AccessibilityFragment<VM>,
    node_index: usize,
) -> Option<Rect> {
    let node = fragment.nodes.get(node_index)?;
    if !node.hits.is_empty() {
        return node
            .hits
            .iter()
            .find_map(|hit| accessibility_fragment_hit_visible_bounds(fragment, node_index, hit));
    }
    let bounds = node.bounds;
    if bounds.is_empty() {
        return (node.clip_rect.is_none() && fragment.clip_rect.is_none()).then_some(bounds);
    }
    let mut visible = match node.clip_rect {
        Some(clip) => bounds.intersect(clip)?,
        None => bounds,
    };
    if let Some(clip) = fragment.clip_rect {
        visible = visible.intersect(clip)?;
    }
    Some(visible)
}

pub(crate) fn accessibility_fragment_hit_visible_bounds<VM>(
    fragment: &AccessibilityFragment<VM>,
    node_index: usize,
    hit: &HitRegion<VM>,
) -> Option<Rect> {
    if hit.rect.is_empty() {
        return None;
    }
    let mut bounds = match hit.clip_rect {
        Some(clip) => hit.rect.intersect(clip)?,
        None => hit.rect,
    };
    if let Some(clip) = fragment.nodes.get(node_index)?.clip_rect {
        bounds = bounds.intersect(clip)?;
    }
    if let Some(clip) = fragment.clip_rect {
        bounds = bounds.intersect(clip)?;
    }
    Some(bounds)
}

/// Resolves a route against the current fragment and applies the same node/ancestor clipping
/// rules as the tree builder. `resolved_path` is a fast exact lookup; the WidgetId fallback keeps
/// a stable synthetic NodeId live when unrelated siblings move between two AccessKit updates.
pub(crate) fn live_accessibility_fragment_node_index<VM>(
    fragment: &AccessibilityFragment<VM>,
    resolved_path: &[usize],
    widget_id: WidgetId,
) -> Option<usize> {
    if fragment.has_duplicate_widget_ids || fragment.nodes.is_empty() {
        return None;
    }
    let node_index = fragment
        .nodes
        .iter()
        .position(|node| {
            node.widget_id == widget_id && node.resolved_path.as_slice() == resolved_path
        })
        .or_else(|| {
            fragment
                .nodes
                .iter()
                .position(|node| node.widget_id == widget_id)
        })?;
    let target_path = fragment.nodes.get(node_index)?.resolved_path.as_slice();
    let mut current_index = 0usize;
    accessibility_fragment_node_visible_bounds(fragment, current_index)?;
    for child_position in target_path {
        current_index = fragment
            .nodes
            .get(current_index)?
            .children
            .iter()
            .find_map(|(resolved_child_index, node_index)| {
                (*resolved_child_index == *child_position).then_some(*node_index)
            })?;
        accessibility_fragment_node_visible_bounds(fragment, current_index)?;
    }
    (current_index == node_index).then_some(node_index)
}

fn resolved_at_fragment_path<'a, VM>(
    root: &'a ResolvedElement<VM>,
    path: &[usize],
) -> Option<&'a ResolvedElement<VM>> {
    let mut resolved = root;
    for child_index in path {
        resolved = children_of(&resolved.kind).get(*child_index)?;
    }
    Some(resolved)
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
    scroll_regions: Option<&[ScrollRegion]>,
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
    apply_widget_semantics(&mut node, resolved, computed, scroll_regions);
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
    if resolved.splitter_handle.is_some() {
        return Role::Splitter;
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
        ResolvedWidgetKind::Image { .. } | ResolvedWidgetKind::Icon { .. } => Role::Image,
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
    scroll_regions: Option<&[ScrollRegion]>,
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
        node.set_selected(
            list_item
                .selection
                .selected_key_membership
                .resolve_ref(|membership| membership.contains(&list_item.key)),
        );
        node.set_position_in_set(list_item.item_index + 1);
        node.set_size_of_set(list_item.selection.sibling_keys.len());
        node.add_action(Action::Click);
        if list_item.disabled.resolve() {
            node.set_disabled();
        }
    }

    if let Some(tree_node) = resolved.tree_node.as_ref() {
        node.set_selected(tree_node.selected);
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
        node.set_selected(cell.selected);
        node.add_action(Action::Click);
        if cell.disabled.resolve() {
            node.set_disabled();
        }
    }

    if let Some(splitter) = resolved.splitter_handle.as_ref() {
        let sizes = splitter.current_sizes();
        let current = sizes
            .get(splitter.index)
            .copied()
            .unwrap_or_default()
            .clamp(0.0, 1.0);
        node.set_numeric_value(current as f64);
        node.set_min_numeric_value(
            splitter
                .constraints
                .get(splitter.index)
                .map(|(min, _)| *min)
                .unwrap_or(0.0) as f64,
        );
        node.set_max_numeric_value(
            splitter
                .constraints
                .get(splitter.index)
                .map(|(_, max)| *max)
                .unwrap_or(1.0) as f64,
        );
        node.set_numeric_value_step(splitter.step as f64);
        node.add_action(Action::Increment);
        node.add_action(Action::Decrement);
        node.add_action(Action::SetValue);
        node.add_action(Action::Click);
    }

    match &resolved.kind {
        ResolvedWidgetKind::Container { layout, .. } if layout.scroll_view.is_some() => {
            apply_scroll_region(
                node,
                scroll_regions.unwrap_or(computed.scroll_regions.as_slice()),
                resolved.id,
            );
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
                node.set_size_of_set(list_item.selection.sibling_keys.len());
                if list_item.selection_mode == crate::ui::widget::ListSelectionMode::Multiple {
                    node.set_multiselectable();
                }
            }
        }
        ResolvedWidgetKind::Text { text, .. } => {
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
            HitInteraction::SplitterHandle { .. } => {
                node.add_action(Action::Increment);
                node.add_action(Action::Decrement);
                node.add_action(Action::SetValue);
                node.add_action(Action::Click);
            }
            HitInteraction::TextInput { .. } => {
                node.add_action(Action::SetValue);
            }
            _ => {}
        }
    }
}

fn apply_scroll_region(node: &mut Node, regions: &[ScrollRegion], widget_id: WidgetId) {
    let Some(region) = regions.iter().find(|region| region.id == widget_id) else {
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
        | HitInteraction::SplitterHandle { id, .. }
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

#[cfg(test)]
mod registry_tests {
    use super::*;

    fn route(widget_id: u64) -> PortalAccessibilityNodeRoute {
        PortalAccessibilityNodeRoute {
            source_window_instance_id: 7,
            source_publication_generation: None,
            owner_path: SmallVec::from_slice(&[OverlayId::new(11)]),
            resolved_path: SmallVec::new(),
            widget_id: WidgetId::from_raw(widget_id),
        }
    }

    #[test]
    fn registry_classifies_local_nodes_by_membership_not_allocator_position() {
        let mut registry = AccessibilityNodeRegistry::default();
        registry.next_node_id = 10;
        registry.begin_update();
        let mut included = HashSet::from([NodeId(10), NodeId(9)]);
        let portal_id = registry
            .node_id_for(3, route(17), &included)
            .expect("Portal route should allocate below reserved local ids");
        assert_eq!(portal_id, NodeId(8));
        included.insert(portal_id);
        registry.finish_update(&included);

        assert!(registry.is_live_local_node_id(NodeId(10)));
        assert!(registry.is_live_local_node_id(NodeId(9)));
        assert!(!registry.is_live_local_node_id(portal_id));
        assert_eq!(registry.live_route(portal_id), Some(&route(17)));
    }

    #[test]
    fn registry_rekeys_a_live_portal_route_that_collides_with_a_new_local_node() {
        let mut registry = AccessibilityNodeRegistry::default();
        registry.next_node_id = 10;
        registry.begin_update();
        let first = registry
            .node_id_for(3, route(17), &HashSet::new())
            .expect("initial Portal route");
        assert_eq!(first, NodeId(10));
        registry.finish_update(&HashSet::from([first]));

        registry.begin_update();
        let mut included = HashSet::from([first]);
        let replacement = registry
            .node_id_for(3, route(17), &included)
            .expect("colliding route should get a new id");
        assert_eq!(replacement, NodeId(9));
        included.insert(replacement);
        registry.finish_update(&included);

        assert!(registry.is_live_local_node_id(first));
        assert!(registry.live_route(first).is_none());
        assert_eq!(registry.live_route(replacement), Some(&route(17)));
    }
}
