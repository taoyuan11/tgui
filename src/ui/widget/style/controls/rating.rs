use super::*;

#[derive(Clone, Debug, PartialEq)]
pub struct RatingStyle {
    pub active: Value<Color>,
    pub inactive: Value<Color>,
    pub hover: Value<Color>,
    pub size: Dp,
    pub gap: Dp,
}

impl RatingStyle {
    pub fn default_for_theme(theme: &Theme) -> Self {
        Self {
            active: Value::Static(theme.colors.warning),
            inactive: Value::Static(theme.colors.outline_muted),
            hover: Value::Static(theme.colors.warning.lighten(0.12)),
            size: theme.spacing.lg,
            gap: theme.spacing.xxs,
        }
    }
}
