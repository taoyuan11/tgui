use super::*;

impl<VM: 'static> BoundRuntimeHandler<VM> {
    fn should_defer_mouse_click(
        &self,
        interactions: &InteractionHandlers<VM>,
        has_click_handler: bool,
    ) -> bool {
        has_click_handler
            && self
                .active_gesture
                .as_ref()
                .map(|gesture| {
                    gesture.source == crate::ui::widget::GestureSource::Mouse
                        && gesture.recognizer.on_long_press.is_some()
                        && interactions.on_double_click.is_none()
                        && interactions
                            .gesture
                            .as_ref()
                            .and_then(|recognizer| recognizer.on_double_tap.as_ref())
                            .is_none()
                })
                .unwrap_or(false)
    }

    fn should_defer_touch_click(
        &self,
        interactions: &InteractionHandlers<VM>,
        has_click_handler: bool,
    ) -> bool {
        has_click_handler
            && self
                .active_gesture
                .as_ref()
                .map(|gesture| {
                    gesture.source == crate::ui::widget::GestureSource::Touch
                        && gesture.recognizer.on_pinch.is_some()
                        && interactions
                            .gesture
                            .as_ref()
                            .and_then(|recognizer| recognizer.on_pinch.as_ref())
                            .is_some()
                })
                .unwrap_or(false)
    }

    pub(in crate::runtime) fn handle_mouse_press(
        &mut self,
        viewport: Rect,
        now: Instant,
        button: CanvasMouseButton,
    ) {
        self.flush_pending_click_if_due(now);

        let hit_path = self.hit_path(viewport);
        let active_trap = self.active_focus_trap_scope();
        let focus_restore = self
            .cursor_position
            .and_then(|point| self.consume_overlay_close_handlers_outside_click(point));
        let Some(hit) = hit_path.last().cloned() else {
            if active_trap.is_some() && focus_restore.is_none() {
                self.pending_click = None;
                self.pressed_widget = None;
                return;
            }
            self.close_all_open_selects_except(None);
            self.clear_selected_text();
            self.update_focus(None, None, false);
            if let Some(widget_id) = focus_restore {
                let _ = self.restore_overlay_focus_if_needed(widget_id);
            }
            self.pending_click = None;
            self.pressed_widget = None;
            return;
        };

        if matches!(hit, HitInteraction::Disabled { .. }) {
            if active_trap.is_some() && focus_restore.is_none() {
                self.pending_click = None;
                self.pressed_widget = None;
                return;
            }
            self.close_all_open_selects_except(None);
            self.clear_selected_text();
            self.update_focus(None, None, false);
            if let Some(widget_id) = focus_restore {
                let _ = self.restore_overlay_focus_if_needed(widget_id);
            }
            self.pending_click = None;
            self.pressed_widget = None;
            return;
        }

        if matches!(hit, HitInteraction::CanvasItem { .. }) {
            self.clear_selected_text();
            self.update_focus(None, None, false);
            self.pressed_widget = None;

            for interaction in hit_path.into_iter().rev() {
                match interaction {
                    HitInteraction::CanvasItem {
                        id,
                        item_id,
                        item_interactions,
                        canvas_origin,
                        item_origin,
                        inverse_transform,
                        text_hits,
                        ..
                    } => {
                        let context = CanvasPointerContext {
                            item_id,
                            canvas_origin,
                            item_origin,
                            inverse_transform,
                            text_hits,
                        };
                        if let (Some(position), Some(command)) = (
                            self.cursor_position,
                            item_interactions.on_mouse_down.clone(),
                        ) {
                            self.execute_canvas_mouse_command(
                                &command,
                                context.clone(),
                                position,
                                Some(button),
                            );
                        }
                        if self.active_canvas_drag.is_none()
                            && (item_interactions.on_drag_start.is_some()
                                || item_interactions.on_drag.is_some()
                                || item_interactions.on_drag_end.is_some())
                        {
                            if let Some(position) = self.cursor_position {
                                self.active_canvas_drag = Some(super::super::ActiveCanvasDrag {
                                    button,
                                    context: context.clone(),
                                    start_position: position,
                                    started: false,
                                    on_mouse_up: item_interactions.on_mouse_up.clone(),
                                    on_drag_start: item_interactions.on_drag_start.clone(),
                                    on_drag: item_interactions.on_drag.clone(),
                                    on_drag_end: item_interactions.on_drag_end.clone(),
                                });
                            }
                        }
                        if self.dispatch_canvas_click(
                            HoverTargetId::CanvasItem {
                                widget_id: id,
                                item_id,
                            },
                            &item_interactions,
                            context.clone(),
                            now,
                            button,
                        ) {
                            return;
                        }
                    }
                    HitInteraction::Widget {
                        id, interactions, ..
                    } => {
                        self.dispatch_widget_click(HoverTargetId::Widget(id), interactions, now);
                        return;
                    }
                    _ => {}
                }
            }
            self.pending_click = None;
            return;
        }

        let pointer_position = self.cursor_position;
        let clicked_select = matches!(
            &hit,
            HitInteraction::SelectTrigger { .. } | HitInteraction::SelectOption { .. }
        );
        let hit_target_id = hit.target_id();
        let text_input_hit = matches!(&hit, HitInteraction::TextInput { .. });
        let (
            widget_id,
            interactions,
            focus_target,
            focus_command,
            click_handler,
            select_toggle,
            selectable_text,
            slider_drag,
        ): (
            WidgetId,
            InteractionHandlers<VM>,
            Option<WidgetId>,
            Option<Command<VM>>,
            Option<ClickHandler<VM>>,
            Option<(WidgetId, bool, Option<ValueCommand<VM, bool>>)>,
            Option<(
                WidgetId,
                Rect,
                crate::ui::layout::Insets,
                Text,
                String,
                bool,
                bool,
                bool,
                usize,
            )>,
            Option<(
                WidgetId,
                Option<ValueCommand<VM, f32>>,
                f32,
                f32,
                f32,
                Rect,
                f32,
            )>,
        ) = match hit {
            HitInteraction::Widget {
                id,
                interactions,
                focusable,
                ..
            } => (
                id,
                interactions.clone(),
                focusable.then_some(id),
                focusable.then_some(interactions.on_focus.clone()).flatten(),
                interactions.on_click.clone().map(ClickHandler::Command),
                None,
                None,
                None,
            ),
            HitInteraction::TabTrigger {
                id, interactions, ..
            } => (
                id,
                interactions.clone(),
                Some(id),
                interactions.on_focus.clone(),
                interactions.on_click.clone().map(ClickHandler::Command),
                None,
                None,
                None,
            ),
            HitInteraction::SelectableText {
                id,
                frame,
                padding,
                interactions,
                text_style,
                text,
            } => {
                let cursor = pointer_position.map(|point| {
                    text_cursor_index_at_point(
                        &self.font_manager,
                        &self.theme,
                        self.unit_context(),
                        frame,
                        padding,
                        &text_style,
                        &text,
                        false,
                        false,
                        false,
                        Point::ZERO,
                        point,
                    )
                });
                (
                    id,
                    interactions.clone(),
                    None,
                    None,
                    interactions.on_click.clone().map(ClickHandler::Command),
                    None,
                    cursor.map(|cursor| {
                        (
                            id, frame, padding, text_style, text, false, false, false, cursor,
                        )
                    }),
                    None,
                )
            }
            HitInteraction::Switch {
                id,
                interactions,
                on_change,
                current,
            } => (
                id,
                interactions.clone(),
                Some(id),
                interactions.on_focus.clone(),
                on_change
                    .clone()
                    .map(|command| ClickHandler::Toggle(command, !current))
                    .or_else(|| interactions.on_click.clone().map(ClickHandler::Command)),
                None,
                None,
                None,
            ),
            HitInteraction::Checkbox {
                id,
                interactions,
                on_change,
                current,
            } => (
                id,
                interactions.clone(),
                Some(id),
                interactions.on_focus.clone(),
                on_change
                    .clone()
                    .map(|command| ClickHandler::Toggle(command, !current))
                    .or_else(|| interactions.on_click.clone().map(ClickHandler::Command)),
                None,
                None,
                None,
            ),
            HitInteraction::Radio {
                id,
                interactions,
                on_change,
                current,
            } => (
                id,
                interactions.clone(),
                Some(id),
                interactions.on_focus.clone(),
                on_change
                    .clone()
                    .map(|command| ClickHandler::Toggle(command, !current))
                    .or_else(|| interactions.on_click.clone().map(ClickHandler::Command)),
                None,
                None,
                None,
            ),
            HitInteraction::Slider {
                id,
                interactions,
                on_change,
                value,
                min,
                max,
                step,
                track_rect,
                ..
            } => (
                id,
                interactions.clone(),
                Some(id),
                interactions.on_focus.clone(),
                interactions.on_click.clone().map(ClickHandler::Command),
                None,
                None,
                Some((id, on_change.clone(), min, max, step, track_rect, value)),
            ),
            HitInteraction::SelectTrigger {
                id,
                interactions,
                on_open_change,
                is_open: _,
            } => {
                let is_open = self.resolved_select_open_state(id).unwrap_or(false);
                (
                    id,
                    interactions.clone(),
                    Some(id),
                    interactions.on_focus.clone(),
                    interactions.on_click.clone().map(ClickHandler::Command),
                    Some((id, !is_open, on_open_change.clone())),
                    None,
                    None,
                )
            }
            HitInteraction::TextInput {
                id,
                interactions,
                controller,
                multiline,
                auto_wrap,
                show_scrollbar,
                frame,
                padding,
                text_style,
                ..
            } => (
                id,
                interactions.clone(),
                Some(id),
                interactions.on_focus.clone(),
                interactions.on_click.clone().map(ClickHandler::Command),
                None,
                pointer_position.map(|point| {
                    let value = self.text_input_current_value(id, &controller);
                    let scroll = self.scroll_states.get(&id).copied().unwrap_or(Point::ZERO);
                    let cursor = self.text_input_cursor_index_at_point(
                        id,
                        frame,
                        padding,
                        &text_style,
                        &value,
                        multiline,
                        auto_wrap,
                        show_scrollbar,
                        scroll,
                        point,
                    );
                    (
                        id,
                        frame,
                        padding,
                        text_style,
                        value,
                        multiline,
                        auto_wrap,
                        show_scrollbar,
                        cursor,
                    )
                }),
                None,
            ),
            HitInteraction::SelectOption {
                id,
                option_index: _,
                interactions,
                on_select,
                ref on_open_change,
            } => (
                id,
                interactions.clone(),
                None,
                None,
                Some(ClickHandler::SelectOption {
                    widget_id: id,
                    command: on_select.clone(),
                    on_open_change: on_open_change.clone(),
                }),
                Some((id, false, on_open_change.clone())),
                None,
                None,
            ),
            HitInteraction::Disabled { .. } => unreachable!("disabled hit handled above"),
            HitInteraction::CanvasItem { .. } => unreachable!("canvas item handled above"),
        };

        if selectable_text.is_none() {
            self.clear_selected_text();
        }

        let hit_scope_path = {
            let computed = self.computed_scene();
            computed
                .hit_regions
                .iter()
                .chain(computed.overlay_hit_regions.iter())
                .find(|region| region.interaction.target_id() == hit_target_id)
                .map(|region| region.scope_path.clone())
                .unwrap_or_default()
        };

        if let Some(trap) = active_trap.as_ref() {
            if !hit_scope_path.starts_with(trap) && focus_restore.is_none() {
                self.pending_click = None;
                self.pressed_widget = None;
                return;
            }
        }

        if !clicked_select {
            self.close_all_open_selects_except(None);
        }

        self.update_focus(
            focus_target.map(|id| FocusedWidget {
                widget_id: id,
                scope_path: hit_scope_path.clone(),
                on_blur: interactions.on_blur.clone(),
            }),
            focus_command,
            false,
        );
        if focus_target.is_none() {
            if let Some(widget_id) = focus_restore {
                let _ = self.restore_overlay_focus_if_needed(widget_id);
            }
        }
        if text_input_hit {
            self.focus_visible = true;
            self.reset_caret_blink();
            self.sync_ime_state();
        }
        if let Some((select_id, next_open, on_open_change)) = select_toggle {
            self.close_all_open_selects_except(next_open.then_some(select_id));
            let _ = self.set_select_open_state(select_id, next_open, on_open_change.as_ref());
        }
        self.pressed_widget = Some(widget_id);

        if let Some((slider_id, on_change, min, max, step, track_rect, current_value)) = slider_drag
        {
            if let Some(position) = pointer_position {
                self.begin_slider_drag(
                    slider_id,
                    on_change.clone(),
                    min,
                    max,
                    step,
                    track_rect,
                    current_value,
                );
                let value = self.slider_value_for_position(position, track_rect, min, max, step);
                if (value - current_value).abs() > f32::EPSILON
                    && self.apply_slider_value(on_change.as_ref(), value, min, max, step, false)
                {
                    if let Some(active) = self.active_slider_drag.as_mut() {
                        active.current_value = value;
                    }
                    let _ = self.patch_active_slider_scene(Instant::now());
                }
            }
        }

        if let Some((
            widget_id,
            frame,
            padding,
            text_style,
            text,
            multiline,
            auto_wrap,
            show_scrollbar,
            cursor,
        )) = selectable_text
        {
            self.begin_text_selection(
                widget_id,
                frame,
                padding,
                text_style,
                text,
                multiline,
                auto_wrap,
                show_scrollbar,
                cursor,
            );
        }

        if let Some(handler) = click_handler {
            let defer_pointer_click = self.should_defer_mouse_click(&interactions, true)
                || self.should_defer_touch_click(&interactions, true);
            if defer_pointer_click {
                self.deferred_mouse_click = Some(super::super::state::DeferredMouseClick {
                    widget_id,
                    interactions,
                    click_handler: Some(handler),
                });
                return;
            }
            if self.gesture_consumes_click() {
                return;
            }
            let gesture_double_tap = interactions
                .gesture
                .as_ref()
                .and_then(|gesture| gesture.on_double_tap.clone());
            if interactions.on_double_click.is_some() || gesture_double_tap.is_some() {
                let target_id = HoverTargetId::Widget(widget_id);
                let is_double_click = self
                    .pending_click
                    .as_ref()
                    .map(|pending| pending.target_id == target_id && pending.deadline > now)
                    .unwrap_or(false);

                if is_double_click {
                    self.pending_click = None;
                    if let Some(command) = gesture_double_tap {
                        if let Some(gesture) = self.active_gesture.as_ref() {
                            self.execute_value_command(&command, gesture.double_tap_event());
                        }
                    } else if let Some(command) = interactions
                        .on_double_click
                        .clone()
                        .map(ClickHandler::Command)
                    {
                        self.execute_click_handler(&command, self.cursor_position);
                    } else {
                        self.execute_click_handler(&handler, self.cursor_position);
                    }
                } else {
                    self.pending_click = Some(PendingClick {
                        target_id,
                        deadline: now + super::super::DOUBLE_CLICK_THRESHOLD,
                        position: self.cursor_position.unwrap_or(Point::ZERO),
                        command: Some(handler),
                    });
                }
            } else {
                self.execute_click_handler(&handler, self.cursor_position);
            }
        } else {
            self.dispatch_widget_click(HoverTargetId::Widget(widget_id), interactions, now);
        }
    }
}
