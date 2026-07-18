use super::*;
use crate::ui::widget::{
    ScrollRegion, TreeCheckChange, TreeCheckState, TreeCheckTrigger, TreeDropEvent,
    TreeDropPosition, TreeExpandChange, TreeExpandTrigger, TreeNodeAction, TreeNodeState,
    TreeSelectionChange, TreeSelectionMode, TreeSelectionTrigger, WidgetKey,
};
use smallvec::SmallVec;
use std::collections::HashSet;

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
            splitter: None,
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

    pub(in crate::runtime) fn dispatch_tree_accessibility_click(
        &mut self,
        state: &TreeNodeState<VM>,
    ) -> bool {
        if state.disabled.resolve() {
            return false;
        }
        self.remember_tree_focus(state);
        let _ = self.dispatch_tree_selection(state, TreeSelectionTrigger::Click, false, false);
        true
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
        let snapshot = state.controlled_keys.expanded.resolve();
        let mut expanded_keys = snapshot.ordered.to_vec();
        if expanded {
            if !snapshot.membership.contains(&state.key) {
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
        let current = state
            .controlled_keys
            .checked
            .resolve_ref(|snapshot| snapshot.ordered.to_vec());
        let will_check = state.check_state != TreeCheckState::Checked;
        let affected_membership = affected_keys.iter().cloned().collect::<HashSet<_>>();
        let mut checked_keys = current
            .into_iter()
            .filter(|key| !affected_membership.contains(key))
            .collect::<Vec<_>>();
        if will_check {
            let mut output_membership = checked_keys.iter().cloned().collect::<HashSet<_>>();
            for key in &affected_keys {
                if output_membership.insert(key.clone()) {
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

    fn tree_targets(&mut self, tree_id: WidgetId) -> SmallVec<[TreeKeyboardTarget<VM>; 16]> {
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
            .collect::<SmallVec<[_; 16]>>();
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
            return self.focus_tree_target(targets[next_position].clone(), extend);
        }

        let scan_limit = self.tree_virtual_navigation_scan_limit(&current.state);
        let Some((target_key, target_virtual_row)) =
            next_tree_virtual_target(&current.state, step, scan_limit)
        else {
            return false;
        };
        if !self.materialize_tree_virtual_row(&current.state, target_virtual_row) {
            return false;
        }
        let target = self
            .tree_targets(current.state.tree_id)
            .into_iter()
            .find(|target| target.state.key == target_key);
        let Some(target) = target else {
            return false;
        };
        self.focus_tree_target(target, extend)
    }

    pub(super) fn enter_focused_tree_root(&mut self, end: bool, extend: bool) -> bool {
        let Some(tree_id) = self.focused_widget_id() else {
            return false;
        };
        let targets = self.tree_targets(tree_id);
        let Some(seed) = targets.first() else {
            return false;
        };
        let scan_limit = self.tree_virtual_navigation_scan_limit(&seed.state);
        let Some((_, target_key, target_virtual_row)) =
            tree_edge_virtual_target(&seed.state, end, scan_limit)
        else {
            return true;
        };
        self.focus_tree_logical_target(&seed.state, target_key, target_virtual_row, extend)
    }

    pub(super) fn move_focused_tree_node_to_edge(&mut self, end: bool, extend: bool) -> bool {
        let Some(current) = self.focused_tree_target() else {
            return false;
        };
        let scan_limit = self.tree_virtual_navigation_scan_limit(&current.state);
        let Some((_, target_key, target_virtual_row)) =
            tree_edge_virtual_target(&current.state, end, scan_limit)
        else {
            return true;
        };
        if target_key == current.state.key {
            return false;
        }
        self.focus_tree_logical_target(&current.state, target_key, target_virtual_row, extend)
    }

    pub(super) fn page_focused_tree_node(&mut self, direction: i32, extend: bool) -> bool {
        let Some(current) = self.focused_tree_target() else {
            return false;
        };
        if direction == 0 || current.state.visible_keys.is_empty() {
            return false;
        }
        let Some(region) = self.scroll_region_for_tree(current.state.tree_id) else {
            return false;
        };
        let fallback_extent = row_extent_for_tree_state(&current.state);
        let spacing = current.state.item_spacing.max(Dp::ZERO);
        let (current_top, _) = self.tree_virtual_row_bounds(
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
        let max_virtual_row = current.state.visible_keys.len() - 1;
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
            let (candidate_top, _) = self.tree_virtual_row_bounds(
                &current.state,
                target_virtual_row,
                fallback_extent,
                spacing,
            );
            if candidate_top + Dp::new(0.01) < target_offset {
                target_virtual_row = target_virtual_row.saturating_add(1).min(max_virtual_row);
            }
        }
        let scan_limit = self.tree_virtual_navigation_scan_limit(&current.state);
        let Some((target_index, target_key, target_virtual_row)) =
            tree_page_virtual_target(&current.state, target_virtual_row, direction, scan_limit)
        else {
            return if direction < 0 {
                current.state.row_index > 0
            } else {
                current.state.row_index + 1 < current.state.visible_keys.len()
            };
        };
        if target_index == current.state.row_index {
            return false;
        }
        let _ = self.materialize_tree_virtual_page(&current.state, direction);
        self.focus_tree_logical_target(&current.state, target_key, target_virtual_row, extend)
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

    fn ensure_tree_row_visible(&mut self, state: &TreeNodeState<VM>) -> bool {
        let Some(region) = self.scroll_region_for_tree(state.tree_id) else {
            return false;
        };
        let current = self.effective_scroll_offset(region.id, region.scroll_offset);
        let row_extent = row_extent_for_tree_state(state);
        let spacing = state.item_spacing.max(Dp::ZERO);
        let (row_top, row_bottom) =
            self.tree_virtual_row_bounds(state, state.row_index, row_extent, spacing);
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

    fn tree_virtual_navigation_scan_limit(&mut self, state: &TreeNodeState<VM>) -> usize {
        self.scroll_region_for_tree(state.tree_id)
            .map(|region| {
                (region.content_viewport.height / row_extent_for_tree_state(state))
                    .ceil()
                    .max(1.0) as usize
                    + 4
            })
            .unwrap_or(16)
    }

    fn materialize_tree_virtual_row(
        &mut self,
        state: &TreeNodeState<VM>,
        virtual_row_index: usize,
    ) -> bool {
        let Some(region) = self.scroll_region_for_tree(state.tree_id) else {
            return false;
        };
        let current = self.effective_scroll_offset(region.id, region.scroll_offset);
        let fallback_extent = row_extent_for_tree_state(state);
        let spacing = state.item_spacing.max(Dp::ZERO);
        let (row_top, row_bottom) =
            self.tree_virtual_row_bounds(state, virtual_row_index, fallback_extent, spacing);
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

    fn tree_virtual_row_bounds(
        &self,
        state: &TreeNodeState<VM>,
        virtual_row_index: usize,
        fallback_extent: Dp,
        spacing: Dp,
    ) -> (Dp, Dp) {
        self.virtual_states
            .get(&state.tree_id)
            .map(|cache| cache.item_main_bounds(virtual_row_index, fallback_extent, spacing))
            .unwrap_or_else(|| {
                let top = (fallback_extent + spacing) * virtual_row_index as f32;
                (top, top + fallback_extent)
            })
    }

    fn materialize_tree_virtual_page(&mut self, state: &TreeNodeState<VM>, direction: i32) -> bool {
        let Some(region) = self.scroll_region_for_tree(state.tree_id) else {
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

    fn focus_tree_logical_target(
        &mut self,
        state: &TreeNodeState<VM>,
        target_key: WidgetKey,
        target_virtual_row: usize,
        extend: bool,
    ) -> bool {
        if let Some(target) = self
            .tree_targets(state.tree_id)
            .into_iter()
            .find(|target| target.state.key == target_key)
        {
            return self.focus_tree_target(target, extend);
        }
        if !self.materialize_tree_virtual_row(state, target_virtual_row) {
            return false;
        }
        let target = self
            .tree_targets(state.tree_id)
            .into_iter()
            .find(|target| target.state.key == target_key);
        let Some(target) = target else {
            return false;
        };
        self.focus_tree_target(target, extend)
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

fn next_tree_virtual_target<VM>(
    state: &TreeNodeState<VM>,
    step: i32,
    scan_limit: usize,
) -> Option<(WidgetKey, usize)> {
    if step == 0 {
        return None;
    }
    let mut index = state.row_index;
    for _ in 0..scan_limit {
        index = if step < 0 {
            index.checked_sub(1)?
        } else {
            index.checked_add(1)?
        };
        let key = state.visible_keys.get(index)?;
        if !state
            .visible_disabled
            .get(index)
            .map(|disabled| disabled.resolve())
            .unwrap_or(false)
        {
            return Some((key.clone(), index));
        }
    }
    None
}

fn tree_edge_virtual_target<VM>(
    state: &TreeNodeState<VM>,
    end: bool,
    scan_limit: usize,
) -> Option<(usize, WidgetKey, usize)> {
    let len = state.visible_keys.len();
    for offset in 0..scan_limit.min(len) {
        let index = if end { len - 1 - offset } else { offset };
        if tree_node_disabled(state, index) {
            continue;
        }
        return Some((index, state.visible_keys.get(index)?.clone(), index));
    }
    None
}

fn tree_page_virtual_target<VM>(
    state: &TreeNodeState<VM>,
    target_virtual_row: usize,
    direction: i32,
    scan_limit: usize,
) -> Option<(usize, WidgetKey, usize)> {
    let len = state.visible_keys.len();
    if len == 0 || direction == 0 {
        return None;
    }
    let mut index = if direction < 0 {
        target_virtual_row.min(state.row_index.saturating_sub(1))
    } else {
        target_virtual_row
            .min(len - 1)
            .max(state.row_index.saturating_add(1))
    };
    for _ in 0..scan_limit {
        if !tree_node_disabled(state, index) {
            return Some((index, state.visible_keys.get(index)?.clone(), index));
        }
        index = if direction < 0 {
            match index.checked_sub(1) {
                Some(index) => index,
                None => break,
            }
        } else {
            match index.checked_add(1).filter(|index| *index < len) {
                Some(index) => index,
                None => break,
            }
        };
    }
    None
}

fn tree_node_disabled<VM>(state: &TreeNodeState<VM>, index: usize) -> bool {
    state
        .visible_disabled
        .get(index)
        .map(|disabled| disabled.resolve())
        .unwrap_or(false)
}

fn next_tree_selected_keys<VM>(
    state: &TreeNodeState<VM>,
    anchor_key: Option<&WidgetKey>,
    extend: bool,
    toggle: bool,
) -> Vec<WidgetKey> {
    match state.selection_mode {
        TreeSelectionMode::None => state
            .selection
            .selected_keys
            .resolve_ref(|keys| keys.to_vec()),
        TreeSelectionMode::Single => vec![state.key.clone()],
        TreeSelectionMode::Multiple if extend => {
            let anchor = anchor_key.unwrap_or(&state.key);
            merge_tree_selected_range(state, tree_range_keys(state, anchor, &state.key))
        }
        TreeSelectionMode::Multiple if toggle => {
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
            (!state
                .visible_disabled
                .get(index)
                .map(|disabled| disabled.resolve())
                .unwrap_or(false))
            .then(|| key.clone())
        })
        .collect()
}

fn merge_tree_selected_range<VM>(
    state: &TreeNodeState<VM>,
    range: Vec<WidgetKey>,
) -> Vec<WidgetKey> {
    let selected = state.selection.selected_key_membership.resolve();
    let added = range.into_iter().collect::<HashSet<_>>();
    state
        .visible_keys
        .iter()
        .filter(|key| selected.contains(*key) || added.contains(*key))
        .cloned()
        .collect()
}

fn row_extent_for_tree_state<VM>(state: &TreeNodeState<VM>) -> Dp {
    state.item_extent.max(1.0)
}
