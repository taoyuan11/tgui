#[cfg(feature = "audio")]
mod audio;
mod avatar;
mod background;
mod badge;
mod breadcrumb;
mod button;
mod calendar;
mod canvas;
mod card;
mod carousel;
mod checkbox;
mod collapse;
mod color_picker;
mod combobox;
mod common;
mod container;
mod core;
mod date_picker;
mod divider;
mod drawer;
mod gesture;
mod icon;
mod image;
mod input;
mod input_controls;
mod list;
mod menu;
mod modal;
mod number_input;
mod overlay;
mod p3_support;
mod pagination;
mod popover;
mod portal;
mod progress_bar;
mod radio;
mod rating;
mod rich_text;
mod scroll_view;
mod select;
mod skeleton;
mod slider;
mod slider_shared;
mod spinner;
mod splitter;
mod style;
mod switch;
mod table;
mod tabs;
mod text;
mod textarea;
mod time_picker;
mod toast;
mod tooltip;
mod tree;
mod upload;
#[cfg(feature = "video")]
mod video;
mod r#virtual;

#[cfg(feature = "audio")]
pub use audio::Audio;
pub use avatar::{Avatar, AvatarGroup, AvatarSource};
pub use background::{
    BackgroundBrush, BackgroundGradientStop, BackgroundImage, BackgroundLinearGradient,
    BackgroundRadialGradient,
};
pub use badge::{Badge, BadgeContent, BadgePlacement};
pub use breadcrumb::{Breadcrumb, BreadcrumbItem};
pub use button::Button;
pub use calendar::{Calendar, CalendarChangeTrigger, CalendarSelectionChange, CalendarStyle};
pub use canvas::{
    Canvas, CanvasBlendMode, CanvasBrush, CanvasColorFilter, CanvasDragEvent, CanvasEffect,
    CanvasFillRule, CanvasGradientStop, CanvasGroup, CanvasGroupMode, CanvasGroupShape,
    CanvasImage, CanvasImageOptions, CanvasInnerShadow, CanvasItem, CanvasItemId, CanvasItemKind,
    CanvasLinearGradient, CanvasMouseButton, CanvasMouseEvent, CanvasParagraphStyle, CanvasPath,
    CanvasPathOpError, CanvasPointerEvent, CanvasRadialGradient, CanvasRecorder, CanvasScene,
    CanvasSceneDebugInfo, CanvasSceneDebugNode, CanvasSceneDebugStats, CanvasSceneHit,
    CanvasSceneQueryOptions, CanvasSceneVisit, CanvasShadow, CanvasStroke, CanvasStrokeAlignment,
    CanvasStrokeCap, CanvasStrokeJoin, CanvasSvgPathError, CanvasText, CanvasTextHit,
    CanvasTextHorizontalAlign, CanvasTextOverflow, CanvasTextSpan, CanvasTextStyle,
    CanvasTextVerticalAlign, CanvasTextWrap, CanvasTransform2D, CanvasWheelEvent, PathBuilder,
};
pub use card::Card;
pub use carousel::Carousel;
pub use checkbox::Checkbox;
pub use collapse::{Accordion, AccordionItem, Collapse};
pub use color_picker::{
    ColorPicker, ColorPickerChange, ColorPickerChangeTrigger, ColorPickerStyle,
};
pub use combobox::{AutoComplete, Combobox, ComboboxChange, ComboboxOption};
pub(crate) use common::{
    slider_effective_step, slider_resolve_value, slider_value_from_normalized,
    text_input_content_geometry, text_input_content_viewport, text_input_layout_width,
    BackdropBlurPrimitive, BrushPrimitiveData, CanvasCompositePrimitive,
    CanvasItemInteractionHandlers, CanvasTextHitRegion, CanvasTextSpanPrimitive, ClipMask,
    CompositionState, ComputedScene, DataGridCellState, DataGridHeaderState,
    DataGridResizeHandleState, DefaultActivation, FocusScopeState, FocusTargetMeta, HitGeometry,
    HitInteraction, HitRegion, HitTargetId, InteractionHandlers, LifecycleEventHandlers,
    LifecycleEventState, ListItemState, MeasureContext, MediaEventPhase, MediaEventState,
    MeshPrimitive, MeshVertex, RenderCommand, ScrollRegion, ScrollbarAxis, ScrollbarHandle,
    SplitterHandleState, TextEditState, TextInputContentGeometry, TexturePrimitive, TreeNodeState,
    WidgetStateMap,
};
pub use common::{
    CursorStyle, DividerOrientation, FileDropEvent, FocusScopeOptions, Point, Rect,
    RenderPrimitive, ScenePrimitives, TabPlacement, TextPrimitive, WidgetId, WidgetKey,
};
pub use container::{Flex, Grid, IntoLengthValue, Stack};
#[cfg(feature = "bench-support")]
pub use core::bench_support::{
    default_bench_viewport, WidgetBenchmarkContext, WidgetBenchmarkStats,
};
#[cfg(feature = "bench-support")]
pub use core::bench_support_ext;
pub(crate) use core::LifecycleSnapshot;
#[cfg(feature = "audio")]
pub(crate) use core::LifecycleWidgetKind;
pub(crate) use core::{
    build_external_portal_overlay, collect_portal_content_scene, resolve_external_portal_anchor,
    ActiveTooltipState, CollectContext, CollectedSceneCache, FocusCollectState, ResolvedElement,
    ResolvedSceneLayout, ResolvedWidgetKind, SceneChunkParts, TextInputLayoutOverride,
    TooltipTrigger, VisualContextSnapshot,
};
pub use core::{rect, Element, WidgetCommand, WidgetEventResult, WidgetStyleExt, WidgetTree};
pub use date_picker::{DatePicker, DatePickerChange, DatePickerStyle};
pub use divider::Divider;
pub use drawer::{Drawer, DrawerHost, DrawerMode, DrawerPlacement};
pub use gesture::{
    DoubleTapEvent, EdgeSwipeEvent, GestureEdge, GestureEdgeSet, GesturePhase, GestureRecognizer,
    GestureSource, LongPressEvent, PinchGestureEvent, SwipeAxis, SwipeDirection, SwipeGestureEvent,
};
pub use icon::{BuiltinIcon, Icon, IconSource};
pub use image::Image;
pub use input::Input;
pub use list::{
    List, ListItem, ListItemAction, ListItemContext, ListSection, ListSelectionChange,
    ListSelectionMode, ListSelectionTrigger, ListStyle,
};
pub use menu::{
    ChordKey, ContextMenu, KeyChord, Menu, MenuBar, MenuBarEntry, MenuIcon, MenuItem, MenuItemKind,
};
pub(crate) use menu::{ContextMenuDescriptor, MenuItemState};
pub use modal::{Modal, ModalAction};
pub use number_input::{
    NumberInput, NumberInputChange, NumberInputChangeTrigger, NumberInputStyle,
};
#[allow(unused_imports)] // Public overlay aliases are part of the widgets API surface.
pub use overlay::{
    Alignment as OverlayAlignment, Anchor as OverlayAnchor, AnchorKey as OverlayAnchorKey,
    AnchorSource as OverlayAnchorSource, FlipPolicy as OverlayFlipPolicy, OverlayId, OverlayLayer,
    Placement as OverlayPlacement, PlacementOptions as OverlayPlacementOptions,
    Side as OverlaySide, SolvedPlacement as OverlaySolvedPlacement,
};
pub use pagination::{Pagination, PaginationChange};
pub use popover::{Popover, PopoverTriggerMode};
pub use portal::{LayerStack, Portal, PortalAnchor, PortalTarget};
pub use progress_bar::ProgressBar;
pub(crate) use r#virtual::VirtualSceneStateUpdate;
pub use r#virtual::{
    ItemLayout, ItemSource, VirtualArrangement, VirtualDirection, VirtualList, VirtualViewport,
};
pub(crate) use r#virtual::{VirtualCacheState, MEASURED_EXTENT_INVALIDATION_EPSILON};
pub use radio::{Radio, RadioGroup, RadioOption};
pub use rating::{Rating, RatingChange};
pub use rich_text::{RichText, RichTextImage, RichTextLinkClick};
pub use scroll_view::ScrollView;
pub use select::{Select, SelectOption};
pub use skeleton::{Skeleton, SkeletonShape};
pub use slider::Slider;
pub use slider_shared::SliderOrientation;
pub use spinner::Spinner;
pub(crate) use splitter::{splitter_adjusted_sizes, splitter_reset_sizes};
pub use splitter::{Pane, ResizablePanels, Splitter, SplitterAxis, SplitterResize};
pub use style::{
    AvatarShape, AvatarStyle, BadgeStyle, BadgeTone, BreadcrumbStyle, ButtonSelector, ButtonStyle,
    CanvasStyle, CardStyle, CarouselStyle, CheckboxStyle, CollapseStyle, ComboboxStyle,
    ContainerStyle, DividerStyle, DrawerStyle, FocusRingOverride, IconStyle, ImageStyle,
    InputStyle, MenuBarStyle, MenuStyle, ModalStyle, PaginationStyle, PopoverStyle,
    ProgressBarStyle, RadioStyle, RatingStyle, RichTextStyle, SelectStyle, SkeletonStyle,
    SliderStyle, SpinnerStyle, SplitterStyle, StyleSelector, StyleSheet, SwitchStyle, TabsStyle,
    TextWidgetStyle, TextareaStyle, ToastStyle, TooltipStyle, VideoStyle, VideoSurfaceStyle,
    WidgetSurfaceStyle,
};
pub use switch::Switch;
pub use table::{
    DataGrid, DataGridCellAction, DataGridCellContext, DataGridCellEditCommit, DataGridColumn,
    DataGridColumnPin, DataGridColumnReorderEvent, DataGridColumnWidthChange, DataGridDensity,
    DataGridHeaderContext, DataGridRow, DataGridSection, DataGridSelectionChange,
    DataGridSelectionMode, DataGridSelectionTrigger, DataGridSort, DataGridSortChange,
    DataGridSortDirection, DataGridSortTrigger, DataGridStyle, Table,
};
pub use tabs::{TabItem, TabView, Tabs, TabsOverflowMode, TabsReorderEvent};
pub use text::{IntoTextContent, Text};
pub use textarea::Textarea;
pub use time_picker::{TimePicker, TimePickerChange, TimePickerStyle};
pub use toast::ToastHost;
pub use tooltip::Tooltip;
pub(crate) use tree::tree_check_state;
pub use tree::{
    Tree, TreeCheckChange, TreeCheckState, TreeCheckTrigger, TreeDropEvent, TreeDropPosition,
    TreeExpandChange, TreeExpandTrigger, TreeNode, TreeNodeAction, TreeNodeContext,
    TreeSelectionChange, TreeSelectionMode, TreeSelectionTrigger, TreeStyle,
};
pub use upload::{
    Upload, UploadFile, UploadFileId, UploadRejection, UploadRemove, UploadSelection, UploadStatus,
    UploadStyle,
};
#[cfg(feature = "video")]
pub use video::{Video, VideoSurface};
