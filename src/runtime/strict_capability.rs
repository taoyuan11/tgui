use super::*;
use crate::foundation::binding::{DependencyOwner, DependencyPhase};

impl<VM: 'static> BoundRuntimeHandler<VM> {
    pub(super) fn rebuild_strict_capability_report(&mut self) {
        let strict = self.strict_reactive_tree();
        let Some(cached) = self.cached_scene.as_mut() else {
            return;
        };
        if !strict {
            cached.strict_capability_report = None;
            return;
        }
        let Some(layout) = cached.layout.as_ref() else {
            let report = StrictCapabilityReport {
                entries: Vec::new(),
                has_global_reject_policy: cached.dependencies.has_global_dependency(),
            };
            report.enforce_no_missing_plans();
            cached.strict_capability_report = Some(report);
            return;
        };

        let mut owners = cached
            .dependencies
            .all_owners()
            .into_iter()
            .collect::<Vec<_>>();
        owners.sort_by_key(strict_capability_owner_key);
        let entries = owners
            .into_iter()
            .map(|owner| {
                let widget_id = WidgetId::from_raw(owner.widget_id);
                let kind = if layout.path_for(widget_id).is_none() {
                    StrictCapabilityKind::DetachedReject
                } else {
                    match owner.phase {
                        DependencyPhase::Structure => StrictCapabilityKind::StructureSlot,
                        DependencyPhase::Layout => {
                            strict_layout_capability_kind(cached, widget_id, owner)
                        }
                        DependencyPhase::Scene => {
                            strict_scene_capability_kind(cached, widget_id, owner)
                        }
                    }
                };
                StrictCapabilityEntry { owner, kind }
            })
            .collect();

        let report = StrictCapabilityReport {
            entries,
            has_global_reject_policy: cached.dependencies.has_global_dependency(),
        };
        report.enforce_no_missing_plans();
        cached.strict_capability_report = Some(report);
    }
}

fn strict_layout_capability_kind<VM>(
    cached: &CachedScene<VM>,
    widget_id: WidgetId,
    owner: DependencyOwner,
) -> StrictCapabilityKind {
    let Some(property) = owner.property else {
        return StrictCapabilityKind::LayoutReject;
    };
    if cached
        .layout_slot_bindings
        .contains_key(&(widget_id, property))
    {
        return StrictCapabilityKind::LayoutSlot;
    }
    if property == PropertySlot::Offset {
        return strict_scene_capability_kind(cached, widget_id, owner);
    }
    StrictCapabilityKind::LayoutReject
}

fn strict_scene_capability_kind<VM>(
    cached: &CachedScene<VM>,
    widget_id: WidgetId,
    owner: DependencyOwner,
) -> StrictCapabilityKind {
    let Some(property) = owner.property else {
        return StrictCapabilityKind::SceneReject;
    };
    if cached
        .reactive_slot_bindings
        .contains_key(&(widget_id, property))
    {
        return StrictCapabilityKind::DirectSlot;
    }
    if property == PropertySlot::Offset
        && cached.computed.transform_records.contains_key(&widget_id)
    {
        return StrictCapabilityKind::TransformRecord;
    }
    StrictCapabilityKind::SceneReject
}

fn strict_capability_owner_key(owner: &DependencyOwner) -> (u64, u8, u8, u8) {
    (
        owner.widget_id,
        match owner.phase {
            DependencyPhase::Structure => 0,
            DependencyPhase::Layout => 1,
            DependencyPhase::Scene => 2,
        },
        u8::from(owner.property.is_some()),
        owner.property.map(|property| property as u8).unwrap_or(0),
    )
}
