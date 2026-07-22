use super::*;
use crate::ui::widget::ScrollRegion;
use smallvec::SmallVec;
use std::collections::{HashMap, HashSet};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum FocusHitStream {
    Normal,
    Overlay,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct FocusHitLocation {
    stream: FocusHitStream,
    index: usize,
}

impl FocusHitLocation {
    fn region<'a, VM>(
        self,
        computed: &'a crate::ui::widget::ComputedScene<VM>,
    ) -> Option<&'a crate::ui::widget::HitRegion<VM>> {
        match self.stream {
            FocusHitStream::Normal => computed.hit_regions.get(self.index),
            FocusHitStream::Overlay => computed.overlay_hit_regions.get(self.index),
        }
    }
}

struct FocusCandidate {
    widget_id: WidgetId,
    tab_index: Option<i32>,
    order: usize,
    scope_path: Vec<WidgetId>,
    location: FocusHitLocation,
    /// Whether the hit occurrence is a text input.  Focus navigation already has to index every
    /// focusable occurrence, so retaining this bit avoids rescanning the full hit stream when the
    /// runtime only needs to decide whether IME/text-edit handling applies.
    is_text_input: bool,
}

impl Clone for FocusCandidate {
    fn clone(&self) -> Self {
        Self {
            widget_id: self.widget_id,
            tab_index: self.tab_index,
            order: self.order,
            scope_path: self.scope_path.clone(),
            location: self.location,
            is_text_input: self.is_text_input,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct ActivationLocations {
    enter: Option<FocusHitLocation>,
    space: Option<FocusHitLocation>,
}

pub(in crate::runtime) struct FocusNavigationSnapshot {
    scene_key: (u64, u64),
    active_trap_scope: Option<Vec<WidgetId>>,
    active_auto_focus_scope: Option<Vec<WidgetId>>,
    /// Candidates remain in source order so a new scene can be validated without sorting or
    /// cloning scope paths. `tab_order` is the sorted indirection used by keyboard navigation.
    candidates: Vec<FocusCandidate>,
    tab_order: Vec<usize>,
    tab_position_by_widget: HashMap<WidgetId, usize>,
    activations: HashMap<WidgetId, ActivationLocations>,
    text_input_widgets: HashSet<WidgetId>,
}

fn scope_path_within(path: &[WidgetId], scope: &[WidgetId]) -> bool {
    path.starts_with(scope)
}

impl FocusNavigationSnapshot {
    fn from_scene<VM>(computed: &crate::ui::widget::ComputedScene<VM>) -> Self {
        let active_trap_scope = active_focus_trap_scope_from_scene(computed);
        let active_auto_focus_scope = computed
            .focus_scopes
            .iter()
            .rev()
            .find(|scope| scope.active && scope.options.is_auto_focus_first())
            .map(|scope| scope.path.as_slice());
        let mut candidates = Vec::new();
        let mut seen_inline = SmallVec::<[WidgetId; 16]>::new();
        let mut seen_heap = None;
        let mut activations = HashMap::<WidgetId, ActivationLocations>::new();
        let mut text_input_widgets = HashSet::new();
        for (stream, regions) in [
            (FocusHitStream::Normal, computed.hit_regions.as_slice()),
            (
                FocusHitStream::Overlay,
                computed.overlay_hit_regions.as_slice(),
            ),
        ] {
            for (index, region) in regions.iter().enumerate() {
                let location = FocusHitLocation { stream, index };
                if let crate::ui::widget::HitInteraction::TextInput { id, .. } = &region.interaction
                {
                    text_input_widgets.insert(*id);
                }
                if let Some((widget_id, enter, space)) = region.interaction.keyboard_activation() {
                    let locations = activations.entry(widget_id).or_default();
                    if enter && locations.enter.is_none() {
                        locations.enter = Some(location);
                    }
                    if space && locations.space.is_none() {
                        locations.space = Some(location);
                    }
                }

                let Some(focus) = region.focus.as_ref() else {
                    continue;
                };
                if focus.tab_index.unwrap_or(0) < 0
                    || active_trap_scope
                        .as_ref()
                        .is_some_and(|trap| !scope_path_within(&focus.scope_path, trap))
                    || !insert_seen_focus(&mut seen_inline, &mut seen_heap, focus.widget_id)
                {
                    continue;
                }
                candidates.push(FocusCandidate {
                    widget_id: focus.widget_id,
                    tab_index: focus.tab_index,
                    order: focus.order,
                    scope_path: focus.scope_path.clone(),
                    location,
                    is_text_input: matches!(
                        region.interaction,
                        crate::ui::widget::HitInteraction::TextInput { .. }
                    ),
                });
            }
        }
        let mut tab_order = (0..candidates.len()).collect::<Vec<_>>();
        tab_order.sort_by(|left, right| {
            let left = &candidates[*left];
            let right = &candidates[*right];
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
        let tab_position_by_widget = tab_order
            .iter()
            .enumerate()
            .map(|(position, candidate_index)| (candidates[*candidate_index].widget_id, position))
            .collect();
        Self {
            scene_key: computed.focus_navigation_cache_key(),
            active_trap_scope: active_trap_scope.map(<[WidgetId]>::to_vec),
            active_auto_focus_scope: active_auto_focus_scope.map(<[WidgetId]>::to_vec),
            candidates,
            tab_order,
            tab_position_by_widget,
            activations,
            text_input_widgets,
        }
    }

    fn first_candidate_in_scope(&self, scope: &[WidgetId]) -> Option<&FocusCandidate> {
        self.tab_order
            .iter()
            .map(|index| &self.candidates[*index])
            .find(|candidate| scope_path_within(&candidate.scope_path, scope))
    }

    fn candidate_for_widget(&self, widget_id: WidgetId) -> Option<&FocusCandidate> {
        let position = *self.tab_position_by_widget.get(&widget_id)?;
        self.tab_order
            .get(position)
            .and_then(|index| self.candidates.get(*index))
    }

    fn activation_for(
        &self,
        widget_id: WidgetId,
        enter: bool,
        space: bool,
    ) -> Option<FocusHitLocation> {
        let locations = self.activations.get(&widget_id)?;
        if enter {
            locations.enter
        } else if space {
            locations.space
        } else {
            None
        }
    }

    /// Exact semantic validation against a newly materialized scene. Unchanged paint/focus-style
    /// recollections can transfer the existing snapshot to the new scene serial; a key change is
    /// already a cold path, so retaining the complete text-input set keeps wrapper hit regions
    /// from being mistaken for ordinary controls.
    fn matches_scene<VM>(&self, computed: &crate::ui::widget::ComputedScene<VM>) -> bool {
        let active_trap_scope = active_focus_trap_scope_from_scene(computed);
        let active_auto_focus_scope = computed
            .focus_scopes
            .iter()
            .rev()
            .find(|scope| scope.active && scope.options.is_auto_focus_first())
            .map(|scope| scope.path.as_slice());
        if self.active_trap_scope.as_deref() != active_trap_scope
            || self.active_auto_focus_scope.as_deref() != active_auto_focus_scope
        {
            return false;
        }

        let mut candidate_cursor = 0;
        let mut matched_enter = 0;
        let mut matched_space = 0;
        let mut text_input_widgets = HashSet::new();

        for (stream, regions) in [
            (FocusHitStream::Normal, computed.hit_regions.as_slice()),
            (
                FocusHitStream::Overlay,
                computed.overlay_hit_regions.as_slice(),
            ),
        ] {
            for (index, region) in regions.iter().enumerate() {
                let location = FocusHitLocation { stream, index };
                if let crate::ui::widget::HitInteraction::TextInput { id, .. } = &region.interaction
                {
                    text_input_widgets.insert(*id);
                }
                if let Some((widget_id, enter, space)) = region.interaction.keyboard_activation() {
                    let Some(expected) = self.activations.get(&widget_id) else {
                        return false;
                    };
                    if enter {
                        match expected.enter {
                            Some(expected_location) if expected_location == location => {
                                matched_enter += 1;
                            }
                            Some(expected_location) if expected_location < location => {}
                            _ => return false,
                        }
                    }
                    if space {
                        match expected.space {
                            Some(expected_location) if expected_location == location => {
                                matched_space += 1;
                            }
                            Some(expected_location) if expected_location < location => {}
                            _ => return false,
                        }
                    }
                }

                let Some(focus) = region.focus.as_ref() else {
                    continue;
                };
                if focus.tab_index.unwrap_or(0) < 0
                    || active_trap_scope
                        .as_ref()
                        .is_some_and(|trap| !scope_path_within(&focus.scope_path, trap))
                {
                    continue;
                }
                let Some(first_occurrence) = self.candidate_for_widget(focus.widget_id) else {
                    return false;
                };
                if first_occurrence.location != location {
                    if first_occurrence.location < location {
                        continue;
                    }
                    return false;
                }
                let Some(expected) = self.candidates.get(candidate_cursor) else {
                    return false;
                };
                if expected.location != location
                    || expected.widget_id != focus.widget_id
                    || expected.tab_index != focus.tab_index
                    || expected.order != focus.order
                    || expected.scope_path != focus.scope_path
                    || expected.is_text_input
                        != matches!(
                            region.interaction,
                            crate::ui::widget::HitInteraction::TextInput { .. }
                        )
                {
                    return false;
                }
                candidate_cursor += 1;
            }
        }

        candidate_cursor == self.candidates.len()
            && text_input_widgets == self.text_input_widgets
            && matched_enter
                == self
                    .activations
                    .values()
                    .filter(|locations| locations.enter.is_some())
                    .count()
            && matched_space
                == self
                    .activations
                    .values()
                    .filter(|locations| locations.space.is_some())
                    .count()
    }
}

fn active_focus_trap_scope_from_scene<VM>(
    computed: &crate::ui::widget::ComputedScene<VM>,
) -> Option<&[WidgetId]> {
    computed
        .focus_scopes
        .iter()
        .rev()
        .find(|scope| scope.active && scope.options.is_trap())
        .map(|scope| scope.path.as_slice())
}

fn insert_seen_focus(
    seen_inline: &mut SmallVec<[WidgetId; 16]>,
    seen_heap: &mut Option<std::collections::HashSet<WidgetId>>,
    widget_id: WidgetId,
) -> bool {
    if let Some(seen) = seen_heap.as_mut() {
        return seen.insert(widget_id);
    }

    if seen_inline.contains(&widget_id) {
        return false;
    }

    if seen_inline.len() < 16 {
        seen_inline.push(widget_id);
        return true;
    }

    let mut seen = seen_inline
        .iter()
        .copied()
        .collect::<std::collections::HashSet<_>>();
    let inserted = seen.insert(widget_id);
    *seen_heap = Some(seen);
    inserted
}

fn focus_target_at<VM>(
    computed: &crate::ui::widget::ComputedScene<VM>,
    candidate: &FocusCandidate,
) -> Option<(FocusedWidget<VM>, Option<Command<VM>>)> {
    let focus = candidate.location.region(computed)?.focus.as_ref()?;
    if focus.widget_id != candidate.widget_id
        || focus.tab_index != candidate.tab_index
        || focus.order != candidate.order
        || focus.scope_path != candidate.scope_path
    {
        return None;
    }
    Some((
        FocusedWidget {
            widget_id: focus.widget_id,
            scope_path: focus.scope_path.clone(),
            on_blur: focus.on_blur.clone(),
        },
        focus.on_focus.clone(),
    ))
}

impl<VM: 'static> BoundRuntimeHandler<VM> {
    fn refresh_focus_navigation_cache_from_current_scene(&mut self) {
        let Some(computed) = self.cached_scene.as_ref().map(|cached| &cached.computed) else {
            self.focus_navigation_cache = None;
            return;
        };
        let scene_key = computed.focus_navigation_cache_key();
        let mut cache = self.focus_navigation_cache.take();
        #[cfg(any(test, feature = "bench-support"))]
        let mut built = false;
        #[cfg(any(test, feature = "bench-support"))]
        let mut validated = false;
        #[cfg(any(test, feature = "bench-support"))]
        let mut hit = false;

        match cache.as_mut() {
            Some(snapshot) if snapshot.scene_key == scene_key => {
                #[cfg(any(test, feature = "bench-support"))]
                {
                    hit = true;
                }
            }
            Some(snapshot) => {
                #[cfg(any(test, feature = "bench-support"))]
                {
                    validated = true;
                }
                if snapshot.matches_scene(computed) {
                    snapshot.scene_key = scene_key;
                    #[cfg(any(test, feature = "bench-support"))]
                    {
                        hit = true;
                    }
                } else {
                    cache = Some(FocusNavigationSnapshot::from_scene(computed));
                    #[cfg(any(test, feature = "bench-support"))]
                    {
                        built = true;
                    }
                }
            }
            None => {
                cache = Some(FocusNavigationSnapshot::from_scene(computed));
                #[cfg(any(test, feature = "bench-support"))]
                {
                    built = true;
                }
            }
        }
        self.focus_navigation_cache = cache;

        #[cfg(any(test, feature = "bench-support"))]
        {
            self.focus_navigation_cache_builds += u64::from(built);
            self.focus_navigation_cache_validations += u64::from(validated);
            self.focus_navigation_cache_hits += u64::from(hit);
        }
    }

    fn ensure_focus_navigation_cache(&mut self) {
        let _ = self.computed_scene();
        self.refresh_focus_navigation_cache_from_current_scene();
    }

    pub(in crate::runtime) fn retarget_focus_navigation_cache_to_current_scene(&mut self) {
        let Some(scene_key) = self
            .cached_scene
            .as_ref()
            .map(|cached| cached.computed.focus_navigation_cache_key())
        else {
            return;
        };
        if let Some(snapshot) = self.focus_navigation_cache.as_mut() {
            snapshot.scene_key = scene_key;
        }
    }

    pub(in crate::runtime) fn cached_focus_target_is_text_input(
        &self,
        widget_id: WidgetId,
    ) -> Option<bool> {
        let computed = &self.cached_scene.as_ref()?.computed;
        let snapshot = self.focus_navigation_cache.as_ref()?;
        if snapshot.scene_key != computed.focus_navigation_cache_key() {
            return None;
        }
        let candidate = snapshot.candidate_for_widget(widget_id)?;
        let region = candidate.location.region(computed)?;
        let focus = region.focus.as_ref()?;
        if focus.widget_id != widget_id {
            return None;
        }
        Some(candidate.is_text_input || snapshot.text_input_widgets.contains(&widget_id))
    }

    #[cfg(any(test, feature = "bench-support"))]
    pub(in crate::runtime) fn focus_navigation_cache_stats(&self) -> (u64, u64, u64) {
        (
            self.focus_navigation_cache_builds,
            self.focus_navigation_cache_validations,
            self.focus_navigation_cache_hits,
        )
    }

    pub(in crate::runtime) fn clear_focus_after_scene_target_removed(&mut self) {
        self.accessibility_focused_node = None;
        self.active_key_repeat = None;
        let previous = self.focused_widget.take();
        self.focus_visible = false;
        let Some(previous) = previous else {
            return;
        };

        self.clear_tooltip_focus_suppression_if_needed(previous.widget_id);
        let cached_region = self.cached_text_input_region_data(previous.widget_id);
        let cached_flush = self.cached_text_input_flush_data(previous.widget_id);
        let controller = cached_region
            .as_ref()
            .map(|region| region.controller.clone())
            .or_else(|| cached_flush.map(|flush| flush.controller));
        if controller.is_some() {
            let flushed = self.flush_text_input_session(previous.widget_id);
            if cached_region
                .as_ref()
                .is_some_and(|region| !region.multiline)
            {
                let controller = controller.expect("cached text input controller");
                let current_value = self.text_input_current_value(previous.widget_id, &controller);
                self.reset_single_line_input_focus_state(previous.widget_id, &current_value);
            }
            if flushed.requires_global_invalidation {
                self.invalidation.mark_dirty();
            }
        }
        if let Some(command) = previous.on_blur {
            self.execute_command(&command);
        }
        self.invalidate_text_input_scene();
        self.sync_ime_state();
    }

    pub(super) fn focused_scroll_region(&mut self) -> Option<ScrollRegion> {
        let focused_id = self.focused_widget_id()?;
        // CRITICAL: Use cached scroll_regions to avoid stack overflow
        let scroll_regions = self
            .cached_scene
            .as_ref()?
            .computed
            .scroll_regions
            .as_slice();
        scroll_regions.iter().copied().find(|region| {
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
        self.ensure_focus_navigation_cache();
        self.focus_navigation_cache
            .as_ref()
            .and_then(|snapshot| snapshot.active_trap_scope.clone())
    }

    pub(in crate::runtime) fn reconcile_auto_focus_after_scene_update(&mut self) -> bool {
        if self.cached_scene.is_none() {
            self.active_auto_focus_scope = None;
            return false;
        }
        self.refresh_focus_navigation_cache_from_current_scene();
        let Some(snapshot) = self.focus_navigation_cache.as_ref() else {
            self.active_auto_focus_scope = None;
            return false;
        };
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
                snapshot
                    .candidate_for_widget(focused.widget_id)
                    .is_some_and(|candidate| scope_path_within(&candidate.scope_path, &scope))
            })
            .unwrap_or(false);
        if current_focus_in_scope {
            return false;
        }
        let Some(candidate) = snapshot.first_candidate_in_scope(&scope) else {
            return false;
        };
        let candidate = candidate.clone();
        let Some((next, on_focus)) = self
            .cached_scene
            .as_ref()
            .and_then(|cached| focus_target_at(&cached.computed, &candidate))
        else {
            self.focus_navigation_cache = None;
            return false;
        };
        self.update_focus(Some(next), on_focus, true);
        true
    }

    pub(in crate::runtime) fn activate_focused_widget(&mut self, enter: bool, space: bool) -> bool {
        if let Some(handled) = self.activate_focused_portal_accessibility_node(enter, space) {
            return handled;
        }
        let Some(focused_id) = self.focused_widget_id() else {
            return false;
        };
        self.ensure_focus_navigation_cache();
        let location = self
            .focus_navigation_cache
            .as_ref()
            .and_then(|snapshot| snapshot.activation_for(focused_id, enter, space));
        let interaction = location.and_then(|location| {
            let computed = &self.cached_scene.as_ref()?.computed;
            let interaction = &location.region(computed)?.interaction;
            let (id, handles_enter, handles_space) = interaction.keyboard_activation()?;
            (id == focused_id && ((enter && handles_enter) || (space && handles_space)))
                .then(|| interaction.clone())
        });
        if location.is_some() && interaction.is_none() {
            self.focus_navigation_cache = None;
        }
        interaction
            .is_some_and(|interaction| self.dispatch_accessibility_click_interaction(interaction))
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
        self.ensure_focus_navigation_cache();
        let candidates = self
            .focus_navigation_cache
            .as_ref()
            .map(|snapshot| {
                snapshot
                    .tab_order
                    .iter()
                    .map(|index| snapshot.candidates[*index].clone())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let Some(computed) = self.cached_scene.as_ref().map(|cached| &cached.computed) else {
            return Vec::new();
        };
        candidates
            .iter()
            .filter_map(|candidate| {
                focus_target_at(computed, candidate).map(|(focused, _)| focused)
            })
            .collect()
    }

    pub(super) fn advance_focus(&mut self, reverse: bool) -> bool {
        self.ensure_focus_navigation_cache();
        let Some(snapshot) = self.focus_navigation_cache.as_ref() else {
            return false;
        };
        if snapshot.tab_order.is_empty() {
            return false;
        }
        let current = self.focused_widget_id();
        let next_position =
            match current.and_then(|id| snapshot.tab_position_by_widget.get(&id).copied()) {
                Some(index) if reverse => {
                    if index == 0 {
                        snapshot.tab_order.len() - 1
                    } else {
                        index - 1
                    }
                }
                Some(index) => (index + 1) % snapshot.tab_order.len(),
                None if reverse => snapshot.tab_order.len() - 1,
                None => 0,
            };
        let candidate = snapshot.candidates[snapshot.tab_order[next_position]].clone();
        let Some((next, on_focus)) = self
            .cached_scene
            .as_ref()
            .and_then(|cached| focus_target_at(&cached.computed, &candidate))
        else {
            self.focus_navigation_cache = None;
            return false;
        };
        self.update_focus(Some(next), on_focus, true);
        true
    }

    pub(in crate::runtime) fn update_focus(
        &mut self,
        next_widget: Option<FocusedWidget<VM>>,
        on_focus: Option<Command<VM>>,
        focus_visible: bool,
    ) {
        self.update_focus_with_accessibility_node(next_widget, on_focus, focus_visible, None);
    }

    pub(in crate::runtime) fn update_focus_with_accessibility_node(
        &mut self,
        next_widget: Option<FocusedWidget<VM>>,
        on_focus: Option<Command<VM>>,
        focus_visible: bool,
        accessibility_node: Option<accesskit::NodeId>,
    ) {
        self.accessibility_focused_node = accessibility_node;
        let current_id = self
            .focused_widget
            .as_ref()
            .map(|focused| focused.widget_id);
        let next_id = next_widget.as_ref().map(|focused| focused.widget_id);
        // The focus navigation snapshot carries the text-input bit for every keyboard focus
        // target.  Use it to keep ordinary focus transitions entirely out of the scene collector;
        // only an unknown target falls back to the exact region lookup.
        let previous_text_input = current_id.and_then(|widget_id| {
            self.cached_focus_target_is_text_input(widget_id)
                .or_else(|| {
                    self.cached_scene
                        .as_ref()
                        .filter(|_| self.text_input_regions.contains_key(&widget_id))
                        .map(|_| true)
                })
        });
        let next_text_input = next_id.and_then(|widget_id| {
            self.cached_focus_target_is_text_input(widget_id)
                .or_else(|| {
                    self.cached_scene
                        .as_ref()
                        .filter(|_| self.text_input_regions.contains_key(&widget_id))
                        .map(|_| true)
                })
        });
        let previous_single_line_input = match previous_text_input {
            Some(true) => current_id.and_then(|widget_id| {
                self.cached_text_input_region_data(widget_id)
                    .or_else(|| self.text_input_region_data(widget_id))
                    .map(|region| (widget_id, region))
                    .filter(|(_, region)| !region.multiline)
            }),
            Some(false) => None,
            None => current_id
                .and_then(|widget_id| {
                    self.text_input_region_data(widget_id)
                        .map(|region| (widget_id, region))
                })
                .filter(|(_, region)| !region.multiline),
        };
        let text_input_state_known = current_id
            .map(|_| previous_text_input.is_some())
            .unwrap_or(true)
            && next_id.map(|_| next_text_input.is_some()).unwrap_or(true);
        let text_input_transition = !text_input_state_known
            || previous_text_input == Some(true)
            || next_text_input == Some(true);
        let tooltip_focus_transition = current_id
            .is_some_and(|widget_id| self.widget_has_tooltip_in_computed(widget_id))
            || next_id.is_some_and(|widget_id| self.widget_has_tooltip_in_computed(widget_id));

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

        if text_input_transition {
            self.invalidate_text_input_scene();
            self.sync_ime_state();
        } else if tooltip_focus_transition {
            // Focus-triggered tooltips need one immediate collect to establish the animation's
            // start time. Ordinary controls without tooltips keep the O(1) focus-cache path.
            self.invalidate_computed_scene();
            let _ = self.computed_scene();
        }
    }
}
