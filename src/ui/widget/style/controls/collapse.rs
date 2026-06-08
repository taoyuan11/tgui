use super::*;

#[derive(Clone, Debug, PartialEq)]
pub struct CollapseStyle {
    pub header_background: StateValue<Value<Color>>,
    pub header_foreground: Value<Color>,
    pub panel_background: Value<Color>,
    pub border: Value<Color>,
    pub border_width: Dp,
    pub radius: Dp,
    pub padding: Insets,
    pub gap: Dp,
    pub text_style: TextStyle,
}

impl CollapseStyle {
    pub fn default_for_theme(theme: &Theme) -> Self {
        let palette = palette_from_theme(theme);
        Self {
            header_background: stateful_colors(
                Color::TRANSPARENT,
                palette.surface_high.lighten(surface_hover_lighten()),
                palette.surface_high.darken(surface_hover_lighten()),
                Color::TRANSPARENT,
            ),
            header_foreground: Value::Static(theme.colors.on_surface),
            panel_background: Value::Static(theme.colors.surface),
            border: Value::Static(theme.colors.outline_muted),
            border_width: theme.border.thin,
            radius: theme.radius.lg,
            padding: Insets::all(theme.spacing.md),
            gap: theme.spacing.xs,
            text_style: theme.typography.label.clone(),
        }
    }
}
