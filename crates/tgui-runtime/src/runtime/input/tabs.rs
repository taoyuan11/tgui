use super::*;

struct TabKeyboardTarget<VM> {
    id: WidgetId,
    group_id: WidgetId,
    scroll_container_id: Option<WidgetId>,
    index: usize,
    placement: crate::ui::widget::TabPlacement,
    rect: Rect,
    clip_rect: Option<Rect>,
    key: String,
    label: String,
    on_change: Option<ValueCommand<VM, (String, String)>>,
    focus: Option<crate::ui::widget::FocusTargetMeta<VM>>,
}

impl<VM> Clone for TabKeyboardTarget<VM> {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            group_id: self.group_id,
            scroll_container_id: self.scroll_container_id,
            index: self.index,
            placement: self.placement,
            rect: self.rect,
            clip_rect: self.clip_rect,
            key: self.key.clone(),
            label: self.label.clone(),
            on_change: self.on_change.clone(),
            focus: self.focus.clone(),
        }
    }
}

impl<VM: 'static> BoundRuntimeHandler<VM> {
    fn focused_tab_target(&mut self) -> Option<TabKeyboardTarget<VM>> {
        let focused_id = self.focused_widget_id()?;
        let computed = self.computed_scene();
        computed
            .hit_regions
            .iter()
            .chain(computed.overlay_hit_regions.iter())
            .find_map(|region| match &region.interaction {
                HitInteraction::TabTrigger {
                    id,
                    group_id,
                    scroll_container_id,
                    index,
                    placement,
                    key,
                    label,
                    on_change,
                    ..
                } if *id == focused_id => Some(TabKeyboardTarget {
                    id: *id,
                    group_id: *group_id,
                    scroll_container_id: *scroll_container_id,
                    index: *index,
                    placement: *placement,
                    rect: region.rect,
                    clip_rect: region.clip_rect,
                    key: key.clone(),
                    label: label.clone(),
                    on_change: on_change.clone(),
                    focus: region.focus.clone(),
                }),
                _ => None,
            })
    }

    fn tab_targets_in_group(&mut self, group_id: WidgetId) -> Vec<TabKeyboardTarget<VM>> {
        let computed = self.computed_scene();
        let mut targets: Vec<_> = computed
            .hit_regions
            .iter()
            .chain(computed.overlay_hit_regions.iter())
            .filter_map(|region| match &region.interaction {
                HitInteraction::TabTrigger {
                    id,
                    group_id: candidate_group,
                    scroll_container_id,
                    index,
                    placement,
                    key,
                    label,
                    on_change,
                    ..
                } if *candidate_group == group_id => Some(TabKeyboardTarget {
                    id: *id,
                    group_id: *candidate_group,
                    scroll_container_id: *scroll_container_id,
                    index: *index,
                    placement: *placement,
                    rect: region.rect,
                    clip_rect: region.clip_rect,
                    key: key.clone(),
                    label: label.clone(),
                    on_change: on_change.clone(),
                    focus: region.focus.clone(),
                }),
                _ => None,
            })
            .collect();
        targets.sort_by_key(|target| target.index);
        targets
    }

    pub(super) fn move_focused_tab(&mut self, step: i32) -> bool {
        let Some(current) = self.focused_tab_target() else {
            return false;
        };
        let targets = self.tab_targets_in_group(current.group_id);
        if targets.len() <= 1 {
            return false;
        }
        let Some(current_position) = targets.iter().position(|target| target.id == current.id)
        else {
            return false;
        };
        let next_position = if step < 0 {
            if current_position == 0 {
                targets.len() - 1
            } else {
                current_position - 1
            }
        } else {
            (current_position + 1) % targets.len()
        };
        self.focus_tab_target(targets[next_position].clone())
    }

    pub(super) fn move_focused_tab_to_edge(&mut self, end: bool) -> bool {
        let Some(current) = self.focused_tab_target() else {
            return false;
        };
        let targets = self.tab_targets_in_group(current.group_id);
        let Some(target) = (if end { targets.last() } else { targets.first() }) else {
            return false;
        };
        if target.id == current.id {
            return false;
        }
        self.focus_tab_target(target.clone())
    }

    pub(super) fn focused_tab_is_horizontal(&mut self) -> Option<bool> {
        self.focused_tab_target()
            .map(|target| target.placement.is_horizontal())
    }

    fn focus_tab_target(&mut self, target: TabKeyboardTarget<VM>) -> bool {
        let Some(focus) = target.focus.clone() else {
            return false;
        };
        self.ensure_tab_target_visible(&target);
        self.update_focus(
            Some(FocusedWidget {
                widget_id: target.id,
                scope_path: focus.scope_path,
                on_blur: focus.on_blur.clone(),
            }),
            focus.on_focus,
            true,
        );
        true
    }

    fn ensure_tab_target_visible(&mut self, target: &TabKeyboardTarget<VM>) {
        let Some(scroll_container_id) = target.scroll_container_id else {
            return;
        };
        let Some(region) = self
            .computed_scene()
            .scroll_regions
            .iter()
            .copied()
            .find(|region| region.id == scroll_container_id)
        else {
            return;
        };
        let Some(mut viewport) = region.content_viewport.intersect(region.visible_frame) else {
            return;
        };
        if let Some(clip_rect) = target.clip_rect {
            let Some(clipped_viewport) = viewport.intersect(clip_rect) else {
                return;
            };
            viewport = clipped_viewport;
        }

        let current = region.scroll_offset;
        let max_offset = region.max_offset();
        let mut next = current;
        if target.placement.is_horizontal() && region.can_scroll_x() {
            next.x = if target.rect.x < viewport.x {
                current.x - (viewport.x - target.rect.x)
            } else if target.rect.right() > viewport.right() {
                current.x + (target.rect.right() - viewport.right())
            } else {
                current.x
            }
            .clamp(Dp::ZERO, max_offset.x);
        } else if !target.placement.is_horizontal() && region.can_scroll_y() {
            next.y = if target.rect.y < viewport.y {
                current.y - (viewport.y - target.rect.y)
            } else if target.rect.bottom() > viewport.bottom() {
                current.y + (target.rect.bottom() - viewport.bottom())
            } else {
                current.y
            }
            .clamp(Dp::ZERO, max_offset.y);
        }

        if (next.x - current.x).abs() <= 0.01 && (next.y - current.y).abs() <= 0.01 {
            return;
        }
        self.cancel_scroll_motion(scroll_container_id);
        self.set_scroll_offset(scroll_container_id, next);
    }

    pub(super) fn finish_tab_reorder(&mut self) -> bool {
        let Some(active) = self.active_tab_reorder.take() else {
            return false;
        };
        let target = self
            .hit_path(self.viewport_rect())
            .into_iter()
            .rev()
            .find_map(|interaction| match interaction {
                HitInteraction::TabTrigger {
                    group_id,
                    index,
                    key,
                    ..
                } if group_id == active.group_id => Some((index, key)),
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
            crate::ui::widget::TabsReorderEvent {
                from_index: active.from_index,
                to_index,
                key: active.key,
                target_key,
                placement: active.placement,
            },
        );
        true
    }
}
