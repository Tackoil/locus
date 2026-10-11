//! Deterministic Eberly nearest-point root and envelope-theorem derivatives.
use super::{ellipse_normal, sub};
use crate::geom::Point;
use crate::geom::route::{RoutePoint, RouteScalar};

/// Nearest local ellipse point; medial-axis ties select nonnegative local y.
#[must_use]
pub fn ellipse_nearest(x: f64, y: f64, a: f64, b: f64) -> Point {
    let (sx, sy) = (
        if x < 0.0 { -1.0 } else { 1.0 },
        if y < 0.0 { -1.0 } else { 1.0 },
    );
    let (x, y) = (x.abs(), y.abs());
    let nearest = if y == 0.0 {
        axis_nearest(x, a, b)
    } else {
        root_nearest(x, y, a, b)
    };
    Point::new(sx * nearest.x, sy * nearest.y)
}
fn axis_nearest(x: f64, a: f64, b: f64) -> Point {
    let difference = a * a - b * b;
    if difference > 0.0 && a * x < difference {
        let qx = a * a * x / difference;
        Point::new(qx, b * libm::sqrt((1.0 - (qx / a) * (qx / a)).max(0.0)))
    } else {
        Point::new(a, 0.0)
    }
}
fn root_nearest(local_x: f64, local_y: f64, major: f64, minor: f64) -> Point {
    // Solve for root=t+b² rather than t: root stays positive near the medial axis.
    let difference = major * major - minor * minor;
    let mut low = minor * local_y;
    let mut high = minor * minor + libm::hypot(major * local_x, minor * local_y);
    for _ in 0..crate::preset::ELLIPSE_DISTANCE_ITERATIONS {
        let root = (low + high) * 0.5;
        let major_ratio = major * local_x / (root + difference);
        let minor_ratio = minor * local_y / root;
        if major_ratio * major_ratio + minor_ratio * minor_ratio > 1.0 {
            low = root;
        } else {
            high = root;
        }
    }
    let root = (low + high) * 0.5;
    Point::new(
        major * major * local_x / (root + difference),
        minor * minor * local_y / root,
    )
}

/// Exact ellipse signed distance and smooth-boundary envelope derivatives.
#[must_use]
pub fn ellipse_distance<S: RouteScalar>(
    point: &RoutePoint<S>,
    center: &RoutePoint<S>,
    major: &S,
    minor: &S,
    cosine: &S,
    sine: &S,
) -> S {
    let delta = sub(point, center);
    let local_x = delta.x.mul(cosine).add(&delta.y.mul(sine));
    let local_y = delta.y.mul(cosine).sub(&delta.x.mul(sine));
    let nearest = ellipse_nearest(
        local_x.value(),
        local_y.value(),
        major.value(),
        minor.value(),
    );
    let normal = ellipse_normal(nearest, major.value(), minor.value());
    // Fixed nearest parameter differentiates the envelope, including distance zero.
    let qx = major.scale(nearest.x / major.value());
    let qy = minor.scale(nearest.y / minor.value());
    local_x
        .sub(&qx)
        .scale(normal.x)
        .add(&local_y.sub(&qy).scale(normal.y))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn medial_axis_and_circle_nearest_points() {
        assert_eq!(ellipse_nearest(0.0, 0.0, 4.0, 1.0), Point::new(0.0, 1.0));
        let q = ellipse_nearest(1.0, 0.0, 4.0, 1.0);
        assert!(((Point::new(1.0, 0.0) - q).norm() - libm::sqrt(14.0 / 15.0)).abs() < 1e-12);
        let q = ellipse_nearest(1.0, 1.0, 2.0, 2.0);
        assert!((q.x - libm::sqrt(2.0)).abs() < 1e-12);
        assert!((q.y - libm::sqrt(2.0)).abs() < 1e-12);
    }
    #[test]
    fn exact_distance_inside_outside_and_rotated() {
        let origin = RoutePoint { x: 0.0, y: 0.0 };
        let p = RoutePoint { x: 1.0, y: 0.0 };
        assert!(
            (ellipse_distance(&p, &origin, &4.0, &1.0, &1.0, &0.0) + libm::sqrt(14.0 / 15.0)).abs()
                < 1e-12
        );
        let p = RoutePoint { x: 0.0, y: 5.0 };
        assert!((ellipse_distance(&p, &origin, &4.0, &1.0, &0.0, &1.0) - 1.0).abs() < 1e-12);
    }
}
