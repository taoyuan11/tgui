use super::super::*;

pub(crate) fn centered_text_frame(
    inner: Rect,
    measured_width: f32,
    measured_height: f32,
    line_height: f32,
    center_horizontally: bool,
) -> Rect {
    let content_height = inner
        .height
        .min(measured_height.max(line_height))
        .max(Dp::new(line_height));
    let content_width = inner.width.min(measured_width).max(0.0);
    let content_x = if center_horizontally {
        inner.x + ((inner.width - content_width).max(0.0) * 0.5)
    } else {
        inner.x
    };
    let content_y = inner.y + ((inner.height - content_height).max(0.0) * 0.5);

    Rect::new(content_x, content_y, content_width, content_height)
}

pub(crate) fn push_focus_ring_primitives(
    scene: &mut ScenePrimitives,
    frame: Rect,
    border_radius: f32,
    focus_ring: Option<&crate::theme::FocusRingStyle>,
    opacity: f32,
) {
    let Some(focus_ring) = focus_ring else {
        return;
    };
    if !focus_ring.enabled {
        return;
    }

    let width = focus_ring.width.get().max(0.0);
    if width <= 0.0 {
        return;
    }
    let gap = focus_ring.gap.get().max(0.0);
    let expansion = gap + (width * 0.5);
    let ring_frame = Rect::new(
        frame.x - expansion,
        frame.y - expansion,
        frame.width + expansion * 2.0,
        frame.height + expansion * 2.0,
    );
    if ring_frame.is_empty() {
        return;
    }

    scene.push_overlay_shape(RenderPrimitive {
        rect: ring_frame,
        color: focus_ring.color.with_alpha_factor(opacity),
        corner_radius: border_radius + expansion,
        stroke_width: width,
        clip_rect: None,
        clip_mask: None,
    });
}

pub(crate) fn push_border_primitives(
    scene: &mut ScenePrimitives,
    frame: Rect,
    border_width: f32,
    border_color: Color,
    border_radius: f32,
    clip_rect: Option<Rect>,
    clip_mask: Option<ClipMask>,
) {
    if border_color.a == 0 {
        return;
    }

    let thickness = border_width
        .min((frame.width * 0.5).get())
        .min((frame.height * 0.5).get())
        .max(0.0);
    if thickness <= 0.0 {
        return;
    }

    scene.push_shape(RenderPrimitive {
        rect: frame,
        color: border_color,
        corner_radius: border_radius,
        stroke_width: thickness,
        clip_rect,
        clip_mask,
    });
}
