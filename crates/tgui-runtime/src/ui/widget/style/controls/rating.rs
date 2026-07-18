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
        let (size, gap) = match theme.density {
            Density::Compact => (dp(16.0), dp(2.0)),
            Density::Comfortable => (dp(20.0), dp(4.0)),
            Density::Spacious => (dp(24.0), dp(6.0)),
        };
        Self {
            active: Value::Static(theme.colors.warning),
            inactive: Value::Static(theme.colors.outline_muted),
            hover: Value::Static(theme.colors.warning.lighten(0.12)),
            size,
            gap,
        }
    }
}
