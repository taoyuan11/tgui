use crate::foundation::color::Color;

#[derive(Clone, Debug)]
pub struct Theme {
    pub palette: Palette,
    pub spacing: Spacing,
    pub typography: Typography,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            palette: Palette::default(),
            spacing: Spacing::default(),
            typography: Typography::default(),
        }
    }
}

#[derive(Clone, Debug)]
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

#[derive(Clone, Debug)]
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

#[derive(Clone, Debug)]
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
