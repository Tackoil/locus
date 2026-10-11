//! Shared retained span collision geometry with conservative curve error tubes.

use super::{Core, core_separation, maximum, minimum, norm, sub};
use crate::geom::route::{Route, RoutePoint, RouteScalar};

/// Fixed curve subdivision count shared by primal and differentiated avoidance.
pub use crate::preset::COLLISION_SUBDIVISIONS;

/// Stroke cores for a logical route; straight spans are exact.
#[must_use]
pub fn route_cores<S: RouteScalar>(route: &Route<S>, stroke: &S) -> Vec<Core<S>> {
    let Some(control) = &route.control else {
        let points = std::iter::once(route.from.clone())
            .chain(route.via.iter().cloned())
            .chain(std::iter::once(route.to.clone()))
            .collect::<Vec<_>>();
        return span_cores(&points, stroke);
    };
    let second = RoutePoint {
        x: route.from.x.sub(&control.x.scale(2.0)).add(&route.to.x),
        y: route.from.y.sub(&control.y.scale(2.0)).add(&route.to.y),
    };
    let count = f64::from(COLLISION_SUBDIVISIONS);
    let radius = stroke.add(&norm(&second).scale(1.0 / (4.0 * count * count)));
    let points = (0..=COLLISION_SUBDIVISIONS)
        .map(|i| route.sample(&stroke.constant(f64::from(i) / count)))
        .collect::<Vec<_>>();
    span_cores(&points, &radius)
}

/// Cubic Bézier collision spans, inflated by an upper bound on chord error.
#[must_use]
pub fn cubic_cores<S: RouteScalar>(controls: &[RoutePoint<S>; 4], stroke: &S) -> Vec<Core<S>> {
    let first = second_difference(&controls[0], &controls[1], &controls[2]);
    let last = second_difference(&controls[1], &controls[2], &controls[3]);
    let count = f64::from(COLLISION_SUBDIVISIONS);
    let radius = stroke.add(&maximum(norm(&first), norm(&last)).scale(0.75 / (count * count)));
    let points = (0..=COLLISION_SUBDIVISIONS)
        .map(|i| cubic_sample(controls, &stroke.constant(f64::from(i) / count)))
        .collect::<Vec<_>>();
    span_cores(&points, &radius)
}

fn second_difference<S: RouteScalar>(
    a: &RoutePoint<S>,
    b: &RoutePoint<S>,
    c: &RoutePoint<S>,
) -> RoutePoint<S> {
    let before = sub(b, a);
    let after = sub(c, b);
    sub(&after, &before)
}

fn cubic_sample<S: RouteScalar>(points: &[RoutePoint<S>; 4], parameter: &S) -> RoutePoint<S> {
    let first = interpolate(&points[0], &points[1], parameter);
    let middle = interpolate(&points[1], &points[2], parameter);
    let last = interpolate(&points[2], &points[3], parameter);
    interpolate(
        &interpolate(&first, &middle, parameter),
        &interpolate(&middle, &last, parameter),
        parameter,
    )
}

fn interpolate<S: RouteScalar>(a: &RoutePoint<S>, b: &RoutePoint<S>, t: &S) -> RoutePoint<S> {
    RoutePoint {
        x: a.x.add(&b.x.sub(&a.x).mul(t)),
        y: a.y.add(&b.y.sub(&a.y).mul(t)),
    }
}

fn span_cores<S: RouteScalar>(points: &[RoutePoint<S>], radius: &S) -> Vec<Core<S>> {
    points
        .windows(2)
        .map(|pair| Core::new(pair.to_vec(), radius.clone()))
        .collect()
}

/// Minimum signed separation across two retained stroke or shape core lists.
#[must_use]
pub fn cores_separation<S: RouteScalar>(
    a: &[Core<S>],
    b: &[Core<S>],
    forced: Option<usize>,
) -> Option<S> {
    a.iter()
        .flat_map(|first| {
            b.iter()
                .map(move |second| core_separation(first, second, forced))
        })
        .reduce(minimum)
}
