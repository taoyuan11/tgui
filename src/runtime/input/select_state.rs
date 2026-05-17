use crate::foundation::view_model::{Command, ValueCommand};
use crate::ui::widget::{HitInteraction, WidgetId};

use super::{BoundRuntimeHandler, HoverTransitionHandler};

impl<VM: 'static> BoundRuntimeHandler<VM> {
    pub(in crate::runtime) fn resolved_select_open_state(
        &mut self,
        widget_id: WidgetId,
    ) -> Option<bool> {
        if let Some(open) = self.select_open_states.get(&widget_id).copied() {
            return Some(open);
        }
        let computed = self.computed_scene();
        computed
            .hit_regions
            .iter()
            .chain(computed.overlay_hit_regions.iter())
            .find_map(|region| match &region.interaction {
                HitInteraction::SelectTrigger { id, is_open, .. } if *id == widget_id => {
                    Some(*is_open)
                }
                _ => None,
            })
    }

    pub(in crate::runtime) fn set_select_open_state(
        &mut self,
        widget_id: WidgetId,
        open: bool,
        on_open_change: Option<&ValueCommand<VM, bool>>,
    ) -> bool {
        let previous = self
            .resolved_select_open_state(widget_id)
            .or_else(|| self.select_open_states.get(&widget_id).copied())
            .unwrap_or(false);
        if previous == open {
            return false;
        }

        self.select_open_states.insert(widget_id, open);
        if let Some(command) = on_open_change {
            self.execute_value_command(command, open);
        }
        self.invalidate_scene_with_reason("select_open_state");
        true
    }

    pub(in crate::runtime) fn close_all_open_selects_except(
        &mut self,
        keep_open: Option<WidgetId>,
    ) -> bool {
        let select_triggers: Vec<_> = self
            .computed_scene()
            .hit_regions
            .iter()
            .filter_map(|region| match &region.interaction {
                HitInteraction::SelectTrigger {
                    id, on_open_change, ..
                } => Some((*id, on_open_change.clone())),
                _ => None,
            })
            .collect();

        let mut changed = false;
        for (id, on_open_change) in select_triggers {
            if Some(id) == keep_open || !self.resolved_select_open_state(id).unwrap_or(false) {
                continue;
            }
            changed |= self.set_select_open_state(id, false, on_open_change.as_ref());
        }
        changed
    }
}

pub(in crate::runtime) struct HoverMoveOrTransition;

impl HoverMoveOrTransition {
    pub(in crate::runtime) fn into_transition<VM>(
        command: Command<VM>,
    ) -> HoverTransitionHandler<VM> {
        HoverTransitionHandler::Command(command)
    }
}
