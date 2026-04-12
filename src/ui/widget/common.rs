use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

use crate::animation::{AnimationEngine, AnimationKey, WidgetProperty};
use crate::media::{TextureFrame, VideoPlaybackStatus};
use taffy::NodeId as TaffyNodeId;

use crate::foundation::color::Color;
use crate::foundation::view_model::{Command, ValueCommand};
use crate::text::font::FontWeight;
use crate::ui::layout::{Align, Axis, Insets, Justify, Overflow, ScrollbarStyle, Value, Wrap};

use super::text::Text;
use super::{image::Image, video::Video};

static NEXT_WIDGET_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CursorStyle {
    Default,
    Pointer,
    Text,
    Crosshair,
    Move,
    NotAllowed,
    Grab,
    Grabbing,
    EwResize,
    NsResize,
    NeswResize,
    NwseResize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct WidgetId(u64);

impl WidgetId {
    pub(crate) fn next() -> Self {
        Self(NEXT_WIDGET_ID.fetch_add(1, Ordering::Relaxed))
    }

    pub(crate) fn raw(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub fn contains(self, point: Point) -> bool {
        point.x >= self.x
            && point.x <= self.x + self.width
            && point.y >= self.y
            && point.y <= self.y + self.height
    }

    pub(crate) fn inset(self, insets: Insets) -> Self {
        let width = (self.width - insets.left - insets.right).max(0.0);
        let height = (self.height - insets.top - insets.bottom).max(0.0);
        Self {
            x: self.x + insets.left,
            y: self.y + insets.top,
            width,
            height,
        }
    }

    pub(crate) fn right(self) -> f32 {
        self.x + self.width
    }

    pub(crate) fn bottom(self) -> f32 {
        self.y + self.height
    }

    pub(crate) fn is_empty(self) -> bool {
        self.width <= 0.0 || self.height <= 0.0
    }

    pub(crate) fn intersect(self, other: Self) -> Option<Self> {
        let x = self.x.max(other.x);
        let y = self.y.max(other.y);
        let right = self.right().min(other.right());
        let bottom = self.bottom().min(other.bottom());
        let width = right - x;
        let height = bottom - y;
        (width > 0.0 && height > 0.0).then_some(Self::new(x, y, width, height))
    }

    pub(crate) fn union(self, other: Self) -> Self {
        let x = self.x.min(other.x);
        let y = self.y.min(other.y);
        let right = self.right().max(other.right());
        let bottom = self.bottom().max(other.bottom());
        Self::new(x, y, right - x, bottom - y)
    }
}

#[derive(Clone)]
pub struct VisualStyle {
    pub border_color: Value<Color>,
    pub border_radius: Value<f32>,
    pub border_width: Value<f32>,
    pub opacity: Value<f32>,
    pub offset: Value<Point>,
}

impl Default for VisualStyle {
    fn default() -> Self {
        Self {
            border_color: Value::Static(Color::TRANSPARENT),
            border_radius: Value::Static(0.0),
            border_width: Value::Static(0.0),
            opacity: Value::Static(1.0),
            offset: Value::Static(Point::ZERO),
        }
    }
}

pub(crate) struct InteractionHandlers<VM> {
    pub on_click: Option<Command<VM>>,
    pub on_double_click: Option<Command<VM>>,
    pub on_focus: Option<Command<VM>>,
    pub on_blur: Option<Command<VM>>,
    pub on_mouse_enter: Option<Command<VM>>,
    pub on_mouse_leave: Option<Command<VM>>,
    pub on_mouse_move: Option<ValueCommand<VM, Point>>,
    pub cursor_style: Option<CursorStyle>,
}

pub(crate) enum MediaEventPhase {
    Loading,
    Success,
    Error(String),
    Play,
    Pause,
    Resume,
    End,
    Seek,
}

impl Clone for MediaEventPhase {
    fn clone(&self) -> Self {
        match self {
            Self::Loading => Self::Loading,
            Self::Success => Self::Success,
            Self::Error(error) => Self::Error(error.clone()),
            Self::Play => Self::Play,
            Self::Pause => Self::Pause,
            Self::Resume => Self::Resume,
            Self::End => Self::End,
            Self::Seek => Self::Seek,
        }
    }
}

impl PartialEq for MediaEventPhase {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Loading, Self::Loading) | (Self::Success, Self::Success) => true,
            (Self::Play, Self::Play)
            | (Self::Pause, Self::Pause)
            | (Self::Resume, Self::Resume)
            | (Self::End, Self::End)
            | (Self::Seek, Self::Seek) => true,
            (Self::Error(left), Self::Error(right)) => left == right,
            _ => false,
        }
    }
}

impl Eq for MediaEventPhase {}

pub(crate) struct MediaEventHandlers<VM> {
    pub on_loading: Option<Command<VM>>,
    pub on_success: Option<Command<VM>>,
    pub on_error: Option<ValueCommand<VM, String>>,
    pub on_play: Option<Command<VM>>,
    pub on_pause: Option<Command<VM>>,
    pub on_resume: Option<Command<VM>>,
    pub on_end: Option<Command<VM>>,
    pub on_seek: Option<Command<VM>>,
}

impl<VM> Clone for MediaEventHandlers<VM> {
    fn clone(&self) -> Self {
        Self {
            on_loading: self.on_loading.clone(),
            on_success: self.on_success.clone(),
            on_error: self.on_error.clone(),
            on_play: self.on_play.clone(),
            on_pause: self.on_pause.clone(),
            on_resume: self.on_resume.clone(),
            on_end: self.on_end.clone(),
            on_seek: self.on_seek.clone(),
        }
    }
}

impl<VM> Default for MediaEventHandlers<VM> {
    fn default() -> Self {
        Self {
            on_loading: None,
            on_success: None,
            on_error: None,
            on_play: None,
            on_pause: None,
            on_resume: None,
            on_end: None,
            on_seek: None,
        }
    }
}

impl<VM> MediaEventHandlers<VM> {
    pub(crate) fn has_any(&self) -> bool {
        self.on_loading.is_some()
            || self.on_success.is_some()
            || self.on_error.is_some()
            || self.on_play.is_some()
            || self.on_pause.is_some()
            || self.on_resume.is_some()
            || self.on_end.is_some()
            || self.on_seek.is_some()
    }
}

#[derive(Clone)]
pub(crate) struct MediaEventState<VM> {
    pub widget_id: WidgetId,
    pub media_phase: Option<MediaEventPhase>,
    pub video_status: Option<VideoPlaybackStatus>,
    pub seek_generation: Option<u64>,
    pub handlers: MediaEventHandlers<VM>,
}

impl<VM> Clone for InteractionHandlers<VM> {
    fn clone(&self) -> Self {
        Self {
            on_click: self.on_click.clone(),
            on_double_click: self.on_double_click.clone(),
            on_focus: self.on_focus.clone(),
            on_blur: self.on_blur.clone(),
            on_mouse_enter: self.on_mouse_enter.clone(),
            on_mouse_leave: self.on_mouse_leave.clone(),
            on_mouse_move: self.on_mouse_move.clone(),
            cursor_style: self.cursor_style,
        }
    }
}

impl<VM> Default for InteractionHandlers<VM> {
    fn default() -> Self {
        Self {
            on_click: None,
            on_double_click: None,
            on_focus: None,
            on_blur: None,
            on_mouse_enter: None,
            on_mouse_leave: None,
            on_mouse_move: None,
            cursor_style: None,
        }
    }
}

impl<VM> InteractionHandlers<VM> {
    pub(crate) fn has_any(&self) -> bool {
        self.on_click.is_some()
            || self.on_double_click.is_some()
            || self.on_focus.is_some()
            || self.on_blur.is_some()
            || self.on_mouse_enter.is_some()
            || self.on_mouse_leave.is_some()
            || self.on_mouse_move.is_some()
            || self.cursor_style.is_some()
    }
}

impl Value<Color> {
    pub(crate) fn resolve_widget(
        &self,
        animations: &mut AnimationEngine,
        widget_id: WidgetId,
        property: WidgetProperty,
        now: Instant,
    ) -> Color {
        animations.resolve_color(
            AnimationKey::Widget {
                id: widget_id.raw(),
                property,
            },
            self.resolve(),
            self.transition(),
            now,
        )
    }
}

impl Value<f32> {
    pub(crate) fn resolve_widget(
        &self,
        animations: &mut AnimationEngine,
        widget_id: WidgetId,
        property: WidgetProperty,
        now: Instant,
    ) -> f32 {
        animations.resolve_f32(
            AnimationKey::Widget {
                id: widget_id.raw(),
                property,
            },
            self.resolve(),
            self.transition(),
            now,
        )
    }

    pub(crate) fn resolve_widget_clamped(
        &self,
        animations: &mut AnimationEngine,
        widget_id: WidgetId,
        property: WidgetProperty,
        now: Instant,
        min: f32,
        max: f32,
    ) -> f32 {
        self.resolve_widget(animations, widget_id, property, now)
            .clamp(min, max)
    }
}

impl Value<Point> {
    pub(crate) fn resolve_widget(
        &self,
        animations: &mut AnimationEngine,
        widget_id: WidgetId,
        property: WidgetProperty,
        now: Instant,
    ) -> Point {
        animations.resolve_point(
            AnimationKey::Widget {
                id: widget_id.raw(),
                property,
            },
            self.resolve(),
            self.transition(),
            now,
        )
    }
}

impl Value<Insets> {
    pub(crate) fn resolve_widget(
        &self,
        animations: &mut AnimationEngine,
        widget_id: WidgetId,
        property: WidgetProperty,
        now: Instant,
    ) -> Insets {
        animations.resolve_insets(
            AnimationKey::Widget {
                id: widget_id.raw(),
                property,
            },
            self.resolve(),
            self.transition(),
            now,
        )
    }
}

#[derive(Clone, Copy)]
pub struct RenderPrimitive {
    pub rect: Rect,
    pub color: Color,
    pub corner_radius: f32,
    pub stroke_width: f32,
    pub clip_rect: Option<Rect>,
}

#[derive(Clone)]
pub struct TextPrimitive {
    pub content: String,
    pub frame: Rect,
    pub color: Color,
    pub font_family: Option<String>,
    pub font_size: f32,
    pub font_weight: FontWeight,
    pub line_height: f32,
    pub letter_spacing: f32,
    pub clip_rect: Option<Rect>,
}

#[derive(Clone)]
pub struct TexturePrimitive {
    pub texture: Arc<TextureFrame>,
    pub frame: Rect,
    pub clip_rect: Option<Rect>,
}

#[derive(Clone, Default)]
pub struct ScenePrimitives {
    pub shapes: Vec<RenderPrimitive>,
    pub textures: Vec<TexturePrimitive>,
    pub texts: Vec<TextPrimitive>,
    pub overlay_shapes: Vec<RenderPrimitive>,
    pub overlay_texts: Vec<TextPrimitive>,
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum ContainerKind {
    Flow,
    Stack,
    Row,
    Column,
    Grid { columns: usize },
    Flex { direction: Axis, wrap: Wrap },
}

#[derive(Clone, Debug)]
pub(crate) struct ContainerLayout {
    pub kind: ContainerKind,
    pub padding: Value<Insets>,
    pub gap: Value<f32>,
    pub justify: Justify,
    pub align: Align,
    pub align_x: Option<Align>,
    pub align_y: Option<Align>,
    pub overflow_x: Overflow,
    pub overflow_y: Overflow,
    pub scrollbar_style: ScrollbarStyle,
}

impl ContainerLayout {
    pub(crate) fn flow() -> Self {
        Self {
            kind: ContainerKind::Flow,
            padding: Value::Static(Insets::ZERO),
            gap: Value::Static(0.0),
            justify: Justify::Start,
            align: Align::Start,
            align_x: None,
            align_y: None,
            overflow_x: Overflow::Hidden,
            overflow_y: Overflow::Hidden,
            scrollbar_style: ScrollbarStyle::default(),
        }
    }
}

pub(crate) enum WidgetKind<VM> {
    Container {
        layout: ContainerLayout,
        children: Vec<super::core::Element<VM>>,
    },
    Text {
        text: Text,
    },
    Image {
        image: Image,
    },
    Button {
        label: Text,
    },
    Input {
        text: Text,
        placeholder: Text,
        on_change: Option<ValueCommand<VM, String>>,
    },
    Video {
        video: Video,
    },
}

#[derive(Clone)]
pub(crate) enum MeasureContext {
    None,
    Text(Text),
    Image(Image),
    Button(Text),
    Input { text: Text, placeholder: Text },
    Video { id: WidgetId, video: Video },
}

pub(crate) struct LayoutNode {
    pub node: TaffyNodeId,
    pub children: Vec<LayoutNode>,
}

pub(crate) enum HitInteraction<VM> {
    Widget {
        id: WidgetId,
        interactions: InteractionHandlers<VM>,
        focusable: bool,
    },
    FocusInput {
        id: WidgetId,
        interactions: InteractionHandlers<VM>,
        on_change: Option<ValueCommand<VM, String>>,
        text: String,
    },
}

impl<VM> Clone for HitInteraction<VM> {
    fn clone(&self) -> Self {
        match self {
            Self::Widget {
                id,
                interactions,
                focusable,
            } => Self::Widget {
                id: *id,
                interactions: interactions.clone(),
                focusable: *focusable,
            },
            Self::FocusInput {
                id,
                interactions,
                on_change,
                text,
            } => Self::FocusInput {
                id: *id,
                interactions: interactions.clone(),
                on_change: on_change.clone(),
                text: text.clone(),
            },
        }
    }
}

#[derive(Clone)]
pub(crate) struct HitRegion<VM> {
    pub rect: Rect,
    pub clip_rect: Option<Rect>,
    pub interaction: HitInteraction<VM>,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ScrollRegion {
    pub id: WidgetId,
    pub content_viewport: Rect,
    pub visible_frame: Rect,
    pub content_bounds: Rect,
    pub scroll_offset: Point,
    pub overflow_x: Overflow,
    pub overflow_y: Overflow,
    pub horizontal_track: Option<Rect>,
    pub horizontal_thumb: Option<Rect>,
    pub vertical_track: Option<Rect>,
    pub vertical_thumb: Option<Rect>,
}

impl ScrollRegion {
    pub(crate) fn max_offset(self) -> Point {
        Point {
            x: (self.content_bounds.right() - self.content_viewport.right()).max(0.0),
            y: (self.content_bounds.bottom() - self.content_viewport.bottom()).max(0.0),
        }
    }

    pub(crate) fn can_scroll_x(self) -> bool {
        self.overflow_x == Overflow::Scroll && self.max_offset().x > 0.0
    }

    pub(crate) fn can_scroll_y(self) -> bool {
        self.overflow_y == Overflow::Scroll && self.max_offset().y > 0.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum ScrollbarAxis {
    Horizontal,
    Vertical,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct ScrollbarHandle {
    pub id: WidgetId,
    pub axis: ScrollbarAxis,
}

pub(crate) struct ComputedScene<VM> {
    pub scene: ScenePrimitives,
    pub hit_regions: Vec<HitRegion<VM>>,
    pub scroll_regions: Vec<ScrollRegion>,
    pub ime_cursor_area: Option<Rect>,
}

impl<VM> Default for ComputedScene<VM> {
    fn default() -> Self {
        Self {
            scene: ScenePrimitives::default(),
            hit_regions: Vec::new(),
            scroll_regions: Vec::new(),
            ime_cursor_area: None,
        }
    }
}

impl Point {
    pub const ZERO: Self = Self { x: 0.0, y: 0.0 };
}

#[derive(Clone, Debug, Default)]
pub(crate) struct InputEditState {
    pub cursor: usize,
    pub anchor: usize,
    pub composition: Option<CompositionState>,
}

impl InputEditState {
    pub(crate) fn caret_at(text: &str) -> Self {
        let end = text.len();
        Self {
            cursor: end,
            anchor: end,
            composition: None,
        }
    }

    pub(crate) fn selection_range(&self) -> Option<(usize, usize)> {
        (self.cursor != self.anchor)
            .then_some((self.cursor.min(self.anchor), self.cursor.max(self.anchor)))
    }

    pub(crate) fn clamped_to(mut self, text: &str) -> Self {
        let len = text.len();
        self.cursor = self.cursor.min(len);
        self.anchor = self.anchor.min(len);
        if let Some(composition) = &mut self.composition {
            composition.replace_range.0 = composition.replace_range.0.min(len);
            composition.replace_range.1 = composition.replace_range.1.min(len);
            if composition.replace_range.0 > composition.replace_range.1 {
                composition.replace_range =
                    (composition.replace_range.1, composition.replace_range.1);
            }
        }
        self
    }
}

#[derive(Clone, Debug)]
pub(crate) struct CompositionState {
    pub replace_range: (usize, usize),
    pub text: String,
    pub cursor: Option<(usize, usize)>,
}

#[derive(Clone)]
pub(crate) struct InputSnapshot<VM> {
    pub id: WidgetId,
    pub on_change: Option<ValueCommand<VM, String>>,
    pub text: String,
}

#[derive(Clone, Default)]
pub(crate) struct RenderedWidgetScene {
    pub primitives: ScenePrimitives,
    pub scroll_regions: Vec<ScrollRegion>,
    pub ime_cursor_area: Option<Rect>,
}
