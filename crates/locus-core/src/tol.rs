//! The single home of numeric tolerance policy in `locus-core`.

use locus_syntax::{Constructor, Expr, File, OpStmt, PropValue, Statement};

use crate::eval::env::Environment;
use crate::eval::expr::{ExpressionEvaluator, InferredDimension};
use crate::eval::{infer_dimensions, property_dimension};
use crate::geom::{BBox, Vec2};
use crate::scene::{Dimension, ResolvedScene};

/// Default normalized tolerance for assertions and geometric incidence.
pub const DEFAULT_EPSILON: f64 = 1.0e-9;

/// Normalized threshold for constructor preconditions called out by the spec.
pub const CONSTRUCTION_EPSILON: f64 = 1.0e-12;

/// Normative maximum normalized relation residual for solve convergence.
pub(crate) const SOLVE_EPSILON: f64 = 1.0e-12;

/// Proves that two equality targets cannot share a value at solve tolerance.
///
/// For `|x-t| < e*max(base, |x|, |t|)`, every solution lies inside
/// `t +/- e*max(base, |t|)/(1-e)`. Outward rounding keeps this conservative.
/// The same bounds also enclose solutions using a fixed normalization scale.
pub(crate) fn solve_targets_disjoint(lhs: f64, rhs: f64, base: f64) -> bool {
    if !base.is_finite() || base <= 0.0 {
        return false;
    }
    let (lower, upper) = if lhs <= rhs { (lhs, rhs) } else { (rhs, lhs) };
    let margin = |target: f64| {
        ((SOLVE_EPSILON.next_up() * base.max(target.abs())).next_up()
            / (1.0 - SOLVE_EPSILON).next_down())
        .next_up()
    };
    (lower + margin(lower)).next_up() < (upper - margin(upper)).next_down()
}

/// Normative relative singular-value threshold for Jacobian rank.
pub(crate) const RANK_FACTOR: f64 = 1.0e-10;

/// Normative one-sided Jacobi pair skip factor (`2^-48`).
pub(crate) const JACOBI_SKIP_FACTOR: f64 = 3.552_713_678_800_501e-15;

/// Normative normalized component threshold for `hint_far`.
pub(crate) const HINT_FAR_THRESHOLD: f64 = 0.25;

/// Normalized separation below which distinct named points are visually crowded.
const CROWDING_FACTOR: f64 = 0.02;

/// Largest integer considered when describing a detected line ratio.
const MAX_DETECTED_RATIO_TERM: u32 = 12;

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
                .filter(|value| value.is_finite() && *value > 0.0)
                .max_by(f64::total_cmp)
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
        fixed_scale_zero(residual, 1.0, self.epsilon)
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

    /// Tests a constructor's dimensionless scalar threshold (`1e-12`).
    #[must_use]
    pub fn construction_scalar_zero(self, residual: f64) -> bool {
        normalized_zero(residual, 1.0, CONSTRUCTION_EPSILON)
    }

    /// Tests a constructor's angular threshold (`1e-12` radians).
    #[must_use]
    pub fn construction_angle_zero(self, residual: f64) -> bool {
        fixed_scale_zero(residual, 1.0, CONSTRUCTION_EPSILON)
    }

    /// Tests whether two nonzero directions are parallel using `abs(sin(theta))`.
    #[must_use]
    pub fn directions_parallel(self, lhs: Vec2, rhs: Vec2) -> bool {
        normalized_direction_cross(lhs, rhs) <= self.epsilon
    }

    /// Tests whether two nonzero directions are perpendicular using `abs(cos(theta))`.
    #[must_use]
    pub fn directions_perpendicular(self, lhs: Vec2, rhs: Vec2) -> bool {
        normalized_direction_dot(lhs, rhs) <= self.epsilon
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

    /// Tests the tolerance-shrunk open interval required by strict `between`.
    #[must_use]
    pub fn parameter_strictly_inside_unit(self, value: f64) -> bool {
        value > self.epsilon && value < 1.0 - self.epsilon
    }

    /// Tests whether a length margin is strictly positive beyond the tolerance dead band.
    #[must_use]
    pub fn length_strictly_positive(self, margin: f64) -> bool {
        margin / self.diameter > self.epsilon
    }

    /// Tests whether `inner` is contained in `outer`, including tolerance at each edge.
    #[must_use]
    pub fn bbox_contains(self, outer: BBox, inner: BBox) -> bool {
        self.length_nonnegative(inner.min.x - outer.min.x)
            && self.length_nonnegative(inner.min.y - outer.min.y)
            && self.length_nonnegative(outer.max.x - inner.max.x)
            && self.length_nonnegative(outer.max.y - inner.max.y)
    }

    /// Tests whether two points are close enough to merit a visual-crowding diagnostic.
    #[must_use]
    pub fn points_crowded(self, lhs: crate::geom::Point, rhs: crate::geom::Point) -> bool {
        normalized_length(measure_distance(lhs, rhs), self.diameter) <= CROWDING_FACTOR
    }

    /// Detects a small positive integer ratio at incidence tolerance.
    #[must_use]
    pub fn small_integer_ratio(self, lhs: f64, rhs: f64) -> Option<(u32, u32)> {
        if !lhs.is_finite() || !rhs.is_finite() || lhs <= 0.0 || rhs <= 0.0 {
            return None;
        }
        let ratio = lhs / rhs;
        for denominator in 1..=MAX_DETECTED_RATIO_TERM {
            for numerator in 1..=MAX_DETECTED_RATIO_TERM {
                let candidate = f64::from(numerator) / f64::from(denominator);
                if self.scalar_zero(ratio - candidate, ratio, candidate) {
                    let divisor = greatest_common_divisor(numerator, denominator);
                    return Some((numerator / divisor, denominator / divisor));
                }
            }
        }
        None
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

    fn length_nonnegative(self, value: f64) -> bool {
        value / self.diameter >= -self.epsilon
    }
}

/// Derives assertion and inspection tolerance from the same solved geometry and source fallbacks.
pub(crate) fn resolved_scene_tolerance(file: &File, scene: &ResolvedScene) -> Tolerance {
    let dimensions = infer_dimensions(file).unwrap_or_default();
    let mut environment = Environment::new();
    for (id, object) in &scene.objects {
        let _ = environment.insert(id.clone(), object.clone());
    }
    let mut fallbacks = Vec::new();
    for op in file.statements.iter().filter_map(operation) {
        let tolerance = Tolerance::from_bbox_with_fallbacks(&scene.bbox, &fallbacks);
        let mut evaluator = ExpressionEvaluator::new(&environment, tolerance, &dimensions);
        fallbacks.extend(length_fallbacks(&mut evaluator, op));
    }
    Tolerance::from_bbox_with_fallbacks(&scene.bbox, &fallbacks)
}

fn operation(statement: &Statement) -> Option<&OpStmt> {
    match statement {
        Statement::Op(op) => Some(op),
        Statement::Rel(_)
        | Statement::Assert(_)
        | Statement::Meta(_)
        | Statement::Style(_)
        | Statement::Class(_) => None,
    }
}

fn length_fallbacks(evaluator: &mut ExpressionEvaluator<'_>, op: &OpStmt) -> Vec<f64> {
    let mut values = Vec::new();
    collect_constructor_fallbacks(evaluator, op, &mut values);
    collect_constructor_length_literals(evaluator, &op.ctor, &mut values);
    for property in &op.props.items {
        if property_dimension(&property.name.name)
            != Some(InferredDimension::Number(Dimension::Length))
        {
            continue;
        }
        if let PropValue::Expr(expression) = &property.value
            && let Ok(value) = evaluator.eval_number(expression, Dimension::Length)
        {
            values.push(value);
        }
        if let PropValue::Expr(expression) = &property.value {
            collect_expression_length_literals(evaluator, expression, &mut values);
        }
    }
    values
}

fn collect_constructor_length_literals(
    evaluator: &mut ExpressionEvaluator<'_>,
    constructor: &Constructor,
    values: &mut Vec<f64>,
) {
    match constructor {
        Constructor::Call { args, .. } => collect_expression_list_literals(evaluator, args, values),
        Constructor::Expr { expr, .. } | Constructor::FreeNum { hint: expr, .. } => {
            collect_expression_length_literals(evaluator, expr, values);
        }
    }
}

fn collect_expression_length_literals(
    evaluator: &mut ExpressionEvaluator<'_>,
    expression: &Expr,
    values: &mut Vec<f64>,
) {
    if collect_length_literal(evaluator, expression, values) {
        return;
    }
    collect_compound_length_literals(evaluator, expression, values);
}

fn collect_length_literal(
    evaluator: &mut ExpressionEvaluator<'_>,
    expression: &Expr,
    values: &mut Vec<f64>,
) -> bool {
    let Expr::Number(_) = expression else {
        return false;
    };
    if evaluator.expression_dimension(expression)
        == Some(InferredDimension::Number(Dimension::Length))
    {
        push_evaluated_length(evaluator, expression, true, values);
    }
    true
}

fn collect_compound_length_literals(
    evaluator: &mut ExpressionEvaluator<'_>,
    expression: &Expr,
    values: &mut Vec<f64>,
) {
    match expression {
        Expr::Call { args, .. } => collect_expression_list_literals(evaluator, args, values),
        Expr::Property { base, args, .. } => {
            collect_expression_length_literals(evaluator, base, values);
            collect_expression_list_literals(evaluator, args, values);
        }
        Expr::Group { expr, .. } | Expr::Unary { expr, .. } => {
            collect_expression_length_literals(evaluator, expr, values);
        }
        Expr::Binary { lhs, rhs, .. } => {
            collect_expression_length_literals(evaluator, lhs, values);
            collect_expression_length_literals(evaluator, rhs, values);
        }
        Expr::Number(_) | Expr::Boolean { .. } | Expr::String(_) | Expr::Tex(_) | Expr::Ref(_) => {}
    }
}

fn collect_expression_list_literals(
    evaluator: &mut ExpressionEvaluator<'_>,
    expressions: &[Expr],
    values: &mut Vec<f64>,
) {
    for expression in expressions {
        collect_expression_length_literals(evaluator, expression, values);
    }
}

fn collect_constructor_fallbacks(
    evaluator: &mut ExpressionEvaluator<'_>,
    op: &OpStmt,
    values: &mut Vec<f64>,
) {
    match &op.ctor {
        Constructor::FreeNum { hint, .. } => {
            collect_named_length_fallback(evaluator, &op.id.name, hint, true, values);
        }
        Constructor::Expr { expr, .. } => {
            collect_named_length_fallback(evaluator, &op.id.name, expr, false, values);
        }
        Constructor::Call { name, args, .. } => {
            collect_call_length_fallbacks(evaluator, &name.name, args, values);
        }
    }
}

fn collect_named_length_fallback(
    evaluator: &mut ExpressionEvaluator<'_>,
    name: &str,
    expression: &locus_syntax::Expr,
    absolute: bool,
    values: &mut Vec<f64>,
) {
    if evaluator.dimension_of(name) == Some(InferredDimension::Number(Dimension::Length)) {
        push_evaluated_length(evaluator, expression, absolute, values);
    }
}

fn collect_call_length_fallbacks(
    evaluator: &mut ExpressionEvaluator<'_>,
    name: &str,
    args: &[locus_syntax::Expr],
    values: &mut Vec<f64>,
) {
    for (index, argument) in args.iter().enumerate() {
        if crate::eval::constructor_arg_dimension_for_arity(name, index, args.len())
            == Some(InferredDimension::Number(Dimension::Length))
        {
            push_evaluated_length(evaluator, argument, false, values);
        }
    }
}

fn push_evaluated_length(
    evaluator: &mut ExpressionEvaluator<'_>,
    expression: &locus_syntax::Expr,
    absolute: bool,
    values: &mut Vec<f64>,
) {
    if let Ok(value) = evaluator.eval_number(expression, Dimension::Length) {
        values.push(if absolute { value.abs() } else { value });
    }
}

fn normalized_zero(residual: f64, dimension_scale: f64, epsilon: f64) -> bool {
    residual.abs() / dimension_scale.max(residual.abs()) <= epsilon
}

fn fixed_scale_zero(residual: f64, dimension_scale: f64, epsilon: f64) -> bool {
    residual.abs() / dimension_scale <= epsilon
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

fn normalized_direction_dot(lhs: Vec2, rhs: Vec2) -> f64 {
    let norm_product = lhs.norm() * rhs.norm();
    if norm_product == 0.0 {
        f64::INFINITY
    } else {
        lhs.dot(rhs).abs() / norm_product
    }
}

fn normalized_length(value: f64, diameter: f64) -> f64 {
    value.abs() / diameter.max(value.abs())
}

fn measure_distance(lhs: crate::geom::Point, rhs: crate::geom::Point) -> f64 {
    (rhs - lhs).norm()
}

const fn greatest_common_divisor(mut lhs: u32, mut rhs: u32) -> u32 {
    while rhs != 0 {
        let remainder = lhs % rhs;
        lhs = rhs;
        rhs = remainder;
    }
    lhs
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
    fn zero_bbox_uses_largest_positive_fallback() {
        let bbox = BBox::from_points([Point::new(2.0, 2.0)]).expect("finite bbox");
        let tolerance = Tolerance::from_bbox_with_fallbacks(&bbox, &[0.0, 7.0, 9.0]);
        assert_eq!(tolerance.diameter(), 9.0);
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
