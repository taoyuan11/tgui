use super::*;

#[derive(Clone, Debug, PartialEq)]
pub struct CarouselStyle {
    pub gap: Dp,
    pub indicator_size: Dp,
    pub indicator_gap: Dp,
    pub indicator: Value<Color>,
    pub active_indicator: Value<Color>,
}

impl CarouselStyle {
    pub fn default_for_theme(theme: &Theme) -> Self {
        Self {
            gap: theme.spacing.sm,
            indicator_size: theme.spacing.xs,
            indicator_gap: theme.spacing.xs,
            indicator: Value::Static(theme.colors.outline),
            active_indicator: Value::Static(theme.colors.primary),
        }
    }
}
