use super::*;

/// 测试探针：记录 splice 快路径成功命中的次数，让测试能断言「确实走了 splice 而非
/// 回退到 recompose」。仅在测试构建里编译，热路径零成本。
#[cfg(test)]
pub(in crate::runtime) mod splice_probe {
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

#[cfg(test)]
pub(in crate::runtime) mod focus_ring_overlay_patch_probe {
    use std::cell::Cell;

    thread_local! {
        static HITS: Cell<u64> = const { Cell::new(0) };
        static REJECTS: Cell<u64> = const { Cell::new(0) };
    }

    pub(in crate::runtime) fn record_hit() {
        HITS.with(|hits| hits.set(hits.get() + 1));
    }

    pub(in crate::runtime) fn record_reject() {
        REJECTS.with(|rejects| rejects.set(rejects.get() + 1));
    }

    pub(in crate::runtime) fn reset() {
        HITS.with(|hits| hits.set(0));
        REJECTS.with(|rejects| rejects.set(0));
    }

    pub(in crate::runtime) fn hits() -> u64 {
        HITS.with(Cell::get)
    }

    pub(in crate::runtime) fn rejects() -> u64 {
        REJECTS.with(Cell::get)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ScenePatchMode {
    General,
    FocusRingOverlay,
}

struct ScenePatch<VM> {
    root: WidgetId,
    old_ids: Vec<WidgetId>,
    cache: CollectedSceneCache<VM>,
}

struct SceneSplicePlan<VM> {
    new_chunk: ComputedScene<VM>,
    ancestor_offsets: Vec<(WidgetId, crate::ui::widget::SceneCounts, usize, usize)>,
    computed_offset: crate::ui::widget::SceneCounts,
    computed_hit_offset: usize,
    computed_scroll_offset: usize,
}

fn apply_focus_ring_overlay_patch_to_cache<VM: 'static>(
    cached: &mut CachedScene<VM>,
    mut patches: Vec<ScenePatch<VM>>,
    roots: &[WidgetId],
) -> bool {
    let Some(layout) = cached.layout.as_ref() else {
        return false;
    };
    if !cached.computed.allows_focus_ring_overlay_patch() || roots.len() != patches.len() {
        return false;
    }

    let unique_roots = roots.iter().copied().collect::<HashSet<_>>();
    if unique_roots.len() != roots.len()
        || roots.iter().copied().any(|root| {
            let mut parent = layout.parent_of(root);
            while let Some(current) = parent {
                if unique_roots.contains(&current) {
                    return true;
                }
                parent = layout.parent_of(current);
            }
            false
        })
    {
        return false;
    }

    let old_global_has_ring = cached
        .computed
        .scene
        .focus_ring_overlay_shape()
        .ok()
        .flatten()
        .is_some();
    let mut old_ring_roots = 0usize;
    let mut new_ring_roots = 0usize;
    let mut affected_ancestors = HashSet::new();
    let mut new_ring_ancestors = HashSet::new();
    let mut new_overlay_source = crate::ui::widget::ScenePrimitives::default();
    let mut scene_owner_ids = HashSet::new();

    // Complete every structural check before mutating the cache. A failed qualification therefore
    // leaves the existing scene intact for the caller's generic/full-recollect fallback.
    for patch in &patches {
        let old_id_set = patch.old_ids.iter().copied().collect::<HashSet<_>>();
        let new_id_set = patch.cache.chunks.keys().copied().collect::<HashSet<_>>();
        if old_id_set != new_id_set || !new_id_set.contains(&patch.root) {
            return false;
        }
        scene_owner_ids.extend(patch.old_ids.iter().map(|id| id.raw()));

        for widget_id in &patch.old_ids {
            let Some(old_chunk) = cached.scene_chunks.get(widget_id) else {
                return false;
            };
            let Some(new_chunk) = patch.cache.chunks.get(widget_id) else {
                return false;
            };
            if !old_chunk.focus_ring_overlay_patch_compatible_with(new_chunk) {
                return false;
            }
        }

        let old_root = cached
            .scene_chunks
            .get(&patch.root)
            .expect("validated old focus root chunk");
        if old_root
            .scene
            .focus_ring_overlay_shape()
            .ok()
            .flatten()
            .is_some()
        {
            old_ring_roots += 1;
            if !old_root
                .scene
                .focus_ring_overlay_equal(&cached.computed.scene)
            {
                return false;
            }
        }

        let new_root = patch
            .cache
            .chunks
            .get(&patch.root)
            .expect("validated new focus root chunk");
        let new_root_has_ring = new_root
            .scene
            .focus_ring_overlay_shape()
            .ok()
            .flatten()
            .is_some();
        if new_root_has_ring {
            new_ring_roots += 1;
            if new_ring_roots > 1
                || !new_overlay_source.replace_focus_ring_overlay_from(&new_root.scene)
            {
                return false;
            }
        }

        let mut parent = layout.parent_of(patch.root);
        while let Some(current) = parent {
            affected_ancestors.insert(current);
            if new_root_has_ring {
                new_ring_ancestors.insert(current);
            }
            parent = layout.parent_of(current);
        }
    }

    if old_global_has_ring != (old_ring_roots == 1) || old_ring_roots > 1 {
        return false;
    }

    for ancestor in &affected_ancestors {
        let Some(chunk) = cached.scene_chunks.get(ancestor) else {
            return false;
        };
        if !chunk.allows_focus_ring_overlay_patch() {
            return false;
        }
        if chunk
            .scene
            .focus_ring_overlay_shape()
            .ok()
            .flatten()
            .is_some()
            && !chunk.scene.focus_ring_overlay_equal(&cached.computed.scene)
        {
            return false;
        }
        let Some(parts) = cached.scene_chunk_parts.get(ancestor) else {
            return false;
        };
        for local in [&parts.before_children, &parts.after_children] {
            if !local.allows_focus_ring_overlay_patch()
                || local
                    .scene
                    .focus_ring_overlay_shape()
                    .ok()
                    .flatten()
                    .is_some()
            {
                return false;
            }
        }
    }

    // Subtree collection numbers focus targets from a fresh local counter. Preserve the orders
    // assigned by the full-tree collection before these owned replacement chunks enter the cache;
    // otherwise a later unrelated ancestor recomposition can silently reorder Tab navigation.
    for patch in &mut patches {
        for widget_id in &patch.old_ids {
            let Some(previous) = cached.scene_chunks.get(widget_id) else {
                return false;
            };
            let Some(replacement) = patch.cache.chunks.get_mut(widget_id) else {
                return false;
            };
            if !replacement.preserve_focus_orders_from(previous) {
                return false;
            }
        }

        let previous_part_ids = patch
            .old_ids
            .iter()
            .copied()
            .filter(|widget_id| cached.scene_chunk_parts.contains_key(widget_id))
            .collect::<HashSet<_>>();
        let replacement_part_ids = patch
            .cache
            .chunk_parts
            .keys()
            .copied()
            .collect::<HashSet<_>>();
        if previous_part_ids != replacement_part_ids {
            return false;
        }
        for widget_id in previous_part_ids {
            let previous = cached
                .scene_chunk_parts
                .get(&widget_id)
                .expect("validated previous focus chunk parts");
            let replacement = patch
                .cache
                .chunk_parts
                .get_mut(&widget_id)
                .expect("validated replacement focus chunk parts");
            if !replacement
                .before_children
                .preserve_focus_orders_from(&previous.before_children)
                || !replacement
                    .after_children
                    .preserve_focus_orders_from(&previous.after_children)
                || !previous
                    .before_children
                    .focus_ring_overlay_patch_compatible_with(&replacement.before_children)
                || !previous
                    .after_children
                    .focus_ring_overlay_patch_compatible_with(&replacement.after_children)
            {
                return false;
            }
        }
    }

    cached
        .dependencies
        .remove_widget_phase_owners(&scene_owner_ids, DependencyPhase::Scene);
    cached
        .computed
        .dependencies
        .remove_widget_phase_owners(&scene_owner_ids, DependencyPhase::Scene);

    for patch in patches {
        for old_id in &patch.old_ids {
            cached.lifecycle_states.remove(old_id);
        }
        cached.scene_chunks.extend(patch.cache.chunks);
        cached.scene_chunk_parts.extend(patch.cache.chunk_parts);
        cached.visual_contexts.extend(patch.cache.visual_contexts);
        cached.lifecycle_states.extend(patch.cache.lifecycle_states);
        cached.dependencies.merge_from(&patch.cache.dependencies);
        cached
            .computed
            .dependencies
            .merge_from(&patch.cache.dependencies);
    }

    let empty_overlay_source = crate::ui::widget::ScenePrimitives::default();
    for ancestor in affected_ancestors {
        let source = if new_ring_ancestors.contains(&ancestor) {
            &new_overlay_source
        } else {
            &empty_overlay_source
        };
        let Some(chunk) = cached.scene_chunks.get_mut(&ancestor) else {
            unreachable!("ancestor chunks were validated before focus-ring patch commit");
        };
        let source_shape = source
            .focus_ring_overlay_shape()
            .expect("validated focus-ring source")
            .copied();
        chunk
            .scene
            .replace_validated_focus_ring_overlay_shape(source_shape);
    }
    let computed_source = if new_ring_roots == 1 {
        &new_overlay_source
    } else {
        &empty_overlay_source
    };
    let computed_shape = computed_source
        .focus_ring_overlay_shape()
        .expect("validated computed focus-ring source")
        .copied();
    cached
        .computed
        .scene
        .replace_validated_focus_ring_overlay_shape(computed_shape);

    #[cfg(test)]
    focus_ring_overlay_patch_probe::record_hit();
    true
}

impl<VM: 'static> BoundRuntimeHandler<VM> {
    pub(super) fn patch_cached_layout_for_roots(
        &mut self,
        roots: &[WidgetId],
        now: Instant,
    ) -> bool {
        with_runtime_scene_patch_stack(|| self.patch_cached_layout_for_roots_inner(roots, now))
    }

    fn patch_cached_layout_for_roots_inner(&mut self, roots: &[WidgetId], now: Instant) -> bool {
        let started_at = text_profile_enabled().then_some(Instant::now());
        let contains_virtual = {
            let Some(cached) = self.cached_scene.as_ref() else {
                return false;
            };
            let Some(layout) = cached.layout.as_ref() else {
                return false;
            };
            roots
                .iter()
                .any(|root| layout.subtree_contains_virtual(*root))
        };
        // A patched subtree can include a Virtual descendant just as readily as a scroll-driven
        // patch. Preserve its measured index, viewport hint, stable keyed ids, and scroll offset
        // without paying to clone runtime state for unrelated roots elsewhere in the tree.
        if contains_virtual {
            return self.patch_cached_layout_for_roots_with_runtime_state_inner(roots, now);
        }

        let cached = self
            .cached_scene
            .as_ref()
            .expect("layout cache was validated above");
        let layout = cached
            .layout
            .as_ref()
            .expect("layout cache was validated above");
        let touched_owner_ids = roots
            .iter()
            .flat_map(|root| layout.subtree_widget_ids(*root))
            .map(|widget_id| widget_id.raw())
            .collect::<HashSet<_>>();
        self.invalidation
            .remove_reactive_targets_for_widgets(&touched_owner_ids);

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
            now,
        ) {
            Ok(removed_ids) => removed_ids,
            Err(_) => return false,
        };

        cached.dependencies = layout.dependencies().clone();
        cached.computed_valid = false;
        let _ = layout;
        let _ = cached;
        self.prune_removed_widget_state(&removed_ids);
        self.rebuild_layout_slot_bindings();
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

    pub(super) fn patch_cached_layout_for_roots_with_runtime_state(
        &mut self,
        roots: &[WidgetId],
        now: Instant,
    ) -> bool {
        with_runtime_scene_patch_stack(|| {
            self.patch_cached_layout_for_roots_with_runtime_state_inner(roots, now)
        })
    }

    fn patch_cached_layout_for_roots_with_runtime_state_inner(
        &mut self,
        roots: &[WidgetId],
        now: Instant,
    ) -> bool {
        let started_at = text_profile_enabled().then_some(Instant::now());
        let Some(cached) = self.cached_scene.as_ref() else {
            return false;
        };
        let Some(layout) = cached.layout.as_ref() else {
            return false;
        };
        let touched_owner_ids = roots
            .iter()
            .flat_map(|root| layout.subtree_widget_ids(*root))
            .map(|widget_id| widget_id.raw())
            .collect::<HashSet<_>>();
        self.invalidation
            .remove_reactive_targets_for_widgets(&touched_owner_ids);

        let theme = self.animated_theme(now);
        let viewport = self.viewport_rect();
        let scroll_states = self.scroll_states.clone();
        let virtual_states = self.virtual_states.clone();
        let reduced_motion = self.reduced_motion;
        let style_sheet = self.config.style_sheet.clone();

        let Some(cached) = self.cached_scene.as_mut() else {
            return false;
        };
        let Some(layout) = cached.layout.as_mut() else {
            return false;
        };
        let removed_ids = match layout.patch_layout_roots_with_runtime_state(
            roots,
            &self.font_manager,
            &theme,
            &self.media_manager,
            &mut self.animation_engine,
            &scroll_states,
            &virtual_states,
            viewport,
            now,
            reduced_motion,
            &style_sheet,
        ) {
            Ok(removed_ids) => removed_ids,
            Err(_) => return false,
        };

        cached.dependencies = layout.dependencies().clone();
        cached.computed_valid = false;
        let _ = layout;
        let _ = cached;
        self.prune_removed_widget_state(&removed_ids);
        self.rebuild_layout_slot_bindings();
        self.text_input_regions.clear();
        if let Some(started_at) = started_at {
            log_text_profile(
                "textarea_patch_layout",
                started_at.elapsed(),
                format!(
                    "roots={:?} removed_ids={} computed_valid=false runtime_state=true",
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
        with_runtime_scene_patch_stack(|| {
            self.patch_cached_scene_for_roots_inner(
                roots,
                now,
                sync_runtime_scene_state,
                ScenePatchMode::General,
            )
        })
    }

    pub(super) fn patch_cached_focus_ring_scene_for_roots(
        &mut self,
        roots: &[WidgetId],
        now: Instant,
        sync_runtime_scene_state: bool,
    ) -> bool {
        with_runtime_scene_patch_stack(|| {
            self.patch_cached_scene_for_roots_inner(
                roots,
                now,
                sync_runtime_scene_state,
                ScenePatchMode::FocusRingOverlay,
            )
        })
    }

    fn patch_cached_scene_for_roots_inner(
        &mut self,
        roots: &[WidgetId],
        now: Instant,
        sync_runtime_scene_state: bool,
        patch_mode: ScenePatchMode,
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
        let active_tooltip = self.resolve_active_tooltip(now);
        let active_hover_popover = self.resolve_active_hover_popover();
        let scene_owner_ids_to_replace = {
            let Some(cached) = self.cached_scene.as_ref() else {
                return false;
            };
            let Some(layout) = cached.layout.as_ref() else {
                return false;
            };
            roots
                .iter()
                .flat_map(|root| layout.subtree_widget_ids(*root))
                .map(|widget_id| widget_id.raw())
                .collect::<HashSet<_>>()
        };
        self.invalidation.remove_reactive_targets_for_widget_phase(
            &scene_owner_ids_to_replace,
            DependencyPhase::Scene,
        );
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
                self.reduced_motion,
                visual_context,
                self.hovered_scrollbar,
                active_scrollbar,
                &widget_states,
                &self.select_open_states,
                &self.scroll_states,
                &self.virtual_states,
                viewport,
                now,
                focused_input,
                focused_text_state.as_ref(),
                focused_text_value,
                focused_text_layout,
                Some(&text_layout_overrides),
                active_slider_value,
                self.selected_text,
                selected_text_state.as_ref(),
                caret_visible,
                active_tooltip,
                active_hover_popover,
                &self.config.style_sheet,
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
            patches.push(ScenePatch {
                root: *root,
                old_ids,
                cache,
            });
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
        let patched_next_toast_wakeup = patches
            .iter()
            .filter_map(|patch| patch.cache.next_toast_wakeup)
            .min();

        let mut scene_owner_ids = HashSet::new();
        let mut ancestor_ids = HashSet::new();
        for root in roots {
            let mut parent = layout.parent_of(*root);
            while let Some(current) = parent {
                ancestor_ids.insert(current);
                parent = layout.parent_of(current);
            }
        }

        if patch_mode == ScenePatchMode::FocusRingOverlay {
            ancestor_count = ancestor_ids.len();
            if !apply_focus_ring_overlay_patch_to_cache(
                self.cached_scene.as_mut().expect("validated cached scene"),
                patches,
                roots,
            ) {
                #[cfg(test)]
                focus_ring_overlay_patch_probe::record_reject();
                return false;
            }
            let Some(cached) = self.cached_scene.as_mut() else {
                return false;
            };
            root_command_count = cached.computed.scene.commands.len();
            root_text_count = cached.computed.scene.texts.len();
            root_hit_region_count = cached.computed.hit_regions.len();
            root_scroll_region_count = cached.computed.scroll_regions.len();
            cached.computed_valid = true;
            cached.animation_epoch = self.animation_epoch;
            cached.layout_animation_epoch = self.layout_animation_epoch;
            cached.accessibility_animation_epoch = self.accessibility_animation_epoch;
            if sync_runtime_scene_state {
                cached.focused_widget = focused_widget;
                cached.focus_visible = self.focus_visible;
                cached.pressed_widget = self.pressed_widget;
                cached.selected_text = self.selected_text;
                cached.caret_visible = caret_visible;
                cached.theme_epoch = self.theme_store.version();
                cached.style_sheet_version = self.config.style_sheet.version();
                cached.density = self.theme.density;
                cached.reduced_motion = self.reduced_motion;
                cached.text_scale_bits = cached.units.font_scale().to_bits();
                cached.scroll_epoch = self.scroll_epoch;
                cached.hover_epoch = self.hover_epoch;
                cached.text_input_epoch = self.text_input_epoch;
                cached.external_portal_revision = self.external_portal_revision;
                cached.hovered_scrollbar = self.hovered_scrollbar;
                cached.active_scrollbar = active_scrollbar;
            }
        } else {
            let recompose_started_at = text_profile_enabled().then_some(Instant::now());
            {
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

                // Splice 资格判定：必须在「存入新 chunk」之前读取旧 root chunk。
                // 多个 root 只有在全部互不嵌套、且每个 subtree 的新旧命令/命中/滚动数量
                // 完全一致时才整体命中。任一 root 不满足就整体回退到 recompose，避免半快半慢
                // 带来偏移漂移。
                let can_attempt_splice = computed_allows_direct_scene_splice(&cached.computed);
                let splice_plans: Option<Vec<SceneSplicePlan<VM>>> = {
                    let mut unique_roots = HashSet::new();
                    let unique = roots.iter().copied().all(|root| unique_roots.insert(root));
                    let disjoint = roots.iter().copied().all(|root| {
                        let mut parent = layout.parent_of(root);
                        while let Some(current) = parent {
                            if unique_roots.contains(&current) {
                                return false;
                            }
                            parent = layout.parent_of(current);
                        }
                        true
                    });
                    if can_attempt_splice && unique && disjoint && roots.len() == patches.len() {
                        let mut plans = Vec::with_capacity(patches.len());
                        let mut ok = true;
                        for patch in &patches {
                            let target = patch.root;
                            let Some(new_chunk) = patch.cache.chunks.get(&target) else {
                                ok = false;
                                break;
                            };
                            let Some(old_chunk) = cached.scene_chunks.get(&target) else {
                                ok = false;
                                break;
                            };
                            if !(new_chunk.is_simple_for_splice()
                                && old_chunk.is_simple_for_splice()
                                && new_chunk.scene_counts() == old_chunk.scene_counts()
                                && new_chunk.hit_regions.len() == old_chunk.hit_regions.len()
                                && new_chunk.scroll_regions.len() == old_chunk.scroll_regions.len())
                            {
                                ok = false;
                                break;
                            }
                            let Some(ancestor_offsets) = layout.scene_splice_ancestor_offsets(
                                target,
                                &cached.scene_chunk_parts,
                                &cached.scene_chunks,
                            ) else {
                                ok = false;
                                break;
                            };
                            let (computed_offset, computed_hit_offset, computed_scroll_offset) =
                                ancestor_offsets
                                    .first()
                                    .map(|(_, offset, hit_offset, scroll_offset)| {
                                        (*offset, *hit_offset, *scroll_offset)
                                    })
                                    .unwrap_or_default();
                            plans.push(SceneSplicePlan {
                                new_chunk: new_chunk.clone(),
                                ancestor_offsets,
                                computed_offset,
                                computed_hit_offset,
                                computed_scroll_offset,
                            });
                        }
                        ok.then_some(plans)
                    } else {
                        None
                    }
                };

                for patch in patches {
                    let new_ids: HashSet<_> = patch.cache.chunks.keys().copied().collect();
                    for old_id in &patch.old_ids {
                        cached.lifecycle_states.remove(old_id);
                        if !new_ids.contains(old_id) {
                            cached.scene_chunks.remove(old_id);
                            cached.scene_chunk_parts.remove(old_id);
                            cached.visual_contexts.remove(old_id);
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

                // Splice 快路径：把每个目标子树的新 chunk 原地覆盖进每个严格祖先 chunk
                // 的稳定区间，跳过「逐级 recompose 向上重合成」。纯连接模型下，这与 recompose
                // 的结果逐字节等价（只有目标子树的字节变化，且新旧数量一致 → 后续偏移不动）。
                // 任一前置不满足（缺 chunk_parts / 子 chunk / 偏移越界）即 return false，
                // 由调用方 invalidate_computed_scene() 安全回退整帧重收集。
                let did_splice = if let Some(plans) = splice_plans.as_ref() {
                    let mut ok = true;
                    for plan in plans {
                        for (ancestor_id, offset, hit_offset, scroll_offset) in
                            &plan.ancestor_offsets
                        {
                            let Some(ancestor_chunk) = cached.scene_chunks.get_mut(ancestor_id)
                            else {
                                ok = false;
                                break;
                            };
                            if !ancestor_chunk.splice_chunk_in_place(
                                offset,
                                *hit_offset,
                                *scroll_offset,
                                &plan.new_chunk,
                            ) {
                                ok = false;
                                break;
                            }
                        }
                        if !ok {
                            break;
                        }
                    }
                    if !ok {
                        return false;
                    }
                    #[cfg(test)]
                    splice_probe::record_hit();
                    true
                } else {
                    false
                };

                if !did_splice {
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

                let can_splice_computed_directly = did_splice
                    && self.external_portal_requests.is_empty()
                    && computed_allows_direct_scene_splice(&cached.computed);
                if can_splice_computed_directly {
                    let Some(plans) = splice_plans.as_ref() else {
                        return false;
                    };
                    for plan in plans {
                        if !cached.computed.splice_chunk_in_place(
                            &plan.computed_offset,
                            plan.computed_hit_offset,
                            plan.computed_scroll_offset,
                            &plan.new_chunk,
                        ) {
                            return false;
                        }
                    }
                    root_hit_region_count = cached.computed.hit_regions.len();
                    root_scroll_region_count = cached.computed.scroll_regions.len();
                } else {
                    let root_clone_started_at = text_profile_enabled().then_some(Instant::now());
                    let Some(mut root_chunk) = cached.scene_chunks.get(&layout.root_id()).cloned()
                    else {
                        return false;
                    };
                    root_chunk.finalize_portals(viewport);
                    root_chunk.assign_new_prepare_cache_serial();
                    root_hit_region_count = root_chunk.hit_regions.len();
                    root_scroll_region_count = root_chunk.scroll_regions.len();
                    if let Some(root_clone_started_at) = root_clone_started_at {
                        root_clone_elapsed_ms =
                            root_clone_started_at.elapsed().as_secs_f64() * 1000.0;
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
                }
                cached.computed_valid = true;
                cached.animation_epoch = self.animation_epoch;
                cached.layout_animation_epoch = self.layout_animation_epoch;
                cached.accessibility_animation_epoch = self.accessibility_animation_epoch;
                if sync_runtime_scene_state {
                    cached.focused_widget = focused_widget;
                    cached.focus_visible = self.focus_visible;
                    cached.pressed_widget = self.pressed_widget;
                    cached.selected_text = self.selected_text;
                    cached.caret_visible = caret_visible;
                    cached.theme_epoch = self.theme_store.version();
                    cached.style_sheet_version = self.config.style_sheet.version();
                    cached.density = self.theme.density;
                    cached.reduced_motion = self.reduced_motion;
                    cached.text_scale_bits = cached.units.font_scale().to_bits();
                    cached.scroll_epoch = self.scroll_epoch;
                    cached.hover_epoch = self.hover_epoch;
                    cached.text_input_epoch = self.text_input_epoch;
                    cached.external_portal_revision = self.external_portal_revision;
                    cached.hovered_scrollbar = self.hovered_scrollbar;
                    cached.active_scrollbar = active_scrollbar;
                }
            }
        }
        // Preserve the old portal behavior: menu keyboard state is derived from the newly
        // patched cached scene before that scene is moved out for portal collection.
        let external_portal_widget_states = self.widget_state_map(active_scrollbar);
        // The patched scene is now fully owned by `cached.computed`. Move it out while
        // external portals are appended instead of cloning the entire retained scene.
        // Every exit below restores the moved value before returning.
        let mut updated_computed = {
            let Some(cached) = self.cached_scene.as_mut() else {
                return false;
            };
            std::mem::take(&mut cached.computed)
        };
        self.append_external_portals_to_computed(
            &mut updated_computed,
            &external_portal_widget_states,
            now,
        );
        if let Some(cached) = self.cached_scene.as_mut() {
            cached
                .dependencies
                .merge_from(&updated_computed.dependencies);
        }

        let actual_focused_input = self.focused_text_input_id_cached(&updated_computed);
        let actual_caret_visible = self.caret_visible_at(now, actual_focused_input);
        if actual_focused_input != focused_input || actual_caret_visible != caret_visible {
            if let Some(cached) = self.cached_scene.as_mut() {
                cached.computed = updated_computed;
            }
            return false;
        }

        let updated_hit_region_count = updated_computed.hit_regions.len();
        let updated_scroll_region_count = updated_computed.scroll_regions.len();
        let virtual_layout_invalidated = self.sync_virtual_state_updates(&updated_computed);
        if virtual_layout_invalidated {
            if let Some(cached) = self.cached_scene.as_mut() {
                cached.computed = updated_computed;
                cached.layout_valid = false;
                cached.computed_valid = false;
            }
            self.text_input_regions.clear();
            return false;
        }
        self.sync_text_inputs_from_computed(&updated_computed);
        if let Some(cached) = self.cached_scene.as_mut() {
            cached.computed = updated_computed;
        }
        self.rebuild_scroll_view_controller_bindings();
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
                    updated_hit_region_count,
                    updated_scroll_region_count,
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
        if let Some(deadline) = patched_next_toast_wakeup {
            self.next_toast_wakeup_deadline = Some(
                self.next_toast_wakeup_deadline
                    .map_or(deadline, |current| current.min(deadline)),
            );
        }
        true
    }
}

fn computed_allows_direct_scene_splice<VM>(computed: &ComputedScene<VM>) -> bool {
    let portal_counts = computed.portal_overlay_counts;
    computed.scene.counts().has_no_overlay()
        && computed.overlay_hit_regions.is_empty()
        && computed.overlay_close_handlers.is_empty()
        && computed.accessibility_fragments.is_empty()
        && computed.portal_entries.is_empty()
        && computed.external_portal_requests.is_empty()
        && portal_counts.shapes == 0
        && portal_counts.textures == 0
        && portal_counts.meshes == 0
        && portal_counts.texts == 0
        && portal_counts.text_decorations == 0
        && portal_counts.commands == 0
        && portal_counts.hits == 0
        && portal_counts.close_handlers == 0
        && portal_counts.focus_scopes == 0
        && portal_counts.accessibility_fragments == 0
        && computed.overlay_layer_graph.layers.is_empty()
        && computed.overlay_layer_graph.anchor_slots.is_empty()
        && computed.overlay_layers.iter().all(|bucket| {
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
}

fn with_runtime_scene_patch_stack<R>(f: impl FnOnce() -> R) -> R {
    #[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
    {
        // Patch collection can be entered for every hover/interaction update. Preserve the
        // overflow guard without paying for a new stack on every successful fast-path call.
        const RUNTIME_SCENE_PATCH_STACK_RED_ZONE: usize = 2 * 1024 * 1024;
        const RUNTIME_SCENE_PATCH_STACK_SIZE: usize = 16 * 1024 * 1024;
        return stacker::maybe_grow(
            RUNTIME_SCENE_PATCH_STACK_RED_ZONE,
            RUNTIME_SCENE_PATCH_STACK_SIZE,
            f,
        );
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        f()
    }
}
