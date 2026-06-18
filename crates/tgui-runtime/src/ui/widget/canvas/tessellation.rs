mod effects;
mod mesh;
mod shadow;

use lyon::algorithms::aabb::bounding_box;
use lyon::path::Path;

use crate::ui::widget::common;

pub(super) use self::effects::resolve_canvas_effects;
#[cfg(test)]
pub(super) use self::mesh::CanvasBrushData;
pub(super) use self::mesh::{tessellate_fill, tessellate_stroke};
use self::shadow::shadow_texture_for_path;
pub(super) use self::shadow::{canvas_text_hit_cache_key, shadow_padding};
use super::*;

pub(super) fn path_base_bounds(path: &CanvasPath) -> Option<RectBounds> {
    let bounds = path.path.control_bounds()?;
    let mut rect = RectBounds::from_min_max(bounds.min.x, bounds.min.y, bounds.max.x, bounds.max.y);
    if let Some(stroke) = path.stroke.as_ref() {
        let expansion = match stroke.alignment {
            CanvasStrokeAlignment::Center => stroke.width.get() * 0.5,
            CanvasStrokeAlignment::Inside => 0.0,
            CanvasStrokeAlignment::Outside => stroke.width.get(),
        };
        rect = rect.expand(expansion);
    }
    Some(rect)
}

pub(super) fn tessellate_path(
    path: &CanvasPath,
    origin: Point,
    opacity: f32,
    clip: CanvasClipContext,
    media: &MediaManager,
    units: UnitContext,
) -> CanvasRenderOutput {
    let fill = path.fill.as_ref().map(Value::resolve);
    let stroke = path.stroke.clone();
    let mut output = CanvasRenderOutput::default();
    let effective_opacity = opacity * path.style.opacity;

    if path.style.transform == CanvasTransform2D::IDENTITY {
        if let Some(optimized) = tessellate_axis_aligned_rounded_rect(
            path,
            origin,
            clip,
            fill.as_ref(),
            stroke.as_ref(),
            effective_opacity,
        ) {
            output.commands.extend(optimized.commands);
            output.textures.extend(optimized.textures);
            return output;
        }
    }

    let lyon_path = path.path.to_lyon_path();

    if let Some(shadow) = path.shadow.as_ref().map(Value::resolve) {
        if let Some(texture) = shadow_texture_for_path(
            path,
            &lyon_path,
            fill.as_ref(),
            stroke.as_ref(),
            shadow,
            effective_opacity,
            origin,
            clip,
            media,
            units,
        ) {
            output.textures.push(texture);
        }
    }

    if let Some(fill_brush) = fill.as_ref() {
        if let Some(mesh) = tessellate_fill(
            &lyon_path,
            path.fill_rule,
            fill_brush,
            effective_opacity,
            origin,
            clip,
        ) {
            output.meshes.push(mesh);
        }
    }

    if let Some(stroke) = stroke.as_ref() {
        if let Some(mesh) = tessellate_stroke(&lyon_path, stroke, effective_opacity, origin, clip) {
            output.meshes.push(mesh);
        }
    }

    output
}

pub(super) fn tessellate_axis_aligned_rounded_rect(
    path: &CanvasPath,
    origin: Point,
    clip: CanvasClipContext,
    fill: Option<&CanvasBrush>,
    stroke: Option<&CanvasStroke>,
    opacity: f32,
) -> Option<CanvasRenderOutput> {
    let PathShapeHint::RoundedRect { rect, radius } = path.path.shape_hint()?;
    let frame = offset_rect(rect, origin);
    if frame.is_empty() {
        return Some(CanvasRenderOutput::default());
    }

    let mut output = CanvasRenderOutput::default();
    let corner_radius = radius.get().max(0.0);

    if let Some(fill_brush) = fill {
        push_rounded_rect_fill_command(
            &mut output,
            frame,
            corner_radius,
            fill_brush,
            opacity,
            clip,
        )?;
    }

    if let Some(stroke) = stroke {
        push_rounded_rect_stroke_command(&mut output, frame, corner_radius, stroke, opacity, clip)?;
    }

    Some(output)
}

fn push_rounded_rect_fill_command(
    output: &mut CanvasRenderOutput,
    frame: Rect,
    corner_radius: f32,
    brush: &CanvasBrush,
    opacity: f32,
    clip: CanvasClipContext,
) -> Option<()> {
    match brush {
        CanvasBrush::Solid(color) => {
            output
                .commands
                .push(RenderCommand::Shape(common::RenderPrimitive {
                    rect: frame,
                    color: color.with_alpha_factor(opacity),
                    corner_radius,
                    stroke_width: 0.0,
                    clip_rect: clip.clip_rect,
                    clip_mask: clip.clip_mask,
                }));
        }
        _ => {
            output
                .commands
                .push(RenderCommand::Brush(common::BrushPrimitive {
                    rect: frame,
                    brush: background_brush_from_canvas(brush, opacity)?,
                    corner_radius,
                    clip_rect: clip.clip_rect,
                    clip_mask: clip.clip_mask,
                }));
        }
    }
    Some(())
}

fn push_rounded_rect_stroke_command(
    output: &mut CanvasRenderOutput,
    frame: Rect,
    corner_radius: f32,
    stroke: &CanvasStroke,
    opacity: f32,
    clip: CanvasClipContext,
) -> Option<()> {
    if stroke.dash_pattern.is_some()
        || stroke.line_cap != CanvasStrokeCap::Butt
        || stroke.line_join != CanvasStrokeJoin::Miter
    {
        return None;
    }

    let width = stroke.width.get().max(0.0);
    if width <= 0.0 {
        return Some(());
    }

    let brush = stroke.brush.resolve();
    match brush {
        CanvasBrush::Solid(color) => {
            let (rect, radius, stroke_width) =
                rounded_rect_stroke_geometry(frame, corner_radius, width, stroke.alignment)?;
            output
                .commands
                .push(RenderCommand::Shape(common::RenderPrimitive {
                    rect,
                    color: color.with_alpha_factor(opacity),
                    corner_radius: radius,
                    stroke_width,
                    clip_rect: clip.clip_rect,
                    clip_mask: clip.clip_mask,
                }));
        }
        _ => return None,
    }
    Some(())
}

fn rounded_rect_stroke_geometry(
    frame: Rect,
    radius: f32,
    width: f32,
    alignment: CanvasStrokeAlignment,
) -> Option<(Rect, f32, f32)> {
    match alignment {
        CanvasStrokeAlignment::Center => Some((frame, radius, width)),
        CanvasStrokeAlignment::Inside => {
            let inset = width * 0.5;
            let rect = Rect::new(
                frame.x + inset,
                frame.y + inset,
                (frame.width.get() - width).max(0.0),
                (frame.height.get() - width).max(0.0),
            );
            Some((rect, (radius - inset).max(0.0), width))
        }
        CanvasStrokeAlignment::Outside => {
            let expansion = width * 0.5;
            let rect = Rect::new(
                frame.x - expansion,
                frame.y - expansion,
                frame.width.get() + width,
                frame.height.get() + width,
            );
            Some((rect, radius + expansion, width))
        }
    }
}

fn background_brush_from_canvas(brush: &CanvasBrush, opacity: f32) -> Option<BackgroundBrush> {
    match brush {
        CanvasBrush::Solid(color) => Some(BackgroundBrush::Solid(color.with_alpha_factor(opacity))),
        CanvasBrush::LinearGradient(gradient) => Some(BackgroundBrush::LinearGradient(
            BackgroundLinearGradient::new(
                gradient.start,
                gradient.end,
                gradient
                    .stops
                    .iter()
                    .map(|stop| {
                        BackgroundGradientStop::new(
                            stop.offset,
                            stop.color.with_alpha_factor(opacity),
                        )
                    })
                    .collect::<Vec<_>>(),
            ),
        )),
        CanvasBrush::RadialGradient(gradient) => Some(BackgroundBrush::RadialGradient(
            BackgroundRadialGradient::new(
                gradient.center,
                gradient.radius,
                gradient
                    .stops
                    .iter()
                    .map(|stop| {
                        BackgroundGradientStop::new(
                            stop.offset,
                            stop.color.with_alpha_factor(opacity),
                        )
                    })
                    .collect::<Vec<_>>(),
            ),
        )),
    }
}

pub(super) fn transformed_path_size(
    path: &Path,
    min_x: f32,
    min_y: f32,
    scale_factor: f32,
) -> (u32, u32) {
    let bounds = bounding_box(path.iter());
    let width = ((bounds.max.x - min_x) * scale_factor).ceil().max(1.0) as u32;
    let height = ((bounds.max.y - min_y) * scale_factor).ceil().max(1.0) as u32;
    (width, height)
}
