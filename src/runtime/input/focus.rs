use super::*;
use crate::ui::widget::ScrollRegion;

struct FocusCandidate<VM> {
    widget_id: WidgetId,
    tab_index: Option<i32>,
    order: usize,
    scope_path: Vec<WidgetId>,
    on_focus: Option<Command<VM>>,
    on_blur: Option<Command<VM>>,
}

impl<VM> Clone for FocusCandidate<VM> {
    fn clone(&self) -> Self {
        Self {
            widget_id: self.widget_id,
            tab_index: self.tab_index,
            order: self.order,
            scope_path: self.scope_path.clone(),
            on_focus: self.on_focus.clone(),
            on_blur: self.on_blur.clone(),
        }
    }
}

#[derive(Clone)]
struct FocusNavigationSnapshot<VM> {
    active_trap_scope: Option<Vec<WidgetId>>,
    active_auto_focus_scope: Option<Vec<WidgetId>>,
    candidates: Vec<FocusCandidate<VM>>,
}

fn scope_path_within(path: &[WidgetId], scope: &[WidgetId]) -> bool {
    path.starts_with(scope)
}

impl<VM> FocusNavigationSnapshot<VM> {
    fn from_scene(computed: &crate::ui::widget::ComputedScene<VM>) -> Self {
        let active_trap_scope = computed
            .focus_scopes
            .iter()
            .rev()
            .find(|scope| scope.active && scope.options.is_trap())
            .map(|scope| scope.path.clone());
        let active_auto_focus_scope = computed
            .focus_scopes
            .iter()
            .rev()
            .find(|scope| scope.active && scope.options.is_auto_focus_first())
            .map(|scope| scope.path.clone());
        let mut candidates = Vec::new();
        let mut seen = HashSet::new();
        for region in computed
            .hit_regions
            .iter()
            .chain(computed.overlay_hit_regions.iter())
        {
            let Some(focus) = region.focus.as_ref() else {
                continue;
            };
            if focus.tab_index.unwrap_or(0) < 0 {
                continue;
            }
            if let Some(trap) = active_trap_scope.as_ref() {
                if !scope_path_within(&focus.scope_path, trap) {
                    continue;
                }
            }
            if !seen.insert(focus.widget_id) {
                continue;
            }
            candidates.push(FocusCandidate {
                widget_id: focus.widget_id,
                tab_index: focus.tab_index,
                order: focus.order,
                scope_path: focus.scope_path.clone(),
                on_focus: focus.on_focus.clone(),
                on_blur: focus.on_blur.clone(),
            });
        }
        candidates.sort_by(|left, right| {
            let left_bucket = left.tab_index.unwrap_or(0);
            let right_bucket = right.tab_index.unwrap_or(0);
            match (left_bucket > 0, right_bucket > 0) {
                (true, true) => left_bucket
                    .cmp(&right_bucket)
                    .then_with(|| left.order.cmp(&right.order)),
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                (false, false) => left.order.cmp(&right.order),
            }
        });
        Self {
            active_trap_scope,
            active_auto_focus_scope,
            candidates,
        }
    }

    fn first_candidate_in_scope(&self, scope: &[WidgetId]) -> Option<FocusCandidate<VM>> {
        self.candidates
            .iter()
            .find(|candidate| scope_path_within(&candidate.scope_path, scope))
            .cloned()
    }
}

impl<VM: 'static> BoundRuntimeHandler<VM> {
    pub(super) fn focused_scroll_region(&mut self) -> Option<ScrollRegion> {
        let focused_id = self.focused_widget_id()?;
        self.scroll_regions().into_iter().find(|region| {
            region.id == focused_id && (region.can_scroll_x() || region.can_scroll_y())
        })
    }

    pub(super) fn scroll_focused_region_by_pages(&mut self, direction: i32) -> bool {
        let Some(region) = self.focused_scroll_region() else {
            return false;
        };
        let current = self.effective_scroll_offset(region.id, region.scroll_offset);
        let page_x = (region.content_viewport.width * 0.9).max(Dp::ZERO);
        let page_y = (region.content_viewport.height * 0.9).max(Dp::ZERO);
        let max = region.max_offset();
        let next = Point::new(
            if region.can_scroll_x() {
                (current.x + page_x * direction as f32).clamp(Dp::ZERO, max.x)
            } else {
                current.x
            },
            if region.can_scroll_y() {
                (current.y + page_y * direction as f32).clamp(Dp::ZERO, max.y)
            } else {
                current.y
            },
        );
        if (next.x - current.x).abs() <= 0.01 && (next.y - current.y).abs() <= 0.01 {
            return false;
        }
        self.set_smooth_scroll_target(region.id, next);
        true
    }

    pub(super) fn scroll_focused_region_to_edge(&mut self, end: bool) -> bool {
        let Some(region) = self.focused_scroll_region() else {
            return false;
        };
        let current = self.effective_scroll_offset(region.id, region.scroll_offset);
        let max = region.max_offset();
        let next = Point::new(
            if region.can_scroll_x() {
                if end {
                    max.x
                } else {
                    Dp::ZERO
                }
            } else {
                current.x
            },
            if region.can_scroll_y() {
                if end {
                    max.y
                } else {
                    Dp::ZERO
                }
            } else {
                current.y
            },
        );
        if (next.x - current.x).abs() <= 0.01 && (next.y - current.y).abs() <= 0.01 {
            return false;
        }
        self.set_smooth_scroll_target(region.id, next);
        true
    }

    pub(super) fn active_focus_trap_scope(&mut self) -> Option<Vec<WidgetId>> {
        FocusNavigationSnapshot::from_scene(self.computed_scene()).active_trap_scope
    }

    fn focus_candidates(&mut self) -> Vec<FocusCandidate<VM>> {
        FocusNavigationSnapshot::from_scene(self.computed_scene()).candidates
    }

    pub(in crate::runtime) fn reconcile_auto_focus_after_scene_update(&mut self) -> bool {
        let Some(cached) = self.cached_scene.as_ref() else {
            self.active_auto_focus_scope = None;
            return false;
        };
        let snapshot = FocusNavigationSnapshot::from_scene(&cached.computed);
        let next_scope = snapshot.active_auto_focus_scope.clone();
        if self.active_auto_focus_scope == next_scope {
            return false;
        }
        self.active_auto_focus_scope = next_scope.clone();

        let Some(scope) = next_scope else {
            return false;
        };
        let current_focus_in_scope = self
            .focused_widget
            .as_ref()
            .map(|focused| {
                snapshot.candidates.iter().any(|candidate| {
                    candidate.widget_id == focused.widget_id
                        && scope_path_within(&candidate.scope_path, &scope)
                })
            })
            .unwrap_or(false);
        if current_focus_in_scope {
            return false;
        }
        let Some(next) = snapshot.first_candidate_in_scope(&scope) else {
            return false;
        };
        self.update_focus(
            Some(FocusedWidget {
                widget_id: next.widget_id,
                scope_path: next.scope_path,
                on_blur: next.on_blur,
            }),
            next.on_focus,
            true,
        );
        true
    }

    pub(in crate::runtime) fn activate_focused_widget(&mut self, enter: bool, space: bool) -> bool {
        let Some(focused_id) = self.focused_widget_id() else {
            return false;
        };
        let computed = self.computed_scene().clone();
        for region in computed
            .hit_regions
            .iter()
            .chain(computed.overlay_hit_regions.iter())
        {
            let handles_key = match &region.interaction {
                HitInteraction::Widget {
                    id,
                    interactions,
                    default_activation,
                    ..
                } if *id == focused_id => {
                    if (enter && default_activation.handles_enter())
                        || (space && default_activation.handles_space())
                    {
                        if let Some(command) = interactions.on_click.as_ref() {
                            self.execute_command(command);
                            return true;
                        }
                    }
                    false
                }
                HitInteraction::Checkbox {
                    id,
                    on_change,
                    current,
                    ..
                } if *id == focused_id && space => {
                    if let Some(command) = on_change.as_ref() {
                        self.execute_value_command(command, !current);
                        return true;
                    }
                    false
                }
                HitInteraction::Radio {
                    id,
                    on_change,
                    current,
                    ..
                } if *id == focused_id && space => {
                    if let Some(command) = on_change.as_ref() {
                        self.execute_value_command(command, !current);
                        return true;
                    }
                    false
                }
                HitInteraction::Switch {
                    id,
                    on_change,
                    current,
                    ..
                } if *id == focused_id && space => {
                    if let Some(command) = on_change.as_ref() {
                        self.execute_value_command(command, !current);
                        return true;
                    }
                    false
                }
                HitInteraction::SelectTrigger {
                    id,
                    on_open_change,
                    is_open,
                    ..
                } if *id == focused_id && enter => {
                    let next_open = !is_open;
                    self.close_all_open_selects_except(next_open.then_some(*id));
                    let _ = self.set_select_open_state(*id, next_open, on_open_change.as_ref());
                    true
                }
                HitInteraction::TabTrigger {
                    id,
                    key,
                    label,
                    on_change,
                    ..
                } if *id == focused_id && (enter || space) => {
                    if let Some(command) = on_change.as_ref() {
                        self.execute_value_command(command, (key.clone(), label.clone()));
                        return true;
                    }
                    false
                }
                _ => false,
            };
            if handles_key {
                return true;
            }
        }
        false
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

    pub(in crate::runtime) fn selected_text_for_copy(&mut self) -> Option<String> {
        let Some(widget_id) = self.selected_text else {
            return None;
        };
        if let Some(region) = self.sync_text_input_buffer(widget_id) {
            let current_value = self.text_input_current_value(widget_id, &region.controller);
            let (start, end) = self
                .text_edit_state(widget_id)
                .cloned()
                .unwrap_or_else(|| self.default_text_edit_state(widget_id, &current_value))
                .clamped_to(&current_value)
                .selection_range()?;
            return self.text_input_buffers.get(&widget_id).map(|state| {
                RopeBuffer::from_str(state.current_text()).slice_byte_range_to_string(start, end)
            });
        }

        let text = self.selected_text_content(widget_id)?;
        let (start, end) = self
            .text_edit_state(widget_id)
            .cloned()
            .unwrap_or_else(|| self.default_text_edit_state(widget_id, &text))
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
            self.invalidate_text_input_scene();
            return true;
        }
        false
    }

    pub(super) fn begin_text_selection(
        &mut self,
        widget_id: WidgetId,
        input: TextInputSnapshot,
        cursor: usize,
    ) {
        self.selected_text = Some(widget_id);
        self.active_text_selection = Some(TextSelectionDrag {
            widget_id,
            input: input.clone(),
        });
        self.update_text_edit_state(widget_id, &input.text, |state| {
            state.cursor = cursor;
            state.anchor = cursor;
            state.composition = None;
        });
        self.invalidate_text_input_scene();
        self.reset_caret_blink();
    }

    pub(in crate::runtime) fn handle_text_selection_drag(&mut self) -> bool {
        let Some(drag) = self.active_text_selection.clone() else {
            return false;
        };
        let Some(point) = self.cursor_position else {
            return false;
        };
        let input = drag.input.as_context();
        let cursor = self.text_input_cursor_index_at_point(
            drag.widget_id,
            input,
            ScrollContext::new(
                self.scroll_states
                    .get(&drag.widget_id)
                    .copied()
                    .unwrap_or(Point::ZERO),
            ),
            point,
        );
        self.selected_text = Some(drag.widget_id);
        let changed = self.update_text_edit_state(drag.widget_id, input.text, |state| {
            state.cursor = cursor;
            state.composition = None;
        });
        if changed {
            if let Some(state) = self.text_edit_states.get(&drag.widget_id).cloned() {
                self.ensure_text_input_caret_visible(drag.widget_id, input, &state);
            }
            self.reset_caret_blink();
        }
        changed
    }

    pub(super) fn end_text_selection_drag(&mut self) -> bool {
        if self.active_text_selection.take().is_some() {
            self.invalidate_text_input_scene();
            return true;
        }
        false
    }

    pub(in crate::runtime) fn ime_cursor_request_data(
        caret_rect: Rect,
        units: UnitContext,
    ) -> ImeRequestData {
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

    pub(in crate::runtime) fn focusable_widgets_in_tab_order(&mut self) -> Vec<FocusedWidget<VM>> {
        self.focus_candidates()
            .into_iter()
            .map(|candidate| FocusedWidget {
                widget_id: candidate.widget_id,
                scope_path: candidate.scope_path,
                on_blur: candidate.on_blur,
            })
            .collect()
    }

    pub(super) fn advance_focus(&mut self, reverse: bool) -> bool {
        let focusable = self.focus_candidates();
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
        self.update_focus(
            Some(FocusedWidget {
                widget_id: next.widget_id,
                scope_path: next.scope_path,
                on_blur: next.on_blur,
            }),
            next.on_focus,
            true,
        );
        true
    }

    pub(in crate::runtime) fn update_focus(
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

        if current_id == next_id {
            self.focused_widget = next_widget;
            self.focus_visible = next_id.is_some() && focus_visible;
            return;
        }

        self.active_key_repeat = None;

        if let Some(previous) = self.focused_widget.take() {
            self.clear_tooltip_focus_suppression_if_needed(previous.widget_id);
            if let Some((widget_id, region)) = previous_single_line_input.as_ref() {
                if *widget_id == previous.widget_id {
                    let flushed = self.flush_text_input_session(*widget_id);
                    let current_value =
                        self.text_input_current_value(*widget_id, &region.controller);
                    self.reset_single_line_input_focus_state(*widget_id, &current_value);
                    if flushed.requires_global_invalidation {
                        self.invalidation.mark_dirty();
                    }
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

        self.invalidate_text_input_scene();
        self.sync_ime_state();
    }
}
