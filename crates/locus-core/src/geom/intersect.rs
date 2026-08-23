//! Type-pair intersections with deterministic, first-operand branch ordering.

use thiserror::Error;

use super::measure::normalize_angle;
use super::{Arc, Circle, Line, Point, Ray, Segment, Vec2, arc_contains_angle, circle_angle};
use crate::tol::Tolerance;

/// A stable, parser-independent intersection failure suitable for E2xx diagnostics.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum IntersectError {
    /// The two objects have no finite common point.
    #[error("E201: objects do not intersect")]
    NoIntersection,
    /// The objects share infinitely many points.
    #[error("E203: coincident objects have infinitely many intersections")]
    Coincident,
    /// A stored object has no direction for the requested operation.
    #[error("E203: {object} is degenerate and has no intersection carrier")]
    Degenerate {
        /// Kind of the directionless stored object.
        object: &'static str,
    },
    /// Multiple ordered branches exist but no explicit index was supplied.
    #[error("E207: intersection has {count} branches; an explicit branch index is required")]
    Ambiguous {
        /// Number of ordered finite roots.
        count: usize,
    },
    /// An explicit branch does not exist.
    #[error("E208: branch {branch} is out of range for {count} intersections")]
    InvalidBranch {
        /// Requested explicit index.
        branch: usize,
        /// Number of ordered finite roots.
        count: usize,
    },
    /// Arithmetic produced a non-finite candidate.
    #[error("E202: intersection produced a non-finite coordinate")]
    NonFinite,
}

impl IntersectError {
    /// Stable construction diagnostic code.
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::NoIntersection => "E201",
            Self::Coincident | Self::Degenerate { .. } => "E203",
            Self::Ambiguous { .. } => "E207",
            Self::InvalidBranch { .. } => "E208",
            Self::NonFinite => "E202",
        }
    }
}

/// Computes all intersections, ordered by the specification's first-object rule.
pub trait Intersect<Rhs = Self> {
    /// Returns one or two finite intersections in deterministic branch order.
    fn intersections(&self, rhs: &Rhs, tolerance: Tolerance) -> Result<Vec<Point>, IntersectError>;
}

/// Resolves an optional explicit branch index.
///
/// An omitted index is accepted only for a unique intersection. For two roots,
/// `k` indexes the already sorted result from [`Intersect::intersections`].
pub fn intersect<Lhs, Rhs>(
    lhs: &Lhs,
    rhs: &Rhs,
    branch: Option<usize>,
    tolerance: Tolerance,
) -> Result<Point, IntersectError>
where
    Lhs: Intersect<Rhs>,
{
    let points = lhs.intersections(rhs, tolerance)?;
    match (points.len(), branch) {
        (1, None | Some(0)) => Ok(points[0]),
        (count, None) => Err(IntersectError::Ambiguous { count }),
        (count, Some(branch)) => points
            .get(branch)
            .copied()
            .ok_or(IntersectError::InvalidBranch { branch, count }),
    }
}

#[derive(Clone, Copy)]
enum ParameterDomain {
    Line,
    Ray,
    Segment,
}

#[derive(Clone, Copy)]
struct LinearCarrier {
    origin: Point,
    direction: Vec2,
    domain: ParameterDomain,
    kind: &'static str,
}

#[derive(Clone, Copy)]
struct ParameterInterval {
    lower: f64,
    upper: f64,
}

trait AsLinearCarrier {
    fn carrier(&self) -> LinearCarrier;
}

impl AsLinearCarrier for Line {
    fn carrier(&self) -> LinearCarrier {
        LinearCarrier {
            origin: self.start,
            direction: self.direction(),
            domain: ParameterDomain::Line,
            kind: "line",
        }
    }
}

impl AsLinearCarrier for Ray {
    fn carrier(&self) -> LinearCarrier {
        LinearCarrier {
            origin: self.start,
            direction: self.direction(),
            domain: ParameterDomain::Ray,
            kind: "ray",
        }
    }
}

impl AsLinearCarrier for Segment {
    fn carrier(&self) -> LinearCarrier {
        LinearCarrier {
            origin: self.start,
            direction: self.direction(),
            domain: ParameterDomain::Segment,
            kind: "segment",
        }
    }
}

fn linear_linear<Lhs: AsLinearCarrier, Rhs: AsLinearCarrier>(
    lhs: &Lhs,
    rhs: &Rhs,
    tolerance: Tolerance,
) -> Result<Vec<Point>, IntersectError> {
    let first = checked_carrier(lhs.carrier(), tolerance)?;
    let second = checked_carrier(rhs.carrier(), tolerance)?;
    let offset = second.origin - first.origin;
    if tolerance.directions_parallel(first.direction, second.direction) {
        return if tolerance.vector_on_direction(offset, first.direction) {
            collinear_linear_intersection(first, second, tolerance)
        } else {
            Err(IntersectError::NoIntersection)
        };
    }
    let denominator = first.direction.cross(second.direction);
    let first_parameter = offset.cross(second.direction) / denominator;
    let second_parameter = offset.cross(first.direction) / denominator;
    if parameter_is_in_domain(first_parameter, first.domain, tolerance)
        && parameter_is_in_domain(second_parameter, second.domain, tolerance)
    {
        finite_points(vec![first.origin + first.direction * first_parameter])
    } else {
        Err(IntersectError::NoIntersection)
    }
}

fn linear_circle<Lhs: AsLinearCarrier>(
    lhs: &Lhs,
    circle: Circle,
    tolerance: Tolerance,
) -> Result<Vec<Point>, IntersectError> {
    let carrier = checked_carrier(lhs.carrier(), tolerance)?;
    let norm_squared = carrier.direction.norm_squared();
    let to_center = circle.center - carrier.origin;
    let center_parameter = to_center.dot(carrier.direction) / norm_squared;
    let closest = carrier.origin + carrier.direction * center_parameter;
    let center_distance = (closest - circle.center).norm();
    if center_distance > circle.radius && !tolerance.length_eq(center_distance, circle.radius) {
        return Err(IntersectError::NoIntersection);
    }

    // The quadratic roots are `t0 +/- sqrt((r^2-d^2)/|v|^2)`. Sorting these
    // numeric parameters implements the spec rule for line/ray/segment as o1.
    let radial_square =
        (circle.radius * circle.radius - center_distance * center_distance).max(0.0);
    let parameter_offset = libm::sqrt(radial_square / norm_squared);
    let mut parameters = if tolerance.length_eq(center_distance, circle.radius) {
        vec![center_parameter]
    } else {
        vec![
            center_parameter - parameter_offset,
            center_parameter + parameter_offset,
        ]
    };
    parameters.retain(|parameter| parameter_is_in_domain(*parameter, carrier.domain, tolerance));
    if parameters.is_empty() {
        return Err(IntersectError::NoIntersection);
    }
    parameters.sort_by(f64::total_cmp);
    finite_points(
        parameters
            .into_iter()
            .map(|parameter| carrier.origin + carrier.direction * parameter)
            .collect(),
    )
}

fn circle_circle(
    lhs: Circle,
    rhs: Circle,
    tolerance: Tolerance,
) -> Result<Vec<Point>, IntersectError> {
    let centers = rhs.center - lhs.center;
    let center_distance = centers.norm();
    if tolerance.length_zero(center_distance) {
        return if tolerance.length_eq(lhs.radius, rhs.radius) {
            Err(IntersectError::Coincident)
        } else {
            Err(IntersectError::NoIntersection)
        };
    }
    let radius_sum = lhs.radius + rhs.radius;
    let radius_difference = (lhs.radius - rhs.radius).abs();
    if (center_distance > radius_sum && !tolerance.length_eq(center_distance, radius_sum))
        || (center_distance < radius_difference
            && !tolerance.length_eq(center_distance, radius_difference))
    {
        return Err(IntersectError::NoIntersection);
    }

    // Scale every length before evaluating the circle chord. The equivalent
    // unscaled Heron product has units L^4 and therefore underflows/overflows
    // long before the representable intersection coordinates do.
    let scale = lhs.radius.max(rhs.radius).max(center_distance);
    let lhs_radius = lhs.radius / scale;
    let rhs_radius = rhs.radius / scale;
    let distance = center_distance / scale;
    let sum = lhs_radius + rhs_radius;
    let difference = (lhs_radius - rhs_radius).abs();
    let along = scale * 0.5 * (distance + (lhs_radius - rhs_radius) * sum / distance);
    let base = lhs.center + centers * (along / center_distance);
    let perpendicular = centers.perpendicular() / center_distance;
    let tangent = tolerance.length_eq(center_distance, radius_sum)
        || tolerance.length_eq(center_distance, radius_difference);
    let mut points = if tangent {
        vec![base]
    } else {
        let height_product =
            (sum + distance) * (sum - distance) * (distance + difference) * (distance - difference);
        let height = scale * (libm::sqrt(height_product.max(0.0)) / (2.0 * distance));
        vec![base + perpendicular * height, base - perpendicular * height]
    };

    // For a circle as o1 the normative branch key is its normalized CCW angle
    // from +x, independent of the orientation or position of the second object.
    sort_by_circle_angle(&mut points, lhs);
    finite_points(points)
}

macro_rules! impl_linear_pair {
    ($lhs:ty, $rhs:ty) => {
        impl Intersect<$rhs> for $lhs {
            fn intersections(
                &self,
                rhs: &$rhs,
                tolerance: Tolerance,
            ) -> Result<Vec<Point>, IntersectError> {
                linear_linear(self, rhs, tolerance)
            }
        }
    };
}

impl_linear_pair!(Line, Line);
impl_linear_pair!(Line, Ray);
impl_linear_pair!(Line, Segment);
impl_linear_pair!(Ray, Line);
impl_linear_pair!(Ray, Ray);
impl_linear_pair!(Ray, Segment);
impl_linear_pair!(Segment, Line);
impl_linear_pair!(Segment, Ray);
impl_linear_pair!(Segment, Segment);

macro_rules! impl_linear_circle_pair {
    ($linear:ty) => {
        impl Intersect<Circle> for $linear {
            fn intersections(
                &self,
                circle: &Circle,
                tolerance: Tolerance,
            ) -> Result<Vec<Point>, IntersectError> {
                linear_circle(self, *circle, tolerance)
            }
        }

        impl Intersect<$linear> for Circle {
            fn intersections(
                &self,
                linear: &$linear,
                tolerance: Tolerance,
            ) -> Result<Vec<Point>, IntersectError> {
                let mut points = linear_circle(linear, *self, tolerance)?;
                sort_by_circle_angle(&mut points, *self);
                Ok(points)
            }
        }
    };
}

impl_linear_circle_pair!(Line);
impl_linear_circle_pair!(Ray);
impl_linear_circle_pair!(Segment);

impl Intersect<Circle> for Circle {
    fn intersections(
        &self,
        rhs: &Circle,
        tolerance: Tolerance,
    ) -> Result<Vec<Point>, IntersectError> {
        circle_circle(*self, *rhs, tolerance)
    }
}

macro_rules! impl_linear_arc_pair {
    ($linear:ty) => {
        impl Intersect<Arc> for $linear {
            fn intersections(
                &self,
                arc: &Arc,
                tolerance: Tolerance,
            ) -> Result<Vec<Point>, IntersectError> {
                let carrier = self.carrier();
                let points = linear_circle(self, arc.circle, tolerance)?;
                let mut filtered = filter_to_arc(points, *arc, tolerance)?;
                filtered.sort_by(|lhs, rhs| {
                    parameter_on_carrier(*lhs, carrier)
                        .total_cmp(&parameter_on_carrier(*rhs, carrier))
                });
                Ok(filtered)
            }
        }

        impl Intersect<$linear> for Arc {
            fn intersections(
                &self,
                linear: &$linear,
                tolerance: Tolerance,
            ) -> Result<Vec<Point>, IntersectError> {
                let points = linear_circle(linear, self.circle, tolerance)?;
                let mut filtered = filter_to_arc(points, *self, tolerance)?;
                sort_by_circle_angle(&mut filtered, self.circle);
                Ok(filtered)
            }
        }
    };
}

impl_linear_arc_pair!(Line);
impl_linear_arc_pair!(Ray);
impl_linear_arc_pair!(Segment);

impl Intersect<Arc> for Circle {
    fn intersections(&self, arc: &Arc, tolerance: Tolerance) -> Result<Vec<Point>, IntersectError> {
        if circles_coincident(*self, arc.circle, tolerance) {
            return Err(IntersectError::Coincident);
        }
        let mut points = filter_to_arc(
            circle_circle(*self, arc.circle, tolerance)?,
            *arc,
            tolerance,
        )?;
        sort_by_circle_angle(&mut points, *self);
        Ok(points)
    }
}

impl Intersect<Circle> for Arc {
    fn intersections(
        &self,
        circle: &Circle,
        tolerance: Tolerance,
    ) -> Result<Vec<Point>, IntersectError> {
        if circles_coincident(self.circle, *circle, tolerance) {
            return Err(IntersectError::Coincident);
        }
        let mut points = filter_to_arc(
            circle_circle(self.circle, *circle, tolerance)?,
            *self,
            tolerance,
        )?;
        sort_by_circle_angle(&mut points, self.circle);
        Ok(points)
    }
}

impl Intersect<Arc> for Arc {
    fn intersections(&self, rhs: &Arc, tolerance: Tolerance) -> Result<Vec<Point>, IntersectError> {
        if circles_coincident(self.circle, rhs.circle, tolerance) {
            return same_circle_arcs(*self, *rhs, tolerance);
        }
        let points = circle_circle(self.circle, rhs.circle, tolerance)?;
        let points = filter_to_arc(points, *self, tolerance)?;
        let mut points = filter_to_arc(points, *rhs, tolerance)?;
        sort_by_circle_angle(&mut points, self.circle);
        Ok(points)
    }
}

fn checked_carrier(
    carrier: LinearCarrier,
    tolerance: Tolerance,
) -> Result<LinearCarrier, IntersectError> {
    if tolerance.construction_length_zero(carrier.direction.norm()) {
        Err(IntersectError::Degenerate {
            object: carrier.kind,
        })
    } else {
        Ok(carrier)
    }
}

fn collinear_linear_intersection(
    first: LinearCarrier,
    second: LinearCarrier,
    tolerance: Tolerance,
) -> Result<Vec<Point>, IntersectError> {
    let first_interval = mapped_parameter_interval(first, first);
    let second_interval = mapped_parameter_interval(second, first);
    let lower = first_interval.lower.max(second_interval.lower);
    let upper = first_interval.upper.min(second_interval.upper);
    let finite_bounds = lower.is_finite() && upper.is_finite();
    let separation = (upper - lower).abs() * first.direction.norm();
    if finite_bounds && tolerance.length_zero(separation) {
        let parameter = lower + (upper - lower) * 0.5;
        finite_points(vec![first.origin + first.direction * parameter])
    } else if lower < upper {
        Err(IntersectError::Coincident)
    } else {
        Err(IntersectError::NoIntersection)
    }
}

fn mapped_parameter_interval(
    carrier: LinearCarrier,
    reference: LinearCarrier,
) -> ParameterInterval {
    let reference_norm_squared = reference.direction.norm_squared();
    let origin =
        (carrier.origin - reference.origin).dot(reference.direction) / reference_norm_squared;
    let step = carrier.direction.dot(reference.direction) / reference_norm_squared;
    match carrier.domain {
        ParameterDomain::Line => ParameterInterval {
            lower: f64::NEG_INFINITY,
            upper: f64::INFINITY,
        },
        ParameterDomain::Ray if step >= 0.0 => ParameterInterval {
            lower: origin,
            upper: f64::INFINITY,
        },
        ParameterDomain::Ray => ParameterInterval {
            lower: f64::NEG_INFINITY,
            upper: origin,
        },
        ParameterDomain::Segment => {
            let end = origin + step;
            ParameterInterval {
                lower: origin.min(end),
                upper: origin.max(end),
            }
        }
    }
}

fn parameter_is_in_domain(parameter: f64, domain: ParameterDomain, tolerance: Tolerance) -> bool {
    match domain {
        ParameterDomain::Line => true,
        ParameterDomain::Ray => tolerance.parameter_nonnegative(parameter),
        ParameterDomain::Segment => tolerance.parameter_in_closed_interval(parameter, 0.0, 1.0),
    }
}

fn parameter_on_carrier(point: Point, carrier: LinearCarrier) -> f64 {
    (point - carrier.origin).dot(carrier.direction) / carrier.direction.norm_squared()
}

fn sort_by_circle_angle(points: &mut [Point], circle: Circle) {
    points.sort_by(|lhs, rhs| circle_angle(circle, *lhs).total_cmp(&circle_angle(circle, *rhs)));
}

fn arc_contains(arc: Arc, point: Point, tolerance: Tolerance) -> bool {
    arc_contains_angle(arc, circle_angle(arc.circle, point), tolerance)
}

fn arc_contains_strict(arc: Arc, point: Point, tolerance: Tolerance) -> bool {
    arc_contains(arc, point, tolerance)
        && !tolerance.points_eq(point, arc.start)
        && !tolerance.points_eq(point, arc.end)
}

fn filter_to_arc(
    points: Vec<Point>,
    arc: Arc,
    tolerance: Tolerance,
) -> Result<Vec<Point>, IntersectError> {
    let filtered: Vec<_> = points
        .into_iter()
        .filter(|point| arc_contains(arc, *point, tolerance))
        .collect();
    if filtered.is_empty() {
        Err(IntersectError::NoIntersection)
    } else {
        Ok(filtered)
    }
}

fn circles_coincident(lhs: Circle, rhs: Circle, tolerance: Tolerance) -> bool {
    tolerance.points_eq(lhs.center, rhs.center) && tolerance.length_eq(lhs.radius, rhs.radius)
}

fn same_circle_arcs(
    lhs: Arc,
    rhs: Arc,
    tolerance: Tolerance,
) -> Result<Vec<Point>, IntersectError> {
    let lhs_endpoint_inside = arc_contains_strict(rhs, lhs.start, tolerance)
        || arc_contains_strict(rhs, lhs.end, tolerance);
    let rhs_endpoint_inside = arc_contains_strict(lhs, rhs.start, tolerance)
        || arc_contains_strict(lhs, rhs.end, tolerance);
    let interior_overlap = arc_contains_strict(rhs, arc_midpoint(lhs), tolerance)
        || arc_contains_strict(lhs, arc_midpoint(rhs), tolerance);
    if lhs_endpoint_inside || rhs_endpoint_inside || interior_overlap {
        return Err(IntersectError::Coincident);
    }
    let mut points = Vec::new();
    for point in [lhs.start, lhs.end] {
        if arc_contains(rhs, point, tolerance)
            && !points
                .iter()
                .any(|existing| tolerance.points_eq(*existing, point))
        {
            points.push(point);
        }
    }
    if points.is_empty() {
        Err(IntersectError::NoIntersection)
    } else {
        sort_by_circle_angle(&mut points, lhs.circle);
        Ok(points)
    }
}

fn arc_midpoint(arc: Arc) -> Point {
    let start = circle_angle(arc.circle, arc.start);
    let end = circle_angle(arc.circle, arc.end);
    let sweep = if arc.ccw {
        normalize_angle(end - start)
    } else {
        -normalize_angle(start - end)
    };
    let angle = start + sweep * 0.5;
    arc.circle.center + Vec2::new(libm::cos(angle), libm::sin(angle)) * arc.circle.radius
}

fn finite_points(points: Vec<Point>) -> Result<Vec<Point>, IntersectError> {
    if points.iter().all(|point| point.is_finite()) {
        Ok(points)
    } else {
        Err(IntersectError::NonFinite)
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::float_cmp)]
mod tests {
    use super::*;
    use crate::geom::{BBox, construct};

    fn tolerance() -> Tolerance {
        let bbox =
            BBox::from_points([Point::new(-10.0, -10.0), Point::new(10.0, 10.0)]).expect("bbox");
        Tolerance::from_bbox(&bbox)
    }

    fn line() -> Line {
        Line::new(Point::new(-3.0, 0.0), Point::new(3.0, 0.0)).expect("line")
    }

    fn ray() -> Ray {
        Ray::new(Point::new(-3.0, 0.0), Point::new(3.0, 0.0)).expect("ray")
    }

    fn segment() -> Segment {
        Segment::new(Point::new(-2.0, 0.0), Point::new(2.0, 0.0)).expect("segment")
    }

    fn circle() -> Circle {
        Circle::new(Point::new(0.0, 0.0), 1.0).expect("circle")
    }

    fn assert_linear_order(points: &[Point]) {
        assert_eq!(points, &[Point::new(-1.0, 0.0), Point::new(1.0, 0.0)]);
    }

    fn assert_circle_order(points: &[Point]) {
        assert_eq!(points, &[Point::new(1.0, 0.0), Point::new(-1.0, 0.0)]);
    }

    #[test]
    fn every_linear_linear_pair_has_the_unique_domain_filtered_root() {
        let tol = tolerance();
        let crossing_line = Line::new(Point::new(0.0, -2.0), Point::new(0.0, 2.0)).expect("line");
        let crossing_ray = Ray::new(Point::new(0.0, -2.0), Point::new(0.0, 2.0)).expect("ray");
        let crossing_segment =
            Segment::new(Point::new(0.0, -2.0), Point::new(0.0, 2.0)).expect("segment");
        assert_eq!(
            line()
                .intersections(&crossing_line, tol)
                .expect("line/line"),
            vec![Point::new(0.0, 0.0)]
        );
        assert_eq!(
            line().intersections(&crossing_ray, tol).expect("line/ray"),
            vec![Point::new(0.0, 0.0)]
        );
        assert_eq!(
            line()
                .intersections(&crossing_segment, tol)
                .expect("line/segment"),
            vec![Point::new(0.0, 0.0)]
        );
        assert_eq!(
            ray().intersections(&crossing_line, tol).expect("ray/line"),
            vec![Point::new(0.0, 0.0)]
        );
        assert_eq!(
            ray().intersections(&crossing_ray, tol).expect("ray/ray"),
            vec![Point::new(0.0, 0.0)]
        );
        assert_eq!(
            ray()
                .intersections(&crossing_segment, tol)
                .expect("ray/segment"),
            vec![Point::new(0.0, 0.0)]
        );
        assert_eq!(
            segment()
                .intersections(&crossing_line, tol)
                .expect("segment/line"),
            vec![Point::new(0.0, 0.0)]
        );
        assert_eq!(
            segment()
                .intersections(&crossing_ray, tol)
                .expect("segment/ray"),
            vec![Point::new(0.0, 0.0)]
        );
        assert_eq!(
            segment()
                .intersections(&crossing_segment, tol)
                .expect("segment/segment"),
            vec![Point::new(0.0, 0.0)]
        );
    }

    #[test]
    fn short_defining_vectors_do_not_change_line_parallelism() {
        let bbox = BBox::from_points([Point::new(0.0, 0.0), Point::new(100.0, 0.0)]).expect("bbox");
        let tol = Tolerance::from_bbox(&bbox);
        let horizontal = Line::new(Point::new(0.0, 0.0), Point::new(1.0e-6, 0.0)).expect("line");
        let vertical =
            Line::new(Point::new(50.0, -50.0), Point::new(50.0, -50.0 + 1.0e-6)).expect("line");
        let point = intersect(&horizontal, &vertical, None, tol).expect("unique crossing");
        assert!(tol.points_eq(point, Point::new(50.0, 0.0)));
    }

    #[test]
    fn collinear_linear_objects_respect_finite_and_half_line_domains() {
        let tol = tolerance();
        let first = Segment::new(Point::new(0.0, 0.0), Point::new(1.0, 0.0)).expect("segment");
        let touching = Segment::new(Point::new(1.0, 0.0), Point::new(2.0, 0.0)).expect("segment");
        let disjoint = Segment::new(Point::new(2.0, 0.0), Point::new(3.0, 0.0)).expect("segment");
        let overlapping =
            Segment::new(Point::new(0.5, 0.0), Point::new(1.5, 0.0)).expect("segment");
        assert_eq!(
            first.intersections(&touching, tol),
            Ok(vec![Point::new(1.0, 0.0)])
        );
        assert_eq!(
            first.intersections(&disjoint, tol),
            Err(IntersectError::NoIntersection)
        );
        assert_eq!(
            first.intersections(&overlapping, tol),
            Err(IntersectError::Coincident)
        );

        let ray = Ray::new(Point::new(0.0, 0.0), Point::new(1.0, 0.0)).expect("ray");
        let behind = Segment::new(Point::new(-2.0, 0.0), Point::new(-1.0, 0.0)).expect("segment");
        let ray_touch = Segment::new(Point::new(-1.0, 0.0), Point::new(0.0, 0.0)).expect("segment");
        assert_eq!(
            ray.intersections(&behind, tol),
            Err(IntersectError::NoIntersection)
        );
        assert_eq!(
            ray.intersections(&ray_touch, tol),
            Ok(vec![Point::new(0.0, 0.0)])
        );
    }

    #[test]
    fn line_circle_roots_follow_first_operand_parameter_or_angle() {
        let tol = tolerance();
        assert_linear_order(&line().intersections(&circle(), tol).expect("line/circle"));
        assert_circle_order(&circle().intersections(&line(), tol).expect("circle/line"));
    }

    #[test]
    fn ray_circle_roots_follow_first_operand_parameter_or_angle() {
        let tol = tolerance();
        assert_linear_order(&ray().intersections(&circle(), tol).expect("ray/circle"));
        assert_circle_order(&circle().intersections(&ray(), tol).expect("circle/ray"));
    }

    #[test]
    fn segment_circle_roots_follow_first_operand_parameter_or_angle() {
        let tol = tolerance();
        assert_linear_order(
            &segment()
                .intersections(&circle(), tol)
                .expect("segment/circle"),
        );
        assert_circle_order(
            &circle()
                .intersections(&segment(), tol)
                .expect("circle/segment"),
        );
    }

    #[test]
    fn circle_circle_roots_are_sorted_by_lhs_ccw_angle() {
        let tol = tolerance();
        let lhs = Circle::new(Point::new(0.0, 0.0), 2.0).expect("circle");
        let rhs = Circle::new(Point::new(1.0, 0.0), 2.0).expect("circle");
        let points = lhs.intersections(&rhs, tol).expect("circle/circle");
        assert!(points[0].y > 0.0);
        assert!(points[1].y < 0.0);
        assert!(circle_angle(lhs, points[0]) < circle_angle(lhs, points[1]));
    }

    #[test]
    fn small_circle_secant_roots_are_not_collapsed_by_area_tolerance() {
        let bbox = BBox::from_points([Point::new(0.0, 0.0), Point::new(1.0, 0.0)]).expect("bbox");
        let tol = Tolerance::from_bbox(&bbox);
        let lhs = Circle::new(Point::new(0.0, 0.0), 1.0e-5).expect("circle");
        let rhs = Circle::new(Point::new(1.0e-5, 0.0), 1.0e-5).expect("circle");
        let points = lhs.intersections(&rhs, tol).expect("two intersections");
        assert_eq!(points.len(), 2);
        assert!(points[0].y > 0.0);
        assert!(points[1].y < 0.0);
        for point in points {
            assert!(tol.length_eq((point - lhs.center).norm(), lhs.radius));
            assert!(tol.length_eq((point - rhs.center).norm(), rhs.radius));
        }
    }

    #[test]
    fn circle_secant_height_is_stable_at_extreme_representable_scales() {
        for scale in [1.0e-100, 1.0e80] {
            let lhs = Circle::new(Point::new(0.0, 0.0), scale).expect("circle");
            let rhs = Circle::new(Point::new(scale, 0.0), scale).expect("circle");
            let bbox = BBox::from_points([lhs.center, rhs.center]).expect("bbox");
            let tol = Tolerance::from_bbox(&bbox);
            let points = lhs.intersections(&rhs, tol).expect("two intersections");
            assert_eq!(points.len(), 2);
            assert!(!tol.points_eq(points[0], points[1]));
            for point in points {
                assert!(tol.length_eq((point - lhs.center).norm(), lhs.radius));
                assert!(tol.length_eq((point - rhs.center).norm(), rhs.radius));
            }
        }
    }

    #[test]
    fn arc_pairs_filter_supporting_circle_roots_then_apply_o1_order() {
        let tol = tolerance();
        let support = circle();
        let arc = construct::arc(
            support,
            construct::on_circle(support, 5.0 * core::f64::consts::PI / 3.0).expect("start"),
            construct::on_circle(support, 4.0 * core::f64::consts::PI / 3.0).expect("end"),
            true,
            tol,
        )
        .expect("arc");
        assert_linear_order(&line().intersections(&arc, tol).expect("line/arc"));
        assert_circle_order(&arc.intersections(&line(), tol).expect("arc/line"));
        assert_linear_order(&ray().intersections(&arc, tol).expect("ray/arc"));
        assert_circle_order(&arc.intersections(&ray(), tol).expect("arc/ray"));
        assert_linear_order(&segment().intersections(&arc, tol).expect("segment/arc"));
        assert_circle_order(&arc.intersections(&segment(), tol).expect("arc/segment"));

        let other_circle = Circle::new(Point::new(1.0, 0.0), 1.0).expect("circle");
        let other_arc = construct::arc(
            other_circle,
            construct::on_circle(other_circle, core::f64::consts::FRAC_PI_2).expect("start"),
            construct::on_circle(other_circle, 3.0 * core::f64::consts::FRAC_PI_2).expect("end"),
            true,
            tol,
        )
        .expect("arc");
        assert!(arc.intersections(&other_arc, tol).is_ok());
        assert!(support.intersections(&other_arc, tol).is_ok());
        assert!(arc.intersections(&other_circle, tol).is_ok());
    }

    #[test]
    fn explicit_branch_is_required_only_for_multiple_roots() {
        let tol = tolerance();
        assert_eq!(
            intersect(&line(), &circle(), None, tol),
            Err(IntersectError::Ambiguous { count: 2 })
        );
        assert_eq!(
            intersect(&line(), &circle(), Some(0), tol),
            Ok(Point::new(-1.0, 0.0))
        );
        assert_eq!(
            intersect(&line(), &circle(), Some(1), tol),
            Ok(Point::new(1.0, 0.0))
        );
    }

    #[test]
    fn tangent_empty_coincident_and_degenerate_cases_are_typed() {
        let tol = tolerance();
        let tangent = Line::new(Point::new(-2.0, 1.0), Point::new(2.0, 1.0)).expect("line");
        assert_eq!(
            tangent
                .intersections(&circle(), tol)
                .expect("tangent")
                .len(),
            1
        );
        let far = Line::new(Point::new(-2.0, 2.0), Point::new(2.0, 2.0)).expect("line");
        assert_eq!(
            far.intersections(&circle(), tol),
            Err(IntersectError::NoIntersection)
        );
        assert_eq!(
            line().intersections(&line(), tol),
            Err(IntersectError::Coincident)
        );
        let point_segment =
            Segment::new(Point::new(0.0, 0.0), Point::new(0.0, 0.0)).expect("stored");
        let error = point_segment
            .intersections(&circle(), tol)
            .expect_err("degenerate");
        assert_eq!(error.code(), "E203");

        let backwards = Ray::new(Point::new(3.0, 0.0), Point::new(4.0, 0.0)).expect("ray");
        assert_eq!(
            backwards.intersections(&circle(), tol),
            Err(IntersectError::NoIntersection)
        );
    }

    #[test]
    fn identical_arcs_are_coincident_not_two_endpoint_roots() {
        let tol = tolerance();
        let support = circle();
        let arc = construct::arc(
            support,
            Point::new(1.0, 0.0),
            Point::new(0.0, 1.0),
            true,
            tol,
        )
        .expect("arc");
        assert_eq!(
            arc.intersections(&arc, tol),
            Err(IntersectError::Coincident)
        );
    }
}
