use super::*;

#[derive(Clone, Debug, PartialEq)]
pub struct CardStyle {
    pub surface: WidgetSurfaceStyle,
    pub background: Value<Color>,
    pub border: Value<Color>,
    pub border_width: Dp,
    pub radius: Dp,
    pub shadow: Shadow,
    pub padding: Insets,
    pub gap: Dp,
}

impl CardStyle {
    pub fn default_for_theme(theme: &Theme) -> Self {
        Self {
            surface: WidgetSurfaceStyle::default(),
            background: Value::Static(theme.colors.surface_overlay),
            border: Value::Static(theme.colors.outline_muted),
            border_width: theme.border.thin,
            radius: theme.radius.xl,
            shadow: theme.elevation.lg.clone(),
            padding: Insets::all(theme.spacing.md),
            gap: theme.spacing.sm,
        }
    }
}
