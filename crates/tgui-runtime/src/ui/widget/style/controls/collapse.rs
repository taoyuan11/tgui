use super::*;

#[derive(Clone, Debug, PartialEq)]
pub struct CollapseStyle {
    pub header_background: StateValue<Value<Color>>,
    pub header_foreground: Value<Color>,
    pub panel_background: Value<Color>,
    pub border: Value<Color>,
    pub border_width: Dp,
    pub radius: Dp,
    pub header_min_height: Dp,
    pub header_gap: Dp,
    pub padding: Insets,
    pub gap: Dp,
    pub text_style: TextStyle,
}

impl CollapseStyle {
    pub fn default_for_theme(theme: &Theme) -> Self {
        Self::default_for_density(theme, theme.density)
    }

    pub fn default_for_density(theme: &Theme, density: Density) -> Self {
        let palette = palette_from_theme(theme);
        let (header_min_height, header_gap, padding, radius, gap) = match density {
            Density::Compact => (
                dp(32.0),
                theme.spacing.xs,
                Insets::symmetric(theme.spacing.sm, theme.spacing.xs),
                theme.radius.md,
                theme.spacing.xxs,
            ),
            Density::Comfortable => (
                dp(40.0),
                theme.spacing.sm,
                Insets::symmetric(theme.spacing.sm + theme.spacing.xs, theme.spacing.sm),
                theme.radius.lg,
                theme.spacing.xs,
            ),
            Density::Spacious => (
                dp(48.0),
                theme.spacing.sm + theme.spacing.xs,
                Insets::symmetric(theme.spacing.md, theme.spacing.sm + theme.spacing.xs),
                theme.radius.xl,
                theme.spacing.sm,
            ),
        };
        Self {
            header_background: stateful_colors(
                Color::TRANSPARENT,
                palette.primary_container.with_alpha_factor(0.34),
                palette.primary_container.with_alpha_factor(0.5),
                Color::TRANSPARENT,
            ),
            header_foreground: Value::Static(theme.colors.on_surface),
            panel_background: Value::Static(theme.colors.surface),
            border: Value::Static(theme.colors.outline_muted),
            border_width: theme.border.thin,
            radius,
            header_min_height,
            header_gap,
            padding,
            gap,
            text_style: theme.typography.label.clone(),
        }
    }
}
