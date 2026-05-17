use std::borrow::Cow;
use std::time::Instant;

use crate::foundation::binding::TextChangeSet;
use crate::foundation::view_model::{Command, ValueCommand};
use crate::log::text_profile_enabled;
use crate::text::font::{build_layout_info_from_buffer, FontManager, TextFontRequest};
use crate::ui::widget::{Rect, Text, TextEditState};
use cosmic_text::Edit;

use super::text_replacement_bounds;
use super::{super::TextInputBufferState, super::TextInputSessionConfig};

pub(crate) struct TextInputRegionData<VM> {
    pub(crate) controller: crate::foundation::binding::TextController,
    pub(crate) frame: Rect,
    pub(crate) padding: crate::ui::layout::Insets,
    pub(crate) text_style: Text,
    pub(crate) multiline: bool,
    pub(crate) auto_wrap: bool,
    pub(crate) show_scrollbar: bool,
    pub(crate) on_change: Option<Command<VM>>,
    pub(crate) on_change_set: Option<ValueCommand<VM, TextChangeSet>>,
}

impl<VM> Clone for TextInputRegionData<VM> {
    fn clone(&self) -> Self {
        Self {
            controller: self.controller.clone(),
            frame: self.frame,
            padding: self.padding,
            text_style: self.text_style.clone(),
            multiline: self.multiline,
            auto_wrap: self.auto_wrap,
            show_scrollbar: self.show_scrollbar,
            on_change: self.on_change.clone(),
            on_change_set: self.on_change_set.clone(),
        }
    }
}

pub(crate) struct TextInputFlushData<VM> {
    pub(crate) controller: crate::foundation::binding::TextController,
    pub(crate) on_change: Option<Command<VM>>,
    pub(crate) on_change_set: Option<ValueCommand<VM, TextChangeSet>>,
}

impl<VM> Clone for TextInputFlushData<VM> {
    fn clone(&self) -> Self {
        Self {
            controller: self.controller.clone(),
            on_change: self.on_change.clone(),
            on_change_set: self.on_change_set.clone(),
        }
    }
}

#[derive(Clone, Copy, Default)]
pub(crate) struct TextInputFlushOutcome {
    pub(crate) changed: bool,
    pub(crate) requires_global_invalidation: bool,
}

pub(super) fn text_edit_display_text<'a>(text: &'a str, state: &TextEditState) -> Cow<'a, str> {
    if let Some(composition) = state.composition.as_ref() {
        let start = composition.replace_range.0.min(text.len());
        let end = composition.replace_range.1.min(text.len());
        let mut display =
            String::with_capacity(text.len() + composition.text.len().saturating_sub(end - start));
        display.push_str(&text[..start]);
        display.push_str(&composition.text);
        display.push_str(&text[end..]);
        Cow::Owned(display)
    } else {
        Cow::Borrowed(text)
    }
}

pub(super) fn update_session_layout_snapshot(
    font_manager: &FontManager,
    session: &mut TextInputBufferState,
    display_text: &str,
    line_height: f32,
) {
    session.layout_snapshot = Some(if let Some(config) = session.config.as_ref() {
        let request = TextFontRequest {
            preferred_font: config.font_family.as_deref(),
            weight: config.font_weight,
        };
        let font_size = f32::from_bits(config.font_size_bits);
        let line_height = f32::from_bits(config.line_height_bits);
        let letter_spacing = f32::from_bits(config.letter_spacing_bits);
        let wrap_width = f32::from_bits(config.width_bits).max(0.0);
        if config.multiline && config.auto_wrap {
            font_manager.measure_text_layout_wrapped(
                display_text,
                request,
                font_size,
                line_height,
                letter_spacing,
                wrap_width,
            )
        } else {
            font_manager.measure_text_layout(
                display_text,
                request,
                font_size,
                line_height,
                letter_spacing,
            )
        }
    } else {
        session
            .editor
            .with_buffer(|buffer| build_layout_info_from_buffer(buffer, display_text, line_height))
    });
    session.display_text.clear();
    session.display_text.push_str(display_text);
}

#[allow(clippy::too_many_arguments)]
pub(super) fn refresh_session_buffer(
    font_manager: &FontManager,
    session: &mut TextInputBufferState,
    config: TextInputSessionConfig,
    preferred_font: Option<&str>,
    weight: crate::text::font::FontWeight,
    font_size: f32,
    line_height: f32,
    letter_spacing: f32,
    width: f32,
    _height: f32,
    multiline: bool,
    auto_wrap: bool,
    _text_state: &TextEditState,
    display_text: &str,
    edit_replacement: Option<(usize, usize, usize, usize)>,
) {
    let started_at = text_profile_enabled().then_some(Instant::now());
    let config_changed = session.config.as_ref() != Some(&config);
    let text_changed = edit_replacement.is_some() || session.display_text != display_text;

    if !config_changed && !text_changed {
        if session.layout_snapshot.is_none() {
            update_session_layout_snapshot(font_manager, session, display_text, line_height);
        }
        let _ = started_at;
        return;
    }

    session.config = Some(config);
    let layout_updated_incrementally = if multiline {
        let replacement = edit_replacement
            .or_else(|| text_replacement_bounds(&session.display_text, display_text));
        replacement
            .map(|replacement| {
                session.layout_snapshot.as_mut().is_some_and(|previous| {
                    font_manager.update_layout_after_edit(
                        previous,
                        &session.display_text,
                        display_text,
                        TextFontRequest {
                            preferred_font,
                            weight,
                        },
                        font_size,
                        line_height,
                        letter_spacing,
                        auto_wrap.then_some(width.max(0.0)),
                        replacement,
                    )
                })
            })
            .is_some_and(|updated| {
                if updated {
                    session.display_text.clear();
                    session.display_text.push_str(display_text);
                }
                updated
            })
    } else {
        false
    };
    if !layout_updated_incrementally {
        update_session_layout_snapshot(font_manager, session, display_text, line_height);
    }
    let _ = started_at;
}
