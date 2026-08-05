use super::*;
use crate::ui::widget::{
    DataGridCellAction, DataGridCellEditCommit, DataGridCellState, DataGridColumnReorderEvent,
    DataGridColumnWidthChange, DataGridHeaderState, DataGridResizeHandleState,
    DataGridSelectionChange, DataGridSelectionMode, DataGridSelectionTrigger, DataGridSort,
    DataGridSortChange, DataGridSortDirection, DataGridSortTrigger, ScrollRegion, WidgetKey,
};
use smallvec::SmallVec;

struct DataGridKeyboardTarget<VM> {
    id: WidgetId,
    state: DataGridCellState<VM>,
    focus: Option<crate::ui::widget::FocusTargetMeta<VM>>,
}

impl<VM> Clone for DataGridKeyboardTarget<VM> {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            state: self.state.clone(),
            focus: self.focus.clone(),
        }
    }
}

impl<VM: 'static> BoundRuntimeHandler<VM> {
    pub(in crate::runtime) fn dispatch_data_grid_cell_click(
        &mut self,
        state: &DataGridCellState<VM>,
        widget_id: WidgetId,
        now: Instant,
        button: CanvasMouseButton,
    ) -> bool {
        if button != CanvasMouseButton::Left || state.disabled.resolve() || !state.is_actionable() {
            return false;
        }
        let target_id = HoverTargetId::Widget(widget_id);
        let is_double_click = self.pending_click_matches_target(target_id, now);
        if is_double_click {
            self.pending_click = None;
            if state.can_edit() {
                return self.dispatch_data_grid_edit_commit(state);
            }
            return self.dispatch_data_grid_cell_action(state);
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
        self.dispatch_data_grid_selection(state, DataGridSelectionTrigger::Click, extend, toggle)
            || state.can_edit()
            || state.can_act()
    }

    pub(in crate::runtime) fn dispatch_data_grid_accessibility_click(
        &mut self,
        state: &DataGridCellState<VM>,
    ) -> bool {
        if state.disabled.resolve() {
            return false;
        }
        if self.dispatch_data_grid_selection(state, DataGridSelectionTrigger::Click, false, false) {
            return true;
        }
        if state.can_edit() && self.dispatch_data_grid_edit_commit(state) {
            return true;
        }
        self.dispatch_data_grid_cell_action(state)
    }

    pub(in crate::runtime) fn dispatch_data_grid_header_click(
        &mut self,
        state: &DataGridHeaderState<VM>,
        button: CanvasMouseButton,
    ) -> bool {
        if button != CanvasMouseButton::Left {
            return false;
        }
        self.dispatch_data_grid_header_sort(state, self.modifiers.shift_key())
    }

    pub(in crate::runtime) fn dispatch_data_grid_header_accessibility_click(
        &mut self,
        state: &DataGridHeaderState<VM>,
    ) -> bool {
        self.dispatch_data_grid_header_sort(state, false)
    }

    fn dispatch_data_grid_header_sort(
        &mut self,
        state: &DataGridHeaderState<VM>,
        extend: bool,
    ) -> bool {
        if !state.sortable {
            return false;
        }
        let Some(command) = state.on_sort_change.as_ref() else {
            return false;
        };
        let next = next_sort_state(state.sort.resolve(), &state.column_key, extend);
        self.execute_value_command(
            command,
            DataGridSortChange {
                sort: next,
                changed_column: state.column_key.clone(),
                trigger: DataGridSortTrigger::HeaderClick,
            },
        );
        true
    }

    pub(in crate::runtime) fn dispatch_data_grid_resize_click(
        &mut self,
        state: &DataGridResizeHandleState<VM>,
        button: CanvasMouseButton,
    ) -> bool {
        if button != CanvasMouseButton::Left {
            return false;
        }
        let Some(command) = state.on_column_width_change.as_ref() else {
            return false;
        };
        self.execute_value_command(
            command,
            DataGridColumnWidthChange {
                column_key: state.column_key.clone(),
                width: state.width.clamp(
                    state.min_width,
                    state.max_width.unwrap_or(Dp::new(f32::MAX)),
                ),
            },
        );
        true
    }

    pub(in crate::runtime) fn begin_data_grid_column_resize(
        &mut self,
        state: &DataGridResizeHandleState<VM>,
        button: CanvasMouseButton,
    ) -> bool {
        if button != CanvasMouseButton::Left {
            return false;
        }
        let Some(start_position) = self.cursor_position else {
            return false;
        };
        self.active_data_grid_column_resize = Some(super::super::ActiveDataGridColumnResize {
            column_key: state.column_key.clone(),
            start_position,
            start_width: state.width,
            min_width: state.min_width,
            max_width: state.max_width,
            current_width: state.width,
            on_change: state.on_column_width_change.clone(),
        });
        self.invalidate_scene_with_reason("begin_data_grid_column_resize");
        true
    }

    pub(super) fn handle_data_grid_column_resize(&mut self) -> bool {
        let Some(position) = self.cursor_position else {
            return false;
        };
        let Some(active) = self.active_data_grid_column_resize.as_ref() else {
            return false;
        };
        let column_key = active.column_key.clone();
        let start_position = active.start_position;
        let start_width = active.start_width;
        let min_width = active.min_width;
        let max_width = active.max_width.unwrap_or(Dp::new(f32::MAX));
        let current_width = active.current_width;
        let on_change = active.on_change.clone();
        let next_width =
            (start_width + (position.x - start_position.x)).clamp(min_width, max_width);
        if (next_width - current_width).abs() <= Dp::new(0.5) {
            return false;
        }
        if let Some(command) = on_change.as_ref() {
            self.execute_value_command(
                command,
                DataGridColumnWidthChange {
                    column_key,
                    width: next_width,
                },
            );
        }
        if let Some(active) = self.active_data_grid_column_resize.as_mut() {
            active.current_width = next_width;
        }
        self.invalidate_scene_with_reason("data_grid_column_resize");
        true
    }

    pub(super) fn end_data_grid_column_resize(&mut self) -> bool {
        if self.active_data_grid_column_resize.take().is_none() {
            return false;
        }
        self.invalidate_scene_with_reason("end_data_grid_column_resize");
        true
    }

    pub(in crate::runtime) fn begin_data_grid_column_reorder(
        &mut self,
        state: &DataGridHeaderState<VM>,
        button: CanvasMouseButton,
    ) -> bool {
        if button != CanvasMouseButton::Left || !state.reorderable {
            return false;
        }
        self.active_data_grid_column_reorder = Some(super::super::ActiveDataGridColumnReorder {
            grid_id: state.grid_id,
            from_index: state.column_index,
            column_key: state.column_key.clone(),
            on_reorder: state.on_column_reorder.clone(),
        });
        true
    }

    pub(super) fn finish_data_grid_column_reorder(&mut self) -> bool {
        let Some(active) = self.active_data_grid_column_reorder.take() else {
            return false;
        };
        let target = self
            .hit_path(self.viewport_rect())
            .into_iter()
            .rev()
            .find_map(|interaction| match interaction {
                HitInteraction::DataGridHeader { state, .. } if state.grid_id == active.grid_id => {
                    Some((state.column_index, state.column_key))
                }
                _ => None,
            });
        let Some((to_index, target_key)) = target else {
            return false;
        };
        if to_index == active.from_index {
            return false;
        }
        let Some(command) = active.on_reorder else {
            return false;
        };
        self.execute_value_command(
            &command,
            DataGridColumnReorderEvent {
                from_index: active.from_index,
                to_index,
                column_key: active.column_key,
                target_key,
            },
        );
        true
    }

    fn dispatch_data_grid_selection(
        &mut self,
        state: &DataGridCellState<VM>,
        trigger: DataGridSelectionTrigger,
        extend: bool,
        toggle: bool,
    ) -> bool {
        if state.disabled.resolve() || state.selection_mode == DataGridSelectionMode::None {
            return false;
        }
        let Some(command) = state.on_selection_change.as_ref() else {
            return false;
        };
        let anchor = if extend {
            self.data_grid_anchor_states
                .get(&state.grid_id)
                .cloned()
                .unwrap_or_else(|| state.row_key.clone())
        } else {
            state.row_key.clone()
        };
        if !extend {
            self.data_grid_anchor_states
                .insert(state.grid_id, state.row_key.clone());
        }
        self.data_grid_focus_state = Some((
            state.grid_id,
            state.row_key.clone(),
            state.column_key.clone(),
        ));
        let selected_keys = next_selected_keys(state, &anchor, extend, toggle);
        self.execute_value_command(
            command,
            DataGridSelectionChange {
                selected_keys,
                focused_key: Some(state.row_key.clone()),
                anchor_key: Some(anchor),
                changed_key: Some(state.row_key.clone()),
                trigger,
            },
        );
        true
    }

    fn dispatch_data_grid_cell_action(&mut self, state: &DataGridCellState<VM>) -> bool {
        if state.disabled.resolve() {
            return false;
        }
        let Some(command) = state.on_cell_action.as_ref() else {
            return false;
        };
        self.execute_value_command(
            command,
            DataGridCellAction {
                row_index: state.row_index,
                row_key: state.row_key.clone(),
                column_index: state.column_index,
                column_key: state.column_key.clone(),
            },
        );
        true
    }

    fn dispatch_data_grid_edit_commit(&mut self, state: &DataGridCellState<VM>) -> bool {
        if state.disabled.resolve() {
            return false;
        }
        let Some(command) = state.on_cell_edit_commit.as_ref() else {
            return false;
        };
        self.execute_value_command(
            command,
            DataGridCellEditCommit {
                row_index: state.row_index,
                row_key: state.row_key.clone(),
                column_index: state.column_index,
                column_key: state.column_key.clone(),
                value: state.edit_value.clone(),
            },
        );
        true
    }

    fn focused_data_grid_target(&mut self) -> Option<DataGridKeyboardTarget<VM>> {
        let focused_id = self.focused_widget_id()?;
        let computed = self.computed_scene();
        computed
            .hit_regions
            .iter()
            .chain(computed.overlay_hit_regions.iter())
            .find_map(|region| match &region.interaction {
                HitInteraction::DataGridCell { id, state, .. } if *id == focused_id => {
                    Some(DataGridKeyboardTarget {
                        id: *id,
                        state: state.clone(),
                        focus: region.focus.clone(),
                    })
                }
                _ => None,
            })
    }

    fn data_grid_targets(
        &mut self,
        grid_id: WidgetId,
    ) -> SmallVec<[DataGridKeyboardTarget<VM>; 32]> {
        let computed = self.computed_scene();
        let mut targets = computed
            .hit_regions
            .iter()
            .chain(computed.overlay_hit_regions.iter())
            .filter_map(|region| match &region.interaction {
                HitInteraction::DataGridCell { id, state, .. }
                    if state.grid_id == grid_id && !state.disabled.resolve() =>
                {
                    Some(DataGridKeyboardTarget {
                        id: *id,
                        state: state.clone(),
                        focus: region.focus.clone(),
                    })
                }
                _ => None,
            })
            .collect::<SmallVec<[_; 32]>>();
        targets.sort_by_key(|target| (target.state.virtual_row_index, target.state.column_index));
        targets
    }

    pub(super) fn focused_data_grid_cell_is_some(&mut self) -> bool {
        self.focused_data_grid_target().is_some()
    }

    fn focused_data_grid_resize_handle(&mut self) -> Option<DataGridResizeHandleState<VM>> {
        let focused_id = self.focused_widget_id()?;
        let computed = self.computed_scene();
        computed
            .hit_regions
            .iter()
            .chain(computed.overlay_hit_regions.iter())
            .find_map(|region| match &region.interaction {
                HitInteraction::DataGridResizeHandle { id, state, .. } if *id == focused_id => {
                    Some(state.clone())
                }
                _ => None,
            })
    }

    pub(super) fn adjust_focused_data_grid_resize(&mut self, direction: i32) -> bool {
        if direction == 0 {
            return false;
        }
        let Some(state) = self.focused_data_grid_resize_handle() else {
            return false;
        };
        let Some(command) = state.on_column_width_change.as_ref() else {
            return false;
        };
        let max_width = state.max_width.unwrap_or(Dp::new(f32::MAX));
        let width = Dp::new(
            (state.width.get() + state.step.get() * direction as f32)
                .clamp(state.min_width.get(), max_width.get()),
        );
        if (width - state.width).abs() <= Dp::new(0.01) {
            return false;
        }
        self.execute_value_command(
            command,
            DataGridColumnWidthChange {
                column_key: state.column_key,
                width,
            },
        );
        true
    }

    pub(super) fn activate_focused_data_grid_cell(&mut self, enter: bool, space: bool) -> bool {
        let Some(target) = self.focused_data_grid_target() else {
            return false;
        };
        if enter {
            if target.state.can_edit() && self.dispatch_data_grid_edit_commit(&target.state) {
                return true;
            }
            return self.dispatch_data_grid_cell_action(&target.state);
        }
        if space {
            return self.dispatch_data_grid_selection(
                &target.state,
                DataGridSelectionTrigger::Keyboard,
                false,
                true,
            );
        }
        false
    }

    pub(super) fn move_focused_data_grid_cell(
        &mut self,
        row_delta: i32,
        column_delta: i32,
        extend: bool,
    ) -> bool {
        let Some(current) = self.focused_data_grid_target() else {
            return false;
        };
        let targets = self.data_grid_targets(current.state.grid_id);
        let current_row = current.state.virtual_row_index as i32;
        let current_column = current.state.column_index as i32;
        let target_row = (current_row + row_delta).max(0) as usize;
        let target_column = (current_column + column_delta).max(0) as usize;
        if let Some(target) = targets
            .iter()
            .find(|target| {
                target.state.virtual_row_index == target_row
                    && target.state.column_index == target_column
            })
            .cloned()
        {
            if target.id == current.id {
                return false;
            }
            return self.focus_data_grid_target(target, extend);
        }

        // Horizontal movement must never fall through to a cell on another
        // row. Only vertical movement can require a virtual-window update.
        if row_delta == 0 {
            return false;
        }
        let scan_limit = self.data_grid_virtual_navigation_scan_limit(&current.state);
        let Some((target_key, target_virtual_row)) =
            next_data_grid_virtual_target(&current.state, row_delta, scan_limit)
        else {
            return false;
        };
        if let Some(target) = targets.into_iter().find(|target| {
            target.state.row_key == target_key && target.state.column_index == target_column
        }) {
            return self.focus_data_grid_target(target, extend);
        }
        if !self.materialize_data_grid_virtual_row(&current.state, target_virtual_row) {
            return false;
        }
        let target = self
            .data_grid_targets(current.state.grid_id)
            .into_iter()
            .find(|target| {
                target.state.row_key == target_key && target.state.column_index == target_column
            });
        let Some(target) = target else {
            return false;
        };
        self.focus_data_grid_target(target, extend)
    }

    pub(super) fn move_focused_data_grid_cell_to_edge(&mut self, end: bool, extend: bool) -> bool {
        let Some(current) = self.focused_data_grid_target() else {
            return false;
        };
        let scan_limit = self.data_grid_virtual_navigation_scan_limit(&current.state);
        let Some((_, target_key, target_virtual_row)) =
            data_grid_edge_virtual_target(&current.state, end, scan_limit)
        else {
            return true;
        };
        if target_key == current.state.row_key {
            return false;
        }
        self.focus_data_grid_logical_target(
            &current.state,
            target_key,
            target_virtual_row,
            current.state.column_index,
            extend,
        )
    }

    pub(super) fn page_focused_data_grid_cell(&mut self, direction: i32, extend: bool) -> bool {
        let Some(current) = self.focused_data_grid_target() else {
            return false;
        };
        if direction == 0 || current.state.selection.sibling_keys.is_empty() {
            return false;
        }
        let Some(region) = self.scroll_region_for_data_grid(current.state.scroll_container_id)
        else {
            return false;
        };
        let fallback_extent = current.state.item_extent.max(1.0);
        let spacing = current.state.item_spacing.max(Dp::ZERO);
        let (current_top, _) =
            self.data_grid_virtual_row_bounds(&current.state, current.state.virtual_row_index);
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
            let (candidate_top, _) =
                self.data_grid_virtual_row_bounds(&current.state, target_virtual_row);
            if candidate_top + Dp::new(0.01) < target_offset {
                target_virtual_row = target_virtual_row.saturating_add(1).min(max_virtual_row);
            }
        }
        let scan_limit = self.data_grid_virtual_navigation_scan_limit(&current.state);
        let Some((target_index, target_key, target_virtual_row)) = data_grid_page_virtual_target(
            &current.state,
            target_virtual_row,
            direction,
            scan_limit,
        ) else {
            return if direction < 0 {
                current.state.row_index > 0
            } else {
                current.state.row_index + 1 < current.state.selection.sibling_keys.len()
            };
        };
        if target_index == current.state.row_index {
            return false;
        }
        let _ = self.materialize_data_grid_virtual_page(&current.state, direction);
        self.focus_data_grid_logical_target(
            &current.state,
            target_key,
            target_virtual_row,
            current.state.column_index,
            extend,
        )
    }

    fn focus_data_grid_target(&mut self, target: DataGridKeyboardTarget<VM>, extend: bool) -> bool {
        let Some(focus) = target.focus.clone() else {
            return false;
        };
        self.update_focus(
            Some(FocusedWidget {
                widget_id: target.id,
                scope_path: focus.scope_path,
                on_blur: focus.on_blur.clone(),
            }),
            focus.on_focus,
            true,
        );
        self.data_grid_focus_state = Some((
            target.state.grid_id,
            target.state.row_key.clone(),
            target.state.column_key.clone(),
        ));
        self.ensure_data_grid_row_visible(&target.state);
        let _ = self.dispatch_data_grid_selection(
            &target.state,
            DataGridSelectionTrigger::Keyboard,
            extend,
            false,
        );
        true
    }

    fn scroll_region_for_data_grid(&mut self, widget_id: WidgetId) -> Option<ScrollRegion> {
        self.cached_scene
            .as_ref()?
            .computed
            .scroll_regions
            .iter()
            .copied()
            .find(|region| region.id == widget_id)
    }

    fn data_grid_virtual_navigation_scan_limit(&mut self, state: &DataGridCellState<VM>) -> usize {
        self.scroll_region_for_data_grid(state.scroll_container_id)
            .map(|region| {
                (region.content_viewport.height / state.item_extent.max(1.0))
                    .ceil()
                    .max(1.0) as usize
                    + 4
            })
            .unwrap_or(16)
    }

    fn data_grid_virtual_row_bounds(
        &self,
        state: &DataGridCellState<VM>,
        virtual_row_index: usize,
    ) -> (Dp, Dp) {
        let fallback_extent = state.item_extent.max(1.0);
        let spacing = state.item_spacing.max(Dp::ZERO);
        self.virtual_states
            .get(&state.scroll_container_id)
            .map(|cache| cache.item_main_bounds(virtual_row_index, fallback_extent, spacing))
            .unwrap_or_else(|| {
                let top = (fallback_extent + spacing) * virtual_row_index as f32;
                (top, top + fallback_extent)
            })
    }

    fn ensure_data_grid_row_visible(&mut self, state: &DataGridCellState<VM>) -> bool {
        let Some(region) = self.scroll_region_for_data_grid(state.scroll_container_id) else {
            return false;
        };
        let current = self.effective_scroll_offset(region.id, region.scroll_offset);
        let (row_top, row_bottom) =
            self.data_grid_virtual_row_bounds(state, state.virtual_row_index);
        let next_y = if row_top < current.y {
            row_top
        } else if row_bottom > current.y + region.content_viewport.height {
            row_bottom - region.content_viewport.height
        } else {
            current.y
        }
        .clamp(Dp::ZERO, region.max_offset().y);
        if (next_y - current.y).abs() <= 0.01 {
            return false;
        }
        self.set_smooth_scroll_target(region.id, Point::new(current.x, next_y));
        true
    }

    fn materialize_data_grid_virtual_row(
        &mut self,
        state: &DataGridCellState<VM>,
        virtual_row_index: usize,
    ) -> bool {
        let Some(region) = self.scroll_region_for_data_grid(state.scroll_container_id) else {
            return false;
        };
        let current = self.effective_scroll_offset(region.id, region.scroll_offset);
        let (row_top, row_bottom) = self.data_grid_virtual_row_bounds(state, virtual_row_index);
        let next_y = if row_top < current.y {
            row_top
        } else if row_bottom > current.y + region.content_viewport.height {
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

    fn materialize_data_grid_virtual_page(
        &mut self,
        state: &DataGridCellState<VM>,
        direction: i32,
    ) -> bool {
        let Some(region) = self.scroll_region_for_data_grid(state.scroll_container_id) else {
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

    fn focus_data_grid_logical_target(
        &mut self,
        state: &DataGridCellState<VM>,
        target_key: WidgetKey,
        target_virtual_row: usize,
        target_column: usize,
        extend: bool,
    ) -> bool {
        if let Some(target) = self
            .data_grid_targets(state.grid_id)
            .into_iter()
            .find(|target| {
                target.state.row_key == target_key && target.state.column_index == target_column
            })
        {
            return self.focus_data_grid_target(target, extend);
        }
        if !self.materialize_data_grid_virtual_row(state, target_virtual_row) {
            return false;
        }
        let target = self
            .data_grid_targets(state.grid_id)
            .into_iter()
            .find(|target| {
                target.state.row_key == target_key && target.state.column_index == target_column
            });
        let Some(target) = target else {
            return false;
        };
        self.focus_data_grid_target(target, extend)
    }
}

fn next_data_grid_virtual_target<VM>(
    state: &DataGridCellState<VM>,
    row_delta: i32,
    scan_limit: usize,
) -> Option<(WidgetKey, usize)> {
    if row_delta == 0 {
        return None;
    }
    let mut index = state.row_index;
    let mut remaining = row_delta.unsigned_abs() as usize;
    for _ in 0..scan_limit {
        index = if row_delta < 0 {
            index.checked_sub(1)?
        } else {
            index.checked_add(1)?
        };
        let key = state.selection.sibling_keys.get(index)?;
        let virtual_row_index = *state.selection.sibling_virtual_row_indices.get(index)?;
        if state
            .selection
            .sibling_disabled
            .get(index)
            .map(|disabled| disabled.resolve())
            .unwrap_or(false)
        {
            continue;
        }
        remaining = remaining.saturating_sub(1);
        if remaining == 0 {
            return Some((key.clone(), virtual_row_index));
        }
    }
    None
}

fn data_grid_edge_virtual_target<VM>(
    state: &DataGridCellState<VM>,
    end: bool,
    scan_limit: usize,
) -> Option<(usize, WidgetKey, usize)> {
    let len = state.selection.sibling_keys.len();
    for offset in 0..scan_limit.min(len) {
        let index = if end { len - 1 - offset } else { offset };
        if data_grid_row_disabled(state, index) {
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

fn data_grid_page_virtual_target<VM>(
    state: &DataGridCellState<VM>,
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
        .min(state.row_index.saturating_sub(1))
    } else {
        insertion
            .min(virtual_rows.len() - 1)
            .max(state.row_index.saturating_add(1))
    };
    for _ in 0..scan_limit {
        if !data_grid_row_disabled(state, index) {
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

fn data_grid_row_disabled<VM>(state: &DataGridCellState<VM>, index: usize) -> bool {
    state
        .selection
        .sibling_disabled
        .get(index)
        .map(|disabled| disabled.resolve())
        .unwrap_or(false)
}

fn next_sort_state(
    current: Vec<DataGridSort>,
    column_key: &WidgetKey,
    extend: bool,
) -> Vec<DataGridSort> {
    let current_direction = current
        .iter()
        .find(|entry| &entry.column_key == column_key)
        .map(|entry| entry.direction);
    let next_direction = current_direction
        .map(DataGridSortDirection::toggled)
        .unwrap_or(DataGridSortDirection::Ascending);
    if extend {
        let mut next = current;
        if let Some(entry) = next
            .iter_mut()
            .find(|entry| &entry.column_key == column_key)
        {
            entry.direction = next_direction;
        } else {
            next.push(DataGridSort {
                column_key: column_key.clone(),
                direction: next_direction,
            });
        }
        next
    } else {
        vec![DataGridSort {
            column_key: column_key.clone(),
            direction: next_direction,
        }]
    }
}

fn next_selected_keys<VM>(
    state: &DataGridCellState<VM>,
    anchor: &WidgetKey,
    extend: bool,
    toggle: bool,
) -> Vec<WidgetKey> {
    match state.selection_mode {
        DataGridSelectionMode::None => state
            .selection
            .selected_keys
            .resolve_ref(|keys| keys.to_vec()),
        DataGridSelectionMode::Single => vec![state.row_key.clone()],
        DataGridSelectionMode::Multiple if extend => range_keys(state, anchor, &state.row_key),
        DataGridSelectionMode::Multiple if toggle => {
            let mut keys = state
                .selection
                .selected_keys
                .resolve_ref(|keys| keys.to_vec());
            if let Some(index) = keys.iter().position(|key| key == &state.row_key) {
                keys.remove(index);
            } else {
                keys.push(state.row_key.clone());
            }
            keys
        }
        DataGridSelectionMode::Multiple => vec![state.row_key.clone()],
    }
}

fn range_keys<VM>(
    state: &DataGridCellState<VM>,
    anchor: &WidgetKey,
    focused: &WidgetKey,
) -> Vec<WidgetKey> {
    let focused_index = state
        .selection
        .sibling_keys
        .iter()
        .position(|key| key == focused)
        .unwrap_or(state.row_index);
    let anchor_index = state
        .selection
        .sibling_keys
        .iter()
        .position(|candidate| candidate == anchor)
        .unwrap_or(focused_index);
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
