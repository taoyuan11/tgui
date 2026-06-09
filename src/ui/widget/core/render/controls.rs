use super::super::*;
use super::{
    centered_text_frame, push_border_primitives, push_focus_ring_primitives, push_text_primitives,
    rounded_rect_shadow_texture, RoundedRectShadowSpec,
};
use crate::ui::widget::common;

#[derive(Clone, Copy)]
pub(crate) struct SliderGeometry {
    pub(crate) track_rect: Rect,
    pub(crate) thumb_rect: Rect,
}

pub(crate) fn slider_geometry(
    frame: Rect,
    slider_style: &ResolvedSliderStyle,
    show_value_label: bool,
    units: UnitContext,
) -> SliderGeometry {
    let thumb_size = units.resolve_dp(slider_style.thumb_size).max(0.0);
    let track_height = units
        .resolve_dp(slider_style.track_height)
        .min(frame.height.get())
        .max(1.0);
    let label_height = if show_value_label {
        slider_style
            .text_style
            .line_height
            .map(|value| units.resolve_sp(value))
            .unwrap_or_else(|| units.resolve_sp(slider_style.text_style.size))
    } else {
        0.0
    };
    let label_gap = if show_value_label {
        units.resolve_dp(slider_style.label_gap)
    } else {
        0.0
    };
    let track_available_top = frame.y + Dp::new(label_height + label_gap);
    let track_available_height = (frame.bottom() - track_available_top).max(track_height);
    let track_y = track_available_top + ((track_available_height - track_height).max(0.0) * 0.5);
    let half_thumb = Dp::new(thumb_size * 0.5);
    let track_x = frame.x + half_thumb.min(frame.width * 0.5);
    let track_width = (frame.width - (half_thumb * 2.0)).max(0.0);
    let track_rect = Rect::new(track_x, track_y, track_width, track_height);
    let thumb_rect = Rect::new(
        track_rect.x,
        track_rect.y + (track_rect.height * 0.5) - half_thumb,
        thumb_size,
        thumb_size,
    );

    SliderGeometry {
        track_rect,
        thumb_rect,
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn push_slider_primitives(
    frame: Rect,
    value: f32,
    min: f32,
    max: f32,
    step: f32,
    show_ticks: bool,
    show_value_label: bool,
    tick_count: usize,
    value_label: Option<&str>,
    slider_style: &ResolvedSliderStyle,
    opacity: f32,
    widget_id: WidgetId,
    clip_rect: Option<Rect>,
    clip_mask: Option<ClipMask>,
    font_manager: &FontManager,
    theme: &Theme,
    units: UnitContext,
    animations: &mut AnimationEngine,
    now: std::time::Instant,
    scene: &mut ScenePrimitives,
    media: &MediaManager,
    transition: Option<Transition>,
) -> SliderGeometry {
    let mut geometry = slider_geometry(frame, slider_style, show_value_label, units);
    let track_radius = (geometry.track_rect.height.get() * 0.5)
        .min(units.resolve_dp(slider_style.radius))
        .max(0.0);
    let thumb_radius = (geometry.thumb_rect.width.get() * 0.5)
        .min(units.resolve_dp(slider_style.radius))
        .max(0.0);
    let normalized = common::slider_normalized_value(value, min, max, step).clamp(0.0, 1.0);
    // Slider values can update every pointer move while dragging. Animating the thumb/fill
    // makes the visual position lag behind the cursor, so keep these updates immediate.
    let thumb_offset = Dp::new(geometry.track_rect.width.get() * normalized);
    let active_extent = Dp::new(geometry.track_rect.width.get() * normalized);

    let track_color = animations.resolve_color(
        crate::animation::AnimationKey::Widget {
            id: widget_id.raw(),
            property: WidgetProperty::SliderTrackColor,
        },
        slider_style.track,
        transition,
        now,
    );
    let active_track_color = animations.resolve_color(
        crate::animation::AnimationKey::Widget {
            id: widget_id.raw(),
            property: WidgetProperty::SliderActiveTrackColor,
        },
        slider_style.active_track,
        transition,
        now,
    );
    let thumb_color = animations.resolve_color(
        crate::animation::AnimationKey::Widget {
            id: widget_id.raw(),
            property: WidgetProperty::SliderThumbColor,
        },
        slider_style.thumb,
        transition,
        now,
    );
    let tick_color = animations.resolve_color(
        crate::animation::AnimationKey::Widget {
            id: widget_id.raw(),
            property: WidgetProperty::SliderTickColor,
        },
        slider_style.tick,
        transition,
        now,
    );

    scene.push_shape(RenderPrimitive {
        rect: geometry.track_rect,
        color: track_color.with_alpha_factor(opacity),
        corner_radius: track_radius,
        stroke_width: 0.0,
        clip_rect,
        clip_mask,
    });

    if active_extent > Dp::ZERO {
        scene.push_shape(RenderPrimitive {
            rect: Rect::new(
                geometry.track_rect.x,
                geometry.track_rect.y,
                active_extent.min(geometry.track_rect.width),
                geometry.track_rect.height,
            ),
            color: active_track_color.with_alpha_factor(opacity),
            corner_radius: track_radius,
            stroke_width: 0.0,
            clip_rect,
            clip_mask,
        });
    }

    geometry.thumb_rect.x =
        (geometry.track_rect.x + thumb_offset - (geometry.thumb_rect.width * 0.5)).clamp(
            frame.x,
            (frame.right() - geometry.thumb_rect.width).max(frame.x),
        );
    let thumb_border_width = units
        .resolve_dp(slider_style.border_width)
        .max(0.0)
        .min((geometry.thumb_rect.width.get() * 0.5).max(0.0));
    if show_ticks && tick_count >= 2 {
        let tick_height = units.resolve_dp(slider_style.tick_size).max(1.0);
        let tick_width = (geometry.track_rect.height * 0.5).max(1.0);
        for index in 0..tick_count {
            let normalized = index as f32 / (tick_count.saturating_sub(1)) as f32;
            let x = geometry.track_rect.x + Dp::new(geometry.track_rect.width.get() * normalized)
                - (tick_width * 0.5);
            let y =
                geometry.track_rect.y + ((geometry.track_rect.height - tick_height).max(0.0) * 0.5);
            scene.push_shape(RenderPrimitive {
                rect: Rect::new(x, y, tick_width, tick_height),
                color: tick_color.with_alpha_factor(opacity),
                corner_radius: (tick_width.min(tick_height).get() * 0.5).max(0.0),
                stroke_width: 0.0,
                clip_rect,
                clip_mask,
            });
        }
    }
    if let Some(shadow) = slider_style.thumb_shadow.clone() {
        if let Some(texture) = rounded_rect_shadow_texture(
            geometry.thumb_rect,
            thumb_radius,
            RoundedRectShadowSpec {
                shadow,
                opacity,
                clip_rect,
                clip_mask,
            },
            media,
            units,
        ) {
            scene.push_texture(texture);
        }
    }
    scene.push_shape(RenderPrimitive {
        rect: geometry.thumb_rect,
        color: thumb_color.with_alpha_factor(opacity),
        corner_radius: thumb_radius,
        stroke_width: 0.0,
        clip_rect,
        clip_mask,
    });
    if thumb_border_width > 0.0 {
        scene.push_shape(RenderPrimitive {
            rect: geometry.thumb_rect,
            color: track_color.with_alpha_factor(opacity),
            corner_radius: thumb_radius,
            stroke_width: thumb_border_width,
            clip_rect,
            clip_mask,
        });
    }

    if show_value_label {
        if let Some(value_label) = value_label {
            let label = text_with_typography(value_label, &slider_style.text_style);
            let (_, line_height, _) = resolved_text_metrics(&label, theme, units);
            let label_frame = Rect::new(frame.x, frame.y, frame.width, Dp::new(line_height));
            push_text_primitives(
                &label,
                label_frame,
                font_manager,
                theme,
                units,
                animations,
                now,
                scene,
                false,
                false,
                Insets::ZERO,
                None,
                None,
                slider_style.label,
                opacity,
                widget_id,
                clip_rect,
                clip_mask,
            );
        }
    }

    geometry
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn push_checkbox_primitives(
    frame: Rect,
    checked: bool,
    label: Option<&Value<String>>,
    checkbox_style: &ResolvedCheckboxStyle,
    opacity: f32,
    widget_id: WidgetId,
    clip_rect: Option<Rect>,
    focus_clip_rect: Option<Rect>,
    clip_mask: Option<ClipMask>,
    font_manager: &FontManager,
    theme: &Theme,
    units: UnitContext,
    animations: &mut AnimationEngine,
    now: std::time::Instant,
    scene: &mut ScenePrimitives,
    transition: Option<Transition>,
) {
    let box_size = units.resolve_dp(checkbox_style.size);
    let box_frame = Rect::new(
        frame.x,
        frame.y + ((frame.height - box_size) * 0.5).max(Dp::ZERO),
        box_size,
        box_size,
    );
    let radius = units.resolve_dp(checkbox_style.radius);
    let background = animations.resolve_color(
        crate::animation::AnimationKey::Widget {
            id: widget_id.raw(),
            property: WidgetProperty::CheckboxBackground,
        },
        checkbox_style.background,
        transition,
        now,
    );
    let border = animations.resolve_color(
        crate::animation::AnimationKey::Widget {
            id: widget_id.raw(),
            property: WidgetProperty::CheckboxBorder,
        },
        checkbox_style.border,
        transition,
        now,
    );
    let checkmark_opacity = animations
        .resolve_f32(
            crate::animation::AnimationKey::Widget {
                id: widget_id.raw(),
                property: WidgetProperty::CheckboxCheckmarkOpacity,
            },
            if checked { 1.0 } else { 0.0 },
            transition,
            now,
        )
        .clamp(0.0, 1.0);
    let checkmark_color = animations.resolve_color(
        crate::animation::AnimationKey::Widget {
            id: widget_id.raw(),
            property: WidgetProperty::CheckboxCheckmarkColor,
        },
        checkbox_style.checkmark,
        transition,
        now,
    );

    scene.push_shape(RenderPrimitive {
        rect: box_frame,
        color: background.with_alpha_factor(opacity),
        corner_radius: radius,
        stroke_width: 0.0,
        clip_rect,
        clip_mask,
    });
    let border_width = units.resolve_dp(checkbox_style.border_width);
    push_border_primitives(
        scene,
        box_frame,
        border_width,
        border.with_alpha_factor(opacity),
        radius,
        clip_rect,
        clip_mask,
    );
    push_focus_ring_primitives(
        scene,
        box_frame,
        radius,
        checkbox_style.focus_ring.as_ref(),
        opacity,
        focus_clip_rect,
        clip_mask,
    );

    if checkmark_opacity > f32::EPSILON {
        push_checkbox_checkmark_primitives(
            box_frame,
            checkbox_style,
            opacity * checkmark_opacity,
            checkmark_color,
            font_manager,
            units,
            clip_rect,
            clip_mask,
            scene,
        );
    }

    if let Some(label) = label {
        let label = checkbox_label_with_theme(label, checkbox_style);
        let label_x = box_frame.right() + checkbox_style.label_gap;
        let label_frame = Rect::new(
            label_x,
            frame.y + dp(1.0),
            (frame.right() - label_x).max(Dp::ZERO),
            frame.height,
        );
        push_text_primitives(
            &label,
            label_frame,
            font_manager,
            theme,
            units,
            animations,
            now,
            scene,
            false,
            false,
            Insets::ZERO,
            None,
            None,
            checkbox_style.label,
            opacity,
            widget_id,
            clip_rect,
            clip_mask,
        );
    }
}

pub(crate) fn push_checkbox_checkmark_primitives(
    box_frame: Rect,
    checkbox_style: &ResolvedCheckboxStyle,
    opacity: f32,
    checkmark_color: Color,
    font_manager: &FontManager,
    units: UnitContext,
    clip_rect: Option<Rect>,
    clip_mask: Option<ClipMask>,
    scene: &mut ScenePrimitives,
) {
    let font_size = units
        .resolve_sp(checkbox_style.text_style.size)
        .min(box_frame.width.get())
        .min(box_frame.height.get())
        .max(1.0);
    let line_height = font_size;
    let letter_spacing = 0.0;
    let text_request = TextFontRequest {
        preferred_font: Some(ICON_FONT_FAMILY),
        weight: checkbox_style.text_style.weight,
    };
    let resolved = font_manager.resolve_text(CHECKBOX_CHECKMARK_ICON, text_request.clone());
    let layout = font_manager.measure_text_layout(
        CHECKBOX_CHECKMARK_ICON,
        text_request,
        font_size,
        line_height,
        letter_spacing,
    );
    let mut icon_frame = centered_text_frame(
        box_frame,
        layout.width.max(font_size),
        layout.height.max(line_height),
        line_height,
        true,
    );

    icon_frame.y += dp(1.0);

    scene.push_text(TextPrimitive {
        content: Arc::from(CHECKBOX_CHECKMARK_ICON.to_string()),
        rich_spans: None,
        frame: icon_frame,
        quad: None,
        color: checkmark_color.with_alpha_factor(opacity),
        force_color: true,
        font_family: Some(Arc::from(resolved.primary_font)),
        font_size,
        font_weight: checkbox_style.text_style.weight,
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

#[allow(clippy::too_many_arguments)]
pub(crate) fn push_radio_primitives(
    frame: Rect,
    checked: bool,
    label: Option<&Value<String>>,
    radio_style: &ResolvedRadioStyle,
    opacity: f32,
    widget_id: WidgetId,
    clip_rect: Option<Rect>,
    focus_clip_rect: Option<Rect>,
    clip_mask: Option<ClipMask>,
    font_manager: &FontManager,
    theme: &Theme,
    units: UnitContext,
    animations: &mut AnimationEngine,
    now: std::time::Instant,
    scene: &mut ScenePrimitives,
    transition: Option<Transition>,
) {
    let size = units.resolve_dp(radio_style.size);
    let control_frame = Rect::new(
        frame.x,
        frame.y + ((frame.height - size) * 0.5).max(Dp::ZERO),
        size,
        size,
    );
    let radius = units
        .resolve_dp(radio_style.radius)
        .min(size * 0.5)
        .max(0.0);
    let background = animations.resolve_color(
        crate::animation::AnimationKey::Widget {
            id: widget_id.raw(),
            property: WidgetProperty::RadioBackground,
        },
        radio_style.background,
        transition,
        now,
    );
    let border = animations.resolve_color(
        crate::animation::AnimationKey::Widget {
            id: widget_id.raw(),
            property: WidgetProperty::RadioBorder,
        },
        radio_style.border,
        transition,
        now,
    );
    let indicator_opacity = animations
        .resolve_f32(
            crate::animation::AnimationKey::Widget {
                id: widget_id.raw(),
                property: WidgetProperty::RadioIndicatorOpacity,
            },
            if checked { 1.0 } else { 0.0 },
            transition,
            now,
        )
        .clamp(0.0, 1.0);
    let indicator_color = animations.resolve_color(
        crate::animation::AnimationKey::Widget {
            id: widget_id.raw(),
            property: WidgetProperty::RadioIndicatorColor,
        },
        radio_style.indicator,
        transition,
        now,
    );

    scene.push_shape(RenderPrimitive {
        rect: control_frame,
        color: background.with_alpha_factor(opacity),
        corner_radius: radius,
        stroke_width: 0.0,
        clip_rect,
        clip_mask,
    });
    push_border_primitives(
        scene,
        control_frame,
        units.resolve_dp(radio_style.border_width),
        border.with_alpha_factor(opacity),
        radius,
        clip_rect,
        clip_mask,
    );
    push_focus_ring_primitives(
        scene,
        control_frame,
        radius,
        radio_style.focus_ring.as_ref(),
        opacity,
        focus_clip_rect,
        clip_mask,
    );

    if indicator_opacity > f32::EPSILON {
        let inset = dp(size * 0.28);
        let indicator_frame = control_frame.inset(Insets::all(inset));
        if indicator_frame.width > Dp::ZERO && indicator_frame.height > Dp::ZERO {
            let indicator_radius = (indicator_frame.width.min(indicator_frame.height).get() * 0.5)
                .min(radius)
                .max(0.0);
            scene.push_overlay_shape(RenderPrimitive {
                rect: indicator_frame,
                color: indicator_color.with_alpha_factor(opacity * indicator_opacity),
                corner_radius: indicator_radius,
                stroke_width: 0.0,
                clip_rect,
                clip_mask,
            });
        }
    }

    if let Some(label) = label {
        let label = radio_label_with_theme(label, radio_style);
        let label_x = control_frame.right() + radio_style.label_gap;
        let label_frame = Rect::new(
            label_x,
            frame.y + dp(1.0),
            (frame.right() - label_x).max(Dp::ZERO),
            frame.height,
        );
        push_text_primitives(
            &label,
            label_frame,
            font_manager,
            theme,
            units,
            animations,
            now,
            scene,
            false,
            false,
            Insets::ZERO,
            None,
            None,
            radio_style.label,
            opacity,
            widget_id,
            clip_rect,
            clip_mask,
        );
    }
}
