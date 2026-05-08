use std::collections::HashSet;
use std::time::Instant;

use crate::foundation::view_model::{Command, ValueCommand};
use crate::platform::cursor::{Cursor, CursorIcon};
use crate::platform::dpi::{PhysicalPosition, PhysicalSize};
use crate::platform::event::{
    ElementState, Ime, KeyEvent, MouseButton, MouseScrollDelta, WindowEvent,
};
use crate::platform::keyboard::{Key, KeyCode, NamedKey, PhysicalKey};
use crate::platform::window::ImeRequestData;
use crate::text::font::FontManager;
use crate::text::rope_buffer::RopeBuffer;
use crate::ui::unit::{Dp, UnitContext};
use crate::ui::widget::{
    CanvasItemInteractionHandlers, CanvasMouseButton, HitInteraction, InteractionHandlers, Point,
    Rect, ScrollRegion, ScrollbarAxis, ScrollbarHandle, Text, TextEditState, WidgetId, WidgetTree,
};
use cosmic_text::{Cursor as TextCursor, Edit, Editor, Metrics, Selection, Wrap};

use super::{
    canvas_mouse_button, cursor_icon, is_primary_shortcut_modifier, mouse_scroll_delta,
    text_cursor_index_at_point, BoundRuntimeHandler, CanvasPointerContext, ClickHandler,
    FocusedWidget, HoverMoveHandler, HoverTargetId, HoverTransitionHandler, HoveredWidget,
    PendingClick, ScrollbarDrag, SmoothScrollState, TextSelectionDrag,
};
const INPUT_CARET_WIDTH: f32 = 2.0;
use crate::platform::backend::event_loop::ActiveEventLoop;
use crate::rendering::renderer::RenderStatus;

#[derive(Clone)]
pub(super) struct TextInputRegionData<VM> {
    value: String,
    frame: Rect,
    padding: crate::ui::layout::Insets,
    text_style: Text,
    multiline: bool,
    on_change: Option<ValueCommand<VM, String>>,
}

fn text_edit_display_text(text: &str, state: &TextEditState) -> String {
    if let Some(composition) = state.composition.as_ref() {
        let start = composition.replace_range.0.min(text.len());
        let end = composition.replace_range.1.min(text.len());
        let mut display =
            String::with_capacity(text.len() + composition.text.len().saturating_sub(end - start));
        display.push_str(&text[..start]);
        display.push_str(&composition.text);
        display.push_str(&text[end..]);
        display
    } else {
        text.to_string()
    }
}

fn text_cursor_from_byte_index(text: &str, byte_index: usize) -> TextCursor {
    let mut line = 0usize;
    let mut line_start = 0usize;
    let target = byte_index.min(text.len());

    while let Some(relative) = text[line_start..].find('\n') {
        let line_end = line_start + relative;
        if target <= line_end {
            return TextCursor::new(line, target - line_start);
        }
        line += 1;
        line_start = line_end + 1;
    }

    TextCursor::new(line, target.saturating_sub(line_start))
}

fn apply_text_state_to_editor(editor: &mut Editor<'static>, state: &TextEditState, text: &str) {
    editor.set_cursor(text_cursor_from_byte_index(text, state.cursor.min(text.len())));
    if let Some((start, _)) = state.selection_range() {
        editor.set_selection(Selection::Normal(text_cursor_from_byte_index(
            text,
            start.min(text.len()),
        )));
    } else {
        editor.set_selection(Selection::None);
    }
}

fn buffer_text(editor: &Editor<'static>) -> String {
    editor.with_buffer(|buffer| {
        let mut text = String::new();
        for line in buffer.lines.iter() {
            text.push_str(line.text());
            text.push_str(line.ending().as_str());
        }
        text
    })
}

#[allow(clippy::too_many_arguments)]
fn refresh_session_buffer(
    font_manager: &FontManager,
    session: &mut super::TextInputBufferState,
    config: super::TextInputSessionConfig,
    preferred_font: Option<&str>,
    weight: crate::text::font::FontWeight,
    font_size: f32,
    line_height: f32,
    letter_spacing: f32,
    width: f32,
    height: f32,
    multiline: bool,
    text_state: &TextEditState,
    display_text: &str,
) {
    let needs_reconfigure =
        session.config.as_ref() != Some(&config) || buffer_text(&session.editor) != display_text;

    if !needs_reconfigure {
        apply_text_state_to_editor(&mut session.editor, text_state, display_text);
        return;
    }

    let previous_scroll = session.editor.with_buffer(|buffer| buffer.scroll());
    font_manager.with_font_system(|font_system| {
        session.editor.with_buffer_mut(|buffer| {
            font_manager.configure_buffer(
                font_system,
                buffer,
                display_text,
                crate::text::font::TextFontRequest {
                    preferred_font,
                    weight,
                },
                font_size,
                line_height,
                letter_spacing,
                Some(width),
                Some(height.max(line_height)),
                if multiline {
                    Wrap::WordOrGlyph
                } else {
                    Wrap::None
                },
            );
            buffer.set_scroll(previous_scroll);
            buffer.shape_until_scroll(font_system, false);
        });
    });
    apply_text_state_to_editor(&mut session.editor, text_state, display_text);
    session.config = Some(config);
}

impl<VM: 'static> BoundRuntimeHandler<VM> {
    fn text_input_session_config<'a>(
        &self,
        region: &'a TextInputRegionData<VM>,
    ) -> (
        super::TextInputSessionConfig,
        Option<String>,
        crate::text::font::FontWeight,
        f32,
        f32,
        f32,
        f32,
        f32,
    ) {
        let inner = region.frame.inset(region.padding);
        let (request, font_size, line_height, letter_spacing) =
            super::resolved_input_text_metrics(&self.theme, self.unit_context(), &region.text_style);
        let preferred_font = request.preferred_font.map(ToString::to_string);
        (
            super::TextInputSessionConfig {
                font_family: preferred_font.clone(),
                font_weight: request.weight,
                font_size_bits: font_size.to_bits(),
                line_height_bits: line_height.to_bits(),
                letter_spacing_bits: letter_spacing.to_bits(),
                width_bits: inner.width.get().max(0.0).to_bits(),
                height_bits: inner.height.get().max(0.0).to_bits(),
                multiline: region.multiline,
            },
            preferred_font,
            request.weight,
            font_size,
            line_height,
            letter_spacing,
            inner.width.get().max(0.0),
            inner.height.get().max(0.0),
        )
    }

    fn create_text_input_session(
        &self,
        region: &TextInputRegionData<VM>,
    ) -> super::TextInputBufferState {
        let (config, preferred_font, weight, font_size, line_height, letter_spacing, width, height) =
            self.text_input_session_config(region);
        let buffer = self.font_manager.with_font_system(|font_system| {
            let mut buffer = cosmic_text::Buffer::new(font_system, Metrics::new(font_size, line_height));
            self.font_manager.configure_buffer(
                font_system,
                &mut buffer,
                &region.value,
                crate::text::font::TextFontRequest {
                    preferred_font: preferred_font.as_deref(),
                    weight,
                },
                font_size,
                line_height,
                letter_spacing,
                Some(width),
                Some(height.max(line_height)),
                if region.multiline {
                    Wrap::WordOrGlyph
                } else {
                    Wrap::None
                },
            );
            buffer
        });
        let mut session = super::TextInputBufferState::new(Editor::new(buffer), region.value.clone());
        session.config = Some(config);
        session
    }

    fn cached_text_input_region_data(
        &self,
        widget_id: WidgetId,
    ) -> Option<TextInputRegionData<VM>> {
        let computed = &self.cached_scene.as_ref()?.computed;
        computed
            .hit_regions
            .iter()
            .chain(computed.overlay_hit_regions.iter())
            .find_map(|region| match &region.interaction {
                HitInteraction::TextInput {
                    id,
                    value,
                    frame,
                    padding,
                    text_style,
                    multiline,
                    on_change,
                    ..
                } if *id == widget_id => Some(TextInputRegionData {
                    value: value.clone(),
                    frame: *frame,
                    padding: *padding,
                    text_style: text_style.clone(),
                    multiline: *multiline,
                    on_change: on_change.clone(),
                }),
                _ => None,
            })
    }

    fn text_input_region_data(&mut self, widget_id: WidgetId) -> Option<TextInputRegionData<VM>> {
        let computed = self.computed_scene();
        computed
            .hit_regions
            .iter()
            .chain(computed.overlay_hit_regions.iter())
            .find_map(|region| match &region.interaction {
                HitInteraction::TextInput {
                    id,
                    value,
                    frame,
                    padding,
                    text_style,
                    multiline,
                    on_change,
                    ..
                } if *id == widget_id => Some(TextInputRegionData {
                    value: value.clone(),
                    frame: *frame,
                    padding: *padding,
                    text_style: text_style.clone(),
                    multiline: *multiline,
                    on_change: on_change.clone(),
                }),
                _ => None,
            })
    }

    pub(super) fn sync_text_input_buffer(
        &mut self,
        widget_id: WidgetId,
    ) -> Option<TextInputRegionData<VM>> {
        let region = self.text_input_region_data(widget_id)?;
        if !self.text_input_buffers.contains_key(&widget_id) {
            let session = self.create_text_input_session(&region);
            self.text_input_buffers.insert(widget_id, session);
        }

        let mut state = self
            .text_edit_state(widget_id)
            .cloned()
            .unwrap_or_else(|| TextEditState::caret_at(&region.value));
        let (config, preferred_font, weight, font_size, line_height, letter_spacing, width, height) =
            self.text_input_session_config(&region);
        {
            let session = self
                .text_input_buffers
                .get_mut(&widget_id)
                .expect("text input session should exist");
            if session.current_text == region.value {
                session.external_value = region.value.clone();
            } else if session.external_value != region.value {
                session.external_value = region.value.clone();
                session.current_text = region.value.clone();
                session.rope = ropey::Rope::from_str(&region.value);
                state = state.clamped_to(&region.value);
            }
            state = state.clamped_to(session.current_text());
            let display_text = text_edit_display_text(session.current_text(), &state);
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
                &state,
                &display_text,
            );
        }
        self.text_edit_states.insert(widget_id, state);
        Some(region)
    }

    fn edit_focused_text_input(
        &mut self,
        edit: impl FnOnce(&mut RopeBuffer, &TextEditState) -> Option<TextEditState>,
    ) -> bool {
        let Some(widget_id) = self.focused_text_input_id() else {
            return false;
        };
        let Some(region) = self.sync_text_input_buffer(widget_id) else {
            return false;
        };
        let state = self
            .text_edit_state(widget_id)
            .cloned()
            .unwrap_or_else(|| TextEditState::caret_at(&region.value));
        let (next_value, next_state) = {
            let session = self
                .text_input_buffers
                .get_mut(&widget_id)
                .expect("text input session should be initialized");
            let current_text = session.current_text.clone();
            let state = state.clamped_to(&current_text);
            let mut buffer = RopeBuffer::from_str(&current_text);
            let Some(next_state) = edit(&mut buffer, &state) else {
                return false;
            };
            (buffer.materialize_string(), next_state)
        };
        let changed = if region.value == next_value {
            false
        } else if let Some(command) = region.on_change.as_ref() {
            self.execute_value_command(command, next_value.clone());
            true
        } else {
            false
        };
        let (config, preferred_font, weight, font_size, line_height, letter_spacing, width, height) =
            self.text_input_session_config(&region);
        {
            let session = self
                .text_input_buffers
                .get_mut(&widget_id)
                .expect("text input session should exist after edit");
            if changed {
                session.current_text = next_value.clone();
                session.rope = ropey::Rope::from_str(&next_value);
            } else {
                session.current_text = region.value.clone();
                session.rope = ropey::Rope::from_str(&region.value);
                session.external_value = region.value.clone();
            }
            let canonical_value = if changed {
                session.current_text.as_str()
            } else {
                session.external_value.as_str()
            };
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
                &next_state,
                &display_text,
            );
            self.text_edit_states.insert(widget_id, next_state);
        }
        if let Some(state) = self.text_edit_states.get(&widget_id).cloned() {
            let canonical_value = self
                .text_input_buffers
                .get(&widget_id)
                .map(|session| session.current_text.clone())
                .unwrap_or_else(|| region.value.clone());
            self.ensure_text_input_caret_visible(
                widget_id,
                region.frame,
                region.padding,
                &region.text_style,
                region.multiline,
                &canonical_value,
                &state,
            );
        }
        self.reset_caret_blink();
        self.sync_ime_state();
        changed
    }

    fn insert_text_at_focused_input(&mut self, inserted: &str) -> bool {
        if inserted.is_empty() {
            return false;
        }
        self.edit_focused_text_input(|buffer: &mut RopeBuffer, state: &TextEditState| {
            let (start, end) = state
                .selection_range()
                .unwrap_or((state.cursor, state.cursor));
            buffer.replace_byte_range(start, end, inserted);
            let cursor = start + inserted.len();
            Some(TextEditState {
                cursor,
                anchor: cursor,
                composition: None,
                scroll_x: state.scroll_x,
                scroll_y: state.scroll_y,
                preferred_column_x: None,
            })
        })
    }

    fn delete_backward_at_focused_input(&mut self) -> bool {
        self.edit_focused_text_input(|buffer: &mut RopeBuffer, state: &TextEditState| {
            let (start, end) = if let Some(range) = state.selection_range() {
                range
            } else if state.cursor > 0 {
                (buffer.prev_char_boundary_byte(state.cursor), state.cursor)
            } else {
                return None;
            };
            buffer.replace_byte_range(start, end, "");
            Some(TextEditState {
                cursor: start,
                anchor: start,
                composition: None,
                scroll_x: state.scroll_x,
                scroll_y: state.scroll_y,
                preferred_column_x: None,
            })
        })
    }

    fn delete_forward_at_focused_input(&mut self) -> bool {
        self.edit_focused_text_input(|buffer: &mut RopeBuffer, state: &TextEditState| {
            let (start, end) = if let Some(range) = state.selection_range() {
                range
            } else if state.cursor < buffer.len_bytes() {
                (state.cursor, buffer.next_char_boundary_byte(state.cursor))
            } else {
                return None;
            };
            buffer.replace_byte_range(start, end, "");
            Some(TextEditState {
                cursor: start,
                anchor: start,
                composition: None,
                scroll_x: state.scroll_x,
                scroll_y: state.scroll_y,
                preferred_column_x: None,
            })
        })
    }

    fn move_focused_input_cursor(
        &mut self,
        next_index: impl FnOnce(&RopeBuffer, &TextEditState) -> usize,
        extend_selection: bool,
    ) -> bool {
        let Some(widget_id) = self.focused_text_input_id() else {
            return false;
        };
        let Some(region) = self.sync_text_input_buffer(widget_id) else {
            return false;
        };
        let state = self
            .text_edit_state(widget_id)
            .cloned()
            .unwrap_or_else(|| TextEditState::caret_at(&region.value));
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
            let buffer = RopeBuffer::from_str(session.current_text());
            next_index(&buffer, &clamped_state)
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
            self.ensure_text_input_caret_visible(
                widget_id,
                region.frame,
                region.padding,
                &region.text_style,
                region.multiline,
                &region.value,
                &state,
            );
        }
        self.reset_caret_blink();
        self.sync_ime_state();
        true
    }

    fn ensure_text_input_caret_visible(
        &mut self,
        widget_id: WidgetId,
        frame: Rect,
        padding: crate::ui::layout::Insets,
        text_style: &Text,
        multiline: bool,
        text: &str,
        state: &TextEditState,
    ) {
        let inner = frame.inset(padding);
        let (display_text, caret_index) = if let Some(composition) = state.composition.as_ref() {
            let start = composition.replace_range.0.min(text.len());
            let end = composition.replace_range.1.min(text.len());
            let mut display = String::with_capacity(
                text.len() + composition.text.len().saturating_sub(end - start),
            );
            display.push_str(&text[..start]);
            display.push_str(&composition.text);
            display.push_str(&text[end..]);
            let caret_offset = composition
                .cursor
                .map(|(_, end)| end.min(composition.text.len()))
                .unwrap_or(composition.text.len());
            (display, start + caret_offset)
        } else {
            (text.to_string(), state.cursor.min(text.len()))
        };
        let (layout, _, line_height) = super::input_text_layout(
            &self.font_manager,
            &self.theme,
            self.unit_context(),
            text_style,
            &display_text,
            multiline,
            inner.width.get(),
        );
        let caret = caret_index.min(display_text.len());
        let caret_x = layout.x_for_index(caret);
        let caret_y = layout.top_for_index(caret);
        let caret_h = layout.line_height_for_index(caret).max(line_height);
        let max_x = if multiline {
            (layout.width - inner.width.get()).max(0.0)
        } else {
            (layout.width + INPUT_CARET_WIDTH - inner.width.get()).max(0.0)
        };
        let max_y = (layout.height.max(line_height) - inner.height.get()).max(0.0);
        let mut next_scroll = Point::new(
            state.scroll_x.clamp(0.0, max_x),
            state.scroll_y.clamp(0.0, max_y),
        );

        if multiline {
            if caret_y < next_scroll.y.get() {
                next_scroll.y = Dp::new(caret_y);
            } else if caret_y + caret_h > next_scroll.y.get() + inner.height.get() {
                next_scroll.y = Dp::new((caret_y + caret_h - inner.height.get()).max(0.0));
            }
        } else {
            let caret_right = caret_x + INPUT_CARET_WIDTH;
            if caret_x < next_scroll.x.get() {
                next_scroll.x = Dp::new(caret_x);
            } else if caret_right > next_scroll.x.get() + inner.width.get() {
                next_scroll.x = Dp::new((caret_right - inner.width.get()).max(0.0));
            }
        }

        next_scroll.x = next_scroll.x.clamp(0.0, max_x);
        next_scroll.y = next_scroll.y.clamp(0.0, max_y);
        self.set_scroll_offset(widget_id, next_scroll);
    }

    fn move_focused_input_cursor_vertically(
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
        let state = self
            .text_edit_state(widget_id)
            .cloned()
            .unwrap_or_else(|| TextEditState::caret_at(&region.value))
            .clamped_to(&region.value);
        let (layout, _, _) = super::input_text_layout(
            &self.font_manager,
            &self.theme,
            self.unit_context(),
            &text_style,
            &region.value,
            true,
            inner.width.get(),
        );
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
            frame,
            padding,
            &text_style,
            true,
            &region.value,
            &next_state,
        );
        self.reset_caret_blink();
        self.sync_ime_state();
        true
    }

    fn select_all_focused_input(&mut self) -> bool {
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
            self.ensure_text_input_caret_visible(
                widget_id,
                region.frame,
                region.padding,
                &region.text_style,
                region.multiline,
                &region.value,
                &state,
            );
        }
        self.selected_text = Some(widget_id);
        self.reset_caret_blink();
        self.sync_ime_state();
        true
    }

    fn paste_into_focused_input(&mut self) -> bool {
        let Some(text) = self.clipboard.get_text() else {
            return false;
        };
        self.insert_text_at_focused_input(&text)
    }

    fn cut_selected_text_from_input(&mut self) -> bool {
        let Some(selected) = self.selected_text_for_copy() else {
            return false;
        };
        self.clipboard.set_text(selected);
        self.delete_backward_at_focused_input()
    }

    fn update_focused_input_composition(
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
        let changed = self.update_text_edit_state(widget_id, &region.value, |state| {
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
                self.ensure_text_input_caret_visible(
                    widget_id,
                    region.frame,
                    region.padding,
                    &region.text_style,
                    region.multiline,
                    &region.value,
                    &state,
                );
            }
            self.reset_caret_blink();
            self.sync_ime_state();
        }
        changed
    }

    fn clear_focused_input_composition(&mut self) -> bool {
        let Some(widget_id) = self.focused_text_input_id() else {
            return false;
        };
        let Some(region) = self.sync_text_input_buffer(widget_id) else {
            return false;
        };
        let changed = self.update_text_edit_state(widget_id, &region.value, |state| {
            state.composition = None;
        });
        if changed {
            if let Some(state) = self.text_edit_states.get(&widget_id).cloned() {
                self.ensure_text_input_caret_visible(
                    widget_id,
                    region.frame,
                    region.padding,
                    &region.text_style,
                    region.multiline,
                    &region.value,
                    &state,
                );
            }
            self.sync_ime_state();
        }
        changed
    }

    pub(super) fn handle_ime_event(&mut self, event: &Ime) -> bool {
        match event {
            Ime::Enabled => {
                self.sync_ime_state();
                false
            }
            Ime::Preedit(text, cursor) => {
                self.update_focused_input_composition(text.clone(), *cursor)
            }
            Ime::Commit(text) => {
                let _ = self.clear_focused_input_composition();
                self.insert_text_at_focused_input(text)
            }
            Ime::DeleteSurrounding {
                before_bytes,
                after_bytes,
            } => self.edit_focused_text_input(|buffer: &mut RopeBuffer, state: &TextEditState| {
                let start = buffer.clamp_byte_boundary(state.cursor.saturating_sub(*before_bytes));
                let end = buffer.clamp_byte_boundary(
                    state
                        .cursor
                        .saturating_add(*after_bytes)
                        .min(buffer.len_bytes()),
                );
                if start >= end {
                    return None;
                }
                buffer.replace_byte_range(start, end, "");
                Some(TextEditState {
                    cursor: start,
                    anchor: start,
                    composition: None,
                    scroll_x: state.scroll_x,
                    scroll_y: state.scroll_y,
                    preferred_column_x: None,
                })
            }),
            Ime::Disabled => self.clear_focused_input_composition(),
        }
    }

    pub(super) fn flush_pending_click_if_due(&mut self, now: Instant) {
        let should_flush = self
            .pending_click
            .as_ref()
            .map(|pending| pending.deadline <= now)
            .unwrap_or(false);
        if !should_flush {
            return;
        }

        if let Some(pending) = self.pending_click.take() {
            if let Some(command) = pending.command {
                self.execute_click_handler(&command, self.cursor_position);
            }
        }
    }

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

    pub(super) fn selected_text_for_copy(&mut self) -> Option<String> {
        let Some(widget_id) = self.selected_text else {
            return None;
        };
        if let Some(region) = self.sync_text_input_buffer(widget_id) {
            let (start, end) = self
                .text_edit_state(widget_id)
                .cloned()
                .unwrap_or_else(|| crate::ui::widget::TextEditState::caret_at(&region.value))
                .clamped_to(&region.value)
                .selection_range()?;
            return self
                .text_input_buffers
                .get(&widget_id)
                .map(|state| RopeBuffer::from_str(state.current_text()).slice_byte_range_to_string(start, end));
        }

        let text = self.selected_text_content(widget_id)?;
        let (start, end) = self
            .text_edit_state(widget_id)
            .cloned()
            .unwrap_or_else(|| crate::ui::widget::TextEditState::caret_at(&text))
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
            self.invalidate_scene();
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
        });
        self.update_text_edit_state(widget_id, &text, |state| {
            state.cursor = cursor;
            state.anchor = cursor;
            state.composition = None;
        });
        if let Some(state) = self.text_edit_states.get(&widget_id).cloned() {
            self.ensure_text_input_caret_visible(
                widget_id,
                frame,
                padding,
                &text_style,
                multiline,
                &text,
                &state,
            );
        }
        self.reset_caret_blink();
    }

    pub(super) fn handle_text_selection_drag(&mut self) -> bool {
        let Some(drag) = self.active_text_selection.clone() else {
            return false;
        };
        let Some(point) = self.cursor_position else {
            return false;
        };

        let cursor = text_cursor_index_at_point(
            &self.font_manager,
            &self.theme,
            self.unit_context(),
            drag.frame,
            drag.padding,
            &drag.text_style,
            &drag.text,
            drag.multiline,
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
            self.invalidate_scene();
            return true;
        }
        false
    }

    pub(super) fn ime_cursor_request_data(caret_rect: Rect, units: UnitContext) -> ImeRequestData {
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

    pub(super) fn focusable_widgets_in_tab_order(&mut self) -> Vec<FocusedWidget<VM>> {
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

    pub(super) fn handle_keyboard_input(&mut self, event: &KeyEvent) -> bool {
        if event.state != ElementState::Pressed {
            return false;
        }

        if event.repeat && !self.allows_repeated_keyboard_input(event) {
            return false;
        }

        match event.physical_key {
            PhysicalKey::Code(KeyCode::Tab)
                if !is_primary_shortcut_modifier(self.modifiers) && !self.modifiers.alt_key() =>
            {
                self.advance_focus(self.modifiers.shift_key())
            }
            PhysicalKey::Code(KeyCode::Backspace) => self.delete_backward_at_focused_input(),
            PhysicalKey::Code(KeyCode::Delete) => self.delete_forward_at_focused_input(),
            PhysicalKey::Code(KeyCode::ArrowLeft) => {
                let extend = self.modifiers.shift_key();
                self.move_focused_input_cursor(
                    |buffer: &RopeBuffer, state: &TextEditState| {
                        if let Some((start, _)) = state.selection_range() {
                            if extend {
                                buffer.prev_char_boundary_byte(state.cursor)
                            } else {
                                start
                            }
                        } else {
                            buffer.prev_char_boundary_byte(state.cursor)
                        }
                    },
                    extend,
                )
            }
            PhysicalKey::Code(KeyCode::ArrowRight) => {
                let extend = self.modifiers.shift_key();
                self.move_focused_input_cursor(
                    |buffer: &RopeBuffer, state: &TextEditState| {
                        if let Some((_, end)) = state.selection_range() {
                            if extend {
                                buffer.next_char_boundary_byte(state.cursor)
                            } else {
                                end
                            }
                        } else {
                            buffer.next_char_boundary_byte(state.cursor)
                        }
                    },
                    extend,
                )
            }
            PhysicalKey::Code(KeyCode::ArrowUp) => {
                self.move_focused_input_cursor_vertically(-1, self.modifiers.shift_key())
            }
            PhysicalKey::Code(KeyCode::ArrowDown) => {
                self.move_focused_input_cursor_vertically(1, self.modifiers.shift_key())
            }
            PhysicalKey::Code(KeyCode::Home) => {
                self.move_focused_input_cursor(|_, _| 0, self.modifiers.shift_key())
            }
            PhysicalKey::Code(KeyCode::End) => self.move_focused_input_cursor(
                |buffer: &RopeBuffer, _state: &TextEditState| buffer.len_bytes(),
                self.modifiers.shift_key(),
            ),
            PhysicalKey::Code(KeyCode::KeyA) if is_primary_shortcut_modifier(self.modifiers) => {
                self.select_all_focused_input()
            }
            PhysicalKey::Code(KeyCode::KeyC) if is_primary_shortcut_modifier(self.modifiers) => {
                self.copy_selected_text_to_clipboard()
            }
            PhysicalKey::Code(KeyCode::KeyV) if is_primary_shortcut_modifier(self.modifiers) => {
                self.paste_into_focused_input()
            }
            PhysicalKey::Code(KeyCode::KeyX) if is_primary_shortcut_modifier(self.modifiers) => {
                self.cut_selected_text_from_input()
            }
            _ if !is_primary_shortcut_modifier(self.modifiers)
                && !self.modifiers.alt_key()
                && self.focused_text_input_id().is_some() =>
            {
                match &event.logical_key {
                    Key::Named(NamedKey::Enter) => {
                        let Some(id) = self.focused_text_input_id() else {
                            return false;
                        };
                        let Some(region) = self.sync_text_input_buffer(id) else {
                            return false;
                        };
                        region.multiline && self.insert_text_at_focused_input("\n")
                    }
                    _ => event
                        .text
                        .as_ref()
                        .map(|text| text.as_str())
                        .filter(|text| !text.is_empty() && *text != "\r" && *text != "\u{8}")
                        .map(|text| self.insert_text_at_focused_input(text))
                        .unwrap_or(false),
                }
            }
            _ => false,
        }
    }

    fn allows_repeated_keyboard_input(&mut self, event: &KeyEvent) -> bool {
        match event.physical_key {
            PhysicalKey::Code(
                KeyCode::Backspace
                | KeyCode::Delete
                | KeyCode::ArrowLeft
                | KeyCode::ArrowRight
                | KeyCode::ArrowUp
                | KeyCode::ArrowDown
                | KeyCode::Home
                | KeyCode::End,
            ) => self.focused_text_input_id().is_some(),
            _ if !is_primary_shortcut_modifier(self.modifiers)
                && !self.modifiers.alt_key()
                && self.focused_text_input_id().is_some() =>
            {
                matches!(&event.logical_key, Key::Named(NamedKey::Enter))
                    || event
                        .text
                        .as_ref()
                        .map(|text| {
                            let text = text.as_str();
                            !text.is_empty() && text != "\r" && text != "\u{8}"
                        })
                        .unwrap_or(false)
            }
            _ => false,
        }
    }

    pub(super) fn resolved_select_open_state(&mut self, widget_id: WidgetId) -> Option<bool> {
        if let Some(open) = self.select_open_states.get(&widget_id).copied() {
            return Some(open);
        }
        let computed = self.computed_scene();
        computed
            .hit_regions
            .iter()
            .chain(computed.overlay_hit_regions.iter())
            .find_map(|region| match &region.interaction {
                HitInteraction::SelectTrigger { id, is_open, .. } if *id == widget_id => {
                    Some(*is_open)
                }
                _ => None,
            })
    }

    pub(super) fn set_select_open_state(
        &mut self,
        widget_id: WidgetId,
        open: bool,
        on_open_change: Option<&ValueCommand<VM, bool>>,
    ) -> bool {
        let previous = self
            .resolved_select_open_state(widget_id)
            .or_else(|| self.select_open_states.get(&widget_id).copied())
            .unwrap_or(false);
        if previous == open {
            return false;
        }

        self.select_open_states.insert(widget_id, open);
        if let Some(command) = on_open_change {
            self.execute_value_command(command, open);
        }
        self.invalidate_scene();
        true
    }

    pub(super) fn close_all_open_selects_except(&mut self, keep_open: Option<WidgetId>) -> bool {
        let select_triggers: Vec<_> = self
            .computed_scene()
            .hit_regions
            .iter()
            .filter_map(|region| match &region.interaction {
                HitInteraction::SelectTrigger {
                    id, on_open_change, ..
                } => Some((*id, on_open_change.clone())),
                _ => None,
            })
            .collect();

        let mut changed = false;
        for (id, on_open_change) in select_triggers {
            if Some(id) == keep_open || !self.resolved_select_open_state(id).unwrap_or(false) {
                continue;
            }
            changed |= self.set_select_open_state(id, false, on_open_change.as_ref());
        }
        changed
    }

    pub(super) fn hit_path(&mut self, _viewport: Rect) -> Vec<HitInteraction<VM>> {
        let Some(point) = self.cursor_position else {
            return Vec::new();
        };
        WidgetTree::hit_path_from_computed(self.computed_scene(), point)
    }

    pub(super) fn hover_path(&mut self, viewport: Rect) -> Vec<HoveredWidget<VM>> {
        self.hit_path(viewport)
            .into_iter()
            .map(|interaction| match interaction {
                HitInteraction::Disabled { id } => HoveredWidget {
                    target_id: HoverTargetId::Widget(id),
                    cursor_style: Some(crate::ui::widget::CursorStyle::NotAllowed),
                    on_mouse_enter: None,
                    on_mouse_leave: None,
                    on_mouse_move: None,
                },
                HitInteraction::Widget {
                    id, interactions, ..
                } => HoveredWidget {
                    target_id: HoverTargetId::Widget(id),
                    cursor_style: interactions.cursor_style.map(|c| c.resolve()),
                    on_mouse_enter: interactions
                        .on_mouse_enter
                        .map(HoverMoveOrTransition::into_transition),
                    on_mouse_leave: interactions
                        .on_mouse_leave
                        .map(HoverMoveOrTransition::into_transition),
                    on_mouse_move: interactions.on_mouse_move.map(HoverMoveHandler::Point),
                },
                HitInteraction::SelectableText {
                    id, interactions, ..
                } => HoveredWidget {
                    target_id: HoverTargetId::Widget(id),
                    cursor_style: interactions
                        .cursor_style
                        .map(|c| c.resolve())
                        .or(Some(crate::ui::widget::CursorStyle::Text)),
                    on_mouse_enter: interactions
                        .on_mouse_enter
                        .map(HoverMoveOrTransition::into_transition),
                    on_mouse_leave: interactions
                        .on_mouse_leave
                        .map(HoverMoveOrTransition::into_transition),
                    on_mouse_move: interactions.on_mouse_move.map(HoverMoveHandler::Point),
                },
                HitInteraction::Switch {
                    id, interactions, ..
                }
                | HitInteraction::Checkbox {
                    id, interactions, ..
                }
                | HitInteraction::Radio {
                    id, interactions, ..
                }
                | HitInteraction::TextInput {
                    id, interactions, ..
                }
                | HitInteraction::SelectTrigger {
                    id, interactions, ..
                } => HoveredWidget {
                    target_id: HoverTargetId::Widget(id),
                    cursor_style: interactions.cursor_style.map(|c| c.resolve()),
                    on_mouse_enter: interactions
                        .on_mouse_enter
                        .map(HoverMoveOrTransition::into_transition),
                    on_mouse_leave: interactions
                        .on_mouse_leave
                        .map(HoverMoveOrTransition::into_transition),
                    on_mouse_move: interactions.on_mouse_move.map(HoverMoveHandler::Point),
                },
                HitInteraction::SelectOption {
                    id,
                    option_index,
                    interactions,
                    ..
                } => HoveredWidget {
                    target_id: HoverTargetId::SelectOption {
                        widget_id: id,
                        option_index,
                    },
                    cursor_style: interactions.cursor_style.map(|c| c.resolve()),
                    on_mouse_enter: interactions
                        .on_mouse_enter
                        .map(HoverMoveOrTransition::into_transition),
                    on_mouse_leave: interactions
                        .on_mouse_leave
                        .map(HoverMoveOrTransition::into_transition),
                    on_mouse_move: interactions.on_mouse_move.map(HoverMoveHandler::Point),
                },
                HitInteraction::CanvasItem {
                    id,
                    item_id,
                    item_interactions,
                    cursor_style,
                    canvas_origin,
                    item_origin,
                } => {
                    let context = CanvasPointerContext {
                        item_id,
                        canvas_origin,
                        item_origin,
                    };
                    HoveredWidget {
                        target_id: HoverTargetId::CanvasItem {
                            widget_id: id,
                            item_id,
                        },
                        cursor_style,
                        on_mouse_enter: item_interactions
                            .on_mouse_enter
                            .map(|command| HoverTransitionHandler::Canvas(command, context)),
                        on_mouse_leave: item_interactions
                            .on_mouse_leave
                            .map(|command| HoverTransitionHandler::Canvas(command, context)),
                        on_mouse_move: item_interactions
                            .on_mouse_move
                            .map(|command| HoverMoveHandler::Canvas(command, context)),
                    }
                }
            })
            .collect()
    }

    pub(super) fn handle_hover(&mut self, viewport: Rect) -> bool {
        let revision_before = self.invalidation.revision();
        let cursor_position = self.cursor_position;
        let next_hovered = self.hover_path(viewport);
        let hover_path_changed = self.hovered_widgets.len() != next_hovered.len()
            || self
                .hovered_widgets
                .iter()
                .zip(next_hovered.iter())
                .any(|(previous, next)| previous.target_id != next.target_id);
        let mut prefix_len = 0usize;
        while prefix_len < self.hovered_widgets.len()
            && prefix_len < next_hovered.len()
            && self.hovered_widgets[prefix_len].target_id == next_hovered[prefix_len].target_id
        {
            prefix_len += 1;
        }

        let previous_hovered = std::mem::take(&mut self.hovered_widgets);
        for previous in previous_hovered[prefix_len..].iter().rev() {
            if let Some(command) = previous.on_mouse_leave.as_ref() {
                self.execute_hover_transition_handler(command, cursor_position);
            }
        }

        for hovered in next_hovered[prefix_len..].iter().rev() {
            if let Some(command) = hovered.on_mouse_enter.as_ref() {
                self.execute_hover_transition_handler(command, cursor_position);
            }
        }

        if let Some(position) = cursor_position {
            for hovered in next_hovered.iter().rev() {
                if let Some(command) = hovered.on_mouse_move.as_ref() {
                    self.execute_hover_move_handler(command, position);
                }
            }
        }

        self.hovered_widgets = next_hovered;
        if hover_path_changed {
            self.hover_epoch = self.hover_epoch.wrapping_add(1);
        }
        let scrollbar_changed = self.sync_scrollbar_hover();
        let cursor_changed = self.update_cursor_icon();
        hover_path_changed
            || scrollbar_changed
            || cursor_changed
            || self.invalidation.revision() != revision_before
    }

    pub(super) fn handle_mouse_wheel(&mut self, delta: MouseScrollDelta) -> bool {
        let Some(cursor_position) = self.cursor_position else {
            return false;
        };

        let mut scroll_delta = mouse_scroll_delta(delta);
        if scroll_delta.x.abs() <= f32::EPSILON && self.modifiers.shift_key() {
            scroll_delta.x = scroll_delta.y;
            scroll_delta.y = Dp::ZERO;
        }
        if scroll_delta.x.abs() <= f32::EPSILON && scroll_delta.y.abs() <= f32::EPSILON {
            return false;
        }

        for interaction in self.hit_path(self.viewport_rect()).into_iter().rev() {
            if let HitInteraction::CanvasItem {
                item_id,
                ref item_interactions,
                canvas_origin,
                item_origin,
                ..
            } = interaction
            {
                if let Some(command) = &item_interactions.on_wheel {
                    self.execute_canvas_wheel_command(
                        command,
                        CanvasPointerContext {
                            item_id,
                            canvas_origin,
                            item_origin,
                        },
                        cursor_position,
                        scroll_delta,
                    );
                    return true;
                }
            }

            if let HitInteraction::TextInput {
                id,
                value,
                frame,
                padding,
                text_style,
                multiline: true,
                ..
            } = interaction
            {
                if self.scroll_multiline_text_input(
                    id,
                    &value,
                    frame,
                    padding,
                    &text_style,
                    scroll_delta,
                ) {
                    return true;
                }
            }
        }

        let scroll_regions = self.scroll_regions();
        for region in scroll_regions.iter().rev().copied() {
            if region.visible_frame.is_empty() || !region.visible_frame.contains(cursor_position) {
                continue;
            }

            let max_offset = region.max_offset();
            let mut next_offset = region.scroll_offset;
            if region.can_scroll_x() {
                next_offset.x = (next_offset.x - scroll_delta.x).clamp(0.0, max_offset.x);
            }
            if region.can_scroll_y() {
                next_offset.y = (next_offset.y - scroll_delta.y).clamp(0.0, max_offset.y);
            }

            if (next_offset.x - region.scroll_offset.x).abs() > 0.01
                || (next_offset.y - region.scroll_offset.y).abs() > 0.01
            {
                self.set_smooth_scroll_target(region, next_offset);
                return true;
            }
        }

        false
    }

    fn scroll_multiline_text_input(
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

        let (layout, _, line_height) = super::input_text_layout(
            &self.font_manager,
            &self.theme,
            self.unit_context(),
            text_style,
            value,
            true,
            inner.width.get(),
        );
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

    pub(super) fn sync_scrollbar_hover(&mut self) -> bool {
        let next_hovered = if let Some(drag) = self.active_scrollbar_drag {
            Some(drag.handle)
        } else {
            self.scrollbar_thumb_hit()
        };

        if self.hovered_scrollbar != next_hovered {
            self.hovered_scrollbar = next_hovered;
            self.invalidate_scene();
            return true;
        }

        false
    }

    pub(super) fn scrollbar_thumb_hit(&mut self) -> Option<ScrollbarHandle> {
        let cursor_position = self.cursor_position?;
        let scroll_regions = self.scroll_regions();
        scroll_regions.iter().rev().find_map(|region| {
            if region.visible_frame.is_empty() || !region.visible_frame.contains(cursor_position) {
                return None;
            }
            if region
                .vertical_thumb
                .map(|thumb: Rect| thumb.contains(cursor_position))
                .unwrap_or(false)
            {
                return Some(ScrollbarHandle {
                    id: region.id,
                    axis: ScrollbarAxis::Vertical,
                });
            }
            if region
                .horizontal_thumb
                .map(|thumb: Rect| thumb.contains(cursor_position))
                .unwrap_or(false)
            {
                return Some(ScrollbarHandle {
                    id: region.id,
                    axis: ScrollbarAxis::Horizontal,
                });
            }
            None
        })
    }

    pub(super) fn begin_scrollbar_drag(&mut self) -> bool {
        let Some(handle) = self.scrollbar_thumb_hit() else {
            return false;
        };
        let Some(cursor_position) = self.cursor_position else {
            return false;
        };
        let scroll_regions = self.scroll_regions();
        let Some(region) = scroll_regions
            .iter()
            .copied()
            .find(|region| region.id == handle.id)
        else {
            return false;
        };

        let (track, thumb, max_offset) = match handle.axis {
            ScrollbarAxis::Horizontal => (
                region.horizontal_track,
                region.horizontal_thumb,
                region.max_offset().x,
            ),
            ScrollbarAxis::Vertical => (
                region.vertical_track,
                region.vertical_thumb,
                region.max_offset().y,
            ),
        };
        let (Some(track), Some(thumb)) = (track, thumb) else {
            return false;
        };

        self.smooth_scroll_states.remove(&handle.id);
        self.active_scrollbar_drag = Some(ScrollbarDrag {
            handle,
            start_cursor: cursor_position,
            start_scroll_offset: region.scroll_offset,
            track,
            thumb,
            max_offset,
        });
        self.hovered_scrollbar = Some(handle);
        self.invalidate_scene();
        true
    }

    pub(super) fn handle_scrollbar_drag(&mut self) -> bool {
        let Some(drag) = self.active_scrollbar_drag else {
            return false;
        };
        let Some(cursor_position) = self.cursor_position else {
            return false;
        };

        let (travel, delta) = match drag.handle.axis {
            ScrollbarAxis::Horizontal => (
                (drag.track.width - drag.thumb.width).max(0.0),
                cursor_position.x - drag.start_cursor.x,
            ),
            ScrollbarAxis::Vertical => (
                (drag.track.height - drag.thumb.height).max(0.0),
                cursor_position.y - drag.start_cursor.y,
            ),
        };

        let mut next_offset = drag.start_scroll_offset;
        let axis_offset = if travel <= 0.0 || drag.max_offset <= 0.0 {
            Dp::ZERO
        } else {
            (delta / travel) * drag.max_offset
        };

        match drag.handle.axis {
            ScrollbarAxis::Horizontal => {
                next_offset.x =
                    (drag.start_scroll_offset.x + axis_offset).clamp(0.0, drag.max_offset)
            }
            ScrollbarAxis::Vertical => {
                next_offset.y =
                    (drag.start_scroll_offset.y + axis_offset).clamp(0.0, drag.max_offset)
            }
        }

        let previous = self
            .scroll_states
            .get(&drag.handle.id)
            .copied()
            .unwrap_or_else(|| {
                if drag.start_scroll_offset.x.abs() <= 0.01
                    && drag.start_scroll_offset.y.abs() <= 0.01
                {
                    Point::ZERO
                } else {
                    drag.start_scroll_offset
                }
            });
        if (previous.x - next_offset.x).abs() > 0.01 || (previous.y - next_offset.y).abs() > 0.01 {
            self.set_scroll_offset(drag.handle.id, next_offset);
            return true;
        }

        false
    }

    pub(super) fn handle_canvas_drag(&mut self) -> bool {
        let Some(cursor_position) = self.cursor_position else {
            return false;
        };

        let Some(mut drag) = self.active_canvas_drag.take() else {
            return false;
        };

        if !drag.started {
            if let Some(command) = drag.on_drag_start.clone() {
                self.execute_canvas_drag_command(
                    &command,
                    drag.context,
                    drag.start_position,
                    cursor_position,
                    drag.button,
                );
            }
            drag.started = true;
        }

        if let Some(command) = drag.on_drag.clone() {
            self.execute_canvas_drag_command(
                &command,
                drag.context,
                drag.start_position,
                cursor_position,
                drag.button,
            );
        }

        self.active_canvas_drag = Some(drag);
        true
    }

    pub(super) fn end_scrollbar_drag(&mut self) -> bool {
        if self.active_scrollbar_drag.take().is_none() {
            return false;
        }
        self.sync_scrollbar_hover();
        self.invalidate_scene();
        true
    }

    pub(super) fn end_canvas_drag(&mut self) -> bool {
        let Some(drag) = self.active_canvas_drag.take() else {
            return false;
        };
        let Some(cursor_position) = self.cursor_position else {
            return false;
        };
        if let Some(command) = drag.on_drag_end {
            self.execute_canvas_drag_command(
                &command,
                drag.context,
                drag.start_position,
                cursor_position,
                drag.button,
            );
        }
        true
    }

    pub(super) fn update_cursor_icon(&mut self) -> bool {
        let next_icon = if self.active_scrollbar_drag.is_some() || self.hovered_scrollbar.is_some()
        {
            CursorIcon::Pointer
        } else if self.active_text_selection.is_some() {
            CursorIcon::Text
        } else if let Some(cursor_style) = self
            .hovered_widgets
            .iter()
            .rev()
            .find_map(|hovered| hovered.cursor_style)
        {
            cursor_icon(cursor_style)
        } else {
            CursorIcon::Default
        };

        if self.cursor_icon == Some(next_icon) {
            return false;
        }

        self.cursor_icon = Some(next_icon);
        if let Some(window) = self.window.as_ref() {
            window.set_cursor(Cursor::Icon(next_icon));
        }
        true
    }

    pub(super) fn set_scroll_offset(&mut self, widget_id: WidgetId, offset: Point) {
        let offset = Point::new(offset.x.max(Dp::ZERO), offset.y.max(Dp::ZERO));
        let previous = self
            .scroll_states
            .get(&widget_id)
            .copied()
            .unwrap_or(Point::ZERO);
        let mut changed =
            (previous.x - offset.x).abs() > 0.01 || (previous.y - offset.y).abs() > 0.01;

        if let Some(state) = self.text_edit_states.get_mut(&widget_id) {
            if (state.scroll_x - offset.x).abs() > 0.01 || (state.scroll_y - offset.y).abs() > 0.01
            {
                state.scroll_x = offset.x;
                state.scroll_y = offset.y;
                changed = true;
            }
        }

        if !changed {
            return;
        }

        if offset.x.abs() <= 0.01 && offset.y.abs() <= 0.01 {
            self.scroll_states.remove(&widget_id);
        } else {
            self.scroll_states.insert(widget_id, offset);
        }
        self.scroll_epoch = self.scroll_epoch.wrapping_add(1);
        self.invalidate_scene();
    }

    pub(super) fn set_smooth_scroll_target(&mut self, region: ScrollRegion, target: Point) {
        self.smooth_scroll_states
            .insert(region.id, SmoothScrollState { target });
        self.invalidation.mark_dirty();
        if let Some(window) = self.window.as_ref() {
            window.request_redraw();
        }
    }

    pub(super) fn advance_smooth_scroll(&mut self) -> bool {
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
            if dx.abs().get() <= super::SMOOTH_SCROLL_EPSILON
                && dy.abs().get() <= super::SMOOTH_SCROLL_EPSILON
            {
                self.set_scroll_offset(widget_id, state.target);
                finished.push(widget_id);
                changed = true;
                continue;
            }

            let next = Point::new(
                current.x + dx * super::SMOOTH_SCROLL_LERP,
                current.y + dy * super::SMOOTH_SCROLL_LERP,
            );
            self.set_scroll_offset(widget_id, next);
            changed = true;
        }

        for widget_id in finished {
            self.smooth_scroll_states.remove(&widget_id);
        }

        changed
    }

    fn reset_single_line_input_focus_state(&mut self, widget_id: WidgetId, text: &str) {
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

    pub(super) fn update_focus(
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

        if current_id == next_id && self.focus_visible == focus_visible {
            return;
        }

        if let Some(previous) = self.focused_widget.take() {
            if let Some((widget_id, region)) = previous_single_line_input.as_ref() {
                if *widget_id == previous.widget_id {
                    self.reset_single_line_input_focus_state(*widget_id, &region.value);
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

        self.invalidate_scene();
    }

    pub(super) fn dispatch_widget_click(
        &mut self,
        target_id: HoverTargetId,
        interactions: InteractionHandlers<VM>,
        now: Instant,
    ) {
        let is_double_click = self
            .pending_click
            .as_ref()
            .map(|pending| pending.target_id == target_id && pending.deadline > now)
            .unwrap_or(false);

        if is_double_click {
            self.pending_click = None;
            if let Some(command) = interactions.on_double_click.or(interactions.on_click) {
                self.execute_command(&command);
            }
            return;
        }

        if interactions.on_double_click.is_some() {
            self.pending_click = Some(PendingClick {
                target_id,
                deadline: now + super::DOUBLE_CLICK_THRESHOLD,
                command: interactions.on_click.map(ClickHandler::Command),
            });
        } else if let Some(command) = interactions.on_click {
            self.execute_command(&command);
        } else {
            self.pending_click = None;
        }
    }

    pub(super) fn dispatch_canvas_click(
        &mut self,
        target_id: HoverTargetId,
        item_interactions: &CanvasItemInteractionHandlers<VM>,
        context: CanvasPointerContext,
        now: Instant,
        button: CanvasMouseButton,
    ) -> bool {
        let is_double_click = self
            .pending_click
            .as_ref()
            .map(|pending| pending.target_id == target_id && pending.deadline > now)
            .unwrap_or(false);

        if is_double_click {
            self.pending_click = None;
            if let Some(command) = item_interactions
                .on_double_click
                .clone()
                .or(item_interactions.on_click.clone())
            {
                self.execute_click_handler(
                    &ClickHandler::Canvas(command, context, Some(button)),
                    self.cursor_position,
                );
                return true;
            }
            return false;
        }

        if item_interactions.on_double_click.is_some() {
            self.pending_click = Some(PendingClick {
                target_id,
                deadline: now + super::DOUBLE_CLICK_THRESHOLD,
                command: item_interactions
                    .on_click
                    .clone()
                    .map(|command| ClickHandler::Canvas(command, context, Some(button))),
            });
            return true;
        }

        if let Some(command) = item_interactions.on_click.clone() {
            self.execute_click_handler(
                &ClickHandler::Canvas(command, context, Some(button)),
                self.cursor_position,
            );
            return true;
        }

        false
    }

    pub(super) fn handle_mouse_press(
        &mut self,
        viewport: Rect,
        now: Instant,
        button: CanvasMouseButton,
    ) {
        self.flush_pending_click_if_due(now);

        let hit_path = self.hit_path(viewport);
        let Some(hit) = hit_path.last().cloned() else {
            self.close_all_open_selects_except(None);
            self.clear_selected_text();
            self.update_focus(None, None, false);
            self.pending_click = None;
            self.pressed_widget = None;
            return;
        };

        if matches!(hit, HitInteraction::Disabled { .. }) {
            self.close_all_open_selects_except(None);
            self.clear_selected_text();
            self.update_focus(None, None, false);
            self.pending_click = None;
            self.pressed_widget = None;
            return;
        }

        if matches!(hit, HitInteraction::CanvasItem { .. }) {
            self.clear_selected_text();
            self.update_focus(None, None, false);
            self.pressed_widget = None;

            for interaction in hit_path.into_iter().rev() {
                match interaction {
                    HitInteraction::CanvasItem {
                        id,
                        item_id,
                        item_interactions,
                        canvas_origin,
                        item_origin,
                        ..
                    } => {
                        let context = CanvasPointerContext {
                            item_id,
                            canvas_origin,
                            item_origin,
                        };
                        if let (Some(position), Some(command)) = (
                            self.cursor_position,
                            item_interactions.on_mouse_down.clone(),
                        ) {
                            self.execute_canvas_mouse_command(
                                &command,
                                context,
                                position,
                                Some(button),
                            );
                        }
                        if self.active_canvas_drag.is_none()
                            && (item_interactions.on_drag_start.is_some()
                                || item_interactions.on_drag.is_some()
                                || item_interactions.on_drag_end.is_some())
                        {
                            if let Some(position) = self.cursor_position {
                                self.active_canvas_drag = Some(super::ActiveCanvasDrag {
                                    button,
                                    context,
                                    start_position: position,
                                    started: false,
                                    on_mouse_up: item_interactions.on_mouse_up.clone(),
                                    on_drag_start: item_interactions.on_drag_start.clone(),
                                    on_drag: item_interactions.on_drag.clone(),
                                    on_drag_end: item_interactions.on_drag_end.clone(),
                                });
                            }
                        }
                        if self.dispatch_canvas_click(
                            HoverTargetId::CanvasItem {
                                widget_id: id,
                                item_id,
                            },
                            &item_interactions,
                            context,
                            now,
                            button,
                        ) {
                            return;
                        }
                    }
                    HitInteraction::Widget {
                        id, interactions, ..
                    } => {
                        self.dispatch_widget_click(HoverTargetId::Widget(id), interactions, now);
                        return;
                    }
                    _ => {}
                }
            }
            self.pending_click = None;
            return;
        }

        let pointer_position = self.cursor_position;
        let clicked_select = matches!(
            &hit,
            HitInteraction::SelectTrigger { .. } | HitInteraction::SelectOption { .. }
        );
        let text_input_hit = matches!(&hit, HitInteraction::TextInput { .. });
        let (
            widget_id,
            interactions,
            focus_target,
            focus_command,
            click_handler,
            select_toggle,
            selectable_text,
        ) = match hit {
            HitInteraction::Widget {
                id,
                interactions,
                focusable,
            } => (
                id,
                interactions.clone(),
                focusable.then_some(id),
                focusable.then_some(interactions.on_focus.clone()).flatten(),
                interactions.on_click.clone().map(ClickHandler::Command),
                None,
                None,
            ),
            HitInteraction::SelectableText {
                id,
                frame,
                padding,
                interactions,
                text_style,
                text,
            } => {
                let cursor = pointer_position.map(|point| {
                    text_cursor_index_at_point(
                        &self.font_manager,
                        &self.theme,
                        self.unit_context(),
                        frame,
                        padding,
                        &text_style,
                        &text,
                        false,
                        Point::ZERO,
                        point,
                    )
                });
                (
                    id,
                    interactions.clone(),
                    None,
                    None,
                    interactions.on_click.clone().map(ClickHandler::Command),
                    None,
                    cursor.map(|cursor| (id, frame, padding, text_style, text, false, cursor)),
                )
            }
            HitInteraction::Switch {
                id,
                interactions,
                on_change,
                current,
            } => (
                id,
                interactions.clone(),
                Some(id),
                interactions.on_focus.clone(),
                on_change
                    .clone()
                    .map(|command| ClickHandler::Toggle(command, !current))
                    .or_else(|| interactions.on_click.clone().map(ClickHandler::Command)),
                None,
                None,
            ),
            HitInteraction::Checkbox {
                id,
                interactions,
                on_change,
                current,
            } => (
                id,
                interactions.clone(),
                Some(id),
                interactions.on_focus.clone(),
                on_change
                    .clone()
                    .map(|command| ClickHandler::Toggle(command, !current))
                    .or_else(|| interactions.on_click.clone().map(ClickHandler::Command)),
                None,
                None,
            ),
            HitInteraction::Radio {
                id,
                interactions,
                on_change,
                current,
            } => (
                id,
                interactions.clone(),
                Some(id),
                interactions.on_focus.clone(),
                on_change
                    .clone()
                    .map(|command| ClickHandler::Toggle(command, !current))
                    .or_else(|| interactions.on_click.clone().map(ClickHandler::Command)),
                None,
                None,
            ),
            HitInteraction::SelectTrigger {
                id,
                interactions,
                on_open_change,
                is_open: _,
            } => {
                let is_open = self.resolved_select_open_state(id).unwrap_or(false);
                (
                    id,
                    interactions.clone(),
                    Some(id),
                    interactions.on_focus.clone(),
                    interactions.on_click.clone().map(ClickHandler::Command),
                    Some((id, !is_open, on_open_change.clone())),
                    None,
                )
            }
            HitInteraction::TextInput {
                id,
                interactions,
                value,
                placeholder: _,
                on_change: _,
                multiline,
                frame,
                padding,
                text_style,
            } => (
                id,
                interactions.clone(),
                Some(id),
                interactions.on_focus.clone(),
                interactions.on_click.clone().map(ClickHandler::Command),
                None,
                pointer_position.map(|point| {
                    let scroll = self.scroll_states.get(&id).copied().unwrap_or(Point::ZERO);
                    let cursor = text_cursor_index_at_point(
                        &self.font_manager,
                        &self.theme,
                        self.unit_context(),
                        frame,
                        padding,
                        &text_style,
                        &value,
                        multiline,
                        scroll,
                        point,
                    );
                    (id, frame, padding, text_style, value, multiline, cursor)
                }),
            ),
            HitInteraction::SelectOption {
                id,
                option_index: _,
                interactions,
                on_select,
                ref on_open_change,
            } => (
                id,
                interactions.clone(),
                None,
                None,
                Some(ClickHandler::SelectOption {
                    widget_id: id,
                    command: on_select.clone(),
                    on_open_change: on_open_change.clone(),
                }),
                Some((id, false, on_open_change.clone())),
                None,
            ),
            HitInteraction::Disabled { .. } => unreachable!("disabled hit handled above"),
            HitInteraction::CanvasItem { .. } => unreachable!("canvas item handled above"),
        };

        if selectable_text.is_none() {
            self.clear_selected_text();
        }

        if !clicked_select {
            self.close_all_open_selects_except(None);
        }

        self.update_focus(
            focus_target.map(|id| FocusedWidget {
                widget_id: id,
                on_blur: interactions.on_blur.clone(),
            }),
            focus_command,
            false,
        );
        if text_input_hit {
            self.focus_visible = true;
            self.reset_caret_blink();
            self.sync_ime_state();
        }
        if let Some((select_id, next_open, on_open_change)) = select_toggle {
            self.close_all_open_selects_except(next_open.then_some(select_id));
            let _ = self.set_select_open_state(select_id, next_open, on_open_change.as_ref());
        }
        self.pressed_widget = Some(widget_id);

        if let Some((widget_id, frame, padding, text_style, text, multiline, cursor)) =
            selectable_text
        {
            self.begin_text_selection(
                widget_id, frame, padding, text_style, text, multiline, cursor,
            );
        }

        if let Some(handler) = click_handler {
            if interactions.on_double_click.is_some() {
                let target_id = HoverTargetId::Widget(widget_id);
                let is_double_click = self
                    .pending_click
                    .as_ref()
                    .map(|pending| pending.target_id == target_id && pending.deadline > now)
                    .unwrap_or(false);

                if is_double_click {
                    self.pending_click = None;
                    if let Some(command) = interactions
                        .on_double_click
                        .clone()
                        .map(ClickHandler::Command)
                    {
                        self.execute_click_handler(&command, self.cursor_position);
                    } else {
                        self.execute_click_handler(&handler, self.cursor_position);
                    }
                } else {
                    self.pending_click = Some(PendingClick {
                        target_id,
                        deadline: now + super::DOUBLE_CLICK_THRESHOLD,
                        command: Some(handler),
                    });
                }
            } else {
                self.execute_click_handler(&handler, self.cursor_position);
            }
        } else {
            self.dispatch_widget_click(HoverTargetId::Widget(widget_id), interactions, now);
        }
    }

    pub(super) fn handle_canvas_mouse_release(&mut self, button: CanvasMouseButton) {
        if let Some(position) = self.cursor_position {
            if let Some(drag) = self.active_canvas_drag.as_ref() {
                let context = drag.context;
                if let Some(command) = drag.on_mouse_up.clone() {
                    self.execute_canvas_mouse_command(&command, context, position, Some(button));
                }
            } else {
                for interaction in self.hit_path(self.viewport_rect()).into_iter().rev() {
                    if let HitInteraction::CanvasItem {
                        item_id,
                        item_interactions,
                        canvas_origin,
                        item_origin,
                        ..
                    } = interaction
                    {
                        if let Some(command) = item_interactions.on_mouse_up {
                            self.execute_canvas_mouse_command(
                                &command,
                                CanvasPointerContext {
                                    item_id,
                                    canvas_origin,
                                    item_origin,
                                },
                                position,
                                Some(button),
                            );
                            break;
                        }
                    }
                }
            }
        }
        self.end_canvas_drag();
    }

    pub(super) fn handle_bound_window_event(
        &mut self,
        event_loop: &dyn ActiveEventLoop,
        event: WindowEvent,
    ) -> bool {
        if let WindowEvent::PointerMoved { position, .. } = &event {
            self.set_pointer_position(*position);
        }

        if let WindowEvent::ModifiersChanged(modifiers) = &event {
            self.modifiers = modifiers.state();
        }

        if matches!(event, WindowEvent::PointerLeft { .. }) {
            self.clear_pointer_position();
            if let Some(window) = self.window.as_ref() {
                window.request_redraw();
            }
        }

        if Self::should_dispatch_widget_event(&event) {
            let viewport = self.viewport_rect();
            let revision_before = self.invalidation.revision();
            let mut needs_redraw = !matches!(event, WindowEvent::PointerMoved { .. });

            match &event {
                WindowEvent::PointerMoved { .. } => {
                    if self.active_scrollbar_drag.is_some() {
                        needs_redraw |= self.handle_scrollbar_drag();
                        needs_redraw |= self.sync_scrollbar_hover();
                        needs_redraw |= self.update_cursor_icon();
                    } else if self.active_canvas_drag.is_some() {
                        needs_redraw |= self.handle_canvas_drag();
                        needs_redraw |= self.handle_hover(viewport);
                    } else if self.active_text_selection.is_some() {
                        needs_redraw |= self.handle_text_selection_drag();
                        needs_redraw |= self.handle_hover(viewport);
                    } else {
                        needs_redraw |= self.handle_hover(viewport);
                    }
                }
                WindowEvent::MouseWheel { delta, .. } => {
                    needs_redraw |= self.handle_mouse_wheel(*delta);
                }
                WindowEvent::PointerButton {
                    state: ElementState::Pressed,
                    position,
                    button,
                    ..
                } => {
                    self.set_pointer_position(*position);
                    let canvas_button = canvas_mouse_button(button.clone().mouse_button());
                    if button.clone().mouse_button() == Some(MouseButton::Left)
                        && self.begin_scrollbar_drag()
                    {
                        needs_redraw = true;
                        needs_redraw |= self.update_cursor_icon();
                    } else if let Some(canvas_button) = canvas_button {
                        self.handle_mouse_press(viewport, Instant::now(), canvas_button);
                    }
                }
                WindowEvent::KeyboardInput { event, .. } => {
                    needs_redraw |= self.handle_keyboard_input(event);
                }
                _ => {}
            }

            if self.invalidation.revision() != revision_before {
                needs_redraw = true;
            }

            if needs_redraw {
                if let Some(window) = self.window.as_ref() {
                    window.request_redraw();
                }
            }
        }

        if let Some(window_command) = self
            .commands
            .iter()
            .find(|entry| entry.trigger.matches(&event))
            .cloned()
        {
            self.execute_command(&window_command.command);
            if let Some(window) = self.window.as_ref() {
                window.request_redraw();
            }
        }

        if self.drain_window_requests() {
            return true;
        }

        match event {
            WindowEvent::CloseRequested => {
                return self.close_policy() == super::WindowClosePolicy::Close
            }
            WindowEvent::Focused(false) => {
                self.end_scrollbar_drag();
                self.end_canvas_drag();
                self.pressed_widget = None;
                self.update_focus(None, None, false);
                if let Some(window) = self.window.as_ref() {
                    window.request_redraw();
                }
            }
            WindowEvent::ThemeChanged(theme) => {
                self.apply_window_theme(Some(theme));
                self.sync_bindings(Instant::now());
                if let Some(window) = self.window.as_ref() {
                    window.request_redraw();
                }
            }
            WindowEvent::ScaleFactorChanged { .. } => {
                self.apply_window_theme(None);
                self.invalidate_scene();
                if let Some(window) = self.window.as_ref() {
                    if let Some(renderer) = self.renderer.as_mut() {
                        renderer.resize(window.surface_size(), window.scale_factor() as f32);
                    }
                    window.request_redraw();
                }
            }
            WindowEvent::Ime(event) => {
                let needs_redraw = self.handle_ime_event(&event);
                if needs_redraw {
                    if let Some(window) = self.window.as_ref() {
                        window.request_redraw();
                    }
                }
            }
            WindowEvent::PointerButton {
                state: ElementState::Released,
                position,
                button,
                ..
            } => {
                self.set_pointer_position(position);
                if let Some(canvas_button) = canvas_mouse_button(button.clone().mouse_button()) {
                    self.handle_canvas_mouse_release(canvas_button);
                }
                self.end_scrollbar_drag();
                self.pressed_widget = None;
                self.end_text_selection_drag();
                self.handle_hover(self.viewport_rect());
                self.update_cursor_icon();
                if let Some(window) = self.window.as_ref() {
                    window.request_redraw();
                }
            }
            WindowEvent::SurfaceResized(size) => {
                self.invalidate_scene();
                if let Some(renderer) = self.renderer.as_mut() {
                    let scale_factor = self
                        .window
                        .as_ref()
                        .map(|window| window.scale_factor() as f32)
                        .unwrap_or(1.0);
                    renderer.resize(size, scale_factor);
                }

                if let Some(window) = self.window.as_ref() {
                    window.request_redraw();
                }
            }
            WindowEvent::RedrawRequested => match self.render_current_frame() {
                Ok(RenderStatus::Rendered | RenderStatus::SkipFrame) => {}
                Ok(RenderStatus::ReconfigureSurface) => {
                    if let Some(renderer) = self.renderer.as_mut() {
                        renderer.reconfigure();
                    }
                    match self.render_current_frame() {
                        Ok(RenderStatus::Rendered | RenderStatus::SkipFrame) => {}
                        Ok(RenderStatus::ReconfigureSurface) => {}
                        Err(error) => self.fail(event_loop, error),
                    }
                }
                Err(error) => self.fail(event_loop, error),
            },
            _ => {}
        }

        false
    }

    pub(super) fn handle_bound_about_to_wait(&mut self, event_loop: &dyn ActiveEventLoop) -> bool {
        let now = Instant::now();
        let theme_changed = self.refresh_platform_theme();
        if theme_changed {
            self.sync_bindings(now);
            if let Some(window) = self.window.as_ref() {
                window.request_redraw();
            }
        }
        self.request_redraw_if_dirty(now);
        #[cfg(all(target_os = "android", feature = "android"))]
        let animation_frame_advanced = self.drive_animations(event_loop, now);
        #[cfg(not(all(target_os = "android", feature = "android")))]
        self.drive_animations(event_loop, now);
        #[cfg(all(target_os = "android", feature = "android"))]
        if theme_changed || animation_frame_advanced {
            self.render_immediately(event_loop);
        }
        self.drain_window_requests()
    }
}

struct HoverMoveOrTransition;

impl HoverMoveOrTransition {
    fn into_transition<VM>(command: Command<VM>) -> HoverTransitionHandler<VM> {
        HoverTransitionHandler::Command(command)
    }
}
