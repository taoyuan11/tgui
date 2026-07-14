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
        Self::default_for_density(theme, theme.density)
    }

    pub fn default_for_density(theme: &Theme, density: Density) -> Self {
        let gap = match density {
            Density::Compact => theme.spacing.xs,
            Density::Comfortable => theme.spacing.sm - theme.spacing.xxs,
            Density::Spacious => theme.spacing.sm,
        };
        Self {
            foreground: Value::Static(theme.colors.on_surface_muted),
            current_foreground: Value::Static(theme.colors.on_surface),
            separator: Value::Static(theme.colors.outline),
            gap,
            text_style: theme.typography.label.clone(),
        }
    }
}
