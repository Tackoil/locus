//! Geometry primitives and pure constructors, intersections, and measures.

use std::ops::{Add, Div, Mul, Neg, Sub};

use thiserror::Error;

use crate::tol::Tolerance;

pub mod construct;
pub mod intersect;
pub mod measure;

/// A stable, parser-independent construction failure suitable for E2xx diagnostics.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum ConstructError {
    /// A coordinate, measurement, or constructor result was not finite.
    #[error("E202: {operation} produced or received a non-finite value")]
    NonFinite {
        /// Constructor or operation name.
        operation: &'static str,
    },
    /// An operation required a direction or uniquely defined center.
    #[error("E203: {operation} is undefined for degenerate geometry: {reason}")]
    Degenerate {
        /// Constructor or operation name.
        operation: &'static str,
        /// Failed geometric precondition.
        reason: &'static str,
    },
    /// A value violated a constructor-specific domain.
    #[error("E204: {operation} domain error: {reason}")]
    Domain {
        /// Constructor or operation name.
        operation: &'static str,
        /// Failed scalar or object-domain precondition.
        reason: &'static str,
    },
    /// A polygon failed the simple-polygon requirements.
    #[error("E205: invalid polygon: {reason}")]
    InvalidPolygon {
        /// Failed simple-polygon precondition.
        reason: &'static str,
    },
    /// A requested arc endpoint was not incident on its circle.
    #[error("E206: arc endpoint is not on its circle")]
    ArcEndpointNotOnCircle,
}

impl ConstructError {
    /// Returns the stable diagnostic code without coupling the kernel to source spans.
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::NonFinite { .. } => "E202",
            Self::Degenerate { .. } => "E203",
            Self::Domain { .. } => "E204",
            Self::InvalidPolygon { .. } => "E205",
            Self::ArcEndpointNotOnCircle => "E206",
        }
    }
}

/// A Cartesian point in y-up scene space.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Point {
    /// Horizontal coordinate.
    pub x: f64,
    /// Vertical coordinate.
    pub y: f64,
}

impl Point {
    /// Creates a point. Evaluation boundaries are responsible for rejecting non-finite inputs.
    #[must_use]
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    /// Returns whether both coordinates are finite.
    #[must_use]
    pub fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite()
    }
}

/// A two-dimensional vector.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec2 {
    /// Horizontal component.
    pub x: f64,
    /// Vertical component.
    pub y: f64,
}

impl Vec2 {
    /// Creates a vector.
    #[must_use]
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    /// Dot product.
    #[must_use]
    pub fn dot(self, rhs: Self) -> f64 {
        self.x * rhs.x + self.y * rhs.y
    }

    /// Two-dimensional scalar cross product.
    #[must_use]
    pub fn cross(self, rhs: Self) -> f64 {
        self.x * rhs.y - self.y * rhs.x
    }

    /// Squared Euclidean norm.
    #[must_use]
    pub fn norm_squared(self) -> f64 {
        self.dot(self)
    }

    /// Euclidean norm, evaluated by pinned `libm`.
    #[must_use]
    pub fn norm(self) -> f64 {
        libm::sqrt(self.norm_squared())
    }

    /// A 90-degree counterclockwise rotation.
    #[must_use]
    pub const fn perpendicular(self) -> Self {
        Self::new(-self.y, self.x)
    }

    /// Returns whether both components are finite.
    #[must_use]
    pub fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite()
    }
}

impl Add<Vec2> for Point {
    type Output = Self;

    fn add(self, rhs: Vec2) -> Self::Output {
        Self::new(self.x + rhs.x, self.y + rhs.y)
    }
}

impl Sub<Vec2> for Point {
    type Output = Self;

    fn sub(self, rhs: Vec2) -> Self::Output {
        Self::new(self.x - rhs.x, self.y - rhs.y)
    }
}

impl Sub for Point {
    type Output = Vec2;

    fn sub(self, rhs: Self) -> Self::Output {
        Vec2::new(self.x - rhs.x, self.y - rhs.y)
    }
}

impl Add for Vec2 {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self::new(self.x + rhs.x, self.y + rhs.y)
    }
}

impl Sub for Vec2 {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self::new(self.x - rhs.x, self.y - rhs.y)
    }
}

impl Neg for Vec2 {
    type Output = Self;

    fn neg(self) -> Self::Output {
        Self::new(-self.x, -self.y)
    }
}

impl Mul<f64> for Vec2 {
    type Output = Self;

    fn mul(self, rhs: f64) -> Self::Output {
        Self::new(self.x * rhs, self.y * rhs)
    }
}

impl Div<f64> for Vec2 {
    type Output = Self;

    fn div(self, rhs: f64) -> Self::Output {
        Self::new(self.x / rhs, self.y / rhs)
    }
}

/// An infinite directed line through two distinct defining points.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Line {
    /// First defining point and parameter origin.
    pub start: Point,
    /// Second defining point; increasing parameter travels toward this point.
    pub through: Point,
}

impl Line {
    /// Constructs a line from distinct, finite points.
    pub fn new(start: Point, through: Point) -> Result<Self, ConstructError> {
        validate_finite_points("line", &[start, through])?;
        if start == through {
            return Err(ConstructError::Degenerate {
                operation: "line",
                reason: "defining points coincide",
            });
        }
        Ok(Self { start, through })
    }

    /// Direction from `start` toward `through`.
    #[must_use]
    pub fn direction(self) -> Vec2 {
        self.through - self.start
    }
}

/// A directed half-line whose parameter domain is `[0, +infinity)`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ray {
    /// Finite endpoint and parameter origin.
    pub start: Point,
    /// Point fixing the positive direction.
    pub through: Point,
}

impl Ray {
    /// Constructs a ray from distinct, finite points.
    pub fn new(start: Point, through: Point) -> Result<Self, ConstructError> {
        validate_finite_points("ray", &[start, through])?;
        if start == through {
            return Err(ConstructError::Degenerate {
                operation: "ray",
                reason: "defining points coincide",
            });
        }
        Ok(Self { start, through })
    }

    /// Direction from `start` toward `through`.
    #[must_use]
    pub fn direction(self) -> Vec2 {
        self.through - self.start
    }
}

/// A finite segment. Zero-length segments may be stored.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Segment {
    /// Parameter-zero endpoint.
    pub start: Point,
    /// Parameter-one endpoint.
    pub end: Point,
}

impl Segment {
    /// Constructs a finite segment, including a zero-length one.
    pub fn new(start: Point, end: Point) -> Result<Self, ConstructError> {
        validate_finite_points("segment", &[start, end])?;
        Ok(Self { start, end })
    }

    /// Direction from `start` toward `end`.
    #[must_use]
    pub fn direction(self) -> Vec2 {
        self.end - self.start
    }

    /// Midpoint anchor.
    #[must_use]
    pub fn midpoint(self) -> Point {
        Point::new(
            self.start.x + (self.end.x - self.start.x) * 0.5,
            self.start.y + (self.end.y - self.start.y) * 0.5,
        )
    }
}

/// A circle with strictly positive radius.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Circle {
    /// Center anchor.
    pub center: Point,
    /// Strictly positive radius.
    pub radius: f64,
}

impl Circle {
    /// Constructs a finite circle with a strictly positive radius.
    pub fn new(center: Point, radius: f64) -> Result<Self, ConstructError> {
        validate_finite_points("circle", &[center])?;
        if !radius.is_finite() {
            return Err(ConstructError::NonFinite {
                operation: "circle",
            });
        }
        if radius <= 0.0 {
            return Err(ConstructError::Domain {
                operation: "circle",
                reason: "radius must be strictly positive",
            });
        }
        Ok(Self { center, radius })
    }

    /// Point anchor at a CCW angle in radians from +x.
    pub fn at(self, angle: f64) -> Result<Point, ConstructError> {
        construct::on_circle(self, angle)
    }
}

/// A directed, non-full-circle arc.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Arc {
    /// Supporting circle.
    pub circle: Circle,
    /// Directed sweep start.
    pub start: Point,
    /// Directed sweep end.
    pub end: Point,
    /// `true` for a counterclockwise sweep, `false` for clockwise.
    pub ccw: bool,
}

/// A simple polygon with vertices stored in user-provided order.
#[derive(Clone, Debug, PartialEq)]
pub struct Polygon {
    vertices: Vec<Point>,
}

impl Polygon {
    pub(crate) fn from_validated_vertices(vertices: Vec<Point>) -> Self {
        Self { vertices }
    }

    /// Ordered polygon vertices.
    #[must_use]
    pub fn vertices(&self) -> &[Point] {
        &self.vertices
    }

    /// Area centroid anchor.
    pub fn centroid(&self) -> Result<Point, ConstructError> {
        construct::polygon_centroid(self)
    }
}

/// An axis-aligned geometry bounding box.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BBox {
    /// Minimum x/y corner.
    pub min: Point,
    /// Maximum x/y corner.
    pub max: Point,
}

impl BBox {
    /// Constructs the smallest box containing all supplied finite points.
    pub fn from_points(points: impl IntoIterator<Item = Point>) -> Result<Self, ConstructError> {
        let mut iter = points.into_iter();
        let first = iter.next().ok_or(ConstructError::Domain {
            operation: "bbox",
            reason: "at least one point is required",
        })?;
        validate_finite_points("bbox", &[first])?;
        let mut bbox = Self {
            min: first,
            max: first,
        };
        for point in iter {
            validate_finite_points("bbox", &[point])?;
            bbox.include_point(point);
        }
        Ok(bbox)
    }

    /// Width of the box.
    #[must_use]
    pub fn width(self) -> f64 {
        self.max.x - self.min.x
    }

    /// Height of the box.
    #[must_use]
    pub fn height(self) -> f64 {
        self.max.y - self.min.y
    }

    /// Diagonal length, used as the tolerance characteristic length.
    #[must_use]
    pub fn diameter(self) -> f64 {
        libm::sqrt(self.width() * self.width() + self.height() * self.height())
    }

    /// Center anchor.
    #[must_use]
    pub fn center(self) -> Point {
        Point::new(
            self.min.x + self.width() * 0.5,
            self.min.y + self.height() * 0.5,
        )
    }

    /// North anchor.
    #[must_use]
    pub fn north(self) -> Point {
        Point::new(self.center().x, self.max.y)
    }

    /// South anchor.
    #[must_use]
    pub fn south(self) -> Point {
        Point::new(self.center().x, self.min.y)
    }

    /// East anchor.
    #[must_use]
    pub fn east(self) -> Point {
        Point::new(self.max.x, self.center().y)
    }

    /// West anchor.
    #[must_use]
    pub fn west(self) -> Point {
        Point::new(self.min.x, self.center().y)
    }

    /// Northeast anchor.
    #[must_use]
    pub const fn northeast(self) -> Point {
        self.max
    }

    /// Northwest anchor.
    #[must_use]
    pub fn northwest(self) -> Point {
        Point::new(self.min.x, self.max.y)
    }

    /// Southeast anchor.
    #[must_use]
    pub fn southeast(self) -> Point {
        Point::new(self.max.x, self.min.y)
    }

    /// Southwest anchor.
    #[must_use]
    pub const fn southwest(self) -> Point {
        self.min
    }

    /// Extends the box to contain a point.
    pub fn include_point(&mut self, point: Point) {
        self.min.x = self.min.x.min(point.x);
        self.min.y = self.min.y.min(point.y);
        self.max.x = self.max.x.max(point.x);
        self.max.y = self.max.y.max(point.y);
    }

    /// Extends the box to contain another box.
    pub fn include_bbox(&mut self, other: Self) {
        self.include_point(other.min);
        self.include_point(other.max);
    }

    /// Bounding box of a circle, including all of its geometry extents.
    #[must_use]
    pub fn for_circle(circle: Circle) -> Self {
        let delta = Vec2::new(circle.radius, circle.radius);
        Self {
            min: circle.center - delta,
            max: circle.center + delta,
        }
    }

    /// Bounding box of a line's finite defining points.
    #[must_use]
    pub fn for_line(line: Line) -> Self {
        Self::from_two_points(line.start, line.through)
    }

    /// Bounding box of a ray's finite defining points.
    #[must_use]
    pub fn for_ray(ray: Ray) -> Self {
        Self::from_two_points(ray.start, ray.through)
    }

    /// Bounding box of a segment.
    #[must_use]
    pub fn for_segment(segment: Segment) -> Self {
        Self::from_two_points(segment.start, segment.end)
    }

    /// Bounding box of the visible directed arc, including cardinal extrema.
    #[must_use]
    pub fn for_arc(arc: Arc) -> Self {
        let mut bbox = Self::from_two_points(arc.start, arc.end);
        for angle in [
            0.0,
            core::f64::consts::FRAC_PI_2,
            core::f64::consts::PI,
            3.0 * core::f64::consts::FRAC_PI_2,
        ] {
            if arc_contains_angle(arc, angle, Tolerance::angular_default()) {
                bbox.include_point(
                    arc.circle.center
                        + Vec2::new(libm::cos(angle), libm::sin(angle)) * arc.circle.radius,
                );
            }
        }
        bbox
    }

    /// Bounding box of polygon vertices.
    #[must_use]
    pub fn for_polygon(polygon: &Polygon) -> Self {
        let first = polygon.vertices[0];
        let mut bbox = Self {
            min: first,
            max: first,
        };
        for point in &polygon.vertices[1..] {
            bbox.include_point(*point);
        }
        bbox
    }

    fn from_two_points(first: Point, second: Point) -> Self {
        Self {
            min: Point::new(first.x.min(second.x), first.y.min(second.y)),
            max: Point::new(first.x.max(second.x), first.y.max(second.y)),
        }
    }
}

pub(crate) fn circle_angle(circle: Circle, point: Point) -> f64 {
    measure::normalize_angle(libm::atan2(
        point.y - circle.center.y,
        point.x - circle.center.x,
    ))
}

pub(crate) fn arc_contains_angle(arc: Arc, candidate: f64, tolerance: Tolerance) -> bool {
    let start = circle_angle(arc.circle, arc.start);
    let end = circle_angle(arc.circle, arc.end);
    let candidate = measure::normalize_angle(candidate);
    let (sweep, offset) = if arc.ccw {
        (
            measure::normalize_angle(end - start),
            measure::normalize_angle(candidate - start),
        )
    } else {
        (
            measure::normalize_angle(start - end),
            measure::normalize_angle(start - candidate),
        )
    };
    tolerance.angle_in_closed_sweep(offset, sweep)
}

pub(crate) fn validate_finite_points(
    operation: &'static str,
    points: &[Point],
) -> Result<(), ConstructError> {
    if points.iter().all(|point| point.is_finite()) {
        Ok(())
    } else {
        Err(ConstructError::NonFinite { operation })
    }
}

pub(crate) fn validate_finite_result(
    operation: &'static str,
    point: Point,
) -> Result<Point, ConstructError> {
    if point.is_finite() {
        Ok(point)
    } else {
        Err(ConstructError::NonFinite { operation })
    }
}
