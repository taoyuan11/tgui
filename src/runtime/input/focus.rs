use super::*;

impl<VM: 'static> BoundRuntimeHandler<VM> {
    pub(super) fn selected_text_content(&mut self, widget_id: WidgetId) -> Option<String> {
        self.computed_scene()
            .hit_regions
            .iter()
            .find_map(|region| match &region.interaction {
                HitInteraction::SelectableText { id, text, .. } if *id == widget_id => {
                    Some(text.clone())
                }
                _ => None,
            })
    }

    pub(in crate::runtime) fn selected_text_for_copy(&mut self) -> Option<String> {
        let Some(widget_id) = self.selected_text else {
            return None;
        };
        if let Some(region) = self.sync_text_input_buffer(widget_id) {
            let current_value = self.text_input_current_value(widget_id, &region.controller);
            let (start, end) = self
                .text_edit_state(widget_id)
                .cloned()
                .unwrap_or_else(|| self.default_text_edit_state(widget_id, &current_value))
                .clamped_to(&current_value)
                .selection_range()?;
            return self.text_input_buffers.get(&widget_id).map(|state| {
                RopeBuffer::from_str(state.current_text()).slice_byte_range_to_string(start, end)
            });
        }

        let text = self.selected_text_content(widget_id)?;
        let (start, end) = self
            .text_edit_state(widget_id)
            .cloned()
            .unwrap_or_else(|| self.default_text_edit_state(widget_id, &text))
            .clamped_to(&text)
            .selection_range()?;
        Some(text[start..end].to_string())
    }

    pub(super) fn copy_selected_text_to_clipboard(&mut self) -> bool {
        let Some(text) = self.selected_text_for_copy() else {
            return false;
        };
        self.clipboard.set_text(text);
        true
    }

    pub(super) fn clear_selected_text(&mut self) -> bool {
        let had_selection = self.selected_text.take().is_some();
        let was_dragging = self.active_text_selection.take().is_some();
        if had_selection || was_dragging {
            self.invalidate_text_input_scene();
            return true;
        }
        false
    }

    pub(super) fn begin_text_selection(
        &mut self,
        widget_id: WidgetId,
        frame: Rect,
        padding: crate::ui::layout::Insets,
        text_style: Text,
        text: String,
        multiline: bool,
        auto_wrap: bool,
        show_scrollbar: bool,
        cursor: usize,
    ) {
        self.selected_text = Some(widget_id);
        self.active_text_selection = Some(TextSelectionDrag {
            widget_id,
            frame,
            padding,
            text_style: text_style.clone(),
            text: text.clone(),
            multiline,
            auto_wrap,
            show_scrollbar,
        });
        self.update_text_edit_state(widget_id, &text, |state| {
            state.cursor = cursor;
            state.anchor = cursor;
            state.composition = None;
        });
        self.invalidate_text_input_scene();
        self.reset_caret_blink();
    }

    pub(in crate::runtime) fn handle_text_selection_drag(&mut self) -> bool {
        let Some(drag) = self.active_text_selection.clone() else {
            return false;
        };
        let Some(point) = self.cursor_position else {
            return false;
        };
        let cursor = self.text_input_cursor_index_at_point(
            drag.widget_id,
            drag.frame,
            drag.padding,
            &drag.text_style,
            &drag.text,
            drag.multiline,
            drag.auto_wrap,
            drag.show_scrollbar,
            self.scroll_states
                .get(&drag.widget_id)
                .copied()
                .unwrap_or(Point::ZERO),
            point,
        );
        self.selected_text = Some(drag.widget_id);
        let changed = self.update_text_edit_state(drag.widget_id, &drag.text, |state| {
            state.cursor = cursor;
            state.composition = None;
        });
        if changed {
            if let Some(state) = self.text_edit_states.get(&drag.widget_id).cloned() {
                self.ensure_text_input_caret_visible(
                    drag.widget_id,
                    drag.frame,
                    drag.padding,
                    &drag.text_style,
                    drag.multiline,
                    drag.auto_wrap,
                    drag.show_scrollbar,
                    &drag.text,
                    &state,
                );
            }
            self.reset_caret_blink();
        }
        changed
    }

    pub(super) fn end_text_selection_drag(&mut self) -> bool {
        if self.active_text_selection.take().is_some() {
            self.invalidate_text_input_scene();
            return true;
        }
        false
    }

    pub(in crate::runtime) fn ime_cursor_request_data(
        caret_rect: Rect,
        units: UnitContext,
    ) -> ImeRequestData {
        ImeRequestData::default().with_cursor_area(
            PhysicalPosition::new(
                units.logical_to_physical(caret_rect.x.get()).round() as i32,
                units.logical_to_physical(caret_rect.y.get()).round() as i32,
            )
            .into(),
            PhysicalSize::new(
                units
                    .logical_to_physical(caret_rect.width.get())
                    .ceil()
                    .max(1.0) as u32,
                units
                    .logical_to_physical(caret_rect.height.get())
                    .ceil()
                    .max(1.0) as u32,
            )
            .into(),
        )
    }

    pub(in crate::runtime) fn focusable_widgets_in_tab_order(&mut self) -> Vec<FocusedWidget<VM>> {
        let mut focusable = Vec::new();
        let mut seen = HashSet::new();

        for region in &self.computed_scene().hit_regions {
            let candidate = match &region.interaction {
                HitInteraction::Widget {
                    id,
                    interactions,
                    focusable: true,
                } => Some(FocusedWidget {
                    widget_id: *id,
                    on_blur: interactions.on_blur.clone(),
                }),
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
                } => Some(FocusedWidget {
                    widget_id: *id,
                    on_blur: interactions.on_blur.clone(),
                }),
                _ => None,
            };

            if let Some(candidate) = candidate {
                if seen.insert(candidate.widget_id) {
                    focusable.push(candidate);
                }
            }
        }

        focusable
    }

    pub(super) fn advance_focus(&mut self, reverse: bool) -> bool {
        let focusable = self.focusable_widgets_in_tab_order();
        if focusable.is_empty() {
            return false;
        }

        let current = self.focused_widget_id();
        let next_index = match current.and_then(|id| {
            focusable
                .iter()
                .position(|candidate| candidate.widget_id == id)
        }) {
            Some(index) if reverse => {
                if index == 0 {
                    focusable.len() - 1
                } else {
                    index - 1
                }
            }
            Some(index) => (index + 1) % focusable.len(),
            None if reverse => focusable.len() - 1,
            None => 0,
        };

        let next = focusable
            .into_iter()
            .nth(next_index)
            .expect("focus target index should be in bounds");
        self.update_focus(Some(next), None, true);
        true
    }

    pub(in crate::runtime) fn update_focus(
        &mut self,
        next_widget: Option<FocusedWidget<VM>>,
        on_focus: Option<Command<VM>>,
        focus_visible: bool,
    ) {
        let current_id = self
            .focused_widget
            .as_ref()
            .map(|focused| focused.widget_id);
        let next_id = next_widget.as_ref().map(|focused| focused.widget_id);
        let previous_single_line_input = current_id
            .and_then(|widget_id| {
                self.cached_text_input_region_data(widget_id)
                    .or_else(|| self.text_input_region_data(widget_id))
                    .map(|region| (widget_id, region))
            })
            .filter(|(_, region)| !region.multiline);

        if current_id == next_id {
            self.focused_widget = next_widget;
            self.focus_visible = next_id.is_some() && focus_visible;
            return;
        }

        self.active_key_repeat = None;

        if let Some(previous) = self.focused_widget.take() {
            if let Some((widget_id, region)) = previous_single_line_input.as_ref() {
                if *widget_id == previous.widget_id {
                    let flushed = self.flush_text_input_session(*widget_id);
                    let current_value =
                        self.text_input_current_value(*widget_id, &region.controller);
                    self.reset_single_line_input_focus_state(*widget_id, &current_value);
                    if flushed.requires_global_invalidation {
                        self.invalidation.mark_dirty();
                    }
                }
            }
            if let Some(command) = previous.on_blur {
                self.execute_command(&command);
            }
        }

        self.focused_widget = next_widget;
        self.focus_visible = next_id.is_some() && focus_visible;

        if let Some(command) = on_focus {
            if next_id.is_some() {
                self.execute_command(&command);
            }
        }

        self.invalidate_text_input_scene();
    }
}
