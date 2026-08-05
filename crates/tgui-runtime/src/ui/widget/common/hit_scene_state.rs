use super::*;
use crate::runtime::overlay::PortalEntry;
use crate::runtime::overlay::{AnchorKey, AnchorSource};
use crate::runtime::portal::ExternalPortalRequest;
use crate::ui::widget::VirtualSceneStateUpdate;
use smallvec::SmallVec;
use std::collections::BTreeMap;
use std::ops::Range;
use std::sync::OnceLock;

const HIT_TEST_INDEX_MIN_REGIONS: usize = 64;
const HIT_TEST_CELL_HEIGHT: f32 = 64.0;
const HIT_TEST_MAX_CELLS_PER_REGION: i64 = 32;
const SCROLL_REGION_LOOKUP_MIN_REGIONS: usize = 16;

#[derive(Debug, Default)]
pub(crate) struct HitTestIndex {
    normal: HitTestGrid,
    overlay: HitTestGrid,
}

#[derive(Debug, Default)]
struct HitTestGrid {
    cells: Vec<HitTestCell>,
    global: Vec<usize>,
    /// Regions with a retained visual transform cannot be bucketed by their base `rect.y`: the
    /// current transform offset can change without rebuilding this index. They remain a normally
    /// tiny, ordered side stream and are merged with the exact indexed candidates at query time.
    transformed: Vec<usize>,
    region_count: usize,
}

#[derive(Debug)]
struct HitTestCell {
    coordinate: i64,
    indices: SmallVec<[usize; 8]>,
}

impl HitTestIndex {
    fn build<VM>(normal: &[HitRegion<VM>], overlay: &[HitRegion<VM>]) -> Self {
        Self {
            normal: HitTestGrid::build(normal),
            overlay: HitTestGrid::build(overlay),
        }
    }

    pub(crate) fn for_each_normal_candidate(&self, y: Dp, visit: impl FnMut(usize)) {
        self.normal.for_each_candidate(y, visit);
    }

    pub(crate) fn for_each_overlay_candidate(&self, y: Dp, visit: impl FnMut(usize)) {
        self.overlay.for_each_candidate(y, visit);
    }

    fn is_selective(&self) -> bool {
        let transformed = self.normal.transformed.len() + self.overlay.transformed.len();
        let total = self.normal.region_count + self.overlay.region_count;
        let indexed = total.saturating_sub(transformed);

        // When most regions move together, merging a large transformed side stream would add
        // bookkeeping without shortening the scan. Preserve the exact legacy full-scan fallback.
        indexed >= HIT_TEST_INDEX_MIN_REGIONS && indexed.saturating_mul(8) >= total
    }
}

impl HitTestGrid {
    fn build<VM>(regions: &[HitRegion<VM>]) -> Self {
        let mut cells = BTreeMap::<i64, SmallVec<[usize; 8]>>::new();
        let mut global = Vec::new();
        let mut transformed = Vec::new();

        for (index, hit) in regions.iter().enumerate() {
            if !hit.transform_chain.is_empty() {
                transformed.push(index);
                continue;
            }
            let start = hit.rect.y.get();
            let end = hit.rect.bottom().get();
            if !start.is_finite() || !end.is_finite() || end < start {
                global.push(index);
                continue;
            }

            let start_cell = hit_test_cell(start);
            let end_cell = hit_test_cell(end);
            let cell_count = end_cell.saturating_sub(start_cell).saturating_add(1);
            if cell_count > HIT_TEST_MAX_CELLS_PER_REGION {
                global.push(index);
                continue;
            }

            for coordinate in start_cell..=end_cell {
                cells.entry(coordinate).or_default().push(index);
            }
        }

        Self {
            cells: cells
                .into_iter()
                .map(|(coordinate, indices)| HitTestCell {
                    coordinate,
                    indices,
                })
                .collect(),
            global,
            transformed,
            region_count: regions.len(),
        }
    }

    fn for_each_candidate(&self, y: Dp, mut visit: impl FnMut(usize)) {
        let cell_indices = y
            .get()
            .is_finite()
            .then(|| hit_test_cell(y.get()))
            .and_then(|coordinate| {
                self.cells
                    .binary_search_by_key(&coordinate, |cell| cell.coordinate)
                    .ok()
                    .map(|index| self.cells[index].indices.as_slice())
            });
        let cell_indices = cell_indices.unwrap_or(&[]);

        // All three streams are in original hit-region order. Merging them retains exact z-order;
        // a region belongs to exactly one of global, the queried cell, or transformed.
        let mut global_index = 0;
        let mut cell_index = 0;
        let mut transformed_index = 0;
        loop {
            let global = self.global.get(global_index).copied().unwrap_or(usize::MAX);
            let cell = cell_indices.get(cell_index).copied().unwrap_or(usize::MAX);
            let transformed = self
                .transformed
                .get(transformed_index)
                .copied()
                .unwrap_or(usize::MAX);
            let next = global.min(cell).min(transformed);
            if next == usize::MAX {
                break;
            }
            if next == global {
                global_index += 1;
            } else if next == cell {
                cell_index += 1;
            } else {
                transformed_index += 1;
            }
            visit(next);
        }
    }
}

fn hit_test_cell(value: f32) -> i64 {
    (value / HIT_TEST_CELL_HEIGHT).floor() as i64
}

#[derive(Debug, Default)]
pub(crate) struct ScrollRegionLookupIndex {
    scrollable: Box<[usize]>,
    scrollbars: Box<[usize]>,
}

impl ScrollRegionLookupIndex {
    fn build(regions: &[ScrollRegion]) -> Self {
        let mut scrollable = Vec::new();
        let mut scrollbars = Vec::new();
        for (index, region) in regions.iter().copied().enumerate() {
            if region.can_scroll_x() || region.can_scroll_y() {
                scrollable.push(index);
            }
            if region.horizontal_thumb.is_some() || region.vertical_thumb.is_some() {
                scrollbars.push(index);
            }
        }
        Self {
            scrollable: scrollable.into_boxed_slice(),
            scrollbars: scrollbars.into_boxed_slice(),
        }
    }

    pub(crate) fn scrollable_indices(&self) -> &[usize] {
        &self.scrollable
    }

    pub(crate) fn scrollbar_indices(&self) -> &[usize] {
        &self.scrollbars
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
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
    pub(crate) accessibility_fragments: SmallVec<[AccessibilityFragment<VM>; 1]>,
    pub carousel_auto_play: SmallVec<[CarouselAutoPlayState<VM>; 1]>,
    pub overlay_anchors: HashMap<AnchorKey, Rect>,
    pub portal_entries: SmallVec<[PortalEntry<VM>; 1]>,
    pub external_portal_requests: SmallVec<[ExternalPortalRequest<VM>; 1]>,
    /// 每个 `OverlayLayer` 的暂存桶。`emit_overlay` 写入此处，
    /// `finalize_overlay_layers` 在 collect 收尾时按 layer 顺序合并到 `scene.overlay_*` /
    /// `overlay_hit_regions` / `overlay_close_handlers`，从而强制 z-order
    /// （Tooltip < Popover < Menu < Modal）。
    pub overlay_layers: Vec<OverlayLayerBucket<VM>>,
    pub(crate) overlay_layer_graph: OverlayLayerGraph,
    pub scroll_regions: SmallVec<[ScrollRegion; 1]>,
    pub ime_cursor_area: Option<Rect>,
    pub virtual_state_updates: SmallVec<[VirtualSceneStateUpdate; 1]>,
    pub(crate) transform_records: HashMap<WidgetId, TransformRecord>,
    pub(crate) dependencies: DependencyGraph,
    /// Changes only when keyboard focus-navigation metadata may have changed. Paint and retained
    /// transform writes deliberately leave this epoch untouched.
    focus_navigation_epoch: u64,
    hit_test_index: OnceLock<Box<HitTestIndex>>,
    scroll_region_lookup_index: OnceLock<Box<ScrollRegionLookupIndex>>,
}

/// Append-only position inside a `ComputedScene`.
///
/// Large primitive/command/hit payloads are represented by stream lengths. The few map-like
/// metadata channels keep their small baseline values because collection may replace an existing
/// anchor or transform record rather than append a new entry.
#[derive(Clone)]
pub(crate) struct ComputedSceneCursor {
    scene: ScenePrimitiveCursor,
    hit_regions: usize,
    overlay_hit_regions: usize,
    overlay_close_handlers: usize,
    portal_overlay_counts: PortalOverlayCounts,
    focus_scopes: usize,
    accessibility_fragments: usize,
    carousel_auto_play: usize,
    overlay_anchors: HashMap<AnchorKey, Rect>,
    portal_entries: usize,
    external_portal_requests: usize,
    overlay_layers: [OverlayLayerBucketCursor; OVERLAY_LAYER_COUNT],
    overlay_layer_graph: OverlayLayerGraphCursor,
    scroll_regions: usize,
    ime_cursor_area: Option<Rect>,
    virtual_state_updates: usize,
    transform_record_ids: SmallVec<[WidgetId; 4]>,
}

#[derive(Clone)]
pub(crate) struct ComputedScenePrefixCursor {
    cursor: ComputedSceneCursor,
    overlay_layer_graph: OverlayLayerGraph,
    transform_records: HashMap<WidgetId, TransformRecord>,
    dependencies: DependencyGraph,
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
            accessibility_fragments: self.accessibility_fragments.clone(),
            carousel_auto_play: self.carousel_auto_play.clone(),
            overlay_anchors: self.overlay_anchors.clone(),
            portal_entries: self.portal_entries.clone(),
            external_portal_requests: self.external_portal_requests.clone(),
            overlay_layers: self.overlay_layers.iter().cloned().collect(),
            overlay_layer_graph: self.overlay_layer_graph.clone(),
            scroll_regions: self.scroll_regions.clone(),
            ime_cursor_area: self.ime_cursor_area,
            virtual_state_updates: self.virtual_state_updates.clone(),
            transform_records: self.transform_records.clone(),
            dependencies: self.dependencies.clone(),
            focus_navigation_epoch: self.focus_navigation_epoch,
            hit_test_index: OnceLock::new(),
            scroll_region_lookup_index: OnceLock::new(),
        }
    }
}

impl<VM> ComputedScene<VM> {
    pub(crate) fn assign_new_prepare_cache_serial(&mut self) {
        self.scene.assign_new_prepare_cache_serial();
    }

    pub(crate) fn focus_navigation_cache_key(&self) -> (u64, u64) {
        (
            self.scene.prepare_cache_serial(),
            self.focus_navigation_epoch,
        )
    }

    fn mark_focus_navigation_metadata_changed(&mut self) {
        self.focus_navigation_epoch = self.focus_navigation_epoch.wrapping_add(1);
    }
}

pub(crate) const OVERLAY_LAYER_COUNT: usize = 5;

pub(crate) fn fresh_overlay_layers<VM>() -> Vec<OverlayLayerBucket<VM>> {
    (0..OVERLAY_LAYER_COUNT)
        .map(|_| OverlayLayerBucket::default())
        .collect()
}

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
    fn cursor(&self) -> OverlayLayerGraphCursor {
        OverlayLayerGraphCursor {
            layers: self.layers.len(),
            anchor_slots: self.anchor_slots.clone(),
        }
    }

    #[cfg(test)]
    #[allow(dead_code)]
    fn delta_since(&self, base: &Self) -> Self {
        self.delta_since_cursor(&base.cursor())
    }

    fn delta_since_cursor(&self, base: &OverlayLayerGraphCursor) -> Self {
        let mut delta = Self::default();
        delta
            .layers
            .extend(self.layers.iter().skip(base.layers).cloned());
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

#[derive(Clone, Debug, Default)]
struct OverlayLayerGraphCursor {
    layers: usize,
    anchor_slots: SmallVec<[OverlayAnchorSlot; 4]>,
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
    pub accessibility_fragments: usize,
}

pub(crate) struct AccessibilityFragmentNode<VM> {
    pub(crate) widget_id: WidgetId,
    pub(crate) resolved_path: SmallVec<[usize; 4]>,
    pub(crate) bounds: Rect,
    pub(crate) clip_rect: Option<Rect>,
    pub(crate) hits: SmallVec<[HitRegion<VM>; 1]>,
    pub(crate) scroll_regions: SmallVec<[ScrollRegion; 1]>,
    pub(crate) children: SmallVec<[(usize, usize); 4]>,
    pub(crate) synthetic_semantics: Option<AccessibilitySyntheticSemantics>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AccessibilitySyntheticRole {
    Menu,
    MenuItem,
    MenuItemCheckbox,
    Tooltip,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AccessibilitySyntheticSemantics {
    pub(crate) role: AccessibilitySyntheticRole,
    pub(crate) label: Option<String>,
    pub(crate) disabled: bool,
    pub(crate) checked: Option<bool>,
    pub(crate) expanded: Option<bool>,
    pub(crate) has_menu_popup: bool,
}

impl<VM> Clone for AccessibilityFragmentNode<VM> {
    fn clone(&self) -> Self {
        Self {
            widget_id: self.widget_id,
            resolved_path: self.resolved_path.clone(),
            bounds: self.bounds,
            clip_rect: self.clip_rect,
            hits: self.hits.clone(),
            scroll_regions: self.scroll_regions.clone(),
            children: self.children.clone(),
            synthetic_semantics: self.synthetic_semantics.clone(),
        }
    }
}

pub(crate) struct AccessibilityFragment<VM> {
    pub(crate) source_window_instance_id: Option<u64>,
    pub(crate) source_publication_generation: Option<u64>,
    pub(crate) source_open: Option<crate::ui::layout::Value<bool>>,
    pub(crate) owner_path: SmallVec<[crate::runtime::overlay::OverlayId; 2]>,
    pub(crate) scope_path: Vec<WidgetId>,
    pub(crate) clip_rect: Option<Rect>,
    /// A cloned `Element` can repeat one raw `WidgetId` at multiple resolved paths. Until hit
    /// metadata carries an occurrence path, publishing such a fragment would make actions
    /// ambiguous, so accessibility conservatively omits it while visual rendering remains intact.
    pub(crate) has_duplicate_widget_ids: bool,
    pub(crate) resolved_root: std::sync::Arc<crate::ui::widget::ResolvedElement<VM>>,
    pub(crate) nodes: Vec<AccessibilityFragmentNode<VM>>,
}

impl<VM> Clone for AccessibilityFragment<VM> {
    fn clone(&self) -> Self {
        Self {
            source_window_instance_id: self.source_window_instance_id,
            source_publication_generation: self.source_publication_generation,
            source_open: self.source_open.clone(),
            owner_path: self.owner_path.clone(),
            scope_path: self.scope_path.clone(),
            clip_rect: self.clip_rect,
            has_duplicate_widget_ids: self.has_duplicate_widget_ids,
            resolved_root: std::sync::Arc::clone(&self.resolved_root),
            nodes: self.nodes.clone(),
        }
    }
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
    pub accessibility_fragments: SmallVec<[AccessibilityFragment<VM>; 1]>,
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
            accessibility_fragments: SmallVec::new(),
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
            accessibility_fragments: self.accessibility_fragments.clone(),
        }
    }
}

impl<VM> OverlayLayerBucket<VM> {
    pub(crate) fn push_command(&mut self, command: RenderCommand, source: Option<WidgetId>) {
        self.commands.push(command);
        self.command_sources.push(source);
    }

    #[cfg(test)]
    #[allow(dead_code)]
    fn delta_since(&self, base: &Self) -> Self {
        self.delta_since_cursor(&base.cursor())
    }

    fn cursor(&self) -> OverlayLayerBucketCursor {
        OverlayLayerBucketCursor {
            commands: self.commands.len(),
            command_sources: self.command_sources.len(),
            backdrop_blurs: self.backdrop_blurs.len(),
            shapes: self.shapes.len(),
            textures: self.textures.len(),
            meshes: self.meshes.len(),
            texts: self.texts.len(),
            text_decorations: self.text_decorations.len(),
            hits: self.hits.len(),
            close_handlers: self.close_handlers.len(),
            focus_scopes: self.focus_scopes.len(),
            accessibility_fragments: self.accessibility_fragments.len(),
        }
    }

    fn delta_since_cursor(&self, base: &OverlayLayerBucketCursor) -> Self {
        let mut delta = Self::default();
        delta
            .commands
            .extend(self.commands.iter().skip(base.commands).cloned());
        delta.command_sources.extend(
            self.command_sources
                .iter()
                .skip(base.command_sources)
                .copied(),
        );
        delta.backdrop_blurs.extend(
            self.backdrop_blurs
                .iter()
                .skip(base.backdrop_blurs)
                .copied(),
        );
        delta
            .shapes
            .extend(self.shapes.iter().skip(base.shapes).copied());
        delta
            .textures
            .extend(self.textures.iter().skip(base.textures).cloned());
        delta
            .meshes
            .extend(self.meshes.iter().skip(base.meshes).cloned());
        delta
            .texts
            .extend(self.texts.iter().skip(base.texts).cloned());
        delta.text_decorations.extend(
            self.text_decorations
                .iter()
                .skip(base.text_decorations)
                .cloned(),
        );
        delta.hits.extend(self.hits.iter().skip(base.hits).cloned());
        delta.close_handlers.extend(
            self.close_handlers
                .iter()
                .skip(base.close_handlers)
                .cloned(),
        );
        delta
            .focus_scopes
            .extend(self.focus_scopes.iter().skip(base.focus_scopes).cloned());
        delta.accessibility_fragments.extend(
            self.accessibility_fragments
                .iter()
                .skip(base.accessibility_fragments)
                .cloned(),
        );
        delta
    }

    fn prefix_at_cursor(&self, cursor: &OverlayLayerBucketCursor) -> Self {
        let mut prefix = Self::default();
        prefix
            .commands
            .extend(self.commands.iter().take(cursor.commands).cloned());
        prefix.command_sources.extend(
            self.command_sources
                .iter()
                .take(cursor.command_sources)
                .copied(),
        );
        prefix.backdrop_blurs.extend(
            self.backdrop_blurs
                .iter()
                .take(cursor.backdrop_blurs)
                .copied(),
        );
        prefix
            .shapes
            .extend(self.shapes.iter().take(cursor.shapes).copied());
        prefix
            .textures
            .extend(self.textures.iter().take(cursor.textures).cloned());
        prefix
            .meshes
            .extend(self.meshes.iter().take(cursor.meshes).cloned());
        prefix
            .texts
            .extend(self.texts.iter().take(cursor.texts).cloned());
        prefix.text_decorations.extend(
            self.text_decorations
                .iter()
                .take(cursor.text_decorations)
                .cloned(),
        );
        prefix
            .hits
            .extend(self.hits.iter().take(cursor.hits).cloned());
        prefix.close_handlers.extend(
            self.close_handlers
                .iter()
                .take(cursor.close_handlers)
                .cloned(),
        );
        prefix
            .focus_scopes
            .extend(self.focus_scopes.iter().take(cursor.focus_scopes).cloned());
        prefix.accessibility_fragments.extend(
            self.accessibility_fragments
                .iter()
                .take(cursor.accessibility_fragments)
                .cloned(),
        );
        prefix
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
        self.accessibility_fragments
            .extend(other.accessibility_fragments.iter().cloned());
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct OverlayLayerBucketCursor {
    commands: usize,
    command_sources: usize,
    backdrop_blurs: usize,
    shapes: usize,
    textures: usize,
    meshes: usize,
    texts: usize,
    text_decorations: usize,
    hits: usize,
    close_handlers: usize,
    focus_scopes: usize,
    accessibility_fragments: usize,
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
            scene: ScenePrimitives::new_prepare_cache_root(),
            hit_regions: SmallVec::new(),
            overlay_hit_regions: SmallVec::new(),
            overlay_close_handlers: SmallVec::new(),
            portal_overlay_counts: PortalOverlayCounts::default(),
            focus_scopes: SmallVec::new(),
            accessibility_fragments: SmallVec::new(),
            carousel_auto_play: SmallVec::new(),
            overlay_anchors: HashMap::new(),
            portal_entries: SmallVec::new(),
            external_portal_requests: SmallVec::new(),
            overlay_layers: fresh_overlay_layers(),
            overlay_layer_graph: OverlayLayerGraph::default(),
            scroll_regions: SmallVec::new(),
            ime_cursor_area: None,
            virtual_state_updates: SmallVec::new(),
            dependencies: DependencyGraph::default(),
            transform_records: HashMap::new(),
            focus_navigation_epoch: 0,
            hit_test_index: OnceLock::new(),
            scroll_region_lookup_index: OnceLock::new(),
        }
    }
}

impl<VM> ComputedScene<VM> {
    pub(crate) fn hit_test_index(&self) -> Option<&HitTestIndex> {
        if self.hit_regions.len() + self.overlay_hit_regions.len() < HIT_TEST_INDEX_MIN_REGIONS {
            return None;
        }
        let index = self.hit_test_index.get_or_init(|| {
            Box::new(HitTestIndex::build(
                &self.hit_regions,
                &self.overlay_hit_regions,
            ))
        });
        index.is_selective().then_some(index.as_ref())
    }

    pub(crate) fn invalidate_hit_test_index(&mut self) {
        self.hit_test_index = OnceLock::new();
    }

    pub(crate) fn scroll_region_lookup_index(&self) -> Option<&ScrollRegionLookupIndex> {
        if self.scroll_regions.len() < SCROLL_REGION_LOOKUP_MIN_REGIONS {
            return None;
        }
        Some(
            self.scroll_region_lookup_index
                .get_or_init(|| Box::new(ScrollRegionLookupIndex::build(&self.scroll_regions)))
                .as_ref(),
        )
    }

    pub(crate) fn invalidate_scroll_region_lookup_index(&mut self) {
        self.scroll_region_lookup_index = OnceLock::new();
    }

    /// Whether scrolling can change bounds or scroll-value semantics exposed to AccessKit.
    ///
    /// Every container emits a `ScrollRegion`, including non-scrolling containers. Using the raw
    /// runtime scroll epoch would therefore rebuild the accessibility tree for programmatic/no-op
    /// offsets that clamp back to zero. Reuse the lazy interaction index for larger scenes and
    /// scan at most the small-scene threshold otherwise.
    pub(crate) fn has_accessible_scroll_state(&self) -> bool {
        self.scroll_region_lookup_index()
            .map(|index| !index.scrollable_indices().is_empty())
            .unwrap_or_else(|| {
                self.scroll_regions
                    .iter()
                    .copied()
                    .any(|region| region.can_scroll_x() || region.can_scroll_y())
            })
    }

    /// Keep inline visual primitives but make a retained subtree completely
    /// non-interactive. This is the logical half of a view transition: the old
    /// panel may continue fading/sliding out, yet it must immediately release
    /// pointer, keyboard focus, IME, scrolling, autoplay, and overlay ownership.
    ///
    /// The caller may restore its own inactive focus-scope sentinel afterwards;
    /// keeping that sentinel makes scene-splice conservatively fall back to
    /// ancestor recomposition, where this gate is applied again.
    pub(crate) fn clear_interactive_subtree_channels(&mut self) {
        self.mark_focus_navigation_metadata_changed();
        self.scene.clear_overlay_streams();
        self.hit_regions.clear();
        self.overlay_hit_regions.clear();
        self.overlay_close_handlers.clear();
        self.portal_overlay_counts = PortalOverlayCounts::default();
        self.focus_scopes.clear();
        self.accessibility_fragments.clear();
        self.carousel_auto_play.clear();
        self.overlay_anchors.clear();
        self.portal_entries.clear();
        self.external_portal_requests.clear();
        for layer in &mut self.overlay_layers {
            layer.commands.clear();
            layer.command_sources.clear();
            layer.backdrop_blurs.clear();
            layer.shapes.clear();
            layer.textures.clear();
            layer.meshes.clear();
            layer.texts.clear();
            layer.text_decorations.clear();
            layer.hits.clear();
            layer.close_handlers.clear();
            layer.focus_scopes.clear();
            layer.accessibility_fragments.clear();
        }
        self.overlay_layer_graph.layers.clear();
        self.overlay_layer_graph.anchor_slots.clear();
        self.scroll_regions.clear();
        self.ime_cursor_area = None;
        self.invalidate_hit_test_index();
        self.invalidate_scroll_region_lookup_index();
    }

    /// Reset a materialized ancestor chunk for recomposition while retaining all reusable
    /// backing allocations.
    ///
    /// The subsequent `extend` sequence is exactly the same `before + children + after` order as
    /// the legacy clone-based path. Only storage ownership changes: the old flat ancestor chunk is
    /// moved out of the cache and becomes the output buffer for its replacement.
    pub(crate) fn clear_for_recompose(&mut self, seed: &Self) {
        self.mark_focus_navigation_metadata_changed();
        self.scene.clear_for_recompose(&seed.scene);
        self.hit_regions.clear();
        self.overlay_hit_regions.clear();
        self.overlay_close_handlers.clear();
        self.portal_overlay_counts = PortalOverlayCounts::default();
        self.focus_scopes.clear();
        self.accessibility_fragments.clear();
        self.carousel_auto_play.clear();
        self.overlay_anchors.clear();
        self.portal_entries.clear();
        self.external_portal_requests.clear();
        for layer in &mut self.overlay_layers {
            layer.commands.clear();
            layer.command_sources.clear();
            layer.backdrop_blurs.clear();
            layer.shapes.clear();
            layer.textures.clear();
            layer.meshes.clear();
            layer.texts.clear();
            layer.text_decorations.clear();
            layer.hits.clear();
            layer.close_handlers.clear();
            layer.focus_scopes.clear();
            layer.accessibility_fragments.clear();
        }
        self.overlay_layer_graph.layers.clear();
        self.overlay_layer_graph.anchor_slots.clear();
        self.scroll_regions.clear();
        self.ime_cursor_area = None;
        self.virtual_state_updates.clear();
        self.transform_records.clear();
        self.dependencies.clear();
        self.invalidate_hit_test_index();
        self.invalidate_scroll_region_lookup_index();
    }

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

    pub(crate) fn cursor(&self) -> ComputedSceneCursor {
        ComputedSceneCursor {
            scene: self.scene.cursor(),
            hit_regions: self.hit_regions.len(),
            overlay_hit_regions: self.overlay_hit_regions.len(),
            overlay_close_handlers: self.overlay_close_handlers.len(),
            portal_overlay_counts: self.portal_overlay_counts,
            focus_scopes: self.focus_scopes.len(),
            accessibility_fragments: self.accessibility_fragments.len(),
            carousel_auto_play: self.carousel_auto_play.len(),
            overlay_anchors: self.overlay_anchors.clone(),
            portal_entries: self.portal_entries.len(),
            external_portal_requests: self.external_portal_requests.len(),
            overlay_layers: std::array::from_fn(|index| self.overlay_layers[index].cursor()),
            overlay_layer_graph: self.overlay_layer_graph.cursor(),
            scroll_regions: self.scroll_regions.len(),
            ime_cursor_area: self.ime_cursor_area,
            virtual_state_updates: self.virtual_state_updates.len(),
            transform_record_ids: self.transform_records.keys().copied().collect(),
        }
    }

    pub(crate) fn prefix_cursor(&self) -> ComputedScenePrefixCursor {
        ComputedScenePrefixCursor {
            cursor: self.cursor(),
            overlay_layer_graph: self.overlay_layer_graph.clone(),
            transform_records: self.transform_records.clone(),
            dependencies: self.dependencies.clone(),
        }
    }

    #[cfg(any(test, feature = "bench-support"))]
    #[allow(dead_code)]
    pub(crate) fn delta_since(&self, base: &ComputedScene<VM>) -> ComputedScene<VM> {
        self.delta_since_cursor(&base.cursor())
    }

    pub(crate) fn delta_since_cursor(&self, base: &ComputedSceneCursor) -> ComputedScene<VM> {
        let mut delta = ComputedScene {
            scene: self.scene.delta_since_cursor(&base.scene),
            ..Default::default()
        };
        delta
            .hit_regions
            .extend(self.hit_regions.iter().skip(base.hit_regions).cloned());
        delta.overlay_hit_regions.extend(
            self.overlay_hit_regions
                .iter()
                .skip(base.overlay_hit_regions)
                .cloned(),
        );
        delta.overlay_close_handlers.extend(
            self.overlay_close_handlers
                .iter()
                .skip(base.overlay_close_handlers)
                .cloned(),
        );
        delta
            .focus_scopes
            .extend(self.focus_scopes.iter().skip(base.focus_scopes).cloned());
        delta.accessibility_fragments.extend(
            self.accessibility_fragments
                .iter()
                .skip(base.accessibility_fragments)
                .cloned(),
        );
        delta.carousel_auto_play.extend(
            self.carousel_auto_play
                .iter()
                .skip(base.carousel_auto_play)
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
                .skip(base.portal_entries)
                .cloned(),
        );
        delta.external_portal_requests.extend(
            self.external_portal_requests
                .iter()
                .skip(base.external_portal_requests)
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
        delta.portal_overlay_counts.text_decorations = self
            .portal_overlay_counts
            .text_decorations
            .saturating_sub(base.portal_overlay_counts.text_decorations);
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
        delta.portal_overlay_counts.accessibility_fragments = self
            .portal_overlay_counts
            .accessibility_fragments
            .saturating_sub(base.portal_overlay_counts.accessibility_fragments);
        for i in 0..OVERLAY_LAYER_COUNT {
            delta.overlay_layers[i] =
                self.overlay_layers[i].delta_since_cursor(&base.overlay_layers[i]);
        }
        delta.overlay_layer_graph = self
            .overlay_layer_graph
            .delta_since_cursor(&base.overlay_layer_graph);
        delta.scroll_regions.extend(
            self.scroll_regions
                .iter()
                .skip(base.scroll_regions)
                .copied(),
        );
        if base.ime_cursor_area.is_none() {
            delta.ime_cursor_area = self.ime_cursor_area;
        }
        delta.virtual_state_updates.extend(
            self.virtual_state_updates
                .iter()
                .skip(base.virtual_state_updates)
                .cloned(),
        );
        delta.transform_records.extend(
            self.transform_records
                .iter()
                .filter(|(id, _)| !base.transform_record_ids.contains(id))
                .map(|(id, record)| (*id, *record)),
        );
        delta.dependencies = self.dependencies.clone();
        delta
    }

    pub(crate) fn prefix_at_cursor(
        &self,
        prefix_cursor: &ComputedScenePrefixCursor,
    ) -> ComputedScene<VM> {
        let cursor = &prefix_cursor.cursor;
        let mut prefix = ComputedScene {
            scene: self.scene.prefix_at_cursor(&cursor.scene),
            portal_overlay_counts: cursor.portal_overlay_counts,
            overlay_anchors: cursor.overlay_anchors.clone(),
            overlay_layer_graph: prefix_cursor.overlay_layer_graph.clone(),
            ime_cursor_area: cursor.ime_cursor_area,
            transform_records: prefix_cursor.transform_records.clone(),
            dependencies: prefix_cursor.dependencies.clone(),
            ..Default::default()
        };
        prefix
            .hit_regions
            .extend(self.hit_regions.iter().take(cursor.hit_regions).cloned());
        prefix.overlay_hit_regions.extend(
            self.overlay_hit_regions
                .iter()
                .take(cursor.overlay_hit_regions)
                .cloned(),
        );
        prefix.overlay_close_handlers.extend(
            self.overlay_close_handlers
                .iter()
                .take(cursor.overlay_close_handlers)
                .cloned(),
        );
        prefix
            .focus_scopes
            .extend(self.focus_scopes.iter().take(cursor.focus_scopes).cloned());
        prefix.accessibility_fragments.extend(
            self.accessibility_fragments
                .iter()
                .take(cursor.accessibility_fragments)
                .cloned(),
        );
        prefix.carousel_auto_play.extend(
            self.carousel_auto_play
                .iter()
                .take(cursor.carousel_auto_play)
                .cloned(),
        );
        prefix.portal_entries.extend(
            self.portal_entries
                .iter()
                .take(cursor.portal_entries)
                .cloned(),
        );
        prefix.external_portal_requests.extend(
            self.external_portal_requests
                .iter()
                .take(cursor.external_portal_requests)
                .cloned(),
        );
        for i in 0..OVERLAY_LAYER_COUNT {
            prefix.overlay_layers[i] =
                self.overlay_layers[i].prefix_at_cursor(&cursor.overlay_layers[i]);
        }
        prefix.scroll_regions.extend(
            self.scroll_regions
                .iter()
                .take(cursor.scroll_regions)
                .copied(),
        );
        prefix.virtual_state_updates.extend(
            self.virtual_state_updates
                .iter()
                .take(cursor.virtual_state_updates)
                .cloned(),
        );
        prefix
    }

    pub(crate) fn extend(&mut self, other: &ComputedScene<VM>) {
        if !other.hit_regions.is_empty()
            || !other.overlay_hit_regions.is_empty()
            || !other.focus_scopes.is_empty()
        {
            self.mark_focus_navigation_metadata_changed();
        }
        self.invalidate_hit_test_index();
        self.invalidate_scroll_region_lookup_index();
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
        self.accessibility_fragments
            .extend(other.accessibility_fragments.iter().cloned());
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
        self.portal_overlay_counts.accessibility_fragments +=
            other.portal_overlay_counts.accessibility_fragments;
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
        if self
            .overlay_layers
            .iter()
            .any(|layer| !layer.hits.is_empty() || !layer.focus_scopes.is_empty())
        {
            self.mark_focus_navigation_metadata_changed();
        }
        self.invalidate_hit_test_index();
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
            let OverlayLayerBucket {
                commands,
                command_sources,
                hits,
                close_handlers,
                focus_scopes,
                accessibility_fragments,
                ..
            } = bucket;
            let mut command_sources = command_sources.into_iter();
            for command in commands {
                self.scene
                    .push_portal_overlay_command(command, command_sources.next().unwrap_or(None));
            }
            debug_assert!(
                command_sources.next().is_none(),
                "overlay command sources must stay aligned with overlay commands"
            );
            self.overlay_hit_regions.extend(hits);
            self.overlay_close_handlers.extend(close_handlers);
            self.focus_scopes.extend(focus_scopes);
            self.accessibility_fragments.extend(accessibility_fragments);
        }
    }

    pub(crate) fn finalize_portals(&mut self, viewport: Rect) {
        if !self.portal_entries.is_empty() {
            self.mark_focus_navigation_metadata_changed();
        }
        self.invalidate_hit_test_index();
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
        let base_accessibility_fragments = self
            .accessibility_fragments
            .len()
            .saturating_sub(self.portal_overlay_counts.accessibility_fragments);

        self.scene.overlay_shapes.truncate(base_shapes);
        self.scene.overlay_textures.truncate(base_textures);
        self.scene.overlay_meshes.truncate(base_meshes);
        self.scene.overlay_texts.truncate(base_texts);
        self.scene
            .overlay_text_decorations
            .truncate(base_text_decorations);
        self.scene.overlay_commands.truncate(base_commands);
        self.scene.overlay_command_sources.truncate(base_commands);
        self.scene
            .overlay_command_provenance
            .truncate(base_commands);
        self.overlay_hit_regions.truncate(base_hits);
        self.overlay_close_handlers.truncate(base_close_handlers);
        self.focus_scopes.truncate(base_focus_scopes);
        self.accessibility_fragments
            .truncate(base_accessibility_fragments);
        self.portal_overlay_counts = PortalOverlayCounts::default();
        self.overlay_layer_graph.retain_before(
            base_commands,
            base_hits,
            base_close_handlers,
            base_focus_scopes,
        );
        self.overlay_layers = fresh_overlay_layers();
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
            accessibility_fragments: self
                .accessibility_fragments
                .len()
                .saturating_sub(base_accessibility_fragments),
        };
    }

    pub(crate) fn finalize_additional_portals(
        &mut self,
        viewport: Rect,
        entries: impl IntoIterator<Item = PortalEntry<VM>>,
    ) {
        self.mark_focus_navigation_metadata_changed();
        self.invalidate_hit_test_index();
        let base_shapes = self.scene.overlay_shapes.len();
        let base_textures = self.scene.overlay_textures.len();
        let base_meshes = self.scene.overlay_meshes.len();
        let base_texts = self.scene.overlay_texts.len();
        let base_text_decorations = self.scene.overlay_text_decorations.len();
        let base_commands = self.scene.overlay_commands.len();
        let base_hits = self.overlay_hit_regions.len();
        let base_close_handlers = self.overlay_close_handlers.len();
        let base_focus_scopes = self.focus_scopes.len();
        let base_accessibility_fragments = self.accessibility_fragments.len();

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
        self.portal_overlay_counts.accessibility_fragments += self
            .accessibility_fragments
            .len()
            .saturating_sub(base_accessibility_fragments);
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

    /// Strict eligibility for the bounded focus-ring patch.  Only the single shape overlay
    /// emitted by `push_focus_ring_primitives` is permitted; every structural or interactive
    /// overlay channel forces the caller back to the general subtree patch.
    pub(crate) fn allows_focus_ring_overlay_patch(&self) -> bool {
        self.scene.focus_ring_overlay_shape().is_ok()
            && self.overlay_hit_regions.is_empty()
            && self.overlay_close_handlers.is_empty()
            && self.focus_scopes.is_empty()
            && self.accessibility_fragments.is_empty()
            && self.carousel_auto_play.is_empty()
            && self.overlay_anchors.is_empty()
            && self.portal_entries.is_empty()
            && self.external_portal_requests.is_empty()
            && self.ime_cursor_area.is_none()
            && self.virtual_state_updates.is_empty()
            && self.transform_records.is_empty()
            && self.portal_overlay_counts.shapes == 0
            && self.portal_overlay_counts.textures == 0
            && self.portal_overlay_counts.meshes == 0
            && self.portal_overlay_counts.texts == 0
            && self.portal_overlay_counts.text_decorations == 0
            && self.portal_overlay_counts.commands == 0
            && self.portal_overlay_counts.hits == 0
            && self.portal_overlay_counts.close_handlers == 0
            && self.portal_overlay_counts.focus_scopes == 0
            && self.portal_overlay_counts.accessibility_fragments == 0
            && self.overlay_layers.iter().all(|bucket| {
                bucket.commands.is_empty()
                    && bucket.command_sources.is_empty()
                    && bucket.backdrop_blurs.is_empty()
                    && bucket.shapes.is_empty()
                    && bucket.textures.is_empty()
                    && bucket.meshes.is_empty()
                    && bucket.texts.is_empty()
                    && bucket.text_decorations.is_empty()
                    && bucket.hits.is_empty()
                    && bucket.close_handlers.is_empty()
                    && bucket.focus_scopes.is_empty()
                    && bucket.accessibility_fragments.is_empty()
            })
            && self.overlay_layer_graph.layers.is_empty()
            && self.overlay_layer_graph.anchor_slots.is_empty()
    }

    /// Prove that focus changed only the shape-only overlay stream.  Main draw commands, hit
    /// geometry, focus-navigation metadata, and scroll geometry must remain item-identical.
    pub(crate) fn focus_ring_overlay_patch_compatible_with(&self, next: &Self) -> bool {
        self.allows_focus_ring_overlay_patch()
            && next.allows_focus_ring_overlay_patch()
            && self.scene.main_streams_equal(&next.scene)
            && self.hit_regions.len() == next.hit_regions.len()
            && self
                .hit_regions
                .iter()
                .zip(&next.hit_regions)
                .all(|(current, replacement)| current.focus_overlay_patch_metadata_eq(replacement))
            && self.scroll_regions == next.scroll_regions
    }

    /// A subtree recollection starts with a fresh focus-order counter, while the retained scene
    /// stores orders assigned during the full-tree walk. Focus-ring patches change only paint; keep
    /// the existing global order in the replacement chunks so a later ancestor recomposition
    /// cannot make Tab order depend on which subtree was recollected first.
    pub(crate) fn preserve_focus_orders_from(&mut self, source: &Self) -> bool {
        fn preserve<VM>(target: &mut [HitRegion<VM>], source: &[HitRegion<VM>]) -> bool {
            if target.len() != source.len() {
                return false;
            }
            for (target, source) in target.iter_mut().zip(source) {
                match (&source.focus, &mut target.focus) {
                    (None, None) => {}
                    (Some(source), Some(target))
                        if source.widget_id == target.widget_id
                            && source.tab_index == target.tab_index
                            && source.scope_path == target.scope_path =>
                    {
                        target.order = source.order;
                    }
                    _ => return false,
                }
            }
            true
        }

        preserve(&mut self.hit_regions, &source.hit_regions)
            && preserve(&mut self.overlay_hit_regions, &source.overlay_hit_regions)
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

    pub(crate) fn write_texture_mask_tint_slot(
        &mut self,
        offset: &crate::ui::widget::common::SceneCounts,
        slot: crate::ui::widget::common::TexturePrimitiveSlot,
        color: Color,
    ) -> bool {
        self.scene.write_texture_mask_tint_slot(offset, slot, color)
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
    /// （overlay/portal/focus/anchor/carousel/virtual/ime/transform/外部 portal 请求）任一非空
    /// 都判定为不可 splice，干净回退到 recompose。
    pub(crate) fn is_simple_for_splice(&self) -> bool {
        self.scene.counts().has_no_overlay()
            && self.overlay_hit_regions.is_empty()
            && self.overlay_close_handlers.is_empty()
            && self.focus_scopes.is_empty()
            && self.accessibility_fragments.is_empty()
            && self.carousel_auto_play.is_empty()
            && self.overlay_anchors.is_empty()
            && self.portal_entries.is_empty()
            && self.external_portal_requests.is_empty()
            && self.ime_cursor_area.is_none()
            && self.virtual_state_updates.is_empty()
            && self.transform_records.is_empty()
            && self.portal_overlay_counts.commands == 0
            && self.portal_overlay_counts.shapes == 0
            && self.portal_overlay_counts.textures == 0
            && self.portal_overlay_counts.meshes == 0
            && self.portal_overlay_counts.texts == 0
            && self.portal_overlay_counts.hits == 0
            && self.portal_overlay_counts.close_handlers == 0
            && self.portal_overlay_counts.focus_scopes == 0
            && self.portal_overlay_counts.accessibility_fragments == 0
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
                    && bucket.accessibility_fragments.is_empty()
            })
    }

    /// Button hover 子树 patch 的严格结构资格。
    ///
    /// 与 [`Self::is_simple_for_splice`] 相同地拒绝 overlay / portal / focus scope / anchor /
    /// carousel / virtual / IME / transform 等结构性内容，但允许主场景 accessibility
    /// fragment。普通 Button 必然产生 accessibility fragment；子树 patch 会通过既有
    /// recompose 路径完整替换并向祖先合成这些 fragment，而底层原地 splice 仍继续使用
    /// 更严格的 `is_simple_for_splice`，不会尝试原地覆盖无障碍元数据。
    pub(crate) fn is_simple_for_button_hover_recompose(&self) -> bool {
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
            && self.transform_records.is_empty()
            && self.portal_overlay_counts.commands == 0
            && self.portal_overlay_counts.shapes == 0
            && self.portal_overlay_counts.textures == 0
            && self.portal_overlay_counts.meshes == 0
            && self.portal_overlay_counts.texts == 0
            && self.portal_overlay_counts.hits == 0
            && self.portal_overlay_counts.close_handlers == 0
            && self.portal_overlay_counts.focus_scopes == 0
            && self.portal_overlay_counts.accessibility_fragments == 0
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
                    && bucket.accessibility_fragments.is_empty()
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
        if !self.scene.splice_in_place(offset, &chunk.scene) {
            return false;
        }
        let focus_navigation_changed = !self.hit_regions[hit_offset..hit_end]
            .iter()
            .zip(chunk.hit_regions.iter())
            .all(|(current, replacement)| current.focus_navigation_metadata_eq(replacement));
        if focus_navigation_changed {
            self.mark_focus_navigation_metadata_changed();
        }
        self.invalidate_hit_test_index();
        self.invalidate_scroll_region_lookup_index();
        self.hit_regions[hit_offset..hit_end].clone_from_slice(&chunk.hit_regions);
        self.scroll_regions[scroll_offset..scroll_end].clone_from_slice(&chunk.scroll_regions);
        true
    }
}
