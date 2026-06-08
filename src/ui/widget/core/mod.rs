use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use taffy::prelude::{
    length, line, span, AlignContent as TaffyAlignContent, AlignItems as TaffyAlignItems,
    AvailableSpace, Dimension, Display, FlexDirection, FlexWrap, FromFr, FromLength, FromPercent,
    GridTemplateComponent, JustifyContent as TaffyJustifyContent, LengthPercentage,
    LengthPercentageAuto, Position as TaffyPosition, Style as TaffyStyle, TaffyAuto, TaffyTree,
    TaffyZero, TrackSizingFunction,
};
use taffy::Size as TaffySize;

use crate::animation::{AnimationEngine, Transition, WidgetProperty};
#[cfg(feature = "audio")]
use crate::audio::Audio as PublicAudio;
use crate::foundation::binding::{
    track_dependency_scope, with_dependency_collection, DependencyGraph, DependencyPhase,
    TextChangeSet, TextController,
};
use crate::foundation::color::Color;
use crate::foundation::view_model::{Command, ValueCommand};
use crate::media::{
    media_placeholder_color, media_placeholder_label, resolve_media_rect, ContentFit,
    IntrinsicSize, MediaManager, RasterRequest,
};
use crate::text::font::{FontManager, TextFontRequest, TextLayoutInfo, ICON_FONT_FAMILY};
use crate::ui::layout::{
    Align, Axis, Insets, Justify, LayoutStyle, Length, Overflow, PositionType, Track, Value, Wrap,
};
use crate::ui::theme::{Theme, WidgetState};
use crate::ui::unit::{dp, sp, Dp, Sp, UnitContext};
#[cfg(feature = "video")]
use crate::video::VideoSurface as PublicVideoSurface;

use super::canvas::{
    canvas_scene_bounds, tessellate_canvas_scene_items, CanvasScene, CanvasSceneHit,
};
#[cfg(test)]
use super::common::RenderedWidgetScene;
#[cfg(feature = "video")]
use super::common::VideoTexturePrimitive;
use super::common::{
    text_input_content_geometry, text_input_content_viewport, text_input_layout_width,
    BackdropBlurPrimitive, BrushPrimitive, ClipMask, ComputedScene, ContainerKind, ContainerLayout,
    CursorStyle, FileDropEvent, HitGeometry, HitInteraction, HitRegion, InteractionHandlers,
    LayoutNode, LifecycleEventHandlers, LifecycleEventState, MeasureContext, MediaEventHandlers,
    MediaEventPhase, MediaEventState, Point, Rect, RenderPrimitive, ScenePrimitives, ScrollRegion,
    ScrollbarAxis, ScrollbarHandle, SelectOptionState, SliderValueFormatter, TextEditState,
    TextInputContentGeometry, TextPrimitive, TexturePrimitive, VisualStyle, WidgetId, WidgetKey,
    WidgetKind, WidgetStateMap,
};
#[cfg(feature = "video")]
use super::style::VideoSurfaceStyle as WidgetVideoSurfaceStyle;
use super::style::{
    ButtonStyle as WidgetButtonStyle, CheckboxStyle as WidgetCheckboxStyle,
    DividerStyle as WidgetDividerStyle, InputStyle as WidgetInputStyle,
    ProgressBarStyle as WidgetProgressBarStyle, RadioStyle as WidgetRadioStyle,
    SelectStyle as WidgetSelectStyle, SliderStyle as WidgetSliderStyle,
    SpinnerStyle as WidgetSpinnerStyle, StyleResolver, SwitchStyle as WidgetSwitchStyle,
    TextareaStyle as WidgetTextareaStyle, ToastStyle as WidgetToastStyle,
};
use super::text::{IntoTextContent, Text};

#[cfg(feature = "bench-support")]
pub mod bench_support;
mod collect_profile;
mod element;
mod element_path;
mod element_resolve;
mod layout;
mod render;
mod resolved;
mod resolved_eq;
mod resolved_eq_lifecycle;
mod resolved_events;
mod resolved_freeze;
mod resolved_layout;
mod scene;
mod scene_layout;
mod style;
#[cfg(test)]
mod tests;
mod tree;
mod tree_query;
mod tree_scene;
mod types;

pub use self::element::WidgetStyleExt;
use self::element_path::resolve_subtree_from_source_path;
use self::layout::*;
use self::render::*;
pub(crate) use self::resolved::{
    build_external_portal_overlay, collect_portal_content_scene, resolve_external_portal_anchor,
};
pub(crate) use self::scene::{
    ActiveTooltipState, CollectContext, CollectedSceneCache, FocusCollectState, SceneChunkParts,
    TextInputLayoutOverride, TooltipTrigger, VisualContext, VisualContextSnapshot,
};
pub(crate) use self::scene_layout::ResolvedSceneLayout;
use self::scene_layout::*;
use self::style::*;
use self::tree::with_widget_stack;
pub use self::tree::{rect, WidgetCommand, WidgetEventResult, WidgetTree};
pub use self::types::Element;
use self::types::*;
pub(crate) use self::types::{
    LifecycleSnapshot, LifecycleWidgetKind, ResolvedElement, ResolvedWidgetKind,
};

/// Caret width in logical pixels.
pub(super) const CARET_WIDTH: f32 = 2.0;

/// Caret end gap in logical pixels.
pub(super) const CARET_END_GAP: f32 = 1.0;

/// Default intrinsic width for selects when no explicit width is set.
const SELECT_DEFAULT_WIDTH: f32 = 160.0;

const CHECKBOX_CHECKMARK_ICON: &str = "check";
const SELECT_ARROW_ICON: &str = "keyboard_arrow_down";
