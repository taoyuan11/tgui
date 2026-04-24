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
//! use tgui::{dp, Application};
//!
//! fn main() -> Result<(), tgui::TguiError> {
//!     Application::new()
//!         .title("Hello tgui")
//!         .window_size(dp(960.0), dp(640.0))
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
mod dialog;
mod foundation;
mod log;
mod media;
pub mod platform;
mod rendering;
mod runtime;
mod text;
mod ui;
#[cfg(feature = "video")]
pub mod video;

#[macro_export]
/// Collects one or more widgets into a `Vec<Element<_>>`.
///
/// This is useful when a container child list mixes different widget types and
/// you want a compact call site.
///
/// ```rust
/// use tgui::{el, Column, Text};
///
/// let _column = Column::<()>::new().child(el![
///     Text::new("First"),
///     Text::new("Second"),
/// ]);
/// ```
macro_rules! el {
    ($($child:expr),* $(,)?) => {
        ::std::vec![$($crate::Element::from($child)),*]
    };
}

pub use crate::log::{tgui_log, Log, LogLevel};
pub use animation::{
    AnimatedValue, AnimationControllerBuilder, AnimationControllerHandle, AnimationCurve,
    AnimationSpec, AnimationStatus, Easing, FillMode, Keyframe, Keyframes, Playback,
    PlaybackDirection, Repeat, Transition,
};
pub use application::{Application, WindowClosePolicy, WindowRole, WindowSpec};
pub use dialog::{
    DialogError, Dialogs, FileDialogOptions, MessageDialogButtons, MessageDialogLevel,
    MessageDialogOptions, MessageDialogResult,
};
pub use foundation::binding::{Binding, Observable, ViewModelContext};
pub use foundation::color::Color;
pub use foundation::error::TguiError;
pub use foundation::event::InputTrigger;
pub use foundation::view_model::{Command, CommandContext, ValueCommand, ViewModel};
pub use media::{ContentFit, MediaBytes, MediaSource};
pub use text::font::FontWeight;
pub use ui::layout::{Align, Axis, Insets, LayoutStyle, Overflow, ScrollbarStyle, Value, Wrap};
pub use ui::theme::{Theme, ThemeMode};
pub use ui::unit::{dp, sp, Dp, Sp};
pub use ui::widget::{
    rect, Button, Canvas, CanvasBooleanOp, CanvasBrush, CanvasGradientStop, CanvasItem,
    CanvasItemId, CanvasLinearGradient, CanvasPath, CanvasPathOpError, CanvasPointerEvent,
    CanvasRadialGradient, CanvasShadow, CanvasStroke, Column, Container, CursorStyle, Element,
    Flex, Grid, Image, Input, PathBuilder, Point, Rect, Row, Stack, Text, WidgetCommand,
    WidgetEventResult, WidgetId, WidgetTree,
};
#[cfg(feature = "video")]
pub use video::{
    PlaybackState, VideoController, VideoMetrics, VideoSize, VideoSource, VideoSurface,
};
