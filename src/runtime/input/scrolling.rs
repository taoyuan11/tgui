use super::super::state::{SMOOTH_SCROLL_EPSILON, SMOOTH_SCROLL_LERP};
use super::*;

impl<VM: 'static> BoundRuntimeHandler<VM> {
    pub(super) fn begin_touch_scroll_drag(&mut self, viewport: Rect) -> bool {
        if self.active_touch_scroll.is_some()
            || self.active_scrollbar_drag.is_some()
            || self.active_slider_drag.is_some()
            || self.active_canvas_drag.is_some()
            || self.active_text_selection.is_some()
            || self.active_pinch.is_some()
            || self
                .active_gesture
                .as_ref()
                .map(|gesture| gesture.captured || gesture.long_press_triggered)
                .unwrap_or(false)
        {
            return false;
        }

        let Some(cursor_position) = self.cursor_position else {
            return false;
        };
        let hit_path = self.hit_path(viewport);
        if hit_path
            .last()
            .map(Self::touch_hit_claims_drag)
            .unwrap_or(false)
        {
            return false;
        }

        let Some(region) = self.scroll_regions().into_iter().rev().find(|region| {
            !region.visible_frame.is_empty()
                && region.visible_frame.contains(cursor_position)
                && (region.can_scroll_x() || region.can_scroll_y())
        }) else {
            return false;
        };

        self.smooth_scroll_states.remove(&region.id);
        self.active_touch_scroll = Some(TouchScrollDrag {
            widget_id: region.id,
            start_cursor: cursor_position,
            start_scroll_offset: self.effective_scroll_offset(region.id, region.scroll_offset),
            max_offset: region.max_offset(),
            visible_frame: region.visible_frame,
            can_scroll_x: region.can_scroll_x(),
            can_scroll_y: region.can_scroll_y(),
            activated: false,
        });
        true
    }

    pub(super) fn handle_touch_scroll_drag(&mut self) -> bool {
        let Some(mut drag) = self.active_touch_scroll else {
            return false;
        };
        let Some(cursor_position) = self.cursor_position else {
            return false;
        };

        let delta = Point::new(
            cursor_position.x - drag.start_cursor.x,
            cursor_position.y - drag.start_cursor.y,
        );
        let activate_x = drag.can_scroll_x
            && delta.x.abs().get() >= super::super::TOUCH_SCROLL_ACTIVATION_THRESHOLD;
        let activate_y = drag.can_scroll_y
            && delta.y.abs().get() >= super::super::TOUCH_SCROLL_ACTIVATION_THRESHOLD;
        if !drag.activated && !activate_x && !activate_y {
            return false;
        }

        drag.activated = true;
        self.pending_click = None;
        self.active_touch_scroll = Some(drag);

        let mut next_offset = Point::new(
            if drag.can_scroll_x {
                (drag.start_scroll_offset.x - delta.x).clamp(0.0, drag.max_offset.x)
            } else {
                drag.start_scroll_offset.x
            },
            if drag.can_scroll_y {
                (drag.start_scroll_offset.y - delta.y).clamp(0.0, drag.max_offset.y)
            } else {
                drag.start_scroll_offset.y
            },
        );

        if !drag.visible_frame.contains(cursor_position) {
            next_offset = Point::new(
                if drag.can_scroll_x {
                    next_offset.x.clamp(Dp::ZERO, drag.max_offset.x)
                } else {
                    next_offset.x
                },
                if drag.can_scroll_y {
                    next_offset.y.clamp(Dp::ZERO, drag.max_offset.y)
                } else {
                    next_offset.y
                },
            );
        }
        let previous = self
            .scroll_states
            .get(&drag.widget_id)
            .copied()
            .unwrap_or(drag.start_scroll_offset);
        if (previous.x - next_offset.x).abs() > 0.01 || (previous.y - next_offset.y).abs() > 0.01 {
            self.set_scroll_offset(drag.widget_id, next_offset);
            return true;
        }

        false
    }

    pub(super) fn end_touch_scroll_drag(&mut self) -> bool {
        self.active_touch_scroll.take().is_some()
    }

    fn touch_hit_claims_drag(interaction: &HitInteraction<VM>) -> bool {
        match interaction {
            HitInteraction::Disabled { .. } => false,
            HitInteraction::Widget { interactions, .. } => interactions.on_mouse_move.is_some(),
            HitInteraction::SelectableText { .. }
            | HitInteraction::Slider { .. }
            | HitInteraction::TextInput { .. }
            | HitInteraction::CanvasItem { .. } => true,
            HitInteraction::Switch { .. }
            | HitInteraction::Checkbox { .. }
            | HitInteraction::Radio { .. }
            | HitInteraction::SelectTrigger { .. }
            | HitInteraction::TabTrigger { .. }
            | HitInteraction::SelectOption { .. } => false,
        }
    }

    pub(super) fn effective_scroll_offset(&self, widget_id: WidgetId, fallback: Point) -> Point {
        self.smooth_scroll_states
            .get(&widget_id)
            .map(|state| state.target)
            .or_else(|| self.scroll_states.get(&widget_id).copied())
            .unwrap_or(fallback)
    }

    pub(super) fn scroll_multiline_text_input(
        &mut self,
        widget_id: WidgetId,
        value: &str,
        frame: Rect,
        padding: crate::ui::layout::Insets,
        text_style: &Text,
        auto_wrap: bool,
        show_scrollbar: bool,
        scroll_delta: Point,
    ) -> bool {
        let content_viewport = crate::ui::widget::text_input_content_viewport(
            frame,
            padding,
            true,
            show_scrollbar,
            &self.theme,
            self.unit_context(),
        );
        if content_viewport.is_empty()
            || (scroll_delta.x.abs() <= f32::EPSILON && scroll_delta.y.abs() <= f32::EPSILON)
        {
            return false;
        }

        let (_, _, line_height, _) =
            super::super::resolved_input_text_metrics(&self.theme, self.unit_context(), text_style);
        let layout = self
            .text_input_layout_snapshot(widget_id)
            .cloned()
            .unwrap_or_else(|| {
                let (layout, _, _) = super::super::input_text_layout(
                    &self.font_manager,
                    &self.theme,
                    self.unit_context(),
                    text_style,
                    value,
                    true,
                    auto_wrap,
                    crate::ui::widget::text_input_layout_width(
                        content_viewport,
                        true,
                        auto_wrap,
                        super::INPUT_CARET_WIDTH,
                    ),
                );
                layout
            });
        let geometry = crate::ui::widget::text_input_content_geometry(
            &layout,
            line_height,
            content_viewport,
            true,
            auto_wrap,
            self.scroll_states
                .get(&widget_id)
                .copied()
                .unwrap_or(Point::ZERO),
            super::INPUT_CARET_WIDTH,
        );
        let max_scroll_x = if auto_wrap {
            Dp::ZERO
        } else {
            (geometry.content_width - content_viewport.width).max(0.0)
        };
        let max_scroll_y = (geometry.content_height - content_viewport.height).max(0.0);
        if max_scroll_x <= Dp::ZERO && max_scroll_y <= Dp::ZERO {
            return false;
        }

        let current = self
            .scroll_states
            .get(&widget_id)
            .copied()
            .unwrap_or(Point::ZERO);
        let next = Point::new(
            if auto_wrap {
                Dp::ZERO
            } else {
                (current.x - scroll_delta.x).clamp(Dp::ZERO, max_scroll_x)
            },
            (current.y - scroll_delta.y).clamp(Dp::ZERO, max_scroll_y),
        );
        if (next.x - current.x).abs() <= 0.01 && (next.y - current.y).abs() <= 0.01 {
            return false;
        }

        self.smooth_scroll_states.remove(&widget_id);
        self.set_scroll_offset(widget_id, next);
        true
    }

    pub(in crate::runtime) fn set_smooth_scroll_target(
        &mut self,
        widget_id: WidgetId,
        target: Point,
    ) {
        self.smooth_scroll_states
            .insert(widget_id, SmoothScrollState { target });
        let _ = self.advance_smooth_scroll();
    }

    pub(in crate::runtime) fn advance_smooth_scroll(&mut self) -> bool {
        if self.smooth_scroll_states.is_empty() {
            return false;
        }

        let mut changed = false;
        let mut finished = Vec::new();
        let updates: Vec<_> = self
            .smooth_scroll_states
            .iter()
            .map(|(widget_id, state)| (*widget_id, *state))
            .collect();

        for (widget_id, state) in updates {
            let current = self
                .scroll_states
                .get(&widget_id)
                .copied()
                .unwrap_or(Point::ZERO);
            let dx = state.target.x - current.x;
            let dy = state.target.y - current.y;
            if dx.abs().get() <= SMOOTH_SCROLL_EPSILON && dy.abs().get() <= SMOOTH_SCROLL_EPSILON {
                self.set_scroll_offset(widget_id, state.target);
                finished.push(widget_id);
                changed = true;
                continue;
            }

            let next = Point::new(
                current.x + dx * SMOOTH_SCROLL_LERP,
                current.y + dy * SMOOTH_SCROLL_LERP,
            );
            self.set_scroll_offset(widget_id, next);
            changed = true;
        }

        for widget_id in finished {
            self.smooth_scroll_states.remove(&widget_id);
        }

        changed
    }

    pub(super) fn reset_single_line_input_focus_state(&mut self, widget_id: WidgetId, text: &str) {
        let changed = self.update_text_edit_state(widget_id, text, |state| {
            state.cursor = 0;
            state.anchor = 0;
            state.composition = None;
            state.scroll_x = Dp::ZERO;
            state.scroll_y = Dp::ZERO;
            state.preferred_column_x = None;
        });

        self.smooth_scroll_states.remove(&widget_id);
        self.set_scroll_offset(widget_id, Point::ZERO);

        if changed {
            self.sync_ime_state();
        }
    }
}
