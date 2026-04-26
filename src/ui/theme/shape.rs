use crate::foundation::color::Color;
use crate::ui::unit::{dp, Dp};

#[derive(Clone, Debug, PartialEq)]
pub struct RadiusScale {
    pub none: Dp,
    pub sm: Dp,
    pub md: Dp,
    pub lg: Dp,
    pub xl: Dp,
    pub full: Dp,
}

impl Default for RadiusScale {
    fn default() -> Self {
        Self {
            none: Dp::ZERO,
            sm: dp(4.0),
            md: dp(8.0),
            lg: dp(12.0),
            xl: dp(16.0),
            full: dp(999.0),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct BorderScale {
    pub none: Dp,
    pub thin: Dp,
    pub normal: Dp,
    pub thick: Dp,
}

impl Default for BorderScale {
    fn default() -> Self {
        Self {
            none: Dp::ZERO,
            thin: dp(1.0),
            normal: dp(1.5),
            thick: dp(2.0),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Shadow {
    pub offset_x: Dp,
    pub offset_y: Dp,
    pub blur: Dp,
    pub spread: Dp,
    pub color: Color,
}

impl Default for Shadow {
    fn default() -> Self {
        Self {
            offset_x: Dp::ZERO,
            offset_y: Dp::ZERO,
            blur: Dp::ZERO,
            spread: Dp::ZERO,
            color: Color::TRANSPARENT,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ElevationScale {
    pub none: Shadow,
    pub sm: Shadow,
    pub md: Shadow,
    pub lg: Shadow,
}

impl Default for ElevationScale {
    fn default() -> Self {
        Self {
            none: Shadow::default(),
            sm: Shadow {
                offset_x: Dp::ZERO,
                offset_y: dp(2.0),
                blur: dp(8.0),
                spread: Dp::ZERO,
                color: Color::hexa(0x00000024),
            },
            md: Shadow {
                offset_x: Dp::ZERO,
                offset_y: dp(8.0),
                blur: dp(24.0),
                spread: dp(-4.0),
                color: Color::hexa(0x00000033),
            },
            lg: Shadow {
                offset_x: Dp::ZERO,
                offset_y: dp(14.0),
                blur: dp(36.0),
                spread: dp(-6.0),
                color: Color::hexa(0x00000040),
            },
        }
    }
}
