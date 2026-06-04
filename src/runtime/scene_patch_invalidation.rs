use super::*;
use smallvec::SmallVec;

impl<VM: 'static> BoundRuntimeHandler<VM> {
    pub(super) fn invalidate_cached_scene_for_dependencies(
        &mut self,
        dirty_kind: DirtyDependencySet,
        dirty_dependencies: &HashSet<crate::foundation::binding::DependencyId>,
        now: Instant,
    ) -> &'static str {
        let started_at = text_profile_enabled().then_some(Instant::now());
        let Some(cached) = self.cached_scene.as_ref() else {
            return "no_cache";
        };
        if matches!(dirty_kind, DirtyDependencySet::Clean) {
            return "clean";
        }
        if matches!(dirty_kind, DirtyDependencySet::Global)
            || cached.dependencies.has_global_dependency()
        {
            self.invalidate_scene_with_reason("global_dependency_rebuild");
            return "global_full_rebuild";
        }

        let Some(layout) = cached.layout.as_ref() else {
            self.invalidate_scene_with_reason("layout_missing");
            return "layout_missing";
        };

        let mut layout_affected_ids = HashSet::new();
        let mut scene_affected_ids = HashSet::new();
        let mut detached_scene_dependency = false;
        for dependency in dirty_dependencies {
            let Some(owners) = cached.dependencies.owners_for(*dependency) else {
                continue;
            };
            for owner in owners {
                let widget_id = WidgetId::from_raw(owner.widget_id);
                if layout.path_for(widget_id).is_none() {
                    detached_scene_dependency = true;
                    continue;
                }
                match owner.phase {
                    DependencyPhase::Structure | DependencyPhase::Layout => {
                        layout_affected_ids.insert(widget_id);
                    }
                    DependencyPhase::Scene => {
                        scene_affected_ids.insert(widget_id);
                    }
                }
            }
        }

        if detached_scene_dependency {
            self.invalidate_computed_scene();
            return "detached_scene_dependency_recollect";
        }

        let scene_only_layout_ids = layout_affected_ids
            .iter()
            .copied()
            .filter(|widget_id| layout.can_patch_layout_dependency_as_scene(*widget_id))
            .collect::<SmallVec<[WidgetId; 16]>>();
        for widget_id in scene_only_layout_ids {
            layout_affected_ids.remove(&widget_id);
            scene_affected_ids.insert(widget_id);
        }

        let action = if !layout_affected_ids.is_empty() {
            let roots = self.highest_layout_roots_smallvec(layout, &layout_affected_ids);
            if roots.is_empty() {
                "unrelated"
            } else {
                let mut scene_ids = layout_affected_ids.clone();
                scene_ids.extend(scene_affected_ids.iter().copied());
                let scene_roots = self.highest_layout_roots_smallvec(layout, &scene_ids);

                if self.patch_cached_layout_for_roots(&roots, now) {
                    if self.patch_cached_scene_for_roots(&scene_roots, now, false) {
                        "layout_scene_subtree_patch"
                    } else {
                        self.invalidate_computed_scene();
                        "layout_subtree_patch_scene_recollect"
                    }
                } else {
                    self.invalidate_scene_with_reason("layout_patch_failed");
                    "global_full_rebuild"
                }
            }
        } else if !scene_affected_ids.is_empty() {
            if scene_affected_ids
                .iter()
                .all(|widget_id| Self::computed_scene_has_text_input(&cached.computed, *widget_id))
            {
                let roots = self.highest_layout_roots_smallvec(layout, &scene_affected_ids);
                if roots.is_empty() {
                    "unrelated"
                } else if self.patch_cached_scene_for_roots(&roots, now, true) {
                    "text_input_scene_patch"
                } else {
                    self.invalidate_computed_scene();
                    "text_input_scene_recollect"
                }
            } else {
                let roots = self.highest_layout_roots_smallvec(layout, &scene_affected_ids);
                if roots.is_empty() {
                    "unrelated"
                } else if self.patch_cached_scene_for_roots(&roots, now, false) {
                    "scene_subtree_patch"
                } else {
                    self.invalidate_computed_scene();
                    "scene_full_recollect"
                }
            }
        } else {
            "unrelated"
        };
        if let Some(started_at) = started_at {
            log_text_profile(
                "textarea_invalidation",
                started_at.elapsed(),
                format!(
                    "dirty_kind={} dirty_dependencies={} layout_affected={} scene_affected={} layout_ids={:?} scene_ids={:?} action={}",
                    dirty_dependency_set_label(dirty_kind),
                    dirty_dependencies.len(),
                    layout_affected_ids.len(),
                    scene_affected_ids.len(),
                    layout_affected_ids,
                    scene_affected_ids,
                    action
                ),
            );
        }
        action
    }
}
