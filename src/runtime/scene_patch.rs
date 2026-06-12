use super::*;

/// 测试探针：记录 splice 快路径成功命中的次数，让测试能断言「确实走了 splice 而非
/// 回退到 recompose」。仅在测试构建里编译，热路径零成本。
#[cfg(all(test, feature = "fine-grained-splice"))]
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
        let active_tooltip = self.resolve_active_tooltip(now);
        let active_hover_popover = self.resolve_active_hover_popover();
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
                self.reduced_motion,
                visual_context,
                self.hovered_scrollbar,
                active_scrollbar,
                &widget_states,
                &self.select_open_states,
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
        let mut updated_computed = {
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

            // Phase 1 splice 资格判定：必须在「存入新 chunk」之前读取旧 root chunk。
            // 仅当单 root patch、且该 subtree 的合并 chunk 新旧各流命令数量完全一致、
            // 且新旧都不含 overlay/portal/scroll/focus 等结构性内容时，才记下 splice 计划。
            // 任一条件不满足 → splice_plan 为 None，走原 recompose。
            #[cfg(feature = "fine-grained-splice")]
            let splice_plan: Option<(WidgetId, ComputedScene<VM>)> =
                if roots.len() == 1 && patches.len() == 1 {
                    let target = roots[0];
                    match (
                        patches[0].cache.chunks.get(&target),
                        cached.scene_chunks.get(&target),
                    ) {
                        (Some(new_chunk), Some(old_chunk))
                            if new_chunk.is_simple_for_splice()
                                && old_chunk.is_simple_for_splice()
                                && new_chunk.scene_counts() == old_chunk.scene_counts()
                                && new_chunk.hit_regions.len() == old_chunk.hit_regions.len() =>
                        {
                            Some((target, new_chunk.clone()))
                        }
                        _ => None,
                    }
                } else {
                    None
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

            // Phase 1 splice 快路径：把目标子树的新 chunk 原地覆盖进每个严格祖先 chunk
            // 的稳定区间，跳过「逐级 recompose 向上重合成」。纯连接模型下，这与 recompose
            // 的结果逐字节等价（只有目标子树的字节变化，且新旧数量一致 → 后续偏移不动）。
            // 任一前置不满足（缺 chunk_parts / 子 chunk / 偏移越界）即 return false，
            // 由调用方 invalidate_computed_scene() 安全回退整帧重收集。
            #[cfg(feature = "fine-grained-splice")]
            let did_splice = if let Some((target, new_chunk)) = splice_plan.as_ref() {
                match layout.scene_splice_ancestor_offsets(
                    *target,
                    &cached.scene_chunk_parts,
                    &cached.scene_chunks,
                ) {
                    Some(offsets) => {
                        let mut ok = true;
                        for (ancestor_id, offset, hit_offset, scroll_offset) in &offsets {
                            let Some(ancestor_chunk) = cached.scene_chunks.get_mut(ancestor_id)
                            else {
                                ok = false;
                                break;
                            };
                            if !ancestor_chunk.splice_chunk_in_place(
                                offset,
                                *hit_offset,
                                *scroll_offset,
                                new_chunk,
                            ) {
                                ok = false;
                                break;
                            }
                        }
                        if !ok {
                            return false;
                        }
                        #[cfg(test)]
                        splice_probe::record_hit();
                        true
                    }
                    None => false,
                }
            } else {
                false
            };
            #[cfg(not(feature = "fine-grained-splice"))]
            let did_splice = false;

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

            let root_clone_started_at = text_profile_enabled().then_some(Instant::now());
            let Some(mut root_chunk) = cached.scene_chunks.get(&layout.root_id()).cloned() else {
                return false;
            };
            root_chunk.finalize_portals(viewport);
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
                cached.style_sheet_version = self.config.style_sheet.version();
                cached.density = self.theme.density;
                cached.reduced_motion = self.reduced_motion;
                cached.text_scale_bits = cached.units.font_scale().to_bits();
                cached.animation_epoch = self.animation_epoch;
                cached.layout_animation_epoch = self.layout_animation_epoch;
                cached.scroll_epoch = self.scroll_epoch;
                cached.hover_epoch = self.hover_epoch;
                cached.text_input_epoch = self.text_input_epoch;
                cached.external_portal_revision = self.external_portal_revision;
                cached.hovered_scrollbar = self.hovered_scrollbar;
                cached.active_scrollbar = active_scrollbar;
            }
            cached.computed.clone()
        };
        self.append_external_portals_to_computed(&mut updated_computed, now);
        if let Some(cached) = self.cached_scene.as_mut() {
            cached
                .dependencies
                .merge_from(&updated_computed.dependencies);
            cached.computed = updated_computed.clone();
        }

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
