//! DrawerPlacement —— Drawer 出现的方向。

/// Drawer 从屏幕哪个边缘滑出。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum DrawerPlacement {
    /// 从左边缘滑出。
    #[default]
    Left,
    /// 从右边缘滑出。
    Right,
    /// 从顶部边缘滑出。
    Top,
    /// 从底部边缘滑出。
    Bottom,
}
