use super::*;
use crate::ui::layout::{Length, Value};

#[derive(Clone)]
pub(super) struct LayoutSlotBinding {
    pub(super) widget_id: WidgetId,
    pub(super) property: PropertySlot,
}

pub(super) fn is_layout_property_slot(property: PropertySlot) -> bool {
    matches!(
        property,
        PropertySlot::Width
            | PropertySlot::Height
            | PropertySlot::MinWidth
            | PropertySlot::MinHeight
            | PropertySlot::MaxWidth
            | PropertySlot::MaxHeight
            | PropertySlot::Margin
            | PropertySlot::Padding
            | PropertySlot::Grow
            | PropertySlot::Shrink
            | PropertySlot::Basis
            | PropertySlot::AspectRatio
            | PropertySlot::GridRow
            | PropertySlot::GridColumn
            | PropertySlot::Inset
            | PropertySlot::TextContent
    )
}

impl<VM: 'static> BoundRuntimeHandler<VM> {
    pub(super) fn rebuild_layout_slot_bindings(&mut self) {
        let bindings = self.build_layout_slot_bindings();
        let Some(cached) = self.cached_scene.as_mut() else {
            return;
        };
        cached.layout_slot_bindings = bindings;
    }

    fn build_layout_slot_bindings(&self) -> HashMap<(WidgetId, PropertySlot), LayoutSlotBinding> {
        let Some(cached) = self.cached_scene.as_ref() else {
            return HashMap::new();
        };
        let Some(layout) = cached.layout.as_ref() else {
            return HashMap::new();
        };
        let mut bindings = HashMap::new();
        for owner in cached.dependencies.property_owners() {
            if owner.phase != DependencyPhase::Layout {
                continue;
            }
            let Some(property) = owner.property else {
                continue;
            };
            if !is_layout_property_slot(property) {
                continue;
            }
            let widget_id = WidgetId::from_raw(owner.widget_id);
            if layout.path_for(widget_id).is_none() {
                continue;
            }
            bindings.insert(
                (widget_id, property),
                LayoutSlotBinding {
                    widget_id,
                    property,
                },
            );
        }
        bindings
    }

    pub(super) fn try_update_reactive_layout_slots(
        &mut self,
        targets: &[(WidgetId, PropertySlot)],
        now: Instant,
    ) -> bool {
        if targets.is_empty()
            || targets
                .iter()
                .any(|(_, property)| !is_layout_property_slot(*property))
        {
            return false;
        }

        let mut unique_targets = Vec::with_capacity(targets.len());
        let mut seen_targets = HashSet::new();
        for &(widget_id, property) in targets {
            if seen_targets.insert((widget_id, property)) {
                unique_targets.push((widget_id, property));
            }
        }

        let Some(cached) = self.cached_scene.as_ref() else {
            return false;
        };
        for (widget_id, property) in &unique_targets {
            let Some(binding) = cached.layout_slot_bindings.get(&(*widget_id, *property)) else {
                return false;
            };
            if binding.widget_id != *widget_id || binding.property != *property {
                return false;
            }
        }

        let mut widget_ids = Vec::new();
        let mut seen_widgets = HashSet::new();
        for (widget_id, _) in &unique_targets {
            if seen_widgets.insert(*widget_id) {
                widget_ids.push(*widget_id);
            }
        }
        let scene_roots = {
            let Some(cached) = self.cached_scene.as_ref() else {
                return false;
            };
            let Some(layout) = cached.layout.as_ref() else {
                return false;
            };
            let scene_root_candidates = unique_targets
                .iter()
                .map(|(widget_id, property)| {
                    self.layout_slot_scene_root(layout, *widget_id, *property)
                })
                .collect::<HashSet<_>>();
            let roots = self.highest_layout_roots_smallvec(layout, &scene_root_candidates);
            if roots.is_empty() {
                return false;
            }
            roots.into_iter().collect::<Vec<_>>()
        };
        let owner_ids = widget_ids
            .iter()
            .map(|widget_id| widget_id.raw())
            .collect::<HashSet<_>>();
        self.invalidation
            .remove_reactive_targets_for_widget_phase(&owner_ids, DependencyPhase::Layout);

        let theme = self.animated_theme(now);
        let viewport = self.viewport_rect();
        {
            let Some(cached) = self.cached_scene.as_mut() else {
                return false;
            };
            let Some(layout) = cached.layout.as_mut() else {
                return false;
            };
            let mut dependencies = cached.dependencies.clone();
            if layout
                .update_layout_style_slots(
                    &widget_ids,
                    &self.font_manager,
                    &theme,
                    &self.media_manager,
                    &mut self.animation_engine,
                    viewport,
                    now,
                )
                .is_err()
            {
                return false;
            }
            dependencies.remove_widget_phase_owners(&owner_ids, DependencyPhase::Layout);
            dependencies.merge_from(layout.dependencies());
            cached.dependencies = dependencies;
            cached.layout_valid = true;
            cached.computed_valid = false;
        }

        if !self.patch_cached_scene_for_roots(&scene_roots, now, true) {
            self.invalidate_computed_scene();
            return false;
        }
        self.rebuild_layout_slot_bindings();
        self.rebuild_strict_capability_report();
        true
    }

    fn layout_slot_scene_root(
        &self,
        layout: &ResolvedSceneLayout<VM>,
        widget_id: WidgetId,
        property: PropertySlot,
    ) -> WidgetId {
        let mut current = match property {
            PropertySlot::Padding => widget_id,
            _ => layout.parent_of(widget_id).unwrap_or(widget_id),
        };

        loop {
            if self.layout_slot_is_scene_patch_boundary(layout, current) {
                return current;
            }
            let Some(parent) = layout.parent_of(current) else {
                return current;
            };
            current = parent;
        }
    }

    fn layout_slot_is_scene_patch_boundary(
        &self,
        layout: &ResolvedSceneLayout<VM>,
        widget_id: WidgetId,
    ) -> bool {
        if widget_id == layout.root_id() {
            return true;
        }
        let Some(resolved) = layout.resolved_widget(widget_id) else {
            return false;
        };
        layout_length_is_definite_px(resolved.layout.width.as_ref())
            && layout_length_is_definite_px(resolved.layout.height.as_ref())
    }
}

fn layout_length_is_definite_px(value: Option<&Value<Length>>) -> bool {
    matches!(value.map(Value::resolve_untracked), Some(Length::Px(_)))
}
