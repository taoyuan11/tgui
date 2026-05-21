use super::*;

impl<VM: 'static> BoundRuntimeHandler<VM> {
    pub(in crate::runtime) fn computed_scene(&mut self) -> &ComputedScene<VM> {
        let started_at = text_profile_enabled().then_some(Instant::now());
        let viewport = self.viewport_rect();
        let units = self.unit_context();
        let now = Instant::now();
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
                            let collected = tree.collect_scene_cache_from_layout_with_focus_value(
                                &self.font_manager,
                                layout,
                                &theme,
                                &self.media_manager,
                                &mut self.animation_engine,
                                self.hovered_scrollbar,
                                active_scrollbar,
                                &widget_states,
                                &self.select_open_states,
                                &self.scroll_states,
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
                                    .collect_scene_cache_from_layout_with_focus_value(
                                        &self.font_manager,
                                        layout,
                                        &theme,
                                        &self.media_manager,
                                        &mut self.animation_engine,
                                        self.hovered_scrollbar,
                                        active_scrollbar,
                                        &widget_states,
                                        &self.select_open_states,
                                        &self.scroll_states,
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
                            let layout = tree.build_scene_layout(
                                &self.font_manager,
                                &theme,
                                &self.media_manager,
                                &mut self.animation_engine,
                                units,
                                viewport,
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
                            let collected = tree.collect_scene_cache_from_layout_with_focus_value(
                                &self.font_manager,
                                &layout,
                                &theme,
                                &self.media_manager,
                                &mut self.animation_engine,
                                self.hovered_scrollbar,
                                active_scrollbar,
                                &widget_states,
                                &self.select_open_states,
                                &self.scroll_states,
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
                            let collected = tree.collect_scene_cache_from_layout_with_focus_value(
                                &self.font_manager,
                                &layout,
                                &theme,
                                &self.media_manager,
                                &mut self.animation_engine,
                                self.hovered_scrollbar,
                                active_scrollbar,
                                &widget_states,
                                &self.select_open_states,
                                &self.scroll_states,
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
                    },
                ),
            };
            let computed = collected.computed.clone();
            self.next_tooltip_wakeup_deadline = collected.next_tooltip_wakeup;
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
                animation_epoch: self.animation_epoch,
                layout_animation_epoch: self.layout_animation_epoch,
                scroll_epoch: self.scroll_epoch,
                hover_epoch: self.hover_epoch,
                text_input_epoch: self.text_input_epoch,
                hovered_scrollbar: self.hovered_scrollbar,
                active_scrollbar,
                computed_valid: true,
                dependencies: {
                    let mut dependencies = DependencyGraph::default();
                    if let Some(layout) = layout.as_ref() {
                        dependencies.merge_from(layout.dependencies());
                    }
                    dependencies.merge_from(&collected.dependencies);
                    dependencies
                },
                layout,
                computed,
                lifecycle_states: collected.lifecycle_states,
                scene_chunks: collected.chunks,
                scene_chunk_parts: collected.chunk_parts,
                visual_contexts: collected.visual_contexts,
            });

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
        states
    }

    pub(in crate::runtime) fn scroll_regions(&mut self) -> Vec<ScrollRegion> {
        self.computed_scene().scroll_regions.clone()
    }

    pub(in crate::runtime) fn ime_cursor_area(&mut self) -> Option<Rect> {
        self.computed_scene().ime_cursor_area
    }
}
