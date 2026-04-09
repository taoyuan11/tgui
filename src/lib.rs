//! `tgui` is a GPU-accelerated Rust GUI framework built around a small MVVM-style API.
//!
//! The crate is organized around a few core building blocks:
//!
//! - [`Application`] configures the window, theme, fonts, and runtime entry point.
//! - [`ViewModelContext`] creates reactive state such as [`Observable`] and [`AnimatedValue`].
//! - [`Binding`] derives UI-facing values from state and can opt into declarative transitions.
//! - [`Command`] and [`ValueCommand`] connect widget events back to your view model.
//! - Layout and widgets such as [`Column`], [`Button`], [`Input`], and [`Text`] build the widget tree.
//!
//! A minimal app looks like this:
//!
//! ```no_run
//! use tgui::Application;
//!
//! fn main() -> Result<(), tgui::TguiError> {
//!     Application::new()
//!         .title("Hello tgui")
//!         .window_size(960, 640)
//!         .run()
//! }
//! ```
//!
//! For a state-driven application, create a view model and bind the root view:
//!
//! ```no_run
//! use tgui::{Application, Button, Column, Command, Observable, Text, ViewModelContext};
//!
//! struct CounterVm {
//!     count: Observable<u32>,
//! }
//!
//! impl CounterVm {
//!     fn new(ctx: &ViewModelContext) -> Self {
//!         Self {
//!             count: ctx.observable(0),
//!         }
//!     }
//!
//!     fn increment(&mut self) {
//!         self.count.update(|value| *value += 1);
//!     }
//!
//!     fn view(&self) -> tgui::Element<Self> {
//!         Column::new()
//!             .child(Text::new(
//!                 self.count.binding().map(|count| format!("Count: {count}")),
//!             ))
//!             .child(Button::new(Text::new("Increment")).on_click(Command::new(Self::increment)))
//!             .into()
//!     }
//! }
//!
//! fn main() -> Result<(), tgui::TguiError> {
//!     Application::new()
//!         .with_view_model(CounterVm::new)
//!         .root_view(CounterVm::view)
//!         .run()
//! }
//! ```
mod animation;
mod application;
mod foundation;
pub mod platform;
mod rendering;
mod runtime;
mod text;
mod ui;

#[macro_export]
/// Collects multiple widgets into a `Vec<Element<_>>`.
///
/// This is mainly useful when an API expects a vector of children and you want
/// to keep the call site compact.
///
/// ```rust
/// use tgui::{children, Column, Text};
///
/// let _column = Column::<()>::new().children(children![
///     Text::new("First"),
///     Text::new("Second"),
/// ]);
/// ```
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
pub use ui::layout::{
    Align, Axis, Insets, Justify, LayoutStyle, Overflow, ScrollbarStyle, Value, Wrap,
};
pub use ui::theme::{Theme, ThemeMode};
pub use ui::widget::{
    rect, Button, Column, Container, CursorStyle, Element, Flex, Grid, Input, Point, Rect, Row,
    Stack, Text, WidgetCommand, WidgetEventResult, WidgetId, WidgetTree,
};
