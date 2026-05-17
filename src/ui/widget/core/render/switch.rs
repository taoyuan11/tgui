use super::super::*;

pub(crate) fn default_switch_transition() -> crate::animation::Transition {
    crate::animation::Transition::ease_in_out(std::time::Duration::from_millis(180))
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn push_switch_primitives(
    background_frame: Rect,
    background_radius: f32,
    padding: Insets,
    checked: bool,
    active_thumb_color: Color,
    inactive_thumb_color: Color,
    opacity: f32,
    widget_id: WidgetId,
    clip_rect: Option<Rect>,
    clip_mask: Option<ClipMask>,
    animations: &mut AnimationEngine,
    scene: &mut ScenePrimitives,
    now: std::time::Instant,
) {
    let inner = background_frame.inset(padding);
    if inner.width <= Dp::ZERO || inner.height <= Dp::ZERO {
        return;
    }

    let thumb_diameter = inner.height.min(inner.width);
    if thumb_diameter <= Dp::ZERO {
        return;
    }

    let travel = (inner.width - thumb_diameter).max(Dp::ZERO);
    let thumb_offset = animations.resolve_dp(
        crate::animation::AnimationKey::Widget {
            id: widget_id.raw(),
            property: WidgetProperty::SwitchThumbOffset,
        },
        if checked { travel } else { Dp::ZERO },
        Some(default_switch_transition()),
        now,
    );
    let thumb_color = animations.resolve_color(
        crate::animation::AnimationKey::Widget {
            id: widget_id.raw(),
            property: WidgetProperty::SwitchThumbColor,
        },
        if checked {
            active_thumb_color
        } else {
            inactive_thumb_color
        },
        Some(default_switch_transition()),
        now,
    );

    scene.push_overlay_shape(RenderPrimitive {
        rect: Rect::new(
            inner.x + thumb_offset,
            inner.y + ((inner.height - thumb_diameter) / 2.0),
            thumb_diameter,
            thumb_diameter,
        ),
        color: thumb_color.with_alpha_factor(opacity),
        corner_radius: (thumb_diameter.get() * 0.5).min(background_radius),
        stroke_width: 0.0,
        clip_rect,
        clip_mask,
    });
}
