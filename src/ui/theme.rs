use crate::foundation::color::Color;
use crate::platform::window::Theme as WindowTheme;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThemeMode {
    Light,
    Dark,
    System,
}

#[derive(Clone, Debug, PartialEq)]
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
    pub fn light() -> Self {
        Self {
            palette: Palette::light(),
            spacing: Spacing::default(),
            typography: Typography::default(),
        }
    }

    pub fn dark() -> Self {
        Self {
            palette: Palette::dark(),
            spacing: Spacing::default(),
            typography: Typography::default(),
        }
    }

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
pub struct Spacing {
    pub xs: f32,
    pub sm: f32,
    pub md: f32,
    pub lg: f32,
}

impl Default for Spacing {
    fn default() -> Self {
        Self {
            xs: 4.0,
            sm: 8.0,
            md: 16.0,
            lg: 24.0,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Typography {
    pub font_family: Option<String>,
    pub font_size: f32,
}

impl Default for Typography {
    fn default() -> Self {
        Self {
            font_family: None,
            font_size: 16.0,
        }
    }
}
