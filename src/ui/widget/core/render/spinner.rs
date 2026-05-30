use std::f32::consts::PI;
use std::sync::Arc;
use std::time::Duration;

use super::super::*;
use crate::ui::widget::{MeshPrimitive, MeshVertex};

pub(crate) fn default_spinner_phase_transition() -> Transition {
    Transition::linear(Duration::from_millis(900)).repeat_forever()
}

fn solid_mesh_color(color: Color, opacity: f32) -> [[f32; 4]; 8] {
    let rgba = color.with_alpha_factor(opacity).to_linear_rgba_f32();
    let mut colors = [[0.0; 4]; 8];
    colors[0] = rgba;
    colors[1] = rgba;
    colors
}

fn ring_arc_mesh(
    center: Point,
    radius: f32,
    thickness: f32,
    start_angle: f32,
    sweep_radians: f32,
    color: Color,
    opacity: f32,
    clip_rect: Option<Rect>,
    clip_mask: Option<ClipMask>,
) -> Option<MeshPrimitive> {
    let outer_radius = radius.max(0.5);
    let inner_radius = (outer_radius - thickness.max(0.5)).max(0.0);
    let sweep = sweep_radians.abs().clamp(0.01, PI * 2.0);
    let segments = ((outer_radius * sweep / 6.0).ceil() as usize).clamp(12, 64);
    let brush_meta = [0.0, 1.0, 0.0, 0.0];
    let stop_colors = solid_mesh_color(color, opacity);
    let mut vertices = Vec::with_capacity(segments * 6);
    let mut triangles = Vec::with_capacity(segments * 2);

    for index in 0..segments {
        let t0 = index as f32 / segments as f32;
        let t1 = (index + 1) as f32 / segments as f32;
        let a0 = start_angle + sweep * t0;
        let a1 = start_angle + sweep * t1;
        let outer0 = Point::new(
            center.x + Dp::new(a0.cos() * outer_radius),
            center.y + Dp::new(a0.sin() * outer_radius),
        );
        let outer1 = Point::new(
            center.x + Dp::new(a1.cos() * outer_radius),
            center.y + Dp::new(a1.sin() * outer_radius),
        );
        let inner0 = Point::new(
            center.x + Dp::new(a0.cos() * inner_radius),
            center.y + Dp::new(a0.sin() * inner_radius),
        );
        let inner1 = Point::new(
            center.x + Dp::new(a1.cos() * inner_radius),
            center.y + Dp::new(a1.sin() * inner_radius),
        );

        let quad = [outer0, outer1, inner1, inner0];
        let local = |point: Point| {
            [
                point.x.get() - center.x.get(),
                point.y.get() - center.y.get(),
            ]
        };
        let push_vertex = |point: Point, vertices: &mut Vec<MeshVertex>| {
            vertices.push(MeshVertex {
                position: [point.x.get(), point.y.get()],
                local_position: local(point),
                brush_meta,
                gradient_data0: [0.0; 4],
                gradient_data1: [0.0; 4],
                stop_offsets0: [0.0; 4],
                stop_offsets1: [0.0; 4],
                stop_colors,
            });
        };

        push_vertex(quad[0], &mut vertices);
        push_vertex(quad[1], &mut vertices);
        push_vertex(quad[2], &mut vertices);
        triangles.push([quad[0], quad[1], quad[2]]);
        push_vertex(quad[0], &mut vertices);
        push_vertex(quad[2], &mut vertices);
        push_vertex(quad[3], &mut vertices);
        triangles.push([quad[0], quad[2], quad[3]]);
    }

    Some(MeshPrimitive {
        vertices: Arc::from(vertices),
        triangles: Arc::from(triangles),
        clip_rect,
        clip_mask,
    })
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn push_spinner_primitives(
    frame: Rect,
    phase: f32,
    style: &crate::ui::widget::style::SpinnerStyle,
    size_override: Option<Dp>,
    thickness_override: Option<Dp>,
    track_override: Option<bool>,
    opacity: f32,
    clip_rect: Option<Rect>,
    clip_mask: Option<ClipMask>,
    units: UnitContext,
    scene: &mut ScenePrimitives,
) -> Rect {
    let size = units
        .resolve_dp(size_override.unwrap_or(style.size))
        .max(1.0);
    let thickness = units
        .resolve_dp(thickness_override.unwrap_or(style.thickness))
        .max(1.0)
        .min(size * 0.5);
    let spinner_rect = Rect::new(
        frame.x + ((frame.width - size).max(Dp::ZERO) * 0.5),
        frame.y + ((frame.height - size).max(Dp::ZERO) * 0.5),
        size,
        size,
    );
    let center = Point::new(
        spinner_rect.x + spinner_rect.width * 0.5,
        spinner_rect.y + spinner_rect.height * 0.5,
    );
    let radius = (spinner_rect.width.min(spinner_rect.height).get() * 0.5).max(0.5);
    let show_track = track_override.unwrap_or(style.show_track);

    if show_track {
        if let Some(mesh) = ring_arc_mesh(
            center,
            radius,
            thickness,
            -PI * 0.5,
            PI * 2.0,
            style.track_color.resolve(),
            opacity,
            clip_rect,
            clip_mask,
        ) {
            scene.push_mesh(mesh);
        }
    }

    let sweep = style.sweep_degrees.clamp(10.0, 359.0).to_radians();
    let start_angle = -PI * 0.5 + phase.clamp(0.0, 1.0) * PI * 2.0;
    if let Some(mesh) = ring_arc_mesh(
        center,
        radius,
        thickness,
        start_angle,
        sweep,
        style.indicator_color.resolve(),
        opacity,
        clip_rect,
        clip_mask,
    ) {
        scene.push_mesh(mesh);
    }

    spinner_rect
}
