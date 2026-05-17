use super::super::state::{SMOOTH_SCROLL_EPSILON, SMOOTH_SCROLL_LERP};
use super::*;

impl<VM: 'static> BoundRuntimeHandler<VM> {
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
        scroll_delta: Point,
    ) -> bool {
        let inner = frame.inset(padding);
        if inner.is_empty() || scroll_delta.y.abs() <= f32::EPSILON {
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
                    true,
                    inner.width.get(),
                );
                layout
            });
        let max_scroll_y = Dp::new((layout.height.max(line_height) - inner.height.get()).max(0.0));
        if max_scroll_y <= Dp::ZERO {
            return false;
        }

        let current = self
            .scroll_states
            .get(&widget_id)
            .copied()
            .unwrap_or(Point::ZERO);
        let next = Point::new(
            Dp::ZERO,
            (current.y - scroll_delta.y).clamp(Dp::ZERO, max_scroll_y),
        );
        if (next.y - current.y).abs() <= 0.01 {
            return false;
        }

        self.smooth_scroll_states.remove(&widget_id);
        self.set_scroll_offset(widget_id, next);
        true
    }

    pub(super) fn set_smooth_scroll_target(&mut self, widget_id: WidgetId, target: Point) {
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
