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
    pub fn default_for_theme(theme: &Theme) -> Self {
        let palette = palette_from_theme(theme);
        Self {
            surface: WidgetSurfaceStyle::default(),
            color: Value::Static(palette.outline_muted),
            thickness: Value::Static(theme.border.thin),
            dash_length: theme.spacing.xs,
            dash_gap: theme.spacing.xs,
            inset: Value::Static(Dp::ZERO),
            label_color: Value::Static(palette.on_surface_muted),
            label_gap: theme.spacing.sm,
            text_style: theme.typography.label.clone(),
        }
    }
}
