use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use crate::animation::{AnimationEngine, AnimationKey, WidgetProperty};
use taffy::NodeId as TaffyNodeId;

use crate::foundation::color::Color;
use crate::foundation::view_model::{Command, ValueCommand};
use crate::text::font::FontWeight;
use crate::ui::layout::{Align, Axis, Insets, Justify, Value, Wrap};

use super::text::Text;

static NEXT_WIDGET_ID: AtomicU64 = AtomicU64::new(1);

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
}

#[derive(Clone, Default)]
pub struct ScenePrimitives {
    pub shapes: Vec<RenderPrimitive>,
    pub texts: Vec<TextPrimitive>,
    pub overlay_shapes: Vec<RenderPrimitive>,
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
    Button {
        label: Text,
    },
    Input {
        text: Text,
        placeholder: Text,
        on_change: Option<ValueCommand<VM, String>>,
    },
}

#[derive(Clone)]
pub(crate) enum MeasureContext {
    None,
    Text(Text),
    Button(Text),
    Input { text: Text, placeholder: Text },
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
    pub interaction: HitInteraction<VM>,
}

pub(crate) struct ComputedScene<VM> {
    pub scene: ScenePrimitives,
    pub hit_regions: Vec<HitRegion<VM>>,
    pub ime_cursor_area: Option<Rect>,
}

impl<VM> Default for ComputedScene<VM> {
    fn default() -> Self {
        Self {
            scene: ScenePrimitives::default(),
            hit_regions: Vec::new(),
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
    pub ime_cursor_area: Option<Rect>,
}
