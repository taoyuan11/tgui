use super::select_state::HoverMoveOrTransition;
use super::*;
use crate::runtime::state::{
    RetainedButtonHoverPatch, RetainedButtonPressedPatch, RetainedHoverPatch, RetainedRowHoverPatch,
};
use crate::ui::widget::HitPath;
use smallvec::SmallVec;

impl<VM: 'static> BoundRuntimeHandler<VM> {
    pub(in crate::runtime) fn is_simple_button_hover_root(
        layout: &crate::ui::widget::ResolvedSceneLayout<VM>,
        id: WidgetId,
    ) -> bool {
        let Some(widget) = layout.resolved_widget(id) else {
            return false;
        };
        let crate::ui::widget::ResolvedWidgetKind::Button { disabled, .. } = &widget.kind else {
            return false;
        };
        let interactions = &widget.interactions;
        !disabled.resolve_untracked()
            && widget.tooltip.is_none()
            && widget.popover.is_none()
            && widget.menu.is_none()
            && widget.context_menu.is_none()
            && widget.modal.is_none()
            && widget.drawer.is_none()
            && widget.tab_trigger.is_none()
            && widget.list_item.is_none()
            && widget.tree_root.is_none()
            && widget.tree_node.is_none()
            && widget.data_grid_root.is_none()
            && widget.data_grid_cell.is_none()
            && widget.data_grid_header.is_none()
            && widget.data_grid_resize_handle.is_none()
            && widget.splitter_handle.is_none()
            && widget.carousel_auto_play.is_none()
            && widget.focus.scope.is_none()
            && interactions.on_double_click.is_none()
            && interactions.on_focus.is_none()
            && interactions.on_blur.is_none()
            && interactions.on_mouse_enter.is_none()
            && interactions.on_mouse_leave.is_none()
            && interactions.on_mouse_move.is_none()
            && interactions.on_file_drop.is_none()
            && interactions.gesture.is_none()
            && !widget.lifecycle_events.has_any()
            && !widget.media_events.has_any()
    }

    pub(in crate::runtime) fn button_hover_runtime_is_idle(&self) -> bool {
        self.pressed_widget.is_none() && self.button_visual_runtime_is_idle_ignoring_pressed()
    }

    pub(in crate::runtime) fn button_visual_runtime_is_idle_ignoring_pressed(&self) -> bool {
        !self.animation_engine.has_active_animations()
            && self.next_tooltip_wakeup_deadline.is_none()
            && self.next_toast_wakeup_deadline.is_none()
            && self.active_gesture.is_none()
            && self.active_pinch.is_none()
            && self.active_scrollbar_drag.is_none()
            && self.active_touch_scroll.is_none()
            && self.active_slider_drag.is_none()
            && self.active_canvas_drag.is_none()
            && self.active_tab_reorder.is_none()
            && self.active_tree_drag.is_none()
            && self.active_data_grid_column_resize.is_none()
            && self.active_splitter_resize.is_none()
            && self.active_data_grid_column_reorder.is_none()
            && self.active_text_selection.is_none()
            && self.pending_click.is_none()
            && self.deferred_mouse_click.is_none()
    }

    pub(in crate::runtime) fn is_simple_button_pressed_root(
        layout: &crate::ui::widget::ResolvedSceneLayout<VM>,
        id: WidgetId,
    ) -> bool {
        Self::is_simple_button_hover_root(layout, id)
            && layout
                .resolved_widget(id)
                .is_some_and(|widget| widget.interactions.on_click.is_none())
    }

    pub(in crate::runtime) fn hovered_simple_button(
        &self,
        layout: &crate::ui::widget::ResolvedSceneLayout<VM>,
    ) -> Option<WidgetId> {
        if !Self::button_hover_path_is_passive(&self.hovered_widgets) {
            return None;
        }
        let mut button = None;
        for hovered in &self.hovered_widgets {
            let HoverTargetId::Widget(id) = hovered.target_id else {
                continue;
            };
            if matches!(
                layout.resolved_widget(id).map(|widget| &widget.kind),
                Some(crate::ui::widget::ResolvedWidgetKind::Button { .. })
            ) && button.replace(id).is_some()
            {
                return None;
            }
        }
        button
    }

    pub(in crate::runtime) fn retained_button_pressed_patch_candidate(
        &self,
        source_pressed_widget: Option<WidgetId>,
    ) -> Option<RetainedButtonPressedPatch> {
        let next_pressed_widget = self.pressed_widget;
        if source_pressed_widget == next_pressed_widget
            || self.invalidation.revision() != self.last_invalidation_revision
            || self.invalidation.root_rebuild_revision() != self.last_root_rebuild_revision
            || !self.button_visual_runtime_is_idle_ignoring_pressed()
        {
            return None;
        }
        let button = source_pressed_widget.or(next_pressed_widget)?;
        if source_pressed_widget.is_some_and(|id| id != button)
            || next_pressed_widget.is_some_and(|id| id != button)
        {
            return None;
        }
        let cached = self.cached_scene.as_ref()?;
        if !cached.computed_valid
            || !cached.layout_valid
            || cached.gpu_scroll_deferred
            || cached.pressed_widget != source_pressed_widget
            || cached.hover_epoch != self.hover_epoch
            || cached.focused_widget != self.focused_widget_id()
            || cached.focus_visible != self.focus_visible
            || !cached.computed.is_simple_for_button_hover_recompose()
            || !cached.lifecycle_states.is_empty()
            || !cached.media_texture_bindings.is_empty()
            || !self.media_event_states.is_empty()
            || !self.external_portal_requests.is_empty()
        {
            return None;
        }
        let layout = cached.layout.as_ref()?;
        if layout.contains_virtual()
            || self.hovered_simple_button(layout) != Some(button)
            || !Self::is_simple_button_pressed_root(layout, button)
            || !cached
                .scene_chunks
                .get(&button)
                .is_some_and(|chunk| chunk.is_simple_for_button_hover_recompose())
            || !cached.visual_contexts.contains_key(&button)
        {
            return None;
        }
        Some(RetainedButtonPressedPatch {
            button,
            source_pressed_widget,
            next_pressed_widget,
            source_hover_epoch: self.hover_epoch,
            source_invalidation_revision: self.invalidation.revision(),
            source_root_rebuild_revision: self.invalidation.root_rebuild_revision(),
        })
    }

    pub(in crate::runtime) fn button_hover_path_is_passive(widgets: &[HoveredWidget<VM>]) -> bool {
        widgets.iter().all(|hovered| {
            hovered.row_patch_kind.is_none()
                && hovered.on_mouse_enter.is_none()
                && hovered.on_mouse_leave.is_none()
                && hovered.on_mouse_move.is_none()
        })
    }

    fn is_simple_retained_hover_widget(widget: &crate::ui::widget::ResolvedElement<VM>) -> bool {
        let interactions = &widget.interactions;
        widget.tooltip.is_none()
            && widget.popover.is_none()
            && widget.menu.is_none()
            && widget.context_menu.is_none()
            && widget.modal.is_none()
            && widget.drawer.is_none()
            && widget.tab_trigger.is_none()
            && widget.list_item.is_none()
            && widget.tree_root.is_none()
            && widget.tree_node.is_none()
            && widget.data_grid_root.is_none()
            && widget.data_grid_cell.is_none()
            && widget.data_grid_header.is_none()
            && widget.data_grid_resize_handle.is_none()
            && widget.splitter_handle.is_none()
            && widget.carousel_auto_play.is_none()
            && widget.focus.scope.is_none()
            && interactions.on_double_click.is_none()
            && interactions.on_focus.is_none()
            && interactions.on_blur.is_none()
            && interactions.on_mouse_enter.is_none()
            && interactions.on_mouse_leave.is_none()
            && interactions.on_mouse_move.is_none()
            && interactions.on_file_drop.is_none()
            && interactions.gesture.is_none()
            && !widget.lifecycle_events.has_any()
            && !widget.media_events.has_any()
    }

    pub(in crate::runtime) fn is_simple_retained_hover_root(
        layout: &crate::ui::widget::ResolvedSceneLayout<VM>,
        root: WidgetId,
    ) -> bool {
        let ids = layout.subtree_widget_ids(root);
        !ids.is_empty()
            && ids.into_iter().all(|id| {
                layout
                    .resolved_widget(id)
                    .is_some_and(Self::is_simple_retained_hover_widget)
            })
    }

    /// Returns the shallowest changed widget in a divergent hover-path suffix. Recollecting this
    /// widget covers every remaining changed state as long as all suffix entries are descendants.
    pub(in crate::runtime) fn retained_hover_suffix_root(
        layout: &crate::ui::widget::ResolvedSceneLayout<VM>,
        suffix: &[HoveredWidget<VM>],
    ) -> Option<Option<WidgetId>> {
        let Some(first) = suffix.first() else {
            return Some(None);
        };
        let HoverTargetId::Widget(root) = first.target_id else {
            return None;
        };
        let root_path = layout.path_for(root)?;
        for hovered in suffix {
            let HoverTargetId::Widget(id) = hovered.target_id else {
                return None;
            };
            if !layout.path_for(id)?.starts_with(root_path) {
                return None;
            }
        }
        Some(Some(root))
    }

    pub(in crate::runtime) fn retained_button_hover_patch_candidate(
        &self,
        next_hovered: &[HoveredWidget<VM>],
    ) -> Option<RetainedButtonHoverPatch> {
        if self.invalidation.revision() != self.last_invalidation_revision
            || self.invalidation.root_rebuild_revision() != self.last_root_rebuild_revision
            || !self.button_hover_runtime_is_idle()
        {
            return None;
        }
        let cached = self.cached_scene.as_ref()?;
        if !cached.computed_valid
            || !cached.layout_valid
            || cached.gpu_scroll_deferred
            || cached.hover_epoch != self.hover_epoch
            || !cached.computed.is_simple_for_button_hover_recompose()
            || !cached.lifecycle_states.is_empty()
            || !cached.media_texture_bindings.is_empty()
            || !self.media_event_states.is_empty()
        {
            return None;
        }
        let layout = cached.layout.as_ref()?;
        if layout.contains_virtual() {
            return None;
        }

        if !Self::button_hover_path_is_passive(&self.hovered_widgets)
            || !Self::button_hover_path_is_passive(next_hovered)
        {
            return None;
        }

        let mut prefix_len = 0usize;
        while prefix_len < self.hovered_widgets.len()
            && prefix_len < next_hovered.len()
            && self.hovered_widgets[prefix_len].target_id == next_hovered[prefix_len].target_id
        {
            prefix_len += 1;
        }
        let button_from_suffix = |suffix: &[HoveredWidget<VM>]| match suffix {
            [] => Some(None),
            [hovered] => match hovered.target_id {
                HoverTargetId::Widget(id) => Some(Some(id)),
                _ => None,
            },
            _ => None,
        };
        let previous_button = button_from_suffix(&self.hovered_widgets[prefix_len..])?;
        let next_button = button_from_suffix(&next_hovered[prefix_len..])?;
        if previous_button == next_button || (previous_button.is_none() && next_button.is_none()) {
            return None;
        }
        for id in [previous_button, next_button].into_iter().flatten() {
            if !Self::is_simple_button_hover_root(layout, id)
                || !cached
                    .scene_chunks
                    .get(&id)
                    .is_some_and(|chunk| chunk.is_simple_for_button_hover_recompose())
                || !cached.visual_contexts.contains_key(&id)
            {
                return None;
            }
        }

        Some(RetainedButtonHoverPatch {
            previous_button,
            next_button,
            source_hover_epoch: self.hover_epoch,
            source_invalidation_revision: self.invalidation.revision(),
            source_root_rebuild_revision: self.invalidation.root_rebuild_revision(),
        })
    }

    pub(in crate::runtime) fn retained_hover_patch_candidate(
        &self,
        next_hovered: &[HoveredWidget<VM>],
    ) -> Option<RetainedHoverPatch> {
        if self.invalidation.revision() != self.last_invalidation_revision
            || self.invalidation.root_rebuild_revision() != self.last_root_rebuild_revision
            || !self.button_hover_runtime_is_idle()
        {
            return None;
        }
        let cached = self.cached_scene.as_ref()?;
        if !cached.computed_valid
            || !cached.layout_valid
            || cached.gpu_scroll_deferred
            || cached.hover_epoch != self.hover_epoch
            || !Self::button_hover_path_is_passive(&self.hovered_widgets)
            || !Self::button_hover_path_is_passive(next_hovered)
        {
            return None;
        }
        let layout = cached.layout.as_ref()?;
        if layout.contains_virtual() {
            return None;
        }

        let mut prefix_len = 0usize;
        while prefix_len < self.hovered_widgets.len()
            && prefix_len < next_hovered.len()
            && self.hovered_widgets[prefix_len].target_id == next_hovered[prefix_len].target_id
        {
            prefix_len += 1;
        }
        let previous_root =
            Self::retained_hover_suffix_root(layout, &self.hovered_widgets[prefix_len..])?;
        let next_root = Self::retained_hover_suffix_root(layout, &next_hovered[prefix_len..])?;
        if previous_root == next_root || (previous_root.is_none() && next_root.is_none()) {
            return None;
        }
        for root in [previous_root, next_root].into_iter().flatten() {
            if !Self::is_simple_retained_hover_root(layout, root)
                || !cached
                    .scene_chunks
                    .get(&root)
                    .is_some_and(|chunk| chunk.is_simple_for_button_hover_recompose())
                || !cached.visual_contexts.contains_key(&root)
            {
                return None;
            }
        }

        Some(RetainedHoverPatch {
            previous_root,
            next_root,
            source_hover_epoch: self.hover_epoch,
            source_invalidation_revision: self.invalidation.revision(),
            source_root_rebuild_revision: self.invalidation.root_rebuild_revision(),
        })
    }

    pub(in crate::runtime) fn retained_row_hover_patch_candidate(
        &self,
        next_hovered: &[HoveredWidget<VM>],
    ) -> Option<RetainedRowHoverPatch> {
        if self.invalidation.revision() != self.last_invalidation_revision
            || self.invalidation.root_rebuild_revision() != self.last_root_rebuild_revision
            || self.animation_engine.has_active_animations()
            || self.next_tooltip_wakeup_deadline.is_some()
            || self.next_toast_wakeup_deadline.is_some()
        {
            return None;
        }
        let cached = self.cached_scene.as_ref()?;
        if !cached.computed_valid || !cached.layout_valid || cached.hover_epoch != self.hover_epoch
        {
            return None;
        }
        let layout = cached.layout.as_ref()?;
        let split_path = |widgets: &[HoveredWidget<VM>]| {
            let mut row = None;
            let mut remainder = SmallVec::<[HoverTargetId; 8]>::new();
            for hovered in widgets {
                match (hovered.target_id, hovered.row_patch_kind) {
                    (HoverTargetId::Widget(id), Some(kind))
                        if layout
                            .resolved_widget(id)
                            .and_then(|widget| widget.retained_hover_row_kind())
                            == Some(kind) =>
                    {
                        if row.replace((id, kind)).is_some() {
                            return None;
                        }
                    }
                    (target, _) => remainder.push(target),
                }
            }
            Some((row, remainder))
        };
        let (previous_row, previous_remainder) = split_path(&self.hovered_widgets)?;
        let (next_row, next_remainder) = split_path(next_hovered)?;
        if previous_remainder != next_remainder || previous_row == next_row {
            return None;
        }
        Some(RetainedRowHoverPatch {
            previous_row,
            next_row,
            source_hover_epoch: self.hover_epoch,
            source_invalidation_revision: self.invalidation.revision(),
            source_root_rebuild_revision: self.invalidation.root_rebuild_revision(),
        })
    }

    pub(in crate::runtime) fn hit_path(&mut self, _viewport: Rect) -> HitPath<VM> {
        let Some(point) = self.cursor_position else {
            return HitPath::new();
        };
        WidgetTree::hit_path_from_computed(self.computed_scene(), point)
    }

    pub(in crate::runtime) fn hover_path(
        &mut self,
        viewport: Rect,
    ) -> SmallVec<[HoveredWidget<VM>; 8]> {
        let hit_path = self.hit_path(viewport);
        let topmost_canvas_item = hit_path
            .iter()
            .rposition(|interaction| matches!(interaction, HitInteraction::CanvasItem { .. }));

        hit_path
            .into_iter()
            .enumerate()
            .filter(|(index, interaction)| {
                !matches!(interaction, HitInteraction::CanvasItem { .. })
                    || Some(*index) == topmost_canvas_item
            })
            .map(|(_, interaction)| interaction)
            .filter(|interaction| !matches!(interaction, HitInteraction::Occluder { .. }))
            .map(|interaction| match interaction {
                HitInteraction::Disabled { id } => HoveredWidget {
                    target_id: HoverTargetId::Widget(id),
                    row_patch_kind: None,
                    cursor_style: Some(crate::ui::widget::CursorStyle::NotAllowed),
                    on_mouse_enter: None,
                    on_mouse_leave: None,
                    on_mouse_move: None,
                },
                HitInteraction::Widget {
                    id, interactions, ..
                } => HoveredWidget {
                    target_id: HoverTargetId::Widget(id),
                    row_patch_kind: None,
                    cursor_style: interactions.cursor_style.map(|c| c.resolve()),
                    on_mouse_enter: interactions
                        .on_mouse_enter
                        .map(HoverMoveOrTransition::into_transition),
                    on_mouse_leave: interactions
                        .on_mouse_leave
                        .map(HoverMoveOrTransition::into_transition),
                    on_mouse_move: interactions.on_mouse_move.map(HoverMoveHandler::Point),
                },
                HitInteraction::SelectableText {
                    id, interactions, ..
                } => HoveredWidget {
                    target_id: HoverTargetId::Widget(id),
                    row_patch_kind: None,
                    cursor_style: interactions
                        .cursor_style
                        .map(|c| c.resolve())
                        .or(Some(crate::ui::widget::CursorStyle::Text)),
                    on_mouse_enter: interactions
                        .on_mouse_enter
                        .map(HoverMoveOrTransition::into_transition),
                    on_mouse_leave: interactions
                        .on_mouse_leave
                        .map(HoverMoveOrTransition::into_transition),
                    on_mouse_move: interactions.on_mouse_move.map(HoverMoveHandler::Point),
                },
                HitInteraction::Switch {
                    id, interactions, ..
                }
                | HitInteraction::Checkbox {
                    id, interactions, ..
                }
                | HitInteraction::Radio {
                    id, interactions, ..
                }
                | HitInteraction::Slider {
                    id, interactions, ..
                }
                | HitInteraction::TextInput {
                    id, interactions, ..
                }
                | HitInteraction::SelectTrigger {
                    id, interactions, ..
                }
                | HitInteraction::TreeDisclosure {
                    id, interactions, ..
                }
                | HitInteraction::TreeCheckbox {
                    id, interactions, ..
                }
                | HitInteraction::DataGridHeader {
                    id, interactions, ..
                }
                | HitInteraction::DataGridResizeHandle {
                    id, interactions, ..
                }
                | HitInteraction::SplitterHandle {
                    id, interactions, ..
                }
                | HitInteraction::TabTrigger {
                    id, interactions, ..
                } => HoveredWidget {
                    target_id: HoverTargetId::Widget(id),
                    row_patch_kind: None,
                    cursor_style: interactions.cursor_style.map(|c| c.resolve()),
                    on_mouse_enter: interactions
                        .on_mouse_enter
                        .map(HoverMoveOrTransition::into_transition),
                    on_mouse_leave: interactions
                        .on_mouse_leave
                        .map(HoverMoveOrTransition::into_transition),
                    on_mouse_move: interactions.on_mouse_move.map(HoverMoveHandler::Point),
                },
                HitInteraction::ListItem {
                    id, interactions, ..
                } => HoveredWidget {
                    target_id: HoverTargetId::Widget(id),
                    row_patch_kind: Some(crate::ui::widget::RetainedHoverRowKind::List),
                    cursor_style: interactions.cursor_style.map(|c| c.resolve()),
                    on_mouse_enter: interactions
                        .on_mouse_enter
                        .map(HoverMoveOrTransition::into_transition),
                    on_mouse_leave: interactions
                        .on_mouse_leave
                        .map(HoverMoveOrTransition::into_transition),
                    on_mouse_move: interactions.on_mouse_move.map(HoverMoveHandler::Point),
                },
                HitInteraction::TreeNode {
                    id, interactions, ..
                } => HoveredWidget {
                    target_id: HoverTargetId::Widget(id),
                    row_patch_kind: Some(crate::ui::widget::RetainedHoverRowKind::Tree),
                    cursor_style: interactions.cursor_style.map(|c| c.resolve()),
                    on_mouse_enter: interactions
                        .on_mouse_enter
                        .map(HoverMoveOrTransition::into_transition),
                    on_mouse_leave: interactions
                        .on_mouse_leave
                        .map(HoverMoveOrTransition::into_transition),
                    on_mouse_move: interactions.on_mouse_move.map(HoverMoveHandler::Point),
                },
                HitInteraction::DataGridCell {
                    state,
                    interactions,
                    ..
                } => HoveredWidget {
                    // All cells in a row deliberately collapse to one hover
                    // target. This keeps the visual state continuous when the
                    // pointer crosses pinned/unpinned column boundaries.
                    target_id: HoverTargetId::Widget(state.row_id),
                    row_patch_kind: Some(crate::ui::widget::RetainedHoverRowKind::DataGrid),
                    cursor_style: interactions.cursor_style.map(|c| c.resolve()),
                    on_mouse_enter: interactions
                        .on_mouse_enter
                        .map(HoverMoveOrTransition::into_transition),
                    on_mouse_leave: interactions
                        .on_mouse_leave
                        .map(HoverMoveOrTransition::into_transition),
                    on_mouse_move: interactions.on_mouse_move.map(HoverMoveHandler::Point),
                },
                HitInteraction::SelectOption {
                    state_id,
                    option_index,
                    interactions,
                    ..
                } => HoveredWidget {
                    target_id: HoverTargetId::SelectOption {
                        widget_id: state_id,
                        option_index,
                    },
                    row_patch_kind: None,
                    cursor_style: interactions.cursor_style.map(|c| c.resolve()),
                    on_mouse_enter: interactions
                        .on_mouse_enter
                        .map(HoverMoveOrTransition::into_transition),
                    on_mouse_leave: interactions
                        .on_mouse_leave
                        .map(HoverMoveOrTransition::into_transition),
                    on_mouse_move: interactions.on_mouse_move.map(HoverMoveHandler::Point),
                },
                HitInteraction::CanvasItem {
                    id,
                    item_id,
                    item_interactions,
                    cursor_style,
                    canvas_origin,
                    item_origin,
                    inverse_transform,
                    text_hits,
                } => {
                    let context = CanvasPointerContext {
                        item_id,
                        canvas_origin,
                        item_origin,
                        inverse_transform,
                        text_hits,
                    };
                    HoveredWidget {
                        target_id: HoverTargetId::CanvasItem {
                            widget_id: id,
                            item_id,
                        },
                        row_patch_kind: None,
                        cursor_style,
                        on_mouse_enter: item_interactions.on_mouse_enter.map(|command| {
                            HoverTransitionHandler::Canvas(command, context.clone())
                        }),
                        on_mouse_leave: item_interactions.on_mouse_leave.map(|command| {
                            HoverTransitionHandler::Canvas(command, context.clone())
                        }),
                        on_mouse_move: item_interactions
                            .on_mouse_move
                            .map(|command| HoverMoveHandler::Canvas(command, context)),
                    }
                }
                HitInteraction::Occluder { .. } => unreachable!("occluders are filtered above"),
            })
            .collect()
    }

    pub(in crate::runtime) fn handle_hover(&mut self, viewport: Rect) -> bool {
        let started_at = text_profile_enabled().then_some(Instant::now());
        let revision_before = self.invalidation.revision();
        let cursor_position = self.cursor_position;
        let next_hovered = self.hover_path(viewport);
        let hover_path_changed = self.hovered_widgets.len() != next_hovered.len()
            || self
                .hovered_widgets
                .iter()
                .zip(next_hovered.iter())
                .any(|(previous, next)| previous.target_id != next.target_id);
        self.button_pressed_patch_pending = None;
        self.button_hover_patch_pending = None;
        self.hover_patch_pending = None;
        if hover_path_changed {
            self.button_hover_patch_pending =
                self.retained_button_hover_patch_candidate(&next_hovered);
            self.row_hover_patch_pending = self.retained_row_hover_patch_candidate(&next_hovered);
            if self.button_hover_patch_pending.is_none() && self.row_hover_patch_pending.is_none() {
                self.hover_patch_pending = self.retained_hover_patch_candidate(&next_hovered);
            }
        }
        let mut prefix_len = 0usize;
        while prefix_len < self.hovered_widgets.len()
            && prefix_len < next_hovered.len()
            && self.hovered_widgets[prefix_len].target_id == next_hovered[prefix_len].target_id
        {
            prefix_len += 1;
        }

        let previous_hovered = std::mem::take(&mut self.hovered_widgets);
        for previous in previous_hovered[prefix_len..].iter().rev() {
            if let Some(command) = previous.on_mouse_leave.as_ref() {
                self.execute_hover_transition_handler(command, cursor_position);
            }
        }

        // 退出 hover 的 widget：从 tooltip_hover_started_at 移除。
        for previous in previous_hovered[prefix_len..].iter() {
            if let HoverTargetId::Widget(id) = previous.target_id {
                self.tooltip_hover_started_at.remove(&id);
                self.clear_tooltip_hover_suppression_if_needed(id);
            }
        }

        // 新进入 hover 的 widget：记录起始时间，用于 Tooltip delay 计算。
        let now = Instant::now();
        for hovered in next_hovered[prefix_len..].iter() {
            if let HoverTargetId::Widget(id) = hovered.target_id {
                self.tooltip_hover_started_at.entry(id).or_insert(now);
            }
        }

        for hovered in next_hovered[prefix_len..].iter().rev() {
            if let Some(command) = hovered.on_mouse_enter.as_ref() {
                self.execute_hover_transition_handler(command, cursor_position);
            }
        }

        if let Some(position) = cursor_position {
            for hovered in next_hovered.iter().rev() {
                if let Some(command) = hovered.on_mouse_move.as_ref() {
                    self.execute_hover_move_handler(command, position);
                }
            }
        }

        self.hovered_widgets = next_hovered;
        if hover_path_changed {
            self.hover_epoch = self.hover_epoch.wrapping_add(1);
        }
        let scrollbar_changed = self.sync_scrollbar_hover();
        let cursor_changed = self.update_cursor_icon();
        let changed = hover_path_changed
            || scrollbar_changed
            || cursor_changed
            || self.invalidation.revision() != revision_before;
        let _ = started_at;
        changed
    }
}
