use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::Arc;

use lyon::algorithms::measure::{PathMeasurements, SampleType};
use lyon::path::Path;
use lyon::tessellation::{
    BuffersBuilder, FillOptions, FillTessellator, FillVertex, FillVertexConstructor,
    StrokeOptions, StrokeTessellator, StrokeVertex, StrokeVertexConstructor, VertexBuffers,
};

use crate::foundation::color::Color;
use crate::ui::unit::Dp;
use crate::ui::widget::common;

use super::super::*;

pub(in super::super) fn tessellate_fill(
    path: &Path,
    fill_rule: CanvasFillRule,
    brush: &CanvasBrush,
    opacity: f32,
    origin: Point,
    clip: CanvasClipContext,
) -> Option<MeshPrimitive> {
    let brush_data = CanvasBrushData::from_brush(brush, opacity)?;
    let mut geometry = VertexBuffers::<[f32; 2], u32>::new();
    let mut tessellator = FillTessellator::new();
    let mut options = FillOptions::default();
    options.fill_rule = fill_rule.to_lyon();
    tessellator
        .tessellate_path(
            path,
            &options,
            &mut BuffersBuilder::new(&mut geometry, FillVertexCtor),
        )
        .ok()?;

    build_mesh_primitive(geometry, brush_data, origin, clip.clip_rect, clip.clip_mask)
}

pub(in super::super) fn tessellate_stroke(
    path: &Path,
    stroke: &CanvasStroke,
    opacity: f32,
    origin: Point,
    clip: CanvasClipContext,
) -> Option<MeshPrimitive> {
    let brush = stroke.brush.resolve();
    let brush_data = CanvasBrushData::from_brush(&brush, opacity)?;
    let dashed = dashed_path(path, stroke);
    let source_path = dashed.as_ref().unwrap_or(path);

    let mut geometry = VertexBuffers::<[f32; 2], u32>::new();
    let mut tessellator = StrokeTessellator::new();
    let mut options = StrokeOptions::default();
    options.line_width = stroke.width.get().max(0.0);
    options.start_cap = match stroke.line_cap {
        CanvasStrokeCap::Butt => lyon::tessellation::LineCap::Butt,
        CanvasStrokeCap::Square => lyon::tessellation::LineCap::Square,
        CanvasStrokeCap::Round => lyon::tessellation::LineCap::Round,
    };
    options.end_cap = options.start_cap;
    options.line_join = match stroke.line_join {
        CanvasStrokeJoin::Miter => lyon::tessellation::LineJoin::Miter,
        CanvasStrokeJoin::Bevel => lyon::tessellation::LineJoin::Bevel,
        CanvasStrokeJoin::Round => lyon::tessellation::LineJoin::Round,
    };
    options.miter_limit = stroke.miter_limit.max(0.0);
    tessellator
        .tessellate_path(
            source_path,
            &options,
            &mut BuffersBuilder::new(&mut geometry, StrokeVertexCtor),
        )
        .ok()?;

    build_mesh_primitive(geometry, brush_data, origin, clip.clip_rect, clip.clip_mask)
}

fn build_mesh_primitive(
    geometry: VertexBuffers<[f32; 2], u32>,
    brush: CanvasBrushData,
    origin: Point,
    clip_rect: Option<common::Rect>,
    clip_mask: Option<ClipMask>,
) -> Option<MeshPrimitive> {
    if geometry.indices.is_empty() {
        return None;
    }

    let mut vertices = Vec::with_capacity(geometry.indices.len());
    let mut triangles = Vec::with_capacity(geometry.indices.len() / 3);
    for indices in geometry.indices.chunks_exact(3) {
        let mut triangle_points = [Point::ZERO; 3];
        for (slot, index) in indices.iter().enumerate() {
            let source = geometry.vertices[*index as usize];
            let point_value = Point::new(origin.x + source[0], origin.y + source[1]);
            triangle_points[slot] = point_value;
            vertices.push(MeshVertex {
                position: [point_value.x.get(), point_value.y.get()],
                local_position: [source[0], source[1]],
                brush_meta: brush.brush_meta,
                gradient_data0: brush.gradient_data0,
                gradient_data1: brush.gradient_data1,
                stop_offsets0: brush.stop_offsets0,
                stop_offsets1: brush.stop_offsets1,
                stop_colors: brush.stop_colors,
            });
        }
        triangles.push(triangle_points);
    }

    Some(MeshPrimitive {
        vertices: Arc::from(vertices),
        triangles: Arc::from(triangles),
        clip_rect,
        clip_mask,
    })
}

#[derive(Clone, Copy)]
pub(in super::super) struct CanvasBrushData {
    brush_meta: [f32; 4],
    gradient_data0: [f32; 4],
    gradient_data1: [f32; 4],
    stop_offsets0: [f32; 4],
    stop_offsets1: [f32; 4],
    stop_colors: [[f32; 4]; 8],
}

impl CanvasBrushData {
    pub(in super::super) fn from_brush(brush: &CanvasBrush, opacity: f32) -> Option<Self> {
        match brush {
            CanvasBrush::Solid(color) => {
                let mut stop_colors = [[0.0; 4]; 8];
                stop_colors[0] = color_to_f32(color.with_alpha_factor(opacity));
                Some(Self {
                    brush_meta: [0.0, 1.0, 0.0, 0.0],
                    gradient_data0: [0.0; 4],
                    gradient_data1: [0.0; 4],
                    stop_offsets0: [0.0; 4],
                    stop_offsets1: [0.0; 4],
                    stop_colors,
                })
            }
            CanvasBrush::LinearGradient(gradient) => {
                let stops = normalized_gradient_stops(&gradient.stops, opacity)?;
                Some(Self::gradient_data(
                    1.0,
                    &stops,
                    [
                        gradient.start.x.get(),
                        gradient.start.y.get(),
                        gradient.end.x.get(),
                        gradient.end.y.get(),
                    ],
                    [0.0; 4],
                ))
            }
            CanvasBrush::RadialGradient(gradient) => {
                let stops = normalized_gradient_stops(&gradient.stops, opacity)?;
                Some(Self::gradient_data(
                    2.0,
                    &stops,
                    [0.0; 4],
                    [
                        gradient.center.x.get(),
                        gradient.center.y.get(),
                        gradient.radius.get().max(0.0001),
                        0.0,
                    ],
                ))
            }
        }
    }

    fn gradient_data(
        kind: f32,
        stops: &[(f32, [f32; 4])],
        gradient_data0: [f32; 4],
        gradient_data1: [f32; 4],
    ) -> Self {
        let mut stop_offsets0 = [0.0; 4];
        let mut stop_offsets1 = [0.0; 4];
        let mut stop_colors = [[0.0; 4]; 8];

        for (index, (offset, color)) in stops.iter().enumerate() {
            if index < 4 {
                stop_offsets0[index] = *offset;
            } else {
                stop_offsets1[index - 4] = *offset;
            }
            stop_colors[index] = *color;
        }

        Self {
            brush_meta: [kind, stops.len() as f32, 0.0, 0.0],
            gradient_data0,
            gradient_data1,
            stop_offsets0,
            stop_offsets1,
            stop_colors,
        }
    }
}

fn normalized_gradient_stops(
    stops: &[CanvasGradientStop],
    opacity: f32,
) -> Option<Vec<(f32, [f32; 4])>> {
    let mut normalized = stops
        .iter()
        .map(|stop| {
            (
                stop.offset.clamp(0.0, 1.0),
                color_to_f32(stop.color.with_alpha_factor(opacity)),
            )
        })
        .collect::<Vec<_>>();
    normalized.sort_by(|a, b| a.0.total_cmp(&b.0));

    if normalized.is_empty() {
        normalized.push((0.0, color_to_f32(Color::TRANSPARENT)));
    }

    if normalized.len() > MAX_CANVAS_GRADIENT_STOPS {
        let mut compressed = Vec::with_capacity(MAX_CANVAS_GRADIENT_STOPS);
        for index in 0..MAX_CANVAS_GRADIENT_STOPS {
            let offset = if MAX_CANVAS_GRADIENT_STOPS == 1 {
                0.0
            } else {
                index as f32 / (MAX_CANVAS_GRADIENT_STOPS - 1) as f32
            };
            compressed.push((offset, sample_gradient_color(&normalized, offset)));
        }
        return Some(compressed);
    }

    Some(normalized)
}

pub(super) fn dashed_path(path: &Path, stroke: &CanvasStroke) -> Option<Path> {
    let pattern = stroke.dash_pattern.as_ref()?;
    let normalized = normalize_dash_pattern(pattern)?;
    let measurements = PathMeasurements::from_path(path, CANVAS_FLATTEN_TOLERANCE);
    let total_length = measurements.length();
    if total_length <= 0.0 {
        return None;
    }

    let mut sampler = measurements.create_sampler(path, SampleType::Distance);
    let mut builder = Path::builder();
    let cycle_length: f32 = normalized.iter().sum();
    if cycle_length <= 0.0 {
        return None;
    }

    let mut cursor = (-stroke.dash_offset.get()).rem_euclid(cycle_length);
    let mut phase = 0usize;
    while cursor > normalized[phase] && cycle_length > 0.0 {
        cursor -= normalized[phase];
        phase = (phase + 1) % normalized.len();
    }

    let mut distance = 0.0_f32;
    let mut local_offset = cursor;
    while distance < total_length {
        let segment_length = (normalized[phase] - local_offset).max(0.0);
        let end = (distance + segment_length).min(total_length);
        if phase % 2 == 0 && end > distance {
            if catch_unwind(AssertUnwindSafe(|| {
                sampler.split_range(distance..end, &mut builder)
            }))
            .is_err()
            {
                return None;
            }
        }
        distance = end;
        phase = (phase + 1) % normalized.len();
        local_offset = 0.0;
        if segment_length <= 0.0 && normalized[phase] <= 0.0 {
            break;
        }
    }

    let dashed = builder.build();
    (dashed.iter().next().is_some()).then_some(dashed)
}

pub(super) fn normalize_dash_pattern(pattern: &[Dp]) -> Option<Vec<f32>> {
    let mut values = pattern
        .iter()
        .map(|value| value.get().max(0.0))
        .collect::<Vec<_>>();
    if values.len() < 2 {
        return None;
    }
    if values.iter().all(|value| *value == 0.0) {
        return None;
    }
    if values.len() % 2 != 0 {
        values.extend(values.clone());
    }
    Some(values)
}

fn color_to_f32(color: Color) -> [f32; 4] {
    color.to_linear_rgba_f32()
}

fn sample_gradient_color(stops: &[(f32, [f32; 4])], offset: f32) -> [f32; 4] {
    if stops.is_empty() {
        return [0.0; 4];
    }
    if offset <= stops[0].0 {
        return stops[0].1;
    }
    if offset >= stops[stops.len() - 1].0 {
        return stops[stops.len() - 1].1;
    }

    for pair in stops.windows(2) {
        let (start_offset, start_color) = pair[0];
        let (end_offset, end_color) = pair[1];
        if offset <= end_offset {
            let span = (end_offset - start_offset).max(f32::EPSILON);
            let t = ((offset - start_offset) / span).clamp(0.0, 1.0);
            let mut color = [0.0; 4];
            for index in 0..4 {
                color[index] = start_color[index] + (end_color[index] - start_color[index]) * t;
            }
            return color;
        }
    }

    stops[stops.len() - 1].1
}

struct FillVertexCtor;

impl FillVertexConstructor<[f32; 2]> for FillVertexCtor {
    fn new_vertex(&mut self, vertex: FillVertex<'_>) -> [f32; 2] {
        let position = vertex.position();
        [position.x, position.y]
    }
}

struct StrokeVertexCtor;

impl StrokeVertexConstructor<[f32; 2]> for StrokeVertexCtor {
    fn new_vertex(&mut self, vertex: StrokeVertex<'_, '_>) -> [f32; 2] {
        let position = vertex.position();
        [position.x, position.y]
    }
}
