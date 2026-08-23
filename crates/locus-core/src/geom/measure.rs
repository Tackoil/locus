//! Deterministic geometric measurements.

use super::{ConstructError, Point, Polygon, Segment};
use crate::tol::Tolerance;

/// Euclidean distance between two points.
#[must_use]
pub fn dist(lhs: Point, rhs: Point) -> f64 {
    (rhs - lhs).norm()
}

/// Signed horizontal displacement `x(Q) - x(P)`.
#[must_use]
pub fn hdist(lhs: Point, rhs: Point) -> f64 {
    rhs.x - lhs.x
}

/// Signed vertical displacement `y(Q) - y(P)`.
#[must_use]
pub fn vdist(lhs: Point, rhs: Point) -> f64 {
    rhs.y - lhs.y
}

/// Unsigned interior angle `ABC` in radians, in `(0, pi)`.
pub fn angle(a: Point, b: Point, c: Point, tolerance: Tolerance) -> Result<f64, ConstructError> {
    let ba = a - b;
    let bc = c - b;
    if tolerance.construction_length_zero(ba.norm())
        || tolerance.construction_length_zero(bc.norm())
    {
        return Err(ConstructError::Degenerate {
            operation: "angle",
            reason: "angle ray has zero length",
        });
    }
    let result = libm::atan2(ba.cross(bc).abs(), ba.dot(bc));
    if result > 0.0 && result < core::f64::consts::PI {
        Ok(result)
    } else {
        Err(ConstructError::Degenerate {
            operation: "angle",
            reason: "angle rays are collinear",
        })
    }
}

/// CCW direction of `A -> B` from +x, in `[0, 2*pi)` radians.
pub fn dir_angle(a: Point, b: Point, tolerance: Tolerance) -> Result<f64, ConstructError> {
    let direction = b - a;
    if tolerance.construction_length_zero(direction.norm()) {
        return Err(ConstructError::Degenerate {
            operation: "dir_angle",
            reason: "points coincide",
        });
    }
    Ok(normalize_angle(libm::atan2(direction.y, direction.x)))
}

/// Length of a segment.
#[must_use]
pub fn length(segment: Segment) -> f64 {
    dist(segment.start, segment.end)
}

/// Unsigned polygon area.
#[must_use]
pub fn area(polygon: &Polygon) -> f64 {
    signed_area_vertices(polygon.vertices()).abs()
}

/// Polygon perimeter, accumulated in vertex order.
#[must_use]
pub fn perimeter(polygon: &Polygon) -> f64 {
    let vertices = polygon.vertices();
    vertices
        .iter()
        .copied()
        .zip(vertices.iter().copied().cycle().skip(1))
        .take(vertices.len())
        .map(|(start, end)| dist(start, end))
        .sum()
}

/// Affine parameter of `point` along the directed carrier `start -> end`.
///
/// The result is 0 at `start`, 1 at `end`, and may lie outside that interval.
pub fn ratio_along_line(
    point: Point,
    start: Point,
    end: Point,
    tolerance: Tolerance,
) -> Result<f64, ConstructError> {
    let direction = end - start;
    let length = direction.norm();
    if tolerance.construction_length_zero(length) {
        return Err(ConstructError::Degenerate {
            operation: "ratio_along_line",
            reason: "line-defining points coincide",
        });
    }
    Ok((point - start).dot(direction) / direction.norm_squared())
}

pub(crate) fn signed_area_vertices(vertices: &[Point]) -> f64 {
    let origin = vertices[0];
    vertices
        .iter()
        .copied()
        .zip(vertices.iter().copied().cycle().skip(1))
        .take(vertices.len())
        .map(|(start, end)| (start - origin).cross(end - origin))
        .sum::<f64>()
        * 0.5
}

pub(crate) fn normalize_angle(angle: f64) -> f64 {
    let tau = 2.0 * core::f64::consts::PI;
    let reduced = angle % tau;
    if reduced < 0.0 {
        reduced + tau
    } else {
        reduced
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::float_cmp)]
mod tests {
    use super::*;
    use crate::geom::{BBox, construct};

    fn tolerance() -> Tolerance {
        let bbox = BBox::from_points([Point::new(-10.0, -10.0), Point::new(10.0, 10.0)])
            .expect("valid bbox");
        Tolerance::from_bbox(&bbox)
    }

    #[test]
    fn coordinate_and_angle_conventions_are_y_up_and_ccw() {
        let tol = tolerance();
        assert_eq!(hdist(Point::new(1.0, 2.0), Point::new(4.0, 8.0)), 3.0);
        assert_eq!(vdist(Point::new(1.0, 2.0), Point::new(4.0, 8.0)), 6.0);
        let direction =
            dir_angle(Point::new(0.0, 0.0), Point::new(0.0, 2.0), tol).expect("nonzero direction");
        assert!(tol.angle_zero(direction - core::f64::consts::FRAC_PI_2));
    }

    #[test]
    fn polygon_area_perimeter_and_ratio_have_known_answers() {
        let tol = tolerance();
        let triangle = construct::polygon(
            vec![
                Point::new(0.0, 0.0),
                Point::new(4.0, 0.0),
                Point::new(0.0, 3.0),
            ],
            tol,
        )
        .expect("simple triangle");
        assert_eq!(area(&triangle), 6.0);
        assert_eq!(perimeter(&triangle), 12.0);
        assert_eq!(
            ratio_along_line(
                Point::new(3.0, 0.0),
                Point::new(1.0, 0.0),
                Point::new(5.0, 0.0),
                tol,
            )
            .expect("defined line"),
            0.5
        );
    }

    #[test]
    fn polygon_area_is_translation_invariant() {
        let offset = Point::new(1.0e8, 1.0e8);
        let bbox =
            BBox::from_points([offset, Point::new(offset.x + 1.0, offset.y + 1.0)]).expect("bbox");
        let tol = Tolerance::from_bbox(&bbox);
        let square = construct::polygon(
            vec![
                offset,
                Point::new(offset.x + 1.0, offset.y),
                Point::new(offset.x + 1.0, offset.y + 1.0),
                Point::new(offset.x, offset.y + 1.0),
            ],
            tol,
        )
        .expect("translated unit square");
        assert_eq!(area(&square), 1.0);
    }
}
