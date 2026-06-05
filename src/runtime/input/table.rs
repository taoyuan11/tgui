use super::*;
use crate::ui::widget::{
    DataGridCellAction, DataGridCellEditCommit, DataGridCellState, DataGridColumnReorderEvent,
    DataGridColumnWidthChange, DataGridHeaderState, DataGridResizeHandleState,
    DataGridSelectionChange, DataGridSelectionMode, DataGridSelectionTrigger, DataGridSort,
    DataGridSortChange, DataGridSortDirection, DataGridSortTrigger, WidgetKey,
};

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
        if button != CanvasMouseButton::Left || state.disabled.resolve() {
            return false;
        }
        let target_id = HoverTargetId::Widget(widget_id);
        let is_double_click = self.pending_click_matches_target(target_id, now);
        if is_double_click {
            self.pending_click = None;
            if state.editable {
                return self.dispatch_data_grid_edit_commit(state);
            }
            return self.dispatch_data_grid_cell_action(state);
        }
        self.pending_click = Some(PendingClick {
            target_id,
            deadline: now + super::super::DOUBLE_CLICK_THRESHOLD,
            position: self.cursor_position.unwrap_or(Point::ZERO),
            command: None,
        });
        let extend = self.modifiers.shift_key();
        let toggle = is_primary_shortcut_modifier(self.modifiers);
        self.dispatch_data_grid_selection(state, DataGridSelectionTrigger::Click, extend, toggle)
    }

    pub(in crate::runtime) fn dispatch_data_grid_header_click(
        &mut self,
        state: &DataGridHeaderState<VM>,
        button: CanvasMouseButton,
    ) -> bool {
        if button != CanvasMouseButton::Left || !state.sortable {
            return false;
        }
        let Some(command) = state.on_sort_change.as_ref() else {
            return false;
        };
        let next = next_sort_state(
            state.sort.resolve(),
            &state.column_key,
            self.modifiers.shift_key(),
        );
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
        if state.selection_mode == DataGridSelectionMode::None {
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

    fn data_grid_targets(&mut self, grid_id: WidgetId) -> Vec<DataGridKeyboardTarget<VM>> {
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
            .collect::<Vec<_>>();
        targets.sort_by_key(|target| (target.state.virtual_row_index, target.state.column_index));
        targets
    }

    pub(super) fn focused_data_grid_cell_is_some(&mut self) -> bool {
        self.focused_data_grid_target().is_some()
    }

    pub(super) fn activate_focused_data_grid_cell(&mut self, enter: bool, space: bool) -> bool {
        let Some(target) = self.focused_data_grid_target() else {
            return false;
        };
        if enter {
            if target.state.editable && self.dispatch_data_grid_edit_commit(&target.state) {
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
        let Some(current_position) = targets.iter().position(|target| target.id == current.id)
        else {
            return false;
        };
        let current_row = current.state.virtual_row_index as i32;
        let current_column = current.state.column_index as i32;
        let target_row = (current_row + row_delta).max(0) as usize;
        let target_column = (current_column + column_delta).max(0) as usize;
        let target = targets
            .iter()
            .find(|target| {
                target.state.virtual_row_index == target_row
                    && target.state.column_index == target_column
            })
            .or_else(|| {
                if row_delta != 0 || column_delta != 0 {
                    let next = if row_delta < 0 || column_delta < 0 {
                        current_position.saturating_sub(1)
                    } else {
                        (current_position + 1).min(targets.len().saturating_sub(1))
                    };
                    targets.get(next)
                } else {
                    None
                }
            });
        let Some(target) = target.cloned() else {
            return false;
        };
        if target.id == current.id {
            return false;
        }
        self.focus_data_grid_target(target, extend)
    }

    pub(super) fn move_focused_data_grid_cell_to_edge(&mut self, end: bool, extend: bool) -> bool {
        let Some(current) = self.focused_data_grid_target() else {
            return false;
        };
        let targets = self.data_grid_targets(current.state.grid_id);
        let Some(target) = (if end { targets.last() } else { targets.first() }) else {
            return false;
        };
        if target.id == current.id {
            return false;
        }
        self.focus_data_grid_target(target.clone(), extend)
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
        let _ = self.dispatch_data_grid_selection(
            &target.state,
            DataGridSelectionTrigger::Keyboard,
            extend,
            false,
        );
        true
    }
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
        DataGridSelectionMode::None => state.selected_keys.resolve(),
        DataGridSelectionMode::Single => vec![state.row_key.clone()],
        DataGridSelectionMode::Multiple if extend => range_keys(state, anchor, &state.row_key),
        DataGridSelectionMode::Multiple if toggle => {
            let mut keys = state.selected_keys.resolve();
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
        .sibling_keys
        .iter()
        .position(|key| key == focused)
        .unwrap_or(state.row_index);
    let anchor_index = state
        .sibling_keys
        .iter()
        .position(|candidate| candidate == anchor)
        .unwrap_or(focused_index);
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
