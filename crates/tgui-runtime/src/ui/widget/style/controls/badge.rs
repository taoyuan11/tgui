use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BadgeTone {
    Neutral,
    Primary,
    Success,
    Warning,
    Error,
    Info,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BadgeStyle {
    pub background: Value<Color>,
    pub foreground: Value<Color>,
    pub dot_size: Dp,
    pub min_height: Dp,
    pub padding_x: Dp,
    pub radius: Dp,
    pub text_style: TextStyle,
}

impl BadgeStyle {
    pub fn default_for_theme(theme: &Theme, tone: BadgeTone) -> Self {
        Self::default_for_density(theme, theme.density, tone)
    }

    pub fn default_for_density(theme: &Theme, density: Density, tone: BadgeTone) -> Self {
        let (background, foreground) = tone_colors(theme, tone);
        let (dot_size, min_height, padding_x) = match density {
            Density::Compact => (
                theme.spacing.sm - theme.spacing.xxs,
                theme.spacing.md,
                theme.spacing.xs,
            ),
            Density::Comfortable => (
                theme.spacing.sm,
                theme.spacing.md + theme.spacing.xs,
                theme.spacing.sm - theme.spacing.xxs,
            ),
            Density::Spacious => (
                theme.spacing.sm + theme.spacing.xxs,
                theme.spacing.lg,
                theme.spacing.sm,
            ),
        };
        Self {
            background: Value::Static(background),
            foreground: Value::Static(foreground),
            dot_size,
            min_height,
            padding_x,
            radius: theme.radius.full,
            text_style: theme.typography.label_small.clone(),
        }
    }
}

fn tone_colors(theme: &Theme, tone: BadgeTone) -> (Color, Color) {
    match tone {
        BadgeTone::Neutral => (theme.colors.surface_high, theme.colors.on_surface),
        BadgeTone::Primary => (theme.colors.primary, theme.colors.on_primary),
        BadgeTone::Success => (theme.colors.success, theme.colors.on_success),
        BadgeTone::Warning => (theme.colors.warning, theme.colors.on_warning),
        BadgeTone::Error => (theme.colors.error, theme.colors.on_error),
        BadgeTone::Info => (
            theme.colors.primary_container,
            theme.colors.on_primary_container,
        ),
    }
}
