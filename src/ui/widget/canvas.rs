use std::collections::HashSet;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use geo::{BooleanOps, Coord, LineString, MultiPolygon, Polygon};
use image::{DynamicImage, RgbaImage};
use lyon::algorithms::aabb::bounding_box;
use lyon::algorithms::measure::{PathMeasurements, SampleType};
use lyon::math::point;
use lyon::path::iterator::PathIterator;
use lyon::path::{Path, PathEvent};
use lyon::tessellation::{
    BuffersBuilder, FillOptions, FillTessellator, FillVertex, FillVertexConstructor, StrokeOptions,
    StrokeTessellator, StrokeVertex, StrokeVertexConstructor, VertexBuffers,
};
use resvg::tiny_skia;

use crate::foundation::binding::Signal;
use crate::foundation::color::Color;
use crate::foundation::error::TguiError;
use crate::foundation::view_model::{Command, ValueCommand};
use crate::media::{
    resolve_media_rect, ContentFit, MediaManager, MediaSource, RasterRequest, TextureFrame,
};
use crate::text::font::FontWeight;
use crate::theme::ResolvedThemeMode;
use crate::ui::layout::{Align, Insets, LayoutStyle, Value};
use crate::ui::unit::{Dp, Sp, UnitContext};

use super::common::{
    CanvasItemInteractionHandlers, ClipMask, CursorStyle, InteractionHandlers,
    LifecycleEventHandlers, MediaEventHandlers, MeshPrimitive, MeshVertex, Point, Rect,
    TextPrimitive, TexturePrimitive, VisualStyle, WidgetId, WidgetKind,
};
use super::container::{set_layout_inset, set_layout_length, set_layout_lengths, IntoLengthValue};
use super::core::Element;
use super::style::CanvasStyle;

const MAX_CANVAS_GRADIENT_STOPS: usize = 8;
const CANVAS_FLATTEN_TOLERANCE: f32 = 0.1;
const SHADOW_BLUR_PADDING_MULTIPLIER: f32 = 3.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CanvasItemId(u64);

impl CanvasItemId {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

impl From<u64> for CanvasItemId {
    fn from(value: u64) -> Self {
        Self(value)
    }
}

impl From<u32> for CanvasItemId {
    fn from(value: u32) -> Self {
        Self(value as u64)
    }
}

impl From<usize> for CanvasItemId {
    fn from(value: usize) -> Self {
        Self(value as u64)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CanvasMouseButton {
    Left,
    Right,
    Middle,
    Back,
    Forward,
    Other(u16),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CanvasMouseEvent {
    pub item_id: CanvasItemId,
    pub button: Option<CanvasMouseButton>,
    pub canvas_position: Point,
    pub scene_position: Point,
    pub local_position: Point,
}

pub type CanvasPointerEvent = CanvasMouseEvent;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CanvasWheelEvent {
    pub item_id: CanvasItemId,
    pub delta: Point,
    pub canvas_position: Point,
    pub scene_position: Point,
    pub local_position: Point,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CanvasDragEvent {
    pub item_id: CanvasItemId,
    pub button: CanvasMouseButton,
    pub start_canvas_position: Point,
    pub start_scene_position: Point,
    pub start_local_position: Point,
    pub canvas_position: Point,
    pub scene_position: Point,
    pub local_position: Point,
    pub delta: Point,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CanvasGradientStop {
    pub offset: f32,
    pub color: Color,
}

impl CanvasGradientStop {
    pub fn new(offset: f32, color: Color) -> Self {
        Self {
            offset: offset.clamp(0.0, 1.0),
            color,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct CanvasLinearGradient {
    pub start: Point,
    pub end: Point,
    pub stops: Vec<CanvasGradientStop>,
}

impl CanvasLinearGradient {
    pub fn new(
        start: impl Into<Point>,
        end: impl Into<Point>,
        stops: impl Into<Vec<CanvasGradientStop>>,
    ) -> Self {
        Self {
            start: start.into(),
            end: end.into(),
            stops: stops.into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct CanvasRadialGradient {
    pub center: Point,
    pub radius: Dp,
    pub stops: Vec<CanvasGradientStop>,
}

impl CanvasRadialGradient {
    pub fn new(
        center: impl Into<Point>,
        radius: impl Into<Dp>,
        stops: impl Into<Vec<CanvasGradientStop>>,
    ) -> Self {
        Self {
            center: center.into(),
            radius: radius.into(),
            stops: stops.into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum CanvasBrush {
    Solid(Color),
    LinearGradient(CanvasLinearGradient),
    RadialGradient(CanvasRadialGradient),
}

impl From<Color> for CanvasBrush {
    fn from(value: Color) -> Self {
        Self::Solid(value)
    }
}

impl From<CanvasLinearGradient> for CanvasBrush {
    fn from(value: CanvasLinearGradient) -> Self {
        Self::LinearGradient(value)
    }
}

impl From<CanvasRadialGradient> for CanvasBrush {
    fn from(value: CanvasRadialGradient) -> Self {
        Self::RadialGradient(value)
    }
}

impl From<Color> for Value<CanvasBrush> {
    fn from(value: Color) -> Self {
        Value::Static(CanvasBrush::Solid(value))
    }
}

impl From<CanvasLinearGradient> for Value<CanvasBrush> {
    fn from(value: CanvasLinearGradient) -> Self {
        Value::Static(CanvasBrush::LinearGradient(value))
    }
}

impl From<CanvasRadialGradient> for Value<CanvasBrush> {
    fn from(value: CanvasRadialGradient) -> Self {
        Value::Static(CanvasBrush::RadialGradient(value))
    }
}

impl From<Signal<Color>> for Value<CanvasBrush> {
    fn from(value: Signal<Color>) -> Self {
        Value::Signal(value.map(CanvasBrush::Solid))
    }
}

impl From<Value<Color>> for Value<CanvasBrush> {
    fn from(value: Value<Color>) -> Self {
        match value {
            Value::Static(color) => Value::Static(CanvasBrush::Solid(color)),
            Value::Signal(signal) => Value::Signal(signal.map(CanvasBrush::Solid)),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CanvasShadow {
    pub color: Color,
    pub offset: Point,
    pub blur: Dp,
}

impl CanvasShadow {
    pub fn new(color: Color, offset: impl Into<Point>, blur: impl Into<Dp>) -> Self {
        Self {
            color,
            offset: offset.into(),
            blur: blur.into(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum CanvasBlendMode {
    #[default]
    Normal,
    Multiply,
    Screen,
    Overlay,
    Darken,
    Lighten,
    ColorDodge,
    ColorBurn,
    HardLight,
    SoftLight,
    Difference,
    Exclusion,
    Plus,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum CanvasFillRule {
    #[default]
    NonZero,
    EvenOdd,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum CanvasStrokeCap {
    #[default]
    Butt,
    Square,
    Round,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum CanvasStrokeJoin {
    #[default]
    Miter,
    Bevel,
    Round,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum CanvasStrokeAlignment {
    #[default]
    Center,
    Inside,
    Outside,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CanvasTransform2D {
    pub matrix: [f32; 6],
}

impl Default for CanvasTransform2D {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl CanvasTransform2D {
    pub const IDENTITY: Self = Self {
        matrix: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
    };

    pub const fn from_matrix(matrix: [f32; 6]) -> Self {
        Self { matrix }
    }

    pub fn translate(x: impl Into<Dp>, y: impl Into<Dp>) -> Self {
        Self::from_matrix([1.0, 0.0, 0.0, 1.0, x.into().get(), y.into().get()])
    }

    pub fn scale(x: f32, y: f32) -> Self {
        Self::from_matrix([x, 0.0, 0.0, y, 0.0, 0.0])
    }

    pub fn rotate(radians: f32) -> Self {
        let (sin, cos) = radians.sin_cos();
        Self::from_matrix([cos, sin, -sin, cos, 0.0, 0.0])
    }

    pub fn then(self, other: Self) -> Self {
        let [a1, b1, c1, d1, e1, f1] = self.matrix;
        let [a2, b2, c2, d2, e2, f2] = other.matrix;
        Self::from_matrix([
            a1 * a2 + c1 * b2,
            b1 * a2 + d1 * b2,
            a1 * c2 + c1 * d2,
            b1 * c2 + d1 * d2,
            a1 * e2 + c1 * f2 + e1,
            b1 * e2 + d1 * f2 + f1,
        ])
    }

    pub fn apply(self, point_value: Point) -> Point {
        let [a, b, c, d, e, f] = self.matrix;
        let x = point_value.x.get();
        let y = point_value.y.get();
        Point::new(a * x + c * y + e, b * x + d * y + f)
    }

    pub fn inverse(self) -> Option<Self> {
        let [a, b, c, d, e, f] = self.matrix;
        let det = a * d - b * c;
        if det.abs() <= f32::EPSILON {
            return None;
        }
        let inv_det = 1.0 / det;
        Some(Self::from_matrix([
            d * inv_det,
            -b * inv_det,
            -c * inv_det,
            a * inv_det,
            (c * f - d * e) * inv_det,
            (b * e - a * f) * inv_det,
        ]))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct CanvasItemStyle {
    pub id: CanvasItemId,
    pub transform: CanvasTransform2D,
    pub opacity: f32,
    pub blend_mode: CanvasBlendMode,
    pub cursor: Option<CursorStyle>,
    pub visible: bool,
    pub hit_test: bool,
}

impl CanvasItemStyle {
    pub fn new(id: impl Into<CanvasItemId>) -> Self {
        Self {
            id: id.into(),
            transform: CanvasTransform2D::IDENTITY,
            opacity: 1.0,
            blend_mode: CanvasBlendMode::Normal,
            cursor: None,
            visible: true,
            hit_test: true,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CanvasBooleanOp {
    Union,
    Intersection,
    Difference,
    Xor,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CanvasPathOpError {
    OpenSubpath,
    InvalidGeometry,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CanvasSvgPathError(pub String);

#[derive(Clone, Debug)]
pub struct CanvasStroke {
    pub width: Dp,
    pub brush: Value<CanvasBrush>,
    pub dash_pattern: Option<Vec<Dp>>,
    pub dash_offset: Dp,
    pub line_cap: CanvasStrokeCap,
    pub line_join: CanvasStrokeJoin,
    pub miter_limit: f32,
    pub alignment: CanvasStrokeAlignment,
}

impl CanvasStroke {
    pub fn new(width: impl Into<Dp>, color: Color) -> Self {
        Self::with_brush(width, CanvasBrush::Solid(color))
    }

    pub fn with_brush(width: impl Into<Dp>, brush: impl Into<Value<CanvasBrush>>) -> Self {
        Self {
            width: width.into(),
            brush: brush.into(),
            dash_pattern: None,
            dash_offset: Dp::ZERO,
            line_cap: CanvasStrokeCap::Butt,
            line_join: CanvasStrokeJoin::Miter,
            miter_limit: StrokeOptions::DEFAULT_MITER_LIMIT,
            alignment: CanvasStrokeAlignment::Center,
        }
    }

    pub fn dash<I, T>(mut self, pattern: I) -> Self
    where
        I: IntoIterator<Item = T>,
        T: Into<Dp>,
    {
        self.dash_pattern = Some(pattern.into_iter().map(Into::into).collect());
        self
    }

    pub fn dash_offset(mut self, offset: impl Into<Dp>) -> Self {
        self.dash_offset = offset.into();
        self
    }

    pub fn line_cap(mut self, line_cap: CanvasStrokeCap) -> Self {
        self.line_cap = line_cap;
        self
    }

    pub fn line_join(mut self, line_join: CanvasStrokeJoin) -> Self {
        self.line_join = line_join;
        self
    }

    pub fn miter_limit(mut self, miter_limit: f32) -> Self {
        self.miter_limit = miter_limit.max(0.0);
        self
    }

    pub fn alignment(mut self, alignment: CanvasStrokeAlignment) -> Self {
        self.alignment = alignment;
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum PathCommand {
    MoveTo(Point),
    LineTo(Point),
    QuadTo {
        ctrl: Point,
        to: Point,
    },
    CubicTo {
        ctrl1: Point,
        ctrl2: Point,
        to: Point,
    },
    Close,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct PathBuilder {
    commands: Vec<PathCommand>,
}

impl PathBuilder {
    pub fn new() -> Self {
        Self {
            commands: Vec::new(),
        }
    }

    pub fn move_to(mut self, x: impl Into<Dp>, y: impl Into<Dp>) -> Self {
        self.commands.push(PathCommand::MoveTo(Point::new(x, y)));
        self
    }

    pub fn line_to(mut self, x: impl Into<Dp>, y: impl Into<Dp>) -> Self {
        self.commands.push(PathCommand::LineTo(Point::new(x, y)));
        self
    }

    pub fn quad_to(
        mut self,
        ctrl_x: impl Into<Dp>,
        ctrl_y: impl Into<Dp>,
        x: impl Into<Dp>,
        y: impl Into<Dp>,
    ) -> Self {
        self.commands.push(PathCommand::QuadTo {
            ctrl: Point::new(ctrl_x, ctrl_y),
            to: Point::new(x, y),
        });
        self
    }

    pub fn cubic_to(
        mut self,
        ctrl1_x: impl Into<Dp>,
        ctrl1_y: impl Into<Dp>,
        ctrl2_x: impl Into<Dp>,
        ctrl2_y: impl Into<Dp>,
        x: impl Into<Dp>,
        y: impl Into<Dp>,
    ) -> Self {
        self.commands.push(PathCommand::CubicTo {
            ctrl1: Point::new(ctrl1_x, ctrl1_y),
            ctrl2: Point::new(ctrl2_x, ctrl2_y),
            to: Point::new(x, y),
        });
        self
    }

    pub fn close(mut self) -> Self {
        self.commands.push(PathCommand::Close);
        self
    }

    pub fn rect(
        self,
        x: impl Into<Dp>,
        y: impl Into<Dp>,
        width: impl Into<Dp>,
        height: impl Into<Dp>,
    ) -> Self {
        let x = x.into();
        let y = y.into();
        let width = width.into().max(Dp::ZERO);
        let height = height.into().max(Dp::ZERO);
        self.move_to(x, y)
            .line_to(x + width, y)
            .line_to(x + width, y + height)
            .line_to(x, y + height)
            .close()
    }

    pub fn rounded_rect(
        self,
        x: impl Into<Dp>,
        y: impl Into<Dp>,
        width: impl Into<Dp>,
        height: impl Into<Dp>,
        radius: impl Into<Dp>,
    ) -> Self {
        let x = x.into();
        let y = y.into();
        let width = width.into().max(Dp::ZERO);
        let height = height.into().max(Dp::ZERO);
        let radius = radius
            .into()
            .max(Dp::ZERO)
            .min(width * 0.5)
            .min(height * 0.5);
        if radius <= Dp::ZERO {
            return self.rect(x, y, width, height);
        }

        let right = x + width;
        let bottom = y + height;
        let r = radius.get();
        let mut builder = self.move_to(x + radius, y);
        builder = append_arc_segments(
            builder,
            Point::new(right - radius, y + radius),
            r,
            -std::f32::consts::FRAC_PI_2,
            std::f32::consts::FRAC_PI_2,
            false,
        );
        builder = append_arc_segments(
            builder,
            Point::new(right - radius, bottom - radius),
            r,
            0.0,
            std::f32::consts::FRAC_PI_2,
            true,
        );
        builder = append_arc_segments(
            builder,
            Point::new(x + radius, bottom - radius),
            r,
            std::f32::consts::FRAC_PI_2,
            std::f32::consts::FRAC_PI_2,
            true,
        );
        builder = append_arc_segments(
            builder,
            Point::new(x + radius, y + radius),
            r,
            std::f32::consts::PI,
            std::f32::consts::FRAC_PI_2,
            true,
        );
        builder.close()
    }

    pub fn circle(
        self,
        center_x: impl Into<Dp>,
        center_y: impl Into<Dp>,
        radius: impl Into<Dp>,
    ) -> Self {
        let radius = radius.into();
        self.ellipse(center_x, center_y, radius, radius)
    }

    pub fn ellipse(
        self,
        center_x: impl Into<Dp>,
        center_y: impl Into<Dp>,
        radius_x: impl Into<Dp>,
        radius_y: impl Into<Dp>,
    ) -> Self {
        let center = Point::new(center_x, center_y);
        let radius_x = radius_x.into().max(Dp::ZERO).get();
        let radius_y = radius_y.into().max(Dp::ZERO).get();
        if radius_x <= 0.0 || radius_y <= 0.0 {
            return self;
        }

        let segments = 32usize;
        let mut builder = self.move_to(center.x.get() + radius_x, center.y.get());
        for index in 1..segments {
            let angle = (index as f32 / segments as f32) * std::f32::consts::TAU;
            builder = builder.line_to(
                center.x.get() + angle.cos() * radius_x,
                center.y.get() + angle.sin() * radius_y,
            );
        }
        builder.close()
    }

    pub fn arc(
        self,
        center_x: impl Into<Dp>,
        center_y: impl Into<Dp>,
        radius: impl Into<Dp>,
        start_angle: f32,
        sweep_angle: f32,
    ) -> Self {
        let center = Point::new(center_x, center_y);
        let connect_with_line = !self.commands.is_empty();
        append_arc_segments(
            self,
            center,
            radius.into().max(Dp::ZERO).get(),
            start_angle,
            sweep_angle,
            connect_with_line,
        )
    }

    pub fn arc_to(
        self,
        ctrl_x: impl Into<Dp>,
        ctrl_y: impl Into<Dp>,
        x: impl Into<Dp>,
        y: impl Into<Dp>,
        radius: impl Into<Dp>,
    ) -> Self {
        let Some(PathCommand::MoveTo(from) | PathCommand::LineTo(from)) =
            self.commands.last().copied()
        else {
            return self.move_to(ctrl_x, ctrl_y).line_to(x, y);
        };

        let ctrl = Point::new(ctrl_x, ctrl_y);
        let to = Point::new(x, y);
        let radius = radius.into().max(Dp::ZERO).get();
        if radius <= 0.0 {
            return self.line_to(ctrl.x, ctrl.y).line_to(to.x, to.y);
        }

        let p0 = (from.x.get(), from.y.get());
        let p1 = (ctrl.x.get(), ctrl.y.get());
        let p2 = (to.x.get(), to.y.get());
        let v1 = normalize_vec((p0.0 - p1.0, p0.1 - p1.1));
        let v2 = normalize_vec((p2.0 - p1.0, p2.1 - p1.1));
        let dot = (v1.0 * v2.0 + v1.1 * v2.1).clamp(-1.0, 1.0);
        let angle = dot.acos();
        if angle <= 1e-3 || (std::f32::consts::PI - angle).abs() <= 1e-3 {
            return self.line_to(ctrl.x, ctrl.y).line_to(to.x, to.y);
        }

        let tangent = radius / (angle * 0.5).tan();
        let start = (p1.0 + v1.0 * tangent, p1.1 + v1.1 * tangent);
        let end = (p1.0 + v2.0 * tangent, p1.1 + v2.1 * tangent);
        let bisector = normalize_vec((v1.0 + v2.0, v1.1 + v2.1));
        let center_distance = radius / (angle * 0.5).sin();
        let center = (
            p1.0 + bisector.0 * center_distance,
            p1.1 + bisector.1 * center_distance,
        );
        let start_angle = (start.1 - center.1).atan2(start.0 - center.0);
        let mut sweep = (end.1 - center.1).atan2(end.0 - center.0) - start_angle;
        if sweep <= -std::f32::consts::PI {
            sweep += std::f32::consts::TAU;
        } else if sweep > std::f32::consts::PI {
            sweep -= std::f32::consts::TAU;
        }

        append_arc_segments(
            self.line_to(start.0, start.1),
            Point::new(center.0, center.1),
            radius,
            start_angle,
            sweep,
            true,
        )
    }

    pub fn svg_path(mut self, data: &str) -> Result<Self, CanvasSvgPathError> {
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
                svgtypes::PathSegment::EllipticalArc { abs, x, y, .. } => {
                    let to = svg_point(abs, current, x as f32, y as f32);
                    self = self.line_to(to.0, to.1);
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

    pub fn boolean(
        &self,
        op: CanvasBooleanOp,
        other: &PathBuilder,
    ) -> Result<PathBuilder, CanvasPathOpError> {
        let lhs = self.to_multi_polygon()?;
        let rhs = other.to_multi_polygon()?;
        let result = match op {
            CanvasBooleanOp::Union => lhs.union(&rhs),
            CanvasBooleanOp::Intersection => lhs.intersection(&rhs),
            CanvasBooleanOp::Difference => lhs.difference(&rhs),
            CanvasBooleanOp::Xor => lhs.xor(&rhs),
        };
        Ok(Self::from_multi_polygon(&result))
    }

    pub fn union(&self, other: &PathBuilder) -> Result<PathBuilder, CanvasPathOpError> {
        self.boolean(CanvasBooleanOp::Union, other)
    }

    pub fn intersection(&self, other: &PathBuilder) -> Result<PathBuilder, CanvasPathOpError> {
        self.boolean(CanvasBooleanOp::Intersection, other)
    }

    pub fn difference(&self, other: &PathBuilder) -> Result<PathBuilder, CanvasPathOpError> {
        self.boolean(CanvasBooleanOp::Difference, other)
    }

    pub fn xor(&self, other: &PathBuilder) -> Result<PathBuilder, CanvasPathOpError> {
        self.boolean(CanvasBooleanOp::Xor, other)
    }

    fn commands_internal(&self) -> &[PathCommand] {
        &self.commands
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
        (path.iter().next().is_some()).then(|| bounding_box(path.iter()))
    }

    fn to_multi_polygon(&self) -> Result<MultiPolygon<f64>, CanvasPathOpError> {
        let rings = self.flattened_closed_rings()?;
        Ok(MultiPolygon(
            rings
                .into_iter()
                .map(|ring| {
                    Polygon::new(
                        LineString::from(
                            ring.into_iter()
                                .map(|point_value| Coord {
                                    x: point_value.x as f64,
                                    y: point_value.y as f64,
                                })
                                .collect::<Vec<_>>(),
                        ),
                        Vec::new(),
                    )
                })
                .collect(),
        ))
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

#[derive(Clone, Debug)]
pub struct CanvasPath {
    pub style: CanvasItemStyle,
    pub path: PathBuilder,
    pub fill: Option<Value<CanvasBrush>>,
    pub fill_rule: CanvasFillRule,
    pub stroke: Option<CanvasStroke>,
    pub shadow: Option<Value<CanvasShadow>>,
}

impl CanvasPath {
    pub fn new(id: impl Into<CanvasItemId>, path: PathBuilder) -> Self {
        Self {
            style: CanvasItemStyle::new(id),
            path,
            fill: None,
            fill_rule: CanvasFillRule::NonZero,
            stroke: None,
            shadow: None,
        }
    }

    pub fn fill(mut self, brush: impl Into<Value<CanvasBrush>>) -> Self {
        self.fill = Some(brush.into());
        self
    }

    pub fn stroke(mut self, stroke: CanvasStroke) -> Self {
        self.stroke = Some(stroke);
        self
    }

    pub fn fill_rule(mut self, fill_rule: CanvasFillRule) -> Self {
        self.fill_rule = fill_rule;
        self
    }

    pub fn shadow(mut self, shadow: impl Into<Value<CanvasShadow>>) -> Self {
        self.shadow = Some(shadow.into());
        self
    }

    pub fn transform(mut self, transform: CanvasTransform2D) -> Self {
        self.style.transform = transform;
        self
    }

    pub fn opacity(mut self, opacity: f32) -> Self {
        self.style.opacity = opacity.clamp(0.0, 1.0);
        self
    }

    pub fn blend_mode(mut self, blend_mode: CanvasBlendMode) -> Self {
        self.style.blend_mode = blend_mode;
        self
    }

    pub fn cursor(mut self, cursor: CursorStyle) -> Self {
        self.style.cursor = Some(cursor);
        self
    }

    pub fn visible(mut self, visible: bool) -> Self {
        self.style.visible = visible;
        self
    }

    pub fn hit_test(mut self, hit_test: bool) -> Self {
        self.style.hit_test = hit_test;
        self
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct CanvasTextStyle {
    pub font_family: Option<String>,
    pub color: Color,
    pub font_size: Sp,
    pub font_weight: FontWeight,
    pub line_height: Option<Sp>,
    pub letter_spacing: Sp,
}

impl Default for CanvasTextStyle {
    fn default() -> Self {
        Self {
            font_family: None,
            color: Color::BLACK,
            font_size: Sp::new(14.0),
            font_weight: FontWeight::NORMAL,
            line_height: None,
            letter_spacing: Sp::ZERO,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum CanvasTextWrap {
    #[default]
    Word,
    Glyph,
    None,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum CanvasTextHorizontalAlign {
    #[default]
    Start,
    Center,
    End,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum CanvasTextVerticalAlign {
    #[default]
    Start,
    Center,
    End,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum CanvasTextOverflow {
    #[default]
    Clip,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CanvasParagraphStyle {
    pub wrap: CanvasTextWrap,
    pub horizontal_align: CanvasTextHorizontalAlign,
    pub vertical_align: CanvasTextVerticalAlign,
    pub overflow: CanvasTextOverflow,
}

impl Default for CanvasParagraphStyle {
    fn default() -> Self {
        Self {
            wrap: CanvasTextWrap::Word,
            horizontal_align: CanvasTextHorizontalAlign::Start,
            vertical_align: CanvasTextVerticalAlign::Start,
            overflow: CanvasTextOverflow::Clip,
        }
    }
}

#[derive(Clone, Debug)]
pub struct CanvasText {
    pub style: CanvasItemStyle,
    pub frame: Rect,
    pub content: String,
    pub text_style: CanvasTextStyle,
    pub paragraph_style: CanvasParagraphStyle,
}

impl CanvasText {
    pub fn new(id: impl Into<CanvasItemId>, frame: Rect, content: impl Into<String>) -> Self {
        Self {
            style: CanvasItemStyle::new(id),
            frame,
            content: content.into(),
            text_style: CanvasTextStyle::default(),
            paragraph_style: CanvasParagraphStyle::default(),
        }
    }

    pub fn text_style(mut self, text_style: CanvasTextStyle) -> Self {
        self.text_style = text_style;
        self
    }

    pub fn paragraph_style(mut self, paragraph_style: CanvasParagraphStyle) -> Self {
        self.paragraph_style = paragraph_style;
        self
    }

    pub fn transform(mut self, transform: CanvasTransform2D) -> Self {
        self.style.transform = transform;
        self
    }

    pub fn opacity(mut self, opacity: f32) -> Self {
        self.style.opacity = opacity.clamp(0.0, 1.0);
        self
    }

    pub fn blend_mode(mut self, blend_mode: CanvasBlendMode) -> Self {
        self.style.blend_mode = blend_mode;
        self
    }

    pub fn cursor(mut self, cursor: CursorStyle) -> Self {
        self.style.cursor = Some(cursor);
        self
    }

    pub fn visible(mut self, visible: bool) -> Self {
        self.style.visible = visible;
        self
    }

    pub fn hit_test(mut self, hit_test: bool) -> Self {
        self.style.hit_test = hit_test;
        self
    }
}

#[derive(Clone, Debug)]
pub struct CanvasImage {
    pub style: CanvasItemStyle,
    pub frame: Rect,
    pub source: MediaSource,
    pub fit: ContentFit,
    pub corner_radius: Dp,
}

impl CanvasImage {
    pub fn new(id: impl Into<CanvasItemId>, frame: Rect, source: impl Into<MediaSource>) -> Self {
        Self {
            style: CanvasItemStyle::new(id),
            frame,
            source: source.into(),
            fit: ContentFit::Contain,
            corner_radius: Dp::ZERO,
        }
    }

    pub fn fit(mut self, fit: ContentFit) -> Self {
        self.fit = fit;
        self
    }

    pub fn corner_radius(mut self, corner_radius: impl Into<Dp>) -> Self {
        self.corner_radius = corner_radius.into().max(Dp::ZERO);
        self
    }

    pub fn transform(mut self, transform: CanvasTransform2D) -> Self {
        self.style.transform = transform;
        self
    }

    pub fn opacity(mut self, opacity: f32) -> Self {
        self.style.opacity = opacity.clamp(0.0, 1.0);
        self
    }

    pub fn blend_mode(mut self, blend_mode: CanvasBlendMode) -> Self {
        self.style.blend_mode = blend_mode;
        self
    }

    pub fn cursor(mut self, cursor: CursorStyle) -> Self {
        self.style.cursor = Some(cursor);
        self
    }

    pub fn visible(mut self, visible: bool) -> Self {
        self.style.visible = visible;
        self
    }

    pub fn hit_test(mut self, hit_test: bool) -> Self {
        self.style.hit_test = hit_test;
        self
    }
}

#[derive(Clone, Debug)]
pub struct CanvasGroup {
    pub style: CanvasItemStyle,
    pub items: Vec<CanvasItem>,
}

impl CanvasGroup {
    pub fn new(id: impl Into<CanvasItemId>, items: impl Into<Vec<CanvasItem>>) -> Self {
        Self {
            style: CanvasItemStyle::new(id),
            items: items.into(),
        }
    }

    pub fn transform(mut self, transform: CanvasTransform2D) -> Self {
        self.style.transform = transform;
        self
    }

    pub fn opacity(mut self, opacity: f32) -> Self {
        self.style.opacity = opacity.clamp(0.0, 1.0);
        self
    }

    pub fn blend_mode(mut self, blend_mode: CanvasBlendMode) -> Self {
        self.style.blend_mode = blend_mode;
        self
    }

    pub fn visible(mut self, visible: bool) -> Self {
        self.style.visible = visible;
        self
    }

    pub fn hit_test(mut self, hit_test: bool) -> Self {
        self.style.hit_test = hit_test;
        self
    }
}

#[derive(Clone, Debug)]
pub enum CanvasClipShape {
    Rect(Rect),
    RoundedRect { rect: Rect, radius: Dp },
    Path(PathBuilder),
}

#[derive(Clone, Debug)]
pub struct CanvasClip {
    pub style: CanvasItemStyle,
    pub clip: CanvasClipShape,
    pub items: Vec<CanvasItem>,
}

impl CanvasClip {
    pub fn new(
        id: impl Into<CanvasItemId>,
        clip: CanvasClipShape,
        items: impl Into<Vec<CanvasItem>>,
    ) -> Self {
        Self {
            style: CanvasItemStyle::new(id),
            clip,
            items: items.into(),
        }
    }

    pub fn transform(mut self, transform: CanvasTransform2D) -> Self {
        self.style.transform = transform;
        self
    }

    pub fn opacity(mut self, opacity: f32) -> Self {
        self.style.opacity = opacity.clamp(0.0, 1.0);
        self
    }

    pub fn visible(mut self, visible: bool) -> Self {
        self.style.visible = visible;
        self
    }
}

#[derive(Clone, Debug)]
pub struct CanvasLayer {
    pub style: CanvasItemStyle,
    pub items: Vec<CanvasItem>,
}

impl CanvasLayer {
    pub fn new(id: impl Into<CanvasItemId>, items: impl Into<Vec<CanvasItem>>) -> Self {
        Self {
            style: CanvasItemStyle::new(id),
            items: items.into(),
        }
    }

    pub fn transform(mut self, transform: CanvasTransform2D) -> Self {
        self.style.transform = transform;
        self
    }

    pub fn opacity(mut self, opacity: f32) -> Self {
        self.style.opacity = opacity.clamp(0.0, 1.0);
        self
    }

    pub fn blend_mode(mut self, blend_mode: CanvasBlendMode) -> Self {
        self.style.blend_mode = blend_mode;
        self
    }

    pub fn visible(mut self, visible: bool) -> Self {
        self.style.visible = visible;
        self
    }
}

#[derive(Clone, Debug)]
pub struct CanvasMask {
    pub style: CanvasItemStyle,
    pub mask: Vec<CanvasItem>,
    pub content: Vec<CanvasItem>,
}

impl CanvasMask {
    pub fn new(
        id: impl Into<CanvasItemId>,
        mask: impl Into<Vec<CanvasItem>>,
        content: impl Into<Vec<CanvasItem>>,
    ) -> Self {
        Self {
            style: CanvasItemStyle::new(id),
            mask: mask.into(),
            content: content.into(),
        }
    }

    pub fn transform(mut self, transform: CanvasTransform2D) -> Self {
        self.style.transform = transform;
        self
    }

    pub fn opacity(mut self, opacity: f32) -> Self {
        self.style.opacity = opacity.clamp(0.0, 1.0);
        self
    }

    pub fn blend_mode(mut self, blend_mode: CanvasBlendMode) -> Self {
        self.style.blend_mode = blend_mode;
        self
    }

    pub fn visible(mut self, visible: bool) -> Self {
        self.style.visible = visible;
        self
    }
}

#[derive(Clone, Debug)]
pub enum CanvasItem {
    Path(CanvasPath),
    Text(CanvasText),
    Image(CanvasImage),
    Group(CanvasGroup),
    Clip(CanvasClip),
    Layer(CanvasLayer),
    Mask(CanvasMask),
}

impl CanvasItem {
    pub fn path(id: impl Into<CanvasItemId>, path: PathBuilder) -> Self {
        Self::Path(CanvasPath::new(id, path))
    }

    pub fn text(id: impl Into<CanvasItemId>, frame: Rect, content: impl Into<String>) -> Self {
        Self::Text(CanvasText::new(id, frame, content))
    }

    pub fn image(id: impl Into<CanvasItemId>, frame: Rect, source: impl Into<MediaSource>) -> Self {
        Self::Image(CanvasImage::new(id, frame, source))
    }

    pub fn group(id: impl Into<CanvasItemId>, items: impl Into<Vec<CanvasItem>>) -> Self {
        Self::Group(CanvasGroup::new(id, items))
    }

    pub(crate) fn id(&self) -> CanvasItemId {
        match self {
            Self::Path(path) => path.style.id,
            Self::Text(text) => text.style.id,
            Self::Image(image) => image.style.id,
            Self::Group(group) => group.style.id,
            Self::Clip(clip) => clip.style.id,
            Self::Layer(layer) => layer.style.id,
            Self::Mask(mask) => mask.style.id,
        }
    }

    pub(crate) fn layout_bounds(&self) -> Option<RectBounds> {
        let bounds = match self {
            Self::Path(path) => {
                let mut rect = path_base_bounds(path)?;
                if let Some(shadow) = path.shadow.as_ref().map(Value::resolve) {
                    rect = rect.expand_for_shadow(shadow);
                }
                Some(rect)
            }
            Self::Text(text) => Some(RectBounds::from_rect(text.frame)),
            Self::Image(image) => Some(RectBounds::from_rect(image.frame)),
            Self::Group(group) => canvas_bounds(&group.items),
            Self::Clip(clip) => {
                clip_shape_bounds(&clip.clip).or_else(|| canvas_bounds(&clip.items))
            }
            Self::Layer(layer) => canvas_bounds(&layer.items),
            Self::Mask(mask) => canvas_bounds(&mask.content),
        }?;
        Some(transform_bounds(bounds, self.style().transform))
    }

    pub(crate) fn hit_bounds(&self) -> Option<RectBounds> {
        if !self.style().hit_test || !self.style().visible {
            return None;
        }
        let bounds = match self {
            Self::Path(path) => path_base_bounds(path),
            Self::Text(text) => Some(RectBounds::from_rect(text.frame)),
            Self::Image(image) => Some(RectBounds::from_rect(image.frame)),
            Self::Group(group) => canvas_bounds(&group.items),
            Self::Clip(clip) => {
                clip_shape_bounds(&clip.clip).or_else(|| canvas_bounds(&clip.items))
            }
            Self::Layer(layer) => canvas_bounds(&layer.items),
            Self::Mask(mask) => canvas_bounds(&mask.content),
        }?;
        Some(transform_bounds(bounds, self.style().transform))
    }

    pub(crate) fn tessellate(
        &self,
        origin: Point,
        opacity: f32,
        clip_context: CanvasClipContext,
        media: &MediaManager,
        units: UnitContext,
    ) -> CanvasRenderOutput {
        if !self.style().visible {
            return CanvasRenderOutput::default();
        }

        let mut output = match self {
            Self::Path(path) => tessellate_path(path, origin, opacity, clip_context, media, units),
            Self::Text(text) => tessellate_text(text, origin, opacity, clip_context),
            Self::Image(image) => {
                tessellate_image(image, origin, opacity, clip_context, media, units)
            }
            Self::Group(group) => tessellate_items(
                &group.items,
                origin,
                opacity * group.style.opacity,
                clip_context,
                media,
                units,
            ),
            Self::Clip(clip) => {
                let nested_clip = CanvasClipContext {
                    clip_rect: compose_clip_rect(
                        clip_context.clip_rect,
                        clip_shape_clip_rect(&clip.clip, origin),
                    ),
                    clip_mask: clip_context.clip_mask,
                };
                tessellate_items(
                    &clip.items,
                    origin,
                    opacity * clip.style.opacity,
                    nested_clip,
                    media,
                    units,
                )
            }
            Self::Layer(layer) => tessellate_items(
                &layer.items,
                origin,
                opacity * layer.style.opacity,
                clip_context,
                media,
                units,
            ),
            Self::Mask(mask) => tessellate_items(
                &mask.content,
                origin,
                opacity * mask.style.opacity,
                clip_context,
                media,
                units,
            ),
        };

        apply_transform_to_output(&mut output, self.style().transform, origin);
        output
    }

    pub(crate) fn style(&self) -> &CanvasItemStyle {
        match self {
            Self::Path(path) => &path.style,
            Self::Text(text) => &text.style,
            Self::Image(image) => &image.style,
            Self::Group(group) => &group.style,
            Self::Clip(clip) => &clip.style,
            Self::Layer(layer) => &layer.style,
            Self::Mask(mask) => &mask.style,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct RectBounds {
    pub min_x: f32,
    pub min_y: f32,
    pub max_x: f32,
    pub max_y: f32,
}

impl RectBounds {
    pub(crate) fn from_min_max(min_x: f32, min_y: f32, max_x: f32, max_y: f32) -> Self {
        Self {
            min_x,
            min_y,
            max_x,
            max_y,
        }
    }

    pub(crate) fn from_rect(rect: Rect) -> Self {
        Self::from_min_max(
            rect.x.get(),
            rect.y.get(),
            rect.right().get(),
            rect.bottom().get(),
        )
    }

    pub(crate) fn width(self) -> f32 {
        (self.max_x - self.min_x).max(0.0)
    }

    pub(crate) fn height(self) -> f32 {
        (self.max_y - self.min_y).max(0.0)
    }

    pub(crate) fn expand(self, amount: f32) -> Self {
        Self {
            min_x: self.min_x - amount,
            min_y: self.min_y - amount,
            max_x: self.max_x + amount,
            max_y: self.max_y + amount,
        }
    }

    pub(crate) fn expand_for_shadow(self, shadow: CanvasShadow) -> Self {
        let padding = shadow_padding(shadow.blur);
        Self {
            min_x: self.min_x + shadow.offset.x.get().min(0.0) - padding,
            min_y: self.min_y + shadow.offset.y.get().min(0.0) - padding,
            max_x: self.max_x + shadow.offset.x.get().max(0.0) + padding,
            max_y: self.max_y + shadow.offset.y.get().max(0.0) + padding,
        }
    }

    pub(crate) fn union(self, other: Self) -> Self {
        Self {
            min_x: self.min_x.min(other.min_x),
            min_y: self.min_y.min(other.min_y),
            max_x: self.max_x.max(other.max_x),
            max_y: self.max_y.max(other.max_y),
        }
    }
}

fn transform_bounds(bounds: RectBounds, transform: CanvasTransform2D) -> RectBounds {
    let corners = [
        Point::new(bounds.min_x, bounds.min_y),
        Point::new(bounds.max_x, bounds.min_y),
        Point::new(bounds.max_x, bounds.max_y),
        Point::new(bounds.min_x, bounds.max_y),
    ];
    let mut min_x = f32::INFINITY;
    let mut min_y = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut max_y = f32::NEG_INFINITY;
    for corner in corners {
        let transformed = transform.apply(corner);
        min_x = min_x.min(transformed.x.get());
        min_y = min_y.min(transformed.y.get());
        max_x = max_x.max(transformed.x.get());
        max_y = max_y.max(transformed.y.get());
    }
    RectBounds::from_min_max(min_x, min_y, max_x, max_y)
}

fn clip_shape_bounds(shape: &CanvasClipShape) -> Option<RectBounds> {
    match shape {
        CanvasClipShape::Rect(rect) => Some(RectBounds::from_rect(*rect)),
        CanvasClipShape::RoundedRect { rect, .. } => Some(RectBounds::from_rect(*rect)),
        CanvasClipShape::Path(path) => path.control_bounds().map(|bounds| {
            RectBounds::from_min_max(bounds.min.x, bounds.min.y, bounds.max.x, bounds.max.y)
        }),
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct CanvasClipContext {
    pub(crate) clip_rect: Option<Rect>,
    pub(crate) clip_mask: Option<ClipMask>,
}

fn clip_shape_clip_rect(shape: &CanvasClipShape, origin: Point) -> Option<Rect> {
    match shape {
        CanvasClipShape::Rect(rect) => Some(offset_rect(*rect, origin)),
        CanvasClipShape::RoundedRect { rect, .. } => Some(offset_rect(*rect, origin)),
        CanvasClipShape::Path(path) => path.control_bounds().map(|bounds| {
            Rect::new(
                origin.x + bounds.min.x,
                origin.y + bounds.min.y,
                bounds.max.x - bounds.min.x,
                bounds.max.y - bounds.min.y,
            )
        }),
    }
}

fn compose_clip_rect(lhs: Option<Rect>, rhs: Option<Rect>) -> Option<Rect> {
    match (lhs, rhs) {
        (Some(lhs), Some(rhs)) => lhs.intersect(rhs),
        (Some(lhs), None) => Some(lhs),
        (None, Some(rhs)) => Some(rhs),
        (None, None) => None,
    }
}

fn offset_rect(rect: Rect, origin: Point) -> Rect {
    Rect::new(
        origin.x + rect.x,
        origin.y + rect.y,
        rect.width,
        rect.height,
    )
}

fn tessellate_items(
    items: &[CanvasItem],
    origin: Point,
    opacity: f32,
    clip: CanvasClipContext,
    media: &MediaManager,
    units: UnitContext,
) -> CanvasRenderOutput {
    let mut output = CanvasRenderOutput::default();
    for item in items {
        let rendered = item.tessellate(origin, opacity, clip, media, units);
        output.meshes.extend(rendered.meshes);
        output.textures.extend(rendered.textures);
        output.texts.extend(rendered.texts);
    }
    output
}

fn tessellate_text(
    text: &CanvasText,
    origin: Point,
    opacity: f32,
    clip: CanvasClipContext,
) -> CanvasRenderOutput {
    let frame = offset_rect(text.frame, origin);
    let line_height = text
        .text_style
        .line_height
        .unwrap_or(Sp::new(text.text_style.font_size.get() * 1.2));
    CanvasRenderOutput {
        texts: vec![TextPrimitive {
            content: text.content.clone(),
            frame,
            color: text
                .text_style
                .color
                .with_alpha_factor(opacity * text.style.opacity),
            force_color: true,
            font_family: text.text_style.font_family.clone(),
            font_size: text.text_style.font_size.get(),
            font_weight: text.text_style.font_weight,
            line_height: line_height.get(),
            letter_spacing: text.text_style.letter_spacing.get(),
            clip_rect: clip.clip_rect,
            clip_mask: clip.clip_mask,
        }],
        ..Default::default()
    }
}

fn tessellate_image(
    image: &CanvasImage,
    origin: Point,
    _opacity: f32,
    clip: CanvasClipContext,
    media: &MediaManager,
    units: UnitContext,
) -> CanvasRenderOutput {
    let frame = offset_rect(image.frame, origin);
    let metadata = media.image_snapshot(&image.source, None);
    let target_frame = resolve_media_rect(frame, metadata.intrinsic_size, image.fit);
    let snapshot = if let Some(raster_request) =
        RasterRequest::from_frame(target_frame, units.scale_factor())
    {
        media.image_snapshot(&image.source, Some(raster_request))
    } else {
        metadata
    };
    let Some(texture) = snapshot.texture else {
        return CanvasRenderOutput::default();
    };

    CanvasRenderOutput {
        textures: vec![TexturePrimitive {
            texture,
            frame: target_frame,
            corner_radius: image.corner_radius.get(),
            clip_rect: clip.clip_rect,
            clip_mask: clip.clip_mask,
        }],
        ..Default::default()
    }
}

fn apply_transform_to_output(
    output: &mut CanvasRenderOutput,
    transform: CanvasTransform2D,
    origin: Point,
) {
    if transform == CanvasTransform2D::IDENTITY {
        return;
    }

    for mesh in &mut output.meshes {
        let mut vertices = mesh.vertices.to_vec();
        for vertex in &mut vertices {
            let point_value = transform.apply(Point::new(
                vertex.position[0] - origin.x.get(),
                vertex.position[1] - origin.y.get(),
            ));
            vertex.position = [
                origin.x.get() + point_value.x.get(),
                origin.y.get() + point_value.y.get(),
            ];
        }
        mesh.vertices = Arc::from(vertices);

        let mut triangles = mesh.triangles.to_vec();
        for triangle in &mut triangles {
            for point_value in triangle.iter_mut() {
                let transformed = transform.apply(Point::new(
                    point_value.x - origin.x,
                    point_value.y - origin.y,
                ));
                *point_value = Point::new(origin.x + transformed.x, origin.y + transformed.y);
            }
        }
        mesh.triangles = Arc::from(triangles);
    }

    for texture in &mut output.textures {
        if let Some(rect) = transform_axis_aligned_rect(texture.frame, transform) {
            texture.frame = rect;
        }
    }

    for text in &mut output.texts {
        if let Some(rect) = transform_axis_aligned_rect(text.frame, transform) {
            text.frame = rect;
        }
    }
}

fn transform_axis_aligned_rect(rect: Rect, transform: CanvasTransform2D) -> Option<Rect> {
    let [a, b, c, d, e, f] = transform.matrix;
    if b.abs() > 1e-4 || c.abs() > 1e-4 {
        return None;
    }
    Some(Rect::new(
        rect.x.get() * a + e,
        rect.y.get() * d + f,
        rect.width.get() * a.abs(),
        rect.height.get() * d.abs(),
    ))
}

pub(crate) fn canvas_bounds(items: &[CanvasItem]) -> Option<RectBounds> {
    let mut bounds: Option<RectBounds> = None;
    for item in items {
        if let Some(item_bounds) = item.layout_bounds() {
            bounds = Some(match bounds {
                Some(existing) => existing.union(item_bounds),
                None => item_bounds,
            });
        }
    }
    bounds
}

pub struct Canvas<VM> {
    element: Element<VM>,
}

macro_rules! impl_canvas_layout_api {
    () => {
        pub fn size(mut self, width: impl IntoLengthValue, height: impl IntoLengthValue) -> Self {
            set_layout_lengths(&mut self.element.layout, width, height);
            self
        }

        pub fn width(mut self, width: impl IntoLengthValue) -> Self {
            set_layout_length(&mut self.element.layout.width, width);
            self
        }

        pub fn height(mut self, height: impl IntoLengthValue) -> Self {
            set_layout_length(&mut self.element.layout.height, height);
            self
        }

        pub fn min_width(mut self, width: impl IntoLengthValue) -> Self {
            set_layout_length(&mut self.element.layout.min_width, width);
            self
        }

        pub fn min_height(mut self, height: impl IntoLengthValue) -> Self {
            set_layout_length(&mut self.element.layout.min_height, height);
            self
        }

        pub fn max_width(mut self, width: impl IntoLengthValue) -> Self {
            set_layout_length(&mut self.element.layout.max_width, width);
            self
        }

        pub fn max_height(mut self, height: impl IntoLengthValue) -> Self {
            set_layout_length(&mut self.element.layout.max_height, height);
            self
        }

        pub fn aspect_ratio(mut self, aspect_ratio: impl Into<Value<f32>>) -> Self {
            self.element.layout.aspect_ratio = Some(aspect_ratio.into());
            self
        }

        pub fn margin(mut self, insets: impl Into<Value<Insets>>) -> Self {
            self.element.layout.margin = insets.into();
            self
        }

        pub fn padding(mut self, insets: impl Into<Value<Insets>>) -> Self {
            self.element.layout.padding = Some(insets.into());
            self
        }

        pub fn grow(mut self, grow: impl Into<Value<f32>>) -> Self {
            self.element.layout.grow = grow.into();
            self
        }

        pub fn shrink(mut self, shrink: impl Into<Value<f32>>) -> Self {
            self.element.layout.shrink = shrink.into();
            self
        }

        pub fn basis(mut self, basis: impl IntoLengthValue) -> Self {
            self.element.layout.basis = Some(basis.into_length_value());
            self
        }

        pub fn align_self(mut self, align: Align) -> Self {
            self.element.layout.align_self = Some(align);
            self
        }

        pub fn justify_self(mut self, align: Align) -> Self {
            self.element.layout.justify_self = Some(align);
            self
        }

        pub fn column(mut self, start: usize) -> Self {
            self.element.layout.column_start = Some(start.max(1));
            self
        }

        pub fn row(mut self, start: usize) -> Self {
            self.element.layout.row_start = Some(start.max(1));
            self
        }

        pub fn column_span(mut self, span: usize) -> Self {
            self.element.layout.column_span = span.max(1);
            self
        }

        pub fn row_span(mut self, span: usize) -> Self {
            self.element.layout.row_span = span.max(1);
            self
        }

        pub fn position_absolute(mut self) -> Self {
            self.element.layout.position_type = crate::ui::layout::PositionType::Absolute;
            self
        }

        pub fn left(mut self, value: impl IntoLengthValue) -> Self {
            set_layout_inset(&mut self.element.layout.left, value);
            self
        }

        pub fn top(mut self, value: impl IntoLengthValue) -> Self {
            set_layout_inset(&mut self.element.layout.top, value);
            self
        }

        pub fn right(mut self, value: impl IntoLengthValue) -> Self {
            set_layout_inset(&mut self.element.layout.right, value);
            self
        }

        pub fn bottom(mut self, value: impl IntoLengthValue) -> Self {
            set_layout_inset(&mut self.element.layout.bottom, value);
            self
        }

        pub fn inset(mut self, value: impl IntoLengthValue + Copy) -> Self {
            set_layout_inset(&mut self.element.layout.left, value);
            set_layout_inset(&mut self.element.layout.top, value);
            set_layout_inset(&mut self.element.layout.right, value);
            set_layout_inset(&mut self.element.layout.bottom, value);
            self
        }
    };
}

impl<VM> Canvas<VM> {
    pub fn new(items: impl Into<Value<Vec<CanvasItem>>>) -> Self {
        Self {
            element: Element {
                id: WidgetId::next(),
                key: None,
                layout: LayoutStyle::default(),
                visual: VisualStyle::default(),
                interactions: InteractionHandlers::default(),
                lifecycle_events: LifecycleEventHandlers::default(),
                media_events: MediaEventHandlers::default(),
                background: None,
                kind: WidgetKind::Canvas {
                    items: items.into(),
                    item_interactions: CanvasItemInteractionHandlers::default(),
                    style: None,
                },
            },
        }
    }

    impl_canvas_layout_api!();

    pub fn style(
        mut self,
        resolver: impl Fn(ResolvedThemeMode) -> CanvasStyle + Send + Sync + 'static,
    ) -> Self {
        if let WidgetKind::Canvas { style, .. } = &mut self.element.kind {
            *style = Some(super::style::StyleResolver::new(resolver));
        }
        self
    }

    pub fn key(mut self, key: impl Into<super::WidgetKey>) -> Self {
        self.element.key = Some(key.into());
        self
    }

    pub fn on_click(mut self, command: crate::foundation::view_model::Command<VM>) -> Self {
        self.element.interactions.on_click = Some(command);
        self
    }

    pub fn on_double_click(mut self, command: crate::foundation::view_model::Command<VM>) -> Self {
        self.element.interactions.on_double_click = Some(command);
        self
    }

    pub fn on_mouse_enter(mut self, command: crate::foundation::view_model::Command<VM>) -> Self {
        self.element.interactions.on_mouse_enter = Some(command);
        self
    }

    pub fn on_mouse_leave(mut self, command: crate::foundation::view_model::Command<VM>) -> Self {
        self.element.interactions.on_mouse_leave = Some(command);
        self
    }

    pub fn on_mouse_move(mut self, command: ValueCommand<VM, Point>) -> Self {
        self.element.interactions.on_mouse_move = Some(command);
        self
    }

    pub fn on_mount(mut self, command: Command<VM>) -> Self {
        self.element.lifecycle_events.on_mount = Some(command);
        self
    }

    pub fn on_unmount(mut self, command: Command<VM>) -> Self {
        self.element.lifecycle_events.on_unmount = Some(command);
        self
    }

    pub fn on_update(mut self, command: Command<VM>) -> Self {
        self.element.lifecycle_events.on_update = Some(command);
        self
    }

    pub fn on_item_click(mut self, command: ValueCommand<VM, CanvasMouseEvent>) -> Self {
        if let WidgetKind::Canvas {
            item_interactions, ..
        } = &mut self.element.kind
        {
            item_interactions.on_click = Some(command);
        }
        self
    }

    pub fn on_item_double_click(mut self, command: ValueCommand<VM, CanvasMouseEvent>) -> Self {
        if let WidgetKind::Canvas {
            item_interactions, ..
        } = &mut self.element.kind
        {
            item_interactions.on_double_click = Some(command);
        }
        self
    }

    pub fn on_item_mouse_down(mut self, command: ValueCommand<VM, CanvasMouseEvent>) -> Self {
        if let WidgetKind::Canvas {
            item_interactions, ..
        } = &mut self.element.kind
        {
            item_interactions.on_mouse_down = Some(command);
        }
        self
    }

    pub fn on_item_mouse_up(mut self, command: ValueCommand<VM, CanvasMouseEvent>) -> Self {
        if let WidgetKind::Canvas {
            item_interactions, ..
        } = &mut self.element.kind
        {
            item_interactions.on_mouse_up = Some(command);
        }
        self
    }

    pub fn on_item_mouse_enter(mut self, command: ValueCommand<VM, CanvasMouseEvent>) -> Self {
        if let WidgetKind::Canvas {
            item_interactions, ..
        } = &mut self.element.kind
        {
            item_interactions.on_mouse_enter = Some(command);
        }
        self
    }

    pub fn on_item_mouse_leave(mut self, command: ValueCommand<VM, CanvasMouseEvent>) -> Self {
        if let WidgetKind::Canvas {
            item_interactions, ..
        } = &mut self.element.kind
        {
            item_interactions.on_mouse_leave = Some(command);
        }
        self
    }

    pub fn on_item_mouse_move(mut self, command: ValueCommand<VM, CanvasMouseEvent>) -> Self {
        if let WidgetKind::Canvas {
            item_interactions, ..
        } = &mut self.element.kind
        {
            item_interactions.on_mouse_move = Some(command);
        }
        self
    }

    pub fn on_item_wheel(mut self, command: ValueCommand<VM, CanvasWheelEvent>) -> Self {
        if let WidgetKind::Canvas {
            item_interactions, ..
        } = &mut self.element.kind
        {
            item_interactions.on_wheel = Some(command);
        }
        self
    }

    pub fn on_item_drag_start(mut self, command: ValueCommand<VM, CanvasDragEvent>) -> Self {
        if let WidgetKind::Canvas {
            item_interactions, ..
        } = &mut self.element.kind
        {
            item_interactions.on_drag_start = Some(command);
        }
        self
    }

    pub fn on_item_drag(mut self, command: ValueCommand<VM, CanvasDragEvent>) -> Self {
        if let WidgetKind::Canvas {
            item_interactions, ..
        } = &mut self.element.kind
        {
            item_interactions.on_drag = Some(command);
        }
        self
    }

    pub fn on_item_drag_end(mut self, command: ValueCommand<VM, CanvasDragEvent>) -> Self {
        if let WidgetKind::Canvas {
            item_interactions, ..
        } = &mut self.element.kind
        {
            item_interactions.on_drag_end = Some(command);
        }
        self
    }

    pub fn cursor(mut self, cursor: impl Into<Value<CursorStyle>>) -> Self {
        self.element.interactions.cursor_style = Some(cursor.into());
        self
    }
}

impl<VM> From<Canvas<VM>> for Element<VM> {
    fn from(value: Canvas<VM>) -> Self {
        value.element
    }
}

#[derive(Default)]
pub(crate) struct CanvasRenderOutput {
    pub textures: Vec<TexturePrimitive>,
    pub meshes: Vec<MeshPrimitive>,
    pub texts: Vec<TextPrimitive>,
}

fn path_base_bounds(path: &CanvasPath) -> Option<RectBounds> {
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

fn tessellate_path(
    path: &CanvasPath,
    origin: Point,
    opacity: f32,
    clip: CanvasClipContext,
    media: &MediaManager,
    units: UnitContext,
) -> CanvasRenderOutput {
    let lyon_path = path.path.to_lyon_path();
    let fill = path.fill.as_ref().map(Value::resolve);
    let stroke = path.stroke.clone();
    let mut output = CanvasRenderOutput::default();

    if let Some(shadow) = path.shadow.as_ref().map(Value::resolve) {
        if let Some(texture) = shadow_texture_for_path(
            path,
            &lyon_path,
            fill.as_ref(),
            stroke.as_ref(),
            shadow,
            opacity,
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
            fill_brush,
            path.fill_rule,
            opacity,
            origin,
            clip,
        ) {
            output.meshes.push(mesh);
        }
    }

    if let Some(stroke) = stroke.as_ref() {
        if let Some(mesh) = tessellate_stroke(&lyon_path, stroke, opacity, origin, clip) {
            output.meshes.push(mesh);
        }
    }

    output
}

fn tessellate_fill(
    path: &Path,
    brush: &CanvasBrush,
    fill_rule: CanvasFillRule,
    opacity: f32,
    origin: Point,
    clip: CanvasClipContext,
) -> Option<MeshPrimitive> {
    let brush_data = CanvasBrushData::from_brush(brush, opacity)?;
    let mut geometry = VertexBuffers::<[f32; 2], u32>::new();
    let mut tessellator = FillTessellator::new();
    let mut options = FillOptions::default();
    options.fill_rule = match fill_rule {
        CanvasFillRule::NonZero => lyon::path::FillRule::NonZero,
        CanvasFillRule::EvenOdd => lyon::path::FillRule::EvenOdd,
    };
    tessellator
        .tessellate_path(
            path,
            &options,
            &mut BuffersBuilder::new(&mut geometry, FillVertexCtor),
        )
        .ok()?;

    build_mesh_primitive(geometry, brush_data, origin, clip.clip_rect, clip.clip_mask)
}

fn tessellate_stroke(
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
    clip_rect: Option<super::common::Rect>,
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
struct CanvasBrushData {
    brush_meta: [f32; 4],
    gradient_data0: [f32; 4],
    gradient_data1: [f32; 4],
    stop_offsets0: [f32; 4],
    stop_offsets1: [f32; 4],
    stop_colors: [[f32; 4]; 8],
}

impl CanvasBrushData {
    fn from_brush(brush: &CanvasBrush, opacity: f32) -> Option<Self> {
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

fn dashed_path(path: &Path, stroke: &CanvasStroke) -> Option<Path> {
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
            sampler.split_range(distance..end, &mut builder);
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

fn normalize_dash_pattern(pattern: &[Dp]) -> Option<Vec<f32>> {
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

fn shadow_texture_for_path(
    path: &CanvasPath,
    lyon_path: &Path,
    fill: Option<&CanvasBrush>,
    stroke: Option<&CanvasStroke>,
    shadow: CanvasShadow,
    opacity: f32,
    origin: Point,
    clip: CanvasClipContext,
    media: &MediaManager,
    units: UnitContext,
) -> Option<TexturePrimitive> {
    let base_bounds = path_base_bounds(path)?;
    let padding = shadow_padding(shadow.blur);
    let min_x = base_bounds.min_x + shadow.offset.x.get().min(0.0) - padding;
    let min_y = base_bounds.min_y + shadow.offset.y.get().min(0.0) - padding;
    let max_x = base_bounds.max_x + shadow.offset.x.get().max(0.0) + padding;
    let max_y = base_bounds.max_y + shadow.offset.y.get().max(0.0) + padding;
    let frame = super::common::Rect::new(
        origin.x + min_x,
        origin.y + min_y,
        (max_x - min_x).max(1.0),
        (max_y - min_y).max(1.0),
    );
    let width = units.logical_to_physical(frame.width.get()).ceil().max(1.0) as u32;
    let height = units
        .logical_to_physical(frame.height.get())
        .ceil()
        .max(1.0) as u32;

    let cache_key = canvas_shadow_cache_key(path, shadow, opacity, units.scale_factor());
    let texture = media
        .canvas_shadow_texture(cache_key, width, height, || {
            rasterize_canvas_shadow(
                lyon_path,
                fill.is_some(),
                stroke,
                shadow,
                opacity,
                min_x,
                min_y,
                units.scale_factor(),
            )
        })
        .ok()??;

    Some(TexturePrimitive {
        texture,
        frame,
        corner_radius: 0.0,
        clip_rect: clip.clip_rect,
        clip_mask: clip.clip_mask,
    })
}

fn rasterize_canvas_shadow(
    path: &Path,
    has_fill: bool,
    stroke: Option<&CanvasStroke>,
    shadow: CanvasShadow,
    opacity: f32,
    min_x: f32,
    min_y: f32,
    scale_factor: f32,
) -> Result<TextureFrame, TguiError> {
    let dashed = stroke.and_then(|stroke| dashed_path(path, stroke));
    let source_path = dashed.as_ref().unwrap_or(path);
    let tiny_path = to_tiny_skia_path(source_path, min_x, min_y, scale_factor)?;

    let width = transformed_path_size(source_path, min_x, min_y, scale_factor).0;
    let height = transformed_path_size(source_path, min_x, min_y, scale_factor).1;
    let mut pixmap = tiny_skia::Pixmap::new(width, height).ok_or_else(|| {
        TguiError::Media(format!(
            "failed to allocate canvas shadow surface {}x{}",
            width, height
        ))
    })?;
    let mut paint = tiny_skia::Paint::default();
    paint.set_color_rgba8(255, 255, 255, 255);

    if has_fill {
        pixmap.as_mut().fill_path(
            &tiny_path,
            &paint,
            tiny_skia::FillRule::EvenOdd,
            tiny_skia::Transform::identity(),
            None,
        );
    }

    if let Some(stroke) = stroke {
        let mut stroke_style = tiny_skia::Stroke::default();
        stroke_style.width = stroke.width.get().max(0.0) * scale_factor;
        stroke_style.line_cap = match stroke.line_cap {
            CanvasStrokeCap::Butt => tiny_skia::LineCap::Butt,
            CanvasStrokeCap::Square => tiny_skia::LineCap::Square,
            CanvasStrokeCap::Round => tiny_skia::LineCap::Round,
        };
        stroke_style.line_join = match stroke.line_join {
            CanvasStrokeJoin::Miter => tiny_skia::LineJoin::Miter,
            CanvasStrokeJoin::Bevel => tiny_skia::LineJoin::Bevel,
            CanvasStrokeJoin::Round => tiny_skia::LineJoin::Round,
        };
        stroke_style.miter_limit = stroke.miter_limit.max(0.0);
        if let Some(pattern) = stroke
            .dash_pattern
            .as_ref()
            .and_then(|pattern| normalize_dash_pattern(pattern))
        {
            stroke_style.dash = tiny_skia::StrokeDash::new(
                pattern
                    .into_iter()
                    .map(|value| value * scale_factor)
                    .collect(),
                stroke.dash_offset.get() * scale_factor,
            );
        }
        pixmap.as_mut().stroke_path(
            &tiny_path,
            &paint,
            &stroke_style,
            tiny_skia::Transform::identity(),
            None,
        );
    }

    let blurred = DynamicImage::ImageRgba8(
        RgbaImage::from_raw(width, height, pixmap.data().to_vec()).ok_or_else(|| {
            TguiError::Media("failed to create canvas shadow image buffer".to_string())
        })?,
    )
    .fast_blur((shadow.blur.get() * scale_factor).max(0.0));

    let mut pixels = blurred.to_rgba8().into_raw();
    let shadow_color = shadow.color.with_alpha_factor(opacity);
    for pixel in pixels.chunks_exact_mut(4) {
        let alpha = pixel[3] as f32 / 255.0;
        pixel[0] = ((shadow_color.r as f32) * alpha).round().clamp(0.0, 255.0) as u8;
        pixel[1] = ((shadow_color.g as f32) * alpha).round().clamp(0.0, 255.0) as u8;
        pixel[2] = ((shadow_color.b as f32) * alpha).round().clamp(0.0, 255.0) as u8;
        pixel[3] = ((shadow_color.a as f32) * alpha).round().clamp(0.0, 255.0) as u8;
    }

    Ok(TextureFrame::new(width, height, pixels))
}

fn transformed_path_size(path: &Path, min_x: f32, min_y: f32, scale_factor: f32) -> (u32, u32) {
    let bounds = bounding_box(path.iter());
    let width = ((bounds.max.x - min_x) * scale_factor).ceil().max(1.0) as u32;
    let height = ((bounds.max.y - min_y) * scale_factor).ceil().max(1.0) as u32;
    (width, height)
}

fn to_tiny_skia_path(
    path: &Path,
    min_x: f32,
    min_y: f32,
    scale_factor: f32,
) -> Result<tiny_skia::Path, TguiError> {
    let mut builder = tiny_skia::PathBuilder::new();
    for event in path.iter() {
        match event {
            PathEvent::Begin { at } => {
                builder.move_to((at.x - min_x) * scale_factor, (at.y - min_y) * scale_factor)
            }
            PathEvent::Line { to, .. } => {
                builder.line_to((to.x - min_x) * scale_factor, (to.y - min_y) * scale_factor)
            }
            PathEvent::Quadratic { ctrl, to, .. } => builder.quad_to(
                (ctrl.x - min_x) * scale_factor,
                (ctrl.y - min_y) * scale_factor,
                (to.x - min_x) * scale_factor,
                (to.y - min_y) * scale_factor,
            ),
            PathEvent::Cubic {
                ctrl1, ctrl2, to, ..
            } => builder.cubic_to(
                (ctrl1.x - min_x) * scale_factor,
                (ctrl1.y - min_y) * scale_factor,
                (ctrl2.x - min_x) * scale_factor,
                (ctrl2.y - min_y) * scale_factor,
                (to.x - min_x) * scale_factor,
                (to.y - min_y) * scale_factor,
            ),
            PathEvent::End { close, .. } => {
                if close {
                    builder.close();
                }
            }
        }
    }

    builder.finish().ok_or_else(|| {
        TguiError::Media("failed to finish canvas shadow path rasterization".to_string())
    })
}

fn canvas_shadow_cache_key(
    path: &CanvasPath,
    shadow: CanvasShadow,
    opacity: f32,
    scale_factor: f32,
) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    for command in path.path.commands_internal() {
        match *command {
            PathCommand::MoveTo(point_value) => {
                0u8.hash(&mut hasher);
                hash_point(point_value, &mut hasher);
            }
            PathCommand::LineTo(point_value) => {
                1u8.hash(&mut hasher);
                hash_point(point_value, &mut hasher);
            }
            PathCommand::QuadTo { ctrl, to } => {
                2u8.hash(&mut hasher);
                hash_point(ctrl, &mut hasher);
                hash_point(to, &mut hasher);
            }
            PathCommand::CubicTo { ctrl1, ctrl2, to } => {
                3u8.hash(&mut hasher);
                hash_point(ctrl1, &mut hasher);
                hash_point(ctrl2, &mut hasher);
                hash_point(to, &mut hasher);
            }
            PathCommand::Close => {
                4u8.hash(&mut hasher);
            }
        }
    }
    path.fill.is_some().hash(&mut hasher);
    if let Some(stroke) = path.stroke.as_ref() {
        hash_f32(stroke.width.get(), &mut hasher);
        if let Some(pattern) = stroke.dash_pattern.as_ref() {
            pattern.len().hash(&mut hasher);
            for value in pattern {
                hash_f32(value.get(), &mut hasher);
            }
        } else {
            0usize.hash(&mut hasher);
        }
        hash_f32(stroke.dash_offset.get(), &mut hasher);
    } else {
        0u8.hash(&mut hasher);
    }
    shadow.color.hash(&mut hasher);
    hash_point(shadow.offset, &mut hasher);
    hash_f32(shadow.blur.get(), &mut hasher);
    hash_f32(opacity, &mut hasher);
    hash_f32(scale_factor, &mut hasher);
    hasher.finish()
}

fn hash_point(point_value: Point, hasher: &mut impl Hasher) {
    hash_f32(point_value.x.get(), hasher);
    hash_f32(point_value.y.get(), hasher);
}

fn hash_f32(value: f32, hasher: &mut impl Hasher) {
    value.to_bits().hash(hasher);
}

fn shadow_padding(blur: Dp) -> f32 {
    blur.get().max(0.0) * SHADOW_BLUR_PADDING_MULTIPLIER
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

fn append_arc_segments(
    mut builder: PathBuilder,
    center: Point,
    radius: f32,
    start_angle: f32,
    sweep_angle: f32,
    connect_with_line: bool,
) -> PathBuilder {
    if radius <= 0.0 || sweep_angle.abs() <= f32::EPSILON {
        return builder;
    }

    let steps = ((sweep_angle.abs() / std::f32::consts::FRAC_PI_8).ceil() as usize).max(1);
    for index in 0..=steps {
        let t = index as f32 / steps as f32;
        let angle = start_angle + sweep_angle * t;
        let point_value = Point::new(
            center.x.get() + angle.cos() * radius,
            center.y.get() + angle.sin() * radius,
        );
        if index == 0 {
            builder = if connect_with_line {
                builder.line_to(point_value.x, point_value.y)
            } else {
                builder.move_to(point_value.x, point_value.y)
            };
        } else {
            builder = builder.line_to(point_value.x, point_value.y);
        }
    }
    builder
}

fn normalize_vec(value: (f32, f32)) -> (f32, f32) {
    let length = (value.0 * value.0 + value.1 * value.1).sqrt();
    if length <= f32::EPSILON {
        (0.0, 0.0)
    } else {
        (value.0 / length, value.1 / length)
    }
}

fn svg_point(abs: bool, current: (f32, f32), x: f32, y: f32) -> (f32, f32) {
    if abs {
        (x, y)
    } else {
        (current.0 + x, current.1 + y)
    }
}

fn points_approx_equal(lhs: lyon::math::Point, rhs: lyon::math::Point) -> bool {
    (lhs.x - rhs.x).abs() <= 1e-3 && (lhs.y - rhs.y).abs() <= 1e-3
}

fn dedupe_ring_points(points: &mut Vec<lyon::math::Point>) {
    let mut deduped = Vec::with_capacity(points.len());
    for point_value in points.iter().copied() {
        if deduped
            .last()
            .map(|last| !points_approx_equal(*last, point_value))
            .unwrap_or(true)
        {
            deduped.push(point_value);
        }
    }
    if deduped.len() >= 2
        && deduped
            .first()
            .zip(deduped.last())
            .map(|(first, last)| !points_approx_equal(*first, *last))
            .unwrap_or(false)
    {
        deduped.push(deduped[0]);
    }
    *points = deduped;
}

fn append_ring(mut builder: PathBuilder, ring: &LineString<f64>) -> PathBuilder {
    let mut points = ring.points().collect::<Vec<_>>();
    if points.len() < 3 {
        return builder;
    }
    if let Some(first) = points.first().copied() {
        if points.last().map(|last| last != &first).unwrap_or(false) {
            points.push(first);
        }
    }

    let unique = points
        .iter()
        .map(|point_value| (point_value.x().to_bits(), point_value.y().to_bits()))
        .collect::<HashSet<_>>();
    if unique.len() < 3 {
        return builder;
    }

    let first = points[0];
    builder = builder.move_to(first.x() as f32, first.y() as f32);
    for point_value in points.iter().skip(1).take(points.len().saturating_sub(2)) {
        builder = builder.line_to(point_value.x() as f32, point_value.y() as f32);
    }
    builder.close()
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

#[cfg(test)]
mod tests {
    use super::{
        canvas_bounds, CanvasBooleanOp, CanvasBrush, CanvasGradientStop, CanvasItem, CanvasPath,
        CanvasShadow, CanvasStroke, PathBuilder,
    };
    use crate::foundation::color::Color;
    use crate::ui::unit::dp;
    use crate::ui::widget::Point;

    #[test]
    fn bounds_include_stroke_width() {
        let item = CanvasItem::Path(
            CanvasPath::new(
                7_u64,
                PathBuilder::new()
                    .move_to(10.0, 10.0)
                    .line_to(30.0, 10.0)
                    .line_to(30.0, 20.0)
                    .close(),
            )
            .stroke(CanvasStroke::new(dp(8.0), Color::WHITE)),
        );

        let bounds = item.hit_bounds().expect("bounds should exist");
        assert_eq!(bounds.min_x, 6.0);
        assert_eq!(bounds.max_x, 34.0);
    }

    #[test]
    fn canvas_bounds_union_all_items() {
        let items = vec![
            CanvasItem::path(
                1_u64,
                PathBuilder::new()
                    .move_to(0.0, 0.0)
                    .line_to(20.0, 0.0)
                    .line_to(20.0, 10.0)
                    .close(),
            ),
            CanvasItem::path(
                2_u64,
                PathBuilder::new()
                    .move_to(50.0, 25.0)
                    .line_to(80.0, 25.0)
                    .line_to(80.0, 40.0)
                    .close(),
            ),
        ];

        let bounds = canvas_bounds(&items).expect("bounds should exist");
        assert_eq!(bounds.width(), 80.0);
        assert_eq!(bounds.height(), 40.0);
    }

    #[test]
    fn canvas_bounds_include_shadow_expansion() {
        let item = CanvasItem::Path(
            CanvasPath::new(
                1_u64,
                PathBuilder::new()
                    .move_to(0.0, 0.0)
                    .line_to(20.0, 0.0)
                    .line_to(20.0, 20.0)
                    .close(),
            )
            .shadow(CanvasShadow::new(
                Color::BLACK,
                crate::ui::widget::Point::new(4.0, 6.0),
                dp(5.0),
            )),
        );

        let bounds = item.layout_bounds().expect("layout bounds should exist");
        assert!(bounds.max_x > 20.0);
        assert!(bounds.max_y > 20.0);
    }

    #[test]
    fn boolean_union_combines_rectangles() {
        let lhs = PathBuilder::new()
            .move_to(0.0, 0.0)
            .line_to(20.0, 0.0)
            .line_to(20.0, 20.0)
            .line_to(0.0, 20.0)
            .close();
        let rhs = PathBuilder::new()
            .move_to(10.0, 0.0)
            .line_to(30.0, 0.0)
            .line_to(30.0, 20.0)
            .line_to(10.0, 20.0)
            .close();

        let union = lhs
            .boolean(CanvasBooleanOp::Union, &rhs)
            .expect("boolean union should succeed");
        let union_bounds = union.control_bounds().expect("union bounds");
        assert_eq!(union_bounds.min.x, 0.0);
        assert_eq!(union_bounds.max.x, 30.0);
    }

    #[test]
    fn boolean_difference_rejects_open_subpaths() {
        let lhs = PathBuilder::new().move_to(0.0, 0.0).line_to(10.0, 0.0);
        let rhs = PathBuilder::new()
            .move_to(0.0, 0.0)
            .line_to(10.0, 0.0)
            .line_to(10.0, 10.0)
            .close();

        assert!(lhs.difference(&rhs).is_err());
    }

    #[test]
    fn gradients_with_many_stops_are_compressed_for_rendering() {
        let stops = (0..9)
            .map(|index| CanvasGradientStop::new(index as f32 / 8.0, Color::WHITE))
            .collect::<Vec<_>>();
        let gradient = CanvasBrush::LinearGradient(super::CanvasLinearGradient::new(
            crate::ui::widget::Point::new(0.0, 0.0),
            crate::ui::widget::Point::new(10.0, 0.0),
            stops,
        ));

        assert!(super::CanvasBrushData::from_brush(&gradient, 1.0).is_some());
    }

    #[test]
    fn fill_and_stroke_tessellate_separately() {
        let path = PathBuilder::new()
            .move_to(0.0, 0.0)
            .line_to(40.0, 0.0)
            .line_to(40.0, 30.0)
            .line_to(0.0, 30.0)
            .close()
            .to_lyon_path();

        assert!(super::tessellate_fill(
            &path,
            &CanvasBrush::Solid(Color::WHITE),
            super::CanvasFillRule::NonZero,
            1.0,
            Point::ZERO,
            super::CanvasClipContext::default()
        )
        .is_some());
        assert!(super::tessellate_stroke(
            &path,
            &CanvasStroke::new(dp(4.0), Color::WHITE),
            1.0,
            Point::ZERO,
            super::CanvasClipContext::default()
        )
        .is_some());
    }
}
