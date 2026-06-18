use crate::ui::unit::Dp;
use crate::ui::widget::common::{Point, Rect};

/// 浮层锚点。`Rect` 用于挂在某个 widget 边框上的浮层；`Point` 用于挂在某个屏幕点（光标 / 命中点）。
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Anchor {
    /// 矩形锚点（典型场景：trigger widget 的当前 frame）。
    Rect(Rect),
    /// 点锚点（典型场景：右键菜单跟随光标）。
    Point(Point),
}

impl Anchor {
    /// 转成统一的锚点矩形。`Point` 视作零尺寸矩形。
    pub fn to_rect(self) -> Rect {
        match self {
            Anchor::Rect(rect) => rect,
            Anchor::Point(p) => Rect::new(p.x, p.y, Dp::ZERO, Dp::ZERO),
        }
    }
}

impl From<Rect> for Anchor {
    fn from(rect: Rect) -> Self {
        Anchor::Rect(rect)
    }
}

impl From<Point> for Anchor {
    fn from(point: Point) -> Self {
        Anchor::Point(point)
    }
}
