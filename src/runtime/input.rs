use std::collections::HashSet;
use std::sync::Arc;
use std::time::Instant;

mod editing;
mod focus;
mod hovering;
mod interaction;
mod key_repeat;
mod navigation;
mod platform_keys;
mod pointer_click;
mod pointer_press;
mod scrolling;
mod select_state;
mod session;
mod slider;
mod text_input;
mod window_events;

use super::{
    canvas_mouse_button, cursor_icon, is_primary_shortcut_modifier, mouse_scroll_delta,
    text_cursor_index_at_point, BoundRuntimeHandler, CanvasPointerContext, ClickHandler,
    FocusedWidget, HoverMoveHandler, HoverTargetId, HoverTransitionHandler, HoveredWidget,
    PendingClick, ScrollbarDrag, SliderDrag, SmoothScrollState, TextSelectionDrag,
};
use crate::foundation::binding::TextChange;
use crate::foundation::view_model::{Command, ValueCommand};
use crate::log::{log_text_profile, text_profile_enabled};
use crate::platform::cursor::{Cursor, CursorIcon};
use crate::platform::dpi::{PhysicalPosition, PhysicalSize};
use crate::platform::event::{
    ElementState, Ime, KeyEvent, MouseButton, MouseScrollDelta, WindowEvent,
};
use crate::platform::keyboard::{Key, KeyCode, NamedKey, PhysicalKey};
use crate::platform::window::ImeRequestData;
use crate::text::rope_buffer::RopeBuffer;
use crate::ui::unit::{Dp, UnitContext};
use crate::ui::widget::{
    CanvasItemInteractionHandlers, CanvasMouseButton, HitInteraction, InteractionHandlers, Point,
    Rect, ScrollbarAxis, ScrollbarHandle, Text, TextEditState, WidgetId, WidgetTree,
};
pub(super) const INPUT_CARET_WIDTH: f32 = 2.0;
use self::platform_keys::is_key_physically_pressed;
use self::session::text_replacement_bounds;
use self::text_input::{refresh_session_buffer, text_edit_display_text};
pub(super) use self::text_input::{TextInputFlushData, TextInputFlushOutcome, TextInputRegionData};
use crate::platform::backend::event_loop::ActiveEventLoop;
use crate::rendering::renderer::RenderStatus;

impl<VM: 'static> BoundRuntimeHandler<VM> {
    fn flush_text_input_session(&mut self, widget_id: WidgetId) -> TextInputFlushOutcome {
        let started_at = text_profile_enabled().then_some(Instant::now());
        let (flush_data, flush_source) =
            if let Some(region) = self.cached_text_input_region_data(widget_id) {
                (
                    TextInputFlushData {
                        controller: region.controller,
                        on_change: region.on_change,
                        on_change_set: region.on_change_set,
                    },
                    "cached_region",
                )
            } else if let Some(flush_data) = self.cached_text_input_flush_data(widget_id) {
                (flush_data, "cached_flush")
            } else if let Some(region) = self.text_input_region_data(widget_id) {
                (
                    TextInputFlushData {
                        controller: region.controller,
                        on_change: region.on_change,
                        on_change_set: region.on_change_set,
                    },
                    "computed_region",
                )
            } else {
                return TextInputFlushOutcome::default();
            };
        let Some(session) = self.text_input_buffers.get_mut(&widget_id) else {
            return TextInputFlushOutcome::default();
        };
        let Some(mut change_set) = session.take_pending_change_set() else {
            return TextInputFlushOutcome::default();
        };
        let next_text = session.current_text.clone();
        let _wake_guard = self.invalidation.suppress_wakeups();

        let controller_started_at = Instant::now();
        let end_revision = flush_data
            .controller
            .set_text_local_assuming_changed(next_text.clone());
        let controller_duration = controller_started_at.elapsed();
        change_set.end_revision = end_revision;
        session.external_value = next_text;
        session.external_revision = end_revision;
        session.pending_changes.clear();
        session.pending_start_revision = None;

        let change_count = change_set.changes.len();
        let text_len = session.external_value.len();
        let callbacks_started_at = Instant::now();
        if let Some(command) = flush_data.on_change_set.as_ref() {
            self.execute_value_command_without_invalidation(command, change_set.clone());
        }
        if let Some(command) = flush_data.on_change.as_ref() {
            self.execute_command_without_invalidation(command);
        }
        let callbacks_duration = callbacks_started_at.elapsed();
        let outcome = TextInputFlushOutcome {
            changed: true,
            requires_global_invalidation: false,
        };
        if let Some(started_at) = started_at {
            log_text_profile(
                "textarea_flush_session",
                started_at.elapsed(),
                format!(
                    "widget={:?} changes={} text_len={} controller_ms={:.3} callbacks_ms={:.3} revision={} source={}",
                    widget_id,
                    change_count,
                    text_len,
                    controller_duration.as_secs_f64() * 1000.0,
                    callbacks_duration.as_secs_f64() * 1000.0,
                    end_revision,
                    flush_source,
                ),
            );
        }
        outcome
    }

    pub(super) fn flush_pending_text_input_changes(&mut self) -> TextInputFlushOutcome {
        let started_at = text_profile_enabled().then_some(Instant::now());
        let widget_ids: Vec<_> = self
            .text_input_buffers
            .iter()
            .filter_map(|(widget_id, session)| {
                (!session.pending_changes.is_empty()).then_some(*widget_id)
            })
            .collect();
        let dirty_count = widget_ids.len();
        let mut outcome = TextInputFlushOutcome::default();
        for widget_id in widget_ids {
            let next = self.flush_text_input_session(widget_id);
            outcome.changed |= next.changed;
            outcome.requires_global_invalidation |= next.requires_global_invalidation;
        }
        if let Some(started_at) = started_at {
            log_text_profile(
                "textarea_flush_pending",
                started_at.elapsed(),
                format!(
                    "dirty_widgets={} changed={} requires_global_invalidation={}",
                    dirty_count, outcome.changed, outcome.requires_global_invalidation
                ),
            );
        }
        outcome
    }

    pub(super) fn handle_ime_event(&mut self, event: &Ime) -> bool {
        let started_at = text_profile_enabled().then_some(Instant::now());
        let handled = match event {
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
            }),
            Ime::Disabled => self.clear_focused_input_composition(),
        };
        let _ = started_at;
        handled
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

    pub(super) fn handle_keyboard_input(&mut self, event: &KeyEvent) -> bool {
        let started_at = text_profile_enabled().then_some(Instant::now());
        if event.state != ElementState::Pressed {
            return false;
        }

        if event.repeat && !self.allows_repeated_keyboard_input(event) {
            return false;
        }

        // When the platform IME is composing text, key presses such as pinyin
        // letters, candidate navigation, Enter, and Backspace should stay owned
        // by the IME. If we also treat them as direct text-edit commands, CJK
        // input gets corrupted with stray Latin characters or cursor moves.
        if self.focused_input_has_active_composition()
            && !is_primary_shortcut_modifier(self.modifiers)
            && !self.modifiers.alt_key()
        {
            return false;
        }

        let handled = match event.physical_key {
            PhysicalKey::Code(KeyCode::Tab)
                if !is_primary_shortcut_modifier(self.modifiers) && !self.modifiers.alt_key() =>
            {
                self.advance_focus(self.modifiers.shift_key())
            }
            PhysicalKey::Code(KeyCode::Backspace) => self.delete_backward_at_focused_input(),
            PhysicalKey::Code(KeyCode::Delete) => self.delete_forward_at_focused_input(),
            PhysicalKey::Code(KeyCode::ArrowLeft) => {
                if self.adjust_focused_slider(-1, None) {
                    true
                } else {
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
            }
            PhysicalKey::Code(KeyCode::ArrowRight) => {
                if self.adjust_focused_slider(1, None) {
                    true
                } else {
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
            }
            PhysicalKey::Code(KeyCode::ArrowUp) => {
                self.adjust_focused_slider(1, None)
                    || self.move_focused_input_cursor_vertically(-1, self.modifiers.shift_key())
            }
            PhysicalKey::Code(KeyCode::ArrowDown) => {
                self.adjust_focused_slider(-1, None)
                    || self.move_focused_input_cursor_vertically(1, self.modifiers.shift_key())
            }
            PhysicalKey::Code(KeyCode::Home) => {
                self.adjust_focused_slider(0, Some(false))
                    || self.move_focused_input_cursor(|_, _| 0, self.modifiers.shift_key())
            }
            PhysicalKey::Code(KeyCode::End) => {
                self.adjust_focused_slider(0, Some(true))
                    || self.move_focused_input_cursor(
                        |buffer: &RopeBuffer, _state: &TextEditState| buffer.len_bytes(),
                        self.modifiers.shift_key(),
                    )
            }
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
        };
        if let Some(started_at) = started_at {
            let current_len = self
                .focused_text_input_id()
                .and_then(|widget_id| self.text_input_buffers.get(&widget_id))
                .map(|session| session.current_text.len())
                .unwrap_or(0);
            log_text_profile(
                "textarea_keyboard",
                started_at.elapsed(),
                format!(
                    "key={:?} logical={:?} repeat={} handled={} focused_input={:?} text_len={}",
                    event.physical_key,
                    event.logical_key,
                    event.repeat,
                    handled,
                    self.focused_text_input_id(),
                    current_len,
                ),
            );
        }
        handled
    }

    pub(super) fn handle_platform_keyboard_input(&mut self, event: &KeyEvent) -> bool {
        match event.state {
            ElementState::Released => {
                self.disarm_key_repeat(event.physical_key);
                false
            }
            ElementState::Pressed if event.repeat => false,
            ElementState::Pressed => {
                let handled = self.handle_keyboard_input(event);
                if handled && self.allows_repeated_keyboard_input(event) {
                    self.arm_key_repeat(event, Instant::now());
                }
                handled
            }
        }
    }

    pub(super) fn drive_key_repeat(&mut self, now: Instant) -> bool {
        let Some(active) = self.active_key_repeat.clone() else {
            return false;
        };
        if active.next_fire_at > now {
            return false;
        }

        if !is_key_physically_pressed(active.event.physical_key) {
            self.disarm_key_repeat(active.event.physical_key);
            return false;
        }

        let mut repeated_event = active.event;
        repeated_event.repeat = true;
        if !self.allows_repeated_keyboard_input(&repeated_event) {
            self.disarm_key_repeat(repeated_event.physical_key);
            return false;
        }

        let handled = self.handle_keyboard_input(&repeated_event);
        if self
            .active_key_repeat
            .as_ref()
            .map(|state| state.event.physical_key == repeated_event.physical_key)
            .unwrap_or(false)
        {
            if self.allows_repeated_keyboard_input(&repeated_event) {
                if let Some(state) = self.active_key_repeat.as_mut() {
                    state.next_fire_at = now + super::KEY_REPEAT_INTERVAL;
                }
            } else {
                self.disarm_key_repeat(repeated_event.physical_key);
            }
        }
        handled
    }

    pub(super) fn next_key_repeat_deadline(&self) -> Option<Instant> {
        self.active_key_repeat
            .as_ref()
            .map(|state| state.next_fire_at)
    }
}
