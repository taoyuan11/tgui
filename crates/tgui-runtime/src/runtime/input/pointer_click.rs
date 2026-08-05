use super::*;

impl<VM: 'static> BoundRuntimeHandler<VM> {
    pub(in crate::runtime::input) fn pending_click_matches_target(
        &self,
        target_id: HoverTargetId,
        now: Instant,
    ) -> bool {
        let Some(pending) = self.pending_click.as_ref() else {
            return false;
        };
        if !click_targets_match(pending.target_id, target_id) || pending.deadline <= now {
            return false;
        }
        let Some(position) = self.cursor_position else {
            return false;
        };
        let dx = (position.x - pending.position.x).abs().get();
        let dy = (position.y - pending.position.y).abs().get();
        dx < super::super::LONG_PRESS_MOVE_TOLERANCE && dy < super::super::LONG_PRESS_MOVE_TOLERANCE
    }

    pub(super) fn dispatch_widget_click(
        &mut self,
        target_id: HoverTargetId,
        interactions: InteractionHandlers<VM>,
        now: Instant,
    ) {
        if self.gesture_consumes_click() {
            self.pending_click = None;
            return;
        }
        let is_double_click = self.pending_click_matches_target(target_id, now);

        if is_double_click {
            self.pending_click = None;
            if let Some(gesture) = self.active_gesture.as_ref() {
                if gesture.target_id == target_id {
                    if let (Some(command), event) = (
                        interactions
                            .gesture
                            .as_ref()
                            .and_then(|recognizer| recognizer.on_double_tap.clone()),
                        gesture.double_tap_event(),
                    ) {
                        self.execute_value_command(&command, event);
                        return;
                    }
                }
            }
            if let Some(command) = interactions.on_double_click.or(interactions.on_click) {
                self.execute_command(&command);
            }
            return;
        }

        if interactions.on_double_click.is_some()
            || interactions
                .gesture
                .as_ref()
                .and_then(|gesture| gesture.on_double_tap.as_ref())
                .is_some()
        {
            self.pending_click = Some(PendingClick {
                target_id,
                deadline: now + super::super::DOUBLE_CLICK_THRESHOLD,
                position: self.cursor_position.unwrap_or(Point::ZERO),
                command: interactions.on_click.map(ClickHandler::Command),
                splitter: None,
            });
        } else if let Some(command) = interactions.on_click {
            self.execute_command(&command);
        } else {
            self.pending_click = None;
        }
    }

    pub(super) fn dispatch_canvas_click(
        &mut self,
        target_id: HoverTargetId,
        item_interactions: &CanvasItemInteractionHandlers<VM>,
        context: CanvasPointerContext,
        now: Instant,
        button: CanvasMouseButton,
    ) -> bool {
        if self.gesture_consumes_click() {
            self.pending_click = None;
            return false;
        }
        let is_double_click = self.pending_click_matches_target(target_id, now);

        if is_double_click {
            self.pending_click = None;
            if let Some(command) = item_interactions
                .on_double_click
                .clone()
                .or(item_interactions.on_click.clone())
            {
                self.execute_click_handler(
                    &ClickHandler::Canvas(command, context, Some(button)),
                    self.cursor_position,
                );
                return true;
            }
            return false;
        }

        if item_interactions.on_double_click.is_some() {
            self.pending_click = Some(PendingClick {
                target_id,
                deadline: now + super::super::DOUBLE_CLICK_THRESHOLD,
                position: self.cursor_position.unwrap_or(Point::ZERO),
                command: item_interactions
                    .on_click
                    .clone()
                    .map(|command| ClickHandler::Canvas(command, context, Some(button))),
                splitter: None,
            });
            return true;
        }

        if let Some(command) = item_interactions.on_click.clone() {
            self.execute_click_handler(
                &ClickHandler::Canvas(command, context, Some(button)),
                self.cursor_position,
            );
            return true;
        }

        false
    }

    pub(in crate::runtime) fn handle_canvas_mouse_release(&mut self, button: CanvasMouseButton) {
        if self
            .active_canvas_drag
            .as_ref()
            .is_some_and(|drag| drag.button != button)
        {
            return;
        }
        if let Some(position) = self.cursor_position {
            if let Some(drag) = self.active_canvas_drag.as_ref() {
                let context = drag.context.clone();
                if let Some(command) = drag.on_mouse_up.clone() {
                    self.execute_canvas_mouse_command(&command, context, position, Some(button));
                }
            } else {
                for interaction in self.hit_path(self.viewport_rect()).into_iter().rev() {
                    if let HitInteraction::CanvasItem {
                        item_id,
                        item_interactions,
                        canvas_origin,
                        item_origin,
                        inverse_transform,
                        text_hits,
                        ..
                    } = interaction
                    {
                        if let Some(command) = item_interactions.on_mouse_up {
                            self.execute_canvas_mouse_command(
                                &command,
                                CanvasPointerContext {
                                    item_id,
                                    canvas_origin,
                                    item_origin,
                                    inverse_transform,
                                    text_hits,
                                },
                                position,
                                Some(button),
                            );
                            break;
                        }
                    }
                }
            }
        }
        self.end_canvas_drag();
    }
}

fn click_targets_match(left: HoverTargetId, right: HoverTargetId) -> bool {
    match (left, right) {
        (
            HoverTargetId::SplitterHandle {
                axis: left_axis,
                index: left_index,
                pane_count: left_pane_count,
                ..
            },
            HoverTargetId::SplitterHandle {
                axis: right_axis,
                index: right_index,
                pane_count: right_pane_count,
                ..
            },
        ) => {
            left_axis == right_axis
                && left_index == right_index
                && left_pane_count == right_pane_count
        }
        _ => left == right,
    }
}
