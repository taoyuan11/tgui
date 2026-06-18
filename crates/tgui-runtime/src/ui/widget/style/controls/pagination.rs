use super::*;

#[derive(Clone, Debug, PartialEq)]
pub struct PaginationStyle {
    pub gap: Dp,
    pub page_width: Dp,
    pub jump_width: Dp,
    pub text_style: TextStyle,
}

impl PaginationStyle {
    pub fn default_for_theme(theme: &Theme) -> Self {
        Self {
            gap: theme.spacing.xs,
            page_width: theme.spacing.xxl,
            jump_width: dp(96.0),
            text_style: theme.typography.label.clone(),
        }
    }
}
