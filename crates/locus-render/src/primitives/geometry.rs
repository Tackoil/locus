//! Visible geometry, directed arc sweeps, and finite clipping.
use super::{Command, Shape, path};
use crate::viewport::Viewport;
use locus_core::{Arc, BBox, Point, PointMark, ResolvedObject, Segment, Vec2, geom::Bspline};
use locus_core::{PathGeometry, ResolvedPath};

pub(super) fn geometry(object: &ResolvedObject, view: Viewport) -> Vec<Shape> {
    match object {
        ResolvedObject::Point(point) => point_shapes(point.geometry, point.mark, view),
        ResolvedObject::Path(ResolvedPath {
            geometry: PathGeometry::Segment(segment),
            ..
        }) => vec![segment_shape(*segment, view)],
        ResolvedObject::Line(line) => {
            clipped(line.start, line.direction(), f64::NEG_INFINITY, view)
        }
        ResolvedObject::Ray(ray) => clipped(ray.start, ray.direction(), 0.0, view),
        _ => curved_geometry(object, view),
    }
}

fn curved_geometry(object: &ResolvedObject, view: Viewport) -> Vec<Shape> {
    match object {
        ResolvedObject::Circle(circle) => vec![Shape::Circle {
            center: view.to_svg(circle.center),
            radius: circle.radius * view.pixels_per_unit,
            disk: false,
        }],
        ResolvedObject::Arc(arc) => {
            arc_shapes(arc.geometry, arc.angle_mark_count.unwrap_or(1), view)
        }
        ResolvedObject::Ellipse(ellipse) => vec![Shape::Ellipse {
            center: view.to_svg(ellipse.center),
            radius_x: ellipse.semimajor * view.pixels_per_unit,
            radius_y: ellipse.semiminor * view.pixels_per_unit,
            rotation: -ellipse.rotation,
        }],
        ResolvedObject::RoundedRect(rect) => {
            let bounds = rect.bbox();
            vec![Shape::RoundedRect {
                min: view.to_svg(bounds.northwest()),
                max: view.to_svg(bounds.southeast()),
                radius: rect.corner_radius * view.pixels_per_unit,
            }]
        }
        ResolvedObject::Bspline(spline) => spline_shape(spline, view).into_iter().collect(),
        ResolvedObject::Polygon(poly)
        | ResolvedObject::Rect(poly)
        | ResolvedObject::Diamond(poly) => vec![path(
            poly.vertices().iter().map(|p| view.to_svg(*p)),
            true,
            false,
        )],
        _ => Vec::new(),
    }
}

fn spline_shape(spline: &Bspline, view: Viewport) -> Option<Shape> {
    let spans = spline.bezier_spans().ok()?;
    let first = spans.first()?;
    let mut commands = vec![Command::Move(view.to_svg(first[0]))];
    commands.extend(spans.into_iter().map(|span| {
        Command::Cubic(
            view.to_svg(span[1]),
            view.to_svg(span[2]),
            view.to_svg(span[3]),
        )
    }));
    Some(Shape::Path {
        commands,
        closed: false,
        ink: false,
    })
}

fn point_shapes(point: Point, mark: PointMark, view: Viewport) -> Vec<Shape> {
    let p = view.to_svg(point);
    match mark {
        PointMark::None => Vec::new(),
        PointMark::Dot => vec![Shape::Circle {
            center: p,
            radius: 2.0,
            disk: true,
        }],
        PointMark::Cross => vec![
            path(
                [p + Vec2::new(-2.0, -2.0), p + Vec2::new(2.0, 2.0)],
                false,
                false,
            ),
            path(
                [p + Vec2::new(-2.0, 2.0), p + Vec2::new(2.0, -2.0)],
                false,
                false,
            ),
        ],
    }
}

pub(super) fn segment_shape(segment: Segment, view: Viewport) -> Shape {
    path(
        [view.to_svg(segment.start), view.to_svg(segment.end)],
        false,
        false,
    )
}

fn clipped(start: Point, direction: Vec2, low: f64, view: Viewport) -> Vec<Shape> {
    clip(start, direction, low, view.bounds)
        .map(|s| vec![segment_shape(s, view)])
        .unwrap_or_default()
}

// Exact parameter-domain clipping is not a tolerance test.
fn clip(start: Point, direction: Vec2, mut low: f64, bounds: BBox) -> Option<Segment> {
    let mut high = f64::INFINITY;
    for (origin, delta, min, max) in [
        (start.x, direction.x, bounds.min.x, bounds.max.x),
        (start.y, direction.y, bounds.min.y, bounds.max.y),
    ] {
        if delta == 0.0 {
            if origin < min || origin > max {
                return None;
            }
        } else {
            let a = (min - origin) / delta;
            let b = (max - origin) / delta;
            low = low.max(a.min(b));
            high = high.min(a.max(b));
        }
    }
    (low <= high && low.is_finite() && high.is_finite()).then(|| Segment {
        start: start + direction * low,
        end: start + direction * high,
    })
}

fn arc_shapes(arc: Arc, count: u8, view: Viewport) -> Vec<Shape> {
    let center = view.to_svg(arc.circle.center);
    let start = view.to_svg(arc.start);
    let end = view.to_svg(arc.end);
    let angle = libm::atan2(start.y - center.y, start.x - center.x);
    let end_angle = libm::atan2(end.y - center.y, end.x - center.x);
    let sweep = if arc.ccw {
        -positive_turn(angle - end_angle)
    } else {
        positive_turn(end_angle - angle)
    };
    (0..count)
        .map(|index| {
            let radius = arc.circle.radius * view.pixels_per_unit + f64::from(index) * 3.0;
            let offset = f64::from(index) * 3.0;
            Shape::Arc {
                start: start + Vec2::new(libm::cos(angle), libm::sin(angle)) * offset,
                end: end + Vec2::new(libm::cos(end_angle), libm::sin(end_angle)) * offset,
                radius,
                sweep,
                angle,
            }
        })
        .collect()
}

fn positive_turn(angle: f64) -> f64 {
    let reduced = libm::fmod(angle, core::f64::consts::TAU);
    if reduced < 0.0 {
        reduced + core::f64::consts::TAU
    } else {
        reduced
    }
}
