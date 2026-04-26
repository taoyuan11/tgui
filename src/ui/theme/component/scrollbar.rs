use crate::foundation::color::Color;
use crate::ui::theme::state::{Stateful, WidgetState};
use crate::ui::unit::Dp;

#[derive(Clone, Debug, PartialEq)]
pub struct ScrollbarTheme {
    pub track: Stateful<Color>,
    pub thumb: Stateful<Color>,
    pub width: Dp,
    pub radius: Dp,
}

impl ScrollbarTheme {
    pub fn track_color(&self, state: WidgetState) -> Color {
        self.track.resolve(state)
    }

    pub fn thumb_color(&self, state: WidgetState) -> Color {
        self.thumb.resolve(state)
    }
}
