use super::*;

#[derive(Clone, Debug, PartialEq)]
pub struct RichTextStyle {
    pub foreground: Value<Color>,
    pub link: Value<Color>,
    pub code_background: Value<Color>,
    pub code_foreground: Value<Color>,
    pub blockquote_border: Value<Color>,
    pub text_style: TextStyle,
    pub code_text_style: TextStyle,
    pub gap: Dp,
}

impl RichTextStyle {
    pub fn default_for_theme(theme: &Theme) -> Self {
        Self {
            foreground: Value::Static(theme.colors.on_surface),
            link: Value::Static(theme.colors.primary),
            code_background: Value::Static(theme.colors.surface_high),
            code_foreground: Value::Static(theme.colors.on_surface),
            blockquote_border: Value::Static(theme.colors.outline),
            text_style: theme.typography.body.clone(),
            code_text_style: theme.typography.code.clone(),
            gap: theme.spacing.sm,
        }
    }
}
