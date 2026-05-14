//! `tgui` is a GPU-accelerated Rust GUI framework built around a small MVVM-style API.
//!
//! The crate is organized around a few core building blocks:
//!
//! - [`application::Application`] configures the window, theme, fonts, and runtime entry point.
//! - [`mvvm::ViewModelContext`] creates reactive state such as [`mvvm::State`] and
//!   [`animation::AnimatedValue`].
//! - [`mvvm::Signal`] derives UI-facing values from state and can opt into declarative transitions.
//! - [`mvvm::Command`] and [`mvvm::ValueCommand`] connect widget events back to your view model.
//! - Layout and widgets such as [`layout::Flex`], [`widgets::Button`], and [`widgets::Text`]
//!   build the widget tree.
//!
//! Applications are always backed by an explicit view model:
//!
//! ```no_run
//! use tgui::application::Application;
//! use tgui::layout::Axis;
//! use tgui::mvvm::{Command, State, ViewModel, ViewModelContext};
//! use tgui::widgets::{Button, Element, Flex, Text};
//!
//! struct CounterVm {
//!     count: State<u32>,
//! }
//!
//! impl CounterVm {
//!     fn new(ctx: &ViewModelContext) -> Self {
//!         Self {
//!             count: ctx.state(0),
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
//!                 self.count.signal().map(|count| format!("Count: {count}")),
//!             ))
//!             .child(Button::new("Increment").on_click(Command::new(Self::increment)))
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
pub mod notification;
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
    () => {
        ::std::vec::Vec::new()
    };
    ($($child:expr),* $(,)?) => {
        {
            let mut children = ::std::vec::Vec::new();
            $(
                children.push($crate::widgets::Element::from($child));
            )*
            children
        }
    };
}

/// Canvas drawing widgets and drawing primitives.
pub mod canvas {
    pub use crate::ui::widget::{
        Canvas, CanvasBlendMode, CanvasBooleanOp, CanvasBrush, CanvasClip, CanvasClipShape,
        CanvasDragEvent, CanvasFillRule, CanvasGradientStop, CanvasGroup, CanvasImage, CanvasItem,
        CanvasItemId, CanvasItemStyle, CanvasLayer, CanvasLinearGradient, CanvasMask,
        CanvasMouseButton, CanvasMouseEvent, CanvasParagraphStyle, CanvasPath, CanvasPathOpError,
        CanvasPointerEvent, CanvasRadialGradient, CanvasShadow, CanvasStroke,
        CanvasStrokeAlignment, CanvasStrokeCap, CanvasStrokeJoin, CanvasSvgPathError, CanvasText,
        CanvasTextHorizontalAlign, CanvasTextOverflow, CanvasTextStyle, CanvasTextVerticalAlign,
        CanvasTextWrap, CanvasTransform2D, CanvasWheelEvent, PathBuilder,
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
    pub use crate::foundation::binding::{
        Signal, State, TextChange, TextChangeSet, TextController, TextSnapshot, ViewModelContext,
    };
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
    pub use crate::application::{Application, MsaaMode, WindowClosePolicy, WindowRole, WindowSpec};
    pub use crate::canvas::{
        Canvas, CanvasBlendMode, CanvasBooleanOp, CanvasBrush, CanvasClip, CanvasClipShape,
        CanvasDragEvent, CanvasFillRule, CanvasGradientStop, CanvasGroup, CanvasImage, CanvasItem,
        CanvasItemId, CanvasItemStyle, CanvasLayer, CanvasLinearGradient, CanvasMask,
        CanvasMouseButton, CanvasMouseEvent, CanvasParagraphStyle, CanvasPath, CanvasPathOpError,
        CanvasPointerEvent, CanvasRadialGradient, CanvasShadow, CanvasStroke,
        CanvasStrokeAlignment, CanvasStrokeCap, CanvasStrokeJoin, CanvasSvgPathError, CanvasText,
        CanvasTextHorizontalAlign, CanvasTextOverflow, CanvasTextStyle, CanvasTextVerticalAlign,
        CanvasTextWrap, CanvasTransform2D, CanvasWheelEvent, PathBuilder,
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
        Command, CommandContext, Signal, State, TextChange, TextChangeSet, TextController,
        TextSnapshot, ValueCommand, ViewModel, ViewModelContext, WindowControl,
        WindowResizeDirection,
    };
    pub use crate::notification::{
        NotificationAction, NotificationActionEvent, NotificationError, NotificationOptions,
        NotificationPermission, Notifications,
    };
    pub use crate::theme::{
        BorderScale, ColorScheme, ElevationScale, FocusRingStyle, FontWeight, MotionScale,
        RadiusScale, ResolvedThemeMode, Shadow, SpaceScale, Stateful, TextStyle, Theme, ThemeMode,
        ThemeSet, ThemeStore, TypeScale, WidgetState,
    };
    #[cfg(feature = "video")]
    pub use crate::video::{
        PlaybackState, VideoController, VideoMetrics, VideoSize, VideoSource, VideoSurface,
    };
    pub use crate::widgets::{
        rect, BackgroundBrush, BackgroundGradientStop, BackgroundImage, BackgroundLinearGradient,
        BackgroundRadialGradient, Button, ButtonStyle, CanvasStyle, Checkbox, CheckboxStyle,
        ContainerStyle, CursorStyle, Element, FocusRingOverride, Image, ImageStyle, Input,
        InputStyle, IntoTextContent, Radio, RadioGroup, RadioOption, RadioStyle, Select,
        SelectOption, SelectStyle, Switch, SwitchStyle, Text, TextWidgetStyle, Textarea,
        TextareaStyle, VideoSurfaceStyle, WidgetCommand, WidgetEventResult, WidgetSurfaceStyle,
        WidgetTree,
    };
}

/// Theme tokens, state resolution, and theme storage.
pub mod theme {
    pub use crate::ui::theme::{
        BorderScale, ColorScheme, ElevationScale, FocusRingStyle, FontWeight, MotionScale,
        RadiusScale, ResolvedThemeMode, Shadow, SpaceScale, Stateful, TextStyle, Theme, ThemeMode,
        ThemeSet, ThemeStore, TypeScale, WidgetState,
    };
}

/// Built-in widgets and widget-tree infrastructure.
pub mod widgets {
    pub use crate::layout::{Flex, Grid, IntoLengthValue, Stack};
    pub use crate::mvvm::{TextChange, TextChangeSet, TextController, TextSnapshot};
    pub use crate::ui::widget::{
        rect, BackgroundBrush, BackgroundGradientStop, BackgroundImage, BackgroundLinearGradient,
        BackgroundRadialGradient, Button, ButtonStyle, CanvasStyle, Checkbox, CheckboxStyle,
        ContainerStyle, CursorStyle, Element, FocusRingOverride, Image, ImageStyle, Input,
        InputStyle, IntoTextContent, Radio, RadioGroup, RadioOption, RadioStyle, Select,
        SelectOption, SelectStyle, Switch, SwitchStyle, Text, TextWidgetStyle, Textarea,
        TextareaStyle, VideoSurfaceStyle, WidgetCommand, WidgetEventResult, WidgetSurfaceStyle,
        WidgetTree,
    };
    #[cfg(feature = "video")]
    pub use crate::video::VideoSurface;
}
