use super::*;

pub(crate) fn bounds_rect(bounds: RectBounds) -> Rect {
    Rect::new(bounds.min_x, bounds.min_y, bounds.width(), bounds.height())
}

pub(crate) fn rect_from_bounds(bounds: RectBounds) -> Rect {
    bounds_rect(bounds)
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

    pub(crate) fn from_rect(rect: Rect) -> Self {
        Self::from_min_max(
            rect.x.get(),
            rect.y.get(),
            rect.right().get(),
            rect.bottom().get(),
        )
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

pub(crate) fn transform_bounds(bounds: RectBounds, transform: CanvasTransform2D) -> RectBounds {
    if transform == CanvasTransform2D::IDENTITY {
        return bounds;
    }

    let corners = [
        Point::new(bounds.min_x, bounds.min_y),
        Point::new(bounds.max_x, bounds.min_y),
        Point::new(bounds.max_x, bounds.max_y),
        Point::new(bounds.min_x, bounds.max_y),
    ];
    let mut min_x = f32::INFINITY;
    let mut min_y = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut max_y = f32::NEG_INFINITY;
    for corner in corners {
        let transformed = transform.apply(corner);
        min_x = min_x.min(transformed.x.get());
        min_y = min_y.min(transformed.y.get());
        max_x = max_x.max(transformed.x.get());
        max_y = max_y.max(transformed.y.get());
    }
    RectBounds::from_min_max(min_x, min_y, max_x, max_y)
}

pub(crate) fn group_shape_bounds(shape: &CanvasGroupShape) -> Option<RectBounds> {
    match shape {
        CanvasGroupShape::Path { path, .. } => path.control_bounds().map(|bounds| {
            RectBounds::from_min_max(bounds.min.x, bounds.min.y, bounds.max.x, bounds.max.y)
        }),
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct CanvasClipContext {
    pub(crate) clip_rect: Option<Rect>,
    pub(crate) clip_mask: Option<ClipMask>,
}

pub(crate) fn group_shape_clip_rect(shape: &CanvasGroupShape, origin: Point) -> Option<Rect> {
    match shape {
        CanvasGroupShape::Path { path, .. } => path.control_bounds().map(|bounds| {
            Rect::new(
                origin.x + bounds.min.x,
                origin.y + bounds.min.y,
                bounds.max.x - bounds.min.x,
                bounds.max.y - bounds.min.y,
            )
        }),
    }
}

pub(crate) fn compose_clip_rect(lhs: Option<Rect>, rhs: Option<Rect>) -> Option<Rect> {
    match (lhs, rhs) {
        (Some(lhs), Some(rhs)) => lhs.intersect(rhs),
        (Some(lhs), None) => Some(lhs),
        (None, Some(rhs)) => Some(rhs),
        (None, None) => None,
    }
}

pub(crate) fn offset_rect(rect: Rect, origin: Point) -> Rect {
    Rect::new(
        origin.x + rect.x,
        origin.y + rect.y,
        rect.width,
        rect.height,
    )
}

pub(crate) fn transform_path_builder(
    path: &PathBuilder,
    transform: CanvasTransform2D,
) -> PathBuilder {
    if transform == CanvasTransform2D::IDENTITY {
        return path.clone();
    }

    let mut builder = PathBuilder::new().fill_rule(path.fill_rule);
    for command in path.commands_internal() {
        builder = match *command {
            PathCommand::MoveTo(point_value) => {
                let point_value = transform.apply(point_value);
                builder.move_to(point_value.x, point_value.y)
            }
            PathCommand::LineTo(point_value) => {
                let point_value = transform.apply(point_value);
                builder.line_to(point_value.x, point_value.y)
            }
            PathCommand::QuadTo { ctrl, to } => {
                let ctrl = transform.apply(ctrl);
                let to = transform.apply(to);
                builder.quad_to(ctrl.x, ctrl.y, to.x, to.y)
            }
            PathCommand::CubicTo { ctrl1, ctrl2, to } => {
                let ctrl1 = transform.apply(ctrl1);
                let ctrl2 = transform.apply(ctrl2);
                let to = transform.apply(to);
                builder.cubic_to(ctrl1.x, ctrl1.y, ctrl2.x, ctrl2.y, to.x, to.y)
            }
            PathCommand::Close => builder.close(),
        };
    }
    builder
}

pub(crate) fn transform_path_builder_owned(
    path: PathBuilder,
    transform: CanvasTransform2D,
) -> PathBuilder {
    if transform == CanvasTransform2D::IDENTITY {
        return path;
    }

    let PathBuilder {
        commands,
        fill_rule,
        shape_hint: _,
    } = path;
    let mut transformed = Vec::with_capacity(commands.len());
    for command in commands {
        transformed.push(match command {
            PathCommand::MoveTo(point_value) => PathCommand::MoveTo(transform.apply(point_value)),
            PathCommand::LineTo(point_value) => PathCommand::LineTo(transform.apply(point_value)),
            PathCommand::QuadTo { ctrl, to } => PathCommand::QuadTo {
                ctrl: transform.apply(ctrl),
                to: transform.apply(to),
            },
            PathCommand::CubicTo { ctrl1, ctrl2, to } => PathCommand::CubicTo {
                ctrl1: transform.apply(ctrl1),
                ctrl2: transform.apply(ctrl2),
                to: transform.apply(to),
            },
            PathCommand::Close => PathCommand::Close,
        });
    }

    PathBuilder {
        commands: transformed,
        fill_rule,
        shape_hint: None,
    }
}

pub(crate) fn transform_rect_quad(
    rect: Rect,
    transform: CanvasTransform2D,
    origin: Point,
) -> [Point; 4] {
    let corners = [
        Point::new(rect.x - origin.x, rect.y - origin.y),
        Point::new(rect.right() - origin.x, rect.y - origin.y),
        Point::new(rect.right() - origin.x, rect.bottom() - origin.y),
        Point::new(rect.x - origin.x, rect.bottom() - origin.y),
    ];
    corners.map(|corner| {
        let transformed = transform.apply(corner);
        Point::new(origin.x + transformed.x, origin.y + transformed.y)
    })
}

pub(crate) fn quad_bounds_rect(quad: [Point; 4]) -> Option<Rect> {
    let mut min_x = f32::INFINITY;
    let mut min_y = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut max_y = f32::NEG_INFINITY;
    for point_value in quad {
        min_x = min_x.min(point_value.x.get());
        min_y = min_y.min(point_value.y.get());
        max_x = max_x.max(point_value.x.get());
        max_y = max_y.max(point_value.y.get());
    }
    if !min_x.is_finite() || !min_y.is_finite() || !max_x.is_finite() || !max_y.is_finite() {
        return None;
    }
    Some(Rect::new(
        min_x,
        min_y,
        (max_x - min_x).max(0.0),
        (max_y - min_y).max(0.0),
    ))
}
