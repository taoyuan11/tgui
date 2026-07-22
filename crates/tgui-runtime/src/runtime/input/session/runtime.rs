use super::*;

impl<VM: 'static> BoundRuntimeHandler<VM> {
    pub(in crate::runtime) fn reset_caret_blink(&mut self) {
        self.caret_blink_origin = Instant::now();
        self.invalidate_text_input_scene();
    }

    pub(in crate::runtime) fn focused_text_input_id_cached(
        &self,
        computed: &ComputedScene<VM>,
    ) -> Option<WidgetId> {
        let focused = self.focused_widget_id()?;

        // The navigation snapshot is keyed by the scene's focus metadata serial.  When it
        // matches this computed scene, the text-input bit is an exact O(1) answer and avoids
        // walking every normal/overlay hit region on each frame.
        let is_current_cached_scene = self
            .cached_scene
            .as_ref()
            .is_some_and(|cached| std::ptr::eq(&cached.computed, computed));
        if is_current_cached_scene {
            if let Some(is_text_input) = self.cached_focus_target_is_text_input(focused) {
                return is_text_input.then_some(focused);
            }
            // `text_input_regions` is synchronized from the same computed scene.  It can answer
            // the positive case without allocation; a miss is intentionally inconclusive and
            // falls back to the exact hit-stream scan below.
            if self.text_input_regions.contains_key(&focused) {
                return Some(focused);
            }
        }

        computed
            .hit_regions
            .iter()
            .chain(computed.overlay_hit_regions.iter())
            .find_map(|region| match &region.interaction {
                HitInteraction::TextInput { id, .. } if *id == focused => Some(*id),
                _ => None,
            })
    }

    pub(in crate::runtime) fn focused_text_overrides<'a>(
        text_input_buffers: &'a HashMap<WidgetId, TextInputBufferState>,
        focused_input: Option<WidgetId>,
        focused_text_state: Option<&TextEditState>,
    ) -> (
        Option<&'a str>,
        Option<&'a crate::text::font::TextLayoutInfo>,
    ) {
        let Some(widget_id) = focused_input else {
            return (None, None);
        };
        let Some(state) = text_input_buffers.get(&widget_id) else {
            return (None, None);
        };

        let text = Some(state.current_text.as_str());
        let layout = state.layout_snapshot.as_ref();
        let _ = focused_text_state;
        (text, layout)
    }

    pub(in crate::runtime) fn stable_text_layout_overrides<'a>(
        text_input_buffers: &'a HashMap<WidgetId, TextInputBufferState>,
    ) -> HashMap<WidgetId, TextInputLayoutOverride<'a>> {
        text_input_buffers
            .iter()
            .filter_map(|(widget_id, state)| {
                let layout = state.layout_snapshot.as_ref()?;
                if state.has_unresolved_local_edits() || state.display_text != state.current_text()
                {
                    return None;
                }
                Some((
                    *widget_id,
                    TextInputLayoutOverride {
                        revision: state.external_revision,
                        text: state.current_text(),
                        layout,
                    },
                ))
            })
            .collect()
    }

    pub(in crate::runtime) fn sync_text_inputs_from_computed(
        &mut self,
        computed: &ComputedScene<VM>,
    ) {
        let mut widget_ids = smallvec::SmallVec::<[WidgetId; 4]>::new();
        self.text_input_regions.clear();
        self.text_input_flush_data.clear();
        for region in computed
            .hit_regions
            .iter()
            .chain(computed.overlay_hit_regions.iter())
        {
            let HitInteraction::TextInput {
                id,
                controller,
                frame,
                padding,
                text_style,
                multiline,
                auto_wrap,
                show_scrollbar,
                on_change,
                on_change_set,
                ..
            } = &region.interaction
            else {
                continue;
            };

            widget_ids.push(*id);
            self.text_input_regions.insert(
                *id,
                TextInputRegionData {
                    controller: controller.clone(),
                    frame: *frame,
                    padding: *padding,
                    text_style: text_style.clone(),
                    multiline: *multiline,
                    auto_wrap: *auto_wrap,
                    show_scrollbar: *show_scrollbar,
                    on_change: on_change.clone(),
                    on_change_set: on_change_set.clone(),
                },
            );
            self.text_input_flush_data.insert(
                *id,
                TextInputFlushData {
                    controller: controller.clone(),
                    on_change: on_change.clone(),
                    on_change_set: on_change_set.clone(),
                },
            );
        }

        if widget_ids.len() <= 8 {
            self.text_input_buffers
                .retain(|widget_id, _| widget_ids.contains(widget_id));
        } else {
            let active_ids: std::collections::HashSet<_> = widget_ids.iter().copied().collect();
            self.text_input_buffers
                .retain(|widget_id, _| active_ids.contains(widget_id));
        }

        for widget_id in widget_ids {
            let _ = self.sync_text_input_buffer(widget_id);
        }
    }

    pub(in crate::runtime) fn caret_visible_at(
        &self,
        now: Instant,
        focused_text_input: Option<WidgetId>,
    ) -> bool {
        focused_text_input.is_some()
            && ((now.duration_since(self.caret_blink_origin).as_millis()
                / CARET_BLINK_INTERVAL.as_millis())
                % 2
                == 0)
    }

    pub(in crate::runtime) fn caret_blink_needs_redraw(&self, now: Instant) -> bool {
        let Some(cached) = self.cached_scene.as_ref() else {
            return false;
        };
        let focused_input = self.focused_text_input_id_cached(&cached.computed);
        cached.caret_visible != self.caret_visible_at(now, focused_input)
    }

    pub(in crate::runtime) fn next_caret_blink_deadline(&self, now: Instant) -> Option<Instant> {
        let focused = self.focused_widget_id()?;
        if !self.text_input_regions.contains_key(&focused) {
            return None;
        }
        let elapsed = now.saturating_duration_since(self.caret_blink_origin);
        let interval_ms = CARET_BLINK_INTERVAL.as_millis() as u64;
        let elapsed_ms = elapsed.as_millis() as u64;
        let next_step = (elapsed_ms / interval_ms) + 1;
        Some(self.caret_blink_origin + Duration::from_millis(next_step * interval_ms))
    }
}
