use super::*;

struct RadioGroupKeyboardTarget<VM> {
    id: WidgetId,
    group: crate::ui::widget::RadioGroupInteraction,
    on_change: Option<ValueCommand<VM, bool>>,
    current: bool,
    focus: Option<crate::ui::widget::FocusTargetMeta<VM>>,
}

impl<VM> Clone for RadioGroupKeyboardTarget<VM> {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            group: self.group,
            on_change: self.on_change.clone(),
            current: self.current,
            focus: self.focus.clone(),
        }
    }
}

impl<VM: 'static> BoundRuntimeHandler<VM> {
    fn focused_radio_group_target(&mut self) -> Option<RadioGroupKeyboardTarget<VM>> {
        let focused_id = self.focused_widget_id()?;
        let computed = self.computed_scene();
        computed
            .hit_regions
            .iter()
            .chain(computed.overlay_hit_regions.iter())
            .find_map(|region| match &region.interaction {
                HitInteraction::Radio {
                    id,
                    interactions,
                    on_change,
                    current,
                } if *id == focused_id => {
                    interactions
                        .radio_group
                        .map(|group| RadioGroupKeyboardTarget {
                            id: *id,
                            group,
                            on_change: on_change.clone(),
                            current: *current,
                            focus: region.focus.clone(),
                        })
                }
                _ => None,
            })
    }

    fn radio_targets_in_group(&mut self, group_id: WidgetId) -> Vec<RadioGroupKeyboardTarget<VM>> {
        let computed = self.computed_scene();
        let mut targets = computed
            .hit_regions
            .iter()
            .chain(computed.overlay_hit_regions.iter())
            .filter_map(|region| match &region.interaction {
                HitInteraction::Radio {
                    id,
                    interactions,
                    on_change,
                    current,
                } => interactions
                    .radio_group
                    .filter(|group| group.group_id == group_id)
                    .map(|group| RadioGroupKeyboardTarget {
                        id: *id,
                        group,
                        on_change: on_change.clone(),
                        current: *current,
                        focus: region.focus.clone(),
                    }),
                _ => None,
            })
            .collect::<Vec<_>>();
        targets.sort_by_key(|target| target.group.index);
        targets
    }

    pub(super) fn move_focused_radio_group(
        &mut self,
        direction: crate::ui::layout::Axis,
        step: i32,
    ) -> bool {
        let Some(current) = self.focused_radio_group_target() else {
            return false;
        };
        if current.group.direction != direction {
            return false;
        }

        let targets = self.radio_targets_in_group(current.group.group_id);
        let Some(current_position) = targets.iter().position(|target| target.id == current.id)
        else {
            return true;
        };
        if targets.len() <= 1 {
            return true;
        }
        let next_position = if step < 0 {
            current_position.checked_sub(1).unwrap_or(targets.len() - 1)
        } else {
            (current_position + 1) % targets.len()
        };
        self.focus_and_select_radio_group_target(targets[next_position].clone());
        true
    }

    fn focus_and_select_radio_group_target(&mut self, target: RadioGroupKeyboardTarget<VM>) {
        if let Some(focus) = target.focus {
            self.update_focus(
                Some(FocusedWidget {
                    widget_id: target.id,
                    scope_path: focus.scope_path,
                    on_blur: focus.on_blur.clone(),
                }),
                focus.on_focus,
                true,
            );
        }
        if !target.current {
            if let Some(command) = target.on_change {
                self.execute_value_command(&command, true);
            }
        }
    }
}
