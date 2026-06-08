use super::*;

#[derive(Clone, Debug, PartialEq)]
pub struct BreadcrumbStyle {
    pub foreground: Value<Color>,
    pub current_foreground: Value<Color>,
    pub separator: Value<Color>,
    pub gap: Dp,
    pub text_style: TextStyle,
}

impl BreadcrumbStyle {
    pub fn default_for_theme(theme: &Theme) -> Self {
        Self {
            foreground: Value::Static(theme.colors.primary),
            current_foreground: Value::Static(theme.colors.on_surface),
            separator: Value::Static(theme.colors.on_surface_muted),
            gap: theme.spacing.xs,
            text_style: theme.typography.label.clone(),
        }
    }
}
