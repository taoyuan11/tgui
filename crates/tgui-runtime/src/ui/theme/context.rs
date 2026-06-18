use super::{ResolvedThemeMode, Theme};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Density {
    Compact,
    #[default]
    Comfortable,
    Spacious,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ControlSize {
    Small,
    #[default]
    Medium,
    Large,
}

#[derive(Clone, Copy)]
pub struct StyleContext<'a> {
    pub theme: &'a Theme,
    pub mode: ResolvedThemeMode,
    pub density: Density,
    pub reduced_motion: bool,
    pub text_scale: f32,
}

impl<'a> StyleContext<'a> {
    pub fn new(theme: &'a Theme, mode: ResolvedThemeMode) -> Self {
        Self {
            theme,
            mode,
            density: theme.density,
            reduced_motion: false,
            text_scale: 1.0,
        }
    }

    pub fn from_theme(theme: &'a Theme) -> Self {
        Self::new(theme, theme.mode)
    }

    pub fn with_reduced_motion(mut self, reduced_motion: bool) -> Self {
        self.reduced_motion = reduced_motion;
        self
    }

    pub fn with_text_scale(mut self, text_scale: f32) -> Self {
        self.text_scale = text_scale.max(0.1);
        self
    }

    pub fn with_density(mut self, density: Density) -> Self {
        self.density = density;
        self
    }
}
