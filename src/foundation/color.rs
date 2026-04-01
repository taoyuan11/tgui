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
}
