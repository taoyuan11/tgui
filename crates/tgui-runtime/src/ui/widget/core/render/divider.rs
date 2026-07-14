use super::super::*;
use super::push_text_primitives;
use crate::ui::widget::common::DividerOrientation;

/// 沿一条轴画一段线（实线或虚线）。
///
/// `horizontal` 为真时线沿 x 方向铺开、`cross_center` 是 y；否则线沿 y 方向、`cross_center` 是 x。
#[allow(clippy::too_many_arguments)]
fn push_line_segments(
    scene: &mut ScenePrimitives,
    horizontal: bool,
    cross_center: f32,
    start: f32,
    end: f32,
    thickness: f32,
    dashed: bool,
    dash_length: f32,
    dash_gap: f32,
    color: Color,
    clip_rect: Option<Rect>,
    clip_mask: Option<ClipMask>,
) {
    if end <= start || thickness <= 0.0 {
        return;
    }
    let half = thickness * 0.5;
    let mut emit = |pos: f32, len: f32| {
        if len <= 0.0 {
            return;
        }
        let rect = if horizontal {
            Rect::new(
                Dp::new(pos),
                Dp::new(cross_center - half),
                Dp::new(len),
                Dp::new(thickness),
            )
        } else {
            Rect::new(
                Dp::new(cross_center - half),
                Dp::new(pos),
                Dp::new(thickness),
                Dp::new(len),
            )
        };
        scene.push_shape(RenderPrimitive {
            rect,
            color,
            corner_radius: 0.0,
            stroke_width: 0.0,
            clip_rect,
            clip_mask,
        });
    };

    if dashed {
        let step = (dash_length + dash_gap).max(0.5);
        let mut pos = start;
        while pos < end {
            emit(pos, dash_length.min(end - pos));
            pos += step;
        }
    } else {
        emit(start, end - start);
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn push_divider_primitives(
    frame: Rect,
    orientation: DividerOrientation,
    dashed: bool,
    color: Color,
    thickness: f32,
    inset: f32,
    dash_length: f32,
    dash_gap: f32,
    label: Option<&Value<String>>,
    label_gap: f32,
    style: &crate::ui::widget::style::DividerStyle,
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
) {
    let color = color.with_alpha_factor(opacity);
    let x = frame.x.get();
    let y = frame.y.get();
    let w = frame.width.get();
    let h = frame.height.get();

    if orientation.is_horizontal() {
        let cy = y + h * 0.5;
        let x0 = x + inset;
        let x1 = x + w - inset;

        if let Some(label) = label {
            // 标签在 frame 内水平 + 垂直居中（push_text_primitives 的 center_horizontally）。
            // 线分成左右两段，给标签留出 lw + 两侧 label_gap 的空隙。
            let label_text = text_with_typography(label.clone(), &style.text_style);
            let (label_width, _) =
                measure_text_content_with_layout(&label_text, font_manager, theme, units);
            let center = x + w * 0.5;
            let left_end = center - label_width * 0.5 - label_gap;
            let right_start = center + label_width * 0.5 + label_gap;

            push_line_segments(
                scene,
                true,
                cy,
                x0,
                left_end,
                thickness,
                dashed,
                dash_length,
                dash_gap,
                color,
                clip_rect,
                clip_mask,
            );
            push_line_segments(
                scene,
                true,
                cy,
                right_start,
                x1,
                thickness,
                dashed,
                dash_length,
                dash_gap,
                color,
                clip_rect,
                clip_mask,
            );

            push_text_primitives(
                &label_text,
                frame,
                font_manager,
                theme,
                units,
                animations,
                now,
                scene,
                false,
                true,
                Insets::ZERO,
                None,
                None,
                style.label_color.resolve(),
                opacity,
                widget_id,
                clip_rect,
                clip_mask,
            );
        } else {
            push_line_segments(
                scene,
                true,
                cy,
                x0,
                x1,
                thickness,
                dashed,
                dash_length,
                dash_gap,
                color,
                clip_rect,
                clip_mask,
            );
        }
    } else {
        // 垂直分隔线不支持标签。
        let cx = x + w * 0.5;
        let y0 = y + inset;
        let y1 = y + h - inset;
        push_line_segments(
            scene,
            false,
            cx,
            y0,
            y1,
            thickness,
            dashed,
            dash_length,
            dash_gap,
            color,
            clip_rect,
            clip_mask,
        );
    }
}
