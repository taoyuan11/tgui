use std::collections::HashMap;
use std::time::{Duration, Instant};

use crate::foundation::binding::TextController;
use crate::log::text_profile_enabled;
use crate::ui::widget::{
    ComputedScene, HitInteraction, Point, TextEditState, TextInputLayoutOverride, WidgetId,
};
use cosmic_text::{Editor, Metrics, Wrap};

use super::super::{TextInputBufferState, CARET_BLINK_INTERVAL};
use super::text_input::{
    refresh_session_buffer, text_edit_display_text, update_session_layout_snapshot,
};
use super::{
    text_cursor_index_at_point, BoundRuntimeHandler, ScrollContext, TextInputContext,
    TextInputFlushData, TextInputRegionData, INPUT_CARET_WIDTH,
};

mod runtime;

pub(super) fn text_replacement_bounds(
    old_text: &str,
    new_text: &str,
) -> Option<(usize, usize, usize, usize)> {
    if old_text == new_text {
        return None;
    }

    let mut prefix = 0usize;
    let mut old_iter = old_text.chars();
    let mut new_iter = new_text.chars();
    loop {
        match (old_iter.next(), new_iter.next()) {
            (Some(old_char), Some(new_char)) if old_char == new_char => {
                prefix += old_char.len_utf8();
            }
            _ => break,
        }
    }

    let old_remaining = &old_text[prefix..];
    let new_remaining = &new_text[prefix..];
    let mut suffix = 0usize;
    let mut old_rev = old_remaining.chars().rev();
    let mut new_rev = new_remaining.chars().rev();
    loop {
        match (old_rev.next(), new_rev.next()) {
            (Some(old_char), Some(new_char))
                if old_char == new_char
                    && suffix + old_char.len_utf8() <= old_remaining.len()
                    && suffix + new_char.len_utf8() <= new_remaining.len() =>
            {
                suffix += old_char.len_utf8();
            }
            _ => break,
        }
    }

    Some((
        prefix,
        old_text.len().saturating_sub(suffix),
        prefix,
        new_text.len().saturating_sub(suffix),
    ))
}

impl<VM: 'static> BoundRuntimeHandler<VM> {
    pub(super) fn text_input_session_config(
        &self,
        region: &TextInputRegionData<VM>,
    ) -> (
        super::super::TextInputSessionConfig,
        Option<String>,
        crate::text::font::FontWeight,
        f32,
        f32,
        f32,
        f32,
        f32,
    ) {
        let content_viewport = crate::ui::widget::text_input_content_viewport(
            region.frame,
            region.padding,
            region.multiline,
            region.show_scrollbar,
            &self.theme,
            self.unit_context(),
        );
        let (request, font_size, line_height, letter_spacing) =
            super::super::resolved_input_text_metrics(
                &self.theme,
                self.unit_context(),
                &region.text_style,
            );
        let layout_width = crate::ui::widget::text_input_layout_width(
            content_viewport,
            region.multiline,
            region.auto_wrap,
            INPUT_CARET_WIDTH,
        );
        let preferred_font = request.preferred_font.map(ToString::to_string);
        (
            super::super::TextInputSessionConfig {
                font_family: preferred_font.clone(),
                font_weight: request.weight,
                font_size_bits: font_size.to_bits(),
                line_height_bits: line_height.to_bits(),
                letter_spacing_bits: letter_spacing.to_bits(),
                width_bits: layout_width.to_bits(),
                multiline: region.multiline,
                auto_wrap: region.auto_wrap,
            },
            preferred_font,
            request.weight,
            font_size,
            line_height,
            letter_spacing,
            layout_width,
            content_viewport.height.get().max(0.0),
        )
    }

    pub(super) fn create_text_input_session(
        &self,
        region: &TextInputRegionData<VM>,
    ) -> super::super::TextInputBufferState {
        let started_at = text_profile_enabled().then_some(Instant::now());
        let snapshot = region.controller.snapshot();
        let (
            config,
            _preferred_font,
            _weight,
            font_size,
            line_height,
            _letter_spacing,
            width,
            height,
        ) = self.text_input_session_config(region);
        let buffer = self.font_manager.with_font_system(|font_system| {
            let mut buffer =
                cosmic_text::Buffer::new(font_system, Metrics::new(font_size, line_height));
            buffer.set_size(Some(width), Some(height.max(line_height)));
            buffer.set_wrap(if region.multiline && region.auto_wrap {
                Wrap::WordOrGlyph
            } else {
                Wrap::None
            });
            buffer
        });
        let mut session = super::super::TextInputBufferState::new(
            Editor::new(buffer),
            snapshot.text,
            snapshot.revision,
        );
        session.config = Some(config);
        let display_text = session.display_text.clone();
        update_session_layout_snapshot(
            &self.font_manager,
            &mut session,
            &display_text,
            line_height,
        );
        let _ = started_at;
        session
    }

    pub(super) fn cached_text_input_region_data(
        &self,
        widget_id: WidgetId,
    ) -> Option<TextInputRegionData<VM>> {
        self.text_input_regions.get(&widget_id).cloned()
    }

    pub(super) fn cached_text_input_flush_data(
        &self,
        widget_id: WidgetId,
    ) -> Option<TextInputFlushData<VM>> {
        self.text_input_flush_data.get(&widget_id).cloned()
    }

    pub(super) fn text_input_region_data(
        &mut self,
        widget_id: WidgetId,
    ) -> Option<TextInputRegionData<VM>> {
        if let Some(region) = self.cached_text_input_region_data(widget_id) {
            return Some(region);
        }
        let computed = self.computed_scene();
        let region = computed
            .hit_regions
            .iter()
            .chain(computed.overlay_hit_regions.iter())
            .find_map(|region| match &region.interaction {
                HitInteraction::TextInput {
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
                } if *id == widget_id => Some(TextInputRegionData {
                    controller: controller.clone(),
                    frame: *frame,
                    padding: *padding,
                    text_style: text_style.clone(),
                    multiline: *multiline,
                    auto_wrap: *auto_wrap,
                    show_scrollbar: *show_scrollbar,
                    on_change: on_change.clone(),
                    on_change_set: on_change_set.clone(),
                }),
                _ => None,
            });
        if let Some(region) = region.clone() {
            self.text_input_regions.insert(widget_id, region);
        }
        region
    }

    #[cfg(test)]
    pub(crate) fn text_input_session_config_signature_for_test(
        &mut self,
        widget_id: WidgetId,
        frame: crate::ui::widget::Rect,
    ) -> Option<(u64, f32)> {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut region = self.text_input_region_data(widget_id)?;
        region.frame = frame;
        let (config, _, _, _, _, _, _, height) = self.text_input_session_config(&region);
        let mut hasher = DefaultHasher::new();
        config.hash(&mut hasher);
        Some((hasher.finish(), height))
    }

    pub(crate) fn text_input_current_value(
        &self,
        widget_id: WidgetId,
        controller: &TextController,
    ) -> String {
        self.text_input_buffers
            .get(&widget_id)
            .map(|session| session.current_text.clone())
            .unwrap_or_else(|| controller.text())
    }

    pub(crate) fn refresh_text_input_session_display(
        &mut self,
        widget_id: WidgetId,
        region: &TextInputRegionData<VM>,
        current_value: &str,
        state: &TextEditState,
    ) {
        let (config, preferred_font, weight, font_size, line_height, letter_spacing, width, height) =
            self.text_input_session_config(region);
        let Some(session) = self.text_input_buffers.get_mut(&widget_id) else {
            return;
        };
        let display_text = text_edit_display_text(current_value, state);
        refresh_session_buffer(
            &self.font_manager,
            session,
            config,
            preferred_font.as_deref(),
            weight,
            font_size,
            line_height,
            letter_spacing,
            width,
            height,
            region.multiline,
            region.auto_wrap,
            state,
            display_text.as_ref(),
            None,
        );
    }

    pub(super) fn text_input_layout_snapshot(
        &self,
        widget_id: WidgetId,
    ) -> Option<&crate::text::font::TextLayoutInfo> {
        self.text_input_buffers
            .get(&widget_id)
            .and_then(|session| session.layout_snapshot.as_ref())
    }

    pub(super) fn text_input_cursor_index_at_point(
        &self,
        widget_id: WidgetId,
        input: TextInputContext<'_>,
        scroll: ScrollContext,
        point: Point,
    ) -> usize {
        if input.text.is_empty() {
            return 0;
        }

        let (_, _, line_height, _) = super::super::resolved_input_text_metrics(
            &self.theme,
            self.unit_context(),
            input.text_style,
        );
        let content_viewport = input.content_viewport(&self.theme, self.unit_context());
        if let Some(layout) = self.text_input_buffers.get(&widget_id).and_then(|session| {
            (session.display_text == input.text)
                .then_some(session.layout_snapshot.as_ref())
                .flatten()
        }) {
            return super::super::text_cursor_index_from_layout_at_point(
                layout,
                line_height,
                content_viewport,
                input,
                scroll,
                point,
            );
        }

        text_cursor_index_at_point(
            &self.font_manager,
            &self.theme,
            self.unit_context(),
            input,
            scroll,
            point,
        )
    }

    pub(crate) fn sync_text_input_buffer(
        &mut self,
        widget_id: WidgetId,
    ) -> Option<TextInputRegionData<VM>> {
        let region = self.text_input_region_data(widget_id)?;
        if !self.text_input_buffers.contains_key(&widget_id) {
            let session = self.create_text_input_session(&region);
            self.text_input_buffers.insert(widget_id, session);
        }

        let mut state = self.text_edit_state(widget_id).cloned().unwrap_or_else(|| {
            let current_text = self
                .text_input_buffers
                .get(&widget_id)
                .map(|session| session.current_text.as_str())
                .unwrap_or("");
            self.default_text_edit_state(widget_id, current_text)
        });
        let (config, preferred_font, weight, font_size, line_height, letter_spacing, width, height) =
            self.text_input_session_config(&region);
        {
            let session = self
                .text_input_buffers
                .get_mut(&widget_id)
                .expect("text input session should exist");
            let mut text_changed = false;
            let controller_revision = region.controller.revision();
            if session.external_revision != controller_revision {
                let snapshot = region.controller.snapshot();
                if session.current_text == snapshot.text {
                    session.external_value = snapshot.text;
                    session.external_revision = snapshot.revision;
                } else {
                    session.external_value = snapshot.text.clone();
                    session.external_revision = snapshot.revision;
                    session.current_text = snapshot.text.clone();
                    session.rope = ropey::Rope::from_str(&snapshot.text);
                    session.pending_changes.clear();
                    session.pending_start_revision = None;
                    state = state.clamped_to(&snapshot.text);
                    text_changed = true;
                }
            }
            state = state.clamped_to(session.current_text());
            let config_changed = session.config.as_ref() != Some(&config);
            let should_refresh = text_changed
                || config_changed
                || state.composition.is_some()
                || session.layout_snapshot.is_none();
            if should_refresh {
                let current_text = std::mem::take(&mut session.current_text);
                let display_text = text_edit_display_text(current_text.as_str(), &state);
                refresh_session_buffer(
                    &self.font_manager,
                    session,
                    config,
                    preferred_font.as_deref(),
                    weight,
                    font_size,
                    line_height,
                    letter_spacing,
                    width,
                    height,
                    region.multiline,
                    region.auto_wrap,
                    &state,
                    display_text.as_ref(),
                    None,
                );
                session.current_text = current_text;
            }
        }
        self.text_edit_states.insert(widget_id, state);
        Some(region)
    }
}
