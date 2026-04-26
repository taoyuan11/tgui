use crate::foundation::color::Color;
use crate::ui::theme::state::{Stateful, WidgetState};
use crate::ui::theme::typography::TextStyle;
use crate::ui::unit::Dp;

#[derive(Clone, Debug, PartialEq)]
pub struct InputTheme {
    pub background: Stateful<Color>,
    pub text: Stateful<Color>,
    pub placeholder: Stateful<Color>,
    pub border: Stateful<Color>,
    pub cursor: Color,
    pub selection: Color,
    pub radius: Dp,
    pub padding_x: Dp,
    pub padding_y: Dp,
    pub min_height: Dp,
    pub text_style: TextStyle,
}

#[derive(Clone, Debug, PartialEq)]
pub struct InputStyle {
    pub background: Color,
    pub text: Color,
    pub placeholder: Color,
    pub border: Color,
    pub cursor: Color,
    pub selection: Color,
    pub radius: Dp,
    pub padding_x: Dp,
    pub padding_y: Dp,
    pub min_height: Dp,
    pub text_style: TextStyle,
}

impl InputTheme {
    pub fn resolve(&self, state: WidgetState) -> InputStyle {
        InputStyle {
            background: self.background.resolve(state),
            text: self.text.resolve(state),
            placeholder: self.placeholder.resolve(state),
            border: self.border.resolve(state),
            cursor: self.cursor,
            selection: self.selection,
            radius: self.radius,
            padding_x: self.padding_x,
            padding_y: self.padding_y,
            min_height: self.min_height,
            text_style: self.text_style.clone(),
        }
    }
}
