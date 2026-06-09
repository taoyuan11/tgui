use super::*;
use crate::foundation::binding::ScrollRequestMode;
use crate::foundation::binding::{ScrollRequest, ScrollViewController};
use crate::ui::unit::Dp;
impl<VM: 'static> BoundRuntimeHandler<VM> {
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
