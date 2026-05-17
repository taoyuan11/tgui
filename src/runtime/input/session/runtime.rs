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
    ) -> (Option<&'a str>, Option<&'a crate::text::font::TextLayoutInfo>) {
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

    pub(in crate::runtime) fn sync_text_input_regions_from_computed(
        &mut self,
        computed: &ComputedScene<VM>,
    ) {
        let mut regions = HashMap::new();
        let mut flush_data = HashMap::new();
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

            regions.insert(
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
            flush_data.insert(
                *id,
                TextInputFlushData {
                    controller: controller.clone(),
                    on_change: on_change.clone(),
                    on_change_set: on_change_set.clone(),
                },
            );
        }
        self.text_input_regions = regions;
        self.text_input_flush_data = flush_data;
    }

    pub(in crate::runtime) fn sync_visible_text_input_buffers(
        &mut self,
        computed: &ComputedScene<VM>,
    ) {
        let widget_ids: Vec<_> = computed
            .hit_regions
            .iter()
            .chain(computed.overlay_hit_regions.iter())
            .filter_map(|region| match &region.interaction {
                HitInteraction::TextInput { id, .. } => Some(*id),
                _ => None,
            })
            .collect();
        for widget_id in widget_ids {
            let _ = self.sync_text_input_buffer(widget_id);
        }
    }

    pub(in crate::runtime) fn prune_text_input_buffers(&mut self, computed: &ComputedScene<VM>) {
        let active_ids: HashSet<_> = computed
            .hit_regions
            .iter()
            .chain(computed.overlay_hit_regions.iter())
            .filter_map(|region| match &region.interaction {
                HitInteraction::TextInput { id, .. } => Some(*id),
                _ => None,
            })
            .collect();
        self.text_input_buffers
            .retain(|widget_id, _| active_ids.contains(widget_id));
        self.text_input_regions
            .retain(|widget_id, _| active_ids.contains(widget_id));
        self.text_input_flush_data
            .retain(|widget_id, _| active_ids.contains(widget_id));
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
