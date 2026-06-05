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
// 可选的 mimalloc 全局分配器。仅在显式开启 `mimalloc` feature 时生效。
// 场景收集热路径分配密集，mimalloc 通常带来 10-25% 分配吞吐提升。
#[cfg(feature = "mimalloc")]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

mod accessibility;
pub mod animation;
pub mod application;
#[cfg(feature = "audio")]
pub mod audio;
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

#[macro_export]
/// Initializes `tgui` logging from the calling crate's `Cargo.toml`.
///
/// The macro embeds the application manifest at compile time and resolves
/// relative log paths from the manifest directory.
macro_rules! init_logging_from_cargo_toml {
    () => {
        $crate::logging::configure_logging_from_manifest(
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml")),
            ::std::path::Path::new(env!("CARGO_MANIFEST_DIR")),
        )
    };
}

/// Canvas drawing widgets and drawing primitives.
pub mod canvas {
    pub use crate::ui::widget::{
        Canvas, CanvasBlendMode, CanvasBrush, CanvasColorFilter, CanvasDragEvent, CanvasEffect,
        CanvasFillRule, CanvasGradientStop, CanvasGroup, CanvasGroupMode, CanvasGroupShape,
        CanvasImage, CanvasImageOptions, CanvasInnerShadow, CanvasItem, CanvasItemId,
        CanvasItemKind, CanvasLinearGradient, CanvasMouseButton, CanvasMouseEvent,
        CanvasParagraphStyle, CanvasPath, CanvasPathOpError, CanvasPointerEvent,
        CanvasRadialGradient, CanvasRecorder, CanvasScene, CanvasSceneDebugInfo,
        CanvasSceneDebugNode, CanvasSceneDebugStats, CanvasSceneHit, CanvasSceneQueryOptions,
        CanvasSceneVisit, CanvasShadow, CanvasStroke, CanvasStrokeAlignment, CanvasStrokeCap,
        CanvasStrokeJoin, CanvasSvgPathError, CanvasText, CanvasTextHit, CanvasTextHorizontalAlign,
        CanvasTextOverflow, CanvasTextSpan, CanvasTextStyle, CanvasTextVerticalAlign,
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

    /// Crate-wide alias for the canonical [`TguiError`] type.
    ///
    /// Prefer this name in user code so that any future move of the concrete
    /// error type stays a non-breaking change. The alias is part of the public
    /// API contract and will not be removed.
    pub type Error = TguiError;

    /// Crate-wide [`Result`] alias bound to [`TguiError`].
    ///
    /// Mirrors the pattern used by `std::io::Result`. The default error type
    /// is fixed so call sites can write `tgui::core::Result<T>` without
    /// repeating the error parameter.
    pub type Result<T, E = TguiError> = ::core::result::Result<T, E>;
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
    pub use crate::log::{
        configure_logging, configure_logging_from_manifest, tgui_log, Log, LogConfig,
        LogConfigError, LogFileConfig, LogLevel,
    };
}

/// MVVM state, bindings, commands, and view model contracts.
pub mod mvvm {
    pub use crate::foundation::binding::{
        ScrollRequest, ScrollRequestMode, ScrollViewController, Signal, State, TextChange,
        TextChangeSet, TextController, TextSnapshot, Toast, ToastAction, ToastId, ToastKind,
        ToastPlacement, ToastQueue, ViewModelContext,
    };
    pub use crate::foundation::form::{
        Form, FormField, FormSnapshot, FormStatus, TextFormField, ValidationErrors,
        ValidationVisualState,
    };
    pub use crate::foundation::task::Tasks;
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
    pub use crate::application::{
        Application, MsaaMode, ResourceBudget, WindowClosePolicy, WindowRole, WindowSpec,
    };
    #[cfg(feature = "audio")]
    pub use crate::audio::{Audio, AudioController, AudioMetrics, AudioPlaybackState, AudioSource};
    pub use crate::canvas::{
        Canvas, CanvasBlendMode, CanvasBrush, CanvasColorFilter, CanvasDragEvent, CanvasEffect,
        CanvasFillRule, CanvasGradientStop, CanvasGroup, CanvasGroupMode, CanvasGroupShape,
        CanvasImage, CanvasImageOptions, CanvasInnerShadow, CanvasItem, CanvasItemId,
        CanvasItemKind, CanvasLinearGradient, CanvasMouseButton, CanvasMouseEvent,
        CanvasParagraphStyle, CanvasPath, CanvasPathOpError, CanvasPointerEvent,
        CanvasRadialGradient, CanvasRecorder, CanvasScene, CanvasSceneDebugInfo,
        CanvasSceneDebugNode, CanvasSceneDebugStats, CanvasSceneHit, CanvasSceneQueryOptions,
        CanvasSceneVisit, CanvasShadow, CanvasStroke, CanvasStrokeAlignment, CanvasStrokeCap,
        CanvasStrokeJoin, CanvasSvgPathError, CanvasText, CanvasTextHit, CanvasTextHorizontalAlign,
        CanvasTextOverflow, CanvasTextSpan, CanvasTextStyle, CanvasTextVerticalAlign,
        CanvasTextWrap, CanvasTransform2D, CanvasWheelEvent, PathBuilder,
    };
    pub use crate::core::{
        dp, sp, Color, Dp, Error, InputTrigger, Point, Rect, Result, Sp, TguiError, WidgetId,
    };
    pub use crate::dialog::{
        DialogError, Dialogs, FileDialogOptions, MessageDialogButtons, MessageDialogLevel,
        MessageDialogOptions, MessageDialogResult,
    };
    pub use crate::el;
    pub use crate::layout::{
        fr, pct, Align, Axis, Flex, Grid, Insets, IntoLengthValue, Justify, LayoutStyle, Length,
        Overflow, PositionType, ScrollbarStyle, Stack, Track, Value, Wrap,
    };
    pub use crate::logging::{tgui_log, Log, LogConfig, LogConfigError, LogFileConfig, LogLevel};
    pub use crate::media::{ContentFit, MediaBytes, MediaSource};
    pub use crate::mvvm::{
        Command, CommandContext, Form, FormField, FormSnapshot, FormStatus, ScrollRequest,
        ScrollRequestMode, ScrollViewController, Signal, State, Tasks, TextChange, TextChangeSet,
        TextController, TextFormField, TextSnapshot, Toast, ToastAction, ToastId, ToastKind,
        ToastPlacement, ToastQueue, ValidationErrors, ValidationVisualState, ValueCommand,
        ViewModel, ViewModelContext, WindowControl, WindowResizeDirection,
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
    #[cfg(feature = "bench-support")]
    pub use crate::ui::widget::{
        default_bench_viewport, WidgetBenchmarkContext, WidgetBenchmarkStats,
    };
    #[cfg(feature = "video")]
    pub use crate::video::{
        VideoController, VideoMetrics, VideoPlaybackState, VideoSize, VideoSource, VideoSurface,
    };
    pub use crate::widgets::{
        rect, BackgroundBrush, BackgroundGradientStop, BackgroundImage, BackgroundLinearGradient,
        BackgroundRadialGradient, Button, ButtonStyle, CanvasStyle, Checkbox, CheckboxStyle,
        ChordKey, ContainerStyle, ContextMenu, CursorStyle, DataGrid, DataGridCellAction,
        DataGridCellContext, DataGridCellEditCommit, DataGridColumn, DataGridColumnPin,
        DataGridColumnReorderEvent, DataGridColumnWidthChange, DataGridDensity,
        DataGridHeaderContext, DataGridRow, DataGridSection, DataGridSelectionChange,
        DataGridSelectionMode, DataGridSelectionTrigger, DataGridSort, DataGridSortChange,
        DataGridSortDirection, DataGridSortTrigger, DataGridStyle, Divider, DividerOrientation,
        DividerStyle, DoubleTapEvent, Drawer, DrawerHost, DrawerMode, DrawerPlacement, DrawerStyle,
        EdgeSwipeEvent, Element, FocusRingOverride, FocusScopeOptions, GestureEdge, GestureEdgeSet,
        GesturePhase, GestureRecognizer, GestureSource, Image, ImageStyle, Input, InputStyle,
        IntoTextContent, ItemLayout, ItemSource, KeyChord, LayerStack, List, ListItem,
        ListItemAction, ListItemContext, ListSection, ListSelectionChange, ListSelectionMode,
        ListSelectionTrigger, ListStyle, LongPressEvent, Menu, MenuBar, MenuBarEntry, MenuBarStyle,
        MenuIcon, MenuItem, MenuItemKind, MenuStyle, Modal, ModalAction, ModalStyle,
        OverlayAlignment, OverlayAnchorKey, OverlayFlipPolicy, OverlayLayer, OverlayPlacement,
        OverlaySide, PinchGestureEvent, Popover, PopoverStyle, PopoverTriggerMode, Portal,
        PortalAnchor, PortalTarget, ProgressBar, ProgressBarStyle, Radio, RadioGroup, RadioOption,
        RadioStyle, ScrollView, Select, SelectOption, SelectStyle, Slider, SliderStyle, Spinner,
        SpinnerStyle, SwipeAxis, SwipeDirection, SwipeGestureEvent, Switch, SwitchStyle, TabItem,
        TabPlacement, TabView, Table, Tabs, TabsOverflowMode, TabsReorderEvent, TabsStyle, Text,
        TextWidgetStyle, Textarea, TextareaStyle, ToastHost, ToastStyle, Tooltip, TooltipStyle,
        VideoSurfaceStyle, VirtualArrangement, VirtualDirection, VirtualList, VirtualViewport,
        WidgetCommand, WidgetEventResult, WidgetKey, WidgetSurfaceStyle, WidgetTree,
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
    #[cfg(feature = "audio")]
    pub use crate::audio::Audio;
    pub use crate::layout::{Flex, Grid, IntoLengthValue, Stack};
    pub use crate::mvvm::{
        ScrollRequest, ScrollRequestMode, ScrollViewController, TextChange, TextChangeSet,
        TextController, TextSnapshot, Toast, ToastAction, ToastId, ToastKind, ToastPlacement,
        ToastQueue,
    };
    #[cfg(feature = "bench-support")]
    pub use crate::ui::widget::{
        default_bench_viewport, WidgetBenchmarkContext, WidgetBenchmarkStats,
    };
    pub use crate::ui::widget::{
        rect, BackgroundBrush, BackgroundGradientStop, BackgroundImage, BackgroundLinearGradient,
        BackgroundRadialGradient, Button, ButtonStyle, CanvasStyle, Checkbox, CheckboxStyle,
        ChordKey, ContainerStyle, ContextMenu, CursorStyle, DataGrid, DataGridCellAction,
        DataGridCellContext, DataGridCellEditCommit, DataGridColumn, DataGridColumnPin,
        DataGridColumnReorderEvent, DataGridColumnWidthChange, DataGridDensity,
        DataGridHeaderContext, DataGridRow, DataGridSection, DataGridSelectionChange,
        DataGridSelectionMode, DataGridSelectionTrigger, DataGridSort, DataGridSortChange,
        DataGridSortDirection, DataGridSortTrigger, DataGridStyle, Divider, DividerOrientation,
        DividerStyle, DoubleTapEvent, Drawer, DrawerHost, DrawerMode, DrawerPlacement, DrawerStyle,
        EdgeSwipeEvent, Element, FocusRingOverride, FocusScopeOptions, GestureEdge, GestureEdgeSet,
        GesturePhase, GestureRecognizer, GestureSource, Image, ImageStyle, Input, InputStyle,
        IntoTextContent, ItemLayout, ItemSource, KeyChord, LayerStack, List, ListItem,
        ListItemAction, ListItemContext, ListSection, ListSelectionChange, ListSelectionMode,
        ListSelectionTrigger, ListStyle, LongPressEvent, Menu, MenuBar, MenuBarEntry, MenuBarStyle,
        MenuIcon, MenuItem, MenuItemKind, MenuStyle, Modal, ModalAction, ModalStyle,
        OverlayAlignment, OverlayAnchorKey, OverlayFlipPolicy, OverlayLayer, OverlayPlacement,
        OverlaySide, PinchGestureEvent, Popover, PopoverStyle, PopoverTriggerMode, Portal,
        PortalAnchor, PortalTarget, ProgressBar, ProgressBarStyle, Radio, RadioGroup, RadioOption,
        RadioStyle, ScrollView, Select, SelectOption, SelectStyle, Slider, SliderStyle, Spinner,
        SpinnerStyle, SwipeAxis, SwipeDirection, SwipeGestureEvent, Switch, SwitchStyle, TabItem,
        TabPlacement, TabView, Table, Tabs, TabsOverflowMode, TabsReorderEvent, TabsStyle, Text,
        TextWidgetStyle, Textarea, TextareaStyle, ToastHost, ToastStyle, Tooltip, TooltipStyle,
        VideoSurfaceStyle, VirtualArrangement, VirtualDirection, VirtualList, VirtualViewport,
        WidgetCommand, WidgetEventResult, WidgetKey, WidgetSurfaceStyle, WidgetTree,
    };
    #[cfg(feature = "video")]
    pub use crate::video::VideoSurface;
}
