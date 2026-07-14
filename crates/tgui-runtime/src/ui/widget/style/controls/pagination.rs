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
        Self::default_for_density(theme, theme.density)
    }

    pub fn default_for_density(theme: &Theme, density: Density) -> Self {
        let (gap, page_width, jump_width) = match density {
            Density::Compact => (theme.spacing.xs, dp(32.0), dp(80.0)),
            Density::Comfortable => (theme.spacing.sm - theme.spacing.xxs, dp(40.0), dp(96.0)),
            Density::Spacious => (theme.spacing.sm, dp(48.0), dp(112.0)),
        };
        Self {
            gap,
            page_width,
            jump_width,
            text_style: theme.typography.label.clone(),
        }
    }
}
