use crate::ui::layout::Insets;
use crate::ui::unit::Dp;

pub(crate) const CLIP_CULL_MARGIN: f32 = 1.0;

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

    #[inline]
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
    #[inline]
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

    /// True when `self` lies entirely outside `clip` (with a 1px margin to stay
    /// conservative against the renderer's sub-pixel scissor rounding). A primitive
    /// for which this holds is scissored to zero pixels by the renderer, so dropping
    /// it from the scene is rendering-identical.
    pub(crate) fn fully_outside(self, clip: Rect) -> bool {
        self.right() < clip.x - CLIP_CULL_MARGIN
            || self.x > clip.right() + CLIP_CULL_MARGIN
            || self.bottom() < clip.y - CLIP_CULL_MARGIN
            || self.y > clip.bottom() + CLIP_CULL_MARGIN
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

#[cfg(test)]
mod tests {
    use super::*;

    fn r(x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect::new(x, y, w, h)
    }

    #[test]
    fn overlapping_rect_is_not_outside() {
        let clip = r(0.0, 0.0, 100.0, 100.0);
        assert!(!r(10.0, 10.0, 20.0, 20.0).fully_outside(clip));
        // Partial overlap on each edge still counts as inside.
        assert!(!r(-10.0, 10.0, 20.0, 20.0).fully_outside(clip));
        assert!(!r(90.0, 10.0, 20.0, 20.0).fully_outside(clip));
        assert!(!r(10.0, -10.0, 20.0, 20.0).fully_outside(clip));
        assert!(!r(10.0, 90.0, 20.0, 20.0).fully_outside(clip));
    }

    #[test]
    fn clearly_separated_rect_is_outside_on_every_side() {
        let clip = r(0.0, 0.0, 100.0, 100.0);
        assert!(r(-50.0, 10.0, 20.0, 20.0).fully_outside(clip)); // left
        assert!(r(150.0, 10.0, 20.0, 20.0).fully_outside(clip)); // right
        assert!(r(10.0, -50.0, 20.0, 20.0).fully_outside(clip)); // above
        assert!(r(10.0, 150.0, 20.0, 20.0).fully_outside(clip)); // below
    }

    #[test]
    fn margin_keeps_rects_within_a_pixel_inside() {
        let clip = r(0.0, 0.0, 100.0, 100.0);
        // A rect ending 0.5px before the clip edge stays inside the 1px margin,
        // so the renderer's sub-pixel scissor rounding can still touch it.
        assert!(!r(-20.0, 10.0, 19.5, 20.0).fully_outside(clip));
        // Pushed clear of the margin it is safe to drop.
        assert!(r(-20.0, 10.0, 18.0, 20.0).fully_outside(clip));
    }

    #[test]
    fn touching_edge_is_not_outside() {
        let clip = r(0.0, 0.0, 100.0, 100.0);
        // Right edge of the rect exactly on the clip's left edge.
        assert!(!r(-20.0, 10.0, 20.0, 20.0).fully_outside(clip));
        // Left edge of the rect exactly on the clip's right edge.
        assert!(!r(100.0, 10.0, 20.0, 20.0).fully_outside(clip));
    }
}
