use super::*;

/// 分隔线样式。
#[derive(Clone, Debug, PartialEq)]
pub struct DividerStyle {
    pub surface: WidgetSurfaceStyle,
    /// 线条颜色。
    pub color: Value<Color>,
    /// 线条粗细（水平时为高度，垂直时为宽度）。
    pub thickness: Value<Dp>,
    /// 虚线单段长度。
    pub dash_length: Dp,
    /// 虚线段间距。
    pub dash_gap: Dp,
    /// 两端内缩（线条沿主轴方向从两端缩进的距离）。
    pub inset: Value<Dp>,
    /// 标签颜色（仅水平分隔线带标签时使用）。
    pub label_color: Value<Color>,
    /// 标签与线条之间的间距。
    pub label_gap: Dp,
    /// 标签文本样式。
    pub text_style: TextStyle,
}

impl DividerStyle {
    pub fn default_for(mode: ResolvedThemeMode) -> Self {
        let palette = palette(mode);
        Self {
            surface: WidgetSurfaceStyle::default(),
            color: Value::Static(palette.outline),
            thickness: Value::Static(dp(1.0)),
            dash_length: dp(4.0),
            dash_gap: dp(3.0),
            inset: Value::Static(dp(0.0)),
            label_color: Value::Static(palette.on_surface),
            label_gap: dp(8.0),
            text_style: label_text_style(),
        }
    }
}
