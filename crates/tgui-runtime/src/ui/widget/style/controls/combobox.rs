use super::*;

#[derive(Clone, Debug, PartialEq)]
pub struct ComboboxStyle {
    pub width: Dp,
    pub menu_width: Dp,
    pub option_height: Dp,
    pub max_visible_options: usize,
    pub highlight: Value<Color>,
    pub empty_foreground: Value<Color>,
}

impl ComboboxStyle {
    pub fn default_for_theme(theme: &Theme) -> Self {
        Self::default_for_density(theme, theme.density)
    }

    pub fn default_for_density(theme: &Theme, density: Density) -> Self {
        let (width, option_height) = match density {
            Density::Compact => (dp(240.0), dp(32.0)),
            Density::Comfortable => (dp(260.0), dp(40.0)),
            Density::Spacious => (dp(288.0), dp(48.0)),
        };
        Self {
            width,
            menu_width: width,
            option_height,
            max_visible_options: 6,
            highlight: Value::Static(theme.colors.primary_container.with_alpha_factor(0.72)),
            empty_foreground: Value::Static(theme.colors.on_surface_muted),
        }
    }
}
