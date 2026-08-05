use std::sync::Arc;
use std::time::Instant;

mod calendar;
mod combobox;
mod editing;
mod focus;
mod gesture;
mod hovering;
mod interaction;
#[cfg(test)]
pub(crate) use interaction::scroll_region_lookup_probe;
mod key_repeat;
mod list;
mod navigation;
mod overlay_close;
mod platform_keys;
mod pointer_click;
mod pointer_press;
mod radio_group;
mod scrolling;
mod select_state;
mod session;
mod slider;
mod splitter;
mod table;
mod tabs;
mod text_input;
mod tree;
mod window_events;

use super::{
    canvas_mouse_button, cursor_icon, is_primary_shortcut_modifier, mouse_scroll_delta,
    text_cursor_index_at_point, BoundRuntimeHandler, CanvasPointerContext, ClickHandler,
    FocusedWidget, HoverMoveHandler, HoverTargetId, HoverTransitionHandler, HoveredWidget,
    PendingClick, PendingSplitterClick, ScrollbarDrag, SliderDrag, SmoothScrollState,
    TextSelectionDrag, TouchScrollDrag, TouchScrollInertiaState,
};
use crate::foundation::binding::TextChange;
use crate::foundation::view_model::{Command, ValueCommand};
use crate::log::{log_text_profile, text_profile_enabled};
use crate::platform::cursor::{Cursor, CursorIcon};
use crate::platform::dpi::{PhysicalPosition, PhysicalSize};
use crate::platform::event::{
    ButtonSource, ElementState, Ime, KeyEvent, MouseButton, MouseScrollDelta, WindowEvent,
};
use crate::platform::keyboard::{Key, KeyCode, NamedKey, PhysicalKey};
use crate::platform::window::ImeRequestData;
use crate::text::rope_buffer::{
    next_grapheme_boundary_byte, prev_grapheme_boundary_byte, RopeBuffer,
};
use crate::ui::unit::{Dp, UnitContext};
use crate::ui::widget::{
    CanvasItemInteractionHandlers, CanvasMouseButton, HitInteraction, InteractionHandlers, Point,
    Rect, ScrollbarAxis, ScrollbarHandle, TextEditState, WidgetId, WidgetTree,
};
pub(super) const INPUT_CARET_WIDTH: f32 = 2.0;
pub(in crate::runtime) use self::focus::FocusNavigationSnapshot;
use self::platform_keys::is_key_physically_pressed;
use self::session::text_replacement_bounds;
use self::text_input::{refresh_session_buffer, text_edit_display_text};
pub(super) use self::text_input::{
    ScrollContext, TextInputContext, TextInputFlushData, TextInputFlushOutcome,
    TextInputRegionData, TextInputSnapshot,
};
use crate::platform::backend::event_loop::ActiveEventLoop;
use crate::rendering::renderer::RenderStatus;

pub(super) fn normalize_text_input_value(text: &str, multiline: bool) -> String {
    let mut normalized = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(character) = chars.next() {
        match character {
            '\r' => {
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
                if multiline {
                    normalized.push('\n');
                }
            }
            '\n' | '\u{0085}' | '\u{2028}' | '\u{2029}' if multiline => normalized.push('\n'),
            '\n' | '\u{0085}' | '\u{2028}' | '\u{2029}' => {}
            character => normalized.push(character),
        }
    }
    normalized
}

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
            Ime::Disabled => self.clear_focused_input_composition(),
        };
        let _ = started_at;
        handled
    }

    pub(super) fn flush_pending_click_if_due(&mut self, now: Instant) {
        if self.pending_click_waits_for_splitter_release() {
            return;
        }
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

    pub(super) fn pending_click_deadline(&self) -> Option<Instant> {
        if self.pending_click_waits_for_splitter_release() {
            None
        } else {
            self.pending_click.as_ref().map(|pending| pending.deadline)
        }
    }

    fn pending_click_waits_for_splitter_release(&self) -> bool {
        let Some(pending_splitter) = self
            .pending_click
            .as_ref()
            .and_then(|pending| pending.splitter)
        else {
            return false;
        };
        let Some(active) = self.active_splitter_resize.as_ref() else {
            return false;
        };
        active.axis == pending_splitter.axis
            && active.index == pending_splitter.index
            && active.start_sizes.len() == pending_splitter.pane_count
    }

    pub(super) fn handle_keyboard_input(&mut self, event: &KeyEvent) -> bool {
        let started_at = text_profile_enabled().then_some(Instant::now());
        if event.state != ElementState::Pressed {
            return false;
        }

        if event.repeat && !self.allows_repeated_keyboard_input(event) {
            return false;
        }

        let requests_context_menu =
            matches!(event.physical_key, PhysicalKey::Code(KeyCode::ContextMenu))
                || (matches!(event.physical_key, PhysicalKey::Code(KeyCode::F10))
                    && self.modifiers == crate::platform::keyboard::ModifiersState::SHIFT);
        if requests_context_menu
            && self
                .focused_widget_id()
                .is_some_and(|widget_id| self.open_context_menu_semantically(widget_id))
        {
            return true;
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

        // 全局菜单快捷键派发：在所有其它键盘处理之前查 cached scene 里挂的 menu /
        // context_menu 上声明的 KeyChord。命中即执行 on_select 并吞键。
        if let PhysicalKey::Code(code) = event.physical_key {
            if self.dispatch_global_menu_shortcut(self.modifiers, &event.logical_key, code) {
                return true;
            }
        }

        let handled = match event.physical_key {
            PhysicalKey::Code(KeyCode::Escape) => {
                self.consume_topmost_overlay_close_handler_escape()
            }
            PhysicalKey::Code(KeyCode::ArrowUp) if self.topmost_open_menu_id().is_some() => {
                self.advance_menu_keyboard_cursor(-1)
            }
            PhysicalKey::Code(KeyCode::ArrowDown) if self.topmost_open_menu_id().is_some() => {
                self.advance_menu_keyboard_cursor(1)
            }
            PhysicalKey::Code(KeyCode::ArrowLeft) if self.topmost_open_menu_id().is_some() => {
                self.leave_submenu_or_advance_menubar(-1)
            }
            PhysicalKey::Code(KeyCode::ArrowRight) if self.topmost_open_menu_id().is_some() => {
                self.enter_submenu_or_advance_menubar(1)
            }
            PhysicalKey::Code(KeyCode::Enter) | PhysicalKey::Code(KeyCode::NumpadEnter)
                if self.topmost_open_menu_id().is_some() =>
            {
                self.activate_menu_keyboard_cursor()
            }
            PhysicalKey::Code(KeyCode::Space)
                if self.topmost_open_menu_id().is_some()
                    && self.focused_text_input_id().is_none() =>
            {
                self.activate_menu_keyboard_cursor()
            }
            PhysicalKey::Code(_)
                if self.topmost_open_menu_id().is_some()
                    && self.focused_text_input_id().is_none()
                    && self.modifiers == crate::platform::keyboard::ModifiersState::empty() =>
            {
                if let Key::Character(text) = &event.logical_key {
                    if let Some(letter) = text.chars().next() {
                        if letter.is_alphanumeric() && self.type_ahead_menu_cursor(letter) {
                            true
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                } else {
                    false
                }
            }
            PhysicalKey::Code(KeyCode::Tab)
                if !is_primary_shortcut_modifier(self.modifiers) && !self.modifiers.alt_key() =>
            {
                self.advance_focus(self.modifiers.shift_key())
            }
            PhysicalKey::Code(KeyCode::Enter) | PhysicalKey::Code(KeyCode::NumpadEnter) => {
                if self.activate_focused_select_option() {
                    true
                } else if self.activate_first_open_popover_option_from_input() {
                    true
                } else if let Some(id) = self.focused_text_input_id() {
                    if is_primary_shortcut_modifier(self.modifiers) || self.modifiers.alt_key() {
                        false
                    } else {
                        self.sync_text_input_buffer(id)
                            .is_some_and(|region| region.multiline)
                            && self.insert_text_at_focused_input("\n")
                    }
                } else {
                    self.activate_focused_data_grid_cell(true, false)
                        || self.activate_focused_tree_node(true, false)
                        || self.activate_focused_list_item(true, false)
                        || self.activate_focused_widget(true, false)
                }
            }
            PhysicalKey::Code(KeyCode::Space) if self.focused_text_input_id().is_none() => {
                self.activate_focused_select_option()
                    || self.activate_focused_data_grid_cell(false, true)
                    || self.activate_focused_tree_node(false, true)
                    || self.activate_focused_list_item(false, true)
                    || self.activate_focused_widget(false, true)
            }
            PhysicalKey::Code(KeyCode::Backspace) => self.delete_backward_at_focused_input(),
            PhysicalKey::Code(KeyCode::Delete) => self.delete_forward_at_focused_input(),
            PhysicalKey::Code(KeyCode::ArrowLeft) => {
                if self.move_focused_calendar_day_by_days(-1) {
                    true
                } else if self.move_focused_radio_group(crate::ui::layout::Axis::Horizontal, -1) {
                    true
                } else if self.focused_tab_is_horizontal() == Some(true)
                    && self.move_focused_tab(-1)
                {
                    true
                } else if self.adjust_focused_data_grid_resize(-1) {
                    true
                } else if self.move_focused_data_grid_cell(0, -1, self.modifiers.shift_key()) {
                    true
                } else if self.collapse_or_focus_parent_tree_node() {
                    true
                } else if self.adjust_focused_splitter(crate::ui::layout::Axis::Horizontal, -1) {
                    true
                } else if self.adjust_focused_slider(-1, None, Some(true)) {
                    true
                } else {
                    let extend = self.modifiers.shift_key();
                    self.move_focused_input_cursor(
                        |text: &str, is_ascii: bool, state: &TextEditState| {
                            if let Some((start, _)) = state.selection_range() {
                                if extend {
                                    prev_grapheme_boundary_byte(text, state.cursor, is_ascii)
                                } else {
                                    start
                                }
                            } else {
                                prev_grapheme_boundary_byte(text, state.cursor, is_ascii)
                            }
                        },
                        extend,
                    )
                }
            }
            PhysicalKey::Code(KeyCode::ArrowRight) => {
                if self.move_focused_calendar_day_by_days(1) {
                    true
                } else if self.move_focused_radio_group(crate::ui::layout::Axis::Horizontal, 1) {
                    true
                } else if self.focused_tab_is_horizontal() == Some(true) && self.move_focused_tab(1)
                {
                    true
                } else if self.adjust_focused_data_grid_resize(1) {
                    true
                } else if self.move_focused_data_grid_cell(0, 1, self.modifiers.shift_key()) {
                    true
                } else if self.expand_or_focus_child_tree_node() {
                    true
                } else if self.adjust_focused_splitter(crate::ui::layout::Axis::Horizontal, 1) {
                    true
                } else if self.adjust_focused_slider(1, None, Some(true)) {
                    true
                } else {
                    let extend = self.modifiers.shift_key();
                    self.move_focused_input_cursor(
                        |text: &str, is_ascii: bool, state: &TextEditState| {
                            if let Some((_, end)) = state.selection_range() {
                                if extend {
                                    next_grapheme_boundary_byte(text, state.cursor, is_ascii)
                                } else {
                                    end
                                }
                            } else {
                                next_grapheme_boundary_byte(text, state.cursor, is_ascii)
                            }
                        },
                        extend,
                    )
                }
            }
            PhysicalKey::Code(KeyCode::ArrowUp) => {
                self.adjust_focused_number_input(1)
                    || self.move_focused_select_option(-1)
                    || self.focus_open_popover_option_from_input(-1)
                    || self.move_focused_calendar_day_by_days(-7)
                    || self.move_focused_radio_group(crate::ui::layout::Axis::Vertical, -1)
                    || (self.focused_tab_is_horizontal() == Some(false)
                        && self.move_focused_tab(-1))
                    || self.move_focused_data_grid_cell(-1, 0, self.modifiers.shift_key())
                    || self.move_focused_tree_node(-1, self.modifiers.shift_key())
                    || self.enter_focused_tree_root(true, self.modifiers.shift_key())
                    || self.move_focused_list_item(-1, self.modifiers.shift_key())
                    || self.enter_focused_list_root(true, self.modifiers.shift_key())
                    || self.adjust_focused_splitter(crate::ui::layout::Axis::Vertical, -1)
                    || self.adjust_focused_slider(1, None, Some(false))
                    || self.move_focused_input_cursor_vertically(-1, self.modifiers.shift_key())
            }
            PhysicalKey::Code(KeyCode::ArrowDown) => {
                self.adjust_focused_number_input(-1)
                    || self.move_focused_select_option(1)
                    || self.focus_open_popover_option_from_input(1)
                    || self.move_focused_calendar_day_by_days(7)
                    || self.move_focused_radio_group(crate::ui::layout::Axis::Vertical, 1)
                    || (self.focused_tab_is_horizontal() == Some(false) && self.move_focused_tab(1))
                    || self.move_focused_data_grid_cell(1, 0, self.modifiers.shift_key())
                    || self.move_focused_tree_node(1, self.modifiers.shift_key())
                    || self.enter_focused_tree_root(false, self.modifiers.shift_key())
                    || self.move_focused_list_item(1, self.modifiers.shift_key())
                    || self.enter_focused_list_root(false, self.modifiers.shift_key())
                    || self.adjust_focused_splitter(crate::ui::layout::Axis::Vertical, 1)
                    || self.adjust_focused_slider(-1, None, Some(false))
                    || self.move_focused_input_cursor_vertically(1, self.modifiers.shift_key())
            }
            PhysicalKey::Code(KeyCode::Home) => {
                self.move_focused_calendar_day_to_week_edge(false)
                    || self.move_focused_tab_to_edge(false)
                    || self.move_focused_data_grid_cell_to_edge(false, self.modifiers.shift_key())
                    || self.move_focused_tree_node_to_edge(false, self.modifiers.shift_key())
                    || self.enter_focused_tree_root(false, self.modifiers.shift_key())
                    || self.move_focused_list_item_to_edge(false, self.modifiers.shift_key())
                    || self.enter_focused_list_root(false, self.modifiers.shift_key())
                    || self.adjust_focused_slider(0, Some(false), None)
                    || self.move_focused_input_cursor(|_, _, _| 0, self.modifiers.shift_key())
                    || self.scroll_focused_region_to_edge(false)
            }
            PhysicalKey::Code(KeyCode::End) => {
                self.move_focused_calendar_day_to_week_edge(true)
                    || self.move_focused_tab_to_edge(true)
                    || self.move_focused_data_grid_cell_to_edge(true, self.modifiers.shift_key())
                    || self.move_focused_tree_node_to_edge(true, self.modifiers.shift_key())
                    || self.enter_focused_tree_root(true, self.modifiers.shift_key())
                    || self.move_focused_list_item_to_edge(true, self.modifiers.shift_key())
                    || self.enter_focused_list_root(true, self.modifiers.shift_key())
                    || self.adjust_focused_slider(0, Some(true), None)
                    || self.move_focused_input_cursor(
                        |text: &str, _is_ascii: bool, _state: &TextEditState| text.len(),
                        self.modifiers.shift_key(),
                    )
                    || self.scroll_focused_region_to_edge(true)
            }
            PhysicalKey::Code(KeyCode::PageUp) => {
                self.move_focused_calendar_day_by_months(-1)
                    || self.page_focused_slider(1)
                    || self.page_focused_data_grid_cell(-1, self.modifiers.shift_key())
                    || self.page_focused_tree_node(-1, self.modifiers.shift_key())
                    || self.enter_focused_tree_root(false, self.modifiers.shift_key())
                    || self.page_focused_list_item(-1, self.modifiers.shift_key())
                    || self.enter_focused_list_root(false, self.modifiers.shift_key())
                    || self.scroll_focused_region_by_pages(-1)
            }
            PhysicalKey::Code(KeyCode::PageDown) => {
                self.move_focused_calendar_day_by_months(1)
                    || self.page_focused_slider(-1)
                    || self.page_focused_data_grid_cell(1, self.modifiers.shift_key())
                    || self.page_focused_tree_node(1, self.modifiers.shift_key())
                    || self.enter_focused_tree_root(false, self.modifiers.shift_key())
                    || self.page_focused_list_item(1, self.modifiers.shift_key())
                    || self.enter_focused_list_root(false, self.modifiers.shift_key())
                    || self.scroll_focused_region_by_pages(1)
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
                    // Physical Enter/NumpadEnter are handled above. Keep this fallback for
                    // platforms that report Enter without a mappable physical key.
                    Key::Named(NamedKey::Enter) => self.insert_text_at_focused_input("\n"),
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
                // Evaluate repeatability while the pre-event scene/cache is still valid.  A
                // keyboard activation command may conservatively invalidate the scene; asking
                // the same question after dispatch would synchronously rebuild a large tree just
                // to decide that Enter/Space is not repeatable.
                let allows_repeat = self.allows_repeated_keyboard_input(event);
                let handled = self.handle_keyboard_input(event);
                if handled && allows_repeat {
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
