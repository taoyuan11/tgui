use std::collections::HashMap;
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
use crate::foundation::color::Color;
use crate::foundation::view_model::{Command, ValueCommand};
use crate::media::{
    media_placeholder_color, media_placeholder_label, resolve_media_rect, ContentFit,
    IntrinsicSize, MediaManager, RasterRequest,
};
use crate::text::font::{FontManager, TextFontRequest, ICON_FONT_FAMILY};
use crate::ui::layout::{
    Align, Axis, Insets, Justify, LayoutStyle, Length, Overflow, PositionType, Track, Value, Wrap,
};
use crate::ui::theme::{Theme, WidgetState};
use crate::ui::unit::{dp, sp, Dp, Sp, UnitContext};
#[cfg(feature = "video")]
use crate::video::VideoSurface as PublicVideoSurface;

use super::canvas::{canvas_bounds, CanvasClipContext, CanvasItem};
#[cfg(test)]
use super::common::RenderedWidgetScene;
use super::common::{
    BackdropBlurPrimitive, BrushPrimitive, ClipMask, ComputedScene, ContainerKind, ContainerLayout,
    CursorStyle, HitGeometry, HitInteraction, HitRegion, InteractionHandlers, LayoutNode,
    MeasureContext, MediaEventHandlers, MediaEventPhase, MediaEventState, Point, Rect,
    RenderPrimitive, ScenePrimitives, ScrollRegion, ScrollbarAxis, ScrollbarHandle,
    SelectOptionState, TextEditState, TextPrimitive, TexturePrimitive, VisualStyle, WidgetId,
    WidgetKind, WidgetStateMap,
};
#[cfg(feature = "video")]
use super::style::VideoSurfaceStyle as WidgetVideoSurfaceStyle;
use super::style::{
    ButtonStyle as WidgetButtonStyle, CheckboxStyle as WidgetCheckboxStyle,
    InputStyle as WidgetInputStyle, RadioStyle as WidgetRadioStyle,
    SelectStyle as WidgetSelectStyle, SwitchStyle as WidgetSwitchStyle,
    TextareaStyle as WidgetTextareaStyle,
};
use super::text::Text;

mod element;
mod layout;
mod render;
mod resolved;
mod style;
#[cfg(test)]
mod tests;
mod tree;

use self::layout::*;
use self::render::*;
use self::style::*;
pub use self::tree::{rect, WidgetCommand, WidgetEventResult, WidgetTree};

/// Caret width in logical pixels.
pub(super) const CARET_WIDTH: f32 = 2.0;

/// Caret end gap in logical pixels.
pub(super) const CARET_END_GAP: f32 = 1.0;

/// Default intrinsic width for selects when no explicit width is set.
const SELECT_DEFAULT_WIDTH: f32 = 160.0;

const CHECKBOX_CHECKMARK_ICON: &str = "\u{e687}";
const SELECT_ARROW_ICON: &str = "\u{e686}";

pub struct Element<VM> {
    pub(crate) id: WidgetId,
    pub(crate) layout: LayoutStyle,
    pub(crate) visual: VisualStyle,
    pub(crate) interactions: InteractionHandlers<VM>,
    pub(crate) media_events: MediaEventHandlers<VM>,
    pub(crate) background: Option<Value<Color>>,
    pub(crate) kind: WidgetKind<VM>,
}

impl<VM> Clone for Element<VM> {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            layout: self.layout.clone(),
            visual: self.visual.clone(),
            interactions: self.interactions.clone(),
            media_events: self.media_events.clone(),
            background: self.background.clone(),
            kind: self.kind.clone(),
        }
    }
}

#[derive(Clone)]
struct ResolvedElement<VM> {
    id: WidgetId,
    layout: LayoutStyle,
    visual: VisualStyle,
    interactions: InteractionHandlers<VM>,
    media_events: MediaEventHandlers<VM>,
    background: Option<Value<Color>>,
    kind: ResolvedWidgetKind<VM>,
}

#[derive(Clone)]
enum ResolvedWidgetKind<VM> {
    Container {
        layout: ContainerLayout,
        children: Vec<ResolvedElement<VM>>,
    },
    Text {
        text: Text,
    },
    Image {
        image: super::image::Image,
    },
    Canvas {
        items: Vec<CanvasItem>,
        item_interactions: super::common::CanvasItemInteractionHandlers<VM>,
    },
    #[cfg(feature = "video")]
    VideoSurface {
        video: PublicVideoSurface,
        style: WidgetVideoSurfaceStyle,
    },
    Button {
        label: Value<String>,
        disabled: Value<bool>,
        style: WidgetButtonStyle,
    },
    Checkbox {
        checked: Value<bool>,
        label: Option<Value<String>>,
        on_change: Option<ValueCommand<VM, bool>>,
        disabled: Value<bool>,
        style: WidgetCheckboxStyle,
    },
    Radio {
        checked: Value<bool>,
        label: Option<Value<String>>,
        on_change: Option<ValueCommand<VM, bool>>,
        disabled: Value<bool>,
        style: WidgetRadioStyle,
    },
    Switch {
        checked: Value<bool>,
        on_change: Option<ValueCommand<VM, bool>>,
        active_background: Option<Value<Color>>,
        inactive_background: Option<Value<Color>>,
        active_thumb_color: Option<Value<Color>>,
        inactive_thumb_color: Option<Value<Color>>,
        disabled: Value<bool>,
        style: WidgetSwitchStyle,
    },
    Select {
        selected_label: Value<Option<String>>,
        placeholder: Value<String>,
        options: Vec<SelectOptionState<VM>>,
        open: Option<Value<bool>>,
        on_open_change: Option<ValueCommand<VM, bool>>,
        disabled: Value<bool>,
        style: WidgetSelectStyle,
    },
    Input {
        value: Value<String>,
        placeholder: Value<String>,
        on_change: Option<ValueCommand<VM, String>>,
        disabled: Value<bool>,
        style: WidgetInputStyle,
    },
    Textarea {
        value: Value<String>,
        placeholder: Value<String>,
        on_change: Option<ValueCommand<VM, String>>,
        disabled: Value<bool>,
        style: WidgetTextareaStyle,
    },
}

struct CollectContext<'a, 'b> {
    taffy: &'a TaffyTree<MeasureContext>,
    font_manager: &'a FontManager,
    theme: &'a Theme,
    media: &'a MediaManager,
    focused_input: Option<WidgetId>,
    focused_text_state: Option<&'a TextEditState>,
    caret_visible: bool,
    selected_text: Option<WidgetId>,
    selected_text_state: Option<&'a TextEditState>,
    hovered_scrollbar: Option<ScrollbarHandle>,
    active_scrollbar: Option<ScrollbarHandle>,
    widget_states: &'a WidgetStateMap,
    select_open_states: &'a HashMap<WidgetId, bool>,
    scroll_offsets: &'a HashMap<WidgetId, Point>,
    viewport: Rect,
    units: UnitContext,
    animations: &'b mut AnimationEngine,
    now: std::time::Instant,
}

#[derive(Clone, Copy)]
struct VisualContext {
    origin: Point,
    opacity: f32,
    clip_rect: Rect,
    clip_mask: Option<ClipMask>,
}

#[derive(Clone)]
pub(crate) struct ResolvedSceneLayout<VM> {
    resolved_root: ResolvedElement<VM>,
    layout_root: LayoutNode,
    taffy: TaffyTree<MeasureContext>,
    units: UnitContext,
}

fn media_event_phase(loading: bool, error: Option<&str>) -> Option<MediaEventPhase> {
    if loading {
        Some(MediaEventPhase::Loading)
    } else if let Some(error) = error {
        Some(MediaEventPhase::Error(error.to_string()))
    } else {
        Some(MediaEventPhase::Success)
    }
}
