use crate::foundation::color::Color;
use crate::ui::theme::state::{Stateful, WidgetState};
use crate::ui::theme::typography::TextStyle;
use crate::ui::unit::Dp;

#[derive(Clone, Debug, PartialEq)]
pub struct ButtonTheme {
    pub primary: ButtonVariant,
    pub secondary: ButtonVariant,
    pub ghost: ButtonVariant,
    pub danger: ButtonVariant,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ButtonVariant {
    pub container: Stateful<Color>,
    pub content: Stateful<Color>,
    pub border: Stateful<Color>,
    pub border_width: Dp,
    pub radius: Dp,
    pub padding_x: Dp,
    pub padding_y: Dp,
    pub min_height: Dp,
    pub text_style: TextStyle,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ButtonStyle {
    pub background: Color,
    pub foreground: Color,
    pub border_color: Color,
    pub border_width: Dp,
    pub radius: Dp,
    pub padding_x: Dp,
    pub padding_y: Dp,
    pub min_height: Dp,
    pub text_style: TextStyle,
}

impl ButtonVariant {
    pub fn resolve(&self, state: WidgetState) -> ButtonStyle {
        ButtonStyle {
            background: resolve_button_fill(&self.container, state),
            foreground: resolve_button_fill(&self.content, state),
            border_color: resolve_button_border(&self.border, state),
            border_width: self.border_width,
            radius: self.radius,
            padding_x: self.padding_x,
            padding_y: self.padding_y,
            min_height: self.min_height,
            text_style: self.text_style.clone(),
        }
    }
}

fn resolve_button_fill<T: Clone>(stateful: &Stateful<T>, state: WidgetState) -> T {
    if state.disabled {
        return stateful.disabled.clone();
    }
    if state.pressed {
        return stateful.pressed.clone();
    }
    if state.hovered {
        return stateful.hovered.clone();
    }
    if state.focused {
        return stateful.focused.clone();
    }
    stateful.normal.clone()
}

fn resolve_button_border<T: Clone>(stateful: &Stateful<T>, state: WidgetState) -> T {
    if state.disabled {
        return stateful.disabled.clone();
    }
    if state.focused {
        return stateful.focused.clone();
    }
    if state.pressed {
        return stateful.pressed.clone();
    }
    if state.hovered {
        return stateful.hovered.clone();
    }
    stateful.normal.clone()
}
