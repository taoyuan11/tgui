use super::*;

impl<VM: 'static> BoundRuntimeHandler<VM> {
    pub(super) fn dispatch_widget_click(
        &mut self,
        target_id: HoverTargetId,
        interactions: InteractionHandlers<VM>,
        now: Instant,
    ) {
        let is_double_click = self
            .pending_click
            .as_ref()
            .map(|pending| pending.target_id == target_id && pending.deadline > now)
            .unwrap_or(false);

        if is_double_click {
            self.pending_click = None;
            if let Some(command) = interactions.on_double_click.or(interactions.on_click) {
                self.execute_command(&command);
            }
            return;
        }

        if interactions.on_double_click.is_some() {
            self.pending_click = Some(PendingClick {
                target_id,
                deadline: now + super::super::DOUBLE_CLICK_THRESHOLD,
                command: interactions.on_click.map(ClickHandler::Command),
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
        let is_double_click = self
            .pending_click
            .as_ref()
            .map(|pending| pending.target_id == target_id && pending.deadline > now)
            .unwrap_or(false);

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
                command: item_interactions
                    .on_click
                    .clone()
                    .map(|command| ClickHandler::Canvas(command, context, Some(button))),
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
