use super::*;

impl<VM: 'static> BoundRuntimeHandler<VM> {
    pub(super) fn highest_layout_roots(
        &self,
        layout: &ResolvedSceneLayout<VM>,
        affected_ids: &HashSet<WidgetId>,
    ) -> Vec<WidgetId> {
        let mut roots = affected_ids
            .iter()
            .copied()
            .filter(|widget_id| {
                let mut parent = layout.parent_of(*widget_id);
                while let Some(current) = parent {
                    if affected_ids.contains(&current) {
                        return false;
                    }
                    parent = layout.parent_of(current);
                }
                true
            })
            .collect::<Vec<_>>();
        roots.sort_by_key(|widget_id| std::cmp::Reverse(layout.depth_of(*widget_id)));
        roots
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
        let roots = self.highest_layout_roots(layout, &affected_ids);
        if roots.is_empty() {
            return false;
        }
        self.patch_cached_scene_for_roots(&roots, now, true)
    }
}
