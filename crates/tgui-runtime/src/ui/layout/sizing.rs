use crate::ui::unit::Dp;

/// 表示布局长度的基础单位。
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Length {
    #[default]
    Auto,
    Px(Dp),
    Percent(f32),
}

impl Length {
    /// 自动尺寸常量。
    pub const AUTO: Self = Self::Auto;
}

impl From<Dp> for Length {
    fn from(value: Dp) -> Self {
        Self::Px(value)
    }
}

impl From<f32> for Length {
    fn from(value: f32) -> Self {
        Self::Px(Dp::from(value))
    }
}

impl From<f64> for Length {
    fn from(value: f64) -> Self {
        Self::Px(Dp::from(value))
    }
}

impl From<i32> for Length {
    fn from(value: i32) -> Self {
        Self::Px(Dp::from(value))
    }
}

impl From<u32> for Length {
    fn from(value: u32) -> Self {
        Self::Px(Dp::from(value))
    }
}

/// 表示网格轨道尺寸的类型。
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Track {
    #[default]
    Auto,
    Px(Dp),
    Percent(f32),
    Fr(f32),
}

impl From<Dp> for Track {
    fn from(value: Dp) -> Self {
        Self::Px(value)
    }
}

/// 将百分比数值转换为 `Length::Percent`。
///
/// # 参数
/// - `value`：按 0 到 100 语义传入的百分比值。
///
/// # 返回值
/// 返回布局长度百分比值。
pub const fn pct(value: f32) -> Length {
    Length::Percent(value / 100.0)
}

/// 将分数单位数值转换为 `Track::Fr`。
///
/// # 参数
/// - `value`：网格分数单位值。
///
/// # 返回值
/// 返回网格轨道分数值。
pub const fn fr(value: f32) -> Track {
    Track::Fr(value)
}
