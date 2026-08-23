//! The single home of numeric tolerance policy in `locus-core`.

use crate::geom::{BBox, Vec2};

/// Default normalized tolerance for assertions and geometric incidence.
pub const DEFAULT_EPSILON: f64 = 1.0e-9;

/// Normalized threshold for constructor preconditions called out by the spec.
pub const CONSTRUCTION_EPSILON: f64 = 1.0e-12;

/// A normalized tolerance together with the scene characteristic length.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tolerance {
    diameter: f64,
    epsilon: f64,
}

impl Tolerance {
    /// Default tolerance for comparisons whose normalization is independent of `D`.
    pub(crate) const fn angular_default() -> Self {
        Self {
            diameter: 1.0,
            epsilon: DEFAULT_EPSILON,
        }
    }

    /// Builds the default tolerance from a geometry-only bounding box.
    #[must_use]
    pub fn from_bbox(bbox: &BBox) -> Self {
        Self::from_bbox_with_fallbacks(bbox, &[])
    }

    /// Builds the default tolerance, applying the spec's zero-diameter fallback.
    #[must_use]
    pub fn from_bbox_with_fallbacks(bbox: &BBox, positive_lengths: &[f64]) -> Self {
        let bbox_diameter = bbox.diameter();
        let diameter = if bbox_diameter > 0.0 && bbox_diameter.is_finite() {
            bbox_diameter
        } else {
            positive_lengths
                .iter()
                .copied()
                .find(|value| value.is_finite() && *value > 0.0)
                .unwrap_or(1.0)
        };
        Self {
            diameter,
            epsilon: DEFAULT_EPSILON,
        }
    }

    /// Builds a tolerance with a caller-supplied normalized assert factor.
    ///
    /// Returns `None` when either input violates the finite-positive domain.
    #[must_use]
    pub fn with_epsilon(diameter: f64, epsilon: f64) -> Option<Self> {
        (diameter.is_finite() && diameter > 0.0 && epsilon.is_finite() && epsilon > 0.0)
            .then_some(Self { diameter, epsilon })
    }

    /// Returns the characteristic scene length.
    #[must_use]
    pub const fn diameter(self) -> f64 {
        self.diameter
    }

    /// Returns the normalized assert/incidence factor.
    #[must_use]
    pub const fn epsilon(self) -> f64 {
        self.epsilon
    }

    /// Tests a length-valued residual with the default/user assert factor.
    #[must_use]
    pub fn length_zero(self, residual: f64) -> bool {
        normalized_zero(residual, self.diameter, self.epsilon)
    }

    /// Tests equality of two length-valued quantities.
    #[must_use]
    pub fn length_eq(self, lhs: f64, rhs: f64) -> bool {
        normalized_eq(lhs, rhs, self.diameter, self.epsilon)
    }

    /// Tests an area-valued residual with the default/user assert factor.
    #[must_use]
    pub fn area_zero(self, residual: f64) -> bool {
        normalized_zero(residual, self.diameter * self.diameter, self.epsilon)
    }

    /// Tests equality of two area-valued quantities.
    #[must_use]
    pub fn area_eq(self, lhs: f64, rhs: f64) -> bool {
        normalized_eq(lhs, rhs, self.diameter * self.diameter, self.epsilon)
    }

    /// Tests an angle-valued residual (angles normalize by one radian).
    #[must_use]
    pub fn angle_zero(self, residual: f64) -> bool {
        normalized_zero(residual, 1.0, self.epsilon)
    }

    /// Tests a scalar-valued residual.
    #[must_use]
    pub fn scalar_zero(self, residual: f64, lhs: f64, rhs: f64) -> bool {
        let scale = 1.0_f64.max(lhs.abs()).max(rhs.abs());
        residual.abs() / scale <= self.epsilon
    }

    /// Tests a constructor's normalized length threshold (`1e-12`).
    #[must_use]
    pub fn construction_length_zero(self, residual: f64) -> bool {
        normalized_zero(residual, self.diameter, CONSTRUCTION_EPSILON)
    }

    /// Tests a constructor's normalized length equality threshold (`1e-12`).
    #[must_use]
    pub fn construction_length_eq(self, lhs: f64, rhs: f64) -> bool {
        normalized_eq(lhs, rhs, self.diameter, CONSTRUCTION_EPSILON)
    }

    /// Tests a constructor's normalized area threshold (`1e-12`).
    #[must_use]
    pub fn construction_area_zero(self, residual: f64) -> bool {
        normalized_zero(
            residual,
            self.diameter * self.diameter,
            CONSTRUCTION_EPSILON,
        )
    }

    /// Tests whether two nonzero directions are parallel using `abs(sin(theta))`.
    #[must_use]
    pub fn directions_parallel(self, lhs: Vec2, rhs: Vec2) -> bool {
        normalized_direction_cross(lhs, rhs) <= self.epsilon
    }

    /// Tests direction parallelism at the constructor threshold (`1e-12`).
    #[must_use]
    pub fn construction_directions_parallel(self, lhs: Vec2, rhs: Vec2) -> bool {
        normalized_direction_cross(lhs, rhs) <= CONSTRUCTION_EPSILON
    }

    /// Tests whether an offset lies on a carrier direction at incidence tolerance.
    #[must_use]
    pub fn vector_on_direction(self, offset: Vec2, direction: Vec2) -> bool {
        normalized_vector_distance(offset, direction, self.diameter, self.epsilon)
    }

    /// Tests carrier incidence at the constructor threshold (`1e-12`).
    #[must_use]
    pub fn construction_vector_on_direction(self, offset: Vec2, direction: Vec2) -> bool {
        normalized_vector_distance(offset, direction, self.diameter, CONSTRUCTION_EPSILON)
    }

    /// Tests whether an angular offset lies in a directed sweep, including tolerance.
    #[must_use]
    pub fn angle_in_closed_sweep(self, offset: f64, sweep: f64) -> bool {
        let distance_before_start = 2.0 * core::f64::consts::PI - offset;
        (offset >= -self.epsilon && offset <= sweep + self.epsilon)
            || distance_before_start <= self.epsilon
    }

    /// Tests whether a parameter lies in `[0, 1]` at construction tolerance.
    #[must_use]
    pub fn construction_parameter_in_unit_interval(self, value: f64) -> bool {
        let scale = 1.0_f64.max(value.abs());
        value >= -CONSTRUCTION_EPSILON * scale && value <= 1.0 + CONSTRUCTION_EPSILON * scale
    }

    /// Returns true when `value` lies in `[lower, upper]`, including tolerance.
    #[must_use]
    pub fn parameter_in_closed_interval(self, value: f64, lower: f64, upper: f64) -> bool {
        let scale = 1.0_f64.max(value.abs()).max(lower.abs()).max(upper.abs());
        value >= lower - self.epsilon * scale && value <= upper + self.epsilon * scale
    }

    /// Returns true when a dimensionless carrier parameter is nonnegative.
    #[must_use]
    pub fn parameter_nonnegative(self, value: f64) -> bool {
        let scale = 1.0_f64.max(value.abs());
        value >= -self.epsilon * scale
    }

    /// Tests point equality using independent normalized coordinate residuals.
    #[must_use]
    pub fn points_eq(self, lhs: crate::geom::Point, rhs: crate::geom::Point) -> bool {
        self.length_zero(lhs.x - rhs.x) && self.length_zero(lhs.y - rhs.y)
    }

    /// Tests point equality at the tighter constructor threshold (`1e-12`).
    #[must_use]
    pub fn construction_points_eq(self, lhs: crate::geom::Point, rhs: crate::geom::Point) -> bool {
        self.construction_length_zero(lhs.x - rhs.x) && self.construction_length_zero(lhs.y - rhs.y)
    }
}

fn normalized_zero(residual: f64, dimension_scale: f64, epsilon: f64) -> bool {
    residual.abs() / dimension_scale.max(residual.abs()) <= epsilon
}

fn normalized_eq(lhs: f64, rhs: f64, dimension_scale: f64, epsilon: f64) -> bool {
    let scale = dimension_scale.max(lhs.abs()).max(rhs.abs());
    (lhs - rhs).abs() / scale <= epsilon
}

fn normalized_direction_cross(lhs: Vec2, rhs: Vec2) -> f64 {
    let norm_product = lhs.norm() * rhs.norm();
    if norm_product == 0.0 {
        f64::INFINITY
    } else {
        lhs.cross(rhs).abs() / norm_product
    }
}

fn normalized_vector_distance(offset: Vec2, direction: Vec2, diameter: f64, epsilon: f64) -> bool {
    let direction_length = direction.norm();
    direction_length > 0.0
        && normalized_zero(
            offset.cross(direction) / direction_length,
            diameter,
            epsilon,
        )
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::float_cmp)]
mod tests {
    use super::*;
    use crate::geom::Point;

    #[test]
    fn bbox_diameter_scales_length_tolerance() {
        let bbox = BBox::from_points([Point::new(0.0, 0.0), Point::new(300.0, 400.0)])
            .expect("finite bbox");
        let tolerance = Tolerance::from_bbox(&bbox);
        assert!(tolerance.length_zero(5.0e-7));
        assert!(!tolerance.length_zero(5.1e-7));
    }

    #[test]
    fn zero_bbox_uses_first_positive_fallback() {
        let bbox = BBox::from_points([Point::new(2.0, 2.0)]).expect("finite bbox");
        let tolerance = Tolerance::from_bbox_with_fallbacks(&bbox, &[0.0, 7.0, 9.0]);
        assert_eq!(tolerance.diameter(), 7.0);
    }

    #[test]
    fn point_equality_is_translation_invariant() {
        let bbox =
            BBox::from_points([Point::new(0.0, 0.0), Point::new(1.0, 0.0)]).expect("finite bbox");
        let tolerance = Tolerance::from_bbox(&bbox);
        let near_origin = (Point::new(1.0, 0.0), Point::new(0.0, 1.0));
        let translated = (
            Point::new(1.0e12 + 1.0, 1.0e12),
            Point::new(1.0e12, 1.0e12 + 1.0),
        );
        assert_eq!(
            tolerance.construction_points_eq(near_origin.0, near_origin.1),
            tolerance.construction_points_eq(translated.0, translated.1),
        );
        assert!(!tolerance.construction_points_eq(translated.0, translated.1));
    }
}
