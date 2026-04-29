use crate::foundation::color::Color;
use crate::ui::theme::state::{Stateful, WidgetState};
use crate::ui::theme::typography::TextStyle;
use crate::ui::unit::Dp;

#[derive(Clone, Debug, PartialEq)]
pub struct CheckboxTheme {
    pub background: Stateful<Color>,
    pub background_checked: Stateful<Color>,
    pub border: Stateful<Color>,
    pub border_checked: Stateful<Color>,
    pub checkmark: Stateful<Color>,
    pub label: Stateful<Color>,
    pub border_width: Dp,
    pub radius: Dp,
    pub size: Dp,
    pub label_gap: Dp,
    pub text_style: TextStyle,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CheckboxStyle {
    pub background: Color,
    pub border: Color,
    pub checkmark: Color,
    pub label: Color,
    pub border_width: Dp,
    pub radius: Dp,
    pub size: Dp,
    pub label_gap: Dp,
    pub text_style: TextStyle,
}

impl CheckboxTheme {
    pub fn resolve(&self, state: WidgetState, checked: bool) -> CheckboxStyle {
        let mut control_state = state;
        control_state.selected = checked;

        CheckboxStyle {
            background: if checked {
                self.background_checked.resolve(control_state)
            } else {
                self.background.resolve(control_state)
            },
            border: if checked {
                self.border_checked.resolve(control_state)
            } else {
                self.border.resolve(control_state)
            },
            checkmark: self.checkmark.resolve(control_state),
            label: self.label.resolve(control_state),
            border_width: self.border_width,
            radius: self.radius,
            size: self.size,
            label_gap: self.label_gap,
            text_style: self.text_style.clone(),
        }
    }
}
