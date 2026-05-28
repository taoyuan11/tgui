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
    pub fn default_for(mode: ResolvedThemeMode) -> Self {
        let palette = palette(mode);
        Self {
            surface: WidgetSurfaceStyle::default(),
            track_color: Value::Static(palette.outline_muted.with_alpha_factor(0.55)),
            indicator_color: Value::Static(palette.primary),
            size: dp(20.0),
            thickness: dp(3.0),
            sweep_degrees: 110.0,
            show_track: true,
        }
    }
}
