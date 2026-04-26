//! `tgui` is a GPU-accelerated Rust GUI framework built around a small MVVM-style API.
//!
//! The crate is organized around a few core building blocks:
//!
//! - [`Application`] configures the window, theme, fonts, and runtime entry point.
//! - [`ViewModelContext`] creates reactive state such as [`Observable`] and [`AnimatedValue`].
//! - [`Binding`] derives UI-facing values from state and can opt into declarative transitions.
//! - [`Command`] and [`ValueCommand`] connect widget events back to your view model.
//! - Layout and widgets such as [`Flex`], [`Button`], [`Input`], and [`Text`] build the widget tree.
//!
//! Applications are always backed by an explicit view model:
//!
//! ```no_run
//! use tgui::{
//!     Application, Axis, Button, Command, Flex, Observable, Text, ViewModel, ViewModelContext,
//! };
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
//!         Flex::new(Axis::Vertical)
//!             .child(Text::new(
//!                 self.count.binding().map(|count| format!("Count: {count}")),
//!             ))
//!             .child(Button::new(Text::new("Increment")).on_click(Command::new(Self::increment)))
//!             .into()
//!     }
//! }
//!
//! impl ViewModel for CounterVm {}
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
/// use tgui::{el, Axis, Element, Flex, Text, ViewModel};
///
/// struct AppVm;
/// impl ViewModel for AppVm {}
///
/// let _column: Element<AppVm> = Flex::<AppVm>::new(Axis::Vertical).child(el![
///     Text::new("First"),
///     Text::new("Second"),
/// ]).into();
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
pub use ui::layout::{
    fr, pct, Align, Axis, Insets, Justify, LayoutStyle, Length, Overflow, PositionType,
    ScrollbarStyle, Track, Value, Wrap,
};
pub use ui::theme::{
    BorderScale, ColorScheme, ComponentTheme, ElevationScale, FontWeight, MotionScale, RadiusScale,
    Shadow, SpaceScale, Stateful, TextStyle, Theme, ThemeMode, ThemeSet, ThemeStore, TypeScale,
    WidgetState,
};
pub use ui::unit::{dp, sp, Dp, Sp};
pub use ui::widget::{
    rect, Button, Canvas, CanvasBooleanOp, CanvasBrush, CanvasGradientStop, CanvasItem,
    CanvasItemId, CanvasLinearGradient, CanvasPath, CanvasPathOpError, CanvasPointerEvent,
    CanvasRadialGradient, CanvasShadow, CanvasStroke, CursorStyle, Element, Flex, Grid, Image,
    Input, IntoLengthValue, PathBuilder, Point, Rect, Stack, Switch, Text, WidgetCommand,
    WidgetEventResult, WidgetId, WidgetTree,
};
#[cfg(feature = "video")]
pub use video::{
    PlaybackState, VideoController, VideoMetrics, VideoSize, VideoSource, VideoSurface,
};
