use super::*;
use crate::ui::widget::{
    ScrollRegion, TreeCheckChange, TreeCheckState, TreeCheckTrigger, TreeDropEvent,
    TreeDropPosition, TreeExpandChange, TreeExpandTrigger, TreeNodeAction, TreeNodeState,
    TreeSelectionChange, TreeSelectionMode, TreeSelectionTrigger, WidgetKey,
};

struct TreeKeyboardTarget<VM> {
    id: WidgetId,
    state: TreeNodeState<VM>,
    focus: Option<crate::ui::widget::FocusTargetMeta<VM>>,
}

impl<VM> Clone for TreeKeyboardTarget<VM> {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            state: self.state.clone(),
            focus: self.focus.clone(),
        }
    }
}

impl<VM: 'static> BoundRuntimeHandler<VM> {
    pub(in crate::runtime) fn dispatch_tree_node_click(
        &mut self,
        state: &TreeNodeState<VM>,
        widget_id: WidgetId,
        now: Instant,
        button: CanvasMouseButton,
        pointer_position: Option<Point>,
    ) -> bool {
        if button != CanvasMouseButton::Left || self.gesture_consumes_click() {
            return false;
        }
        if state.disabled.resolve() {
            return false;
        }
        self.remember_tree_focus(state);

        if self.tree_pointer_hits_disclosure(state, widget_id, pointer_position) {
            self.pending_click = None;
            return self.dispatch_tree_expand(state, TreeExpandTrigger::Click, !state.expanded);
        }
        if self.tree_pointer_hits_checkbox(state, widget_id, pointer_position) {
            self.pending_click = None;
            return self.dispatch_tree_check(state, TreeCheckTrigger::Click);
        }

        self.begin_tree_drag(state, button);

        let target_id = HoverTargetId::Widget(widget_id);
        let is_double_click = self.pending_click_matches_target(target_id, now);
        if is_double_click {
            self.pending_click = None;
            return self.dispatch_tree_node_action(state);
        }
        self.pending_click = Some(PendingClick {
            target_id,
            deadline: now + super::super::DOUBLE_CLICK_THRESHOLD,
            position: self.cursor_position.unwrap_or(Point::ZERO),
            command: None,
        });

        let extend = self.modifiers.shift_key();
        let toggle = is_primary_shortcut_modifier(self.modifiers);
        self.dispatch_tree_selection(state, TreeSelectionTrigger::Click, extend, toggle)
    }

    pub(in crate::runtime) fn dispatch_tree_keyboard_selection(
        &mut self,
        state: &TreeNodeState<VM>,
        toggle: bool,
    ) -> bool {
        self.dispatch_tree_selection(state, TreeSelectionTrigger::Keyboard, false, toggle)
    }

    pub(in crate::runtime) fn dispatch_tree_disclosure_click(
        &mut self,
        state: &TreeNodeState<VM>,
        button: CanvasMouseButton,
    ) -> bool {
        if button != CanvasMouseButton::Left || self.gesture_consumes_click() {
            return false;
        }
        self.pending_click = None;
        self.remember_tree_focus(state);
        self.dispatch_tree_expand(state, TreeExpandTrigger::Click, !state.expanded)
    }

    pub(in crate::runtime) fn dispatch_tree_checkbox_click(
        &mut self,
        state: &TreeNodeState<VM>,
        button: CanvasMouseButton,
    ) -> bool {
        if button != CanvasMouseButton::Left || self.gesture_consumes_click() {
            return false;
        }
        self.pending_click = None;
        self.remember_tree_focus(state);
        self.dispatch_tree_check(state, TreeCheckTrigger::Click)
    }

    fn dispatch_tree_selection(
        &mut self,
        state: &TreeNodeState<VM>,
        trigger: TreeSelectionTrigger,
        extend: bool,
        toggle: bool,
    ) -> bool {
        if state.disabled.resolve() || state.selection_mode == TreeSelectionMode::None {
            return false;
        }
        let selected_keys = next_tree_selected_keys(
            state,
            self.tree_anchor_states.get(&state.tree_id),
            extend,
            toggle,
        );
        let anchor_key = self.update_tree_anchor(state, extend);
        self.tree_focus_state = Some((state.tree_id, state.key.clone()));
        let Some(command) = state.on_selection_change.as_ref() else {
            return false;
        };
        self.execute_value_command(
            command,
            TreeSelectionChange {
                selected_keys,
                focused_key: Some(state.key.clone()),
                anchor_key,
                changed_key: Some(state.key.clone()),
                trigger,
            },
        );
        true
    }

    fn update_tree_anchor(&mut self, state: &TreeNodeState<VM>, extend: bool) -> Option<WidgetKey> {
        let next = if extend {
            self.tree_anchor_states
                .get(&state.tree_id)
                .cloned()
                .unwrap_or_else(|| state.key.clone())
        } else {
            state.key.clone()
        };
        self.tree_anchor_states.insert(state.tree_id, next.clone());
        Some(next)
    }

    fn dispatch_tree_expand(
        &mut self,
        state: &TreeNodeState<VM>,
        trigger: TreeExpandTrigger,
        expanded: bool,
    ) -> bool {
        if state.disabled.resolve() || !state.has_children || state.expanded == expanded {
            return false;
        }
        let mut expanded_keys = state.expanded_keys.resolve();
        if expanded {
            if !expanded_keys.iter().any(|key| key == &state.key) {
                expanded_keys.push(state.key.clone());
            }
        } else {
            expanded_keys.retain(|key| key != &state.key);
        }
        let Some(command) = state.on_expand_change.as_ref() else {
            return false;
        };
        self.execute_value_command(
            command,
            TreeExpandChange {
                expanded_keys,
                key: state.key.clone(),
                expanded,
                trigger,
            },
        );
        true
    }

    fn dispatch_tree_check(
        &mut self,
        state: &TreeNodeState<VM>,
        trigger: TreeCheckTrigger,
    ) -> bool {
        if state.disabled.resolve() || !state.checkable.resolve() {
            return false;
        }
        let affected_keys = state.check_target_keys.to_vec();
        if affected_keys.is_empty() {
            return false;
        }
        let current = state.checked_keys.resolve();
        let will_check = state.check_state != TreeCheckState::Checked;
        let mut checked_keys = current
            .into_iter()
            .filter(|key| !affected_keys.iter().any(|candidate| candidate == key))
            .collect::<Vec<_>>();
        if will_check {
            for key in affected_keys.iter() {
                if !checked_keys.iter().any(|candidate| candidate == key) {
                    checked_keys.push(key.clone());
                }
            }
        }
        let check_state =
            crate::ui::widget::tree_check_state(&state.check_target_keys, &checked_keys);
        let Some(command) = state.on_check_change.as_ref() else {
            return false;
        };
        self.execute_value_command(
            command,
            TreeCheckChange {
                checked_keys,
                key: state.key.clone(),
                checked: will_check,
                check_state,
                affected_keys,
                trigger,
            },
        );
        true
    }

    fn dispatch_tree_node_action(&mut self, state: &TreeNodeState<VM>) -> bool {
        if state.disabled.resolve() {
            return false;
        }
        let Some(command) = state.on_node_action.as_ref() else {
            return false;
        };
        self.execute_value_command(
            command,
            TreeNodeAction {
                index: state.node_index,
                key: state.key.clone(),
            },
        );
        true
    }

    fn begin_tree_drag(&mut self, state: &TreeNodeState<VM>, button: CanvasMouseButton) -> bool {
        if button != CanvasMouseButton::Left
            || state.disabled.resolve()
            || !state.draggable
            || state.on_drop.is_none()
        {
            return false;
        }
        self.active_tree_drag = Some(super::super::ActiveTreeDrag {
            tree_id: state.tree_id,
            dragged_key: state.key.clone(),
            descendant_keys: state.descendant_keys.clone(),
            on_drop: state.on_drop.clone(),
        });
        true
    }

    pub(super) fn finish_tree_drop(&mut self) -> bool {
        let Some(active) = self.active_tree_drag.take() else {
            return false;
        };
        let Some(position) = self.cursor_position else {
            return false;
        };
        let target = self.tree_drop_target_at(position, active.tree_id);
        let Some((target_key, target_rect)) = target else {
            return false;
        };
        if target_key == active.dragged_key
            || active.descendant_keys.iter().any(|key| key == &target_key)
        {
            return false;
        }
        let Some(command) = active.on_drop else {
            return false;
        };
        let local_y = position.y - target_rect.y;
        let third = target_rect.height / 3.0;
        let position = if local_y < third {
            TreeDropPosition::Before
        } else if local_y > third * 2.0 {
            TreeDropPosition::After
        } else {
            TreeDropPosition::Inside
        };
        self.execute_value_command(
            &command,
            TreeDropEvent {
                dragged_key: active.dragged_key,
                target_key,
                position,
            },
        );
        true
    }

    fn tree_drop_target_at(
        &mut self,
        position: Point,
        tree_id: WidgetId,
    ) -> Option<(WidgetKey, Rect)> {
        let computed = self.computed_scene();
        computed
            .hit_regions
            .iter()
            .chain(computed.overlay_hit_regions.iter())
            .rev()
            .find_map(|region| match &region.interaction {
                HitInteraction::TreeNode { state, .. }
                    if state.tree_id == tree_id
                        && !state.disabled.resolve()
                        && region.rect.contains(position) =>
                {
                    Some((state.key.clone(), region.rect))
                }
                _ => None,
            })
    }

    fn focused_tree_target(&mut self) -> Option<TreeKeyboardTarget<VM>> {
        let focused_id = self.focused_widget_id()?;
        if let Some(target) = self.tree_target_by_widget_id(focused_id) {
            self.remember_tree_focus(&target.state);
            return Some(target);
        }
        if self.focused_widget_is_tree_root(focused_id) {
            return self.remembered_tree_target();
        }
        None
    }

    pub(super) fn focused_tree_node_is_some(&mut self) -> bool {
        self.focused_tree_target().is_some()
    }

    fn tree_target_by_widget_id(&mut self, widget_id: WidgetId) -> Option<TreeKeyboardTarget<VM>> {
        let computed = self.computed_scene();
        computed
            .hit_regions
            .iter()
            .chain(computed.overlay_hit_regions.iter())
            .find_map(|region| match &region.interaction {
                HitInteraction::TreeNode { id, state, .. } if *id == widget_id => {
                    Some(TreeKeyboardTarget {
                        id: *id,
                        state: state.clone(),
                        focus: region.focus.clone(),
                    })
                }
                _ => None,
            })
    }

    fn focused_widget_is_tree_root(&mut self, widget_id: WidgetId) -> bool {
        let computed = self.computed_scene();
        computed
            .hit_regions
            .iter()
            .chain(computed.overlay_hit_regions.iter())
            .any(|region| {
                matches!(
                    &region.interaction,
                    HitInteraction::TreeNode { state, .. } if state.tree_id == widget_id
                )
            })
    }

    fn remembered_tree_target(&mut self) -> Option<TreeKeyboardTarget<VM>> {
        let (tree_id, key) = self.tree_focus_state.clone()?;
        let computed = self.computed_scene();
        computed
            .hit_regions
            .iter()
            .chain(computed.overlay_hit_regions.iter())
            .find_map(|region| match &region.interaction {
                HitInteraction::TreeNode { id, state, .. }
                    if state.tree_id == tree_id
                        && state.key == key
                        && !state.disabled.resolve() =>
                {
                    Some(TreeKeyboardTarget {
                        id: *id,
                        state: state.clone(),
                        focus: region.focus.clone(),
                    })
                }
                _ => None,
            })
    }

    fn tree_targets(&mut self, tree_id: WidgetId) -> Vec<TreeKeyboardTarget<VM>> {
        let computed = self.computed_scene();
        let mut targets = computed
            .hit_regions
            .iter()
            .chain(computed.overlay_hit_regions.iter())
            .filter_map(|region| match &region.interaction {
                HitInteraction::TreeNode { id, state, .. }
                    if state.tree_id == tree_id && !state.disabled.resolve() =>
                {
                    Some(TreeKeyboardTarget {
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

    pub(super) fn activate_focused_tree_node(&mut self, enter: bool, space: bool) -> bool {
        let Some(target) = self.focused_tree_target() else {
            return false;
        };
        if enter {
            return self.dispatch_tree_node_action(&target.state);
        }
        if space {
            if target.state.checkable.resolve() {
                return self.dispatch_tree_check(&target.state, TreeCheckTrigger::Keyboard);
            }
            return self.dispatch_tree_keyboard_selection(&target.state, true);
        }
        false
    }

    pub(super) fn move_focused_tree_node(&mut self, step: i32, extend: bool) -> bool {
        let Some(current) = self.focused_tree_target() else {
            return false;
        };
        let targets = self.tree_targets(current.state.tree_id);
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
        self.focus_tree_target(targets[next_position].clone(), extend)
    }

    pub(super) fn enter_focused_tree_root(&mut self, end: bool, extend: bool) -> bool {
        let Some(tree_id) = self.focused_widget_id() else {
            return false;
        };
        let targets = self.tree_targets(tree_id);
        let Some(target) = (if end { targets.last() } else { targets.first() }) else {
            return false;
        };
        self.focus_tree_target(target.clone(), extend)
    }

    pub(super) fn move_focused_tree_node_to_edge(&mut self, end: bool, extend: bool) -> bool {
        let Some(current) = self.focused_tree_target() else {
            return false;
        };
        let targets = self.tree_targets(current.state.tree_id);
        let Some(target) = (if end { targets.last() } else { targets.first() }) else {
            return false;
        };
        if target.id == current.id {
            return false;
        }
        self.focus_tree_target(target.clone(), extend)
    }

    pub(super) fn page_focused_tree_node(&mut self, direction: i32, extend: bool) -> bool {
        let Some(current) = self.focused_tree_target() else {
            return false;
        };
        let targets = self.tree_targets(current.state.tree_id);
        if targets.len() <= 1 {
            return false;
        }
        let Some(current_position) = targets.iter().position(|target| target.id == current.id)
        else {
            return false;
        };
        let page = self
            .scroll_region_for_tree(current.state.tree_id)
            .map(|region| {
                let row_extent = row_extent_for_tree_state(&current.state);
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
        self.focus_tree_target(targets[next_position].clone(), extend)
    }

    pub(super) fn collapse_or_focus_parent_tree_node(&mut self) -> bool {
        let Some(current) = self.focused_tree_target() else {
            return false;
        };
        if current.state.has_children
            && current.state.expanded
            && self.dispatch_tree_expand(&current.state, TreeExpandTrigger::Keyboard, false)
        {
            return true;
        }
        let Some(parent_key) = current.state.parent_key.clone() else {
            return false;
        };
        let target = self
            .tree_targets(current.state.tree_id)
            .into_iter()
            .find(|target| target.state.key == parent_key);
        let Some(target) = target else {
            return false;
        };
        self.focus_tree_target(target, false)
    }

    pub(super) fn expand_or_focus_child_tree_node(&mut self) -> bool {
        let Some(current) = self.focused_tree_target() else {
            return false;
        };
        if current.state.has_children
            && !current.state.expanded
            && self.dispatch_tree_expand(&current.state, TreeExpandTrigger::Keyboard, true)
        {
            return true;
        }
        let first_child = current.state.child_keys.first().cloned();
        let Some(first_child) = first_child else {
            return false;
        };
        let target = self
            .tree_targets(current.state.tree_id)
            .into_iter()
            .find(|target| target.state.key == first_child);
        let Some(target) = target else {
            return false;
        };
        self.focus_tree_target(target, false)
    }

    fn focus_tree_target(&mut self, target: TreeKeyboardTarget<VM>, extend: bool) -> bool {
        let Some(focus) = target.focus.clone() else {
            return false;
        };
        self.remember_tree_focus(&target.state);
        self.update_focus(
            Some(FocusedWidget {
                widget_id: target.id,
                scope_path: focus.scope_path,
                on_blur: focus.on_blur.clone(),
            }),
            focus.on_focus,
            true,
        );
        self.ensure_tree_row_visible(&target.state);
        let _ = self.dispatch_tree_selection(
            &target.state,
            TreeSelectionTrigger::Keyboard,
            extend,
            false,
        );
        true
    }

    fn remember_tree_focus(&mut self, state: &TreeNodeState<VM>) {
        self.tree_focus_state = Some((state.tree_id, state.key.clone()));
    }

    fn scroll_region_for_tree(&mut self, widget_id: WidgetId) -> Option<ScrollRegion> {
        self.scroll_regions()
            .into_iter()
            .find(|region| region.id == widget_id)
    }

    fn ensure_tree_row_visible(&mut self, state: &TreeNodeState<VM>) -> bool {
        let Some(region) = self.scroll_region_for_tree(state.tree_id) else {
            return false;
        };
        let current = self.effective_scroll_offset(region.id, region.scroll_offset);
        let row_extent = row_extent_for_tree_state(state);
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

    fn tree_pointer_hits_disclosure(
        &mut self,
        state: &TreeNodeState<VM>,
        widget_id: WidgetId,
        pointer_position: Option<Point>,
    ) -> bool {
        if !state.has_children {
            return false;
        }
        let Some(local_x) = self.tree_local_pointer_x(widget_id, pointer_position) else {
            return false;
        };
        let start = state.item_padding.left + state.indent_width * state.depth as f32;
        local_x >= start && local_x <= start + state.disclosure_width
    }

    fn tree_pointer_hits_checkbox(
        &mut self,
        state: &TreeNodeState<VM>,
        widget_id: WidgetId,
        pointer_position: Option<Point>,
    ) -> bool {
        if !state.checkable.resolve() {
            return false;
        }
        let Some(local_x) = self.tree_local_pointer_x(widget_id, pointer_position) else {
            return false;
        };
        let start = state.item_padding.left
            + state.indent_width * state.depth as f32
            + state.disclosure_width;
        local_x >= start && local_x <= start + state.checkbox_width
    }

    fn tree_local_pointer_x(
        &mut self,
        widget_id: WidgetId,
        pointer_position: Option<Point>,
    ) -> Option<Dp> {
        let position = pointer_position.or(self.cursor_position)?;
        let computed = self.computed_scene();
        computed
            .hit_regions
            .iter()
            .chain(computed.overlay_hit_regions.iter())
            .find_map(|region| match &region.interaction {
                HitInteraction::TreeNode { id, .. } if *id == widget_id => {
                    Some(position.x - region.rect.x)
                }
                _ => None,
            })
    }
}

fn next_tree_selected_keys<VM>(
    state: &TreeNodeState<VM>,
    anchor_key: Option<&WidgetKey>,
    extend: bool,
    toggle: bool,
) -> Vec<WidgetKey> {
    match state.selection_mode {
        TreeSelectionMode::None => state.selected_keys.resolve(),
        TreeSelectionMode::Single => vec![state.key.clone()],
        TreeSelectionMode::Multiple if extend => {
            let anchor = anchor_key.unwrap_or(&state.key);
            merge_tree_selected_range(state, tree_range_keys(state, anchor, &state.key))
        }
        TreeSelectionMode::Multiple if toggle => {
            let mut keys = state.selected_keys.resolve();
            if let Some(index) = keys.iter().position(|key| key == &state.key) {
                keys.remove(index);
            } else {
                keys.push(state.key.clone());
            }
            keys
        }
        TreeSelectionMode::Multiple => vec![state.key.clone()],
    }
}

fn tree_range_keys<VM>(
    state: &TreeNodeState<VM>,
    anchor: &WidgetKey,
    focused: &WidgetKey,
) -> Vec<WidgetKey> {
    let Some(anchor_index) = state.visible_keys.iter().position(|key| key == anchor) else {
        return vec![focused.clone()];
    };
    let Some(focused_index) = state.visible_keys.iter().position(|key| key == focused) else {
        return vec![focused.clone()];
    };
    let start = anchor_index.min(focused_index);
    let end = anchor_index.max(focused_index);
    state
        .visible_keys
        .iter()
        .enumerate()
        .skip(start)
        .take(end - start + 1)
        .filter_map(|(index, key)| {
            (!state.visible_disabled.get(index).copied().unwrap_or(false)).then(|| key.clone())
        })
        .collect()
}

fn merge_tree_selected_range<VM>(
    state: &TreeNodeState<VM>,
    range: Vec<WidgetKey>,
) -> Vec<WidgetKey> {
    let mut selected = state.selected_keys.resolve();
    for key in range {
        if !selected.iter().any(|selected_key| selected_key == &key) {
            selected.push(key);
        }
    }
    state
        .visible_keys
        .iter()
        .filter(|key| selected.iter().any(|selected_key| selected_key == *key))
        .cloned()
        .collect()
}

fn row_extent_for_tree_state<VM>(state: &TreeNodeState<VM>) -> Dp {
    state.item_extent.max(1.0)
}
