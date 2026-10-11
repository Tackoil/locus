//! Axis-aligned boxes with circular corner arcs.

use super::{Arc, BBox, Circle, ConstructError, Point, Segment, Vec2};

/// An axis-aligned rectangle with equal circular corner radii.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RoundedRect {
    /// Center anchor.
    pub center: Point,
    /// Full outer width.
    pub width: f64,
    /// Full outer height.
    pub height: f64,
    /// Corner radius, between zero and half the smaller dimension.
    pub corner_radius: f64,
}

impl RoundedRect {
    /// Constructs a rounded rectangle; dimensions must be positive and finite.
    pub fn new(
        center: Point,
        width: f64,
        height: f64,
        corner_radius: f64,
    ) -> Result<Self, ConstructError> {
        if !center.is_finite()
            || ![width, height, corner_radius]
                .into_iter()
                .all(f64::is_finite)
        {
            return Err(ConstructError::NonFinite {
                operation: "rounded_rect",
            });
        }
        if width <= 0.0
            || height <= 0.0
            || corner_radius < 0.0
            || corner_radius > width.min(height) / 2.0
        {
            return Err(ConstructError::Domain {
                operation: "rounded_rect",
                reason: "width and height must be positive; radius must satisfy 0 <= r <= min(width, height)/2",
            });
        }
        let rect = Self {
            center,
            width,
            height,
            corner_radius,
        };
        let bounds = rect.bbox();
        super::validate_finite_points("rounded_rect", &[bounds.min, bounds.max])?;
        Ok(rect)
    }

    /// Exact outer bounds, independent of the corner radius.
    #[must_use]
    pub fn bbox(self) -> BBox {
        let half = Vec2::new(self.width / 2.0, self.height / 2.0);
        BBox {
            min: self.center - half,
            max: self.center + half,
        }
    }

    /// Exact enclosed area.
    #[must_use]
    pub fn area(self) -> f64 {
        self.width * self.height
            - (4.0 - core::f64::consts::PI) * self.corner_radius * self.corner_radius
    }

    /// Exact boundary length.
    #[must_use]
    pub fn perimeter(self) -> f64 {
        2.0 * (self.width + self.height) - (8.0 - core::f64::consts::TAU) * self.corner_radius
    }

    /// Euclidean distance to the boundary, negative in the interior.
    #[must_use]
    pub fn signed_distance(self, point: Point) -> f64 {
        let r = self.corner_radius;
        let x = (point.x - self.center.x).abs() - (self.width / 2.0 - r);
        let y = (point.y - self.center.y).abs() - (self.height / 2.0 - r);
        libm::hypot(x.max(0.0), y.max(0.0)) + x.max(y).min(0.0) - r
    }

    /// Four counterclockwise corner arcs, starting at the bottom-right corner.
    /// A zero-radius rectangle has no arcs.
    #[must_use]
    pub fn corner_arcs(self) -> Vec<Arc> {
        let r = self.corner_radius;
        if r == 0.0 {
            return Vec::new();
        }
        let x = self.width / 2.0 - r;
        let y = self.height / 2.0 - r;
        [
            (x, -y, 0.0, -1.0, 1.0, 0.0),
            (x, y, 1.0, 0.0, 0.0, 1.0),
            (-x, y, 0.0, 1.0, -1.0, 0.0),
            (-x, -y, -1.0, 0.0, 0.0, -1.0),
        ]
        .map(|(cx, cy, sx, sy, ex, ey)| {
            let center = self.center + Vec2::new(cx, cy);
            Arc {
                circle: Circle { center, radius: r },
                start: center + Vec2::new(sx * r, sy * r),
                end: center + Vec2::new(ex * r, ey * r),
                ccw: true,
            }
        })
        .to_vec()
    }

    /// Straight portions of the boundary in counterclockwise order.
    #[must_use]
    pub fn sides(self) -> [Segment; 4] {
        let BBox { min, max } = self.bbox();
        let r = self.corner_radius;
        [
            Segment {
                start: Point::new(min.x + r, min.y),
                end: Point::new(max.x - r, min.y),
            },
            Segment {
                start: Point::new(max.x, min.y + r),
                end: Point::new(max.x, max.y - r),
            },
            Segment {
                start: Point::new(max.x - r, max.y),
                end: Point::new(min.x + r, max.y),
            },
            Segment {
                start: Point::new(min.x, max.y - r),
                end: Point::new(min.x, min.y + r),
            },
        ]
    }
}
