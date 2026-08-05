use super::*;

impl<VM: 'static> BoundRuntimeHandler<VM> {
    pub(super) fn allows_repeated_keyboard_input(&mut self, event: &KeyEvent) -> bool {
        match event.physical_key {
            PhysicalKey::Code(
                KeyCode::Backspace
                | KeyCode::Delete
                | KeyCode::ArrowLeft
                | KeyCode::ArrowRight
                | KeyCode::ArrowUp
                | KeyCode::ArrowDown
                | KeyCode::PageUp
                | KeyCode::PageDown
                | KeyCode::Home
                | KeyCode::End,
            ) => {
                self.focused_text_input_id().is_some()
                    || self.focused_slider_hit().is_some()
                    || self.focused_scroll_region().is_some()
                    || self.focused_data_grid_cell_is_some()
                    || self.focused_tree_node_is_some()
                    || self.focused_list_item_is_some()
                    || self.focused_calendar_day().is_some()
            }
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

    pub(super) fn arm_key_repeat(&mut self, event: &KeyEvent, now: Instant) {
        let next_fire_at = now + super::super::KEY_REPEAT_INITIAL_DELAY;
        self.active_key_repeat = Some(super::super::ActiveKeyRepeat {
            event: event.clone(),
            next_fire_at,
        });
    }

    pub(super) fn disarm_key_repeat(&mut self, physical_key: PhysicalKey) {
        if self
            .active_key_repeat
            .as_ref()
            .map(|state| state.event.physical_key == physical_key)
            .unwrap_or(false)
        {
            self.active_key_repeat = None;
        }
    }
}
