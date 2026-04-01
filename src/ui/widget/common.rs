use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use crate::animation::{AnimationEngine, AnimationKey, Transition, WidgetProperty};
use taffy::NodeId as TaffyNodeId;

use crate::foundation::binding::Binding;
use crate::foundation::color::Color;
use crate::foundation::view_model::{Command, ValueCommand};
use crate::text::font::FontWeight;
use crate::ui::layout::{Align, Axis, Insets, Justify, Wrap};

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
    pub opacity: Value<f32>,
    pub offset: Value<Point>,
}

impl Default for VisualStyle {
    fn default() -> Self {
        Self {
            opacity: Value::Static(1.0),
            offset: Value::Static(Point::ZERO),
        }
    }
}

#[derive(Clone)]
pub enum Value<T> {
    Static(T),
    Bound(Binding<T>),
}

impl<T: Clone> Value<T> {
    pub fn resolve(&self) -> T {
        match self {
            Self::Static(value) => value.clone(),
            Self::Bound(binding) => binding.get(),
        }
    }

    pub(crate) fn transition(&self) -> Option<Transition> {
        match self {
            Self::Static(_) => None,
            Self::Bound(binding) => binding.transition(),
        }
    }
}

impl<T> From<T> for Value<T> {
    fn from(value: T) -> Self {
        Self::Static(value)
    }
}

impl<T> From<Binding<T>> for Value<T> {
    fn from(value: Binding<T>) -> Self {
        Self::Bound(value)
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
            self.resolve().clamp(0.0, 1.0),
            self.transition(),
            now,
        )
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

#[derive(Clone, Copy)]
pub struct RenderPrimitive {
    pub rect: Rect,
    pub color: Color,
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

#[derive(Clone, Copy, Debug)]
pub(crate) struct ContainerLayout {
    pub kind: ContainerKind,
    pub padding: Insets,
    pub gap: f32,
    pub justify: Justify,
    pub align: Align,
    pub align_x: Option<Align>,
    pub align_y: Option<Align>,
}

impl ContainerLayout {
    pub(crate) fn flow() -> Self {
        Self {
            kind: ContainerKind::Flow,
            padding: Insets::ZERO,
            gap: 0.0,
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
        on_click: Option<Command<VM>>,
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

#[derive(Clone)]
pub(crate) enum HitInteraction<VM> {
    Command(Command<VM>),
    FocusInput {
        id: WidgetId,
        on_change: Option<ValueCommand<VM, String>>,
        text: String,
    },
}

#[derive(Clone)]
pub(crate) struct HitRegion<VM> {
    pub rect: Rect,
    pub interaction: HitInteraction<VM>,
}

pub(crate) struct ComputedScene<VM> {
    pub scene: ScenePrimitives,
    pub hit_regions: Vec<HitRegion<VM>>,
}

impl<VM> Default for ComputedScene<VM> {
    fn default() -> Self {
        Self {
            scene: ScenePrimitives::default(),
            hit_regions: Vec::new(),
        }
    }
}

impl Point {
    pub const ZERO: Self = Self { x: 0.0, y: 0.0 };
}
