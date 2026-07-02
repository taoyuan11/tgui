use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AvatarShape {
    Circle,
    Square,
    Rounded,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AvatarStyle {
    pub background: Value<Color>,
    pub foreground: Value<Color>,
    pub size: Dp,
    pub radius: Dp,
    pub text_style: TextStyle,
    pub group_overlap: Dp,
    pub group_overflow_background: Value<Color>,
}

impl AvatarStyle {
    pub fn default_for_theme(theme: &Theme, shape: AvatarShape) -> Self {
        Self {
            background: Value::Static(theme.colors.surface_low),
            foreground: Value::Static(theme.colors.on_surface_muted),
            size: dp(40.0),
            radius: match shape {
                AvatarShape::Circle => theme.radius.full,
                AvatarShape::Square => Dp::ZERO,
                AvatarShape::Rounded => theme.radius.lg,
            },
            text_style: theme.typography.label.clone(),
            group_overlap: dp(10.0),
            group_overflow_background: Value::Static(theme.colors.surface_high),
        }
    }
}
