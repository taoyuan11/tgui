use super::super::*;
use super::centered_text_frame;
use std::sync::Arc;

pub(crate) fn push_text_primitives(
    text: &Text,
    frame: Rect,
    font_manager: &FontManager,
    theme: &Theme,
    units: UnitContext,
    animations: &mut AnimationEngine,
    now: std::time::Instant,
    scene: &mut ScenePrimitives,
    show_caret: bool,
    center_horizontally: bool,
    padding: Insets,
    caret_content: Option<&str>,
    selection_state: Option<&TextEditState>,
    fallback_color: Color,
    opacity: f32,
    widget_id: WidgetId,
    clip_rect: Option<Rect>,
    clip_mask: Option<ClipMask>,
) {
    let started_at = crate::log::text_profile_enabled().then_some(std::time::Instant::now());
    let resolve_content_started_at =
        crate::log::text_profile_enabled().then_some(std::time::Instant::now());
    let content = text.content.resolve();
    let resolve_content_elapsed_ms = resolve_content_started_at
        .map(|started_at| started_at.elapsed().as_secs_f64() * 1000.0)
        .unwrap_or(0.0);
    let default_style = &theme.typography.body;
    let text_request = TextFontRequest {
        preferred_font: text
            .font_family
            .as_deref()
            .or(default_style.font_family.as_deref()),
        weight: text.font_weight.unwrap_or(default_style.weight),
    };
    let resolve_font_started_at =
        crate::log::text_profile_enabled().then_some(std::time::Instant::now());
    let resolved = font_manager.resolve_text(&content, text_request.clone());
    let resolve_font_elapsed_ms = resolve_font_started_at
        .map(|started_at| started_at.elapsed().as_secs_f64() * 1000.0)
        .unwrap_or(0.0);

    let color = text
        .color
        .as_ref()
        .map(|color| color.resolve_widget(animations, widget_id, WidgetProperty::TextColor, now))
        .unwrap_or(fallback_color);
    let (font_size, line_height, letter_spacing) = resolved_text_metrics(text, theme, units);
    let inner = frame.inset(padding);
    let requires_precise_layout = selection_state.is_some() || show_caret || center_horizontally;
    let layout_started_at = crate::log::text_profile_enabled().then_some(std::time::Instant::now());
    let current_layout = requires_precise_layout.then(|| {
        font_manager.measure_text_layout(
            &content,
            text_request.clone(),
            font_size,
            line_height,
            letter_spacing,
        )
    });
    let layout_elapsed_ms = layout_started_at
        .map(|started_at| started_at.elapsed().as_secs_f64() * 1000.0)
        .unwrap_or(0.0);
    let content_frame = if let Some(current_layout) = current_layout.as_ref() {
        centered_text_frame(
            inner,
            current_layout.width,
            current_layout.height,
            line_height,
            center_horizontally,
        )
    } else {
        centered_text_frame(inner, inner.width.get(), line_height, line_height, false)
    };
    let primary_font = resolved.primary_font.clone();

    if let Some(current_layout) = current_layout.as_ref() {
        if let Some((selection_start, selection_end)) = selection_state
            .cloned()
            .unwrap_or_else(|| TextEditState::caret_at(&content))
            .clamped_to(&content)
            .selection_range()
        {
            let selection_start = selection_start.min(content.len());
            let selection_end = selection_end.min(content.len());
            let selection_start_x = current_layout.x_for_index(selection_start);
            let selection_end_x = current_layout.x_for_index(selection_end);
            let selection_width = (selection_end_x - selection_start_x).max(0.0);
            if selection_width > 0.0 {
                scene.push_shape(RenderPrimitive {
                    rect: Rect::new(
                        content_frame.x + selection_start_x,
                        content_frame.y,
                        selection_width,
                        content_frame.height.max(Dp::new(line_height)),
                    ),
                    color: theme.colors.selection.with_alpha_factor(opacity),
                    corner_radius: 4.0,
                    stroke_width: 0.0,
                    clip_rect,
                    clip_mask,
                });
            }
        }
    }

    scene.push_text(TextPrimitive {
        content: Arc::from(content.clone()),
        rich_spans: None,
        frame: content_frame,
        quad: None,
        color: color.with_alpha_factor(opacity),
        force_color: false,
        font_family: Some(Arc::from(primary_font.clone())),
        font_size,
        font_weight: text.font_weight.unwrap_or(default_style.weight),
        line_height,
        letter_spacing,
        wrap: crate::ui::widget::CanvasTextWrap::Word,
        overflow: crate::ui::widget::CanvasTextOverflow::Clip,
        horizontal_align: crate::ui::widget::CanvasTextHorizontalAlign::Start,
        vertical_align: crate::ui::widget::CanvasTextVerticalAlign::Start,
        clip_rect,
        clip_mask,
    });

    if show_caret {
        let current_layout = current_layout
            .as_ref()
            .expect("text caret rendering requires precise layout");
        let caret_width = caret_content
            .map(|caret_text| {
                font_manager.measure_text_raw(
                    caret_text,
                    text_request,
                    font_size,
                    line_height,
                    letter_spacing,
                )
            })
            .map(|(width, _)| width)
            .unwrap_or(current_layout.width);
        let caret_x = (inner.x + inner.width.min(caret_width) + CARET_END_GAP).max(inner.x);
        scene.push_overlay_shape(RenderPrimitive {
            rect: Rect::new(
                caret_x,
                content_frame.y,
                CARET_WIDTH,
                content_frame.height.max(Dp::new(line_height)),
            ),
            color: theme.colors.on_surface.with_alpha_factor(opacity),
            corner_radius: 0.0,
            stroke_width: 0.0,
            clip_rect,
            clip_mask,
        });
    }

    if let Some(started_at) = started_at {
        let elapsed = started_at.elapsed();
        if elapsed >= std::time::Duration::from_millis(1)
            || resolve_font_elapsed_ms >= 1.0
            || layout_elapsed_ms >= 1.0
        {
            let mut preview = content.replace('\n', "\\n");
            if preview.len() > 48 {
                preview.truncate(48);
                preview.push_str("...");
            }
            crate::log::log_text_profile(
                "textarea_text_widget",
                elapsed,
                format!(
                    "widget={:?} len={} resolve_content_ms={:.3} resolve_font_ms={:.3} layout_ms={:.3} fast_path={} selection={} caret={} center={} font={} preview={:?}",
                    widget_id,
                    content.len(),
                    resolve_content_elapsed_ms,
                    resolve_font_elapsed_ms,
                    layout_elapsed_ms,
                    !requires_precise_layout,
                    selection_state.is_some(),
                    show_caret,
                    center_horizontally,
                    primary_font,
                    preview,
                ),
            );
        }
    }
}
