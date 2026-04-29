use crate::foundation::color::Color;
use crate::ui::theme::state::{Stateful, WidgetState};
use crate::ui::theme::typography::TextStyle;
use crate::ui::unit::Dp;

#[derive(Clone, Debug, PartialEq)]
pub struct SelectTheme {
    pub background: Stateful<Color>,
    pub text: Stateful<Color>,
    pub placeholder: Stateful<Color>,
    pub border: Stateful<Color>,
    pub arrow: Stateful<Color>,
    pub menu_background: Color,
    pub option_background: Stateful<Color>,
    pub selected_option_background: Color,
    pub border_width: Dp,
    pub radius: Dp,
    pub padding_x: Dp,
    pub padding_y: Dp,
    pub min_height: Dp,
    pub option_height: Dp,
    pub menu_gap: Dp,
    pub text_style: TextStyle,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SelectStyle {
    pub background: Color,
    pub text: Color,
    pub placeholder: Color,
    pub border: Color,
    pub arrow: Color,
    pub menu_background: Color,
    pub option_background: Color,
    pub selected_option_background: Color,
    pub border_width: Dp,
    pub radius: Dp,
    pub padding_x: Dp,
    pub padding_y: Dp,
    pub min_height: Dp,
    pub option_height: Dp,
    pub menu_gap: Dp,
    pub text_style: TextStyle,
}

impl SelectTheme {
    pub fn resolve(&self, state: WidgetState) -> SelectStyle {
        SelectStyle {
            background: self.background.resolve(state),
            text: self.text.resolve(state),
            placeholder: self.placeholder.resolve(state),
            border: self.border.resolve(state),
            arrow: self.arrow.resolve(state),
            menu_background: self.menu_background,
            option_background: self.option_background.resolve(state),
            selected_option_background: self.selected_option_background,
            border_width: self.border_width,
            radius: self.radius,
            padding_x: self.padding_x,
            padding_y: self.padding_y,
            min_height: self.min_height,
            option_height: self.option_height,
            menu_gap: self.menu_gap,
            text_style: self.text_style.clone(),
        }
    }
}
