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

fn srgb_channel_to_linear_f32(channel: u8) -> f32 {
    let srgb = channel as f32 / 255.0;
    if srgb <= 0.04045 {
        srgb / 12.92
    } else {
        ((srgb + 0.055) / 1.055).powf(2.4)
    }
}

#[cfg(test)]
mod tests {
    use super::Color;

    #[test]
    fn rgb_sets_opaque_alpha() {
        assert_eq!(Color::rgb(12, 34, 56), Color::rgba(12, 34, 56, 255));
    }

    #[test]
    fn hex_decodes_rgb_channels() {
        assert_eq!(Color::hex(0x123456), Color::rgba(0x12, 0x34, 0x56, 0xFF));
    }

    #[test]
    fn hexa_decodes_rgba_channels() {
        assert_eq!(Color::hexa(0x12345678), Color::rgba(0x12, 0x34, 0x56, 0x78));
    }

    #[test]
    fn wgpu_round_trip_preserves_channels() {
        let color = Color::rgba(16, 32, 64, 128);
        let round_trip = Color::from(wgpu::Color::from(color));
        assert_eq!(round_trip, color);
    }

    #[test]
    fn linear_rgba_uses_srgb_decoding_for_rgb_channels() {
        let color = Color::hexa(0xFF4D4FFF);
        let linear = color.to_linear_rgba_f32();

        assert!((linear[0] - 1.0).abs() < 1e-6);
        assert!((linear[1] - 0.07421357).abs() < 1e-6);
        assert!((linear[2] - 0.07818742).abs() < 1e-6);
        assert!((linear[3] - 1.0).abs() < 1e-6);
    }
}
