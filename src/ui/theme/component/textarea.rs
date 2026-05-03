use crate::foundation::color::Color;
use crate::ui::theme::state::{Stateful, WidgetState};
use crate::ui::theme::typography::TextStyle;
use crate::ui::unit::Dp;

use super::ScrollbarTheme;

#[derive(Clone, Debug, PartialEq)]
pub struct TextAreaTheme {
    pub background: Stateful<Color>,
    pub text: Stateful<Color>,
    pub placeholder: Stateful<Color>,
    pub border: Stateful<Color>,
    pub cursor: Color,
    pub selection: Color,
    pub scroll_background: Color,
    pub scroll_shadow: Color,
    pub radius: Dp,
    pub padding_x: Dp,
    pub padding_y: Dp,
    pub min_height: Dp,
    pub text_style: TextStyle,
    pub scrollbar: Option<ScrollbarTheme>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TextAreaStyle {
    pub background: Color,
    pub text: Color,
    pub placeholder: Color,
    pub border: Color,
    pub cursor: Color,
    pub selection: Color,
    pub scroll_background: Color,
    pub scroll_shadow: Color,
    pub radius: Dp,
    pub padding_x: Dp,
    pub padding_y: Dp,
    pub min_height: Dp,
    pub text_style: TextStyle,
    pub scrollbar: Option<ScrollbarTheme>,
}

impl TextAreaTheme {
    pub fn resolve(&self, state: WidgetState) -> TextAreaStyle {
        TextAreaStyle {
            background: self.background.resolve(state),
            text: self.text.resolve(state),
            placeholder: self.placeholder.resolve(state),
            border: self.border.resolve(state),
            cursor: self.cursor,
            selection: self.selection,
            scroll_background: self.scroll_background,
            scroll_shadow: self.scroll_shadow,
            radius: self.radius,
            padding_x: self.padding_x,
            padding_y: self.padding_y,
            min_height: self.min_height,
            text_style: self.text_style.clone(),
            scrollbar: self.scrollbar.clone(),
        }
    }
}
