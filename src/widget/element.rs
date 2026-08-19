use super::{
    BuildContext, LifecycleCallback, LifecycleEvent, PropertyImpact, PropertyValue, View,
    WidgetNode, WidgetType,
};
use crate::core::{
    ArenaStats, DenseArena, ElementId, Error, PropertyId, Result, TreeLinks, WidgetKey,
};
use crate::event::{EventHandler, EventTargetTree};
use crate::layout::{LayoutBoundaries, LayoutNodeInput, LayoutStyle, MeasureSpec};
use crate::state::{
    DependencyOwner, DependencyPhase, DependencySet, StateWriteGuard, UiCommand, UiThread,
    capture_dependencies,
};
use std::any::Any;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::mem::size_of;

struct StateSlot {
    id: u64,
    value: Box<dyn Any>,
}

pub(crate) struct ElementNode {
    pub(crate) links: TreeLinks<ElementId>,
    pub(crate) widget_type: WidgetType,
    pub(crate) key: Option<WidgetKey>,
    pub(crate) properties: Vec<(PropertyId, PropertyValue)>,
    pub(crate) property_impacts: Vec<(PropertyId, PropertyImpact)>,
    lifecycle: Option<LifecycleCallback>,
    event_handler: Option<EventHandler>,
    focusable: bool,
    enabled: bool,
    layout_style: LayoutStyle,
    measure: Option<MeasureSpec>,
    scroll_offset: crate::core::Point,
    hit_test: bool,
    boundaries: LayoutBoundaries,
    state_slots: Vec<StateSlot>,
    dependencies: BTreeMap<DependencyPhase, DependencySet>,
    subscriptions: Vec<Box<dyn Any>>,
    cleanup: Vec<Box<dyn FnOnce()>>,
    allocation_count: u64,
}

impl fmt::Debug for ElementNode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ElementNode")
            .field("links", &self.links)
            .field("widget_type", &self.widget_type)
            .field("key", &self.key)
            .field("properties", &self.properties)
            .field("lifecycle", &self.lifecycle)
            .field("event_handler", &self.event_handler)
            .field("focusable", &self.focusable)
            .field("enabled", &self.enabled)
            .field("state_slot_count", &self.state_slots.len())
            .field("dependency_phase_count", &self.dependencies.len())
            .field("subscription_count", &self.subscription_count())
            .field("cleanup_count", &self.cleanup.len())
            .field("allocation_count", &self.allocation_count)
            .finish()
    }
}

impl ElementNode {
    fn from_widget(widget: &WidgetNode, parent: Option<ElementId>) -> Self {
        let mut links = TreeLinks::new();
        links.set_parent(parent);
        let properties = widget.properties.clone();
        let property_impacts = widget.property_impacts.clone();
        let allocation_count =
            u64::from(!properties.is_empty()) + u64::from(!property_impacts.is_empty());
        Self {
            links,
            widget_type: widget.widget_type.clone(),
            key: widget.key.clone(),
            properties,
            property_impacts,
            lifecycle: widget.lifecycle.clone(),
            event_handler: widget.event_handler.clone(),
            focusable: widget.focusable,
            enabled: widget.enabled,
            layout_style: widget.layout_style.clone(),
            measure: widget.measure.clone(),
            scroll_offset: widget.scroll_offset,
            hit_test: widget.hit_test,
            boundaries: widget.boundaries,
            state_slots: Vec::new(),
            dependencies: BTreeMap::new(),
            subscriptions: Vec::new(),
            cleanup: Vec::new(),
            allocation_count,
        }
    }

    fn auxiliary_heap_bytes(&self) -> usize {
        self.properties
            .capacity()
            .saturating_mul(size_of::<(PropertyId, PropertyValue)>())
            .saturating_add(
                self.property_impacts
                    .capacity()
                    .saturating_mul(size_of::<(PropertyId, PropertyImpact)>()),
            )
            .saturating_add(
                self.state_slots
                    .capacity()
                    .saturating_mul(size_of::<StateSlot>()),
            )
            .saturating_add(
                self.dependencies
                    .len()
                    .saturating_mul(size_of::<(DependencyPhase, DependencySet)>()),
            )
            .saturating_add(
                self.subscriptions
                    .capacity()
                    .saturating_mul(size_of::<Box<dyn Any>>()),
            )
            .saturating_add(
                self.cleanup
                    .capacity()
                    .saturating_mul(size_of::<Box<dyn FnOnce()>>()),
            )
    }

    fn subscription_count(&self) -> usize {
        self.subscriptions.len()
            + self
                .dependencies
                .values()
                .map(DependencySet::len)
                .sum::<usize>()
    }
}

/// A safe fallback or ambiguity observed during reconciliation.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ReconcileDiagnostic {
    DuplicateKey { parent: ElementId, key: WidgetKey },
    DuplicateExistingKey { parent: ElementId, key: WidgetKey },
    RebuiltAmbiguousChildren { parent: ElementId },
}

/// Work performed by one mount or reconciliation pass.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ReconcileReport {
    pub mounted: usize,
    pub updated: usize,
    pub moved: usize,
    pub replaced: usize,
    pub unmounted: usize,
    pub diagnostics: Vec<ReconcileDiagnostic>,
    removed_ids: Vec<ElementId>,
    invalidations: BTreeMap<ElementId, ElementInvalidation>,
    lifecycle_events: Vec<(LifecycleCallback, LifecycleEvent)>,
}

/// Observable invalidation caused by reconciliation of one retained Element.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ElementInvalidation {
    element: ElementId,
    structure: bool,
    impact: PropertyImpact,
}

impl ElementInvalidation {
    pub const fn element(self) -> ElementId {
        self.element
    }

    pub const fn structure_changed(self) -> bool {
        self.structure
    }

    pub const fn property_impact(self) -> PropertyImpact {
        self.impact
    }
}

impl ReconcileReport {
    pub fn removed_ids(&self) -> &[ElementId] {
        &self.removed_ids
    }

    pub fn used_safe_fallback(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|item| matches!(item, ReconcileDiagnostic::RebuiltAmbiguousChildren { .. }))
    }

    pub fn invalidations(&self) -> impl Iterator<Item = ElementInvalidation> + '_ {
        self.invalidations.values().copied()
    }

    fn record_invalidation(&mut self, element: ElementId, structure: bool, impact: PropertyImpact) {
        self.invalidations
            .entry(element)
            .and_modify(|invalidation| {
                invalidation.structure |= structure;
                invalidation.impact = invalidation.impact.union(impact);
            })
            .or_insert(ElementInvalidation {
                element,
                structure,
                impact,
            });
    }

    fn absorb(&mut self, mut other: Self) {
        self.mounted += other.mounted;
        self.updated += other.updated;
        self.moved += other.moved;
        self.replaced += other.replaced;
        self.unmounted += other.unmounted;
        self.diagnostics.append(&mut other.diagnostics);
        self.removed_ids.append(&mut other.removed_ids);
        for invalidation in other.invalidations.into_values() {
            self.record_invalidation(
                invalidation.element,
                invalidation.structure,
                invalidation.impact,
            );
        }
        self.lifecycle_events.append(&mut other.lifecycle_events);
    }
}

/// Read-only per-element allocation and topology diagnostics.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ElementNodeDiagnostics {
    pub id: ElementId,
    pub parent: Option<ElementId>,
    pub widget_type: WidgetType,
    pub key: Option<WidgetKey>,
    pub state_slots: usize,
    pub subscriptions: usize,
    pub cleanup_callbacks: usize,
    /// Known auxiliary allocation events owned by this element. Shared `Arc`
    /// backing stores are excluded; arena storage is reported separately.
    pub allocation_count: u64,
    /// Known vector/table storage, excluding type-erased payload and shared
    /// string/callback backing stores whose ownership is not local.
    pub estimated_heap_bytes: usize,
}

/// Aggregate retained-tree storage diagnostics.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ElementTreeStats {
    pub arena: ArenaStats,
    pub state_slots: usize,
    pub subscriptions: usize,
    pub cleanup_callbacks: usize,
    pub allocation_count: u64,
    pub estimated_node_bytes: usize,
    pub estimated_heap_bytes: usize,
}

/// UI-owned persistent tree underlying immutable widget declarations.
pub(crate) struct ElementTree {
    owner: UiThread,
    dependency_owner: DependencyOwner,
    arena: DenseArena<ElementNode, ElementId>,
    root: Option<ElementId>,
}

struct ViewRoot;

impl ElementTree {
    pub(crate) fn new() -> Self {
        Self {
            owner: UiThread::current(),
            dependency_owner: DependencyOwner::new(),
            arena: DenseArena::new(),
            root: None,
        }
    }

    pub(crate) fn with_capacity(capacity: usize) -> Self {
        Self {
            owner: UiThread::current(),
            dependency_owner: DependencyOwner::new(),
            arena: DenseArena::with_capacity(capacity),
            root: None,
        }
    }

    pub(crate) fn root(&self) -> Option<ElementId> {
        self.root
    }

    pub(crate) fn dependency_owner(&self) -> DependencyOwner {
        self.dependency_owner
    }

    pub(crate) fn has_view_root(&self) -> bool {
        self.root
            .and_then(|root| self.widget_type(root))
            .is_some_and(|widget_type| *widget_type == WidgetType::of::<ViewRoot>())
    }

    pub(crate) fn contains(&self, id: ElementId) -> bool {
        self.arena.contains(id)
    }

    pub(crate) fn len(&self) -> usize {
        self.arena.len()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.arena.is_empty()
    }

    pub(crate) fn parent(&self, id: ElementId) -> Option<ElementId> {
        self.arena.get(id).and_then(|node| node.links.parent())
    }

    pub(crate) fn children(&self, parent: ElementId) -> Vec<ElementId> {
        let Some(node) = self.arena.get(parent) else {
            return Vec::new();
        };
        let mut result = Vec::new();
        let mut current = node.links.first_child();
        while let Some(id) = current {
            let Some(child) = self.arena.get(id) else {
                break;
            };
            result.push(id);
            current = child.links.next_sibling();
        }
        result
    }

    pub(crate) fn ids(&self) -> impl ExactSizeIterator<Item = ElementId> + '_ {
        self.arena.ids()
    }

    pub(crate) fn widget_type(&self, id: ElementId) -> Option<&WidgetType> {
        self.arena.get(id).map(|node| &node.widget_type)
    }

    pub(crate) fn key(&self, id: ElementId) -> Option<&WidgetKey> {
        self.arena.get(id).and_then(|node| node.key.as_ref())
    }

    pub(crate) fn property(&self, id: ElementId, property: PropertyId) -> Option<&PropertyValue> {
        let node = self.arena.get(id)?;
        let index = node
            .properties
            .binary_search_by_key(&property, |(candidate, _)| *candidate)
            .ok()?;
        Some(&node.properties[index].1)
    }

    pub(crate) fn layout_boundaries(&self, id: ElementId) -> Option<LayoutBoundaries> {
        self.arena.get(id).map(|node| node.boundaries)
    }

    pub(crate) fn mount(&mut self, widget: WidgetNode) -> Result<ReconcileReport> {
        self.owner.assert_current()?;
        let _state_guard = StateWriteGuard::enter("element reconciliation");
        if self.root.is_some() {
            return Err(Error::invalid_input(
                Some("root".to_owned()),
                "an element root is already mounted",
            ));
        }
        let mut report = ReconcileReport::default();
        let root = self.mount_subtree(None, widget, &mut report);
        self.root = Some(root);
        report.record_invalidation(root, true, PropertyImpact::ALL);
        self.publish_lifecycle(&mut report);
        Ok(report)
    }

    pub(crate) fn reconcile(&mut self, widget: WidgetNode) -> Result<ReconcileReport> {
        self.owner.assert_current()?;
        let _state_guard = StateWriteGuard::enter("element reconciliation");
        let Some(root) = self.root else {
            return self.mount(widget);
        };
        let mut report = ReconcileReport::default();
        if self.matches_widget(root, &widget) {
            self.reconcile_node(root, widget, &mut report)?;
        } else {
            self.unmount_subtree(root, &mut report)?;
            report.replaced += 1;
            let new_root = self.mount_subtree(None, widget, &mut report);
            self.root = Some(new_root);
            report.record_invalidation(new_root, true, PropertyImpact::ALL);
        }
        self.publish_lifecycle(&mut report);
        Ok(report)
    }

    pub(crate) fn unmount(&mut self) -> Result<ReconcileReport> {
        self.owner.assert_current()?;
        let _state_guard = StateWriteGuard::enter("element teardown");
        let mut report = ReconcileReport::default();
        if let Some(root) = self.root.take() {
            self.unmount_subtree(root, &mut report)?;
        }
        self.publish_lifecycle(&mut report);
        Ok(report)
    }

    pub(crate) fn rebuild_view(&mut self, view: &dyn View) -> Result<ReconcileReport> {
        self.owner.assert_current()?;
        let mut report = ReconcileReport::default();
        if self.root.is_none() {
            report.absorb(self.mount(WidgetNode::new::<ViewRoot>())?);
        }
        let root = self.root.expect("view root was mounted");
        if self.widget_type(root) != Some(&WidgetType::of::<ViewRoot>()) {
            return Err(Error::invalid_input(
                Some("view".to_owned()),
                "a direct widget root is already mounted",
            ));
        }

        let (declaration, dependencies) =
            capture_dependencies(self.dependency_owner, root, DependencyPhase::Build, || {
                view.build_view(&mut BuildContext::new())
            })?;
        report.absorb(self.reconcile(WidgetNode::new::<ViewRoot>().with_child(declaration))?);
        self.replace_dependencies(root, DependencyPhase::Build, dependencies)?;
        Ok(report)
    }

    /// Runs a non-build element phase under reactive dependency tracking and
    /// atomically replaces that phase's subscriptions after a successful read.
    /// Layout, paint, and semantics pipelines plug into this seam in later
    /// milestones without gaining mutable access to reactive State.
    #[allow(dead_code)]
    pub(crate) fn capture_phase_dependencies<R>(
        &mut self,
        element: ElementId,
        phase: DependencyPhase,
        read: impl FnOnce() -> Result<R>,
    ) -> Result<R> {
        self.owner.assert_current()?;
        if !self.contains(element) {
            return Err(Error::invalid_input(
                Some("element".to_owned()),
                "cannot capture dependencies for a stale element ID",
            ));
        }
        let (output, dependencies) =
            capture_dependencies(self.dependency_owner, element, phase, read)?;
        self.replace_dependencies(element, phase, dependencies)?;
        Ok(output)
    }

    pub(crate) fn layout_inputs(&self) -> Result<Vec<LayoutNodeInput>> {
        self.owner.assert_current()?;
        let Some(root) = self.root else {
            return Ok(Vec::new());
        };
        let mut result = Vec::with_capacity(self.len());
        let mut stack = vec![root];
        let mut seen = BTreeSet::new();
        while let Some(id) = stack.pop() {
            if !seen.insert(id) {
                return Err(Error::compile(
                    "layout_sync",
                    "Element topology contains a cycle",
                ));
            }
            let node = self.arena.get(id).ok_or_else(|| {
                Error::compile("layout_sync", "Element topology contains a stale ID")
            })?;
            let children = self.children(id);
            stack.extend(children.iter().rev().copied());
            result.push(LayoutNodeInput {
                id,
                parent: node.links.parent(),
                children,
                style: node.layout_style.clone(),
                measure: node.measure.clone(),
                scroll_offset: node.scroll_offset,
                hit_test: node.hit_test,
                boundaries: node.boundaries,
            });
        }
        if result.len() != self.len() {
            return Err(Error::compile(
                "layout_sync",
                "Element topology is disconnected",
            ));
        }
        Ok(result)
    }

    fn matches_widget(&self, id: ElementId, widget: &WidgetNode) -> bool {
        self.arena.get(id).is_some_and(|element| {
            element.key == widget.key && element.widget_type == widget.widget_type
        })
    }

    fn mount_subtree(
        &mut self,
        parent: Option<ElementId>,
        widget: WidgetNode,
        report: &mut ReconcileReport,
    ) -> ElementId {
        let element = ElementNode::from_widget(&widget, parent);
        let id = self.arena.insert(element);
        report.mounted += 1;
        if let Some(lifecycle) = widget.lifecycle.as_ref() {
            report
                .lifecycle_events
                .push((lifecycle.clone(), LifecycleEvent::Mounted(id)));
        }

        for key in duplicate_widget_keys(&widget.children) {
            report
                .diagnostics
                .push(ReconcileDiagnostic::DuplicateKey { parent: id, key });
        }

        let children = widget
            .children
            .into_iter()
            .map(|child| self.mount_subtree(Some(id), child, report))
            .collect::<Vec<_>>();
        self.rewrite_children(id, &children);
        id
    }

    fn reconcile_node(
        &mut self,
        id: ElementId,
        widget: WidgetNode,
        report: &mut ReconcileReport,
    ) -> Result<()> {
        let old_child_identity = self
            .children(id)
            .into_iter()
            .filter_map(|child| {
                self.arena
                    .get(child)
                    .map(|node| (node.key.clone(), node.widget_type.clone()))
            })
            .collect::<Vec<_>>();
        let new_child_identity = widget
            .children
            .iter()
            .map(|child| (child.key.clone(), child.widget_type.clone()))
            .collect::<Vec<_>>();
        let lifecycle = widget.lifecycle.clone();
        let children = widget.children.clone();
        let (
            property_impact,
            layout_changed,
            input_behavior_changed,
            hit_test_changed,
            boundary_changed,
            changed,
        ) = {
            let node = self
                .arena
                .get(id)
                .ok_or_else(|| Error::compile("reconcile", "matched element became stale"))?;
            let (property_impact, property_changed) = changed_property_impact(node, &widget);
            let layout_changed = node.layout_style != widget.layout_style
                || node.measure != widget.measure
                || node.scroll_offset != widget.scroll_offset;
            let input_behavior_changed = node.event_handler != widget.event_handler
                || node.focusable != widget.focusable
                || node.enabled != widget.enabled;
            let hit_test_changed = node.hit_test != widget.hit_test;
            let boundary_changed = node.boundaries != widget.boundaries;
            let changed = property_changed
                || !property_impact.is_empty()
                || layout_changed
                || input_behavior_changed
                || hit_test_changed
                || boundary_changed
                || node.lifecycle != lifecycle
                || old_child_identity != new_child_identity;
            (
                property_impact,
                layout_changed,
                input_behavior_changed,
                hit_test_changed,
                boundary_changed,
                changed,
            )
        };
        let node = self
            .arena
            .get_mut(id)
            .ok_or_else(|| Error::compile("reconcile", "matched element became stale"))?;
        node.properties = widget.properties;
        node.property_impacts = widget.property_impacts;
        node.lifecycle = lifecycle.clone();
        node.event_handler = widget.event_handler;
        node.focusable = widget.focusable;
        node.enabled = widget.enabled;
        node.layout_style = widget.layout_style;
        node.measure = widget.measure;
        node.scroll_offset = widget.scroll_offset;
        node.hit_test = widget.hit_test;
        node.boundaries = widget.boundaries;
        if changed {
            report.updated += 1;
            if let Some(lifecycle) = lifecycle {
                report
                    .lifecycle_events
                    .push((lifecycle, LifecycleEvent::Updated(id)));
            }
        }
        let mut impact = property_impact;
        if layout_changed {
            impact = impact.union(PropertyImpact::LAYOUT);
        }
        if input_behavior_changed {
            impact = impact.union(
                PropertyImpact::PAINT
                    .union(PropertyImpact::HIT_TEST)
                    .union(PropertyImpact::SEMANTICS),
            );
        }
        if hit_test_changed {
            impact = impact.union(PropertyImpact::HIT_TEST);
        }
        if boundary_changed {
            impact = PropertyImpact::ALL;
        }
        if old_child_identity != new_child_identity {
            report.record_invalidation(id, true, PropertyImpact::ALL);
        }
        if !impact.is_empty() {
            report.record_invalidation(id, false, impact);
        }
        self.reconcile_children(id, children, report)
    }

    fn reconcile_children(
        &mut self,
        parent: ElementId,
        widgets: Vec<WidgetNode>,
        report: &mut ReconcileReport,
    ) -> Result<()> {
        let old = self.children(parent);
        let duplicate_new = duplicate_widget_keys(&widgets);
        let duplicate_old = duplicate_element_keys(&self.arena, &old);
        if !duplicate_new.is_empty() || !duplicate_old.is_empty() {
            for key in duplicate_new {
                report
                    .diagnostics
                    .push(ReconcileDiagnostic::DuplicateKey { parent, key });
            }
            for key in duplicate_old {
                report
                    .diagnostics
                    .push(ReconcileDiagnostic::DuplicateExistingKey { parent, key });
            }
            report
                .diagnostics
                .push(ReconcileDiagnostic::RebuiltAmbiguousChildren { parent });
            report.record_invalidation(parent, true, PropertyImpact::ALL);
            for child in old {
                self.unmount_subtree(child, report)?;
            }
            let children = widgets
                .into_iter()
                .map(|widget| self.mount_subtree(Some(parent), widget, report))
                .collect::<Vec<_>>();
            self.rewrite_children(parent, &children);
            return Ok(());
        }

        let mut keyed_by_identity = BTreeMap::new();
        let mut keyed_by_key = BTreeMap::new();
        let mut unkeyed = Vec::new();
        for (index, id) in old.iter().copied().enumerate() {
            let node = self
                .arena
                .get(id)
                .ok_or_else(|| Error::compile("reconcile", "child link points to a stale ID"))?;
            if let Some(key) = node.key.clone() {
                keyed_by_identity.insert((key.clone(), node.widget_type.clone()), (index, id));
                keyed_by_key.insert(key, (index, id));
            } else {
                unkeyed.push((index, id));
            }
        }

        let mut used = BTreeSet::new();
        let mut unkeyed_index = 0;
        let mut next = Vec::with_capacity(widgets.len());
        for (new_index, widget) in widgets.into_iter().enumerate() {
            let matched = if let Some(key) = widget.key.clone() {
                keyed_by_identity
                    .get(&(key, widget.widget_type.clone()))
                    .copied()
            } else {
                let candidate = unkeyed.get(unkeyed_index).copied();
                unkeyed_index += 1;
                candidate
            };

            if let Some((old_index, id)) = matched {
                if self.matches_widget(id, &widget) {
                    used.insert(id);
                    if old_index != new_index {
                        report.moved += 1;
                    }
                    self.reconcile_node(id, widget, report)?;
                    next.push(id);
                    continue;
                }
            }

            if let Some(key) = widget.key.clone() {
                if let Some((_, replaced)) = keyed_by_key.get(&key).copied() {
                    if !used.contains(&replaced) {
                        used.insert(replaced);
                        self.unmount_subtree(replaced, report)?;
                        report.replaced += 1;
                    }
                }
            } else if let Some((_, replaced)) = matched {
                if !used.contains(&replaced) {
                    used.insert(replaced);
                    self.unmount_subtree(replaced, report)?;
                    report.replaced += 1;
                }
            }
            next.push(self.mount_subtree(Some(parent), widget, report));
        }

        for id in old {
            if !used.contains(&id) && self.arena.contains(id) {
                self.unmount_subtree(id, report)?;
            }
        }
        self.rewrite_children(parent, &next);
        Ok(())
    }

    fn rewrite_children(&mut self, parent: ElementId, children: &[ElementId]) {
        if let Some(node) = self.arena.get_mut(parent) {
            node.links.set_first_child(children.first().copied());
        }
        for (index, child) in children.iter().copied().enumerate() {
            if let Some(node) = self.arena.get_mut(child) {
                node.links.set_parent(Some(parent));
                node.links
                    .set_next_sibling(children.get(index + 1).copied());
            }
        }
    }

    fn unmount_subtree(&mut self, id: ElementId, report: &mut ReconcileReport) -> Result<()> {
        if !self.arena.contains(id) {
            return Ok(());
        }
        for child in self.children(id) {
            self.unmount_subtree(child, report)?;
        }

        let mut node = self
            .arena
            .remove(id)
            .ok_or_else(|| Error::compile("unmount", "element became stale during cleanup"))?;
        for cleanup in node.cleanup.drain(..) {
            cleanup();
        }
        node.dependencies.clear();
        node.subscriptions.clear();
        node.state_slots.clear();
        if let Some(lifecycle) = node.lifecycle {
            report
                .lifecycle_events
                .push((lifecycle, LifecycleEvent::Unmounted(id)));
        }
        report.unmounted += 1;
        report.removed_ids.push(id);
        if self.root == Some(id) {
            self.root = None;
        }
        Ok(())
    }

    fn publish_lifecycle(&self, report: &mut ReconcileReport) {
        for (callback, event) in report.lifecycle_events.drain(..) {
            callback.invoke(event);
        }
    }

    #[allow(dead_code)]
    pub(crate) fn set_state_slot<T: 'static>(
        &mut self,
        element: ElementId,
        slot: u64,
        value: T,
    ) -> Result<()> {
        self.owner.assert_current()?;
        let node = self
            .arena
            .get_mut(element)
            .ok_or_else(|| Error::invalid_input(Some("element".to_owned()), "stale element ID"))?;
        node.allocation_count += 1; // the new type-erased payload Box
        match node
            .state_slots
            .binary_search_by_key(&slot, |entry| entry.id)
        {
            Ok(index) => node.state_slots[index].value = Box::new(value),
            Err(index) => {
                if node.state_slots.len() == node.state_slots.capacity() {
                    node.allocation_count += 1;
                }
                node.state_slots.insert(
                    index,
                    StateSlot {
                        id: slot,
                        value: Box::new(value),
                    },
                );
            }
        }
        Ok(())
    }

    #[allow(dead_code)]
    pub(crate) fn state_slot<T: 'static>(&self, element: ElementId, slot: u64) -> Option<&T> {
        let node = self.arena.get(element)?;
        let index = node
            .state_slots
            .binary_search_by_key(&slot, |entry| entry.id)
            .ok()?;
        node.state_slots[index].value.downcast_ref()
    }

    #[allow(dead_code)]
    pub(crate) fn replace_subscription<T: 'static>(
        &mut self,
        element: ElementId,
        index: usize,
        subscription: T,
    ) -> Result<()> {
        self.owner.assert_current()?;
        let node = self
            .arena
            .get_mut(element)
            .ok_or_else(|| Error::invalid_input(Some("element".to_owned()), "stale element ID"))?;
        let subscription = Box::new(subscription) as Box<dyn Any>;
        node.allocation_count += 1;
        if index < node.subscriptions.len() {
            node.subscriptions[index] = subscription;
        } else if index == node.subscriptions.len() {
            if node.subscriptions.len() == node.subscriptions.capacity() {
                node.allocation_count += 1;
            }
            node.subscriptions.push(subscription);
        } else {
            return Err(Error::invalid_input(
                Some("subscription index".to_owned()),
                "subscription slots must be appended without gaps",
            ));
        }
        Ok(())
    }

    pub(crate) fn replace_dependencies(
        &mut self,
        element: ElementId,
        phase: DependencyPhase,
        dependencies: DependencySet,
    ) -> Result<()> {
        self.owner.assert_current()?;
        let node = self
            .arena
            .get_mut(element)
            .ok_or_else(|| Error::invalid_input(Some("element".to_owned()), "stale element ID"))?;
        if dependencies.is_empty() {
            node.dependencies.remove(&phase);
        } else {
            if !node.dependencies.contains_key(&phase) {
                node.allocation_count += 1;
            }
            node.dependencies.insert(phase, dependencies);
        }
        Ok(())
    }

    #[allow(dead_code)]
    pub(crate) fn register_cleanup<F>(&mut self, element: ElementId, cleanup: F) -> Result<()>
    where
        F: FnOnce() + 'static,
    {
        self.owner.assert_current()?;
        let node = self
            .arena
            .get_mut(element)
            .ok_or_else(|| Error::invalid_input(Some("element".to_owned()), "stale element ID"))?;
        node.allocation_count += 1;
        if node.cleanup.len() == node.cleanup.capacity() {
            node.allocation_count += 1;
        }
        node.cleanup.push(Box::new(cleanup));
        Ok(())
    }

    pub(crate) fn diagnostics(&self) -> Vec<ElementNodeDiagnostics> {
        let mut result = self
            .arena
            .iter()
            .map(|(id, node)| ElementNodeDiagnostics {
                id,
                parent: node.links.parent(),
                widget_type: node.widget_type.clone(),
                key: node.key.clone(),
                state_slots: node.state_slots.len(),
                subscriptions: node.subscription_count(),
                cleanup_callbacks: node.cleanup.len(),
                allocation_count: node.allocation_count,
                estimated_heap_bytes: node.auxiliary_heap_bytes(),
            })
            .collect::<Vec<_>>();
        result.sort_by_key(|node| (node.id.slot(), node.id.generation()));
        result
    }

    pub(crate) fn stats(&self) -> ElementTreeStats {
        let arena = self.arena.stats();
        let mut state_slots = 0;
        let mut subscriptions = 0;
        let mut cleanup_callbacks = 0;
        let mut allocation_count = 0_u64;
        let mut estimated_heap_bytes = 0;
        for node in self.arena.values() {
            state_slots += node.state_slots.len();
            subscriptions += node.subscription_count();
            cleanup_callbacks += node.cleanup.len();
            allocation_count += node.allocation_count;
            estimated_heap_bytes += node.auxiliary_heap_bytes();
        }
        ElementTreeStats {
            arena,
            state_slots,
            subscriptions,
            cleanup_callbacks,
            allocation_count,
            estimated_node_bytes: self
                .arena
                .capacity()
                .saturating_mul(size_of::<ElementNode>()),
            estimated_heap_bytes,
        }
    }
}

impl Default for ElementTree {
    fn default() -> Self {
        Self::new()
    }
}

impl EventTargetTree<UiCommand> for ElementTree {
    fn root(&self) -> Option<ElementId> {
        self.root()
    }

    fn contains(&self, element: ElementId) -> bool {
        self.contains(element)
    }

    fn parent(&self, element: ElementId) -> Option<ElementId> {
        self.parent(element)
    }

    fn event_handler(&self, element: ElementId) -> Option<EventHandler<UiCommand>> {
        self.arena
            .get(element)
            .and_then(|node| node.event_handler.clone())
    }

    fn is_focusable(&self, element: ElementId) -> bool {
        self.arena.get(element).is_some_and(|node| node.focusable)
    }

    fn is_enabled(&self, element: ElementId) -> bool {
        self.arena.get(element).is_some_and(|node| node.enabled)
    }
}

impl Drop for ElementTree {
    fn drop(&mut self) {
        let _state_guard = StateWriteGuard::enter("element teardown");
        let mut report = ReconcileReport::default();
        if let Some(root) = self.root.take() {
            let result = self.unmount_subtree(root, &mut report);
            debug_assert!(
                result.is_ok(),
                "element teardown must remain generation-valid"
            );
        }
        self.publish_lifecycle(&mut report);
    }
}

fn duplicate_widget_keys(widgets: &[WidgetNode]) -> Vec<WidgetKey> {
    let mut seen = BTreeSet::new();
    let mut duplicates = BTreeSet::new();
    for key in widgets.iter().filter_map(|widget| widget.key.clone()) {
        if !seen.insert(key.clone()) {
            duplicates.insert(key);
        }
    }
    duplicates.into_iter().collect()
}

fn changed_property_impact(node: &ElementNode, widget: &WidgetNode) -> (PropertyImpact, bool) {
    let mut ids = BTreeSet::new();
    ids.extend(node.properties.iter().map(|(id, _)| *id));
    ids.extend(widget.properties.iter().map(|(id, _)| *id));
    let mut impact = PropertyImpact::NONE;
    let mut property_changed = false;
    for id in ids {
        let old = node
            .properties
            .binary_search_by_key(&id, |(property, _)| *property)
            .ok()
            .map(|index| &node.properties[index].1);
        let new = widget
            .properties
            .binary_search_by_key(&id, |(property, _)| *property)
            .ok()
            .map(|index| &widget.properties[index].1);
        let old_impact = old.map_or(PropertyImpact::NONE, |_| {
            node.property_impacts
                .binary_search_by_key(&id, |(property, _)| *property)
                .ok()
                .map_or(PropertyImpact::ALL, |index| node.property_impacts[index].1)
        });
        let new_impact = new.map_or(PropertyImpact::NONE, |_| widget.property_impact(id));
        if old != new || old_impact != new_impact {
            property_changed = true;
            impact = impact.union(old_impact).union(new_impact);
        }
    }
    (impact, property_changed)
}

fn duplicate_element_keys(
    arena: &DenseArena<ElementNode, ElementId>,
    elements: &[ElementId],
) -> Vec<WidgetKey> {
    let mut seen = BTreeSet::new();
    let mut duplicates = BTreeSet::new();
    for key in elements
        .iter()
        .filter_map(|id| arena.get(*id).and_then(|node| node.key.clone()))
    {
        if !seen.insert(key.clone()) {
            duplicates.insert(key);
        }
    }
    duplicates.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    struct Root;
    struct Item;
    struct Other;
    struct A;
    struct B;
    struct C;
    struct X;
    struct Child;

    fn leaf(kind: &'static str, key: impl Into<WidgetKey>) -> WidgetNode {
        let node = match kind {
            "item" => WidgetNode::new::<Item>(),
            "other" => WidgetNode::new::<Other>(),
            unexpected => panic!("unexpected test widget kind {unexpected}"),
        };
        node.with_key(key)
    }

    fn root(children: impl IntoIterator<Item = WidgetNode>) -> WidgetNode {
        WidgetNode::new::<Root>().with_children(children)
    }

    fn keyed_ids(tree: &ElementTree) -> BTreeMap<WidgetKey, ElementId> {
        tree.ids()
            .filter_map(|id| tree.key(id).cloned().map(|key| (key, id)))
            .collect()
    }

    #[test]
    fn keyed_reorder_preserves_ids_state_slots_and_subscriptions() {
        use crate::state::{State, UpdateTxn};

        let state = State::new(1_u32);
        let mut tree = ElementTree::new();
        tree.mount(root([
            leaf("item", "a"),
            leaf("item", "b"),
            leaf("item", "c"),
        ]))
        .unwrap();
        let before = keyed_ids(&tree);
        tree.set_state_slot(before[&WidgetKey::from("b")], 7, 99_u32)
            .unwrap();
        tree.capture_phase_dependencies(
            before[&WidgetKey::from("b")],
            DependencyPhase::Paint,
            || state.get(),
        )
        .unwrap();

        let report = tree
            .reconcile(root([
                leaf("item", "c"),
                leaf("item", "a"),
                leaf("item", "b"),
            ]))
            .unwrap();
        let after = keyed_ids(&tree);

        assert_eq!(before, after);
        assert!(report.moved >= 2);
        assert_eq!(
            tree.state_slot::<u32>(after[&WidgetKey::from("b")], 7),
            Some(&99)
        );

        let mut transaction = UpdateTxn::<()>::new();
        state.set(&mut transaction, 2).unwrap();
        let receipt = transaction.commit(|_| Ok(())).unwrap();
        assert_eq!(receipt.invalidations().len(), 1);
        assert_eq!(
            receipt.invalidations()[0].element(),
            after[&WidgetKey::from("b")]
        );
        assert_eq!(receipt.invalidations()[0].phase(), DependencyPhase::Paint);
    }

    #[test]
    fn insertion_deletion_and_type_replacement_are_generation_safe() {
        let mut tree = ElementTree::new();
        tree.mount(root([leaf("item", "a"), leaf("item", "b")]))
            .unwrap();
        let old = keyed_ids(&tree);
        let stale_b = old[&WidgetKey::from("b")];

        let report = tree
            .reconcile(root([leaf("other", "b"), leaf("item", "c")]))
            .unwrap();
        let current = keyed_ids(&tree);
        assert!(report.replaced >= 1);
        assert!(!tree.contains(stale_b));
        assert_ne!(current[&WidgetKey::from("b")], stale_b);
        if current[&WidgetKey::from("b")].slot() == stale_b.slot() {
            assert_ne!(
                current[&WidgetKey::from("b")].generation(),
                stale_b.generation()
            );
        }
    }

    #[test]
    fn keyless_children_match_only_by_relative_position_and_type() {
        let mut tree = ElementTree::new();
        tree.mount(root([
            WidgetNode::new::<A>(),
            WidgetNode::new::<B>(),
            WidgetNode::new::<C>(),
        ]))
        .unwrap();
        let parent = tree.root().unwrap();
        let old = tree.children(parent);

        tree.reconcile(root([
            WidgetNode::new::<X>(),
            WidgetNode::new::<B>(),
            WidgetNode::new::<C>(),
        ]))
        .unwrap();
        let new = tree.children(parent);
        assert_ne!(new[0], old[0]);
        assert_eq!(new[1], old[1]);
        assert_eq!(new[2], old[2]);
    }

    #[test]
    fn duplicate_keys_are_diagnosed_and_use_full_sibling_fallback() {
        let mut tree = ElementTree::new();
        tree.mount(root([leaf("item", "a"), leaf("item", "b")]))
            .unwrap();
        let old = tree.children(tree.root().unwrap());

        let report = tree
            .reconcile(root([leaf("item", "a"), leaf("item", "a")]))
            .unwrap();
        assert!(report.used_safe_fallback());
        assert!(old.into_iter().all(|id| !tree.contains(id)));
    }

    #[test]
    fn lifecycle_is_parent_first_on_mount_and_child_first_on_unmount() {
        let events = Rc::new(RefCell::new(Vec::new()));
        let callback = |identity, events: Rc<RefCell<Vec<(u64, LifecycleEvent)>>>| {
            LifecycleCallback::new(identity, move |event| {
                events.borrow_mut().push((identity, event));
            })
        };
        let child = WidgetNode::new::<Child>().with_lifecycle(callback(2, events.clone()));
        let declaration = WidgetNode::new::<Root>()
            .with_lifecycle(callback(1, events.clone()))
            .with_child(child);
        let mut tree = ElementTree::new();
        tree.mount(declaration).unwrap();
        tree.unmount().unwrap();

        let identities = events
            .borrow()
            .iter()
            .map(|(identity, _)| *identity)
            .collect::<Vec<_>>();
        assert_eq!(identities, [1, 2, 2, 1]);
    }

    #[test]
    fn unchanged_declarations_do_not_publish_updates() {
        let events = Rc::new(RefCell::new(Vec::new()));
        let events_copy = events.clone();
        let callback = LifecycleCallback::new(1, move |event| {
            events_copy.borrow_mut().push(event);
        });
        let declaration = WidgetNode::new::<Root>()
            .with_lifecycle(callback.clone())
            .with_child(WidgetNode::new::<Child>());
        let mut tree = ElementTree::new();
        tree.mount(declaration.clone()).unwrap();
        events.borrow_mut().clear();

        let report = tree.reconcile(declaration).unwrap();
        assert_eq!(report.updated, 0);
        assert!(events.borrow().is_empty());
    }

    #[test]
    fn none_impact_property_changes_still_publish_updates_without_dirty_work() {
        let events = Rc::new(RefCell::new(Vec::new()));
        let events_copy = events.clone();
        let lifecycle = LifecycleCallback::new(1, move |event| {
            events_copy.borrow_mut().push(event);
        });
        let declaration = |value| {
            WidgetNode::new::<Root>()
                .with_property(PropertyId::new(7), value)
                .with_property_impact(PropertyId::new(7), PropertyImpact::NONE)
                .with_lifecycle(lifecycle.clone())
        };

        let mut tree = ElementTree::new();
        tree.mount(declaration(1_u64)).unwrap();
        let root = tree.root().unwrap();
        events.borrow_mut().clear();

        let report = tree.reconcile(declaration(2_u64)).unwrap();

        assert_eq!(report.updated, 1);
        assert_eq!(report.invalidations().count(), 0);
        assert_eq!(events.borrow().as_slice(), &[LifecycleEvent::Updated(root)]);
    }

    #[test]
    fn dropping_a_mounted_tree_runs_teardown_and_lifecycle() {
        let lifecycle = Rc::new(RefCell::new(Vec::new()));
        let cleanups = Rc::new(RefCell::new(0));
        let lifecycle_copy = lifecycle.clone();
        let callback = LifecycleCallback::new(1, move |event| {
            lifecycle_copy.borrow_mut().push(event);
        });
        {
            let mut tree = ElementTree::new();
            tree.mount(WidgetNode::new::<Root>().with_lifecycle(callback))
                .unwrap();
            let root = tree.root().unwrap();
            let cleanups_copy = cleanups.clone();
            tree.register_cleanup(root, move || *cleanups_copy.borrow_mut() += 1)
                .unwrap();
        }

        assert_eq!(*cleanups.borrow(), 1);
        assert!(matches!(
            lifecycle.borrow().as_slice(),
            [LifecycleEvent::Mounted(_), LifecycleEvent::Unmounted(_)]
        ));
    }

    #[test]
    fn lifecycle_callbacks_cannot_publish_reentrant_state() {
        use crate::state::{State, UpdateTxn};
        use std::cell::Cell;

        let state = State::new(0_u32);
        let callback_state = state.clone();
        let rejected = Rc::new(Cell::new(false));
        let rejected_copy = rejected.clone();
        let callback = LifecycleCallback::new(1, move |event| {
            if matches!(event, LifecycleEvent::Mounted(_)) {
                let mut transaction = UpdateTxn::<()>::new();
                rejected_copy.set(callback_state.set(&mut transaction, 1).is_err());
            }
        });

        let mut tree = ElementTree::new();
        tree.mount(WidgetNode::new::<Root>().with_lifecycle(callback))
            .unwrap();
        assert!(rejected.get());
        assert_eq!(state.get().unwrap(), 0);
    }

    #[test]
    fn unmount_drops_slots_subscriptions_and_runs_cleanup() {
        struct DropFlag(Rc<RefCell<usize>>);
        impl Drop for DropFlag {
            fn drop(&mut self) {
                *self.0.borrow_mut() += 1;
            }
        }

        let drops = Rc::new(RefCell::new(0));
        let cleanups = Rc::new(RefCell::new(0));
        let mut tree = ElementTree::new();
        tree.mount(root([leaf("item", "a")])).unwrap();
        let child = tree.children(tree.root().unwrap())[0];
        tree.set_state_slot(child, 1, DropFlag(drops.clone()))
            .unwrap();
        tree.replace_subscription(child, 0, DropFlag(drops.clone()))
            .unwrap();
        let cleanups_copy = cleanups.clone();
        tree.register_cleanup(child, move || *cleanups_copy.borrow_mut() += 1)
            .unwrap();

        tree.reconcile(root([])).unwrap();
        assert_eq!(*drops.borrow(), 2);
        assert_eq!(*cleanups.borrow(), 1);
        assert!(!tree.contains(child));
    }

    #[test]
    fn replacing_phase_dependencies_unsubscribes_old_signals() {
        use crate::state::{State, UpdateTxn, capture_dependencies};

        let first = State::new(1_u32);
        let second = State::new(2_u32);
        let mut tree = ElementTree::new();
        tree.mount(root([leaf("item", "a")])).unwrap();
        let child = tree.children(tree.root().unwrap())[0];

        let owner = tree.dependency_owner();
        let (_, initial) = capture_dependencies(owner, child, DependencyPhase::Build, || {
            Ok((first.get()?, second.get()?))
        })
        .unwrap();
        tree.replace_dependencies(child, DependencyPhase::Build, initial)
            .unwrap();

        let (_, replacement) =
            capture_dependencies(owner, child, DependencyPhase::Build, || second.get()).unwrap();
        tree.replace_dependencies(child, DependencyPhase::Build, replacement)
            .unwrap();

        let mut transaction = UpdateTxn::<()>::new();
        first.set(&mut transaction, 9).unwrap();
        let receipt = transaction.commit(|_| Ok(())).unwrap();
        assert!(receipt.invalidations().is_empty());

        let mut transaction = UpdateTxn::<()>::new();
        second.set(&mut transaction, 10).unwrap();
        let receipt = transaction.commit(|_| Ok(())).unwrap();
        assert_eq!(receipt.invalidations()[0].element(), child);
    }

    #[test]
    fn non_build_phases_track_dependencies_and_forbid_writes() {
        use crate::state::{State, UpdateTxn};

        let state = State::new(1_u32);
        let mut tree = ElementTree::new();
        tree.mount(root([leaf("item", "a")])).unwrap();
        let child = tree.children(tree.root().unwrap())[0];
        let phases = [
            DependencyPhase::Measure,
            DependencyPhase::Layout,
            DependencyPhase::Paint,
            DependencyPhase::Semantics,
        ];

        for phase in phases {
            let mut rejected = UpdateTxn::<()>::new();
            let value = tree
                .capture_phase_dependencies(child, phase, || {
                    let error = state.set(&mut rejected, 9).unwrap_err();
                    assert!(error.to_string().contains(&format!("{phase:?}")));
                    state.get()
                })
                .unwrap();
            assert_eq!(value, 1);
        }

        let mut transaction = UpdateTxn::<()>::new();
        state.set(&mut transaction, 2).unwrap();
        let receipt = transaction.commit(|_| Ok(())).unwrap();
        assert_eq!(receipt.invalidations().len(), phases.len());
        for phase in phases {
            assert!(receipt.invalidations().iter().any(|invalidation| {
                invalidation.element() == child && invalidation.phase() == phase
            }));
        }
    }

    #[test]
    fn diagnostics_include_slots_generations_and_auxiliary_allocations() {
        let mut tree = ElementTree::with_capacity(4);
        tree.mount(root([leaf("item", "a")])).unwrap();
        let child = tree.children(tree.root().unwrap())[0];
        tree.set_state_slot(child, 1, 7_u32).unwrap();
        let diagnostics = tree.diagnostics();
        let child_diagnostics = diagnostics.iter().find(|node| node.id == child).unwrap();
        assert_eq!(child_diagnostics.id.generation(), child.generation());
        assert_eq!(child_diagnostics.state_slots, 1);
        assert!(child_diagnostics.allocation_count >= 2);
        assert!(tree.stats().arena.slots >= 2);
    }
}
