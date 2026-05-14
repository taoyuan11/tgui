use std::collections::HashSet;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use geo::{BooleanOps, Contains, Coord, LineString, MultiPolygon, Polygon};
use lyon::geom::{Angle, ArcFlags, SvgArc};
use image::{DynamicImage, RgbaImage};
use lyon::algorithms::aabb::bounding_box;
use lyon::algorithms::measure::{PathMeasurements, SampleType};
use lyon::math::{point, vector};
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
    resolve_media_rect, ContentFit, IntrinsicSize, MediaManager, MediaSource, RasterRequest,
    TextureFrame,
};
use crate::text::font::FontWeight;
use crate::theme::ResolvedThemeMode;
use crate::ui::layout::{Align, Insets, LayoutStyle, Value};
use crate::ui::unit::{Dp, Sp, UnitContext};

use super::background::{
    BackgroundBrush, BackgroundGradientStop, BackgroundLinearGradient, BackgroundRadialGradient,
};
use super::common::{
    CanvasCompositePrimitive, CanvasItemInteractionHandlers, ClipMask, CursorStyle,
    InteractionHandlers, LifecycleEventHandlers, MediaEventHandlers, MeshPrimitive, MeshVertex,
    Point, Rect, RenderCommand, TextPrimitive, TexturePrimitive, VisualStyle, WidgetId, WidgetKind,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum CanvasFillRule {
    #[default]
    NonZero,
    EvenOdd,
}

impl CanvasFillRule {
    fn to_lyon(self) -> lyon::path::FillRule {
        match self {
            Self::NonZero => lyon::path::FillRule::NonZero,
            Self::EvenOdd => lyon::path::FillRule::EvenOdd,
        }
    }

    fn to_tiny_skia(self) -> tiny_skia::FillRule {
        match self {
            Self::NonZero => tiny_skia::FillRule::Winding,
            Self::EvenOdd => tiny_skia::FillRule::EvenOdd,
        }
    }
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
struct CanvasItemStyle {
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CanvasPathOpError {
    OpenSubpath,
}

impl std::fmt::Display for CanvasPathOpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::OpenSubpath => write!(f, "canvas path operations require closed subpaths"),
        }
    }
}

impl std::error::Error for CanvasPathOpError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CanvasSvgPathError(pub String);

#[derive(Clone, Debug, PartialEq)]
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

#[derive(Clone, Copy, Debug, PartialEq)]
enum PathShapeHint {
    RoundedRect { rect: Rect, radius: Dp },
}

#[derive(Clone, Debug, PartialEq)]
pub struct PathBuilder {
    commands: Vec<PathCommand>,
    fill_rule: CanvasFillRule,
    shape_hint: Option<PathShapeHint>,
}

impl PathBuilder {
    pub fn new() -> Self {
        Self {
            commands: Vec::new(),
            fill_rule: CanvasFillRule::NonZero,
            shape_hint: None,
        }
    }

    pub fn fill_rule(mut self, fill_rule: CanvasFillRule) -> Self {
        self.fill_rule = fill_rule;
        self
    }

    pub fn even_odd(self) -> Self {
        self.fill_rule(CanvasFillRule::EvenOdd)
    }

    pub fn non_zero(self) -> Self {
        self.fill_rule(CanvasFillRule::NonZero)
    }

    pub fn move_to(mut self, x: impl Into<Dp>, y: impl Into<Dp>) -> Self {
        self.shape_hint = None;
        self.commands.push(PathCommand::MoveTo(Point::new(x, y)));
        self
    }

    pub fn line_to(mut self, x: impl Into<Dp>, y: impl Into<Dp>) -> Self {
        self.shape_hint = None;
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
        self.shape_hint = None;
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
        self.shape_hint = None;
        self.commands.push(PathCommand::CubicTo {
            ctrl1: Point::new(ctrl1_x, ctrl1_y),
            ctrl2: Point::new(ctrl2_x, ctrl2_y),
            to: Point::new(x, y),
        });
        self
    }

    pub fn close(mut self) -> Self {
        self.shape_hint = None;
        self.commands.push(PathCommand::Close);
        self
    }

    pub fn rect(
        mut self,
        x: impl Into<Dp>,
        y: impl Into<Dp>,
        width: impl Into<Dp>,
        height: impl Into<Dp>,
    ) -> Self {
        let started_empty = self.commands.is_empty();
        let x = x.into();
        let y = y.into();
        let width = width.into().max(Dp::ZERO);
        let height = height.into().max(Dp::ZERO);
        self = self
            .move_to(x, y)
            .line_to(x + width, y)
            .line_to(x + width, y + height)
            .line_to(x, y + height)
            .close();
        if started_empty {
            self.shape_hint = Some(PathShapeHint::RoundedRect {
                rect: Rect::new(x, y, width, height),
                radius: Dp::ZERO,
            });
        }
        self
    }

    pub fn rounded_rect(
        self,
        x: impl Into<Dp>,
        y: impl Into<Dp>,
        width: impl Into<Dp>,
        height: impl Into<Dp>,
        radius: impl Into<Dp>,
    ) -> Self {
        let started_empty = self.commands.is_empty();
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
        builder = builder.close();
        if started_empty {
            builder.shape_hint = Some(PathShapeHint::RoundedRect {
                rect: Rect::new(x, y, width, height),
                radius,
            });
        }
        builder
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
        let mut builder = self;
        builder.shape_hint = None;
        append_arc_segments(
            builder,
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
        let mut this = self;
        this.shape_hint = None;
        let Some(PathCommand::MoveTo(from) | PathCommand::LineTo(from)) =
            this.commands.last().copied()
        else {
            return this.move_to(ctrl_x, ctrl_y).line_to(x, y);
        };

        let ctrl = Point::new(ctrl_x, ctrl_y);
        let to = Point::new(x, y);
        let radius = radius.into().max(Dp::ZERO).get();
        if radius <= 0.0 {
            return this.line_to(ctrl.x, ctrl.y).line_to(to.x, to.y);
        }

        let p0 = (from.x.get(), from.y.get());
        let p1 = (ctrl.x.get(), ctrl.y.get());
        let p2 = (to.x.get(), to.y.get());
        let v1 = normalize_vec((p0.0 - p1.0, p0.1 - p1.1));
        let v2 = normalize_vec((p2.0 - p1.0, p2.1 - p1.1));
        let dot = (v1.0 * v2.0 + v1.1 * v2.1).clamp(-1.0, 1.0);
        let angle = dot.acos();
        if angle <= 1e-3 || (std::f32::consts::PI - angle).abs() <= 1e-3 {
            return this.line_to(ctrl.x, ctrl.y).line_to(to.x, to.y);
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
            this.line_to(start.0, start.1),
            Point::new(center.0, center.1),
            radius,
            start_angle,
            sweep,
            true,
        )
    }

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

    fn commands_internal(&self) -> &[PathCommand] {
        &self.commands
    }

    fn shape_hint(&self) -> Option<PathShapeHint> {
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
        (path.iter().next().is_some()).then(|| bounding_box(path.iter()))
    }

    fn to_multi_polygon_with_rule(
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

#[derive(Clone, Debug, PartialEq)]
struct CanvasPath {
    pub style: CanvasItemStyle,
    pub path: PathBuilder,
    pub fill_rule: CanvasFillRule,
    pub fill: Option<Value<CanvasBrush>>,
    pub stroke: Option<CanvasStroke>,
    pub shadow: Option<Value<CanvasShadow>>,
}

impl CanvasPath {
    pub fn new(id: impl Into<CanvasItemId>, path: PathBuilder) -> Self {
        Self {
            style: CanvasItemStyle::new(id),
            path,
            fill_rule: CanvasFillRule::NonZero,
            fill: None,
            stroke: None,
            shadow: None,
        }
    }

    pub fn fill_rule(mut self, fill_rule: CanvasFillRule) -> Self {
        self.fill_rule = fill_rule;
        self
    }

    pub fn fill(mut self, brush: impl Into<Value<CanvasBrush>>) -> Self {
        self.fill = Some(brush.into());
        self
    }

    pub fn stroke(mut self, stroke: CanvasStroke) -> Self {
        self.stroke = Some(stroke);
        self
    }

    pub fn shadow(mut self, shadow: impl Into<Value<CanvasShadow>>) -> Self {
        self.shadow = Some(shadow.into());
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
    Ellipsis,
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

#[derive(Clone, Debug, PartialEq)]
struct CanvasText {
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

#[derive(Clone, Debug, PartialEq)]
struct CanvasImage {
    pub style: CanvasItemStyle,
    pub frame: Rect,
    pub source: MediaSource,
    pub fit: ContentFit,
    pub corner_radius: Dp,
    pub source_rect: Option<Rect>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CanvasImageOptions {
    pub fit: ContentFit,
    pub corner_radius: Dp,
    pub source_rect: Option<Rect>,
}

impl Default for CanvasImageOptions {
    fn default() -> Self {
        Self {
            fit: ContentFit::Contain,
            corner_radius: Dp::ZERO,
            source_rect: None,
        }
    }
}

impl CanvasImageOptions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn fit(mut self, fit: ContentFit) -> Self {
        self.fit = fit;
        self
    }

    pub fn corner_radius(mut self, corner_radius: impl Into<Dp>) -> Self {
        self.corner_radius = corner_radius.into();
        self
    }

    pub fn source_rect(mut self, source_rect: Rect) -> Self {
        self.source_rect = Some(source_rect);
        self
    }
}

impl CanvasImage {
    pub fn new(id: impl Into<CanvasItemId>, frame: Rect, source: impl Into<MediaSource>) -> Self {
        Self {
            style: CanvasItemStyle::new(id),
            frame,
            source: source.into(),
            fit: ContentFit::Contain,
            corner_radius: Dp::ZERO,
            source_rect: None,
        }
    }

    pub fn options(mut self, options: CanvasImageOptions) -> Self {
        self.fit = options.fit;
        self.corner_radius = options.corner_radius;
        self.source_rect = options.source_rect;
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
enum CanvasClipShape {
    Path {
        path: PathBuilder,
        fill_rule: CanvasFillRule,
    },
}

#[derive(Clone, Debug, PartialEq)]
struct CanvasClip {
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

}

#[derive(Clone, Debug, PartialEq)]
enum CanvasItem {
    Path(CanvasPath),
    Text(CanvasText),
    Image(CanvasImage),
    Clip(CanvasClip),
}

impl CanvasItem {
    pub(crate) fn id(&self) -> CanvasItemId {
        match self {
            Self::Path(path) => path.style.id,
            Self::Text(text) => text.style.id,
            Self::Image(image) => image.style.id,
            Self::Clip(clip) => clip.style.id,
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
            Self::Clip(clip) => {
                clip_shape_bounds(&clip.clip).or_else(|| canvas_bounds(&clip.items))
            }
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
            Self::Clip(clip) => {
                clip_shape_bounds(&clip.clip).or_else(|| canvas_bounds(&clip.items))
            }
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

        if item_requires_composite(self) {
            return tessellate_composite_item(self, origin, opacity, clip_context, media, units);
        }

        let mut output = match self {
            Self::Path(path) => tessellate_path(path, origin, opacity, clip_context, media, units),
            Self::Text(text) => tessellate_text(text, origin, opacity, clip_context),
            Self::Image(image) => {
                tessellate_image(image, origin, opacity, clip_context, media, units)
            }
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
        };

        apply_transform_to_output(&mut output, self.style().transform, origin);
        output
    }

    pub(crate) fn style(&self) -> &CanvasItemStyle {
        match self {
            Self::Path(path) => &path.style,
            Self::Text(text) => &text.style,
            Self::Image(image) => &image.style,
            Self::Clip(clip) => &clip.style,
        }
    }
}

fn item_requires_composite(item: &CanvasItem) -> bool {
    match item {
        CanvasItem::Path(path) => path.style.blend_mode != CanvasBlendMode::Normal,
        CanvasItem::Text(text) => text.style.blend_mode != CanvasBlendMode::Normal,
        CanvasItem::Image(image) => image.style.blend_mode != CanvasBlendMode::Normal,
        CanvasItem::Clip(_) => true,
    }
}

fn bounds_rect(bounds: RectBounds) -> Rect {
    Rect::new(bounds.min_x, bounds.min_y, bounds.width(), bounds.height())
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
        CanvasClipShape::Path { path, .. } => path.control_bounds().map(|bounds| {
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
        CanvasClipShape::Path { path, .. } => path.control_bounds().map(|bounds| {
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

fn transform_path_builder(path: &PathBuilder, transform: CanvasTransform2D) -> PathBuilder {
    if transform == CanvasTransform2D::IDENTITY {
        return path.clone();
    }

    let mut builder = PathBuilder::new().fill_rule(path.fill_rule);
    for command in path.commands_internal() {
        builder = match *command {
            PathCommand::MoveTo(point_value) => {
                let point_value = transform.apply(point_value);
                builder.move_to(point_value.x, point_value.y)
            }
            PathCommand::LineTo(point_value) => {
                let point_value = transform.apply(point_value);
                builder.line_to(point_value.x, point_value.y)
            }
            PathCommand::QuadTo { ctrl, to } => {
                let ctrl = transform.apply(ctrl);
                let to = transform.apply(to);
                builder.quad_to(ctrl.x, ctrl.y, to.x, to.y)
            }
            PathCommand::CubicTo { ctrl1, ctrl2, to } => {
                let ctrl1 = transform.apply(ctrl1);
                let ctrl2 = transform.apply(ctrl2);
                let to = transform.apply(to);
                builder.cubic_to(ctrl1.x, ctrl1.y, ctrl2.x, ctrl2.y, to.x, to.y)
            }
            PathCommand::Close => builder.close(),
        };
    }
    builder
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
        output.commands.extend(rendered.commands);
        output.meshes.extend(rendered.meshes);
        output.textures.extend(rendered.textures);
        output.texts.extend(rendered.texts);
    }
    output
}

fn output_to_commands(output: CanvasRenderOutput) -> Vec<RenderCommand> {
    let mut commands = output.commands;
    commands.extend(output.meshes.into_iter().map(RenderCommand::Mesh));
    commands.extend(output.textures.into_iter().map(RenderCommand::Texture));
    commands.extend(output.texts.into_iter().map(RenderCommand::Text));
    commands
}

fn tessellate_composite_item(
    item: &CanvasItem,
    origin: Point,
    opacity: f32,
    clip: CanvasClipContext,
    media: &MediaManager,
    units: UnitContext,
) -> CanvasRenderOutput {
    let mut output = CanvasRenderOutput::default();
    let Some(bounds) = item.layout_bounds() else {
        return output;
    };
    let bounds_rect = offset_rect(bounds_rect(bounds), origin);
    let style = item.style();

    let (content_output, mask_commands, blend_mode) = match item {
        CanvasItem::Path(path) => (
            tessellate_path(path, origin, opacity * style.opacity, clip, media, units),
            None,
            style.blend_mode,
        ),
        CanvasItem::Text(text) => (
            tessellate_text(text, origin, opacity * style.opacity, clip),
            None,
            style.blend_mode,
        ),
        CanvasItem::Image(image) => (
            tessellate_image(image, origin, opacity * style.opacity, clip, media, units),
            None,
            style.blend_mode,
        ),
        CanvasItem::Clip(clip_item) => {
            let nested_output =
                tessellate_items(&clip_item.items, origin, opacity, clip, media, units);
            let CanvasClipShape::Path { path, fill_rule } = &clip_item.clip;
            let mask = tessellate_path(
                &CanvasPath::new(clip_item.style.id, path.clone())
                    .fill_rule(*fill_rule)
                    .fill(Color::WHITE),
                origin,
                1.0,
                CanvasClipContext::default(),
                media,
                units,
            );
            let mask_commands = Some(output_to_commands(mask).into());
            (nested_output, mask_commands, CanvasBlendMode::Normal)
        }
    };

    let mut content_output = content_output;
    if style.transform != CanvasTransform2D::IDENTITY {
        apply_transform_to_output(&mut content_output, style.transform, origin);
    }

    let content_commands: Arc<[RenderCommand]> = output_to_commands(content_output).into();
    output
        .commands
        .push(RenderCommand::CanvasComposite(CanvasCompositePrimitive {
            bounds: bounds_rect,
            opacity: 1.0,
            blend_mode,
            clip_rect: clip.clip_rect,
            clip_mask: clip.clip_mask,
            content_commands,
            mask_commands,
        }));
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
            quad: None,
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
            wrap: text.paragraph_style.wrap,
            overflow: text.paragraph_style.overflow,
            horizontal_align: text.paragraph_style.horizontal_align,
            vertical_align: text.paragraph_style.vertical_align,
            clip_rect: clip.clip_rect,
            clip_mask: clip.clip_mask,
        }],
        ..Default::default()
    }
}

fn tessellate_image(
    image: &CanvasImage,
    origin: Point,
    opacity: f32,
    clip: CanvasClipContext,
    media: &MediaManager,
    units: UnitContext,
) -> CanvasRenderOutput {
    let frame = offset_rect(image.frame, origin);
    let metadata = media.image_snapshot(&image.source, None);
    let intrinsic_size = metadata.intrinsic_size;
    let source_rect = normalized_source_rect(image.source_rect, intrinsic_size);
    let source_size = source_rect
        .map(|rect| intrinsic_size_from_rect(rect))
        .unwrap_or(intrinsic_size);
    let target_frame = resolve_media_rect(frame, source_size, image.fit);
    let snapshot = if let Some(raster_request) =
        raster_request_for_image(metadata.intrinsic_size, source_rect, target_frame, units)
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
            quad: None,
            uv_rect: source_rect.and_then(|rect| source_rect_to_uv_rect(rect, intrinsic_size)),
            corner_radius: image.corner_radius.get(),
            opacity: opacity * image.style.opacity,
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
        let quad = transform_rect_quad(texture.frame, transform, origin);
        texture.quad = Some(quad);
        if let Some(rect) = quad_bounds_rect(quad) {
            texture.frame = rect;
        }
    }

    for text in &mut output.texts {
        let quad = transform_rect_quad(text.frame, transform, origin);
        text.quad = Some(quad);
        if let Some(rect) = quad_bounds_rect(quad) {
            text.frame = rect;
        }
    }
}

fn transform_rect_quad(rect: Rect, transform: CanvasTransform2D, origin: Point) -> [Point; 4] {
    let corners = [
        Point::new(rect.x - origin.x, rect.y - origin.y),
        Point::new(rect.right() - origin.x, rect.y - origin.y),
        Point::new(rect.right() - origin.x, rect.bottom() - origin.y),
        Point::new(rect.x - origin.x, rect.bottom() - origin.y),
    ];
    corners.map(|corner| {
        let transformed = transform.apply(corner);
        Point::new(origin.x + transformed.x, origin.y + transformed.y)
    })
}

fn quad_bounds_rect(quad: [Point; 4]) -> Option<Rect> {
    let mut min_x = f32::INFINITY;
    let mut min_y = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut max_y = f32::NEG_INFINITY;
    for point_value in quad {
        min_x = min_x.min(point_value.x.get());
        min_y = min_y.min(point_value.y.get());
        max_x = max_x.max(point_value.x.get());
        max_y = max_y.max(point_value.y.get());
    }
    if !min_x.is_finite() || !min_y.is_finite() || !max_x.is_finite() || !max_y.is_finite() {
        return None;
    }
    Some(Rect::new(
        min_x,
        min_y,
        (max_x - min_x).max(0.0),
        (max_y - min_y).max(0.0),
    ))
}

fn canvas_bounds(items: &[CanvasItem]) -> Option<RectBounds> {
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

#[derive(Clone, Default, PartialEq)]
pub struct CanvasScene {
    items: Vec<CanvasItem>,
}

impl CanvasScene {
    pub fn empty() -> Self {
        Self::default()
    }

    fn from_items(items: Vec<CanvasItem>) -> Self {
        Self { items }
    }

    fn items(&self) -> &[CanvasItem] {
        &self.items
    }
}

pub(crate) fn canvas_scene_bounds(scene: &CanvasScene) -> Option<RectBounds> {
    canvas_bounds(scene.items())
}

#[derive(Clone)]
struct CanvasRecorderState {
    transform: CanvasTransform2D,
    fill: Option<Value<CanvasBrush>>,
    fill_rule: CanvasFillRule,
    stroke: Option<CanvasStroke>,
    shadow: Option<Value<CanvasShadow>>,
    opacity: f32,
    blend_mode: CanvasBlendMode,
    cursor: Option<CursorStyle>,
    visible: bool,
    hit_test: bool,
    text_style: CanvasTextStyle,
    paragraph_style: CanvasParagraphStyle,
}

impl Default for CanvasRecorderState {
    fn default() -> Self {
        Self {
            transform: CanvasTransform2D::IDENTITY,
            fill: Some(Value::Static(CanvasBrush::Solid(Color::BLACK))),
            fill_rule: CanvasFillRule::NonZero,
            stroke: Some(CanvasStroke::new(Dp::new(1.0), Color::BLACK)),
            shadow: None,
            opacity: 1.0,
            blend_mode: CanvasBlendMode::Normal,
            cursor: None,
            visible: true,
            hit_test: true,
            text_style: CanvasTextStyle::default(),
            paragraph_style: CanvasParagraphStyle::default(),
        }
    }
}

#[derive(Clone)]
struct CanvasRecorderFrame {
    state: CanvasRecorderState,
    items: Vec<CanvasItem>,
    clip_path: Option<PathBuilder>,
    clipped_items: Vec<CanvasItem>,
}

impl CanvasRecorderFrame {
    fn new(state: CanvasRecorderState) -> Self {
        Self {
            state,
            items: Vec::new(),
            clip_path: None,
            clipped_items: Vec::new(),
        }
    }
}

pub struct CanvasRecorder {
    frames: Vec<CanvasRecorderFrame>,
    current_path: PathBuilder,
    next_auto_id: u64,
    pending_item_id: Option<CanvasItemId>,
}

impl Default for CanvasRecorder {
    fn default() -> Self {
        Self::new()
    }
}

impl CanvasRecorder {
    pub fn new() -> Self {
        Self::with_auto_ids(1_u64)
    }

    pub fn with_auto_ids(start: impl Into<CanvasItemId>) -> Self {
        let start = start.into().get();
        Self {
            frames: vec![CanvasRecorderFrame::new(CanvasRecorderState::default())],
            current_path: PathBuilder::new(),
            next_auto_id: start,
            pending_item_id: None,
        }
    }

    pub fn build(builder: impl FnOnce(&mut Self)) -> CanvasScene {
        let mut recorder = Self::new();
        builder(&mut recorder);
        recorder.finish()
    }

    pub fn finish(mut self) -> CanvasScene {
        while self.frames.len() > 1 {
            self.restore();
        }
        let frame = self.frames.pop().expect("recorder root frame should exist");
        CanvasScene::from_items(self.finalize_frame(frame))
    }

    pub fn save(&mut self) -> &mut Self {
        let state = self.current_state().clone();
        self.frames.push(CanvasRecorderFrame::new(state));
        self
    }

    pub fn restore(&mut self) -> &mut Self {
        if self.frames.len() <= 1 {
            return self;
        }

        let frame = self.frames.pop().expect("nested recorder frame should exist");
        let items = self.finalize_frame(frame);
        for item in items {
            self.push_item(item);
        }
        self
    }

    pub fn next_item_id(&mut self, id: impl Into<CanvasItemId>) -> &mut Self {
        self.pending_item_id = Some(id.into());
        self
    }

    pub fn begin_path(&mut self) -> &mut Self {
        self.current_path = PathBuilder::new().fill_rule(self.current_state().fill_rule);
        self
    }

    pub fn close_path(&mut self) -> &mut Self {
        self.current_path = self.current_path.clone().close();
        self
    }

    pub fn move_to(&mut self, x: impl Into<Dp>, y: impl Into<Dp>) -> &mut Self {
        self.current_path = self.current_path.clone().move_to(x, y);
        self
    }

    pub fn line_to(&mut self, x: impl Into<Dp>, y: impl Into<Dp>) -> &mut Self {
        self.current_path = self.current_path.clone().line_to(x, y);
        self
    }

    pub fn quad_to(
        &mut self,
        ctrl_x: impl Into<Dp>,
        ctrl_y: impl Into<Dp>,
        x: impl Into<Dp>,
        y: impl Into<Dp>,
    ) -> &mut Self {
        self.current_path = self.current_path.clone().quad_to(ctrl_x, ctrl_y, x, y);
        self
    }

    pub fn cubic_to(
        &mut self,
        ctrl1_x: impl Into<Dp>,
        ctrl1_y: impl Into<Dp>,
        ctrl2_x: impl Into<Dp>,
        ctrl2_y: impl Into<Dp>,
        x: impl Into<Dp>,
        y: impl Into<Dp>,
    ) -> &mut Self {
        self.current_path = self
            .current_path
            .clone()
            .cubic_to(ctrl1_x, ctrl1_y, ctrl2_x, ctrl2_y, x, y);
        self
    }

    pub fn arc(
        &mut self,
        center_x: impl Into<Dp>,
        center_y: impl Into<Dp>,
        radius: impl Into<Dp>,
        start_angle: f32,
        sweep_angle: f32,
    ) -> &mut Self {
        self.current_path = self
            .current_path
            .clone()
            .arc(center_x, center_y, radius, start_angle, sweep_angle);
        self
    }

    pub fn arc_to(
        &mut self,
        ctrl_x: impl Into<Dp>,
        ctrl_y: impl Into<Dp>,
        x: impl Into<Dp>,
        y: impl Into<Dp>,
        radius: impl Into<Dp>,
    ) -> &mut Self {
        self.current_path = self
            .current_path
            .clone()
            .arc_to(ctrl_x, ctrl_y, x, y, radius);
        self
    }

    pub fn rect(
        &mut self,
        x: impl Into<Dp>,
        y: impl Into<Dp>,
        width: impl Into<Dp>,
        height: impl Into<Dp>,
    ) -> &mut Self {
        self.current_path = self.current_path.clone().rect(x, y, width, height);
        self
    }

    pub fn rounded_rect(
        &mut self,
        x: impl Into<Dp>,
        y: impl Into<Dp>,
        width: impl Into<Dp>,
        height: impl Into<Dp>,
        radius: impl Into<Dp>,
    ) -> &mut Self {
        self.current_path = self
            .current_path
            .clone()
            .rounded_rect(x, y, width, height, radius);
        self
    }

    pub fn circle(
        &mut self,
        center_x: impl Into<Dp>,
        center_y: impl Into<Dp>,
        radius: impl Into<Dp>,
    ) -> &mut Self {
        self.current_path = self.current_path.clone().circle(center_x, center_y, radius);
        self
    }

    pub fn ellipse(
        &mut self,
        center_x: impl Into<Dp>,
        center_y: impl Into<Dp>,
        radius_x: impl Into<Dp>,
        radius_y: impl Into<Dp>,
    ) -> &mut Self {
        self.current_path = self
            .current_path
            .clone()
            .ellipse(center_x, center_y, radius_x, radius_y);
        self
    }

    pub fn svg_path(&mut self, data: &str) -> Result<&mut Self, CanvasSvgPathError> {
        self.current_path = self.current_path.clone().svg_path(data)?;
        Ok(self)
    }

    pub fn draw_path(&mut self, path: impl Into<PathBuilder>) -> &mut Self {
        self.draw_path_internal(path.into())
    }

    pub fn fill(&mut self) -> &mut Self {
        let mut item = CanvasPath::new(self.take_item_id(), self.transformed_current_path())
            .fill_rule(self.current_state().fill_rule);
        if let Some(fill) = self.current_state().fill.clone() {
            item = item.fill(fill);
        }
        self.push_item(self.apply_path_state(item, true));
        self
    }

    pub fn stroke(&mut self) -> &mut Self {
        let mut item = CanvasPath::new(self.take_item_id(), self.transformed_current_path())
            .fill_rule(self.current_state().fill_rule);
        if let Some(stroke) = self.current_state().stroke.clone() {
            item = item.stroke(stroke);
        }
        self.push_item(self.apply_path_state(item, false));
        self
    }

    pub fn fill_and_stroke(&mut self) -> &mut Self {
        let mut item = CanvasPath::new(self.take_item_id(), self.transformed_current_path())
            .fill_rule(self.current_state().fill_rule);
        if let Some(fill) = self.current_state().fill.clone() {
            item = item.fill(fill);
        }
        if let Some(stroke) = self.current_state().stroke.clone() {
            item = item.stroke(stroke);
        }
        self.push_item(self.apply_path_state(item, true));
        self
    }

    pub fn clip(&mut self) -> &mut Self {
        let path = self.transformed_current_path();
        if path.commands_internal().is_empty() {
            return self;
        }

        if self.current_frame().clip_path.is_some() && !self.current_frame().clipped_items.is_empty() {
            let clip_item = self.take_pending_clip_group();
            self.current_frame().items.push(clip_item);
        }

        let new_clip = match self.current_frame().clip_path.clone() {
            Some(existing) => existing.intersect(&path).unwrap_or(path.clone()),
            None => path,
        };
        self.current_frame().clip_path = Some(new_clip);
        self
    }

    pub fn translate(&mut self, x: impl Into<Dp>, y: impl Into<Dp>) -> &mut Self {
        self.current_state_mut().transform = self
            .current_state()
            .transform
            .then(CanvasTransform2D::translate(x, y));
        self
    }

    pub fn scale(&mut self, x: f32, y: f32) -> &mut Self {
        self.current_state_mut().transform = self
            .current_state()
            .transform
            .then(CanvasTransform2D::scale(x, y));
        self
    }

    pub fn rotate(&mut self, radians: f32) -> &mut Self {
        self.current_state_mut().transform = self
            .current_state()
            .transform
            .then(CanvasTransform2D::rotate(radians));
        self
    }

    pub fn transform(&mut self, transform: CanvasTransform2D) -> &mut Self {
        self.current_state_mut().transform = self.current_state().transform.then(transform);
        self
    }

    pub fn set_fill(&mut self, fill: impl Into<Value<CanvasBrush>>) -> &mut Self {
        self.current_state_mut().fill = Some(fill.into());
        self
    }

    pub fn set_fill_rule(&mut self, fill_rule: CanvasFillRule) -> &mut Self {
        self.current_state_mut().fill_rule = fill_rule;
        self.current_path = self.current_path.clone().fill_rule(fill_rule);
        self
    }

    pub fn clear_fill(&mut self) -> &mut Self {
        self.current_state_mut().fill = None;
        self
    }

    pub fn set_stroke(&mut self, stroke: CanvasStroke) -> &mut Self {
        self.current_state_mut().stroke = Some(stroke);
        self
    }

    pub fn clear_stroke(&mut self) -> &mut Self {
        self.current_state_mut().stroke = None;
        self
    }

    pub fn set_shadow(&mut self, shadow: impl Into<Value<CanvasShadow>>) -> &mut Self {
        self.current_state_mut().shadow = Some(shadow.into());
        self
    }

    pub fn clear_shadow(&mut self) -> &mut Self {
        self.current_state_mut().shadow = None;
        self
    }

    pub fn set_opacity(&mut self, opacity: f32) -> &mut Self {
        self.current_state_mut().opacity = opacity.clamp(0.0, 1.0);
        self
    }

    pub fn set_blend_mode(&mut self, blend_mode: CanvasBlendMode) -> &mut Self {
        self.current_state_mut().blend_mode = blend_mode;
        self
    }

    pub fn set_cursor(&mut self, cursor: CursorStyle) -> &mut Self {
        self.current_state_mut().cursor = Some(cursor);
        self
    }

    pub fn clear_cursor(&mut self) -> &mut Self {
        self.current_state_mut().cursor = None;
        self
    }

    pub fn set_hit_test(&mut self, hit_test: bool) -> &mut Self {
        self.current_state_mut().hit_test = hit_test;
        self
    }

    pub fn set_visible(&mut self, visible: bool) -> &mut Self {
        self.current_state_mut().visible = visible;
        self
    }

    pub fn set_text_style(&mut self, text_style: CanvasTextStyle) -> &mut Self {
        self.current_state_mut().text_style = text_style;
        self
    }

    pub fn set_paragraph_style(&mut self, paragraph_style: CanvasParagraphStyle) -> &mut Self {
        self.current_state_mut().paragraph_style = paragraph_style;
        self
    }

    pub fn fill_rect(
        &mut self,
        x: impl Into<Dp>,
        y: impl Into<Dp>,
        width: impl Into<Dp>,
        height: impl Into<Dp>,
    ) -> &mut Self {
        self.draw_fill_shape(PathBuilder::new().rect(x, y, width, height))
    }

    pub fn stroke_rect(
        &mut self,
        x: impl Into<Dp>,
        y: impl Into<Dp>,
        width: impl Into<Dp>,
        height: impl Into<Dp>,
    ) -> &mut Self {
        self.draw_stroke_shape(PathBuilder::new().rect(x, y, width, height))
    }

    pub fn fill_round_rect(
        &mut self,
        x: impl Into<Dp>,
        y: impl Into<Dp>,
        width: impl Into<Dp>,
        height: impl Into<Dp>,
        radius: impl Into<Dp>,
    ) -> &mut Self {
        self.draw_fill_shape(PathBuilder::new().rounded_rect(x, y, width, height, radius))
    }

    pub fn stroke_round_rect(
        &mut self,
        x: impl Into<Dp>,
        y: impl Into<Dp>,
        width: impl Into<Dp>,
        height: impl Into<Dp>,
        radius: impl Into<Dp>,
    ) -> &mut Self {
        self.draw_stroke_shape(PathBuilder::new().rounded_rect(x, y, width, height, radius))
    }

    pub fn fill_circle(
        &mut self,
        center_x: impl Into<Dp>,
        center_y: impl Into<Dp>,
        radius: impl Into<Dp>,
    ) -> &mut Self {
        self.draw_fill_shape(PathBuilder::new().circle(center_x, center_y, radius))
    }

    pub fn stroke_circle(
        &mut self,
        center_x: impl Into<Dp>,
        center_y: impl Into<Dp>,
        radius: impl Into<Dp>,
    ) -> &mut Self {
        self.draw_stroke_shape(PathBuilder::new().circle(center_x, center_y, radius))
    }

    pub fn fill_ellipse(
        &mut self,
        center_x: impl Into<Dp>,
        center_y: impl Into<Dp>,
        radius_x: impl Into<Dp>,
        radius_y: impl Into<Dp>,
    ) -> &mut Self {
        self.draw_fill_shape(PathBuilder::new().ellipse(center_x, center_y, radius_x, radius_y))
    }

    pub fn stroke_ellipse(
        &mut self,
        center_x: impl Into<Dp>,
        center_y: impl Into<Dp>,
        radius_x: impl Into<Dp>,
        radius_y: impl Into<Dp>,
    ) -> &mut Self {
        self.draw_stroke_shape(PathBuilder::new().ellipse(center_x, center_y, radius_x, radius_y))
    }

    pub fn draw_line(
        &mut self,
        start_x: impl Into<Dp>,
        start_y: impl Into<Dp>,
        end_x: impl Into<Dp>,
        end_y: impl Into<Dp>,
    ) -> &mut Self {
        self.draw_stroke_shape(
            PathBuilder::new()
                .move_to(start_x, start_y)
                .line_to(end_x, end_y),
        )
    }

    fn draw_path_internal(&mut self, path: PathBuilder) -> &mut Self {
        let mut item = CanvasPath::new(
            self.take_item_id(),
            transform_path_builder(&path, self.current_state().transform),
        )
        .fill_rule(path.fill_rule);
        if let Some(fill) = self.current_state().fill.clone() {
            item = item.fill(fill);
        }
        if let Some(stroke) = self.current_state().stroke.clone() {
            item = item.stroke(stroke);
        }
        self.push_item(self.apply_path_state(item, true));
        self
    }

    pub fn draw_svg_path(&mut self, data: &str) -> Result<&mut Self, CanvasSvgPathError> {
        let path = PathBuilder::new()
            .fill_rule(self.current_state().fill_rule)
            .svg_path(data)?;
        Ok(self.draw_path_internal(path))
    }

    pub fn draw_text(&mut self, frame: Rect, content: impl Into<String>) -> &mut Self {
        let mut text = CanvasText::new(self.take_item_id(), frame, content)
            .text_style(self.current_state().text_style.clone())
            .paragraph_style(self.current_state().paragraph_style.clone())
            .transform(self.current_state().transform)
            .opacity(self.current_state().opacity)
            .blend_mode(self.current_state().blend_mode)
            .visible(self.current_state().visible)
            .hit_test(self.current_state().hit_test);
        if let Some(cursor) = self.current_state().cursor {
            text = text.cursor(cursor);
        }
        self.push_item(CanvasItem::Text(text));
        self
    }

    pub fn draw_image(&mut self, frame: Rect, source: impl Into<MediaSource>) -> &mut Self {
        self.draw_image_with_options(frame, source, CanvasImageOptions::default())
    }

    pub fn draw_image_with_options(
        &mut self,
        frame: Rect,
        source: impl Into<MediaSource>,
        options: CanvasImageOptions,
    ) -> &mut Self {
        let mut image = CanvasImage::new(self.take_item_id(), frame, source)
            .options(options)
            .transform(self.current_state().transform)
            .opacity(self.current_state().opacity)
            .blend_mode(self.current_state().blend_mode)
            .visible(self.current_state().visible)
            .hit_test(self.current_state().hit_test);
        if let Some(cursor) = self.current_state().cursor {
            image = image.cursor(cursor);
        }
        self.push_item(CanvasItem::Image(image));
        self
    }

    fn draw_fill_shape(&mut self, path: PathBuilder) -> &mut Self {
        let mut item = CanvasPath::new(
            self.take_item_id(),
            transform_path_builder(&path, self.current_state().transform),
        )
        .fill_rule(self.current_state().fill_rule);
        if let Some(fill) = self.current_state().fill.clone() {
            item = item.fill(fill);
        }
        self.push_item(self.apply_path_state(item, true));
        self
    }

    fn draw_stroke_shape(&mut self, path: PathBuilder) -> &mut Self {
        let mut item = CanvasPath::new(
            self.take_item_id(),
            transform_path_builder(&path, self.current_state().transform),
        )
        .fill_rule(self.current_state().fill_rule);
        if let Some(stroke) = self.current_state().stroke.clone() {
            item = item.stroke(stroke);
        }
        self.push_item(self.apply_path_state(item, false));
        self
    }

    fn transformed_current_path(&self) -> PathBuilder {
        transform_path_builder(&self.current_path, self.current_state().transform)
    }

    fn apply_path_state(&self, mut item: CanvasPath, include_shadow: bool) -> CanvasItem {
        if include_shadow {
            if let Some(shadow) = self.current_state().shadow.clone() {
                item = item.shadow(shadow);
            }
        }
        item = item
            .opacity(self.current_state().opacity)
            .blend_mode(self.current_state().blend_mode)
            .visible(self.current_state().visible)
            .hit_test(self.current_state().hit_test);
        if let Some(cursor) = self.current_state().cursor {
            item = item.cursor(cursor);
        }
        CanvasItem::Path(item)
    }

    fn current_frame(&mut self) -> &mut CanvasRecorderFrame {
        self.frames
            .last_mut()
            .expect("recorder should always have an active frame")
    }

    fn current_state(&self) -> &CanvasRecorderState {
        &self
            .frames
            .last()
            .expect("recorder should always have an active frame")
            .state
    }

    fn current_state_mut(&mut self) -> &mut CanvasRecorderState {
        &mut self.current_frame().state
    }

    fn take_item_id(&mut self) -> CanvasItemId {
        self.pending_item_id.take().unwrap_or_else(|| self.take_generated_id())
    }

    fn take_generated_id(&mut self) -> CanvasItemId {
        let id = CanvasItemId::new(self.next_auto_id);
        self.next_auto_id = self.next_auto_id.saturating_add(1);
        id
    }

    fn push_item(&mut self, item: CanvasItem) {
        let frame = self.current_frame();
        if frame.clip_path.is_some() {
            frame.clipped_items.push(item);
        } else {
            frame.items.push(item);
        }
    }

    fn finalize_frame(&mut self, mut frame: CanvasRecorderFrame) -> Vec<CanvasItem> {
        if frame.clip_path.is_some() && !frame.clipped_items.is_empty() {
            let clip_id = self.take_generated_id();
            let clip_path = frame.clip_path.take().expect("clip path should exist");
            let fill_rule = clip_path.fill_rule;
            frame.items.push(CanvasItem::Clip(CanvasClip::new(
                clip_id,
                CanvasClipShape::Path {
                    path: clip_path,
                    fill_rule,
                },
                frame.clipped_items,
            )));
        }
        frame.items
    }

    fn take_pending_clip_group(&mut self) -> CanvasItem {
        let frame = self.current_frame();
        let clip_path = frame
            .clip_path
            .clone()
            .expect("clip path should exist before finalizing a clip group");
        let clipped_items = std::mem::take(&mut frame.clipped_items);
        let fill_rule = clip_path.fill_rule;
        CanvasItem::Clip(CanvasClip::new(
            self.take_generated_id(),
            CanvasClipShape::Path {
                path: clip_path,
                fill_rule,
            },
            clipped_items,
        ))
    }
}

#[doc(hidden)]
pub trait IntoCanvasContent {
    fn into_canvas_scene(self) -> Value<CanvasScene>;
}

impl IntoCanvasContent for CanvasScene {
    fn into_canvas_scene(self) -> Value<CanvasScene> {
        Value::Static(self)
    }
}

impl IntoCanvasContent for Value<CanvasScene> {
    fn into_canvas_scene(self) -> Value<CanvasScene> {
        self
    }
}

impl IntoCanvasContent for Signal<CanvasScene> {
    fn into_canvas_scene(self) -> Value<CanvasScene> {
        Value::Signal(self)
    }
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
    pub fn new(scene: impl IntoCanvasContent) -> Self {
        Self::from_scene(scene.into_canvas_scene())
    }

    pub(crate) fn from_scene(scene: impl Into<Value<CanvasScene>>) -> Self {
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
                    scene: scene.into(),
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
    pub commands: Vec<RenderCommand>,
}

pub(crate) struct CanvasSceneItemRender {
    pub item_id: CanvasItemId,
    pub cursor: Option<CursorStyle>,
    pub hit_bounds: Option<RectBounds>,
    pub output: CanvasRenderOutput,
}

pub(crate) fn tessellate_canvas_scene_items(
    scene: &CanvasScene,
    origin: Point,
    opacity: f32,
    clip_rect: Option<Rect>,
    clip_mask: Option<ClipMask>,
    media: &MediaManager,
    units: UnitContext,
) -> Vec<CanvasSceneItemRender> {
    let clip = CanvasClipContext {
        clip_rect,
        clip_mask,
    };
    scene
        .items()
        .iter()
        .map(|item| CanvasSceneItemRender {
            item_id: item.id(),
            cursor: item.style().cursor,
            hit_bounds: item.hit_bounds(),
            output: item.tessellate(origin, opacity, clip, media, units),
        })
        .collect()
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

fn tessellate_axis_aligned_rounded_rect(
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
        push_rounded_rect_stroke_command(
            &mut output,
            frame,
            corner_radius,
            stroke,
            opacity,
            clip,
        )?;
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
            output.commands.push(RenderCommand::Shape(
                super::common::RenderPrimitive {
                    rect: frame,
                    color: color.with_alpha_factor(opacity),
                    corner_radius,
                    stroke_width: 0.0,
                    clip_rect: clip.clip_rect,
                    clip_mask: clip.clip_mask,
                },
            ));
        }
        _ => {
            output.commands.push(RenderCommand::Brush(
                super::common::BrushPrimitive {
                    rect: frame,
                    brush: background_brush_from_canvas(brush, opacity)?,
                    corner_radius,
                    clip_rect: clip.clip_rect,
                    clip_mask: clip.clip_mask,
                },
            ));
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
            output.commands.push(RenderCommand::Shape(
                super::common::RenderPrimitive {
                    rect,
                    color: color.with_alpha_factor(opacity),
                    corner_radius: radius,
                    stroke_width,
                    clip_rect: clip.clip_rect,
                    clip_mask: clip.clip_mask,
                },
            ));
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

fn tessellate_fill(
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
                path.fill_rule,
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
        quad: None,
        uv_rect: None,
        corner_radius: 0.0,
        opacity: 1.0,
        clip_rect: clip.clip_rect,
        clip_mask: clip.clip_mask,
    })
}

fn rasterize_canvas_shadow(
    path: &Path,
    has_fill: bool,
    stroke: Option<&CanvasStroke>,
    fill_rule: CanvasFillRule,
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
            fill_rule.to_tiny_skia(),
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
    path.fill_rule.hash(&mut hasher);
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

fn append_svg_arc_segments(
    mut builder: PathBuilder,
    from: (f32, f32),
    to: (f32, f32),
    radius_x: f32,
    radius_y: f32,
    x_axis_rotation_degrees: f32,
    large_arc: bool,
    sweep: bool,
) -> PathBuilder {
    if (from.0 - to.0).abs() <= f32::EPSILON && (from.1 - to.1).abs() <= f32::EPSILON {
        return builder;
    }

    if radius_x.abs() <= f32::EPSILON || radius_y.abs() <= f32::EPSILON {
        return builder.line_to(to.0, to.1);
    }

    let arc = SvgArc {
        from: point(from.0, from.1),
        to: point(to.0, to.1),
        radii: vector(radius_x.abs(), radius_y.abs()),
        x_rotation: Angle::degrees(x_axis_rotation_degrees),
        flags: ArcFlags { large_arc, sweep },
    };

    let mut segments = Vec::new();
    arc.for_each_cubic_bezier(&mut |segment| {
        segments.push((
            segment.ctrl1.x,
            segment.ctrl1.y,
            segment.ctrl2.x,
            segment.ctrl2.y,
            segment.to.x,
            segment.to.y,
        ));
    });
    for (ctrl1_x, ctrl1_y, ctrl2_x, ctrl2_y, to_x, to_y) in segments {
        builder = builder.cubic_to(
            ctrl1_x, ctrl1_y, ctrl2_x, ctrl2_y, to_x, to_y,
        );
    }

    builder
}

fn normalized_source_rect(source_rect: Option<Rect>, intrinsic_size: IntrinsicSize) -> Option<Rect> {
    let mut rect = source_rect?;
    if intrinsic_size.width <= 0.0 || intrinsic_size.height <= 0.0 {
        return None;
    }

    let min_x = rect.x.get().clamp(0.0, intrinsic_size.width);
    let min_y = rect.y.get().clamp(0.0, intrinsic_size.height);
    let max_x = (rect.x + rect.width).get().clamp(min_x, intrinsic_size.width);
    let max_y = (rect.y + rect.height).get().clamp(min_y, intrinsic_size.height);
    rect.x = Dp::new(min_x);
    rect.y = Dp::new(min_y);
    rect.width = Dp::new((max_x - min_x).max(0.0));
    rect.height = Dp::new((max_y - min_y).max(0.0));
    (!rect.is_empty()).then_some(rect)
}

fn intrinsic_size_from_rect(rect: Rect) -> IntrinsicSize {
    IntrinsicSize {
        width: rect.width.get().max(0.0),
        height: rect.height.get().max(0.0),
    }
}

fn source_rect_to_uv_rect(source_rect: Rect, intrinsic_size: IntrinsicSize) -> Option<Rect> {
    if intrinsic_size.width <= 0.0 || intrinsic_size.height <= 0.0 {
        return None;
    }

    Some(Rect::new(
        source_rect.x.get() / intrinsic_size.width,
        source_rect.y.get() / intrinsic_size.height,
        source_rect.width.get() / intrinsic_size.width,
        source_rect.height.get() / intrinsic_size.height,
    ))
}

fn raster_request_for_image(
    intrinsic_size: IntrinsicSize,
    source_rect: Option<Rect>,
    target_frame: Rect,
    units: UnitContext,
) -> Option<RasterRequest> {
    let mut request = RasterRequest::from_frame(target_frame, units.scale_factor())?;
    if let Some(source_rect) = source_rect {
        if source_rect.width > 0.0 && source_rect.height > 0.0 {
            let width_ratio = intrinsic_size.width / source_rect.width.get().max(f32::EPSILON);
            let height_ratio = intrinsic_size.height / source_rect.height.get().max(f32::EPSILON);
            let width = (request.width() as f32 * width_ratio).ceil().max(1.0) as u32;
            let height = (request.height() as f32 * height_ratio).ceil().max(1.0) as u32;
            request = RasterRequest::new_clamped(width, height);
        }
    }
    Some(request)
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

#[derive(Clone, Copy)]
enum CanvasPathBooleanOperation {
    Union,
    Intersection,
    Difference,
    Xor,
}

struct PolygonizedRing {
    line: LineString<f64>,
    polygon: Polygon<f64>,
    abs_area: f64,
    orientation: i32,
    parent: Option<usize>,
    winding: i32,
    inside_filled: bool,
    active_shell: Option<usize>,
}

fn rings_to_multi_polygon(
    rings: Vec<Vec<lyon::math::Point>>,
    fill_rule: CanvasFillRule,
) -> MultiPolygon<f64> {
    let mut ring_infos = rings
        .into_iter()
        .filter_map(|ring| {
            let coords = ring
                .into_iter()
                .map(|point_value| Coord {
                    x: point_value.x as f64,
                    y: point_value.y as f64,
                })
                .collect::<Vec<_>>();
            if coords.len() < 4 {
                return None;
            }

            let line = LineString::from(coords);
            let area = signed_ring_area(&line);
            Some(PolygonizedRing {
                polygon: Polygon::new(line.clone(), Vec::new()),
                line,
                abs_area: area.abs(),
                orientation: if area >= 0.0 { 1 } else { -1 },
                parent: None,
                winding: 0,
                inside_filled: false,
                active_shell: None,
            })
        })
        .collect::<Vec<_>>();

    for index in 0..ring_infos.len() {
        let point_value = ring_infos[index]
            .line
            .0
            .first()
            .copied()
            .unwrap_or(Coord { x: 0.0, y: 0.0 });
        let test_point = geo::Point::new(point_value.x, point_value.y);
        ring_infos[index].parent = (0..ring_infos.len())
            .filter(|candidate| *candidate != index && ring_infos[*candidate].abs_area > ring_infos[index].abs_area)
            .filter(|candidate| ring_infos[*candidate].polygon.contains(&test_point))
            .min_by(|left, right| {
                ring_infos[*left]
                    .abs_area
                    .total_cmp(&ring_infos[*right].abs_area)
            });
    }

    let mut order = (0..ring_infos.len()).collect::<Vec<_>>();
    order.sort_by(|left, right| {
        ring_infos[*right]
            .abs_area
            .total_cmp(&ring_infos[*left].abs_area)
    });

    let mut polygons = Vec::<(LineString<f64>, Vec<LineString<f64>>)>::new();
    for index in order {
        let parent = ring_infos[index].parent;
        let parent_filled = parent.map(|value| ring_infos[value].inside_filled).unwrap_or(false);
        let parent_winding = parent.map(|value| ring_infos[value].winding).unwrap_or(0);
        let active_shell_outside = parent.and_then(|value| ring_infos[value].active_shell);
        let current_winding = match fill_rule {
            CanvasFillRule::NonZero => parent_winding + ring_infos[index].orientation,
            CanvasFillRule::EvenOdd => parent_winding + 1,
        };
        let current_filled = match fill_rule {
            CanvasFillRule::NonZero => current_winding != 0,
            CanvasFillRule::EvenOdd => !parent_filled,
        };

        ring_infos[index].winding = current_winding;
        ring_infos[index].inside_filled = current_filled;
        ring_infos[index].active_shell = match (parent_filled, current_filled) {
            (false, true) => {
                polygons.push((oriented_ring(&ring_infos[index].line, true), Vec::new()));
                Some(polygons.len() - 1)
            }
            (true, false) => {
                if let Some(shell_index) = active_shell_outside {
                    polygons[shell_index]
                        .1
                        .push(oriented_ring(&ring_infos[index].line, false));
                }
                None
            }
            (_, true) => active_shell_outside,
            (_, false) => None,
        };
    }

    MultiPolygon(
        polygons
            .into_iter()
            .map(|(shell, holes)| Polygon::new(shell, holes))
            .collect(),
    )
}

fn signed_ring_area(ring: &LineString<f64>) -> f64 {
    let coords = &ring.0;
    if coords.len() < 3 {
        return 0.0;
    }

    let mut area = 0.0;
    for window in coords.windows(2) {
        area += window[0].x * window[1].y - window[1].x * window[0].y;
    }
    area * 0.5
}

fn oriented_ring(ring: &LineString<f64>, counter_clockwise: bool) -> LineString<f64> {
    let area = signed_ring_area(ring);
    let is_counter_clockwise = area >= 0.0;
    if is_counter_clockwise == counter_clockwise {
        return ring.clone();
    }

    let mut coords = ring.0.clone();
    coords.reverse();
    LineString::from(coords)
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
        canvas_scene_bounds, normalized_source_rect, source_rect_to_uv_rect,
        tessellate_axis_aligned_rounded_rect, tessellate_canvas_scene_items, CanvasBrush,
        CanvasFillRule, CanvasGradientStop, CanvasImageOptions, CanvasRecorder, CanvasScene,
        CanvasShadow, CanvasStroke, CanvasTextOverflow, PathBuilder, PathCommand,
    };
    use crate::foundation::binding::InvalidationSignal;
    use crate::foundation::color::Color;
    use crate::media::{ContentFit, IntrinsicSize, MediaManager, MediaSource};
    use crate::ui::layout::Value;
    use crate::ui::unit::dp;
    use crate::ui::unit::UnitContext;
    use crate::ui::widget::{Point, Rect, RenderCommand};

    fn test_media() -> MediaManager {
        MediaManager::new(InvalidationSignal::new())
    }

    fn rendered_items(scene: &CanvasScene) -> Vec<super::CanvasSceneItemRender> {
        tessellate_canvas_scene_items(
            scene,
            Point::ZERO,
            1.0,
            None,
            None,
            &test_media(),
            UnitContext::default(),
        )
    }

    #[test]
    fn bounds_include_stroke_width() {
        let scene = CanvasRecorder::build(|canvas| {
            canvas
                .next_item_id(7_u64)
                .set_stroke(CanvasStroke::new(dp(8.0), Color::WHITE))
                .begin_path()
                .move_to(10.0, 10.0)
                .line_to(30.0, 10.0)
                .line_to(30.0, 20.0)
                .close_path()
                .stroke();
        });

        let rendered = rendered_items(&scene);
        let bounds = rendered[0].hit_bounds.expect("bounds should exist");
        assert_eq!(bounds.min_x, 6.0);
        assert_eq!(bounds.max_x, 34.0);
    }

    #[test]
    fn canvas_bounds_union_all_items() {
        let scene = CanvasRecorder::build(|canvas| {
            canvas
                .next_item_id(1_u64)
                .begin_path()
                .move_to(0.0, 0.0)
                .line_to(20.0, 0.0)
                .line_to(20.0, 10.0)
                .close_path()
                .fill()
                .next_item_id(2_u64)
                .begin_path()
                .move_to(50.0, 25.0)
                .line_to(80.0, 25.0)
                .line_to(80.0, 40.0)
                .close_path()
                .fill();
        });

        let bounds = canvas_scene_bounds(&scene).expect("bounds should exist");
        assert_eq!(bounds.width(), 80.0);
        assert_eq!(bounds.height(), 40.0);
    }

    #[test]
    fn canvas_bounds_include_shadow_expansion() {
        let scene = CanvasRecorder::build(|canvas| {
            canvas
                .next_item_id(1_u64)
                .set_shadow(CanvasShadow::new(
                    Color::BLACK,
                    crate::ui::widget::Point::new(4.0, 6.0),
                    dp(5.0),
                ))
                .fill_rect(0.0, 0.0, 20.0, 20.0);
        });

        let bounds = canvas_scene_bounds(&scene).expect("layout bounds should exist");
        assert!(bounds.max_x > 20.0);
        assert!(bounds.max_y > 20.0);
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
    fn rounded_rect_fill_prefers_non_mesh_fast_path() {
        let scene = CanvasRecorder::build(|canvas| {
            canvas
                .next_item_id(1_u64)
                .set_fill(Color::WHITE)
                .fill_round_rect(0.0, 0.0, 80.0, 40.0, 12.0);
        });
        let item = scene.items().first().expect("rounded rect item should exist");
        let super::CanvasItem::Path(path) = item else {
            panic!("rounded rect should record as a path");
        };
        let fill = path.fill.as_ref().map(Value::resolve);

        let output = tessellate_axis_aligned_rounded_rect(
            path,
            Point::ZERO,
            super::CanvasClipContext::default(),
            fill.as_ref(),
            None,
            1.0,
        )
        .expect("rounded rect should use fast path");

        assert!(output.meshes.is_empty());
        assert_eq!(output.commands.len(), 1);
        assert!(matches!(output.commands[0], crate::ui::widget::RenderCommand::Shape(_)));
    }

    #[test]
    fn canvas_recorder_auto_ids_are_stable() {
        let scene = CanvasRecorder::build(|canvas| {
            canvas.fill_rect(0.0, 0.0, 20.0, 20.0);
            canvas.draw_text(Rect::new(0.0, 0.0, 40.0, 20.0), "hello");
            canvas.stroke_circle(20.0, 20.0, 8.0);
        });
        let rendered = rendered_items(&scene);

        assert_eq!(rendered[0].item_id, 1_u64.into());
        assert_eq!(rendered[1].item_id, 2_u64.into());
        assert_eq!(rendered[2].item_id, 3_u64.into());
    }

    #[test]
    fn canvas_recorder_save_restore_restores_state() {
        let scene = CanvasRecorder::build(|canvas| {
            canvas.set_opacity(0.25).translate(10.0, 5.0);
            canvas.save();
            canvas.set_opacity(0.9).translate(50.0, 0.0);
            canvas.fill_rect(0.0, 0.0, 10.0, 10.0);
            canvas.restore();
            canvas.fill_rect(0.0, 0.0, 10.0, 10.0);
        });
        let rendered = rendered_items(&scene);
        let first_bounds = rendered[0].hit_bounds.expect("first bounds");
        let second_bounds = rendered[1].hit_bounds.expect("second bounds");
        assert!(first_bounds.min_x > second_bounds.min_x);
    }

    #[test]
    fn canvas_recorder_clip_scopes_items_inside_current_frame() {
        let scene = CanvasRecorder::build(|canvas| {
            canvas.save();
            canvas.rect(0.0, 0.0, 40.0, 40.0).clip();
            canvas.fill_rect(10.0, 10.0, 20.0, 20.0);
            canvas.restore();
            canvas.fill_rect(50.0, 0.0, 20.0, 20.0);
        });
        let rendered = rendered_items(&scene);

        assert!(matches!(
            rendered[0].output.commands.first(),
            Some(RenderCommand::CanvasComposite(_))
        ));
        assert_eq!(rendered.len(), 2);
    }

    #[test]
    fn canvas_recorder_shortcuts_match_manual_paths() {
        let shortcut = CanvasRecorder::build(|canvas| {
            canvas.fill_round_rect(0.0, 0.0, 80.0, 40.0, 12.0);
        });
        let manual = CanvasRecorder::build(|canvas| {
            canvas.begin_path();
            canvas.rounded_rect(0.0, 0.0, 80.0, 40.0, 12.0);
            canvas.fill();
        });

        assert_eq!(
            canvas_scene_bounds(&shortcut).expect("shortcut bounds"),
            canvas_scene_bounds(&manual).expect("manual bounds")
        );
    }

    #[test]
    fn canvas_text_overflow_ellipsis_can_be_configured() {
        let scene = CanvasRecorder::build(|canvas| {
            canvas
                .set_paragraph_style(super::CanvasParagraphStyle {
                    overflow: CanvasTextOverflow::Ellipsis,
                    ..Default::default()
                })
                .draw_text(Rect::new(0.0, 0.0, 60.0, 20.0), "hello");
        });
        let rendered = rendered_items(&scene);
        let text = rendered[0]
            .output
            .texts
            .first()
            .expect("text primitive should exist");

        assert_eq!(text.overflow, CanvasTextOverflow::Ellipsis);
    }

    #[test]
    fn svg_elliptical_arc_generates_curve_segments() {
        let path = PathBuilder::new()
            .svg_path("M 10 10 A 30 20 0 0 1 60 40")
            .expect("svg path should parse");

        assert!(path
            .commands_internal()
            .iter()
            .any(|command| matches!(command, PathCommand::CubicTo { .. })));
    }

    #[test]
    fn even_odd_boolean_conversion_preserves_hole() {
        let path = PathBuilder::new()
            .rect(0.0, 0.0, 100.0, 100.0)
            .rect(25.0, 25.0, 50.0, 50.0)
            .fill_rule(CanvasFillRule::EvenOdd);

        let polygon = path
            .to_multi_polygon_with_rule(CanvasFillRule::EvenOdd)
            .expect("closed rings should polygonize");

        assert_eq!(polygon.0.len(), 1);
        assert_eq!(polygon.0[0].interiors().len(), 1);
    }

    #[test]
    fn path_boolean_difference_returns_hollow_shape() {
        let outer = PathBuilder::new().rect(0.0, 0.0, 100.0, 100.0);
        let inner = PathBuilder::new().rect(25.0, 25.0, 50.0, 50.0);
        let diff = outer.difference(&inner).expect("difference should succeed");
        let polygon = diff
            .to_multi_polygon_with_rule(CanvasFillRule::NonZero)
            .expect("difference result should polygonize");

        assert_eq!(polygon.0.len(), 1);
        assert_eq!(polygon.0[0].interiors().len(), 1);
    }

    #[test]
    fn draw_image_with_options_records_configuration() {
        let scene = CanvasRecorder::build(|canvas| {
            canvas.draw_image_with_options(
                Rect::new(0.0, 0.0, 100.0, 60.0),
                MediaSource::bytes(vec![137, 80, 78, 71]),
                CanvasImageOptions::new()
                    .fit(ContentFit::Cover)
                    .corner_radius(dp(12.0))
                    .source_rect(Rect::new(10.0, 20.0, 30.0, 40.0)),
            );
        });

        let super::CanvasItem::Image(image) = &scene.items()[0] else {
            panic!("expected image item");
        };
        assert_eq!(image.fit, ContentFit::Cover);
        assert_eq!(image.corner_radius, dp(12.0));
        assert_eq!(image.source_rect, Some(Rect::new(10.0, 20.0, 30.0, 40.0)));
    }

    #[test]
    fn source_rect_is_normalized_and_converted_to_uv() {
        let intrinsic = IntrinsicSize {
            width: 200.0,
            height: 100.0,
        };
        let normalized =
            normalized_source_rect(Some(Rect::new(-20.0, 10.0, 80.0, 120.0)), intrinsic)
                .expect("source rect should normalize");
        let uv = source_rect_to_uv_rect(normalized, intrinsic).expect("uv rect should resolve");

        assert_eq!(normalized, Rect::new(0.0, 10.0, 60.0, 90.0));
        assert_eq!(uv, Rect::new(0.0, 0.1, 0.3, 0.9));
    }
}
