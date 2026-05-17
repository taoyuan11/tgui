use super::*;

impl<VM: 'static> BoundRuntimeHandler<VM> {
    pub(super) fn prune_removed_widget_state(&mut self, removed_ids: &HashSet<WidgetId>) {
        if removed_ids.is_empty() {
            return;
        }

        if let Some(cached) = self.cached_scene.as_mut() {
            for removed_id in removed_ids {
                cached.scene_chunks.remove(removed_id);
                cached.scene_chunk_parts.remove(removed_id);
                cached.visual_contexts.remove(removed_id);
                cached.lifecycle_states.remove(removed_id);
            }
        }

        self.hovered_widgets
            .retain(|hovered| match hovered.target_id {
                HoverTargetId::Widget(id) => !removed_ids.contains(&id),
                HoverTargetId::SelectOption { widget_id, .. } => !removed_ids.contains(&widget_id),
                HoverTargetId::CanvasItem { widget_id, .. } => !removed_ids.contains(&widget_id),
            });
        if self
            .hovered_scrollbar
            .map(|handle| removed_ids.contains(&handle.id))
            .unwrap_or(false)
        {
            self.hovered_scrollbar = None;
        }
        if self
            .active_scrollbar_drag
            .map(|drag| removed_ids.contains(&drag.handle.id))
            .unwrap_or(false)
        {
            self.active_scrollbar_drag = None;
        }
        if self
            .active_slider_drag
            .as_ref()
            .map(|drag| removed_ids.contains(&drag.widget_id))
            .unwrap_or(false)
        {
            self.active_slider_drag = None;
        }
        if self
            .pressed_widget
            .map(|widget_id| removed_ids.contains(&widget_id))
            .unwrap_or(false)
        {
            self.pressed_widget = None;
        }
        if self
            .focused_widget
            .as_ref()
            .map(|focused| removed_ids.contains(&focused.widget_id))
            .unwrap_or(false)
        {
            self.focused_widget = None;
            self.focus_visible = false;
            self.active_key_repeat = None;
        }
        if self
            .selected_text
            .map(|widget_id| removed_ids.contains(&widget_id))
            .unwrap_or(false)
        {
            self.selected_text = None;
        }
        if self
            .active_text_selection
            .as_ref()
            .map(|drag| removed_ids.contains(&drag.widget_id))
            .unwrap_or(false)
        {
            self.active_text_selection = None;
        }
        if self
            .pending_click
            .as_ref()
            .map(|pending| match pending.target_id {
                HoverTargetId::Widget(id) => removed_ids.contains(&id),
                HoverTargetId::SelectOption { widget_id, .. } => removed_ids.contains(&widget_id),
                HoverTargetId::CanvasItem { widget_id, .. } => removed_ids.contains(&widget_id),
            })
            .unwrap_or(false)
        {
            self.pending_click = None;
        }

        self.text_edit_states
            .retain(|widget_id, _| !removed_ids.contains(widget_id));
        self.text_input_buffers
            .retain(|widget_id, _| !removed_ids.contains(widget_id));
        self.text_input_regions
            .retain(|widget_id, _| !removed_ids.contains(widget_id));
        self.text_input_flush_data
            .retain(|widget_id, _| !removed_ids.contains(widget_id));
        self.scroll_states
            .retain(|widget_id, _| !removed_ids.contains(widget_id));
        self.smooth_scroll_states
            .retain(|widget_id, _| !removed_ids.contains(widget_id));
        self.select_open_states
            .retain(|widget_id, _| !removed_ids.contains(widget_id));
        self.media_event_states
            .retain(|widget_id, _| !removed_ids.contains(widget_id));
    }
}
