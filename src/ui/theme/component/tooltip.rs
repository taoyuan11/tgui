use crate::foundation::color::Color;
use crate::ui::theme::typography::TextStyle;
use crate::ui::unit::Dp;

#[derive(Clone, Debug, PartialEq)]
pub struct TooltipTheme {
    pub background: Color,
    pub text: Color,
    pub radius: Dp,
    pub padding_x: Dp,
    pub padding_y: Dp,
    pub text_style: TextStyle,
}
