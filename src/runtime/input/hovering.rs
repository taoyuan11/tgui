use super::select_state::HoverMoveOrTransition;
use super::*;

impl<VM: 'static> BoundRuntimeHandler<VM> {
    pub(in crate::runtime) fn hit_path(&mut self, _viewport: Rect) -> Vec<HitInteraction<VM>> {
        let Some(point) = self.cursor_position else {
            return Vec::new();
        };
        WidgetTree::hit_path_from_computed(self.computed_scene(), point)
    }

    pub(in crate::runtime) fn hover_path(&mut self, viewport: Rect) -> Vec<HoveredWidget<VM>> {
        let hit_path = self.hit_path(viewport);
        let topmost_canvas_item = hit_path
            .iter()
            .rposition(|interaction| matches!(interaction, HitInteraction::CanvasItem { .. }));

        hit_path
            .into_iter()
            .enumerate()
            .filter(|(index, interaction)| {
                !matches!(interaction, HitInteraction::CanvasItem { .. })
                    || Some(*index) == topmost_canvas_item
            })
            .map(|(_, interaction)| interaction)
            .map(|interaction| match interaction {
                HitInteraction::Disabled { id } => HoveredWidget {
                    target_id: HoverTargetId::Widget(id),
                    cursor_style: Some(crate::ui::widget::CursorStyle::NotAllowed),
                    on_mouse_enter: None,
                    on_mouse_leave: None,
                    on_mouse_move: None,
                },
                HitInteraction::Widget {
                    id, interactions, ..
                } => HoveredWidget {
                    target_id: HoverTargetId::Widget(id),
                    cursor_style: interactions.cursor_style.map(|c| c.resolve()),
                    on_mouse_enter: interactions
                        .on_mouse_enter
                        .map(HoverMoveOrTransition::into_transition),
                    on_mouse_leave: interactions
                        .on_mouse_leave
                        .map(HoverMoveOrTransition::into_transition),
                    on_mouse_move: interactions.on_mouse_move.map(HoverMoveHandler::Point),
                },
                HitInteraction::SelectableText {
                    id, interactions, ..
                } => HoveredWidget {
                    target_id: HoverTargetId::Widget(id),
                    cursor_style: interactions
                        .cursor_style
                        .map(|c| c.resolve())
                        .or(Some(crate::ui::widget::CursorStyle::Text)),
                    on_mouse_enter: interactions
                        .on_mouse_enter
                        .map(HoverMoveOrTransition::into_transition),
                    on_mouse_leave: interactions
                        .on_mouse_leave
                        .map(HoverMoveOrTransition::into_transition),
                    on_mouse_move: interactions.on_mouse_move.map(HoverMoveHandler::Point),
                },
                HitInteraction::Switch {
                    id, interactions, ..
                }
                | HitInteraction::Checkbox {
                    id, interactions, ..
                }
                | HitInteraction::Radio {
                    id, interactions, ..
                }
                | HitInteraction::Slider {
                    id, interactions, ..
                }
                | HitInteraction::TextInput {
                    id, interactions, ..
                }
                | HitInteraction::SelectTrigger {
                    id, interactions, ..
                } => HoveredWidget {
                    target_id: HoverTargetId::Widget(id),
                    cursor_style: interactions.cursor_style.map(|c| c.resolve()),
                    on_mouse_enter: interactions
                        .on_mouse_enter
                        .map(HoverMoveOrTransition::into_transition),
                    on_mouse_leave: interactions
                        .on_mouse_leave
                        .map(HoverMoveOrTransition::into_transition),
                    on_mouse_move: interactions.on_mouse_move.map(HoverMoveHandler::Point),
                },
                HitInteraction::SelectOption {
                    id,
                    option_index,
                    interactions,
                    ..
                } => HoveredWidget {
                    target_id: HoverTargetId::SelectOption {
                        widget_id: id,
                        option_index,
                    },
                    cursor_style: interactions.cursor_style.map(|c| c.resolve()),
                    on_mouse_enter: interactions
                        .on_mouse_enter
                        .map(HoverMoveOrTransition::into_transition),
                    on_mouse_leave: interactions
                        .on_mouse_leave
                        .map(HoverMoveOrTransition::into_transition),
                    on_mouse_move: interactions.on_mouse_move.map(HoverMoveHandler::Point),
                },
                HitInteraction::CanvasItem {
                    id,
                    item_id,
                    item_interactions,
                    cursor_style,
                    canvas_origin,
                    item_origin,
                    inverse_transform,
                    text_hits,
                } => {
                    let context = CanvasPointerContext {
                        item_id,
                        canvas_origin,
                        item_origin,
                        inverse_transform,
                        text_hits,
                    };
                    HoveredWidget {
                        target_id: HoverTargetId::CanvasItem {
                            widget_id: id,
                            item_id,
                        },
                        cursor_style,
                        on_mouse_enter: item_interactions.on_mouse_enter.map(|command| {
                            HoverTransitionHandler::Canvas(command, context.clone())
                        }),
                        on_mouse_leave: item_interactions.on_mouse_leave.map(|command| {
                            HoverTransitionHandler::Canvas(command, context.clone())
                        }),
                        on_mouse_move: item_interactions
                            .on_mouse_move
                            .map(|command| HoverMoveHandler::Canvas(command, context)),
                    }
                }
            })
            .collect()
    }

    pub(in crate::runtime) fn handle_hover(&mut self, viewport: Rect) -> bool {
        let started_at = text_profile_enabled().then_some(Instant::now());
        let revision_before = self.invalidation.revision();
        let cursor_position = self.cursor_position;
        let next_hovered = self.hover_path(viewport);
        let hover_path_changed = self.hovered_widgets.len() != next_hovered.len()
            || self
                .hovered_widgets
                .iter()
                .zip(next_hovered.iter())
                .any(|(previous, next)| previous.target_id != next.target_id);
        let mut prefix_len = 0usize;
        while prefix_len < self.hovered_widgets.len()
            && prefix_len < next_hovered.len()
            && self.hovered_widgets[prefix_len].target_id == next_hovered[prefix_len].target_id
        {
            prefix_len += 1;
        }

        let previous_hovered = std::mem::take(&mut self.hovered_widgets);
        for previous in previous_hovered[prefix_len..].iter().rev() {
            if let Some(command) = previous.on_mouse_leave.as_ref() {
                self.execute_hover_transition_handler(command, cursor_position);
            }
        }

        // 退出 hover 的 widget：从 tooltip_hover_started_at 移除。
        for previous in previous_hovered[prefix_len..].iter() {
            if let HoverTargetId::Widget(id) = previous.target_id {
                self.tooltip_hover_started_at.remove(&id);
            }
        }

        // 新进入 hover 的 widget：记录起始时间，用于 Tooltip delay 计算。
        let now = Instant::now();
        for hovered in next_hovered[prefix_len..].iter() {
            if let HoverTargetId::Widget(id) = hovered.target_id {
                self.tooltip_hover_started_at.entry(id).or_insert(now);
            }
        }

        for hovered in next_hovered[prefix_len..].iter().rev() {
            if let Some(command) = hovered.on_mouse_enter.as_ref() {
                self.execute_hover_transition_handler(command, cursor_position);
            }
        }

        if let Some(position) = cursor_position {
            for hovered in next_hovered.iter().rev() {
                if let Some(command) = hovered.on_mouse_move.as_ref() {
                    self.execute_hover_move_handler(command, position);
                }
            }
        }

        self.hovered_widgets = next_hovered;
        if hover_path_changed {
            self.hover_epoch = self.hover_epoch.wrapping_add(1);
        }
        let scrollbar_changed = self.sync_scrollbar_hover();
        let cursor_changed = self.update_cursor_icon();
        let changed = hover_path_changed
            || scrollbar_changed
            || cursor_changed
            || self.invalidation.revision() != revision_before;
        let _ = started_at;
        changed
    }
}
