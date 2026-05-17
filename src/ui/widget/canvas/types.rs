use super::*;

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
    pub(crate) fn to_lyon(self) -> lyon::path::FillRule {
        match self {
            Self::NonZero => lyon::path::FillRule::NonZero,
            Self::EvenOdd => lyon::path::FillRule::EvenOdd,
        }
    }

    pub(crate) fn to_tiny_skia(self) -> tiny_skia::FillRule {
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
