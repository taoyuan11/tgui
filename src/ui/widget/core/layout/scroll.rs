use super::super::*;

pub(crate) fn apply_overflow_clip(
    parent_clip: Rect,
    frame: Rect,
    overflow_x: Overflow,
    overflow_y: Overflow,
) -> Rect {
    let x = if matches!(overflow_x, Overflow::Hidden | Overflow::Scroll) {
        parent_clip.x.max(frame.x)
    } else {
        parent_clip.x
    };
    let y = if matches!(overflow_y, Overflow::Hidden | Overflow::Scroll) {
        parent_clip.y.max(frame.y)
    } else {
        parent_clip.y
    };
    let right = if matches!(overflow_x, Overflow::Hidden | Overflow::Scroll) {
        parent_clip.right().min(frame.right())
    } else {
        parent_clip.right()
    };
    let bottom = if matches!(overflow_y, Overflow::Hidden | Overflow::Scroll) {
        parent_clip.bottom().min(frame.bottom())
    } else {
        parent_clip.bottom()
    };

    Rect::new(x, y, (right - x).max(0.0), (bottom - y).max(0.0))
}

pub(crate) fn apply_overflow_clip_mask(
    parent_clip_mask: Option<ClipMask>,
    frame: Rect,
    corner_radius: f32,
    overflow_x: Overflow,
    overflow_y: Overflow,
) -> Option<ClipMask> {
    if matches!(overflow_x, Overflow::Hidden | Overflow::Scroll)
        && matches!(overflow_y, Overflow::Hidden | Overflow::Scroll)
        && corner_radius > 0.0
        && frame.width > Dp::ZERO
        && frame.height > Dp::ZERO
    {
        return Some(ClipMask {
            rect: frame,
            corner_radius,
        });
    }

    parent_clip_mask
}

#[derive(Clone, Copy, Default)]
pub(crate) struct ScrollbarGeometry {
    pub(crate) horizontal_track: Option<Rect>,
    pub(crate) horizontal_thumb: Option<Rect>,
    pub(crate) vertical_track: Option<Rect>,
    pub(crate) vertical_thumb: Option<Rect>,
}

pub(crate) fn compute_scrollbar_geometry(
    viewport: Rect,
    content_bounds: Rect,
    scroll_offset: Point,
    layout: &ContainerLayout,
    theme: &Theme,
    units: UnitContext,
) -> ScrollbarGeometry {
    let can_scroll_x =
        layout.overflow_x == Overflow::Scroll && content_bounds.right() > viewport.right();
    let can_scroll_y =
        layout.overflow_y == Overflow::Scroll && content_bounds.bottom() > viewport.bottom();
    if !can_scroll_x && !can_scroll_y {
        return ScrollbarGeometry::default();
    }

    let defaults = crate::ui::widget::ContainerStyle::default_for_theme(theme).scrollbar;
    let style = layout.scrollbar_style;
    let thickness = units.resolve_dp(
        style
            .thickness
            .or(defaults.thickness)
            .unwrap_or(dp(5.0))
            .max(dp(2.0)),
    );
    let inset_bounds = viewport.inset(style.insets.unwrap_or(Insets::ZERO));
    if inset_bounds.is_empty() {
        return ScrollbarGeometry::default();
    }

    let vertical_track = can_scroll_y.then(|| {
        Rect::new(
            (inset_bounds.right() - thickness).max(inset_bounds.x),
            inset_bounds.y,
            Dp::new(thickness).min(inset_bounds.width),
            (inset_bounds.height - if can_scroll_x { thickness } else { 0.0 }).max(0.0),
        )
    });
    let horizontal_track = can_scroll_x.then(|| {
        Rect::new(
            inset_bounds.x,
            (inset_bounds.bottom() - thickness).max(inset_bounds.y),
            (inset_bounds.width - if can_scroll_y { thickness } else { 0.0 }).max(0.0),
            Dp::new(thickness).min(inset_bounds.height),
        )
    });

    ScrollbarGeometry {
        horizontal_thumb: horizontal_track
            .filter(|track| !track.is_empty())
            .map(|track| {
                scrollbar_thumb_rect(
                    track,
                    viewport.width.get(),
                    scroll_offset.x.get(),
                    (content_bounds.right() - viewport.x)
                        .max(viewport.width)
                        .get(),
                    units.resolve_dp(
                        style
                            .min_thumb_length
                            .or(defaults.min_thumb_length)
                            .unwrap_or(dp(12.0))
                            .max(Dp::new(thickness)),
                    ),
                    Axis::Horizontal,
                )
            }),
        vertical_thumb: vertical_track
            .filter(|track| !track.is_empty())
            .map(|track| {
                scrollbar_thumb_rect(
                    track,
                    viewport.height.get(),
                    scroll_offset.y.get(),
                    (content_bounds.bottom() - viewport.y)
                        .max(viewport.height)
                        .get(),
                    units.resolve_dp(
                        style
                            .min_thumb_length
                            .or(defaults.min_thumb_length)
                            .unwrap_or(dp(12.0))
                            .max(Dp::new(thickness)),
                    ),
                    Axis::Vertical,
                )
            }),
        horizontal_track: horizontal_track.filter(|track| !track.is_empty()),
        vertical_track: vertical_track.filter(|track| !track.is_empty()),
    }
}

pub(crate) fn push_scrollbar_primitives(
    scene: &mut ScenePrimitives,
    theme: &Theme,
    clip_rect: Rect,
    opacity: f32,
    layout: &ContainerLayout,
    geometry: ScrollbarGeometry,
    widget_id: WidgetId,
    hovered_scrollbar: Option<ScrollbarHandle>,
    active_scrollbar: Option<ScrollbarHandle>,
) {
    if geometry.horizontal_track.is_none() && geometry.vertical_track.is_none() {
        return;
    }

    let track_clip = Some(clip_rect);
    let defaults = crate::ui::widget::ContainerStyle::default_for_theme(theme).scrollbar;
    let style = layout.scrollbar_style;
    let track_color = style
        .track_color
        .or(defaults.track_color)
        .unwrap_or(Color::TRANSPARENT)
        .with_alpha_factor(opacity);
    let thumb_color_for = |axis| {
        let handle = ScrollbarHandle {
            id: widget_id,
            axis,
        };
        let mut state = crate::ui::theme::WidgetState::default();
        if active_scrollbar == Some(handle) {
            state.pressed = true;
        } else if hovered_scrollbar == Some(handle) {
            state.hovered = true;
        }
        if state.pressed {
            style
                .active_thumb_color
                .or(style.thumb_color)
                .or(defaults.active_thumb_color)
                .or(defaults.thumb_color)
                .unwrap_or(Color::TRANSPARENT)
                .with_alpha_factor(opacity)
        } else if state.hovered {
            style
                .hover_thumb_color
                .or(style.thumb_color)
                .or(defaults.hover_thumb_color)
                .or(defaults.thumb_color)
                .unwrap_or(Color::TRANSPARENT)
                .with_alpha_factor(opacity)
        } else {
            style
                .thumb_color
                .or(defaults.thumb_color)
                .unwrap_or(Color::TRANSPARENT)
                .with_alpha_factor(opacity)
        }
    };
    let thickness = style
        .thickness
        .or(defaults.thickness)
        .unwrap_or(dp(12.0))
        .max(dp(2.0))
        .get();
    let radius = style
        .radius
        .or(defaults.radius)
        .unwrap_or(dp(999.0))
        .max(Dp::ZERO)
        .min(Dp::new(thickness * 0.5))
        .get();

    if let Some(track) = geometry.vertical_track {
        scene.push_overlay_shape(RenderPrimitive {
            rect: track,
            color: track_color,
            corner_radius: radius,
            stroke_width: 0.0,
            clip_rect: track_clip,
            clip_mask: None,
        });
        let thumb = geometry
            .vertical_thumb
            .expect("vertical thumb should exist with vertical track");
        scene.push_overlay_shape(RenderPrimitive {
            rect: thumb,
            color: thumb_color_for(ScrollbarAxis::Vertical),
            corner_radius: radius,
            stroke_width: 0.0,
            clip_rect: track_clip,
            clip_mask: None,
        });
    }

    if let Some(track) = geometry.horizontal_track {
        scene.push_overlay_shape(RenderPrimitive {
            rect: track,
            color: track_color,
            corner_radius: radius,
            stroke_width: 0.0,
            clip_rect: track_clip,
            clip_mask: None,
        });
        let thumb = geometry
            .horizontal_thumb
            .expect("horizontal thumb should exist with horizontal track");
        scene.push_overlay_shape(RenderPrimitive {
            rect: thumb,
            color: thumb_color_for(ScrollbarAxis::Horizontal),
            corner_radius: radius,
            stroke_width: 0.0,
            clip_rect: track_clip,
            clip_mask: None,
        });
    }
}

pub(crate) fn scrollbar_thumb_rect(
    track: Rect,
    viewport_extent: f32,
    scroll_offset: f32,
    content_extent: f32,
    min_thumb_length: f32,
    axis: Axis,
) -> Rect {
    let track_extent = match axis {
        Axis::Horizontal => track.width,
        Axis::Vertical => track.height,
    }
    .max(0.0)
    .get();
    let max_offset = (content_extent - viewport_extent).max(0.0);
    let mut thumb_extent = if content_extent <= 0.0 {
        track_extent
    } else {
        track_extent * (viewport_extent / content_extent)
    };
    thumb_extent = thumb_extent.clamp(min_thumb_length.min(track_extent), track_extent);
    let travel = (track_extent - thumb_extent).max(0.0);
    let thumb_offset = if max_offset <= 0.0 || travel <= 0.0 {
        0.0
    } else {
        (scroll_offset.clamp(0.0, max_offset) / max_offset) * travel
    };

    match axis {
        Axis::Horizontal => Rect::new(track.x + thumb_offset, track.y, thumb_extent, track.height),
        Axis::Vertical => Rect::new(track.x, track.y + thumb_offset, track.width, thumb_extent),
    }
}
