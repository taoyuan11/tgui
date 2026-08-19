//! Backend-neutral accessibility semantics and committed tree snapshots.
//!
//! The internal tree is available in headless builds. The optional
//! `accessibility` feature adds AccessKit serialization and target-specific
//! UIA, NSAccessibility, or AT-SPI adapter crates at the platform boundary.

use crate::core::{ElementId, Error, Rect, Result, SemanticRevision, WindowId};
use crate::event::{AccessibilityAction, AccessibilityActionEvent, UiEvent};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::Arc;

/// Stable accessibility identity within one window tree.
///
/// Element slot and generation are packed without hashing, so keyed reorder
/// preserves identity while slot reuse always produces a different NodeId.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NodeId(u64);

impl NodeId {
    pub const fn from_element(element: ElementId) -> Self {
        Self(((element.generation() as u64) << 32) | element.slot() as u64)
    }

    pub const fn get(self) -> u64 {
        self.0
    }

    pub const fn element(self) -> Option<ElementId> {
        let generation = (self.0 >> 32) as u32;
        if generation == 0 {
            None
        } else {
            Some(ElementId::from_parts(self.0 as u32, generation))
        }
    }
}

impl fmt::Debug for NodeId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "AccessibilityNodeId({:#018x})", self.0)
    }
}

/// Platform-independent accessible role set used by widgets and native hosts.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum Role {
    #[default]
    Generic,
    Window,
    Group,
    Text,
    Button,
    Image,
    Link,
    CheckBox,
    RadioButton,
    TextInput,
    List,
    ListItem,
    ListBox,
    ListBoxOption,
    ScrollView,
    Slider,
    ProgressIndicator,
    WebView,
    NativeHost,
}

/// Action vocabulary advertised by a semantics node.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum ActionKind {
    Activate,
    Focus,
    Increment,
    Decrement,
    SetValue,
    ScrollIntoView,
    Custom(Arc<str>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CheckedState {
    False,
    True,
    Mixed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CollectionInfo {
    pub item_count: usize,
    pub selected_count: usize,
}

/// Position is one-based.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CollectionItemInfo {
    pub position_in_set: usize,
    pub set_size: usize,
    pub current: bool,
}

/// Accessibility boundary exposed by a foreign native host.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NativeSemanticsBridge {
    Opaque,
    /// The node grafts a platform-owned AccessKit subtree with this UUID.
    Subtree([u8; 16]),
}

/// Immutable semantics declaration copied from a Widget into its Element.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Semantics {
    role: Role,
    name: Option<Arc<str>>,
    value: Option<Arc<str>>,
    description: Option<Arc<str>>,
    enabled: bool,
    hidden: bool,
    focusable: bool,
    focused: bool,
    selected: Option<bool>,
    checked: Option<CheckedState>,
    expanded: Option<bool>,
    actions: BTreeSet<ActionKind>,
    collection: Option<CollectionInfo>,
    collection_item: Option<CollectionItemInfo>,
    native_bridge: Option<NativeSemanticsBridge>,
}

impl Semantics {
    pub fn new(role: Role) -> Self {
        Self {
            role,
            ..Self::default()
        }
    }

    pub fn text(value: impl Into<Arc<str>>) -> Self {
        Self::new(Role::Text).with_value(value)
    }

    pub const fn role(&self) -> Role {
        self.role
    }
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }
    pub fn value(&self) -> Option<&str> {
        self.value.as_deref()
    }
    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }
    pub const fn is_enabled(&self) -> bool {
        self.enabled
    }
    pub const fn is_hidden(&self) -> bool {
        self.hidden
    }
    pub const fn is_focusable(&self) -> bool {
        self.focusable
    }
    pub const fn is_focused(&self) -> bool {
        self.focused
    }
    pub const fn selected(&self) -> Option<bool> {
        self.selected
    }
    pub const fn checked(&self) -> Option<CheckedState> {
        self.checked
    }
    pub const fn expanded(&self) -> Option<bool> {
        self.expanded
    }
    pub const fn collection(&self) -> Option<&CollectionInfo> {
        self.collection.as_ref()
    }
    pub const fn collection_item(&self) -> Option<&CollectionItemInfo> {
        self.collection_item.as_ref()
    }
    pub const fn native_bridge(&self) -> Option<&NativeSemanticsBridge> {
        self.native_bridge.as_ref()
    }
    pub fn actions(&self) -> impl ExactSizeIterator<Item = &ActionKind> {
        self.actions.iter()
    }
    pub fn supports(&self, action: &ActionKind) -> bool {
        self.actions.contains(action)
    }

    pub fn with_name(mut self, name: impl Into<Arc<str>>) -> Self {
        self.name = Some(name.into());
        self
    }
    pub fn with_value(mut self, value: impl Into<Arc<str>>) -> Self {
        self.value = Some(value.into());
        self
    }
    pub fn with_description(mut self, description: impl Into<Arc<str>>) -> Self {
        self.description = Some(description.into());
        self
    }
    pub const fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
    pub const fn with_hidden(mut self, hidden: bool) -> Self {
        self.hidden = hidden;
        self
    }
    pub const fn with_focusable(mut self, focusable: bool) -> Self {
        self.focusable = focusable;
        self
    }
    pub const fn with_focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }
    pub const fn with_selected(mut self, selected: bool) -> Self {
        self.selected = Some(selected);
        self
    }
    pub const fn with_checked(mut self, checked: CheckedState) -> Self {
        self.checked = Some(checked);
        self
    }
    pub const fn with_expanded(mut self, expanded: bool) -> Self {
        self.expanded = Some(expanded);
        self
    }
    pub fn with_action(mut self, action: ActionKind) -> Self {
        self.actions.insert(action);
        self
    }
    pub fn with_actions(mut self, actions: impl IntoIterator<Item = ActionKind>) -> Self {
        self.actions.extend(actions);
        self
    }
    pub fn with_collection(mut self, collection: CollectionInfo) -> Self {
        self.collection = Some(collection);
        self
    }
    pub fn with_collection_item(mut self, item: CollectionItemInfo) -> Self {
        self.collection_item = Some(item);
        self
    }
    pub fn with_native_bridge(mut self, bridge: NativeSemanticsBridge) -> Self {
        self.native_bridge = Some(bridge);
        self
    }
}

impl Default for Semantics {
    fn default() -> Self {
        Self {
            role: Role::Generic,
            name: None,
            value: None,
            description: None,
            enabled: true,
            hidden: false,
            focusable: false,
            focused: false,
            selected: None,
            checked: None,
            expanded: None,
            actions: BTreeSet::new(),
            collection: None,
            collection_item: None,
            native_bridge: None,
        }
    }
}

/// Element semantics paired with committed logical geometry.
#[derive(Clone, Debug, PartialEq)]
pub struct SemanticNodeInput {
    pub element: ElementId,
    pub parent: Option<ElementId>,
    pub bounds: Option<Rect>,
    pub semantics: Semantics,
}

impl SemanticNodeInput {
    pub fn new(element: ElementId, semantics: Semantics) -> Self {
        Self {
            element,
            parent: None,
            bounds: None,
            semantics,
        }
    }
    pub const fn with_parent(mut self, parent: Option<ElementId>) -> Self {
        self.parent = parent;
        self
    }
    pub const fn with_bounds(mut self, bounds: Option<Rect>) -> Self {
        self.bounds = bounds;
        self
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct AccessibilityNode {
    id: NodeId,
    element: ElementId,
    parent: Option<NodeId>,
    children: Vec<NodeId>,
    bounds: Option<Rect>,
    focused: bool,
    semantics: Semantics,
}

impl AccessibilityNode {
    pub const fn id(&self) -> NodeId {
        self.id
    }
    pub const fn element(&self) -> ElementId {
        self.element
    }
    pub const fn parent(&self) -> Option<NodeId> {
        self.parent
    }
    pub fn children(&self) -> &[NodeId] {
        &self.children
    }
    pub const fn bounds(&self) -> Option<Rect> {
        self.bounds
    }
    pub const fn is_focused(&self) -> bool {
        self.focused
    }
    pub const fn semantics(&self) -> &Semantics {
        &self.semantics
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SemanticUpdateReasons(u8);

impl SemanticUpdateReasons {
    pub const NONE: Self = Self(0);
    pub const SEMANTICS: Self = Self(1 << 0);
    pub const FOCUS: Self = Self(1 << 1);
    pub const LAYOUT_BOUNDS: Self = Self(1 << 2);
    pub const ACCESSIBLE_SCROLL: Self = Self(1 << 3);
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }
}

/// Immutable accessibility output committed as part of the CPU snapshot.
#[derive(Clone, Debug, PartialEq)]
pub struct SemanticSnapshot {
    revision: SemanticRevision,
    node_count: usize,
    fingerprint: u64,
    root: Option<NodeId>,
    focus: Option<NodeId>,
    nodes: Vec<AccessibilityNode>,
}

impl SemanticSnapshot {
    /// Compatibility constructor for externally supplied snapshot headers.
    pub const fn new(revision: SemanticRevision, node_count: usize, fingerprint: u64) -> Self {
        Self {
            revision,
            node_count,
            fingerprint,
            root: None,
            focus: None,
            nodes: Vec::new(),
        }
    }
    pub const fn empty(revision: SemanticRevision) -> Self {
        Self::new(revision, 0, 0)
    }
    pub const fn revision(&self) -> SemanticRevision {
        self.revision
    }
    pub const fn node_count(&self) -> usize {
        self.node_count
    }
    pub const fn fingerprint(&self) -> u64 {
        self.fingerprint
    }
    pub const fn root(&self) -> Option<NodeId> {
        self.root
    }
    pub const fn focus(&self) -> Option<NodeId> {
        self.focus
    }
    pub fn nodes(&self) -> &[AccessibilityNode] {
        &self.nodes
    }
    pub fn node(&self, id: NodeId) -> Option<&AccessibilityNode> {
        self.nodes.iter().find(|node| node.id == id)
    }
    pub fn node_for_element(&self, element: ElementId) -> Option<&AccessibilityNode> {
        self.node(NodeId::from_element(element))
    }
    /// Resolves a committed NodeId to a generation-checked Element event.
    /// Unsupported and stale actions are ignored.
    pub fn action_event(
        &self,
        window: WindowId,
        id: NodeId,
        action: AccessibilityAction,
    ) -> Option<UiEvent> {
        let node = self.node(id)?;
        let supported = match &action {
            AccessibilityAction::Activate => node.semantics.supports(&ActionKind::Activate),
            AccessibilityAction::Focus => {
                node.semantics.focusable || node.semantics.supports(&ActionKind::Focus)
            }
            AccessibilityAction::Increment => node.semantics.supports(&ActionKind::Increment),
            AccessibilityAction::Decrement => node.semantics.supports(&ActionKind::Decrement),
            AccessibilityAction::SetValue(_) => node.semantics.supports(&ActionKind::SetValue),
            AccessibilityAction::ScrollIntoView => {
                node.semantics.supports(&ActionKind::ScrollIntoView)
            }
            AccessibilityAction::Custom(description) => node
                .semantics
                .supports(&ActionKind::Custom(description.clone())),
        };
        supported.then(|| {
            UiEvent::AccessibilityAction(AccessibilityActionEvent::for_window(
                window,
                node.element,
                action,
            ))
        })
    }
    pub(crate) fn observable_eq(&self, other: &Self) -> bool {
        self.node_count == other.node_count
            && self.fingerprint == other.fingerprint
            && self.root == other.root
            && self.focus == other.focus
            && self.nodes == other.nodes
    }
}

impl Default for SemanticSnapshot {
    fn default() -> Self {
        Self::empty(SemanticRevision::ZERO)
    }
}

#[derive(Clone, Debug, Default)]
pub struct AccessibilityTree {
    committed: SemanticSnapshot,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SemanticUpdate {
    pub snapshot: SemanticSnapshot,
    pub performed: bool,
    pub observable_changed: bool,
    pub reasons: SemanticUpdateReasons,
}

impl AccessibilityTree {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn from_snapshot(snapshot: SemanticSnapshot) -> Self {
        Self {
            committed: snapshot,
        }
    }
    pub const fn committed(&self) -> &SemanticSnapshot {
        &self.committed
    }

    pub fn update(
        &mut self,
        inputs: impl IntoIterator<Item = SemanticNodeInput>,
        focused: Option<ElementId>,
        reasons: SemanticUpdateReasons,
    ) -> Result<SemanticUpdate> {
        if reasons.is_empty() {
            return Ok(SemanticUpdate {
                snapshot: self.committed.clone(),
                performed: false,
                observable_changed: false,
                reasons,
            });
        }
        let candidate = build_snapshot(self.committed.revision, inputs, focused)?;
        let observable_changed = !candidate.observable_eq(&self.committed);
        let snapshot = if observable_changed {
            let revision = self
                .committed
                .revision
                .checked_next()
                .map_err(|error| Error::compile("semantic_revision", error.to_string()))?;
            SemanticSnapshot {
                revision,
                ..candidate
            }
        } else {
            self.committed.clone()
        };
        self.committed = snapshot.clone();
        Ok(SemanticUpdate {
            snapshot,
            performed: true,
            observable_changed,
            reasons,
        })
    }
}

pub fn compare_accessibility_snapshots(
    incremental: &SemanticSnapshot,
    rebuilt: &SemanticSnapshot,
) -> Result<()> {
    if incremental.revision != rebuilt.revision {
        return Err(Error::compile(
            "accessibility_equivalence",
            "semantic revisions differ",
        ));
    }
    if !incremental.observable_eq(rebuilt) {
        return Err(Error::compile(
            "accessibility_equivalence",
            "accessibility trees differ",
        ));
    }
    Ok(())
}

fn build_snapshot(
    revision: SemanticRevision,
    inputs: impl IntoIterator<Item = SemanticNodeInput>,
    focused: Option<ElementId>,
) -> Result<SemanticSnapshot> {
    let inputs = inputs.into_iter().collect::<Vec<_>>();
    if inputs.is_empty() {
        return Ok(SemanticSnapshot::empty(revision));
    }
    let ids = inputs
        .iter()
        .map(|input| (input.element, NodeId::from_element(input.element)))
        .collect::<BTreeMap<_, _>>();
    if ids.len() != inputs.len() {
        return Err(Error::compile(
            "accessibility_tree",
            "duplicate ElementId in semantic input",
        ));
    }
    let mut root = None;
    let mut children = BTreeMap::<ElementId, Vec<NodeId>>::new();
    for input in &inputs {
        if let Some(bounds) = input.bounds {
            bounds.validate().map_err(Error::from)?;
        }
        match input.parent {
            Some(parent) => {
                if parent == input.element || !ids.contains_key(&parent) {
                    return Err(Error::compile(
                        "accessibility_tree",
                        "semantic parent is stale, missing, or self-referential",
                    ));
                }
                children
                    .entry(parent)
                    .or_default()
                    .push(NodeId::from_element(input.element));
            }
            None if root.replace(NodeId::from_element(input.element)).is_some() => {
                return Err(Error::compile(
                    "accessibility_tree",
                    "semantic input has more than one root",
                ));
            }
            None => {}
        }
    }
    let root = root.ok_or_else(|| Error::compile("accessibility_tree", "semantic root missing"))?;
    let explicit_focus = focused.map(NodeId::from_element);
    if explicit_focus.is_some_and(|focus| !ids.values().any(|candidate| *candidate == focus)) {
        return Err(Error::compile(
            "accessibility_tree",
            "focused ElementId is not in semantic input",
        ));
    }
    let declared_focus = inputs
        .iter()
        .find(|input| input.semantics.focused)
        .map(|input| NodeId::from_element(input.element));
    let focus = explicit_focus.or(declared_focus).or(Some(root));
    let nodes = inputs
        .into_iter()
        .map(|input| {
            let id = NodeId::from_element(input.element);
            AccessibilityNode {
                id,
                element: input.element,
                parent: input.parent.map(NodeId::from_element),
                children: children.remove(&input.element).unwrap_or_default(),
                bounds: input.bounds,
                focused: focus == Some(id),
                semantics: input.semantics,
            }
        })
        .collect::<Vec<_>>();
    validate_connected(root, &nodes)?;
    let fingerprint = semantic_fingerprint(root, focus, &nodes);
    Ok(SemanticSnapshot {
        revision,
        node_count: nodes.len(),
        fingerprint,
        root: Some(root),
        focus,
        nodes,
    })
}

fn validate_connected(root: NodeId, nodes: &[AccessibilityNode]) -> Result<()> {
    let by_id = nodes
        .iter()
        .map(|node| (node.id, node))
        .collect::<BTreeMap<_, _>>();
    let mut visited = BTreeSet::new();
    let mut stack = vec![root];
    while let Some(id) = stack.pop() {
        if !visited.insert(id) {
            return Err(Error::compile(
                "accessibility_tree",
                "semantic tree contains a cycle or duplicate child",
            ));
        }
        let node = by_id.get(&id).ok_or_else(|| {
            Error::compile("accessibility_tree", "semantic child NodeId is missing")
        })?;
        stack.extend(node.children.iter().rev().copied());
    }
    if visited.len() != nodes.len() {
        return Err(Error::compile(
            "accessibility_tree",
            "semantic tree contains a disconnected node",
        ));
    }
    Ok(())
}

fn semantic_fingerprint(root: NodeId, focus: Option<NodeId>, nodes: &[AccessibilityNode]) -> u64 {
    let mut hash = Fingerprint::new();
    hash.u64(root.get());
    hash.u64(focus.map_or(0, NodeId::get));
    for node in nodes {
        hash.u64(node.id.get());
        hash.u64(node.parent.map_or(0, NodeId::get));
        for child in &node.children {
            hash.u64(child.get());
        }
        if let Some(bounds) = node.bounds {
            for value in [
                bounds.origin.x,
                bounds.origin.y,
                bounds.size.width,
                bounds.size.height,
            ] {
                hash.u64(u64::from(value.to_bits()));
            }
        }
        hash.u64(node.semantics.role as u64);
        hash.text(node.semantics.name.as_deref());
        hash.text(node.semantics.value.as_deref());
        hash.text(node.semantics.description.as_deref());
        hash.u64(u64::from(node.semantics.enabled));
        hash.u64(u64::from(node.semantics.hidden));
        hash.u64(u64::from(node.semantics.focusable));
        hash.u64(u64::from(node.focused));
        hash.u64(node.semantics.selected.map_or(2, u64::from));
        hash.u64(node.semantics.checked.map_or(3, |value| value as u64));
        hash.u64(node.semantics.expanded.map_or(2, u64::from));
        for action in &node.semantics.actions {
            hash.text(Some(&format!("{action:?}")));
        }
        if let Some(collection) = &node.semantics.collection {
            hash.u64(collection.item_count as u64);
            hash.u64(collection.selected_count as u64);
        }
        if let Some(item) = &node.semantics.collection_item {
            hash.u64(item.position_in_set as u64);
            hash.u64(item.set_size as u64);
            hash.u64(u64::from(item.current));
        }
        if let Some(bridge) = &node.semantics.native_bridge {
            match bridge {
                NativeSemanticsBridge::Opaque => hash.u64(1),
                NativeSemanticsBridge::Subtree(bytes) => {
                    hash.u64(2);
                    hash.bytes(bytes);
                }
            }
        }
    }
    hash.finish()
}

struct Fingerprint(u64);
impl Fingerprint {
    const fn new() -> Self {
        Self(0xcbf2_9ce4_8422_2325)
    }
    fn bytes(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.0 ^= u64::from(*byte);
            self.0 = self.0.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    fn u64(&mut self, value: u64) {
        self.bytes(&value.to_le_bytes());
    }
    fn text(&mut self, value: Option<&str>) {
        if let Some(value) = value {
            self.u64(value.len() as u64);
            self.bytes(value.as_bytes());
        } else {
            self.u64(u64::MAX);
        }
    }
    const fn finish(self) -> u64 {
        self.0
    }
}

/// Converts a committed AccessKit request back into the normal event system.
#[cfg(feature = "accessibility")]
pub fn accesskit_action_event(
    window: WindowId,
    snapshot: &SemanticSnapshot,
    request: &accesskit::ActionRequest,
) -> Result<Option<UiEvent>> {
    if request.target_tree != accesskit::TreeId::ROOT {
        return Ok(None);
    }
    let id = NodeId(request.target_node.0);
    let Some(node) = snapshot.node(id) else {
        return Ok(None);
    };
    let action = translate_accesskit_action(node, request)?;
    Ok(action.map(|action| {
        UiEvent::AccessibilityAction(AccessibilityActionEvent::for_window(
            window,
            node.element,
            action,
        ))
    }))
}

#[cfg(feature = "accessibility")]
fn translate_accesskit_action(
    node: &AccessibilityNode,
    request: &accesskit::ActionRequest,
) -> Result<Option<AccessibilityAction>> {
    use accesskit::{Action, ActionData};
    let semantic = &node.semantics;
    let translated = match request.action {
        Action::Click if semantic.supports(&ActionKind::Activate) => AccessibilityAction::Activate,
        Action::Focus if semantic.focusable || semantic.supports(&ActionKind::Focus) => {
            AccessibilityAction::Focus
        }
        Action::Increment if semantic.supports(&ActionKind::Increment) => {
            AccessibilityAction::Increment
        }
        Action::Decrement if semantic.supports(&ActionKind::Decrement) => {
            AccessibilityAction::Decrement
        }
        Action::ScrollIntoView if semantic.supports(&ActionKind::ScrollIntoView) => {
            AccessibilityAction::ScrollIntoView
        }
        Action::SetValue if semantic.supports(&ActionKind::SetValue) => {
            let value = match request.data.as_ref() {
                Some(ActionData::Value(value)) => Arc::<str>::from(value.as_ref()),
                Some(ActionData::NumericValue(value)) => Arc::<str>::from(value.to_string()),
                _ => {
                    return Err(Error::invalid_input(
                        Some("accessibility_action".to_owned()),
                        "AccessKit SetValue action is missing value data",
                    ));
                }
            };
            AccessibilityAction::SetValue(value)
        }
        Action::CustomAction => {
            let Some(ActionData::CustomAction(id)) = request.data else {
                return Ok(None);
            };
            let Some(ActionKind::Custom(description)) = semantic
                .actions
                .iter()
                .filter(|action| matches!(action, ActionKind::Custom(_)))
                .nth(id.max(0) as usize)
            else {
                return Ok(None);
            };
            AccessibilityAction::Custom(description.clone())
        }
        _ => return Ok(None),
    };
    Ok(Some(translated))
}

#[cfg(feature = "accessibility")]
impl SemanticSnapshot {
    pub fn to_accesskit_update(&self) -> Option<accesskit::TreeUpdate> {
        use accesskit::{Tree, TreeId, TreeUpdate};
        let root = self.root?;
        let focus = self.focus.unwrap_or(root);
        let mut tree = Tree::new(accesskit::NodeId(root.get()));
        tree.toolkit_name = Some("tgui".to_owned());
        tree.toolkit_version = Some(env!("CARGO_PKG_VERSION").to_owned());
        Some(TreeUpdate {
            nodes: self
                .nodes
                .iter()
                .map(|node| (accesskit::NodeId(node.id.get()), accesskit_node(node)))
                .collect(),
            tree: Some(tree),
            tree_id: TreeId::ROOT,
            focus: accesskit::NodeId(focus.get()),
        })
    }
}

#[cfg(feature = "accessibility")]
fn accesskit_node(source: &AccessibilityNode) -> accesskit::Node {
    use accesskit::{Action, AriaCurrent, CustomAction, Node, Toggled};
    let semantics = &source.semantics;
    let mut node = Node::new(accesskit_role(semantics.role));
    node.set_children(
        source
            .children
            .iter()
            .map(|child| accesskit::NodeId(child.get()))
            .collect::<Vec<_>>(),
    );
    if let Some(bounds) = source.bounds {
        node.set_bounds(accesskit::Rect {
            x0: f64::from(bounds.origin.x),
            y0: f64::from(bounds.origin.y),
            x1: f64::from(bounds.origin.x + bounds.size.width),
            y1: f64::from(bounds.origin.y + bounds.size.height),
        });
    }
    if let Some(name) = semantics.name.as_deref() {
        node.set_label(name);
    }
    if let Some(value) = semantics.value.as_deref() {
        node.set_value(value);
    }
    if let Some(description) = semantics.description.as_deref() {
        node.set_description(description);
    }
    if semantics.hidden {
        node.set_hidden();
    }
    if !semantics.enabled {
        node.set_disabled();
    }
    if let Some(selected) = semantics.selected {
        node.set_selected(selected);
    }
    if let Some(expanded) = semantics.expanded {
        node.set_expanded(expanded);
    }
    if let Some(checked) = semantics.checked {
        node.set_toggled(match checked {
            CheckedState::False => Toggled::False,
            CheckedState::True => Toggled::True,
            CheckedState::Mixed => Toggled::Mixed,
        });
    }
    if let Some(collection) = &semantics.collection {
        node.set_size_of_set(collection.item_count);
    }
    if let Some(item) = &semantics.collection_item {
        node.set_position_in_set(item.position_in_set);
        node.set_size_of_set(item.set_size);
        if item.current {
            node.set_aria_current(AriaCurrent::True);
        }
    }
    let mut custom_id = 0_i32;
    for action in &semantics.actions {
        match action {
            ActionKind::Activate => node.add_action(Action::Click),
            ActionKind::Focus => node.add_action(Action::Focus),
            ActionKind::Increment => node.add_action(Action::Increment),
            ActionKind::Decrement => node.add_action(Action::Decrement),
            ActionKind::SetValue => node.add_action(Action::SetValue),
            ActionKind::ScrollIntoView => node.add_action(Action::ScrollIntoView),
            ActionKind::Custom(description) => {
                node.add_action(Action::CustomAction);
                node.push_custom_action(CustomAction {
                    id: custom_id,
                    description: description.to_string().into_boxed_str(),
                });
                custom_id += 1;
            }
        }
    }
    if semantics.focusable && !node.supports_action(Action::Focus) {
        node.add_action(Action::Focus);
    }
    if let Some(NativeSemanticsBridge::Subtree(uuid)) = &semantics.native_bridge {
        node.set_tree_id(accesskit::TreeId(accesskit::Uuid::from_bytes(*uuid)));
    }
    node
}

#[cfg(feature = "accessibility")]
fn accesskit_role(role: Role) -> accesskit::Role {
    match role {
        Role::Generic => accesskit::Role::GenericContainer,
        Role::Window => accesskit::Role::Window,
        Role::Group => accesskit::Role::Group,
        Role::Text => accesskit::Role::Label,
        Role::Button => accesskit::Role::Button,
        Role::Image => accesskit::Role::Image,
        Role::Link => accesskit::Role::Link,
        Role::CheckBox => accesskit::Role::CheckBox,
        Role::RadioButton => accesskit::Role::RadioButton,
        Role::TextInput => accesskit::Role::TextInput,
        Role::List => accesskit::Role::List,
        Role::ListItem => accesskit::Role::ListItem,
        Role::ListBox => accesskit::Role::ListBox,
        Role::ListBoxOption => accesskit::Role::ListBoxOption,
        Role::ScrollView => accesskit::Role::ScrollView,
        Role::Slider => accesskit::Role::Slider,
        Role::ProgressIndicator => accesskit::Role::ProgressIndicator,
        Role::WebView => accesskit::Role::WebView,
        Role::NativeHost => accesskit::Role::EmbeddedObject,
    }
}

/// Target-specific adapter types are kept at the outer platform boundary.
#[cfg(feature = "accessibility")]
pub mod platform_adapter {
    #[cfg(target_os = "macos")]
    pub use accesskit_macos::Adapter;
    #[cfg(any(
        target_os = "linux",
        target_os = "dragonfly",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd"
    ))]
    pub use accesskit_unix::Adapter;
    #[cfg(target_os = "windows")]
    pub use accesskit_windows::Adapter;
}

pub const ADAPTER_ENABLED: bool = cfg!(feature = "accessibility");

#[cfg(test)]
mod tests {
    use super::*;
    fn element(slot: u32, generation: u32) -> ElementId {
        ElementId::from_parts(slot, generation)
    }

    #[test]
    fn node_id_preserves_generation_and_slot() {
        let first = NodeId::from_element(element(7, 1));
        let reused = NodeId::from_element(element(7, 2));
        assert_ne!(first, reused);
        assert_eq!(first.element(), Some(element(7, 1)));
        assert_eq!(reused.element(), Some(element(7, 2)));
    }

    #[test]
    fn update_is_gated_and_revision_only_tracks_observable_change() {
        let root = element(0, 1);
        let input = || {
            vec![
                SemanticNodeInput::new(
                    root,
                    Semantics::new(Role::Button)
                        .with_name("Save")
                        .with_action(ActionKind::Activate),
                )
                .with_bounds(Some(Rect::from_xywh(1.0, 2.0, 30.0, 12.0))),
            ]
        };
        let mut tree = AccessibilityTree::new();
        let skipped = tree
            .update(input(), None, SemanticUpdateReasons::NONE)
            .unwrap();
        assert!(!skipped.performed);
        let first = tree
            .update(input(), Some(root), SemanticUpdateReasons::SEMANTICS)
            .unwrap();
        assert!(first.observable_changed);
        assert_eq!(first.snapshot.revision(), SemanticRevision::new(1));
        let same = tree
            .update(input(), Some(root), SemanticUpdateReasons::LAYOUT_BOUNDS)
            .unwrap();
        assert!(!same.observable_changed);
        assert_eq!(same.snapshot.revision(), SemanticRevision::new(1));
    }

    #[test]
    fn reorder_keeps_node_ids_and_changes_only_topology() {
        let root = element(0, 1);
        let a = element(1, 1);
        let b = element(2, 1);
        let input = |order: [ElementId; 2]| {
            let mut result = vec![SemanticNodeInput::new(root, Semantics::new(Role::List))];
            result.extend(order.map(|id| {
                SemanticNodeInput::new(id, Semantics::new(Role::ListItem)).with_parent(Some(root))
            }));
            result
        };
        let mut tree = AccessibilityTree::new();
        let first = tree
            .update(input([a, b]), None, SemanticUpdateReasons::SEMANTICS)
            .unwrap();
        let a_id = first.snapshot.node_for_element(a).unwrap().id();
        let second = tree
            .update(input([b, a]), None, SemanticUpdateReasons::SEMANTICS)
            .unwrap();
        assert_eq!(second.snapshot.node_for_element(a).unwrap().id(), a_id);
        assert_eq!(
            second
                .snapshot
                .node(second.snapshot.root().unwrap())
                .unwrap()
                .children(),
            [NodeId::from_element(b), NodeId::from_element(a)]
        );
    }

    #[test]
    fn invalid_topology_keeps_committed_tree() {
        let root = element(0, 1);
        let child = element(1, 1);
        let mut tree = AccessibilityTree::new();
        tree.update(
            [SemanticNodeInput::new(root, Semantics::default())],
            None,
            SemanticUpdateReasons::SEMANTICS,
        )
        .unwrap();
        let committed = tree.committed().clone();
        assert!(
            tree.update(
                [SemanticNodeInput::new(child, Semantics::default()).with_parent(Some(root))],
                None,
                SemanticUpdateReasons::SEMANTICS
            )
            .is_err()
        );
        assert_eq!(tree.committed(), &committed);
    }
}
