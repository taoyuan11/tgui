use super::*;

/// Spinner 样式。
#[derive(Clone, Debug, PartialEq)]
pub struct SpinnerStyle {
    pub surface: WidgetSurfaceStyle,
    pub track_color: Value<Color>,
    pub indicator_color: Value<Color>,
    pub size: Dp,
    pub thickness: Dp,
    pub sweep_degrees: f32,
    pub show_track: bool,
}

impl SpinnerStyle {
    pub fn default_for_theme(theme: &Theme) -> Self {
        Self::default_for_density(theme, theme.density)
    }

    pub fn default_for_density(theme: &Theme, density: Density) -> Self {
        let palette = palette_from_theme(theme);
        let (size, thickness) = match density {
            Density::Compact => (dp(16.0), dp(2.0)),
            Density::Comfortable => (dp(20.0), dp(2.0)),
            Density::Spacious => (dp(24.0), dp(3.0)),
        };
        Self {
            surface: WidgetSurfaceStyle::default(),
            track_color: Value::Static(palette.outline_muted.with_alpha_factor(0.72)),
            indicator_color: Value::Static(palette.primary),
            size,
            thickness,
            sweep_degrees: 104.0,
            show_track: true,
        }
    }
}
