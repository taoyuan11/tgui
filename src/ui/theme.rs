use crate::foundation::color::Color;
use crate::platform::window::Theme as WindowTheme;
use crate::ui::unit::{dp, sp, Dp, Sp};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// High-level theme mode selection for the application runtime.
pub enum ThemeMode {
    Light,
    Dark,
    System,
}

#[derive(Clone, Debug, PartialEq)]
/// Complete theme definition used by widgets and the runtime.
pub struct Theme {
    pub palette: Palette,
    pub spacing: Spacing,
    pub typography: Typography,
}

impl Default for Theme {
    fn default() -> Self {
        Self::dark()
    }
}

impl Theme {
    /// Creates the built-in light theme.
    pub fn light() -> Self {
        Self {
            palette: Palette::light(),
            spacing: Spacing::default(),
            typography: Typography::default(),
        }
    }

    /// Creates the built-in dark theme.
    pub fn dark() -> Self {
        Self {
            palette: Palette::dark(),
            spacing: Spacing::default(),
            typography: Typography::default(),
        }
    }

    /// Resolves a concrete theme from a requested mode and the platform theme when needed.
    pub fn from_mode(mode: ThemeMode, system_theme: Option<WindowTheme>) -> Self {
        match mode {
            ThemeMode::Light => Self::light(),
            ThemeMode::Dark => Self::dark(),
            ThemeMode::System => Self::from_window_theme(system_theme),
        }
    }

    pub(crate) fn from_window_theme(theme: Option<WindowTheme>) -> Self {
        match theme.unwrap_or(WindowTheme::Dark) {
            WindowTheme::Light => Self::light(),
            WindowTheme::Dark => Self::dark(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
/// Pair of concrete themes used when resolving [`ThemeMode::Light`],
/// [`ThemeMode::Dark`], and [`ThemeMode::System`].
pub struct ThemeSet {
    pub light: Theme,
    pub dark: Theme,
}

impl Default for ThemeSet {
    fn default() -> Self {
        Self {
            light: Theme::light(),
            dark: Theme::dark(),
        }
    }
}

impl ThemeSet {
    /// Creates a theme set from explicit light and dark themes.
    pub fn new(light: Theme, dark: Theme) -> Self {
        Self { light, dark }
    }

    pub(crate) fn resolve_mode(&self, mode: ThemeMode, system_theme: Option<WindowTheme>) -> Theme {
        match mode {
            ThemeMode::Light => self.light.clone(),
            ThemeMode::Dark => self.dark.clone(),
            ThemeMode::System => self.resolve_window_theme(system_theme),
        }
    }

    pub(crate) fn resolve_window_theme(&self, theme: Option<WindowTheme>) -> Theme {
        match theme.unwrap_or(WindowTheme::Dark) {
            WindowTheme::Light => self.light.clone(),
            WindowTheme::Dark => self.dark.clone(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
/// Color palette used by the built-in widgets.
pub struct Palette {
    pub window_background: Color,
    pub surface: Color,
    pub surface_muted: Color,
    pub accent: Color,
    pub text: Color,
    pub text_muted: Color,
    pub input_background: Color,
}

impl Default for Palette {
    fn default() -> Self {
        Self::dark()
    }
}

impl Palette {
    /// Returns the built-in light palette.
    pub fn light() -> Self {
        Self {
            window_background: Color::hexa(0xF5F7FBFF),
            surface: Color::hexa(0xFFFFFFFF),
            surface_muted: Color::hexa(0xE9EDF5FF),
            accent: Color::hexa(0x2F6FEBFF),
            text: Color::hexa(0x18202AFF),
            text_muted: Color::hexa(0x5C6773E0),
            input_background: Color::hexa(0xFFFFFFFF),
        }
    }

    /// Returns the built-in dark palette.
    pub fn dark() -> Self {
        Self {
            window_background: Color::hexa(0x14171CFF),
            surface: Color::hexa(0x242933FF),
            surface_muted: Color::hexa(0x2E3340FF),
            accent: Color::hexa(0x2E579EEB),
            text: Color::hexa(0xF0F2F7FF),
            text_muted: Color::hexa(0xBAC2CFD9),
            input_background: Color::hexa(0x262933F5),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
/// Spacing scale shared by widgets and layout helpers.
pub struct Spacing {
    pub xs: Dp,
    pub sm: Dp,
    pub md: Dp,
    pub lg: Dp,
}

impl Default for Spacing {
    fn default() -> Self {
        Self {
            xs: dp(4.0),
            sm: dp(8.0),
            md: dp(16.0),
            lg: dp(24.0),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
/// Default typography settings shared by built-in text widgets.
pub struct Typography {
    pub font_family: Option<String>,
    pub font_size: Sp,
}

impl Default for Typography {
    fn default() -> Self {
        Self {
            font_family: None,
            font_size: sp(16.0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Theme, ThemeMode, ThemeSet};
    use crate::foundation::color::Color;
    use crate::platform::window::Theme as WindowTheme;

    fn themed_pair() -> (Theme, Theme) {
        let mut light = Theme::light();
        light.palette.accent = Color::hexa(0x123456FF);
        let mut dark = Theme::dark();
        dark.palette.accent = Color::hexa(0xABCDEF88);
        (light, dark)
    }

    #[test]
    fn default_theme_set_uses_builtin_light_and_dark_themes() {
        let themes = ThemeSet::default();

        assert_eq!(themes.light, Theme::light());
        assert_eq!(themes.dark, Theme::dark());
    }

    #[test]
    fn theme_set_resolves_explicit_light_and_dark_modes() {
        let (light, dark) = themed_pair();
        let themes = ThemeSet::new(light.clone(), dark.clone());

        assert_eq!(themes.resolve_mode(ThemeMode::Light, None), light);
        assert_eq!(themes.resolve_mode(ThemeMode::Dark, None), dark);
    }

    #[test]
    fn theme_set_resolves_system_mode_from_window_theme() {
        let (light, dark) = themed_pair();
        let themes = ThemeSet::new(light.clone(), dark.clone());

        assert_eq!(
            themes.resolve_mode(ThemeMode::System, Some(WindowTheme::Light)),
            light
        );
        assert_eq!(
            themes.resolve_mode(ThemeMode::System, Some(WindowTheme::Dark)),
            dark
        );
    }

    #[test]
    fn theme_set_defaults_unknown_system_theme_to_dark() {
        let (_light, dark) = themed_pair();
        let themes = ThemeSet::new(Theme::light(), dark.clone());

        assert_eq!(themes.resolve_mode(ThemeMode::System, None), dark);
    }
}
