use super::*;
use crate::runtime::overlay::PortalEntry;
use crate::runtime::overlay::{AnchorKey, AnchorSource};
use crate::runtime::portal::ExternalPortalRequest;
use crate::ui::widget::VirtualSceneStateUpdate;
use smallvec::SmallVec;
use std::ops::Range;

#[derive(Clone, Copy, Debug)]
pub(crate) struct ScrollRegion {
    pub id: WidgetId,
    pub content_viewport: Rect,
    pub visible_frame: Rect,
    pub content_bounds: Rect,
    pub gpu_base_scroll_offset: Point,
    pub scroll_offset: Point,
    pub overflow_x: Overflow,
    pub overflow_y: Overflow,
    pub horizontal_track: Option<Rect>,
    pub horizontal_thumb: Option<Rect>,
    pub vertical_track: Option<Rect>,
    pub vertical_thumb: Option<Rect>,
}

impl ScrollRegion {
    pub(crate) fn max_offset(self) -> Point {
        Point {
            x: (self.content_bounds.right() - self.content_viewport.right()).max(0.0),
            y: (self.content_bounds.bottom() - self.content_viewport.bottom()).max(0.0),
        }
    }

    pub(crate) fn can_scroll_x(self) -> bool {
        self.overflow_x == Overflow::Scroll && self.max_offset().x > Dp::ZERO
    }

    pub(crate) fn can_scroll_y(self) -> bool {
        self.overflow_y == Overflow::Scroll && self.max_offset().y > Dp::ZERO
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct TransformRecord {
    pub(crate) id: WidgetId,
    pub(crate) base_offset: Point,
    pub(crate) current_offset: Point,
}

impl TransformRecord {
    pub(crate) fn delta(self) -> Point {
        Point {
            x: self.current_offset.x - self.base_offset.x,
            y: self.current_offset.y - self.base_offset.y,
        }
    }
}

pub(crate) struct ComputedScene<VM> {
    pub scene: ScenePrimitives,
    pub hit_regions: SmallVec<[HitRegion<VM>; 1]>,
    pub overlay_hit_regions: SmallVec<[HitRegion<VM>; 1]>,
    pub overlay_close_handlers: SmallVec<[crate::runtime::overlay::OverlayCloseHandle<VM>; 1]>,
    pub portal_overlay_counts: PortalOverlayCounts,
    pub focus_scopes: SmallVec<[FocusScopeState; 1]>,
    pub carousel_auto_play: SmallVec<[CarouselAutoPlayState<VM>; 1]>,
    pub overlay_anchors: HashMap<AnchorKey, Rect>,
    pub portal_entries: SmallVec<[PortalEntry<VM>; 1]>,
    pub external_portal_requests: SmallVec<[ExternalPortalRequest<VM>; 1]>,
    /// 每个 `OverlayLayer` 的暂存桶。`emit_overlay` 写入此处，
    /// `finalize_overlay_layers` 在 collect 收尾时按 layer 顺序合并到 `scene.overlay_*` /
    /// `overlay_hit_regions` / `overlay_close_handlers`，从而强制 z-order
    /// （Tooltip < Popover < Menu < Modal）。
    pub overlay_layers: [OverlayLayerBucket<VM>; OVERLAY_LAYER_COUNT],
    pub(crate) overlay_layer_graph: OverlayLayerGraph,
    pub scroll_regions: SmallVec<[ScrollRegion; 1]>,
    pub ime_cursor_area: Option<Rect>,
    pub virtual_state_updates: SmallVec<[VirtualSceneStateUpdate; 1]>,
    pub(crate) transform_records: HashMap<WidgetId, TransformRecord>,
    pub(crate) dependencies: DependencyGraph,
}

impl<VM> Clone for ComputedScene<VM> {
    fn clone(&self) -> Self {
        Self {
            scene: self.scene.clone(),
            hit_regions: self.hit_regions.clone(),
            overlay_hit_regions: self.overlay_hit_regions.clone(),
            overlay_close_handlers: self.overlay_close_handlers.clone(),
            portal_overlay_counts: self.portal_overlay_counts,
            focus_scopes: self.focus_scopes.clone(),
            carousel_auto_play: self.carousel_auto_play.clone(),
            overlay_anchors: self.overlay_anchors.clone(),
            portal_entries: self.portal_entries.clone(),
            external_portal_requests: self.external_portal_requests.clone(),
            overlay_layers: std::array::from_fn(|i| self.overlay_layers[i].clone()),
            overlay_layer_graph: self.overlay_layer_graph.clone(),
            scroll_regions: self.scroll_regions.clone(),
            ime_cursor_area: self.ime_cursor_area,
            virtual_state_updates: self.virtual_state_updates.clone(),
            transform_records: self.transform_records.clone(),
            dependencies: self.dependencies.clone(),
        }
    }
}

pub(crate) const OVERLAY_LAYER_COUNT: usize = 5;

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct OverlayLayerGraph {
    pub(crate) layers: SmallVec<[OverlayLayerGraphEntry; 4]>,
    pub(crate) anchor_slots: SmallVec<[OverlayAnchorSlot; 4]>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct OverlayLayerGraphEntry {
    pub(crate) layer: crate::runtime::overlay::OverlayLayer,
    pub(crate) command_range: Range<usize>,
    pub(crate) hit_range: Range<usize>,
    pub(crate) close_handler_range: Range<usize>,
    pub(crate) focus_scope_range: Range<usize>,
    pub(crate) sources: SmallVec<[Option<WidgetId>; 4]>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct OverlayAnchorSlot {
    pub(crate) key: AnchorKey,
    pub(crate) rect: Rect,
}

#[derive(Clone, Copy, Debug, Default)]
struct OverlayLayerGraphOffsets {
    commands: usize,
    hits: usize,
    close_handlers: usize,
    focus_scopes: usize,
}

impl OverlayLayerGraph {
    fn delta_since(&self, base: &Self) -> Self {
        let mut delta = Self::default();
        delta
            .layers
            .extend(self.layers.iter().skip(base.layers.len()).cloned());
        delta.anchor_slots.extend(
            self.anchor_slots
                .iter()
                .filter(|slot| {
                    base.anchor_slots
                        .iter()
                        .find(|candidate| candidate.key == slot.key)
                        .map(|candidate| candidate.rect)
                        != Some(slot.rect)
                })
                .copied(),
        );
        delta
    }

    fn extend_from(&mut self, other: &Self, offsets: OverlayLayerGraphOffsets) {
        self.layers
            .extend(other.layers.iter().cloned().map(|mut entry| {
                entry.command_range = (entry.command_range.start + offsets.commands)
                    ..(entry.command_range.end + offsets.commands);
                entry.hit_range =
                    (entry.hit_range.start + offsets.hits)..(entry.hit_range.end + offsets.hits);
                entry.close_handler_range = (entry.close_handler_range.start
                    + offsets.close_handlers)
                    ..(entry.close_handler_range.end + offsets.close_handlers);
                entry.focus_scope_range = (entry.focus_scope_range.start + offsets.focus_scopes)
                    ..(entry.focus_scope_range.end + offsets.focus_scopes);
                entry
            }));
        for slot in other.anchor_slots.iter().copied() {
            self.upsert_anchor(slot.key, slot.rect);
        }
    }

    fn upsert_anchor(&mut self, key: AnchorKey, rect: Rect) {
        if let Some(slot) = self.anchor_slots.iter_mut().find(|slot| slot.key == key) {
            slot.rect = rect;
        } else {
            self.anchor_slots.push(OverlayAnchorSlot { key, rect });
        }
    }

    fn retain_before(
        &mut self,
        commands: usize,
        hits: usize,
        close_handlers: usize,
        focus_scopes: usize,
    ) {
        self.layers.retain(|entry| {
            entry.command_range.end <= commands
                && entry.hit_range.end <= hits
                && entry.close_handler_range.end <= close_handlers
                && entry.focus_scope_range.end <= focus_scopes
        });
    }

    fn push_layer<VM>(
        &mut self,
        layer: crate::runtime::overlay::OverlayLayer,
        bucket: &OverlayLayerBucket<VM>,
        offsets: OverlayLayerGraphOffsets,
    ) {
        if bucket.commands.is_empty()
            && bucket.hits.is_empty()
            && bucket.close_handlers.is_empty()
            && bucket.focus_scopes.is_empty()
        {
            return;
        }
        self.layers.push(OverlayLayerGraphEntry {
            layer,
            command_range: offsets.commands..(offsets.commands + bucket.commands.len()),
            hit_range: offsets.hits..(offsets.hits + bucket.hits.len()),
            close_handler_range: offsets.close_handlers
                ..(offsets.close_handlers + bucket.close_handlers.len()),
            focus_scope_range: offsets.focus_scopes
                ..(offsets.focus_scopes + bucket.focus_scopes.len()),
            sources: bucket.command_sources.iter().copied().collect(),
        });
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct PortalOverlayCounts {
    pub shapes: usize,
    pub textures: usize,
    pub meshes: usize,
    pub texts: usize,
    pub text_decorations: usize,
    pub commands: usize,
    pub hits: usize,
    pub close_handlers: usize,
    pub focus_scopes: usize,
}

/// 单个 `OverlayLayer` 的暂存桶。
pub(crate) struct OverlayLayerBucket<VM> {
    pub commands: SmallVec<[RenderCommand; 1]>,
    pub command_sources: SmallVec<[Option<WidgetId>; 1]>,
    pub backdrop_blurs: SmallVec<[BackdropBlurPrimitive; 1]>,
    pub shapes: SmallVec<[RenderPrimitive; 1]>,
    pub textures: SmallVec<[TexturePrimitive; 1]>,
    pub meshes: SmallVec<[MeshPrimitive; 1]>,
    pub texts: SmallVec<[TextPrimitive; 1]>,
    pub text_decorations: SmallVec<[TextDecorationPrimitive; 1]>,
    pub hits: SmallVec<[HitRegion<VM>; 1]>,
    pub close_handlers: SmallVec<[crate::runtime::overlay::OverlayCloseHandle<VM>; 1]>,
    pub focus_scopes: SmallVec<[FocusScopeState; 1]>,
}

impl<VM> Default for OverlayLayerBucket<VM> {
    fn default() -> Self {
        Self {
            commands: SmallVec::new(),
            command_sources: SmallVec::new(),
            backdrop_blurs: SmallVec::new(),
            shapes: SmallVec::new(),
            textures: SmallVec::new(),
            meshes: SmallVec::new(),
            texts: SmallVec::new(),
            text_decorations: SmallVec::new(),
            hits: SmallVec::new(),
            close_handlers: SmallVec::new(),
            focus_scopes: SmallVec::new(),
        }
    }
}

impl<VM> Clone for OverlayLayerBucket<VM> {
    fn clone(&self) -> Self {
        Self {
            commands: self.commands.clone(),
            command_sources: self.command_sources.clone(),
            backdrop_blurs: self.backdrop_blurs.clone(),
            shapes: self.shapes.clone(),
            textures: self.textures.clone(),
            meshes: self.meshes.clone(),
            texts: self.texts.clone(),
            text_decorations: self.text_decorations.clone(),
            hits: self.hits.clone(),
            close_handlers: self.close_handlers.clone(),
            focus_scopes: self.focus_scopes.clone(),
        }
    }
}

impl<VM> OverlayLayerBucket<VM> {
    fn delta_since(&self, base: &Self) -> Self {
        let mut delta = Self::default();
        delta
            .commands
            .extend(self.commands.iter().skip(base.commands.len()).cloned());
        delta.command_sources.extend(
            self.command_sources
                .iter()
                .skip(base.command_sources.len())
                .copied(),
        );
        delta.backdrop_blurs.extend(
            self.backdrop_blurs
                .iter()
                .skip(base.backdrop_blurs.len())
                .copied(),
        );
        delta
            .shapes
            .extend(self.shapes.iter().skip(base.shapes.len()).copied());
        delta
            .textures
            .extend(self.textures.iter().skip(base.textures.len()).cloned());
        delta
            .meshes
            .extend(self.meshes.iter().skip(base.meshes.len()).cloned());
        delta
            .texts
            .extend(self.texts.iter().skip(base.texts.len()).cloned());
        delta.text_decorations.extend(
            self.text_decorations
                .iter()
                .skip(base.text_decorations.len())
                .cloned(),
        );
        delta
            .hits
            .extend(self.hits.iter().skip(base.hits.len()).cloned());
        delta.close_handlers.extend(
            self.close_handlers
                .iter()
                .skip(base.close_handlers.len())
                .cloned(),
        );
        delta.focus_scopes.extend(
            self.focus_scopes
                .iter()
                .skip(base.focus_scopes.len())
                .cloned(),
        );
        delta
    }

    fn extend_from(&mut self, other: &Self) {
        self.commands.extend(other.commands.iter().cloned());
        self.command_sources
            .extend(other.command_sources.iter().copied());
        self.backdrop_blurs
            .extend(other.backdrop_blurs.iter().copied());
        self.shapes.extend(other.shapes.iter().copied());
        self.textures.extend(other.textures.iter().cloned());
        self.meshes.extend(other.meshes.iter().cloned());
        self.texts.extend(other.texts.iter().cloned());
        self.text_decorations
            .extend(other.text_decorations.iter().cloned());
        self.hits.extend(other.hits.iter().cloned());
        self.close_handlers
            .extend(other.close_handlers.iter().cloned());
        self.focus_scopes.extend(other.focus_scopes.iter().cloned());
    }
}

#[derive(Clone, Default)]
pub(crate) struct WidgetStateMap {
    states: HashMap<WidgetId, WidgetState>,
    select_option_states: HashMap<(WidgetId, usize), WidgetState>,
}

impl WidgetStateMap {
    pub(crate) fn set(&mut self, id: WidgetId, state: WidgetState) {
        self.states.insert(id, state);
    }

    pub(crate) fn get(&self, id: WidgetId) -> WidgetState {
        self.states.get(&id).copied().unwrap_or_default()
    }

    pub(crate) fn set_select_option(
        &mut self,
        widget_id: WidgetId,
        option_index: usize,
        state: WidgetState,
    ) {
        self.select_option_states
            .insert((widget_id, option_index), state);
    }

    pub(crate) fn get_select_option(
        &self,
        widget_id: WidgetId,
        option_index: usize,
    ) -> WidgetState {
        self.select_option_states
            .get(&(widget_id, option_index))
            .copied()
            .unwrap_or_default()
    }
}

impl<VM> Default for ComputedScene<VM> {
    fn default() -> Self {
        Self {
            scene: ScenePrimitives::default(),
            hit_regions: SmallVec::new(),
            overlay_hit_regions: SmallVec::new(),
            overlay_close_handlers: SmallVec::new(),
            portal_overlay_counts: PortalOverlayCounts::default(),
            focus_scopes: SmallVec::new(),
            carousel_auto_play: SmallVec::new(),
            overlay_anchors: HashMap::new(),
            portal_entries: SmallVec::new(),
            external_portal_requests: SmallVec::new(),
            overlay_layers: std::array::from_fn(|_| OverlayLayerBucket::default()),
            overlay_layer_graph: OverlayLayerGraph::default(),
            scroll_regions: SmallVec::new(),
            ime_cursor_area: None,
            virtual_state_updates: SmallVec::new(),
            dependencies: DependencyGraph::default(),
            transform_records: HashMap::new(),
        }
    }
}

impl<VM> ComputedScene<VM> {
    pub(crate) fn fill_gpu_scroll_container(&mut self, id: WidgetId) {
        self.scene.fill_gpu_scroll_container(id);
        for hit in &mut self.hit_regions {
            if hit.gpu_scroll_container.is_none() {
                hit.gpu_scroll_container = Some(id);
            }
        }
        for hit in &mut self.overlay_hit_regions {
            if hit.gpu_scroll_container.is_none() {
                hit.gpu_scroll_container = Some(id);
            }
        }
    }

    pub(crate) fn delta_since(&self, base: &ComputedScene<VM>) -> ComputedScene<VM> {
        let mut delta = ComputedScene {
            scene: self.scene.delta_since(&base.scene),
            ..Default::default()
        };
        delta.hit_regions.extend(
            self.hit_regions
                .iter()
                .skip(base.hit_regions.len())
                .cloned(),
        );
        delta.overlay_hit_regions.extend(
            self.overlay_hit_regions
                .iter()
                .skip(base.overlay_hit_regions.len())
                .cloned(),
        );
        delta.overlay_close_handlers.extend(
            self.overlay_close_handlers
                .iter()
                .skip(base.overlay_close_handlers.len())
                .cloned(),
        );
        delta.focus_scopes.extend(
            self.focus_scopes
                .iter()
                .skip(base.focus_scopes.len())
                .cloned(),
        );
        delta.carousel_auto_play.extend(
            self.carousel_auto_play
                .iter()
                .skip(base.carousel_auto_play.len())
                .cloned(),
        );
        delta.overlay_anchors.extend(
            self.overlay_anchors
                .iter()
                .filter(|(key, rect)| base.overlay_anchors.get(key) != Some(*rect))
                .map(|(key, rect)| (*key, *rect)),
        );
        delta.portal_entries.extend(
            self.portal_entries
                .iter()
                .skip(base.portal_entries.len())
                .cloned(),
        );
        delta.external_portal_requests.extend(
            self.external_portal_requests
                .iter()
                .skip(base.external_portal_requests.len())
                .cloned(),
        );
        delta.portal_overlay_counts.shapes = self
            .portal_overlay_counts
            .shapes
            .saturating_sub(base.portal_overlay_counts.shapes);
        delta.portal_overlay_counts.textures = self
            .portal_overlay_counts
            .textures
            .saturating_sub(base.portal_overlay_counts.textures);
        delta.portal_overlay_counts.meshes = self
            .portal_overlay_counts
            .meshes
            .saturating_sub(base.portal_overlay_counts.meshes);
        delta.portal_overlay_counts.texts = self
            .portal_overlay_counts
            .texts
            .saturating_sub(base.portal_overlay_counts.texts);
        delta.portal_overlay_counts.commands = self
            .portal_overlay_counts
            .commands
            .saturating_sub(base.portal_overlay_counts.commands);
        delta.portal_overlay_counts.hits = self
            .portal_overlay_counts
            .hits
            .saturating_sub(base.portal_overlay_counts.hits);
        delta.portal_overlay_counts.close_handlers = self
            .portal_overlay_counts
            .close_handlers
            .saturating_sub(base.portal_overlay_counts.close_handlers);
        delta.portal_overlay_counts.focus_scopes = self
            .portal_overlay_counts
            .focus_scopes
            .saturating_sub(base.portal_overlay_counts.focus_scopes);
        for i in 0..OVERLAY_LAYER_COUNT {
            delta.overlay_layers[i] = self.overlay_layers[i].delta_since(&base.overlay_layers[i]);
        }
        delta.overlay_layer_graph = self
            .overlay_layer_graph
            .delta_since(&base.overlay_layer_graph);
        delta.scroll_regions.extend(
            self.scroll_regions
                .iter()
                .skip(base.scroll_regions.len())
                .copied(),
        );
        if base.ime_cursor_area.is_none() {
            delta.ime_cursor_area = self.ime_cursor_area;
        }
        delta.virtual_state_updates.extend(
            self.virtual_state_updates
                .iter()
                .skip(base.virtual_state_updates.len())
                .cloned(),
        );
        delta.transform_records.extend(
            self.transform_records
                .iter()
                .filter(|(id, _)| !base.transform_records.contains_key(id))
                .map(|(id, record)| (*id, *record)),
        );
        delta.dependencies = self.dependencies.clone();
        delta
    }

    pub(crate) fn extend(&mut self, other: &ComputedScene<VM>) {
        let graph_offsets = OverlayLayerGraphOffsets {
            commands: self.scene.overlay_commands.len(),
            hits: self.overlay_hit_regions.len(),
            close_handlers: self.overlay_close_handlers.len(),
            focus_scopes: self.focus_scopes.len(),
        };
        self.scene.extend(&other.scene);
        self.hit_regions.extend(other.hit_regions.iter().cloned());
        self.overlay_hit_regions
            .extend(other.overlay_hit_regions.iter().cloned());
        self.overlay_close_handlers
            .extend(other.overlay_close_handlers.iter().cloned());
        self.focus_scopes.extend(other.focus_scopes.iter().cloned());
        self.carousel_auto_play
            .extend(other.carousel_auto_play.iter().cloned());
        self.overlay_anchors
            .extend(other.overlay_anchors.iter().map(|(k, v)| (*k, *v)));
        self.portal_entries
            .extend(other.portal_entries.iter().cloned());
        self.external_portal_requests
            .extend(other.external_portal_requests.iter().cloned());
        self.portal_overlay_counts.shapes += other.portal_overlay_counts.shapes;
        self.portal_overlay_counts.textures += other.portal_overlay_counts.textures;
        self.portal_overlay_counts.meshes += other.portal_overlay_counts.meshes;
        self.portal_overlay_counts.texts += other.portal_overlay_counts.texts;
        self.portal_overlay_counts.text_decorations += other.portal_overlay_counts.text_decorations;
        self.portal_overlay_counts.commands += other.portal_overlay_counts.commands;
        self.portal_overlay_counts.hits += other.portal_overlay_counts.hits;
        self.portal_overlay_counts.close_handlers += other.portal_overlay_counts.close_handlers;
        self.portal_overlay_counts.focus_scopes += other.portal_overlay_counts.focus_scopes;
        for i in 0..OVERLAY_LAYER_COUNT {
            self.overlay_layers[i].extend_from(&other.overlay_layers[i]);
        }
        self.overlay_layer_graph
            .extend_from(&other.overlay_layer_graph, graph_offsets);
        self.scroll_regions
            .extend(other.scroll_regions.iter().copied());
        if self.ime_cursor_area.is_none() {
            self.ime_cursor_area = other.ime_cursor_area;
        }
        self.virtual_state_updates
            .extend(other.virtual_state_updates.iter().cloned());
        self.transform_records.extend(
            other
                .transform_records
                .iter()
                .map(|(id, record)| (*id, *record)),
        );
        self.dependencies.merge_from(&other.dependencies);
    }

    pub(crate) fn finalize_overlay_layers(&mut self) {
        for layer in crate::runtime::overlay::OverlayLayer::ALL {
            let bucket = std::mem::take(&mut self.overlay_layers[layer.index()]);
            debug_assert_eq!(
                bucket.commands.len(),
                bucket.command_sources.len(),
                "overlay command sources must stay aligned with overlay commands"
            );
            let graph_offsets = OverlayLayerGraphOffsets {
                commands: self.scene.overlay_commands.len(),
                hits: self.overlay_hit_regions.len(),
                close_handlers: self.overlay_close_handlers.len(),
                focus_scopes: self.focus_scopes.len(),
            };
            self.overlay_layer_graph
                .push_layer(layer, &bucket, graph_offsets);
            self.scene.backdrop_blurs.extend(bucket.backdrop_blurs);
            self.scene.overlay_shapes.extend(bucket.shapes);
            self.scene.overlay_textures.extend(bucket.textures);
            self.scene.overlay_meshes.extend(bucket.meshes);
            self.scene.overlay_texts.extend(bucket.texts);
            self.scene
                .overlay_text_decorations
                .extend(bucket.text_decorations);
            self.scene.overlay_commands.extend(bucket.commands);
            self.scene
                .overlay_command_sources
                .extend(bucket.command_sources);
            self.overlay_hit_regions.extend(bucket.hits);
            self.overlay_close_handlers.extend(bucket.close_handlers);
            self.focus_scopes.extend(bucket.focus_scopes);
        }
    }

    pub(crate) fn finalize_portals(&mut self, viewport: Rect) {
        let base_shapes = self
            .scene
            .overlay_shapes
            .len()
            .saturating_sub(self.portal_overlay_counts.shapes);
        let base_textures = self
            .scene
            .overlay_textures
            .len()
            .saturating_sub(self.portal_overlay_counts.textures);
        let base_meshes = self
            .scene
            .overlay_meshes
            .len()
            .saturating_sub(self.portal_overlay_counts.meshes);
        let base_texts = self
            .scene
            .overlay_texts
            .len()
            .saturating_sub(self.portal_overlay_counts.texts);
        let base_text_decorations = self
            .scene
            .overlay_text_decorations
            .len()
            .saturating_sub(self.portal_overlay_counts.text_decorations);
        let base_commands = self
            .scene
            .overlay_commands
            .len()
            .saturating_sub(self.portal_overlay_counts.commands);
        let base_hits = self
            .overlay_hit_regions
            .len()
            .saturating_sub(self.portal_overlay_counts.hits);
        let base_close_handlers = self
            .overlay_close_handlers
            .len()
            .saturating_sub(self.portal_overlay_counts.close_handlers);
        let base_focus_scopes = self
            .focus_scopes
            .len()
            .saturating_sub(self.portal_overlay_counts.focus_scopes);

        self.scene.overlay_shapes.truncate(base_shapes);
        self.scene.overlay_textures.truncate(base_textures);
        self.scene.overlay_meshes.truncate(base_meshes);
        self.scene.overlay_texts.truncate(base_texts);
        self.scene
            .overlay_text_decorations
            .truncate(base_text_decorations);
        self.scene.overlay_commands.truncate(base_commands);
        self.scene.overlay_command_sources.truncate(base_commands);
        self.overlay_hit_regions.truncate(base_hits);
        self.overlay_close_handlers.truncate(base_close_handlers);
        self.focus_scopes.truncate(base_focus_scopes);
        self.portal_overlay_counts = PortalOverlayCounts::default();
        self.overlay_layer_graph.retain_before(
            base_commands,
            base_hits,
            base_close_handlers,
            base_focus_scopes,
        );
        self.overlay_layers = std::array::from_fn(|_| OverlayLayerBucket::default());
        crate::runtime::overlay::collect::finalize_portal_entries(self, viewport);
        self.finalize_overlay_layers();
        self.portal_overlay_counts = PortalOverlayCounts {
            shapes: self.scene.overlay_shapes.len().saturating_sub(base_shapes),
            textures: self
                .scene
                .overlay_textures
                .len()
                .saturating_sub(base_textures),
            meshes: self.scene.overlay_meshes.len().saturating_sub(base_meshes),
            texts: self.scene.overlay_texts.len().saturating_sub(base_texts),
            text_decorations: self
                .scene
                .overlay_text_decorations
                .len()
                .saturating_sub(base_text_decorations),
            commands: self
                .scene
                .overlay_commands
                .len()
                .saturating_sub(base_commands),
            hits: self.overlay_hit_regions.len().saturating_sub(base_hits),
            close_handlers: self
                .overlay_close_handlers
                .len()
                .saturating_sub(base_close_handlers),
            focus_scopes: self.focus_scopes.len().saturating_sub(base_focus_scopes),
        };
    }

    pub(crate) fn finalize_additional_portals(
        &mut self,
        viewport: Rect,
        entries: impl IntoIterator<Item = PortalEntry<VM>>,
    ) {
        let base_shapes = self.scene.overlay_shapes.len();
        let base_textures = self.scene.overlay_textures.len();
        let base_meshes = self.scene.overlay_meshes.len();
        let base_texts = self.scene.overlay_texts.len();
        let base_text_decorations = self.scene.overlay_text_decorations.len();
        let base_commands = self.scene.overlay_commands.len();
        let base_hits = self.overlay_hit_regions.len();
        let base_close_handlers = self.overlay_close_handlers.len();
        let base_focus_scopes = self.focus_scopes.len();

        self.portal_entries.extend(entries);
        crate::runtime::overlay::collect::finalize_portal_entries(self, viewport);
        self.finalize_overlay_layers();

        self.portal_overlay_counts.shapes +=
            self.scene.overlay_shapes.len().saturating_sub(base_shapes);
        self.portal_overlay_counts.textures += self
            .scene
            .overlay_textures
            .len()
            .saturating_sub(base_textures);
        self.portal_overlay_counts.meshes +=
            self.scene.overlay_meshes.len().saturating_sub(base_meshes);
        self.portal_overlay_counts.texts +=
            self.scene.overlay_texts.len().saturating_sub(base_texts);
        self.portal_overlay_counts.text_decorations += self
            .scene
            .overlay_text_decorations
            .len()
            .saturating_sub(base_text_decorations);
        self.portal_overlay_counts.commands += self
            .scene
            .overlay_commands
            .len()
            .saturating_sub(base_commands);
        self.portal_overlay_counts.hits += self.overlay_hit_regions.len().saturating_sub(base_hits);
        self.portal_overlay_counts.close_handlers += self
            .overlay_close_handlers
            .len()
            .saturating_sub(base_close_handlers);
        self.portal_overlay_counts.focus_scopes +=
            self.focus_scopes.len().saturating_sub(base_focus_scopes);
    }

    pub(crate) fn register_overlay_anchor(&mut self, key: AnchorKey, rect: Rect) {
        self.overlay_anchors.insert(key, rect);
        self.overlay_layer_graph.upsert_anchor(key, rect);
    }

    pub(crate) fn register_widget_overlay_anchor(&mut self, widget_id: WidgetId, rect: Rect) {
        self.register_overlay_anchor(AnchorKey::widget(widget_id), rect);
    }

    pub(crate) fn register_caret_overlay_anchor(&mut self, widget_id: WidgetId, rect: Rect) {
        self.register_overlay_anchor(AnchorKey::caret(widget_id), rect);
    }

    pub(crate) fn register_focus_scope(&mut self, scope: FocusScopeState) {
        self.focus_scopes.push(scope);
    }

    pub(crate) fn resolve_overlay_anchor(&self, key: AnchorKey) -> Option<Rect> {
        self.overlay_anchors
            .get(&key)
            .copied()
            .or_else(|| match key.source() {
                AnchorSource::Caret(_) => None,
                AnchorSource::Widget(_) | AnchorSource::Point => None,
            })
    }

    #[cfg(test)]
    pub(crate) fn rendered(&self) -> RenderedWidgetScene {
        RenderedWidgetScene {
            primitives: self.scene.clone(),
            scroll_regions: self.scroll_regions.to_vec(),
            ime_cursor_area: self.ime_cursor_area,
        }
    }

    /// 各主渲染流命令数量。Splice：沿 root→target 路径累加，定位子树区间起点。
    pub(crate) fn scene_counts(&self) -> crate::ui::widget::common::SceneCounts {
        self.scene.counts()
    }

    pub(crate) fn can_write_shape_color_slot(
        &self,
        offset: &crate::ui::widget::common::SceneCounts,
        slot: crate::ui::widget::common::ShapePrimitiveSlot,
    ) -> bool {
        self.scene.can_write_shape_color_slot(offset, slot)
    }

    pub(crate) fn write_shape_color_slot(
        &mut self,
        offset: &crate::ui::widget::common::SceneCounts,
        slot: crate::ui::widget::common::ShapePrimitiveSlot,
        color: Color,
    ) -> bool {
        self.scene.write_shape_color_slot(offset, slot, color)
    }

    pub(crate) fn can_write_brush_slot(
        &self,
        offset: &crate::ui::widget::common::SceneCounts,
        slot: crate::ui::widget::common::BrushPrimitiveSlot,
    ) -> bool {
        self.scene.can_write_brush_slot(offset, slot)
    }

    pub(crate) fn write_brush_slot(
        &mut self,
        offset: &crate::ui::widget::common::SceneCounts,
        slot: crate::ui::widget::common::BrushPrimitiveSlot,
        primitive: crate::ui::widget::common::BrushPrimitive,
    ) -> bool {
        self.scene.write_brush_slot(offset, slot, primitive)
    }

    pub(crate) fn can_write_backdrop_blur_slot(
        &self,
        offset: &crate::ui::widget::common::SceneCounts,
        slot: crate::ui::widget::common::BackdropBlurPrimitiveSlot,
    ) -> bool {
        self.scene.can_write_backdrop_blur_slot(offset, slot)
    }

    pub(crate) fn write_backdrop_blur_slot(
        &mut self,
        offset: &crate::ui::widget::common::SceneCounts,
        slot: crate::ui::widget::common::BackdropBlurPrimitiveSlot,
        primitive: crate::ui::widget::common::BackdropBlurPrimitive,
    ) -> bool {
        self.scene.write_backdrop_blur_slot(offset, slot, primitive)
    }

    pub(crate) fn write_shape_rect_slot(
        &mut self,
        offset: &crate::ui::widget::common::SceneCounts,
        slot: crate::ui::widget::common::ShapePrimitiveSlot,
        rect: Rect,
    ) -> bool {
        self.scene.write_shape_rect_slot(offset, slot, rect)
    }

    pub(crate) fn write_shape_corner_radius_slot(
        &mut self,
        offset: &crate::ui::widget::common::SceneCounts,
        slot: crate::ui::widget::common::ShapePrimitiveSlot,
        corner_radius: f32,
    ) -> bool {
        self.scene
            .write_shape_corner_radius_slot(offset, slot, corner_radius)
    }

    pub(crate) fn write_shape_stroke_width_slot(
        &mut self,
        offset: &crate::ui::widget::common::SceneCounts,
        slot: crate::ui::widget::common::ShapePrimitiveSlot,
        stroke_width: f32,
    ) -> bool {
        self.scene
            .write_shape_stroke_width_slot(offset, slot, stroke_width)
    }

    pub(crate) fn can_write_text_color_slot(
        &self,
        offset: &crate::ui::widget::common::SceneCounts,
        slot: crate::ui::widget::common::TextPrimitiveSlot,
    ) -> bool {
        self.scene.can_write_text_color_slot(offset, slot)
    }

    pub(crate) fn write_text_color_slot(
        &mut self,
        offset: &crate::ui::widget::common::SceneCounts,
        slot: crate::ui::widget::common::TextPrimitiveSlot,
        color: Color,
    ) -> bool {
        self.scene.write_text_color_slot(offset, slot, color)
    }

    pub(crate) fn write_text_slot(
        &mut self,
        offset: &crate::ui::widget::common::SceneCounts,
        slot: crate::ui::widget::common::TextPrimitiveSlot,
        primitive: crate::ui::widget::common::TextPrimitive,
    ) -> bool {
        self.scene.write_text_slot(offset, slot, primitive)
    }

    pub(crate) fn write_text_content_slot(
        &mut self,
        offset: &crate::ui::widget::common::SceneCounts,
        slot: crate::ui::widget::common::TextPrimitiveSlot,
        content: Arc<str>,
        font_family: Option<Arc<str>>,
    ) -> bool {
        self.scene
            .write_text_content_slot(offset, slot, content, font_family)
    }

    pub(crate) fn can_write_text_decoration_slot(
        &self,
        offset: &crate::ui::widget::common::SceneCounts,
        slot: crate::ui::widget::common::TextDecorationPrimitiveSlot,
    ) -> bool {
        self.scene.can_write_text_decoration_slot(offset, slot)
    }

    pub(crate) fn write_text_decoration_slot(
        &mut self,
        offset: &crate::ui::widget::common::SceneCounts,
        slot: crate::ui::widget::common::TextDecorationPrimitiveSlot,
        primitive: crate::ui::widget::common::TextDecorationPrimitive,
    ) -> bool {
        self.scene
            .write_text_decoration_slot(offset, slot, primitive)
    }

    pub(crate) fn can_write_overlay_text_decoration_slot(
        &self,
        offset: &crate::ui::widget::common::SceneCounts,
        slot: crate::ui::widget::common::OverlayTextDecorationPrimitiveSlot,
    ) -> bool {
        self.scene
            .can_write_overlay_text_decoration_slot(offset, slot)
    }

    pub(crate) fn write_overlay_text_decoration_slot(
        &mut self,
        offset: &crate::ui::widget::common::SceneCounts,
        slot: crate::ui::widget::common::OverlayTextDecorationPrimitiveSlot,
        primitive: crate::ui::widget::common::TextDecorationPrimitive,
    ) -> bool {
        self.scene
            .write_overlay_text_decoration_slot(offset, slot, primitive)
    }

    pub(crate) fn can_write_texture_opacity_slot(
        &self,
        offset: &crate::ui::widget::common::SceneCounts,
        slot: crate::ui::widget::common::TexturePrimitiveSlot,
    ) -> bool {
        self.scene.can_write_texture_opacity_slot(offset, slot)
    }

    pub(crate) fn write_texture_opacity_slot(
        &mut self,
        offset: &crate::ui::widget::common::SceneCounts,
        slot: crate::ui::widget::common::TexturePrimitiveSlot,
        opacity: f32,
    ) -> bool {
        self.scene.write_texture_opacity_slot(offset, slot, opacity)
    }

    pub(crate) fn write_texture_slot(
        &mut self,
        offset: &crate::ui::widget::common::SceneCounts,
        slot: crate::ui::widget::common::TexturePrimitiveSlot,
        primitive: crate::ui::widget::common::TexturePrimitive,
    ) -> bool {
        self.scene.write_texture_slot(offset, slot, primitive)
    }

    pub(crate) fn texture_slot(
        &self,
        offset: &crate::ui::widget::common::SceneCounts,
        slot: crate::ui::widget::common::TexturePrimitiveSlot,
    ) -> Option<&crate::ui::widget::common::TexturePrimitive> {
        self.scene.texture_slot(offset, slot)
    }

    /// 该 chunk 是否只产生「可原地覆盖的主渲染视觉命令 + 可同步覆盖的位置元数据」，
    /// 不含任何会改变后续偏移、需要 finalize、或无法定位的内容。
    ///
    /// 允许非空且 splice 会同步覆盖的：`hit_regions`、`scroll_regions`（二者都按子树
    /// 连续排布，splice 用对应 offset + 数量一致性原地覆盖）。其余结构性内容
    /// （overlay/portal/focus/anchor/carousel/virtual/ime/外部 portal 请求）任一非空
    /// 都判定为不可 splice，干净回退到 recompose。
    pub(crate) fn is_simple_for_splice(&self) -> bool {
        self.scene.counts().has_no_overlay()
            && self.overlay_hit_regions.is_empty()
            && self.overlay_close_handlers.is_empty()
            && self.focus_scopes.is_empty()
            && self.carousel_auto_play.is_empty()
            && self.overlay_anchors.is_empty()
            && self.portal_entries.is_empty()
            && self.external_portal_requests.is_empty()
            && self.ime_cursor_area.is_none()
            && self.virtual_state_updates.is_empty()
            && self.portal_overlay_counts.commands == 0
            && self.portal_overlay_counts.shapes == 0
            && self.portal_overlay_counts.textures == 0
            && self.portal_overlay_counts.meshes == 0
            && self.portal_overlay_counts.texts == 0
            && self.portal_overlay_counts.hits == 0
            && self.portal_overlay_counts.close_handlers == 0
            && self.portal_overlay_counts.focus_scopes == 0
            && self.overlay_layers.iter().all(|bucket| {
                bucket.commands.is_empty()
                    && bucket.backdrop_blurs.is_empty()
                    && bucket.shapes.is_empty()
                    && bucket.textures.is_empty()
                    && bucket.meshes.is_empty()
                    && bucket.texts.is_empty()
                    && bucket.hits.is_empty()
                    && bucket.close_handlers.is_empty()
                    && bucket.focus_scopes.is_empty()
            })
    }

    /// 把 `chunk` 的主渲染流 + `hit_regions` + `scroll_regions` 原地覆盖到 `self`
    /// 从各自 offset 起的区间。任一流越界（数量不一致）立即返回 `false`——调用方必须
    /// 在调用前用数量一致性 + `is_simple_for_splice` 把关，确保不会中途失败留下半成品。
    pub(crate) fn splice_chunk_in_place(
        &mut self,
        offset: &crate::ui::widget::common::SceneCounts,
        hit_offset: usize,
        scroll_offset: usize,
        chunk: &ComputedScene<VM>,
    ) -> bool {
        if !self.scene.splice_in_place(offset, &chunk.scene) {
            return false;
        }
        let Some(hit_end) = hit_offset.checked_add(chunk.hit_regions.len()) else {
            return false;
        };
        if hit_end > self.hit_regions.len() {
            return false;
        }
        let Some(scroll_end) = scroll_offset.checked_add(chunk.scroll_regions.len()) else {
            return false;
        };
        if scroll_end > self.scroll_regions.len() {
            return false;
        }
        self.hit_regions[hit_offset..hit_end].clone_from_slice(&chunk.hit_regions);
        self.scroll_regions[scroll_offset..scroll_end].clone_from_slice(&chunk.scroll_regions);
        true
    }
}
