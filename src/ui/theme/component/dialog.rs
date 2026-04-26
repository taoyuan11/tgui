use crate::foundation::color::Color;
use crate::ui::theme::shape::Shadow;
use crate::ui::theme::typography::TextStyle;
use crate::ui::unit::Dp;

#[derive(Clone, Debug, PartialEq)]
pub struct DialogTheme {
    pub background: Color,
    pub scrim: Color,
    pub border_color: Color,
    pub radius: Dp,
    pub shadow: Shadow,
    pub title_style: TextStyle,
    pub body_style: TextStyle,
}
