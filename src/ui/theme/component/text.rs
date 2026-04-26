use crate::foundation::color::Color;
use crate::ui::theme::typography::TextStyle;

#[derive(Clone, Debug, PartialEq)]
pub struct TextTheme {
    pub default: TextStyle,
    pub muted_color: Color,
    pub primary_color: Color,
}
