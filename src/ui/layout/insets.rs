use crate::ui::unit::Dp;

/// 表示四个方向边距或内边距的值对象。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Insets {
    pub left: Dp,
    pub top: Dp,
    pub right: Dp,
    pub bottom: Dp,
}

impl Insets {
    /// 全零边距常量。
    pub const ZERO: Self = Self {
        left: Dp::ZERO,
        top: Dp::ZERO,
        right: Dp::ZERO,
        bottom: Dp::ZERO,
    };

    /// 为四个方向设置相同值。
    ///
    /// # 参数
    /// - `value`：四边统一使用的距离值。
    ///
    /// # 返回值
    /// 返回四边一致的 `Insets`。
    pub fn all(value: Dp) -> Self {
        Self {
            left: value,
            top: value,
            right: value,
            bottom: value,
        }
    }

    /// 分别设置水平和垂直方向的值。
    ///
    /// # 参数
    /// - `horizontal`：左右两侧的距离值。
    /// - `vertical`：上下两侧的距离值。
    ///
    /// # 返回值
    /// 返回对称边距的 `Insets`。
    pub fn symmetric(horizontal: Dp, vertical: Dp) -> Self {
        Self {
            left: horizontal,
            right: horizontal,
            top: vertical,
            bottom: vertical,
        }
    }

    /// 仅设置顶部值。
    ///
    /// # 参数
    /// - `value`：顶部距离值。
    ///
    /// # 返回值
    /// 返回仅顶部非零的 `Insets`。
    pub fn top(value: Dp) -> Self {
        Self {
            left: Dp::ZERO,
            top: value,
            right: Dp::ZERO,
            bottom: Dp::ZERO,
        }
    }

    /// 仅设置底部值。
    ///
    /// # 参数
    /// - `value`：底部距离值。
    ///
    /// # 返回值
    /// 返回仅底部非零的 `Insets`。
    pub fn bottom(value: Dp) -> Self {
        Self {
            left: Dp::ZERO,
            top: Dp::ZERO,
            right: Dp::ZERO,
            bottom: value,
        }
    }

    /// 仅设置右侧值。
    ///
    /// # 参数
    /// - `value`：右侧距离值。
    ///
    /// # 返回值
    /// 返回仅右侧非零的 `Insets`。
    pub fn right(value: Dp) -> Self {
        Self {
            left: Dp::ZERO,
            top: Dp::ZERO,
            right: value,
            bottom: Dp::ZERO,
        }
    }

    /// 仅设置左侧值。
    ///
    /// # 参数
    /// - `value`：左侧距离值。
    ///
    /// # 返回值
    /// 返回仅左侧非零的 `Insets`。
    pub fn left(value: Dp) -> Self {
        Self {
            left: value,
            top: Dp::ZERO,
            right: Dp::ZERO,
            bottom: Dp::ZERO,
        }
    }
}

impl Default for Insets {
    fn default() -> Self {
        Self::ZERO
    }
}
