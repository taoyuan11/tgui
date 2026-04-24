use std::collections::HashSet;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use geo::{BooleanOps, Coord, LineString, MultiPolygon, Polygon};
use image::{DynamicImage, RgbaImage};
use lyon::algorithms::aabb::bounding_box;
use lyon::algorithms::measure::{PathMeasurements, SampleType};
use lyon::math::point;
use lyon::path::iterator::PathIterator;
use lyon::path::{Path, PathEvent};
use lyon::tessellation::{
    BuffersBuilder, FillOptions, FillTessellator, FillVertex, FillVertexConstructor, StrokeOptions,
    StrokeTessellator, StrokeVertex, StrokeVertexConstructor, VertexBuffers,
};
use resvg::tiny_skia;

use crate::foundation::binding::Binding;
use crate::foundation::color::Color;
use crate::foundation::error::TguiError;
use crate::foundation::view_model::ValueCommand;
use crate::media::{MediaManager, TextureFrame};
use crate::ui::layout::{Insets, LayoutStyle, Value};
use crate::ui::unit::{Dp, UnitContext};

use super::common::{
    CanvasItemInteractionHandlers, CursorStyle, InteractionHandlers, MediaEventHandlers,
    MeshPrimitive, MeshVertex, Point, TexturePrimitive, VisualStyle, WidgetId, WidgetKind,
};
use super::core::Element;

const MAX_CANVAS_GRADIENT_STOPS: usize = 8;
const CANVAS_FLATTEN_TOLERANCE: f32 = 0.1;
const SHADOW_BLUR_PADDING_MULTIPLIER: f32 = 3.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CanvasItemId(u64);

impl CanvasItemId {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

impl From<u64> for CanvasItemId {
    fn from(value: u64) -> Self {
        Self(value)
    }
}

impl From<u32> for CanvasItemId {
    fn from(value: u32) -> Self {
        Self(value as u64)
    }
}

impl From<usize> for CanvasItemId {
    fn from(value: usize) -> Self {
        Self(value as u64)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CanvasPointerEvent {
    pub item_id: CanvasItemId,
    pub canvas_position: Point,
    pub local_position: Point,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CanvasGradientStop {
    pub offset: f32,
    pub color: Color,
}

impl CanvasGradientStop {
    pub fn new(offset: f32, color: Color) -> Self {
        Self {
            offset: offset.clamp(0.0, 1.0),
            color,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct CanvasLinearGradient {
    pub start: Point,
    pub end: Point,
    pub stops: Vec<CanvasGradientStop>,
}

impl CanvasLinearGradient {
    pub fn new(
        start: impl Into<Point>,
        end: impl Into<Point>,
        stops: impl Into<Vec<CanvasGradientStop>>,
    ) -> Self {
        Self {
            start: start.into(),
            end: end.into(),
            stops: stops.into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct CanvasRadialGradient {
    pub center: Point,
    pub radius: Dp,
    pub stops: Vec<CanvasGradientStop>,
}

impl CanvasRadialGradient {
    pub fn new(
        center: impl Into<Point>,
        radius: impl Into<Dp>,
        stops: impl Into<Vec<CanvasGradientStop>>,
    ) -> Self {
        Self {
            center: center.into(),
            radius: radius.into(),
            stops: stops.into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum CanvasBrush {
    Solid(Color),
    LinearGradient(CanvasLinearGradient),
    RadialGradient(CanvasRadialGradient),
}

impl From<Color> for CanvasBrush {
    fn from(value: Color) -> Self {
        Self::Solid(value)
    }
}

impl From<CanvasLinearGradient> for CanvasBrush {
    fn from(value: CanvasLinearGradient) -> Self {
        Self::LinearGradient(value)
    }
}

impl From<CanvasRadialGradient> for CanvasBrush {
    fn from(value: CanvasRadialGradient) -> Self {
        Self::RadialGradient(value)
    }
}

impl From<Color> for Value<CanvasBrush> {
    fn from(value: Color) -> Self {
        Value::Static(CanvasBrush::Solid(value))
    }
}

impl From<CanvasLinearGradient> for Value<CanvasBrush> {
    fn from(value: CanvasLinearGradient) -> Self {
        Value::Static(CanvasBrush::LinearGradient(value))
    }
}

impl From<CanvasRadialGradient> for Value<CanvasBrush> {
    fn from(value: CanvasRadialGradient) -> Self {
        Value::Static(CanvasBrush::RadialGradient(value))
    }
}

impl From<Binding<Color>> for Value<CanvasBrush> {
    fn from(value: Binding<Color>) -> Self {
        Value::Bound(value.map(CanvasBrush::Solid))
    }
}

impl From<Value<Color>> for Value<CanvasBrush> {
    fn from(value: Value<Color>) -> Self {
        match value {
            Value::Static(color) => Value::Static(CanvasBrush::Solid(color)),
            Value::Bound(binding) => Value::Bound(binding.map(CanvasBrush::Solid)),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CanvasShadow {
    pub color: Color,
    pub offset: Point,
    pub blur: Dp,
}

impl CanvasShadow {
    pub fn new(color: Color, offset: impl Into<Point>, blur: impl Into<Dp>) -> Self {
        Self {
            color,
            offset: offset.into(),
            blur: blur.into(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CanvasBooleanOp {
    Union,
    Intersection,
    Difference,
    Xor,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CanvasPathOpError {
    OpenSubpath,
    InvalidGeometry,
}

#[derive(Clone, Debug)]
pub struct CanvasStroke {
    pub width: Dp,
    pub brush: Value<CanvasBrush>,
    pub dash_pattern: Option<Vec<Dp>>,
    pub dash_offset: Dp,
}

impl CanvasStroke {
    pub fn new(width: impl Into<Dp>, color: Color) -> Self {
        Self::with_brush(width, CanvasBrush::Solid(color))
    }

    pub fn with_brush(width: impl Into<Dp>, brush: impl Into<Value<CanvasBrush>>) -> Self {
        Self {
            width: width.into(),
            brush: brush.into(),
            dash_pattern: None,
            dash_offset: Dp::ZERO,
        }
    }

    pub fn dash<I, T>(mut self, pattern: I) -> Self
    where
        I: IntoIterator<Item = T>,
        T: Into<Dp>,
    {
        self.dash_pattern = Some(pattern.into_iter().map(Into::into).collect());
        self
    }

    pub fn dash_offset(mut self, offset: impl Into<Dp>) -> Self {
        self.dash_offset = offset.into();
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum PathCommand {
    MoveTo(Point),
    LineTo(Point),
    QuadTo {
        ctrl: Point,
        to: Point,
    },
    CubicTo {
        ctrl1: Point,
        ctrl2: Point,
        to: Point,
    },
    Close,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct PathBuilder {
    commands: Vec<PathCommand>,
}

impl PathBuilder {
    pub fn new() -> Self {
        Self {
            commands: Vec::new(),
        }
    }

    pub fn move_to(mut self, x: impl Into<Dp>, y: impl Into<Dp>) -> Self {
        self.commands.push(PathCommand::MoveTo(Point::new(x, y)));
        self
    }

    pub fn line_to(mut self, x: impl Into<Dp>, y: impl Into<Dp>) -> Self {
        self.commands.push(PathCommand::LineTo(Point::new(x, y)));
        self
    }

    pub fn quad_to(
        mut self,
        ctrl_x: impl Into<Dp>,
        ctrl_y: impl Into<Dp>,
        x: impl Into<Dp>,
        y: impl Into<Dp>,
    ) -> Self {
        self.commands.push(PathCommand::QuadTo {
            ctrl: Point::new(ctrl_x, ctrl_y),
            to: Point::new(x, y),
        });
        self
    }

    pub fn cubic_to(
        mut self,
        ctrl1_x: impl Into<Dp>,
        ctrl1_y: impl Into<Dp>,
        ctrl2_x: impl Into<Dp>,
        ctrl2_y: impl Into<Dp>,
        x: impl Into<Dp>,
        y: impl Into<Dp>,
    ) -> Self {
        self.commands.push(PathCommand::CubicTo {
            ctrl1: Point::new(ctrl1_x, ctrl1_y),
            ctrl2: Point::new(ctrl2_x, ctrl2_y),
            to: Point::new(x, y),
        });
        self
    }

    pub fn close(mut self) -> Self {
        self.commands.push(PathCommand::Close);
        self
    }

    pub fn boolean(
        &self,
        op: CanvasBooleanOp,
        other: &PathBuilder,
    ) -> Result<PathBuilder, CanvasPathOpError> {
        let lhs = self.to_multi_polygon()?;
        let rhs = other.to_multi_polygon()?;
        let result = match op {
            CanvasBooleanOp::Union => lhs.union(&rhs),
            CanvasBooleanOp::Intersection => lhs.intersection(&rhs),
            CanvasBooleanOp::Difference => lhs.difference(&rhs),
            CanvasBooleanOp::Xor => lhs.xor(&rhs),
        };
        Ok(Self::from_multi_polygon(&result))
    }

    pub fn union(&self, other: &PathBuilder) -> Result<PathBuilder, CanvasPathOpError> {
        self.boolean(CanvasBooleanOp::Union, other)
    }

    pub fn intersection(&self, other: &PathBuilder) -> Result<PathBuilder, CanvasPathOpError> {
        self.boolean(CanvasBooleanOp::Intersection, other)
    }

    pub fn difference(&self, other: &PathBuilder) -> Result<PathBuilder, CanvasPathOpError> {
        self.boolean(CanvasBooleanOp::Difference, other)
    }

    pub fn xor(&self, other: &PathBuilder) -> Result<PathBuilder, CanvasPathOpError> {
        self.boolean(CanvasBooleanOp::Xor, other)
    }

    #[cfg(test)]
    fn commands(&self) -> &[PathCommand] {
        &self.commands
    }

    fn commands_internal(&self) -> &[PathCommand] {
        &self.commands
    }

    pub(crate) fn to_lyon_path(&self) -> Path {
        let mut builder = Path::builder();
        let mut subpath_open = false;
        for command in &self.commands {
            match *command {
                PathCommand::MoveTo(point_value) => {
                    if subpath_open {
                        builder.end(false);
                    }
                    builder.begin(point(point_value.x.get(), point_value.y.get()));
                    subpath_open = true;
                }
                PathCommand::LineTo(point_value) => {
                    builder.line_to(point(point_value.x.get(), point_value.y.get()));
                }
                PathCommand::QuadTo { ctrl, to } => {
                    builder.quadratic_bezier_to(
                        point(ctrl.x.get(), ctrl.y.get()),
                        point(to.x.get(), to.y.get()),
                    );
                }
                PathCommand::CubicTo { ctrl1, ctrl2, to } => {
                    builder.cubic_bezier_to(
                        point(ctrl1.x.get(), ctrl1.y.get()),
                        point(ctrl2.x.get(), ctrl2.y.get()),
                        point(to.x.get(), to.y.get()),
                    );
                }
                PathCommand::Close => {
                    builder.end(true);
                    subpath_open = false;
                }
            }
        }
        if subpath_open {
            builder.end(false);
        }
        builder.build()
    }

    pub(crate) fn control_bounds(&self) -> Option<lyon::geom::Box2D<f32>> {
        let path = self.to_lyon_path();
        (path.iter().next().is_some()).then(|| bounding_box(path.iter()))
    }

    fn to_multi_polygon(&self) -> Result<MultiPolygon<f64>, CanvasPathOpError> {
        let rings = self.flattened_closed_rings()?;
        Ok(MultiPolygon(
            rings
                .into_iter()
                .map(|ring| {
                    Polygon::new(
                        LineString::from(
                            ring.into_iter()
                                .map(|point_value| Coord {
                                    x: point_value.x as f64,
                                    y: point_value.y as f64,
                                })
                                .collect::<Vec<_>>(),
                        ),
                        Vec::new(),
                    )
                })
                .collect(),
        ))
    }

    fn flattened_closed_rings(&self) -> Result<Vec<Vec<lyon::math::Point>>, CanvasPathOpError> {
        let path = self.to_lyon_path();
        let mut rings = Vec::new();
        let mut current = Vec::new();
        let mut first = None;

        for event in path.iter().flattened(CANVAS_FLATTEN_TOLERANCE) {
            match event {
                PathEvent::Begin { at } => {
                    current.clear();
                    current.push(at);
                    first = Some(at);
                }
                PathEvent::Line { to, .. } => {
                    current.push(to);
                }
                PathEvent::End { close, .. } => {
                    if !close {
                        return Err(CanvasPathOpError::OpenSubpath);
                    }

                    if let Some(first_point) = first.take() {
                        if current
                            .last()
                            .map(|last| !points_approx_equal(*last, first_point))
                            .unwrap_or(false)
                        {
                            current.push(first_point);
                        }
                    }

                    dedupe_ring_points(&mut current);
                    if current.len() >= 4 {
                        rings.push(std::mem::take(&mut current));
                    } else {
                        current.clear();
                    }
                }
                PathEvent::Quadratic { .. } | PathEvent::Cubic { .. } => {}
            }
        }

        Ok(rings)
    }

    fn from_multi_polygon(multi: &MultiPolygon<f64>) -> Self {
        let mut builder = PathBuilder::new();
        for polygon in &multi.0 {
            builder = append_ring(builder, polygon.exterior());
            for interior in polygon.interiors() {
                builder = append_ring(builder, interior);
            }
        }
        builder
    }
}

#[derive(Clone, Debug)]
pub struct CanvasPath {
    pub id: CanvasItemId,
    pub path: PathBuilder,
    pub fill: Option<Value<CanvasBrush>>,
    pub stroke: Option<CanvasStroke>,
    pub shadow: Option<Value<CanvasShadow>>,
}

impl CanvasPath {
    pub fn new(id: impl Into<CanvasItemId>, path: PathBuilder) -> Self {
        Self {
            id: id.into(),
            path,
            fill: None,
            stroke: None,
            shadow: None,
        }
    }

    pub fn fill(mut self, brush: impl Into<Value<CanvasBrush>>) -> Self {
        self.fill = Some(brush.into());
        self
    }

    pub fn stroke(mut self, stroke: CanvasStroke) -> Self {
        self.stroke = Some(stroke);
        self
    }

    pub fn shadow(mut self, shadow: impl Into<Value<CanvasShadow>>) -> Self {
        self.shadow = Some(shadow.into());
        self
    }
}

#[derive(Clone, Debug)]
pub enum CanvasItem {
    Path(CanvasPath),
}

impl CanvasItem {
    pub fn path(id: impl Into<CanvasItemId>, path: PathBuilder) -> Self {
        Self::Path(CanvasPath::new(id, path))
    }

    pub(crate) fn id(&self) -> CanvasItemId {
        match self {
            Self::Path(path) => path.id,
        }
    }

    pub(crate) fn layout_bounds(&self) -> Option<RectBounds> {
        match self {
            Self::Path(path) => {
                let mut rect = path_base_bounds(path)?;
                if let Some(shadow) = path.shadow.as_ref().map(Value::resolve) {
                    rect = rect.expand_for_shadow(shadow);
                }
                Some(rect)
            }
        }
    }

    pub(crate) fn hit_bounds(&self) -> Option<RectBounds> {
        match self {
            Self::Path(path) => path_base_bounds(path),
        }
    }

    pub(crate) fn tessellate(
        &self,
        origin: Point,
        opacity: f32,
        clip_rect: Option<super::common::Rect>,
        media: &MediaManager,
        units: UnitContext,
    ) -> CanvasRenderOutput {
        match self {
            Self::Path(path) => tessellate_path(path, origin, opacity, clip_rect, media, units),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct RectBounds {
    pub min_x: f32,
    pub min_y: f32,
    pub max_x: f32,
    pub max_y: f32,
}

impl RectBounds {
    pub(crate) fn from_min_max(min_x: f32, min_y: f32, max_x: f32, max_y: f32) -> Self {
        Self {
            min_x,
            min_y,
            max_x,
            max_y,
        }
    }

    pub(crate) fn width(self) -> f32 {
        (self.max_x - self.min_x).max(0.0)
    }

    pub(crate) fn height(self) -> f32 {
        (self.max_y - self.min_y).max(0.0)
    }

    pub(crate) fn expand(self, amount: f32) -> Self {
        Self {
            min_x: self.min_x - amount,
            min_y: self.min_y - amount,
            max_x: self.max_x + amount,
            max_y: self.max_y + amount,
        }
    }

    pub(crate) fn expand_for_shadow(self, shadow: CanvasShadow) -> Self {
        let padding = shadow_padding(shadow.blur);
        Self {
            min_x: self.min_x + shadow.offset.x.get().min(0.0) - padding,
            min_y: self.min_y + shadow.offset.y.get().min(0.0) - padding,
            max_x: self.max_x + shadow.offset.x.get().max(0.0) + padding,
            max_y: self.max_y + shadow.offset.y.get().max(0.0) + padding,
        }
    }

    pub(crate) fn union(self, other: Self) -> Self {
        Self {
            min_x: self.min_x.min(other.min_x),
            min_y: self.min_y.min(other.min_y),
            max_x: self.max_x.max(other.max_x),
            max_y: self.max_y.max(other.max_y),
        }
    }
}

pub(crate) fn canvas_bounds(items: &[CanvasItem]) -> Option<RectBounds> {
    let mut bounds: Option<RectBounds> = None;
    for item in items {
        if let Some(item_bounds) = item.layout_bounds() {
            bounds = Some(match bounds {
                Some(existing) => existing.union(item_bounds),
                None => item_bounds,
            });
        }
    }
    bounds
}

pub struct Canvas<VM> {
    element: Element<VM>,
}

impl<VM> Canvas<VM> {
    pub fn new(items: impl Into<Value<Vec<CanvasItem>>>) -> Self {
        Self {
            element: Element {
                id: WidgetId::next(),
                layout: LayoutStyle::default(),
                visual: VisualStyle::default(),
                interactions: InteractionHandlers::default(),
                media_events: MediaEventHandlers::default(),
                background: None,
                kind: WidgetKind::Canvas {
                    items: items.into(),
                    item_interactions: CanvasItemInteractionHandlers::default(),
                },
            },
        }
    }

    pub fn size(mut self, width: impl Into<Value<Dp>>, height: impl Into<Value<Dp>>) -> Self {
        self.element.layout.width = Some(width.into());
        self.element.layout.height = Some(height.into());
        self.element.layout.fill_width = false;
        self.element.layout.fill_height = false;
        self
    }

    pub fn width(mut self, width: impl Into<Value<Dp>>) -> Self {
        self.element.layout.width = Some(width.into());
        self.element.layout.fill_width = false;
        self
    }

    pub fn height(mut self, height: impl Into<Value<Dp>>) -> Self {
        self.element.layout.height = Some(height.into());
        self.element.layout.fill_height = false;
        self
    }

    pub fn fill_width(mut self) -> Self {
        self.element.layout.fill_width = true;
        self.element.layout.width = None;
        self
    }

    pub fn fill_height(mut self) -> Self {
        self.element.layout.fill_height = true;
        self.element.layout.height = None;
        self
    }

    pub fn fill_size(mut self) -> Self {
        self.element.layout.fill_width = true;
        self.element.layout.fill_height = true;
        self.element.layout.width = None;
        self.element.layout.height = None;
        self
    }

    pub fn margin(mut self, insets: impl Into<Value<Insets>>) -> Self {
        self.element.layout.margin = insets.into();
        self
    }

    pub fn padding(mut self, insets: impl Into<Value<Insets>>) -> Self {
        self.element.layout.padding = insets.into();
        self
    }

    pub fn grow(mut self, grow: impl Into<Value<f32>>) -> Self {
        self.element.layout.grow = grow.into();
        self
    }

    pub fn background(mut self, color: impl Into<Value<Color>>) -> Self {
        self.element.background = Some(color.into());
        self
    }

    pub fn border(mut self, width: impl Into<Value<Dp>>, color: impl Into<Value<Color>>) -> Self {
        self.element.visual.border_width = width.into();
        self.element.visual.border_color = color.into();
        self
    }

    pub fn border_color(mut self, color: impl Into<Value<Color>>) -> Self {
        self.element.visual.border_color = color.into();
        self
    }

    pub fn border_radius(mut self, radius: impl Into<Value<Dp>>) -> Self {
        self.element.visual.border_radius = radius.into();
        self
    }

    pub fn border_width(mut self, width: impl Into<Value<Dp>>) -> Self {
        self.element.visual.border_width = width.into();
        self
    }

    pub fn opacity(mut self, opacity: impl Into<Value<f32>>) -> Self {
        self.element.visual.opacity = opacity.into();
        self
    }

    pub fn offset(mut self, offset: impl Into<Value<Point>>) -> Self {
        self.element.visual.offset = offset.into();
        self
    }

    pub fn on_click(mut self, command: crate::foundation::view_model::Command<VM>) -> Self {
        self.element.interactions.on_click = Some(command);
        self
    }

    pub fn on_double_click(mut self, command: crate::foundation::view_model::Command<VM>) -> Self {
        self.element.interactions.on_double_click = Some(command);
        self
    }

    pub fn on_mouse_enter(mut self, command: crate::foundation::view_model::Command<VM>) -> Self {
        self.element.interactions.on_mouse_enter = Some(command);
        self
    }

    pub fn on_mouse_leave(mut self, command: crate::foundation::view_model::Command<VM>) -> Self {
        self.element.interactions.on_mouse_leave = Some(command);
        self
    }

    pub fn on_mouse_move(mut self, command: ValueCommand<VM, Point>) -> Self {
        self.element.interactions.on_mouse_move = Some(command);
        self
    }

    pub fn on_item_click(mut self, command: ValueCommand<VM, CanvasPointerEvent>) -> Self {
        if let WidgetKind::Canvas {
            item_interactions, ..
        } = &mut self.element.kind
        {
            item_interactions.on_click = Some(command);
        }
        self
    }

    pub fn on_item_mouse_enter(mut self, command: ValueCommand<VM, CanvasPointerEvent>) -> Self {
        if let WidgetKind::Canvas {
            item_interactions, ..
        } = &mut self.element.kind
        {
            item_interactions.on_mouse_enter = Some(command);
        }
        self
    }

    pub fn on_item_mouse_leave(mut self, command: ValueCommand<VM, CanvasPointerEvent>) -> Self {
        if let WidgetKind::Canvas {
            item_interactions, ..
        } = &mut self.element.kind
        {
            item_interactions.on_mouse_leave = Some(command);
        }
        self
    }

    pub fn on_item_mouse_move(mut self, command: ValueCommand<VM, CanvasPointerEvent>) -> Self {
        if let WidgetKind::Canvas {
            item_interactions, ..
        } = &mut self.element.kind
        {
            item_interactions.on_mouse_move = Some(command);
        }
        self
    }

    pub fn cursor(mut self, cursor: impl Into<Value<CursorStyle>>) -> Self {
        self.element.interactions.cursor_style = Some(cursor.into());
        self
    }
}

impl<VM> From<Canvas<VM>> for Element<VM> {
    fn from(value: Canvas<VM>) -> Self {
        value.element
    }
}

#[derive(Default)]
pub(crate) struct CanvasRenderOutput {
    pub textures: Vec<TexturePrimitive>,
    pub meshes: Vec<MeshPrimitive>,
}

fn path_base_bounds(path: &CanvasPath) -> Option<RectBounds> {
    let bounds = path.path.control_bounds()?;
    let mut rect = RectBounds::from_min_max(bounds.min.x, bounds.min.y, bounds.max.x, bounds.max.y);
    if let Some(stroke) = path.stroke.as_ref() {
        rect = rect.expand(stroke.width.get() * 0.5);
    }
    Some(rect)
}

fn tessellate_path(
    path: &CanvasPath,
    origin: Point,
    opacity: f32,
    clip_rect: Option<super::common::Rect>,
    media: &MediaManager,
    units: UnitContext,
) -> CanvasRenderOutput {
    let lyon_path = path.path.to_lyon_path();
    let fill = path.fill.as_ref().map(Value::resolve);
    let stroke = path.stroke.clone();
    let mut output = CanvasRenderOutput::default();

    if let Some(shadow) = path.shadow.as_ref().map(Value::resolve) {
        if let Some(texture) = shadow_texture_for_path(
            path,
            &lyon_path,
            fill.as_ref(),
            stroke.as_ref(),
            shadow,
            opacity,
            origin,
            clip_rect,
            media,
            units,
        ) {
            output.textures.push(texture);
        }
    }

    if let Some(fill_brush) = fill.as_ref() {
        if let Some(mesh) = tessellate_fill(&lyon_path, fill_brush, opacity, origin, clip_rect) {
            output.meshes.push(mesh);
        }
    }

    if let Some(stroke) = stroke.as_ref() {
        if let Some(mesh) = tessellate_stroke(&lyon_path, stroke, opacity, origin, clip_rect) {
            output.meshes.push(mesh);
        }
    }

    output
}

fn tessellate_fill(
    path: &Path,
    brush: &CanvasBrush,
    opacity: f32,
    origin: Point,
    clip_rect: Option<super::common::Rect>,
) -> Option<MeshPrimitive> {
    let brush_data = CanvasBrushData::from_brush(brush, opacity)?;
    let mut geometry = VertexBuffers::<[f32; 2], u32>::new();
    let mut tessellator = FillTessellator::new();
    tessellator
        .tessellate_path(
            path,
            &FillOptions::default(),
            &mut BuffersBuilder::new(&mut geometry, FillVertexCtor),
        )
        .ok()?;

    build_mesh_primitive(geometry, brush_data, origin, clip_rect)
}

fn tessellate_stroke(
    path: &Path,
    stroke: &CanvasStroke,
    opacity: f32,
    origin: Point,
    clip_rect: Option<super::common::Rect>,
) -> Option<MeshPrimitive> {
    let brush = stroke.brush.resolve();
    let brush_data = CanvasBrushData::from_brush(&brush, opacity)?;
    let dashed = dashed_path(path, stroke);
    let source_path = dashed.as_ref().unwrap_or(path);

    let mut geometry = VertexBuffers::<[f32; 2], u32>::new();
    let mut tessellator = StrokeTessellator::new();
    let mut options = StrokeOptions::default();
    options.line_width = stroke.width.get().max(0.0);
    tessellator
        .tessellate_path(
            source_path,
            &options,
            &mut BuffersBuilder::new(&mut geometry, StrokeVertexCtor),
        )
        .ok()?;

    build_mesh_primitive(geometry, brush_data, origin, clip_rect)
}

fn build_mesh_primitive(
    geometry: VertexBuffers<[f32; 2], u32>,
    brush: CanvasBrushData,
    origin: Point,
    clip_rect: Option<super::common::Rect>,
) -> Option<MeshPrimitive> {
    if geometry.indices.is_empty() {
        return None;
    }

    let mut vertices = Vec::with_capacity(geometry.indices.len());
    let mut triangles = Vec::with_capacity(geometry.indices.len() / 3);
    for indices in geometry.indices.chunks_exact(3) {
        let mut triangle_points = [Point::ZERO; 3];
        for (slot, index) in indices.iter().enumerate() {
            let source = geometry.vertices[*index as usize];
            let point_value = Point::new(origin.x + source[0], origin.y + source[1]);
            triangle_points[slot] = point_value;
            vertices.push(MeshVertex {
                position: [point_value.x.get(), point_value.y.get()],
                local_position: [source[0], source[1]],
                brush_meta: brush.brush_meta,
                gradient_data0: brush.gradient_data0,
                gradient_data1: brush.gradient_data1,
                stop_offsets0: brush.stop_offsets0,
                stop_offsets1: brush.stop_offsets1,
                stop_colors: brush.stop_colors,
            });
        }
        triangles.push(triangle_points);
    }

    Some(MeshPrimitive {
        vertices: Arc::from(vertices),
        triangles: Arc::from(triangles),
        clip_rect,
    })
}

#[derive(Clone, Copy)]
struct CanvasBrushData {
    brush_meta: [f32; 4],
    gradient_data0: [f32; 4],
    gradient_data1: [f32; 4],
    stop_offsets0: [f32; 4],
    stop_offsets1: [f32; 4],
    stop_colors: [[f32; 4]; 8],
}

impl CanvasBrushData {
    fn from_brush(brush: &CanvasBrush, opacity: f32) -> Option<Self> {
        match brush {
            CanvasBrush::Solid(color) => {
                let mut stop_colors = [[0.0; 4]; 8];
                stop_colors[0] = color_to_f32(color.with_alpha_factor(opacity));
                Some(Self {
                    brush_meta: [0.0, 1.0, 0.0, 0.0],
                    gradient_data0: [0.0; 4],
                    gradient_data1: [0.0; 4],
                    stop_offsets0: [0.0; 4],
                    stop_offsets1: [0.0; 4],
                    stop_colors,
                })
            }
            CanvasBrush::LinearGradient(gradient) => {
                let stops = normalized_gradient_stops(&gradient.stops, opacity)?;
                Some(Self::gradient_data(
                    1.0,
                    &stops,
                    [
                        gradient.start.x.get(),
                        gradient.start.y.get(),
                        gradient.end.x.get(),
                        gradient.end.y.get(),
                    ],
                    [0.0; 4],
                ))
            }
            CanvasBrush::RadialGradient(gradient) => {
                let stops = normalized_gradient_stops(&gradient.stops, opacity)?;
                Some(Self::gradient_data(
                    2.0,
                    &stops,
                    [0.0; 4],
                    [
                        gradient.center.x.get(),
                        gradient.center.y.get(),
                        gradient.radius.get().max(0.0001),
                        0.0,
                    ],
                ))
            }
        }
    }

    fn gradient_data(
        kind: f32,
        stops: &[(f32, [f32; 4])],
        gradient_data0: [f32; 4],
        gradient_data1: [f32; 4],
    ) -> Self {
        let mut stop_offsets0 = [0.0; 4];
        let mut stop_offsets1 = [0.0; 4];
        let mut stop_colors = [[0.0; 4]; 8];

        for (index, (offset, color)) in stops.iter().enumerate() {
            if index < 4 {
                stop_offsets0[index] = *offset;
            } else {
                stop_offsets1[index - 4] = *offset;
            }
            stop_colors[index] = *color;
        }

        Self {
            brush_meta: [kind, stops.len() as f32, 0.0, 0.0],
            gradient_data0,
            gradient_data1,
            stop_offsets0,
            stop_offsets1,
            stop_colors,
        }
    }
}

fn normalized_gradient_stops(
    stops: &[CanvasGradientStop],
    opacity: f32,
) -> Option<Vec<(f32, [f32; 4])>> {
    if stops.len() > MAX_CANVAS_GRADIENT_STOPS {
        return None;
    }

    let mut normalized = stops
        .iter()
        .map(|stop| {
            (
                stop.offset.clamp(0.0, 1.0),
                color_to_f32(stop.color.with_alpha_factor(opacity)),
            )
        })
        .collect::<Vec<_>>();
    normalized.sort_by(|a, b| a.0.total_cmp(&b.0));

    if normalized.is_empty() {
        normalized.push((0.0, color_to_f32(Color::TRANSPARENT)));
    }

    Some(normalized)
}

fn dashed_path(path: &Path, stroke: &CanvasStroke) -> Option<Path> {
    let pattern = stroke.dash_pattern.as_ref()?;
    let normalized = normalize_dash_pattern(pattern)?;
    let measurements = PathMeasurements::from_path(path, CANVAS_FLATTEN_TOLERANCE);
    let total_length = measurements.length();
    if total_length <= 0.0 {
        return None;
    }

    let mut sampler = measurements.create_sampler(path, SampleType::Distance);
    let mut builder = Path::builder();
    let cycle_length: f32 = normalized.iter().sum();
    if cycle_length <= 0.0 {
        return None;
    }

    let mut cursor = (-stroke.dash_offset.get()).rem_euclid(cycle_length);
    let mut phase = 0usize;
    while cursor > normalized[phase] && cycle_length > 0.0 {
        cursor -= normalized[phase];
        phase = (phase + 1) % normalized.len();
    }

    let mut distance = 0.0_f32;
    let mut local_offset = cursor;
    while distance < total_length {
        let segment_length = (normalized[phase] - local_offset).max(0.0);
        let end = (distance + segment_length).min(total_length);
        if phase % 2 == 0 && end > distance {
            sampler.split_range(distance..end, &mut builder);
        }
        distance = end;
        phase = (phase + 1) % normalized.len();
        local_offset = 0.0;
        if segment_length <= 0.0 && normalized[phase] <= 0.0 {
            break;
        }
    }

    let dashed = builder.build();
    (dashed.iter().next().is_some()).then_some(dashed)
}

fn normalize_dash_pattern(pattern: &[Dp]) -> Option<Vec<f32>> {
    let mut values = pattern
        .iter()
        .map(|value| value.get().max(0.0))
        .collect::<Vec<_>>();
    if values.len() < 2 {
        return None;
    }
    if values.iter().all(|value| *value == 0.0) {
        return None;
    }
    if values.len() % 2 != 0 {
        values.extend(values.clone());
    }
    Some(values)
}

fn shadow_texture_for_path(
    path: &CanvasPath,
    lyon_path: &Path,
    fill: Option<&CanvasBrush>,
    stroke: Option<&CanvasStroke>,
    shadow: CanvasShadow,
    opacity: f32,
    origin: Point,
    clip_rect: Option<super::common::Rect>,
    media: &MediaManager,
    units: UnitContext,
) -> Option<TexturePrimitive> {
    let base_bounds = path_base_bounds(path)?;
    let padding = shadow_padding(shadow.blur);
    let min_x = base_bounds.min_x + shadow.offset.x.get().min(0.0) - padding;
    let min_y = base_bounds.min_y + shadow.offset.y.get().min(0.0) - padding;
    let max_x = base_bounds.max_x + shadow.offset.x.get().max(0.0) + padding;
    let max_y = base_bounds.max_y + shadow.offset.y.get().max(0.0) + padding;
    let frame = super::common::Rect::new(
        origin.x + min_x,
        origin.y + min_y,
        (max_x - min_x).max(1.0),
        (max_y - min_y).max(1.0),
    );
    let width = units.logical_to_physical(frame.width.get()).ceil().max(1.0) as u32;
    let height = units
        .logical_to_physical(frame.height.get())
        .ceil()
        .max(1.0) as u32;

    let cache_key = canvas_shadow_cache_key(path, shadow, opacity, units.scale_factor());
    let texture = media
        .canvas_shadow_texture(cache_key, width, height, || {
            rasterize_canvas_shadow(
                lyon_path,
                fill.is_some(),
                stroke,
                shadow,
                opacity,
                min_x,
                min_y,
                units.scale_factor(),
            )
        })
        .ok()??;

    Some(TexturePrimitive {
        texture,
        frame,
        clip_rect,
    })
}

fn rasterize_canvas_shadow(
    path: &Path,
    has_fill: bool,
    stroke: Option<&CanvasStroke>,
    shadow: CanvasShadow,
    opacity: f32,
    min_x: f32,
    min_y: f32,
    scale_factor: f32,
) -> Result<TextureFrame, TguiError> {
    let dashed = stroke.and_then(|stroke| dashed_path(path, stroke));
    let source_path = dashed.as_ref().unwrap_or(path);
    let tiny_path = to_tiny_skia_path(source_path, min_x, min_y, scale_factor)?;

    let width = transformed_path_size(source_path, min_x, min_y, scale_factor).0;
    let height = transformed_path_size(source_path, min_x, min_y, scale_factor).1;
    let mut pixmap = tiny_skia::Pixmap::new(width, height).ok_or_else(|| {
        TguiError::Media(format!(
            "failed to allocate canvas shadow surface {}x{}",
            width, height
        ))
    })?;
    let mut paint = tiny_skia::Paint::default();
    paint.set_color_rgba8(255, 255, 255, 255);

    if has_fill {
        pixmap.as_mut().fill_path(
            &tiny_path,
            &paint,
            tiny_skia::FillRule::EvenOdd,
            tiny_skia::Transform::identity(),
            None,
        );
    }

    if let Some(stroke) = stroke {
        let mut stroke_style = tiny_skia::Stroke::default();
        stroke_style.width = stroke.width.get().max(0.0) * scale_factor;
        if let Some(pattern) = stroke
            .dash_pattern
            .as_ref()
            .and_then(|pattern| normalize_dash_pattern(pattern))
        {
            stroke_style.dash = tiny_skia::StrokeDash::new(
                pattern
                    .into_iter()
                    .map(|value| value * scale_factor)
                    .collect(),
                stroke.dash_offset.get() * scale_factor,
            );
        }
        pixmap.as_mut().stroke_path(
            &tiny_path,
            &paint,
            &stroke_style,
            tiny_skia::Transform::identity(),
            None,
        );
    }

    let blurred = DynamicImage::ImageRgba8(
        RgbaImage::from_raw(width, height, pixmap.data().to_vec()).ok_or_else(|| {
            TguiError::Media("failed to create canvas shadow image buffer".to_string())
        })?,
    )
    .fast_blur((shadow.blur.get() * scale_factor).max(0.0));

    let mut pixels = blurred.to_rgba8().into_raw();
    let shadow_color = shadow.color.with_alpha_factor(opacity);
    for pixel in pixels.chunks_exact_mut(4) {
        let alpha = pixel[3] as f32 / 255.0;
        pixel[0] = ((shadow_color.r as f32) * alpha).round().clamp(0.0, 255.0) as u8;
        pixel[1] = ((shadow_color.g as f32) * alpha).round().clamp(0.0, 255.0) as u8;
        pixel[2] = ((shadow_color.b as f32) * alpha).round().clamp(0.0, 255.0) as u8;
        pixel[3] = ((shadow_color.a as f32) * alpha).round().clamp(0.0, 255.0) as u8;
    }

    Ok(TextureFrame::new(width, height, pixels))
}

fn transformed_path_size(path: &Path, min_x: f32, min_y: f32, scale_factor: f32) -> (u32, u32) {
    let bounds = bounding_box(path.iter());
    let width = ((bounds.max.x - min_x) * scale_factor).ceil().max(1.0) as u32;
    let height = ((bounds.max.y - min_y) * scale_factor).ceil().max(1.0) as u32;
    (width, height)
}

fn to_tiny_skia_path(
    path: &Path,
    min_x: f32,
    min_y: f32,
    scale_factor: f32,
) -> Result<tiny_skia::Path, TguiError> {
    let mut builder = tiny_skia::PathBuilder::new();
    for event in path.iter() {
        match event {
            PathEvent::Begin { at } => {
                builder.move_to((at.x - min_x) * scale_factor, (at.y - min_y) * scale_factor)
            }
            PathEvent::Line { to, .. } => {
                builder.line_to((to.x - min_x) * scale_factor, (to.y - min_y) * scale_factor)
            }
            PathEvent::Quadratic { ctrl, to, .. } => builder.quad_to(
                (ctrl.x - min_x) * scale_factor,
                (ctrl.y - min_y) * scale_factor,
                (to.x - min_x) * scale_factor,
                (to.y - min_y) * scale_factor,
            ),
            PathEvent::Cubic {
                ctrl1, ctrl2, to, ..
            } => builder.cubic_to(
                (ctrl1.x - min_x) * scale_factor,
                (ctrl1.y - min_y) * scale_factor,
                (ctrl2.x - min_x) * scale_factor,
                (ctrl2.y - min_y) * scale_factor,
                (to.x - min_x) * scale_factor,
                (to.y - min_y) * scale_factor,
            ),
            PathEvent::End { close, .. } => {
                if close {
                    builder.close();
                }
            }
        }
    }

    builder.finish().ok_or_else(|| {
        TguiError::Media("failed to finish canvas shadow path rasterization".to_string())
    })
}

fn canvas_shadow_cache_key(
    path: &CanvasPath,
    shadow: CanvasShadow,
    opacity: f32,
    scale_factor: f32,
) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    for command in path.path.commands_internal() {
        match *command {
            PathCommand::MoveTo(point_value) => {
                0u8.hash(&mut hasher);
                hash_point(point_value, &mut hasher);
            }
            PathCommand::LineTo(point_value) => {
                1u8.hash(&mut hasher);
                hash_point(point_value, &mut hasher);
            }
            PathCommand::QuadTo { ctrl, to } => {
                2u8.hash(&mut hasher);
                hash_point(ctrl, &mut hasher);
                hash_point(to, &mut hasher);
            }
            PathCommand::CubicTo { ctrl1, ctrl2, to } => {
                3u8.hash(&mut hasher);
                hash_point(ctrl1, &mut hasher);
                hash_point(ctrl2, &mut hasher);
                hash_point(to, &mut hasher);
            }
            PathCommand::Close => {
                4u8.hash(&mut hasher);
            }
        }
    }
    path.fill.is_some().hash(&mut hasher);
    if let Some(stroke) = path.stroke.as_ref() {
        hash_f32(stroke.width.get(), &mut hasher);
        if let Some(pattern) = stroke.dash_pattern.as_ref() {
            pattern.len().hash(&mut hasher);
            for value in pattern {
                hash_f32(value.get(), &mut hasher);
            }
        } else {
            0usize.hash(&mut hasher);
        }
        hash_f32(stroke.dash_offset.get(), &mut hasher);
    } else {
        0u8.hash(&mut hasher);
    }
    shadow.color.hash(&mut hasher);
    hash_point(shadow.offset, &mut hasher);
    hash_f32(shadow.blur.get(), &mut hasher);
    hash_f32(opacity, &mut hasher);
    hash_f32(scale_factor, &mut hasher);
    hasher.finish()
}

fn hash_point(point_value: Point, hasher: &mut impl Hasher) {
    hash_f32(point_value.x.get(), hasher);
    hash_f32(point_value.y.get(), hasher);
}

fn hash_f32(value: f32, hasher: &mut impl Hasher) {
    value.to_bits().hash(hasher);
}

fn shadow_padding(blur: Dp) -> f32 {
    blur.get().max(0.0) * SHADOW_BLUR_PADDING_MULTIPLIER
}

fn color_to_f32(color: Color) -> [f32; 4] {
    [
        color.r as f32 / 255.0,
        color.g as f32 / 255.0,
        color.b as f32 / 255.0,
        color.a as f32 / 255.0,
    ]
}

fn points_approx_equal(lhs: lyon::math::Point, rhs: lyon::math::Point) -> bool {
    (lhs.x - rhs.x).abs() <= 1e-3 && (lhs.y - rhs.y).abs() <= 1e-3
}

fn dedupe_ring_points(points: &mut Vec<lyon::math::Point>) {
    let mut deduped = Vec::with_capacity(points.len());
    for point_value in points.iter().copied() {
        if deduped
            .last()
            .map(|last| !points_approx_equal(*last, point_value))
            .unwrap_or(true)
        {
            deduped.push(point_value);
        }
    }
    if deduped.len() >= 2
        && deduped
            .first()
            .zip(deduped.last())
            .map(|(first, last)| !points_approx_equal(*first, *last))
            .unwrap_or(false)
    {
        deduped.push(deduped[0]);
    }
    *points = deduped;
}

fn append_ring(mut builder: PathBuilder, ring: &LineString<f64>) -> PathBuilder {
    let mut points = ring.points().collect::<Vec<_>>();
    if points.len() < 3 {
        return builder;
    }
    if let Some(first) = points.first().copied() {
        if points.last().map(|last| last != &first).unwrap_or(false) {
            points.push(first);
        }
    }

    let unique = points
        .iter()
        .map(|point_value| (point_value.x().to_bits(), point_value.y().to_bits()))
        .collect::<HashSet<_>>();
    if unique.len() < 3 {
        return builder;
    }

    let first = points[0];
    builder = builder.move_to(first.x() as f32, first.y() as f32);
    for point_value in points.iter().skip(1).take(points.len().saturating_sub(2)) {
        builder = builder.line_to(point_value.x() as f32, point_value.y() as f32);
    }
    builder.close()
}

struct FillVertexCtor;

impl FillVertexConstructor<[f32; 2]> for FillVertexCtor {
    fn new_vertex(&mut self, vertex: FillVertex<'_>) -> [f32; 2] {
        let position = vertex.position();
        [position.x, position.y]
    }
}

struct StrokeVertexCtor;

impl StrokeVertexConstructor<[f32; 2]> for StrokeVertexCtor {
    fn new_vertex(&mut self, vertex: StrokeVertex<'_, '_>) -> [f32; 2] {
        let position = vertex.position();
        [position.x, position.y]
    }
}

#[cfg(test)]
mod tests {
    use super::{
        canvas_bounds, CanvasBooleanOp, CanvasBrush, CanvasGradientStop, CanvasItem, CanvasPath,
        CanvasShadow, CanvasStroke, PathBuilder,
    };
    use crate::foundation::color::Color;
    use crate::ui::unit::dp;

    #[test]
    fn path_builder_records_commands() {
        let path = PathBuilder::new()
            .move_to(0.0, 0.0)
            .line_to(10.0, 0.0)
            .quad_to(15.0, 5.0, 10.0, 10.0)
            .cubic_to(8.0, 12.0, 4.0, 12.0, 0.0, 10.0)
            .close();

        assert_eq!(path.commands().len(), 5);
    }

    #[test]
    fn bounds_include_stroke_width() {
        let item = CanvasItem::Path(
            CanvasPath::new(
                7_u64,
                PathBuilder::new()
                    .move_to(10.0, 10.0)
                    .line_to(30.0, 10.0)
                    .line_to(30.0, 20.0)
                    .close(),
            )
            .stroke(CanvasStroke::new(dp(8.0), Color::WHITE)),
        );

        let bounds = item.hit_bounds().expect("bounds should exist");
        assert_eq!(bounds.min_x, 6.0);
        assert_eq!(bounds.max_x, 34.0);
    }

    #[test]
    fn canvas_bounds_union_all_items() {
        let items = vec![
            CanvasItem::path(
                1_u64,
                PathBuilder::new()
                    .move_to(0.0, 0.0)
                    .line_to(20.0, 0.0)
                    .line_to(20.0, 10.0)
                    .close(),
            ),
            CanvasItem::path(
                2_u64,
                PathBuilder::new()
                    .move_to(50.0, 25.0)
                    .line_to(80.0, 25.0)
                    .line_to(80.0, 40.0)
                    .close(),
            ),
        ];

        let bounds = canvas_bounds(&items).expect("bounds should exist");
        assert_eq!(bounds.width(), 80.0);
        assert_eq!(bounds.height(), 40.0);
    }

    #[test]
    fn canvas_bounds_include_shadow_expansion() {
        let item = CanvasItem::Path(
            CanvasPath::new(
                1_u64,
                PathBuilder::new()
                    .move_to(0.0, 0.0)
                    .line_to(20.0, 0.0)
                    .line_to(20.0, 20.0)
                    .close(),
            )
            .shadow(CanvasShadow::new(
                Color::BLACK,
                crate::ui::widget::Point::new(4.0, 6.0),
                dp(5.0),
            )),
        );

        let bounds = item.layout_bounds().expect("layout bounds should exist");
        assert!(bounds.max_x > 20.0);
        assert!(bounds.max_y > 20.0);
    }

    #[test]
    fn boolean_union_combines_rectangles() {
        let lhs = PathBuilder::new()
            .move_to(0.0, 0.0)
            .line_to(20.0, 0.0)
            .line_to(20.0, 20.0)
            .line_to(0.0, 20.0)
            .close();
        let rhs = PathBuilder::new()
            .move_to(10.0, 0.0)
            .line_to(30.0, 0.0)
            .line_to(30.0, 20.0)
            .line_to(10.0, 20.0)
            .close();

        let union = lhs
            .boolean(CanvasBooleanOp::Union, &rhs)
            .expect("boolean union should succeed");
        let union_bounds = union.control_bounds().expect("union bounds");
        assert_eq!(union_bounds.min.x, 0.0);
        assert_eq!(union_bounds.max.x, 30.0);
    }

    #[test]
    fn boolean_difference_rejects_open_subpaths() {
        let lhs = PathBuilder::new().move_to(0.0, 0.0).line_to(10.0, 0.0);
        let rhs = PathBuilder::new()
            .move_to(0.0, 0.0)
            .line_to(10.0, 0.0)
            .line_to(10.0, 10.0)
            .close();

        assert!(lhs.difference(&rhs).is_err());
    }

    #[test]
    fn solid_color_still_converts_to_brush() {
        let brush = CanvasBrush::from(Color::WHITE);
        assert!(matches!(brush, CanvasBrush::Solid(Color::WHITE)));
    }

    #[test]
    fn gradient_stop_count_limit_is_enforced() {
        let stops = (0..9)
            .map(|index| CanvasGradientStop::new(index as f32 / 8.0, Color::WHITE))
            .collect::<Vec<_>>();
        let gradient = CanvasBrush::LinearGradient(super::CanvasLinearGradient::new(
            crate::ui::widget::Point::new(0.0, 0.0),
            crate::ui::widget::Point::new(10.0, 0.0),
            stops,
        ));

        assert!(super::CanvasBrushData::from_brush(&gradient, 1.0).is_none());
    }

    #[test]
    fn fill_and_stroke_tessellate_separately() {
        let path = PathBuilder::new()
            .move_to(0.0, 0.0)
            .line_to(40.0, 0.0)
            .line_to(40.0, 30.0)
            .line_to(0.0, 30.0)
            .close()
            .to_lyon_path();

        assert!(super::tessellate_fill(
            &path,
            &CanvasBrush::Solid(Color::WHITE),
            1.0,
            crate::ui::widget::Point::ZERO,
            None
        )
        .is_some());
        assert!(super::tessellate_stroke(
            &path,
            &CanvasStroke::new(dp(4.0), Color::WHITE),
            1.0,
            crate::ui::widget::Point::ZERO,
            None
        )
        .is_some());
    }
}
