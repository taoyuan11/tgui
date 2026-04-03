mod animation;
mod application;
mod foundation;
mod rendering;
mod runtime;
mod text;
mod ui;

#[macro_export]
macro_rules! children {
    ($($child:expr),* $(,)?) => {
        ::std::vec![$(::core::convert::Into::into($child)),*]
    };
}

pub use animation::{
    AnimatedValue, AnimationControllerBuilder, AnimationControllerHandle, AnimationCurve,
    AnimationSpec, AnimationStatus, Easing, FillMode, Keyframe, Keyframes, Playback,
    PlaybackDirection, Repeat, Transition,
};
pub use application::Application;
pub use foundation::binding::{Binding, Observable, ViewModelContext};
pub use foundation::color::Color;
pub use foundation::error::TguiError;
pub use foundation::event::InputTrigger;
pub use foundation::view_model::{Command, ValueCommand, ViewModel};
pub use text::font::FontWeight;
pub use ui::layout::{Align, Axis, Insets, Justify, LayoutStyle, Value, Wrap};
pub use ui::theme::{Theme, ThemeMode};
pub use ui::widget::{
    rect, Button, Column, Container, Element, Flex, Grid, Input, Point, Rect, Row, Stack, Text,
    WidgetCommand, WidgetEventResult, WidgetId, WidgetTree,
};
