use super::*;

#[derive(Clone, Debug, PartialEq)]
pub struct SkeletonStyle {
    pub base: Value<Color>,
    pub highlight: Value<Color>,
    pub radius: Dp,
    pub line_height: Dp,
    pub gap: Dp,
}

impl SkeletonStyle {
    pub fn default_for_theme(theme: &Theme) -> Self {
        Self {
            base: Value::Static(theme.colors.surface_high),
            highlight: Value::Static(theme.colors.surface_overlay),
            radius: theme.radius.md,
            line_height: theme.spacing.md,
            gap: theme.spacing.xs,
        }
    }
}
