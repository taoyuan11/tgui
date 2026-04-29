use std::collections::HashMap;
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
use crate::ui::theme::WidgetState;
use crate::ui::unit::{Dp, UnitContext};
#[cfg(feature = "video")]
use crate::video::VideoSurface;

use super::background::{BackgroundBrush, BackgroundGradientStop, BackgroundImage};
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
    pub border_color: Option<Value<Color>>,
    pub border_radius: Option<Value<Dp>>,
    pub border_width: Option<Value<Dp>>,
    pub background_brush: Option<Value<BackgroundBrush>>,
    pub background_image: Option<Value<BackgroundImage>>,
    pub background_blur: Value<Dp>,
    pub opacity: Value<f32>,
    pub offset: Value<Point>,
}

impl Default for VisualStyle {
    fn default() -> Self {
        Self {
            border_color: None,
            border_radius: None,
            border_width: None,
            background_brush: None,
            background_image: None,
            background_blur: Value::Static(Dp::ZERO),
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

impl Value<BackgroundBrush> {
    pub(crate) fn resolve_widget(&self) -> BackgroundBrush {
        match self {
            Value::Static(value) => value.clone(),
            Value::Bound(binding) => binding.get(),
        }
    }
}

impl Value<BackgroundImage> {
    pub(crate) fn resolve_widget(&self) -> BackgroundImage {
        match self {
            Value::Static(value) => value.clone(),
            Value::Bound(binding) => binding.get(),
        }
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

#[derive(Clone, Debug, PartialEq)]
pub struct BrushPrimitive {
    pub rect: Rect,
    pub brush: BackgroundBrush,
    pub corner_radius: f32,
    pub clip_rect: Option<Rect>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BackdropBlurPrimitive {
    pub rect: Rect,
    pub corner_radius: f32,
    pub blur_radius: f32,
    pub clip_rect: Option<Rect>,
}

#[derive(Clone)]
pub struct TextPrimitive {
    pub content: String,
    pub frame: Rect,
    pub color: Color,
    pub force_color: bool,
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
    pub corner_radius: f32,
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
    BackdropBlur(BackdropBlurPrimitive),
    Brush(BrushPrimitive),
    Shape(RenderPrimitive),
    Texture(TexturePrimitive),
    Text(TextPrimitive),
    Mesh(MeshPrimitive),
}

#[derive(Clone, Default)]
pub struct ScenePrimitives {
    pub backdrop_blurs: Vec<BackdropBlurPrimitive>,
    pub brushes: Vec<BrushPrimitive>,
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
    pub(crate) fn push_backdrop_blur(&mut self, primitive: BackdropBlurPrimitive) {
        self.backdrop_blurs.push(primitive);
        self.commands.push(RenderCommand::BackdropBlur(primitive));
    }

    pub(crate) fn push_brush(&mut self, primitive: BrushPrimitive) {
        self.brushes.push(primitive.clone());
        self.commands.push(RenderCommand::Brush(primitive));
    }

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

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct BrushPrimitiveData {
    pub brush_meta: [f32; 4],
    pub gradient_data0: [f32; 4],
    pub gradient_data1: [f32; 4],
    pub stop_offsets0: [f32; 4],
    pub stop_offsets1: [f32; 4],
    pub stop_colors: [[f32; 4]; 7],
}

impl BrushPrimitiveData {
    pub(crate) fn from_background_brush(brush: &BackgroundBrush, opacity: f32) -> Option<Self> {
        match brush {
            BackgroundBrush::Solid(color) => Some(Self {
                brush_meta: [0.0, 2.0, 0.0, 0.0],
                gradient_data0: [0.0; 4],
                gradient_data1: [0.0; 4],
                stop_offsets0: [0.0, 1.0, 0.0, 0.0],
                stop_offsets1: [0.0; 4],
                stop_colors: solid_stop_colors(color.with_alpha_factor(opacity)),
            }),
            BackgroundBrush::LinearGradient(gradient) => {
                let stops = normalized_background_stops(&gradient.stops, opacity)?;
                Some(Self::gradient(
                    1.0,
                    stops.len() as f32,
                    [
                        gradient.start.x.get(),
                        gradient.start.y.get(),
                        gradient.end.x.get(),
                        gradient.end.y.get(),
                    ],
                    [0.0; 4],
                    stops,
                ))
            }
            BackgroundBrush::RadialGradient(gradient) => {
                let stops = normalized_background_stops(&gradient.stops, opacity)?;
                Some(Self::gradient(
                    2.0,
                    stops.len() as f32,
                    [0.0; 4],
                    [
                        gradient.center.x.get(),
                        gradient.center.y.get(),
                        gradient.radius.get().max(0.0001),
                        0.0,
                    ],
                    stops,
                ))
            }
        }
    }

    fn gradient(
        kind: f32,
        stop_count: f32,
        gradient_data0: [f32; 4],
        gradient_data1: [f32; 4],
        stops: Vec<BackgroundGradientStopData>,
    ) -> Self {
        let mut stop_offsets0 = [0.0; 4];
        let mut stop_offsets1 = [0.0; 4];
        let mut stop_colors = [[0.0; 4]; 7];

        for (index, stop) in stops.iter().enumerate() {
            if index < 4 {
                stop_offsets0[index] = stop.offset;
            } else {
                stop_offsets1[index - 4] = stop.offset;
            }
            stop_colors[index] = stop.color;
        }

        Self {
            brush_meta: [kind, stop_count, 0.0, 0.0],
            gradient_data0,
            gradient_data1,
            stop_offsets0,
            stop_offsets1,
            stop_colors,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct BackgroundGradientStopData {
    offset: f32,
    color: [f32; 4],
}

fn normalized_background_stops(
    stops: &[BackgroundGradientStop],
    opacity: f32,
) -> Option<Vec<BackgroundGradientStopData>> {
    if stops.is_empty() || stops.len() > 7 {
        return None;
    }

    Some(
        stops
            .iter()
            .map(|stop| {
                let color = stop.color.with_alpha_factor(opacity);
                BackgroundGradientStopData {
                    offset: stop.offset,
                    color: color.to_linear_rgba_f32(),
                }
            })
            .collect(),
    )
}

fn solid_stop_colors(color: Color) -> [[f32; 4]; 7] {
    let rgba = color.to_linear_rgba_f32();
    let mut colors = [[0.0; 4]; 7];
    colors[0] = rgba;
    colors[1] = rgba;
    colors
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
    pub padding: Option<Value<Insets>>,
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
            padding: None,
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
        variant: ButtonVariantKind,
    },
    Checkbox {
        checked: Value<bool>,
        label: Option<Text>,
        on_change: Option<ValueCommand<VM, bool>>,
        disabled: Value<bool>,
    },
    Radio {
        checked: Value<bool>,
        label: Option<Text>,
        on_change: Option<ValueCommand<VM, bool>>,
        disabled: Value<bool>,
    },
    Switch {
        checked: Value<bool>,
        on_change: Option<ValueCommand<VM, bool>>,
        active_background: Option<Value<Color>>,
        inactive_background: Option<Value<Color>>,
        active_thumb_color: Option<Value<Color>>,
        inactive_thumb_color: Option<Value<Color>>,
        disabled: Value<bool>,
    },
    Input {
        text: Text,
        placeholder: Text,
        on_change: Option<ValueCommand<VM, String>>,
        disabled: Value<bool>,
    },
    Select {
        selected_label: Value<Option<String>>,
        placeholder: Text,
        options: Vec<SelectOptionState<VM>>,
        disabled: Value<bool>,
    },
}

pub(crate) struct SelectOptionState<VM> {
    pub label: Text,
    pub selected: Value<bool>,
    pub disabled: Value<bool>,
    pub on_select: Option<Command<VM>>,
}

impl<VM> Clone for SelectOptionState<VM> {
    fn clone(&self) -> Self {
        Self {
            label: self.label.clone(),
            selected: self.selected.clone(),
            disabled: self.disabled.clone(),
            on_select: self.on_select.clone(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ButtonVariantKind {
    Primary,
    Secondary,
    Ghost,
    Danger,
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
            Self::Button {
                label,
                disabled,
                variant,
            } => Self::Button {
                label: label.clone(),
                disabled: disabled.clone(),
                variant: *variant,
            },
            Self::Checkbox {
                checked,
                label,
                on_change,
                disabled,
            } => Self::Checkbox {
                checked: checked.clone(),
                label: label.clone(),
                on_change: on_change.clone(),
                disabled: disabled.clone(),
            },
            Self::Radio {
                checked,
                label,
                on_change,
                disabled,
            } => Self::Radio {
                checked: checked.clone(),
                label: label.clone(),
                on_change: on_change.clone(),
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
            Self::Select {
                selected_label,
                placeholder,
                options,
                disabled,
            } => Self::Select {
                selected_label: selected_label.clone(),
                placeholder: placeholder.clone(),
                options: options.clone(),
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
    Button {
        label: Text,
        variant: ButtonVariantKind,
    },
    Checkbox {
        checked: bool,
        label: Option<Text>,
    },
    Radio {
        checked: bool,
        label: Option<Text>,
    },
    Switch {
        checked: bool,
    },
    Input {
        text: Text,
        placeholder: Text,
    },
    Select {
        selected_label: Option<String>,
        placeholder: Text,
    },
}

#[derive(Clone)]
pub(crate) struct LayoutNode {
    pub node: TaffyNodeId,
    pub children: Vec<LayoutNode>,
}

pub(crate) enum HitInteraction<VM> {
    Disabled {
        id: WidgetId,
    },
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
    Checkbox {
        id: WidgetId,
        interactions: InteractionHandlers<VM>,
        on_change: Option<ValueCommand<VM, bool>>,
        current: bool,
    },
    Radio {
        id: WidgetId,
        interactions: InteractionHandlers<VM>,
        on_change: Option<ValueCommand<VM, bool>>,
        current: bool,
    },
    SelectTrigger {
        id: WidgetId,
        interactions: InteractionHandlers<VM>,
    },
    SelectOption {
        id: WidgetId,
        option_index: usize,
        interactions: InteractionHandlers<VM>,
        on_select: Option<Command<VM>>,
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
            Self::Disabled { id } => Self::Disabled { id: *id },
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
            Self::Checkbox {
                id,
                interactions,
                on_change,
                current,
            } => Self::Checkbox {
                id: *id,
                interactions: interactions.clone(),
                on_change: on_change.clone(),
                current: *current,
            },
            Self::Radio {
                id,
                interactions,
                on_change,
                current,
            } => Self::Radio {
                id: *id,
                interactions: interactions.clone(),
                on_change: on_change.clone(),
                current: *current,
            },
            Self::SelectTrigger { id, interactions } => Self::SelectTrigger {
                id: *id,
                interactions: interactions.clone(),
            },
            Self::SelectOption {
                id,
                option_index,
                interactions,
                on_select,
            } => Self::SelectOption {
                id: *id,
                option_index: *option_index,
                interactions: interactions.clone(),
                on_select: on_select.clone(),
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
            Self::Disabled { id }
            | Self::Widget { id, .. }
            | Self::FocusInput { id, .. }
            | Self::SelectableText { id, .. }
            | Self::Switch { id, .. }
            | Self::Checkbox { id, .. }
            | Self::Radio { id, .. }
            | Self::SelectTrigger { id, .. } => HitTargetId::Widget(*id),
            Self::SelectOption {
                id, option_index, ..
            } => HitTargetId::SelectOption {
                widget_id: *id,
                option_index: *option_index,
            },
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
    SelectOption {
        widget_id: WidgetId,
        option_index: usize,
    },
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
    pub overlay_hit_regions: Vec<HitRegion<VM>>,
    pub scroll_regions: Vec<ScrollRegion>,
    pub ime_cursor_area: Option<Rect>,
}

#[derive(Clone, Default)]
pub(crate) struct WidgetStateMap {
    states: HashMap<WidgetId, WidgetState>,
    select_option_states: HashMap<(WidgetId, usize), WidgetState>,
}

impl WidgetStateMap {
    pub(crate) fn set(&mut self, id: WidgetId, state: WidgetState) {
        self.states.insert(id, state);
    }

    pub(crate) fn get(&self, id: WidgetId) -> WidgetState {
        self.states.get(&id).copied().unwrap_or_default()
    }

    pub(crate) fn set_select_option(
        &mut self,
        widget_id: WidgetId,
        option_index: usize,
        state: WidgetState,
    ) {
        self.select_option_states
            .insert((widget_id, option_index), state);
    }

    pub(crate) fn get_select_option(
        &self,
        widget_id: WidgetId,
        option_index: usize,
    ) -> WidgetState {
        self.select_option_states
            .get(&(widget_id, option_index))
            .copied()
            .unwrap_or_default()
    }
}

impl<VM> Default for ComputedScene<VM> {
    fn default() -> Self {
        Self {
            scene: ScenePrimitives::default(),
            hit_regions: Vec::new(),
            overlay_hit_regions: Vec::new(),
            scroll_regions: Vec::new(),
            ime_cursor_area: None,
        }
    }
}

impl<VM> ComputedScene<VM> {
    #[cfg(test)]
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
        self.cursor = clamp_to_char_boundary(text, self.cursor);
        self.anchor = clamp_to_char_boundary(text, self.anchor);
        if let Some(composition) = &mut self.composition {
            composition.replace_range.0 = clamp_to_char_boundary(text, composition.replace_range.0);
            composition.replace_range.1 = clamp_to_char_boundary(text, composition.replace_range.1);
            if composition.replace_range.0 > composition.replace_range.1 {
                composition.replace_range =
                    (composition.replace_range.1, composition.replace_range.1);
            }
            if let Some((start, end)) = composition.cursor {
                let start = clamp_to_char_boundary(&composition.text, start);
                let end = clamp_to_char_boundary(&composition.text, end);
                composition.cursor = Some(if start <= end {
                    (start, end)
                } else {
                    (end, end)
                });
            }
        }
        self.scroll_x = self.scroll_x.max(Dp::ZERO);
        self
    }
}

fn clamp_to_char_boundary(text: &str, index: usize) -> usize {
    let mut index = index.min(text.len());
    while index > 0 && !text.is_char_boundary(index) {
        index -= 1;
    }
    index
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CompositionState {
    pub replace_range: (usize, usize),
    pub text: String,
    pub cursor: Option<(usize, usize)>,
}

#[cfg(test)]
mod tests {
    use super::{CompositionState, InputEditState};
    use crate::ui::unit::Dp;

    #[test]
    fn input_edit_state_clamps_to_utf8_char_boundaries() {
        let text = "输入框示例输入框示例输入框示例";

        let state = InputEditState {
            cursor: 25,
            anchor: 29,
            composition: Some(CompositionState {
                replace_range: (25, 29),
                text: "提示".to_string(),
                cursor: Some((1, 4)),
            }),
            scroll_x: Dp::ZERO,
        }
        .clamped_to(text);

        assert_eq!(state.cursor, 24);
        assert_eq!(state.anchor, 27);
        assert_eq!(
            state
                .composition
                .as_ref()
                .map(|composition| composition.replace_range),
            Some((24, 27))
        );
        assert_eq!(
            state
                .composition
                .as_ref()
                .and_then(|composition| composition.cursor),
            Some((0, 3))
        );
    }
}

#[derive(Clone)]
pub(crate) struct InputSnapshot<VM> {
    pub id: WidgetId,
    pub on_change: Option<ValueCommand<VM, String>>,
    pub text: String,
}

#[cfg(test)]
#[derive(Clone, Default)]
pub(crate) struct RenderedWidgetScene {
    pub primitives: ScenePrimitives,
    pub scroll_regions: Vec<ScrollRegion>,
    #[allow(dead_code)]
    pub ime_cursor_area: Option<Rect>,
}
