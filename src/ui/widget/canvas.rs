use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use geo::{BooleanOps, Contains, Coord, LineString, MultiPolygon, Polygon};
use image::{DynamicImage, RgbaImage};
use lyon::algorithms::aabb::bounding_box;
use lyon::algorithms::measure::{PathMeasurements, SampleType};
use lyon::geom::{Angle, ArcFlags, SvgArc};
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
use crate::text::font::{FontCatalog, FontManager, FontWeight, TextFontRequest};
use crate::theme::ResolvedThemeMode;
use crate::ui::layout::{Align, Insets, LayoutStyle, Value};
use crate::ui::unit::{Dp, Sp, UnitContext};
use unicode_segmentation::UnicodeSegmentation;

use super::background::{
    BackgroundBrush, BackgroundGradientStop, BackgroundLinearGradient, BackgroundRadialGradient,
};
use super::common::{
    CanvasCompositePrimitive, CanvasItemInteractionHandlers, CanvasTextSpanPrimitive, ClipMask,
    CursorStyle, InteractionHandlers, LifecycleEventHandlers, MediaEventHandlers, MeshPrimitive,
    MeshVertex, Point, Rect, RenderCommand, TextPrimitive, TexturePrimitive, VisualStyle, WidgetId,
    WidgetKind,
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
    pub text_hit: Option<CanvasTextHit>,
}

pub type CanvasPointerEvent = CanvasMouseEvent;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CanvasWheelEvent {
    pub item_id: CanvasItemId,
    pub delta: Point,
    pub canvas_position: Point,
    pub scene_position: Point,
    pub local_position: Point,
    pub text_hit: Option<CanvasTextHit>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CanvasDragEvent {
    pub item_id: CanvasItemId,
    pub button: CanvasMouseButton,
    pub start_canvas_position: Point,
    pub start_scene_position: Point,
    pub start_local_position: Point,
    pub start_text_hit: Option<CanvasTextHit>,
    pub canvas_position: Point,
    pub scene_position: Point,
    pub local_position: Point,
    pub text_hit: Option<CanvasTextHit>,
    pub delta: Point,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CanvasTextHit {
    pub utf8_start: usize,
    pub utf8_end: usize,
    pub line_index: usize,
    pub line_start: usize,
    pub line_end: usize,
    pub line_top: Dp,
    pub line_height: Dp,
    pub line_width: Dp,
    pub cluster_bounds: Rect,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CanvasTextSpan {
    pub content: String,
    pub style: CanvasTextStyle,
}

impl CanvasTextSpan {
    pub fn new(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            style: CanvasTextStyle::default(),
        }
    }

    pub fn style(mut self, style: CanvasTextStyle) -> Self {
        self.style = style;
        self
    }
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

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CanvasInnerShadow {
    pub color: Color,
    pub offset: Point,
    pub blur: Dp,
}

impl CanvasInnerShadow {
    pub fn new(color: Color, offset: impl Into<Point>, blur: impl Into<Dp>) -> Self {
        Self {
            color,
            offset: offset.into(),
            blur: blur.into(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CanvasColorFilter {
    pub multiply: [f32; 4],
    pub add: [f32; 4],
}

impl CanvasColorFilter {
    pub const IDENTITY: Self = Self {
        multiply: [1.0, 1.0, 1.0, 1.0],
        add: [0.0, 0.0, 0.0, 0.0],
    };

    pub const fn linear(multiply: [f32; 4], add: [f32; 4]) -> Self {
        Self { multiply, add }
    }

    pub fn multiply(color: Color) -> Self {
        let rgba = color.to_linear_rgba_f32();
        Self::linear(rgba, [0.0; 4])
    }

    pub fn tint(color: Color, amount: f32) -> Self {
        let amount = amount.clamp(0.0, 1.0);
        let rgba = color.to_linear_rgba_f32();
        let base = 1.0 - amount;
        Self::linear(
            [base, base, base, 1.0],
            [rgba[0] * amount, rgba[1] * amount, rgba[2] * amount, 0.0],
        )
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum CanvasEffect {
    Blur(Dp),
    ColorFilter(CanvasColorFilter),
    InnerShadow(CanvasInnerShadow),
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
pub(crate) struct CanvasItemStyle {
    pub id: CanvasItemId,
    pub name: Option<String>,
    pub transform: CanvasTransform2D,
    pub opacity: f32,
    pub blend_mode: CanvasBlendMode,
    pub effects: Vec<CanvasEffect>,
    pub isolation: bool,
    pub cursor: Option<CursorStyle>,
    pub visible: bool,
    pub hit_test: bool,
}

impl CanvasItemStyle {
    pub fn new(id: impl Into<CanvasItemId>) -> Self {
        Self {
            id: id.into(),
            name: None,
            transform: CanvasTransform2D::IDENTITY,
            opacity: 1.0,
            blend_mode: CanvasBlendMode::Normal,
            effects: Vec::new(),
            isolation: false,
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
pub struct CanvasPath {
    style: CanvasItemStyle,
    path: PathBuilder,
    fill_rule: CanvasFillRule,
    fill: Option<Value<CanvasBrush>>,
    stroke: Option<CanvasStroke>,
    shadow: Option<Value<CanvasShadow>>,
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

    pub fn id(&self) -> CanvasItemId {
        self.style.id
    }

    pub fn name(&self) -> Option<&str> {
        self.style.name.as_deref()
    }

    pub fn path(&self) -> &PathBuilder {
        &self.path
    }

    pub fn fill_brush(&self) -> Option<&Value<CanvasBrush>> {
        self.fill.as_ref()
    }

    pub fn stroke_style(&self) -> Option<&CanvasStroke> {
        self.stroke.as_ref()
    }

    pub fn shadow_style(&self) -> Option<&Value<CanvasShadow>> {
        self.shadow.as_ref()
    }

    pub fn transform(mut self, transform: CanvasTransform2D) -> Self {
        self.style.transform = transform;
        self
    }

    pub fn name_item(mut self, name: impl Into<String>) -> Self {
        self.style.name = Some(name.into());
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

    pub fn effects(mut self, effects: impl Into<Vec<CanvasEffect>>) -> Self {
        self.style.effects = effects.into();
        self
    }

    pub fn isolation(mut self, isolation: bool) -> Self {
        self.style.isolation = isolation;
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
enum CanvasTextContent {
    Plain(String),
    Rich(Vec<CanvasTextSpan>),
}

impl CanvasTextContent {
    fn plain_text(&self) -> String {
        match self {
            Self::Plain(content) => content.clone(),
            Self::Rich(spans) => spans.iter().map(|span| span.content.as_str()).collect(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct CanvasText {
    style: CanvasItemStyle,
    frame: Rect,
    content: CanvasTextContent,
    text_style: CanvasTextStyle,
    paragraph_style: CanvasParagraphStyle,
}

impl CanvasText {
    pub fn new(id: impl Into<CanvasItemId>, frame: Rect, content: impl Into<String>) -> Self {
        Self {
            style: CanvasItemStyle::new(id),
            frame,
            content: CanvasTextContent::Plain(content.into()),
            text_style: CanvasTextStyle::default(),
            paragraph_style: CanvasParagraphStyle::default(),
        }
    }

    pub fn rich(
        id: impl Into<CanvasItemId>,
        frame: Rect,
        spans: impl Into<Vec<CanvasTextSpan>>,
    ) -> Self {
        Self {
            style: CanvasItemStyle::new(id),
            frame,
            content: CanvasTextContent::Rich(spans.into()),
            text_style: CanvasTextStyle::default(),
            paragraph_style: CanvasParagraphStyle::default(),
        }
    }

    pub fn text_style(mut self, text_style: CanvasTextStyle) -> Self {
        self.text_style = text_style;
        self
    }

    pub fn id(&self) -> CanvasItemId {
        self.style.id
    }

    pub fn name(&self) -> Option<&str> {
        self.style.name.as_deref()
    }

    pub fn frame(&self) -> Rect {
        self.frame
    }

    pub fn plain_text(&self) -> String {
        self.content.plain_text()
    }

    pub fn name_item(mut self, name: impl Into<String>) -> Self {
        self.style.name = Some(name.into());
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

    pub fn effects(mut self, effects: impl Into<Vec<CanvasEffect>>) -> Self {
        self.style.effects = effects.into();
        self
    }

    pub fn isolation(mut self, isolation: bool) -> Self {
        self.style.isolation = isolation;
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
pub struct CanvasImage {
    style: CanvasItemStyle,
    frame: Rect,
    source: MediaSource,
    fit: ContentFit,
    corner_radius: Dp,
    source_rect: Option<Rect>,
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

    pub fn id(&self) -> CanvasItemId {
        self.style.id
    }

    pub fn name(&self) -> Option<&str> {
        self.style.name.as_deref()
    }

    pub fn frame(&self) -> Rect {
        self.frame
    }

    pub fn source(&self) -> &MediaSource {
        &self.source
    }

    pub fn fit_mode(&self) -> ContentFit {
        self.fit
    }

    pub fn name_item(mut self, name: impl Into<String>) -> Self {
        self.style.name = Some(name.into());
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

    pub fn effects(mut self, effects: impl Into<Vec<CanvasEffect>>) -> Self {
        self.style.effects = effects.into();
        self
    }

    pub fn isolation(mut self, isolation: bool) -> Self {
        self.style.isolation = isolation;
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
pub enum CanvasGroupShape {
    Path {
        path: PathBuilder,
        fill_rule: CanvasFillRule,
    },
}

impl CanvasGroupShape {
    pub fn path(path: PathBuilder) -> Self {
        Self::Path {
            fill_rule: path.fill_rule,
            path,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum CanvasGroupMode {
    Clip,
    Mask,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CanvasGroup {
    style: CanvasItemStyle,
    mode: CanvasGroupMode,
    shape: CanvasGroupShape,
    items: Vec<CanvasItem>,
}

impl CanvasGroup {
    pub fn new(
        id: impl Into<CanvasItemId>,
        mode: CanvasGroupMode,
        shape: CanvasGroupShape,
        items: impl Into<Vec<CanvasItem>>,
    ) -> Self {
        Self {
            style: CanvasItemStyle::new(id),
            mode,
            shape,
            items: items.into(),
        }
    }

    pub fn id(&self) -> CanvasItemId {
        self.style.id
    }

    pub fn name(&self) -> Option<&str> {
        self.style.name.as_deref()
    }

    pub fn mode(&self) -> &CanvasGroupMode {
        &self.mode
    }

    pub fn shape(&self) -> &CanvasGroupShape {
        &self.shape
    }

    pub fn items(&self) -> &[CanvasItem] {
        &self.items
    }

    pub fn items_mut(&mut self) -> &mut Vec<CanvasItem> {
        &mut self.items
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

    pub fn effects(mut self, effects: impl Into<Vec<CanvasEffect>>) -> Self {
        self.style.effects = effects.into();
        self
    }

    pub fn isolation(mut self, isolation: bool) -> Self {
        self.style.isolation = isolation;
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

    pub fn name_item(mut self, name: impl Into<String>) -> Self {
        self.style.name = Some(name.into());
        self
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum CanvasItem {
    Path(CanvasPath),
    Text(CanvasText),
    Image(CanvasImage),
    Group(CanvasGroup),
}

impl CanvasItem {
    pub fn id(&self) -> CanvasItemId {
        match self {
            Self::Path(path) => path.style.id,
            Self::Text(text) => text.style.id,
            Self::Image(image) => image.style.id,
            Self::Group(group) => group.style.id,
        }
    }

    pub fn name(&self) -> Option<&str> {
        self.style().name.as_deref()
    }

    pub fn kind(&self) -> CanvasItemKind {
        match self {
            Self::Path(_) => CanvasItemKind::Path,
            Self::Text(_) => CanvasItemKind::Text,
            Self::Image(_) => CanvasItemKind::Image,
            Self::Group(_) => CanvasItemKind::Group,
        }
    }

    pub fn children(&self) -> &[CanvasItem] {
        match self {
            Self::Group(group) => group.items(),
            _ => &[],
        }
    }

    pub fn children_mut(&mut self) -> Option<&mut Vec<CanvasItem>> {
        match self {
            Self::Group(group) => Some(group.items_mut()),
            _ => None,
        }
    }

    pub fn bounds_rect(&self) -> Option<Rect> {
        self.layout_bounds().map(rect_from_bounds)
    }

    pub fn hit_bounds_rect(&self) -> Option<Rect> {
        self.hit_bounds().map(rect_from_bounds)
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
            Self::Group(group) => {
                let nested_clip = CanvasClipContext {
                    clip_rect: compose_clip_rect(
                        clip_context.clip_rect,
                        match group.mode {
                            CanvasGroupMode::Clip => group_shape_clip_rect(&group.shape, origin),
                            CanvasGroupMode::Mask => None,
                        },
                    ),
                    clip_mask: clip_context.clip_mask,
                };
                tessellate_items(
                    &group.items,
                    origin,
                    opacity * group.style.opacity,
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
            Self::Group(group) => &group.style,
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
            Self::Group(group) => {
                group_shape_bounds(&group.shape).or_else(|| canvas_bounds(&group.items))
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
            Self::Group(group) => {
                group_shape_bounds(&group.shape).or_else(|| canvas_bounds(&group.items))
            }
        }?;
        Some(transform_bounds(bounds, self.style().transform))
    }
}

impl From<CanvasPath> for CanvasItem {
    fn from(value: CanvasPath) -> Self {
        Self::Path(value)
    }
}

impl From<CanvasText> for CanvasItem {
    fn from(value: CanvasText) -> Self {
        Self::Text(value)
    }
}

impl From<CanvasImage> for CanvasItem {
    fn from(value: CanvasImage) -> Self {
        Self::Image(value)
    }
}

impl From<CanvasGroup> for CanvasItem {
    fn from(value: CanvasGroup) -> Self {
        Self::Group(value)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CanvasItemKind {
    Path,
    Text,
    Image,
    Group,
}

fn item_requires_composite(item: &CanvasItem) -> bool {
    match item {
        CanvasItem::Path(path) => {
            path.style.blend_mode != CanvasBlendMode::Normal
                || path.style.isolation
                || !path.style.effects.is_empty()
        }
        CanvasItem::Text(text) => {
            text.style.blend_mode != CanvasBlendMode::Normal
                || text.style.isolation
                || !text.style.effects.is_empty()
        }
        CanvasItem::Image(image) => {
            image.style.blend_mode != CanvasBlendMode::Normal
                || image.style.isolation
                || !image.style.effects.is_empty()
        }
        CanvasItem::Group(_) => true,
    }
}

fn bounds_rect(bounds: RectBounds) -> Rect {
    Rect::new(bounds.min_x, bounds.min_y, bounds.width(), bounds.height())
}

fn rect_from_bounds(bounds: RectBounds) -> Rect {
    bounds_rect(bounds)
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

fn group_shape_bounds(shape: &CanvasGroupShape) -> Option<RectBounds> {
    match shape {
        CanvasGroupShape::Path { path, .. } => path.control_bounds().map(|bounds| {
            RectBounds::from_min_max(bounds.min.x, bounds.min.y, bounds.max.x, bounds.max.y)
        }),
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct CanvasClipContext {
    pub(crate) clip_rect: Option<Rect>,
    pub(crate) clip_mask: Option<ClipMask>,
}

fn group_shape_clip_rect(shape: &CanvasGroupShape, origin: Point) -> Option<Rect> {
    match shape {
        CanvasGroupShape::Path { path, .. } => path.control_bounds().map(|bounds| {
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
    let resolved_effects = resolve_canvas_effects(&style.effects);

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
        CanvasItem::Group(group_item) => {
            let nested_output =
                tessellate_items(&group_item.items, origin, opacity, clip, media, units);
            let CanvasGroupShape::Path { path, fill_rule } = &group_item.shape;
            let mask = tessellate_path(
                &CanvasPath::new(group_item.style.id, path.clone())
                    .fill_rule(*fill_rule)
                    .fill(Color::WHITE),
                origin,
                1.0,
                CanvasClipContext::default(),
                media,
                units,
            );
            let mask_commands = match group_item.mode {
                CanvasGroupMode::Clip | CanvasGroupMode::Mask => {
                    Some(output_to_commands(mask).into())
                }
            };
            (nested_output, mask_commands, style.blend_mode)
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
            blur_radius: resolved_effects.blur_radius,
            color_filter: resolved_effects.color_filter,
            inner_shadow_color: resolved_effects.inner_shadow.map(|shadow| shadow.color),
            inner_shadow_offset: resolved_effects
                .inner_shadow
                .map(|shadow| shadow.offset)
                .unwrap_or(Point::ZERO),
            inner_shadow_blur_radius: resolved_effects
                .inner_shadow
                .map(|shadow| shadow.blur.get().max(0.0))
                .unwrap_or(0.0),
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
    let content = text.content.plain_text();
    let rich_spans = match &text.content {
        CanvasTextContent::Plain(_) => None,
        CanvasTextContent::Rich(spans) => Some(Arc::from(
            spans
                .iter()
                .cloned()
                .map(|span| CanvasTextSpanPrimitive {
                    content: span.content,
                    font_family: span.style.font_family,
                    color: span
                        .style
                        .color
                        .with_alpha_factor(opacity * text.style.opacity),
                    font_size: span.style.font_size.get(),
                    font_weight: span.style.font_weight,
                    line_height: span.style.line_height.map(|height| height.get()),
                    letter_spacing: span.style.letter_spacing.get(),
                })
                .collect::<Vec<_>>(),
        )),
    };
    CanvasRenderOutput {
        texts: vec![TextPrimitive {
            content,
            rich_spans,
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
    pub const STABLE_JSON_FORMAT: &str = "tgui.canvas.scene";
    pub const STABLE_JSON_VERSION: u32 = 1;

    pub fn empty() -> Self {
        Self::default()
    }

    pub fn from_items(items: impl Into<Vec<CanvasItem>>) -> Self {
        Self {
            items: items.into(),
        }
    }

    pub fn items(&self) -> &[CanvasItem] {
        &self.items
    }

    pub fn items_mut(&mut self) -> &mut Vec<CanvasItem> {
        &mut self.items
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn bounds(&self) -> Option<Rect> {
        canvas_scene_bounds(self).map(rect_from_bounds)
    }

    pub fn push(&mut self, item: impl Into<CanvasItem>) {
        self.items.push(item.into());
    }

    pub fn insert(&mut self, index: usize, item: impl Into<CanvasItem>) {
        self.items.insert(index.min(self.items.len()), item.into());
    }

    pub fn clear(&mut self) {
        self.items.clear();
    }

    pub fn remove(&mut self, id: CanvasItemId) -> Option<CanvasItem> {
        remove_item_by_id(&mut self.items, id)
    }

    pub fn contains_id(&self, id: CanvasItemId) -> bool {
        self.find(id).is_some()
    }

    pub fn contains_name(&self, name: &str) -> bool {
        self.find_named(name).is_some()
    }

    pub fn find(&self, id: CanvasItemId) -> Option<&CanvasItem> {
        find_item_by_id(&self.items, id)
    }

    pub fn find_mut(&mut self, id: CanvasItemId) -> Option<&mut CanvasItem> {
        find_item_mut_by_id(&mut self.items, id)
    }

    pub fn find_named(&self, name: &str) -> Option<&CanvasItem> {
        find_item_by_name(&self.items, name)
    }

    pub fn find_named_mut(&mut self, name: &str) -> Option<&mut CanvasItem> {
        find_item_mut_by_name(&mut self.items, name)
    }

    pub fn visit(&self, mut visitor: impl FnMut(CanvasSceneVisit<'_>)) {
        let mut index_path = Vec::new();
        visit_scene_items(&self.items, 0, &mut index_path, &mut visitor);
    }

    pub fn debug_info(&self) -> CanvasSceneDebugInfo {
        let mut stats = CanvasSceneDebugStats {
            root_items: self.items.len(),
            bounds: self.bounds(),
            ..Default::default()
        };
        let nodes = self
            .items
            .iter()
            .enumerate()
            .map(|(index, item)| {
                let mut path = vec![index];
                build_debug_node(item, 0, &mut path, &mut stats)
            })
            .collect();
        CanvasSceneDebugInfo { stats, nodes }
    }

    pub fn query_point(&self, scene_position: Point) -> Option<CanvasSceneHit> {
        self.query_point_with(&CanvasSceneQueryOptions::default(), scene_position)
    }

    pub fn query_point_all(&self, scene_position: Point) -> Vec<CanvasSceneHit> {
        self.query_point_all_with(&CanvasSceneQueryOptions::default(), scene_position)
    }

    pub fn query_point_with(
        &self,
        options: &CanvasSceneQueryOptions,
        scene_position: Point,
    ) -> Option<CanvasSceneHit> {
        self.query_point_all_with(options, scene_position)
            .into_iter()
            .next()
    }

    pub fn query_point_all_with(
        &self,
        options: &CanvasSceneQueryOptions,
        scene_position: Point,
    ) -> Vec<CanvasSceneHit> {
        let context = options.as_context();
        query_canvas_scene_hits(self, &context.font_manager, context.units, scene_position)
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn query_point_with_runtime_context(
        &self,
        font_manager: &FontManager,
        units: UnitContext,
        scene_position: Point,
    ) -> Option<CanvasSceneHit> {
        self.query_point_all_with_runtime_context(font_manager, units, scene_position)
            .into_iter()
            .next()
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn query_point_all_with_runtime_context(
        &self,
        font_manager: &FontManager,
        units: UnitContext,
        scene_position: Point,
    ) -> Vec<CanvasSceneHit> {
        query_canvas_scene_hits(self, font_manager, units, scene_position)
    }

    pub fn export_json(&self) -> String {
        export_canvas_scene_json(self)
    }

    pub fn export_debug_text(&self) -> String {
        self.debug_info().to_pretty_text()
    }

    pub fn export_debug_json(&self) -> String {
        self.debug_info().to_pretty_json()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct CanvasSceneHit {
    pub item_id: CanvasItemId,
    pub name: Option<String>,
    pub kind: CanvasItemKind,
    pub depth: usize,
    pub index_path: Vec<usize>,
    pub scene_position: Point,
    pub local_position: Point,
    pub bounds: Option<Rect>,
    pub text_hit: Option<CanvasTextHit>,
}

pub struct CanvasSceneQueryOptions {
    font_catalog: FontCatalog,
    scale_factor: f32,
    font_scale: f32,
}

impl Default for CanvasSceneQueryOptions {
    fn default() -> Self {
        Self {
            font_catalog: FontCatalog::default(),
            scale_factor: 1.0,
            font_scale: 1.0,
        }
    }
}

impl CanvasSceneQueryOptions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn scale_factor(mut self, scale_factor: f32) -> Self {
        self.scale_factor = scale_factor;
        self
    }

    pub fn font_scale(mut self, font_scale: f32) -> Self {
        self.font_scale = font_scale;
        self
    }

    pub fn font_bytes(mut self, name: impl Into<String>, bytes: &'static [u8]) -> Self {
        self.font_catalog.register_font(name, bytes);
        self
    }

    pub fn font_file(
        mut self,
        name: impl Into<String>,
        path: impl Into<std::path::PathBuf>,
    ) -> Self {
        self.font_catalog.register_font_file(name, path);
        self
    }

    pub fn default_font(mut self, name: impl Into<String>) -> Self {
        self.font_catalog.set_default_font(name);
        self
    }

    fn as_context(&self) -> CanvasSceneQueryContext {
        CanvasSceneQueryContext::new(
            &self.font_catalog,
            UnitContext::new(self.scale_factor, self.font_scale),
        )
    }
}

#[derive(Debug)]
pub struct CanvasSceneVisit<'a> {
    pub depth: usize,
    pub index_path: Vec<usize>,
    pub item: &'a CanvasItem,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct CanvasSceneDebugStats {
    pub root_items: usize,
    pub total_items: usize,
    pub named_items: usize,
    pub visible_items: usize,
    pub hit_testable_items: usize,
    pub path_items: usize,
    pub text_items: usize,
    pub image_items: usize,
    pub group_items: usize,
    pub max_depth: usize,
    pub bounds: Option<Rect>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CanvasSceneDebugNode {
    pub id: CanvasItemId,
    pub name: Option<String>,
    pub kind: CanvasItemKind,
    pub depth: usize,
    pub index_path: Vec<usize>,
    pub visible: bool,
    pub hit_test: bool,
    pub opacity: f32,
    pub blend_mode: CanvasBlendMode,
    pub bounds: Option<Rect>,
    pub child_count: usize,
    pub summary: String,
    pub children: Vec<CanvasSceneDebugNode>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CanvasSceneDebugInfo {
    pub stats: CanvasSceneDebugStats,
    pub nodes: Vec<CanvasSceneDebugNode>,
}

impl CanvasSceneDebugInfo {
    pub fn to_pretty_text(&self) -> String {
        let mut out = String::new();
        out.push_str("CanvasScene\n");
        out.push_str(&format!(
            "  root_items={} total_items={} named_items={} visible_items={} hit_testable_items={} max_depth={}\n",
            self.stats.root_items,
            self.stats.total_items,
            self.stats.named_items,
            self.stats.visible_items,
            self.stats.hit_testable_items,
            self.stats.max_depth,
        ));
        out.push_str(&format!(
            "  kinds: path={} text={} image={} group={}\n",
            self.stats.path_items,
            self.stats.text_items,
            self.stats.image_items,
            self.stats.group_items,
        ));
        if let Some(bounds) = self.stats.bounds {
            out.push_str(&format!(
                "  bounds: x={:.1} y={:.1} width={:.1} height={:.1}\n",
                bounds.x.get(),
                bounds.y.get(),
                bounds.width.get(),
                bounds.height.get(),
            ));
        }
        for node in &self.nodes {
            write_debug_node_text(&mut out, node);
        }
        out
    }

    pub fn to_pretty_json(&self) -> String {
        let mut out = String::new();
        out.push_str("{\n");
        out.push_str("  \"stats\": ");
        write_debug_stats_json(&mut out, &self.stats, 1);
        out.push_str(",\n  \"nodes\": [\n");
        for (index, node) in self.nodes.iter().enumerate() {
            write_debug_node_json(&mut out, node, 2);
            if index + 1 != self.nodes.len() {
                out.push(',');
            }
            out.push('\n');
        }
        out.push_str("  ]\n}");
        out
    }
}

fn visit_scene_items<'a>(
    items: &'a [CanvasItem],
    depth: usize,
    index_path: &mut Vec<usize>,
    visitor: &mut impl FnMut(CanvasSceneVisit<'a>),
) {
    for (index, item) in items.iter().enumerate() {
        index_path.push(index);
        visitor(CanvasSceneVisit {
            depth,
            index_path: index_path.clone(),
            item,
        });
        if let CanvasItem::Group(group) = item {
            visit_scene_items(&group.items, depth + 1, index_path, visitor);
        }
        index_path.pop();
    }
}

fn find_item_by_id(items: &[CanvasItem], id: CanvasItemId) -> Option<&CanvasItem> {
    for item in items {
        if item.id() == id {
            return Some(item);
        }
        if let CanvasItem::Group(group) = item {
            if let Some(found) = find_item_by_id(&group.items, id) {
                return Some(found);
            }
        }
    }
    None
}

fn find_item_mut_by_id(items: &mut [CanvasItem], id: CanvasItemId) -> Option<&mut CanvasItem> {
    for item in items.iter_mut() {
        if item.id() == id {
            return Some(item);
        }
        if let CanvasItem::Group(group) = item {
            if let Some(found) = find_item_mut_by_id(&mut group.items, id) {
                return Some(found);
            }
        }
    }
    None
}

fn find_item_by_name<'a>(items: &'a [CanvasItem], name: &str) -> Option<&'a CanvasItem> {
    for item in items {
        if item.name() == Some(name) {
            return Some(item);
        }
        if let CanvasItem::Group(group) = item {
            if let Some(found) = find_item_by_name(&group.items, name) {
                return Some(found);
            }
        }
    }
    None
}

fn find_item_mut_by_name<'a>(
    items: &'a mut [CanvasItem],
    name: &str,
) -> Option<&'a mut CanvasItem> {
    for item in items.iter_mut() {
        if item.name() == Some(name) {
            return Some(item);
        }
        if let CanvasItem::Group(group) = item {
            if let Some(found) = find_item_mut_by_name(&mut group.items, name) {
                return Some(found);
            }
        }
    }
    None
}

fn remove_item_by_id(items: &mut Vec<CanvasItem>, id: CanvasItemId) -> Option<CanvasItem> {
    let mut index = 0;
    while index < items.len() {
        if items[index].id() == id {
            return Some(items.remove(index));
        }
        if let CanvasItem::Group(group) = &mut items[index] {
            if let Some(removed) = remove_item_by_id(&mut group.items, id) {
                return Some(removed);
            }
        }
        index += 1;
    }
    None
}

fn build_debug_node(
    item: &CanvasItem,
    depth: usize,
    index_path: &mut Vec<usize>,
    stats: &mut CanvasSceneDebugStats,
) -> CanvasSceneDebugNode {
    stats.total_items += 1;
    stats.max_depth = stats.max_depth.max(depth);
    if item.name().is_some() {
        stats.named_items += 1;
    }
    if item.style().visible {
        stats.visible_items += 1;
    }
    if item.style().hit_test {
        stats.hit_testable_items += 1;
    }
    match item.kind() {
        CanvasItemKind::Path => stats.path_items += 1,
        CanvasItemKind::Text => stats.text_items += 1,
        CanvasItemKind::Image => stats.image_items += 1,
        CanvasItemKind::Group => stats.group_items += 1,
    }

    let summary = match item {
        CanvasItem::Path(path) => format!(
            "path(fill={}, stroke={}, shadow={})",
            path.fill.is_some(),
            path.stroke.is_some(),
            path.shadow.is_some()
        ),
        CanvasItem::Text(text) => format!("text(chars={})", text.plain_text().chars().count()),
        CanvasItem::Image(image) => format!("image(fit={:?})", image.fit),
        CanvasItem::Group(group) => {
            format!("group(mode={:?}, items={})", group.mode, group.items.len())
        }
    };

    let children = match item {
        CanvasItem::Group(group) => group
            .items
            .iter()
            .enumerate()
            .map(|(index, child)| {
                index_path.push(index);
                let node = build_debug_node(child, depth + 1, index_path, stats);
                index_path.pop();
                node
            })
            .collect(),
        _ => Vec::new(),
    };

    CanvasSceneDebugNode {
        id: item.id(),
        name: item.name().map(ToOwned::to_owned),
        kind: item.kind(),
        depth,
        index_path: index_path.clone(),
        visible: item.style().visible,
        hit_test: item.style().hit_test,
        opacity: item.style().opacity,
        blend_mode: item.style().blend_mode,
        bounds: item.layout_bounds().map(rect_from_bounds),
        child_count: item.children().len(),
        summary,
        children,
    }
}

fn write_debug_node_text(out: &mut String, node: &CanvasSceneDebugNode) {
    let indent = "  ".repeat(node.depth + 1);
    out.push_str(&format!(
        "{}- {:?} id={}{} visible={} hit_test={} opacity={:.2}",
        indent,
        node.kind,
        node.id.get(),
        node.name
            .as_ref()
            .map(|name| format!(" name=\"{}\"", name))
            .unwrap_or_default(),
        node.visible,
        node.hit_test,
        node.opacity,
    ));
    if let Some(bounds) = node.bounds {
        out.push_str(&format!(
            " bounds=({:.1}, {:.1}, {:.1}, {:.1})",
            bounds.x.get(),
            bounds.y.get(),
            bounds.width.get(),
            bounds.height.get(),
        ));
    }
    out.push_str(&format!(" {}\n", node.summary));
    for child in &node.children {
        write_debug_node_text(out, child);
    }
}

fn write_debug_stats_json(out: &mut String, stats: &CanvasSceneDebugStats, indent: usize) {
    out.push_str("{\n");
    let prefix = "  ".repeat(indent + 1);
    out.push_str(&format!("{prefix}\"root_items\": {},\n", stats.root_items));
    out.push_str(&format!(
        "{prefix}\"total_items\": {},\n",
        stats.total_items
    ));
    out.push_str(&format!(
        "{prefix}\"named_items\": {},\n",
        stats.named_items
    ));
    out.push_str(&format!(
        "{prefix}\"visible_items\": {},\n",
        stats.visible_items
    ));
    out.push_str(&format!(
        "{prefix}\"hit_testable_items\": {},\n",
        stats.hit_testable_items
    ));
    out.push_str(&format!("{prefix}\"path_items\": {},\n", stats.path_items));
    out.push_str(&format!("{prefix}\"text_items\": {},\n", stats.text_items));
    out.push_str(&format!(
        "{prefix}\"image_items\": {},\n",
        stats.image_items
    ));
    out.push_str(&format!(
        "{prefix}\"group_items\": {},\n",
        stats.group_items
    ));
    out.push_str(&format!("{prefix}\"max_depth\": {},\n", stats.max_depth));
    out.push_str(&format!("{prefix}\"bounds\": "));
    write_optional_rect_json(out, stats.bounds, indent + 1);
    out.push_str(&format!("\n{}", "  ".repeat(indent)));
    out.push('}');
}

fn write_debug_node_json(out: &mut String, node: &CanvasSceneDebugNode, indent: usize) {
    let prefix = "  ".repeat(indent);
    out.push_str(&format!("{prefix}{{\n"));
    let field_prefix = "  ".repeat(indent + 1);
    out.push_str(&format!("{field_prefix}\"id\": {},\n", node.id.get()));
    out.push_str(&format!(
        "{field_prefix}\"name\": {},\n",
        node.name
            .as_deref()
            .map(json_string)
            .unwrap_or_else(|| "null".to_string())
    ));
    out.push_str(&format!("{field_prefix}\"kind\": \"{:?}\",\n", node.kind));
    out.push_str(&format!("{field_prefix}\"depth\": {},\n", node.depth));
    out.push_str(&format!(
        "{field_prefix}\"index_path\": {},\n",
        json_usize_array(&node.index_path)
    ));
    out.push_str(&format!("{field_prefix}\"visible\": {},\n", node.visible));
    out.push_str(&format!("{field_prefix}\"hit_test\": {},\n", node.hit_test));
    out.push_str(&format!("{field_prefix}\"opacity\": {},\n", node.opacity));
    out.push_str(&format!(
        "{field_prefix}\"blend_mode\": \"{:?}\",\n",
        node.blend_mode
    ));
    out.push_str(&format!("{field_prefix}\"bounds\": "));
    write_optional_rect_json(out, node.bounds, indent + 1);
    out.push_str(",\n");
    out.push_str(&format!(
        "{field_prefix}\"child_count\": {},\n",
        node.child_count
    ));
    out.push_str(&format!(
        "{field_prefix}\"summary\": {},\n",
        json_string(&node.summary)
    ));
    out.push_str(&format!("{field_prefix}\"children\": ["));
    if node.children.is_empty() {
        out.push_str("]\n");
    } else {
        out.push('\n');
        for (index, child) in node.children.iter().enumerate() {
            write_debug_node_json(out, child, indent + 2);
            if index + 1 != node.children.len() {
                out.push(',');
            }
            out.push('\n');
        }
        out.push_str(&format!("{field_prefix}]\n"));
    }
    out.push_str(&format!("{prefix}}}"));
}

fn write_optional_rect_json(out: &mut String, rect: Option<Rect>, indent: usize) {
    match rect {
        Some(rect) => {
            let prefix = "  ".repeat(indent + 1);
            out.push_str("{\n");
            out.push_str(&format!("{prefix}\"x\": {},\n", rect.x.get()));
            out.push_str(&format!("{prefix}\"y\": {},\n", rect.y.get()));
            out.push_str(&format!("{prefix}\"width\": {},\n", rect.width.get()));
            out.push_str(&format!("{prefix}\"height\": {}\n", rect.height.get()));
            out.push_str(&format!("{}{}", "  ".repeat(indent), "}"));
        }
        None => out.push_str("null"),
    }
}

fn json_usize_array(values: &[usize]) -> String {
    let mut out = String::from("[");
    for (index, value) in values.iter().enumerate() {
        if index > 0 {
            out.push_str(", ");
        }
        out.push_str(&value.to_string());
    }
    out.push(']');
    out
}

fn json_string(value: &str) -> String {
    let mut out = String::from("\"");
    for ch in value.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\u{08}' => out.push_str("\\b"),
            '\u{0C}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            ch if ch <= '\u{1F}' => out.push_str(&format!("\\u{:04X}", ch as u32)),
            _ => out.push(ch),
        }
    }
    out.push('"');
    out
}

fn query_canvas_scene_hits(
    scene: &CanvasScene,
    font_manager: &FontManager,
    units: UnitContext,
    scene_position: Point,
) -> Vec<CanvasSceneHit> {
    let query_session = CanvasSceneQuerySession::new(font_manager, units);
    let mut metadata = HashMap::new();
    scene.visit(|entry| {
        metadata.insert(
            entry.item.id(),
            (
                entry.depth,
                entry.index_path,
                entry.item.kind(),
                entry.item.name().map(ToOwned::to_owned),
                entry.item.hit_bounds_rect(),
            ),
        );
    });

    let mut hits = Vec::new();
    let mut index_path = Vec::new();
    collect_query_hits_recursive(
        scene.items(),
        scene_position,
        &mut index_path,
        &query_session,
        &metadata,
        &mut hits,
    );
    hits
}

fn collect_query_hits_recursive(
    items: &[CanvasItem],
    scene_position: Point,
    index_path: &mut Vec<usize>,
    query_session: &CanvasSceneQuerySession<'_>,
    metadata: &HashMap<
        CanvasItemId,
        (
            usize,
            Vec<usize>,
            CanvasItemKind,
            Option<String>,
            Option<Rect>,
        ),
    >,
    hits: &mut Vec<CanvasSceneHit>,
) {
    for index in (0..items.len()).rev() {
        let item = &items[index];
        index_path.push(index);
        if !item.style().visible || !item.style().hit_test {
            index_path.pop();
            continue;
        }

        let contains = item_contains_scene_point(item, scene_position);
        if let CanvasItem::Group(group) = item {
            if contains {
                collect_query_hits_recursive(
                    &group.items,
                    scene_position,
                    index_path,
                    query_session,
                    metadata,
                    hits,
                );
            }
        }

        if contains {
            let local_position = item_event_local_position(item, scene_position);
            let (depth, stored_path, kind, name, bounds) =
                metadata.get(&item.id()).cloned().unwrap_or((
                    index_path.len().saturating_sub(1),
                    index_path.clone(),
                    item.kind(),
                    item.name().map(ToOwned::to_owned),
                    item.hit_bounds_rect(),
                ));
            let text_hit = item_text_hit_at_point(item, scene_position, query_session);
            hits.push(CanvasSceneHit {
                item_id: item.id(),
                name,
                kind,
                depth,
                index_path: stored_path,
                scene_position,
                local_position,
                bounds,
                text_hit,
            });
        }

        index_path.pop();
    }
}

struct CanvasSceneQueryContext {
    font_manager: FontManager,
    units: UnitContext,
}

impl CanvasSceneQueryContext {
    fn new(font_catalog: &FontCatalog, units: UnitContext) -> Self {
        Self {
            font_manager: FontManager::new(font_catalog),
            units,
        }
    }
}

struct CanvasSceneQuerySession<'a> {
    font_manager: &'a FontManager,
    units: UnitContext,
    text_hit_cache: RefCell<HashMap<u64, Arc<[CanvasTextHitEntry]>>>,
}

impl<'a> CanvasSceneQuerySession<'a> {
    fn new(font_manager: &'a FontManager, units: UnitContext) -> Self {
        Self {
            font_manager,
            units,
            text_hit_cache: RefCell::new(HashMap::new()),
        }
    }

    fn text_hits_for_item(&self, item: &CanvasItem) -> Arc<[CanvasTextHitEntry]> {
        let cache_key = canvas_text_hit_cache_key(item, self.units);
        if let Some(cached) = self.text_hit_cache.borrow().get(&cache_key).cloned() {
            return cached;
        }
        let computed = item_text_hits(item, self.font_manager, Point::ZERO, self.units);
        self.text_hit_cache
            .borrow_mut()
            .insert(cache_key, Arc::clone(&computed));
        computed
    }
}

fn item_contains_scene_point(item: &CanvasItem, scene_position: Point) -> bool {
    let Some(bounds) = item.hit_bounds_rect() else {
        return false;
    };
    if !bounds.contains(scene_position) {
        return false;
    }

    match scene_hit_geometry_for_item(item) {
        Some(geometry) => hit_geometry_contains(&geometry, scene_position),
        None => true,
    }
}

fn scene_hit_geometry_for_item(item: &CanvasItem) -> Option<CanvasHitGeometry> {
    match item {
        CanvasItem::Path(path) => path_scene_hit_geometry(path),
        CanvasItem::Text(text) => Some(CanvasHitGeometry::Quad(
            if text.style.transform == CanvasTransform2D::IDENTITY {
                rect_to_quad(text.frame)
            } else {
                transform_rect_quad(text.frame, text.style.transform, Point::ZERO)
            },
        )),
        CanvasItem::Image(image) => Some(CanvasHitGeometry::Quad(
            if image.style.transform == CanvasTransform2D::IDENTITY {
                rect_to_quad(image.frame)
            } else {
                transform_rect_quad(image.frame, image.style.transform, Point::ZERO)
            },
        )),
        CanvasItem::Group(group) => group_scene_hit_geometry(group),
    }
}

fn path_scene_hit_geometry(path: &CanvasPath) -> Option<CanvasHitGeometry> {
    let mut triangles = Vec::new();
    let lyon_path = path.path.to_lyon_path();
    let clip = CanvasClipContext::default();

    if path.fill.is_some() {
        if let Some(mesh) = tessellate_fill(
            &lyon_path,
            path.fill_rule,
            &CanvasBrush::Solid(Color::BLACK),
            1.0,
            Point::ZERO,
            clip,
        ) {
            triangles.extend(mesh.triangles.iter().copied());
        }
    }

    if let Some(stroke) = path.stroke.as_ref() {
        if let Some(mesh) = tessellate_stroke(&lyon_path, stroke, 1.0, Point::ZERO, clip) {
            triangles.extend(mesh.triangles.iter().copied());
        }
    }

    if triangles.is_empty() {
        return None;
    }

    let geometry = CanvasHitGeometry::Triangles(Arc::from(triangles));
    Some(transform_hit_geometry(
        &geometry,
        path.style.transform,
        Point::ZERO,
    ))
}

fn group_scene_hit_geometry(group: &CanvasGroup) -> Option<CanvasHitGeometry> {
    let CanvasGroupShape::Path { path, fill_rule } = &group.shape;
    let lyon_path = path.to_lyon_path();
    let mesh = tessellate_fill(
        &lyon_path,
        *fill_rule,
        &CanvasBrush::Solid(Color::BLACK),
        1.0,
        Point::ZERO,
        CanvasClipContext::default(),
    )?;
    let geometry = CanvasHitGeometry::Triangles(mesh.triangles);
    Some(transform_hit_geometry(
        &geometry,
        group.style.transform,
        Point::ZERO,
    ))
}

fn transform_hit_geometry(
    geometry: &CanvasHitGeometry,
    transform: CanvasTransform2D,
    origin: Point,
) -> CanvasHitGeometry {
    if transform == CanvasTransform2D::IDENTITY {
        return geometry.clone();
    }

    match geometry {
        CanvasHitGeometry::Quad(quad) => CanvasHitGeometry::Quad(quad.map(|point_value| {
            transform.apply(Point::new(
                point_value.x - origin.x,
                point_value.y - origin.y,
            ))
        })),
        CanvasHitGeometry::Triangles(triangles) => {
            let transformed = triangles
                .iter()
                .map(|triangle| {
                    triangle.map(|point_value| {
                        let local = Point::new(point_value.x - origin.x, point_value.y - origin.y);
                        transform.apply(local)
                    })
                })
                .collect::<Vec<_>>();
            CanvasHitGeometry::Triangles(Arc::from(transformed))
        }
    }
}

fn hit_geometry_contains(geometry: &CanvasHitGeometry, point: Point) -> bool {
    match geometry {
        CanvasHitGeometry::Quad(quad) => {
            point_in_triangle(point, quad[0], quad[1], quad[2])
                || point_in_triangle(point, quad[0], quad[2], quad[3])
        }
        CanvasHitGeometry::Triangles(triangles) => triangles
            .iter()
            .any(|triangle| point_in_triangle(point, triangle[0], triangle[1], triangle[2])),
    }
}

fn point_in_triangle(point: Point, a: Point, b: Point, c: Point) -> bool {
    let point_sign = |lhs: Point, rhs: Point, other: Point| {
        (lhs.x.get() - other.x.get()) * (rhs.y.get() - other.y.get())
            - (rhs.x.get() - other.x.get()) * (lhs.y.get() - other.y.get())
    };

    let d1 = point_sign(point, a, b);
    let d2 = point_sign(point, b, c);
    let d3 = point_sign(point, c, a);
    let has_neg = d1 < 0.0 || d2 < 0.0 || d3 < 0.0;
    let has_pos = d1 > 0.0 || d2 > 0.0 || d3 > 0.0;
    !(has_neg && has_pos)
}

fn item_event_local_position(item: &CanvasItem, scene_position: Point) -> Point {
    let item_origin = item_local_origin(item);
    let local = Point::new(
        scene_position.x - item_origin.x,
        scene_position.y - item_origin.y,
    );
    let [a, b, c, d, e, f] = item
        .style()
        .transform
        .inverse()
        .unwrap_or(CanvasTransform2D::IDENTITY)
        .matrix;
    Point::new(
        a * local.x.get() + c * local.y.get() + e,
        b * local.x.get() + d * local.y.get() + f,
    )
}

fn item_text_hit_at_point(
    item: &CanvasItem,
    scene_position: Point,
    query_session: &CanvasSceneQuerySession<'_>,
) -> Option<CanvasTextHit> {
    let CanvasItem::Text(_) = item else {
        return None;
    };
    let text_hits = query_session.text_hits_for_item(item);
    text_hits
        .iter()
        .find(|entry| hit_geometry_contains(&CanvasHitGeometry::Quad(entry.quad), scene_position))
        .map(|entry| entry.hit)
}

fn export_canvas_scene_json(scene: &CanvasScene) -> String {
    let mut out = String::new();
    out.push_str("{\n");
    out.push_str(&format!(
        "  \"format\": {},\n",
        json_string(CanvasScene::STABLE_JSON_FORMAT)
    ));
    out.push_str(&format!(
        "  \"version\": {},\n",
        CanvasScene::STABLE_JSON_VERSION
    ));
    out.push_str("  \"bounds\": ");
    write_optional_rect_json(&mut out, scene.bounds(), 1);
    out.push_str(",\n  \"items\": [\n");
    for (index, item) in scene.items.iter().enumerate() {
        write_canvas_scene_item_json(&mut out, item, 2);
        if index + 1 != scene.items.len() {
            out.push(',');
        }
        out.push('\n');
    }
    out.push_str("  ]\n}");
    out
}

fn write_canvas_scene_item_json(out: &mut String, item: &CanvasItem, indent: usize) {
    let prefix = "  ".repeat(indent);
    let field_prefix = "  ".repeat(indent + 1);
    out.push_str(&format!("{prefix}{{\n"));
    out.push_str(&format!("{field_prefix}\"id\": {},\n", item.id().get()));
    out.push_str(&format!(
        "{field_prefix}\"name\": {},\n",
        item.name()
            .map(json_string)
            .unwrap_or_else(|| "null".to_string())
    ));
    out.push_str(&format!(
        "{field_prefix}\"kind\": {},\n",
        json_string(canvas_item_kind_name(item.kind()))
    ));
    write_canvas_item_style_json(out, item.style(), indent + 1);
    out.push_str(",\n");

    match item {
        CanvasItem::Path(path) => write_canvas_path_payload_json(out, path, indent + 1),
        CanvasItem::Text(text) => write_canvas_text_payload_json(out, text, indent + 1),
        CanvasItem::Image(image) => write_canvas_image_payload_json(out, image, indent + 1),
        CanvasItem::Group(group) => write_canvas_group_payload_json(out, group, indent + 1),
    }

    out.push('\n');
    out.push_str(&format!("{prefix}}}"));
}

fn write_canvas_item_style_json(out: &mut String, style: &CanvasItemStyle, indent: usize) {
    let prefix = "  ".repeat(indent);
    out.push_str(&format!("{prefix}\"style\": {{\n"));
    let field_prefix = "  ".repeat(indent + 1);
    out.push_str(&format!(
        "{field_prefix}\"transform\": {},\n",
        json_f32_array(&style.transform.matrix)
    ));
    out.push_str(&format!("{field_prefix}\"opacity\": {},\n", style.opacity));
    out.push_str(&format!(
        "{field_prefix}\"blend_mode\": {},\n",
        json_string(canvas_blend_mode_name(style.blend_mode))
    ));
    out.push_str(&format!(
        "{field_prefix}\"isolation\": {},\n",
        style.isolation
    ));
    out.push_str(&format!("{field_prefix}\"visible\": {},\n", style.visible));
    out.push_str(&format!(
        "{field_prefix}\"hit_test\": {},\n",
        style.hit_test
    ));
    out.push_str(&format!("{field_prefix}\"effects\": "));
    write_canvas_effects_json(out, &style.effects, indent + 1);
    out.push_str(&format!("\n{prefix}}}"));
}

fn write_canvas_path_payload_json(out: &mut String, path: &CanvasPath, indent: usize) {
    let prefix = "  ".repeat(indent);
    let field_prefix = "  ".repeat(indent + 1);
    out.push_str(&format!("{prefix}\"payload\": {{\n"));
    out.push_str(&format!(
        "{field_prefix}\"fill_rule\": {},\n",
        json_string(canvas_fill_rule_name(path.fill_rule))
    ));
    out.push_str(&format!("{field_prefix}\"path\": "));
    write_path_builder_json(out, &path.path, indent + 1);
    out.push_str(",\n");
    out.push_str(&format!("{field_prefix}\"fill\": "));
    write_optional_brush_value_json(out, path.fill.as_ref(), indent + 1);
    out.push_str(",\n");
    out.push_str(&format!("{field_prefix}\"stroke\": "));
    write_optional_stroke_json(out, path.stroke.as_ref(), indent + 1);
    out.push_str(",\n");
    out.push_str(&format!("{field_prefix}\"shadow\": "));
    write_optional_shadow_value_json(out, path.shadow.as_ref(), indent + 1);
    out.push_str(&format!("\n{prefix}}}"));
}

fn write_canvas_text_payload_json(out: &mut String, text: &CanvasText, indent: usize) {
    let prefix = "  ".repeat(indent);
    let field_prefix = "  ".repeat(indent + 1);
    out.push_str(&format!("{prefix}\"payload\": {{\n"));
    out.push_str(&format!("{field_prefix}\"frame\": "));
    write_rect_json(out, text.frame, indent + 1);
    out.push_str(",\n");
    out.push_str(&format!("{field_prefix}\"content\": "));
    write_text_content_json(out, &text.content, indent + 1);
    out.push_str(",\n");
    out.push_str(&format!("{field_prefix}\"text_style\": "));
    write_text_style_json(out, &text.text_style, indent + 1);
    out.push_str(",\n");
    out.push_str(&format!("{field_prefix}\"paragraph_style\": "));
    write_paragraph_style_json(out, &text.paragraph_style, indent + 1);
    out.push_str(&format!("\n{prefix}}}"));
}

fn write_canvas_image_payload_json(out: &mut String, image: &CanvasImage, indent: usize) {
    let prefix = "  ".repeat(indent);
    let field_prefix = "  ".repeat(indent + 1);
    out.push_str(&format!("{prefix}\"payload\": {{\n"));
    out.push_str(&format!("{field_prefix}\"frame\": "));
    write_rect_json(out, image.frame, indent + 1);
    out.push_str(",\n");
    out.push_str(&format!("{field_prefix}\"source\": "));
    write_media_source_json(out, &image.source, indent + 1);
    out.push_str(",\n");
    out.push_str(&format!(
        "{field_prefix}\"fit\": {},\n",
        json_string(content_fit_name(image.fit))
    ));
    out.push_str(&format!(
        "{field_prefix}\"corner_radius\": {},\n",
        image.corner_radius.get()
    ));
    out.push_str(&format!("{field_prefix}\"source_rect\": "));
    write_optional_rect_json(out, image.source_rect, indent + 1);
    out.push_str(&format!("\n{prefix}}}"));
}

fn write_canvas_group_payload_json(out: &mut String, group: &CanvasGroup, indent: usize) {
    let prefix = "  ".repeat(indent);
    let field_prefix = "  ".repeat(indent + 1);
    out.push_str(&format!("{prefix}\"payload\": {{\n"));
    out.push_str(&format!(
        "{field_prefix}\"mode\": {},\n",
        json_string(canvas_group_mode_name(&group.mode))
    ));
    out.push_str(&format!("{field_prefix}\"shape\": "));
    write_group_shape_json(out, &group.shape, indent + 1);
    out.push_str(",\n");
    out.push_str(&format!("{field_prefix}\"items\": [\n"));
    for (index, child) in group.items.iter().enumerate() {
        write_canvas_scene_item_json(out, child, indent + 2);
        if index + 1 != group.items.len() {
            out.push(',');
        }
        out.push('\n');
    }
    out.push_str(&format!("{field_prefix}]\n{prefix}}}"));
}

fn write_group_shape_json(out: &mut String, shape: &CanvasGroupShape, indent: usize) {
    match shape {
        CanvasGroupShape::Path { path, fill_rule } => {
            let prefix = "  ".repeat(indent);
            let field_prefix = "  ".repeat(indent + 1);
            out.push_str("{\n");
            out.push_str(&format!("{field_prefix}\"kind\": \"path\",\n"));
            out.push_str(&format!(
                "{field_prefix}\"fill_rule\": {},\n",
                json_string(canvas_fill_rule_name(*fill_rule))
            ));
            out.push_str(&format!("{field_prefix}\"path\": "));
            write_path_builder_json(out, path, indent + 1);
            out.push_str(&format!("\n{prefix}}}"));
        }
    }
}

fn write_path_builder_json(out: &mut String, path: &PathBuilder, indent: usize) {
    let prefix = "  ".repeat(indent);
    let field_prefix = "  ".repeat(indent + 1);
    out.push_str("{\n");
    out.push_str(&format!(
        "{field_prefix}\"fill_rule\": {},\n",
        json_string(canvas_fill_rule_name(path.fill_rule))
    ));
    out.push_str(&format!("{field_prefix}\"commands\": [\n"));
    for (index, command) in path.commands.iter().enumerate() {
        write_path_command_json(out, command, indent + 2);
        if index + 1 != path.commands.len() {
            out.push(',');
        }
        out.push('\n');
    }
    out.push_str(&format!("{field_prefix}]\n{prefix}}}"));
}

fn write_path_command_json(out: &mut String, command: &PathCommand, indent: usize) {
    let prefix = "  ".repeat(indent);
    let field_prefix = "  ".repeat(indent + 1);
    out.push_str(&format!("{prefix}{{\n"));
    match command {
        PathCommand::MoveTo(point_value) => {
            out.push_str(&format!("{field_prefix}\"kind\": \"move_to\",\n"));
            out.push_str(&format!("{field_prefix}\"point\": "));
            write_point_json(out, *point_value, indent + 1);
        }
        PathCommand::LineTo(point_value) => {
            out.push_str(&format!("{field_prefix}\"kind\": \"line_to\",\n"));
            out.push_str(&format!("{field_prefix}\"point\": "));
            write_point_json(out, *point_value, indent + 1);
        }
        PathCommand::QuadTo { ctrl, to } => {
            out.push_str(&format!("{field_prefix}\"kind\": \"quad_to\",\n"));
            out.push_str(&format!("{field_prefix}\"ctrl\": "));
            write_point_json(out, *ctrl, indent + 1);
            out.push_str(",\n");
            out.push_str(&format!("{field_prefix}\"to\": "));
            write_point_json(out, *to, indent + 1);
        }
        PathCommand::CubicTo { ctrl1, ctrl2, to } => {
            out.push_str(&format!("{field_prefix}\"kind\": \"cubic_to\",\n"));
            out.push_str(&format!("{field_prefix}\"ctrl1\": "));
            write_point_json(out, *ctrl1, indent + 1);
            out.push_str(",\n");
            out.push_str(&format!("{field_prefix}\"ctrl2\": "));
            write_point_json(out, *ctrl2, indent + 1);
            out.push_str(",\n");
            out.push_str(&format!("{field_prefix}\"to\": "));
            write_point_json(out, *to, indent + 1);
        }
        PathCommand::Close => {
            out.push_str(&format!("{field_prefix}\"kind\": \"close\"\n"));
            out.push_str(&format!("{prefix}}}"));
            return;
        }
    }
    out.push_str(&format!("\n{prefix}}}"));
}

fn write_optional_brush_value_json(
    out: &mut String,
    brush: Option<&Value<CanvasBrush>>,
    indent: usize,
) {
    match brush {
        Some(Value::Static(brush)) => write_brush_json(out, brush, indent),
        Some(Value::Signal(_)) => out.push_str("{\"kind\":\"dynamic\"}"),
        None => out.push_str("null"),
    }
}

fn write_brush_json(out: &mut String, brush: &CanvasBrush, indent: usize) {
    let prefix = "  ".repeat(indent);
    let field_prefix = "  ".repeat(indent + 1);
    out.push_str("{\n");
    match brush {
        CanvasBrush::Solid(color) => {
            out.push_str(&format!("{field_prefix}\"kind\": \"solid\",\n"));
            out.push_str(&format!(
                "{field_prefix}\"color\": {}",
                json_string(&color_hex(*color))
            ));
        }
        CanvasBrush::LinearGradient(gradient) => {
            out.push_str(&format!("{field_prefix}\"kind\": \"linear_gradient\",\n"));
            out.push_str(&format!("{field_prefix}\"start\": "));
            write_point_json(out, gradient.start, indent + 1);
            out.push_str(",\n");
            out.push_str(&format!("{field_prefix}\"end\": "));
            write_point_json(out, gradient.end, indent + 1);
            out.push_str(",\n");
            out.push_str(&format!("{field_prefix}\"stops\": "));
            write_gradient_stops_json(out, &gradient.stops, indent + 1);
        }
        CanvasBrush::RadialGradient(gradient) => {
            out.push_str(&format!("{field_prefix}\"kind\": \"radial_gradient\",\n"));
            out.push_str(&format!("{field_prefix}\"center\": "));
            write_point_json(out, gradient.center, indent + 1);
            out.push_str(",\n");
            out.push_str(&format!(
                "{field_prefix}\"radius\": {},\n",
                gradient.radius.get()
            ));
            out.push_str(&format!("{field_prefix}\"stops\": "));
            write_gradient_stops_json(out, &gradient.stops, indent + 1);
        }
    }
    out.push_str(&format!("\n{prefix}}}"));
}

fn write_gradient_stops_json(out: &mut String, stops: &[CanvasGradientStop], indent: usize) {
    let prefix = "  ".repeat(indent);
    let field_prefix = "  ".repeat(indent + 1);
    out.push_str("[\n");
    for (index, stop) in stops.iter().enumerate() {
        out.push_str(&format!("{field_prefix}{{\n"));
        out.push_str(&format!("{field_prefix}  \"offset\": {},\n", stop.offset));
        out.push_str(&format!(
            "{field_prefix}  \"color\": {}\n",
            json_string(&color_hex(stop.color))
        ));
        out.push_str(&format!("{field_prefix}}}"));
        if index + 1 != stops.len() {
            out.push(',');
        }
        out.push('\n');
    }
    out.push_str(&format!("{prefix}]"));
}

fn write_optional_stroke_json(out: &mut String, stroke: Option<&CanvasStroke>, indent: usize) {
    let Some(stroke) = stroke else {
        out.push_str("null");
        return;
    };

    let prefix = "  ".repeat(indent);
    let field_prefix = "  ".repeat(indent + 1);
    out.push_str("{\n");
    out.push_str(&format!(
        "{field_prefix}\"width\": {},\n",
        stroke.width.get()
    ));
    out.push_str(&format!("{field_prefix}\"brush\": "));
    match &stroke.brush {
        Value::Static(brush) => write_brush_json(out, brush, indent + 1),
        Value::Signal(_) => out.push_str("{\"kind\":\"dynamic\"}"),
    }
    out.push_str(",\n");
    out.push_str(&format!(
        "{field_prefix}\"dash_pattern\": {},\n",
        stroke
            .dash_pattern
            .as_ref()
            .map(|pattern| json_dp_array(pattern))
            .unwrap_or_else(|| "null".to_string())
    ));
    out.push_str(&format!(
        "{field_prefix}\"dash_offset\": {},\n",
        stroke.dash_offset.get()
    ));
    out.push_str(&format!(
        "{field_prefix}\"line_cap\": {},\n",
        json_string(canvas_stroke_cap_name(stroke.line_cap))
    ));
    out.push_str(&format!(
        "{field_prefix}\"line_join\": {},\n",
        json_string(canvas_stroke_join_name(stroke.line_join))
    ));
    out.push_str(&format!(
        "{field_prefix}\"miter_limit\": {},\n",
        stroke.miter_limit
    ));
    out.push_str(&format!(
        "{field_prefix}\"alignment\": {}\n",
        json_string(canvas_stroke_alignment_name(stroke.alignment))
    ));
    out.push_str(&format!("{prefix}}}"));
}

fn write_optional_shadow_value_json(
    out: &mut String,
    shadow: Option<&Value<CanvasShadow>>,
    indent: usize,
) {
    match shadow {
        Some(Value::Static(shadow)) => {
            let prefix = "  ".repeat(indent);
            let field_prefix = "  ".repeat(indent + 1);
            out.push_str("{\n");
            out.push_str(&format!(
                "{field_prefix}\"color\": {},\n",
                json_string(&color_hex(shadow.color))
            ));
            out.push_str(&format!("{field_prefix}\"offset\": "));
            write_point_json(out, shadow.offset, indent + 1);
            out.push_str(",\n");
            out.push_str(&format!("{field_prefix}\"blur\": {}\n", shadow.blur.get()));
            out.push_str(&format!("{prefix}}}"));
        }
        Some(Value::Signal(_)) => out.push_str("{\"kind\":\"dynamic\"}"),
        None => out.push_str("null"),
    }
}

fn write_canvas_effects_json(out: &mut String, effects: &[CanvasEffect], indent: usize) {
    let prefix = "  ".repeat(indent);
    let field_prefix = "  ".repeat(indent + 1);
    out.push_str("[\n");
    for (index, effect) in effects.iter().enumerate() {
        out.push_str(&format!("{field_prefix}{{\n"));
        match effect {
            CanvasEffect::Blur(radius) => {
                out.push_str(&format!("{field_prefix}  \"kind\": \"blur\",\n"));
                out.push_str(&format!("{field_prefix}  \"radius\": {}\n", radius.get()));
            }
            CanvasEffect::ColorFilter(filter) => {
                out.push_str(&format!("{field_prefix}  \"kind\": \"color_filter\",\n"));
                out.push_str(&format!(
                    "{field_prefix}  \"multiply\": {},\n",
                    json_f32_array(&filter.multiply)
                ));
                out.push_str(&format!(
                    "{field_prefix}  \"add\": {}\n",
                    json_f32_array(&filter.add)
                ));
            }
            CanvasEffect::InnerShadow(shadow) => {
                out.push_str(&format!("{field_prefix}  \"kind\": \"inner_shadow\",\n"));
                out.push_str(&format!(
                    "{field_prefix}  \"color\": {},\n",
                    json_string(&color_hex(shadow.color))
                ));
                out.push_str(&format!("{field_prefix}  \"offset\": "));
                write_point_json(out, shadow.offset, indent + 2);
                out.push_str(",\n");
                out.push_str(&format!(
                    "{field_prefix}  \"blur\": {}\n",
                    shadow.blur.get()
                ));
            }
        }
        out.push_str(&format!("{field_prefix}}}"));
        if index + 1 != effects.len() {
            out.push(',');
        }
        out.push('\n');
    }
    out.push_str(&format!("{prefix}]"));
}

fn write_text_content_json(out: &mut String, content: &CanvasTextContent, indent: usize) {
    match content {
        CanvasTextContent::Plain(text) => {
            out.push_str("{\"kind\":\"plain\",\"text\":");
            out.push_str(&json_string(text));
            out.push('}');
        }
        CanvasTextContent::Rich(spans) => {
            let prefix = "  ".repeat(indent);
            let field_prefix = "  ".repeat(indent + 1);
            out.push_str("{\n");
            out.push_str(&format!("{field_prefix}\"kind\": \"rich\",\n"));
            out.push_str(&format!("{field_prefix}\"spans\": [\n"));
            for (index, span) in spans.iter().enumerate() {
                out.push_str(&format!("{field_prefix}  {{\n"));
                out.push_str(&format!(
                    "{field_prefix}    \"content\": {},\n",
                    json_string(&span.content)
                ));
                out.push_str(&format!("{field_prefix}    \"style\": "));
                write_text_style_json(out, &span.style, indent + 2);
                out.push_str(&format!("\n{field_prefix}  }}"));
                if index + 1 != spans.len() {
                    out.push(',');
                }
                out.push('\n');
            }
            out.push_str(&format!("{field_prefix}]\n{prefix}}}"));
        }
    }
}

fn write_text_style_json(out: &mut String, style: &CanvasTextStyle, indent: usize) {
    let prefix = "  ".repeat(indent);
    let field_prefix = "  ".repeat(indent + 1);
    out.push_str("{\n");
    out.push_str(&format!(
        "{field_prefix}\"font_family\": {},\n",
        style
            .font_family
            .as_deref()
            .map(json_string)
            .unwrap_or_else(|| "null".to_string())
    ));
    out.push_str(&format!(
        "{field_prefix}\"color\": {},\n",
        json_string(&color_hex(style.color))
    ));
    out.push_str(&format!(
        "{field_prefix}\"font_size\": {},\n",
        style.font_size.get()
    ));
    out.push_str(&format!(
        "{field_prefix}\"font_weight\": {},\n",
        style.font_weight.to_raw()
    ));
    out.push_str(&format!(
        "{field_prefix}\"line_height\": {},\n",
        style
            .line_height
            .map(|value| value.get().to_string())
            .unwrap_or_else(|| "null".to_string())
    ));
    out.push_str(&format!(
        "{field_prefix}\"letter_spacing\": {}\n",
        style.letter_spacing.get()
    ));
    out.push_str(&format!("{prefix}}}"));
}

fn write_paragraph_style_json(out: &mut String, style: &CanvasParagraphStyle, indent: usize) {
    let prefix = "  ".repeat(indent);
    let field_prefix = "  ".repeat(indent + 1);
    out.push_str("{\n");
    out.push_str(&format!(
        "{field_prefix}\"wrap\": {},\n",
        json_string(canvas_text_wrap_name(style.wrap))
    ));
    out.push_str(&format!(
        "{field_prefix}\"horizontal_align\": {},\n",
        json_string(canvas_text_horizontal_align_name(style.horizontal_align))
    ));
    out.push_str(&format!(
        "{field_prefix}\"vertical_align\": {},\n",
        json_string(canvas_text_vertical_align_name(style.vertical_align))
    ));
    out.push_str(&format!(
        "{field_prefix}\"overflow\": {}\n",
        json_string(canvas_text_overflow_name(style.overflow))
    ));
    out.push_str(&format!("{prefix}}}"));
}

fn write_media_source_json(out: &mut String, source: &MediaSource, indent: usize) {
    let prefix = "  ".repeat(indent);
    let field_prefix = "  ".repeat(indent + 1);
    out.push_str("{\n");
    match source {
        MediaSource::Path(path) => {
            out.push_str(&format!("{field_prefix}\"kind\": \"path\",\n"));
            out.push_str(&format!(
                "{field_prefix}\"value\": {}\n",
                json_string(&path.to_string_lossy())
            ));
        }
        MediaSource::Url(url) => {
            out.push_str(&format!("{field_prefix}\"kind\": \"url\",\n"));
            out.push_str(&format!("{field_prefix}\"value\": {}\n", json_string(url)));
        }
        MediaSource::Bytes(bytes) => {
            out.push_str(&format!("{field_prefix}\"kind\": \"bytes\",\n"));
            out.push_str(&format!("{field_prefix}\"length\": {},\n", bytes.len()));
            out.push_str(&format!(
                "{field_prefix}\"hex\": {}\n",
                json_string(&hex_bytes(bytes.as_slice()))
            ));
        }
    }
    out.push_str(&format!("{prefix}}}"));
}

fn write_rect_json(out: &mut String, rect: Rect, indent: usize) {
    let prefix = "  ".repeat(indent);
    let field_prefix = "  ".repeat(indent + 1);
    out.push_str("{\n");
    out.push_str(&format!("{field_prefix}\"x\": {},\n", rect.x.get()));
    out.push_str(&format!("{field_prefix}\"y\": {},\n", rect.y.get()));
    out.push_str(&format!("{field_prefix}\"width\": {},\n", rect.width.get()));
    out.push_str(&format!(
        "{field_prefix}\"height\": {}\n",
        rect.height.get()
    ));
    out.push_str(&format!("{prefix}}}"));
}

fn write_point_json(out: &mut String, point_value: Point, indent: usize) {
    let prefix = "  ".repeat(indent);
    let field_prefix = "  ".repeat(indent + 1);
    out.push_str("{\n");
    out.push_str(&format!("{field_prefix}\"x\": {},\n", point_value.x.get()));
    out.push_str(&format!("{field_prefix}\"y\": {}\n", point_value.y.get()));
    out.push_str(&format!("{prefix}}}"));
}

fn json_f32_array(values: &[f32]) -> String {
    let mut out = String::from("[");
    for (index, value) in values.iter().enumerate() {
        if index > 0 {
            out.push_str(", ");
        }
        out.push_str(&value.to_string());
    }
    out.push(']');
    out
}

fn json_dp_array(values: &[Dp]) -> String {
    let mut out = String::from("[");
    for (index, value) in values.iter().enumerate() {
        if index > 0 {
            out.push_str(", ");
        }
        out.push_str(&value.get().to_string());
    }
    out.push(']');
    out
}

fn color_hex(color: Color) -> String {
    format!(
        "#{:02X}{:02X}{:02X}{:02X}",
        color.r, color.g, color.b, color.a
    )
}

fn hex_bytes(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push_str(&format!("{byte:02X}"));
    }
    out
}

fn canvas_item_kind_name(kind: CanvasItemKind) -> &'static str {
    match kind {
        CanvasItemKind::Path => "path",
        CanvasItemKind::Text => "text",
        CanvasItemKind::Image => "image",
        CanvasItemKind::Group => "group",
    }
}

fn canvas_fill_rule_name(fill_rule: CanvasFillRule) -> &'static str {
    match fill_rule {
        CanvasFillRule::NonZero => "non_zero",
        CanvasFillRule::EvenOdd => "even_odd",
    }
}

fn canvas_blend_mode_name(mode: CanvasBlendMode) -> &'static str {
    match mode {
        CanvasBlendMode::Normal => "normal",
        CanvasBlendMode::Multiply => "multiply",
        CanvasBlendMode::Screen => "screen",
        CanvasBlendMode::Overlay => "overlay",
        CanvasBlendMode::Darken => "darken",
        CanvasBlendMode::Lighten => "lighten",
        CanvasBlendMode::ColorDodge => "color_dodge",
        CanvasBlendMode::ColorBurn => "color_burn",
        CanvasBlendMode::HardLight => "hard_light",
        CanvasBlendMode::SoftLight => "soft_light",
        CanvasBlendMode::Difference => "difference",
        CanvasBlendMode::Exclusion => "exclusion",
        CanvasBlendMode::Plus => "plus",
    }
}

fn canvas_stroke_cap_name(cap: CanvasStrokeCap) -> &'static str {
    match cap {
        CanvasStrokeCap::Butt => "butt",
        CanvasStrokeCap::Square => "square",
        CanvasStrokeCap::Round => "round",
    }
}

fn canvas_stroke_join_name(join: CanvasStrokeJoin) -> &'static str {
    match join {
        CanvasStrokeJoin::Miter => "miter",
        CanvasStrokeJoin::Bevel => "bevel",
        CanvasStrokeJoin::Round => "round",
    }
}

fn canvas_stroke_alignment_name(alignment: CanvasStrokeAlignment) -> &'static str {
    match alignment {
        CanvasStrokeAlignment::Center => "center",
        CanvasStrokeAlignment::Inside => "inside",
        CanvasStrokeAlignment::Outside => "outside",
    }
}

fn canvas_group_mode_name(mode: &CanvasGroupMode) -> &'static str {
    match mode {
        CanvasGroupMode::Clip => "clip",
        CanvasGroupMode::Mask => "mask",
    }
}

fn canvas_text_wrap_name(wrap: CanvasTextWrap) -> &'static str {
    match wrap {
        CanvasTextWrap::Word => "word",
        CanvasTextWrap::Glyph => "glyph",
        CanvasTextWrap::None => "none",
    }
}

fn canvas_text_horizontal_align_name(align: CanvasTextHorizontalAlign) -> &'static str {
    match align {
        CanvasTextHorizontalAlign::Start => "start",
        CanvasTextHorizontalAlign::Center => "center",
        CanvasTextHorizontalAlign::End => "end",
    }
}

fn canvas_text_vertical_align_name(align: CanvasTextVerticalAlign) -> &'static str {
    match align {
        CanvasTextVerticalAlign::Start => "start",
        CanvasTextVerticalAlign::Center => "center",
        CanvasTextVerticalAlign::End => "end",
    }
}

fn canvas_text_overflow_name(overflow: CanvasTextOverflow) -> &'static str {
    match overflow {
        CanvasTextOverflow::Clip => "clip",
        CanvasTextOverflow::Ellipsis => "ellipsis",
    }
}

fn content_fit_name(fit: ContentFit) -> &'static str {
    match fit {
        ContentFit::Contain => "contain",
        ContentFit::Cover => "cover",
        ContentFit::Fill => "fill",
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
    effects: Vec<CanvasEffect>,
    isolation: bool,
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
            effects: Vec::new(),
            isolation: false,
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
    group_path: Option<PathBuilder>,
    group_mode: Option<CanvasGroupMode>,
    grouped_items: Vec<CanvasItem>,
}

impl CanvasRecorderFrame {
    fn new(state: CanvasRecorderState) -> Self {
        Self {
            state,
            items: Vec::new(),
            group_path: None,
            group_mode: None,
            grouped_items: Vec::new(),
        }
    }
}

pub struct CanvasRecorder {
    frames: Vec<CanvasRecorderFrame>,
    current_path: PathBuilder,
    next_auto_id: u64,
    pending_item_id: Option<CanvasItemId>,
    pending_item_name: Option<String>,
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
            pending_item_name: None,
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

        let frame = self
            .frames
            .pop()
            .expect("nested recorder frame should exist");
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

    pub fn next_item_name(&mut self, name: impl Into<String>) -> &mut Self {
        self.pending_item_name = Some(name.into());
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
        self.current_path =
            self.current_path
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
        self.begin_group(CanvasGroupMode::Clip)
    }

    pub fn mask(&mut self) -> &mut Self {
        self.begin_group(CanvasGroupMode::Mask)
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

    pub fn push_effect(&mut self, effect: CanvasEffect) -> &mut Self {
        self.current_state_mut().effects.push(effect);
        self
    }

    pub fn set_effects(&mut self, effects: impl Into<Vec<CanvasEffect>>) -> &mut Self {
        self.current_state_mut().effects = effects.into();
        self
    }

    pub fn clear_effects(&mut self) -> &mut Self {
        self.current_state_mut().effects.clear();
        self
    }

    pub fn set_isolation(&mut self, isolation: bool) -> &mut Self {
        self.current_state_mut().isolation = isolation;
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
        let pending_name = self.take_item_name();
        let mut item = CanvasPath::new(
            self.take_item_id(),
            transform_path_builder(&path, self.current_state().transform),
        )
        .fill_rule(path.fill_rule);
        if let Some(name) = pending_name {
            item = item.name_item(name);
        }
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
        let pending_name = self.take_item_name();
        let mut text = CanvasText::new(self.take_item_id(), frame, content)
            .text_style(self.current_state().text_style.clone())
            .paragraph_style(self.current_state().paragraph_style.clone())
            .transform(self.current_state().transform)
            .opacity(self.current_state().opacity)
            .blend_mode(self.current_state().blend_mode)
            .effects(self.current_state().effects.clone())
            .isolation(self.current_state().isolation)
            .visible(self.current_state().visible)
            .hit_test(self.current_state().hit_test);
        if let Some(name) = pending_name {
            text = text.name_item(name);
        }
        if let Some(cursor) = self.current_state().cursor {
            text = text.cursor(cursor);
        }
        self.push_item(CanvasItem::Text(text));
        self
    }

    pub fn draw_rich_text(
        &mut self,
        frame: Rect,
        spans: impl Into<Vec<CanvasTextSpan>>,
    ) -> &mut Self {
        let pending_name = self.take_item_name();
        let mut text = CanvasText::rich(self.take_item_id(), frame, spans)
            .text_style(self.current_state().text_style.clone())
            .paragraph_style(self.current_state().paragraph_style.clone())
            .transform(self.current_state().transform)
            .opacity(self.current_state().opacity)
            .blend_mode(self.current_state().blend_mode)
            .effects(self.current_state().effects.clone())
            .isolation(self.current_state().isolation)
            .visible(self.current_state().visible)
            .hit_test(self.current_state().hit_test);
        if let Some(name) = pending_name {
            text = text.name_item(name);
        }
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
        let pending_name = self.take_item_name();
        let mut image = CanvasImage::new(self.take_item_id(), frame, source)
            .options(options)
            .transform(self.current_state().transform)
            .opacity(self.current_state().opacity)
            .blend_mode(self.current_state().blend_mode)
            .effects(self.current_state().effects.clone())
            .isolation(self.current_state().isolation)
            .visible(self.current_state().visible)
            .hit_test(self.current_state().hit_test);
        if let Some(name) = pending_name {
            image = image.name_item(name);
        }
        if let Some(cursor) = self.current_state().cursor {
            image = image.cursor(cursor);
        }
        self.push_item(CanvasItem::Image(image));
        self
    }

    fn draw_fill_shape(&mut self, path: PathBuilder) -> &mut Self {
        let pending_name = self.take_item_name();
        let mut item = CanvasPath::new(
            self.take_item_id(),
            transform_path_builder(&path, self.current_state().transform),
        )
        .fill_rule(self.current_state().fill_rule);
        if let Some(name) = pending_name {
            item = item.name_item(name);
        }
        if let Some(fill) = self.current_state().fill.clone() {
            item = item.fill(fill);
        }
        self.push_item(self.apply_path_state(item, true));
        self
    }

    fn draw_stroke_shape(&mut self, path: PathBuilder) -> &mut Self {
        let pending_name = self.take_item_name();
        let mut item = CanvasPath::new(
            self.take_item_id(),
            transform_path_builder(&path, self.current_state().transform),
        )
        .fill_rule(self.current_state().fill_rule);
        if let Some(name) = pending_name {
            item = item.name_item(name);
        }
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
            .effects(self.current_state().effects.clone())
            .isolation(self.current_state().isolation)
            .visible(self.current_state().visible)
            .hit_test(self.current_state().hit_test);
        if let Some(cursor) = self.current_state().cursor {
            item = item.cursor(cursor);
        }
        CanvasItem::Path(item)
    }

    fn begin_group(&mut self, mode: CanvasGroupMode) -> &mut Self {
        let path = self.transformed_current_path();
        if path.commands_internal().is_empty() {
            return self;
        }

        let should_flush = {
            let frame = self.current_frame();
            frame.group_path.is_some()
                && !frame.grouped_items.is_empty()
                && frame.group_mode != Some(mode.clone())
        };
        if should_flush {
            let group_item = self.take_pending_group();
            self.current_frame().items.push(group_item);
        }

        let current_mode = self.current_frame().group_mode.clone();
        let current_path = self.current_frame().group_path.clone();
        let new_group_path = match (current_mode, current_path) {
            (Some(CanvasGroupMode::Clip), Some(existing)) if mode == CanvasGroupMode::Clip => {
                existing.intersect(&path).unwrap_or(path.clone())
            }
            (Some(existing_mode), Some(existing)) if existing_mode == mode => {
                existing.intersect(&path).unwrap_or(path.clone())
            }
            _ => path,
        };
        let frame = self.current_frame();
        frame.group_mode = Some(mode);
        frame.group_path = Some(new_group_path);
        self
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
        self.pending_item_id
            .take()
            .unwrap_or_else(|| self.take_generated_id())
    }

    fn take_item_name(&mut self) -> Option<String> {
        self.pending_item_name.take()
    }

    fn take_generated_id(&mut self) -> CanvasItemId {
        let id = CanvasItemId::new(self.next_auto_id);
        self.next_auto_id = self.next_auto_id.saturating_add(1);
        id
    }

    fn push_item(&mut self, item: CanvasItem) {
        let frame = self.current_frame();
        if frame.group_path.is_some() {
            frame.grouped_items.push(item);
        } else {
            frame.items.push(item);
        }
    }

    fn finalize_frame(&mut self, mut frame: CanvasRecorderFrame) -> Vec<CanvasItem> {
        if frame.group_path.is_some() && !frame.grouped_items.is_empty() {
            let group_id = self.take_generated_id();
            let group_path = frame.group_path.take().expect("group path should exist");
            let fill_rule = group_path.fill_rule;
            let mode = frame
                .group_mode
                .take()
                .expect("group mode should exist when group path exists");
            frame.items.push(CanvasItem::Group(CanvasGroup::new(
                group_id,
                mode,
                CanvasGroupShape::Path {
                    path: group_path,
                    fill_rule,
                },
                frame.grouped_items,
            )));
        }
        frame.items
    }

    fn take_pending_group(&mut self) -> CanvasItem {
        let frame = self.current_frame();
        let group_path = frame
            .group_path
            .clone()
            .expect("group path should exist before finalizing a group");
        let grouped_items = std::mem::take(&mut frame.grouped_items);
        let mode = frame
            .group_mode
            .clone()
            .expect("group mode should exist before finalizing a group");
        let fill_rule = group_path.fill_rule;
        CanvasItem::Group(CanvasGroup::new(
            self.take_generated_id(),
            mode,
            CanvasGroupShape::Path {
                path: group_path,
                fill_rule,
            },
            grouped_items,
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

#[derive(Clone)]
pub(crate) enum CanvasHitGeometry {
    Quad([Point; 4]),
    Triangles(Arc<[[Point; 3]]>),
}

#[derive(Clone)]
pub(crate) struct CanvasTextHitEntry {
    pub hit: CanvasTextHit,
    pub quad: [Point; 4],
}

pub(crate) struct CanvasSceneItemRender {
    pub item_id: CanvasItemId,
    pub cursor: Option<CursorStyle>,
    pub hit_bounds: Option<RectBounds>,
    pub hit_geometry: Option<CanvasHitGeometry>,
    pub local_origin: Point,
    pub inverse_transform: CanvasTransform2D,
    pub text_hits: Arc<[CanvasTextHitEntry]>,
    pub output: CanvasRenderOutput,
}

pub(crate) fn tessellate_canvas_scene_items(
    scene: &CanvasScene,
    origin: Point,
    opacity: f32,
    clip_rect: Option<Rect>,
    clip_mask: Option<ClipMask>,
    font_manager: &FontManager,
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
        .map(|item| {
            let output = item.tessellate(origin, opacity, clip, media, units);
            let text_hits = item_text_hits(item, font_manager, origin, units);
            CanvasSceneItemRender {
                item_id: item.id(),
                cursor: item.style().cursor,
                hit_bounds: item.hit_bounds(),
                hit_geometry: item_hit_geometry(item, &output, origin, text_hits.as_ref()),
                local_origin: item_local_origin(item),
                inverse_transform: item
                    .style()
                    .transform
                    .inverse()
                    .unwrap_or(CanvasTransform2D::IDENTITY),
                text_hits,
                output,
            }
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
                .push(RenderCommand::Shape(super::common::RenderPrimitive {
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
                .push(RenderCommand::Brush(super::common::BrushPrimitive {
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
                .push(RenderCommand::Shape(super::common::RenderPrimitive {
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

fn hash_rect(rect: Rect, hasher: &mut impl Hasher) {
    hash_f32(rect.x.get(), hasher);
    hash_f32(rect.y.get(), hasher);
    hash_f32(rect.width.get(), hasher);
    hash_f32(rect.height.get(), hasher);
}

fn hash_f32(value: f32, hasher: &mut impl Hasher) {
    value.to_bits().hash(hasher);
}

fn canvas_text_hit_cache_key(item: &CanvasItem, units: UnitContext) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    item.id().hash(&mut hasher);
    hash_f32(units.scale_factor(), &mut hasher);
    hash_f32(units.resolve_sp(Sp::new(1.0)), &mut hasher);

    if let CanvasItem::Text(text) = item {
        text.plain_text().hash(&mut hasher);
        text.name().hash(&mut hasher);
        hash_rect(text.frame, &mut hasher);
        for value in text.style.transform.matrix {
            hash_f32(value, &mut hasher);
        }
        text.text_style.font_family.hash(&mut hasher);
        text.text_style.color.hash(&mut hasher);
        hash_f32(text.text_style.font_size.get(), &mut hasher);
        text.text_style.font_weight.hash(&mut hasher);
        if let Some(line_height) = text.text_style.line_height {
            1u8.hash(&mut hasher);
            hash_f32(line_height.get(), &mut hasher);
        } else {
            0u8.hash(&mut hasher);
        }
        hash_f32(text.text_style.letter_spacing.get(), &mut hasher);
        canvas_text_wrap_name(text.paragraph_style.wrap).hash(&mut hasher);
        canvas_text_horizontal_align_name(text.paragraph_style.horizontal_align).hash(&mut hasher);
        canvas_text_vertical_align_name(text.paragraph_style.vertical_align).hash(&mut hasher);
        canvas_text_overflow_name(text.paragraph_style.overflow).hash(&mut hasher);
    }

    hasher.finish()
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

#[derive(Clone, Copy)]
struct ResolvedCanvasEffects {
    blur_radius: f32,
    color_filter: Option<CanvasColorFilter>,
    inner_shadow: Option<CanvasInnerShadow>,
}

fn resolve_canvas_effects(effects: &[CanvasEffect]) -> ResolvedCanvasEffects {
    let mut blur_radius: f32 = 0.0;
    let mut color_filter = None;
    let mut inner_shadow = None;
    for effect in effects {
        match effect {
            CanvasEffect::Blur(radius) => {
                blur_radius = blur_radius.max(radius.get().max(0.0));
            }
            CanvasEffect::ColorFilter(filter) => {
                color_filter = Some(*filter);
            }
            CanvasEffect::InnerShadow(shadow) => {
                inner_shadow = Some(*shadow);
            }
        }
    }
    ResolvedCanvasEffects {
        blur_radius,
        color_filter,
        inner_shadow,
    }
}

fn item_local_origin(item: &CanvasItem) -> Point {
    match item {
        CanvasItem::Path(path) => path
            .path
            .control_bounds()
            .map(|bounds| Point::new(bounds.min.x, bounds.min.y))
            .unwrap_or(Point::ZERO),
        CanvasItem::Text(text) => Point::new(text.frame.x, text.frame.y),
        CanvasItem::Image(image) => Point::new(image.frame.x, image.frame.y),
        CanvasItem::Group(group) => group_shape_bounds(&group.shape)
            .map(|bounds| Point::new(bounds.min_x, bounds.min_y))
            .unwrap_or(Point::ZERO),
    }
}

fn item_hit_geometry(
    item: &CanvasItem,
    output: &CanvasRenderOutput,
    origin: Point,
    text_hits: &[CanvasTextHitEntry],
) -> Option<CanvasHitGeometry> {
    match item {
        CanvasItem::Path(_) | CanvasItem::Group(_) => {
            let triangles = output
                .meshes
                .iter()
                .flat_map(|mesh| mesh.triangles.iter().copied())
                .collect::<Vec<_>>();
            (!triangles.is_empty()).then(|| CanvasHitGeometry::Triangles(Arc::from(triangles)))
        }
        CanvasItem::Text(text) => {
            if !text_hits.is_empty() {
                let triangles = text_hits
                    .iter()
                    .flat_map(|entry| {
                        let quad = entry.quad;
                        [[quad[0], quad[1], quad[2]], [quad[0], quad[2], quad[3]]]
                    })
                    .collect::<Vec<_>>();
                return Some(CanvasHitGeometry::Triangles(Arc::from(triangles)));
            }

            output
                .texts
                .first()
                .map(|primitive| {
                    primitive
                        .quad
                        .unwrap_or_else(|| rect_to_quad(primitive.frame))
                })
                .or_else(|| Some(rect_to_quad(offset_rect(text.frame, origin))))
                .map(CanvasHitGeometry::Quad)
        }
        CanvasItem::Image(image) => output
            .textures
            .first()
            .map(|primitive| {
                primitive
                    .quad
                    .unwrap_or_else(|| rect_to_quad(primitive.frame))
            })
            .or_else(|| Some(rect_to_quad(offset_rect(image.frame, origin))))
            .map(CanvasHitGeometry::Quad),
    }
}

fn item_text_hits(
    item: &CanvasItem,
    font_manager: &FontManager,
    origin: Point,
    units: UnitContext,
) -> Arc<[CanvasTextHitEntry]> {
    let CanvasItem::Text(text) = item else {
        return Arc::from([]);
    };
    let content = text.content.plain_text();
    if content.is_empty() {
        return Arc::from([]);
    }

    let line_height = text
        .text_style
        .line_height
        .unwrap_or(Sp::new(text.text_style.font_size.get() * 1.2))
        .get();
    let request = TextFontRequest {
        preferred_font: text.text_style.font_family.as_deref(),
        weight: text.text_style.font_weight,
    };
    let max_width = match text.paragraph_style.wrap {
        CanvasTextWrap::None => None,
        _ => Some(text.frame.width.get().max(0.0)),
    };
    let layout = canvas_text_layout(
        font_manager,
        text,
        &content,
        request,
        line_height,
        max_width,
        units,
    );
    let content_frame = canvas_text_content_frame(text, &layout, origin);
    let mut hits = Vec::new();

    for line_index in 0..layout.line_count() {
        let line_start = layout.line_start(line_index).min(content.len());
        let line_end = layout.line_end(line_index).min(content.len());
        if line_start > line_end {
            continue;
        }
        let line_top = content_frame.y + layout.line_top(line_index);
        let line_height_value = Dp::new(layout.line_height(line_index).max(line_height));
        let line_width = Dp::new(layout.line_width(line_index).max(0.0));

        let mut boundaries = Vec::new();
        let mut cursor = line_start;
        boundaries.push((cursor, layout.x_for_index(cursor)));
        while cursor < line_end {
            let next = next_grapheme_boundary(&content, cursor, line_end);
            boundaries.push((next, layout.x_for_index(next)));
            cursor = next;
        }

        for pair in boundaries.windows(2) {
            let (start, start_x) = pair[0];
            let (end, end_x) = pair[1];
            let width = (end_x - start_x).max(0.0);
            let rect = Rect::new(
                content_frame.x + start_x,
                line_top,
                width.max(1.0),
                line_height_value,
            );
            let quad = if text.style.transform == CanvasTransform2D::IDENTITY {
                rect_to_quad(rect)
            } else {
                transform_rect_quad(rect, text.style.transform, origin)
            };
            hits.push(CanvasTextHitEntry {
                hit: CanvasTextHit {
                    utf8_start: start,
                    utf8_end: end,
                    line_index,
                    line_start,
                    line_end,
                    line_top: Dp::new(layout.line_top(line_index)),
                    line_height: line_height_value,
                    line_width,
                    cluster_bounds: Rect::new(
                        start_x,
                        layout.line_top(line_index),
                        width.max(1.0),
                        line_height_value,
                    ),
                },
                quad,
            });
        }
    }

    Arc::from(hits)
}

fn canvas_text_layout(
    font_manager: &FontManager,
    text: &CanvasText,
    content: &str,
    request: TextFontRequest<'_>,
    line_height: f32,
    max_width: Option<f32>,
    units: UnitContext,
) -> crate::text::font::TextLayoutInfo {
    let font_size = units.resolve_sp(text.text_style.font_size);
    let letter_spacing = units.resolve_sp(text.text_style.letter_spacing);
    match max_width {
        Some(width) => font_manager.measure_text_layout_wrapped(
            content,
            request,
            font_size,
            line_height,
            letter_spacing,
            width,
        ),
        None => font_manager.measure_text_layout(
            content,
            request,
            font_size,
            line_height,
            letter_spacing,
        ),
    }
}

fn canvas_text_content_frame(
    text: &CanvasText,
    layout: &crate::text::font::TextLayoutInfo,
    origin: Point,
) -> Rect {
    let frame = offset_rect(text.frame, origin);
    let width = layout.width.max(0.0).min(frame.width.get());
    let height = layout.height.max(0.0).min(frame.height.get());
    let offset_x = match text.paragraph_style.horizontal_align {
        CanvasTextHorizontalAlign::Start => 0.0,
        CanvasTextHorizontalAlign::Center => (frame.width.get() - width).max(0.0) * 0.5,
        CanvasTextHorizontalAlign::End => (frame.width.get() - width).max(0.0),
    };
    let offset_y = match text.paragraph_style.vertical_align {
        CanvasTextVerticalAlign::Start => 0.0,
        CanvasTextVerticalAlign::Center => (frame.height.get() - height).max(0.0) * 0.5,
        CanvasTextVerticalAlign::End => (frame.height.get() - height).max(0.0),
    };
    Rect::new(frame.x + offset_x, frame.y + offset_y, width, height)
}

fn rect_to_quad(rect: Rect) -> [Point; 4] {
    [
        Point::new(rect.x, rect.y),
        Point::new(rect.right(), rect.y),
        Point::new(rect.right(), rect.bottom()),
        Point::new(rect.x, rect.bottom()),
    ]
}

fn next_grapheme_boundary(text: &str, start: usize, limit: usize) -> usize {
    if start >= limit {
        return limit;
    }
    text[start..limit]
        .grapheme_indices(true)
        .nth(1)
        .map(|(offset, _)| start + offset)
        .unwrap_or(limit)
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
        builder = builder.cubic_to(ctrl1_x, ctrl1_y, ctrl2_x, ctrl2_y, to_x, to_y);
    }

    builder
}

fn normalized_source_rect(
    source_rect: Option<Rect>,
    intrinsic_size: IntrinsicSize,
) -> Option<Rect> {
    let mut rect = source_rect?;
    if intrinsic_size.width <= 0.0 || intrinsic_size.height <= 0.0 {
        return None;
    }

    let min_x = rect.x.get().clamp(0.0, intrinsic_size.width);
    let min_y = rect.y.get().clamp(0.0, intrinsic_size.height);
    let max_x = (rect.x + rect.width)
        .get()
        .clamp(min_x, intrinsic_size.width);
    let max_y = (rect.y + rect.height)
        .get()
        .clamp(min_y, intrinsic_size.height);
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
            .filter(|candidate| {
                *candidate != index && ring_infos[*candidate].abs_area > ring_infos[index].abs_area
            })
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
        let parent_filled = parent
            .map(|value| ring_infos[value].inside_filled)
            .unwrap_or(false);
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
        CanvasColorFilter, CanvasEffect, CanvasFillRule, CanvasGradientStop, CanvasImageOptions,
        CanvasRecorder, CanvasScene, CanvasShadow, CanvasStroke, CanvasTextOverflow,
        CanvasTextSpan, CanvasTextStyle, PathBuilder, PathCommand,
    };
    use crate::foundation::binding::InvalidationSignal;
    use crate::foundation::color::Color;
    use crate::media::{ContentFit, IntrinsicSize, MediaManager, MediaSource};
    use crate::text::font::{FontCatalog, FontManager, FontWeight};
    use crate::ui::layout::Value;
    use crate::ui::unit::dp;
    use crate::ui::unit::UnitContext;
    use crate::ui::widget::{Point, Rect, RenderCommand};

    fn test_media() -> MediaManager {
        MediaManager::new(InvalidationSignal::new())
    }

    fn rendered_items(scene: &CanvasScene) -> Vec<super::CanvasSceneItemRender> {
        let font_manager = FontManager::new(&FontCatalog::default());
        tessellate_canvas_scene_items(
            scene,
            Point::ZERO,
            1.0,
            None,
            None,
            &font_manager,
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
        let item = scene
            .items()
            .first()
            .expect("rounded rect item should exist");
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
        assert!(matches!(
            output.commands[0],
            crate::ui::widget::RenderCommand::Shape(_)
        ));
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
    fn rich_text_records_span_payload() {
        let scene = CanvasRecorder::build(|canvas| {
            canvas.draw_rich_text(
                Rect::new(0.0, 0.0, 120.0, 32.0),
                vec![
                    CanvasTextSpan::new("Hello ").style(CanvasTextStyle {
                        color: Color::WHITE,
                        ..Default::default()
                    }),
                    CanvasTextSpan::new("Canvas").style(CanvasTextStyle {
                        color: Color::hexa(0x38BDF8FF),
                        font_weight: FontWeight::Bold,
                        ..Default::default()
                    }),
                ],
            );
        });
        let rendered = rendered_items(&scene);
        let text = rendered[0]
            .output
            .texts
            .first()
            .expect("text primitive should exist");

        let spans = text.rich_spans.as_ref().expect("rich spans should exist");
        assert_eq!(spans.len(), 2);
        assert_eq!(spans[0].content, "Hello ");
        assert_eq!(spans[1].content, "Canvas");
        assert_eq!(spans[1].font_weight, FontWeight::Bold);
    }

    #[test]
    fn mask_records_composite_mask_commands() {
        let scene = CanvasRecorder::build(|canvas| {
            canvas.save();
            canvas.circle(30.0, 30.0, 20.0).mask();
            canvas.fill_rect(0.0, 0.0, 60.0, 60.0);
            canvas.restore();
        });
        let rendered = rendered_items(&scene);
        let composite = rendered[0]
            .output
            .commands
            .iter()
            .find_map(|command| match command {
                RenderCommand::CanvasComposite(primitive) => Some(primitive),
                _ => None,
            })
            .expect("composite should exist");

        assert!(composite.mask_commands.is_some());
    }

    #[test]
    fn blur_and_color_filter_effects_flow_into_composite() {
        let scene = CanvasRecorder::build(|canvas| {
            canvas
                .set_effects(vec![
                    CanvasEffect::Blur(dp(6.0)),
                    CanvasEffect::ColorFilter(CanvasColorFilter::tint(
                        Color::hexa(0x22C55EFF),
                        0.4,
                    )),
                ])
                .fill_rect(0.0, 0.0, 40.0, 40.0);
        });
        let rendered = rendered_items(&scene);
        let composite = rendered[0]
            .output
            .commands
            .iter()
            .find_map(|command| match command {
                RenderCommand::CanvasComposite(primitive) => Some(primitive),
                _ => None,
            })
            .expect("effect stack should force composite");

        assert!(composite.blur_radius > 0.0);
        assert!(composite.color_filter.is_some());
    }

    #[test]
    fn inner_shadow_effect_flows_into_composite() {
        let scene = CanvasRecorder::build(|canvas| {
            canvas
                .set_effects(vec![CanvasEffect::InnerShadow(
                    super::CanvasInnerShadow::new(
                        Color::hexa(0x111827AA),
                        Point::new(3.0, 4.0),
                        dp(8.0),
                    ),
                )])
                .fill_rect(0.0, 0.0, 40.0, 40.0);
        });
        let rendered = rendered_items(&scene);
        let composite = rendered[0]
            .output
            .commands
            .iter()
            .find_map(|command| match command {
                RenderCommand::CanvasComposite(primitive) => Some(primitive),
                _ => None,
            })
            .expect("effect stack should force composite");

        assert_eq!(composite.inner_shadow_color, Some(Color::hexa(0x111827AA)));
        assert_eq!(composite.inner_shadow_offset, Point::new(3.0, 4.0));
        assert_eq!(composite.inner_shadow_blur_radius, 8.0);
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

    #[test]
    fn canvas_scene_can_query_named_and_nested_items() {
        let scene = CanvasScene::from_items(vec![
            super::CanvasPath::new(1_u64, PathBuilder::new().rect(0.0, 0.0, 10.0, 10.0))
                .name_item("background")
                .into(),
            super::CanvasGroup::new(
                2_u64,
                super::CanvasGroupMode::Clip,
                super::CanvasGroupShape::path(PathBuilder::new().rect(0.0, 0.0, 40.0, 40.0)),
                vec![
                    super::CanvasText::new(3_u64, Rect::new(4.0, 4.0, 20.0, 10.0), "hello")
                        .name_item("label")
                        .into(),
                ],
            )
            .name_item("root-group")
            .into(),
        ]);

        assert!(scene.contains_id(1_u64.into()));
        assert!(scene.contains_name("label"));
        assert_eq!(
            scene.find_named("root-group").map(super::CanvasItem::id),
            Some(2_u64.into())
        );
        assert_eq!(
            scene.find(3_u64.into()).and_then(super::CanvasItem::name),
            Some("label")
        );
    }

    #[test]
    fn canvas_scene_visit_reports_depth_and_paths() {
        let scene = CanvasScene::from_items(vec![super::CanvasGroup::new(
            1_u64,
            super::CanvasGroupMode::Mask,
            super::CanvasGroupShape::path(PathBuilder::new().circle(20.0, 20.0, 20.0)),
            vec![
                super::CanvasPath::new(2_u64, PathBuilder::new().rect(0.0, 0.0, 10.0, 10.0))
                    .name_item("rect")
                    .into(),
            ],
        )
        .into()]);

        let mut visited = Vec::new();
        scene.visit(|entry| {
            visited.push((entry.item.id().get(), entry.depth, entry.index_path));
        });

        assert_eq!(visited.len(), 2);
        assert_eq!(visited[0], (1, 0, vec![0]));
        assert_eq!(visited[1], (2, 1, vec![0, 0]));
    }

    #[test]
    fn canvas_scene_remove_handles_nested_items() {
        let mut scene = CanvasScene::from_items(vec![super::CanvasGroup::new(
            1_u64,
            super::CanvasGroupMode::Clip,
            super::CanvasGroupShape::path(PathBuilder::new().rect(0.0, 0.0, 20.0, 20.0)),
            vec![super::CanvasImage::new(
                2_u64,
                Rect::new(0.0, 0.0, 20.0, 20.0),
                MediaSource::bytes(vec![1, 2, 3]),
            )
            .name_item("thumb")
            .into()],
        )
        .into()]);

        let removed = scene
            .remove(2_u64.into())
            .expect("nested item should be removed");
        assert_eq!(removed.name(), Some("thumb"));
        assert!(!scene.contains_id(2_u64.into()));
    }

    #[test]
    fn canvas_recorder_item_names_are_recorded() {
        let scene = CanvasRecorder::build(|canvas| {
            canvas
                .next_item_name("hero-card")
                .fill_rect(0.0, 0.0, 40.0, 20.0)
                .next_item_name("title")
                .draw_text(Rect::new(0.0, 0.0, 30.0, 10.0), "Hi");
        });

        assert_eq!(
            scene.find_named("hero-card").map(super::CanvasItem::id),
            Some(1_u64.into())
        );
        assert_eq!(
            scene.find_named("title").map(super::CanvasItem::id),
            Some(2_u64.into())
        );
    }

    #[test]
    fn canvas_scene_debug_exports_include_stats_and_names() {
        let scene = CanvasRecorder::build(|canvas| {
            canvas
                .next_item_name("surface")
                .fill_round_rect(0.0, 0.0, 80.0, 40.0, 12.0)
                .next_item_name("caption")
                .draw_text(Rect::new(8.0, 8.0, 60.0, 18.0), "Canvas");
        });

        let debug = scene.debug_info();
        let text = scene.export_debug_text();
        let json = scene.export_debug_json();

        assert_eq!(debug.stats.total_items, 2);
        assert_eq!(debug.stats.named_items, 2);
        assert!(text.contains("surface"));
        assert!(text.contains("caption"));
        assert!(json.contains("\"stats\""));
        assert!(json.contains("\"name\": \"surface\""));
    }

    #[test]
    fn canvas_scene_query_point_and_stable_export_work() {
        let scene = CanvasScene::from_items(vec![super::CanvasGroup::new(
            1_u64,
            super::CanvasGroupMode::Clip,
            super::CanvasGroupShape::path(PathBuilder::new().rect(0.0, 0.0, 100.0, 100.0)),
            vec![
                super::CanvasPath::new(2_u64, PathBuilder::new().rect(10.0, 10.0, 60.0, 40.0))
                    .name_item("card")
                    .fill(Color::WHITE)
                    .into(),
            ],
        )
        .name_item("root")
        .into()]);

        let hit = scene
            .query_point(Point::new(20.0, 20.0))
            .expect("point should hit nested item");
        let all_hits = scene.query_point_all(Point::new(20.0, 20.0));
        let stable = scene.export_json();

        assert_eq!(hit.item_id, 2_u64.into());
        assert_eq!(hit.name.as_deref(), Some("card"));
        assert_eq!(all_hits[0].item_id, 2_u64.into());
        assert!(all_hits.iter().any(|entry| entry.item_id == 1_u64.into()));
        assert!(stable.contains("\"format\": \"tgui.canvas.scene\""));
        assert!(stable.contains("\"version\": 1"));
        assert!(stable.contains("\"kind\": \"group\""));
        assert!(stable.contains("\"name\": \"card\""));
    }

    #[test]
    fn canvas_scene_query_point_returns_text_hit_for_text_items() {
        let scene = CanvasScene::from_items(vec![super::CanvasText::new(
            1_u64,
            Rect::new(0.0, 0.0, 120.0, 32.0),
            "Hello",
        )
        .name_item("label")
        .into()]);

        let hit = scene
            .query_point(Point::new(6.0, 10.0))
            .expect("point should hit text");

        assert_eq!(hit.item_id, 1_u64.into());
        assert!(hit.text_hit.is_some());
        let text_hit = hit.text_hit.expect("text hit should exist");
        assert!(text_hit.utf8_end > text_hit.utf8_start);
    }

    #[test]
    fn stable_export_escapes_control_characters() {
        let scene = CanvasScene::from_items(vec![super::CanvasText::new(
            1_u64,
            Rect::new(0.0, 0.0, 120.0, 32.0),
            "line\u{0001}\u{0008}\u{000C}end",
        )
        .name_item("bad\u{0002}name")
        .into()]);

        let json = scene.export_json();

        assert!(json.contains("bad\\u0002name"));
        assert!(json.contains("line\\u0001\\b\\fend"));
    }

    #[test]
    fn canvas_scene_query_options_drive_explicit_query_context() {
        let scene = CanvasScene::from_items(vec![super::CanvasText::new(
            1_u64,
            Rect::new(0.0, 0.0, 120.0, 32.0),
            "Hello",
        )
        .name_item("label")
        .into()]);
        let options = super::CanvasSceneQueryOptions::new()
            .scale_factor(1.5)
            .font_scale(1.25);

        let hit = scene
            .query_point_with(&options, Point::new(6.0, 10.0))
            .expect("point should hit text with explicit context");
        let all_hits = scene.query_point_all_with(&options, Point::new(6.0, 10.0));

        assert_eq!(hit.item_id, 1_u64.into());
        assert!(hit.text_hit.is_some());
        assert_eq!(all_hits.len(), 1);
    }

    #[test]
    fn runtime_query_context_bridge_reuses_runtime_inputs() {
        let scene = CanvasScene::from_items(vec![super::CanvasText::new(
            1_u64,
            Rect::new(0.0, 0.0, 120.0, 32.0),
            "Hello",
        )
        .name_item("label")
        .into()]);
        let font_manager = FontManager::new(&FontCatalog::default());
        let units = UnitContext::new(1.5, 1.25);

        let hit = scene
            .query_point_with_runtime_context(&font_manager, units, Point::new(6.0, 10.0))
            .expect("point should hit text with runtime context");
        let all_hits =
            scene.query_point_all_with_runtime_context(&font_manager, units, Point::new(6.0, 10.0));

        assert_eq!(hit.item_id, 1_u64.into());
        assert!(hit.text_hit.is_some());
        assert_eq!(all_hits.len(), 1);
    }
}
