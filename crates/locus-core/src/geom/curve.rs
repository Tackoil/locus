//! Analytic ellipse and fixed cubic B-spline geometry.

use super::{ConstructError, Point, Vec2, validate_finite_points, validate_finite_result};

/// A finite ellipse with ordered positive semiaxes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ellipse {
    /// Center anchor.
    pub center: Point,
    /// Semimajor axis length.
    pub semimajor: f64,
    /// Semiminor axis length.
    pub semiminor: f64,
    /// Major-axis rotation in radians, counterclockwise from +x.
    pub rotation: f64,
}

impl Ellipse {
    /// Constructs a finite ellipse satisfying `a >= b > 0`.
    pub fn new(
        center: Point,
        semimajor: f64,
        semiminor: f64,
        rotation: f64,
    ) -> Result<Self, ConstructError> {
        validate_finite_points("ellipse", &[center])?;
        if !semimajor.is_finite() || !semiminor.is_finite() || !rotation.is_finite() {
            return Err(ConstructError::NonFinite {
                operation: "ellipse",
            });
        }
        if semiminor <= 0.0 || semimajor < semiminor {
            return Err(ConstructError::Domain {
                operation: "ellipse",
                reason: "semiaxes must satisfy a >= b > 0",
            });
        }
        Ok(Self {
            center,
            semimajor,
            semiminor,
            rotation,
        })
    }

    /// Unit vector along the major axis.
    #[must_use]
    pub fn major_unit(self) -> Vec2 {
        Vec2::new(libm::cos(self.rotation), libm::sin(self.rotation))
    }

    /// Unit vector along the minor axis.
    #[must_use]
    pub fn minor_unit(self) -> Vec2 {
        Vec2::new(-libm::sin(self.rotation), libm::cos(self.rotation))
    }

    /// Point at ellipse parameter `phi`.
    pub fn at(self, phi: f64) -> Result<Point, ConstructError> {
        if !phi.is_finite() {
            return Err(ConstructError::NonFinite {
                operation: "ellipse.at",
            });
        }
        validate_finite_result(
            "ellipse.at",
            self.center
                + self.major_unit() * (self.semimajor * libm::cos(phi))
                + self.minor_unit() * (self.semiminor * libm::sin(phi)),
        )
    }

    /// Distance from the center to either focus.
    #[must_use]
    pub fn focal_distance(self) -> f64 {
        libm::sqrt((self.semimajor - self.semiminor) * (self.semimajor + self.semiminor))
    }
}

/// A derived open, clamped, uniform cubic B-spline.
#[derive(Clone, Debug, PartialEq)]
pub struct Bspline {
    controls: Vec<Point>,
}

impl Bspline {
    /// Constructs a cubic spline from at least four finite controls.
    pub fn new(controls: Vec<Point>) -> Result<Self, ConstructError> {
        if controls.len() < 4 {
            return Err(ConstructError::Domain {
                operation: "bspline",
                reason: "at least four control points are required",
            });
        }
        validate_finite_points("bspline", &controls)?;
        if controls.first() == controls.last() {
            return Err(ConstructError::Degenerate {
                operation: "bspline",
                reason: "first and last controls must be distinct",
            });
        }
        Ok(Self { controls })
    }

    /// Ordered control points.
    #[must_use]
    pub fn controls(&self) -> &[Point] {
        &self.controls
    }

    /// Evaluates the curve at a scalar parameter in `[0, 1]`.
    pub fn at(&self, parameter: f64) -> Result<Point, ConstructError> {
        validate_parameter(parameter)?;
        if parameter == 0.0 {
            return Ok(self.controls[0]);
        }
        if parameter >= 1.0 {
            return Ok(self.controls[self.controls.len() - 1]);
        }
        let knots = knot_vector(self.controls.len());
        let span = find_span(&knots, self.controls.len(), 3, parameter);
        de_boor(&self.controls, &knots, 3, span, parameter)
    }

    /// Converts every nonzero knot span to its exact cubic Bezier controls.
    #[allow(
        clippy::cast_precision_loss,
        reason = "an allocated control-point count is exactly representable at every feasible size"
    )]
    pub fn bezier_spans(&self) -> Result<Vec<[Point; 4]>, ConstructError> {
        let span_count = self.controls.len() - 3;
        let mut result = Vec::with_capacity(span_count);
        for span in 0..span_count {
            let start = span as f64 / span_count as f64;
            let end = (span + 1) as f64 / span_count as f64;
            let first = self.at(start)?;
            let last = self.at(end)?;
            let first_derivative = self.derivative(start, span)?;
            let last_derivative = self.derivative(end, span)?;
            let width = (end - start) / 3.0;
            result.push([
                first,
                first + first_derivative * width,
                last - last_derivative * width,
                last,
            ]);
        }
        Ok(result)
    }

    fn derivative(&self, parameter: f64, source_span: usize) -> Result<Vec2, ConstructError> {
        let knots = knot_vector(self.controls.len());
        let mut controls = Vec::with_capacity(self.controls.len() - 1);
        for index in 0..self.controls.len() - 1 {
            let denominator = knots[index + 4] - knots[index + 1];
            controls.push((self.controls[index + 1] - self.controls[index]) * (3.0 / denominator));
        }
        let derivative_knots = &knots[1..knots.len() - 1];
        let derivative_span = source_span + 2;
        let point_controls = controls
            .iter()
            .map(|value| Point::new(value.x, value.y))
            .collect::<Vec<_>>();
        let value = de_boor(
            &point_controls,
            derivative_knots,
            2,
            derivative_span,
            parameter,
        )?;
        Ok(Vec2::new(value.x, value.y))
    }
}

fn validate_parameter(parameter: f64) -> Result<(), ConstructError> {
    if !parameter.is_finite() {
        return Err(ConstructError::NonFinite {
            operation: "bspline.at",
        });
    }
    if !(0.0..=1.0).contains(&parameter) {
        return Err(ConstructError::Domain {
            operation: "bspline.at",
            reason: "parameter must be in [0, 1]",
        });
    }
    Ok(())
}

#[allow(
    clippy::cast_precision_loss,
    reason = "an allocated control-point count is exactly representable at every feasible size"
)]
fn knot_vector(control_count: usize) -> Vec<f64> {
    let span_count = control_count - 3;
    let mut knots = vec![0.0; 4];
    knots.extend((1..span_count).map(|index| index as f64 / span_count as f64));
    knots.extend([1.0; 4]);
    knots
}

fn find_span(knots: &[f64], control_count: usize, degree: usize, parameter: f64) -> usize {
    (degree..control_count)
        .find(|index| knots[*index] <= parameter && parameter < knots[*index + 1])
        .unwrap_or(control_count - 1)
}

fn de_boor(
    controls: &[Point],
    knots: &[f64],
    degree: usize,
    span: usize,
    parameter: f64,
) -> Result<Point, ConstructError> {
    let mut values = (0..=degree)
        .map(|index| controls[span - degree + index])
        .collect::<Vec<_>>();
    for level in 1..=degree {
        for index in (level..=degree).rev() {
            let knot_index = span - degree + index;
            let denominator = knots[knot_index + degree + 1 - level] - knots[knot_index];
            let alpha = (parameter - knots[knot_index]) / denominator;
            values[index] = values[index - 1] + (values[index] - values[index - 1]) * alpha;
        }
    }
    validate_finite_result("bspline.at", values[degree])
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn ellipse_anchors_and_bspline_spans_are_analytic() {
        let ellipse = Ellipse::new(Point::new(1.0, 2.0), 3.0, 2.0, 0.0).expect("ellipse");
        assert_eq!(ellipse.at(0.0).expect("major end"), Point::new(4.0, 2.0));
        assert!((ellipse.focal_distance() - libm::sqrt(5.0)).abs() < 1.0e-12);

        let controls = vec![
            Point::new(0.0, 0.0),
            Point::new(1.0, 2.0),
            Point::new(3.0, 2.0),
            Point::new(4.0, 0.0),
            Point::new(5.0, 1.0),
        ];
        let spline = Bspline::new(controls.clone()).expect("B-spline");
        let spans = spline.bezier_spans().expect("Bezier spans");
        assert_eq!(spans.len(), 2);
        assert_eq!(spans[0][0], controls[0]);
        assert_eq!(spans[1][3], controls[4]);
        assert_eq!(spans[0][3], spans[1][0]);
        assert_eq!(spline.at(0.5).expect("interior knot"), spans[0][3]);
    }
}
