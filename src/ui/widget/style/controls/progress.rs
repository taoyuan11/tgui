use super::*;

/// 线性进度条样式。
#[derive(Clone, Debug, PartialEq)]
pub struct ProgressBarStyle {
    pub surface: WidgetSurfaceStyle,
    pub track_color: Value<Color>,
    pub fill_color: Value<Color>,
    pub label_color: Value<Color>,
    pub radius: Value<Dp>,
    pub height: Dp,
    pub gap: Dp,
    pub min_width: Dp,
    pub indeterminate_segment_ratio: f32,
    pub text_style: TextStyle,
}

impl ProgressBarStyle {
    pub fn default_for(mode: ResolvedThemeMode) -> Self {
        let palette = palette(mode);
        Self {
            surface: WidgetSurfaceStyle::default(),
            track_color: Value::Static(palette.outline_muted),
            fill_color: Value::Static(palette.primary),
            label_color: Value::Static(palette.on_surface),
            radius: Value::Static(dp(999.0)),
            height: dp(8.0),
            gap: dp(8.0),
            min_width: dp(120.0),
            indeterminate_segment_ratio: 0.34,
            text_style: label_text_style(),
        }
    }
}
