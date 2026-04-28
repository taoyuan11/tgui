#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub const BLACK: Self = Self::rgb(0, 0, 0);
    pub const WHITE: Self = Self::rgb(255, 255, 255);
    pub const RED: Self = Self::rgb(255, 0, 0);
    pub const GREEN: Self = Self::rgb(0, 255, 0);
    pub const BLUE: Self = Self::rgb(0, 0, 255);
    pub const TRANSPARENT: Self = Self::rgba(0, 0, 0, 0);

    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    pub const fn hex(value: u32) -> Self {
        Self {
            r: ((value >> 16) & 0xFF) as u8,
            g: ((value >> 8) & 0xFF) as u8,
            b: (value & 0xFF) as u8,
            a: 255,
        }
    }

    pub const fn hexa(value: u32) -> Self {
        Self {
            r: ((value >> 24) & 0xFF) as u8,
            g: ((value >> 16) & 0xFF) as u8,
            b: ((value >> 8) & 0xFF) as u8,
            a: (value & 0xFF) as u8,
        }
    }

    pub const fn to_rgba8(self) -> [u8; 4] {
        [self.r, self.g, self.b, self.a]
    }

    pub fn lighten(self, amount: f32) -> Self {
        let amount = amount.clamp(0.0, 1.0);
        Self {
            r: mix_channel(self.r, 255, amount),
            g: mix_channel(self.g, 255, amount),
            b: mix_channel(self.b, 255, amount),
            a: self.a,
        }
    }

    pub fn darken(self, amount: f32) -> Self {
        let amount = amount.clamp(0.0, 1.0);
        Self {
            r: mix_channel(self.r, 0, amount),
            g: mix_channel(self.g, 0, amount),
            b: mix_channel(self.b, 0, amount),
            a: self.a,
        }
    }

    pub(crate) fn to_linear_rgba_f32(self) -> [f32; 4] {
        [
            srgb_channel_to_linear_f32(self.r),
            srgb_channel_to_linear_f32(self.g),
            srgb_channel_to_linear_f32(self.b),
            self.a as f32 / 255.0,
        ]
    }

    pub(crate) fn with_alpha_factor(self, factor: f32) -> Self {
        let alpha = ((self.a as f32) * factor.clamp(0.0, 1.0))
            .round()
            .clamp(0.0, 255.0) as u8;
        Self { a: alpha, ..self }
    }
}

impl Default for Color {
    fn default() -> Self {
        Self::TRANSPARENT
    }
}

impl From<Color> for wgpu::Color {
    fn from(value: Color) -> Self {
        Self {
            r: value.r as f64 / 255.0,
            g: value.g as f64 / 255.0,
            b: value.b as f64 / 255.0,
            a: value.a as f64 / 255.0,
        }
    }
}

impl From<wgpu::Color> for Color {
    fn from(value: wgpu::Color) -> Self {
        Self::rgba(
            float_channel(value.r),
            float_channel(value.g),
            float_channel(value.b),
            float_channel(value.a),
        )
    }
}

fn float_channel(value: f64) -> u8 {
    (value.clamp(0.0, 1.0) * 255.0).round() as u8
}

fn mix_channel(channel: u8, target: u8, amount: f32) -> u8 {
    (channel as f32 + (target as f32 - channel as f32) * amount)
        .round()
        .clamp(0.0, 255.0) as u8
}

fn srgb_channel_to_linear_f32(channel: u8) -> f32 {
    let srgb = channel as f32 / 255.0;
    if srgb <= 0.04045 {
        srgb / 12.92
    } else {
        ((srgb + 0.055) / 1.055).powf(2.4)
    }
}
