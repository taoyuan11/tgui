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
        Self {
            width: dp(260.0),
            menu_width: dp(260.0),
            option_height: theme.spacing.xl,
            max_visible_options: 6,
            highlight: Value::Static(theme.colors.primary_container.with_alpha_factor(0.72)),
            empty_foreground: Value::Static(theme.colors.on_surface_muted),
        }
    }
}
