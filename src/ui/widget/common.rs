use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

use crate::animation::{AnimationEngine, AnimationKey, WidgetProperty};
use crate::media::TextureFrame;
use taffy::NodeId as TaffyNodeId;

use crate::foundation::color::Color;
use crate::foundation::view_model::{Command, ValueCommand};
use crate::text::font::FontWeight;
use crate::ui::layout::{
    Align, Axis, Insets, Justify, Length, Overflow, ScrollbarStyle, Track, Value, Wrap,
};
use crate::ui::unit::{Dp, UnitContext};
#[cfg(feature = "video")]
use crate::video::VideoSurface;

use super::canvas::{CanvasItem, CanvasItemId, CanvasPointerEvent};
use super::image::Image;
use super::text::Text;

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
    pub x: Dp,
    pub y: Dp,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x: Dp,
    pub y: Dp,
    pub width: Dp,
    pub height: Dp,
}

impl Rect {
    pub fn new(
        x: impl Into<Dp>,
        y: impl Into<Dp>,
        width: impl Into<Dp>,
        height: impl Into<Dp>,
    ) -> Self {
        Self {
            x: x.into(),
            y: y.into(),
            width: width.into(),
            height: height.into(),
        }
    }

    pub fn contains(self, point: Point) -> bool {
        point.x >= self.x
            && point.x <= self.x + self.width
            && point.y >= self.y
            && point.y <= self.y + self.height
    }

    pub(crate) fn inset(self, insets: Insets) -> Self {
        let width = (self.width - insets.left - insets.right).max(Dp::ZERO);
        let height = (self.height - insets.top - insets.bottom).max(Dp::ZERO);
        Self {
            x: self.x + insets.left,
            y: self.y + insets.top,
            width,
            height,
        }
    }

    pub(crate) fn right(self) -> Dp {
        self.x + self.width
    }

    pub(crate) fn bottom(self) -> Dp {
        self.y + self.height
    }

    pub(crate) fn is_empty(self) -> bool {
        self.width <= Dp::ZERO || self.height <= Dp::ZERO
    }

    pub(crate) fn intersect(self, other: Self) -> Option<Self> {
        let x = self.x.max(other.x);
        let y = self.y.max(other.y);
        let right = self.right().min(other.right());
        let bottom = self.bottom().min(other.bottom());
        let width = right - x;
        let height = bottom - y;
        (width > Dp::ZERO && height > Dp::ZERO).then_some(Self::new(x, y, width, height))
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
    pub border_radius: Value<Dp>,
    pub border_width: Value<Dp>,
    pub opacity: Value<f32>,
    pub offset: Value<Point>,
}

impl Default for VisualStyle {
    fn default() -> Self {
        Self {
            border_color: Value::Static(Color::TRANSPARENT),
            border_radius: Value::Static(Dp::ZERO),
            border_width: Value::Static(Dp::ZERO),
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
    pub cursor_style: Option<Value<CursorStyle>>,
}

pub(crate) struct CanvasItemInteractionHandlers<VM> {
    pub on_click: Option<ValueCommand<VM, CanvasPointerEvent>>,
    pub on_mouse_enter: Option<ValueCommand<VM, CanvasPointerEvent>>,
    pub on_mouse_leave: Option<ValueCommand<VM, CanvasPointerEvent>>,
    pub on_mouse_move: Option<ValueCommand<VM, CanvasPointerEvent>>,
}

#[derive(Clone, PartialEq, Eq)]
pub(crate) enum MediaEventPhase {
    Loading,
    Success,
    Error(String),
}

pub(crate) struct MediaEventHandlers<VM> {
    pub on_loading: Option<Command<VM>>,
    pub on_success: Option<Command<VM>>,
    pub on_error: Option<ValueCommand<VM, String>>,
}

impl<VM> Clone for MediaEventHandlers<VM> {
    fn clone(&self) -> Self {
        Self {
            on_loading: self.on_loading.clone(),
            on_success: self.on_success.clone(),
            on_error: self.on_error.clone(),
        }
    }
}

impl<VM> Default for MediaEventHandlers<VM> {
    fn default() -> Self {
        Self {
            on_loading: None,
            on_success: None,
            on_error: None,
        }
    }
}

impl<VM> MediaEventHandlers<VM> {
    pub(crate) fn has_any(&self) -> bool {
        self.on_loading.is_some() || self.on_success.is_some() || self.on_error.is_some()
    }

    pub(crate) fn scope<RootVm: 'static>(
        self,
        selector: Arc<dyn for<'a> Fn(&'a mut RootVm) -> &'a mut VM + Send + Sync>,
    ) -> MediaEventHandlers<RootVm>
    where
        VM: 'static,
    {
        MediaEventHandlers {
            on_loading: self
                .on_loading
                .map(|command| command.scope(selector.clone())),
            on_success: self
                .on_success
                .map(|command| command.scope(selector.clone())),
            on_error: self.on_error.map(|command| command.scope(selector)),
        }
    }
}

#[derive(Clone)]
pub(crate) struct MediaEventState<VM> {
    pub widget_id: WidgetId,
    pub media_phase: Option<MediaEventPhase>,
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
            cursor_style: self.cursor_style.clone(),
        }
    }
}

impl<VM> Clone for CanvasItemInteractionHandlers<VM> {
    fn clone(&self) -> Self {
        Self {
            on_click: self.on_click.clone(),
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
            cursor_style: None,
        }
    }
}

impl<VM> Default for CanvasItemInteractionHandlers<VM> {
    fn default() -> Self {
        Self {
            on_click: None,
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
            || self.cursor_style.is_some()
    }

    pub(crate) fn scope<RootVm: 'static>(
        self,
        selector: Arc<dyn for<'a> Fn(&'a mut RootVm) -> &'a mut VM + Send + Sync>,
    ) -> InteractionHandlers<RootVm>
    where
        VM: 'static,
    {
        InteractionHandlers {
            on_click: self.on_click.map(|command| command.scope(selector.clone())),
            on_double_click: self
                .on_double_click
                .map(|command| command.scope(selector.clone())),
            on_focus: self.on_focus.map(|command| command.scope(selector.clone())),
            on_blur: self.on_blur.map(|command| command.scope(selector.clone())),
            on_mouse_enter: self
                .on_mouse_enter
                .map(|command| command.scope(selector.clone())),
            on_mouse_leave: self
                .on_mouse_leave
                .map(|command| command.scope(selector.clone())),
            on_mouse_move: self.on_mouse_move.map(|command| command.scope(selector)),
            cursor_style: self.cursor_style,
        }
    }
}

impl<VM: 'static> CanvasItemInteractionHandlers<VM> {
    pub(crate) fn scope<RootVm: 'static>(
        self,
        selector: Arc<dyn for<'a> Fn(&'a mut RootVm) -> &'a mut VM + Send + Sync>,
    ) -> CanvasItemInteractionHandlers<RootVm> {
        CanvasItemInteractionHandlers {
            on_click: self.on_click.map(|command| command.scope(selector.clone())),
            on_mouse_enter: self
                .on_mouse_enter
                .map(|command| command.scope(selector.clone())),
            on_mouse_leave: self
                .on_mouse_leave
                .map(|command| command.scope(selector.clone())),
            on_mouse_move: self.on_mouse_move.map(|command| command.scope(selector)),
        }
    }
}

impl<VM> CanvasItemInteractionHandlers<VM> {
    pub(crate) fn has_any(&self) -> bool {
        self.on_click.is_some()
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

impl Value<Dp> {
    pub(crate) fn resolve_widget(
        &self,
        animations: &mut AnimationEngine,
        widget_id: WidgetId,
        property: WidgetProperty,
        now: Instant,
    ) -> Dp {
        animations.resolve_dp(
            AnimationKey::Widget {
                id: widget_id.raw(),
                property,
            },
            self.resolve(),
            self.transition(),
            now,
        )
    }

    pub(crate) fn resolve_widget_to_logical(
        &self,
        animations: &mut AnimationEngine,
        widget_id: WidgetId,
        property: WidgetProperty,
        now: Instant,
        units: UnitContext,
    ) -> f32 {
        units.resolve_dp(self.resolve_widget(animations, widget_id, property, now))
    }
}

impl Value<Length> {
    pub(crate) fn resolve_widget(
        &self,
        animations: &mut AnimationEngine,
        widget_id: WidgetId,
        property: WidgetProperty,
        now: Instant,
    ) -> Length {
        match self {
            Value::Static(value) => *value,
            Value::Bound(binding) => {
                let target = binding.get();
                match target {
                    Length::Px(target_dp) => Length::Px(animations.resolve_dp(
                        AnimationKey::Widget {
                            id: widget_id.raw(),
                            property,
                        },
                        target_dp,
                        binding.transition(),
                        now,
                    )),
                    Length::Auto | Length::Percent(_) => target,
                }
            }
        }
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

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MeshVertex {
    pub position: [f32; 2],
    pub local_position: [f32; 2],
    pub brush_meta: [f32; 4],
    pub gradient_data0: [f32; 4],
    pub gradient_data1: [f32; 4],
    pub stop_offsets0: [f32; 4],
    pub stop_offsets1: [f32; 4],
    pub stop_colors: [[f32; 4]; 8],
}

#[derive(Clone)]
pub struct MeshPrimitive {
    pub vertices: Arc<[MeshVertex]>,
    pub(crate) triangles: Arc<[[Point; 3]]>,
    pub clip_rect: Option<Rect>,
}

#[derive(Clone)]
pub(crate) enum RenderCommand {
    Shape(RenderPrimitive),
    Texture(TexturePrimitive),
    Text(TextPrimitive),
    Mesh(MeshPrimitive),
}

#[derive(Clone, Default)]
pub struct ScenePrimitives {
    pub shapes: Vec<RenderPrimitive>,
    pub meshes: Vec<MeshPrimitive>,
    pub textures: Vec<TexturePrimitive>,
    pub texts: Vec<TextPrimitive>,
    pub overlay_shapes: Vec<RenderPrimitive>,
    #[allow(dead_code)]
    pub overlay_meshes: Vec<MeshPrimitive>,
    #[allow(dead_code)]
    pub overlay_texts: Vec<TextPrimitive>,
    pub(crate) commands: Vec<RenderCommand>,
    pub(crate) overlay_commands: Vec<RenderCommand>,
}

impl ScenePrimitives {
    pub(crate) fn push_shape(&mut self, primitive: RenderPrimitive) {
        self.shapes.push(primitive);
        self.commands.push(RenderCommand::Shape(primitive));
    }

    pub(crate) fn push_mesh(&mut self, primitive: MeshPrimitive) {
        self.meshes.push(primitive.clone());
        self.commands.push(RenderCommand::Mesh(primitive));
    }

    pub(crate) fn push_texture(&mut self, primitive: TexturePrimitive) {
        self.textures.push(primitive.clone());
        self.commands.push(RenderCommand::Texture(primitive));
    }

    pub(crate) fn push_text(&mut self, primitive: TextPrimitive) {
        self.texts.push(primitive.clone());
        self.commands.push(RenderCommand::Text(primitive));
    }

    pub(crate) fn push_overlay_shape(&mut self, primitive: RenderPrimitive) {
        self.overlay_shapes.push(primitive);
        self.overlay_commands.push(RenderCommand::Shape(primitive));
    }

    #[allow(dead_code)]
    pub(crate) fn push_overlay_mesh(&mut self, primitive: MeshPrimitive) {
        self.overlay_meshes.push(primitive.clone());
        self.overlay_commands.push(RenderCommand::Mesh(primitive));
    }

    #[allow(dead_code)]
    pub(crate) fn push_overlay_text(&mut self, primitive: TextPrimitive) {
        self.overlay_texts.push(primitive.clone());
        self.overlay_commands.push(RenderCommand::Text(primitive));
    }
}

#[derive(Clone, Debug)]
pub(crate) enum ContainerKind {
    Flow,
    Stack,
    Grid {
        columns: Vec<Track>,
        rows: Vec<Track>,
    },
    Flex {
        direction: Axis,
        wrap: Wrap,
    },
}

#[derive(Clone, Debug)]
pub(crate) struct ContainerLayout {
    pub kind: ContainerKind,
    pub padding: Value<Insets>,
    pub gap: Value<crate::ui::layout::Length>,
    pub justify: Justify,
    pub align: Align,
    pub overflow_x: Overflow,
    pub overflow_y: Overflow,
    pub scrollbar_style: ScrollbarStyle,
}

impl ContainerLayout {
    pub(crate) fn flow() -> Self {
        Self {
            kind: ContainerKind::Flow,
            padding: Value::Static(Insets::ZERO),
            gap: Value::Static(crate::ui::layout::Length::Px(Dp::ZERO)),
            justify: Justify::Start,
            align: Align::Start,
            overflow_x: Overflow::Hidden,
            overflow_y: Overflow::Hidden,
            scrollbar_style: ScrollbarStyle::default(),
        }
    }
}

pub(crate) enum ChildSource<VM> {
    Static(Vec<super::core::Element<VM>>),
    Dynamic(Arc<dyn Fn() -> Vec<super::core::Element<VM>> + Send + Sync>),
}

impl<VM> ChildSource<VM> {
    pub(crate) fn resolve(&self) -> Vec<super::core::Element<VM>> {
        match self {
            Self::Static(children) => children.clone(),
            Self::Dynamic(resolver) => resolver(),
        }
    }

    pub(crate) fn scope<RootVm: 'static>(
        self,
        selector: Arc<dyn for<'a> Fn(&'a mut RootVm) -> &'a mut VM + Send + Sync>,
    ) -> ChildSource<RootVm>
    where
        VM: 'static,
    {
        match self {
            Self::Static(children) => ChildSource::Static(
                children
                    .into_iter()
                    .map(|child| child.scope_with_selector(selector.clone()))
                    .collect(),
            ),
            Self::Dynamic(resolver) => ChildSource::Dynamic(Arc::new(move || {
                resolver()
                    .into_iter()
                    .map(|child| child.scope_with_selector(selector.clone()))
                    .collect()
            })),
        }
    }
}

impl<VM> Clone for ChildSource<VM> {
    fn clone(&self) -> Self {
        match self {
            Self::Static(children) => Self::Static(children.clone()),
            Self::Dynamic(resolver) => Self::Dynamic(resolver.clone()),
        }
    }
}

pub(crate) enum WidgetKind<VM> {
    Container {
        layout: ContainerLayout,
        children: Vec<ChildSource<VM>>,
    },
    Text {
        text: Text,
    },
    Image {
        image: Image,
    },
    Canvas {
        items: Value<Vec<CanvasItem>>,
        item_interactions: CanvasItemInteractionHandlers<VM>,
    },
    #[cfg(feature = "video")]
    VideoSurface {
        video: VideoSurface,
    },
    Button {
        label: Text,
        disabled: Value<bool>,
    },
    Switch {
        checked: Value<bool>,
        on_change: Option<ValueCommand<VM, bool>>,
        active_background: Value<Color>,
        inactive_background: Value<Color>,
        active_thumb_color: Value<Color>,
        inactive_thumb_color: Value<Color>,
        disabled: Value<bool>,
    },
    Input {
        text: Text,
        placeholder: Text,
        on_change: Option<ValueCommand<VM, String>>,
        disabled: Value<bool>,
    },
}

impl<VM> Clone for WidgetKind<VM> {
    fn clone(&self) -> Self {
        match self {
            Self::Container { layout, children } => Self::Container {
                layout: layout.clone(),
                children: children.clone(),
            },
            Self::Text { text } => Self::Text { text: text.clone() },
            Self::Image { image } => Self::Image {
                image: image.clone(),
            },
            Self::Canvas {
                items,
                item_interactions,
            } => Self::Canvas {
                items: items.clone(),
                item_interactions: item_interactions.clone(),
            },
            #[cfg(feature = "video")]
            Self::VideoSurface { video } => Self::VideoSurface {
                video: video.clone(),
            },
            Self::Button { label, disabled } => Self::Button {
                label: label.clone(),
                disabled: disabled.clone(),
            },
            Self::Switch {
                checked,
                on_change,
                active_background,
                inactive_background,
                active_thumb_color,
                inactive_thumb_color,
                disabled,
            } => Self::Switch {
                checked: checked.clone(),
                on_change: on_change.clone(),
                active_background: active_background.clone(),
                inactive_background: inactive_background.clone(),
                active_thumb_color: active_thumb_color.clone(),
                inactive_thumb_color: inactive_thumb_color.clone(),
                disabled: disabled.clone(),
            },
            Self::Input {
                text,
                placeholder,
                on_change,
                disabled,
            } => Self::Input {
                text: text.clone(),
                placeholder: placeholder.clone(),
                on_change: on_change.clone(),
                disabled: disabled.clone(),
            },
        }
    }
}

#[derive(Clone)]
pub(crate) enum MeasureContext {
    None,
    Text(Text),
    Image(Image),
    Canvas(Vec<CanvasItem>),
    #[cfg(feature = "video")]
    VideoSurface(VideoSurface),
    Button(Text),
    Switch,
    Input {
        text: Text,
        placeholder: Text,
    },
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
        frame: Rect,
        padding: Insets,
        interactions: InteractionHandlers<VM>,
        on_change: Option<ValueCommand<VM, String>>,
        text_style: Text,
        text: String,
    },
    SelectableText {
        id: WidgetId,
        frame: Rect,
        padding: Insets,
        interactions: InteractionHandlers<VM>,
        text_style: Text,
        text: String,
    },
    Switch {
        id: WidgetId,
        interactions: InteractionHandlers<VM>,
        on_change: Option<ValueCommand<VM, bool>>,
        current: bool,
    },
    CanvasItem {
        id: WidgetId,
        item_id: CanvasItemId,
        item_interactions: CanvasItemInteractionHandlers<VM>,
        canvas_origin: Point,
        item_origin: Point,
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
                frame,
                padding,
                interactions,
                on_change,
                text_style,
                text,
            } => Self::FocusInput {
                id: *id,
                frame: *frame,
                padding: *padding,
                interactions: interactions.clone(),
                on_change: on_change.clone(),
                text_style: text_style.clone(),
                text: text.clone(),
            },
            Self::SelectableText {
                id,
                frame,
                padding,
                interactions,
                text_style,
                text,
            } => Self::SelectableText {
                id: *id,
                frame: *frame,
                padding: *padding,
                interactions: interactions.clone(),
                text_style: text_style.clone(),
                text: text.clone(),
            },
            Self::Switch {
                id,
                interactions,
                on_change,
                current,
            } => Self::Switch {
                id: *id,
                interactions: interactions.clone(),
                on_change: on_change.clone(),
                current: *current,
            },
            Self::CanvasItem {
                id,
                item_id,
                item_interactions,
                canvas_origin,
                item_origin,
            } => Self::CanvasItem {
                id: *id,
                item_id: *item_id,
                item_interactions: item_interactions.clone(),
                canvas_origin: *canvas_origin,
                item_origin: *item_origin,
            },
        }
    }
}

impl<VM> HitInteraction<VM> {
    pub(crate) fn target_id(&self) -> HitTargetId {
        match self {
            Self::Widget { id, .. }
            | Self::FocusInput { id, .. }
            | Self::SelectableText { id, .. }
            | Self::Switch { id, .. } => HitTargetId::Widget(*id),
            Self::CanvasItem { id, item_id, .. } => HitTargetId::CanvasItem {
                widget_id: *id,
                item_id: *item_id,
            },
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum HitTargetId {
    Widget(WidgetId),
    CanvasItem {
        widget_id: WidgetId,
        item_id: CanvasItemId,
    },
}

#[derive(Clone)]
pub(crate) enum HitGeometry {
    Rect,
    Triangles(Arc<[[Point; 3]]>),
}

impl HitGeometry {
    pub(crate) fn contains(&self, point: Point) -> bool {
        match self {
            Self::Rect => true,
            Self::Triangles(triangles) => triangles
                .iter()
                .any(|triangle| point_in_triangle(point, triangle[0], triangle[1], triangle[2])),
        }
    }
}

#[derive(Clone)]
pub(crate) struct HitRegion<VM> {
    pub rect: Rect,
    pub clip_rect: Option<Rect>,
    pub geometry: HitGeometry,
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
        self.overflow_x == Overflow::Scroll && self.max_offset().x > Dp::ZERO
    }

    pub(crate) fn can_scroll_y(self) -> bool {
        self.overflow_y == Overflow::Scroll && self.max_offset().y > Dp::ZERO
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

#[derive(Clone)]
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

impl<VM> ComputedScene<VM> {
    pub(crate) fn rendered(&self) -> RenderedWidgetScene {
        RenderedWidgetScene {
            primitives: self.scene.clone(),
            scroll_regions: self.scroll_regions.clone(),
            ime_cursor_area: self.ime_cursor_area,
        }
    }
}

impl Point {
    pub const ZERO: Self = Self {
        x: Dp::ZERO,
        y: Dp::ZERO,
    };

    pub fn new(x: impl Into<Dp>, y: impl Into<Dp>) -> Self {
        Self {
            x: x.into(),
            y: y.into(),
        }
    }
}

fn point_in_triangle(point: Point, a: Point, b: Point, c: Point) -> bool {
    let point = (point.x.get(), point.y.get());
    let a = (a.x.get(), a.y.get());
    let b = (b.x.get(), b.y.get());
    let c = (c.x.get(), c.y.get());

    let sign = |p1: (f32, f32), p2: (f32, f32), p3: (f32, f32)| {
        (p1.0 - p3.0) * (p2.1 - p3.1) - (p2.0 - p3.0) * (p1.1 - p3.1)
    };

    let d1 = sign(point, a, b);
    let d2 = sign(point, b, c);
    let d3 = sign(point, c, a);
    let has_neg = d1 < 0.0 || d2 < 0.0 || d3 < 0.0;
    let has_pos = d1 > 0.0 || d2 > 0.0 || d3 > 0.0;
    !(has_neg && has_pos)
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct InputEditState {
    pub cursor: usize,
    pub anchor: usize,
    pub composition: Option<CompositionState>,
    pub scroll_x: Dp,
}

impl InputEditState {
    pub(crate) fn caret_at(text: &str) -> Self {
        let end = text.len();
        Self {
            cursor: end,
            anchor: end,
            composition: None,
            scroll_x: Dp::ZERO,
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
        self.scroll_x = self.scroll_x.max(Dp::ZERO);
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
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
