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
        if let Some(focused) = self.focused_widget.take() {
            if removed_ids.contains(&focused.widget_id) {
                self.focus_visible = false;
                self.active_key_repeat = None;
                self.restore_focus_after_target_removal(&focused, removed_ids);
            } else {
                self.focused_widget = Some(focused);
            }
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
        if self
            .active_gesture
            .as_ref()
            .map(|session| removed_ids.contains(&session.widget_id))
            .unwrap_or(false)
        {
            let _ = self.cancel_active_gesture();
        }
        if self
            .active_pinch
            .as_ref()
            .map(|session| removed_ids.contains(&session.widget_id))
            .unwrap_or(false)
        {
            let _ = self.cancel_active_gesture();
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
        self.virtual_states
            .retain(|widget_id, _| !removed_ids.contains(widget_id));
        self.select_open_states
            .retain(|widget_id, _| !removed_ids.contains(widget_id));
        self.media_event_states
            .retain(|widget_id, _| !removed_ids.contains(widget_id));
    }

    fn restore_focus_after_target_removal(
        &mut self,
        previous: &FocusedWidget<VM>,
        removed_ids: &HashSet<WidgetId>,
    ) {
        let candidates = self.focusable_widgets_in_tab_order();
        if let Some(next) = candidates
            .iter()
            .find(|candidate| {
                candidate.scope_path == previous.scope_path
                    && !removed_ids.contains(&candidate.widget_id)
            })
            .map(|candidate| FocusedWidget {
                widget_id: candidate.widget_id,
                scope_path: candidate.scope_path.clone(),
                on_blur: candidate.on_blur.clone(),
            })
        {
            self.update_focus(Some(next), None, true);
            return;
        }

        let restore_target = self
            .computed_scene()
            .overlay_close_handlers
            .iter()
            .rev()
            .find_map(|handle| {
                handle
                    .return_focus_to
                    .filter(|widget_id| !removed_ids.contains(widget_id))
            });
        if let Some(widget_id) = restore_target {
            if self.restore_overlay_focus_if_needed(widget_id) {
                return;
            }
        }

        self.update_focus(None, None, false);
    }
}
