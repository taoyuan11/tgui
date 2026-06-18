use super::*;
use smallvec::SmallVec;

impl<VM: 'static> BoundRuntimeHandler<VM> {
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
        let roots = self.highest_layout_roots_smallvec(layout, &affected_ids);
        if roots.is_empty() {
            return false;
        }
        self.patch_cached_scene_for_roots(&roots, now, true)
    }

    pub(super) fn patch_animation_scene_widgets(
        &mut self,
        widget_ids: &[u64],
        now: Instant,
    ) -> bool {
        let Some(cached) = self.cached_scene.as_ref() else {
            return false;
        };
        let Some(layout) = cached.layout.as_ref() else {
            return false;
        };

        let mut affected_ids: HashSet<WidgetId> = HashSet::new();
        for widget_id in widget_ids {
            affected_ids.insert(WidgetId::from_raw(*widget_id));
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
