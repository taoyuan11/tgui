use crate::core::{ElementId, Error, LayoutRevision, Point, Rect, Result, Size};

/// Immutable per-Element logical layout output.
#[derive(Clone, Debug, PartialEq)]
pub struct LayoutNode {
    element: ElementId,
    rect: Rect,
    baseline: Option<f32>,
    clip: Option<Rect>,
    scroll_offset: Point,
    scroll_extent: Size,
    hit_bounds: Option<Rect>,
    /// Global preorder paint/hit order within the committed element tree.
    /// This is intentionally independent of Taffy's parent-local `order`.
    order: u32,
}

impl LayoutNode {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        element: ElementId,
        rect: Rect,
        baseline: Option<f32>,
        clip: Option<Rect>,
        scroll_offset: Point,
        scroll_extent: Size,
        hit_bounds: Option<Rect>,
        order: u32,
    ) -> Result<Self> {
        rect.validate().map_err(Error::from)?;
        if let Some(clip) = clip {
            clip.validate().map_err(Error::from)?;
        }
        scroll_offset.validate().map_err(Error::from)?;
        scroll_extent.validate().map_err(Error::from)?;
        if let Some(hit_bounds) = hit_bounds {
            hit_bounds.validate().map_err(Error::from)?;
        }
        if let Some(baseline) = baseline {
            if !baseline.is_finite() || baseline < 0.0 {
                return Err(Error::compile(
                    "layout_snapshot",
                    "a node baseline is non-finite or negative",
                ));
            }
        }
        Ok(Self {
            element,
            rect,
            baseline,
            clip,
            scroll_offset,
            scroll_extent,
            hit_bounds,
            order,
        })
    }

    pub const fn element(&self) -> ElementId {
        self.element
    }

    pub const fn rect(&self) -> Rect {
        self.rect
    }

    pub const fn baseline(&self) -> Option<f32> {
        self.baseline
    }

    pub const fn clip(&self) -> Option<Rect> {
        self.clip
    }

    pub const fn scroll_offset(&self) -> Point {
        self.scroll_offset
    }

    pub const fn scroll_extent(&self) -> Size {
        self.scroll_extent
    }

    pub const fn hit_bounds(&self) -> Option<Rect> {
        self.hit_bounds
    }

    pub const fn order(&self) -> u32 {
        self.order
    }
}

/// Observable logical layout committed for one window.
#[derive(Clone, Debug, PartialEq)]
pub struct LayoutSnapshot {
    revision: LayoutRevision,
    viewport: Size,
    node_count: usize,
    fingerprint: u64,
    nodes: Vec<LayoutNode>,
}

impl LayoutSnapshot {
    /// Compatibility constructor for externally supplied snapshot headers.
    /// Engine-produced snapshots additionally populate [`Self::nodes`].
    pub fn new(
        revision: LayoutRevision,
        viewport: Size,
        node_count: usize,
        fingerprint: u64,
    ) -> Result<Self> {
        viewport.validate().map_err(Error::from)?;
        Ok(Self {
            revision,
            viewport,
            node_count,
            fingerprint,
            nodes: Vec::new(),
        })
    }

    pub const fn empty(revision: LayoutRevision) -> Self {
        Self {
            revision,
            viewport: Size::ZERO,
            node_count: 0,
            fingerprint: 0,
            nodes: Vec::new(),
        }
    }

    pub const fn revision(&self) -> LayoutRevision {
        self.revision
    }

    pub const fn viewport(&self) -> Size {
        self.viewport
    }

    pub const fn node_count(&self) -> usize {
        self.node_count
    }

    pub const fn fingerprint(&self) -> u64 {
        self.fingerprint
    }

    pub fn nodes(&self) -> &[LayoutNode] {
        &self.nodes
    }

    pub fn node(&self, element: ElementId) -> Option<&LayoutNode> {
        self.nodes.iter().find(|node| node.element == element)
    }

    /// Returns the topmost hit-enabled Element using the immutable committed
    /// clip, scroll geometry, and global tree paint order.
    pub fn hit_test(&self, point: Point) -> Option<ElementId> {
        self.nodes
            .iter()
            .enumerate()
            .filter(|(_, node)| node.hit_bounds.is_some_and(|bounds| bounds.contains(point)))
            .max_by_key(|(index, node)| (node.order, *index))
            .map(|(_, node)| node.element)
    }

    pub(crate) fn computed(
        revision: LayoutRevision,
        viewport: Size,
        nodes: Vec<LayoutNode>,
    ) -> Result<Self> {
        viewport.validate().map_err(Error::from)?;
        let fingerprint = fingerprint(viewport, &nodes);
        Ok(Self {
            revision,
            viewport,
            node_count: nodes.len(),
            fingerprint,
            nodes,
        })
    }

    pub(crate) fn with_revision(mut self, revision: LayoutRevision) -> Self {
        self.revision = revision;
        self
    }

    pub(crate) fn observable_eq(&self, other: &Self) -> bool {
        self.viewport == other.viewport
            && self.node_count == other.node_count
            && self.fingerprint == other.fingerprint
            && self.nodes == other.nodes
    }
}

impl Default for LayoutSnapshot {
    fn default() -> Self {
        Self::empty(LayoutRevision::ZERO)
    }
}

/// Headless equivalence check used to validate incremental work against a
/// forced full Taffy rebuild. Revision is deliberately included.
pub fn compare_layout_snapshots(
    incremental: &LayoutSnapshot,
    rebuilt: &LayoutSnapshot,
) -> Result<()> {
    if incremental.revision != rebuilt.revision {
        return Err(Error::compile(
            "layout_equivalence",
            format!(
                "revision differs: incremental {:?}, rebuilt {:?}",
                incremental.revision, rebuilt.revision
            ),
        ));
    }
    if incremental.viewport != rebuilt.viewport {
        return Err(Error::compile(
            "layout_equivalence",
            "viewport differs between incremental and rebuilt snapshots",
        ));
    }
    if incremental.node_count != rebuilt.node_count {
        return Err(Error::compile(
            "layout_equivalence",
            "node count differs between incremental and rebuilt snapshots",
        ));
    }
    if incremental.nodes != rebuilt.nodes {
        return Err(Error::compile(
            "layout_equivalence",
            "geometry, baseline, clip, scroll, or hit output differs",
        ));
    }
    if incremental.fingerprint != rebuilt.fingerprint {
        return Err(Error::compile(
            "layout_equivalence",
            "observable fingerprints differ",
        ));
    }
    Ok(())
}

fn fingerprint(viewport: Size, nodes: &[LayoutNode]) -> u64 {
    let mut hash = 0xcbf29ce484222325_u64;
    push_float(&mut hash, viewport.width);
    push_float(&mut hash, viewport.height);
    push_u64(&mut hash, nodes.len() as u64);
    for node in nodes {
        push_u64(&mut hash, u64::from(node.element.slot()));
        push_u64(&mut hash, u64::from(node.element.generation()));
        push_rect(&mut hash, node.rect);
        push_option_float(&mut hash, node.baseline);
        match node.clip {
            Some(clip) => {
                push_u64(&mut hash, 1);
                push_rect(&mut hash, clip);
            }
            None => push_u64(&mut hash, 0),
        }
        push_float(&mut hash, node.scroll_offset.x);
        push_float(&mut hash, node.scroll_offset.y);
        push_float(&mut hash, node.scroll_extent.width);
        push_float(&mut hash, node.scroll_extent.height);
        match node.hit_bounds {
            Some(bounds) => {
                push_u64(&mut hash, 1);
                push_rect(&mut hash, bounds);
            }
            None => push_u64(&mut hash, 0),
        }
        push_u64(&mut hash, u64::from(node.order));
    }
    hash
}

fn push_rect(hash: &mut u64, rect: Rect) {
    push_float(hash, rect.origin.x);
    push_float(hash, rect.origin.y);
    push_float(hash, rect.size.width);
    push_float(hash, rect.size.height);
}

fn push_option_float(hash: &mut u64, value: Option<f32>) {
    match value {
        Some(value) => {
            push_u64(hash, 1);
            push_float(hash, value);
        }
        None => push_u64(hash, 0),
    }
}

fn push_float(hash: &mut u64, value: f32) {
    let value = if value == 0.0 { 0.0 } else { value };
    push_u64(hash, u64::from(value.to_bits()));
}

fn push_u64(hash: &mut u64, value: u64) {
    for byte in value.to_le_bytes() {
        *hash ^= u64::from(byte);
        *hash = hash.wrapping_mul(0x100000001b3);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hit_test_uses_clip_and_topmost_order() {
        let low = ElementId::from_parts(0, 1);
        let high = ElementId::from_parts(1, 1);
        let nodes = vec![
            LayoutNode::new(
                low,
                Rect::from_xywh(0.0, 0.0, 20.0, 20.0),
                None,
                None,
                Point::ZERO,
                Size::new(20.0, 20.0),
                Some(Rect::from_xywh(0.0, 0.0, 20.0, 20.0)),
                0,
            )
            .unwrap(),
            LayoutNode::new(
                high,
                Rect::from_xywh(5.0, 5.0, 20.0, 20.0),
                None,
                Some(Rect::from_xywh(5.0, 5.0, 5.0, 5.0)),
                Point::ZERO,
                Size::new(20.0, 20.0),
                Some(Rect::from_xywh(5.0, 5.0, 5.0, 5.0)),
                1,
            )
            .unwrap(),
        ];
        let snapshot =
            LayoutSnapshot::computed(LayoutRevision::new(1), Size::new(30.0, 30.0), nodes).unwrap();
        assert_eq!(snapshot.hit_test(Point::new(7.0, 7.0)), Some(high));
        assert_eq!(snapshot.hit_test(Point::new(15.0, 15.0)), Some(low));
    }
}
