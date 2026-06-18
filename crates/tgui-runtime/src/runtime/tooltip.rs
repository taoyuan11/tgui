use super::*;
use crate::runtime::state::TooltipDismissReason;
use crate::ui::widget::{ActiveTooltipState, TooltipTrigger};

impl<VM: 'static> BoundRuntimeHandler<VM> {
    pub(super) fn widget_has_tooltip_in_computed(&mut self, widget_id: WidgetId) -> bool {
        self.cached_scene
            .as_ref()
            .and_then(|cached| cached.layout.as_ref())
            .and_then(|layout| layout.resolved_widget(widget_id))
            .map(|resolved| resolved.tooltip.is_some())
            .unwrap_or(false)
    }

    pub(super) fn clear_tooltip_hover_suppression_if_needed(&mut self, widget_id: WidgetId) {
        self.tooltip_state
            .clear_suppression_for_trigger(TooltipTrigger::Hover, widget_id);
    }

    pub(super) fn clear_tooltip_focus_suppression_if_needed(&mut self, widget_id: WidgetId) {
        self.tooltip_state
            .clear_suppression_for_trigger(TooltipTrigger::Focus, widget_id);
    }

    pub(super) fn reset_tooltip_long_press_session(&mut self, widget_id: WidgetId) {
        self.tooltip_state
            .clear_suppression_for_trigger(TooltipTrigger::LongPress, widget_id);
        if self.tooltip_state.long_press_candidate == Some(widget_id) {
            self.tooltip_state.long_press_candidate = None;
        }
        if matches!(
            self.tooltip_state.active,
            Some(active) if active.widget_id == widget_id && active.trigger == TooltipTrigger::LongPress
        ) {
            self.tooltip_state.clear_active();
        }
    }

    pub(super) fn dismiss_active_tooltip(&mut self, _reason: TooltipDismissReason) -> bool {
        let Some(active) = self.tooltip_state.active else {
            return false;
        };
        self.tooltip_state.suppress(active);
        self.tooltip_state.clear_active();
        self.next_tooltip_wakeup_deadline = None;
        self.invalidate_computed_scene();
        true
    }

    pub(super) fn schedule_tooltip_long_press_hide(&mut self, widget_id: WidgetId, now: Instant) {
        if matches!(
            self.tooltip_state.active,
            Some(active) if active.widget_id == widget_id && active.trigger == TooltipTrigger::LongPress
        ) {
            self.tooltip_state.long_press_release_deadline =
                Some(now + super::TOOLTIP_LONG_PRESS_HIDE_DELAY);
        }
    }

    pub(super) fn flush_tooltip_release_if_due(&mut self, now: Instant) -> bool {
        let should_hide = self
            .tooltip_state
            .long_press_release_deadline
            .map(|deadline| deadline <= now)
            .unwrap_or(false);
        if !should_hide {
            return false;
        }
        self.tooltip_state.clear_active();
        self.invalidate_computed_scene();
        true
    }

    pub(super) fn resolve_active_tooltip(&mut self, now: Instant) -> Option<ActiveTooltipState> {
        if let Some(widget_id) = self.tooltip_state.long_press_candidate {
            if self.widget_has_tooltip_in_computed(widget_id)
                && self.tooltip_state.long_press_suppressed != Some(widget_id)
            {
                let active = ActiveTooltipState {
                    widget_id,
                    trigger: TooltipTrigger::LongPress,
                };
                self.tooltip_state.active = Some(active);
                return Some(active);
            }
        }

        if let Some(widget_id) = self.focused_widget_id() {
            if self.widget_has_tooltip_in_computed(widget_id)
                && self.tooltip_state.focus_suppressed != Some(widget_id)
            {
                let active = ActiveTooltipState {
                    widget_id,
                    trigger: TooltipTrigger::Focus,
                };
                self.tooltip_state.active = Some(active);
                return Some(active);
            }
        }

        let hovered_ids: Vec<_> = self
            .hovered_widgets
            .iter()
            .rev()
            .filter_map(|hovered| match hovered.target_id {
                HoverTargetId::Widget(widget_id) => Some(widget_id),
                _ => None,
            })
            .collect();
        for widget_id in hovered_ids {
            if self.tooltip_state.hover_suppressed == Some(widget_id) {
                continue;
            }
            if !self.widget_has_tooltip_in_computed(widget_id) {
                continue;
            }
            let active = ActiveTooltipState {
                widget_id,
                trigger: TooltipTrigger::Hover,
            };
            self.tooltip_state.active = Some(active);
            return Some(active);
        }

        if self
            .tooltip_state
            .long_press_release_deadline
            .map(|deadline| deadline <= now)
            .unwrap_or(false)
        {
            self.tooltip_state.clear_active();
        } else {
            self.tooltip_state.active = None;
        }
        None
    }
}
