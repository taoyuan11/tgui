use super::{
    AvailableDimension, AvailableSize, KnownDimensions, LayoutBoundaries, LayoutNode,
    LayoutSnapshot, LayoutStyle, MeasureCache, MeasureCacheStats, MeasureHandle, MeasureInput,
    MeasureOutput, MeasureSpec,
};
use crate::core::{DpiScale, ElementId, Error, LayoutRevision, Point, Rect, Result, Size};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct LayoutNodeInput {
    pub id: ElementId,
    pub parent: Option<ElementId>,
    pub children: Vec<ElementId>,
    pub style: LayoutStyle,
    pub measure: Option<MeasureSpec>,
    pub scroll_offset: Point,
    pub hit_test: bool,
    pub boundaries: LayoutBoundaries,
}

impl LayoutNodeInput {
    fn validate(&self) -> Result<()> {
        self.style.validate()?;
        self.scroll_offset.validate().map_err(Error::from)?;
        if self.scroll_offset.x < 0.0 || self.scroll_offset.y < 0.0 {
            return Err(Error::invalid_input(
                Some("scroll_offset".to_owned()),
                "scroll offsets must be non-negative logical pixels",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LayoutPassReport {
    pub full_rebuild: bool,
    pub safe_fallback: bool,
    pub synced_nodes: usize,
    pub removed_nodes: usize,
    pub observable_changed: bool,
    pub measure_cache_before: MeasureCacheStats,
    pub measure_cache_after: MeasureCacheStats,
}

struct PreparedNode {
    input: LayoutNodeInput,
    style: taffy::Style,
}

#[derive(Clone, Copy, Debug, Default)]
struct SyncReport {
    full_rebuild: bool,
    safe_fallback: bool,
    synced_nodes: usize,
    removed_nodes: usize,
}

#[derive(Clone, Copy, Debug)]
struct MeasureRecord {
    output: MeasureOutput,
}

#[derive(Debug)]
pub(crate) struct LayoutEngine {
    tree: taffy::TaffyTree<ElementId>,
    taffy_nodes: BTreeMap<ElementId, taffy::NodeId>,
    inputs: BTreeMap<ElementId, LayoutNodeInput>,
    root: Option<ElementId>,
    measure_cache: MeasureCache,
    measure_records: BTreeMap<ElementId, Vec<MeasureRecord>>,
    last_scale: Option<DpiScale>,
    committed: LayoutSnapshot,
}

impl LayoutEngine {
    pub(crate) fn new() -> Self {
        Self {
            tree: new_taffy_tree(),
            taffy_nodes: BTreeMap::new(),
            inputs: BTreeMap::new(),
            root: None,
            measure_cache: MeasureCache::default(),
            measure_records: BTreeMap::new(),
            last_scale: None,
            committed: LayoutSnapshot::default(),
        }
    }

    pub(crate) fn committed(&self) -> &LayoutSnapshot {
        &self.committed
    }

    pub(crate) fn adopt_committed(&mut self, snapshot: LayoutSnapshot) {
        self.committed = snapshot;
    }

    pub(crate) fn invalidate(&mut self, element: ElementId) -> Result<()> {
        if let Some(node) = self.taffy_nodes.get(&element).copied() {
            self.tree.mark_dirty(node).map_err(taffy_error)?;
        }
        Ok(())
    }

    pub(crate) fn invalidate_measure(&mut self, element: ElementId) -> Result<()> {
        self.measure_cache.invalidate(element);
        self.measure_records.remove(&element);
        self.invalidate(element)
    }

    pub(crate) fn compute(
        &mut self,
        nodes: Vec<LayoutNodeInput>,
        root: Option<ElementId>,
        viewport: Size,
        scale: DpiScale,
        force_full: bool,
        mut run_measure: impl FnMut(ElementId, &MeasureHandle, MeasureInput) -> Result<MeasureOutput>,
    ) -> Result<(LayoutSnapshot, LayoutPassReport)> {
        super::style::validate_viewport(viewport)?;
        let measure_cache_before = self.measure_cache.stats();
        let sync = self.sync(nodes, root, force_full)?;

        if self.last_scale != Some(scale) {
            self.measure_cache.clear();
            let measured = self
                .inputs
                .iter()
                .filter_map(|(element, input)| input.measure.as_ref().map(|_| *element))
                .collect::<Vec<_>>();
            for element in measured {
                self.invalidate(element)?;
            }
            self.last_scale = Some(scale);
        }

        let Some(root_element) = self.root else {
            let candidate = LayoutSnapshot::computed(LayoutRevision::ZERO, viewport, Vec::new())?;
            let observable_changed = !candidate.observable_eq(&self.committed);
            let snapshot = self.commit_candidate(candidate)?;
            let report = LayoutPassReport {
                full_rebuild: sync.full_rebuild,
                safe_fallback: sync.safe_fallback,
                synced_nodes: sync.synced_nodes,
                removed_nodes: sync.removed_nodes,
                observable_changed,
                measure_cache_before,
                measure_cache_after: self.measure_cache.stats(),
            };
            return Ok((snapshot, report));
        };
        let root_node = self.taffy_nodes[&root_element];
        let inputs = &self.inputs;
        let cache = &mut self.measure_cache;
        let mut pass_measure_records: BTreeMap<ElementId, Vec<MeasureRecord>> = BTreeMap::new();
        let mut measure_error = None;
        let mut failed_measurements = BTreeSet::new();
        self.tree
            .compute_layout_with_measure(
                root_node,
                taffy::geometry::Size {
                    width: taffy::AvailableSpace::Definite(viewport.width),
                    height: taffy::AvailableSpace::Definite(viewport.height),
                },
                |known, available, _, context, _| {
                    let Some(element) = context.map(|element| *element) else {
                        return taffy::geometry::Size::ZERO;
                    };
                    let Some(input) = inputs.get(&element) else {
                        return taffy::geometry::Size::ZERO;
                    };
                    let Some(spec) = input.measure.as_ref() else {
                        return taffy::geometry::Size::ZERO;
                    };
                    let measure_input = MeasureInput {
                        known_dimensions: KnownDimensions {
                            width: known.width,
                            height: known.height,
                        },
                        available_space: AvailableSize {
                            width: from_taffy_available(available.width),
                            height: from_taffy_available(available.height),
                        },
                        style_fingerprint: input.style.fingerprint(),
                        content_generation: spec.content_generation(),
                        font_generation: spec.font_generation(),
                        scale,
                    };
                    match cache.get_or_measure(element, spec, measure_input, |handle, input| {
                        run_measure(element, handle, input)
                    }) {
                        Ok(mut output) => {
                            if let Some(width) = known.width {
                                output.size.width = width;
                            }
                            if let Some(height) = known.height {
                                output.size.height = height;
                            }
                            pass_measure_records
                                .entry(element)
                                .or_default()
                                .push(MeasureRecord { output });
                            taffy::geometry::Size {
                                width: output.size.width,
                                height: output.size.height,
                            }
                        }
                        Err(error) => {
                            failed_measurements.insert(element);
                            if measure_error.is_none() {
                                measure_error = Some((element, error));
                            }
                            taffy::geometry::Size {
                                width: known.width.unwrap_or(0.0),
                                height: known.height.unwrap_or(0.0),
                            }
                        }
                    }
                },
            )
            .map_err(taffy_error)?;
        if let Some((_, error)) = measure_error {
            // Taffy can reuse a successfully measured sibling on the retry.
            // Preserve this pass's successful records so its snapshot metadata
            // stays aligned with the new Taffy inputs, while forcing every
            // failed provider to run again.
            for element in &failed_measurements {
                pass_measure_records.remove(element);
                self.measure_cache.invalidate(*element);
                self.measure_records.remove(element);
                self.invalidate(*element)?;
            }
            for (element, records) in pass_measure_records {
                self.measure_records.insert(element, records);
            }
            return Err(error);
        }

        // Taffy may ask for several constraint variants before committing a
        // node's final layout. Keep this pass's variants together so snapshot
        // baselines are selected by the final measured content size rather
        // than by callback ordering.
        for (element, records) in pass_measure_records {
            self.measure_records.insert(element, records);
        }

        let nodes = self.collect_snapshot_nodes(root_element, viewport)?;
        let candidate = LayoutSnapshot::computed(LayoutRevision::ZERO, viewport, nodes)?;
        let observable_changed = !candidate.observable_eq(&self.committed);
        let snapshot = self.commit_candidate(candidate)?;
        Ok((
            snapshot,
            LayoutPassReport {
                full_rebuild: sync.full_rebuild,
                safe_fallback: sync.safe_fallback,
                synced_nodes: sync.synced_nodes,
                removed_nodes: sync.removed_nodes,
                observable_changed,
                measure_cache_before,
                measure_cache_after: self.measure_cache.stats(),
            },
        ))
    }

    fn commit_candidate(&mut self, candidate: LayoutSnapshot) -> Result<LayoutSnapshot> {
        let revision = if candidate.observable_eq(&self.committed) {
            self.committed.revision()
        } else {
            self.committed
                .revision()
                .checked_next()
                .map_err(|error| Error::compile("layout_revision", error.to_string()))?
        };
        let committed = candidate.with_revision(revision);
        self.committed = committed.clone();
        Ok(committed)
    }

    fn sync(
        &mut self,
        nodes: Vec<LayoutNodeInput>,
        root: Option<ElementId>,
        force_full: bool,
    ) -> Result<SyncReport> {
        let prepared = prepare_nodes(nodes, root)?;
        let removed_nodes = self
            .inputs
            .keys()
            .filter(|element| !prepared.contains_key(element))
            .copied()
            .collect::<Vec<_>>();
        if force_full || self.root.is_none() || (self.root != root && root.is_some()) {
            self.rebuild_full(prepared, root)?;
            return Ok(SyncReport {
                full_rebuild: true,
                synced_nodes: self.inputs.len(),
                removed_nodes: removed_nodes.len(),
                ..SyncReport::default()
            });
        }

        let incremental = self.sync_incremental(&prepared, root, &removed_nodes);
        match incremental {
            Ok(synced_nodes) => Ok(SyncReport {
                full_rebuild: false,
                safe_fallback: false,
                synced_nodes,
                removed_nodes: removed_nodes.len(),
            }),
            Err(_) => {
                self.rebuild_full(prepared, root)?;
                Ok(SyncReport {
                    full_rebuild: true,
                    safe_fallback: true,
                    synced_nodes: self.inputs.len(),
                    removed_nodes: removed_nodes.len(),
                })
            }
        }
    }

    fn sync_incremental(
        &mut self,
        prepared: &BTreeMap<ElementId, PreparedNode>,
        root: Option<ElementId>,
        removed: &[ElementId],
    ) -> Result<usize> {
        let mut synced = BTreeSet::new();
        for element in removed {
            if let Some(node) = self.taffy_nodes.remove(element) {
                // Taffy 0.13 removes the node itself but leaves entries in its
                // secondary context map. Clear the context before removal so
                // repeated incremental churn cannot retain stale Element IDs.
                self.tree
                    .set_node_context(node, None)
                    .map_err(taffy_error)?;
                self.tree.remove(node).map_err(taffy_error)?;
            }
            self.measure_cache.invalidate(*element);
            self.measure_records.remove(element);
            synced.insert(*element);
        }

        for (element, prepared_node) in prepared {
            if !self.taffy_nodes.contains_key(element) {
                let node = if prepared_node.input.measure.is_some() {
                    self.tree
                        .new_leaf_with_context(prepared_node.style.clone(), *element)
                        .map_err(taffy_error)?
                } else {
                    self.tree
                        .new_leaf(prepared_node.style.clone())
                        .map_err(taffy_error)?
                };
                self.taffy_nodes.insert(*element, node);
                synced.insert(*element);
                continue;
            }
            let old = self.inputs.get(element);
            let node = self.taffy_nodes[element];
            if old.is_none_or(|old| old.style != prepared_node.input.style) {
                self.tree
                    .set_style(node, prepared_node.style.clone())
                    .map_err(taffy_error)?;
                synced.insert(*element);
            }
            if old.is_none_or(|old| old.measure != prepared_node.input.measure) {
                self.tree
                    .set_node_context(node, prepared_node.input.measure.as_ref().map(|_| *element))
                    .map_err(taffy_error)?;
                self.measure_cache.invalidate(*element);
                self.measure_records.remove(element);
                synced.insert(*element);
            }
            if old.is_some_and(|old| {
                old.scroll_offset != prepared_node.input.scroll_offset
                    || old.hit_test != prepared_node.input.hit_test
                    || old.boundaries != prepared_node.input.boundaries
            }) {
                synced.insert(*element);
            }
        }

        for (element, prepared_node) in prepared {
            let old_children = self.inputs.get(element).map(|old| old.children.as_slice());
            if old_children != Some(prepared_node.input.children.as_slice()) {
                let children = prepared_node
                    .input
                    .children
                    .iter()
                    .map(|child| self.taffy_nodes[child])
                    .collect::<Vec<_>>();
                self.tree
                    .set_children(self.taffy_nodes[element], &children)
                    .map_err(taffy_error)?;
                synced.insert(*element);
            }
        }

        self.inputs = prepared
            .iter()
            .map(|(element, prepared)| (*element, prepared.input.clone()))
            .collect();
        preorder(root, &self.inputs)?;
        self.root = root;
        self.measure_cache
            .retain_elements(|element| self.inputs.contains_key(&element));
        Ok(synced.len())
    }

    fn rebuild_full(
        &mut self,
        prepared: BTreeMap<ElementId, PreparedNode>,
        root: Option<ElementId>,
    ) -> Result<()> {
        let mut tree = new_taffy_tree();
        let mut taffy_nodes = BTreeMap::new();
        for (element, prepared_node) in &prepared {
            let node = if prepared_node.input.measure.is_some() {
                tree.new_leaf_with_context(prepared_node.style.clone(), *element)
                    .map_err(taffy_error)?
            } else {
                tree.new_leaf(prepared_node.style.clone())
                    .map_err(taffy_error)?
            };
            taffy_nodes.insert(*element, node);
        }
        for (element, prepared_node) in &prepared {
            let children = prepared_node
                .input
                .children
                .iter()
                .map(|child| taffy_nodes[child])
                .collect::<Vec<_>>();
            tree.set_children(taffy_nodes[element], &children)
                .map_err(taffy_error)?;
        }
        let inputs = prepared
            .into_iter()
            .map(|(element, prepared)| (element, prepared.input))
            .collect::<BTreeMap<_, _>>();
        preorder(root, &inputs)?;
        self.tree = tree;
        self.taffy_nodes = taffy_nodes;
        self.inputs = inputs;
        self.root = root;
        self.measure_cache.clear();
        self.measure_records.clear();
        Ok(())
    }

    fn collect_snapshot_nodes(&self, root: ElementId, viewport: Size) -> Result<Vec<LayoutNode>> {
        let mut nodes = Vec::with_capacity(self.inputs.len());
        let viewport_clip = if viewport.is_empty() {
            ClipState::Empty
        } else {
            ClipState::Rect(Rect::new(Point::ZERO, viewport))
        };
        let mut paint_order = 0_u32;
        self.collect_node(
            root,
            Point::ZERO,
            viewport_clip,
            &mut paint_order,
            &mut nodes,
        )?;
        Ok(nodes)
    }

    fn collect_node(
        &self,
        element: ElementId,
        parent_origin: Point,
        inherited_clip: ClipState,
        paint_order: &mut u32,
        output: &mut Vec<LayoutNode>,
    ) -> Result<()> {
        let input = self.inputs.get(&element).ok_or_else(|| {
            Error::compile(
                "layout_snapshot",
                "layout traversal reached an unknown Element",
            )
        })?;
        let layout = self
            .tree
            .layout(self.taffy_nodes[&element])
            .map_err(taffy_error)?;
        let origin = Point::new(
            parent_origin.x + layout.location.x,
            parent_origin.y + layout.location.y,
        );
        let rect = Rect::from_xywh(origin.x, origin.y, layout.size.width, layout.size.height);
        rect.validate().map_err(Error::from)?;
        let effective_clip = inherited_clip.apply_overflow(rect, input.style.overflow);
        let max_scroll_x = (layout.content_size.width - layout.size.width).max(0.0);
        let max_scroll_y = (layout.content_size.height - layout.size.height).max(0.0);
        let scroll_offset = Point::new(
            if matches!(input.style.overflow.x, super::Overflow::Scroll) {
                input.scroll_offset.x.clamp(0.0, max_scroll_x)
            } else {
                0.0
            },
            if matches!(input.style.overflow.y, super::Overflow::Scroll) {
                input.scroll_offset.y.clamp(0.0, max_scroll_y)
            } else {
                0.0
            },
        );
        let scroll_extent = Size::new(
            layout.content_size.width.max(layout.size.width),
            layout.content_size.height.max(layout.size.height),
        );
        let hit_bounds = if input.hit_test
            && !matches!(input.style.display, super::Display::None)
            && !rect.size.is_empty()
        {
            effective_clip.clip_rect(rect)
        } else {
            None
        };
        let baseline = self.measure_records.get(&element).and_then(|records| {
            let measured_content = Size::new(
                (layout.content_size.width - layout.padding.left - layout.padding.right).max(0.0),
                (layout.content_size.height - layout.padding.top - layout.padding.bottom).max(0.0),
            );
            records
                .iter()
                .rev()
                .find(|record| sizes_close(record.output.size, measured_content))
                .or_else(|| records.last())
                .and_then(|record| record.output.baseline)
        });
        let order = *paint_order;
        *paint_order = paint_order
            .checked_add(1)
            .ok_or_else(|| Error::compile("layout_snapshot", "paint order exhausted"))?;
        output.push(LayoutNode::new(
            element,
            rect,
            baseline,
            effective_clip.as_option(),
            scroll_offset,
            scroll_extent,
            hit_bounds,
            order,
        )?);

        let child_origin = Point::new(origin.x - scroll_offset.x, origin.y - scroll_offset.y);
        for child in &input.children {
            self.collect_node(*child, child_origin, effective_clip, paint_order, output)?;
        }
        Ok(())
    }
}

impl Default for LayoutEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy)]
enum ClipState {
    Rect(Rect),
    Empty,
}

impl ClipState {
    fn apply_overflow(self, node: Rect, overflow: super::OverflowAxes) -> Self {
        let Self::Rect(inherited) = self else {
            return Self::Empty;
        };
        let min_x = if overflow.x.clips() {
            inherited.min_x().max(node.min_x())
        } else {
            inherited.min_x()
        };
        let max_x = if overflow.x.clips() {
            inherited.max_x().min(node.max_x())
        } else {
            inherited.max_x()
        };
        let min_y = if overflow.y.clips() {
            inherited.min_y().max(node.min_y())
        } else {
            inherited.min_y()
        };
        let max_y = if overflow.y.clips() {
            inherited.max_y().min(node.max_y())
        } else {
            inherited.max_y()
        };
        if max_x <= min_x || max_y <= min_y {
            Self::Empty
        } else {
            Self::Rect(Rect::from_xywh(min_x, min_y, max_x - min_x, max_y - min_y))
        }
    }

    fn clip_rect(self, rect: Rect) -> Option<Rect> {
        match self {
            Self::Rect(clip) => rect.intersection(clip),
            Self::Empty => None,
        }
    }

    fn as_option(self) -> Option<Rect> {
        match self {
            Self::Rect(rect) => Some(rect),
            Self::Empty => Some(Rect::ZERO),
        }
    }
}

fn prepare_nodes(
    nodes: Vec<LayoutNodeInput>,
    root: Option<ElementId>,
) -> Result<BTreeMap<ElementId, PreparedNode>> {
    if nodes.is_empty() {
        if root.is_some() {
            return Err(Error::compile(
                "layout_sync",
                "an empty Element tree cannot have a root",
            ));
        }
        return Ok(BTreeMap::new());
    }
    let root = root.ok_or_else(|| Error::compile("layout_sync", "Element tree has no root"))?;
    let mut result = BTreeMap::new();
    for input in nodes {
        input.validate()?;
        let id = input.id;
        let style = input.style.to_taffy()?;
        if result.insert(id, PreparedNode { input, style }).is_some() {
            return Err(Error::compile(
                "layout_sync",
                "Element tree contains a duplicate ID",
            ));
        }
    }
    if !result.contains_key(&root) {
        return Err(Error::compile(
            "layout_sync",
            "Element root is absent from layout inputs",
        ));
    }
    for (element, prepared) in &result {
        if *element == root && prepared.input.parent.is_some() {
            return Err(Error::compile(
                "layout_sync",
                "Element root unexpectedly has a parent",
            ));
        }
        if let Some(parent) = prepared.input.parent {
            let parent_input = result
                .get(&parent)
                .ok_or_else(|| Error::compile("layout_sync", "an Element parent is absent"))?;
            if !parent_input.input.children.contains(element) {
                return Err(Error::compile(
                    "layout_sync",
                    "parent/child layout links disagree",
                ));
            }
        }
        let mut unique = BTreeSet::new();
        for child in &prepared.input.children {
            if !unique.insert(*child) {
                return Err(Error::compile(
                    "layout_sync",
                    "an Element child occurs more than once",
                ));
            }
            let child_input = result
                .get(child)
                .ok_or_else(|| Error::compile("layout_sync", "an Element child is absent"))?;
            if child_input.input.parent != Some(*element) {
                return Err(Error::compile(
                    "layout_sync",
                    "child/parent layout links disagree",
                ));
            }
        }
    }
    let raw_inputs = result
        .iter()
        .map(|(element, prepared)| (*element, prepared.input.clone()))
        .collect();
    let visited = preorder(Some(root), &raw_inputs)?;
    if visited.len() != result.len() {
        return Err(Error::compile(
            "layout_sync",
            "Element layout inputs are disconnected or cyclic",
        ));
    }
    Ok(result)
}

fn preorder(
    root: Option<ElementId>,
    inputs: &BTreeMap<ElementId, LayoutNodeInput>,
) -> Result<Vec<ElementId>> {
    let Some(root) = root else {
        return Ok(Vec::new());
    };
    let mut result = Vec::with_capacity(inputs.len());
    let mut stack = vec![root];
    let mut visited = BTreeSet::new();
    while let Some(element) = stack.pop() {
        if !visited.insert(element) {
            return Err(Error::compile(
                "layout_sync",
                "Element layout inputs contain a cycle",
            ));
        }
        result.push(element);
        let input = inputs
            .get(&element)
            .ok_or_else(|| Error::compile("layout_sync", "layout traversal found a stale ID"))?;
        stack.extend(input.children.iter().rev().copied());
    }
    Ok(result)
}

fn new_taffy_tree() -> taffy::TaffyTree<ElementId> {
    let mut tree = taffy::TaffyTree::new();
    // Logical layout must not be rounded to physical pixels. Raster stages
    // apply DPI and their own pixel snapping later.
    tree.disable_rounding();
    tree
}

fn from_taffy_available(value: taffy::AvailableSpace) -> AvailableDimension {
    match value {
        taffy::AvailableSpace::Definite(value) => AvailableDimension::Definite(value),
        taffy::AvailableSpace::MinContent => AvailableDimension::MinContent,
        taffy::AvailableSpace::MaxContent => AvailableDimension::MaxContent,
    }
}

fn sizes_close(left: Size, right: Size) -> bool {
    fn close(left: f32, right: f32) -> bool {
        let tolerance = 0.001_f32.max(left.abs().max(right.abs()) * 0.00001);
        (left - right).abs() <= tolerance
    }
    close(left.width, right.width) && close(left.height, right.height)
}

fn taffy_error(error: taffy::TaffyError) -> Error {
    Error::compile("taffy", error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::{
        Dimension, GridTrack, LayoutBoundaries, LayoutSize, MeasureInput, Overflow, OverflowAxes,
        compare_layout_snapshots,
    };
    use std::cell::Cell;
    use std::rc::Rc;

    fn id(slot: u32) -> ElementId {
        ElementId::from_parts(slot, 1)
    }

    fn node(
        id: ElementId,
        parent: Option<ElementId>,
        children: Vec<ElementId>,
        style: LayoutStyle,
    ) -> LayoutNodeInput {
        LayoutNodeInput {
            id,
            parent,
            children,
            style,
            measure: None,
            scroll_offset: Point::ZERO,
            hit_test: true,
            boundaries: LayoutBoundaries::NONE,
        }
    }

    fn run(
        engine: &mut LayoutEngine,
        nodes: Vec<LayoutNodeInput>,
        scale: DpiScale,
        force_full: bool,
    ) -> (LayoutSnapshot, LayoutPassReport) {
        let root = nodes
            .iter()
            .find_map(|node| node.parent.is_none().then_some(node.id));
        engine
            .compute(
                nodes,
                root,
                Size::new(400.0, 300.0),
                scale,
                force_full,
                |_, handle, input| handle.measure(input),
            )
            .unwrap()
    }

    #[test]
    fn flex_grid_and_absolute_styles_are_forwarded_to_taffy() {
        let root = id(0);
        let first = id(1);
        let second = id(2);
        let mut root_style =
            LayoutStyle::flex().with_size(Dimension::Length(300.0), Dimension::Length(100.0));
        root_style.flex_direction = crate::layout::FlexDirection::Row;
        let child_style =
            LayoutStyle::default().with_size(Dimension::Length(100.0), Dimension::Length(50.0));
        let mut engine = LayoutEngine::new();
        let (snapshot, _) = run(
            &mut engine,
            vec![
                node(root, None, vec![first, second], root_style),
                node(first, Some(root), Vec::new(), child_style.clone()),
                node(second, Some(root), Vec::new(), child_style),
            ],
            DpiScale::ONE,
            false,
        );
        assert_eq!(snapshot.node(first).unwrap().rect().origin.x, 0.0);
        assert_eq!(snapshot.node(second).unwrap().rect().origin.x, 100.0);

        let grid_root = id(10);
        let grid_a = id(11);
        let grid_b = id(12);
        let grid_style = LayoutStyle::grid()
            .with_size(Dimension::Length(200.0), Dimension::Length(100.0))
            .with_grid_columns([GridTrack::fraction(1.0), GridTrack::fraction(1.0)]);
        let (grid, _) = run(
            &mut LayoutEngine::new(),
            vec![
                node(grid_root, None, vec![grid_a, grid_b], grid_style),
                node(grid_a, Some(grid_root), Vec::new(), LayoutStyle::default()),
                node(grid_b, Some(grid_root), Vec::new(), LayoutStyle::default()),
            ],
            DpiScale::ONE,
            false,
        );
        assert!(grid.node(grid_a).unwrap().rect().size.width > 0.0);
        assert!(grid.node(grid_b).unwrap().rect().origin.x >= 99.0);

        let absolute_root = id(20);
        let absolute = id(21);
        let mut absolute_style =
            LayoutStyle::default().with_size(Dimension::Length(20.0), Dimension::Length(10.0));
        absolute_style.position = crate::layout::Position::Absolute;
        absolute_style.inset.left = crate::layout::LengthPercentageAuto::Length(12.0);
        absolute_style.inset.top = crate::layout::LengthPercentageAuto::Length(7.0);
        let (absolute_snapshot, _) = run(
            &mut LayoutEngine::new(),
            vec![
                node(
                    absolute_root,
                    None,
                    vec![absolute],
                    LayoutStyle::default()
                        .with_size(Dimension::Length(100.0), Dimension::Length(80.0)),
                ),
                node(absolute, Some(absolute_root), Vec::new(), absolute_style),
            ],
            DpiScale::ONE,
            false,
        );
        assert_eq!(
            absolute_snapshot.node(absolute).unwrap().rect().origin,
            Point::new(12.0, 7.0)
        );
    }

    #[test]
    fn nested_sibling_hit_testing_uses_global_tree_order() {
        let root = id(100);
        let first = id(101);
        let first_a = id(102);
        let first_b = id(103);
        let first_c = id(104);
        let second = id(105);
        let size =
            LayoutStyle::default().with_size(Dimension::Length(100.0), Dimension::Length(100.0));
        let mut absolute = size.clone();
        absolute.position = crate::layout::Position::Absolute;
        absolute.inset.left = crate::layout::LengthPercentageAuto::Length(0.0);
        absolute.inset.top = crate::layout::LengthPercentageAuto::Length(0.0);
        let (snapshot, _) = run(
            &mut LayoutEngine::new(),
            vec![
                node(root, None, vec![first, second], size.clone()),
                node(
                    first,
                    Some(root),
                    vec![first_a, first_b, first_c],
                    absolute.clone(),
                ),
                node(first_a, Some(first), Vec::new(), absolute.clone()),
                node(first_b, Some(first), Vec::new(), absolute.clone()),
                node(first_c, Some(first), Vec::new(), absolute.clone()),
                node(second, Some(root), Vec::new(), absolute),
            ],
            DpiScale::ONE,
            false,
        );

        assert_eq!(snapshot.node(first).unwrap().order(), 1);
        assert_eq!(snapshot.node(first_c).unwrap().order(), 4);
        assert_eq!(snapshot.node(second).unwrap().order(), 5);
        assert_eq!(snapshot.hit_test(Point::new(10.0, 10.0)), Some(second));
    }

    #[test]
    fn measure_cache_baseline_and_dpi_are_deterministic() {
        let root = id(0);
        let calls = Rc::new(Cell::new(0));
        let calls_copy = calls.clone();
        let handle = MeasureHandle::text(move |input: MeasureInput| {
            calls_copy.set(calls_copy.get() + 1);
            let width = input.scale.get() as f32 * 10.0;
            Ok(MeasureOutput::new(Size::new(width, 12.0)).with_baseline(9.0))
        });
        let mut input = node(root, None, Vec::new(), LayoutStyle::default());
        input.measure = Some(MeasureSpec::new(handle));
        let mut engine = LayoutEngine::new();
        let (first, first_report) = run(&mut engine, vec![input.clone()], DpiScale::ONE, false);
        engine.invalidate(root).unwrap();
        let (second, second_report) = run(&mut engine, vec![input.clone()], DpiScale::ONE, false);
        assert_eq!(first, second);
        assert_eq!(calls.get(), 1);
        assert!(second_report.measure_cache_after.hits > first_report.measure_cache_after.hits);
        assert_eq!(first.node(root).unwrap().baseline(), Some(9.0));
        let (dpi, _) = run(&mut engine, vec![input], DpiScale::new(2.0).unwrap(), false);
        assert_eq!(dpi.node(root).unwrap().rect().size.width, 20.0);
        assert_eq!(dpi.viewport(), Size::new(400.0, 300.0));
        assert_eq!(calls.get(), 2);
    }

    #[test]
    fn scroll_geometry_is_clamped_and_clipped_in_logical_pixels() {
        let root = id(0);
        let child = id(1);
        let mut root_style =
            LayoutStyle::default().with_size(Dimension::Length(100.0), Dimension::Length(100.0));
        root_style.overflow = OverflowAxes::new(Overflow::Scroll, Overflow::Scroll);
        let (snapshot, _) = run(
            &mut LayoutEngine::new(),
            vec![
                LayoutNodeInput {
                    id: root,
                    parent: None,
                    children: vec![child],
                    style: root_style,
                    measure: None,
                    scroll_offset: Point::new(0.0, 500.0),
                    hit_test: true,
                    boundaries: LayoutBoundaries::NONE,
                },
                node(
                    child,
                    Some(root),
                    Vec::new(),
                    LayoutStyle::default()
                        .with_size(Dimension::Length(100.0), Dimension::Length(300.0)),
                ),
            ],
            DpiScale::ONE,
            false,
        );
        let root_node = snapshot.node(root).unwrap();
        assert_eq!(root_node.scroll_offset().y, 200.0);
        assert_eq!(root_node.scroll_extent().height, 300.0);
        assert_eq!(snapshot.node(child).unwrap().rect().origin.y, -200.0);
        assert!(snapshot.node(child).unwrap().clip().is_some());
        assert!(snapshot.hit_test(Point::new(50.0, 50.0)).is_some());
    }

    #[test]
    fn overflow_axes_clip_only_the_requested_axis() {
        let root = id(30);
        let child = id(31);
        let mut root_style =
            LayoutStyle::default().with_size(Dimension::Length(100.0), Dimension::Length(100.0));
        root_style.overflow = OverflowAxes::new(Overflow::Visible, Overflow::Hidden);
        let mut child_style =
            LayoutStyle::default().with_size(Dimension::Length(200.0), Dimension::Length(200.0));
        child_style.position = crate::layout::Position::Absolute;
        child_style.inset.left = crate::layout::LengthPercentageAuto::Length(0.0);
        child_style.inset.top = crate::layout::LengthPercentageAuto::Length(0.0);
        let (snapshot, _) = run(
            &mut LayoutEngine::new(),
            vec![
                node(root, None, vec![child], root_style),
                node(child, Some(root), Vec::new(), child_style),
            ],
            DpiScale::ONE,
            false,
        );
        assert_eq!(snapshot.hit_test(Point::new(150.0, 50.0)), Some(child));
        assert_eq!(snapshot.hit_test(Point::new(50.0, 150.0)), None);
        assert_eq!(
            snapshot.node(child).unwrap().clip().unwrap().size.width,
            400.0
        );
        assert_eq!(
            snapshot.node(child).unwrap().clip().unwrap().size.height,
            100.0
        );
    }

    #[test]
    fn incremental_and_forced_full_rebuilds_are_equivalent() {
        let root = id(0);
        let child = id(1);
        let initial = vec![
            node(
                root,
                None,
                vec![child],
                LayoutStyle::default()
                    .with_size(Dimension::Length(100.0), Dimension::Length(100.0)),
            ),
            node(
                child,
                Some(root),
                Vec::new(),
                LayoutStyle::default().with_size(Dimension::Length(20.0), Dimension::Length(20.0)),
            ),
        ];
        let mut incremental = LayoutEngine::new();
        let (old, _) = run(&mut incremental, initial.clone(), DpiScale::ONE, false);
        let mut changed = initial;
        changed[1].style.size = LayoutSize::new(Dimension::Length(40.0), Dimension::Length(20.0));
        let (next, report) = run(&mut incremental, changed.clone(), DpiScale::ONE, false);
        assert!(!report.full_rebuild);
        assert_eq!(old.revision(), LayoutRevision::new(1));
        assert_eq!(next.revision(), LayoutRevision::new(2));
        let mut rebuilt = LayoutEngine::new();
        rebuilt.adopt_committed(old.clone());
        let (full, full_report) = run(&mut rebuilt, changed, DpiScale::ONE, true);
        assert!(full_report.full_rebuild);
        compare_layout_snapshots(&next, &full).unwrap();

        // Rebuilding identical observable output is work, but not a revision.
        let (unchanged_full, _) = run(
            &mut rebuilt,
            vec![
                node(
                    root,
                    None,
                    vec![child],
                    LayoutStyle::default()
                        .with_size(Dimension::Length(100.0), Dimension::Length(100.0)),
                ),
                node(
                    child,
                    Some(root),
                    Vec::new(),
                    LayoutStyle::default()
                        .with_size(Dimension::Length(40.0), Dimension::Length(20.0)),
                ),
            ],
            DpiScale::ONE,
            true,
        );
        assert_eq!(unchanged_full.revision(), full.revision());
    }

    #[test]
    fn removing_a_measured_node_releases_taffy_context() {
        let root = id(60);
        let child = id(61);
        let handle = MeasureHandle::custom(|_| Ok(MeasureOutput::new(Size::new(12.0, 8.0))));
        let mut measured_child = node(child, Some(root), Vec::new(), LayoutStyle::default());
        measured_child.measure = Some(MeasureSpec::new(handle));
        let mut engine = LayoutEngine::new();
        run(
            &mut engine,
            vec![
                node(root, None, vec![child], LayoutStyle::default()),
                measured_child,
            ],
            DpiScale::ONE,
            false,
        );
        let taffy_child = engine.taffy_nodes[&child];
        assert!(engine.tree.get_node_context(taffy_child).is_some());

        run(
            &mut engine,
            vec![node(root, None, Vec::new(), LayoutStyle::default())],
            DpiScale::ONE,
            false,
        );
        assert!(engine.tree.get_node_context(taffy_child).is_none());
    }

    #[test]
    fn measured_baseline_stays_equivalent_after_style_changes() {
        let root = id(40);
        let handle = MeasureHandle::text(|_| {
            Ok(MeasureOutput::new(Size::new(20.0, 12.0)).with_baseline(9.0))
        });
        let mut initial = node(root, None, Vec::new(), LayoutStyle::default());
        initial.measure = Some(MeasureSpec::new(handle));
        let mut incremental = LayoutEngine::new();
        let (previous, _) = run(
            &mut incremental,
            vec![initial.clone()],
            DpiScale::ONE,
            false,
        );

        let mut changed = initial;
        changed.style =
            LayoutStyle::default().with_size(Dimension::Length(30.0), Dimension::Length(20.0));
        let (next, _) = run(
            &mut incremental,
            vec![changed.clone()],
            DpiScale::ONE,
            false,
        );
        let mut rebuilt = LayoutEngine::new();
        rebuilt.adopt_committed(previous);
        let (full, _) = run(&mut rebuilt, vec![changed], DpiScale::ONE, true);

        compare_layout_snapshots(&next, &full).unwrap();
    }

    #[test]
    fn failed_sibling_measurement_preserves_successful_records_for_retry() {
        let root = id(80);
        let first = id(81);
        let second = id(82);
        let first_measure = MeasureHandle::custom(|input: MeasureInput| {
            let baseline = if input.content_generation == 0 {
                3.0
            } else {
                9.0
            };
            Ok(MeasureOutput::new(Size::new(40.0, 20.0)).with_baseline(baseline))
        });
        let fail_second = Rc::new(Cell::new(false));
        let fail_second_copy = fail_second.clone();
        let second_measure = MeasureHandle::custom(move |input: MeasureInput| {
            if fail_second_copy.get() && input.content_generation == 1 {
                return Err(Error::compile("measure_test", "expected failure"));
            }
            Ok(MeasureOutput::new(Size::new(40.0, 20.0)).with_baseline(6.0))
        });
        let root_input = node(
            root,
            None,
            vec![first, second],
            LayoutStyle::default().with_size(Dimension::Length(100.0), Dimension::Length(40.0)),
        );
        let mut first_input = node(first, Some(root), Vec::new(), LayoutStyle::default());
        first_input.measure = Some(MeasureSpec::new(first_measure.clone()));
        let mut second_input = node(second, Some(root), Vec::new(), LayoutStyle::default());
        second_input.measure = Some(MeasureSpec::new(second_measure.clone()));
        let initial = vec![root_input, first_input, second_input];

        let mut incremental = LayoutEngine::new();
        let (previous, _) = run(&mut incremental, initial.clone(), DpiScale::ONE, false);
        let mut changed = initial;
        changed[1].measure = Some(MeasureSpec::new(first_measure).with_content_generation(1));
        changed[2].measure = Some(MeasureSpec::new(second_measure).with_content_generation(1));

        fail_second.set(true);
        let error = incremental
            .compute(
                changed.clone(),
                Some(root),
                Size::new(400.0, 300.0),
                DpiScale::ONE,
                false,
                |_, handle, input| handle.measure(input),
            )
            .unwrap_err();
        assert!(error.to_string().contains("expected failure"));

        fail_second.set(false);
        let (retried, _) = run(&mut incremental, changed.clone(), DpiScale::ONE, false);
        let mut rebuilt = LayoutEngine::new();
        rebuilt.adopt_committed(previous);
        let (full, _) = run(&mut rebuilt, changed, DpiScale::ONE, true);

        assert_eq!(retried.node(first).unwrap().baseline(), Some(9.0));
        compare_layout_snapshots(&retried, &full).unwrap();
    }

    #[test]
    fn baseline_uses_the_measurement_matching_final_content_constraints() {
        let root = id(70);
        let child = id(71);
        let handle = MeasureHandle::custom(|input: MeasureInput| {
            let baseline = match input.available_space.width {
                AvailableDimension::MinContent => 3.0,
                AvailableDimension::MaxContent => 7.0,
                AvailableDimension::Definite(width) if width < 50.0 => 11.0,
                AvailableDimension::Definite(_) => 17.0,
            };
            let width = input
                .available_space
                .width
                .definite()
                .unwrap_or(60.0)
                .min(60.0);
            Ok(MeasureOutput::new(Size::new(width, 20.0)).with_baseline(baseline))
        });
        let mut root_style =
            LayoutStyle::default().with_size(Dimension::Length(100.0), Dimension::Length(40.0));
        root_style.flex_direction = crate::layout::FlexDirection::Row;
        let mut child_input = node(child, Some(root), Vec::new(), LayoutStyle::default());
        child_input.measure = Some(MeasureSpec::new(handle));
        let mut engine = LayoutEngine::new();
        let (snapshot, _) = run(
            &mut engine,
            vec![node(root, None, vec![child], root_style), child_input],
            DpiScale::ONE,
            false,
        );
        // The final flex layout asks with a definite width of 60; an older
        // implementation selected whichever callback happened to run last.
        assert_eq!(snapshot.node(child).unwrap().baseline(), Some(17.0));
    }
}
