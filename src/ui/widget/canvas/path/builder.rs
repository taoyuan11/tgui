use super::super::path_utils::*;
use super::*;

mod operations;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum PathCommand {
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

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum PathShapeHint {
    RoundedRect { rect: Rect, radius: Dp },
}

#[derive(Clone, Debug, PartialEq)]
pub struct PathBuilder {
    pub(crate) commands: Vec<PathCommand>,
    pub(crate) fill_rule: CanvasFillRule,
    pub(crate) shape_hint: Option<PathShapeHint>,
}

impl PathBuilder {
    pub fn new() -> Self {
        Self {
            commands: Vec::new(),
            fill_rule: CanvasFillRule::NonZero,
            shape_hint: None,
        }
    }

    pub fn fill_rule(mut self, fill_rule: CanvasFillRule) -> Self {
        self.fill_rule = fill_rule;
        self
    }

    pub fn even_odd(self) -> Self {
        self.fill_rule(CanvasFillRule::EvenOdd)
    }

    pub fn non_zero(self) -> Self {
        self.fill_rule(CanvasFillRule::NonZero)
    }

    pub fn move_to(mut self, x: impl Into<Dp>, y: impl Into<Dp>) -> Self {
        self.shape_hint = None;
        self.commands.push(PathCommand::MoveTo(Point::new(x, y)));
        self
    }

    pub fn line_to(mut self, x: impl Into<Dp>, y: impl Into<Dp>) -> Self {
        self.shape_hint = None;
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
        self.shape_hint = None;
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
        self.shape_hint = None;
        self.commands.push(PathCommand::CubicTo {
            ctrl1: Point::new(ctrl1_x, ctrl1_y),
            ctrl2: Point::new(ctrl2_x, ctrl2_y),
            to: Point::new(x, y),
        });
        self
    }

    pub fn close(mut self) -> Self {
        self.shape_hint = None;
        self.commands.push(PathCommand::Close);
        self
    }

    pub fn rect(
        mut self,
        x: impl Into<Dp>,
        y: impl Into<Dp>,
        width: impl Into<Dp>,
        height: impl Into<Dp>,
    ) -> Self {
        let started_empty = self.commands.is_empty();
        let x = x.into();
        let y = y.into();
        let width = width.into().max(Dp::ZERO);
        let height = height.into().max(Dp::ZERO);
        self = self
            .move_to(x, y)
            .line_to(x + width, y)
            .line_to(x + width, y + height)
            .line_to(x, y + height)
            .close();
        if started_empty {
            self.shape_hint = Some(PathShapeHint::RoundedRect {
                rect: Rect::new(x, y, width, height),
                radius: Dp::ZERO,
            });
        }
        self
    }

    pub fn rounded_rect(
        self,
        x: impl Into<Dp>,
        y: impl Into<Dp>,
        width: impl Into<Dp>,
        height: impl Into<Dp>,
        radius: impl Into<Dp>,
    ) -> Self {
        let started_empty = self.commands.is_empty();
        let x = x.into();
        let y = y.into();
        let width = width.into().max(Dp::ZERO);
        let height = height.into().max(Dp::ZERO);
        let radius = radius
            .into()
            .max(Dp::ZERO)
            .min(width * 0.5)
            .min(height * 0.5);
        if radius <= Dp::ZERO {
            return self.rect(x, y, width, height);
        }

        let right = x + width;
        let bottom = y + height;
        let r = radius.get();
        let mut builder = self.move_to(x + radius, y);
        builder = append_arc_segments(
            builder,
            Point::new(right - radius, y + radius),
            r,
            -std::f32::consts::FRAC_PI_2,
            std::f32::consts::FRAC_PI_2,
            false,
        );
        builder = append_arc_segments(
            builder,
            Point::new(right - radius, bottom - radius),
            r,
            0.0,
            std::f32::consts::FRAC_PI_2,
            true,
        );
        builder = append_arc_segments(
            builder,
            Point::new(x + radius, bottom - radius),
            r,
            std::f32::consts::FRAC_PI_2,
            std::f32::consts::FRAC_PI_2,
            true,
        );
        builder = append_arc_segments(
            builder,
            Point::new(x + radius, y + radius),
            r,
            std::f32::consts::PI,
            std::f32::consts::FRAC_PI_2,
            true,
        );
        builder = builder.close();
        if started_empty {
            builder.shape_hint = Some(PathShapeHint::RoundedRect {
                rect: Rect::new(x, y, width, height),
                radius,
            });
        }
        builder
    }

    pub fn circle(
        self,
        center_x: impl Into<Dp>,
        center_y: impl Into<Dp>,
        radius: impl Into<Dp>,
    ) -> Self {
        let radius = radius.into();
        self.ellipse(center_x, center_y, radius, radius)
    }

    pub fn ellipse(
        self,
        center_x: impl Into<Dp>,
        center_y: impl Into<Dp>,
        radius_x: impl Into<Dp>,
        radius_y: impl Into<Dp>,
    ) -> Self {
        let center = Point::new(center_x, center_y);
        let radius_x = radius_x.into().max(Dp::ZERO).get();
        let radius_y = radius_y.into().max(Dp::ZERO).get();
        if radius_x <= 0.0 || radius_y <= 0.0 {
            return self;
        }

        let segments = 32usize;
        let mut builder = self.move_to(center.x.get() + radius_x, center.y.get());
        for index in 1..segments {
            let angle = (index as f32 / segments as f32) * std::f32::consts::TAU;
            builder = builder.line_to(
                center.x.get() + angle.cos() * radius_x,
                center.y.get() + angle.sin() * radius_y,
            );
        }
        builder.close()
    }

    pub fn arc(
        self,
        center_x: impl Into<Dp>,
        center_y: impl Into<Dp>,
        radius: impl Into<Dp>,
        start_angle: f32,
        sweep_angle: f32,
    ) -> Self {
        let center = Point::new(center_x, center_y);
        let connect_with_line = !self.commands.is_empty();
        let mut builder = self;
        builder.shape_hint = None;
        append_arc_segments(
            builder,
            center,
            radius.into().max(Dp::ZERO).get(),
            start_angle,
            sweep_angle,
            connect_with_line,
        )
    }

    pub fn arc_to(
        self,
        ctrl_x: impl Into<Dp>,
        ctrl_y: impl Into<Dp>,
        x: impl Into<Dp>,
        y: impl Into<Dp>,
        radius: impl Into<Dp>,
    ) -> Self {
        let mut this = self;
        this.shape_hint = None;
        let Some(PathCommand::MoveTo(from) | PathCommand::LineTo(from)) =
            this.commands.last().copied()
        else {
            return this.move_to(ctrl_x, ctrl_y).line_to(x, y);
        };

        let ctrl = Point::new(ctrl_x, ctrl_y);
        let to = Point::new(x, y);
        let radius = radius.into().max(Dp::ZERO).get();
        if radius <= 0.0 {
            return this.line_to(ctrl.x, ctrl.y).line_to(to.x, to.y);
        }

        let p0 = (from.x.get(), from.y.get());
        let p1 = (ctrl.x.get(), ctrl.y.get());
        let p2 = (to.x.get(), to.y.get());
        let v1 = normalize_vec((p0.0 - p1.0, p0.1 - p1.1));
        let v2 = normalize_vec((p2.0 - p1.0, p2.1 - p1.1));
        let dot = (v1.0 * v2.0 + v1.1 * v2.1).clamp(-1.0, 1.0);
        let angle = dot.acos();
        if angle <= 1e-3 || (std::f32::consts::PI - angle).abs() <= 1e-3 {
            return this.line_to(ctrl.x, ctrl.y).line_to(to.x, to.y);
        }

        let tangent = radius / (angle * 0.5).tan();
        let start = (p1.0 + v1.0 * tangent, p1.1 + v1.1 * tangent);
        let end = (p1.0 + v2.0 * tangent, p1.1 + v2.1 * tangent);
        let bisector = normalize_vec((v1.0 + v2.0, v1.1 + v2.1));
        let center_distance = radius / (angle * 0.5).sin();
        let center = (
            p1.0 + bisector.0 * center_distance,
            p1.1 + bisector.1 * center_distance,
        );
        let start_angle = (start.1 - center.1).atan2(start.0 - center.0);
        let mut sweep = (end.1 - center.1).atan2(end.0 - center.0) - start_angle;
        if sweep <= -std::f32::consts::PI {
            sweep += std::f32::consts::TAU;
        } else if sweep > std::f32::consts::PI {
            sweep -= std::f32::consts::TAU;
        }

        append_arc_segments(
            this.line_to(start.0, start.1),
            Point::new(center.0, center.1),
            radius,
            start_angle,
            sweep,
            true,
        )
    }
}
