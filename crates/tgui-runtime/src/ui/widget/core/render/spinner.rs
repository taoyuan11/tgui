use std::f32::consts::PI;
use std::sync::Arc;
use std::time::Duration;

use super::super::*;
use crate::ui::widget::{MeshPrimitive, MeshVertex};

const SPINNER_TARGET_CHORD_PX: f32 = 2.0;
const SPINNER_MIN_FULL_CIRCLE_SEGMENTS: f32 = 48.0;
const SPINNER_MAX_ARC_SEGMENTS: usize = 192;
const SPINNER_EDGE_AA_PHYSICAL_PX: f32 = 1.0;
const SPINNER_EDGE_AA_THICKNESS_FRACTION: f32 = 0.375;
const RADIUS_EPSILON: f32 = 0.001;

pub(crate) fn default_spinner_phase_transition() -> Transition {
    Transition::linear(Duration::from_millis(900)).repeat_forever()
}

fn spinner_vertex_colors(base_color: [f32; 4], opacity: f32, coverage: f32) -> [[f32; 4]; 8] {
    let mut rgba = base_color;
    rgba[3] *= opacity.clamp(0.0, 1.0) * coverage.clamp(0.0, 1.0);
    let mut colors = [[0.0; 4]; 8];
    colors[0] = rgba;
    colors
}

fn spinner_arc_segments(outer_radius: f32, sweep: f32, scale_factor: f32) -> usize {
    let sweep_fraction = (sweep / (PI * 2.0)).clamp(0.0, 1.0);
    let min_segments = (SPINNER_MIN_FULL_CIRCLE_SEGMENTS * sweep_fraction)
        .ceil()
        .max(6.0);
    let physical_arc_length = outer_radius.max(0.5) * scale_factor.max(1.0 / 64.0) * sweep;
    let segments = (physical_arc_length / SPINNER_TARGET_CHORD_PX)
        .ceil()
        .max(min_segments);
    (segments as usize).clamp(6, SPINNER_MAX_ARC_SEGMENTS)
}

fn spinner_edge_antialias_width(thickness: f32, scale_factor: f32) -> f32 {
    let physical_pixel = SPINNER_EDGE_AA_PHYSICAL_PX / scale_factor.max(1.0 / 64.0);
    physical_pixel
        .min(thickness.max(0.0) * SPINNER_EDGE_AA_THICKNESS_FRACTION)
        .max(0.0)
}

fn push_ring_stop(stops: &mut Vec<(f32, f32)>, radius: f32, coverage: f32) {
    let radius = radius.max(0.0);
    if let Some((last_radius, last_coverage)) = stops.last_mut() {
        if (radius - *last_radius).abs() <= RADIUS_EPSILON {
            *last_coverage = last_coverage.max(coverage);
            return;
        }
    }
    stops.push((radius, coverage));
}

fn ring_point(center: Point, angle: f32, radius: f32) -> Point {
    Point::new(
        center.x + Dp::new(angle.cos() * radius),
        center.y + Dp::new(angle.sin() * radius),
    )
}

fn ring_arc_mesh(
    center: Point,
    radius: f32,
    thickness: f32,
    start_angle: f32,
    sweep_radians: f32,
    edge_antialias: f32,
    scale_factor: f32,
    color: Color,
    opacity: f32,
    clip_rect: Option<Rect>,
    clip_mask: Option<ClipMask>,
) -> Option<MeshPrimitive> {
    let outer_radius = radius.max(0.5);
    let thickness = thickness.max(0.5).min(outer_radius);
    let inner_radius = (outer_radius - thickness).max(0.0);
    let sweep = sweep_radians.abs().clamp(0.01, PI * 2.0);
    let edge_antialias = edge_antialias.min(thickness * 0.5);
    let segments = spinner_arc_segments(outer_radius, sweep, scale_factor);
    let brush_meta = [0.0, 1.0, 0.0, 0.0];
    let base_color = color.to_linear_rgba_f32();
    let mut ring_stops = Vec::with_capacity(4);
    if inner_radius <= RADIUS_EPSILON {
        push_ring_stop(&mut ring_stops, 0.0, 1.0);
    } else {
        push_ring_stop(&mut ring_stops, inner_radius, 0.0);
        push_ring_stop(
            &mut ring_stops,
            (inner_radius + edge_antialias).min(outer_radius),
            1.0,
        );
    }
    let full_outer_radius = (outer_radius - edge_antialias)
        .max(ring_stops.last().map(|(radius, _)| *radius).unwrap_or(0.0));
    push_ring_stop(&mut ring_stops, full_outer_radius, 1.0);
    push_ring_stop(&mut ring_stops, outer_radius, 0.0);

    let band_count = ring_stops.len().saturating_sub(1);
    let mut vertices = Vec::with_capacity(segments * band_count * 6);
    let mut triangles = Vec::with_capacity(segments * band_count * 2);

    for index in 0..segments {
        let t0 = index as f32 / segments as f32;
        let t1 = (index + 1) as f32 / segments as f32;
        let a0 = start_angle + sweep * t0;
        let a1 = start_angle + sweep * t1;

        for band in ring_stops.windows(2) {
            let (inner_band_radius, inner_coverage) = band[0];
            let (outer_band_radius, outer_coverage) = band[1];
            if outer_band_radius - inner_band_radius <= RADIUS_EPSILON {
                continue;
            }

            let inner0 = ring_point(center, a0, inner_band_radius);
            let outer0 = ring_point(center, a0, outer_band_radius);
            let outer1 = ring_point(center, a1, outer_band_radius);
            let inner1 = ring_point(center, a1, inner_band_radius);
            let quad = [
                (inner0, inner_coverage),
                (outer0, outer_coverage),
                (outer1, outer_coverage),
                (inner1, inner_coverage),
            ];
            let local = |point: Point| {
                [
                    point.x.get() - center.x.get(),
                    point.y.get() - center.y.get(),
                ]
            };
            let mut push_vertex = |point: Point, coverage: f32| {
                vertices.push(MeshVertex {
                    position: [point.x.get(), point.y.get()],
                    local_position: local(point),
                    brush_meta,
                    gradient_data0: [0.0; 4],
                    gradient_data1: [0.0; 4],
                    stop_offsets0: [0.0; 4],
                    stop_offsets1: [0.0; 4],
                    stop_colors: spinner_vertex_colors(base_color, opacity, coverage),
                });
            };

            push_vertex(quad[0].0, quad[0].1);
            push_vertex(quad[1].0, quad[1].1);
            push_vertex(quad[2].0, quad[2].1);
            triangles.push([quad[0].0, quad[1].0, quad[2].0]);
            push_vertex(quad[0].0, quad[0].1);
            push_vertex(quad[2].0, quad[2].1);
            push_vertex(quad[3].0, quad[3].1);
            triangles.push([quad[0].0, quad[2].0, quad[3].0]);
        }
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
    let size = units.resolve_dp(size_override.unwrap_or(style.size));
    let size = if size.is_finite() { size.max(1.0) } else { 1.0 };
    let thickness = units.resolve_dp(thickness_override.unwrap_or(style.thickness));
    let thickness = if thickness.is_finite() {
        thickness.max(1.0).min(size * 0.5)
    } else {
        1.0_f32.min(size * 0.5)
    };
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
    let edge_antialias = spinner_edge_antialias_width(thickness, units.scale_factor());

    if show_track {
        if let Some(mesh) = ring_arc_mesh(
            center,
            radius,
            thickness,
            -PI * 0.5,
            PI * 2.0,
            edge_antialias,
            units.scale_factor(),
            style.track_color.resolve(),
            opacity,
            clip_rect,
            clip_mask,
        ) {
            scene.push_mesh(mesh);
        }
    }

    let sweep_degrees = if style.sweep_degrees.is_finite() {
        style.sweep_degrees.clamp(10.0, 359.0)
    } else {
        104.0
    };
    let sweep = sweep_degrees.to_radians();
    let start_angle = -PI * 0.5 + phase.clamp(0.0, 1.0) * PI * 2.0;
    if let Some(mesh) = ring_arc_mesh(
        center,
        radius,
        thickness,
        start_angle,
        sweep,
        edge_antialias,
        units.scale_factor(),
        style.indicator_color.resolve(),
        opacity,
        clip_rect,
        clip_mask,
    ) {
        scene.push_mesh(mesh);
    }

    spinner_rect
}
