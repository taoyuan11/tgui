use crate::ui::unit::{dp, Dp};

#[derive(Clone, Debug, PartialEq)]
pub struct SpaceScale {
    pub xxs: Dp,
    pub xs: Dp,
    pub sm: Dp,
    pub md: Dp,
    pub lg: Dp,
    pub xl: Dp,
    pub xxl: Dp,
}

impl SpaceScale {
    pub fn standard() -> Self {
        Self {
            xxs: dp(2.0),
            xs: dp(4.0),
            sm: dp(8.0),
            md: dp(16.0),
            lg: dp(24.0),
            xl: dp(32.0),
            xxl: dp(40.0),
        }
    }
}

impl Default for SpaceScale {
    fn default() -> Self {
        Self::standard()
    }
}
