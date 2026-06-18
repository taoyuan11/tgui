use super::*;

#[derive(Clone, Debug, PartialEq)]
pub struct IconStyle {
    pub color: Value<Color>,
    pub size: Dp,
}

impl IconStyle {
    pub fn default_for_theme(theme: &Theme) -> Self {
        Self {
            color: Value::Static(theme.colors.on_surface),
            size: theme.spacing.md,
        }
    }
}
