use super::*;

impl<VM: 'static> BoundRuntimeHandler<VM> {
    pub(super) fn prune_removed_widget_state(&mut self, removed_ids: &HashSet<WidgetId>) {
        if removed_ids.is_empty() {
            return;
        }

        let removed_owner_ids = removed_ids
            .iter()
            .map(|widget_id| widget_id.raw())
            .collect::<HashSet<_>>();
        self.invalidation
            .remove_reactive_targets_for_widgets(&removed_owner_ids);

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
                HoverTargetId::SplitterHandle { widget_id, .. } => {
                    !removed_ids.contains(&widget_id)
                }
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
                if !self.restore_list_focus_after_target_removal(&focused, removed_ids)
                    && !self.restore_tree_focus_after_target_removal(&focused, removed_ids)
                {
                    self.restore_focus_after_target_removal(&focused, removed_ids);
                }
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
                HoverTargetId::SplitterHandle { .. } => false,
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
        self.touch_scroll_inertia_states
            .retain(|widget_id, _| !removed_ids.contains(widget_id));
        self.virtual_states
            .retain(|widget_id, _| !removed_ids.contains(widget_id));
        self.select_open_states
            .retain(|widget_id, _| !removed_ids.contains(widget_id));
        self.menu_open_states
            .retain(|widget_id, _| !removed_ids.contains(widget_id));
        self.context_menu_anchor_states
            .retain(|widget_id, _| !removed_ids.contains(widget_id));
        self.menu_keyboard_cursor
            .retain(|widget_id, _| !removed_ids.contains(widget_id));
        self.media_event_states
            .retain(|widget_id, _| !removed_ids.contains(widget_id));
    }

    fn restore_focus_after_target_removal(
        &mut self,
        previous: &FocusedWidget<VM>,
        removed_ids: &HashSet<WidgetId>,
    ) {
        let restore_target = self.focused_overlay_return_target.take().or_else(|| {
            self.cached_scene.as_ref().and_then(|cached| {
                cached
                    .computed
                    .overlay_close_handlers
                    .iter()
                    .rev()
                    .find_map(|handle| {
                        handle
                            .return_focus_to
                            .filter(|widget_id| !removed_ids.contains(widget_id))
                    })
            })
        });
        if let Some(widget_id) = restore_target {
            let target = self.cached_scene.as_ref().and_then(|cached| {
                cached
                    .computed
                    .hit_regions
                    .iter()
                    .chain(cached.computed.overlay_hit_regions.iter())
                    .find_map(|region| {
                        let focus = region.focus.as_ref()?;
                        (focus.widget_id == widget_id && !removed_ids.contains(&widget_id)).then(
                            || FocusedWidget {
                                widget_id,
                                scope_path: focus.scope_path.clone(),
                                on_blur: focus.on_blur.clone(),
                            },
                        )
                    })
            });
            if let Some(target) = target {
                self.update_focus(Some(target), None, true);
                return;
            }
        }

        let mut candidates = self
            .cached_scene
            .as_ref()
            .map(|cached| {
                let mut seen = HashSet::new();
                cached
                    .computed
                    .hit_regions
                    .iter()
                    .chain(cached.computed.overlay_hit_regions.iter())
                    .filter_map(|region| {
                        let focus = region.focus.as_ref()?;
                        (focus.tab_index.unwrap_or(0) >= 0
                            && !removed_ids.contains(&focus.widget_id)
                            && seen.insert(focus.widget_id))
                        .then(|| focus.clone())
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        candidates.sort_by(|left, right| {
            let left_bucket = left.tab_index.unwrap_or(0);
            let right_bucket = right.tab_index.unwrap_or(0);
            match (left_bucket > 0, right_bucket > 0) {
                (true, true) => left_bucket
                    .cmp(&right_bucket)
                    .then_with(|| left.order.cmp(&right.order)),
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                (false, false) => left.order.cmp(&right.order),
            }
        });
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

        self.update_focus(None, None, false);
    }
}
