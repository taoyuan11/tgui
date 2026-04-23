use std::sync::Arc;

use lyon::algorithms::aabb::bounding_box;
use lyon::math::point;
use lyon::path::Path;
use lyon::tessellation::{
    BuffersBuilder, FillOptions, FillTessellator, FillVertex, FillVertexConstructor, StrokeOptions,
    StrokeTessellator, StrokeVertex, StrokeVertexConstructor, VertexBuffers,
};

use crate::foundation::color::Color;
use crate::foundation::view_model::ValueCommand;
use crate::ui::layout::{Insets, LayoutStyle, Value};
use crate::ui::unit::Dp;

use super::common::{
    CanvasItemInteractionHandlers, CursorStyle, InteractionHandlers, MediaEventHandlers,
    MeshPrimitive, MeshVertex, Point, VisualStyle, WidgetId, WidgetKind,
};
use super::core::Element;

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
pub struct CanvasStroke {
    pub width: Dp,
    pub color: Color,
}

impl CanvasStroke {
    pub fn new(width: impl Into<Dp>, color: Color) -> Self {
        Self {
            width: width.into(),
            color,
        }
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

    #[cfg(test)]
    fn commands(&self) -> &[PathCommand] {
        &self.commands
    }

    pub(crate) fn to_lyon_path(&self) -> Path {
        let mut builder = Path::builder();
        for command in &self.commands {
            match *command {
                PathCommand::MoveTo(point_value) => {
                    builder.begin(point(point_value.x.get(), point_value.y.get()));
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
                    builder.close();
                }
            }
        }
        builder.build()
    }

    pub(crate) fn control_bounds(&self) -> Option<lyon::geom::Box2D<f32>> {
        let path = self.to_lyon_path();
        (path.iter().next().is_some()).then(|| bounding_box(path.iter()))
    }
}

#[derive(Clone, Debug)]
pub struct CanvasPath {
    pub id: CanvasItemId,
    pub path: PathBuilder,
    pub fill: Option<Value<Color>>,
    pub stroke: Option<CanvasStroke>,
}

impl CanvasPath {
    pub fn new(id: impl Into<CanvasItemId>, path: PathBuilder) -> Self {
        Self {
            id: id.into(),
            path,
            fill: None,
            stroke: None,
        }
    }

    pub fn fill(mut self, color: impl Into<Value<Color>>) -> Self {
        self.fill = Some(color.into());
        self
    }

    pub fn stroke(mut self, stroke: CanvasStroke) -> Self {
        self.stroke = Some(stroke);
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

    pub(crate) fn bounds(&self) -> Option<RectBounds> {
        match self {
            Self::Path(path) => {
                let bounds = path.path.control_bounds()?;
                let mut rect = RectBounds::from_min_max(
                    bounds.min.x,
                    bounds.min.y,
                    bounds.max.x,
                    bounds.max.y,
                );
                if let Some(stroke) = path.stroke {
                    rect = rect.expand(stroke.width.get() * 0.5);
                }
                Some(rect)
            }
        }
    }

    pub(crate) fn tessellate(
        &self,
        origin: Point,
        opacity: f32,
        clip_rect: Option<super::common::Rect>,
    ) -> Vec<MeshPrimitive> {
        match self {
            Self::Path(path) => tessellate_path(path, origin, opacity, clip_rect),
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
        if let Some(item_bounds) = item.bounds() {
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

fn tessellate_path(
    path: &CanvasPath,
    origin: Point,
    opacity: f32,
    clip_rect: Option<super::common::Rect>,
) -> Vec<MeshPrimitive> {
    let lyon_path = path.path.to_lyon_path();
    let mut primitives = Vec::new();

    if let Some(fill) = path.fill.clone() {
        if let Some(mesh) = tessellate_fill(
            &lyon_path,
            fill.resolve().with_alpha_factor(opacity),
            origin,
            clip_rect,
        ) {
            primitives.push(mesh);
        }
    }

    if let Some(stroke) = path.stroke {
        if let Some(mesh) = tessellate_stroke(&lyon_path, stroke, opacity, origin, clip_rect) {
            primitives.push(mesh);
        }
    }

    primitives
}

fn tessellate_fill(
    path: &Path,
    color: Color,
    origin: Point,
    clip_rect: Option<super::common::Rect>,
) -> Option<MeshPrimitive> {
    let mut geometry = VertexBuffers::<[f32; 2], u32>::new();
    let mut tessellator = FillTessellator::new();
    tessellator
        .tessellate_path(
            path,
            &FillOptions::default(),
            &mut BuffersBuilder::new(&mut geometry, FillVertexCtor),
        )
        .ok()?;

    build_mesh_primitive(geometry, color, origin, clip_rect)
}

fn tessellate_stroke(
    path: &Path,
    stroke: CanvasStroke,
    opacity: f32,
    origin: Point,
    clip_rect: Option<super::common::Rect>,
) -> Option<MeshPrimitive> {
    let mut geometry = VertexBuffers::<[f32; 2], u32>::new();
    let mut tessellator = StrokeTessellator::new();
    let mut options = StrokeOptions::default();
    options.line_width = stroke.width.get().max(0.0);
    tessellator
        .tessellate_path(
            path,
            &options,
            &mut BuffersBuilder::new(&mut geometry, StrokeVertexCtor),
        )
        .ok()?;

    build_mesh_primitive(
        geometry,
        stroke.color.with_alpha_factor(opacity),
        origin,
        clip_rect,
    )
}

fn build_mesh_primitive(
    geometry: VertexBuffers<[f32; 2], u32>,
    color: Color,
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
                color,
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
    use super::{canvas_bounds, CanvasItem, CanvasPath, CanvasStroke, PathBuilder};
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

        let bounds = item.bounds().expect("bounds should exist");
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
}
