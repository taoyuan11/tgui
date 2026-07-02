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
            md: dp(6.0),
            lg: dp(8.0),
            xl: dp(12.0),
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
    pub xl: Shadow,
}

impl Default for ElevationScale {
    fn default() -> Self {
        Self::light()
    }
}

impl ElevationScale {
    pub fn light() -> Self {
        Self {
            none: Shadow::default(),
            sm: Shadow {
                offset_x: Dp::ZERO,
                offset_y: dp(1.0),
                blur: dp(2.0),
                spread: Dp::ZERO,
                color: Color::hexa(0x09090B0D),
            },
            md: Shadow {
                offset_x: Dp::ZERO,
                offset_y: dp(4.0),
                blur: dp(6.0),
                spread: dp(-1.0),
                color: Color::hexa(0x09090B1A),
            },
            lg: Shadow {
                offset_x: Dp::ZERO,
                offset_y: dp(10.0),
                blur: dp(15.0),
                spread: dp(-3.0),
                color: Color::hexa(0x09090B1A),
            },
            xl: Shadow {
                offset_x: Dp::ZERO,
                offset_y: dp(20.0),
                blur: dp(25.0),
                spread: dp(-5.0),
                color: Color::hexa(0x09090B1A),
            },
        }
    }

    pub fn dark() -> Self {
        Self {
            none: Shadow::default(),
            sm: Shadow {
                offset_x: Dp::ZERO,
                offset_y: dp(1.0),
                blur: dp(2.0),
                spread: Dp::ZERO,
                color: Color::hexa(0x00000080),
            },
            md: Shadow {
                offset_x: Dp::ZERO,
                offset_y: dp(4.0),
                blur: dp(6.0),
                spread: dp(-1.0),
                color: Color::hexa(0x00000080),
            },
            lg: Shadow {
                offset_x: Dp::ZERO,
                offset_y: dp(10.0),
                blur: dp(15.0),
                spread: dp(-3.0),
                color: Color::hexa(0x00000080),
            },
            xl: Shadow {
                offset_x: Dp::ZERO,
                offset_y: dp(20.0),
                blur: dp(25.0),
                spread: dp(-5.0),
                color: Color::hexa(0x000000A6),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn radius_scale_matches_neutral_token_table() {
        let radius = RadiusScale::default();
        assert_eq!(radius.none, dp(0.0));
        assert_eq!(radius.sm, dp(4.0));
        assert_eq!(radius.md, dp(6.0));
        assert_eq!(radius.lg, dp(8.0));
        assert_eq!(radius.xl, dp(12.0));
        assert_eq!(radius.full, dp(999.0));
    }

    #[test]
    fn elevation_scale_exposes_xl_shadow_for_dialog_layers() {
        let light = ElevationScale::light();
        assert_eq!(light.sm.offset_y, dp(1.0));
        assert_eq!(light.sm.blur, dp(2.0));
        assert_eq!(light.sm.color, Color::hexa(0x09090B0D));
        assert_eq!(light.xl.offset_y, dp(20.0));
        assert_eq!(light.xl.blur, dp(25.0));
        assert_eq!(light.xl.spread, dp(-5.0));
        assert_eq!(light.xl.color, Color::hexa(0x09090B1A));

        let dark = ElevationScale::dark();
        assert_eq!(dark.md.color, Color::hexa(0x00000080));
        assert_eq!(dark.xl.color, Color::hexa(0x000000A6));
    }
}
