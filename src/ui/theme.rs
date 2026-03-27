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
    pub window_background: wgpu::Color,
    pub surface: wgpu::Color,
    pub surface_muted: wgpu::Color,
    pub accent: wgpu::Color,
    pub text: wgpu::Color,
    pub text_muted: wgpu::Color,
    pub input_background: wgpu::Color,
}

impl Default for Palette {
    fn default() -> Self {
        Self {
            window_background: wgpu::Color {
                r: 0.08,
                g: 0.09,
                b: 0.11,
                a: 1.0,
            },
            surface: wgpu::Color {
                r: 0.14,
                g: 0.16,
                b: 0.20,
                a: 1.0,
            },
            surface_muted: wgpu::Color {
                r: 0.18,
                g: 0.20,
                b: 0.25,
                a: 1.0,
            },
            accent: wgpu::Color {
                r: 0.18,
                g: 0.34,
                b: 0.62,
                a: 0.92,
            },
            text: wgpu::Color {
                r: 0.94,
                g: 0.95,
                b: 0.97,
                a: 1.0,
            },
            text_muted: wgpu::Color {
                r: 0.73,
                g: 0.76,
                b: 0.81,
                a: 0.85,
            },
            input_background: wgpu::Color {
                r: 0.15,
                g: 0.16,
                b: 0.20,
                a: 0.96,
            },
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
