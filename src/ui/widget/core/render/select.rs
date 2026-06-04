use super::super::*;
use super::centered_text_frame;
use std::sync::Arc;

pub(crate) fn default_select_menu_transition() -> crate::animation::Transition {
    crate::animation::Transition::ease_in_out(std::time::Duration::from_millis(160))
}

pub(crate) fn measure_select_content(
    selected_label: Option<&str>,
    placeholder: &Value<String>,
    select_style: &ResolvedSelectStyle,
    font_manager: &FontManager,
    theme: &Theme,
    units: UnitContext,
) -> (f32, f32) {
    let display = selected_label
        .map(|label| select_display_text(text_from_content(label.to_string()), select_style))
        .unwrap_or_else(|| {
            select_display_text(text_from_content(placeholder.clone()), select_style)
        });
    let text_size = measure_text_content(&display, font_manager, theme, units);
    let horizontal = units.resolve_dp(select_style.padding_x) * 2.0 + units.resolve_dp(dp(24.0));
    let vertical = units.resolve_dp(select_style.padding_y) * 2.0;
    (
        SELECT_DEFAULT_WIDTH.max(text_size.0 + horizontal),
        text_size
            .1
            .max(units.resolve_dp(select_style.min_height))
            .max(text_size.1 + vertical),
    )
}

pub(crate) fn select_display_text(mut text: Text, select_style: &ResolvedSelectStyle) -> Text {
    if text.font_family.is_none() {
        text.font_family = select_style.text_style.font_family.clone();
    }
    if text.font_size.is_none() {
        text.font_size = Some(select_style.text_style.size);
    }
    if text.font_weight.is_none() {
        text.font_weight = Some(select_style.text_style.weight);
    }
    if text.letter_spacing.is_none() {
        text.letter_spacing = select_style.text_style.letter_spacing;
    }
    text
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn push_select_primitives(
    frame: Rect,
    selected_label: Option<String>,
    placeholder: &Value<String>,
    select_style: &ResolvedSelectStyle,
    font_manager: &FontManager,
    theme: &Theme,
    units: UnitContext,
    animations: &mut AnimationEngine,
    now: std::time::Instant,
    scene: &mut ScenePrimitives,
    padding: Insets,
    opacity: f32,
    widget_id: WidgetId,
    clip_rect: Option<Rect>,
    clip_mask: Option<ClipMask>,
) {
    let arrow_width = dp(24.0);
    let text_frame = Rect::new(
        frame.x,
        frame.y,
        (frame.width - arrow_width).max(Dp::ZERO),
        frame.height,
    );
    match selected_label {
        Some(label) => push_select_text(
            &select_display_text(text_from_content(label), select_style),
            text_frame,
            font_manager,
            theme,
            units,
            animations,
            now,
            scene,
            padding,
            select_style.text,
            opacity,
            widget_id,
            clip_rect,
            clip_mask,
            false,
        ),
        None => push_select_text(
            &select_display_text(text_from_content(placeholder.clone()), select_style),
            text_frame,
            font_manager,
            theme,
            units,
            animations,
            now,
            scene,
            padding,
            select_style.placeholder,
            opacity,
            widget_id,
            clip_rect,
            clip_mask,
            false,
        ),
    }

    push_select_icon(
        Rect::new(
            (frame.right() - arrow_width).max(frame.x),
            frame.y,
            arrow_width.min(frame.width),
            frame.height,
        ),
        font_manager,
        select_style,
        units,
        scene,
        opacity,
        clip_rect,
        clip_mask,
    );
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn build_select_text_primitive(
    text: &Text,
    frame: Rect,
    font_manager: &FontManager,
    theme: &Theme,
    units: UnitContext,
    animations: &mut AnimationEngine,
    now: std::time::Instant,
    padding: Insets,
    fallback_color: Color,
    opacity: f32,
    widget_id: WidgetId,
    clip_rect: Option<Rect>,
    clip_mask: Option<ClipMask>,
) -> TextPrimitive {
    let content = text.content.resolve();
    let default_style = &theme.typography.body;
    let text_request = TextFontRequest {
        preferred_font: text
            .font_family
            .as_deref()
            .or(default_style.font_family.as_deref()),
        weight: text.font_weight.unwrap_or(default_style.weight),
    };
    let resolved = font_manager.resolve_text(&content, text_request.clone());
    let color = text
        .color
        .as_ref()
        .map(|color| color.resolve_widget(animations, widget_id, WidgetProperty::TextColor, now))
        .unwrap_or(fallback_color)
        .with_alpha_factor(opacity);
    let (font_size, line_height, letter_spacing) = resolved_text_metrics(text, theme, units);
    let inner = frame.inset(padding);
    let layout = font_manager.measure_text_layout(
        &content,
        text_request,
        font_size,
        line_height,
        letter_spacing,
    );
    let content_frame = centered_text_frame(inner, layout.width, layout.height, line_height, false);
    TextPrimitive {
        content: Arc::from(content),
        rich_spans: None,
        frame: content_frame,
        quad: None,
        color,
        force_color: false,
        font_family: Some(Arc::from(resolved.primary_font)),
        font_size,
        font_weight: text.font_weight.unwrap_or(default_style.weight),
        line_height,
        letter_spacing,
        wrap: crate::ui::widget::CanvasTextWrap::None,
        overflow: crate::ui::widget::CanvasTextOverflow::Clip,
        horizontal_align: crate::ui::widget::CanvasTextHorizontalAlign::Start,
        vertical_align: crate::ui::widget::CanvasTextVerticalAlign::Start,
        clip_rect,
        clip_mask,
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn push_select_text(
    text: &Text,
    frame: Rect,
    font_manager: &FontManager,
    theme: &Theme,
    units: UnitContext,
    animations: &mut AnimationEngine,
    now: std::time::Instant,
    scene: &mut ScenePrimitives,
    padding: Insets,
    fallback_color: Color,
    opacity: f32,
    widget_id: WidgetId,
    clip_rect: Option<Rect>,
    clip_mask: Option<ClipMask>,
    overlay: bool,
) {
    let primitive = build_select_text_primitive(
        text,
        frame,
        font_manager,
        theme,
        units,
        animations,
        now,
        padding,
        fallback_color,
        opacity,
        widget_id,
        clip_rect,
        clip_mask,
    );
    if overlay {
        scene.push_overlay_text(primitive);
    } else {
        scene.push_text(primitive);
    }
}

pub(crate) fn push_select_icon(
    frame: Rect,
    font_manager: &FontManager,
    select_style: &ResolvedSelectStyle,
    units: UnitContext,
    scene: &mut ScenePrimitives,
    opacity: f32,
    clip_rect: Option<Rect>,
    clip_mask: Option<ClipMask>,
) {
    let font_size = units
        .resolve_sp(select_style.text_style.size)
        .min(frame.width.get())
        .min(frame.height.get())
        .max(1.0);
    let line_height = font_size;
    let letter_spacing = 0.0;
    let text_request = TextFontRequest {
        preferred_font: Some(ICON_FONT_FAMILY),
        weight: select_style.text_style.weight,
    };
    let resolved = font_manager.resolve_text(SELECT_ARROW_ICON, text_request.clone());
    let layout = font_manager.measure_text_layout(
        SELECT_ARROW_ICON,
        text_request,
        font_size,
        line_height,
        letter_spacing,
    );
    let mut icon_frame = centered_text_frame(
        frame,
        layout.width.max(font_size),
        layout.height.max(line_height),
        line_height,
        true,
    );
    icon_frame.y += dp(3.0);

    scene.push_text(TextPrimitive {
        content: Arc::from(SELECT_ARROW_ICON.to_string()),
        rich_spans: None,
        frame: icon_frame,
        quad: None,
        color: select_style.arrow.with_alpha_factor(opacity),
        force_color: true,
        font_family: Some(Arc::from(resolved.primary_font)),
        font_size,
        font_weight: select_style.text_style.weight,
        line_height,
        letter_spacing,
        wrap: crate::ui::widget::CanvasTextWrap::None,
        overflow: crate::ui::widget::CanvasTextOverflow::Clip,
        horizontal_align: crate::ui::widget::CanvasTextHorizontalAlign::Start,
        vertical_align: crate::ui::widget::CanvasTextVerticalAlign::Start,
        clip_rect,
        clip_mask,
    });
}
