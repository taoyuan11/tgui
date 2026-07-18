use super::*;
use crate::ui::widget::{
    ListItemAction, ListItemState, ListSelectionChange, ListSelectionMode, ListSelectionTrigger,
    ScrollRegion, WidgetKey,
};
use smallvec::SmallVec;
use std::collections::HashSet;

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

    pub(in crate::runtime) fn dispatch_list_item_accessibility_click(
        &mut self,
        state: &ListItemState<VM>,
    ) -> bool {
        if state.disabled.resolve() {
            return false;
        }
        self.remember_list_focus(state);
        let _ = self.dispatch_list_item_selection(state, ListSelectionTrigger::Click, false, false);
        true
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

    fn list_targets(&mut self, list_id: WidgetId) -> SmallVec<[ListKeyboardTarget<VM>; 16]> {
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
            .collect::<SmallVec<[_; 16]>>();
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
        let Some(current_position) = targets.iter().position(|target| target.id == current.id)
        else {
            return false;
        };
        let next_position = if step < 0 {
            current_position.saturating_sub(1)
        } else {
            (current_position + 1).min(targets.len() - 1)
        };
        if next_position != current_position {
            return self.focus_list_target(targets[next_position].clone(), extend);
        }

        // The current hit is at the edge of the materialized virtual window.
        // Resolve only a viewport-sized slice of live disabled metadata, then
        // synchronously move the virtual window and look up the real hit/focus
        // metadata. This keeps the boundary path O(visible), never O(source).
        let scan_limit = self.list_virtual_navigation_scan_limit(&current.state);
        let Some((target_key, target_virtual_row)) =
            next_list_virtual_target(&current.state, step, scan_limit)
        else {
            return false;
        };
        if !self.materialize_list_virtual_row(&current.state, target_virtual_row) {
            return false;
        }
        let target = self
            .list_targets(current.state.list_id)
            .into_iter()
            .find(|target| target.state.key == target_key);
        let Some(target) = target else {
            return false;
        };
        self.focus_list_target(target, extend)
    }

    pub(super) fn enter_focused_list_root(&mut self, end: bool, extend: bool) -> bool {
        let Some(list_id) = self.focused_widget_id() else {
            return false;
        };
        let targets = self.list_targets(list_id);
        let Some(seed) = targets.first() else {
            return false;
        };
        let scan_limit = self.list_virtual_navigation_scan_limit(&seed.state);
        let Some((_, target_key, target_virtual_row)) =
            list_edge_virtual_target(&seed.state, end, scan_limit)
        else {
            return true;
        };
        let _ = self.materialize_list_virtual_edge(&seed.state, end);
        self.focus_list_logical_target(&seed.state, target_key, target_virtual_row, extend)
    }

    pub(super) fn move_focused_list_item_to_edge(&mut self, end: bool, extend: bool) -> bool {
        let Some(current) = self.focused_list_target() else {
            return false;
        };
        let scan_limit = self.list_virtual_navigation_scan_limit(&current.state);
        let Some((_, target_key, target_virtual_row)) =
            list_edge_virtual_target(&current.state, end, scan_limit)
        else {
            return true;
        };
        if target_key == current.state.key {
            return false;
        }
        let _ = self.materialize_list_virtual_edge(&current.state, end);
        self.focus_list_logical_target(&current.state, target_key, target_virtual_row, extend)
    }

    pub(super) fn page_focused_list_item(&mut self, direction: i32, extend: bool) -> bool {
        let Some(current) = self.focused_list_target() else {
            return false;
        };
        if direction == 0 {
            return false;
        }
        let Some(region) = self.scroll_region_for_widget(current.state.list_id) else {
            return false;
        };
        let fallback_extent = row_extent_for_state(&current.state);
        let spacing = current.state.item_spacing.max(Dp::ZERO);
        let (current_top, _) = self.list_virtual_row_bounds(
            &current.state,
            current.state.row_index,
            fallback_extent,
            spacing,
        );
        let target_offset = if direction < 0 {
            (current_top - region.content_viewport.height).max(Dp::ZERO)
        } else {
            current_top + region.content_viewport.height
        };
        let Some(&max_virtual_row) = current.state.selection.sibling_virtual_row_indices.last()
        else {
            return false;
        };
        let mut target_virtual_row = self
            .virtual_states
            .get(&region.id)
            .map(|cache| {
                cache.item_index_at_main_offset(
                    target_offset,
                    fallback_extent,
                    spacing,
                    max_virtual_row,
                )
            })
            .unwrap_or_else(|| {
                let step = fallback_extent + spacing;
                if step <= Dp::ZERO {
                    0
                } else {
                    ((target_offset / step).floor() as usize).min(max_virtual_row)
                }
            });
        if direction > 0 {
            let (candidate_top, _) = self.list_virtual_row_bounds(
                &current.state,
                target_virtual_row,
                fallback_extent,
                spacing,
            );
            if candidate_top + Dp::new(0.01) < target_offset {
                target_virtual_row = target_virtual_row.saturating_add(1).min(max_virtual_row);
            }
        }
        let scan_limit = self.list_virtual_navigation_scan_limit(&current.state);
        let Some((target_index, target_key, target_virtual_row)) =
            list_page_virtual_target(&current.state, target_virtual_row, direction, scan_limit)
        else {
            return if direction < 0 {
                current.state.item_index > 0
            } else {
                current.state.item_index + 1 < current.state.selection.sibling_keys.len()
            };
        };
        if target_index == current.state.item_index {
            return false;
        }
        let _ = self.materialize_list_virtual_page(&current.state, direction);
        self.focus_list_logical_target(&current.state, target_key, target_virtual_row, extend)
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
        // CRITICAL: Use cached scroll_regions to avoid stack overflow
        let scroll_regions = self
            .cached_scene
            .as_ref()?
            .computed
            .scroll_regions
            .as_slice();
        scroll_regions
            .iter()
            .copied()
            .find(|region| region.id == widget_id)
    }

    fn ensure_list_row_visible(&mut self, state: &ListItemState<VM>) -> bool {
        let Some(region) = self.scroll_region_for_widget(state.list_id) else {
            return false;
        };
        let current = self.effective_scroll_offset(region.id, region.scroll_offset);
        let row_extent = row_extent_for_state(state);
        let spacing = state.item_spacing.max(Dp::ZERO);
        let (row_top, row_bottom) =
            self.list_virtual_row_bounds(state, state.row_index, row_extent, spacing);
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

    fn list_virtual_navigation_scan_limit(&mut self, state: &ListItemState<VM>) -> usize {
        self.scroll_region_for_widget(state.list_id)
            .map(|region| {
                (region.content_viewport.height / row_extent_for_state(state))
                    .ceil()
                    .max(1.0) as usize
                    + 4
            })
            .unwrap_or(16)
    }

    fn materialize_list_virtual_row(
        &mut self,
        state: &ListItemState<VM>,
        virtual_row_index: usize,
    ) -> bool {
        let Some(region) = self.scroll_region_for_widget(state.list_id) else {
            return false;
        };
        let current = self.effective_scroll_offset(region.id, region.scroll_offset);
        let fallback_extent = row_extent_for_state(state);
        let spacing = state.item_spacing.max(Dp::ZERO);
        let (row_top, row_bottom) =
            self.list_virtual_row_bounds(state, virtual_row_index, fallback_extent, spacing);
        let viewport_bottom = current.y + region.content_viewport.height;
        let next_y = if row_top < current.y {
            row_top
        } else if row_bottom > viewport_bottom {
            row_bottom - region.content_viewport.height
        } else {
            current.y
        }
        .clamp(Dp::ZERO, region.max_offset().y);
        if (next_y - current.y).abs() <= 0.01 {
            return false;
        }
        self.cancel_scroll_motion(region.id);
        self.set_scroll_offset(region.id, Point::new(current.x, next_y));
        let _ = self.computed_scene();
        true
    }

    fn list_virtual_row_bounds(
        &self,
        state: &ListItemState<VM>,
        virtual_row_index: usize,
        fallback_extent: Dp,
        spacing: Dp,
    ) -> (Dp, Dp) {
        self.virtual_states
            .get(&state.list_id)
            .map(|cache| cache.item_main_bounds(virtual_row_index, fallback_extent, spacing))
            .unwrap_or_else(|| {
                let top = (fallback_extent + spacing) * virtual_row_index as f32;
                (top, top + fallback_extent)
            })
    }

    fn materialize_list_virtual_page(&mut self, state: &ListItemState<VM>, direction: i32) -> bool {
        let Some(region) = self.scroll_region_for_widget(state.list_id) else {
            return false;
        };
        let current = self.effective_scroll_offset(region.id, region.scroll_offset);
        let delta = region.content_viewport.height * direction.signum() as f32;
        let next_y = (current.y + delta).clamp(Dp::ZERO, region.max_offset().y);
        if (next_y - current.y).abs() <= 0.01 {
            return false;
        }
        self.cancel_scroll_motion(region.id);
        self.set_scroll_offset(region.id, Point::new(current.x, next_y));
        let _ = self.computed_scene();
        true
    }

    fn materialize_list_virtual_edge(&mut self, state: &ListItemState<VM>, end: bool) -> bool {
        let Some(region) = self.scroll_region_for_widget(state.list_id) else {
            return false;
        };
        let current = self.effective_scroll_offset(region.id, region.scroll_offset);
        let next_y = if end { region.max_offset().y } else { Dp::ZERO };
        if (next_y - current.y).abs() <= 0.01 {
            return false;
        }
        self.cancel_scroll_motion(region.id);
        self.set_scroll_offset(region.id, Point::new(current.x, next_y));
        let _ = self.computed_scene();
        true
    }

    fn focus_list_logical_target(
        &mut self,
        state: &ListItemState<VM>,
        target_key: WidgetKey,
        target_virtual_row: usize,
        extend: bool,
    ) -> bool {
        if let Some(target) = self
            .list_targets(state.list_id)
            .into_iter()
            .find(|target| target.state.key == target_key)
        {
            return self.focus_list_target(target, extend);
        }
        if !self.materialize_list_virtual_row(state, target_virtual_row) {
            return false;
        }
        let target = self
            .list_targets(state.list_id)
            .into_iter()
            .find(|target| target.state.key == target_key);
        let Some(target) = target else {
            return false;
        };
        self.focus_list_target(target, extend)
    }
}

fn next_list_virtual_target<VM>(
    state: &ListItemState<VM>,
    step: i32,
    scan_limit: usize,
) -> Option<(WidgetKey, usize)> {
    if step == 0 {
        return None;
    }
    let mut index = state.item_index;
    for _ in 0..scan_limit {
        index = if step < 0 {
            index.checked_sub(1)?
        } else {
            index.checked_add(1)?
        };
        let key = state.selection.sibling_keys.get(index)?;
        let virtual_row_index = *state.selection.sibling_virtual_row_indices.get(index)?;
        if !state
            .selection
            .sibling_disabled
            .get(index)
            .map(|disabled| disabled.resolve())
            .unwrap_or(false)
        {
            return Some((key.clone(), virtual_row_index));
        }
    }
    None
}

fn list_edge_virtual_target<VM>(
    state: &ListItemState<VM>,
    end: bool,
    scan_limit: usize,
) -> Option<(usize, WidgetKey, usize)> {
    let len = state.selection.sibling_keys.len();
    for offset in 0..scan_limit.min(len) {
        let index = if end { len - 1 - offset } else { offset };
        if list_item_disabled(state, index) {
            continue;
        }
        return Some((
            index,
            state.selection.sibling_keys.get(index)?.clone(),
            *state.selection.sibling_virtual_row_indices.get(index)?,
        ));
    }
    None
}

fn list_page_virtual_target<VM>(
    state: &ListItemState<VM>,
    target_virtual_row: usize,
    direction: i32,
    scan_limit: usize,
) -> Option<(usize, WidgetKey, usize)> {
    let virtual_rows = state.selection.sibling_virtual_row_indices.as_ref();
    if virtual_rows.is_empty() || direction == 0 {
        return None;
    }
    let insertion = virtual_rows.partition_point(|row| *row < target_virtual_row);
    let mut index = if direction < 0 {
        if insertion < virtual_rows.len() && virtual_rows[insertion] == target_virtual_row {
            insertion
        } else {
            insertion.checked_sub(1)?
        }
        .min(state.item_index.saturating_sub(1))
    } else {
        insertion
            .min(virtual_rows.len() - 1)
            .max(state.item_index.saturating_add(1))
    };
    for _ in 0..scan_limit {
        if !list_item_disabled(state, index) {
            return Some((
                index,
                state.selection.sibling_keys.get(index)?.clone(),
                *virtual_rows.get(index)?,
            ));
        }
        index = if direction < 0 {
            match index.checked_sub(1) {
                Some(index) => index,
                None => break,
            }
        } else {
            match index
                .checked_add(1)
                .filter(|index| *index < virtual_rows.len())
            {
                Some(index) => index,
                None => break,
            }
        };
    }
    None
}

fn list_item_disabled<VM>(state: &ListItemState<VM>, index: usize) -> bool {
    state
        .selection
        .sibling_disabled
        .get(index)
        .map(|disabled| disabled.resolve())
        .unwrap_or(false)
}

fn next_selected_keys<VM>(
    state: &ListItemState<VM>,
    anchor_key: Option<&WidgetKey>,
    extend: bool,
    toggle: bool,
) -> Vec<WidgetKey> {
    match state.selection_mode {
        ListSelectionMode::None => state
            .selection
            .selected_keys
            .resolve_ref(|keys| keys.to_vec()),
        ListSelectionMode::Single => vec![state.key.clone()],
        ListSelectionMode::Multiple if extend => {
            let anchor = anchor_key.unwrap_or(&state.key);
            merge_selected_range(state, range_keys(state, anchor, &state.key))
        }
        ListSelectionMode::Multiple if toggle => {
            let mut keys = state
                .selection
                .selected_keys
                .resolve_ref(|keys| keys.to_vec());
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
    let Some(&anchor_index) = state.selection.sibling_index_by_key.get(anchor) else {
        return vec![focused.clone()];
    };
    let Some(&focused_index) = state.selection.sibling_index_by_key.get(focused) else {
        return vec![focused.clone()];
    };
    let start = anchor_index.min(focused_index);
    let end = anchor_index.max(focused_index);
    state
        .selection
        .sibling_keys
        .iter()
        .enumerate()
        .skip(start)
        .take(end - start + 1)
        .filter_map(|(index, key)| {
            (!state
                .selection
                .sibling_disabled
                .get(index)
                .map(|disabled| disabled.resolve())
                .unwrap_or(false))
            .then(|| key.clone())
        })
        .collect()
}

fn merge_selected_range<VM>(state: &ListItemState<VM>, range: Vec<WidgetKey>) -> Vec<WidgetKey> {
    let mut selected = state
        .selection
        .selected_keys
        .resolve_ref(|keys| keys.iter().cloned().collect::<HashSet<_>>());
    selected.extend(range);
    state
        .selection
        .sibling_keys
        .iter()
        .filter(|key| selected.contains(*key))
        .cloned()
        .collect()
}

fn row_extent_for_state<VM>(state: &ListItemState<VM>) -> Dp {
    state.item_extent.max(1.0)
}
