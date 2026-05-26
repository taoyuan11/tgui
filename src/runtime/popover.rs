use super::*;

impl<VM: 'static> BoundRuntimeHandler<VM> {
    pub(super) fn resolve_active_hover_popover(&mut self) -> Option<WidgetId> {
        let cached = self.cached_scene.as_ref()?;
        let layout = cached.layout.as_ref()?;
        let cursor = self.cursor_position;
        for hovered in self.hovered_widgets.iter().rev() {
            let HoverTargetId::Widget(widget_id) = hovered.target_id else {
                continue;
            };
            let Some(resolved) = layout.resolved_widget(widget_id) else {
                continue;
            };
            let Some(popover) = resolved.popover.as_ref() else {
                continue;
            };
            if popover.disabled.resolve() {
                continue;
            }
            if !popover.trigger_mode.allows_hover() {
                continue;
            }
            return Some(widget_id);
        }
        let Some(cursor) = cursor else {
            return None;
        };
        for handle in cached.computed.overlay_close_handlers.iter().rev() {
            if handle.layer != crate::runtime::overlay::OverlayLayer::Popover {
                continue;
            }
            let Some(widget_id) = handle.source_widget_id else {
                continue;
            };
            if !handle.rect.contains(cursor) {
                continue;
            }
            let Some(resolved) = layout.resolved_widget(widget_id) else {
                continue;
            };
            let Some(popover) = resolved.popover.as_ref() else {
                continue;
            };
            if popover.disabled.resolve() || !popover.trigger_mode.allows_hover() {
                continue;
            }
            return Some(widget_id);
        }
        None
    }
}
