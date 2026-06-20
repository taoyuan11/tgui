use geo::{BooleanOps, MultiPolygon};
use lyon::math::point;
use lyon::path::iterator::PathIterator;
use lyon::path::{Path, PathEvent};

use super::*;

impl PathBuilder {
    pub fn svg_path(mut self, data: &str) -> Result<Self, CanvasSvgPathError> {
        self.shape_hint = None;
        let mut current = (0.0_f32, 0.0_f32);
        let mut subpath_start = (0.0_f32, 0.0_f32);
        let mut last_cubic_ctrl: Option<(f32, f32)> = None;
        let mut last_quad_ctrl: Option<(f32, f32)> = None;

        for segment in svgtypes::PathParser::from(data) {
            let segment = segment.map_err(|error| CanvasSvgPathError(error.to_string()))?;
            match segment {
                svgtypes::PathSegment::MoveTo { abs, x, y } => {
                    let point_value = svg_point(abs, current, x as f32, y as f32);
                    self = self.move_to(point_value.0, point_value.1);
                    current = point_value;
                    subpath_start = point_value;
                    last_cubic_ctrl = None;
                    last_quad_ctrl = None;
                }
                svgtypes::PathSegment::LineTo { abs, x, y } => {
                    let point_value = svg_point(abs, current, x as f32, y as f32);
                    self = self.line_to(point_value.0, point_value.1);
                    current = point_value;
                    last_cubic_ctrl = None;
                    last_quad_ctrl = None;
                }
                svgtypes::PathSegment::HorizontalLineTo { abs, x } => {
                    let point_value = if abs {
                        (x as f32, current.1)
                    } else {
                        (current.0 + x as f32, current.1)
                    };
                    self = self.line_to(point_value.0, point_value.1);
                    current = point_value;
                    last_cubic_ctrl = None;
                    last_quad_ctrl = None;
                }
                svgtypes::PathSegment::VerticalLineTo { abs, y } => {
                    let point_value = if abs {
                        (current.0, y as f32)
                    } else {
                        (current.0, current.1 + y as f32)
                    };
                    self = self.line_to(point_value.0, point_value.1);
                    current = point_value;
                    last_cubic_ctrl = None;
                    last_quad_ctrl = None;
                }
                svgtypes::PathSegment::CurveTo {
                    abs,
                    x1,
                    y1,
                    x2,
                    y2,
                    x,
                    y,
                } => {
                    let ctrl1 = svg_point(abs, current, x1 as f32, y1 as f32);
                    let ctrl2 = svg_point(abs, current, x2 as f32, y2 as f32);
                    let to = svg_point(abs, current, x as f32, y as f32);
                    self = self.cubic_to(ctrl1.0, ctrl1.1, ctrl2.0, ctrl2.1, to.0, to.1);
                    current = to;
                    last_cubic_ctrl = Some(ctrl2);
                    last_quad_ctrl = None;
                }
                svgtypes::PathSegment::SmoothCurveTo { abs, x2, y2, x, y } => {
                    let ctrl1 = last_cubic_ctrl
                        .map(|prev| (current.0 * 2.0 - prev.0, current.1 * 2.0 - prev.1))
                        .unwrap_or(current);
                    let ctrl2 = svg_point(abs, current, x2 as f32, y2 as f32);
                    let to = svg_point(abs, current, x as f32, y as f32);
                    self = self.cubic_to(ctrl1.0, ctrl1.1, ctrl2.0, ctrl2.1, to.0, to.1);
                    current = to;
                    last_cubic_ctrl = Some(ctrl2);
                    last_quad_ctrl = None;
                }
                svgtypes::PathSegment::Quadratic { abs, x1, y1, x, y } => {
                    let ctrl = svg_point(abs, current, x1 as f32, y1 as f32);
                    let to = svg_point(abs, current, x as f32, y as f32);
                    self = self.quad_to(ctrl.0, ctrl.1, to.0, to.1);
                    current = to;
                    last_quad_ctrl = Some(ctrl);
                    last_cubic_ctrl = None;
                }
                svgtypes::PathSegment::SmoothQuadratic { abs, x, y } => {
                    let ctrl = last_quad_ctrl
                        .map(|prev| (current.0 * 2.0 - prev.0, current.1 * 2.0 - prev.1))
                        .unwrap_or(current);
                    let to = svg_point(abs, current, x as f32, y as f32);
                    self = self.quad_to(ctrl.0, ctrl.1, to.0, to.1);
                    current = to;
                    last_quad_ctrl = Some(ctrl);
                    last_cubic_ctrl = None;
                }
                svgtypes::PathSegment::EllipticalArc {
                    abs,
                    rx,
                    ry,
                    x_axis_rotation,
                    large_arc,
                    sweep,
                    x,
                    y,
                } => {
                    let to = svg_point(abs, current, x as f32, y as f32);
                    self = append_svg_arc_segments(
                        self,
                        current,
                        to,
                        rx as f32,
                        ry as f32,
                        x_axis_rotation as f32,
                        large_arc,
                        sweep,
                    );
                    current = to;
                    last_cubic_ctrl = None;
                    last_quad_ctrl = None;
                }
                svgtypes::PathSegment::ClosePath { .. } => {
                    self = self.close();
                    current = subpath_start;
                    last_cubic_ctrl = None;
                    last_quad_ctrl = None;
                }
            }
        }

        Ok(self)
    }

    pub fn union(&self, other: &PathBuilder) -> Result<PathBuilder, CanvasPathOpError> {
        self.boolean_operation_with_rules(
            other,
            self.fill_rule,
            other.fill_rule,
            CanvasPathBooleanOperation::Union,
        )
    }

    pub fn intersect(&self, other: &PathBuilder) -> Result<PathBuilder, CanvasPathOpError> {
        self.boolean_operation_with_rules(
            other,
            self.fill_rule,
            other.fill_rule,
            CanvasPathBooleanOperation::Intersection,
        )
    }

    pub fn difference(&self, other: &PathBuilder) -> Result<PathBuilder, CanvasPathOpError> {
        self.boolean_operation_with_rules(
            other,
            self.fill_rule,
            other.fill_rule,
            CanvasPathBooleanOperation::Difference,
        )
    }

    pub fn xor(&self, other: &PathBuilder) -> Result<PathBuilder, CanvasPathOpError> {
        self.boolean_operation_with_rules(
            other,
            self.fill_rule,
            other.fill_rule,
            CanvasPathBooleanOperation::Xor,
        )
    }

    fn boolean_operation_with_rules(
        &self,
        other: &PathBuilder,
        lhs_rule: CanvasFillRule,
        rhs_rule: CanvasFillRule,
        operation: CanvasPathBooleanOperation,
    ) -> Result<PathBuilder, CanvasPathOpError> {
        let lhs = self.to_multi_polygon_with_rule(lhs_rule)?;
        let rhs = other.to_multi_polygon_with_rule(rhs_rule)?;
        let result = match operation {
            CanvasPathBooleanOperation::Union => lhs.union(&rhs),
            CanvasPathBooleanOperation::Intersection => lhs.intersection(&rhs),
            CanvasPathBooleanOperation::Difference => lhs.difference(&rhs),
            CanvasPathBooleanOperation::Xor => lhs.xor(&rhs),
        };
        Ok(Self::from_multi_polygon(&result).fill_rule(lhs_rule))
    }

    pub(crate) fn commands_internal(&self) -> &[PathCommand] {
        &self.commands
    }

    pub(crate) fn shape_hint(&self) -> Option<PathShapeHint> {
        self.shape_hint
    }

    pub(crate) fn to_lyon_path(&self) -> Path {
        let mut builder = Path::builder();
        let mut subpath_open = false;
        for command in &self.commands {
            match *command {
                PathCommand::MoveTo(point_value) => {
                    if subpath_open {
                        builder.end(false);
                    }
                    builder.begin(point(point_value.x.get(), point_value.y.get()));
                    subpath_open = true;
                }
                PathCommand::LineTo(point_value) => {
                    builder.line_to(point(point_value.x.get(), point_value.y.get()));
                }
                PathCommand::QuadTo { ctrl, to } => {
                    builder.quadratic_bezier_to(
                        point(ctrl.x.get(), ctrl.y.get()),
                        point(to.x.get(), to.y.get()),
                    );
                }
                PathCommand::CubicTo { ctrl1, ctrl2, to } => {
                    builder.cubic_bezier_to(
                        point(ctrl1.x.get(), ctrl1.y.get()),
                        point(ctrl2.x.get(), ctrl2.y.get()),
                        point(to.x.get(), to.y.get()),
                    );
                }
                PathCommand::Close => {
                    builder.end(true);
                    subpath_open = false;
                }
            }
        }
        if subpath_open {
            builder.end(false);
        }
        builder.build()
    }

    pub(crate) fn control_bounds(&self) -> Option<lyon::geom::Box2D<f32>> {
        let path = self.to_lyon_path();
        (path.iter().next().is_some()).then(|| lyon::algorithms::aabb::bounding_box(path.iter()))
    }

    pub(crate) fn to_multi_polygon_with_rule(
        &self,
        fill_rule: CanvasFillRule,
    ) -> Result<MultiPolygon<f64>, CanvasPathOpError> {
        let rings = self.flattened_closed_rings()?;
        Ok(rings_to_multi_polygon(rings, fill_rule))
    }

    fn flattened_closed_rings(&self) -> Result<Vec<Vec<lyon::math::Point>>, CanvasPathOpError> {
        let path = self.to_lyon_path();
        let mut rings = Vec::new();
        let mut current = Vec::new();
        let mut first = None;

        for event in path.iter().flattened(CANVAS_FLATTEN_TOLERANCE) {
            match event {
                PathEvent::Begin { at } => {
                    current.clear();
                    current.push(at);
                    first = Some(at);
                }
                PathEvent::Line { to, .. } => {
                    current.push(to);
                }
                PathEvent::End { close, .. } => {
                    if !close {
                        return Err(CanvasPathOpError::OpenSubpath);
                    }

                    if let Some(first_point) = first.take() {
                        if current
                            .last()
                            .map(|last| !points_approx_equal(*last, first_point))
                            .unwrap_or(false)
                        {
                            current.push(first_point);
                        }
                    }

                    dedupe_ring_points(&mut current);
                    if current.len() >= 4 {
                        rings.push(std::mem::take(&mut current));
                    } else {
                        current.clear();
                    }
                }
                PathEvent::Quadratic { .. } | PathEvent::Cubic { .. } => {}
            }
        }

        Ok(rings)
    }

    fn from_multi_polygon(multi: &MultiPolygon<f64>) -> Self {
        let mut builder = PathBuilder::new();
        for polygon in &multi.0 {
            builder = append_ring(builder, polygon.exterior());
            for interior in polygon.interiors() {
                builder = append_ring(builder, interior);
            }
        }
        builder
    }
}

impl Default for PathBuilder {
    fn default() -> Self {
        Self::new()
    }
}
