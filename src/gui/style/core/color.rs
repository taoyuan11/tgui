#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: f32, // 透明度范围 0.0 (完全透明) 到 1.0 (完全不透明)
}

impl Color {
    /// 基础构造：从 RGB 创建（默认不透明）
    pub fn from_rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 1.0 }
    }

    /// 核心构造：从 RGBA 创建
    pub fn from_rgba(r: u8, g: u8, b: u8, a: f32) -> Self {
        Self { r, g, b, a: a.clamp(0.0, 1.0) }
    }

    /// 从 Hex 字符串创建，支持 #RGB, #RRGGBB, #RRGGBBAA
    pub fn from_hex(hex: &str) -> Self {
        let hex = hex.trim_start_matches('#');
        match hex.len() {
            6 => {
                let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0);
                let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0);
                let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0);
                Self::from_rgb(r, g, b)
            }
            8 => {
                let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0);
                let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0);
                let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0);
                let a = u8::from_str_radix(&hex[6..8], 16).unwrap_or(255) as f32 / 255.0;
                Self::from_rgba(r, g, b, a)
            }
            _ => Self::BLACK,
        }
    }

    /// 兼容 u32 (0xRRGGBB)
    pub fn from_u32(value: u32) -> Self {
        let r = ((value >> 16) & 0xFF) as u8;
        let g = ((value >> 8) & 0xFF) as u8;
        let b = (value & 0xFF) as u8;
        Self::from_rgb(r, g, b)
    }

    /// 转换为不带 Alpha 的 u32，用于 blend 函数的输入
    pub fn to_u32_no_alpha(&self) -> u32 {
        (self.r as u32) << 16 | (self.g as u32) << 8 | (self.b as u32)
    }

    /// 转换为 u64
    pub fn to_u64(&self) -> u64 {
        (self.r as u64) << 32 | (self.g as u64) << 16 | (self.b as u64) << 8 | (self.a as u64)
    }

    /// 获取当前颜色的透明度分量 (0.0 - 1.0)
    pub fn alpha(&self) -> f32 {
        self.a
    }

    pub fn with_alpha_mult(&self, mult: f32) -> Self {
        let mut new_color = *self;
        new_color.a = (self.a * mult).clamp(0.0, 1.0);
        new_color
    }


    // --- 预设颜色 ---
    pub const TRANSPARENT: Self = Self { r: 0, g: 0, b: 0, a: 0.0 };
    pub const BLACK: Self = Self { r: 0, g: 0, b: 0, a: 1.0 };
    pub const WHITE: Self = Self { r: 255, g: 255, b: 255, a: 1.0 };
}