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
    pub fn default_for_theme(theme: &Theme) -> Self {
        Self::default_for_density(theme, theme.density)
    }

    pub fn default_for_density(theme: &Theme, density: Density) -> Self {
        let palette = palette_from_theme(theme);
        let (height, gap, min_width) = match density {
            Density::Compact => (dp(4.0), theme.spacing.sm - theme.spacing.xxs, dp(96.0)),
            Density::Comfortable => (dp(6.0), theme.spacing.sm, dp(120.0)),
            Density::Spacious => (dp(8.0), theme.spacing.sm + theme.spacing.xs, dp(144.0)),
        };
        Self {
            surface: WidgetSurfaceStyle::default(),
            track_color: Value::Static(palette.surface_high),
            fill_color: Value::Static(palette.primary),
            label_color: Value::Static(palette.on_surface_muted),
            radius: Value::Static(theme.radius.full),
            height,
            gap,
            min_width,
            indeterminate_segment_ratio: 0.34,
            text_style: theme.typography.label.clone(),
        }
    }
}
