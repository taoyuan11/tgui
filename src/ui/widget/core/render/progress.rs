use std::time::Duration;

use super::super::*;
use super::push_text_primitives;

pub(crate) fn default_progress_phase_transition() -> Transition {
    Transition::linear(Duration::from_millis(900)).repeat_forever()
}

pub(crate) fn progress_bar_track_rect(
    frame: Rect,
    style: &crate::ui::widget::style::ProgressBarStyle,
    show_label: bool,
    theme: &Theme,
    units: UnitContext,
) -> Rect {
    let track_height = units.resolve_dp(style.height).max(1.0);
    if !show_label {
        return Rect::new(
            frame.x,
            frame.y + ((frame.height - track_height).max(Dp::ZERO) * 0.5),
            frame.width,
            track_height,
        );
    }

    let label_probe = text_with_typography("100%", &style.text_style);
    let (_, label_height, _) = resolved_text_metrics(&label_probe, theme, units);
    Rect::new(
        frame.x,
        frame.y + Dp::new((label_height + units.resolve_dp(style.gap)).max(0.0)),
        frame.width,
        track_height,
    )
}

pub(crate) fn progress_bar_label_frame(
    frame: Rect,
    style: &crate::ui::widget::style::ProgressBarStyle,
    theme: &Theme,
    units: UnitContext,
) -> Rect {
    let label_probe = text_with_typography("100%", &style.text_style);
    let (_, label_height, _) = resolved_text_metrics(&label_probe, theme, units);
    Rect::new(frame.x, frame.y, frame.width, Dp::new(label_height))
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn push_progress_bar_primitives(
    frame: Rect,
    value: f32,
    indeterminate: bool,
    phase: f32,
    show_label: bool,
    label: Option<&Value<String>>,
    style: &crate::ui::widget::style::ProgressBarStyle,
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
) -> Rect {
    let track_rect = progress_bar_track_rect(frame, style, show_label, theme, units);
    let radius = units
        .resolve_dp(style.radius.resolve())
        .min(track_rect.height.get() * 0.5)
        .max(0.0);
    let track_color = style.track_color.resolve().with_alpha_factor(opacity);
    let fill_color = style.fill_color.resolve().with_alpha_factor(opacity);

    scene.push_shape(RenderPrimitive {
        rect: track_rect,
        color: track_color,
        corner_radius: radius,
        stroke_width: 0.0,
        clip_rect,
        clip_mask,
    });

    if track_rect.width > Dp::ZERO {
        if indeterminate {
            let segment_ratio = style.indeterminate_segment_ratio.clamp(0.1, 1.0);
            let segment_width = Dp::new(track_rect.width.get() * segment_ratio)
                .max(dp(8.0))
                .min(track_rect.width);
            let travel = track_rect.width + segment_width;
            let x = track_rect.x - segment_width + Dp::new(travel.get() * phase.clamp(0.0, 1.0));
            let segment_clip_rect = match clip_rect {
                Some(clip) => clip.intersect(track_rect),
                None => Some(track_rect),
            };
            let segment_clip_mask = if radius > 0.0 {
                Some(ClipMask {
                    rect: track_rect,
                    corner_radius: radius,
                })
            } else {
                clip_mask
            };
            if let Some(segment_clip_rect) = segment_clip_rect {
                scene.push_shape(RenderPrimitive {
                    rect: Rect::new(x, track_rect.y, segment_width, track_rect.height),
                    color: fill_color,
                    corner_radius: radius,
                    stroke_width: 0.0,
                    clip_rect: Some(segment_clip_rect),
                    clip_mask: segment_clip_mask,
                });
            }
        } else {
            let fill_width =
                Dp::new(track_rect.width.get() * value.clamp(0.0, 1.0)).min(track_rect.width);
            scene.push_shape(RenderPrimitive {
                rect: Rect::new(track_rect.x, track_rect.y, fill_width, track_rect.height),
                color: fill_color,
                corner_radius: radius,
                stroke_width: 0.0,
                clip_rect,
                clip_mask,
            });
        }
    }

    if show_label {
        let label_text = label
            .cloned()
            .unwrap_or_else(|| Value::Static(format!("{:.0}%", value.clamp(0.0, 1.0) * 100.0)));
        let label_widget = text_with_typography(label_text, &style.text_style);
        push_text_primitives(
            &label_widget,
            progress_bar_label_frame(frame, style, theme, units),
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
            style.label_color.resolve(),
            opacity,
            widget_id,
            clip_rect,
            clip_mask,
        );
    }

    track_rect
}
