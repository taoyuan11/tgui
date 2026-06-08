use super::*;
use crate::ui::widget::{
    ListItemAction, ListItemState, ListSelectionChange, ListSelectionMode, ListSelectionTrigger,
    ScrollRegion, WidgetKey,
};

struct ListKeyboardTarget<VM> {
    id: WidgetId,
    state: ListItemState<VM>,
    focus: Option<crate::ui::widget::FocusTargetMeta<VM>>,
}

impl<VM> Clone for ListKeyboardTarget<VM> {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            state: self.state.clone(),
            focus: self.focus.clone(),
        }
    }
}

impl<VM: 'static> BoundRuntimeHandler<VM> {
    pub(super) fn dispatch_list_item_click(
        &mut self,
        state: &ListItemState<VM>,
        widget_id: WidgetId,
        now: Instant,
        button: CanvasMouseButton,
    ) -> bool {
        if button != CanvasMouseButton::Left || self.gesture_consumes_click() {
            return false;
        }
        if !state.disabled.resolve() {
            self.remember_list_focus(state);
        }

        let target_id = HoverTargetId::Widget(widget_id);
        let is_double_click = self.pending_click_matches_target(target_id, now);
        if is_double_click {
            self.pending_click = None;
            return self.dispatch_list_item_action(state);
        }
        self.pending_click = Some(PendingClick {
            target_id,
            deadline: now + super::super::DOUBLE_CLICK_THRESHOLD,
            position: self.cursor_position.unwrap_or(Point::ZERO),
            command: None,
            splitter: None,
        });

        let extend = self.modifiers.shift_key();
        let toggle = is_primary_shortcut_modifier(self.modifiers);
        self.dispatch_list_item_selection(state, ListSelectionTrigger::Click, extend, toggle)
    }

    pub(in crate::runtime) fn dispatch_list_item_keyboard_selection(
        &mut self,
        state: &ListItemState<VM>,
        toggle: bool,
    ) -> bool {
        self.dispatch_list_item_selection(state, ListSelectionTrigger::Keyboard, false, toggle)
    }

    fn dispatch_list_item_selection(
        &mut self,
        state: &ListItemState<VM>,
        trigger: ListSelectionTrigger,
        extend: bool,
        toggle: bool,
    ) -> bool {
        if state.disabled.resolve() || state.selection_mode == ListSelectionMode::None {
            return false;
        }
        let Some(command) = state.on_selection_change.as_ref() else {
            self.update_list_anchor(state, extend);
            return false;
        };
        let selected_keys = next_selected_keys(
            state,
            self.list_anchor_states.get(&state.list_id),
            extend,
            toggle,
        );
        let anchor_key = self.update_list_anchor(state, extend);
        self.execute_value_command(
            command,
            ListSelectionChange {
                selected_keys,
                focused_key: Some(state.key.clone()),
                anchor_key,
                changed_key: Some(state.key.clone()),
                trigger,
            },
        );
        true
    }

    fn update_list_anchor(&mut self, state: &ListItemState<VM>, extend: bool) -> Option<WidgetKey> {
        let next = if extend {
            self.list_anchor_states
                .get(&state.list_id)
                .cloned()
                .unwrap_or_else(|| state.key.clone())
        } else {
            state.key.clone()
        };
        self.list_anchor_states.insert(state.list_id, next.clone());
        Some(next)
    }

    pub(in crate::runtime) fn dispatch_list_item_action(
        &mut self,
        state: &ListItemState<VM>,
    ) -> bool {
        if state.disabled.resolve() {
            return false;
        }
        let Some(command) = state.on_item_action.as_ref() else {
            return false;
        };
        self.execute_value_command(
            command,
            ListItemAction {
                index: state.item_index,
                key: state.key.clone(),
            },
        );
        true
    }

    fn focused_list_target(&mut self) -> Option<ListKeyboardTarget<VM>> {
        let focused_id = self.focused_widget_id()?;
        if let Some(target) = self.list_target_by_widget_id(focused_id) {
            self.remember_list_focus(&target.state);
            return Some(target);
        }
        if self.focused_widget_is_list_root(focused_id) {
            return self.remembered_list_target();
        }
        None
    }

    pub(super) fn focused_list_item_is_some(&mut self) -> bool {
        self.focused_list_target().is_some()
    }

    fn list_target_by_widget_id(&mut self, widget_id: WidgetId) -> Option<ListKeyboardTarget<VM>> {
        let computed = self.computed_scene();
        computed
            .hit_regions
            .iter()
            .chain(computed.overlay_hit_regions.iter())
            .find_map(|region| match &region.interaction {
                HitInteraction::ListItem { id, state, .. } if *id == widget_id => {
                    Some(ListKeyboardTarget {
                        id: *id,
                        state: state.clone(),
                        focus: region.focus.clone(),
                    })
                }
                _ => None,
            })
    }

    fn focused_widget_is_list_root(&mut self, widget_id: WidgetId) -> bool {
        let computed = self.computed_scene();
        computed
            .hit_regions
            .iter()
            .chain(computed.overlay_hit_regions.iter())
            .any(|region| {
                matches!(
                    &region.interaction,
                    HitInteraction::ListItem { state, .. } if state.list_id == widget_id
                )
            })
    }

    fn remembered_list_target(&mut self) -> Option<ListKeyboardTarget<VM>> {
        let (list_id, key) = self.list_focus_state.clone()?;
        let computed = self.computed_scene();
        computed
            .hit_regions
            .iter()
            .chain(computed.overlay_hit_regions.iter())
            .find_map(|region| match &region.interaction {
                HitInteraction::ListItem { id, state, .. }
                    if state.list_id == list_id
                        && state.key == key
                        && !state.disabled.resolve() =>
                {
                    Some(ListKeyboardTarget {
                        id: *id,
                        state: state.clone(),
                        focus: region.focus.clone(),
                    })
                }
                _ => None,
            })
    }

    fn list_targets(&mut self, list_id: WidgetId) -> Vec<ListKeyboardTarget<VM>> {
        let computed = self.computed_scene();
        let mut targets = computed
            .hit_regions
            .iter()
            .chain(computed.overlay_hit_regions.iter())
            .filter_map(|region| match &region.interaction {
                HitInteraction::ListItem { id, state, .. }
                    if state.list_id == list_id && !state.disabled.resolve() =>
                {
                    Some(ListKeyboardTarget {
                        id: *id,
                        state: state.clone(),
                        focus: region.focus.clone(),
                    })
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        targets.sort_by_key(|target| target.state.row_index);
        targets
    }

    pub(super) fn activate_focused_list_item(&mut self, enter: bool, space: bool) -> bool {
        let Some(target) = self.focused_list_target() else {
            return false;
        };
        if enter {
            return self.dispatch_list_item_action(&target.state);
        }
        if space {
            return self.dispatch_list_item_keyboard_selection(&target.state, true);
        }
        false
    }

    pub(super) fn move_focused_list_item(&mut self, step: i32, extend: bool) -> bool {
        let Some(current) = self.focused_list_target() else {
            return false;
        };
        let targets = self.list_targets(current.state.list_id);
        if targets.len() <= 1 {
            return false;
        }
        let Some(current_position) = targets.iter().position(|target| target.id == current.id)
        else {
            return false;
        };
        let next_position = if step < 0 {
            current_position.saturating_sub(1)
        } else {
            (current_position + 1).min(targets.len() - 1)
        };
        if next_position == current_position {
            return false;
        }
        self.focus_list_target(targets[next_position].clone(), extend)
    }

    pub(super) fn enter_focused_list_root(&mut self, end: bool, extend: bool) -> bool {
        let Some(list_id) = self.focused_widget_id() else {
            return false;
        };
        let targets = self.list_targets(list_id);
        let Some(target) = (if end { targets.last() } else { targets.first() }) else {
            return false;
        };
        self.focus_list_target(target.clone(), extend)
    }

    pub(super) fn move_focused_list_item_to_edge(&mut self, end: bool, extend: bool) -> bool {
        let Some(current) = self.focused_list_target() else {
            return false;
        };
        let targets = self.list_targets(current.state.list_id);
        let Some(target) = (if end { targets.last() } else { targets.first() }) else {
            return false;
        };
        if target.id == current.id {
            return false;
        }
        self.focus_list_target(target.clone(), extend)
    }

    pub(super) fn page_focused_list_item(&mut self, direction: i32, extend: bool) -> bool {
        let Some(current) = self.focused_list_target() else {
            return false;
        };
        let targets = self.list_targets(current.state.list_id);
        if targets.len() <= 1 {
            return false;
        }
        let Some(current_position) = targets.iter().position(|target| target.id == current.id)
        else {
            return false;
        };
        let page = self
            .scroll_region_for_widget(current.state.list_id)
            .map(|region| {
                let row_extent = row_extent_for_state(&current.state);
                (region.content_viewport.height / row_extent)
                    .floor()
                    .max(1.0) as usize
            })
            .unwrap_or(10);
        let next_position = if direction < 0 {
            current_position.saturating_sub(page)
        } else {
            (current_position + page).min(targets.len() - 1)
        };
        if next_position == current_position {
            return false;
        }
        self.focus_list_target(targets[next_position].clone(), extend)
    }

    fn focus_list_target(&mut self, target: ListKeyboardTarget<VM>, extend: bool) -> bool {
        let Some(focus) = target.focus.clone() else {
            return false;
        };
        self.remember_list_focus(&target.state);
        self.update_focus(
            Some(FocusedWidget {
                widget_id: target.id,
                scope_path: focus.scope_path,
                on_blur: focus.on_blur.clone(),
            }),
            focus.on_focus,
            true,
        );
        self.ensure_list_row_visible(&target.state);
        let _ = self.dispatch_list_item_selection(
            &target.state,
            ListSelectionTrigger::Keyboard,
            extend,
            false,
        );
        true
    }

    fn remember_list_focus(&mut self, state: &ListItemState<VM>) {
        self.list_focus_state = Some((state.list_id, state.key.clone()));
    }

    fn scroll_region_for_widget(&mut self, widget_id: WidgetId) -> Option<ScrollRegion> {
        self.scroll_regions()
            .into_iter()
            .find(|region| region.id == widget_id)
    }

    fn ensure_list_row_visible(&mut self, state: &ListItemState<VM>) -> bool {
        let Some(region) = self.scroll_region_for_widget(state.list_id) else {
            return false;
        };
        let current = self.effective_scroll_offset(region.id, region.scroll_offset);
        let row_extent = row_extent_for_state(state);
        let spacing = state.item_spacing.max(Dp::ZERO);
        let row_top = (row_extent + spacing) * state.row_index as f32;
        let row_bottom = row_top + row_extent;
        let viewport_top = current.y;
        let viewport_bottom = current.y + region.content_viewport.height;
        let max = region.max_offset();
        let next_y = if row_top < viewport_top {
            row_top
        } else if row_bottom > viewport_bottom {
            row_bottom - region.content_viewport.height
        } else {
            current.y
        }
        .clamp(Dp::ZERO, max.y);
        if (next_y - current.y).abs() <= 0.01 {
            return false;
        }
        self.set_smooth_scroll_target(region.id, Point::new(current.x, next_y));
        true
    }
}

fn next_selected_keys<VM>(
    state: &ListItemState<VM>,
    anchor_key: Option<&WidgetKey>,
    extend: bool,
    toggle: bool,
) -> Vec<WidgetKey> {
    match state.selection_mode {
        ListSelectionMode::None => state.selected_keys.resolve(),
        ListSelectionMode::Single => vec![state.key.clone()],
        ListSelectionMode::Multiple if extend => {
            let anchor = anchor_key.unwrap_or(&state.key);
            merge_selected_range(state, range_keys(state, anchor, &state.key))
        }
        ListSelectionMode::Multiple if toggle => {
            let mut keys = state.selected_keys.resolve();
            if let Some(index) = keys.iter().position(|key| key == &state.key) {
                keys.remove(index);
            } else {
                keys.push(state.key.clone());
            }
            keys
        }
        ListSelectionMode::Multiple => vec![state.key.clone()],
    }
}

fn range_keys<VM>(
    state: &ListItemState<VM>,
    anchor: &WidgetKey,
    focused: &WidgetKey,
) -> Vec<WidgetKey> {
    let Some(anchor_index) = state.sibling_keys.iter().position(|key| key == anchor) else {
        return vec![focused.clone()];
    };
    let Some(focused_index) = state.sibling_keys.iter().position(|key| key == focused) else {
        return vec![focused.clone()];
    };
    let start = anchor_index.min(focused_index);
    let end = anchor_index.max(focused_index);
    state
        .sibling_keys
        .iter()
        .enumerate()
        .skip(start)
        .take(end - start + 1)
        .filter_map(|(index, key)| {
            (!state.sibling_disabled.get(index).copied().unwrap_or(false)).then(|| key.clone())
        })
        .collect()
}

fn merge_selected_range<VM>(state: &ListItemState<VM>, range: Vec<WidgetKey>) -> Vec<WidgetKey> {
    let mut selected = state.selected_keys.resolve();
    for key in range {
        if !selected.iter().any(|selected_key| selected_key == &key) {
            selected.push(key);
        }
    }
    state
        .sibling_keys
        .iter()
        .filter(|key| selected.iter().any(|selected_key| selected_key == *key))
        .cloned()
        .collect()
}

fn row_extent_for_state<VM>(state: &ListItemState<VM>) -> Dp {
    state.item_extent.max(1.0)
}
