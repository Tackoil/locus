//! Shared live-piece signed distances and shape-aware separation.

mod attachment;
mod ellipse;
mod paths;
mod separation;
pub use ellipse::{ellipse_distance, ellipse_nearest};
pub use paths::{COLLISION_SUBDIVISIONS, cores_separation, cubic_cores, route_cores};
pub use separation::{Core, core_separation};

use super::route::{RoutePoint, RouteScalar};
use super::{Point, Vec2};
use crate::scene::ResolvedObject;
use crate::scene::{PathGeometry, ResolvedPath};

pub(crate) fn sub<S: RouteScalar>(a: &RoutePoint<S>, b: &RoutePoint<S>) -> RoutePoint<S> {
    RoutePoint {
        x: a.x.sub(&b.x),
        y: a.y.sub(&b.y),
    }
}
pub(crate) fn dot<S: RouteScalar>(a: &RoutePoint<S>, b: &RoutePoint<S>) -> S {
    a.x.mul(&b.x).add(&a.y.mul(&b.y))
}
pub(crate) fn norm<S: RouteScalar>(p: &RoutePoint<S>) -> S {
    let square = dot(p, p);
    if square.value() == 0.0 {
        p.x.clone()
    } else {
        square.sqrt()
    }
}

pub(crate) fn primal<S: RouteScalar>(point: &RoutePoint<S>) -> RoutePoint<f64> {
    RoutePoint {
        x: point.x.value(),
        y: point.y.value(),
    }
}
pub(crate) fn minimum<S: RouteScalar>(a: S, b: S) -> S {
    if a.value() <= b.value() { a } else { b }
}
pub(crate) fn maximum<S: RouteScalar>(a: S, b: S) -> S {
    if a.value() >= b.value() { a } else { b }
}
/// Exact distance to a finite segment, with live endpoint/interior selection.
#[must_use]
pub fn segment_distance<S: RouteScalar>(
    point: &RoutePoint<S>,
    start: &RoutePoint<S>,
    end: &RoutePoint<S>,
) -> S {
    let delta = sub(end, start);
    let square = dot(&delta, &delta);
    if square.value() == 0.0 {
        return norm(&sub(point, start));
    }
    let parameter = dot(&sub(point, start), &delta).div(&square);
    let parameter = maximum(
        parameter.constant(0.0),
        minimum(parameter.clone(), parameter.constant(1.0)),
    );
    let nearest = RoutePoint {
        x: start.x.add(&delta.x.mul(&parameter)),
        y: start.y.add(&delta.y.mul(&parameter)),
    };
    let offset = sub(point, &nearest);
    if dot(&offset, &offset).value() == 0.0 {
        let length = square.sqrt();
        offset
            .x
            .mul(&delta.y)
            .sub(&offset.y.mul(&delta.x))
            .div(&length)
    } else {
        norm(&offset)
    }
}
/// Exact signed distance to a simple polygon outline; negative inside.
#[must_use]
pub fn polygon_distance<S: RouteScalar>(vertices: &[RoutePoint<S>], p: &RoutePoint<S>) -> S {
    let mut distance = f64::INFINITY;
    let mut selected = (&vertices[0], &vertices[1]);
    for (a, b) in edges(vertices) {
        let candidate = segment_distance(&primal(p), &primal(a), &primal(b));
        if candidate < distance {
            distance = candidate;
            selected = (a, b);
        }
    }
    let distance = if distance.is_finite() {
        segment_distance(p, selected.0, selected.1)
    } else {
        p.x.constant(distance)
    };
    if distance.value() == 0.0 {
        return boundary_distance(vertices, p, selected);
    }
    if polygon_contains(vertices, p) {
        distance.scale(-1.0)
    } else {
        distance
    }
}
fn boundary_distance<S: RouteScalar>(
    v: &[RoutePoint<S>],
    p: &RoutePoint<S>,
    edge: (&RoutePoint<S>, &RoutePoint<S>),
) -> S {
    let area: f64 = edges(v)
        .map(|(a, b)| a.x.value() * b.y.value() - a.y.value() * b.x.value())
        .sum();
    let d = sub(edge.1, edge.0);
    let offset = sub(p, edge.0);
    offset
        .x
        .mul(&d.y)
        .sub(&offset.y.mul(&d.x))
        .div(&norm(&d))
        .scale(if area < 0.0 { -1.0 } else { 1.0 })
}
pub(crate) fn edges<S>(
    v: &[RoutePoint<S>],
) -> impl Iterator<Item = (&RoutePoint<S>, &RoutePoint<S>)> {
    v.iter()
        .zip(v.iter().cycle().skip(1))
        .take(if v.len() == 2 { 1 } else { v.len() })
}
pub(crate) fn polygon_contains<S: RouteScalar>(v: &[RoutePoint<S>], p: &RoutePoint<S>) -> bool {
    let (x, y) = (p.x.value(), p.y.value());
    let mut inside = false;
    for (a, b) in edges(v) {
        let (ax, ay, bx, by) = (a.x.value(), a.y.value(), b.x.value(), b.y.value());
        if (ay > y) != (by > y) && x < (bx - ax) * (y - ay) / (by - ay) + ax {
            inside = !inside;
        }
    }
    inside
}
/// Rectangle vertices in counterclockwise order, including degenerate cores.
#[must_use]
pub fn rectangle<S: RouteScalar>(
    center: &RoutePoint<S>,
    width: &S,
    height: &S,
) -> Vec<RoutePoint<S>> {
    [(-0.5, -0.5), (0.5, -0.5), (0.5, 0.5), (-0.5, 0.5)]
        .into_iter()
        .map(|(x, y)| RoutePoint {
            x: center.x.add(&width.scale(x)),
            y: center.y.add(&height.scale(y)),
        })
        .collect()
}
/// Exact signed boundary distance for a resolved container.
#[must_use]
pub fn object_signed_distance(object: &ResolvedObject, point: Point) -> Option<f64> {
    let p = RoutePoint::from(point);
    match object {
        ResolvedObject::Circle(c) => Some((point - c.center).norm() - c.radius),
        ResolvedObject::Ellipse(e) => Some(ellipse_distance(
            &p,
            &e.center.into(),
            &e.semimajor,
            &e.semiminor,
            &libm::cos(e.rotation),
            &libm::sin(e.rotation),
        )),
        ResolvedObject::RoundedRect(r) => Some(r.signed_distance(point)),
        ResolvedObject::Polygon(v) | ResolvedObject::Rect(v) | ResolvedObject::Diamond(v) => {
            Some(polygon_distance(
                &v.vertices()
                    .iter()
                    .copied()
                    .map(Into::into)
                    .collect::<Vec<_>>(),
                &p,
            ))
        }
        _ => None,
    }
}
/// Shape-aware signed separation of resolved objects, including stroke clearance.
#[must_use]
pub fn object_separation(
    a: &ResolvedObject,
    b: &ResolvedObject,
    stroke_half_width: f64,
    forced_axis: Option<usize>,
) -> Option<f64> {
    if object_route(a).is_some()
        || object_route(b).is_some()
        || matches!(a, ResolvedObject::Bspline(_))
        || matches!(b, ResolvedObject::Bspline(_))
    {
        return cores_separation(
            &object_cores(a, stroke_half_width)?,
            &object_cores(b, stroke_half_width)?,
            forced_axis,
        );
    }
    if let Some(distance) = ellipse_point_pair(a, b, forced_axis, true) {
        return Some(distance);
    }
    if let Some(distance) = ellipse_point_pair(b, a, forced_axis, false) {
        return Some(distance);
    }
    Some(core_separation(
        &object_core(a, stroke_half_width)?,
        &object_core(b, stroke_half_width)?,
        forced_axis,
    ))
}
fn object_cores(object: &ResolvedObject, stroke: f64) -> Option<Vec<Core<f64>>> {
    if let Some(route) = object_route(object) {
        return Some(route_cores(&route, &stroke));
    }
    if let ResolvedObject::Bspline(spline) = object {
        return Some(
            spline
                .bezier_spans()
                .ok()?
                .iter()
                .flat_map(|span| cubic_cores(&span.map(Into::into), &stroke))
                .collect(),
        );
    }
    Some(vec![object_core(object, stroke)?])
}
fn object_route(object: &ResolvedObject) -> Option<super::route::Route<f64>> {
    object.as_path()?.route().ok().flatten()
}

/// Avoidance separation permitting outward contact at an attached shape endpoint.
/// An endpoint entering the obstacle interior receives no contact exemption.
#[must_use]
pub fn attached_route_separation(
    route: &super::route::Route,
    obstacle: &ResolvedObject,
    stroke: f64,
    clearance: f64,
) -> Option<f64> {
    let start = attached_exit(route, obstacle, stroke + clearance, false);
    let end = attached_exit(route, obstacle, stroke + clearance, true);
    let retained = route.retained((start, end));
    let cores = object_cores(obstacle, stroke)?;
    cores_separation(&route_cores(&retained, &stroke), &cores, None)
}

fn attached_exit(
    route: &super::route::Route,
    obstacle: &ResolvedObject,
    clearance: f64,
    reverse: bool,
) -> f64 {
    let parameter = |t: f64| if reverse { 1.0 - t } else { t };
    let distance = |t| object_signed_distance(obstacle, route.sample(&parameter(t)).into());
    if !distance(0.0).is_some_and(|d| d.abs() <= 1e-10) {
        return parameter(0.0);
    }
    for index in 1..=crate::preset::ATTACHMENT_SEARCH_STEPS {
        let t = f64::from(index) / f64::from(crate::preset::ATTACHMENT_SEARCH_STEPS);
        let Some(value) = distance(t) else {
            return parameter(0.0);
        };
        if value < -1e-12 {
            return parameter(0.0);
        }
        if value >= clearance + 1e-10 {
            return if attachment::prefix_clear(route, obstacle, t, reverse) {
                parameter(t)
            } else {
                parameter(0.0)
            };
        }
    }
    parameter(0.0)
}
fn ellipse_point_pair(
    a: &ResolvedObject,
    b: &ResolvedObject,
    forced: Option<usize>,
    ellipse_first: bool,
) -> Option<f64> {
    let ResolvedObject::Ellipse(ellipse) = a else {
        return None;
    };
    let (point, radius) = match b {
        ResolvedObject::Point(p) => (p.geometry, 0.0),
        ResolvedObject::Circle(c) => (c.center, c.radius),
        _ => return None,
    };
    let actual = object_signed_distance(a, point)? - radius;
    let Some(choice) = forced.filter(|_| actual < 0.0) else {
        return Some(actual);
    };
    let horizontal = (choice / 2) % 2 == 0;
    let delta = point - ellipse.center;
    let offset = if horizontal { delta.x } else { delta.y };
    let sign = if (choice % 2 == 0) == ellipse_first {
        1.0
    } else {
        -1.0
    };
    let support = ellipse_cardinal_support(
        &ellipse.semimajor,
        &ellipse.semiminor,
        &libm::cos(ellipse.rotation),
        &libm::sin(ellipse.rotation),
        horizontal,
    );
    Some(sign * offset - support - radius)
}
/// Exact ellipse support distance on a world horizontal or vertical cardinal axis.
#[must_use]
pub fn ellipse_cardinal_support<S: RouteScalar>(
    major: &S,
    minor: &S,
    cosine: &S,
    sine: &S,
    horizontal: bool,
) -> S {
    let (major_projection, minor_projection) = if horizontal {
        (major.mul(cosine), minor.mul(sine))
    } else {
        (major.mul(sine), minor.mul(cosine))
    };
    major_projection
        .mul(&major_projection)
        .add(&minor_projection.mul(&minor_projection))
        .sqrt()
}

fn object_core(object: &ResolvedObject, stroke: f64) -> Option<Core<f64>> {
    let mut radius = 0.0;
    let vertices = match object {
        ResolvedObject::Point(p) => vec![p.geometry.into()],
        ResolvedObject::Circle(c) => {
            radius = c.radius;
            vec![c.center.into()]
        }
        ResolvedObject::Path(ResolvedPath {
            geometry: PathGeometry::Segment(s),
            ..
        }) => {
            radius = stroke;
            vec![s.start.into(), s.end.into()]
        }
        ResolvedObject::RoundedRect(r) => {
            radius = r.corner_radius;
            rectangle(
                &r.center.into(),
                &(r.width - 2.0 * radius),
                &(r.height - 2.0 * radius),
            )
        }
        ResolvedObject::Polygon(p) | ResolvedObject::Rect(p) | ResolvedObject::Diamond(p) => {
            p.vertices().iter().copied().map(Into::into).collect()
        }
        _ => {
            let b = object.bbox()?;
            rectangle(&b.center().into(), &b.width(), &b.height())
        }
    };
    Some(Core::new(vertices, radius))
}
/// Separation against route spans with conservative quadratic chord-error tubes.
#[must_use]
pub fn route_core_separation<S: RouteScalar>(
    route: &super::route::Route<S>,
    other: &Core<S>,
    stroke: &S,
    forced: Option<usize>,
) -> Option<S> {
    cores_separation(
        &route_cores(route, stroke),
        std::slice::from_ref(other),
        forced,
    )
}
/// Outward normal of the nearest ellipse point in local coordinates.
#[must_use]
pub fn ellipse_normal(nearest: Point, a: f64, b: f64) -> Vec2 {
    let v = Vec2::new(nearest.x / (a * a), nearest.y / (b * b));
    v / v.norm()
}

#[cfg(test)]
#[allow(
    clippy::float_cmp,
    reason = "exact feature distances and touching classifications are part of the contract"
)]
mod tests {
    use super::*;
    fn p(x: f64, y: f64) -> RoutePoint<f64> {
        RoutePoint { x, y }
    }
    #[test]
    fn rectangle_inradius_keeps_deepest_segment() {
        let v = rectangle(&p(0.0, 0.0), &10.0, &2.0);
        for x in [-4.0, 0.0, 4.0] {
            assert_eq!(polygon_distance(&v, &p(x, 0.0)), -1.0);
        }
        assert_eq!(polygon_distance(&v, &p(4.5, 0.0)), -0.5);
        assert!((polygon_distance(&v, &p(6.0, 2.0)) - libm::sqrt(2.0)).abs() < 1e-12);
    }
    #[test]
    fn direct_route_obstacle_uses_its_stroke_interior() -> Result<(), crate::geom::ConstructError> {
        let route =
            super::super::route::Route::direct(Point::new(-2.0, 0.0), Point::new(2.0, 0.0), 0.0)?;
        let square = Core::new(rectangle(&p(0.0, 0.0), &2.0, &2.0), 0.0);
        assert_eq!(
            route_core_separation(&route, &square, &0.1, None),
            Some(-1.1)
        );
        Ok(())
    }
    #[test]
    fn ellipse_forced_sides_leave_disjoint_exact_distance() {
        let ellipse = ResolvedObject::Ellipse(crate::geom::Ellipse {
            center: Point::new(0.0, 0.0),
            semimajor: 4.0,
            semiminor: 1.0,
            rotation: 0.0,
        });
        let point = ResolvedObject::Point(crate::scene::ResolvedPoint::new(Point::new(0.0, 0.0)));
        assert_eq!(object_separation(&ellipse, &point, 0.0, None), Some(-1.0));
        assert_eq!(
            object_separation(&ellipse, &point, 0.0, Some(0)),
            Some(-4.0)
        );
        assert_eq!(
            object_separation(&ellipse, &point, 0.0, Some(1)),
            Some(-4.0)
        );
        let circle = ResolvedObject::Circle(crate::geom::Circle {
            center: Point::new(0.0, 0.0),
            radius: 0.5,
        });
        assert_eq!(
            object_separation(&ellipse, &circle, 0.0, Some(2)),
            Some(-1.5)
        );
        let outside = ResolvedObject::Point(crate::scene::ResolvedPoint::new(Point::new(5.0, 0.0)));
        assert_eq!(
            object_separation(&ellipse, &outside, 0.0, Some(1)),
            Some(1.0)
        );
        let inside = ResolvedObject::Point(crate::scene::ResolvedPoint::new(Point::new(1.0, 0.0)));
        assert_eq!(
            object_separation(&ellipse, &inside, 0.0, Some(0)),
            Some(-3.0)
        );
        assert_eq!(
            object_separation(&inside, &ellipse, 0.0, Some(0)),
            Some(-5.0)
        );
    }
    #[test]
    fn diamond_distance_is_euclidean_not_implicit() {
        let v = vec![p(2.0, 0.0), p(0.0, 1.0), p(-2.0, 0.0), p(0.0, -1.0)];
        assert!((polygon_distance(&v, &p(0.0, 0.0)) + 2.0 / libm::sqrt(5.0)).abs() < 1e-12);
        assert_eq!(polygon_distance(&v, &p(3.0, 0.0)), 1.0);
    }
}
