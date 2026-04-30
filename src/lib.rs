//! `tgui` is a GPU-accelerated Rust GUI framework built around a small MVVM-style API.
//!
//! The crate is organized around a few core building blocks:
//!
//! - [`application::Application`] configures the window, theme, fonts, and runtime entry point.
//! - [`mvvm::ViewModelContext`] creates reactive state such as [`mvvm::Observable`] and
//!   [`animation::AnimatedValue`].
//! - [`mvvm::Binding`] derives UI-facing values from state and can opt into declarative transitions.
//! - [`mvvm::Command`] and [`mvvm::ValueCommand`] connect widget events back to your view model.
//! - Layout and widgets such as [`layout::Flex`], [`widgets::Button`], [`widgets::Input`],
//!   and [`widgets::Text`] build the widget tree.
//!
//! Applications are always backed by an explicit view model:
//!
//! ```no_run
//! use tgui::application::Application;
//! use tgui::layout::Axis;
//! use tgui::mvvm::{Command, Observable, ViewModel, ViewModelContext};
//! use tgui::widgets::{Button, Element, Flex, Text};
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
//!     fn view(&self) -> Element<Self> {
//!         Flex::new(Axis::Vertical)
//!             .child(Text::new(
//!                 self.count.binding().map(|count| format!("Count: {count}")),
//!             ))
//!             .child(Button::new(Text::new("Increment")).on_click(Command::new(Self::increment)))
//!             .into()
//!     }
//! }
//!
//! impl ViewModel for CounterVm {
//!     fn new(ctx: &ViewModelContext) -> Self {
//!         CounterVm::new(ctx)
//!     }
//!
//!     fn view(&self) -> Element<Self> {
//!         CounterVm::view(self)
//!     }
//! }
//!
//! fn main() -> Result<(), tgui::core::TguiError> {
//!     Application::new()
//!         .with_view_model(CounterVm::new)
//!         .root_view(CounterVm::view)
//!         .run()
//! }
//! ```
pub mod animation;
pub mod application;
pub mod dialog;
mod foundation;
mod log;
pub mod media;
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
/// use tgui::el;
/// use tgui::layout::Axis;
/// use tgui::mvvm::{ViewModel, ViewModelContext};
/// use tgui::widgets::{Element, Flex, Text};
///
/// struct AppVm;
/// impl ViewModel for AppVm {
///     fn new(_: &ViewModelContext) -> Self {
///         Self
///     }
///
///     fn view(&self) -> Element<Self> {
///         Text::new("App").into()
///     }
/// }
///
/// let _column: Element<AppVm> = Flex::<AppVm>::new(Axis::Vertical).child(el![
///     Text::new("First"),
///     Text::new("Second"),
/// ]).into();
/// ```
macro_rules! el {
    ($($child:expr),* $(,)?) => {
        ::std::vec![$($crate::widgets::Element::from($child)),*]
    };
}

/// Canvas drawing widgets and drawing primitives.
pub mod canvas {
    pub use crate::ui::widget::{
        Canvas, CanvasBooleanOp, CanvasBrush, CanvasGradientStop, CanvasItem, CanvasItemId,
        CanvasLinearGradient, CanvasPath, CanvasPathOpError, CanvasPointerEvent,
        CanvasRadialGradient, CanvasShadow, CanvasStroke, PathBuilder,
    };
}

/// Foundational types that are shared across multiple subsystems.
pub mod core {
    pub use crate::foundation::color::Color;
    pub use crate::foundation::error::TguiError;
    pub use crate::foundation::event::InputTrigger;
    pub use crate::ui::unit::{dp, sp, Dp, Sp};
    pub use crate::ui::widget::{Point, Rect, WidgetId};
}

/// Layout primitives, sizing helpers, and container widgets.
pub mod layout {
    pub use crate::ui::layout::{
        fr, pct, Align, Axis, Insets, Justify, LayoutStyle, Length, Overflow, PositionType,
        ScrollbarStyle, Track, Value, Wrap,
    };
    pub use crate::ui::unit::{dp, sp, Dp, Sp};
    pub use crate::ui::widget::{Flex, Grid, IntoLengthValue, Stack};
}

/// Logging helpers used by platform integrations and examples.
pub mod logging {
    pub use crate::log::{tgui_log, Log, LogLevel};
}

/// MVVM state, bindings, commands, and view model contracts.
pub mod mvvm {
    pub use crate::foundation::binding::{Binding, Observable, ViewModelContext};
    pub use crate::foundation::view_model::{Command, CommandContext, ValueCommand, ViewModel};
    pub use crate::foundation::window_control::{WindowControl, WindowResizeDirection};
}

/// Convenient imports for small applications and examples.
pub mod prelude {
    pub use crate::animation::{
        AnimatedValue, AnimationControllerBuilder, AnimationControllerHandle, AnimationCurve,
        AnimationSpec, AnimationStatus, Easing, FillMode, Keyframe, Keyframes, Playback,
        PlaybackDirection, Repeat, Transition,
    };
    pub use crate::application::{Application, WindowClosePolicy, WindowRole, WindowSpec};
    pub use crate::canvas::{
        Canvas, CanvasBooleanOp, CanvasBrush, CanvasGradientStop, CanvasItem, CanvasItemId,
        CanvasLinearGradient, CanvasPath, CanvasPathOpError, CanvasPointerEvent,
        CanvasRadialGradient, CanvasShadow, CanvasStroke, PathBuilder,
    };
    pub use crate::core::{dp, sp, Color, Dp, InputTrigger, Point, Rect, Sp, TguiError, WidgetId};
    pub use crate::dialog::{
        DialogError, Dialogs, FileDialogOptions, MessageDialogButtons, MessageDialogLevel,
        MessageDialogOptions, MessageDialogResult,
    };
    pub use crate::el;
    pub use crate::layout::{
        fr, pct, Align, Axis, Flex, Grid, Insets, IntoLengthValue, Justify, LayoutStyle, Length,
        Overflow, PositionType, ScrollbarStyle, Stack, Track, Value, Wrap,
    };
    pub use crate::logging::{tgui_log, Log, LogLevel};
    pub use crate::media::{ContentFit, MediaBytes, MediaSource};
    pub use crate::mvvm::{
        Binding, Command, CommandContext, Observable, ValueCommand, ViewModel, ViewModelContext,
        WindowControl, WindowResizeDirection,
    };
    pub use crate::theme::{
        BorderScale, CheckboxStyle, CheckboxTheme, ColorScheme, ComponentTheme, ElevationScale,
        FontWeight, MotionScale, RadioStyle, RadioTheme, RadiusScale, SelectStyle, SelectTheme,
        Shadow, SpaceScale, Stateful, TextStyle, Theme, ThemeMode, ThemeSet, ThemeStore, TypeScale,
        WidgetState,
    };
    #[cfg(feature = "video")]
    pub use crate::video::{
        PlaybackState, VideoController, VideoMetrics, VideoSize, VideoSource, VideoSurface,
    };
    pub use crate::widgets::{
        rect, BackgroundBrush, BackgroundGradientStop, BackgroundImage, BackgroundLinearGradient,
        BackgroundRadialGradient, Button, Checkbox, CursorStyle, Element, Image, Input, Radio,
        RadioGroup, RadioOption, Select, SelectOption, Switch, Text, WidgetCommand,
        WidgetEventResult, WidgetTree,
    };
}

/// Theme tokens, state resolution, and theme storage.
pub mod theme {
    pub use crate::ui::theme::{
        BorderScale, CheckboxStyle, CheckboxTheme, ColorScheme, ComponentTheme, ElevationScale,
        FontWeight, MotionScale, RadioStyle, RadioTheme, RadiusScale, SelectStyle, SelectTheme,
        Shadow, SpaceScale, Stateful, TextStyle, Theme, ThemeMode, ThemeSet, ThemeStore, TypeScale,
        WidgetState,
    };
}

/// Built-in widgets and widget-tree infrastructure.
pub mod widgets {
    pub use crate::layout::{Flex, Grid, IntoLengthValue, Stack};
    pub use crate::ui::widget::{
        rect, BackgroundBrush, BackgroundGradientStop, BackgroundImage, BackgroundLinearGradient,
        BackgroundRadialGradient, Button, Checkbox, CursorStyle, Element, Image, Input, Radio,
        RadioGroup, RadioOption, Select, SelectOption, Switch, Text, WidgetCommand,
        WidgetEventResult, WidgetTree,
    };
    #[cfg(feature = "video")]
    pub use crate::video::VideoSurface;
}
