use super::*;

impl<VM: 'static> BoundRuntimeHandler<VM> {
    pub(super) fn patch_cached_layout_for_roots(
        &mut self,
        roots: &[WidgetId],
        now: Instant,
    ) -> bool {
        let started_at = text_profile_enabled().then_some(Instant::now());
        let Some(cached) = self.cached_scene.as_ref() else {
            return false;
        };
        let Some(_layout) = cached.layout.as_ref() else {
            return false;
        };

        let theme = self.animated_theme(now);
        let viewport = self.viewport_rect();

        let Some(cached) = self.cached_scene.as_mut() else {
            return false;
        };
        let Some(layout) = cached.layout.as_mut() else {
            return false;
        };
        let removed_ids = match layout.patch_layout_roots(
            roots,
            &self.font_manager,
            &theme,
            &self.media_manager,
            &mut self.animation_engine,
            viewport,
        ) {
            Ok(removed_ids) => removed_ids,
            Err(_) => return false,
        };

        cached.dependencies = layout.dependencies().clone();
        cached.computed_valid = false;
        let _ = layout;
        let _ = cached;
        self.prune_removed_widget_state(&removed_ids);
        self.text_input_regions.clear();
        if let Some(started_at) = started_at {
            log_text_profile(
                "textarea_patch_layout",
                started_at.elapsed(),
                format!(
                    "roots={:?} removed_ids={} computed_valid=false",
                    roots,
                    removed_ids.len()
                ),
            );
        }
        true
    }

    pub(super) fn patch_cached_scene_for_roots(
        &mut self,
        roots: &[WidgetId],
        now: Instant,
        sync_runtime_scene_state: bool,
    ) -> bool {
        let started_at = text_profile_enabled().then_some(Instant::now());
        let collect_started_at = text_profile_enabled().then_some(Instant::now());
        let mut collect_elapsed_ms = 0.0;
        let mut resolve_roots_elapsed_ms = 0.0;
        let mut focus_override_elapsed_ms = 0.0;
        let mut layout_overrides_elapsed_ms = 0.0;
        let mut collect_roots_elapsed_ms = 0.0;
        let mut recompose_elapsed_ms = 0.0;
        let mut root_clone_elapsed_ms = 0.0;
        let mut patched_widget_count = 0usize;
        let mut patch_command_count = 0usize;
        let mut patch_text_count = 0usize;
        let ancestor_count;
        let mut root_command_count = 0usize;
        let mut root_text_count = 0usize;
        let root_hit_region_count;
        let root_scroll_region_count;
        let theme = self.animated_theme(now);
        let resolve_roots_started_at = text_profile_enabled().then_some(Instant::now());
        {
            let Some(cached) = self.cached_scene.as_mut() else {
                return false;
            };
            let Some(layout) = cached.layout.as_mut() else {
                return false;
            };
            if !sync_runtime_scene_state && !layout.patch_resolved_roots(roots, &theme) {
                return false;
            }
        }
        if let Some(resolve_roots_started_at) = resolve_roots_started_at {
            resolve_roots_elapsed_ms = resolve_roots_started_at.elapsed().as_secs_f64() * 1000.0;
            log_text_profile(
                "textarea_patch_scene_resolve_roots",
                resolve_roots_started_at.elapsed(),
                format!(
                    "roots={:?} sync_runtime_scene_state={}",
                    roots, sync_runtime_scene_state
                ),
            );
        }
        let Some(cached) = self.cached_scene.as_ref() else {
            return false;
        };
        let Some(layout) = cached.layout.as_ref() else {
            return false;
        };

        let viewport = self.viewport_rect();
        let active_scrollbar = self.active_scrollbar_drag.map(|drag| drag.handle);
        let widget_states = self.widget_state_map(active_scrollbar);
        let focused_input = self.focused_text_input_id_cached(&cached.computed);
        let focused_text_state = focused_input
            .and_then(|id| self.text_edit_state(id))
            .cloned();
        let selected_text_state = self
            .selected_text
            .and_then(|id| self.text_edit_state(id))
            .cloned();
        let caret_visible = self.caret_visible_at(now, focused_input);
        let focus_override_started_at = text_profile_enabled().then_some(Instant::now());
        let (focused_text_value, focused_text_layout) = Self::focused_text_overrides(
            &self.text_input_buffers,
            focused_input,
            focused_text_state.as_ref(),
        );
        if let Some(focus_override_started_at) = focus_override_started_at {
            focus_override_elapsed_ms = focus_override_started_at.elapsed().as_secs_f64() * 1000.0;
            log_text_profile(
                "textarea_patch_scene_focus_override",
                focus_override_started_at.elapsed(),
                format!(
                    "focused_input={:?} has_value={} has_layout={} has_composition={}",
                    focused_input,
                    focused_text_value.is_some(),
                    focused_text_layout.is_some(),
                    focused_text_state
                        .as_ref()
                        .and_then(|state| state.composition.as_ref())
                        .is_some(),
                ),
            );
        }
        let layout_overrides_started_at = text_profile_enabled().then_some(Instant::now());
        let text_layout_overrides = Self::stable_text_layout_overrides(&self.text_input_buffers);
        if let Some(layout_overrides_started_at) = layout_overrides_started_at {
            layout_overrides_elapsed_ms =
                layout_overrides_started_at.elapsed().as_secs_f64() * 1000.0;
            log_text_profile(
                "textarea_patch_scene_layout_overrides",
                layout_overrides_started_at.elapsed(),
                format!(
                    "overrides={} text_input_buffers={}",
                    text_layout_overrides.len(),
                    self.text_input_buffers.len(),
                ),
            );
        }
        let focused_widget = self.focused_widget_id();

        struct ScenePatch<VM> {
            old_ids: Vec<WidgetId>,
            cache: CollectedSceneCache<VM>,
        }

        let mut patches = Vec::new();
        for root in roots {
            let old_ids = layout.subtree_widget_ids(*root);
            let Some(visual_context) = cached.visual_contexts.get(root).copied() else {
                return false;
            };
            let collect_root_started_at = text_profile_enabled().then_some(Instant::now());
            let active_slider_value = self.active_slider_value_override();
            let Some(cache) = layout.collect_scene_cache_for_widget_with_focus_value(
                *root,
                &self.font_manager,
                &theme,
                &self.media_manager,
                &mut self.animation_engine,
                visual_context,
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
            ) else {
                return false;
            };
            if let Some(collect_root_started_at) = collect_root_started_at {
                let elapsed = collect_root_started_at.elapsed();
                collect_roots_elapsed_ms += elapsed.as_secs_f64() * 1000.0;
                log_text_profile(
                    "textarea_patch_scene_collect_root",
                    elapsed,
                    format!(
                        "root={:?} old_ids={} commands={} texts={} hit_regions={} scroll_regions={}",
                        root,
                        old_ids.len(),
                        cache.computed.scene.commands.len(),
                        cache.computed.scene.texts.len(),
                        cache.computed.hit_regions.len(),
                        cache.computed.scroll_regions.len(),
                    ),
                );
            }
            patches.push(ScenePatch { old_ids, cache });
        }
        if let Some(collect_started_at) = collect_started_at {
            let patched_widget_ids = patches
                .iter()
                .flat_map(|patch| patch.cache.chunks.keys().copied())
                .collect::<Vec<_>>();
            let patch_commands = patches
                .iter()
                .map(|patch| patch.cache.computed.scene.commands.len())
                .sum::<usize>();
            let patch_texts = patches
                .iter()
                .map(|patch| patch.cache.computed.scene.texts.len())
                .sum::<usize>();
            patched_widget_count = patched_widget_ids.len();
            patch_command_count = patch_commands;
            patch_text_count = patch_texts;
            collect_elapsed_ms = collect_started_at.elapsed().as_secs_f64() * 1000.0;
            log_text_profile(
                "textarea_patch_scene_collect",
                std::time::Duration::from_secs_f64(collect_elapsed_ms / 1000.0),
                format!(
                    "roots={:?} patched_widgets={} patched_ids={:?} patch_commands={} patch_texts={}",
                    roots,
                    patched_widget_ids.len(),
                    patched_widget_ids,
                    patch_commands,
                    patch_texts
                ),
            );
        }

        let mut scene_owner_ids = HashSet::new();
        let mut ancestor_ids = HashSet::new();
        for root in roots {
            let mut parent = layout.parent_of(*root);
            while let Some(current) = parent {
                ancestor_ids.insert(current);
                parent = layout.parent_of(current);
            }
        }

        let recompose_started_at = text_profile_enabled().then_some(Instant::now());
        let updated_computed = {
            let Some(cached) = self.cached_scene.as_mut() else {
                return false;
            };
            let Some(layout) = cached.layout.as_ref() else {
                return false;
            };

            for patch in &patches {
                for old_id in &patch.old_ids {
                    scene_owner_ids.insert(old_id.raw());
                }
            }
            cached
                .dependencies
                .remove_widget_phase_owners(&scene_owner_ids, DependencyPhase::Scene);

            for patch in patches {
                let new_ids: HashSet<_> = patch.cache.chunks.keys().copied().collect();
                for old_id in &patch.old_ids {
                    if !new_ids.contains(old_id) {
                        cached.scene_chunks.remove(old_id);
                        cached.scene_chunk_parts.remove(old_id);
                        cached.visual_contexts.remove(old_id);
                        cached.lifecycle_states.remove(old_id);
                    }
                }
                cached.scene_chunks.extend(patch.cache.chunks);
                cached.scene_chunk_parts.extend(patch.cache.chunk_parts);
                cached.visual_contexts.extend(patch.cache.visual_contexts);
                cached.lifecycle_states.extend(patch.cache.lifecycle_states);
                cached.dependencies.merge_from(&patch.cache.dependencies);
            }

            let mut ancestors = ancestor_ids.into_iter().collect::<Vec<_>>();
            ancestors.sort_by_key(|widget_id| std::cmp::Reverse(layout.depth_of(*widget_id)));
            ancestor_count = ancestors.len();
            for ancestor in ancestors {
                if layout
                    .recompose_scene_chunk(
                        ancestor,
                        &cached.scene_chunk_parts,
                        &mut cached.scene_chunks,
                    )
                    .is_none()
                {
                    return false;
                }
            }
            if let Some(recompose_started_at) = recompose_started_at {
                let root_commands = cached
                    .scene_chunks
                    .get(&layout.root_id())
                    .map(|chunk| chunk.scene.commands.len())
                    .unwrap_or(0);
                let root_texts = cached
                    .scene_chunks
                    .get(&layout.root_id())
                    .map(|chunk| chunk.scene.texts.len())
                    .unwrap_or(0);
                root_command_count = root_commands;
                root_text_count = root_texts;
                recompose_elapsed_ms = recompose_started_at.elapsed().as_secs_f64() * 1000.0;
                log_text_profile(
                    "textarea_patch_scene_recompose",
                    std::time::Duration::from_secs_f64(recompose_elapsed_ms / 1000.0),
                    format!(
                        "roots={:?} ancestors={} root_commands={} root_texts={}",
                        roots, ancestor_count, root_commands, root_texts
                    ),
                );
            }

            let root_clone_started_at = text_profile_enabled().then_some(Instant::now());
            let Some(root_chunk) = cached.scene_chunks.get(&layout.root_id()).cloned() else {
                return false;
            };
            root_hit_region_count = root_chunk.hit_regions.len();
            root_scroll_region_count = root_chunk.scroll_regions.len();
            if let Some(root_clone_started_at) = root_clone_started_at {
                root_clone_elapsed_ms = root_clone_started_at.elapsed().as_secs_f64() * 1000.0;
                log_text_profile(
                    "textarea_patch_scene_root_clone",
                    std::time::Duration::from_secs_f64(root_clone_elapsed_ms / 1000.0),
                    format!(
                        "roots={:?} commands={} texts={} hit_regions={} scroll_regions={}",
                        roots,
                        root_chunk.scene.commands.len(),
                        root_chunk.scene.texts.len(),
                        root_chunk.hit_regions.len(),
                        root_chunk.scroll_regions.len()
                    ),
                );
            }
            cached.computed = root_chunk;
            cached.computed_valid = true;
            if sync_runtime_scene_state {
                cached.focused_widget = focused_widget;
                cached.focus_visible = self.focus_visible;
                cached.pressed_widget = self.pressed_widget;
                cached.selected_text = self.selected_text;
                cached.caret_visible = caret_visible;
                cached.theme_epoch = self.theme_store.version();
                cached.animation_epoch = self.animation_epoch;
                cached.layout_animation_epoch = self.layout_animation_epoch;
                cached.scroll_epoch = self.scroll_epoch;
                cached.hover_epoch = self.hover_epoch;
                cached.text_input_epoch = self.text_input_epoch;
                cached.hovered_scrollbar = self.hovered_scrollbar;
                cached.active_scrollbar = active_scrollbar;
            }
            cached.computed.clone()
        };

        let actual_focused_input = self.focused_text_input_id_cached(&updated_computed);
        let actual_caret_visible = self.caret_visible_at(now, actual_focused_input);
        if actual_focused_input != focused_input || actual_caret_visible != caret_visible {
            return false;
        }

        self.prune_text_input_buffers(&updated_computed);
        self.sync_text_input_regions_from_computed(&updated_computed);
        self.sync_visible_text_input_buffers(&updated_computed);
        if let Some(started_at) = started_at {
            log_text_profile(
                "textarea_patch_scene",
                started_at.elapsed(),
                format!(
                    "roots={:?} sync_runtime_scene_state={} focused_input={:?} actual_focused_input={:?} hit_regions={} scroll_regions={} collect_ms={:.3} resolve_roots_ms={:.3} focus_override_ms={:.3} layout_overrides_ms={:.3} collect_roots_ms={:.3} recompose_ms={:.3} root_clone_ms={:.3} patched_widgets={} patch_commands={} patch_texts={} ancestors={} root_commands={} root_texts={} root_hit_regions={} root_scroll_regions={}",
                    roots,
                    sync_runtime_scene_state,
                    focused_input,
                    actual_focused_input,
                    updated_computed.hit_regions.len(),
                    updated_computed.scroll_regions.len(),
                    collect_elapsed_ms,
                    resolve_roots_elapsed_ms,
                    focus_override_elapsed_ms,
                    layout_overrides_elapsed_ms,
                    collect_roots_elapsed_ms,
                    recompose_elapsed_ms,
                    root_clone_elapsed_ms,
                    patched_widget_count,
                    patch_command_count,
                    patch_text_count,
                    ancestor_count,
                    root_command_count,
                    root_text_count,
                    root_hit_region_count,
                    root_scroll_region_count
                ),
            );
        }
        true
    }
}
