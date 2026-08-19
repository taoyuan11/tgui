//! UI-thread Dirty Tree overlay.
//!
//! Dirty state is an index over the retained Element topology, never a second
//! source of UI truth. Every mark is mergeable and repeatable within an epoch;
//! roots are reduced against the current parent/boundary map before being
//! handed to the layout/render/semantic schedulers.

use crate::core::{ElementId, Error, Result};
use crate::layout::LayoutBoundaries;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::ops::{BitOr, BitOrAssign};

/// Reasons tracked by the P2 invalidation index.
#[derive(Clone, Copy, Default, PartialEq, Eq, Hash)]
pub(crate) struct DirtyFlags(u8);

impl DirtyFlags {
    pub(crate) const NONE: Self = Self(0);
    pub(crate) const STRUCTURE: Self = Self(1 << 0);
    pub(crate) const LAYOUT: Self = Self(1 << 1);
    pub(crate) const PAINT: Self = Self(1 << 2);
    pub(crate) const HIT_TEST: Self = Self(1 << 3);
    pub(crate) const SEMANTICS: Self = Self(1 << 4);
    pub(crate) const RESOURCE: Self = Self(1 << 5);

    pub(crate) const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    pub(crate) const fn is_empty(self) -> bool {
        self.0 == 0
    }

    pub(crate) const fn all_non_structure() -> Self {
        Self(
            Self::LAYOUT.0
                | Self::PAINT.0
                | Self::HIT_TEST.0
                | Self::SEMANTICS.0
                | Self::RESOURCE.0,
        )
    }
}

impl BitOr for DirtyFlags {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl BitOrAssign for DirtyFlags {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

impl fmt::Debug for DirtyFlags {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut names = Vec::new();
        for (flag, name) in [
            (Self::STRUCTURE, "STRUCTURE"),
            (Self::LAYOUT, "LAYOUT"),
            (Self::PAINT, "PAINT"),
            (Self::HIT_TEST, "HIT_TEST"),
            (Self::SEMANTICS, "SEMANTICS"),
            (Self::RESOURCE, "RESOURCE"),
        ] {
            if self.contains(flag) {
                names.push(name);
            }
        }
        formatter.write_str(&names.join(" | "))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct DirtyNodeSpec {
    pub id: ElementId,
    pub parent: Option<ElementId>,
    pub boundaries: LayoutBoundaries,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct DirtyNodeState {
    pub self_flags: DirtyFlags,
    pub subtree_flags: DirtyFlags,
    parent: Option<ElementId>,
    boundaries: LayoutBoundaries,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct DirtyRootCounts {
    pub structure: usize,
    pub layout: usize,
    pub paint: usize,
    pub hit_test: usize,
    pub semantics: usize,
    pub resource: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DirtyNodeReport {
    pub id: ElementId,
    pub self_flags: DirtyFlags,
    pub subtree_flags: DirtyFlags,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DirtyBatch {
    pub epoch: u64,
    pub counts: DirtyRootCounts,
    pub structure_roots: Vec<ElementId>,
    pub layout_roots: Vec<ElementId>,
    pub paint_roots: Vec<ElementId>,
    pub hit_test_roots: Vec<ElementId>,
    pub semantics_roots: Vec<ElementId>,
    pub resource_roots: Vec<ElementId>,
    pub nodes: Vec<DirtyNodeReport>,
    pub full_layout_fallback: bool,
}

impl DirtyBatch {
    pub(crate) fn has_layout_work(&self) -> bool {
        self.full_layout_fallback || !self.layout_roots.is_empty()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum QueueKind {
    Structure,
    Layout,
    Paint,
    HitTest,
    Semantics,
    Resource,
}

/// UI-thread-owned dirty index with epoch-based deduplication.
#[derive(Clone, Debug)]
pub(crate) struct DirtyTree {
    nodes: BTreeMap<ElementId, DirtyNodeState>,
    roots: BTreeMap<QueueKind, BTreeSet<ElementId>>,
    epoch: u64,
    full_layout_fallback: bool,
}

impl DirtyTree {
    pub(crate) fn new() -> Self {
        Self {
            nodes: BTreeMap::new(),
            roots: BTreeMap::new(),
            epoch: 1,
            full_layout_fallback: false,
        }
    }

    /// Reconciles the overlay's topology. Existing flags survive; removed IDs
    /// and their stale generations are discarded before the next mark.
    pub(crate) fn sync(&mut self, specs: impl IntoIterator<Item = DirtyNodeSpec>) {
        let next = specs
            .into_iter()
            .map(|spec| (spec.id, (spec.parent, spec.boundaries)))
            .collect::<BTreeMap<_, _>>();
        self.nodes.retain(|id, _| next.contains_key(id));
        for (id, (parent, boundaries)) in next {
            let state = self.nodes.entry(id).or_default();
            state.parent = parent;
            state.boundaries = boundaries;
        }
        for roots in self.roots.values_mut() {
            roots.retain(|id| self.nodes.contains_key(id));
        }
    }

    /// Marks one Element and propagates only subtree bits to the nearest
    /// matching boundary. Repeated calls merge into one root per queue.
    pub(crate) fn mark(
        &mut self,
        element: ElementId,
        flags: DirtyFlags,
        intrinsic_size_changed: bool,
    ) -> Result<()> {
        if !self.nodes.contains_key(&element) {
            self.full_layout_fallback = true;
            let root = self.root_id();
            self.queue_root(QueueKind::Layout, root);
            return Ok(());
        }
        let mut expanded = flags;
        if flags.contains(DirtyFlags::STRUCTURE) {
            expanded |= DirtyFlags::LAYOUT
                | DirtyFlags::PAINT
                | DirtyFlags::HIT_TEST
                | DirtyFlags::SEMANTICS;
        }
        if flags.contains(DirtyFlags::LAYOUT) || intrinsic_size_changed {
            expanded |= DirtyFlags::LAYOUT
                | DirtyFlags::PAINT
                | DirtyFlags::HIT_TEST
                | DirtyFlags::SEMANTICS;
        }
        if flags.contains(DirtyFlags::RESOURCE) {
            expanded |= DirtyFlags::PAINT;
        }

        if let Some(state) = self.nodes.get_mut(&element) {
            state.self_flags |= expanded;
        }

        if flags.contains(DirtyFlags::STRUCTURE) {
            self.propagate_to_boundary(element, DirtyFlags::STRUCTURE, None);
            self.queue_root(QueueKind::Structure, Some(element));
        }
        if expanded.contains(DirtyFlags::LAYOUT) {
            self.propagate_to_boundary(element, DirtyFlags::LAYOUT, Some(BoundaryKind::Layout));
            let root = self.nearest_boundary(element, BoundaryKind::Layout);
            self.queue_root(QueueKind::Layout, root);
        }
        if expanded.contains(DirtyFlags::PAINT) {
            self.propagate_to_boundary(element, DirtyFlags::PAINT, Some(BoundaryKind::Render));
            let root = self.nearest_boundary(element, BoundaryKind::Render);
            self.queue_root(QueueKind::Paint, root);
        }
        if expanded.contains(DirtyFlags::HIT_TEST) {
            self.propagate_to_boundary(element, DirtyFlags::HIT_TEST, Some(BoundaryKind::HitTest));
            let root = self.nearest_boundary(element, BoundaryKind::HitTest);
            self.queue_root(QueueKind::HitTest, root);
        }
        if expanded.contains(DirtyFlags::SEMANTICS) {
            self.propagate_to_boundary(
                element,
                DirtyFlags::SEMANTICS,
                Some(BoundaryKind::Semantics),
            );
            let root = self.nearest_boundary(element, BoundaryKind::Semantics);
            self.queue_root(QueueKind::Semantics, root);
        }
        if flags.contains(DirtyFlags::RESOURCE) {
            self.propagate_to_boundary(element, DirtyFlags::RESOURCE, Some(BoundaryKind::Render));
            let root = self.nearest_boundary(element, BoundaryKind::Render);
            self.queue_root(QueueKind::Resource, root);
        }
        Ok(())
    }

    pub(crate) fn mark_full_layout_fallback(&mut self) {
        self.full_layout_fallback = true;
        let root = self.root_id();
        self.queue_root(QueueKind::Layout, root);
    }

    /// Takes a stable view without clearing it. A failed layout can therefore
    /// be retried, while repeated consumers in one frame observe the same epoch.
    pub(crate) fn batch(&self) -> DirtyBatch {
        let roots = |kind: QueueKind| {
            self.roots
                .get(&kind)
                .map(|roots| roots.iter().copied().collect::<Vec<_>>())
                .unwrap_or_default()
        };
        let structure_roots = roots(QueueKind::Structure);
        let layout_roots = roots(QueueKind::Layout);
        let paint_roots = roots(QueueKind::Paint);
        let hit_test_roots = roots(QueueKind::HitTest);
        let semantics_roots = roots(QueueKind::Semantics);
        let resource_roots = roots(QueueKind::Resource);
        let nodes = self
            .nodes
            .iter()
            .filter_map(|(id, state)| {
                (!state.self_flags.is_empty() || !state.subtree_flags.is_empty()).then_some(
                    DirtyNodeReport {
                        id: *id,
                        self_flags: state.self_flags,
                        subtree_flags: state.subtree_flags,
                    },
                )
            })
            .collect::<Vec<_>>();
        DirtyBatch {
            epoch: self.epoch,
            counts: DirtyRootCounts {
                structure: structure_roots.len(),
                layout: layout_roots.len(),
                paint: paint_roots.len(),
                hit_test: hit_test_roots.len(),
                semantics: semantics_roots.len(),
                resource: resource_roots.len(),
            },
            structure_roots,
            layout_roots,
            paint_roots,
            hit_test_roots,
            semantics_roots,
            resource_roots,
            nodes,
            full_layout_fallback: self.full_layout_fallback,
        }
    }

    pub(crate) fn finish_epoch(&mut self) -> Result<()> {
        self.nodes.values_mut().for_each(|state| {
            state.self_flags = DirtyFlags::NONE;
            state.subtree_flags = DirtyFlags::NONE;
        });
        self.roots.clear();
        self.full_layout_fallback = false;
        self.epoch = self
            .epoch
            .checked_add(1)
            .ok_or_else(|| Error::compile("dirty_epoch", "dirty submission epoch exhausted"))?;
        Ok(())
    }

    fn root_id(&self) -> Option<ElementId> {
        self.nodes
            .iter()
            .find_map(|(id, state)| state.parent.is_none().then_some(*id))
    }

    fn queue_root(&mut self, kind: QueueKind, candidate: Option<ElementId>) {
        let Some(candidate) = candidate else {
            return;
        };
        let existing = self.roots.get(&kind).cloned().unwrap_or_default();
        if existing
            .iter()
            .any(|root| self.is_ancestor(*root, candidate))
        {
            return;
        }
        let descendants = existing
            .iter()
            .filter(|root| self.is_ancestor(candidate, **root))
            .copied()
            .collect::<Vec<_>>();
        let roots = self.roots.entry(kind).or_default();
        for descendant in descendants {
            roots.remove(&descendant);
        }
        roots.insert(candidate);
    }

    fn propagate_to_boundary(
        &mut self,
        element: ElementId,
        flag: DirtyFlags,
        boundary: Option<BoundaryKind>,
    ) {
        let root = boundary.and_then(|kind| self.nearest_boundary(element, kind));
        let mut current = self.nodes.get(&element).and_then(|state| state.parent);
        while let Some(id) = current {
            if let Some(state) = self.nodes.get_mut(&id) {
                state.subtree_flags |= flag;
            }
            if root == Some(id) {
                break;
            }
            current = self.nodes.get(&id).and_then(|state| state.parent);
        }
    }

    fn nearest_boundary(&self, element: ElementId, kind: BoundaryKind) -> Option<ElementId> {
        let mut current = Some(element);
        while let Some(id) = current {
            let Some(state) = self.nodes.get(&id) else {
                break;
            };
            let is_boundary = match kind {
                BoundaryKind::Layout => state.boundaries.layout,
                BoundaryKind::Render => state.boundaries.render,
                BoundaryKind::HitTest => state.boundaries.hit_test,
                BoundaryKind::Semantics => state.boundaries.semantics,
            };
            if is_boundary || state.parent.is_none() {
                return Some(id);
            }
            current = state.parent;
        }
        None
    }

    fn is_ancestor(&self, ancestor: ElementId, descendant: ElementId) -> bool {
        let mut current = Some(descendant);
        while let Some(id) = current {
            if id == ancestor {
                return true;
            }
            current = self.nodes.get(&id).and_then(|state| state.parent);
        }
        false
    }
}

impl Default for DirtyTree {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy)]
enum BoundaryKind {
    Layout,
    Render,
    HitTest,
    Semantics,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(slot: u32) -> ElementId {
        ElementId::from_parts(slot, 1)
    }

    fn tree() -> DirtyTree {
        let root = id(0);
        let boundary = id(1);
        let leaf = id(2);
        let mut tree = DirtyTree::new();
        tree.sync([
            DirtyNodeSpec {
                id: root,
                parent: None,
                boundaries: LayoutBoundaries::NONE,
            },
            DirtyNodeSpec {
                id: boundary,
                parent: Some(root),
                boundaries: LayoutBoundaries::ALL,
            },
            DirtyNodeSpec {
                id: leaf,
                parent: Some(boundary),
                boundaries: LayoutBoundaries::NONE,
            },
        ]);
        tree
    }

    #[test]
    fn paint_stops_at_render_boundary_without_upgrading_parent_self_flags() {
        let leaf = id(2);
        let mut tree = tree();
        tree.mark(leaf, DirtyFlags::PAINT, false).unwrap();
        let batch = tree.batch();
        assert_eq!(batch.paint_roots, vec![id(1)]);
        assert!(
            batch
                .nodes
                .iter()
                .all(|node| { node.id == leaf || node.self_flags.is_empty() })
        );
        let boundary = batch.nodes.iter().find(|node| node.id == id(1)).unwrap();
        assert!(boundary.subtree_flags.contains(DirtyFlags::PAINT));
        assert!(boundary.self_flags.is_empty());
    }

    #[test]
    fn layout_expands_to_paint_hit_and_semantics_but_roots_are_deduplicated() {
        let leaf = id(2);
        let mut tree = tree();
        tree.mark(leaf, DirtyFlags::LAYOUT, false).unwrap();
        tree.mark(leaf, DirtyFlags::LAYOUT, false).unwrap();
        let batch = tree.batch();
        assert_eq!(batch.layout_roots, vec![id(1)]);
        assert_eq!(batch.paint_roots, vec![id(1)]);
        assert_eq!(batch.hit_test_roots, vec![id(1)]);
        assert_eq!(batch.semantics_roots, vec![id(1)]);
        assert_eq!(batch.epoch, 1);
    }

    #[test]
    fn resource_marks_paint_and_intrinsic_change_adds_layout() {
        let leaf = id(2);
        let mut tree = tree();
        tree.mark(leaf, DirtyFlags::RESOURCE, false).unwrap();
        assert!(tree.batch().layout_roots.is_empty());
        tree.finish_epoch().unwrap();
        tree.mark(leaf, DirtyFlags::RESOURCE, true).unwrap();
        assert_eq!(tree.batch().layout_roots, vec![id(1)]);
    }

    #[test]
    fn ancestor_root_replaces_descendant_root_and_batch_is_retryable() {
        let leaf = id(2);
        let boundary = id(1);
        let mut tree = tree();
        tree.mark(leaf, DirtyFlags::PAINT, false).unwrap();
        tree.mark(boundary, DirtyFlags::PAINT, false).unwrap();
        assert_eq!(tree.batch().paint_roots, vec![boundary]);
        let before = tree.batch();
        assert_eq!(before, tree.batch());
        tree.finish_epoch().unwrap();
        assert!(tree.batch().nodes.is_empty());
    }

    #[test]
    fn stale_mark_uses_full_layout_fallback() {
        let mut tree = tree();
        tree.mark(id(99), DirtyFlags::PAINT, false).unwrap();
        assert!(tree.batch().full_layout_fallback);
        assert_eq!(tree.batch().layout_roots, vec![id(0)]);
    }

    #[test]
    fn propagation_matrix_keeps_independent_reasons_independent() {
        let leaf = id(2);
        let mut tree = tree();

        tree.mark(leaf, DirtyFlags::HIT_TEST, false).unwrap();
        let hit = tree.batch();
        assert_eq!(hit.hit_test_roots, vec![id(1)]);
        assert!(hit.layout_roots.is_empty());
        assert!(hit.paint_roots.is_empty());
        assert!(hit.semantics_roots.is_empty());
        tree.finish_epoch().unwrap();

        tree.mark(leaf, DirtyFlags::SEMANTICS, false).unwrap();
        let semantics = tree.batch();
        assert_eq!(semantics.semantics_roots, vec![id(1)]);
        assert!(semantics.layout_roots.is_empty());
        assert!(semantics.paint_roots.is_empty());
        tree.finish_epoch().unwrap();

        tree.mark(leaf, DirtyFlags::STRUCTURE, false).unwrap();
        let structure = tree.batch();
        assert_eq!(structure.structure_roots, vec![leaf]);
        assert_eq!(structure.layout_roots, vec![id(1)]);
        assert_eq!(structure.paint_roots, vec![id(1)]);
        assert_eq!(structure.hit_test_roots, vec![id(1)]);
        assert_eq!(structure.semantics_roots, vec![id(1)]);
    }
}
