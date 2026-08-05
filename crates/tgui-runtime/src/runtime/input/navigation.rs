use super::*;

impl<VM: 'static> BoundRuntimeHandler<VM> {
    pub(in crate::runtime) fn adjust_focused_number_input(&mut self, direction: i32) -> bool {
        let Some(widget_id) = self.focused_text_input_id() else {
            return false;
        };
        let computed = self.computed_scene();
        let behavior = computed
            .hit_regions
            .iter()
            .chain(computed.overlay_hit_regions.iter())
            .find_map(|region| match &region.interaction {
                HitInteraction::TextInput {
                    id, interactions, ..
                } if *id == widget_id => interactions.number_input.clone(),
                _ => None,
            });
        let Some(behavior) = behavior else {
            return false;
        };
        if direction >= 0 {
            self.execute_command(&behavior.increment);
        } else {
            self.execute_command(&behavior.decrement);
        }
        true
    }

    pub(in crate::runtime) fn move_focused_input_cursor(
        &mut self,
        next_index: impl FnOnce(&str, bool, &TextEditState) -> usize,
        extend_selection: bool,
    ) -> bool {
        let Some(widget_id) = self.focused_text_input_id() else {
            return false;
        };
        let Some(region) = self.sync_text_input_buffer(widget_id) else {
            return false;
        };
        let current_value = self.text_input_current_value(widget_id, &region.controller);
        let state = self
            .text_edit_state(widget_id)
            .cloned()
            .unwrap_or_else(|| self.default_text_edit_state(widget_id, &current_value));
        let clamped_state = {
            let session = self
                .text_input_buffers
                .get(&widget_id)
                .expect("text input session should be initialized");
            state.clamped_to(session.current_text())
        };
        let cursor = {
            let session = self
                .text_input_buffers
                .get(&widget_id)
                .expect("text input session should be initialized");
            next_index(
                session.current_text(),
                session.current_text_is_ascii,
                &clamped_state,
            )
        };
        let anchor = if extend_selection {
            clamped_state.anchor
        } else {
            cursor
        };
        self.text_edit_states.insert(
            widget_id,
            TextEditState {
                cursor,
                anchor,
                composition: None,
                scroll_x: clamped_state.scroll_x,
                scroll_y: clamped_state.scroll_y,
                preferred_column_x: None,
            },
        );
        if let Some(state) = self.text_edit_states.get(&widget_id).cloned() {
            self.ensure_text_input_caret_visible(widget_id, region.context(&current_value), &state);
        }
        self.reset_caret_blink();
        self.sync_ime_state();
        true
    }

    pub(in crate::runtime) fn ensure_text_input_caret_visible(
        &mut self,
        widget_id: WidgetId,
        input: TextInputContext<'_>,
        state: &TextEditState,
    ) {
        let content_viewport = input.content_viewport(&self.theme, self.unit_context());
        let (_, _, line_height, _) = super::super::resolved_input_text_metrics(
            &self.theme,
            self.unit_context(),
            input.text_style,
        );
        let layout_owned;
        let (layout, caret) = if let Some(composition) = state.composition.as_ref() {
            let display_text = text_edit_display_text(input.text, state);
            let start = composition.replace_range.0.min(input.text.len());
            let caret_offset = composition
                .cursor
                .map(|(_, end)| end.min(composition.text.len()))
                .unwrap_or(composition.text.len());
            let caret = start + caret_offset;
            let layout_ref = self.text_input_buffers.get(&widget_id).and_then(|session| {
                (session.display_text == display_text.as_ref())
                    .then_some(session.layout_snapshot.as_ref())
                    .flatten()
            });
            let layout = if let Some(layout) = layout_ref {
                layout
            } else {
                let (layout, _, _) = super::super::input_text_layout(
                    &self.font_manager,
                    &self.theme,
                    self.unit_context(),
                    TextInputContext {
                        text: display_text.as_ref(),
                        ..input
                    },
                    input.layout_width(content_viewport),
                );
                layout_owned = layout;
                &layout_owned
            };
            (layout, caret.min(display_text.len()))
        } else {
            let layout = self
                .text_input_buffers
                .get(&widget_id)
                .and_then(|session| session.layout_snapshot.as_ref());
            let layout = if let Some(layout) = layout {
                layout
            } else {
                let (layout, _, _) = super::super::input_text_layout(
                    &self.font_manager,
                    &self.theme,
                    self.unit_context(),
                    input,
                    input.layout_width(content_viewport),
                );
                layout_owned = layout;
                &layout_owned
            };
            (layout, state.cursor.min(input.text.len()))
        };
        let caret_x = layout.x_for_index(caret);
        let caret_y = layout.top_for_index(caret);
        let caret_h = layout.line_height_for_index(caret).max(line_height);
        let max_x = if input.multiline && input.auto_wrap {
            (layout.width - content_viewport.width.get()).max(0.0)
        } else {
            (layout.width + INPUT_CARET_WIDTH - content_viewport.width.get()).max(0.0)
        };
        let max_y = (layout.height.max(line_height) - content_viewport.height.get()).max(0.0);
        let mut next_scroll = Point::new(
            state.scroll_x.clamp(0.0, max_x),
            state.scroll_y.clamp(0.0, max_y),
        );

        if input.multiline {
            if caret_y < next_scroll.y.get() {
                next_scroll.y = Dp::new(caret_y);
            } else if caret_y + caret_h > next_scroll.y.get() + content_viewport.height.get() {
                next_scroll.y =
                    Dp::new((caret_y + caret_h - content_viewport.height.get()).max(0.0));
            }
        }

        if !input.multiline || !input.auto_wrap {
            let caret_right = caret_x + INPUT_CARET_WIDTH;
            if caret_x < next_scroll.x.get() {
                next_scroll.x = Dp::new(caret_x);
            } else if caret_right > next_scroll.x.get() + content_viewport.width.get() {
                next_scroll.x = Dp::new((caret_right - content_viewport.width.get()).max(0.0));
            }
        }

        next_scroll.x = next_scroll.x.clamp(0.0, max_x);
        next_scroll.y = next_scroll.y.clamp(0.0, max_y);
        self.set_scroll_offset(widget_id, next_scroll);
    }

    pub(in crate::runtime) fn move_focused_input_cursor_vertically(
        &mut self,
        direction: i32,
        extend_selection: bool,
    ) -> bool {
        let Some(widget_id) = self.focused_text_input_id() else {
            return false;
        };
        let Some(region) = self.sync_text_input_buffer(widget_id) else {
            return false;
        };
        if !region.multiline {
            return false;
        }
        let (frame, padding, text_style) = {
            let computed = self.computed_scene();
            let Some((frame, padding, text_style)) = computed
                .hit_regions
                .iter()
                .chain(computed.overlay_hit_regions.iter())
                .find_map(|region| match &region.interaction {
                    HitInteraction::TextInput {
                        id,
                        frame,
                        padding,
                        text_style,
                        multiline: true,
                        ..
                    } if *id == widget_id => Some((*frame, *padding, text_style.clone())),
                    _ => None,
                })
            else {
                return false;
            };
            (frame, padding, text_style)
        };
        let inner = frame.inset(padding);
        let current_value = self.text_input_current_value(widget_id, &region.controller);
        let state = self
            .text_edit_state(widget_id)
            .cloned()
            .unwrap_or_else(|| self.default_text_edit_state(widget_id, &current_value));
        let state = state.clamped_to(&current_value);
        let layout = self
            .text_input_layout_snapshot(widget_id)
            .cloned()
            .unwrap_or_else(|| {
                let (layout, _, _) = super::super::input_text_layout(
                    &self.font_manager,
                    &self.theme,
                    self.unit_context(),
                    TextInputContext {
                        frame,
                        padding,
                        text_style: &text_style,
                        text: &current_value,
                        multiline: true,
                        auto_wrap: region.auto_wrap,
                        show_scrollbar: region.show_scrollbar,
                    },
                    inner.width.get(),
                );
                layout
            });
        let current_line = layout.line_index_for_index(state.cursor);
        let target_line = if direction < 0 {
            current_line.saturating_sub(1)
        } else {
            (current_line + 1).min(layout.line_count().saturating_sub(1))
        };
        if target_line == current_line && direction != 0 {
            return true;
        }
        let preferred_x = state
            .preferred_column_x
            .unwrap_or_else(|| layout.x_for_index(state.cursor));
        let cursor = layout.index_for_point(preferred_x, layout.line_top(target_line));
        let anchor = if extend_selection {
            state.anchor
        } else {
            cursor
        };
        let next_state = TextEditState {
            cursor,
            anchor,
            composition: None,
            scroll_x: state.scroll_x,
            scroll_y: state.scroll_y,
            preferred_column_x: Some(preferred_x),
        };
        self.text_edit_states.insert(widget_id, next_state.clone());
        self.ensure_text_input_caret_visible(
            widget_id,
            TextInputContext {
                frame,
                padding,
                text_style: &text_style,
                text: &current_value,
                multiline: true,
                auto_wrap: region.auto_wrap,
                show_scrollbar: region.show_scrollbar,
            },
            &next_state,
        );
        self.reset_caret_blink();
        self.sync_ime_state();
        true
    }

    pub(in crate::runtime) fn select_all_focused_input(&mut self) -> bool {
        let Some(widget_id) = self.focused_text_input_id() else {
            return false;
        };
        let Some(region) = self.sync_text_input_buffer(widget_id) else {
            return false;
        };
        let cursor = self
            .text_input_buffers
            .get(&widget_id)
            .expect("text input session should be initialized")
            .current_text
            .len();
        self.text_edit_states.insert(
            widget_id,
            TextEditState {
                cursor,
                anchor: 0,
                composition: None,
                scroll_x: Dp::ZERO,
                scroll_y: Dp::ZERO,
                preferred_column_x: None,
            },
        );
        if let Some(state) = self.text_edit_states.get(&widget_id).cloned() {
            let current_value = self.text_input_current_value(widget_id, &region.controller);
            self.ensure_text_input_caret_visible(widget_id, region.context(&current_value), &state);
        }
        self.selected_text = Some(widget_id);
        self.reset_caret_blink();
        self.sync_ime_state();
        true
    }
}
