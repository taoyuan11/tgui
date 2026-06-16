use super::super::*;

#[allow(clippy::too_many_arguments)]
pub(crate) struct TextInputRenderOutput {
    pub(crate) ime_cursor_area: Option<Rect>,
    pub(crate) content_width: Dp,
    pub(crate) content_height: Dp,
}

fn multiline_text_slot_count(content_viewport: Rect, line_height: f32) -> usize {
    let line_height = line_height.max(1.0);
    ((content_viewport.height.get().max(0.0) / line_height).ceil() as usize)
        .saturating_add(2)
        .max(1)
}

fn multiline_visible_text_slots<'a>(
    layout: &TextLayoutInfo,
    display_content: &'a str,
    content_frame: Rect,
    content_viewport: Rect,
    line_height: f32,
    slot_count: usize,
) -> Vec<(&'a str, Rect)> {
    let viewport_top = content_viewport.y.get();
    let viewport_bottom = content_viewport.bottom().get();
    let visible_range = layout.line_range_for_vertical_span(
        viewport_top - content_frame.y.get(),
        viewport_bottom - content_frame.y.get(),
    );
    let start_line = visible_range.start;
    let mut slots = Vec::with_capacity(slot_count);
    for slot_index in 0..slot_count {
        let line_index = start_line + slot_index;
        if line_index < visible_range.end {
            let start = layout.line_start(line_index).min(display_content.len());
            let end = layout.line_end(line_index).min(display_content.len());
            let line_top = content_frame.y.get() + layout.line_top(line_index);
            let line_height_value = layout.line_height(line_index).max(line_height);
            let content = if start < end {
                &display_content[start..end]
            } else {
                ""
            };
            let width = if start < end {
                layout.line_width(line_index).max(1.0)
            } else {
                1.0
            };
            slots.push((
                content,
                Rect::new(content_frame.x, line_top, width, line_height_value),
            ));
        } else {
            slots.push((
                "",
                Rect::new(
                    content_frame.x,
                    content_viewport.y.get() + slot_index as f32 * line_height.max(1.0),
                    1.0,
                    line_height.max(1.0),
                ),
            ));
        }
    }
    slots
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn push_text_input_primitives(
    content: &str,
    text: &Text,
    frame: Rect,
    font_manager: &FontManager,
    theme: &Theme,
    units: UnitContext,
    animations: &mut AnimationEngine,
    now: std::time::Instant,
    scene: &mut ScenePrimitives,
    show_caret: bool,
    _caret_visible: bool,
    multiline: bool,
    auto_wrap: bool,
    show_scrollbar: bool,
    padding: Insets,
    scroll_offset: Point,
    edit_state: Option<&TextEditState>,
    fallback_color: Color,
    selection_color: Option<Color>,
    caret_color: Option<Color>,
    opacity: f32,
    widget_id: WidgetId,
    precomputed_layout: Option<&TextLayoutInfo>,
    clip_rect: Option<Rect>,
    clip_mask: Option<ClipMask>,
) -> TextInputRenderOutput {
    let started_at = crate::log::text_profile_enabled().then_some(std::time::Instant::now());
    let default_style = &theme.typography.body;
    let text_request = TextFontRequest {
        preferred_font: text
            .font_family
            .as_deref()
            .or(default_style.font_family.as_deref()),
        weight: text.font_weight.unwrap_or(default_style.weight),
    };
    let resolved_font = font_manager.resolve_text(content, text_request.clone());

    let text_color = text
        .color
        .as_ref()
        .map(|color| color.resolve_widget(animations, widget_id, WidgetProperty::TextColor, now))
        .unwrap_or(fallback_color);
    let (font_size, line_height, letter_spacing) = resolved_text_metrics(text, theme, units);
    let content_viewport =
        text_input_content_viewport(frame, padding, multiline, show_scrollbar, theme, units);
    let content_clip_rect = clip_rect
        .map(|clip| clip.intersect(content_viewport))
        .unwrap_or(Some(content_viewport));
    let wrap_width = text_input_layout_width(content_viewport, multiline, auto_wrap, CARET_WIDTH);
    let base_state = edit_state
        .cloned()
        .unwrap_or_else(|| TextEditState {
            scroll_x: scroll_offset.x,
            scroll_y: scroll_offset.y,
            ..TextEditState::caret_at(content)
        })
        .clamped_to(content);

    let (display_content, display_state, composition_range) =
        if let Some(composition) = base_state.composition.as_ref() {
            let start = composition.replace_range.0.min(content.len());
            let end = composition.replace_range.1.min(content.len());
            let mut display = String::with_capacity(
                content.len() + composition.text.len().saturating_sub(end - start),
            );
            display.push_str(&content[..start]);
            display.push_str(&composition.text);
            display.push_str(&content[end..]);
            let composition_end = start + composition.text.len();
            let caret_offset = composition
                .cursor
                .map(|(_, end)| end.min(composition.text.len()))
                .unwrap_or(composition.text.len());
            let caret = start + caret_offset;
            (
                std::borrow::Cow::Owned(display),
                TextEditState {
                    cursor: caret,
                    anchor: caret,
                    composition: None,
                    scroll_x: base_state.scroll_x,
                    scroll_y: base_state.scroll_y,
                    preferred_column_x: base_state.preferred_column_x,
                },
                Some((start, composition_end)),
            )
        } else {
            (
                std::borrow::Cow::Borrowed(content),
                base_state.clone(),
                None,
            )
        };

    let layout_started_at = std::time::Instant::now();
    let measured_layout;
    let layout = if let Some(layout) = precomputed_layout {
        layout
    } else {
        measured_layout = if multiline && auto_wrap {
            font_manager.measure_text_layout_wrapped(
                &display_content,
                text_request.clone(),
                font_size,
                line_height,
                letter_spacing,
                wrap_width,
            )
        } else {
            font_manager.measure_text_layout(
                &display_content,
                text_request.clone(),
                font_size,
                line_height,
                letter_spacing,
            )
        };
        &measured_layout
    };
    let layout_duration = layout_started_at.elapsed();

    let TextInputContentGeometry {
        content_frame,
        content_width,
        content_height,
        ..
    } = text_input_content_geometry(
        layout,
        line_height,
        content_viewport,
        multiline,
        auto_wrap,
        Point::new(display_state.scroll_x, display_state.scroll_y),
        CARET_WIDTH,
    );

    let selection_fill = selection_color.unwrap_or(theme.colors.selection);
    let caret_fill = caret_color.unwrap_or(theme.colors.on_surface);
    let mut selection_segments = Vec::new();
    if let Some((selection_start, selection_end)) = display_state.selection_range() {
        let start = selection_start.min(display_content.len());
        let end = selection_end.min(display_content.len());
        if start < end {
            let start_line = layout.line_index_for_index(start);
            let end_line = layout.line_index_for_index(end);
            for line_index in start_line..=end_line {
                let line_start = start.max(layout.line_start(line_index));
                let line_end = end.min(layout.line_end(line_index));
                let x0 = layout.x_for_index(line_start);
                let x1 = layout.x_for_index(line_end);
                let width = (x1 - x0).max(0.0);
                if width <= 0.0 {
                    continue;
                }
                selection_segments.push(Rect::new(
                    content_frame.x + x0,
                    content_frame.y + Dp::new(layout.line_top(line_index)),
                    width,
                    Dp::new(layout.line_height(line_index)),
                ));
            }
        }
    }
    if let Some((composition_start, composition_end)) = composition_range {
        let start_line = layout.line_index_for_index(composition_start);
        let end_line = layout.line_index_for_index(composition_end);
        for line_index in start_line..=end_line {
            let line_start = composition_start.max(layout.line_start(line_index));
            let line_end = composition_end.min(layout.line_end(line_index));
            let x0 = layout.x_for_index(line_start);
            let x1 = layout.x_for_index(line_end);
            let width = (x1 - x0).max(0.0);
            if width <= 0.0 {
                continue;
            }
            selection_segments.push(Rect::new(
                content_frame.x + x0,
                content_frame.y + Dp::new(layout.line_top(line_index)),
                width,
                Dp::new(layout.line_height(line_index)),
            ));
        }
    }
    let fixed_selection_slot = show_caret;
    if fixed_selection_slot {
        let (segments, color) = if selection_segments.is_empty() {
            (
                Arc::from(vec![Rect::new(
                    content_frame.x,
                    content_viewport.y,
                    0.0,
                    Dp::new(line_height),
                )]),
                selection_fill.with_alpha_factor(opacity),
            )
        } else {
            (
                Arc::from(vec![selection_segments[0]]),
                selection_fill.with_alpha_factor(opacity),
            )
        };
        scene.push_text_decoration(TextDecorationPrimitive {
            segments,
            color,
            corner_radius: 4.0,
            stroke_width: 0.0,
            clip_rect: content_clip_rect,
            clip_mask,
        });
    } else if !selection_segments.is_empty() {
        scene.push_text_decoration(TextDecorationPrimitive {
            segments: Arc::from(selection_segments),
            color: selection_fill.with_alpha_factor(opacity),
            corner_radius: 4.0,
            stroke_width: 0.0,
            clip_rect: content_clip_rect,
            clip_mask,
        });
    }
    let text_color = text_color.with_alpha_factor(opacity);
    let font_family = Some(resolved_font.primary_font);
    let font_weight = text.font_weight.unwrap_or(default_style.weight);

    if multiline {
        let slot_count = multiline_text_slot_count(content_viewport, line_height);
        for (line_content, line_frame) in multiline_visible_text_slots(
            layout,
            &display_content,
            content_frame,
            content_viewport,
            line_height,
            slot_count,
        ) {
            scene.push_text(TextPrimitive {
                content: Arc::from(line_content.to_string()),
                rich_spans: None,
                frame: line_frame,
                quad: None,
                color: text_color,
                force_color: false,
                font_family: font_family.clone().map(Arc::from),
                font_size,
                font_weight,
                line_height,
                letter_spacing,
                wrap: crate::ui::widget::CanvasTextWrap::None,
                overflow: crate::ui::widget::CanvasTextOverflow::Clip,
                horizontal_align: crate::ui::widget::CanvasTextHorizontalAlign::Start,
                vertical_align: crate::ui::widget::CanvasTextVerticalAlign::Start,
                clip_rect: content_clip_rect,
                clip_mask,
            });
        }
    } else {
        scene.push_text(TextPrimitive {
            content: Arc::from(display_content.as_ref().to_string()),
            rich_spans: None,
            frame: content_frame,
            quad: None,
            color: text_color,
            force_color: false,
            font_family: font_family.map(Arc::from),
            font_size,
            font_weight,
            line_height,
            letter_spacing,
            wrap: crate::ui::widget::CanvasTextWrap::Word,
            overflow: crate::ui::widget::CanvasTextOverflow::Clip,
            horizontal_align: crate::ui::widget::CanvasTextHorizontalAlign::Start,
            vertical_align: crate::ui::widget::CanvasTextVerticalAlign::Start,
            clip_rect: content_clip_rect,
            clip_mask,
        });
    }

    let mut ime_cursor_area = None;
    if show_caret {
        let caret_index = display_state.cursor.min(display_content.len());
        let caret_x = if multiline && auto_wrap {
            (content_frame.x + layout.x_for_index(caret_index))
                .min((content_viewport.right() - CARET_WIDTH).max(content_viewport.x))
        } else {
            content_frame.x + layout.x_for_index(caret_index)
        };
        let caret_y = content_frame.y + Dp::new(layout.top_for_index(caret_index));
        let caret_height = Dp::new(layout.line_height_for_index(caret_index).max(line_height));
        let caret_rect = Rect::new(caret_x, caret_y, CARET_WIDTH, caret_height);
        ime_cursor_area = Some(caret_rect);
        scene.push_overlay_text_decoration(TextDecorationPrimitive {
            segments: Arc::from(vec![caret_rect]),
            color: caret_fill.with_alpha_factor(opacity),
            corner_radius: 0.0,
            stroke_width: 0.0,
            clip_rect: content_clip_rect,
            clip_mask,
        });
    }

    if let Some(started_at) = started_at {
        crate::log::log_text_profile(
            "push_text_input_primitives",
            started_at.elapsed(),
            format!(
                "widget={:?} multiline={} text_len={} layout_ms={:.3} width={:.1} height={:.1}",
                widget_id,
                multiline,
                display_content.len(),
                layout_duration.as_secs_f64() * 1000.0,
                frame.width.get(),
                frame.height.get(),
            ),
        );
    }

    TextInputRenderOutput {
        ime_cursor_area,
        content_width,
        content_height,
    }
}
