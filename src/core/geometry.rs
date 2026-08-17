use std::error::Error as StdError;
use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GeometryErrorKind {
    NonFinite,
    Negative,
    NonPositive,
    OutOfRange,
    Singular,
}

/// Validation failure for a geometry boundary.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GeometryError {
    kind: GeometryErrorKind,
    field: &'static str,
    value: f64,
}

impl GeometryError {
    const fn new(kind: GeometryErrorKind, field: &'static str, value: f64) -> Self {
        Self { kind, field, value }
    }

    pub const fn field(self) -> &'static str {
        self.field
    }

    pub const fn value(self) -> f64 {
        self.value
    }
}

impl fmt::Display for GeometryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let constraint = match self.kind {
            GeometryErrorKind::NonFinite => "must be finite",
            GeometryErrorKind::Negative => "must be non-negative",
            GeometryErrorKind::NonPositive => "must be greater than zero",
            GeometryErrorKind::OutOfRange => "is outside the supported range",
            GeometryErrorKind::Singular => "produces a singular transform",
        };
        write!(
            formatter,
            "{} {constraint} (got {})",
            self.field, self.value
        )
    }
}

impl StdError for GeometryError {}

fn finite(field: &'static str, value: f32) -> Result<(), GeometryError> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(GeometryError::new(
            GeometryErrorKind::NonFinite,
            field,
            f64::from(value),
        ))
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    pub const ZERO: Self = Self::new(0.0, 0.0);

    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    pub fn try_new(x: f32, y: f32) -> Result<Self, GeometryError> {
        let point = Self::new(x, y);
        point.validate()?;
        Ok(point)
    }

    pub fn validate(self) -> Result<(), GeometryError> {
        finite("point.x", self.x)?;
        finite("point.y", self.y)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Size {
    pub width: f32,
    pub height: f32,
}

impl Size {
    pub const ZERO: Self = Self::new(0.0, 0.0);

    pub const fn new(width: f32, height: f32) -> Self {
        Self { width, height }
    }

    pub fn try_new(width: f32, height: f32) -> Result<Self, GeometryError> {
        let size = Self::new(width, height);
        size.validate()?;
        Ok(size)
    }

    pub fn validate(self) -> Result<(), GeometryError> {
        finite("size.width", self.width)?;
        finite("size.height", self.height)?;
        if self.width < 0.0 {
            return Err(GeometryError::new(
                GeometryErrorKind::Negative,
                "size.width",
                f64::from(self.width),
            ));
        }
        if self.height < 0.0 {
            return Err(GeometryError::new(
                GeometryErrorKind::Negative,
                "size.height",
                f64::from(self.height),
            ));
        }
        Ok(())
    }

    pub const fn is_empty(self) -> bool {
        self.width <= 0.0 || self.height <= 0.0
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub origin: Point,
    pub size: Size,
}

impl Rect {
    pub const ZERO: Self = Self::new(Point::ZERO, Size::ZERO);

    pub const fn new(origin: Point, size: Size) -> Self {
        Self { origin, size }
    }

    pub const fn from_xywh(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self::new(Point::new(x, y), Size::new(width, height))
    }

    pub fn try_from_xywh(x: f32, y: f32, width: f32, height: f32) -> Result<Self, GeometryError> {
        let rect = Self::from_xywh(x, y, width, height);
        rect.validate()?;
        Ok(rect)
    }

    pub fn validate(self) -> Result<(), GeometryError> {
        self.origin.validate()?;
        self.size.validate()?;
        finite("rect.max_x", self.max_x())?;
        finite("rect.max_y", self.max_y())
    }

    pub const fn min_x(self) -> f32 {
        self.origin.x
    }

    pub const fn min_y(self) -> f32 {
        self.origin.y
    }

    pub fn max_x(self) -> f32 {
        self.origin.x + self.size.width
    }

    pub fn max_y(self) -> f32 {
        self.origin.y + self.size.height
    }

    /// Half-open containment, matching pixel and layout interval semantics.
    pub fn contains(self, point: Point) -> bool {
        !self.size.is_empty()
            && point.x >= self.min_x()
            && point.y >= self.min_y()
            && point.x < self.max_x()
            && point.y < self.max_y()
    }

    pub fn intersection(self, other: Self) -> Option<Self> {
        let min_x = self.min_x().max(other.min_x());
        let min_y = self.min_y().max(other.min_y());
        let max_x = self.max_x().min(other.max_x());
        let max_y = self.max_y().min(other.max_y());
        if max_x <= min_x || max_y <= min_y {
            None
        } else {
            Some(Self::from_xywh(min_x, min_y, max_x - min_x, max_y - min_y))
        }
    }

    pub fn union(self, other: Self) -> Self {
        let min_x = self.min_x().min(other.min_x());
        let min_y = self.min_y().min(other.min_y());
        let max_x = self.max_x().max(other.max_x());
        let max_y = self.max_y().max(other.max_y());
        Self::from_xywh(min_x, min_y, max_x - min_x, max_y - min_y)
    }
}

/// Affine 2D transform in column-vector form.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transform2D {
    pub m11: f32,
    pub m12: f32,
    pub m21: f32,
    pub m22: f32,
    pub tx: f32,
    pub ty: f32,
}

impl Transform2D {
    pub const IDENTITY: Self = Self {
        m11: 1.0,
        m12: 0.0,
        m21: 0.0,
        m22: 1.0,
        tx: 0.0,
        ty: 0.0,
    };

    pub const fn translation(x: f32, y: f32) -> Self {
        Self {
            tx: x,
            ty: y,
            ..Self::IDENTITY
        }
    }

    pub const fn scale(x: f32, y: f32) -> Self {
        Self {
            m11: x,
            m22: y,
            ..Self::IDENTITY
        }
    }

    pub fn rotation(radians: f32) -> Self {
        let (sin, cos) = radians.sin_cos();
        Self {
            m11: cos,
            m12: sin,
            m21: -sin,
            m22: cos,
            tx: 0.0,
            ty: 0.0,
        }
    }

    pub fn validate(self) -> Result<(), GeometryError> {
        for (field, value) in [
            ("transform.m11", self.m11),
            ("transform.m12", self.m12),
            ("transform.m21", self.m21),
            ("transform.m22", self.m22),
            ("transform.tx", self.tx),
            ("transform.ty", self.ty),
        ] {
            finite(field, value)?;
        }
        Ok(())
    }

    pub fn transform_point(self, point: Point) -> Point {
        Point::new(
            self.m11 * point.x + self.m21 * point.y + self.tx,
            self.m12 * point.x + self.m22 * point.y + self.ty,
        )
    }

    /// Returns a transform that applies `self`, then `next`.
    pub fn then(self, next: Self) -> Self {
        Self {
            m11: next.m11 * self.m11 + next.m21 * self.m12,
            m12: next.m12 * self.m11 + next.m22 * self.m12,
            m21: next.m11 * self.m21 + next.m21 * self.m22,
            m22: next.m12 * self.m21 + next.m22 * self.m22,
            tx: next.m11 * self.tx + next.m21 * self.ty + next.tx,
            ty: next.m12 * self.tx + next.m22 * self.ty + next.ty,
        }
    }

    pub fn inverse(self) -> Result<Self, GeometryError> {
        self.validate()?;
        let determinant = self.m11 * self.m22 - self.m12 * self.m21;
        if !determinant.is_finite() || determinant.abs() <= f32::EPSILON {
            return Err(GeometryError::new(
                GeometryErrorKind::Singular,
                "transform.determinant",
                f64::from(determinant),
            ));
        }
        let inverse = 1.0 / determinant;
        Ok(Self {
            m11: self.m22 * inverse,
            m12: -self.m12 * inverse,
            m21: -self.m21 * inverse,
            m22: self.m11 * inverse,
            tx: (self.m21 * self.ty - self.m22 * self.tx) * inverse,
            ty: (self.m12 * self.tx - self.m11 * self.ty) * inverse,
        })
    }
}

impl Default for Transform2D {
    fn default() -> Self {
        Self::IDENTITY
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CornerRadii {
    pub top_left: f32,
    pub top_right: f32,
    pub bottom_right: f32,
    pub bottom_left: f32,
}

impl CornerRadii {
    pub const ZERO: Self = Self::all(0.0);

    pub const fn all(radius: f32) -> Self {
        Self {
            top_left: radius,
            top_right: radius,
            bottom_right: radius,
            bottom_left: radius,
        }
    }

    pub const fn new(top_left: f32, top_right: f32, bottom_right: f32, bottom_left: f32) -> Self {
        Self {
            top_left,
            top_right,
            bottom_right,
            bottom_left,
        }
    }

    pub fn validate(self) -> Result<(), GeometryError> {
        for (field, value) in [
            ("radii.top_left", self.top_left),
            ("radii.top_right", self.top_right),
            ("radii.bottom_right", self.bottom_right),
            ("radii.bottom_left", self.bottom_left),
        ] {
            finite(field, value)?;
            if value < 0.0 {
                return Err(GeometryError::new(
                    GeometryErrorKind::Negative,
                    field,
                    f64::from(value),
                ));
            }
        }
        Ok(())
    }

    pub fn clamped_to(self, size: Size) -> Self {
        let limit = (size.width.min(size.height) * 0.5).max(0.0);
        Self::new(
            self.top_left.clamp(0.0, limit),
            self.top_right.clamp(0.0, limit),
            self.bottom_right.clamp(0.0, limit),
            self.bottom_left.clamp(0.0, limit),
        )
    }
}

impl Default for CornerRadii {
    fn default() -> Self {
        Self::ZERO
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Clip {
    Rect(Rect),
    RoundedRect { rect: Rect, radii: CornerRadii },
}

impl Clip {
    pub fn validate(self) -> Result<(), GeometryError> {
        match self {
            Self::Rect(rect) => rect.validate(),
            Self::RoundedRect { rect, radii } => {
                rect.validate()?;
                radii.validate()
            }
        }
    }

    pub const fn bounds(self) -> Rect {
        match self {
            Self::Rect(rect) | Self::RoundedRect { rect, .. } => rect,
        }
    }
}

/// sRGB color with validation-free byte channels.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Color {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
    pub alpha: u8,
}

impl Color {
    pub const TRANSPARENT: Self = Self::rgba8(0, 0, 0, 0);
    pub const BLACK: Self = Self::rgb8(0, 0, 0);
    pub const WHITE: Self = Self::rgb8(255, 255, 255);

    pub const fn rgb8(red: u8, green: u8, blue: u8) -> Self {
        Self::rgba8(red, green, blue, 255)
    }

    pub const fn rgba8(red: u8, green: u8, blue: u8, alpha: u8) -> Self {
        Self {
            red,
            green,
            blue,
            alpha,
        }
    }

    pub fn rgba(red: f32, green: f32, blue: f32, alpha: f32) -> Result<Self, GeometryError> {
        let channels = [
            ("color.red", red),
            ("color.green", green),
            ("color.blue", blue),
            ("color.alpha", alpha),
        ];
        for (field, value) in channels {
            finite(field, value)?;
            if !(0.0..=1.0).contains(&value) {
                return Err(GeometryError::new(
                    GeometryErrorKind::OutOfRange,
                    field,
                    f64::from(value),
                ));
            }
        }
        Ok(Self::rgba8(
            (red * 255.0).round() as u8,
            (green * 255.0).round() as u8,
            (blue * 255.0).round() as u8,
            (alpha * 255.0).round() as u8,
        ))
    }
}

impl Default for Color {
    fn default() -> Self {
        Self::TRANSPARENT
    }
}

/// Strictly positive logical-to-physical scale.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct DpiScale(f64);

impl DpiScale {
    pub const ONE: Self = Self(1.0);

    pub fn new(scale: f64) -> Result<Self, GeometryError> {
        if !scale.is_finite() {
            return Err(GeometryError::new(
                GeometryErrorKind::NonFinite,
                "dpi_scale",
                scale,
            ));
        }
        if scale <= 0.0 {
            return Err(GeometryError::new(
                GeometryErrorKind::NonPositive,
                "dpi_scale",
                scale,
            ));
        }
        Ok(Self(scale))
    }

    pub const fn get(self) -> f64 {
        self.0
    }

    pub fn logical_to_physical(self, logical: f32) -> Result<u32, GeometryError> {
        finite("logical_pixels", logical)?;
        let physical = f64::from(logical) * self.0;
        if physical < 0.0 || physical > f64::from(u32::MAX) {
            return Err(GeometryError::new(
                GeometryErrorKind::OutOfRange,
                "physical_pixels",
                physical,
            ));
        }
        Ok(physical.round() as u32)
    }

    pub fn physical_to_logical(self, physical: u32) -> f32 {
        (f64::from(physical) / self.0) as f32
    }
}

impl Default for DpiScale {
    fn default() -> Self {
        Self::ONE
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f32, b: f32) {
        assert!((a - b).abs() < 1.0e-4, "{a} != {b}");
    }

    #[test]
    fn invalid_boundaries_are_rejected() {
        assert!(Size::try_new(-1.0, 2.0).is_err());
        assert!(Point::try_new(f32::NAN, 0.0).is_err());
        assert!(Rect::try_from_xywh(f32::MAX, 0.0, f32::MAX, 1.0).is_err());
        assert!(DpiScale::new(0.0).is_err());
        assert!(Color::rgba(1.1, 0.0, 0.0, 1.0).is_err());
    }

    #[test]
    fn rect_intersection_and_half_open_hit_test_are_defined() {
        let a = Rect::from_xywh(0.0, 0.0, 10.0, 10.0);
        let b = Rect::from_xywh(5.0, 2.0, 10.0, 3.0);
        assert_eq!(a.intersection(b), Some(Rect::from_xywh(5.0, 2.0, 5.0, 3.0)));
        assert!(a.contains(Point::new(0.0, 0.0)));
        assert!(!a.contains(Point::new(10.0, 5.0)));
    }

    #[test]
    fn transform_inverse_round_trips() {
        let transform = Transform2D::translation(10.0, -3.0)
            .then(Transform2D::rotation(0.4))
            .then(Transform2D::scale(2.0, 3.0));
        let point = Point::new(8.0, 5.0);
        let transformed = transform.transform_point(point);
        let restored = transform.inverse().unwrap().transform_point(transformed);
        close(point.x, restored.x);
        close(point.y, restored.y);
        assert!(Transform2D::scale(0.0, 1.0).inverse().is_err());
    }

    #[test]
    fn dpi_conversion_is_explicit_and_checked() {
        let scale = DpiScale::new(1.5).unwrap();
        assert_eq!(scale.logical_to_physical(3.0), Ok(5));
        close(scale.physical_to_logical(6), 4.0);
    }
}
