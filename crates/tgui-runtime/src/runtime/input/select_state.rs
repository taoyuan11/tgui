use crate::foundation::view_model::{Command, ValueCommand};
use crate::ui::unit::Dp;
use crate::ui::widget::{FocusTargetMeta, HitInteraction, Point, ResolvedWidgetKind, WidgetId};

use super::{BoundRuntimeHandler, FocusedWidget, HoverTransitionHandler};

struct SelectKeyboardOption<VM> {
    widget_id: WidgetId,
    option_index: usize,
    disabled: bool,
    on_select: Option<Command<VM>>,
}

struct SelectKeyboardSnapshot<VM> {
    owner_id: WidgetId,
    on_open_change: Option<ValueCommand<VM, bool>>,
    options: Vec<SelectKeyboardOption<VM>>,
}

impl<VM: 'static> BoundRuntimeHandler<VM> {
    fn focused_select_keyboard_snapshot(&self) -> Option<SelectKeyboardSnapshot<VM>> {
        let focused_id = self.focused_widget_id()?;
        let layout = self.cached_scene.as_ref()?.layout.as_ref()?;
        let owner_id = if matches!(
            layout
                .resolved_widget(focused_id)
                .map(|resolved| &resolved.kind),
            Some(ResolvedWidgetKind::Select { .. })
        ) {
            focused_id
        } else {
            layout.all_widget_ids().find(|owner_id| {
                layout
                    .resolved_widget(*owner_id)
                    .is_some_and(|resolved| match &resolved.kind {
                        ResolvedWidgetKind::Select { options, .. } => {
                            options.iter().any(|option| option.widget_id == focused_id)
                        }
                        _ => false,
                    })
            })?
        };
        let resolved = layout.resolved_widget(owner_id)?;
        let ResolvedWidgetKind::Select {
            options,
            on_open_change,
            ..
        } = &resolved.kind
        else {
            return None;
        };
        Some(SelectKeyboardSnapshot {
            owner_id,
            on_open_change: on_open_change.clone(),
            options: options
                .iter()
                .enumerate()
                .map(|(option_index, option)| SelectKeyboardOption {
                    widget_id: option.widget_id,
                    option_index,
                    disabled: option.disabled.resolve(),
                    on_select: option.on_select.clone(),
                })
                .collect(),
        })
    }

    fn materialize_select_option_focus(
        &mut self,
        owner_id: WidgetId,
        option_index: usize,
        option_count: usize,
        option_id: WidgetId,
    ) -> Option<FocusTargetMeta<VM>> {
        let list_id = crate::ui::widget::select_virtual_list_id(owner_id);
        let _ = self.computed_scene();
        let scroll_region = self.cached_scene.as_ref().and_then(|cached| {
            cached
                .computed
                .scroll_regions
                .iter()
                .copied()
                .find(|region| region.id == list_id)
        });
        if let Some(region) =
            scroll_region.filter(|region| region.can_scroll_y() && option_count > 0)
        {
            let current = self.effective_scroll_offset(region.id, region.scroll_offset);
            let item_extent = region.content_bounds.height / option_count as f32;
            let row_top = item_extent * option_index as f32;
            let row_bottom = row_top + item_extent;
            let next_y = if row_top < current.y {
                row_top
            } else if row_bottom > current.y + region.content_viewport.height {
                row_bottom - region.content_viewport.height
            } else {
                current.y
            }
            .clamp(Dp::ZERO, region.max_offset().y);
            if (next_y - current.y).abs() > 0.01 {
                self.cancel_scroll_motion(region.id);
                self.set_scroll_offset(region.id, Point::new(current.x, next_y));
                let _ = self.computed_scene();
            }
        }

        if let Some(focus) = self
            .computed_scene()
            .overlay_hit_regions
            .iter()
            .find_map(|region| match &region.interaction {
                HitInteraction::SelectOption {
                    id,
                    option_index: candidate_index,
                    ..
                } if *id == owner_id && *candidate_index == option_index => region
                    .focus
                    .as_ref()
                    .filter(|focus| focus.widget_id == option_id)
                    .cloned(),
                _ => None,
            })
        {
            return Some(focus);
        }

        // The opening transition initially emits a zero-height visual overlay, so its virtual
        // rows do not have hit regions yet. Keyboard navigation must still enter the active
        // focus trap immediately; the materialized row will provide the same scope on a later
        // animation frame.
        self.cached_scene
            .as_ref()?
            .computed
            .focus_scopes
            .iter()
            .rev()
            .find(|scope| scope.active && scope.scope_id == owner_id)
            .map(|scope| FocusTargetMeta {
                widget_id: option_id,
                tab_index: None,
                order: 0,
                scope_path: scope.path.clone(),
                on_focus: None,
                on_blur: None,
            })
    }

    pub(super) fn move_focused_select_option(&mut self, direction: i32) -> bool {
        let Some(snapshot) = self.focused_select_keyboard_snapshot() else {
            return false;
        };
        let focused_id = self.focused_widget_id();
        let was_open = self
            .resolved_select_open_state(snapshot.owner_id)
            .unwrap_or(false);
        if !was_open {
            let requested = self.set_select_open_state(
                snapshot.owner_id,
                true,
                snapshot.on_open_change.as_ref(),
            );
            if !self
                .resolved_select_open_state(snapshot.owner_id)
                .unwrap_or(false)
            {
                return requested;
            }
        }

        let enabled = snapshot
            .options
            .iter()
            .filter(|option| !option.disabled)
            .collect::<Vec<_>>();
        if enabled.is_empty() {
            return !was_open;
        }
        let next_index = focused_id
            .and_then(|id| enabled.iter().position(|option| option.widget_id == id))
            .map(|index| {
                if direction < 0 {
                    index.checked_sub(1).unwrap_or(enabled.len() - 1)
                } else {
                    (index + 1) % enabled.len()
                }
            })
            .unwrap_or_else(|| if direction < 0 { enabled.len() - 1 } else { 0 });
        let next_id = enabled[next_index].widget_id;
        let Some(focus) = self.materialize_select_option_focus(
            snapshot.owner_id,
            enabled[next_index].option_index,
            snapshot.options.len(),
            next_id,
        ) else {
            return false;
        };
        self.update_focus(
            Some(FocusedWidget {
                widget_id: next_id,
                scope_path: focus.scope_path,
                on_blur: focus.on_blur.clone(),
            }),
            focus.on_focus,
            true,
        );
        true
    }

    pub(super) fn activate_focused_select_option(&mut self) -> bool {
        let Some(snapshot) = self.focused_select_keyboard_snapshot() else {
            return false;
        };
        let Some(focused_id) = self.focused_widget_id() else {
            return false;
        };
        let Some(option) = snapshot
            .options
            .iter()
            .find(|option| option.widget_id == focused_id && !option.disabled)
        else {
            return false;
        };
        let Some(command) = option.on_select.clone() else {
            return false;
        };
        self.execute_command(&command);
        let _ =
            self.set_select_open_state(snapshot.owner_id, false, snapshot.on_open_change.as_ref());
        let _ = self.restore_overlay_focus_if_needed(snapshot.owner_id);
        true
    }

    pub(in crate::runtime) fn resolved_select_open_state(
        &mut self,
        widget_id: WidgetId,
    ) -> Option<bool> {
        if let Some(open) = self
            .cached_scene
            .as_ref()
            .and_then(|cached| cached.layout.as_ref())
            .and_then(|layout| layout.resolved_widget(widget_id))
            .and_then(|resolved| match &resolved.kind {
                ResolvedWidgetKind::Select { open, .. } => open.as_ref(),
                _ => None,
            })
        {
            return Some(open.resolve());
        }
        if let Some(open) = self.select_open_states.get(&widget_id).copied() {
            return Some(open);
        }
        let computed = self.computed_scene();
        computed
            .hit_regions
            .iter()
            .chain(computed.overlay_hit_regions.iter())
            .find_map(|region| match &region.interaction {
                HitInteraction::SelectTrigger { id, is_open, .. } if *id == widget_id => {
                    Some(*is_open)
                }
                _ => None,
            })
    }

    pub(in crate::runtime) fn set_select_open_state(
        &mut self,
        widget_id: WidgetId,
        open: bool,
        on_open_change: Option<&ValueCommand<VM, bool>>,
    ) -> bool {
        let controlled = self
            .cached_scene
            .as_ref()
            .and_then(|cached| cached.layout.as_ref())
            .and_then(|layout| layout.resolved_widget(widget_id))
            .is_some_and(|resolved| {
                matches!(
                    &resolved.kind,
                    ResolvedWidgetKind::Select { open: Some(_), .. }
                )
            });
        let previous = self.resolved_select_open_state(widget_id).unwrap_or(false);
        if previous == open {
            return false;
        }

        if let Some(command) = on_open_change {
            self.execute_value_command(command, open);
        }
        if !controlled {
            self.select_open_states.insert(widget_id, open);
        }
        self.invalidate_scene_with_reason("select_open_state");
        true
    }

    pub(in crate::runtime) fn close_all_open_selects_except(
        &mut self,
        keep_open: Option<WidgetId>,
    ) -> bool {
        let select_triggers: Vec<_> = self
            .computed_scene()
            .hit_regions
            .iter()
            .filter_map(|region| match &region.interaction {
                HitInteraction::SelectTrigger {
                    id, on_open_change, ..
                } => Some((*id, on_open_change.clone())),
                _ => None,
            })
            .collect();

        let mut changed = false;
        for (id, on_open_change) in select_triggers {
            if Some(id) == keep_open || !self.resolved_select_open_state(id).unwrap_or(false) {
                continue;
            }
            changed |= self.set_select_open_state(id, false, on_open_change.as_ref());
        }
        changed
    }
}

pub(in crate::runtime) struct HoverMoveOrTransition;

impl HoverMoveOrTransition {
    pub(in crate::runtime) fn into_transition<VM>(
        command: Command<VM>,
    ) -> HoverTransitionHandler<VM> {
        HoverTransitionHandler::Command(command)
    }
}
