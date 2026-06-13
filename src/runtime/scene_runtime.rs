use super::*;
use crate::foundation::binding::ScrollRequestMode;
use crate::foundation::binding::{ScrollRequest, ScrollViewController};
use crate::ui::unit::Dp;

/// 测试探针：记录纯滚动快路径命中次数，让测试能断言「确实走了滚动快路径而非整帧重收集」。
/// 仅测试构建编译，热路径零成本。
#[cfg(all(test, feature = "transform-only-scroll"))]
pub(in crate::runtime) mod scroll_fast_path_probe {
    use std::cell::Cell;
    thread_local! {
        static HITS: Cell<u64> = const { Cell::new(0) };
    }
    pub(in crate::runtime) fn record_hit() {
        HITS.with(|h| h.set(h.get() + 1));
    }
    pub(in crate::runtime) fn reset() {
        HITS.with(|h| h.set(0));
    }
    pub(in crate::runtime) fn hits() -> u64 {
        HITS.with(Cell::get)
    }
}

#[cfg(feature = "transform-only-scroll-gpu")]
fn gpu_scroll_clip_supported(clip_rect: Option<Rect>, region: ScrollRegion) -> bool {
    let Some(clip_rect) = clip_rect else {
        return false;
    };
    clip_rect == region.content_viewport || clip_rect == region.visible_frame
}

#[cfg(feature = "transform-only-scroll-gpu")]
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
        crate::ui::widget::RenderCommand::Mesh(primitive) => {
            gpu_scroll_clip_supported(primitive.clip_rect, region)
        }
    }
}

#[cfg(feature = "transform-only-scroll-gpu")]
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

#[cfg(feature = "transform-only-scroll-gpu")]
fn gpu_scroll_has_descendant_scroll_region<VM: 'static>(
    layout: &ResolvedSceneLayout<VM>,
    regions: &[ScrollRegion],
    widget_id: WidgetId,
) -> bool {
    let Some(path) = layout.path_for(widget_id) else {
        return true;
    };
    regions.iter().any(|region| {
        region.id != widget_id
            && layout
                .path_for(region.id)
                .map(|candidate| candidate.starts_with(path))
                .unwrap_or(false)
    })
}

#[cfg(feature = "transform-only-scroll-gpu")]
fn translate_gpu_point(point: &mut Point, delta: Point) {
    point.x += delta.x;
    point.y += delta.y;
}

#[cfg(feature = "transform-only-scroll-gpu")]
fn translate_gpu_rect(rect: &mut Rect, delta: Point) {
    rect.x += delta.x;
    rect.y += delta.y;
}

#[cfg(feature = "transform-only-scroll-gpu")]
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

#[cfg(feature = "transform-only-scroll-gpu")]
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

#[cfg(feature = "transform-only-scroll-gpu")]
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

#[cfg(feature = "transform-only-scroll-gpu")]
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

impl<VM: 'static> BoundRuntimeHandler<VM> {
    #[cfg(feature = "transform-only-scroll-gpu")]
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
        if !self
            .renderer
            .as_ref()
            .map(|renderer| renderer.push_constants_supported())
            .unwrap_or(false)
        {
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
            || cached.computed.portal_overlay_counts.commands > 0
            || cached.computed.portal_overlay_counts.hits > 0
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
        if gpu_scroll_has_descendant_scroll_region(
            layout,
            &cached.computed.scroll_regions,
            widget_id,
        ) {
            return false;
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
        translate_gpu_scroll_hits(&mut cached.computed.hit_regions, widget_id, hit_delta);
        for chunk in cached.scene_chunks.values_mut() {
            if let Some(region) = chunk
                .scroll_regions
                .iter_mut()
                .find(|region| region.id == widget_id)
            {
                region.scroll_offset = next_offset;
            }
            translate_gpu_scroll_hits(&mut chunk.hit_regions, widget_id, hit_delta);
        }
        cached.scroll_epoch = self.scroll_epoch;
        cached.caret_visible = caret_visible;
        cached.active_scrollbar = active_scrollbar;
        cached.hovered_scrollbar = self.hovered_scrollbar;
        cached.gpu_scroll_deferred = true;
        self.scroll_dirty_widgets.clear();
        #[cfg(test)]
        scroll_fast_path_probe::record_hit();
        true
    }

    fn apply_scroll_view_controller_state(
        &mut self,
        bindings: Vec<(WidgetId, ScrollRegion, ScrollViewController)>,
    ) -> bool {
        let mut changed = false;
        let mut requests: Vec<(WidgetId, ScrollRegion, ScrollViewController, ScrollRequest)> =
            Vec::new();
        for (widget_id, region, controller) in bindings {
            controller.bind_widget(widget_id);
            controller.sync_offset(region.scroll_offset);
            if let Some(request) = controller.take_request() {
                requests.push((widget_id, region, controller, request));
            }
        }

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
            changed = true;
        }

        changed
    }

    fn consume_scroll_view_requests_from_cached_scene(&mut self) -> bool {
        let bindings = if let Some(cached) = self.cached_scene.as_ref() {
            if let Some(layout) = cached.layout.as_ref() {
                cached
                    .computed
                    .scroll_regions
                    .iter()
                    .filter_map(|region| {
                        let resolved = layout.resolved_widget(region.id)?;
                        let crate::ui::widget::ResolvedWidgetKind::Container { layout, .. } =
                            &resolved.kind
                        else {
                            return None;
                        };
                        let controller = layout.scroll_view.as_ref()?.controller.clone()?;
                        Some((region.id, *region, controller))
                    })
                    .collect()
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };
        self.apply_scroll_view_controller_state(bindings)
    }

    fn sync_virtual_state_updates(&mut self, computed: &ComputedScene<VM>) -> bool {
        let mut layout_invalidated = false;
        for update in &computed.virtual_state_updates {
            let state = self.virtual_states.entry(update.widget_id).or_default();
            state.viewport_hint = Some(update.viewport_hint.clone());
            for (index, extent) in &update.measured_extents {
                let measured_changed = state
                    .measured_extents
                    .get(index)
                    .copied()
                    .map(|previous| {
                        (previous - *extent).abs()
                            > crate::ui::widget::MEASURED_EXTENT_INVALIDATION_EPSILON
                    })
                    .unwrap_or(true);
                if measured_changed {
                    layout_invalidated = layout_invalidated || update.invalidate_layout;
                    state.measured_extents.insert(*index, *extent);
                }
            }
            state.widget_ids_by_key = update.widget_ids_by_key.iter().cloned().collect();
        }
        layout_invalidated
    }

    /// Phase 4 纯滚动快路径。仅当本帧的缓存失配**只有** `scroll_epoch`（其余字段全部匹配、
    /// 布局缓存仍有效、不含 virtual）时尝试：把发生滚动的容器作为 patch 根，复用既有且已测的
    /// `patch_cached_scene_for_roots`（子树作用域重收集 + Phase 1 splice）绕开整树重收集。
    ///
    /// 关键正确性：这里调用的 collect 与整帧重收集是**同一个收集函数**，只是作用域收窄到
    /// 滚动子树。`patch_resolved_roots` 会用最新 `scroll_states` 重新解析子树几何（含
    /// `child_origin = frame - scroll_offset` 平移与 `should_skip_fully_clipped_child` 裁剪），
    /// 因此结果与整帧重收集**逐项等价**——这不是「平移缓存图元」的近似，而是「只重收集受影响
    /// 子树」的精确等价。任一前置不满足 / patch 失败即返回 false，调用方落回整帧重收集。
    ///
    /// 嵌套滚动安全：若一次滚动同时脏了祖孙两个滚动容器，`highest_layout_roots_smallvec`
    /// 只取最高根，重收集其整棵子树（含内层滚动），不会漏更新内层。
    #[cfg(feature = "transform-only-scroll")]
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
        // virtual 容器滚动会改变窗口化 plan（结构性），不在纯滚动快路径覆盖范围。
        if layout.contains_virtual() {
            return false;
        }
        // 收集本帧实际发生滚动、且仍在当前布局树中的脏容器。
        let mut affected: HashSet<WidgetId> = HashSet::new();
        for widget_id in self.scroll_dirty_widgets.iter().copied() {
            if layout.path_for(widget_id).is_some() {
                affected.insert(widget_id);
            }
        }
        if affected.is_empty() {
            return false;
        }
        let roots = self.highest_layout_roots_smallvec(layout, &affected);
        if roots.is_empty() {
            return false;
        }
        let roots = roots.to_vec();
        if !self.patch_cached_scene_for_roots(&roots, now, true) {
            // patch 失败：保持 cached 未被破坏（patch 在失败前不写 computed），落回整帧重收集。
            return false;
        }
        // patch 以 sync_runtime_scene_state=true 同步了 scroll_epoch 等运行时状态。
        #[cfg(feature = "transform-only-scroll-gpu")]
        if let Some(cached) = self.cached_scene.as_mut() {
            cached.gpu_scroll_deferred = false;
        }
        self.scroll_dirty_widgets.clear();
        #[cfg(test)]
        scroll_fast_path_probe::record_hit();
        true
    }

    pub(in crate::runtime) fn computed_scene(&mut self) -> &ComputedScene<VM> {
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
        ) = if let Some(cached) = self.cached_scene.as_ref() {
            let focused_input = self.focused_text_input_id_cached(&cached.computed);
            let focused_text_state = focused_input
                .and_then(|id| self.text_edit_state(id))
                .cloned();
            let caret_visible = self.caret_visible_at(now, focused_input);
            let cache_mismatch = self.scene_cache_mismatch_summary(
                cached,
                viewport,
                units,
                caret_visible,
                active_scrollbar,
            );
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
                "no_cached_scene".to_string(),
            )
        };
        let selected_text_state = self
            .selected_text
            .and_then(|id| self.text_edit_state(id))
            .cloned();
        let active_tooltip = self.resolve_active_tooltip(now);
        let active_hover_popover = self.resolve_active_hover_popover();

        let text_input_patch_roots = self.cached_scene.as_ref().and_then(|cached| {
            (layout_cache_valid
                && !cache_valid
                && {
                    #[cfg(feature = "transform-only-scroll-gpu")]
                    {
                        !cached.gpu_scroll_deferred
                    }
                    #[cfg(not(feature = "transform-only-scroll-gpu"))]
                    {
                        true
                    }
                }
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
                if let Some(started_at) = started_at {
                    log_text_profile(
                        "textarea_computed_scene",
                        started_at.elapsed(),
                        format!(
                            "path=text_input_patch roots={} cache_valid={} layout_cache_valid={} cache_mismatch={}",
                            roots.len(),
                            cache_valid,
                            layout_cache_valid,
                            cache_mismatch
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

        // Phase 4 纯滚动快路径：仅 scroll_epoch 变化时,只重收集滚动子树而非整树。
        // 命中即直接返回已更新的 cached.computed；不命中(前置不满足/patch 失败)
        // 落回下方常规 `!cache_valid` 整帧重收集,行为与未开启特性时完全一致。
        #[cfg(feature = "transform-only-scroll-gpu")]
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
                        cache_valid, layout_cache_valid, cache_mismatch
                    ),
                );
            }
            return &self
                .cached_scene
                .as_ref()
                .expect("pure scroll gpu fast path should preserve cached scene")
                .computed;
        }

        #[cfg(feature = "transform-only-scroll")]
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
                        cache_valid, layout_cache_valid, cache_mismatch
                    ),
                );
            }
            return &self
                .cached_scene
                .as_ref()
                .expect("pure scroll patch should preserve cached scene")
                .computed;
        }

        let widget_states = self.widget_state_map(active_scrollbar);
        if !cache_valid {
            let mut layout_duration = Duration::ZERO;
            let mut collect_duration = Duration::ZERO;
            let mut recollect_duration = Duration::ZERO;
            let mut collect_passes = 0usize;
            let previous_cached = self.cached_scene.take();
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
                            let layout = tree
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
            self.append_external_portals_to_computed(&mut computed, now);
            let virtual_layout_invalidated = self.sync_virtual_state_updates(&computed);
            if virtual_layout_invalidated {
                self.layout_animation_epoch = self.layout_animation_epoch.wrapping_add(1);
                if let Some(window) = self.window.as_ref() {
                    window.request_redraw();
                }
            }
            let focused_input = self.focused_text_input_id_cached(&computed);
            let caret_visible = self.caret_visible_at(now, focused_input);
            self.prune_text_input_buffers(&computed);
            self.sync_text_input_regions_from_computed(&computed);
            self.sync_visible_text_input_buffers(&computed);
            self.cached_scene = Some(CachedScene {
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
                scroll_epoch: self.scroll_epoch,
                hover_epoch: self.hover_epoch,
                text_input_epoch: self.text_input_epoch,
                external_portal_revision: self.external_portal_revision,
                hovered_scrollbar: self.hovered_scrollbar,
                active_scrollbar,
                layout_valid: true,
                computed_valid: true,
                #[cfg(feature = "transform-only-scroll-gpu")]
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
            });
            // 整帧重收集已用最新 scroll_states 重算全树并把 cached.scroll_epoch 同步到当前,
            // 任何积压的滚动脏标记都已被该重收集覆盖,清空避免下帧误判。
            #[cfg(feature = "transform-only-scroll")]
            self.scroll_dirty_widgets.clear();
            if let Some(cached) = self.cached_scene.as_ref() {
                let bindings = if let Some(layout) = cached.layout.as_ref() {
                    cached
                        .computed
                        .scroll_regions
                        .iter()
                        .filter_map(|region| {
                            let resolved = layout.resolved_widget(region.id)?;
                            let crate::ui::widget::ResolvedWidgetKind::Container { layout, .. } =
                                &resolved.kind
                            else {
                                return None;
                            };
                            let controller = layout.scroll_view.as_ref()?.controller.clone()?;
                            Some((region.id, *region, controller))
                        })
                        .collect()
                } else {
                    Vec::new()
                };
                let _ = self.apply_scroll_view_controller_state(bindings);
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
                        cache_mismatch,
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
        if self.focus_visible {
            if let Some(focused) = self.focused_widget.as_ref() {
                let mut state = states.get(focused.widget_id);
                state.focused = true;
                state.focus_visible = true;
                states.set(focused.widget_id, state);
            }
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

    pub(in crate::runtime) fn scroll_regions(&mut self) -> Vec<ScrollRegion> {
        self.computed_scene().scroll_regions.to_vec()
    }
}
