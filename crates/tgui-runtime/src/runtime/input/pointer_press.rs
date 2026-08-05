use super::*;
use crate::ui::widget::{HitTargetId, TreeNodeState};

#[derive(Clone, Copy)]
enum TreeControlPress {
    Disclosure,
    Checkbox,
}

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

    #[allow(clippy::too_many_arguments)]
    fn handle_tree_control_press(
        &mut self,
        id: WidgetId,
        state: &TreeNodeState<VM>,
        interactions: &InteractionHandlers<VM>,
        hit_target_id: HitTargetId,
        active_trap: Option<&Vec<WidgetId>>,
        focus_restore: Option<WidgetId>,
        context_menu_open: Option<(WidgetId, Point)>,
        button: CanvasMouseButton,
        control: TreeControlPress,
    ) {
        self.clear_selected_text();
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

        if let Some(trap) = active_trap {
            if !hit_scope_path.starts_with(trap) && focus_restore.is_none() {
                self.pending_click = None;
                self.pressed_widget = None;
                return;
            }
        }

        self.close_all_open_selects_except(None);
        let disabled = state.disabled.resolve();
        self.update_focus(
            (!disabled).then_some(FocusedWidget {
                widget_id: id,
                scope_path: hit_scope_path,
                on_blur: interactions.on_blur.clone(),
            }),
            (!disabled)
                .then_some(interactions.on_focus.clone())
                .flatten(),
            false,
        );
        if disabled {
            if let Some(widget_id) = focus_restore {
                let _ = self.restore_overlay_focus_if_needed(widget_id);
            }
        }
        if let Some((context_menu_id, position)) = context_menu_open {
            let _ = self.open_context_menu_at(context_menu_id, position);
        } else {
            match control {
                TreeControlPress::Disclosure => {
                    let _ = self.dispatch_tree_disclosure_click(state, button);
                }
                TreeControlPress::Checkbox => {
                    let _ = self.dispatch_tree_checkbox_click(state, button);
                }
            }
        }
        self.pressed_widget = Some(id);
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
        let mut focus_restore = self
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

        if matches!(
            hit,
            HitInteraction::Disabled { .. } | HitInteraction::Occluder { .. }
        ) {
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
                        if button == CanvasMouseButton::Left {
                            self.dispatch_widget_click(
                                HoverTargetId::Widget(id),
                                interactions,
                                now,
                            );
                        }
                        return;
                    }
                    _ => {}
                }
            }
            self.pending_click = None;
            return;
        }

        let pointer_position = self.cursor_position;
        let trigger_widget_id = match hit.target_id() {
            HitTargetId::Widget(widget_id) => Some(widget_id),
            HitTargetId::SelectOption { .. } | HitTargetId::CanvasItem { .. } => None,
        };
        let popover_toggle = if button == CanvasMouseButton::Left {
            trigger_widget_id.and_then(|widget_id| self.popover_trigger_ancestor(widget_id))
        } else {
            None
        };
        let clicked_select = matches!(
            &hit,
            HitInteraction::SelectTrigger { .. } | HitInteraction::SelectOption { .. }
        );
        let hit_target_id = hit.target_id();
        if focus_restore.is_none() {
            if let HitInteraction::Widget { id, .. } = &hit {
                focus_restore = self.backdrop_focus_restore_target(*id);
            }
        }
        let text_input_hit = matches!(&hit, HitInteraction::TextInput { .. });
        if button == CanvasMouseButton::Left {
            if let HitInteraction::TabTrigger {
                group_id,
                index,
                placement,
                key,
                reorderable,
                on_reorder,
                ..
            } = &hit
            {
                if *reorderable {
                    self.active_tab_reorder = Some(super::super::ActiveTabReorder {
                        group_id: *group_id,
                        from_index: *index,
                        key: key.clone(),
                        placement: *placement,
                        on_reorder: on_reorder.clone(),
                    });
                }
            }
        }
        let menu_toggle = (button == CanvasMouseButton::Left)
            .then(|| trigger_widget_id.and_then(|id| self.menu_trigger_ancestor(id)))
            .flatten();
        let context_menu_open = (button == CanvasMouseButton::Right)
            .then(|| {
                trigger_widget_id
                    .and_then(|id| self.context_menu_trigger_ancestor(id))
                    .map(|id| (id, pointer_position.unwrap_or(Point::ZERO)))
            })
            .flatten();

        if let HitInteraction::DataGridCell {
            id,
            state,
            interactions,
        } = hit.clone()
        {
            self.clear_selected_text();
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
            self.close_all_open_selects_except(None);
            let disabled = state.disabled.resolve();
            let focusable = !disabled;
            self.update_focus(
                focusable.then_some(FocusedWidget {
                    widget_id: id,
                    scope_path: hit_scope_path,
                    on_blur: interactions.on_blur.clone(),
                }),
                focusable.then_some(interactions.on_focus.clone()).flatten(),
                false,
            );
            let handled = if let Some((context_menu_id, position)) = context_menu_open {
                self.open_context_menu_at(context_menu_id, position)
            } else {
                self.dispatch_data_grid_cell_click(&state, id, now, button)
            };
            self.pressed_widget = handled.then_some(id);
            return;
        }

        if let HitInteraction::DataGridHeader {
            id,
            state,
            interactions,
        } = hit.clone()
        {
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
            self.update_focus(
                Some(FocusedWidget {
                    widget_id: id,
                    scope_path: hit_scope_path,
                    on_blur: interactions.on_blur.clone(),
                }),
                interactions.on_focus.clone(),
                false,
            );
            if let Some((context_menu_id, position)) = context_menu_open {
                let _ = self.open_context_menu_at(context_menu_id, position);
            } else {
                let _ = self.begin_data_grid_column_reorder(&state, button);
                let _ = self.dispatch_data_grid_header_click(&state, button);
            }
            self.pressed_widget = Some(id);
            return;
        }

        if let HitInteraction::DataGridResizeHandle {
            id,
            state,
            interactions,
        } = hit.clone()
        {
            if button == CanvasMouseButton::Left {
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
                self.update_focus(
                    Some(FocusedWidget {
                        widget_id: id,
                        scope_path: hit_scope_path,
                        on_blur: interactions.on_blur.clone(),
                    }),
                    interactions.on_focus.clone(),
                    false,
                );
            }
            if let Some((context_menu_id, position)) = context_menu_open {
                let _ = self.open_context_menu_at(context_menu_id, position);
            } else {
                let _ = self.begin_data_grid_column_resize(&state, button);
            }
            self.pressed_widget = Some(id);
            return;
        }

        if let HitInteraction::SplitterHandle {
            id,
            state,
            interactions,
            pair_extent,
        } = hit.clone()
        {
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
            self.update_focus(
                Some(FocusedWidget {
                    widget_id: id,
                    scope_path: hit_scope_path,
                    on_blur: interactions.on_blur.clone(),
                }),
                interactions.on_focus.clone(),
                false,
            );
            if let Some((context_menu_id, position)) = context_menu_open {
                let _ = self.open_context_menu_at(context_menu_id, position);
            } else if button == CanvasMouseButton::Left {
                let target_id = HoverTargetId::SplitterHandle {
                    widget_id: id,
                    axis: state.axis,
                    index: state.index,
                    pane_count: state.pane_count(),
                };
                if self.pending_click_matches_target(target_id, now) {
                    self.pending_click = None;
                    self.cancel_splitter_resize();
                    let _ = self.reset_splitter_from_hit(&state);
                } else {
                    self.pending_click = Some(PendingClick {
                        target_id,
                        deadline: now + super::super::DOUBLE_CLICK_THRESHOLD,
                        position: self.cursor_position.unwrap_or(Point::ZERO),
                        command: interactions.on_click.clone().map(ClickHandler::Command),
                        splitter: Some(PendingSplitterClick {
                            axis: state.axis,
                            index: state.index,
                            pane_count: state.pane_count(),
                        }),
                    });
                    let _ = self.begin_splitter_resize(&state, pair_extent, button);
                }
            }
            self.pressed_widget = Some(id);
            return;
        }

        if let HitInteraction::TreeDisclosure {
            id,
            state,
            interactions,
        } = hit.clone()
        {
            self.handle_tree_control_press(
                id,
                &state,
                &interactions,
                hit_target_id,
                active_trap.as_ref(),
                focus_restore,
                context_menu_open,
                button,
                TreeControlPress::Disclosure,
            );
            return;
        }

        if let HitInteraction::TreeCheckbox {
            id,
            state,
            interactions,
        } = hit.clone()
        {
            self.handle_tree_control_press(
                id,
                &state,
                &interactions,
                hit_target_id,
                active_trap.as_ref(),
                focus_restore,
                context_menu_open,
                button,
                TreeControlPress::Checkbox,
            );
            return;
        }

        if let HitInteraction::TreeNode {
            id,
            state,
            interactions,
        } = hit.clone()
        {
            self.clear_selected_text();
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

            self.close_all_open_selects_except(None);
            let disabled = state.disabled.resolve();
            self.update_focus(
                (!disabled).then_some(FocusedWidget {
                    widget_id: id,
                    scope_path: hit_scope_path,
                    on_blur: interactions.on_blur.clone(),
                }),
                (!disabled)
                    .then_some(interactions.on_focus.clone())
                    .flatten(),
                false,
            );
            if disabled {
                if let Some(widget_id) = focus_restore {
                    let _ = self.restore_overlay_focus_if_needed(widget_id);
                }
            }
            if let Some((context_menu_id, position)) = context_menu_open {
                let _ = self.open_context_menu_at(context_menu_id, position);
            } else {
                let _ = self.dispatch_tree_node_click(&state, id, now, button, pointer_position);
            }
            self.pressed_widget = Some(id);
            return;
        }

        if let HitInteraction::ListItem {
            id,
            state,
            interactions,
        } = hit
        {
            self.clear_selected_text();
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

            self.close_all_open_selects_except(None);
            let disabled = state.disabled.resolve();
            self.update_focus(
                (!disabled).then_some(FocusedWidget {
                    widget_id: id,
                    scope_path: hit_scope_path,
                    on_blur: interactions.on_blur.clone(),
                }),
                (!disabled)
                    .then_some(interactions.on_focus.clone())
                    .flatten(),
                false,
            );
            if disabled {
                if let Some(widget_id) = focus_restore {
                    let _ = self.restore_overlay_focus_if_needed(widget_id);
                }
            }
            if let Some((context_menu_id, position)) = context_menu_open {
                let _ = self.open_context_menu_at(context_menu_id, position);
            } else {
                let _ = self.dispatch_list_item_click(&state, id, now, button);
            }
            self.pressed_widget = Some(id);
            return;
        }

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
            Option<(WidgetId, TextInputSnapshot, usize)>,
            Option<(
                WidgetId,
                Option<ValueCommand<VM, f32>>,
                Option<ValueCommand<VM, f32>>,
                f32,
                f32,
                f32,
                crate::ui::widget::SliderOrientation,
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
                let multiline = text.contains('\n');
                let input = TextInputSnapshot {
                    frame,
                    padding,
                    text_style,
                    text,
                    multiline,
                    auto_wrap: false,
                    show_scrollbar: false,
                };
                let cursor = pointer_position.map(|point| {
                    text_cursor_index_at_point(
                        &self.font_manager,
                        &self.theme,
                        self.unit_context(),
                        input.as_context(),
                        ScrollContext::ZERO,
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
                    cursor.map(|cursor| (id, input, cursor)),
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
                (on_change.is_some() || interactions.on_click.is_some()).then(|| {
                    ClickHandler::Toggle {
                        on_change: on_change.clone(),
                        next: Some(!current),
                        on_click: interactions.on_click.clone(),
                    }
                }),
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
                (on_change.is_some() || interactions.on_click.is_some()).then(|| {
                    ClickHandler::Toggle {
                        on_change: on_change.clone(),
                        next: Some(!current),
                        on_click: interactions.on_click.clone(),
                    }
                }),
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
                (on_change.is_some() || interactions.on_click.is_some()).then(|| {
                    ClickHandler::Toggle {
                        on_change: on_change.clone(),
                        next: (!current).then_some(true),
                        on_click: interactions.on_click.clone(),
                    }
                }),
                None,
                None,
                None,
            ),
            HitInteraction::Slider {
                id,
                interactions,
                on_change,
                on_change_end,
                value,
                min,
                max,
                step,
                orientation,
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
                Some((
                    id,
                    on_change.clone(),
                    on_change_end.clone(),
                    min,
                    max,
                    step,
                    orientation,
                    track_rect,
                    value,
                )),
            ),
            HitInteraction::SelectTrigger {
                id,
                interactions,
                on_open_change,
                is_open,
                can_toggle,
            } => (
                id,
                interactions.clone(),
                Some(id),
                interactions.on_focus.clone(),
                interactions.on_click.clone().map(ClickHandler::Command),
                can_toggle.then(|| (id, !is_open, on_open_change.clone())),
                None,
                None,
            ),
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
                    let input = TextInputSnapshot {
                        frame,
                        padding,
                        text_style,
                        text: value,
                        multiline,
                        auto_wrap,
                        show_scrollbar,
                    };
                    let scroll = ScrollContext::new(
                        self.scroll_states.get(&id).copied().unwrap_or(Point::ZERO),
                    );
                    let cursor = self.text_input_cursor_index_at_point(
                        id,
                        input.as_context(),
                        scroll,
                        point,
                    );
                    (id, input, cursor)
                }),
                None,
            ),
            HitInteraction::SelectOption {
                id,
                option_index: _,
                interactions,
                on_select,
                ref on_open_change,
                ref menu_path,
                ..
            } => {
                let is_menu_item = self.is_menu_layer_source(id);
                let can_activate = on_select.is_some() || menu_path.is_some();
                (
                    id,
                    interactions.clone(),
                    None,
                    None,
                    (button == CanvasMouseButton::Left && can_activate).then(|| {
                        ClickHandler::SelectOption {
                            widget_id: id,
                            command: on_select.clone(),
                            on_open_change: on_open_change.clone(),
                            menu_path: menu_path.clone(),
                        }
                    }),
                    (button == CanvasMouseButton::Left && !is_menu_item && on_select.is_some())
                        .then_some((id, false, on_open_change.clone())),
                    None,
                    None,
                )
            }
            HitInteraction::Occluder { .. } => unreachable!("occluder hit handled above"),
            HitInteraction::Disabled { .. } => unreachable!("disabled hit handled above"),
            HitInteraction::CanvasItem { .. } => unreachable!("canvas item handled above"),
            HitInteraction::ListItem { .. } => unreachable!("list item handled above"),
            HitInteraction::TreeNode { .. } => unreachable!("tree node handled above"),
            HitInteraction::TreeDisclosure { .. } | HitInteraction::TreeCheckbox { .. } => {
                unreachable!("tree control hits handled above")
            }
            HitInteraction::DataGridCell { .. }
            | HitInteraction::DataGridHeader { .. }
            | HitInteraction::DataGridResizeHandle { .. } => {
                unreachable!("data grid hits handled above")
            }
            HitInteraction::SplitterHandle { .. } => unreachable!("splitter hits handled above"),
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
            if button == CanvasMouseButton::Left {
                self.close_all_open_selects_except(next_open.then_some(select_id));
                let _ = self.set_select_open_state(select_id, next_open, on_open_change.as_ref());
            }
        }
        if let Some((context_menu_id, position)) = context_menu_open {
            self.close_all_open_selects_except(None);
            let _ = self.open_context_menu_at(context_menu_id, position);
        } else if button == CanvasMouseButton::Left {
            if let Some(popover_id) = popover_toggle {
                let _ = self.toggle_popover_from_trigger_descendant(popover_id);
            }
            if let Some(menu_id) = menu_toggle {
                self.close_all_open_selects_except(None);
                let _ = self.toggle_menu_open_state(menu_id);
            }
        }
        self.pressed_widget = Some(widget_id);

        if context_menu_open.is_some() {
            self.pending_click = None;
            return;
        }

        // Secondary mouse buttons may focus a target and/or open its context menu, but they
        // must never run the primary activation path below. In particular, this keeps a right
        // click from toggling selection controls, starting a slider drag, or dispatching a plain
        // widget's `on_click` command.
        if button != CanvasMouseButton::Left {
            self.pending_click = None;
            self.pressed_widget = None;
            return;
        }

        if let Some((
            slider_id,
            on_change,
            on_change_end,
            min,
            max,
            step,
            orientation,
            track_rect,
            current_value,
        )) = slider_drag
        {
            if let Some(position) = pointer_position {
                self.begin_slider_drag(
                    slider_id,
                    on_change.clone(),
                    on_change_end.clone(),
                    min,
                    max,
                    step,
                    orientation,
                    track_rect,
                    current_value,
                );
                let value = self.slider_value_for_position(
                    position,
                    track_rect,
                    orientation,
                    min,
                    max,
                    step,
                );
                if (value - current_value).abs() > f32::EPSILON {
                    if on_change_end.is_some() {
                        if let Some(active) = self.active_slider_drag.as_mut() {
                            active.current_value = value;
                        }
                        if !self.patch_active_slider_scene(Instant::now()) {
                            self.invalidate_computed_scene();
                        }
                    } else if self.apply_slider_value(
                        on_change.as_ref(),
                        value,
                        min,
                        max,
                        step,
                        false,
                    ) {
                        if let Some(active) = self.active_slider_drag.as_mut() {
                            active.current_value = value;
                            active.committed_value = Some(value);
                        }
                        if !self.patch_active_slider_scene(Instant::now()) {
                            self.invalidate_computed_scene();
                        }
                    }
                }
            }
        }

        if let Some((widget_id, input, cursor)) = selectable_text {
            self.begin_text_selection(widget_id, input, cursor);
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
                        splitter: None,
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
