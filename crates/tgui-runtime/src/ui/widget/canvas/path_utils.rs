use std::collections::HashSet;

use geo::{Contains, Coord, LineString, MultiPolygon, Polygon};
use lyon::geom::{Angle, ArcFlags, SvgArc};
use lyon::math::point;

use super::*;

pub(super) fn append_arc_segments(
    mut builder: PathBuilder,
    center: Point,
    radius: f32,
    start_angle: f32,
    sweep_angle: f32,
    connect_with_line: bool,
) -> PathBuilder {
    if radius <= 0.0 || sweep_angle.abs() <= f32::EPSILON {
        return builder;
    }

    let steps = ((sweep_angle.abs() / std::f32::consts::FRAC_PI_8).ceil() as usize).max(1);
    builder.commands.reserve(steps + 1);
    for index in 0..=steps {
        let t = index as f32 / steps as f32;
        let angle = start_angle + sweep_angle * t;
        let point_value = Point::new(
            center.x.get() + angle.cos() * radius,
            center.y.get() + angle.sin() * radius,
        );
        if index == 0 {
            builder = if connect_with_line {
                builder.line_to(point_value.x, point_value.y)
            } else {
                builder.move_to(point_value.x, point_value.y)
            };
        } else {
            builder = builder.line_to(point_value.x, point_value.y);
        }
    }
    builder
}

pub(super) fn normalize_vec(value: (f32, f32)) -> (f32, f32) {
    let length = (value.0 * value.0 + value.1 * value.1).sqrt();
    if length <= f32::EPSILON {
        (0.0, 0.0)
    } else {
        (value.0 / length, value.1 / length)
    }
}

pub(super) fn svg_point(abs: bool, current: (f32, f32), x: f32, y: f32) -> (f32, f32) {
    if abs {
        (x, y)
    } else {
        (current.0 + x, current.1 + y)
    }
}

pub(super) fn append_svg_arc_segments(
    mut builder: PathBuilder,
    from: (f32, f32),
    to: (f32, f32),
    radius_x: f32,
    radius_y: f32,
    x_axis_rotation_degrees: f32,
    large_arc: bool,
    sweep: bool,
) -> PathBuilder {
    if (from.0 - to.0).abs() <= f32::EPSILON && (from.1 - to.1).abs() <= f32::EPSILON {
        return builder;
    }

    if radius_x.abs() <= f32::EPSILON || radius_y.abs() <= f32::EPSILON {
        return builder.line_to(to.0, to.1);
    }

    let arc = SvgArc {
        from: point(from.0, from.1),
        to: point(to.0, to.1),
        radii: vector(radius_x.abs(), radius_y.abs()),
        x_rotation: Angle::degrees(x_axis_rotation_degrees),
        flags: ArcFlags { large_arc, sweep },
    };

    let mut segments = Vec::new();
    arc.for_each_cubic_bezier(&mut |segment| {
        segments.push((
            segment.ctrl1.x,
            segment.ctrl1.y,
            segment.ctrl2.x,
            segment.ctrl2.y,
            segment.to.x,
            segment.to.y,
        ));
    });
    for (ctrl1_x, ctrl1_y, ctrl2_x, ctrl2_y, to_x, to_y) in segments {
        builder = builder.cubic_to(ctrl1_x, ctrl1_y, ctrl2_x, ctrl2_y, to_x, to_y);
    }

    builder
}

pub(super) fn points_approx_equal(lhs: lyon::math::Point, rhs: lyon::math::Point) -> bool {
    (lhs.x - rhs.x).abs() <= 1e-3 && (lhs.y - rhs.y).abs() <= 1e-3
}

pub(super) fn dedupe_ring_points(points: &mut Vec<lyon::math::Point>) {
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

#[derive(Clone, Copy)]
pub(super) enum CanvasPathBooleanOperation {
    Union,
    Intersection,
    Difference,
    Xor,
}

struct PolygonizedRing {
    line: LineString<f64>,
    polygon: Polygon<f64>,
    abs_area: f64,
    orientation: i32,
    parent: Option<usize>,
    winding: i32,
    inside_filled: bool,
    active_shell: Option<usize>,
}

pub(super) fn rings_to_multi_polygon(
    rings: Vec<Vec<lyon::math::Point>>,
    fill_rule: CanvasFillRule,
) -> MultiPolygon<f64> {
    let mut ring_infos = rings
        .into_iter()
        .filter_map(|ring| {
            let coords = ring
                .into_iter()
                .map(|point_value| Coord {
                    x: point_value.x as f64,
                    y: point_value.y as f64,
                })
                .collect::<Vec<_>>();
            if coords.len() < 4 {
                return None;
            }

            let line = LineString::from(coords);
            let area = signed_ring_area(&line);
            Some(PolygonizedRing {
                polygon: Polygon::new(line.clone(), Vec::new()),
                line,
                abs_area: area.abs(),
                orientation: if area >= 0.0 { 1 } else { -1 },
                parent: None,
                winding: 0,
                inside_filled: false,
                active_shell: None,
            })
        })
        .collect::<Vec<_>>();

    for index in 0..ring_infos.len() {
        let point_value = ring_infos[index]
            .line
            .0
            .first()
            .copied()
            .unwrap_or(Coord { x: 0.0, y: 0.0 });
        let test_point = geo::Point::new(point_value.x, point_value.y);
        ring_infos[index].parent = (0..ring_infos.len())
            .filter(|candidate| {
                *candidate != index && ring_infos[*candidate].abs_area > ring_infos[index].abs_area
            })
            .filter(|candidate| ring_infos[*candidate].polygon.contains(&test_point))
            .min_by(|left, right| {
                ring_infos[*left]
                    .abs_area
                    .total_cmp(&ring_infos[*right].abs_area)
            });
    }

    let mut order = (0..ring_infos.len()).collect::<Vec<_>>();
    order.sort_by(|left, right| {
        ring_infos[*right]
            .abs_area
            .total_cmp(&ring_infos[*left].abs_area)
    });

    let mut polygons = Vec::<(LineString<f64>, Vec<LineString<f64>>)>::new();
    for index in order {
        let parent = ring_infos[index].parent;
        let parent_filled = parent
            .map(|value| ring_infos[value].inside_filled)
            .unwrap_or(false);
        let parent_winding = parent.map(|value| ring_infos[value].winding).unwrap_or(0);
        let active_shell_outside = parent.and_then(|value| ring_infos[value].active_shell);
        let current_winding = match fill_rule {
            CanvasFillRule::NonZero => parent_winding + ring_infos[index].orientation,
            CanvasFillRule::EvenOdd => parent_winding + 1,
        };
        let current_filled = match fill_rule {
            CanvasFillRule::NonZero => current_winding != 0,
            CanvasFillRule::EvenOdd => !parent_filled,
        };

        ring_infos[index].winding = current_winding;
        ring_infos[index].inside_filled = current_filled;
        ring_infos[index].active_shell = match (parent_filled, current_filled) {
            (false, true) => {
                polygons.push((oriented_ring(&ring_infos[index].line, true), Vec::new()));
                Some(polygons.len() - 1)
            }
            (true, false) => {
                if let Some(shell_index) = active_shell_outside {
                    polygons[shell_index]
                        .1
                        .push(oriented_ring(&ring_infos[index].line, false));
                }
                None
            }
            (_, true) => active_shell_outside,
            (_, false) => None,
        };
    }

    MultiPolygon(
        polygons
            .into_iter()
            .map(|(shell, holes)| Polygon::new(shell, holes))
            .collect(),
    )
}

fn signed_ring_area(ring: &LineString<f64>) -> f64 {
    let coords = &ring.0;
    if coords.len() < 3 {
        return 0.0;
    }

    let mut area = 0.0;
    for window in coords.windows(2) {
        area += window[0].x * window[1].y - window[1].x * window[0].y;
    }
    area * 0.5
}

fn oriented_ring(ring: &LineString<f64>, counter_clockwise: bool) -> LineString<f64> {
    let area = signed_ring_area(ring);
    let is_counter_clockwise = area >= 0.0;
    if is_counter_clockwise == counter_clockwise {
        return ring.clone();
    }

    let mut coords = ring.0.clone();
    coords.reverse();
    LineString::from(coords)
}

pub(super) fn append_ring(mut builder: PathBuilder, ring: &LineString<f64>) -> PathBuilder {
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
