use super::*;

impl<VM: 'static> BoundRuntimeHandler<VM> {
    pub(super) fn edit_focused_text_input(
        &mut self,
        edit: impl FnOnce(&mut RopeBuffer, &TextEditState) -> Option<(TextEditState, TextChange)>,
    ) -> bool {
        let started_at = text_profile_enabled().then_some(Instant::now());
        let Some(widget_id) = self.focused_text_input_id() else {
            return false;
        };
        let Some(region) = self.sync_text_input_buffer(widget_id) else {
            return false;
        };
        let current_text = self
            .text_input_buffers
            .get(&widget_id)
            .map(|session| session.current_text.as_str())
            .unwrap_or("");
        let state = self
            .text_edit_state(widget_id)
            .cloned()
            .unwrap_or_else(|| self.default_text_edit_state(widget_id, current_text));
        let (
            text_len_before,
            changed,
            next_rope,
            next_value,
            next_is_ascii,
            next_state,
            text_change,
        ) = {
            let session = self
                .text_input_buffers
                .get_mut(&widget_id)
                .expect("text input session should be initialized");
            let current_text = std::mem::take(&mut session.current_text);
            let state = state.clamped_to(&current_text);
            let mut buffer = RopeBuffer::from_parts(
                std::mem::take(&mut session.rope),
                current_text,
                session.current_text_is_ascii,
            );
            let Some((next_state, text_change)) = edit(&mut buffer, &state) else {
                let (next_rope, next_value, next_is_ascii) = buffer.into_parts();
                session.rope = next_rope;
                session.current_text = next_value;
                session.current_text_is_ascii = next_is_ascii;
                return false;
            };
            let (next_rope, next_value, next_is_ascii) = buffer.into_parts();
            let text_len_before = session.external_value.len();
            let changed = session.external_value != next_value;
            (
                text_len_before,
                changed,
                next_rope,
                next_value,
                next_is_ascii,
                next_state,
                text_change,
            )
        };
        let text_len_after = next_value.len();
        let (config, preferred_font, weight, font_size, line_height, letter_spacing, width, height) =
            self.text_input_session_config(&region);
        {
            let session = self
                .text_input_buffers
                .get_mut(&widget_id)
                .expect("text input session should exist after edit");
            session.rope = next_rope;
            session.current_text_is_ascii = next_is_ascii;
            let edit_replacement = Some((
                text_change.range_bytes.0,
                text_change.range_bytes.1,
                text_change.range_bytes.0,
                text_change.range_bytes.0 + text_change.inserted_text.len(),
            ));
            if changed {
                session.push_pending_change(text_change);
            }
            let canonical_value = next_value.as_str();
            let next_state = next_state.clamped_to(canonical_value);
            let display_text = text_edit_display_text(canonical_value, &next_state);
            refresh_session_buffer(
                &self.font_manager,
                session,
                config,
                preferred_font.as_deref(),
                weight,
                font_size,
                line_height,
                letter_spacing,
                width,
                height,
                region.multiline,
                region.auto_wrap,
                &next_state,
                display_text.as_ref(),
                edit_replacement,
            );
            session.current_text = next_value;
            self.text_edit_states.insert(widget_id, next_state);
        }
        if changed {
            self.invalidate_text_input_scene();
        }
        if let Some(state) = self.text_edit_states.get(&widget_id).cloned() {
            let canonical_value = self
                .text_input_buffers
                .get(&widget_id)
                .map(|session| session.current_text.clone())
                .unwrap_or_default();
            self.ensure_text_input_caret_visible(
                widget_id,
                region.context(&canonical_value),
                &state,
            );
        }
        self.reset_caret_blink();
        self.sync_ime_state();
        if let Some(started_at) = started_at {
            log_text_profile(
                "textarea_input_edit",
                started_at.elapsed(),
                format!(
                    "widget={:?} changed={} text_len={} -> {} multiline={}",
                    widget_id, changed, text_len_before, text_len_after, region.multiline,
                ),
            );
        }
        changed
    }

    pub(super) fn insert_text_at_focused_input(&mut self, inserted: &str) -> bool {
        if inserted.is_empty() {
            return false;
        }
        let Some(widget_id) = self.focused_text_input_id() else {
            return false;
        };
        let Some(region) = self.sync_text_input_buffer(widget_id) else {
            return false;
        };
        let inserted_owned = normalize_text_input_value(inserted, region.multiline);
        if inserted_owned.is_empty() {
            return false;
        }
        let changed =
            self.edit_focused_text_input(|buffer: &mut RopeBuffer, state: &TextEditState| {
                let (start, end) = state
                    .selection_range()
                    .unwrap_or((state.cursor, state.cursor));
                buffer.replace_byte_range(start, end, &inserted_owned);
                let cursor = start + inserted_owned.len();
                Some((
                    TextEditState {
                        cursor,
                        anchor: cursor,
                        composition: None,
                        scroll_x: state.scroll_x,
                        scroll_y: state.scroll_y,
                        preferred_column_x: None,
                    },
                    TextChange::new((start, end), inserted_owned.clone()),
                ))
            });
        if changed {
            let _ = self.open_focused_text_input_popover();
        }
        changed
    }

    pub(super) fn delete_backward_at_focused_input(&mut self) -> bool {
        self.edit_focused_text_input(|buffer: &mut RopeBuffer, state: &TextEditState| {
            let (start, end) = if let Some(range) = state.selection_range() {
                range
            } else if state.cursor > 0 {
                (
                    buffer.prev_grapheme_boundary_byte(state.cursor),
                    state.cursor,
                )
            } else {
                return None;
            };
            buffer.replace_byte_range(start, end, "");
            Some((
                TextEditState {
                    cursor: start,
                    anchor: start,
                    composition: None,
                    scroll_x: state.scroll_x,
                    scroll_y: state.scroll_y,
                    preferred_column_x: None,
                },
                TextChange::new((start, end), ""),
            ))
        })
    }

    pub(super) fn delete_forward_at_focused_input(&mut self) -> bool {
        self.edit_focused_text_input(|buffer: &mut RopeBuffer, state: &TextEditState| {
            let (start, end) = if let Some(range) = state.selection_range() {
                range
            } else if state.cursor < buffer.len_bytes() {
                (
                    state.cursor,
                    buffer.next_grapheme_boundary_byte(state.cursor),
                )
            } else {
                return None;
            };
            buffer.replace_byte_range(start, end, "");
            Some((
                TextEditState {
                    cursor: start,
                    anchor: start,
                    composition: None,
                    scroll_x: state.scroll_x,
                    scroll_y: state.scroll_y,
                    preferred_column_x: None,
                },
                TextChange::new((start, end), ""),
            ))
        })
    }

    pub(super) fn paste_into_focused_input(&mut self) -> bool {
        let Some(text) = self.clipboard.get_text() else {
            return false;
        };
        self.insert_text_at_focused_input(&text)
    }

    pub(super) fn cut_selected_text_from_input(&mut self) -> bool {
        let Some(selected) = self.selected_text_for_copy() else {
            return false;
        };
        self.clipboard.set_text(selected);
        self.delete_backward_at_focused_input()
    }

    pub(super) fn update_focused_input_composition(
        &mut self,
        text: String,
        cursor: Option<(usize, usize)>,
    ) -> bool {
        let Some(widget_id) = self.focused_text_input_id() else {
            return false;
        };
        let Some(region) = self.sync_text_input_buffer(widget_id) else {
            return false;
        };
        let current_value = self.text_input_current_value(widget_id, &region.controller);
        let changed = self.update_text_edit_state(widget_id, &current_value, |state| {
            let replace_range = state
                .selection_range()
                .unwrap_or((state.cursor, state.cursor));
            state.composition = if text.is_empty() {
                None
            } else {
                Some(crate::ui::widget::CompositionState {
                    replace_range,
                    text,
                    cursor,
                })
            };
        });
        if changed {
            if let Some(state) = self.text_edit_states.get(&widget_id).cloned() {
                self.refresh_text_input_session_display(widget_id, &region, &current_value, &state);
                self.ensure_text_input_caret_visible(
                    widget_id,
                    region.context(&current_value),
                    &state,
                );
            }
            self.reset_caret_blink();
            self.sync_ime_state();
        }
        changed
    }

    pub(super) fn clear_focused_input_composition(&mut self) -> bool {
        let Some(widget_id) = self.focused_text_input_id() else {
            return false;
        };
        let Some(region) = self.sync_text_input_buffer(widget_id) else {
            return false;
        };
        let current_value = self.text_input_current_value(widget_id, &region.controller);
        let changed = self.update_text_edit_state(widget_id, &current_value, |state| {
            state.composition = None;
        });
        if changed {
            if let Some(state) = self.text_edit_states.get(&widget_id).cloned() {
                self.refresh_text_input_session_display(widget_id, &region, &current_value, &state);
                self.ensure_text_input_caret_visible(
                    widget_id,
                    region.context(&current_value),
                    &state,
                );
            }
            self.sync_ime_state();
        }
        changed
    }

    pub(super) fn focused_input_has_active_composition(&mut self) -> bool {
        self.focused_text_input_id()
            .and_then(|widget_id| self.text_edit_state(widget_id))
            .and_then(|state| state.composition.as_ref())
            .is_some()
    }
}
