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
        let (padding, gap) = match theme.density {
            Density::Compact => (Insets::all(theme.spacing.sm), theme.spacing.xs),
            Density::Comfortable => (Insets::all(theme.spacing.md), theme.spacing.sm),
            Density::Spacious => (Insets::all(theme.spacing.lg), theme.spacing.md),
        };
        Self {
            surface: WidgetSurfaceStyle::default(),
            background: Value::Static(theme.colors.surface_overlay),
            border: Value::Static(theme.colors.outline_muted),
            border_width: theme.border.thin,
            radius: theme.radius.xl,
            shadow: theme.elevation.lg.clone(),
            padding,
            gap,
        }
    }
}
