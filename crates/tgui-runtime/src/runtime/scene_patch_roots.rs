use super::*;
use smallvec::SmallVec;

impl<VM: 'static> BoundRuntimeHandler<VM> {
    pub(super) fn patch_animation_refresh(
        &mut self,
        animation_refresh: &AnimationRefresh,
        now: Instant,
    ) -> bool {
        let property_targets = animation_refresh.scene_property_targets.as_slice();
        let layout_property_targets = animation_refresh.layout_property_targets.as_slice();
        let direct_layout_patch = !layout_property_targets.is_empty()
            && animation_refresh.scene_widget_ids.is_empty()
            && !animation_refresh.has_unscoped_layout_changes
            && !self.animation_layout_refresh_is_dense(&animation_refresh.layout_widget_ids)
            // Recollecting the layout root preserves no scene or renderer work and adds patch
            // bookkeeping to the same whole-tree collection. Animation has a safe full-layout
            // fallback, unlike generic strict-reactive updates which still use the shared writer.
            && !self.animation_layout_targets_require_root_recollect(layout_property_targets)
            && self.try_update_reactive_layout_slots(layout_property_targets, now);
        if direct_layout_patch {
            super::action_stats::record("animation_reactive_layout_slot_update");
        }
        let direct_property_patch = !direct_layout_patch
            && animation_refresh.layout_widget_ids.is_empty()
            && !property_targets.is_empty()
            && !animation_refresh.has_unscoped_scene_changes
            && (self.try_update_canonical_reactive_transform_records(property_targets, now)
                || self.try_patch_canonical_reactive_property_slots(property_targets, now));
        if direct_property_patch {
            super::action_stats::record("animation_reactive_property_slot_write");
            if let Some(cached) = self.cached_scene.as_mut() {
                cached.computed_valid = true;
                cached.animation_epoch = self.animation_epoch;
                cached.layout_animation_epoch = self.layout_animation_epoch;
                cached.accessibility_animation_epoch = self.accessibility_animation_epoch;
            }
        }
        direct_layout_patch
            || direct_property_patch
            || (animation_refresh.layout_widget_ids.is_empty()
                && !animation_refresh.scene_widget_ids.is_empty()
                && self.patch_animation_scene_widgets_inner(
                    &animation_refresh.scene_widget_ids,
                    now,
                    // A failed retained property write can require a topology-changing bounded
                    // recollect (for example BorderWidth making an inset background appear).
                    // Re-resolving the source element on that path freezes the current animated
                    // value into the resolved snapshot and drops its Signal dependency. Recollect
                    // the existing runtime-bound subtree whenever every scene change is already
                    // property-scoped; unscoped animation changes still need source resolution.
                    !property_targets.is_empty() && !animation_refresh.has_unscoped_scene_changes,
                ))
    }

    pub(super) fn active_slider_value_override(&self) -> Option<(WidgetId, f32)> {
        self.active_slider_drag
            .as_ref()
            .map(|drag| (drag.widget_id, drag.current_value))
    }

    pub(super) fn patch_active_slider_scene(&mut self, now: Instant) -> bool {
        let Some(drag) = self.active_slider_drag.as_ref() else {
            return false;
        };
        let Some(cached) = self.cached_scene.as_ref() else {
            return false;
        };
        let Some(layout) = cached.layout.as_ref() else {
            return false;
        };
        let mut affected_ids = HashSet::new();
        affected_ids.insert(drag.widget_id);
        affected_ids.extend(
            cached
                .dependencies
                .owners_sharing_widget_property(
                    drag.widget_id.raw(),
                    DependencyPhase::Scene,
                    PropertySlot::SliderValue,
                )
                .into_iter()
                .map(|owner| WidgetId::from_raw(owner.widget_id))
                .filter(|widget_id| layout.resolved_widget(*widget_id).is_some()),
        );
        let roots = self.highest_layout_roots_smallvec(layout, &affected_ids);
        if roots.is_empty() {
            return false;
        }
        self.patch_cached_scene_for_roots(&roots, now, true)
    }

    #[cfg(test)]
    pub(super) fn patch_animation_scene_widgets(
        &mut self,
        widget_ids: &[u64],
        now: Instant,
    ) -> bool {
        self.patch_animation_scene_widgets_inner(widget_ids, now, false)
    }

    fn patch_animation_scene_widgets_inner(
        &mut self,
        widget_ids: &[u64],
        now: Instant,
        sync_runtime_scene_state: bool,
    ) -> bool {
        let Some(cached) = self.cached_scene.as_ref() else {
            return false;
        };
        let Some(layout) = cached.layout.as_ref() else {
            return false;
        };

        if widget_ids.is_empty() {
            return false;
        }

        // `AnimationRefresh` canonicalizes these IDs once after all typed animation stores have
        // refreshed. Avoid rebuilding an equivalent HashSet on every animation frame; parent
        // membership is a binary search over the already-sorted slice.
        let roots = layout.highest_roots_from_sorted_raw_ids(widget_ids);
        if roots.is_empty() {
            return false;
        }

        self.patch_cached_scene_for_roots(&roots, now, sync_runtime_scene_state)
    }

    pub(super) fn highest_layout_roots_smallvec(
        &self,
        layout: &ResolvedSceneLayout<VM>,
        affected_ids: &HashSet<WidgetId>,
    ) -> SmallVec<[WidgetId; 16]> {
        let mut roots = SmallVec::<[WidgetId; 16]>::new();
        for widget_id in affected_ids.iter().copied() {
            let mut parent = layout.parent_of(widget_id);
            let mut is_highest = true;
            while let Some(current) = parent {
                if affected_ids.contains(&current) {
                    is_highest = false;
                    break;
                }
                parent = layout.parent_of(current);
            }
            if is_highest {
                roots.push(widget_id);
            }
        }
        roots.sort_by_key(|widget_id| std::cmp::Reverse(layout.depth_of(*widget_id)));
        roots
    }
}
