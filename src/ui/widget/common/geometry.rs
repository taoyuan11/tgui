use crate::ui::layout::Insets;
use crate::ui::unit::Dp;

/// 表示组件坐标系中的二维点。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Point {
    pub x: Dp,
    pub y: Dp,
}

impl Point {
    pub const ZERO: Self = Self {
        x: Dp::ZERO,
        y: Dp::ZERO,
    };

    pub fn new(x: impl Into<Dp>, y: impl Into<Dp>) -> Self {
        Self {
            x: x.into(),
            y: y.into(),
        }
    }
}

/// 表示组件坐标系中的矩形区域。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x: Dp,
    pub y: Dp,
    pub width: Dp,
    pub height: Dp,
}

impl Rect {
    pub fn new(
        x: impl Into<Dp>,
        y: impl Into<Dp>,
        width: impl Into<Dp>,
        height: impl Into<Dp>,
    ) -> Self {
        Self {
            x: x.into(),
            y: y.into(),
            width: width.into(),
            height: height.into(),
        }
    }

    pub fn contains(self, point: Point) -> bool {
        point.x >= self.x
            && point.x <= self.x + self.width
            && point.y >= self.y
            && point.y <= self.y + self.height
    }

    pub(crate) fn inset(self, insets: Insets) -> Self {
        let width = (self.width - insets.left - insets.right).max(Dp::ZERO);
        let height = (self.height - insets.top - insets.bottom).max(Dp::ZERO);
        Self {
            x: self.x + insets.left,
            y: self.y + insets.top,
            width,
            height,
        }
    }

    pub(crate) fn right(self) -> Dp {
        self.x + self.width
    }

    pub(crate) fn bottom(self) -> Dp {
        self.y + self.height
    }

    pub(crate) fn is_empty(self) -> bool {
        self.width <= Dp::ZERO || self.height <= Dp::ZERO
    }

    pub(crate) fn intersect(self, other: Self) -> Option<Self> {
        let x = self.x.max(other.x);
        let y = self.y.max(other.y);
        let right = self.right().min(other.right());
        let bottom = self.bottom().min(other.bottom());
        let width = right - x;
        let height = bottom - y;
        (width > Dp::ZERO && height > Dp::ZERO).then_some(Self::new(x, y, width, height))
    }

    pub(crate) fn union(self, other: Self) -> Self {
        let x = self.x.min(other.x);
        let y = self.y.min(other.y);
        let right = self.right().max(other.right());
        let bottom = self.bottom().max(other.bottom());
        Self::new(x, y, right - x, bottom - y)
    }
}

pub(crate) fn point_in_triangle(point: Point, a: Point, b: Point, c: Point) -> bool {
    let point = (point.x.get(), point.y.get());
    let a = (a.x.get(), a.y.get());
    let b = (b.x.get(), b.y.get());
    let c = (c.x.get(), c.y.get());

    let sign = |p1: (f32, f32), p2: (f32, f32), p3: (f32, f32)| {
        (p1.0 - p3.0) * (p2.1 - p3.1) - (p2.0 - p3.0) * (p1.1 - p3.1)
    };

    let d1 = sign(point, a, b);
    let d2 = sign(point, b, c);
    let d3 = sign(point, c, a);
    let has_neg = d1 < 0.0 || d2 < 0.0 || d3 < 0.0;
    let has_pos = d1 > 0.0 || d2 > 0.0 || d3 > 0.0;
    !(has_neg && has_pos)
}
