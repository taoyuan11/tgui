use crate::foundation::color::Color;
use crate::ui::unit::Dp;

use super::{Align, Insets, PositionType, Value};
use crate::ui::layout::Length;

/// 滚动条的视觉样式定义。
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ScrollbarStyle {
    pub thumb_color: Option<Color>,
    pub hover_thumb_color: Option<Color>,
    pub active_thumb_color: Option<Color>,
    pub track_color: Option<Color>,
    pub thickness: Option<Dp>,
    pub radius: Option<Dp>,
    pub insets: Option<Insets>,
    pub min_thumb_length: Option<Dp>,
}

impl ScrollbarStyle {
    /// 设置滚动条滑块颜色。
    ///
    /// # 参数
    /// - `color`：普通状态下的滑块颜色。
    ///
    /// # 返回值
    /// 返回更新后的滚动条样式。
    pub fn thumb_color(mut self, color: Color) -> Self {
        self.thumb_color = Some(color);
        self
    }

    /// 设置滚动条轨道颜色。
    ///
    /// # 参数
    /// - `color`：轨道背景颜色。
    ///
    /// # 返回值
    /// 返回更新后的滚动条样式。
    pub fn track_color(mut self, color: Color) -> Self {
        self.track_color = Some(color);
        self
    }

    /// 设置悬停状态的滑块颜色。
    ///
    /// # 参数
    /// - `color`：悬停时的滑块颜色。
    ///
    /// # 返回值
    /// 返回更新后的滚动条样式。
    pub fn hover_thumb_color(mut self, color: Color) -> Self {
        self.hover_thumb_color = Some(color);
        self
    }

    /// 设置按下状态的滑块颜色。
    ///
    /// # 参数
    /// - `color`：激活时的滑块颜色。
    ///
    /// # 返回值
    /// 返回更新后的滚动条样式。
    pub fn active_thumb_color(mut self, color: Color) -> Self {
        self.active_thumb_color = Some(color);
        self
    }

    /// 设置滚动条厚度。
    ///
    /// # 参数
    /// - `thickness`：滚动条厚度。
    ///
    /// # 返回值
    /// 返回更新后的滚动条样式。
    pub fn thickness(mut self, thickness: Dp) -> Self {
        self.thickness = Some(thickness);
        self
    }

    /// 设置滚动条圆角半径。
    ///
    /// # 参数
    /// - `radius`：圆角半径。
    ///
    /// # 返回值
    /// 返回更新后的滚动条样式。
    pub fn radius(mut self, radius: Dp) -> Self {
        self.radius = Some(radius);
        self
    }

    /// 设置滚动条相对宿主区域的内缩边距。
    ///
    /// # 参数
    /// - `insets`：滚动条边距。
    ///
    /// # 返回值
    /// 返回更新后的滚动条样式。
    pub fn insets(mut self, insets: Insets) -> Self {
        self.insets = Some(insets);
        self
    }

    /// 设置滑块最小长度。
    ///
    /// # 参数
    /// - `min_thumb_length`：滑块最小可见长度。
    ///
    /// # 返回值
    /// 返回更新后的滚动条样式。
    pub fn min_thumb_length(mut self, min_thumb_length: Dp) -> Self {
        self.min_thumb_length = Some(min_thumb_length);
        self
    }
}

/// 通用 widget 的布局样式定义。
#[derive(Clone, PartialEq)]
pub struct LayoutStyle {
    pub width: Option<Value<Length>>,
    pub height: Option<Value<Length>>,
    pub min_width: Option<Value<Length>>,
    pub min_height: Option<Value<Length>>,
    pub max_width: Option<Value<Length>>,
    pub max_height: Option<Value<Length>>,
    pub aspect_ratio: Option<Value<f32>>,
    pub padding: Option<Value<Insets>>,
    pub margin: Value<Insets>,
    pub grow: Value<f32>,
    pub shrink: Value<f32>,
    pub basis: Option<Value<Length>>,
    pub position_type: PositionType,
    pub left: Option<Value<Length>>,
    pub top: Option<Value<Length>>,
    pub right: Option<Value<Length>>,
    pub bottom: Option<Value<Length>>,
    pub align_self: Option<Align>,
    pub justify_self: Option<Align>,
    pub column_start: Option<usize>,
    pub row_start: Option<usize>,
    pub column_span: usize,
    pub row_span: usize,
}

impl Default for LayoutStyle {
    fn default() -> Self {
        Self {
            width: None,
            height: None,
            min_width: None,
            min_height: None,
            max_width: None,
            max_height: None,
            aspect_ratio: None,
            padding: None,
            margin: Value::Static(Insets::ZERO),
            grow: Value::Static(0.0),
            shrink: Value::Static(1.0),
            basis: None,
            position_type: PositionType::Relative,
            left: None,
            top: None,
            right: None,
            bottom: None,
            align_self: None,
            justify_self: None,
            column_start: None,
            row_start: None,
            column_span: 1,
            row_span: 1,
        }
    }
}
