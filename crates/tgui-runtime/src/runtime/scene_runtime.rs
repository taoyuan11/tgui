use super::*;
use crate::foundation::binding::ScrollRequestMode;
use crate::foundation::binding::{ScrollRequest, ScrollViewController};
use crate::ui::unit::Dp;
use smallvec::SmallVec;

const MAX_VIRTUAL_LAYOUT_FEEDBACK_PASSES: usize = 4;

/// Benchmark-only path accounting for one runtime scene request.
///
/// Unlike timing inferred from the resulting cache flags, these counters distinguish a retained
/// scene recollect from a root layout rebuild even though both leave a valid cache behind. The
/// probe is opt-in, thread-local, and compiled out of normal production builds.
#[cfg(any(test, feature = "bench-support"))]
#[allow(dead_code)]
pub(in crate::runtime) mod frame_path_probe {
    use std::cell::Cell;

    thread_local! {
        static ENABLED: Cell<bool> = const { Cell::new(false) };
        static CACHE_HITS: Cell<u64> = const { Cell::new(0) };
        static SCENE_RECOLLECTS: Cell<u64> = const { Cell::new(0) };
        static LAYOUT_REUSES: Cell<u64> = const { Cell::new(0) };
        static LAYOUT_BUILDS: Cell<u64> = const { Cell::new(0) };
    }

    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub(in crate::runtime) struct Snapshot {
        pub cache_hits: u64,
        pub scene_recollects: u64,
        pub layout_reuses: u64,
        pub layout_builds: u64,
    }

    pub(in crate::runtime) fn begin() {
        CACHE_HITS.with(|value| value.set(0));
        SCENE_RECOLLECTS.with(|value| value.set(0));
        LAYOUT_REUSES.with(|value| value.set(0));
        LAYOUT_BUILDS.with(|value| value.set(0));
        ENABLED.with(|enabled| enabled.set(true));
    }

    pub(in crate::runtime) fn finish() -> Snapshot {
        ENABLED.with(|enabled| enabled.set(false));
        Snapshot {
            cache_hits: CACHE_HITS.with(Cell::get),
            scene_recollects: SCENE_RECOLLECTS.with(Cell::get),
            layout_reuses: LAYOUT_REUSES.with(Cell::get),
            layout_builds: LAYOUT_BUILDS.with(Cell::get),
        }
    }

    #[inline]
    pub(in crate::runtime) fn record_cache_hit() {
        if ENABLED.with(Cell::get) {
            CACHE_HITS.with(|value| value.set(value.get() + 1));
        }
    }

    #[inline]
    pub(in crate::runtime) fn record_scene_recollect(layout_reused: bool) {
        if !ENABLED.with(Cell::get) {
            return;
        }
        SCENE_RECOLLECTS.with(|value| value.set(value.get() + 1));
        if layout_reused {
            LAYOUT_REUSES.with(|value| value.set(value.get() + 1));
        } else {
            LAYOUT_BUILDS.with(|value| value.set(value.get() + 1));
        }
    }
}

#[cfg(test)]
pub(in crate::runtime) mod row_hover_patch_probe {
    use std::cell::Cell;

    thread_local! {
        static HITS: Cell<u64> = const { Cell::new(0) };
    }

    pub(in crate::runtime) fn reset() {
        HITS.with(|hits| hits.set(0));
    }

    pub(in crate::runtime) fn record_hit() {
        HITS.with(|hits| hits.set(hits.get() + 1));
    }

    pub(in crate::runtime) fn hits() -> u64 {
        HITS.with(Cell::get)
    }
}

#[cfg(test)]
pub(in crate::runtime) mod button_hover_patch_probe {
    use std::cell::Cell;

    thread_local! {
        static HITS: Cell<u64> = const { Cell::new(0) };
    }

    pub(in crate::runtime) fn reset() {
        HITS.with(|hits| hits.set(0));
    }

    pub(in crate::runtime) fn record_hit() {
        HITS.with(|hits| hits.set(hits.get() + 1));
    }

    pub(in crate::runtime) fn hits() -> u64 {
        HITS.with(Cell::get)
    }
}

#[cfg(test)]
pub(in crate::runtime) mod button_pressed_patch_probe {
    use std::cell::Cell;

    thread_local! {
        static HITS: Cell<u64> = const { Cell::new(0) };
    }

    pub(in crate::runtime) fn reset() {
        HITS.with(|hits| hits.set(0));
    }

    pub(in crate::runtime) fn record_hit() {
        HITS.with(|hits| hits.set(hits.get() + 1));
    }

    pub(in crate::runtime) fn hits() -> u64 {
        HITS.with(Cell::get)
    }
}

/// 测试探针：记录纯滚动快路径命中次数，让测试能断言「确实走了滚动快路径而非整帧重收集」。
/// 仅测试构建编译，热路径零成本。
#[cfg(any(test, feature = "bench-support"))]
#[allow(dead_code)]
pub(in crate::runtime) mod scroll_fast_path_probe {
    use std::cell::Cell;
    thread_local! {
        static GPU_HITS: Cell<u64> = const { Cell::new(0) };
        static PATCH_HITS: Cell<u64> = const { Cell::new(0) };
        static VIRTUAL_HITS: Cell<u64> = const { Cell::new(0) };
        static VIRTUAL_SCENE_HITS: Cell<u64> = const { Cell::new(0) };
    }
    pub(in crate::runtime) fn record_gpu_hit() {
        GPU_HITS.with(|h| h.set(h.get() + 1));
    }
    pub(in crate::runtime) fn record_patch_hit() {
        PATCH_HITS.with(|h| h.set(h.get() + 1));
    }
    pub(in crate::runtime) fn record_virtual_hit() {
        VIRTUAL_HITS.with(|h| h.set(h.get() + 1));
    }
    pub(in crate::runtime) fn record_virtual_scene_hit() {
        VIRTUAL_SCENE_HITS.with(|h| h.set(h.get() + 1));
    }
    pub(in crate::runtime) fn reset() {
        GPU_HITS.with(|h| h.set(0));
        PATCH_HITS.with(|h| h.set(0));
        VIRTUAL_HITS.with(|h| h.set(0));
        VIRTUAL_SCENE_HITS.with(|h| h.set(0));
    }
    pub(in crate::runtime) fn hits() -> u64 {
        gpu_hits() + patch_hits()
    }
    pub(in crate::runtime) fn gpu_hits() -> u64 {
        GPU_HITS.with(Cell::get)
    }
    pub(in crate::runtime) fn patch_hits() -> u64 {
        PATCH_HITS.with(Cell::get)
    }
    pub(in crate::runtime) fn virtual_hits() -> u64 {
        VIRTUAL_HITS.with(Cell::get)
    }
    pub(in crate::runtime) fn virtual_scene_hits() -> u64 {
        VIRTUAL_SCENE_HITS.with(Cell::get)
    }
}

/// 测试探针：量化 controller 绑定索引的重建扫描量与缓存命中时的访问量。
/// 仅测试构建编译，生产热路径零成本。
#[cfg(test)]
pub(in crate::runtime) mod scroll_view_binding_probe {
    use std::cell::Cell;

    thread_local! {
        static REBUILD_REGION_VISITS: Cell<u64> = const { Cell::new(0) };
        static CONSUME_BINDING_VISITS: Cell<u64> = const { Cell::new(0) };
        static STALE_REBUILDS: Cell<u64> = const { Cell::new(0) };
    }

    pub(in crate::runtime) fn record_rebuild_region_visits(count: usize) {
        REBUILD_REGION_VISITS.with(|value| value.set(value.get() + count as u64));
    }

    pub(in crate::runtime) fn record_consume_binding_visit() {
        CONSUME_BINDING_VISITS.with(|value| value.set(value.get() + 1));
    }

    pub(in crate::runtime) fn record_stale_rebuild() {
        STALE_REBUILDS.with(|value| value.set(value.get() + 1));
    }

    pub(in crate::runtime) fn reset() {
        REBUILD_REGION_VISITS.with(|value| value.set(0));
        CONSUME_BINDING_VISITS.with(|value| value.set(0));
        STALE_REBUILDS.with(|value| value.set(0));
    }

    pub(in crate::runtime) fn rebuild_region_visits() -> u64 {
        REBUILD_REGION_VISITS.with(Cell::get)
    }

    pub(in crate::runtime) fn consume_binding_visits() -> u64 {
        CONSUME_BINDING_VISITS.with(Cell::get)
    }

    pub(in crate::runtime) fn stale_rebuilds() -> u64 {
        STALE_REBUILDS.with(Cell::get)
    }
}

fn gpu_scroll_clip_supported(clip_rect: Option<Rect>, region: ScrollRegion) -> bool {
    let Some(clip_rect) = clip_rect else {
        return false;
    };
    clip_rect == region.content_viewport || clip_rect == region.visible_frame
}

fn gpu_scroll_command_supported(
    command: &crate::ui::widget::RenderCommand,
    region: ScrollRegion,
) -> bool {
    match command {
        crate::ui::widget::RenderCommand::BackdropBlur(_)
        | crate::ui::widget::RenderCommand::CanvasComposite(_) => false,
        crate::ui::widget::RenderCommand::Brush(primitive) => {
            gpu_scroll_clip_supported(primitive.clip_rect, region)
        }
        crate::ui::widget::RenderCommand::Shape(primitive) => {
            gpu_scroll_clip_supported(primitive.clip_rect, region)
        }
        crate::ui::widget::RenderCommand::Texture(primitive) => {
            gpu_scroll_clip_supported(primitive.clip_rect, region)
        }
        #[cfg(feature = "video")]
        crate::ui::widget::RenderCommand::VideoTexture(primitive) => {
            gpu_scroll_clip_supported(primitive.clip_rect, region)
        }
        crate::ui::widget::RenderCommand::Text(primitive) => {
            gpu_scroll_clip_supported(primitive.clip_rect, region)
        }
        crate::ui::widget::RenderCommand::TextDecoration(primitive) => {
            gpu_scroll_clip_supported(primitive.clip_rect, region)
        }
        crate::ui::widget::RenderCommand::Mesh(primitive) => {
            gpu_scroll_clip_supported(primitive.clip_rect, region)
        }
    }
}

fn gpu_scroll_scene_supported(
    scene: &crate::ui::widget::ScenePrimitives,
    widget_id: WidgetId,
    region: ScrollRegion,
) -> bool {
    scene
        .commands
        .iter()
        .zip(scene.command_gpu_scroll_containers())
        .all(|(command, owner)| {
            *owner != Some(widget_id) || gpu_scroll_command_supported(command, region)
        })
}

fn translate_gpu_scroll_region(region: &mut ScrollRegion, delta: Point) {
    translate_gpu_point(&mut region.gpu_base_scroll_offset, delta);
    translate_gpu_rect(&mut region.content_viewport, delta);
    translate_gpu_rect(&mut region.visible_frame, delta);
    translate_gpu_rect(&mut region.content_bounds, delta);
    if let Some(track) = &mut region.horizontal_track {
        translate_gpu_rect(track, delta);
    }
    if let Some(thumb) = &mut region.horizontal_thumb {
        translate_gpu_rect(thumb, delta);
    }
    if let Some(track) = &mut region.vertical_track {
        translate_gpu_rect(track, delta);
    }
    if let Some(thumb) = &mut region.vertical_thumb {
        translate_gpu_rect(thumb, delta);
    }
}

fn translate_gpu_point(point: &mut Point, delta: Point) {
    point.x += delta.x;
    point.y += delta.y;
}

fn translate_gpu_rect(rect: &mut Rect, delta: Point) {
    rect.x += delta.x;
    rect.y += delta.y;
}

fn translate_gpu_hit_geometry(geometry: &mut crate::ui::widget::HitGeometry, delta: Point) {
    match geometry {
        crate::ui::widget::HitGeometry::Rect => {}
        crate::ui::widget::HitGeometry::Quad(quad) => {
            for point in quad {
                translate_gpu_point(point, delta);
            }
        }
        crate::ui::widget::HitGeometry::Triangles(triangles) => {
            let translated = triangles
                .iter()
                .map(|triangle| {
                    let mut triangle = *triangle;
                    for point in &mut triangle {
                        translate_gpu_point(point, delta);
                    }
                    triangle
                })
                .collect::<Vec<_>>();
            *triangles = Arc::from(translated);
        }
    }
}

fn gpu_scroll_hit_supported<VM>(
    hit: &crate::ui::widget::HitRegion<VM>,
    widget_id: WidgetId,
) -> bool {
    hit.gpu_scroll_container != Some(widget_id)
        || !matches!(
            hit.interaction,
            crate::ui::widget::HitInteraction::CanvasItem { .. }
        )
}

fn translate_gpu_hit_interaction<VM>(
    interaction: &mut crate::ui::widget::HitInteraction<VM>,
    delta: Point,
) {
    match interaction {
        crate::ui::widget::HitInteraction::SelectableText { frame, .. } => {
            translate_gpu_rect(frame, delta);
        }
        crate::ui::widget::HitInteraction::Slider {
            track_rect,
            thumb_rect,
            ..
        } => {
            translate_gpu_rect(track_rect, delta);
            translate_gpu_rect(thumb_rect, delta);
        }
        crate::ui::widget::HitInteraction::TextInput { frame, .. } => {
            translate_gpu_rect(frame, delta);
        }
        crate::ui::widget::HitInteraction::CanvasItem { .. } => {}
        _ => {}
    }
}

fn translate_gpu_scroll_hits<VM>(
    hits: &mut [crate::ui::widget::HitRegion<VM>],
    widget_id: WidgetId,
    delta: Point,
) {
    for hit in hits {
        if hit.gpu_scroll_container != Some(widget_id) {
            continue;
        }
        translate_gpu_rect(&mut hit.rect, delta);
        translate_gpu_hit_geometry(&mut hit.geometry, delta);
        translate_gpu_hit_interaction(&mut hit.interaction, delta);
    }
}

fn translate_descendant_scroll_regions(
    descendant_ids: &[WidgetId],
    regions: &mut [ScrollRegion],
    delta: Point,
) {
    for region in regions {
        if descendant_ids.contains(&region.id) {
            translate_gpu_scroll_region(region, delta);
        }
    }
}

impl<VM: 'static> BoundRuntimeHandler<VM> {
    fn try_pure_scroll_gpu_fast_path(
        &mut self,
        viewport: Rect,
        units: UnitContext,
        caret_visible: bool,
        active_scrollbar: Option<ScrollbarHandle>,
    ) -> bool {
        let Some(cached) = self.cached_scene.as_ref() else {
            return false;
        };
        if !self.gpu_scroll_supported {
            return false;
        }
        if !cached.computed_valid
            || !cached.layout_valid
            || cached.scroll_epoch == self.scroll_epoch
            || !self.scene_cache_fields_match_ignoring_scroll(
                cached,
                viewport,
                units,
                caret_visible,
                active_scrollbar,
            )
        {
            return false;
        }
        let Some(layout) = cached.layout.as_ref() else {
            return false;
        };
        if layout.contains_virtual() || self.scroll_dirty_widgets.len() != 1 {
            return false;
        }
        if !cached.computed.overlay_hit_regions.is_empty()
            || !cached.computed.accessibility_fragments.is_empty()
            || cached
                .computed
                .overlay_layers
                .iter()
                .any(|layer| !layer.accessibility_fragments.is_empty())
            || cached.computed.portal_overlay_counts.commands > 0
            || cached.computed.portal_overlay_counts.hits > 0
            || cached
                .computed
                .portal_overlay_counts
                .accessibility_fragments
                > 0
            || !cached.computed.portal_entries.is_empty()
            || !cached.computed.external_portal_requests.is_empty()
            || cached.computed.ime_cursor_area.is_some()
        {
            return false;
        }

        let Some(widget_id) = self.scroll_dirty_widgets.iter().next().copied() else {
            return false;
        };
        let Some(region_index) = cached
            .computed
            .scroll_regions
            .iter()
            .position(|region| region.id == widget_id)
        else {
            return false;
        };
        let old_region = cached.computed.scroll_regions[region_index];
        let Some(path) = layout.path_for(widget_id) else {
            return false;
        };
        let mut descendant_scroll_ids = SmallVec::<[WidgetId; 8]>::new();
        for region in &cached.computed.scroll_regions {
            if region.id == widget_id {
                continue;
            }
            if layout
                .path_for(region.id)
                .map(|candidate| candidate.starts_with(path))
                .unwrap_or(false)
            {
                descendant_scroll_ids.push(region.id);
            }
        }
        if !gpu_scroll_scene_supported(&cached.computed.scene, widget_id, old_region) {
            return false;
        }
        if !cached
            .computed
            .hit_regions
            .iter()
            .all(|hit| gpu_scroll_hit_supported(hit, widget_id))
        {
            return false;
        }
        if old_region.horizontal_track.is_some()
            || old_region.horizontal_thumb.is_some()
            || old_region.vertical_track.is_some()
            || old_region.vertical_thumb.is_some()
        {
            return false;
        }
        let next_offset = self
            .scroll_states
            .get(&widget_id)
            .copied()
            .unwrap_or(Point::ZERO);
        if (next_offset.x - old_region.scroll_offset.x).abs() <= 0.01
            && (next_offset.y - old_region.scroll_offset.y).abs() <= 0.01
        {
            return false;
        }
        let max = old_region.max_offset();
        let next_offset = Point::new(
            if old_region.overflow_x == crate::ui::layout::Overflow::Scroll {
                next_offset.x.clamp(Dp::ZERO, max.x)
            } else {
                Dp::ZERO
            },
            if old_region.overflow_y == crate::ui::layout::Overflow::Scroll {
                next_offset.y.clamp(Dp::ZERO, max.y)
            } else {
                Dp::ZERO
            },
        );

        let Some(cached) = self.cached_scene.as_mut() else {
            return false;
        };
        let hit_delta = Point::new(
            old_region.scroll_offset.x - next_offset.x,
            old_region.scroll_offset.y - next_offset.y,
        );
        cached.computed.scroll_regions[region_index].scroll_offset = next_offset;
        translate_descendant_scroll_regions(
            &descendant_scroll_ids,
            &mut cached.computed.scroll_regions,
            hit_delta,
        );
        // The retained GPU-scroll path updates hit rectangles in place. Drop the lazy spatial
        // candidate cache before translating so the next pointer query rebuilds from new bounds.
        cached.computed.invalidate_hit_test_index();
        translate_gpu_scroll_hits(&mut cached.computed.hit_regions, widget_id, hit_delta);
        for chunk in cached.scene_chunks.values_mut() {
            if let Some(region) = chunk
                .scroll_regions
                .iter_mut()
                .find(|region| region.id == widget_id)
            {
                region.scroll_offset = next_offset;
            }
            translate_descendant_scroll_regions(
                &descendant_scroll_ids,
                &mut chunk.scroll_regions,
                hit_delta,
            );
            translate_gpu_scroll_hits(&mut chunk.hit_regions, widget_id, hit_delta);
        }
        cached.scroll_epoch = self.scroll_epoch;
        cached.caret_visible = caret_visible;
        cached.active_scrollbar = active_scrollbar;
        cached.hovered_scrollbar = self.hovered_scrollbar;
        cached.gpu_scroll_deferred = true;
        self.scroll_dirty_widgets.clear();
        #[cfg(any(test, feature = "bench-support"))]
        scroll_fast_path_probe::record_gpu_hit();
        true
    }

    fn apply_scroll_view_controller_requests(
        &mut self,
        requests: SmallVec<[(WidgetId, ScrollRegion, ScrollViewController, ScrollRequest); 2]>,
    ) -> bool {
        let changed = !requests.is_empty();
        for (widget_id, region, controller, request) in requests {
            let max = region.max_offset();
            let target = Point::new(
                request.offset.x.clamp(Dp::ZERO, max.x),
                request.offset.y.clamp(Dp::ZERO, max.y),
            );
            match request.mode {
                ScrollRequestMode::Immediate => {
                    self.cancel_scroll_motion(widget_id);
                    self.set_scroll_offset(widget_id, target);
                }
                ScrollRequestMode::Smooth => {
                    self.set_smooth_scroll_target(widget_id, target);
                }
            }
            controller.clear_request(request);
        }

        changed
    }

    pub(super) fn rebuild_scroll_view_controller_bindings(&mut self) {
        let Some(cached) = self.cached_scene.as_mut() else {
            return;
        };
        cached.scroll_view_controller_bindings.clear();
        let Some(layout) = cached.layout.as_ref() else {
            return;
        };
        #[cfg(test)]
        scroll_view_binding_probe::record_rebuild_region_visits(
            cached.computed.scroll_regions.len(),
        );
        for (scroll_region_index, region) in cached.computed.scroll_regions.iter().enumerate() {
            let Some(controller) = layout.scroll_view_controller(region.id).cloned() else {
                continue;
            };
            cached.scroll_view_controller_bindings.push(
                crate::runtime::state::ScrollViewControllerBinding {
                    widget_id: region.id,
                    scroll_region_index,
                    controller,
                },
            );
        }
    }

    fn virtual_scroll_scene_roots(
        &self,
        layout: &crate::ui::widget::ResolvedSceneLayout<VM>,
        virtual_roots: &[WidgetId],
    ) -> SmallVec<[WidgetId; 16]> {
        let mut affected = HashSet::new();
        for root in virtual_roots.iter().copied() {
            let scene_root = layout
                .parent_of(root)
                .filter(|parent| {
                    layout
                        .resolved_widget(*parent)
                        .and_then(|resolved| resolved.data_grid_root.as_ref())
                        .is_some_and(|data_grid| data_grid.grid_id == *parent)
                })
                .unwrap_or(root);
            affected.insert(scene_root);
        }
        self.highest_layout_roots_smallvec(layout, &affected)
    }

    fn consume_scroll_view_requests_from_cached_scene(&mut self) -> bool {
        let cache_is_valid = self.cached_scene.as_ref().is_some_and(|cached| {
            cached
                .scroll_view_controller_bindings
                .iter()
                .all(|binding| {
                    cached
                        .computed
                        .scroll_regions
                        .get(binding.scroll_region_index)
                        .is_some_and(|region| region.id == binding.widget_id)
                })
        });
        if !cache_is_valid {
            #[cfg(test)]
            scroll_view_binding_probe::record_stale_rebuild();
            self.rebuild_scroll_view_controller_bindings();
        }

        let mut requests = SmallVec::new();
        let Some(cached) = self.cached_scene.as_ref() else {
            return false;
        };
        for binding in &cached.scroll_view_controller_bindings {
            let Some(region) = cached
                .computed
                .scroll_regions
                .get(binding.scroll_region_index)
                .copied()
            else {
                continue;
            };
            #[cfg(test)]
            scroll_view_binding_probe::record_consume_binding_visit();
            binding.controller.bind_widget(binding.widget_id);
            binding.controller.sync_offset(region.scroll_offset);
            if let Some(request) = binding.controller.take_request() {
                requests.push((
                    binding.widget_id,
                    region,
                    binding.controller.clone(),
                    request,
                ));
            }
        }
        self.apply_scroll_view_controller_requests(requests)
    }

    fn sync_virtual_state_update_list(
        &mut self,
        updates: &[crate::ui::widget::VirtualSceneStateUpdate],
    ) -> bool {
        let mut layout_invalidated = false;
        for update in updates {
            let state = self.virtual_states.entry(update.widget_id).or_default();
            let viewport_changed = state
                .viewport_hint
                .as_ref()
                .map(|previous| {
                    (previous.width - update.viewport_hint.width).abs()
                        > crate::ui::widget::MEASURED_EXTENT_INVALIDATION_EPSILON
                        || (previous.height - update.viewport_hint.height).abs()
                            > crate::ui::widget::MEASURED_EXTENT_INVALIDATION_EPSILON
                })
                .unwrap_or(true);
            layout_invalidated = layout_invalidated || viewport_changed;
            state.viewport_hint = Some(update.viewport_hint.clone());
            if let Some(signature) = update.measurement_signature {
                let prefix_changed = state
                    .measurements
                    .update_measurements(signature, &update.measured_extents);
                layout_invalidated =
                    layout_invalidated || (prefix_changed && update.invalidate_layout);
            }
            state.widget_ids_by_key = update.widget_ids_by_key.iter().cloned().collect();
        }
        layout_invalidated
    }

    pub(super) fn sync_virtual_state_updates(&mut self, computed: &ComputedScene<VM>) -> bool {
        self.sync_virtual_state_update_list(&computed.virtual_state_updates)
    }

    fn try_virtual_scroll_fast_path(
        &mut self,
        viewport: Rect,
        units: UnitContext,
        caret_visible: bool,
        active_scrollbar: Option<ScrollbarHandle>,
        now: Instant,
    ) -> bool {
        let Some(cached) = self.cached_scene.as_ref() else {
            return false;
        };
        if !cached.computed_valid
            || !cached.layout_valid
            || cached.scroll_epoch == self.scroll_epoch
            || !self.scene_cache_fields_match_ignoring_scroll(
                cached,
                viewport,
                units,
                caret_visible,
                active_scrollbar,
            )
        {
            return false;
        }
        let Some(layout) = cached.layout.as_ref() else {
            return false;
        };

        let mut affected = HashSet::new();
        for widget_id in self.scroll_dirty_widgets.iter().copied() {
            if !layout.is_virtual_widget(widget_id) {
                return false;
            }
            affected.insert(widget_id);
        }
        if affected.is_empty() {
            return false;
        }
        let roots = self.highest_layout_roots_smallvec(layout, &affected);
        if roots.is_empty() || roots.iter().any(|root| !layout.is_virtual_widget(*root)) {
            return false;
        }
        let roots = roots.into_iter().collect::<Vec<_>>();

        let scene_roots = self.virtual_scroll_scene_roots(layout, &roots);
        if scene_roots.is_empty() {
            return false;
        }

        if self.try_virtual_scroll_scene_fast_path(&roots, &scene_roots, now) {
            return true;
        }

        for _ in 0..=MAX_VIRTUAL_LAYOUT_FEEDBACK_PASSES {
            if !self.patch_cached_layout_for_roots_with_runtime_state(&roots, now) {
                return false;
            }
            if !self.patch_cached_scene_for_roots(&scene_roots, now, true) {
                return false;
            }

            let updates = self
                .cached_scene
                .as_ref()
                .map(|cached| cached.computed.virtual_state_updates.clone())
                .unwrap_or_default();
            if !self.sync_virtual_state_update_list(&updates) {
                if let Some(cached) = self.cached_scene.as_mut() {
                    cached.gpu_scroll_deferred = false;
                }
                self.scroll_dirty_widgets.clear();
                #[cfg(test)]
                scroll_fast_path_probe::record_virtual_hit();
                return true;
            }

            self.layout_animation_epoch = self.layout_animation_epoch.wrapping_add(1);
            if let Some(window) = self.window.as_ref() {
                window.request_redraw();
            }
        }

        if let Some(cached) = self.cached_scene.as_mut() {
            cached.layout_valid = false;
            cached.computed_valid = false;
        }
        false
    }

    fn try_virtual_scroll_scene_fast_path(
        &mut self,
        roots: &[WidgetId],
        scene_roots: &[WidgetId],
        now: Instant,
    ) -> bool {
        let scroll_states = self.scroll_states.clone();
        let virtual_states = self.virtual_states.clone();
        {
            let Some(cached) = self.cached_scene.as_mut() else {
                return false;
            };
            let Some(layout) = cached.layout.as_mut() else {
                return false;
            };
            if !layout.patch_virtual_scroll_offsets_if_window_stable(
                roots,
                &scroll_states,
                &virtual_states,
            ) {
                return false;
            }
        }

        if !self.patch_cached_scene_for_roots(scene_roots, now, true) {
            return false;
        }

        let updates = self
            .cached_scene
            .as_ref()
            .map(|cached| cached.computed.virtual_state_updates.clone())
            .unwrap_or_default();
        if self.sync_virtual_state_update_list(&updates) {
            if let Some(cached) = self.cached_scene.as_mut() {
                cached.layout_valid = false;
                cached.computed_valid = false;
            }
            self.layout_animation_epoch = self.layout_animation_epoch.wrapping_add(1);
            if let Some(window) = self.window.as_ref() {
                window.request_redraw();
            }
            return false;
        }

        if let Some(cached) = self.cached_scene.as_mut() {
            cached.gpu_scroll_deferred = false;
        }
        self.scroll_dirty_widgets.clear();
        #[cfg(test)]
        scroll_fast_path_probe::record_virtual_scene_hit();
        true
    }

    /// 纯滚动快路径。仅当本帧的缓存失配**只有** `scroll_epoch`（其余字段全部匹配、
    /// 布局缓存仍有效、滚动 root 子树不含 virtual）时尝试：把发生滚动的容器作为 patch 根，
    /// 复用既有且已测的 `patch_cached_scene_for_roots`（子树作用域重收集 + scene splice）
    /// 绕开整树重收集。
    ///
    /// 关键正确性：这里调用的 collect 与整帧重收集是**同一个收集函数**，只是作用域收窄到
    /// 滚动子树。`patch_resolved_roots` 会用最新 `scroll_states` 重新解析子树几何（含
    /// `child_origin = frame - scroll_offset` 平移与 `should_skip_fully_clipped_child` 裁剪），
    /// 因此结果与整帧重收集**逐项等价**——这不是「平移缓存图元」的近似，而是「只重收集受影响
    /// 子树」的精确等价。任一前置不满足 / patch 失败即返回 false，调用方落回整帧重收集。
    ///
    /// 嵌套滚动安全：若一次滚动同时脏了祖孙两个滚动容器，`highest_layout_roots_smallvec`
    /// 只取最高根，重收集其整棵子树（含内层滚动），不会漏更新内层。
    fn try_pure_scroll_fast_path(
        &mut self,
        viewport: Rect,
        units: UnitContext,
        caret_visible: bool,
        active_scrollbar: Option<ScrollbarHandle>,
        now: Instant,
    ) -> bool {
        let Some(cached) = self.cached_scene.as_ref() else {
            return false;
        };
        // 必须：缓存仍 computed_valid、layout_valid，且除 scroll_epoch 外一切匹配，
        // 且确实是 scroll_epoch 发生了变化（否则该路径无事可做，交给常规匹配判定）。
        if !cached.computed_valid || !cached.layout_valid {
            return false;
        }
        if cached.scroll_epoch == self.scroll_epoch {
            return false;
        }
        if !self.scene_cache_fields_match_ignoring_scroll(
            cached,
            viewport,
            units,
            caret_visible,
            active_scrollbar,
        ) {
            return false;
        }
        let Some(layout) = cached.layout.as_ref() else {
            return false;
        };
        // 收集本帧实际发生滚动、且仍在当前布局树中的脏容器。
        let roots = if self.scroll_dirty_widgets.len() == 1 {
            let Some(widget_id) = self.scroll_dirty_widgets.iter().next().copied() else {
                return false;
            };
            if layout.path_for(widget_id).is_none() {
                return false;
            }
            let mut roots = SmallVec::<[WidgetId; 16]>::new();
            roots.push(widget_id);
            roots
        } else {
            let mut affected: HashSet<WidgetId> = HashSet::new();
            for widget_id in self.scroll_dirty_widgets.iter().copied() {
                if layout.path_for(widget_id).is_some() {
                    affected.insert(widget_id);
                }
            }
            if affected.is_empty() {
                return false;
            }
            self.highest_layout_roots_smallvec(layout, &affected)
        };
        if roots.is_empty() {
            return false;
        }
        // virtual 容器滚动会改变窗口化 plan（结构性）；含 virtual 的滚动子树交给
        // virtual-scroll layout+scene patch 处理，普通纯滚动只覆盖稳定子树。
        if roots
            .iter()
            .any(|root| layout.subtree_contains_virtual(*root))
        {
            return false;
        }
        if !self.patch_cached_scene_for_roots(&roots, now, true) {
            // patch 失败：保持 cached 未被破坏（patch 在失败前不写 computed），落回整帧重收集。
            return false;
        }
        // patch 以 sync_runtime_scene_state=true 同步了 scroll_epoch 等运行时状态。
        if let Some(cached) = self.cached_scene.as_mut() {
            cached.gpu_scroll_deferred = false;
        }
        self.scroll_dirty_widgets.clear();
        #[cfg(any(test, feature = "bench-support"))]
        scroll_fast_path_probe::record_patch_hit();
        true
    }

    pub(in crate::runtime) fn computed_scene_mut(&mut self) -> &mut ComputedScene<VM> {
        let _ = self.computed_scene();
        &mut self
            .cached_scene
            .as_mut()
            .expect("computed_scene should populate cached scene")
            .computed
    }

    pub(in crate::runtime) fn computed_scene(&mut self) -> &ComputedScene<VM> {
        let _ = self.observe_root_rebuild_request();
        loop {
            with_runtime_scene_stack(|| {
                let _ = self.computed_scene_with_virtual_feedback(0);
            });
            if !self.reconcile_accessibility_focus_after_scene_update() {
                break;
            }
            self.invalidate_computed_scene();
        }
        &self
            .cached_scene
            .as_ref()
            .expect("computed_scene should populate cached scene")
            .computed
    }

    fn try_toast_prepared_card_fast_path(
        &mut self,
        viewport: Rect,
        units: UnitContext,
        caret_visible: bool,
        active_scrollbar: Option<ScrollbarHandle>,
        now: Instant,
    ) -> bool {
        if !std::mem::take(&mut self.toast_motion_patch_pending) {
            return false;
        }
        let roots = {
            let Some(cached) = self.cached_scene.as_ref() else {
                return false;
            };
            if cached.computed_valid
                || !cached.layout_valid
                || cached.gpu_scroll_deferred
                || cached.scroll_epoch != self.scroll_epoch
                || cached.accessibility_animation_epoch != self.accessibility_animation_epoch
                || !self.scene_cache_fields_match_ignoring_scroll(
                    cached,
                    viewport,
                    units,
                    caret_visible,
                    active_scrollbar,
                )
            {
                return false;
            }
            let Some(layout) = cached.layout.as_ref() else {
                return false;
            };
            layout
                .all_widget_ids()
                .filter(|widget_id| {
                    layout.resolved_widget(*widget_id).is_some_and(|widget| {
                        matches!(
                            widget.kind,
                            crate::ui::widget::ResolvedWidgetKind::ToastHost { .. }
                        )
                    })
                })
                .collect::<SmallVec<[WidgetId; 4]>>()
        };
        if roots.is_empty() {
            return false;
        }

        let patched = crate::ui::widget::with_prepared_toast_card_cache(|| {
            crate::ui::widget::with_toast_base_scene_replay(|| {
                self.patch_cached_scene_for_roots(&roots, now, false)
            })
        });
        if patched {
            super::action_stats::record("toast_prepared_card_scene_patch");
        }
        patched
    }

    /// Recollect only the old and new focus subtrees when keyboard focus changes between ordinary
    /// controls.  Focus metadata/layout is immutable for this path; the retained collector still
    /// performs all normal scene-count and hit-region checks and returns `false` on any uncertain
    /// prerequisite, so the caller cleanly falls back to a full recollect.
    fn try_retained_focus_fast_path(
        &mut self,
        viewport: Rect,
        units: UnitContext,
        caret_visible: bool,
        active_scrollbar: Option<ScrollbarHandle>,
        now: Instant,
    ) -> bool {
        let current_focus = self.focused_widget_id();
        let roots = {
            let Some(cached) = self.cached_scene.as_ref() else {
                return false;
            };
            let focus_changed = cached.focused_widget != current_focus
                || cached.focus_visible != self.focus_visible;
            if !focus_changed
                || !cached.computed_valid
                || !cached.layout_valid
                || cached.gpu_scroll_deferred
                || self.invalidation.revision() != self.last_invalidation_revision
                || self.invalidation.root_rebuild_revision() != self.last_root_rebuild_revision
                || self.animation_engine.has_active_animations()
                || self.next_tooltip_wakeup_deadline.is_some()
                || self.next_toast_wakeup_deadline.is_some()
                || self.active_gesture.is_some()
                || self.active_pinch.is_some()
                || self.active_scrollbar_drag.is_some()
                || self.active_touch_scroll.is_some()
                || self.active_slider_drag.is_some()
                || self.active_canvas_drag.is_some()
                || self.active_tab_reorder.is_some()
                || self.active_tree_drag.is_some()
                || self.active_data_grid_column_resize.is_some()
                || self.active_splitter_resize.is_some()
                || self.active_data_grid_column_reorder.is_some()
                || self.active_text_selection.is_some()
                || self.pending_click.is_some()
                || self.deferred_mouse_click.is_some()
                || !self.lifecycle_event_states.is_empty()
                || !self.media_event_states.is_empty()
                || !self.external_portal_requests.is_empty()
                || !self.scene_cache_fields_match_ignoring_focus(
                    cached,
                    viewport,
                    units,
                    caret_visible,
                    active_scrollbar,
                )
            {
                return false;
            }

            // A missing snapshot cannot prove that a target is not a text input.  Keep the fast
            // path strictly conservative in that case.
            let is_ordinary = |widget_id: Option<WidgetId>| match widget_id {
                None => true,
                Some(widget_id) => self.cached_focus_target_is_text_input(widget_id) == Some(false),
            };
            if !is_ordinary(cached.focused_widget) || !is_ordinary(current_focus) {
                return false;
            }

            let Some(layout) = cached.layout.as_ref() else {
                return false;
            };
            if layout.contains_virtual() {
                return false;
            }
            let mut roots = SmallVec::<[WidgetId; 2]>::new();
            for widget_id in [cached.focused_widget, current_focus].into_iter().flatten() {
                if roots.contains(&widget_id)
                    || layout.path_for(widget_id).is_none()
                    || !cached.visual_contexts.contains_key(&widget_id)
                {
                    return false;
                }
                roots.push(widget_id);
            }
            roots
        };
        if roots.is_empty() {
            return false;
        }

        if !self.patch_cached_focus_ring_scene_for_roots(&roots, now, true)
            && !self.patch_cached_scene_for_roots(&roots, now, true)
        {
            return false;
        }

        // The focus metadata itself was checked through the snapshot and is unchanged by this
        // paint-only patch.  Retargeting avoids a full hit-stream validation on the next Tab.
        self.retarget_focus_navigation_cache_to_current_scene();
        super::action_stats::record("focus_scene_patch");
        true
    }

    fn try_retained_row_hover_fast_path(
        &mut self,
        viewport: Rect,
        units: UnitContext,
        caret_visible: bool,
        active_scrollbar: Option<ScrollbarHandle>,
        now: Instant,
    ) -> bool {
        let Some(pending) = self.row_hover_patch_pending.take() else {
            return false;
        };
        if self.invalidation.revision() != pending.source_invalidation_revision
            || self.last_invalidation_revision != pending.source_invalidation_revision
            || self.invalidation.root_rebuild_revision() != pending.source_root_rebuild_revision
            || self.last_root_rebuild_revision != pending.source_root_rebuild_revision
            || self.animation_engine.has_active_animations()
            || self.next_tooltip_wakeup_deadline.is_some()
            || self.next_toast_wakeup_deadline.is_some()
            || self.hover_epoch != pending.source_hover_epoch.wrapping_add(1)
        {
            return false;
        }
        let roots = {
            let Some(cached) = self.cached_scene.as_ref() else {
                return false;
            };
            if !cached.computed_valid
                || !cached.layout_valid
                || cached.gpu_scroll_deferred
                || cached.hover_epoch != pending.source_hover_epoch
                || cached.scroll_epoch != self.scroll_epoch
                || !self.scene_cache_fields_match_ignoring_scroll_and_hover(
                    cached,
                    viewport,
                    units,
                    caret_visible,
                    active_scrollbar,
                )
            {
                return false;
            }
            let Some(layout) = cached.layout.as_ref() else {
                return false;
            };
            let mut roots = SmallVec::<[WidgetId; 2]>::new();
            for (row, kind) in [pending.previous_row, pending.next_row]
                .into_iter()
                .flatten()
            {
                if roots.contains(&row)
                    || !layout
                        .resolved_widget(row)
                        .is_some_and(|widget| widget.retained_hover_row_kind() == Some(kind))
                {
                    return false;
                }
                roots.push(row);
            }
            roots
        };
        if roots.is_empty() || !self.patch_cached_scene_for_roots(&roots, now, true) {
            return false;
        }
        #[cfg(test)]
        row_hover_patch_probe::record_hit();
        super::action_stats::record("row_hover_scene_patch");
        true
    }

    fn try_retained_button_hover_fast_path(
        &mut self,
        viewport: Rect,
        units: UnitContext,
        caret_visible: bool,
        active_scrollbar: Option<ScrollbarHandle>,
        now: Instant,
    ) -> bool {
        let Some(pending) = self.button_hover_patch_pending.take() else {
            return false;
        };
        if !self.button_hover_runtime_is_idle()
            || self.invalidation.revision() != pending.source_invalidation_revision
            || self.last_invalidation_revision != pending.source_invalidation_revision
            || self.invalidation.root_rebuild_revision() != pending.source_root_rebuild_revision
            || self.last_root_rebuild_revision != pending.source_root_rebuild_revision
            || self.hover_epoch != pending.source_hover_epoch.wrapping_add(1)
        {
            super::action_stats::record("button_hover_patch_reject_guard");
            return false;
        }
        let roots = {
            let Some(cached) = self.cached_scene.as_ref() else {
                return false;
            };
            if !cached.computed_valid
                || !cached.layout_valid
                || cached.gpu_scroll_deferred
                || cached.hover_epoch != pending.source_hover_epoch
                || cached.scroll_epoch != self.scroll_epoch
                || cached.accessibility_animation_epoch != self.accessibility_animation_epoch
                || !cached.computed.is_simple_for_button_hover_recompose()
                || !cached.lifecycle_states.is_empty()
                || !cached.media_texture_bindings.is_empty()
                || !self.media_event_states.is_empty()
                || !self.external_portal_requests.is_empty()
                || !Self::button_hover_path_is_passive(&self.hovered_widgets)
                || !self.scene_cache_fields_match_ignoring_scroll_and_hover(
                    cached,
                    viewport,
                    units,
                    caret_visible,
                    active_scrollbar,
                )
            {
                super::action_stats::record("button_hover_patch_reject_cache");
                return false;
            }
            let Some(layout) = cached.layout.as_ref() else {
                super::action_stats::record("button_hover_patch_reject_layout");
                return false;
            };
            if layout.contains_virtual() {
                super::action_stats::record("button_hover_patch_reject_virtual");
                return false;
            }

            let mut current_button = None;
            for hovered in &self.hovered_widgets {
                let HoverTargetId::Widget(id) = hovered.target_id else {
                    continue;
                };
                if matches!(
                    layout.resolved_widget(id).map(|widget| &widget.kind),
                    Some(crate::ui::widget::ResolvedWidgetKind::Button { .. })
                ) && current_button.replace(id).is_some()
                {
                    super::action_stats::record("button_hover_patch_reject_multiple_buttons");
                    return false;
                }
            }
            if current_button != pending.next_button
                || pending.previous_button == pending.next_button
                || (pending.previous_button.is_none() && pending.next_button.is_none())
            {
                super::action_stats::record("button_hover_patch_reject_target");
                return false;
            }

            let mut roots = SmallVec::<[WidgetId; 2]>::new();
            for id in [pending.previous_button, pending.next_button]
                .into_iter()
                .flatten()
            {
                if roots.contains(&id)
                    || !Self::is_simple_button_hover_root(layout, id)
                    || !cached
                        .scene_chunks
                        .get(&id)
                        .is_some_and(|chunk| chunk.is_simple_for_button_hover_recompose())
                    || !cached.visual_contexts.contains_key(&id)
                {
                    super::action_stats::record("button_hover_patch_reject_root");
                    return false;
                }
                roots.push(id);
            }
            roots
        };
        if roots.is_empty() {
            super::action_stats::record("button_hover_patch_reject_empty");
            return false;
        }
        if !self.patch_cached_scene_for_roots(&roots, now, true) {
            super::action_stats::record("button_hover_patch_reject_patch");
            return false;
        }
        #[cfg(test)]
        button_hover_patch_probe::record_hit();
        super::action_stats::record("button_hover_scene_patch");
        true
    }

    fn try_retained_button_pressed_fast_path(
        &mut self,
        viewport: Rect,
        units: UnitContext,
        caret_visible: bool,
        active_scrollbar: Option<ScrollbarHandle>,
        now: Instant,
    ) -> bool {
        let Some(pending) = self.button_pressed_patch_pending.take() else {
            return false;
        };
        if !self.button_visual_runtime_is_idle_ignoring_pressed()
            || self.invalidation.revision() != pending.source_invalidation_revision
            || self.last_invalidation_revision != pending.source_invalidation_revision
            || self.invalidation.root_rebuild_revision() != pending.source_root_rebuild_revision
            || self.last_root_rebuild_revision != pending.source_root_rebuild_revision
            || self.hover_epoch != pending.source_hover_epoch
            || self.pressed_widget != pending.next_pressed_widget
        {
            super::action_stats::record("button_pressed_patch_reject_guard");
            return false;
        }
        {
            let Some(cached) = self.cached_scene.as_ref() else {
                return false;
            };
            if !cached.computed_valid
                || !cached.layout_valid
                || cached.gpu_scroll_deferred
                || cached.pressed_widget != pending.source_pressed_widget
                || cached.hover_epoch != pending.source_hover_epoch
                || cached.scroll_epoch != self.scroll_epoch
                || cached.accessibility_animation_epoch != self.accessibility_animation_epoch
                || !cached.computed.is_simple_for_button_hover_recompose()
                || !cached.lifecycle_states.is_empty()
                || !cached.media_texture_bindings.is_empty()
                || !self.media_event_states.is_empty()
                || !self.external_portal_requests.is_empty()
                || !self.scene_cache_fields_match_ignoring_scroll_hover_and_pressed(
                    cached,
                    viewport,
                    units,
                    caret_visible,
                    active_scrollbar,
                )
            {
                super::action_stats::record("button_pressed_patch_reject_cache");
                return false;
            }
            let Some(layout) = cached.layout.as_ref() else {
                return false;
            };
            if layout.contains_virtual()
                || self.hovered_simple_button(layout) != Some(pending.button)
                || !Self::is_simple_button_pressed_root(layout, pending.button)
                || !cached
                    .scene_chunks
                    .get(&pending.button)
                    .is_some_and(|chunk| chunk.is_simple_for_button_hover_recompose())
                || !cached.visual_contexts.contains_key(&pending.button)
            {
                super::action_stats::record("button_pressed_patch_reject_root");
                return false;
            }
        }
        if !self.patch_cached_scene_for_roots(&[pending.button], now, true) {
            super::action_stats::record("button_pressed_patch_reject_patch");
            return false;
        }
        #[cfg(test)]
        button_pressed_patch_probe::record_hit();
        super::action_stats::record("button_pressed_scene_patch");
        true
    }

    fn computed_scene_with_virtual_feedback(
        &mut self,
        virtual_feedback_pass: usize,
    ) -> &ComputedScene<VM> {
        let started_at = text_profile_enabled().then_some(Instant::now());
        let viewport = self.viewport_rect();
        let units = self.unit_context();
        let now = Instant::now();
        if self.cached_scene.is_some() {
            if self.consume_scroll_view_requests_from_cached_scene() {
                self.invalidate_scene_with_reason("scroll_view_controller_request");
                self.invalidation.mark_dirty();
            }
        }
        let focused_widget = self.focused_widget_id();
        let active_scrollbar = self.active_scrollbar_drag.map(|drag| drag.handle);
        let (
            cache_valid,
            layout_cache_valid,
            focused_input,
            focused_text_state,
            caret_visible,
            cache_mismatch,
        ) = if self.cached_scene.is_some() {
            let focused_input = self
                .cached_scene
                .as_ref()
                .and_then(|cached| self.focused_text_input_id_cached(&cached.computed));
            let focused_text_state = focused_input
                .and_then(|id| self.text_edit_state(id))
                .cloned();
            let caret_visible = self.caret_visible_at(now, focused_input);
            let _ = self.try_update_focused_text_input_slots(
                now,
                viewport,
                units,
                caret_visible,
                active_scrollbar,
            );
            let _ = self.try_update_caret_visibility_slot(caret_visible);
            let cached = self
                .cached_scene
                .as_ref()
                .expect("cached scene should remain available after caret slot update");
            let cache_mismatch = started_at.map(|_| {
                self.scene_cache_mismatch_summary(
                    cached,
                    viewport,
                    units,
                    caret_visible,
                    active_scrollbar,
                )
            });
            (
                self.scene_cache_matches(cached, viewport, units, caret_visible, active_scrollbar),
                self.scene_layout_cache_matches(cached, viewport, units),
                focused_input,
                focused_text_state,
                caret_visible,
                cache_mismatch,
            )
        } else {
            (
                false,
                false,
                None,
                None,
                false,
                started_at.map(|_| "no_cached_scene".to_string()),
            )
        };
        let can_retarget_focus_navigation = self.cached_scene.as_ref().is_some_and(|cached| {
            self.can_retarget_focus_navigation_after_focus_paint_recollect(
                cached,
                viewport,
                units,
                caret_visible,
                active_scrollbar,
            )
        });
        if !cache_valid
            && layout_cache_valid
            && self.try_retained_focus_fast_path(
                viewport,
                units,
                caret_visible,
                active_scrollbar,
                now,
            )
        {
            return &self
                .cached_scene
                .as_ref()
                .expect("focus patch should preserve cached scene")
                .computed;
        }
        if self.try_toast_prepared_card_fast_path(
            viewport,
            units,
            caret_visible,
            active_scrollbar,
            now,
        ) {
            return &self
                .cached_scene
                .as_ref()
                .expect("Toast prepared-card patch should preserve cached scene")
                .computed;
        }
        if !cache_valid
            && layout_cache_valid
            && self.try_retained_row_hover_fast_path(
                viewport,
                units,
                caret_visible,
                active_scrollbar,
                now,
            )
        {
            return &self
                .cached_scene
                .as_ref()
                .expect("DataGrid hover patch should preserve cached scene")
                .computed;
        }
        if !cache_valid
            && layout_cache_valid
            && self.try_retained_button_hover_fast_path(
                viewport,
                units,
                caret_visible,
                active_scrollbar,
                now,
            )
        {
            return &self
                .cached_scene
                .as_ref()
                .expect("Button hover patch should preserve cached scene")
                .computed;
        }
        if !cache_valid
            && layout_cache_valid
            && self.try_retained_button_pressed_fast_path(
                viewport,
                units,
                caret_visible,
                active_scrollbar,
                now,
            )
        {
            return &self
                .cached_scene
                .as_ref()
                .expect("Button pressed patch should preserve cached scene")
                .computed;
        }
        let selected_text_state = self
            .selected_text
            .and_then(|id| self.text_edit_state(id))
            .cloned();
        let active_tooltip = self.resolve_active_tooltip(now);
        let active_hover_popover = self.resolve_active_hover_popover();

        let text_input_patch_roots = self.cached_scene.as_ref().and_then(|cached| {
            (layout_cache_valid
                && !cache_valid
                && !cached.gpu_scroll_deferred
                && !self.strict_reactive_tree()
                && self.can_patch_text_input_scene(
                    cached,
                    viewport,
                    units,
                    caret_visible,
                    active_scrollbar,
                ))
            .then(|| Self::visible_text_input_roots_from_computed(&cached.computed))
            .filter(|roots| !roots.is_empty())
        });

        if let Some(roots) = text_input_patch_roots {
            if self.patch_cached_scene_for_roots(&roots, now, true) {
                super::action_stats::record("text_input_scene_patch");
                if let Some(started_at) = started_at {
                    log_text_profile(
                        "textarea_computed_scene",
                        started_at.elapsed(),
                        format!(
                            "path=text_input_patch roots={} cache_valid={} layout_cache_valid={} cache_mismatch={}",
                            roots.len(),
                            cache_valid,
                            layout_cache_valid,
                            cache_mismatch.as_deref().unwrap_or("not_profiled")
                        ),
                    );
                }
                return &self
                    .cached_scene
                    .as_ref()
                    .expect("text input scene patch should preserve cached scene")
                    .computed;
            }
        }

        // 纯滚动快路径：仅 scroll_epoch 变化时,只重收集滚动子树而非整树。
        // 命中即直接返回已更新的 cached.computed；不命中(前置不满足/patch 失败)
        // 落回下方常规 `!cache_valid` 整帧重收集,行为与未开启特性时完全一致。
        if !cache_valid
            && layout_cache_valid
            && self.try_pure_scroll_gpu_fast_path(viewport, units, caret_visible, active_scrollbar)
        {
            if let Some(started_at) = started_at {
                log_text_profile(
                    "textarea_computed_scene",
                    started_at.elapsed(),
                    format!(
                        "path=pure_scroll_gpu cache_valid={} layout_cache_valid={} cache_mismatch={}",
                        cache_valid,
                        layout_cache_valid,
                        cache_mismatch.as_deref().unwrap_or("not_profiled")
                    ),
                );
            }
            return &self
                .cached_scene
                .as_ref()
                .expect("pure scroll gpu fast path should preserve cached scene")
                .computed;
        }

        if !cache_valid
            && layout_cache_valid
            && self.try_pure_scroll_fast_path(viewport, units, caret_visible, active_scrollbar, now)
        {
            if let Some(started_at) = started_at {
                log_text_profile(
                    "textarea_computed_scene",
                    started_at.elapsed(),
                    format!(
                        "path=pure_scroll_patch cache_valid={} layout_cache_valid={} cache_mismatch={}",
                        cache_valid,
                        layout_cache_valid,
                        cache_mismatch.as_deref().unwrap_or("not_profiled")
                    ),
                );
            }
            return &self
                .cached_scene
                .as_ref()
                .expect("pure scroll patch should preserve cached scene")
                .computed;
        }

        if !cache_valid
            && self.try_virtual_scroll_fast_path(
                viewport,
                units,
                caret_visible,
                active_scrollbar,
                now,
            )
        {
            if let Some(started_at) = started_at {
                log_text_profile(
                    "textarea_computed_scene",
                    started_at.elapsed(),
                    format!(
                        "path=virtual_scroll_patch cache_valid={} layout_cache_valid={} cache_mismatch={}",
                        cache_valid,
                        layout_cache_valid,
                        cache_mismatch.as_deref().unwrap_or("not_profiled")
                    ),
                );
            }
            return &self
                .cached_scene
                .as_ref()
                .expect("virtual scroll patch should preserve cached scene")
                .computed;
        }

        #[cfg(any(test, feature = "bench-support"))]
        if cache_valid {
            frame_path_probe::record_cache_hit();
        }

        let widget_states = self.widget_state_map(active_scrollbar);
        if !cache_valid {
            #[cfg(any(test, feature = "bench-support"))]
            frame_path_probe::record_scene_recollect(layout_cache_valid);
            let mut layout_duration = Duration::ZERO;
            let mut collect_duration = Duration::ZERO;
            let mut recollect_duration = Duration::ZERO;
            let mut collect_passes = 0usize;
            let previous_cached = self.cached_scene.take();
            let previous_owner_ids = previous_cached
                .as_ref()
                .and_then(|cached| cached.layout.as_ref())
                .map(|layout| {
                    layout
                        .all_widget_ids()
                        .map(|widget_id| widget_id.raw())
                        .collect::<HashSet<_>>()
                })
                .unwrap_or_default();
            if layout_cache_valid {
                self.invalidation.remove_reactive_targets_for_widget_phase(
                    &previous_owner_ids,
                    DependencyPhase::Scene,
                );
            } else {
                self.invalidation
                    .remove_reactive_targets_for_widgets(&previous_owner_ids);
            }
            let theme = self.animated_theme(Instant::now());
            let (layout, collected) = match self.widget_tree.as_ref() {
                Some(tree) => {
                    if layout_cache_valid {
                        let layout = {
                            let cached = previous_cached
                                .as_ref()
                                .expect("layout cache should exist when layout cache is valid");
                            cached
                                .layout
                                .as_ref()
                                .expect("layout should exist when layout cache is valid")
                        };
                        let (focused_text_value, focused_text_layout) =
                            Self::focused_text_overrides(
                                &self.text_input_buffers,
                                focused_input,
                                focused_text_state.as_ref(),
                            );
                        let text_layout_overrides =
                            Self::stable_text_layout_overrides(&self.text_input_buffers);
                        let mut collected = {
                            let collect_started_at = Instant::now();
                            let active_slider_value = self.active_slider_value_override();
                            let gpu_scroll_enabled =
                                self.gpu_scroll_supported && !layout.contains_virtual();
                            let collected = tree
                                .collect_scene_cache_from_layout_with_focus_value_virtual_and_menu_state(
                                    &self.font_manager,
                                    layout,
                                    &theme,
                                    &self.media_manager,
                                    &mut self.animation_engine,
                                    self.reduced_motion,
                                    self.hovered_scrollbar,
                                    active_scrollbar,
                                    &widget_states,
                                    &self.select_open_states,
                                    &self.menu_open_states,
                                    &self.menubar_active_states,
                                    &self.context_menu_anchor_states,
                                    &self.scroll_states,
                                    &self.virtual_states,
                                    viewport,
                                    focused_input,
                                    focused_text_state.as_ref(),
                                    focused_text_value,
                                    focused_text_layout,
                                    Some(&text_layout_overrides),
                                    active_slider_value,
                                    self.selected_text,
                                    selected_text_state.as_ref(),
                                    caret_visible,
                                    &self.tooltip_hover_started_at,
                                    active_tooltip,
                                    active_hover_popover,
                                    gpu_scroll_enabled,
                                    &self.config.style_sheet,
                                );
                            collect_duration += collect_started_at.elapsed();
                            collect_passes += 1;
                            collected
                        };
                        let actual_focused_input =
                            self.focused_text_input_id_cached(&collected.computed);
                        let actual_focused_text_state = actual_focused_input
                            .and_then(|id| self.text_edit_state(id))
                            .cloned();
                        let actual_caret_visible = self.caret_visible_at(now, actual_focused_input);
                        if actual_focused_input != focused_input
                            || actual_caret_visible != caret_visible
                        {
                            let (actual_focused_text_value, actual_focused_text_layout) =
                                Self::focused_text_overrides(
                                    &self.text_input_buffers,
                                    actual_focused_input,
                                    actual_focused_text_state.as_ref(),
                                );
                            let text_layout_overrides =
                                Self::stable_text_layout_overrides(&self.text_input_buffers);
                            collected = {
                                let collect_started_at = Instant::now();
                                let active_slider_value = self.active_slider_value_override();
                                let gpu_scroll_enabled =
                                    self.gpu_scroll_supported && !layout.contains_virtual();
                                let collected = tree
                                    .collect_scene_cache_from_layout_with_focus_value_virtual_and_menu_state(
                                        &self.font_manager,
                                        layout,
                                        &theme,
                                        &self.media_manager,
                                        &mut self.animation_engine,
                                        self.reduced_motion,
                                        self.hovered_scrollbar,
                                        active_scrollbar,
                                        &widget_states,
                                        &self.select_open_states,
                                        &self.menu_open_states,
                                        &self.menubar_active_states,
                                        &self.context_menu_anchor_states,
                                        &self.scroll_states,
                                        &self.virtual_states,
                                        viewport,
                                        actual_focused_input,
                                        actual_focused_text_state.as_ref(),
                                        actual_focused_text_value,
                                        actual_focused_text_layout,
                                        Some(&text_layout_overrides),
                                        active_slider_value,
                                        self.selected_text,
                                        selected_text_state.as_ref(),
                                        actual_caret_visible,
                                        &self.tooltip_hover_started_at,
                                        active_tooltip,
                                        active_hover_popover,
                                        gpu_scroll_enabled,
                                        &self.config.style_sheet,
                                    );
                                recollect_duration += collect_started_at.elapsed();
                                collect_passes += 1;
                                collected
                            };
                        }
                        let layout = previous_cached.and_then(|cached| cached.layout);
                        (layout, collected)
                    } else {
                        let layout = {
                            let layout_started_at = Instant::now();
                            let previous_layout = previous_cached
                                .as_ref()
                                .and_then(|cached| cached.layout.as_ref());
                            let mut layout = tree
                                .build_scene_layout_at_with_previous_style_sheet_and_reduced_motion(
                                    &self.font_manager,
                                    &theme,
                                    &self.media_manager,
                                    &mut self.animation_engine,
                                    units,
                                    &self.scroll_states,
                                    &self.virtual_states,
                                    viewport,
                                    now,
                                    previous_layout,
                                    self.reduced_motion,
                                    &self.config.style_sheet,
                                );
                            layout.set_frame_clock(self.frame_clock.snapshot());
                            layout_duration += layout_started_at.elapsed();
                            layout
                        };
                        let (focused_text_value, focused_text_layout) =
                            Self::focused_text_overrides(
                                &self.text_input_buffers,
                                focused_input,
                                focused_text_state.as_ref(),
                            );
                        let text_layout_overrides =
                            Self::stable_text_layout_overrides(&self.text_input_buffers);
                        let collected = {
                            let collect_started_at = Instant::now();
                            let active_slider_value = self.active_slider_value_override();
                            let gpu_scroll_enabled =
                                self.gpu_scroll_supported && !layout.contains_virtual();
                            let collected = tree
                                .collect_scene_cache_from_layout_with_focus_value_virtual_and_menu_state(
                                    &self.font_manager,
                                    &layout,
                                    &theme,
                                    &self.media_manager,
                                    &mut self.animation_engine,
                                    self.reduced_motion,
                                    self.hovered_scrollbar,
                                    active_scrollbar,
                                    &widget_states,
                                    &self.select_open_states,
                                    &self.menu_open_states,
                                    &self.menubar_active_states,
                                    &self.context_menu_anchor_states,
                                    &self.scroll_states,
                                    &self.virtual_states,
                                    viewport,
                                    focused_input,
                                    focused_text_state.as_ref(),
                                    focused_text_value,
                                    focused_text_layout,
                                    Some(&text_layout_overrides),
                                    active_slider_value,
                                    self.selected_text,
                                    selected_text_state.as_ref(),
                                    caret_visible,
                                    &self.tooltip_hover_started_at,
                                    active_tooltip,
                                    active_hover_popover,
                                    gpu_scroll_enabled,
                                    &self.config.style_sheet,
                                );
                            collect_duration += collect_started_at.elapsed();
                            collect_passes += 1;
                            collected
                        };
                        let actual_focused_input =
                            self.focused_text_input_id_cached(&collected.computed);
                        let actual_focused_text_state = actual_focused_input
                            .and_then(|id| self.text_edit_state(id))
                            .cloned();
                        let actual_caret_visible = self.caret_visible_at(now, actual_focused_input);
                        let collected = if actual_focused_input != focused_input
                            || actual_caret_visible != caret_visible
                        {
                            let (actual_focused_text_value, actual_focused_text_layout) =
                                Self::focused_text_overrides(
                                    &self.text_input_buffers,
                                    actual_focused_input,
                                    actual_focused_text_state.as_ref(),
                                );
                            let text_layout_overrides =
                                Self::stable_text_layout_overrides(&self.text_input_buffers);
                            let collect_started_at = Instant::now();
                            let active_slider_value = self.active_slider_value_override();
                            let gpu_scroll_enabled =
                                self.gpu_scroll_supported && !layout.contains_virtual();
                            let collected = tree
                                .collect_scene_cache_from_layout_with_focus_value_virtual_and_menu_state(
                                    &self.font_manager,
                                    &layout,
                                    &theme,
                                    &self.media_manager,
                                    &mut self.animation_engine,
                                    self.reduced_motion,
                                    self.hovered_scrollbar,
                                    active_scrollbar,
                                    &widget_states,
                                    &self.select_open_states,
                                    &self.menu_open_states,
                                    &self.menubar_active_states,
                                    &self.context_menu_anchor_states,
                                    &self.scroll_states,
                                    &self.virtual_states,
                                    viewport,
                                    actual_focused_input,
                                    actual_focused_text_state.as_ref(),
                                    actual_focused_text_value,
                                    actual_focused_text_layout,
                                    Some(&text_layout_overrides),
                                    active_slider_value,
                                    self.selected_text,
                                    selected_text_state.as_ref(),
                                    actual_caret_visible,
                                    &self.tooltip_hover_started_at,
                                    active_tooltip,
                                    active_hover_popover,
                                    gpu_scroll_enabled,
                                    &self.config.style_sheet,
                                );
                            recollect_duration += collect_started_at.elapsed();
                            collect_passes += 1;
                            collected
                        } else {
                            collected
                        };
                        (Some(layout), collected)
                    }
                }
                None => (
                    None,
                    CollectedSceneCache {
                        computed: ComputedScene::default(),
                        lifecycle_states: HashMap::new(),
                        chunks: HashMap::new(),
                        chunk_parts: HashMap::new(),
                        visual_contexts: HashMap::new(),
                        dependencies: DependencyGraph::default(),
                        next_tooltip_wakeup: None,
                        next_toast_wakeup: None,
                    },
                ),
            };
            self.next_tooltip_wakeup_deadline = collected.next_tooltip_wakeup;
            self.next_toast_wakeup_deadline = collected.next_toast_wakeup;
            // `collected.computed` 此后不再被单独使用(只有这份工作副本会被存入
            // CachedScene),因此直接移动而非克隆整棵根场景 —— 省掉每次场景重建一次
            // 的全场景深拷贝。`collected` 的其余字段在下方按字段分别 move 进 CachedScene,
            // 与这里的部分 move 互不冲突。
            let mut computed = collected.computed;
            self.append_external_portals_to_computed(&mut computed, &widget_states, now);
            computed.assign_new_prepare_cache_serial();
            let virtual_layout_invalidated = self.sync_virtual_state_updates(&computed);
            if virtual_layout_invalidated {
                self.layout_animation_epoch = self.layout_animation_epoch.wrapping_add(1);
                if let Some(window) = self.window.as_ref() {
                    window.request_redraw();
                }
            }
            let focused_input = self.focused_text_input_id_cached(&computed);
            let caret_visible = self.caret_visible_at(now, focused_input);
            self.sync_text_inputs_from_computed(&computed);
            self.cached_scene = Some(Box::new(CachedScene {
                viewport,
                units,
                focused_widget,
                focus_visible: self.focus_visible,
                pressed_widget: self.pressed_widget,
                selected_text: self.selected_text,
                caret_visible,
                theme_epoch: self.theme_store.version(),
                style_sheet_version: self.config.style_sheet.version(),
                density: self.theme.density,
                reduced_motion: self.reduced_motion,
                text_scale_bits: units.font_scale().to_bits(),
                animation_epoch: self.animation_epoch,
                layout_animation_epoch: self.layout_animation_epoch,
                accessibility_animation_epoch: self.accessibility_animation_epoch,
                scroll_epoch: self.scroll_epoch,
                hover_epoch: self.hover_epoch,
                text_input_epoch: self.text_input_epoch,
                external_portal_revision: self.external_portal_revision,
                hovered_scrollbar: self.hovered_scrollbar,
                active_scrollbar,
                layout_valid: true,
                computed_valid: true,
                gpu_scroll_deferred: false,
                dependencies: {
                    let mut dependencies = DependencyGraph::default();
                    if let Some(layout) = layout.as_ref() {
                        dependencies.merge_from(layout.dependencies());
                    }
                    dependencies.merge_from(&computed.dependencies);
                    dependencies
                },
                layout,
                computed,
                lifecycle_states: collected.lifecycle_states,
                scene_chunks: collected.chunks,
                scene_chunk_parts: collected.chunk_parts,
                visual_contexts: collected.visual_contexts,
                layout_slot_bindings: HashMap::new(),
                reactive_slot_bindings: HashMap::new(),
                media_texture_bindings: HashMap::new(),
                media_texture_binding_index: HashMap::new(),
                caret_decoration: None,
                text_input_slot_bindings: HashMap::new(),
                scroll_view_controller_bindings: Vec::new(),
                strict_capability_report: None,
            }));
            if virtual_layout_invalidated {
                if let Some(cached) = self.cached_scene.as_mut() {
                    cached.layout_valid = false;
                    cached.computed_valid = false;
                }
                if virtual_feedback_pass < MAX_VIRTUAL_LAYOUT_FEEDBACK_PASSES {
                    return self.computed_scene_with_virtual_feedback(virtual_feedback_pass + 1);
                }
            }
            self.rebuild_layout_slot_bindings();
            self.rebuild_reactive_slot_bindings(now);
            self.rebuild_media_texture_bindings();
            self.rebuild_caret_decoration_binding();
            self.rebuild_strict_capability_report();
            let cached_caret_visible = self
                .cached_scene
                .as_ref()
                .map(|cached| cached.caret_visible);
            if let Some(caret_visible) = cached_caret_visible {
                let _ = self.try_update_caret_visibility_slot(caret_visible);
            }
            self.rebuild_text_input_slot_bindings();
            self.rebuild_scroll_view_controller_bindings();
            // 整帧重收集已用最新 scroll_states 重算全树并把 cached.scroll_epoch 同步到当前,
            // 任何积压的滚动脏标记都已被该重收集覆盖,清空避免下帧误判。
            self.scroll_dirty_widgets.clear();
            let _ = self.consume_scroll_view_requests_from_cached_scene();

            if can_retarget_focus_navigation {
                self.retarget_focus_navigation_cache_to_current_scene();
            }

            if self.reconcile_auto_focus_after_scene_update() {
                self.invalidate_computed_scene();
                return self.computed_scene();
            }

            if let Some(started_at) = started_at {
                let computed = &self
                    .cached_scene
                    .as_ref()
                    .expect("computed scene cache should exist")
                    .computed;
                log_text_profile(
                    "textarea_computed_scene",
                    started_at.elapsed(),
                    format!(
                        "path=rebuild cache_valid=false layout_cache_valid={} cache_mismatch={} layout_ms={:.3} collect_ms={:.3} recollect_ms={:.3} collect_passes={} focused_input={:?} hit_regions={} scroll_regions={}",
                        layout_cache_valid,
                        cache_mismatch.as_deref().unwrap_or("not_profiled"),
                        layout_duration.as_secs_f64() * 1000.0,
                        collect_duration.as_secs_f64() * 1000.0,
                        recollect_duration.as_secs_f64() * 1000.0,
                        collect_passes,
                        focused_input,
                        computed.hit_regions.len(),
                        computed.scroll_regions.len(),
                    ),
                );
            }
        }

        &self
            .cached_scene
            .as_ref()
            .expect("computed scene cache should exist")
            .computed
    }

    pub(in crate::runtime) fn focused_widget_id(&self) -> Option<WidgetId> {
        self.focused_widget
            .as_ref()
            .map(|focused| focused.widget_id)
    }

    pub(in crate::runtime) fn widget_state_map(
        &self,
        active_scrollbar: Option<ScrollbarHandle>,
    ) -> WidgetStateMap {
        let mut states = WidgetStateMap::default();
        for hovered in &self.hovered_widgets {
            match hovered.target_id {
                HoverTargetId::Widget(id) => {
                    let mut state = states.get(id);
                    state.hovered = true;
                    states.set(id, state);
                }
                HoverTargetId::SplitterHandle { widget_id, .. } => {
                    let mut state = states.get(widget_id);
                    state.hovered = true;
                    states.set(widget_id, state);
                }
                HoverTargetId::SelectOption {
                    widget_id,
                    option_index,
                } => {
                    let mut state = states.get_select_option(widget_id, option_index);
                    state.hovered = true;
                    states.set_select_option(widget_id, option_index, state);
                }
                HoverTargetId::CanvasItem { .. } => {}
            }
        }
        if let Some(id) = self.pressed_widget {
            let mut state = states.get(id);
            state.pressed = true;
            states.set(id, state);
        }
        if let Some(focused) = self.focused_widget.as_ref() {
            let mut state = states.get(focused.widget_id);
            state.focused = true;
            state.focus_visible = self.focus_visible;
            states.set(focused.widget_id, state);
        }
        if let Some(handle) = self.hovered_scrollbar {
            let mut state = states.get(handle.id);
            state.hovered = true;
            states.set(handle.id, state);
        }
        if let Some(handle) = active_scrollbar {
            let mut state = states.get(handle.id);
            state.pressed = true;
            states.set(handle.id, state);
        }
        self.apply_menu_keyboard_cursor_to_states(&mut states);
        states
    }
}

fn with_runtime_scene_stack<R>(f: impl FnOnce() -> R) -> R {
    #[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
    {
        const RUNTIME_SCENE_STACK_SIZE: usize = 16 * 1024 * 1024;
        return stacker::grow(RUNTIME_SCENE_STACK_SIZE, f);
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        f()
    }
}
