use std::sync::Arc;

use super::super::*;
use super::centered_text_frame;
use crate::ui::widget::{MeshPrimitive, MeshVertex};

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
        select_style,
        units,
        scene,
        opacity,
        clip_rect,
        clip_mask,
    );
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn build_select_menu_overlay<VM>(
    widget_id: WidgetId,
    trigger_frame: Rect,
    viewport: Rect,
    options: &[SelectOptionState<VM>],
    on_open_change: Option<&ValueCommand<VM, bool>>,
    select_style: &ResolvedSelectStyle,
    context: &mut CollectContext<'_, '_>,
    opacity: f32,
    open_progress: f32,
) -> Option<(crate::ui::widget::overlay::OverlayContent<VM>, (Dp, Dp))> {
    if options.is_empty() || open_progress <= f32::EPSILON {
        return None;
    }

    let option_height = Dp::new(context.units.resolve_dp(select_style.option_height));
    let menu_height = option_height * options.len() as f32;
    let menu_gap = context.units.resolve_dp(select_style.menu_gap);
    let below_space =
        Dp::new((viewport.bottom().get() - trigger_frame.bottom().get() - menu_gap).max(0.0));
    let above_space = Dp::new((trigger_frame.y.get() - viewport.y.get() - menu_gap).max(0.0));
    let open_down = below_space >= menu_height || below_space >= above_space;
    let available_height = if open_down { below_space } else { above_space };
    let full_height = menu_height.min(available_height).max(Dp::ZERO);
    let visible_height = (full_height * open_progress).max(Dp::ZERO);
    if full_height <= Dp::ZERO {
        return None;
    }

    let menu_width = trigger_frame.width;
    let menu_corner_radius = context.units.resolve_dp(select_style.radius);
    let menu_clip_rect = if open_down {
        Some(Rect::new(0.0, 0.0, menu_width, visible_height))
    } else {
        Some(Rect::new(
            0.0,
            full_height - visible_height,
            menu_width,
            visible_height,
        ))
    };
    let menu_clip_mask = Some(ClipMask {
        rect: Rect::new(0.0, 0.0, menu_width, full_height),
        corner_radius: menu_corner_radius,
    });
    let option_padding = Insets::symmetric(select_style.padding_x, Dp::ZERO);
    let disabled_text = default_select_disabled_text_color(context.theme);
    let mut option_interactions = InteractionHandlers::default();
    option_interactions.cursor_style = Some(Value::Static(CursorStyle::Pointer));
    let mut primitives = vec![crate::ui::widget::overlay::OverlayPrimitive::Shape(
        RenderPrimitive {
            rect: menu_clip_rect.unwrap_or(Rect::new(0.0, 0.0, menu_width, full_height)),
            color: select_style.menu_background.with_alpha_factor(opacity),
            corner_radius: menu_corner_radius,
            stroke_width: 0.0,
            clip_rect: None,
            clip_mask: menu_clip_mask,
        },
    )];
    let mut hits = Vec::new();

    for (index, option) in options.iter().enumerate() {
        let option_frame = Rect::new(0.0, option_height * index as f32, menu_width, option_height);
        let selected = option.selected.resolve();
        let option_disabled = option.disabled.resolve();
        let mut option_state = context.widget_states.get_select_option(widget_id, index);
        option_state.disabled = option_disabled;
        let hovered_option_color = default_select_menu_option_color(context.theme, option_state);
        let option_color = if option_state.hovered || option_state.pressed {
            hovered_option_color
        } else if selected {
            select_style.selected_option_background
        } else {
            hovered_option_color
        };
        if selected || option_color.a > 0 {
            primitives.push(crate::ui::widget::overlay::OverlayPrimitive::Shape(
                RenderPrimitive {
                    rect: option_frame,
                    color: option_color.with_alpha_factor(opacity),
                    corner_radius: 0.0,
                    stroke_width: 0.0,
                    clip_rect: None,
                    clip_mask: menu_clip_mask,
                },
            ));
        }

        primitives.push(crate::ui::widget::overlay::OverlayPrimitive::Text(
            build_select_text_primitive(
                &select_display_text(text_from_content(option.label.clone()), select_style),
                option_frame,
                context.font_manager,
                context.theme,
                context.units,
                context.animations,
                context.now,
                option_padding,
                if option_disabled {
                    disabled_text
                } else {
                    select_style.text
                },
                opacity,
                widget_id,
                None,
                menu_clip_mask,
            ),
        ));

        if open_progress >= 1.0 - f32::EPSILON {
            hits.push(HitRegion {
                rect: option_frame,
                clip_rect: menu_clip_rect,
                geometry: HitGeometry::Rect,
                scope_path: context.focus_scope_path(),
                focus: None,
                interaction: if option_disabled {
                    HitInteraction::Disabled { id: widget_id }
                } else {
                    HitInteraction::SelectOption {
                        id: widget_id,
                        option_index: index,
                        interactions: option_interactions.clone(),
                        on_select: option.on_select.clone(),
                        on_open_change: on_open_change.cloned(),
                    }
                },
            });
        }
    }

    Some((
        crate::ui::widget::overlay::OverlayContent::Batch {
            primitives,
            hits,
            clip_rect: menu_clip_rect,
        },
        (menu_width, full_height),
    ))
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
        content,
        rich_spans: None,
        frame: content_frame,
        quad: None,
        color,
        force_color: false,
        font_family: Some(resolved.primary_font),
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
    select_style: &ResolvedSelectStyle,
    units: UnitContext,
    scene: &mut ScenePrimitives,
    opacity: f32,
    clip_rect: Option<Rect>,
    clip_mask: Option<ClipMask>,
) {
    let icon_width = units.resolve_dp(dp(9.0)).min(frame.width.get()).max(1.0);
    let icon_height = units.resolve_dp(dp(5.0)).min(frame.height.get()).max(1.0);
    let center_x = frame.x + frame.width * 0.5;
    let center_y = frame.y + frame.height * 0.5 + dp(1.0);
    let half_width = Dp::new(icon_width * 0.5);
    let half_height = Dp::new(icon_height * 0.5);
    let points = [
        Point::new(center_x - half_width, center_y - half_height),
        Point::new(center_x + half_width, center_y - half_height),
        Point::new(center_x, center_y + half_height),
    ];
    let brush_meta = [0.0, 1.0, 0.0, 0.0];
    let rgba = select_style
        .arrow
        .with_alpha_factor(opacity)
        .to_linear_rgba_f32();
    let mut stop_colors = [[0.0; 4]; 8];
    stop_colors[0] = rgba;
    stop_colors[1] = rgba;
    let vertices = points
        .iter()
        .map(|point| MeshVertex {
            position: [point.x.get(), point.y.get()],
            local_position: [point.x.get() - frame.x.get(), point.y.get() - frame.y.get()],
            brush_meta,
            gradient_data0: [0.0; 4],
            gradient_data1: [0.0; 4],
            stop_offsets0: [0.0; 4],
            stop_offsets1: [0.0; 4],
            stop_colors,
        })
        .collect::<Vec<_>>();

    scene.push_mesh(MeshPrimitive {
        vertices: Arc::from(vertices),
        triangles: Arc::from([points]),
        clip_rect,
        clip_mask,
    });
}
