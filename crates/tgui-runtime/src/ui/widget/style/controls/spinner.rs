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
        let palette = palette_from_theme(theme);
        Self {
            surface: WidgetSurfaceStyle::default(),
            track_color: Value::Static(palette.surface_low),
            indicator_color: Value::Static(palette.primary),
            size: theme.spacing.lg,
            thickness: theme.border.thick,
            sweep_degrees: 110.0,
            show_track: true,
        }
    }
}
