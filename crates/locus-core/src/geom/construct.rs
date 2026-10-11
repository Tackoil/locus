//! Pure derived-object constructors from specification sections 5.1 through 5.5.

use super::measure::{dist, normalize_angle, signed_area_vertices};
use super::{
    Arc, Circle, ConstructError, Line, Point, Polygon, Ray, Segment, Vec2, validate_finite_points,
    validate_finite_result,
};
use crate::tol::Tolerance;

/// Midpoint of two points.
pub fn midpoint(a: Point, b: Point) -> Result<Point, ConstructError> {
    validate_finite_points("midpoint", &[a, b])?;
    validate_finite_result(
        "midpoint",
        Point::new(a.x + (b.x - a.x) * 0.5, a.y + (b.y - a.y) * 0.5),
    )
}

/// Affine interpolation; `t` may lie outside `[0, 1]`.
pub fn lerp(a: Point, b: Point, t: f64) -> Result<Point, ConstructError> {
    validate_finite_points("lerp", &[a, b])?;
    validate_finite_scalar("lerp", t)?;
    validate_finite_result("lerp", a + (b - a) * t)
}

/// Objects that have a directed linear carrier and a parameter domain.
pub trait FootTarget: Copy {
    /// Carrier parameter of the orthogonal projection, or a domain error.
    fn projected_point(self, point: Point, tolerance: Tolerance) -> Result<Point, ConstructError>;

    /// Closest point on the target's closed parameter domain.
    fn closest_point(self, point: Point, tolerance: Tolerance) -> Result<Point, ConstructError>;
}

/// Perpendicular foot on a line, ray, or segment.
pub fn foot<T: FootTarget>(
    point: Point,
    target: T,
    tolerance: Tolerance,
) -> Result<Point, ConstructError> {
    validate_finite_points("foot", &[point])?;
    target.projected_point(point, tolerance)
}

/// Closest projection on a line, closed ray, or closed segment.
pub fn closest_foot<T: FootTarget>(
    point: Point,
    target: T,
    tolerance: Tolerance,
) -> Result<Point, ConstructError> {
    validate_finite_points("foot", &[point])?;
    target.closest_point(point, tolerance)
}

impl FootTarget for Line {
    fn projected_point(self, point: Point, tolerance: Tolerance) -> Result<Point, ConstructError> {
        project_to_carrier(point, self.start, self.direction(), tolerance).map(|(result, _)| result)
    }

    fn closest_point(self, point: Point, tolerance: Tolerance) -> Result<Point, ConstructError> {
        self.projected_point(point, tolerance)
    }
}

impl FootTarget for Ray {
    fn projected_point(self, point: Point, tolerance: Tolerance) -> Result<Point, ConstructError> {
        let (result, parameter) =
            project_to_carrier(point, self.start, self.direction(), tolerance)?;
        if tolerance.parameter_nonnegative(parameter) {
            Ok(result)
        } else {
            Err(ConstructError::Domain {
                operation: "foot",
                reason: "perpendicular foot lies behind the ray",
            })
        }
    }

    fn closest_point(self, point: Point, tolerance: Tolerance) -> Result<Point, ConstructError> {
        let (_, parameter) = project_to_carrier(point, self.start, self.direction(), tolerance)?;
        Ok(self.start + self.direction() * parameter.max(0.0))
    }
}

impl FootTarget for Segment {
    fn projected_point(self, point: Point, tolerance: Tolerance) -> Result<Point, ConstructError> {
        let (result, parameter) =
            project_to_carrier(point, self.start, self.direction(), tolerance)?;
        if tolerance.parameter_in_closed_interval(parameter, 0.0, 1.0) {
            Ok(result)
        } else {
            Err(ConstructError::Domain {
                operation: "foot",
                reason: "perpendicular foot lies outside the segment",
            })
        }
    }

    fn closest_point(self, point: Point, tolerance: Tolerance) -> Result<Point, ConstructError> {
        let (_, parameter) = project_to_carrier(point, self.start, self.direction(), tolerance)?;
        Ok(self.start + self.direction() * parameter.clamp(0.0, 1.0))
    }
}

/// Nearest point on a circle to an external or internal point.
pub fn project(
    point: Point,
    circle: Circle,
    tolerance: Tolerance,
) -> Result<Point, ConstructError> {
    validate_finite_points("project", &[point, circle.center])?;
    let radial = point - circle.center;
    let radial_length = radial.norm();
    if tolerance.construction_length_zero(radial_length) {
        return Err(ConstructError::Degenerate {
            operation: "project",
            reason: "circle center has no unique nearest point",
        });
    }
    validate_finite_result(
        "project",
        circle.center + radial * (circle.radius / radial_length),
    )
}

/// Rotates a point CCW about an origin by an angle in radians.
pub fn rotate(point: Point, origin: Point, angle: f64) -> Result<Point, ConstructError> {
    validate_finite_points("rotate", &[point, origin])?;
    validate_finite_scalar("rotate", angle)?;
    let relative = point - origin;
    let cosine = libm::cos(angle);
    let sine = libm::sin(angle);
    validate_finite_result(
        "rotate",
        origin
            + Vec2::new(
                relative.x * cosine - relative.y * sine,
                relative.x * sine + relative.y * cosine,
            ),
    )
}

/// Reflects a point over a line.
pub fn reflect_over_line(
    point: Point,
    axis: Line,
    tolerance: Tolerance,
) -> Result<Point, ConstructError> {
    let projection = foot(point, axis, tolerance)?;
    validate_finite_result("reflect", projection + (projection - point))
}

/// Reflects a point through another point.
pub fn reflect_over_point(point: Point, center: Point) -> Result<Point, ConstructError> {
    validate_finite_points("reflect", &[point, center])?;
    validate_finite_result("reflect", center + (center - point))
}

/// Translates by the vector `from -> to`.
pub fn translate(point: Point, from: Point, to: Point) -> Result<Point, ConstructError> {
    validate_finite_points("translate", &[point, from, to])?;
    validate_finite_result("translate", point + (to - from))
}

/// Point on a circle at a CCW angle from +x.
pub fn on_circle(circle: Circle, angle: f64) -> Result<Point, ConstructError> {
    validate_finite_scalar("on_circle", angle)?;
    validate_finite_result(
        "on_circle",
        circle.center + Vec2::new(libm::cos(angle), libm::sin(angle)) * circle.radius,
    )
}

/// Extends `A -> B` by signed distance `d`, retaining the ray domain.
pub fn extend(
    a: Point,
    b: Point,
    extension: f64,
    tolerance: Tolerance,
) -> Result<Point, ConstructError> {
    validate_finite_points("extend", &[a, b])?;
    validate_finite_scalar("extend", extension)?;
    let direction = b - a;
    let original_length = direction.norm();
    if tolerance.construction_length_zero(original_length) {
        return Err(ConstructError::Degenerate {
            operation: "extend",
            reason: "defining points coincide",
        });
    }
    let extended_length = original_length + extension;
    if extended_length < 0.0 && !tolerance.construction_length_zero(extended_length) {
        return Err(ConstructError::Domain {
            operation: "extend",
            reason: "result would lie behind the ray origin",
        });
    }
    validate_finite_result(
        "extend",
        a + direction * (extended_length.max(0.0) / original_length),
    )
}

/// Arithmetic centroid of one or more points.
pub fn centroid(points: &[Point]) -> Result<Point, ConstructError> {
    if points.is_empty() {
        return Err(ConstructError::Domain {
            operation: "centroid",
            reason: "at least one point is required",
        });
    }
    validate_finite_points("centroid", points)?;
    let sum = points.iter().fold(Vec2::new(0.0, 0.0), |acc, point| {
        acc + (*point - Point::new(0.0, 0.0))
    });
    #[allow(clippy::cast_precision_loss)]
    let count = points.len() as f64;
    validate_finite_result("centroid", Point::new(0.0, 0.0) + sum / count)
}

/// Circumcenter of a non-collinear triangle.
pub fn circumcenter(
    a: Point,
    b: Point,
    c: Point,
    tolerance: Tolerance,
) -> Result<Point, ConstructError> {
    validate_triangle("circumcenter", a, b, c, tolerance)?;
    let ab = b - a;
    let ac = c - a;
    let denominator = 2.0 * ab.cross(ac);
    let base_squared = ab.norm_squared();
    let side_squared = ac.norm_squared();
    let local_center = Vec2::new(
        (ac.y * base_squared - ab.y * side_squared) / denominator,
        (ab.x * side_squared - ac.x * base_squared) / denominator,
    );
    validate_finite_result("circumcenter", a + local_center)
}

/// Incenter of a non-collinear triangle.
pub fn incenter(
    a: Point,
    b: Point,
    c: Point,
    tolerance: Tolerance,
) -> Result<Point, ConstructError> {
    validate_triangle("incenter", a, b, c, tolerance)?;
    let weight_a = dist(b, c);
    let weight_b = dist(c, a);
    let weight_c = dist(a, b);
    let total = weight_a + weight_b + weight_c;
    validate_finite_result(
        "incenter",
        Point::new(
            (weight_a * a.x + weight_b * b.x + weight_c * c.x) / total,
            (weight_a * a.y + weight_b * b.y + weight_c * c.y) / total,
        ),
    )
}

/// Orthocenter of a non-collinear triangle.
pub fn orthocenter(
    a: Point,
    b: Point,
    c: Point,
    tolerance: Tolerance,
) -> Result<Point, ConstructError> {
    let center = circumcenter(a, b, c, tolerance)?;
    let local_center = center - a;
    let local_orthocenter = (b - a) + (c - a) - local_center * 2.0;
    validate_finite_result("orthocenter", a + local_orthocenter)
}

/// Line through a point perpendicular to another line.
pub fn perpendicular(point: Point, line: Line) -> Result<Line, ConstructError> {
    validate_finite_points("perpendicular", &[point])?;
    Line::new(point, point + line.direction().perpendicular())
}

/// Line through a point parallel to another line.
pub fn parallel(point: Point, line: Line) -> Result<Line, ConstructError> {
    validate_finite_points("parallel", &[point])?;
    Line::new(point, point + line.direction())
}

/// Perpendicular bisector of a nonzero segment `AB`.
pub fn perp_bisector(a: Point, b: Point, tolerance: Tolerance) -> Result<Line, ConstructError> {
    let segment = Segment::new(a, b)?;
    if tolerance.construction_length_zero(segment.direction().norm()) {
        return Err(ConstructError::Degenerate {
            operation: "perp_bisector",
            reason: "defining points coincide",
        });
    }
    perpendicular(segment.midpoint(), Line::new(a, b)?)
}

/// Interior angle bisector at `B` for nonzero, non-collinear rays `B -> A`, `B -> C`.
pub fn angle_bisector(
    a: Point,
    b: Point,
    c: Point,
    tolerance: Tolerance,
) -> Result<Line, ConstructError> {
    let first_ray = a - b;
    let second_ray = c - b;
    let ba = unit_direction("angle_bisector", b, a, tolerance)?;
    let bc = unit_direction("angle_bisector", b, c, tolerance)?;
    if tolerance.construction_directions_parallel(ba, bc) {
        return Err(ConstructError::Degenerate {
            operation: "angle_bisector",
            reason: "collinear rays have no interior angle",
        });
    }
    let bisector = ba + bc;
    // `ba` and `bc` are unit vectors, so their sum is dimensionless and must
    // not inherit the scene diameter's length scale.
    if tolerance.construction_scalar_zero(bisector.norm()) {
        return Err(ConstructError::Degenerate {
            operation: "angle_bisector",
            reason: "straight angle has no unique interior bisector",
        });
    }
    // Normalize the dimensionless sum, then give the carrier the longer input
    // ray's length. This is large enough for downstream relative checks and is
    // independent of bounds accumulated from previously constructed objects.
    let bisector_length = bisector.norm();
    let carrier_length = first_ray.norm().max(second_ray.norm());
    Line::new(b, b + bisector * (carrier_length / bisector_length))
}

/// One of two tangent lines from a strictly external point.
///
/// Contact points are sorted by their normalized CCW angle from +x. Thus `k=0`
/// is the smaller contact angle and `k=1` is the larger, exactly as spec 5.3.
pub fn tangent_line(
    point: Point,
    circle: Circle,
    branch: usize,
    tolerance: Tolerance,
) -> Result<Line, ConstructError> {
    if branch > 1 {
        return Err(ConstructError::Domain {
            operation: "tangent_line",
            reason: "branch must be 0 or 1",
        });
    }
    let from_center = point - circle.center;
    let center_distance = from_center.norm();
    if center_distance < circle.radius
        || tolerance.construction_length_eq(center_distance, circle.radius)
    {
        return Err(ConstructError::Domain {
            operation: "tangent_line",
            reason: "point must lie strictly outside the circle",
        });
    }
    let base = libm::atan2(from_center.y, from_center.x);
    let offset = libm::acos(circle.radius / center_distance);
    let mut contacts = [
        on_circle(circle, base - offset)?,
        on_circle(circle, base + offset)?,
    ];
    contacts
        .sort_by(|lhs, rhs| contact_angle(circle, *lhs).total_cmp(&contact_angle(circle, *rhs)));
    Line::new(point, contacts[branch])
}

/// Circle centered at `center` and passing through `point`.
pub fn circle_through(
    center: Point,
    point: Point,
    tolerance: Tolerance,
) -> Result<Circle, ConstructError> {
    let radius = dist(center, point);
    if tolerance.construction_length_zero(radius) {
        return Err(ConstructError::Degenerate {
            operation: "circle_through",
            reason: "center and circumference point coincide",
        });
    }
    Circle::new(center, radius)
}

/// Circle whose diameter endpoints are `A` and `B`.
pub fn circle_diameter(a: Point, b: Point, tolerance: Tolerance) -> Result<Circle, ConstructError> {
    let radius = dist(a, b) * 0.5;
    if tolerance.construction_length_zero(radius) {
        return Err(ConstructError::Degenerate {
            operation: "circle_diameter",
            reason: "diameter endpoints coincide",
        });
    }
    Circle::new(midpoint(a, b)?, radius)
}

/// Circumcircle of a non-collinear triangle.
pub fn circumcircle(
    a: Point,
    b: Point,
    c: Point,
    tolerance: Tolerance,
) -> Result<Circle, ConstructError> {
    let center = circumcenter(a, b, c, tolerance)?;
    Circle::new(center, dist(center, a))
}

/// Incircle of a non-collinear triangle.
pub fn incircle(
    a: Point,
    b: Point,
    c: Point,
    tolerance: Tolerance,
) -> Result<Circle, ConstructError> {
    let center = incenter(a, b, c, tolerance)?;
    let side = b - a;
    let radius = (center - a).cross(side).abs() / side.norm();
    Circle::new(center, radius)
}

/// Directed arc with endpoint incidence checked at the construction threshold.
pub fn arc(
    circle: Circle,
    start: Point,
    end: Point,
    ccw: bool,
    tolerance: Tolerance,
) -> Result<Arc, ConstructError> {
    validate_finite_points("arc", &[start, end])?;
    if !tolerance.construction_length_eq(dist(circle.center, start), circle.radius)
        || !tolerance.construction_length_eq(dist(circle.center, end), circle.radius)
    {
        return Err(ConstructError::ArcEndpointNotOnCircle);
    }
    if tolerance.construction_points_eq(start, end) {
        return Err(ConstructError::Degenerate {
            operation: "arc",
            reason: "arc endpoints coincide; use a circle for a full sweep",
        });
    }
    Ok(Arc {
        circle,
        start,
        end,
        ccw,
    })
}

/// Angle-mark arc at vertex `B` from ray `B -> A` to ray `B -> C`.
pub fn angle_arc(
    a: Point,
    b: Point,
    c: Point,
    radius: f64,
    tolerance: Tolerance,
) -> Result<Arc, ConstructError> {
    if !radius.is_finite() || radius <= 0.0 {
        return Err(ConstructError::Domain {
            operation: "angle_arc",
            reason: "radius must be finite and strictly positive",
        });
    }
    let start_direction = unit_direction("angle_arc", b, a, tolerance)?;
    let end_direction = unit_direction("angle_arc", b, c, tolerance)?;
    if tolerance.construction_directions_parallel(start_direction, end_direction) {
        return Err(ConstructError::Degenerate {
            operation: "angle_arc",
            reason: "collinear rays do not define an angle-mark branch",
        });
    }
    let circle = Circle::new(b, radius)?;
    let start = b + start_direction * radius;
    let end = b + end_direction * radius;
    let ccw = start_direction.cross(end_direction) >= 0.0;
    arc(circle, start, end, ccw, tolerance)
}

/// Constructs and validates a simple polygon.
pub fn polygon(vertices: Vec<Point>, tolerance: Tolerance) -> Result<Polygon, ConstructError> {
    validate_polygon_vertices(&vertices, tolerance)?;
    Ok(Polygon::from_validated_vertices(vertices))
}

/// Axis-aligned rectangle, returned as a counterclockwise polygon.
pub fn rect(
    center: Point,
    width: f64,
    height: f64,
    tolerance: Tolerance,
) -> Result<Polygon, ConstructError> {
    validate_positive_dimensions("rect", center, width, height)?;
    let half_width = width * 0.5;
    let half_height = height * 0.5;
    polygon(
        vec![
            Point::new(center.x - half_width, center.y - half_height),
            Point::new(center.x + half_width, center.y - half_height),
            Point::new(center.x + half_width, center.y + half_height),
            Point::new(center.x - half_width, center.y + half_height),
        ],
        tolerance,
    )
}

/// Axis-aligned rhombus, returned as a counterclockwise polygon.
pub fn diamond(
    center: Point,
    width: f64,
    height: f64,
    tolerance: Tolerance,
) -> Result<Polygon, ConstructError> {
    validate_positive_dimensions("diamond", center, width, height)?;
    polygon(
        vec![
            Point::new(center.x, center.y - height * 0.5),
            Point::new(center.x + width * 0.5, center.y),
            Point::new(center.x, center.y + height * 0.5),
            Point::new(center.x - width * 0.5, center.y),
        ],
        tolerance,
    )
}

/// Area centroid anchor of a validated polygon.
pub fn polygon_centroid(polygon: &Polygon) -> Result<Point, ConstructError> {
    let vertices = polygon.vertices();
    let origin = vertices[0];
    let mut weighted_x = 0.0;
    let mut weighted_y = 0.0;
    let mut twice_area = 0.0;
    for (start, end) in vertices
        .iter()
        .copied()
        .zip(vertices.iter().copied().cycle().skip(1))
        .take(vertices.len())
    {
        let local_start = start - origin;
        let local_end = end - origin;
        let cross = local_start.cross(local_end);
        twice_area += cross;
        weighted_x += (local_start.x + local_end.x) * cross;
        weighted_y += (local_start.y + local_end.y) * cross;
    }
    let denominator = 3.0 * twice_area;
    validate_finite_result(
        "polygon.centroid",
        origin + Vec2::new(weighted_x / denominator, weighted_y / denominator),
    )
}

fn project_to_carrier(
    point: Point,
    origin: Point,
    direction: Vec2,
    tolerance: Tolerance,
) -> Result<(Point, f64), ConstructError> {
    if tolerance.construction_length_zero(direction.norm()) {
        return Err(ConstructError::Degenerate {
            operation: "foot",
            reason: "target has no direction",
        });
    }
    let parameter = (point - origin).dot(direction) / direction.norm_squared();
    Ok((
        validate_finite_result("foot", origin + direction * parameter)?,
        parameter,
    ))
}

fn unit_direction(
    operation: &'static str,
    from: Point,
    to: Point,
    tolerance: Tolerance,
) -> Result<Vec2, ConstructError> {
    let direction = to - from;
    let length = direction.norm();
    if tolerance.construction_length_zero(length) {
        Err(ConstructError::Degenerate {
            operation,
            reason: "ray has zero length",
        })
    } else {
        Ok(direction / length)
    }
}

fn validate_triangle(
    operation: &'static str,
    a: Point,
    b: Point,
    c: Point,
    tolerance: Tolerance,
) -> Result<(), ConstructError> {
    validate_finite_points(operation, &[a, b, c])?;
    if tolerance.construction_area_zero((b - a).cross(c - a)) {
        Err(ConstructError::Degenerate {
            operation,
            reason: "triangle points are collinear",
        })
    } else {
        Ok(())
    }
}

fn contact_angle(circle: Circle, point: Point) -> f64 {
    normalize_angle(libm::atan2(
        point.y - circle.center.y,
        point.x - circle.center.x,
    ))
}

fn validate_finite_scalar(operation: &'static str, value: f64) -> Result<(), ConstructError> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(ConstructError::NonFinite { operation })
    }
}

fn validate_positive_dimensions(
    operation: &'static str,
    center: Point,
    width: f64,
    height: f64,
) -> Result<(), ConstructError> {
    validate_finite_points(operation, &[center])?;
    if width.is_finite() && height.is_finite() && width > 0.0 && height > 0.0 {
        Ok(())
    } else {
        Err(ConstructError::Domain {
            operation,
            reason: "width and height must be finite and strictly positive",
        })
    }
}

fn validate_polygon_vertices(
    vertices: &[Point],
    tolerance: Tolerance,
) -> Result<(), ConstructError> {
    if vertices.len() < 3 {
        return Err(ConstructError::InvalidPolygon {
            reason: "at least three vertices are required",
        });
    }
    validate_finite_points("polygon", vertices)?;
    if has_duplicate_vertices(vertices, tolerance) {
        return Err(ConstructError::InvalidPolygon {
            reason: "vertices must be distinct",
        });
    }
    if tolerance.construction_area_zero(signed_area_vertices(vertices)) {
        return Err(ConstructError::InvalidPolygon {
            reason: "signed area must be nonzero",
        });
    }
    if has_adjacent_collinear_overlap(vertices, tolerance) {
        return Err(ConstructError::InvalidPolygon {
            reason: "adjacent collinear edges overlap",
        });
    }
    if has_invalid_edge_intersection(vertices, tolerance) {
        return Err(ConstructError::InvalidPolygon {
            reason: "edges cross or overlap",
        });
    }
    Ok(())
}

fn has_duplicate_vertices(vertices: &[Point], tolerance: Tolerance) -> bool {
    vertices.iter().enumerate().any(|(index, point)| {
        vertices[index + 1..]
            .iter()
            .any(|other| tolerance.construction_points_eq(*point, *other))
    })
}

fn has_adjacent_collinear_overlap(vertices: &[Point], tolerance: Tolerance) -> bool {
    (0..vertices.len()).any(|index| {
        let previous = vertices[(index + vertices.len() - 1) % vertices.len()];
        let current = vertices[index];
        let next = vertices[(index + 1) % vertices.len()];
        let incoming = current - previous;
        let outgoing = next - current;
        tolerance.construction_directions_parallel(incoming, outgoing)
            && incoming.dot(outgoing) < 0.0
    })
}

fn has_invalid_edge_intersection(vertices: &[Point], tolerance: Tolerance) -> bool {
    let edge_count = vertices.len();
    for first in 0..edge_count {
        for second in first + 1..edge_count {
            if edges_are_adjacent(first, second, edge_count) {
                continue;
            }
            let a = vertices[first];
            let b = vertices[(first + 1) % edge_count];
            let c = vertices[second];
            let d = vertices[(second + 1) % edge_count];
            if closed_segments_intersect(a, b, c, d, tolerance) {
                return true;
            }
        }
    }
    false
}

fn edges_are_adjacent(first: usize, second: usize, edge_count: usize) -> bool {
    second == first + 1 || (first == 0 && second + 1 == edge_count)
}

fn closed_segments_intersect(a: Point, b: Point, c: Point, d: Point, tolerance: Tolerance) -> bool {
    let ab = b - a;
    let cd = d - c;
    let offset = c - a;
    if tolerance.construction_directions_parallel(ab, cd) {
        if !tolerance.construction_vector_on_direction(offset, ab) {
            return false;
        }
        return collinear_ranges_overlap(a, b, c, d, tolerance);
    }
    let denominator = ab.cross(cd);
    let first_parameter = offset.cross(cd) / denominator;
    let second_parameter = offset.cross(ab) / denominator;
    tolerance.construction_parameter_in_unit_interval(first_parameter)
        && tolerance.construction_parameter_in_unit_interval(second_parameter)
}

fn collinear_ranges_overlap(a: Point, b: Point, c: Point, d: Point, tolerance: Tolerance) -> bool {
    let direction = b - a;
    let denominator = direction.norm_squared();
    let first = (c - a).dot(direction) / denominator;
    let second = (d - a).dot(direction) / denominator;
    let lower = first.min(second);
    let upper = first.max(second);
    tolerance.construction_parameter_in_unit_interval(lower)
        || tolerance.construction_parameter_in_unit_interval(upper)
        || (lower <= 0.0 && upper >= 1.0)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::float_cmp)]
mod tests {
    use proptest::prelude::*;

    use super::*;
    use crate::geom::{BBox, measure};

    fn tolerance() -> Tolerance {
        let bbox = BBox::from_points([Point::new(-20.0, -20.0), Point::new(20.0, 20.0)])
            .expect("valid bbox");
        Tolerance::from_bbox(&bbox)
    }

    fn close(tol: Tolerance, lhs: Point, rhs: Point) {
        assert!(tol.points_eq(lhs, rhs), "{lhs:?} != {rhs:?}");
    }

    #[test]
    fn point_and_line_constructors_have_known_answers() {
        let tol = tolerance();
        assert_eq!(
            midpoint(Point::new(0.0, 2.0), Point::new(4.0, 6.0)),
            Ok(Point::new(2.0, 4.0))
        );
        assert_eq!(
            lerp(Point::new(0.0, 0.0), Point::new(2.0, 4.0), 1.5),
            Ok(Point::new(3.0, 6.0))
        );
        assert_eq!(
            translate(
                Point::new(1.0, 1.0),
                Point::new(2.0, 3.0),
                Point::new(5.0, 7.0)
            ),
            Ok(Point::new(4.0, 5.0))
        );
        assert_eq!(
            extend(Point::new(0.0, 0.0), Point::new(3.0, 0.0), 2.0, tol),
            Ok(Point::new(5.0, 0.0))
        );

        let axis = Line::new(Point::new(-1.0, 0.0), Point::new(1.0, 0.0)).expect("line");
        assert_eq!(
            foot(Point::new(2.0, 3.0), axis, tol),
            Ok(Point::new(2.0, 0.0))
        );
        assert_eq!(
            reflect_over_line(Point::new(2.0, 3.0), axis, tol),
            Ok(Point::new(2.0, -3.0))
        );
        close(
            tol,
            rotate(
                Point::new(2.0, 0.0),
                Point::new(0.0, 0.0),
                core::f64::consts::FRAC_PI_2,
            )
            .expect("rotate"),
            Point::new(0.0, 2.0),
        );
    }

    #[test]
    fn triangle_centers_and_circles_match_three_four_five_answers() {
        let tol = tolerance();
        let a = Point::new(0.0, 0.0);
        let b = Point::new(4.0, 0.0);
        let c = Point::new(0.0, 3.0);
        assert_eq!(incenter(a, b, c, tol), Ok(Point::new(1.0, 1.0)));
        assert_eq!(circumcenter(a, b, c, tol), Ok(Point::new(2.0, 1.5)));
        assert_eq!(orthocenter(a, b, c, tol), Ok(a));
        let incircle = incircle(a, b, c, tol).expect("incircle");
        assert_eq!(
            incircle,
            Circle::new(Point::new(1.0, 1.0), 1.0).expect("circle")
        );
        let circumcircle = circumcircle(a, b, c, tol).expect("circumcircle");
        assert_eq!(
            circumcircle,
            Circle::new(Point::new(2.0, 1.5), 2.5).expect("circle")
        );
    }

    #[test]
    fn translated_triangle_centers_use_local_coordinates() {
        let a = Point::new(1.0e8, 1.0e8);
        let b = Point::new(1.0e8 + 4.0, 1.0e8);
        let c = Point::new(1.0e8, 1.0e8 + 3.0);
        let bbox = BBox::from_points([a, b, c]).expect("bbox");
        let tol = Tolerance::from_bbox(&bbox);
        let expected_center = Point::new(1.0e8 + 2.0, 1.0e8 + 1.5);
        close(
            tol,
            circumcenter(a, b, c, tol).expect("circumcenter"),
            expected_center,
        );
        let circle = circumcircle(a, b, c, tol).expect("circumcircle");
        close(tol, circle.center, expected_center);
        assert!(tol.length_eq(circle.radius, 2.5));
        close(tol, orthocenter(a, b, c, tol).expect("orthocenter"), a);
    }

    #[test]
    fn translated_arc_endpoints_and_polygon_vertices_remain_distinct() {
        let center = Point::new(1.0e12, 1.0e12);
        let east = Point::new(1.0e12 + 1.0, 1.0e12);
        let north = Point::new(1.0e12, 1.0e12 + 1.0);
        let bbox = BBox::from_points([center, east, north]).expect("bbox");
        let tol = Tolerance::from_bbox(&bbox);
        let circle = Circle::new(center, 1.0).expect("circle");
        assert!(arc(circle, east, north, true, tol).is_ok());
        assert!(!has_duplicate_vertices(&[center, east, north], tol));
    }

    #[test]
    fn translated_polygon_centroid_uses_local_coordinates() {
        let origin = Point::new(1.0e8, 1.0e8);
        let vertices = vec![
            origin,
            Point::new(origin.x + 4.0, origin.y),
            Point::new(origin.x, origin.y + 3.0),
        ];
        let bbox = BBox::from_points(vertices.iter().copied()).expect("bbox");
        let tol = Tolerance::from_bbox(&bbox);
        let poly = polygon(vertices, tol).expect("translated triangle");
        close(
            tol,
            polygon_centroid(&poly).expect("centroid"),
            Point::new(origin.x + 4.0 / 3.0, origin.y + 1.0),
        );
    }

    #[test]
    fn equilateral_centroid_and_polygon_anchors_are_known() {
        let tol = tolerance();
        let height = libm::sqrt(3.0);
        let points = [
            Point::new(-1.0, 0.0),
            Point::new(1.0, 0.0),
            Point::new(0.0, height),
        ];
        let expected = Point::new(0.0, height / 3.0);
        close(tol, centroid(&points).expect("centroid"), expected);
        let poly = polygon(points.to_vec(), tol).expect("polygon");
        close(
            tol,
            polygon_centroid(&poly).expect("polygon centroid"),
            expected,
        );
    }

    #[test]
    fn circle_arc_polygon_rect_and_diamond_domains_are_checked() {
        let tol = tolerance();
        let circle = Circle::new(Point::new(0.0, 0.0), 2.0).expect("circle");
        let start = on_circle(circle, 0.0).expect("point");
        let end = on_circle(circle, core::f64::consts::FRAC_PI_2).expect("point");
        assert!(arc(circle, start, end, true, tol).is_ok());
        assert_eq!(
            measure::area(&rect(Point::new(0.0, 0.0), 4.0, 3.0, tol).expect("rect")),
            12.0
        );
        assert_eq!(
            measure::area(&diamond(Point::new(0.0, 0.0), 4.0, 3.0, tol).expect("diamond")),
            6.0
        );
        assert!(matches!(
            polygon(
                vec![
                    Point::new(0.0, 0.0),
                    Point::new(1.0, 1.0),
                    Point::new(0.0, 1.0),
                    Point::new(1.0, 0.0)
                ],
                tol
            ),
            Err(ConstructError::InvalidPolygon { .. })
        ));
        assert!(matches!(
            polygon(
                vec![
                    Point::new(0.0, 0.0),
                    Point::new(3.0, 0.0),
                    Point::new(1.0, 0.0),
                    Point::new(1.0, 2.0),
                ],
                tol,
            ),
            Err(ConstructError::InvalidPolygon { .. })
        ));
    }

    #[test]
    fn projection_bisectors_and_circle_helpers_have_known_answers() {
        let tol = tolerance();
        let circle = Circle::new(Point::new(0.0, 0.0), 2.0).expect("circle");
        close(
            tol,
            project(Point::new(3.0, 4.0), circle, tol).expect("circle projection"),
            Point::new(1.2, 1.6),
        );
        assert_eq!(
            reflect_over_point(Point::new(3.0, -1.0), Point::new(1.0, 2.0)),
            Ok(Point::new(-1.0, 5.0))
        );
        close(
            tol,
            on_circle(circle, core::f64::consts::FRAC_PI_2).expect("circle anchor"),
            Point::new(0.0, 2.0),
        );
        assert_eq!(
            circle_through(Point::new(0.0, 0.0), Point::new(0.0, 3.0), tol),
            Circle::new(Point::new(0.0, 0.0), 3.0)
        );
        assert_eq!(
            circle_diameter(Point::new(-2.0, 0.0), Point::new(2.0, 0.0), tol),
            Ok(circle)
        );

        let base = Line::new(Point::new(0.0, 0.0), Point::new(2.0, 0.0)).expect("line");
        let perpendicular = perpendicular(Point::new(1.0, 1.0), base).expect("perpendicular");
        let parallel = parallel(Point::new(1.0, 1.0), base).expect("parallel");
        assert!(tol.area_zero(perpendicular.direction().dot(base.direction())));
        assert!(tol.area_zero(parallel.direction().cross(base.direction())));
        let bisector = perp_bisector(Point::new(-2.0, 0.0), Point::new(2.0, 0.0), tol)
            .expect("perpendicular bisector");
        assert_eq!(bisector.start, Point::new(0.0, 0.0));
        let angle_line = angle_bisector(
            Point::new(1.0, 0.0),
            Point::new(0.0, 0.0),
            Point::new(0.0, 1.0),
            tol,
        )
        .expect("angle bisector");
        assert!(tol.length_eq(angle_line.direction().x, angle_line.direction().y));

        let large = 1.0e13;
        let large_bbox =
            BBox::from_points([Point::new(0.0, 0.0), Point::new(large, large)]).expect("bbox");
        let large_tol = Tolerance::from_bbox(&large_bbox);
        let large_angle_line = angle_bisector(
            Point::new(large, 0.0),
            Point::new(0.0, 0.0),
            Point::new(0.0, large),
            large_tol,
        )
        .expect("large right-angle bisector");
        close(
            large_tol,
            foot(Point::new(large, 0.0), large_angle_line, large_tol)
                .expect("project onto large right-angle bisector"),
            Point::new(large * 0.5, large * 0.5),
        );
    }

    #[test]
    fn angle_arc_and_bbox_anchors_have_known_answers() {
        let tol = tolerance();
        let mark = angle_arc(
            Point::new(1.0, 0.0),
            Point::new(0.0, 0.0),
            Point::new(0.0, 1.0),
            2.0,
            tol,
        )
        .expect("angle arc");
        assert_eq!(mark.start, Point::new(2.0, 0.0));
        close(tol, mark.end, Point::new(0.0, 2.0));
        let bbox = BBox::for_arc(mark);
        assert_eq!(bbox.center(), Point::new(1.0, 1.0));
        assert_eq!(bbox.north(), Point::new(1.0, 2.0));
        assert_eq!(bbox.east(), Point::new(2.0, 1.0));
        assert!(crate::geom::arc_contains_angle(
            mark,
            core::f64::consts::FRAC_PI_2 + tol.epsilon() * 0.5,
            tol,
        ));
        assert!(crate::geom::arc_contains_angle(
            mark,
            -tol.epsilon() * 0.5,
            tol,
        ));

        let straight_error = angle_arc(
            Point::new(1.0, 0.0),
            Point::new(0.0, 0.0),
            Point::new(-1.0, 0.0),
            2.0,
            tol,
        )
        .expect_err("straight angle has no branch");
        assert_eq!(straight_error.code(), "E203");
    }

    #[test]
    fn polygon_edge_collinearity_uses_scale_free_constructor_tolerance() {
        let bbox = BBox::from_points([Point::new(0.0, 0.0), Point::new(100.0, 0.0)]).expect("bbox");
        let tol = Tolerance::from_bbox(&bbox);
        assert!(!closed_segments_intersect(
            Point::new(0.0, 0.0),
            Point::new(1.0e-6, 0.0),
            Point::new(0.5e-6, 1.0e-3),
            Point::new(0.5e-6, 1.001e-3),
            tol,
        ));
    }

    #[test]
    fn tangent_branches_are_sorted_by_contact_angle() {
        let tol = tolerance();
        let circle = Circle::new(Point::new(0.0, 0.0), 1.0).expect("circle");
        let point = Point::new(2.0, 0.0);
        let first = tangent_line(point, circle, 0, tol)
            .expect("first tangent")
            .through;
        let second = tangent_line(point, circle, 1, tol)
            .expect("second tangent")
            .through;
        assert!(contact_angle(circle, first) < contact_angle(circle, second));
    }

    #[test]
    fn degenerate_inputs_return_e2xx_ready_errors() {
        let tol = tolerance();
        let error = circumcircle(
            Point::new(0.0, 0.0),
            Point::new(1.0, 0.0),
            Point::new(2.0, 0.0),
            tol,
        )
        .expect_err("collinear triangle");
        assert_eq!(error.code(), "E203");
        assert!(Circle::new(Point::new(0.0, 0.0), 0.0).is_err());
        let zero_segment =
            Segment::new(Point::new(1.0, 1.0), Point::new(1.0, 1.0)).expect("stored segment");
        assert!(foot(Point::new(2.0, 2.0), zero_segment, tol).is_err());
    }

    proptest! {
        #[test]
        fn reflecting_twice_is_identity(
            px in -1.0e3_f64..1.0e3,
            py in -1.0e3_f64..1.0e3,
            ax in -1.0e3_f64..1.0e3,
            ay in -1.0e3_f64..1.0e3,
            dx in 1.0_f64..1.0e3,
            dy in -1.0e3_f64..1.0e3,
        ) {
            let point = Point::new(px, py);
            let axis = Line::new(Point::new(ax, ay), Point::new(ax + dx, ay + dy)).expect("nonzero line");
            let bbox = BBox::from_points([point, axis.start, axis.through]).expect("bbox");
            let tol = Tolerance::from_bbox(&bbox);
            let reflected = reflect_over_line(point, axis, tol).expect("first reflection");
            let restored = reflect_over_line(reflected, axis, tol).expect("second reflection");
            prop_assert!(tol.points_eq(point, restored));
        }
    }
}
