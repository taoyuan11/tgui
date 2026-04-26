use crate::foundation::color::Color;
use crate::ui::theme::shape::Shadow;
use crate::ui::unit::Dp;

#[derive(Clone, Debug, PartialEq)]
pub struct PanelTheme {
    pub background: Color,
    pub border_color: Color,
    pub border_width: Dp,
    pub radius: Dp,
    pub shadow: Shadow,
}
