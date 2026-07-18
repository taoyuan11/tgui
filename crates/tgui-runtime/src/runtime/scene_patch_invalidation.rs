use super::*;
use crate::ui::widget::ResolvedWidgetKind;
use smallvec::SmallVec;

impl<VM: 'static> BoundRuntimeHandler<VM> {
    pub(super) fn strict_reactive_tree(&self) -> bool {
        self.widget_tree
            .as_ref()
            .map(WidgetTree::is_strict_reactive)
            .unwrap_or(false)
    }

    pub(super) fn invalidate_cached_scene_for_dependencies(
        &mut self,
        dirty_kind: DirtyDependencySet,
        dirty_dependencies: &HashSet<crate::foundation::binding::DependencyId>,
        reactive_targets: &[ReactiveTarget],
        reactive_processed_signals: usize,
        now: Instant,
    ) -> &'static str {
        let started_at = text_profile_enabled().then_some(Instant::now());
        if matches!(dirty_kind, DirtyDependencySet::Clean) {
            return "clean";
        }
        let strict_reactive = self.strict_reactive_tree();
        {
            let Some(cached) = self.cached_scene.as_ref() else {
                return "no_cache";
            };
            if matches!(dirty_kind, DirtyDependencySet::Global)
                || cached.dependencies.has_global_dependency()
            {
                if strict_reactive {
                    return "strict_reactive_global_rejected";
                }
                self.invalidate_scene_with_reason("global_dependency_rebuild");
                return "global_full_rebuild";
            }

            if cached.layout.is_none() {
                self.invalidate_scene_with_reason("layout_missing");
                return "layout_missing";
            }
        }

        if reactive_processed_signals > 0 {
            if let Some(action) = self.invalidate_cached_scene_for_reactive_targets(
                reactive_targets,
                dirty_dependencies,
                now,
            ) {
                if let Some(started_at) = started_at {
                    log_text_profile(
                        "textarea_invalidation",
                        started_at.elapsed(),
                        format!(
                            "dirty_kind={} dirty_dependencies={} reactive_targets={} action={}",
                            dirty_dependency_set_label(dirty_kind),
                            dirty_dependencies.len(),
                            reactive_targets.len(),
                            action
                        ),
                    );
                }
                return action;
            }
        }

        let Some(cached) = self.cached_scene.as_ref() else {
            return "no_cache";
        };
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
            if strict_reactive {
                return "strict_reactive_detached_rejected";
            }
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
            if strict_reactive {
                return "strict_reactive_layout_rejected";
            }
            let roots = self.highest_layout_roots_smallvec(layout, &layout_affected_ids);
            if roots.is_empty() {
                "unrelated"
            } else {
                let mut scene_ids = layout_affected_ids.clone();
                scene_ids.extend(scene_affected_ids.iter().copied());
                let scene_roots = self.highest_layout_roots_smallvec(layout, &scene_ids);

                if self.patch_cached_layout_for_roots(&roots, now) {
                    if self.patch_cached_scene_for_roots(&scene_roots, now, true) {
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
            if strict_reactive {
                return "strict_reactive_scene_rejected";
            }
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

    fn invalidate_cached_scene_for_reactive_targets(
        &mut self,
        reactive_targets: &[ReactiveTarget],
        dirty_dependencies: &HashSet<crate::foundation::binding::DependencyId>,
        now: Instant,
    ) -> Option<&'static str> {
        let strict_reactive = self.strict_reactive_tree();
        let Some(cached) = self.cached_scene.as_ref() else {
            return Some("no_cache");
        };
        let Some(layout) = cached.layout.as_ref() else {
            self.invalidate_scene_with_reason("layout_missing");
            return Some("layout_missing");
        };

        let mut layout_affected_ids = HashSet::new();
        let mut structure_affected_ids = HashSet::new();
        let mut scene_affected_ids = HashSet::new();
        let mut scene_property_targets = SmallVec::<[(WidgetId, PropertySlot); 16]>::new();
        let mut layout_property_targets = SmallVec::<[(WidgetId, PropertySlot); 16]>::new();
        let mut saw_scene_owner = false;
        let mut all_scene_owners_are_property_scoped = true;
        let mut all_layout_owners_are_layout_property_scoped = true;
        for target in reactive_targets {
            let owner = match *target {
                ReactiveTarget::Owner(owner) => owner,
                #[cfg(test)]
                ReactiveTarget::Custom(_) => return None,
            };
            let widget_id = WidgetId::from_raw(owner.widget_id);
            if layout.path_for(widget_id).is_none() {
                if strict_reactive {
                    return Some("strict_reactive_detached_rejected");
                }
                self.invalidate_computed_scene();
                return Some("reactive_detached_scene_dependency_recollect");
            }
            match owner.phase {
                DependencyPhase::Structure => {
                    all_layout_owners_are_layout_property_scoped = false;
                    structure_affected_ids.insert(widget_id);
                    layout_affected_ids.insert(widget_id);
                }
                DependencyPhase::Layout => {
                    if owner.phase == DependencyPhase::Layout
                        && owner.property == Some(PropertySlot::Offset)
                        && layout.can_patch_layout_dependency_as_scene(widget_id)
                    {
                        scene_property_targets.push((widget_id, PropertySlot::Offset));
                    }
                    if let Some(property) = owner.property {
                        if is_layout_property_slot(property) {
                            layout_property_targets.push((widget_id, property));
                        } else {
                            all_layout_owners_are_layout_property_scoped = false;
                        }
                    } else {
                        all_layout_owners_are_layout_property_scoped = false;
                    }
                    layout_affected_ids.insert(widget_id);
                }
                DependencyPhase::Scene => {
                    saw_scene_owner = true;
                    all_scene_owners_are_property_scoped &= owner.property.is_some();
                    if let Some(property) = owner.property {
                        scene_property_targets.push((widget_id, property));
                    }
                    scene_affected_ids.insert(widget_id);
                }
            }
        }

        if !strict_reactive {
            if !dirty_dependencies.is_empty() {
                super::action_stats::record("reactive_collect_dependency_lookup");
            }
            for dependency in dirty_dependencies {
                let Some(owners) = cached.dependencies.owners_for(*dependency) else {
                    continue;
                };
                for owner in owners {
                    let widget_id = WidgetId::from_raw(owner.widget_id);
                    if layout.path_for(widget_id).is_none() {
                        continue;
                    }
                    match owner.phase {
                        DependencyPhase::Structure => {
                            all_layout_owners_are_layout_property_scoped = false;
                            structure_affected_ids.insert(widget_id);
                            layout_affected_ids.insert(widget_id);
                        }
                        DependencyPhase::Layout => {
                            if owner.property == Some(PropertySlot::Offset)
                                && layout.can_patch_layout_dependency_as_scene(widget_id)
                            {
                                scene_property_targets.push((widget_id, PropertySlot::Offset));
                            }
                        }
                        DependencyPhase::Scene => {
                            if let Some(property) = owner.property {
                                scene_property_targets.push((widget_id, property));
                            }
                        }
                    }
                }
            }
        }

        let scene_only_layout_ids = layout_affected_ids
            .iter()
            .copied()
            .filter(|widget_id| layout.can_patch_layout_dependency_as_scene(*widget_id))
            .collect::<SmallVec<[WidgetId; 16]>>();
        for widget_id in scene_only_layout_ids.iter().copied() {
            if cached.computed.transform_records.contains_key(&widget_id) {
                scene_property_targets.push((widget_id, PropertySlot::Offset));
            }
        }
        for widget_id in scene_only_layout_ids {
            layout_affected_ids.remove(&widget_id);
            scene_affected_ids.insert(widget_id);
        }

        if strict_reactive && !structure_affected_ids.is_empty() {
            let roots = self.highest_layout_roots_smallvec(layout, &structure_affected_ids);
            if roots.is_empty() {
                return Some("reactive_unrelated");
            }
            if self.patch_cached_layout_for_roots(&roots, now)
                && self.patch_cached_scene_for_roots(&roots, now, true)
            {
                return Some("reactive_structure_slot_update");
            }
            self.invalidate_scene_with_reason("reactive_structure_patch_failed");
            return Some("strict_reactive_layout_rejected");
        }

        if !layout_affected_ids.is_empty() {
            let roots = self.highest_layout_roots_smallvec(layout, &layout_affected_ids);
            let mut scene_ids = layout_affected_ids.clone();
            scene_ids.extend(scene_affected_ids.iter().copied());
            let scene_roots = self.highest_layout_roots_smallvec(layout, &scene_ids);
            let layout_property_ids = layout_property_targets
                .iter()
                .map(|(widget_id, _)| *widget_id)
                .collect::<HashSet<_>>();
            if all_layout_owners_are_layout_property_scoped
                && layout_affected_ids
                    .iter()
                    .all(|widget_id| layout_property_ids.contains(widget_id))
                && self.try_update_reactive_layout_slots(&layout_property_targets, now)
            {
                return Some("reactive_layout_slot_update");
            }
            if strict_reactive {
                return Some("strict_reactive_layout_rejected");
            }
            if roots.is_empty() {
                return Some("reactive_unrelated");
            }
            if self.patch_cached_layout_for_roots(&roots, now) {
                if self.patch_cached_scene_for_roots(&scene_roots, now, true) {
                    Some("reactive_layout_scene_patch")
                } else {
                    self.invalidate_computed_scene();
                    Some("reactive_layout_subtree_patch_scene_recollect")
                }
            } else {
                self.invalidate_scene_with_reason("reactive_layout_patch_failed");
                Some("reactive_global_full_rebuild")
            }
        } else if !scene_affected_ids.is_empty() {
            let roots = self.highest_layout_roots_smallvec(layout, &scene_affected_ids);
            if roots.is_empty() {
                return Some("reactive_unrelated");
            }
            // ToastQueue intentionally changes the detached overlay command structure while the
            // host's root layout stays fixed. It cannot be expressed as a fixed property slot,
            // but it does have an explicit bounded retained plan: recollect exactly the ToastHost
            // subtree and splice/recompose it through the normal scene patcher. Keep every other
            // unscoped strict scene dependency rejected.
            let strict_toast_scene_patch = strict_reactive
                && saw_scene_owner
                && scene_affected_ids.iter().all(|widget_id| {
                    layout.resolved_widget(*widget_id).is_some_and(|widget| {
                        matches!(&widget.kind, ResolvedWidgetKind::ToastHost { .. })
                    })
                });
            // Canvas opacity is property-scoped, but a canvas may emit an arbitrary
            // number of meshes, textures, text primitives, and composite commands.
            // There is no fixed retained slot layout that can update all of those
            // primitives safely.  Keep strict reactive correctness by allowing the
            // narrowly bounded canvas subtree recollect for opacity only; all other
            // unsupported strict scene properties continue to be rejected below.
            let strict_canvas_scene_patch = strict_reactive
                && saw_scene_owner
                && all_scene_owners_are_property_scoped
                && !scene_property_targets.is_empty()
                && scene_property_targets
                    .iter()
                    .all(|(_, property)| *property == PropertySlot::Opacity)
                && scene_affected_ids.iter().all(|widget_id| {
                    layout.resolved_widget(*widget_id).is_some_and(|widget| {
                        matches!(&widget.kind, ResolvedWidgetKind::Canvas { .. })
                    })
                });
            // These surface properties can affect an arbitrary set of primitives owned by one
            // leaf widget, so a fixed slot plan is intentionally unavailable. Their fallback is
            // nevertheless bounded to that leaf subtree and uses the ordinary scene collector,
            // making it suitable for strict mode without risking a stale retained cache.
            let strict_surface_property_scene_patch = strict_reactive
                && saw_scene_owner
                && all_scene_owners_are_property_scoped
                && !scene_property_targets.is_empty()
                && scene_property_targets.iter().all(|(widget_id, property)| {
                    layout.resolved_widget(*widget_id).is_some_and(|widget| {
                        matches!(
                            (&widget.kind, property),
                            (
                                ResolvedWidgetKind::Container { .. },
                                PropertySlot::Background
                                    | PropertySlot::BackgroundBlur
                                    | PropertySlot::BorderColor
                                    | PropertySlot::Offset
                                    | PropertySlot::Opacity
                                    | PropertySlot::Scale
                            ) | (ResolvedWidgetKind::Text { .. }, PropertySlot::Opacity)
                                | (
                                    ResolvedWidgetKind::Image { .. },
                                    PropertySlot::BorderWidth
                                        | PropertySlot::BorderRadius
                                        | PropertySlot::Opacity
                                )
                                | (
                                    ResolvedWidgetKind::Canvas { .. },
                                    PropertySlot::BorderWidth | PropertySlot::BorderRadius
                                )
                        )
                    })
                });
            let text_input_patch = scene_affected_ids
                .iter()
                .all(|widget_id| Self::computed_scene_has_text_input(&cached.computed, *widget_id));
            // A property-scoped dependency is already retained by the resolved widget as a
            // `Value::Signal` (or an equivalent runtime resolver). Re-resolving the source
            // element here can freeze the current value into a fresh resolved snapshot before
            // the scene collector consumes it, which leaves unsupported-slot fallbacks stale
            // (notably Image/Canvas border geometry and decorated Text opacity). Recollect the
            // existing resolved subtree with current runtime state instead. Unscoped scene
            // dependencies still take the source re-resolution path because they may change the
            // widget's scene structure.
            let sync_runtime_scene_state =
                text_input_patch || (saw_scene_owner && all_scene_owners_are_property_scoped);
            if scene_property_targets.is_empty() {
                for widget_id in scene_affected_ids.iter().copied() {
                    if cached.computed.transform_records.contains_key(&widget_id) {
                        scene_property_targets.push((widget_id, PropertySlot::Offset));
                    }
                }
            }
            let offset_property_ids = scene_property_targets
                .iter()
                .filter(|(_, property)| *property == PropertySlot::Offset)
                .map(|(widget_id, _)| *widget_id)
                .collect::<HashSet<_>>();
            let transform_record_targets = scene_property_targets
                .iter()
                .copied()
                .filter(|(widget_id, property)| {
                    *property == PropertySlot::Offset
                        && cached.computed.transform_records.contains_key(widget_id)
                })
                .collect::<SmallVec<[(WidgetId, PropertySlot); 16]>>();
            let strict_missing_direct_slot = strict_reactive
                && cached
                    .strict_capability_report
                    .as_ref()
                    .is_some_and(|report| {
                        scene_property_targets.iter().any(|(widget_id, property)| {
                            report.entries.iter().any(|entry| {
                                entry.owner.widget_id == widget_id.raw()
                                    && entry.owner.property == Some(*property)
                                    && entry.kind == StrictCapabilityKind::DirectSlot
                            }) && !cached
                                .reactive_slot_bindings
                                .contains_key(&(*widget_id, *property))
                        })
                    });
            if strict_missing_direct_slot {
                return Some("strict_reactive_scene_rejected");
            }
            if saw_scene_owner
                && !transform_record_targets.is_empty()
                && scene_property_targets
                    .iter()
                    .all(|(_, property)| *property == PropertySlot::Offset)
                && scene_affected_ids
                    .iter()
                    .all(|widget_id| offset_property_ids.contains(widget_id))
                && self.try_update_reactive_transform_records(&transform_record_targets, now)
            {
                return Some("reactive_transform_record_update");
            }
            if saw_scene_owner
                && all_scene_owners_are_property_scoped
                && self.try_patch_reactive_property_slots(&scene_property_targets, now)
            {
                return Some("reactive_property_slot_write");
            }
            if strict_reactive
                && saw_scene_owner
                && !strict_toast_scene_patch
                && !strict_canvas_scene_patch
                && !strict_surface_property_scene_patch
            {
                return Some("strict_reactive_scene_rejected");
            }
            if self.patch_cached_scene_for_roots(&roots, now, sync_runtime_scene_state) {
                if saw_scene_owner && all_scene_owners_are_property_scoped {
                    Some("reactive_property_scene_patch")
                } else if text_input_patch {
                    Some("reactive_text_input_scene_patch")
                } else {
                    Some("reactive_scene_patch")
                }
            } else {
                self.invalidate_computed_scene();
                Some("reactive_scene_full_recollect")
            }
        } else {
            Some("reactive_clean")
        }
    }
}
