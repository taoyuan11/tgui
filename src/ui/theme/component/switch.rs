use crate::foundation::color::Color;
use crate::ui::layout::Insets;
use crate::ui::theme::state::{Stateful, WidgetState};
use crate::ui::unit::Dp;

#[derive(Clone, Debug, PartialEq)]
pub struct SwitchTheme {
    pub track: Stateful<Color>,
    pub track_checked: Stateful<Color>,
    pub thumb: Stateful<Color>,
    pub thumb_checked: Stateful<Color>,
    pub border: Stateful<Color>,
    pub border_checked: Stateful<Color>,
    pub border_width: Dp,
    pub radius: Dp,
    pub padding: Insets,
    pub width: Dp,
    pub height: Dp,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SwitchStyle {
    pub background: Color,
    pub thumb: Color,
    pub border: Color,
    pub border_width: Dp,
    pub radius: Dp,
    pub padding: Insets,
    pub width: Dp,
    pub height: Dp,
}

impl SwitchTheme {
    pub fn resolve(&self, state: WidgetState, checked: bool) -> SwitchStyle {
        let mut track_state = state;
        if !checked && !track_state.disabled {
            track_state.selected = false;
        } else if checked {
            track_state.selected = true;
        }

        let mut thumb_state = state;
        thumb_state.selected = checked;

        SwitchStyle {
            background: if checked {
                self.track_checked.resolve(track_state)
            } else {
                self.track.resolve(track_state)
            },
            thumb: if checked {
                self.thumb_checked.resolve(thumb_state)
            } else {
                self.thumb.resolve(thumb_state)
            },
            border: if checked {
                self.border_checked.resolve(track_state)
            } else {
                self.border.resolve(track_state)
            },
            border_width: self.border_width,
            radius: self.radius,
            padding: self.padding,
            width: self.width,
            height: self.height,
        }
    }
}
