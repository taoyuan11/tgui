use super::paint::{PaintCommand, validate_commands};
use crate::core::{
    Clip, ElementId, FontHandle, GlyphPageId, LayoutRevision, PropertyId, Rect, RenderNodeId,
    ResourceId, ResourceRevision, Result, SceneRevision, Transform2D,
};
use crate::layout::LayoutSnapshot;
use crate::widget::element::ElementTree;
use crate::widget::{PropertyValue, WidgetType};
use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;
use std::sync::Arc;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ChunkRevisionTuple {
    pub layout: LayoutRevision,
    pub scene: SceneRevision,
    pub resource: ResourceRevision,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ChunkPrerequisites {
    pub renderer_capability: u64,
    pub dpi_scale_bits: u64,
    pub theme_revision: u64,
    pub font_revision: u64,
    pub image_revision: u64,
    pub glyph_revision: u64,
    pub resource_revision: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChunkInvalidationReason {
    Initial,
    Structure,
    Layout,
    Paint,
    Resource,
    Prerequisite,
    FullRebuild,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RenderNodeDescriptor {
    pub element: ElementId,
    pub parent: Option<ElementId>,
    pub bounds: Rect,
    pub transform: Transform2D,
    pub clip: Option<Clip>,
    pub opacity: f32,
    pub z_order: i32,
    pub boundary: bool,
    pub commands: Arc<[PaintCommand]>,
}

impl RenderNodeDescriptor {
    pub fn new(
        element: ElementId,
        bounds: Rect,
        commands: impl IntoIterator<Item = PaintCommand>,
    ) -> Result<Self> {
        bounds.validate().map_err(crate::core::Error::from)?;
        let commands = commands.into_iter().collect::<Vec<_>>();
        validate_commands(&commands)?;
        Ok(Self {
            element,
            parent: None,
            bounds,
            transform: Transform2D::IDENTITY,
            clip: None,
            opacity: 1.0,
            z_order: 0,
            boundary: false,
            commands: commands.into(),
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct RenderNode {
    id: RenderNodeId,
    element: ElementId,
    parent: Option<RenderNodeId>,
    bounds: Rect,
    transform: Transform2D,
    clip: Option<Clip>,
    opacity: f32,
    z_order: i32,
    boundary: bool,
    command_range: Range<usize>,
    chunk_boundary: ElementId,
}

impl RenderNode {
    pub const fn id(&self) -> RenderNodeId {
        self.id
    }
    pub const fn element(&self) -> ElementId {
        self.element
    }
    pub const fn parent(&self) -> Option<RenderNodeId> {
        self.parent
    }
    pub const fn bounds(&self) -> Rect {
        self.bounds
    }
    pub const fn transform(&self) -> Transform2D {
        self.transform
    }
    pub const fn clip(&self) -> Option<Clip> {
        self.clip
    }
    pub const fn opacity(&self) -> f32 {
        self.opacity
    }
    pub const fn z_order(&self) -> i32 {
        self.z_order
    }
    pub const fn is_boundary(&self) -> bool {
        self.boundary
    }
    pub fn command_range(&self) -> Range<usize> {
        self.command_range.clone()
    }
    pub const fn chunk_boundary(&self) -> ElementId {
        self.chunk_boundary
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SceneChunk {
    pub boundary: ElementId,
    pub nodes: Arc<[RenderNodeId]>,
    pub commands: Arc<[PaintCommand]>,
    pub revisions: ChunkRevisionTuple,
    pub prerequisites: ChunkPrerequisites,
    pub invalidation: ChunkInvalidationReason,
    pub fingerprint: u64,
}

impl SceneChunk {
    pub fn command_count(&self) -> usize {
        self.commands.len()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RenderTreeReport {
    pub node_count: usize,
    pub chunk_count: usize,
    pub chunks_rebuilt: usize,
    pub chunks_reused: usize,
    pub full_rebuild: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SceneSnapshot {
    revision: SceneRevision,
    command_count: usize,
    chunk_count: usize,
    fingerprint: u64,
}

impl SceneSnapshot {
    pub const fn new(revision: SceneRevision, command_count: usize, fingerprint: u64) -> Self {
        Self {
            revision,
            command_count,
            chunk_count: 0,
            fingerprint,
        }
    }

    pub const fn empty(revision: SceneRevision) -> Self {
        Self::new(revision, 0, 0)
    }
    pub const fn revision(&self) -> SceneRevision {
        self.revision
    }
    pub const fn command_count(&self) -> usize {
        self.command_count
    }
    pub const fn chunk_count(&self) -> usize {
        self.chunk_count
    }
    pub const fn fingerprint(&self) -> u64 {
        self.fingerprint
    }

    pub(crate) fn with_revision(mut self, revision: SceneRevision) -> Self {
        self.revision = revision;
        self
    }

    pub(crate) fn observable_eq(&self, other: &Self) -> bool {
        self.command_count == other.command_count
            && self.chunk_count == other.chunk_count
            && self.fingerprint == other.fingerprint
    }

    fn from_tree(tree: &RenderTree, revision: SceneRevision) -> Self {
        Self {
            revision,
            command_count: tree.commands.len(),
            chunk_count: tree.chunks.len(),
            fingerprint: tree.fingerprint,
        }
    }
}

#[derive(Clone, Debug)]
pub struct RenderTree {
    nodes: crate::core::DenseArena<RenderNode, RenderNodeId>,
    by_element: BTreeMap<ElementId, RenderNodeId>,
    chunks: Vec<SceneChunk>,
    commands: Vec<PaintCommand>,
    root: Option<RenderNodeId>,
    fingerprint: u64,
    revision: SceneRevision,
}

impl Default for RenderTree {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderTree {
    pub fn new() -> Self {
        Self {
            nodes: crate::core::DenseArena::new(),
            by_element: BTreeMap::new(),
            chunks: Vec::new(),
            commands: Vec::new(),
            root: None,
            fingerprint: 0,
            revision: SceneRevision::ZERO,
        }
    }

    pub fn revision(&self) -> SceneRevision {
        self.revision
    }
    pub fn root(&self) -> Option<RenderNodeId> {
        self.root
    }
    pub fn len(&self) -> usize {
        self.nodes.len()
    }
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }
    pub fn node(&self, id: RenderNodeId) -> Option<&RenderNode> {
        self.nodes.get(id)
    }
    pub fn node_for_element(&self, element: ElementId) -> Option<&RenderNode> {
        self.by_element.get(&element).and_then(|id| self.node(*id))
    }
    pub fn nodes(&self) -> impl Iterator<Item = (RenderNodeId, &RenderNode)> {
        self.nodes.iter()
    }
    pub fn chunks(&self) -> &[SceneChunk] {
        &self.chunks
    }
    pub fn commands(&self) -> &[PaintCommand] {
        &self.commands
    }
    pub fn snapshot(&self) -> SceneSnapshot {
        SceneSnapshot::from_tree(self, self.revision)
    }

    /// Collects the retained scene from the immutable Element/Layout pair.
    /// This is deliberately a read-only operation over both inputs; a failed
    /// collection leaves the previous RenderTree untouched.
    pub(crate) fn collect_elements(
        &mut self,
        elements: &ElementTree,
        layout: &LayoutSnapshot,
        revisions: ChunkRevisionTuple,
        prerequisites: ChunkPrerequisites,
        invalidations: &BTreeMap<ElementId, ChunkInvalidationReason>,
        force_full: bool,
    ) -> Result<(SceneSnapshot, RenderTreeReport)> {
        self.collect_elements_with_presentation(
            elements,
            layout,
            revisions,
            prerequisites,
            invalidations,
            force_full,
            |_, _| None,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn collect_elements_with_presentation(
        &mut self,
        elements: &ElementTree,
        layout: &LayoutSnapshot,
        revisions: ChunkRevisionTuple,
        prerequisites: ChunkPrerequisites,
        invalidations: &BTreeMap<ElementId, ChunkInvalidationReason>,
        force_full: bool,
        presentation: impl Fn(ElementId, PropertyId) -> Option<f32>,
    ) -> Result<(SceneSnapshot, RenderTreeReport)> {
        let mut descriptors = Vec::with_capacity(layout.nodes().len());
        for node in layout.nodes() {
            if !elements.contains(node.element()) {
                return Err(crate::core::Error::compile(
                    "render_collect",
                    "layout contains an element generation that is no longer mounted",
                ));
            }
            let element = node.element();
            let widget_type = elements.widget_type(element).ok_or_else(|| {
                crate::core::Error::compile("render_collect", "element type is stale")
            })?;
            let commands =
                paint_for_element(elements, element, widget_type, node.rect(), &presentation)?;
            let mut descriptor = RenderNodeDescriptor::new(element, node.rect(), commands)?;
            descriptor.opacity = presentation(element, crate::widget::OPACITY)
                .or_else(
                    || match elements.property(element, crate::widget::OPACITY) {
                        Some(PropertyValue::F32(value)) => Some(*value),
                        _ => None,
                    },
                )
                .unwrap_or(1.0);
            descriptor.parent = elements.parent(element);
            descriptor.clip = node.clip().map(Clip::Rect);
            descriptor.z_order = match elements.property(element, crate::native::HOST_Z_ORDER) {
                Some(PropertyValue::I64(value)) => {
                    i32::try_from(*value).unwrap_or(if *value < 0 { i32::MIN } else { i32::MAX })
                }
                _ => i32::try_from(node.order()).unwrap_or(i32::MAX),
            };
            descriptor.boundary = elements
                .layout_boundaries(element)
                .is_some_and(|boundaries| boundaries.render);
            descriptors.push(descriptor);
        }
        // Native hosts and custom retained nodes may override layout order.
        // Stable sorting preserves document order for equal z values.
        descriptors.sort_by_key(|descriptor| descriptor.z_order);
        self.collect_with_invalidations(
            &descriptors,
            revisions,
            prerequisites,
            invalidations,
            force_full,
        )
    }

    /// Atomically replaces the retained tree after validating topology,
    /// command stacks, and all chunk prerequisites.
    pub fn collect(
        &mut self,
        descriptors: &[RenderNodeDescriptor],
        revisions: ChunkRevisionTuple,
        prerequisites: ChunkPrerequisites,
        dirty_elements: &BTreeSet<ElementId>,
        force_full: bool,
    ) -> Result<(SceneSnapshot, RenderTreeReport)> {
        let invalidations = dirty_elements
            .iter()
            .copied()
            .map(|element| (element, ChunkInvalidationReason::Paint))
            .collect::<BTreeMap<_, _>>();
        self.collect_with_invalidations(
            descriptors,
            revisions,
            prerequisites,
            &invalidations,
            force_full,
        )
    }

    pub fn collect_with_invalidations(
        &mut self,
        descriptors: &[RenderNodeDescriptor],
        revisions: ChunkRevisionTuple,
        prerequisites: ChunkPrerequisites,
        invalidations: &BTreeMap<ElementId, ChunkInvalidationReason>,
        force_full: bool,
    ) -> Result<(SceneSnapshot, RenderTreeReport)> {
        let mut candidate = self.clone();
        let report = candidate.collect_in_place(
            descriptors,
            revisions,
            prerequisites,
            invalidations,
            force_full,
        )?;
        *self = candidate;
        Ok((self.snapshot(), report))
    }

    fn collect_in_place(
        &mut self,
        descriptors: &[RenderNodeDescriptor],
        revisions: ChunkRevisionTuple,
        prerequisites: ChunkPrerequisites,
        invalidations: &BTreeMap<ElementId, ChunkInvalidationReason>,
        force_full: bool,
    ) -> Result<RenderTreeReport> {
        validate_descriptors(descriptors)?;
        let descriptor_ids = descriptors
            .iter()
            .map(|descriptor| descriptor.element)
            .collect::<BTreeSet<_>>();
        self.by_element
            .retain(|element, _| descriptor_ids.contains(element));
        self.nodes
            .retain(|_, node| descriptor_ids.contains(&node.element));
        for descriptor in descriptors {
            descriptor
                .bounds
                .validate()
                .map_err(crate::core::Error::from)?;
            descriptor
                .transform
                .validate()
                .map_err(crate::core::Error::from)?;
            if let Some(clip) = descriptor.clip {
                clip.validate().map_err(crate::core::Error::from)?;
            }
            if !descriptor.opacity.is_finite() || !(0.0..=1.0).contains(&descriptor.opacity) {
                return Err(crate::core::Error::compile(
                    "render_tree",
                    "node opacity is invalid",
                ));
            }
            validate_commands(&descriptor.commands)?;
            let id = if let Some(id) = self.by_element.get(&descriptor.element).copied() {
                id
            } else {
                let id = self.nodes.insert(RenderNode {
                    id: RenderNodeId::from_parts(0, 1),
                    element: descriptor.element,
                    parent: None,
                    bounds: descriptor.bounds,
                    transform: descriptor.transform,
                    clip: descriptor.clip,
                    opacity: descriptor.opacity,
                    z_order: descriptor.z_order,
                    boundary: descriptor.boundary,
                    command_range: 0..0,
                    chunk_boundary: descriptor.element,
                });
                if let Some(node) = self.nodes.get_mut(id) {
                    node.id = id;
                }
                self.by_element.insert(descriptor.element, id);
                id
            };
            let parent = descriptor
                .parent
                .and_then(|element| self.by_element.get(&element).copied());
            let node = self
                .nodes
                .get_mut(id)
                .ok_or_else(|| crate::core::Error::compile("render_tree", "node became stale"))?;
            node.parent = parent;
            node.bounds = descriptor.bounds;
            node.transform = descriptor.transform;
            node.clip = descriptor.clip;
            node.opacity = descriptor.opacity;
            node.z_order = descriptor.z_order;
            node.boundary = descriptor.boundary;
        }
        for descriptor in descriptors {
            let id = self.by_element[&descriptor.element];
            let parent = descriptor
                .parent
                .and_then(|element| self.by_element.get(&element).copied());
            if let Some(node) = self.nodes.get_mut(id) {
                node.parent = parent;
            }
        }

        let mut boundaries = BTreeMap::<ElementId, Vec<RenderNodeId>>::new();
        for descriptor in descriptors {
            let mut boundary = descriptor.element;
            if !descriptor.boundary {
                let mut current = descriptor.parent;
                while let Some(parent) = current {
                    let Some(parent_descriptor) = descriptors
                        .iter()
                        .find(|candidate| candidate.element == parent)
                    else {
                        break;
                    };
                    if parent_descriptor.boundary || parent_descriptor.parent.is_none() {
                        boundary = parent;
                        break;
                    }
                    current = parent_descriptor.parent;
                }
            }
            let id = self.by_element[&descriptor.element];
            if let Some(node) = self.nodes.get_mut(id) {
                node.chunk_boundary = boundary;
            }
            boundaries.entry(boundary).or_default().push(id);
        }

        let old_chunks = self
            .chunks
            .iter()
            .map(|chunk| (chunk.boundary, chunk.clone()))
            .collect::<BTreeMap<_, _>>();
        self.commands.clear();
        self.chunks.clear();
        let mut rebuilt = 0;
        let mut reused = 0;
        for (boundary, ids) in boundaries {
            let mut commands = Vec::new();
            for id in &ids {
                let element = self.nodes[*id].element;
                let descriptor = descriptors
                    .iter()
                    .find(|candidate| candidate.element == element)
                    .ok_or_else(|| {
                        crate::core::Error::compile("render_tree", "descriptor disappeared")
                    })?;
                let start = self.commands.len() + commands.len();
                commands.extend(descriptor.commands.iter().cloned());
                let end = start + descriptor.commands.len();
                if let Some(node) = self.nodes.get_mut(*id) {
                    node.command_range = start..end;
                }
            }
            validate_commands(&commands)?;
            let fingerprint = commands_fingerprint(&commands);
            let old = old_chunks.get(&boundary);
            let dirty = force_full
                || ids
                    .iter()
                    .any(|id| invalidations.contains_key(&self.nodes[*id].element));
            let (chunk_revision, invalidation, is_reused) = if !dirty
                && old.is_some_and(|chunk| {
                    chunk.fingerprint == fingerprint && chunk.prerequisites == prerequisites
                }) {
                (
                    old.expect("checked").revisions,
                    old.expect("checked").invalidation,
                    true,
                )
            } else {
                let reason = if force_full {
                    ChunkInvalidationReason::FullRebuild
                } else if old.is_none() {
                    ChunkInvalidationReason::Initial
                } else {
                    ids.iter()
                        .filter_map(|id| invalidations.get(&self.nodes[*id].element))
                        .copied()
                        .max_by_key(|reason| invalidation_priority(*reason))
                        .unwrap_or(ChunkInvalidationReason::Prerequisite)
                };
                (revisions, reason, false)
            };
            if is_reused {
                reused += 1;
            } else {
                rebuilt += 1;
            }
            self.commands.extend(commands.iter().cloned());
            self.chunks.push(SceneChunk {
                boundary,
                nodes: ids.into(),
                commands: commands.into(),
                revisions: chunk_revision,
                prerequisites,
                invalidation,
                fingerprint,
            });
        }
        self.root = descriptors
            .iter()
            .find(|descriptor| descriptor.parent.is_none())
            .and_then(|descriptor| self.by_element.get(&descriptor.element).copied());
        self.fingerprint = commands_fingerprint(&self.commands);
        self.revision = revisions.scene;
        Ok(RenderTreeReport {
            node_count: self.nodes.len(),
            chunk_count: self.chunks.len(),
            chunks_rebuilt: rebuilt,
            chunks_reused: reused,
            full_rebuild: force_full,
        })
    }
}

fn invalidation_priority(reason: ChunkInvalidationReason) -> u8 {
    match reason {
        ChunkInvalidationReason::Initial => 0,
        ChunkInvalidationReason::Paint => 1,
        ChunkInvalidationReason::Resource => 2,
        ChunkInvalidationReason::Layout => 3,
        ChunkInvalidationReason::Structure => 4,
        ChunkInvalidationReason::Prerequisite => 5,
        ChunkInvalidationReason::FullRebuild => 6,
    }
}

fn commands_fingerprint(commands: &[PaintCommand]) -> u64 {
    let mut hash = 0xcbf29ce484222325_u64;
    for command in commands {
        for byte in command.stable_debug().bytes() {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }
    }
    hash
}

fn validate_descriptors(descriptors: &[RenderNodeDescriptor]) -> Result<()> {
    if descriptors.is_empty() {
        return Ok(());
    }
    let ids = descriptors
        .iter()
        .map(|descriptor| descriptor.element)
        .collect::<BTreeSet<_>>();
    if ids.len() != descriptors.len() {
        return Err(crate::core::Error::compile(
            "render_tree",
            "duplicate ElementId in render descriptors",
        ));
    }
    if descriptors
        .iter()
        .filter(|descriptor| descriptor.parent.is_none())
        .count()
        != 1
    {
        return Err(crate::core::Error::compile(
            "render_tree",
            "render descriptors must contain exactly one root",
        ));
    }
    for descriptor in descriptors {
        if descriptor
            .parent
            .is_some_and(|parent| !ids.contains(&parent))
        {
            return Err(crate::core::Error::compile(
                "render_tree",
                "render descriptor parent is stale or absent",
            ));
        }
        let mut current = descriptor.parent;
        let mut seen = BTreeSet::from([descriptor.element]);
        while let Some(parent) = current {
            if !seen.insert(parent) {
                return Err(crate::core::Error::compile(
                    "render_tree",
                    "render descriptor topology contains a cycle",
                ));
            }
            current = descriptors
                .iter()
                .find(|candidate| candidate.element == parent)
                .and_then(|candidate| candidate.parent);
        }
    }
    Ok(())
}

fn paint_for_element(
    elements: &ElementTree,
    element: ElementId,
    widget_type: &WidgetType,
    bounds: Rect,
    presentation: &impl Fn(ElementId, PropertyId) -> Option<f32>,
) -> Result<Vec<PaintCommand>> {
    if *widget_type == WidgetType::of::<crate::widgets::Text>() {
        let content = match elements.property(element, crate::widgets::TEXT_CONTENT) {
            Some(PropertyValue::Text(content)) => content.clone(),
            _ => "".into(),
        };
        let content_revision = content.bytes().fold(0xcbf29ce484222325_u64, |hash, byte| {
            (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3)
        });
        let opacity = effective_opacity(elements, element, presentation);
        let run = super::paint::TextRun {
            layout: ResourceId::from_parts(element.slot(), element.generation()),
            font: FontHandle::from_parts(0, 1),
            glyph_page: Some(GlyphPageId::from_parts(0, 1)),
            bounds,
            color: crate::core::Color::rgba8(0, 0, 0, (opacity * 255.0).round() as u8),
            glyph_count: u32::try_from(content.chars().count()).unwrap_or(u32::MAX),
            content_revision,
        };
        return Ok(vec![PaintCommand::DrawTextRun(run)]);
    }
    if *widget_type == WidgetType::of::<crate::widgets::Button>() {
        let enabled = matches!(
            elements.property(element, crate::widgets::BUTTON_ENABLED),
            Some(PropertyValue::Bool(true))
        );
        let color = if enabled {
            crate::core::Color::rgb8(30, 104, 180)
        } else {
            crate::core::Color::rgb8(140, 140, 140)
        };
        let opacity = effective_opacity(elements, element, presentation);
        return Ok(vec![PaintCommand::DrawRoundedRect {
            rect: bounds,
            radii: crate::core::CornerRadii::all(4.0),
            paint: super::paint::Paint::solid(color).with_opacity(opacity),
        }]);
    }
    if *widget_type == WidgetType::of::<crate::widgets::Image>() {
        let slot = match elements.property(element, crate::widgets::IMAGE_RESOURCE_SLOT) {
            Some(PropertyValue::U64(value)) => u32::try_from(*value).ok(),
            _ => None,
        };
        let generation = match elements.property(element, crate::widgets::IMAGE_RESOURCE_GENERATION)
        {
            Some(PropertyValue::U64(value)) => u32::try_from(*value).ok(),
            _ => None,
        };
        let (Some(slot), Some(generation)) = (slot, generation) else {
            return Ok(Vec::new());
        };
        if generation == 0 {
            return Err(crate::core::Error::compile(
                "image_collect",
                "Image contains a malformed resource generation",
            ));
        }
        return Ok(vec![PaintCommand::DrawImage {
            rect: bounds,
            image: crate::core::ImageHandle::from_parts(slot, generation),
            sampling: super::paint::ImageSampling::Linear,
            opacity: effective_opacity(elements, element, presentation),
        }]);
    }
    if *widget_type == WidgetType::of::<crate::native::NativeHostWidget>() {
        let slot = match elements.property(element, crate::native::HOST_SURFACE_SLOT) {
            Some(PropertyValue::U64(value)) => u32::try_from(*value).ok(),
            _ => None,
        };
        let generation = match elements.property(element, crate::native::HOST_SURFACE_GENERATION) {
            Some(PropertyValue::U64(value)) => u32::try_from(*value).ok(),
            _ => None,
        };
        let (Some(slot), Some(generation)) = (slot, generation) else {
            return Ok(Vec::new());
        };
        if generation == 0 {
            return Err(crate::core::Error::compile(
                "native_host_collect",
                "NativeHostWidget contains a malformed surface generation",
            ));
        }
        let surface = ResourceId::from_parts(slot, generation);
        let offscreen = matches!(
            elements.property(element, crate::native::HOST_OFFSCREEN),
            Some(PropertyValue::Bool(true))
        );
        let opaque = matches!(
            elements.property(element, crate::native::HOST_OPAQUE),
            Some(PropertyValue::Bool(true))
        );
        return Ok(vec![if offscreen {
            PaintCommand::DrawImage {
                rect: bounds,
                image: crate::core::ImageHandle::from_parts(slot, generation),
                sampling: super::paint::ImageSampling::Linear,
                opacity: effective_opacity(elements, element, presentation),
            }
        } else {
            PaintCommand::NativeSurface {
                rect: bounds,
                surface,
                opaque,
            }
        }]);
    }
    if *widget_type == WidgetType::of::<crate::widgets::Container>() {
        return Ok(Vec::new());
    }
    Ok(vec![PaintCommand::Marker(Arc::from(widget_type.name()))])
}

fn effective_opacity(
    elements: &ElementTree,
    element: ElementId,
    presentation: &impl Fn(ElementId, PropertyId) -> Option<f32>,
) -> f32 {
    presentation(element, crate::widget::OPACITY)
        .or_else(
            || match elements.property(element, crate::widget::OPACITY) {
                Some(PropertyValue::F32(value)) => Some(*value),
                _ => None,
            },
        )
        .unwrap_or(1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Color, Rect};

    fn descriptor(slot: u32, color: Color) -> RenderNodeDescriptor {
        RenderNodeDescriptor::new(
            ElementId::from_parts(slot, 1),
            Rect::from_xywh(0.0, 0.0, 10.0, 10.0),
            [PaintCommand::FillRect {
                rect: Rect::from_xywh(0.0, 0.0, 10.0, 10.0),
                color,
            }],
        )
        .unwrap()
    }

    #[test]
    fn render_nodes_retain_ids_and_rebuild_only_dirty_chunks() {
        let mut tree = RenderTree::new();
        let revisions = ChunkRevisionTuple {
            layout: LayoutRevision::new(1),
            scene: SceneRevision::new(1),
            resource: ResourceRevision::ZERO,
        };
        let prerequisites = ChunkPrerequisites::default();
        let mut root = descriptor(0, crate::core::Color::WHITE);
        root.boundary = true;
        let mut child = descriptor(1, crate::core::Color::BLACK);
        child.parent = Some(root.element);
        child.boundary = true;
        let descriptors = [root, child];
        let (_, first) = tree
            .collect(
                &descriptors,
                revisions,
                prerequisites,
                &BTreeSet::new(),
                false,
            )
            .unwrap();
        assert_eq!(first.chunks_rebuilt, 2);
        let first_id = tree
            .node_for_element(ElementId::from_parts(0, 1))
            .unwrap()
            .id();
        let mut dirty = BTreeSet::new();
        dirty.insert(ElementId::from_parts(1, 1));
        let mut next = descriptors.clone();
        next[1].commands = descriptor(1, crate::core::Color::WHITE).commands;
        let revisions = ChunkRevisionTuple {
            scene: SceneRevision::new(2),
            ..revisions
        };
        let (_, second) = tree
            .collect(&next, revisions, prerequisites, &dirty, false)
            .unwrap();
        assert_eq!(second.chunks_reused, 1);
        assert_eq!(
            tree.node_for_element(ElementId::from_parts(0, 1))
                .unwrap()
                .id(),
            first_id
        );
    }
}
