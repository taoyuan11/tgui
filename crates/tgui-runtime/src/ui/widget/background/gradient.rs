use crate::foundation::binding::Signal;
use crate::foundation::color::Color;
use crate::ui::layout::Value;
use crate::ui::unit::Dp;

use super::super::common::Point;

const MAX_BACKGROUND_GRADIENT_STOPS: usize = 7;

/// 背景渐变色标。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BackgroundGradientStop {
    pub offset: f32,
    pub color: Color,
}

impl BackgroundGradientStop {
    /// 创建渐变色标。
    ///
    /// # 参数
    /// - `offset`：色标位置，会按原语义钳制到 `0.0..=1.0`。
    /// - `color`：色标颜色。
    ///
    /// # 返回值
    /// 返回新的渐变色标。
    pub fn new(offset: f32, color: Color) -> Self {
        Self {
            offset: offset.clamp(0.0, 1.0),
            color,
        }
    }
}

/// 线性背景渐变。
#[derive(Clone, Debug, PartialEq)]
pub struct BackgroundLinearGradient {
    pub start: Point,
    pub end: Point,
    pub stops: Vec<BackgroundGradientStop>,
}

impl BackgroundLinearGradient {
    /// 创建线性背景渐变。
    ///
    /// # 参数
    /// - `start`：渐变起点。
    /// - `end`：渐变终点。
    /// - `stops`：渐变色标集合。
    ///
    /// # 返回值
    /// 返回新的线性背景渐变；色标会按原语义排序并截断到上限。
    pub fn new(
        start: impl Into<Point>,
        end: impl Into<Point>,
        stops: impl Into<Vec<BackgroundGradientStop>>,
    ) -> Self {
        Self {
            start: start.into(),
            end: end.into(),
            stops: clamp_background_stops(stops.into()),
        }
    }
}

/// 径向背景渐变。
#[derive(Clone, Debug, PartialEq)]
pub struct BackgroundRadialGradient {
    pub center: Point,
    pub radius: Dp,
    pub stops: Vec<BackgroundGradientStop>,
}

impl BackgroundRadialGradient {
    /// 创建径向背景渐变。
    ///
    /// # 参数
    /// - `center`：渐变中心点。
    /// - `radius`：渐变半径。
    /// - `stops`：渐变色标集合。
    ///
    /// # 返回值
    /// 返回新的径向背景渐变；色标会按原语义排序并截断到上限。
    pub fn new(
        center: impl Into<Point>,
        radius: impl Into<Dp>,
        stops: impl Into<Vec<BackgroundGradientStop>>,
    ) -> Self {
        Self {
            center: center.into(),
            radius: radius.into(),
            stops: clamp_background_stops(stops.into()),
        }
    }
}

/// 背景画刷。
#[derive(Clone, Debug, PartialEq)]
pub enum BackgroundBrush {
    Solid(Color),
    LinearGradient(BackgroundLinearGradient),
    RadialGradient(BackgroundRadialGradient),
}

impl BackgroundBrush {
    /// Applies an inherited surface opacity to every color carried by this brush.
    ///
    /// Widget background brushes are resolved into an owned value before scene collection, so
    /// adjusting the colors in place avoids another gradient-stop allocation. Canvas already
    /// folds its own opacity into the converted brush and therefore does not call this helper.
    pub(crate) fn with_alpha_factor(mut self, opacity: f32) -> Self {
        match &mut self {
            Self::Solid(color) => *color = color.with_alpha_factor(opacity),
            Self::LinearGradient(gradient) => {
                for stop in &mut gradient.stops {
                    stop.color = stop.color.with_alpha_factor(opacity);
                }
            }
            Self::RadialGradient(gradient) => {
                for stop in &mut gradient.stops {
                    stop.color = stop.color.with_alpha_factor(opacity);
                }
            }
        }
        self
    }
}

impl From<Color> for BackgroundBrush {
    fn from(value: Color) -> Self {
        Self::Solid(value)
    }
}

impl From<BackgroundLinearGradient> for BackgroundBrush {
    fn from(value: BackgroundLinearGradient) -> Self {
        Self::LinearGradient(value)
    }
}

impl From<BackgroundRadialGradient> for BackgroundBrush {
    fn from(value: BackgroundRadialGradient) -> Self {
        Self::RadialGradient(value)
    }
}

impl From<Color> for Value<BackgroundBrush> {
    fn from(value: Color) -> Self {
        Value::Static(BackgroundBrush::Solid(value))
    }
}

impl From<BackgroundLinearGradient> for Value<BackgroundBrush> {
    fn from(value: BackgroundLinearGradient) -> Self {
        Value::Static(BackgroundBrush::LinearGradient(value))
    }
}

impl From<BackgroundRadialGradient> for Value<BackgroundBrush> {
    fn from(value: BackgroundRadialGradient) -> Self {
        Value::Static(BackgroundBrush::RadialGradient(value))
    }
}

impl From<Signal<Color>> for Value<BackgroundBrush> {
    fn from(value: Signal<Color>) -> Self {
        Value::Signal(value.map(BackgroundBrush::Solid))
    }
}

impl From<Value<Color>> for Value<BackgroundBrush> {
    fn from(value: Value<Color>) -> Self {
        match value {
            Value::Static(color) => Value::Static(BackgroundBrush::Solid(color)),
            Value::Signal(signal) => Value::Signal(signal.map(BackgroundBrush::Solid)),
        }
    }
}

fn clamp_background_stops(mut stops: Vec<BackgroundGradientStop>) -> Vec<BackgroundGradientStop> {
    if stops.is_empty() {
        return vec![
            BackgroundGradientStop::new(0.0, Color::TRANSPARENT),
            BackgroundGradientStop::new(1.0, Color::TRANSPARENT),
        ];
    }

    stops.sort_by(|left, right| left.offset.total_cmp(&right.offset));
    if stops.len() > MAX_BACKGROUND_GRADIENT_STOPS {
        stops.truncate(MAX_BACKGROUND_GRADIENT_STOPS);
    }
    stops
}
