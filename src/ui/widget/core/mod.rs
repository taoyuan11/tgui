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
use super::common::{
    text_input_content_geometry, text_input_content_viewport, text_input_layout_width,
    BackdropBlurPrimitive, BrushPrimitive, ClipMask, ComputedScene, ContainerKind, ContainerLayout,
    CursorStyle, HitGeometry, HitInteraction, HitRegion, InteractionHandlers, LayoutNode,
    LifecycleEventHandlers, LifecycleEventState, MeasureContext, MediaEventHandlers,
    MediaEventPhase, MediaEventState, Point, Rect, RenderPrimitive, ScenePrimitives, ScrollRegion,
    ScrollbarAxis, ScrollbarHandle, SelectOptionState, SliderValueFormatter, TextEditState,
    TextInputContentGeometry, TextPrimitive, TexturePrimitive, VisualStyle, WidgetId, WidgetKey,
    WidgetKind, WidgetStateMap,
};
#[cfg(feature = "video")]
use super::style::VideoSurfaceStyle as WidgetVideoSurfaceStyle;
use super::style::{
    ButtonStyle as WidgetButtonStyle, CheckboxStyle as WidgetCheckboxStyle,
    InputStyle as WidgetInputStyle, RadioStyle as WidgetRadioStyle,
    SelectStyle as WidgetSelectStyle, SliderStyle as WidgetSliderStyle,
    SwitchStyle as WidgetSwitchStyle,
};
use super::text::{IntoTextContent, Text};

mod element;
mod layout;
mod render;
mod resolved;
mod style;
#[cfg(test)]
mod tests;
mod tree;

use self::element::resolve_subtree_from_source_path;
use self::layout::*;
use self::render::*;
use self::style::*;
use self::tree::with_widget_stack;
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
    pub(crate) key: Option<WidgetKey>,
    pub(crate) layout: LayoutStyle,
    pub(crate) visual: VisualStyle,
    pub(crate) interactions: InteractionHandlers<VM>,
    pub(crate) lifecycle_events: LifecycleEventHandlers<VM>,
    pub(crate) media_events: MediaEventHandlers<VM>,
    pub(crate) background: Option<Value<Color>>,
    pub(crate) kind: WidgetKind<VM>,
}

impl<VM> Clone for Element<VM> {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            key: self.key.clone(),
            layout: self.layout.clone(),
            visual: self.visual.clone(),
            interactions: self.interactions.clone(),
            lifecycle_events: self.lifecycle_events.clone(),
            media_events: self.media_events.clone(),
            background: self.background.clone(),
            kind: self.kind.clone(),
        }
    }
}

pub(crate) struct ResolvedElement<VM> {
    pub(crate) id: WidgetId,
    pub(crate) key: Option<WidgetKey>,
    pub(crate) layout: LayoutStyle,
    pub(crate) visual: VisualStyle,
    pub(crate) interactions: InteractionHandlers<VM>,
    pub(crate) lifecycle_events: LifecycleEventHandlers<VM>,
    pub(crate) media_events: MediaEventHandlers<VM>,
    pub(crate) background: Option<Value<Color>>,
    pub(crate) child_source_spans: Vec<usize>,
    pub(crate) kind: ResolvedWidgetKind<VM>,
}

pub(crate) struct LifecycleSnapshot {
    pub(crate) id: WidgetId,
    pub(crate) key: Option<WidgetKey>,
    pub(crate) layout: LayoutStyle,
    pub(crate) visual: VisualStyle,
    pub(crate) background: Option<Value<Color>>,
    pub(crate) kind: LifecycleWidgetKind,
}

pub(crate) struct LifecycleSelectOption {
    pub(crate) label: Value<String>,
    pub(crate) selected: Value<bool>,
    pub(crate) disabled: Value<bool>,
}

pub(crate) enum ResolvedWidgetKind<VM> {
    Container {
        layout: ContainerLayout,
        children: Vec<ResolvedElement<VM>>,
    },
    Text {
        text: Text,
    },
    #[cfg(feature = "audio")]
    Audio {
        audio: PublicAudio,
    },
    Image {
        image: super::image::Image,
    },
    Canvas {
        scene: Value<CanvasScene>,
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
    Slider {
        value: Value<f32>,
        min: f32,
        max: f32,
        step: f32,
        show_ticks: bool,
        show_value_label: bool,
        tick_count: Option<usize>,
        value_formatter: Option<SliderValueFormatter>,
        on_change: Option<ValueCommand<VM, f32>>,
        disabled: Value<bool>,
        style: WidgetSliderStyle,
    },
    TextEditor {
        controller: TextController,
        placeholder: Value<String>,
        on_change: Option<Command<VM>>,
        on_change_set: Option<ValueCommand<VM, TextChangeSet>>,
        disabled: Value<bool>,
        style: WidgetInputStyle,
        multiline: bool,
        show_scrollbar: Value<bool>,
        auto_wrap: Value<bool>,
    },
}

pub(crate) enum LifecycleWidgetKind {
    Container {
        layout: ContainerLayout,
        child_ids: Vec<WidgetId>,
    },
    Text {
        text: Text,
    },
    #[cfg(feature = "audio")]
    Audio {
        audio: PublicAudio,
    },
    Image {
        image: super::image::Image,
    },
    Canvas {
        scene: Value<CanvasScene>,
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
        disabled: Value<bool>,
        style: WidgetCheckboxStyle,
    },
    Radio {
        checked: Value<bool>,
        label: Option<Value<String>>,
        disabled: Value<bool>,
        style: WidgetRadioStyle,
    },
    Switch {
        checked: Value<bool>,
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
        options: Vec<LifecycleSelectOption>,
        open: Option<Value<bool>>,
        disabled: Value<bool>,
        style: WidgetSelectStyle,
    },
    Slider {
        value: Value<f32>,
        min: f32,
        max: f32,
        step: f32,
        show_ticks: bool,
        show_value_label: bool,
        tick_count: Option<usize>,
        value_formatter: Option<SliderValueFormatter>,
        disabled: Value<bool>,
        style: WidgetSliderStyle,
    },
    TextEditor {
        placeholder: Value<String>,
        disabled: Value<bool>,
        style: WidgetInputStyle,
        multiline: bool,
        show_scrollbar: Value<bool>,
        auto_wrap: Value<bool>,
    },
}

impl<VM> Clone for ResolvedElement<VM> {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            key: self.key.clone(),
            layout: self.layout.clone(),
            visual: self.visual.clone(),
            interactions: self.interactions.clone(),
            lifecycle_events: self.lifecycle_events.clone(),
            media_events: self.media_events.clone(),
            background: self.background.clone(),
            child_source_spans: self.child_source_spans.clone(),
            kind: self.kind.clone(),
        }
    }
}

impl Clone for LifecycleSnapshot {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            key: self.key.clone(),
            layout: self.layout.clone(),
            visual: self.visual.clone(),
            background: self.background.clone(),
            kind: self.kind.clone(),
        }
    }
}

impl Clone for LifecycleSelectOption {
    fn clone(&self) -> Self {
        Self {
            label: self.label.clone(),
            selected: self.selected.clone(),
            disabled: self.disabled.clone(),
        }
    }
}

impl<VM> Clone for ResolvedWidgetKind<VM> {
    fn clone(&self) -> Self {
        match self {
            Self::Container { layout, children } => Self::Container {
                layout: layout.clone(),
                children: children.clone(),
            },
            Self::Text { text } => Self::Text { text: text.clone() },
            #[cfg(feature = "audio")]
            Self::Audio { audio } => Self::Audio {
                audio: audio.clone(),
            },
            Self::Image { image } => Self::Image {
                image: image.clone(),
            },
            Self::Canvas {
                scene,
                item_interactions,
            } => Self::Canvas {
                scene: scene.clone(),
                item_interactions: item_interactions.clone(),
            },
            #[cfg(feature = "video")]
            Self::VideoSurface { video, style } => Self::VideoSurface {
                video: video.clone(),
                style: style.clone(),
            },
            Self::Button {
                label,
                disabled,
                style,
            } => Self::Button {
                label: label.clone(),
                disabled: disabled.clone(),
                style: style.clone(),
            },
            Self::Checkbox {
                checked,
                label,
                on_change,
                disabled,
                style,
            } => Self::Checkbox {
                checked: checked.clone(),
                label: label.clone(),
                on_change: on_change.clone(),
                disabled: disabled.clone(),
                style: style.clone(),
            },
            Self::Radio {
                checked,
                label,
                on_change,
                disabled,
                style,
            } => Self::Radio {
                checked: checked.clone(),
                label: label.clone(),
                on_change: on_change.clone(),
                disabled: disabled.clone(),
                style: style.clone(),
            },
            Self::Switch {
                checked,
                on_change,
                active_background,
                inactive_background,
                active_thumb_color,
                inactive_thumb_color,
                disabled,
                style,
            } => Self::Switch {
                checked: checked.clone(),
                on_change: on_change.clone(),
                active_background: active_background.clone(),
                inactive_background: inactive_background.clone(),
                active_thumb_color: active_thumb_color.clone(),
                inactive_thumb_color: inactive_thumb_color.clone(),
                disabled: disabled.clone(),
                style: style.clone(),
            },
            Self::Select {
                selected_label,
                placeholder,
                options,
                open,
                on_open_change,
                disabled,
                style,
            } => Self::Select {
                selected_label: selected_label.clone(),
                placeholder: placeholder.clone(),
                options: options.clone(),
                open: open.clone(),
                on_open_change: on_open_change.clone(),
                disabled: disabled.clone(),
                style: style.clone(),
            },
            Self::Slider {
                value,
                min,
                max,
                step,
                show_ticks,
                show_value_label,
                tick_count,
                value_formatter,
                on_change,
                disabled,
                style,
            } => Self::Slider {
                value: value.clone(),
                min: *min,
                max: *max,
                step: *step,
                show_ticks: *show_ticks,
                show_value_label: *show_value_label,
                tick_count: *tick_count,
                value_formatter: value_formatter.clone(),
                on_change: on_change.clone(),
                disabled: disabled.clone(),
                style: style.clone(),
            },
            Self::TextEditor {
                controller,
                placeholder,
                on_change,
                on_change_set,
                disabled,
                style,
                multiline,
                show_scrollbar,
                auto_wrap,
            } => Self::TextEditor {
                controller: controller.clone(),
                placeholder: placeholder.clone(),
                on_change: on_change.clone(),
                on_change_set: on_change_set.clone(),
                disabled: disabled.clone(),
                style: style.clone(),
                multiline: *multiline,
                show_scrollbar: show_scrollbar.clone(),
                auto_wrap: auto_wrap.clone(),
            },
        }
    }
}

impl Clone for LifecycleWidgetKind {
    fn clone(&self) -> Self {
        match self {
            Self::Container { layout, child_ids } => Self::Container {
                layout: layout.clone(),
                child_ids: child_ids.clone(),
            },
            Self::Text { text } => Self::Text { text: text.clone() },
            #[cfg(feature = "audio")]
            Self::Audio { audio } => Self::Audio {
                audio: audio.clone(),
            },
            Self::Image { image } => Self::Image {
                image: image.clone(),
            },
            Self::Canvas { scene } => Self::Canvas {
                scene: scene.clone(),
            },
            #[cfg(feature = "video")]
            Self::VideoSurface { video, style } => Self::VideoSurface {
                video: video.clone(),
                style: style.clone(),
            },
            Self::Button {
                label,
                disabled,
                style,
            } => Self::Button {
                label: label.clone(),
                disabled: disabled.clone(),
                style: style.clone(),
            },
            Self::Checkbox {
                checked,
                label,
                disabled,
                style,
            } => Self::Checkbox {
                checked: checked.clone(),
                label: label.clone(),
                disabled: disabled.clone(),
                style: style.clone(),
            },
            Self::Radio {
                checked,
                label,
                disabled,
                style,
            } => Self::Radio {
                checked: checked.clone(),
                label: label.clone(),
                disabled: disabled.clone(),
                style: style.clone(),
            },
            Self::Switch {
                checked,
                active_background,
                inactive_background,
                active_thumb_color,
                inactive_thumb_color,
                disabled,
                style,
            } => Self::Switch {
                checked: checked.clone(),
                active_background: active_background.clone(),
                inactive_background: inactive_background.clone(),
                active_thumb_color: active_thumb_color.clone(),
                inactive_thumb_color: inactive_thumb_color.clone(),
                disabled: disabled.clone(),
                style: style.clone(),
            },
            Self::Select {
                selected_label,
                placeholder,
                options,
                open,
                disabled,
                style,
            } => Self::Select {
                selected_label: selected_label.clone(),
                placeholder: placeholder.clone(),
                options: options.clone(),
                open: open.clone(),
                disabled: disabled.clone(),
                style: style.clone(),
            },
            Self::Slider {
                value,
                min,
                max,
                step,
                show_ticks,
                show_value_label,
                tick_count,
                value_formatter,
                disabled,
                style,
            } => Self::Slider {
                value: value.clone(),
                min: *min,
                max: *max,
                step: *step,
                show_ticks: *show_ticks,
                show_value_label: *show_value_label,
                tick_count: *tick_count,
                value_formatter: value_formatter.clone(),
                disabled: disabled.clone(),
                style: style.clone(),
            },
            Self::TextEditor {
                placeholder,
                disabled,
                style,
                multiline,
                show_scrollbar,
                auto_wrap,
            } => Self::TextEditor {
                placeholder: placeholder.clone(),
                disabled: disabled.clone(),
                style: style.clone(),
                multiline: *multiline,
                show_scrollbar: show_scrollbar.clone(),
                auto_wrap: auto_wrap.clone(),
            },
        }
    }
}

struct CollectContext<'a, 'b> {
    taffy: &'a TaffyTree<MeasureContext>,
    font_manager: &'a FontManager,
    theme: &'a Theme,
    media: &'a MediaManager,
    focused_input: Option<WidgetId>,
    focused_text_state: Option<&'a TextEditState>,
    focused_text_value: Option<&'a str>,
    focused_text_layout: Option<&'a TextLayoutInfo>,
    text_layout_overrides: Option<&'a HashMap<WidgetId, TextInputLayoutOverride<'a>>>,
    active_slider_value: Option<(WidgetId, f32)>,
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

pub(crate) struct TextInputLayoutOverride<'a> {
    pub(crate) revision: u64,
    pub(crate) text: &'a str,
    pub(crate) layout: &'a TextLayoutInfo,
}

#[derive(Clone, Copy)]
struct VisualContext {
    origin: Point,
    opacity: f32,
    clip_rect: Rect,
    clip_mask: Option<ClipMask>,
}

#[derive(Clone, Copy)]
pub(crate) struct VisualContextSnapshot {
    pub(crate) origin: Point,
    pub(crate) opacity: f32,
    pub(crate) clip_rect: Rect,
    pub(crate) clip_mask: Option<ClipMask>,
}

impl From<VisualContext> for VisualContextSnapshot {
    fn from(value: VisualContext) -> Self {
        Self {
            origin: value.origin,
            opacity: value.opacity,
            clip_rect: value.clip_rect,
            clip_mask: value.clip_mask,
        }
    }
}

impl From<VisualContextSnapshot> for VisualContext {
    fn from(value: VisualContextSnapshot) -> Self {
        Self {
            origin: value.origin,
            opacity: value.opacity,
            clip_rect: value.clip_rect,
            clip_mask: value.clip_mask,
        }
    }
}

#[derive(Clone)]
pub(crate) struct CollectedSceneCache<VM> {
    pub(crate) computed: ComputedScene<VM>,
    pub(crate) lifecycle_states: HashMap<WidgetId, LifecycleEventState<VM>>,
    pub(crate) chunks: HashMap<WidgetId, ComputedScene<VM>>,
    pub(crate) chunk_parts: HashMap<WidgetId, SceneChunkParts<VM>>,
    pub(crate) visual_contexts: HashMap<WidgetId, VisualContextSnapshot>,
    pub(crate) dependencies: DependencyGraph,
}

#[derive(Clone)]
pub(crate) struct SceneChunkParts<VM> {
    pub(crate) before_children: ComputedScene<VM>,
    pub(crate) after_children: ComputedScene<VM>,
}

#[derive(Clone)]
pub(crate) struct ResolvedSceneLayout<VM> {
    source_root: Element<VM>,
    resolved_root: ResolvedElement<VM>,
    layout_root: LayoutNode,
    taffy: TaffyTree<MeasureContext>,
    units: UnitContext,
    dependencies: DependencyGraph,
    root_id: WidgetId,
    paths: HashMap<WidgetId, Vec<usize>>,
    parents: HashMap<WidgetId, Option<WidgetId>>,
    depths: HashMap<WidgetId, usize>,
}

impl<VM> ResolvedSceneLayout<VM> {
    pub(crate) fn dependencies(&self) -> &DependencyGraph {
        &self.dependencies
    }

    pub(crate) fn root_id(&self) -> WidgetId {
        self.root_id
    }

    pub(crate) fn path_for(&self, widget_id: WidgetId) -> Option<&[usize]> {
        self.paths.get(&widget_id).map(Vec::as_slice)
    }

    pub(crate) fn parent_of(&self, widget_id: WidgetId) -> Option<WidgetId> {
        self.parents.get(&widget_id).copied().flatten()
    }

    pub(crate) fn depth_of(&self, widget_id: WidgetId) -> usize {
        self.depths.get(&widget_id).copied().unwrap_or_default()
    }

    pub(crate) fn subtree_widget_ids(&self, widget_id: WidgetId) -> Vec<WidgetId> {
        let Some(path) = self.path_for(widget_id) else {
            return Vec::new();
        };
        let mut ids = Vec::new();
        let node = self.resolved_at_path(path);
        collect_resolved_widget_ids(node, &mut ids);
        ids
    }

    pub(crate) fn resolved_widget(&self, widget_id: WidgetId) -> Option<&ResolvedElement<VM>> {
        let path = self.path_for(widget_id)?;
        Some(self.resolved_at_path(path))
    }

    pub(crate) fn can_patch_layout_dependency_as_scene(&self, widget_id: WidgetId) -> bool {
        let Some(node) = self.resolved_widget(widget_id) else {
            return false;
        };
        match &node.kind {
            ResolvedWidgetKind::Text { text } => {
                !text.user_select
                    && node.background.is_none()
                    && !node.interactions.has_any()
                    && !node.lifecycle_events.has_any()
                    && !node.media_events.has_any()
                    && node.visual.border_color.is_none()
                    && node.visual.border_radius.is_none()
                    && node.visual.border_width.is_none()
                    && node.visual.background_brush.is_none()
                    && node.visual.background_image.is_none()
                    && node.visual.background_blur.resolve() == Dp::ZERO
                    && node.visual.shadow.is_none()
                    && node.visual.opacity.resolve() == 1.0
                    && node.visual.offset.resolve() == Point::ZERO
            }
            _ => false,
        }
    }

    #[allow(dead_code)]
    pub(crate) fn query_canvas_scene_at_widget(
        &self,
        widget_id: WidgetId,
        font_manager: &FontManager,
        units: UnitContext,
        scene_position: Point,
    ) -> Option<CanvasSceneHit> {
        self.query_canvas_scene_all_at_widget(widget_id, font_manager, units, scene_position)
            .into_iter()
            .next()
    }

    #[allow(dead_code)]
    pub(crate) fn query_canvas_scene_all_at_widget(
        &self,
        widget_id: WidgetId,
        font_manager: &FontManager,
        units: UnitContext,
        scene_position: Point,
    ) -> Vec<CanvasSceneHit> {
        let Some(node) = self.resolved_widget(widget_id) else {
            return Vec::new();
        };
        let ResolvedWidgetKind::Canvas { scene, .. } = &node.kind else {
            return Vec::new();
        };
        scene
            .resolve()
            .query_point_all_with_runtime_context(font_manager, units, scene_position)
    }

    pub(crate) fn rebuild_indexes(&mut self) {
        self.root_id = self.resolved_root.id;
        let mut path = Vec::new();
        let mut paths = HashMap::new();
        let mut parents = HashMap::new();
        let mut depths = HashMap::new();
        collect_indexes(
            &self.resolved_root,
            None,
            0,
            &mut path,
            &mut paths,
            &mut parents,
            &mut depths,
        );
        self.paths = paths;
        self.parents = parents;
        self.depths = depths;
    }
}

fn collect_indexes<VM>(
    node: &ResolvedElement<VM>,
    parent: Option<WidgetId>,
    depth: usize,
    path: &mut Vec<usize>,
    paths: &mut HashMap<WidgetId, Vec<usize>>,
    parents: &mut HashMap<WidgetId, Option<WidgetId>>,
    depths: &mut HashMap<WidgetId, usize>,
) {
    paths.insert(node.id, path.clone());
    parents.insert(node.id, parent);
    depths.insert(node.id, depth);
    if let ResolvedWidgetKind::Container { children, .. } = &node.kind {
        for (index, child) in children.iter().enumerate() {
            path.push(index);
            collect_indexes(
                child,
                Some(node.id),
                depth + 1,
                path,
                paths,
                parents,
                depths,
            );
            path.pop();
        }
    }
}

impl<VM> ResolvedSceneLayout<VM> {
    fn resolved_at_path(&self, path: &[usize]) -> &ResolvedElement<VM> {
        resolved_at_path(&self.resolved_root, path)
    }

    fn layout_at_path(&self, path: &[usize]) -> &LayoutNode {
        layout_at_path(&self.layout_root, path)
    }

    pub(crate) fn collect_scene_cache_for_widget_with_focus_value(
        &self,
        widget_id: WidgetId,
        font_manager: &FontManager,
        theme: &Theme,
        media: &MediaManager,
        animations: &mut AnimationEngine,
        visual_context: VisualContextSnapshot,
        hovered_scrollbar: Option<ScrollbarHandle>,
        active_scrollbar: Option<ScrollbarHandle>,
        widget_states: &WidgetStateMap,
        select_open_states: &HashMap<WidgetId, bool>,
        scroll_offsets: &HashMap<WidgetId, Point>,
        viewport: Rect,
        focused_input: Option<WidgetId>,
        focused_text_state: Option<&TextEditState>,
        focused_text_value: Option<&str>,
        focused_text_layout: Option<&TextLayoutInfo>,
        text_layout_overrides: Option<&HashMap<WidgetId, TextInputLayoutOverride<'_>>>,
        active_slider_value: Option<(WidgetId, f32)>,
        selected_text: Option<WidgetId>,
        selected_text_state: Option<&TextEditState>,
        caret_visible: bool,
    ) -> Option<CollectedSceneCache<VM>> {
        let path = self.path_for(widget_id)?;
        let ((mut computed, lifecycle_states, chunks, chunk_parts, visual_contexts), dependencies): (
            (
                ComputedScene<VM>,
                HashMap<WidgetId, LifecycleEventState<VM>>,
                HashMap<WidgetId, ComputedScene<VM>>,
                HashMap<WidgetId, SceneChunkParts<VM>>,
                HashMap<WidgetId, VisualContextSnapshot>,
            ),
            DependencyGraph,
        ) = with_widget_stack(|| {
            with_dependency_collection(|| {
                let mut lifecycle_states = HashMap::new();
                let mut chunks = HashMap::new();
                let mut chunk_parts = HashMap::new();
                let mut visual_contexts = HashMap::new();
                let mut context = CollectContext {
                    taffy: &self.taffy,
                    font_manager,
                    theme,
                    media,
                    focused_input,
                    focused_text_state,
                    focused_text_value,
                    focused_text_layout,
                    text_layout_overrides,
                    active_slider_value,
                    caret_visible,
                    selected_text,
                    selected_text_state,
                    hovered_scrollbar,
                    active_scrollbar,
                    widget_states,
                    select_open_states,
                    scroll_offsets,
                    viewport,
                    units: self.units,
                    animations,
                    now: std::time::Instant::now(),
                };
                let computed = self.resolved_at_path(path).collect_subtree_cache(
                    self.layout_at_path(path),
                    visual_context.into(),
                    &mut context,
                    &mut lifecycle_states,
                    &mut chunks,
                    &mut chunk_parts,
                    &mut visual_contexts,
                );
                (
                    computed,
                    lifecycle_states,
                    chunks,
                    chunk_parts,
                    visual_contexts,
                )
            })
        });
        computed.dependencies = dependencies.clone();
        Some(CollectedSceneCache {
            computed,
            lifecycle_states,
            chunks,
            chunk_parts,
            visual_contexts,
            dependencies,
        })
    }

    pub(crate) fn recompose_scene_chunk(
        &self,
        widget_id: WidgetId,
        chunk_parts: &HashMap<WidgetId, SceneChunkParts<VM>>,
        chunks: &mut HashMap<WidgetId, ComputedScene<VM>>,
    ) -> Option<()> {
        let path = self.path_for(widget_id)?;
        let node = self.resolved_at_path(path);
        let parts = chunk_parts.get(&widget_id)?;
        let mut composed = parts.before_children.clone();
        if let ResolvedWidgetKind::Container { children, .. } = &node.kind {
            for child in children {
                let child_chunk = chunks.get(&child.id)?.clone();
                composed.extend(&child_chunk);
            }
        }
        composed.extend(&parts.after_children);
        chunks.insert(widget_id, composed);
        Some(())
    }

    pub(crate) fn patch_layout_roots(
        &mut self,
        roots: &[WidgetId],
        font_manager: &FontManager,
        theme: &Theme,
        media: &MediaManager,
        animations: &mut AnimationEngine,
        viewport: Rect,
    ) -> Result<HashSet<WidgetId>, taffy::TaffyError> {
        let units = self.units;
        let (result, dependencies) = with_dependency_collection(
            || -> Result<(HashSet<WidgetId>, HashSet<u64>), taffy::TaffyError> {
                let mut removed_ids = HashSet::new();
                let mut touched_owner_ids = HashSet::new();

                for root_id in roots {
                    let Some(path) = self.path_for(*root_id).map(|path| path.to_vec()) else {
                        continue;
                    };

                    let previous_ids = self.subtree_widget_ids(*root_id);
                    touched_owner_ids.extend(previous_ids.iter().map(|id| id.raw()));

                    let Some(next) = resolve_subtree_from_source_path(
                        &self.source_root,
                        Some(&self.resolved_root),
                        theme,
                        &path,
                    ) else {
                        continue;
                    };
                    let next_ids = {
                        let mut ids = Vec::new();
                        collect_resolved_widget_ids(&next, &mut ids);
                        ids
                    };
                    let next_id_set: HashSet<_> = next_ids.into_iter().collect();
                    removed_ids.extend(
                        previous_ids
                            .into_iter()
                            .filter(|id| !next_id_set.contains(id)),
                    );

                    patch_layout_at_path(
                        &mut self.resolved_root,
                        &mut self.layout_root,
                        &path,
                        next,
                        &mut self.taffy,
                        animations,
                        theme,
                        units,
                        viewport,
                        None,
                        true,
                    )?;
                    self.rebuild_indexes();
                }

                self.taffy.compute_layout_with_measure(
                    self.layout_root.node,
                    TaffySize {
                        width: AvailableSpace::Definite(viewport.width.get()),
                        height: AvailableSpace::Definite(viewport.height.get()),
                    },
                    |known_dimensions, _, _, node_context, _| {
                        measure_node(
                            node_context,
                            known_dimensions,
                            font_manager,
                            theme,
                            media,
                            units,
                        )
                    },
                )?;

                Ok((removed_ids, touched_owner_ids))
            },
        );
        let (removed_ids, touched_owner_ids) = result?;
        self.dependencies.remove_widget_owners(&touched_owner_ids);
        self.dependencies.merge_from(&dependencies);
        self.rebuild_indexes();
        Ok(removed_ids)
    }

    pub(crate) fn patch_resolved_roots(&mut self, roots: &[WidgetId], theme: &Theme) -> bool {
        for root_id in roots {
            let Some(path) = self.path_for(*root_id).map(|path| path.to_vec()) else {
                continue;
            };
            let Some(next) = resolve_subtree_from_source_path(
                &self.source_root,
                Some(&self.resolved_root),
                theme,
                &path,
            ) else {
                return false;
            };
            if !patch_resolved_at_path(&mut self.resolved_root, &path, next) {
                return false;
            }
        }
        self.rebuild_indexes();
        true
    }
}

fn collect_resolved_widget_ids<VM>(node: &ResolvedElement<VM>, ids: &mut Vec<WidgetId>) {
    ids.push(node.id);
    if let ResolvedWidgetKind::Container { children, .. } = &node.kind {
        for child in children {
            collect_resolved_widget_ids(child, ids);
        }
    }
}

fn patch_layout_at_path<VM>(
    current: &mut ResolvedElement<VM>,
    layout_node: &mut LayoutNode,
    path: &[usize],
    next: ResolvedElement<VM>,
    taffy: &mut TaffyTree<MeasureContext>,
    animations: &mut AnimationEngine,
    theme: &Theme,
    units: UnitContext,
    viewport: Rect,
    parent_kind: Option<ContainerKind>,
    is_root: bool,
) -> Result<(), taffy::TaffyError> {
    if path.is_empty() {
        *current = patch_layout_tree(
            current,
            next,
            layout_node,
            taffy,
            animations,
            theme,
            units,
            parent_kind,
            viewport,
            is_root,
        )?;
        return Ok(());
    }

    let ResolvedWidgetKind::Container { layout, children } = &mut current.kind else {
        return Ok(());
    };
    let child_index = path[0];
    patch_layout_at_path(
        &mut children[child_index],
        &mut layout_node.children[child_index],
        &path[1..],
        next,
        taffy,
        animations,
        theme,
        units,
        viewport,
        Some(layout.kind.clone()),
        false,
    )
}

fn patch_layout_tree<VM>(
    current: &mut ResolvedElement<VM>,
    mut next: ResolvedElement<VM>,
    layout_node: &mut LayoutNode,
    taffy: &mut TaffyTree<MeasureContext>,
    animations: &mut AnimationEngine,
    theme: &Theme,
    units: UnitContext,
    parent_kind: Option<ContainerKind>,
    viewport: Rect,
    is_root: bool,
) -> Result<ResolvedElement<VM>, taffy::TaffyError> {
    let owner = next.id.dependency_owner(DependencyPhase::Layout);
    track_dependency_scope(owner, || {
        let now = std::time::Instant::now();
        let next_parent_kind = match &next.kind {
            ResolvedWidgetKind::Container { layout, .. } => Some(layout.kind.clone()),
            _ => None,
        };

        let old_children = match std::mem::replace(
            &mut current.kind,
            ResolvedWidgetKind::Container {
                layout: ContainerLayout::flow(),
                children: Vec::new(),
            },
        ) {
            ResolvedWidgetKind::Container { children, .. } => children,
            other => {
                current.kind = other;
                Vec::new()
            }
        };
        let old_layout_children = std::mem::take(&mut layout_node.children);
        let mut old_children_by_id: HashMap<_, _> = old_children
            .into_iter()
            .zip(old_layout_children)
            .map(|(child, layout)| (child.id, (child, layout)))
            .collect();

        let next_children = match &mut next.kind {
            ResolvedWidgetKind::Container { children, .. } => std::mem::take(children),
            _ => Vec::new(),
        };

        let mut patched_children = Vec::with_capacity(next_children.len());
        let mut patched_layout_children = Vec::with_capacity(next_children.len());

        for child in next_children {
            if let Some((mut existing_child, mut existing_layout)) =
                old_children_by_id.remove(&child.id)
            {
                let patched_child = patch_layout_tree(
                    &mut existing_child,
                    child,
                    &mut existing_layout,
                    taffy,
                    animations,
                    theme,
                    units,
                    next_parent_kind.clone(),
                    viewport,
                    false,
                )?;
                patched_children.push(patched_child);
                patched_layout_children.push(existing_layout);
            } else {
                let new_layout = child.build_layout_tree(
                    taffy,
                    animations,
                    theme,
                    units,
                    next_parent_kind.clone(),
                    viewport,
                    false,
                    now,
                )?;
                patched_layout_children.push(new_layout);
                patched_children.push(child);
            }
        }

        for (_, (_, stale_layout)) in old_children_by_id {
            remove_layout_subtree(taffy, &stale_layout)?;
        }

        if let ResolvedWidgetKind::Container { children, .. } = &mut next.kind {
            *children = patched_children;
        }

        taffy.set_style(
            layout_node.node,
            next.taffy_style(
                parent_kind,
                viewport,
                is_root,
                animations,
                theme,
                units,
                now,
            ),
        )?;
        if patched_layout_children.is_empty() {
            taffy.set_children(layout_node.node, &[])?;
            taffy.set_node_context(layout_node.node, Some(next.measure_context()))?;
        } else {
            let child_nodes = patched_layout_children
                .iter()
                .map(|child| child.node)
                .collect::<Vec<_>>();
            taffy.set_node_context(layout_node.node, None)?;
            taffy.set_children(layout_node.node, &child_nodes)?;
        }
        layout_node.children = patched_layout_children;
        Ok(next)
    })
}

fn remove_layout_subtree(
    taffy: &mut TaffyTree<MeasureContext>,
    layout_node: &LayoutNode,
) -> Result<(), taffy::TaffyError> {
    for child in &layout_node.children {
        remove_layout_subtree(taffy, child)?;
    }
    taffy.remove(layout_node.node)?;
    Ok(())
}

fn resolved_at_path<'a, VM>(
    node: &'a ResolvedElement<VM>,
    path: &[usize],
) -> &'a ResolvedElement<VM> {
    if path.is_empty() {
        return node;
    }
    let ResolvedWidgetKind::Container { children, .. } = &node.kind else {
        panic!("resolved path descends into a non-container widget");
    };
    resolved_at_path(&children[path[0]], &path[1..])
}

fn patch_resolved_at_path<VM>(
    node: &mut ResolvedElement<VM>,
    path: &[usize],
    next: ResolvedElement<VM>,
) -> bool {
    if path.is_empty() {
        *node = next;
        return true;
    }

    let ResolvedWidgetKind::Container { children, .. } = &mut node.kind else {
        return false;
    };
    let Some(child) = children.get_mut(path[0]) else {
        return false;
    };
    patch_resolved_at_path(child, &path[1..], next)
}

fn layout_at_path<'a>(node: &'a LayoutNode, path: &[usize]) -> &'a LayoutNode {
    if path.is_empty() {
        return node;
    }
    layout_at_path(&node.children[path[0]], &path[1..])
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
