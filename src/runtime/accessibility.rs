use super::*;
use crate::accessibility::{build_tree_update, widget_id_from_node};
use crate::foundation::binding::{TextChange, TextChangeSet};
use crate::ui::widget::HitInteraction;
use accesskit::{Action, ActionData, ActionRequest};

impl<VM: 'static> BoundRuntimeHandler<VM> {
    pub(in crate::runtime) fn initialize_accessibility_adapter(&mut self) {
        if self.accessibility_adapter.is_some() {
            return;
        }
        let Some(window) = self.window.as_ref() else {
            return;
        };
        self.accessibility_adapter = PlatformAccessibilityAdapter::new(
            window.as_ref(),
            self.accessibility_action_sender.clone(),
        );
    }

    pub(in crate::runtime) fn sync_accessibility_tree(&mut self) {
        let Some(cached) = self.cached_scene.as_ref() else {
            return;
        };
        let update = build_tree_update(
            cached.layout.as_ref(),
            &cached.computed,
            self.focused_widget_id(),
            cached.viewport,
        );
        let Some(adapter) = self.accessibility_adapter.as_mut() else {
            return;
        };
        adapter.update_if_active(update);
    }

    pub(in crate::runtime) fn update_accessibility_window_focus_state(&mut self, is_focused: bool) {
        if let Some(adapter) = self.accessibility_adapter.as_mut() {
            adapter.update_window_focus_state(is_focused);
        }
    }

    pub(in crate::runtime) fn drain_accessibility_actions(&mut self) -> bool {
        let mut handled = false;
        while let Ok(request) = self.accessibility_action_receiver.try_recv() {
            handled |= self.handle_accessibility_action(request);
        }
        handled
    }

    fn handle_accessibility_action(&mut self, request: ActionRequest) -> bool {
        let Some(widget_id) = widget_id_from_node(request.target_node) else {
            return false;
        };
        match request.action {
            Action::Focus => self.focus_accessibility_widget(widget_id),
            Action::Click => self.click_accessibility_widget(widget_id),
            Action::Increment | Action::Decrement | Action::SetValue => {
                self.adjust_accessibility_value(widget_id, request.action, request.data)
            }
            Action::ReplaceSelectedText => {
                self.set_accessibility_text_value(widget_id, request.data)
            }
            _ => false,
        }
    }

    fn focus_accessibility_widget(&mut self, widget_id: WidgetId) -> bool {
        let computed = self.computed_scene().clone();
        let target = computed
            .hit_regions
            .iter()
            .chain(computed.overlay_hit_regions.iter())
            .find_map(|region| {
                let focus = region.focus.as_ref()?;
                (focus.widget_id == widget_id).then(|| {
                    (
                        FocusedWidget {
                            widget_id: focus.widget_id,
                            scope_path: focus.scope_path.clone(),
                            on_blur: focus.on_blur.clone(),
                        },
                        focus.on_focus.clone(),
                    )
                })
            });
        let Some((target, on_focus)) = target else {
            return false;
        };
        self.update_focus(Some(target), on_focus, true);
        true
    }

    fn click_accessibility_widget(&mut self, widget_id: WidgetId) -> bool {
        let _ = self.focus_accessibility_widget(widget_id);
        if self.activate_focused_widget(true, true) {
            return true;
        }
        let computed = self.computed_scene().clone();
        let interaction = computed
            .hit_regions
            .iter()
            .chain(computed.overlay_hit_regions.iter())
            .find_map(|region| {
                hit_interaction_widget_id(&region.interaction)
                    .filter(|id| *id == widget_id)
                    .map(|_| region.interaction.clone())
            });
        match interaction {
            Some(HitInteraction::Widget { interactions, .. }) => {
                if let Some(command) = interactions.on_click {
                    self.execute_command(&command);
                    true
                } else {
                    false
                }
            }
            Some(HitInteraction::Checkbox {
                on_change, current, ..
            })
            | Some(HitInteraction::Radio {
                on_change, current, ..
            })
            | Some(HitInteraction::Switch {
                on_change, current, ..
            }) => {
                if let Some(command) = on_change {
                    self.execute_value_command(&command, !current);
                    true
                } else {
                    false
                }
            }
            Some(HitInteraction::SelectTrigger {
                id,
                on_open_change,
                is_open,
                ..
            }) => {
                let next_open = !is_open;
                self.close_all_open_selects_except(next_open.then_some(id));
                let _ = self.set_select_open_state(id, next_open, on_open_change.as_ref());
                true
            }
            Some(HitInteraction::TabTrigger {
                key,
                label,
                on_change,
                ..
            }) => {
                if let Some(command) = on_change {
                    self.execute_value_command(&command, (key, label));
                    true
                } else {
                    false
                }
            }
            Some(HitInteraction::ListItem { state, .. }) => {
                self.dispatch_list_item_keyboard_selection(&state, false)
                    || self.dispatch_list_item_action(&state)
            }
            Some(HitInteraction::DataGridCell { id, state, .. }) => self
                .dispatch_data_grid_cell_click(
                    &state,
                    id,
                    std::time::Instant::now(),
                    crate::ui::widget::CanvasMouseButton::Left,
                ),
            Some(HitInteraction::DataGridHeader { state, .. }) => self
                .dispatch_data_grid_header_click(
                    &state,
                    crate::ui::widget::CanvasMouseButton::Left,
                ),
            Some(HitInteraction::DataGridResizeHandle { state, .. }) => self
                .dispatch_data_grid_resize_click(
                    &state,
                    crate::ui::widget::CanvasMouseButton::Left,
                ),
            Some(HitInteraction::SelectOption {
                on_select,
                on_open_change,
                id,
                ..
            }) => {
                if let Some(command) = on_select {
                    self.execute_command(&command);
                }
                if let Some(command) = on_open_change {
                    self.execute_value_command(&command, false);
                } else {
                    let _ = self.set_select_open_state(id, false, None);
                }
                true
            }
            _ => false,
        }
    }

    fn adjust_accessibility_value(
        &mut self,
        widget_id: WidgetId,
        action: Action,
        data: Option<ActionData>,
    ) -> bool {
        if self.adjust_accessibility_slider(widget_id, action, data.clone()) {
            return true;
        }
        if matches!(action, Action::SetValue) {
            return self.set_accessibility_text_value(widget_id, data);
        }
        false
    }

    fn adjust_accessibility_slider(
        &mut self,
        widget_id: WidgetId,
        action: Action,
        data: Option<ActionData>,
    ) -> bool {
        let computed = self.computed_scene().clone();
        let slider = computed
            .hit_regions
            .iter()
            .chain(computed.overlay_hit_regions.iter())
            .find_map(|region| match &region.interaction {
                HitInteraction::Slider {
                    id,
                    on_change,
                    value,
                    min,
                    max,
                    step,
                    ..
                } if *id == widget_id => Some((on_change.clone(), *value, *min, *max, *step)),
                _ => None,
            });
        let Some((on_change, value, min, max, step)) = slider else {
            return false;
        };
        let next = match action {
            Action::Increment => value + step,
            Action::Decrement => value - step,
            Action::SetValue => match data {
                Some(ActionData::NumericValue(value)) => value as f32,
                Some(ActionData::Value(value)) => match value.parse::<f32>() {
                    Ok(value) => value,
                    Err(_) => return false,
                },
                _ => return false,
            },
            _ => return false,
        };
        self.apply_slider_value(on_change.as_ref(), next, min, max, step, true)
    }

    fn set_accessibility_text_value(
        &mut self,
        widget_id: WidgetId,
        data: Option<ActionData>,
    ) -> bool {
        let text = match data {
            Some(ActionData::Value(value)) => value.to_string(),
            _ => return false,
        };
        let computed = self.computed_scene().clone();
        let input = computed
            .hit_regions
            .iter()
            .chain(computed.overlay_hit_regions.iter())
            .find_map(|region| match &region.interaction {
                HitInteraction::TextInput {
                    id,
                    controller,
                    on_change,
                    on_change_set,
                    ..
                } if *id == widget_id => {
                    Some((controller.clone(), on_change.clone(), on_change_set.clone()))
                }
                _ => None,
            });
        let Some((controller, on_change, on_change_set)) = input else {
            return false;
        };
        let snapshot = controller.snapshot();
        if snapshot.text == text {
            return false;
        }
        let change = TextChange::new((0, snapshot.text.len()), text.clone());
        controller.set_text(text);
        let end_revision = controller.revision();
        if let Some(command) = on_change {
            self.execute_command(&command);
        }
        if let Some(command) = on_change_set {
            self.execute_value_command(
                &command,
                TextChangeSet {
                    start_revision: snapshot.revision,
                    end_revision,
                    changes: vec![change],
                },
            );
        }
        self.invalidate_text_input_scene();
        true
    }
}

fn hit_interaction_widget_id<VM>(interaction: &HitInteraction<VM>) -> Option<WidgetId> {
    match interaction {
        HitInteraction::Occluder { id }
        | HitInteraction::Disabled { id }
        | HitInteraction::Widget { id, .. }
        | HitInteraction::SelectableText { id, .. }
        | HitInteraction::Switch { id, .. }
        | HitInteraction::Checkbox { id, .. }
        | HitInteraction::Radio { id, .. }
        | HitInteraction::SelectTrigger { id, .. }
        | HitInteraction::SelectOption { id, .. }
        | HitInteraction::TabTrigger { id, .. }
        | HitInteraction::ListItem { id, .. }
        | HitInteraction::DataGridCell { id, .. }
        | HitInteraction::DataGridHeader { id, .. }
        | HitInteraction::DataGridResizeHandle { id, .. }
        | HitInteraction::Slider { id, .. }
        | HitInteraction::TextInput { id, .. }
        | HitInteraction::CanvasItem { id, .. } => Some(*id),
    }
}
