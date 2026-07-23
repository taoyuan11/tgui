use super::*;
use crate::accessibility::{
    accessibility_fragment_hit_visible_bounds, build_tree_update_with_registry,
    live_accessibility_fragment_node_index, node_id_from_widget, widget_id_from_node,
    PortalAccessibilityNodeRoute, TreeUpdateKey,
};
use crate::foundation::binding::{TextChange, TextChangeSet};
use crate::ui::widget::{splitter_adjusted_sizes, HitInteraction, SplitterResize};
use accesskit::{Action, ActionData, ActionRequest};

impl<VM: 'static> BoundRuntimeHandler<VM> {
    pub(in crate::runtime) fn reconcile_accessibility_focus_after_scene_update(&mut self) -> bool {
        let Some((focused_widget_id, focused_scope_path)) = self
            .focused_widget
            .as_ref()
            .map(|focused| (focused.widget_id, focused.scope_path.clone()))
        else {
            self.accessibility_focused_node = None;
            return false;
        };
        let Some(cached) = self.cached_scene.as_ref() else {
            return false;
        };
        let computed = &cached.computed;

        if let Some(node_id) = self.accessibility_focused_node {
            let route_is_live = self
                .accessibility_node_registry
                .live_route(node_id)
                .is_some_and(|route| {
                    portal_accessibility_route_matches_focus(
                        computed,
                        route,
                        self.window_instance_id,
                        focused_widget_id,
                        &focused_scope_path,
                    )
                });
            if route_is_live {
                return false;
            }
            self.clear_focus_after_scene_target_removed();
            return true;
        }

        let normal_focus_is_live = computed
            .hit_regions
            .iter()
            .any(|hit| hit_region_matches_focus(hit, focused_widget_id, &focused_scope_path));
        let mut portal_focus = None;
        let mut portal_focus_is_ambiguous = false;
        for (node_id, route) in self.accessibility_node_registry.live_routes() {
            if !portal_accessibility_route_matches_focus(
                computed,
                route,
                self.window_instance_id,
                focused_widget_id,
                &focused_scope_path,
            ) {
                continue;
            }
            if portal_focus.replace(node_id).is_some() {
                portal_focus_is_ambiguous = true;
                break;
            }
        }
        if !normal_focus_is_live && !portal_focus_is_ambiguous {
            if let Some(node_id) = portal_focus {
                self.accessibility_focused_node = Some(node_id);
                return false;
            }
        }
        if portal_focus_is_ambiguous && !normal_focus_is_live {
            self.clear_focus_after_scene_target_removed();
            return true;
        }

        // Local focus ownership is reconciled by the normal widget-state pruning path. A scene
        // transition can temporarily omit its hit region (for example while a drawer opens), so
        // absence here is not sufficient evidence that the focused widget was removed. This
        // accessibility pass may clear focus only when a synthetic Portal occurrence that it
        // owns becomes stale (handled above), or when an exact Portal route is ambiguous.
        false
    }

    #[cfg(test)]
    pub(in crate::runtime) fn accessibility_tree_update_for_test(
        &mut self,
    ) -> accesskit::TreeUpdate {
        let _ = self.computed_scene();
        let focused = self
            .focused_widget
            .as_ref()
            .map(|focused| (focused.widget_id, focused.scope_path.clone()));
        let preferred_portal_node = self.accessibility_focused_node;
        let cached = self
            .cached_scene
            .as_ref()
            .expect("computed scene should be cached");
        let candidate_focus = accessibility_focus_candidate(
            &self.accessibility_node_registry,
            &cached.computed,
            preferred_portal_node,
            focused
                .as_ref()
                .map(|(widget_id, scope_path)| (*widget_id, scope_path.as_slice())),
            self.window_instance_id,
        );
        let mut update = build_tree_update_with_registry(
            cached.layout.as_ref(),
            &cached.computed,
            Some(candidate_focus),
            cached.viewport,
            self.window_instance_id,
            &mut self.accessibility_node_registry,
        );
        let resolved_focus = accessibility_focus_candidate(
            &self.accessibility_node_registry,
            &cached.computed,
            preferred_portal_node,
            focused
                .as_ref()
                .map(|(widget_id, scope_path)| (*widget_id, scope_path.as_slice())),
            self.window_instance_id,
        );
        update.focus = update
            .nodes
            .iter()
            .any(|(node_id, _)| *node_id == resolved_focus)
            .then_some(resolved_focus)
            .unwrap_or(crate::accessibility::ROOT_NODE_ID);
        self.accessibility_focused_node = self
            .accessibility_node_registry
            .live_route(update.focus)
            .is_some()
            .then_some(update.focus);
        update
    }

    pub(in crate::runtime) fn initialize_accessibility_adapter(&mut self) {
        if self.accessibility_adapter.is_some() {
            return;
        }
        let Some(window) = self.window.as_ref() else {
            return;
        };
        self.accessibility_adapter = PlatformAccessibilityAdapter::new(
            window.as_ref(),
            self.accessibility_action_sender.clone(),
        );
    }

    pub(in crate::runtime) fn sync_accessibility_tree(&mut self) {
        let focused = self
            .focused_widget
            .as_ref()
            .map(|focused| (focused.widget_id, focused.scope_path.clone()));
        let preferred_portal_node = self.accessibility_focused_node;
        let invalidation_revision = self.invalidation.revision();
        let Some(cached) = self.cached_scene.as_ref() else {
            return;
        };
        let key = TreeUpdateKey {
            invalidation_revision,
            scene_serial: cached.computed.scene.prepare_cache_serial(),
            viewport: cached.viewport,
            theme_epoch: cached.theme_epoch,
            style_sheet_version: cached.style_sheet_version,
            density: cached.density,
            reduced_motion: cached.reduced_motion,
            text_scale_bits: cached.text_scale_bits,
            accessibility_animation_epoch: cached.accessibility_animation_epoch,
            scroll_epoch: cached
                .computed
                .has_accessible_scroll_state()
                .then_some(cached.scroll_epoch),
            text_input_epoch: cached.text_input_epoch,
            external_portal_revision: cached.external_portal_revision,
        };
        let candidate_focus = accessibility_focus_candidate(
            &self.accessibility_node_registry,
            &cached.computed,
            preferred_portal_node,
            focused
                .as_ref()
                .map(|(widget_id, scope_path)| (*widget_id, scope_path.as_slice())),
            self.window_instance_id,
        );
        let window_instance_id = self.window_instance_id;
        let registry = &mut self.accessibility_node_registry;
        let Some(adapter) = self.accessibility_adapter.as_mut() else {
            return;
        };
        adapter.update_if_active(key, candidate_focus, || {
            let mut update = build_tree_update_with_registry(
                cached.layout.as_ref(),
                &cached.computed,
                Some(candidate_focus),
                cached.viewport,
                window_instance_id,
                registry,
            );
            let resolved_focus = accessibility_focus_candidate(
                registry,
                &cached.computed,
                preferred_portal_node,
                focused
                    .as_ref()
                    .map(|(widget_id, scope_path)| (*widget_id, scope_path.as_slice())),
                window_instance_id,
            );
            update.focus = update
                .nodes
                .iter()
                .any(|(node_id, _)| *node_id == resolved_focus)
                .then_some(resolved_focus)
                .unwrap_or(crate::accessibility::ROOT_NODE_ID);
            update
        });
        let resolved_focus = accessibility_focus_candidate(
            &self.accessibility_node_registry,
            &cached.computed,
            preferred_portal_node,
            focused
                .as_ref()
                .map(|(widget_id, scope_path)| (*widget_id, scope_path.as_slice())),
            window_instance_id,
        );
        self.accessibility_focused_node = self
            .accessibility_node_registry
            .live_route(resolved_focus)
            .is_some()
            .then_some(resolved_focus);
    }

    pub(in crate::runtime) fn update_accessibility_window_focus_state(&mut self, is_focused: bool) {
        if let Some(adapter) = self.accessibility_adapter.as_mut() {
            adapter.update_window_focus_state(is_focused);
        }
    }

    pub(in crate::runtime) fn drain_accessibility_actions(&mut self) -> bool {
        with_accessibility_action_stack(|| {
            let mut handled = false;
            while let Ok(request) = self.accessibility_action_receiver.try_recv() {
                handled |= self.handle_accessibility_action(request);
            }
            handled
        })
    }

    fn handle_accessibility_action(&mut self, request: ActionRequest) -> bool {
        if request.target_tree != accesskit::TreeId::ROOT {
            return false;
        }
        if let Some(route) = self
            .accessibility_node_registry
            .live_route(request.target_node)
            .cloned()
        {
            return self.handle_portal_accessibility_action(
                request.target_node,
                route,
                request.action,
                request.data,
            );
        }
        if !self
            .accessibility_node_registry
            .is_live_local_node_id(request.target_node)
        {
            return false;
        }
        let Some(widget_id) = widget_id_from_node(request.target_node) else {
            return false;
        };
        match request.action {
            Action::Focus => self.focus_accessibility_widget(widget_id),
            Action::Click => self.click_accessibility_widget(widget_id),
            Action::Increment | Action::Decrement | Action::SetValue => {
                self.adjust_accessibility_value(widget_id, request.action, request.data)
            }
            Action::ReplaceSelectedText => {
                self.set_accessibility_text_value(widget_id, request.data)
            }
            _ => false,
        }
    }

    fn handle_portal_accessibility_action(
        &mut self,
        node_id: accesskit::NodeId,
        route: PortalAccessibilityNodeRoute,
        action: Action,
        data: Option<ActionData>,
    ) -> bool {
        let Some((hits, scope_path)) = self.live_portal_accessibility_target(&route) else {
            return false;
        };
        match action {
            Action::Focus => {
                self.focus_portal_accessibility_target(node_id, route.widget_id, &hits, &scope_path)
            }
            Action::Click => {
                let interaction = hits
                    .iter()
                    .filter(|hit| {
                        hit_interaction_widget_id(&hit.interaction) == Some(route.widget_id)
                    })
                    .min_by_key(|hit| accessibility_click_priority(&hit.interaction))
                    .map(|hit| hit.interaction.clone());
                let Some(interaction) = interaction else {
                    return false;
                };
                if accessibility_click_is_disabled(&interaction) {
                    return false;
                }
                let _ = self.focus_portal_accessibility_target(
                    node_id,
                    route.widget_id,
                    &hits,
                    &scope_path,
                );
                self.dispatch_accessibility_click_interaction(interaction)
            }
            Action::Increment | Action::Decrement | Action::SetValue => {
                if let Some(slider) = hits.iter().find_map(|hit| match &hit.interaction {
                    HitInteraction::Slider {
                        id,
                        on_change,
                        value,
                        min,
                        max,
                        step,
                        ..
                    } if *id == route.widget_id => {
                        Some((on_change.clone(), *value, *min, *max, *step))
                    }
                    _ => None,
                }) {
                    return self.apply_accessibility_slider_state(slider, action, data);
                }
                if let Some(state) = hits.iter().find_map(|hit| match &hit.interaction {
                    HitInteraction::SplitterHandle { id, state, .. } if *id == route.widget_id => {
                        Some(state.clone())
                    }
                    _ => None,
                }) {
                    return self.apply_accessibility_splitter_state(state, action, data);
                }
                if !matches!(action, Action::SetValue) {
                    return false;
                }
                let input = hits.iter().find_map(|hit| match &hit.interaction {
                    HitInteraction::TextInput {
                        id,
                        controller,
                        on_change,
                        on_change_set,
                        ..
                    } if *id == route.widget_id => {
                        Some((controller.clone(), on_change.clone(), on_change_set.clone()))
                    }
                    _ => None,
                });
                let Some(input) = input else {
                    return false;
                };
                self.apply_accessibility_text_value(input, data)
            }
            Action::ReplaceSelectedText => {
                let input = hits.iter().find_map(|hit| match &hit.interaction {
                    HitInteraction::TextInput {
                        id,
                        controller,
                        on_change,
                        on_change_set,
                        ..
                    } if *id == route.widget_id => {
                        Some((controller.clone(), on_change.clone(), on_change_set.clone()))
                    }
                    _ => None,
                });
                let Some(input) = input else {
                    return false;
                };
                self.apply_accessibility_text_value(input, data)
            }
            _ => false,
        }
    }

    pub(in crate::runtime) fn activate_focused_portal_accessibility_node(
        &mut self,
        enter: bool,
        space: bool,
    ) -> Option<bool> {
        let node_id = self.accessibility_focused_node?;
        let Some(route) = self
            .accessibility_node_registry
            .live_route(node_id)
            .cloned()
        else {
            return Some(false);
        };
        if self.focused_widget_id() != Some(route.widget_id) {
            return Some(false);
        }
        let Some((hits, _)) = self.live_portal_accessibility_target(&route) else {
            return Some(false);
        };
        let interaction = hits
            .iter()
            .filter(|hit| hit_interaction_widget_id(&hit.interaction) == Some(route.widget_id))
            .min_by_key(|hit| accessibility_click_priority(&hit.interaction))
            .map(|hit| hit.interaction.clone());
        let Some(interaction) = interaction else {
            return Some(false);
        };
        if accessibility_click_is_disabled(&interaction) {
            return Some(false);
        }
        let handles_key = match &interaction {
            HitInteraction::Widget {
                default_activation, ..
            } => {
                (enter && default_activation.handles_enter())
                    || (space && default_activation.handles_space())
            }
            HitInteraction::Checkbox { .. }
            | HitInteraction::Radio { .. }
            | HitInteraction::Switch { .. } => space,
            HitInteraction::SelectTrigger { .. } => enter,
            HitInteraction::TabTrigger { .. } => enter || space,
            _ => false,
        };
        Some(handles_key && self.dispatch_accessibility_click_interaction(interaction))
    }

    fn live_portal_accessibility_target(
        &mut self,
        route: &PortalAccessibilityNodeRoute,
    ) -> Option<(Vec<crate::ui::widget::HitRegion<VM>>, Vec<WidgetId>)> {
        let window_instance_id = self.window_instance_id;
        let computed = self.computed_scene();
        let (fragment, node_index, node) =
            portal_accessibility_route_target(computed, route, window_instance_id)?;
        Some((
            node.hits
                .iter()
                .filter(|hit| {
                    accessibility_fragment_hit_visible_bounds(fragment, node_index, hit).is_some()
                })
                .cloned()
                .collect(),
            fragment.scope_path.clone(),
        ))
    }

    fn focus_portal_accessibility_target(
        &mut self,
        node_id: accesskit::NodeId,
        widget_id: WidgetId,
        hits: &[crate::ui::widget::HitRegion<VM>],
        scope_path: &[WidgetId],
    ) -> bool {
        let target = hits.iter().find_map(|hit| {
            let focus = hit.focus.as_ref()?;
            (focus.widget_id == widget_id && focus.scope_path == scope_path).then(|| {
                (
                    FocusedWidget {
                        widget_id: focus.widget_id,
                        scope_path: focus.scope_path.clone(),
                        on_blur: focus.on_blur.clone(),
                    },
                    focus.on_focus.clone(),
                )
            })
        });
        let Some((target, on_focus)) = target else {
            return false;
        };
        self.update_focus_with_accessibility_node(Some(target), on_focus, true, Some(node_id));
        true
    }

    fn focus_accessibility_widget(&mut self, widget_id: WidgetId) -> bool {
        let target = {
            let computed = self.computed_scene();
            let active_trap = computed
                .focus_scopes
                .iter()
                .rev()
                .find(|scope| scope.active && scope.options.is_trap())
                .map(|scope| scope.path.as_slice());
            let target_in = |regions: &[crate::ui::widget::HitRegion<VM>]| {
                regions.iter().find_map(|region| {
                    let focus = region.focus.as_ref()?;
                    (focus.widget_id == widget_id
                        && accessibility_scope_allows(active_trap, &focus.scope_path))
                    .then(|| {
                        (
                            FocusedWidget {
                                widget_id: focus.widget_id,
                                scope_path: focus.scope_path.clone(),
                                on_blur: focus.on_blur.clone(),
                            },
                            focus.on_focus.clone(),
                        )
                    })
                })
            };
            let normal_owns_widget = hit_stream_contains_widget(&computed.hit_regions, widget_id);
            target_in(&computed.hit_regions).or_else(|| {
                (!normal_owns_widget)
                    .then(|| target_in(&computed.overlay_hit_regions))
                    .flatten()
            })
        };
        let Some((target, on_focus)) = target else {
            return false;
        };
        self.update_focus(Some(target), on_focus, true);
        true
    }

    fn click_accessibility_widget(&mut self, widget_id: WidgetId) -> bool {
        let interaction = {
            let computed = self.computed_scene();
            let active_trap = computed
                .focus_scopes
                .iter()
                .rev()
                .find(|scope| scope.active && scope.options.is_trap())
                .map(|scope| scope.path.as_slice());
            let interaction_in = |regions: &[crate::ui::widget::HitRegion<VM>]| {
                regions
                    .iter()
                    .filter(|region| {
                        accessibility_scope_allows(active_trap, &region.scope_path)
                            && hit_interaction_widget_id(&region.interaction) == Some(widget_id)
                    })
                    .min_by_key(|region| accessibility_click_priority(&region.interaction))
                    .map(|region| region.interaction.clone())
            };
            let normal_owns_widget = hit_stream_contains_widget(&computed.hit_regions, widget_id);
            interaction_in(&computed.hit_regions).or_else(|| {
                (!normal_owns_widget)
                    .then(|| interaction_in(&computed.overlay_hit_regions))
                    .flatten()
            })
        };
        let Some(interaction) = interaction else {
            return false;
        };
        if accessibility_click_is_disabled(&interaction) {
            return false;
        }
        let _ = self.focus_accessibility_widget(widget_id);
        self.dispatch_accessibility_click_interaction(interaction)
    }

    pub(in crate::runtime) fn dispatch_accessibility_click_interaction(
        &mut self,
        interaction: HitInteraction<VM>,
    ) -> bool {
        match interaction {
            HitInteraction::Widget { interactions, .. } => {
                if let Some(command) = interactions.on_click {
                    self.execute_command(&command);
                    true
                } else {
                    false
                }
            }
            HitInteraction::Checkbox {
                on_change, current, ..
            }
            | HitInteraction::Radio {
                on_change, current, ..
            }
            | HitInteraction::Switch {
                on_change, current, ..
            } => {
                if let Some(command) = on_change {
                    self.execute_value_command(&command, !current);
                    true
                } else {
                    false
                }
            }
            HitInteraction::SelectTrigger {
                id,
                on_open_change,
                is_open,
                ..
            } => {
                let next_open = !is_open;
                self.close_all_open_selects_except(next_open.then_some(id));
                let _ = self.set_select_open_state(id, next_open, on_open_change.as_ref());
                true
            }
            HitInteraction::TabTrigger {
                key,
                label,
                on_change,
                ..
            } => {
                if let Some(command) = on_change {
                    self.execute_value_command(&command, (key, label));
                    true
                } else {
                    false
                }
            }
            HitInteraction::ListItem { state, .. } => {
                self.dispatch_list_item_accessibility_click(&state)
            }
            HitInteraction::TreeNode { state, .. }
            | HitInteraction::TreeDisclosure { state, .. }
            | HitInteraction::TreeCheckbox { state, .. } => {
                self.dispatch_tree_accessibility_click(&state)
            }
            HitInteraction::DataGridCell { state, .. } => {
                self.dispatch_data_grid_accessibility_click(&state)
            }
            HitInteraction::DataGridHeader { state, .. } => {
                self.dispatch_data_grid_header_accessibility_click(&state)
            }
            HitInteraction::DataGridResizeHandle { state, .. } => self
                .dispatch_data_grid_resize_click(
                    &state,
                    crate::ui::widget::CanvasMouseButton::Left,
                ),
            HitInteraction::SplitterHandle { state, .. } => self.reset_splitter_from_hit(&state),
            HitInteraction::SelectOption {
                on_select,
                on_open_change,
                id,
                ..
            } => {
                if let Some(command) = on_select {
                    self.execute_command(&command);
                }
                if let Some(command) = on_open_change {
                    self.execute_value_command(&command, false);
                } else {
                    let _ = self.set_select_open_state(id, false, None);
                }
                true
            }
            _ => false,
        }
    }

    fn adjust_accessibility_value(
        &mut self,
        widget_id: WidgetId,
        action: Action,
        data: Option<ActionData>,
    ) -> bool {
        if self.adjust_accessibility_slider(widget_id, action, data.clone()) {
            return true;
        }
        if self.adjust_accessibility_splitter(widget_id, action, data.clone()) {
            return true;
        }
        if matches!(action, Action::SetValue) {
            return self.set_accessibility_text_value(widget_id, data);
        }
        false
    }

    fn adjust_accessibility_splitter(
        &mut self,
        widget_id: WidgetId,
        action: Action,
        data: Option<ActionData>,
    ) -> bool {
        let splitter = {
            let computed = self.computed_scene();
            let splitter_in = |regions: &[crate::ui::widget::HitRegion<VM>]| {
                regions.iter().find_map(|region| match &region.interaction {
                    HitInteraction::SplitterHandle { id, state, .. } if *id == widget_id => {
                        Some(state.clone())
                    }
                    _ => None,
                })
            };
            let normal_owns_widget = hit_stream_contains_widget(&computed.hit_regions, widget_id);
            splitter_in(&computed.hit_regions).or_else(|| {
                (!normal_owns_widget)
                    .then(|| splitter_in(&computed.overlay_hit_regions))
                    .flatten()
            })
        };
        let Some(state) = splitter else {
            return false;
        };
        self.apply_accessibility_splitter_state(state, action, data)
    }

    fn apply_accessibility_splitter_state(
        &mut self,
        state: crate::ui::widget::SplitterHandleState<VM>,
        action: Action,
        data: Option<ActionData>,
    ) -> bool {
        let Some(command) = state.on_resize.as_ref() else {
            return false;
        };
        let current_sizes = state.current_sizes();
        let delta = match action {
            Action::Increment => state.step,
            Action::Decrement => -state.step,
            Action::SetValue => {
                let target = match data {
                    Some(ActionData::NumericValue(value)) => value as f32,
                    Some(ActionData::Value(value)) => match value.parse::<f32>() {
                        Ok(value) => value,
                        Err(_) => return false,
                    },
                    _ => return false,
                };
                target - current_sizes.get(state.index).copied().unwrap_or(0.0)
            }
            _ => return false,
        };
        let sizes = splitter_adjusted_sizes(&current_sizes, &state.constraints, state.index, delta);
        self.execute_value_command(
            command,
            SplitterResize {
                index: state.index,
                sizes,
            },
        );
        true
    }

    fn adjust_accessibility_slider(
        &mut self,
        widget_id: WidgetId,
        action: Action,
        data: Option<ActionData>,
    ) -> bool {
        let slider = {
            let computed = self.computed_scene();
            let slider_in = |regions: &[crate::ui::widget::HitRegion<VM>]| {
                regions.iter().find_map(|region| match &region.interaction {
                    HitInteraction::Slider {
                        id,
                        on_change,
                        value,
                        min,
                        max,
                        step,
                        ..
                    } if *id == widget_id => Some((on_change.clone(), *value, *min, *max, *step)),
                    _ => None,
                })
            };
            let normal_owns_widget = hit_stream_contains_widget(&computed.hit_regions, widget_id);
            slider_in(&computed.hit_regions).or_else(|| {
                (!normal_owns_widget)
                    .then(|| slider_in(&computed.overlay_hit_regions))
                    .flatten()
            })
        };
        let Some((on_change, value, min, max, step)) = slider else {
            return false;
        };
        self.apply_accessibility_slider_state((on_change, value, min, max, step), action, data)
    }

    fn apply_accessibility_slider_state(
        &mut self,
        (on_change, value, min, max, step): (
            Option<crate::foundation::view_model::ValueCommand<VM, f32>>,
            f32,
            f32,
            f32,
            f32,
        ),
        action: Action,
        data: Option<ActionData>,
    ) -> bool {
        let next = match action {
            Action::Increment => value + step,
            Action::Decrement => value - step,
            Action::SetValue => match data {
                Some(ActionData::NumericValue(value)) => value as f32,
                Some(ActionData::Value(value)) => match value.parse::<f32>() {
                    Ok(value) => value,
                    Err(_) => return false,
                },
                _ => return false,
            },
            _ => return false,
        };
        self.apply_slider_value(on_change.as_ref(), next, min, max, step, true)
    }

    fn set_accessibility_text_value(
        &mut self,
        widget_id: WidgetId,
        data: Option<ActionData>,
    ) -> bool {
        let input = {
            let computed = self.computed_scene();
            let input_in = |regions: &[crate::ui::widget::HitRegion<VM>]| {
                regions.iter().find_map(|region| match &region.interaction {
                    HitInteraction::TextInput {
                        id,
                        controller,
                        on_change,
                        on_change_set,
                        ..
                    } if *id == widget_id => {
                        Some((controller.clone(), on_change.clone(), on_change_set.clone()))
                    }
                    _ => None,
                })
            };
            let normal_owns_widget = hit_stream_contains_widget(&computed.hit_regions, widget_id);
            input_in(&computed.hit_regions).or_else(|| {
                (!normal_owns_widget)
                    .then(|| input_in(&computed.overlay_hit_regions))
                    .flatten()
            })
        };
        let Some((controller, on_change, on_change_set)) = input else {
            return false;
        };
        self.apply_accessibility_text_value((controller, on_change, on_change_set), data)
    }

    fn apply_accessibility_text_value(
        &mut self,
        (controller, on_change, on_change_set): (
            crate::foundation::binding::TextController,
            Option<crate::foundation::view_model::Command<VM>>,
            Option<crate::foundation::view_model::ValueCommand<VM, TextChangeSet>>,
        ),
        data: Option<ActionData>,
    ) -> bool {
        let text = match data {
            Some(ActionData::Value(value)) => value.to_string(),
            _ => return false,
        };
        let snapshot = controller.snapshot();
        if snapshot.text == text {
            return false;
        }
        let change = TextChange::new((0, snapshot.text.len()), text.clone());
        controller.set_text(text);
        let end_revision = controller.revision();
        if let Some(command) = on_change {
            self.execute_command(&command);
        }
        if let Some(command) = on_change_set {
            self.execute_value_command(
                &command,
                TextChangeSet {
                    start_revision: snapshot.revision,
                    end_revision,
                    changes: vec![change],
                },
            );
        }
        self.invalidate_text_input_scene();
        true
    }
}

fn portal_accessibility_route_target<'a, VM>(
    computed: &'a crate::ui::widget::ComputedScene<VM>,
    route: &PortalAccessibilityNodeRoute,
    target_window_instance_id: u64,
) -> Option<(
    &'a crate::ui::widget::AccessibilityFragment<VM>,
    usize,
    &'a crate::ui::widget::AccessibilityFragmentNode<VM>,
)> {
    let active_trap = computed
        .focus_scopes
        .iter()
        .rev()
        .find(|scope| scope.active && scope.options.is_trap())
        .map(|scope| scope.path.as_slice());
    let mut matching_fragments = computed.accessibility_fragments.iter().filter(|fragment| {
        fragment
            .source_open
            .as_ref()
            .map(crate::ui::layout::Value::resolve_untracked)
            .unwrap_or(true)
            && fragment
                .source_window_instance_id
                .unwrap_or(target_window_instance_id)
                == route.source_window_instance_id
            && fragment.source_publication_generation == route.source_publication_generation
            && fragment.owner_path == route.owner_path
    });
    let fragment = matching_fragments.next()?;
    if matching_fragments.next().is_some() {
        return None;
    }
    if !accessibility_scope_allows(active_trap, &fragment.scope_path) {
        return None;
    }
    let node_index =
        live_accessibility_fragment_node_index(fragment, &route.resolved_path, route.widget_id)?;
    Some((fragment, node_index, fragment.nodes.get(node_index)?))
}

fn portal_accessibility_route_matches_focus<VM>(
    computed: &crate::ui::widget::ComputedScene<VM>,
    route: &PortalAccessibilityNodeRoute,
    target_window_instance_id: u64,
    widget_id: WidgetId,
    scope_path: &[WidgetId],
) -> bool {
    let Some((fragment, node_index, node)) =
        portal_accessibility_route_target(computed, route, target_window_instance_id)
    else {
        return false;
    };
    node.widget_id == widget_id
        && node.hits.iter().any(|hit| {
            accessibility_fragment_hit_visible_bounds(fragment, node_index, hit).is_some()
                && hit_region_matches_focus(hit, widget_id, scope_path)
        })
}

fn hit_region_matches_focus<VM>(
    hit: &crate::ui::widget::HitRegion<VM>,
    widget_id: WidgetId,
    scope_path: &[WidgetId],
) -> bool {
    if hit.rect.is_empty()
        || hit
            .clip_rect
            .is_some_and(|clip| hit.rect.intersect(clip).is_none())
    {
        return false;
    }
    hit.focus
        .as_ref()
        .is_some_and(|focus| focus.widget_id == widget_id && focus.scope_path == scope_path)
}

fn accessibility_focus_candidate<VM>(
    registry: &crate::accessibility::AccessibilityNodeRegistry,
    computed: &crate::ui::widget::ComputedScene<VM>,
    preferred_portal_node: Option<accesskit::NodeId>,
    focused: Option<(WidgetId, &[WidgetId])>,
    target_window_instance_id: u64,
) -> accesskit::NodeId {
    let Some((widget_id, scope_path)) = focused else {
        return crate::accessibility::ROOT_NODE_ID;
    };
    if let Some(node_id) = preferred_portal_node {
        if registry.live_route(node_id).is_some_and(|route| {
            portal_accessibility_route_matches_focus(
                computed,
                route,
                target_window_instance_id,
                widget_id,
                scope_path,
            )
        }) {
            return node_id;
        }
        return crate::accessibility::ROOT_NODE_ID;
    }

    let raw_node_id = node_id_from_widget(widget_id);
    let normal_focus_is_live = registry.is_live_local_node_id(raw_node_id)
        && computed
            .hit_regions
            .iter()
            .any(|hit| hit_region_matches_focus(hit, widget_id, scope_path));
    let mut portal_focus = None;
    for (node_id, route) in registry.live_routes() {
        if !portal_accessibility_route_matches_focus(
            computed,
            route,
            target_window_instance_id,
            widget_id,
            scope_path,
        ) {
            continue;
        }
        if portal_focus.replace(node_id).is_some() {
            return crate::accessibility::ROOT_NODE_ID;
        }
    }
    match (normal_focus_is_live, portal_focus) {
        (true, None) => raw_node_id,
        (false, Some(node_id)) => node_id,
        _ => crate::accessibility::ROOT_NODE_ID,
    }
}

fn with_accessibility_action_stack<R>(f: impl FnOnce() -> R) -> R {
    #[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
    {
        // Accessibility polling is performed on every event-loop turn. Grow lazily so an idle
        // poll does not allocate and tear down a 16 MiB temporary stack.
        const ACTION_STACK_RED_ZONE: usize = 2 * 1024 * 1024;
        const ACTION_STACK_SIZE: usize = 16 * 1024 * 1024;
        return stacker::maybe_grow(ACTION_STACK_RED_ZONE, ACTION_STACK_SIZE, f);
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        f()
    }
}

fn hit_interaction_widget_id<VM>(interaction: &HitInteraction<VM>) -> Option<WidgetId> {
    match interaction {
        HitInteraction::Occluder { id }
        | HitInteraction::Disabled { id }
        | HitInteraction::Widget { id, .. }
        | HitInteraction::SelectableText { id, .. }
        | HitInteraction::Switch { id, .. }
        | HitInteraction::Checkbox { id, .. }
        | HitInteraction::Radio { id, .. }
        | HitInteraction::SelectTrigger { id, .. }
        | HitInteraction::SelectOption { id, .. }
        | HitInteraction::TabTrigger { id, .. }
        | HitInteraction::ListItem { id, .. }
        | HitInteraction::TreeNode { id, .. }
        | HitInteraction::TreeDisclosure { id, .. }
        | HitInteraction::TreeCheckbox { id, .. }
        | HitInteraction::DataGridCell { id, .. }
        | HitInteraction::DataGridHeader { id, .. }
        | HitInteraction::DataGridResizeHandle { id, .. }
        | HitInteraction::SplitterHandle { id, .. }
        | HitInteraction::Slider { id, .. }
        | HitInteraction::TextInput { id, .. }
        | HitInteraction::CanvasItem { id, .. } => Some(*id),
    }
}

fn hit_stream_contains_widget<VM>(
    regions: &[crate::ui::widget::HitRegion<VM>],
    widget_id: WidgetId,
) -> bool {
    regions.iter().any(|region| {
        hit_interaction_widget_id(&region.interaction) == Some(widget_id)
            || region
                .focus
                .as_ref()
                .is_some_and(|focus| focus.widget_id == widget_id)
    })
}

fn accessibility_scope_allows(active_trap: Option<&[WidgetId]>, scope_path: &[WidgetId]) -> bool {
    active_trap
        .map(|trap| scope_path.starts_with(trap))
        .unwrap_or(true)
}

fn accessibility_click_priority<VM>(interaction: &HitInteraction<VM>) -> u8 {
    match interaction {
        HitInteraction::ListItem { .. }
        | HitInteraction::TreeNode { .. }
        | HitInteraction::DataGridCell { .. } => 0,
        HitInteraction::Widget { .. }
        | HitInteraction::Checkbox { .. }
        | HitInteraction::Radio { .. }
        | HitInteraction::Switch { .. }
        | HitInteraction::SelectTrigger { .. }
        | HitInteraction::TabTrigger { .. }
        | HitInteraction::DataGridHeader { .. }
        | HitInteraction::DataGridResizeHandle { .. }
        | HitInteraction::SplitterHandle { .. }
        | HitInteraction::SelectOption { .. } => 1,
        HitInteraction::TreeDisclosure { .. } | HitInteraction::TreeCheckbox { .. } => 2,
        _ => u8::MAX,
    }
}

fn accessibility_click_is_disabled<VM>(interaction: &HitInteraction<VM>) -> bool {
    match interaction {
        HitInteraction::Disabled { .. } => true,
        HitInteraction::ListItem { state, .. } => state.disabled.resolve(),
        HitInteraction::TreeNode { state, .. }
        | HitInteraction::TreeDisclosure { state, .. }
        | HitInteraction::TreeCheckbox { state, .. } => state.disabled.resolve(),
        HitInteraction::DataGridCell { state, .. } => state.disabled.resolve(),
        _ => false,
    }
}
