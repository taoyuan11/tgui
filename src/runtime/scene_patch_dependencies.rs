use super::*;

impl<VM: 'static> BoundRuntimeHandler<VM> {
    pub(super) fn computed_scene_has_text_input(
        computed: &ComputedScene<VM>,
        widget_id: WidgetId,
    ) -> bool {
        computed
            .hit_regions
            .iter()
            .chain(computed.overlay_hit_regions.iter())
            .any(|region| {
                matches!(
                    &region.interaction,
                    crate::ui::widget::HitInteraction::TextInput { id, .. } if *id == widget_id
                )
            })
    }

    #[allow(dead_code)]
    pub(super) fn patch_cached_layout_for_dependencies(
        &mut self,
        dirty_dependencies: &HashSet<crate::foundation::binding::DependencyId>,
        now: Instant,
    ) -> bool {
        let Some(cached) = self.cached_scene.as_ref() else {
            return false;
        };
        let Some(layout) = cached.layout.as_ref() else {
            return false;
        };

        let mut affected_ids = HashSet::new();
        for dependency in dirty_dependencies {
            let Some(owners) = cached.dependencies.owners_for(*dependency) else {
                continue;
            };
            for owner in owners {
                if matches!(
                    owner.phase,
                    DependencyPhase::Structure | DependencyPhase::Layout
                ) {
                    affected_ids.insert(WidgetId::from_raw(owner.widget_id));
                }
            }
        }
        if affected_ids.is_empty() {
            return false;
        }

        let roots = self.highest_layout_roots_smallvec(layout, &affected_ids);
        if roots.is_empty() {
            return false;
        }
        self.patch_cached_layout_for_roots(&roots, now)
    }

    #[allow(dead_code)]
    pub(super) fn patch_cached_scene_for_dependencies(
        &mut self,
        dirty_dependencies: &HashSet<crate::foundation::binding::DependencyId>,
        now: Instant,
    ) -> bool {
        let Some(cached) = self.cached_scene.as_ref() else {
            return false;
        };
        let Some(layout) = cached.layout.as_ref() else {
            return false;
        };

        let mut affected_ids = HashSet::new();
        for dependency in dirty_dependencies {
            let Some(owners) = cached.dependencies.owners_for(*dependency) else {
                continue;
            };
            for owner in owners {
                if owner.phase == DependencyPhase::Scene {
                    affected_ids.insert(WidgetId::from_raw(owner.widget_id));
                }
            }
        }
        if affected_ids.is_empty() {
            return false;
        }

        let roots = self.highest_layout_roots_smallvec(layout, &affected_ids);
        if roots.is_empty() {
            return false;
        }
        self.patch_cached_scene_for_roots(&roots, now, false)
    }
}
