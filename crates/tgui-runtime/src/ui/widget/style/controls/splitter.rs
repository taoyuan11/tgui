use super::*;

#[derive(Clone, Debug, PartialEq)]
pub struct SplitterStyle {
    pub handle_color: StateValue<Value<Color>>,
    pub handle_thickness: Dp,
    pub hit_extent: Dp,
    pub gap: Dp,
}

impl SplitterStyle {
    pub fn default_for_theme(theme: &Theme) -> Self {
        Self {
            handle_color: stateful_colors(
                theme.colors.outline_muted,
                theme.colors.primary.with_alpha_factor(0.72),
                theme.colors.primary,
                theme.colors.disabled,
            ),
            handle_thickness: theme.border.normal,
            hit_extent: theme.spacing.md,
            gap: Dp::ZERO,
        }
    }
}
