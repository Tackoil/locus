//! Post-resolution assertion evaluation with source-anchored actual values.

mod layout;
use std::collections::BTreeMap;

use locus_syntax::{AssertStmt, BinOp, Collection, Expr, File, Ident, Span, Statement, UnaryOp};
use serde::Serialize;

use crate::Tolerance;
use crate::eval::env::Environment;
use crate::eval::expr::{
    DimensionMap, Expected, ExpressionEvaluator, InferredDimension,
    angle_product_operand_expectations, product_operand_expectations,
};
use crate::eval::{EvalError, Value, infer_dimensions};
use crate::geom::construct;
use crate::geom::measure;
use crate::geom::{Arc, BBox, Line, Point, Polygon, Ray, Segment, Vec2, arc_contains_angle};
use crate::scene::{Dimension, ResolvedObject, ResolvedScene};
use crate::tol::resolved_scene_tolerance;

/// Results for every assertion in source order.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct AssertionReport {
    /// True only when every assertion passes.
    pub passed: bool,
    /// One result per source assertion, in file order.
    pub assertions: Vec<AssertionResult>,
}

/// The outcome and failure-localizing measurements for one assertion.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct AssertionResult {
    /// Zero-based index among assertion statements.
    pub index: usize,
    /// Full assertion statement span.
    pub span: Span,
    /// Effective normalized epsilon.
    pub tolerance: f64,
    /// Evaluated truth value.
    pub passed: bool,
    /// Every measurable expression subterm, populated on failure in post-order.
    pub actuals: Vec<SubtermActual>,
}

/// An evaluated measurable subterm and its exact source location.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct SubtermActual {
    /// Subterm source span.
    pub span: Span,
    /// Runtime value in source semantic units.
    #[serde(flatten)]
    pub actual: ActualValue,
}

/// Serializable runtime values useful for diagnosing an assertion.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ActualValue {
    /// Unitless, length, or area magnitude.
    Number {
        /// Finite magnitude.
        value: f64,
        /// Semantic numeric dimension.
        dimension: ReportDimension,
    },
    /// Angle in radians.
    Angle {
        /// Finite radian magnitude.
        radians: f64,
    },
    /// Predicate or boolean-combinator result.
    Boolean {
        /// Evaluated truth value.
        value: bool,
    },
}

/// Serializable counterpart of the evaluator's numeric dimension.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReportDimension {
    /// Unitless value.
    Scalar,
    /// Scene-space length.
    Length,
    /// Scene-space area.
    Area,
}

/// Evaluates all post-solve assertions against a resolved scene.
pub fn evaluate_assertions(
    file: &File,
    scene: &ResolvedScene,
) -> Result<AssertionReport, EvalError> {
    evaluate_assertions_with_inputs(file, scene, &crate::EvaluationInputs::default())
}

/// Evaluates assertions using the same measured inputs and preset as the solve.
pub fn evaluate_assertions_with_inputs(
    file: &File,
    scene: &ResolvedScene,
    inputs: &crate::EvaluationInputs,
) -> Result<AssertionReport, EvalError> {
    let expanded = crate::program::expand_program(file)?;
    evaluate_assertions_flat(&expanded.file, scene, &expanded.collections, inputs)
        .map_err(|error| crate::program::annotate_error(error, &expanded))
}

fn evaluate_assertions_flat(
    file: &File,
    scene: &ResolvedScene,
    collections: &BTreeMap<String, Collection>,
    inputs: &crate::EvaluationInputs,
) -> Result<AssertionReport, EvalError> {
    let dimensions = infer_dimensions(file)?;
    let mut environment = environment_from_scene(scene)?;
    let base_tolerance = resolved_scene_tolerance(file, scene);
    let aliases = crate::program::alias_identities(file);
    environment.set_aliases(&aliases);
    let mut assertions = Vec::new();
    for assertion in file.statements.iter().filter_map(assertion_statement) {
        assertions.push(evaluate_one(
            assertions.len(),
            assertion,
            &environment,
            &dimensions,
            base_tolerance,
            (collections, &aliases),
            inputs,
        )?);
    }
    Ok(AssertionReport {
        passed: assertions.iter().all(|result| result.passed),
        assertions,
    })
}

fn assertion_statement(statement: &Statement) -> Option<&AssertStmt> {
    if let Statement::Assert(assertion) = statement {
        Some(assertion)
    } else {
        None
    }
}

fn environment_from_scene(scene: &ResolvedScene) -> Result<Environment, EvalError> {
    let mut environment = Environment::new();
    for (id, object) in &scene.objects {
        environment
            .insert(id.clone(), object.clone())
            .map_err(|error| EvalError::new("E302", id.span, error.to_string()))?;
    }
    Ok(environment)
}

fn evaluate_one(
    index: usize,
    assertion: &AssertStmt,
    environment: &Environment,
    dimensions: &DimensionMap,
    base_tolerance: Tolerance,
    collection_context: (&BTreeMap<String, Collection>, &BTreeMap<String, String>),
    inputs: &crate::EvaluationInputs,
) -> Result<AssertionResult, EvalError> {
    let tolerance = assertion_tolerance(assertion, base_tolerance)?;
    let mut context = AssertionContext {
        expressions: ExpressionEvaluator::new(environment, tolerance, dimensions)
            .with_metrics(inputs.metrics),
        tolerance,
        collections: collection_context.0,
        aliases: collection_context.1,
        padding: inputs.metrics.padding,
        metrics: inputs.metrics,
        choices: &inputs.choices,
    };
    let passed = context.eval_boolean(&assertion.expr)?;
    let actuals = if passed {
        Vec::new()
    } else {
        context.collect_actuals(&assertion.expr)?
    };
    Ok(AssertionResult {
        index,
        span: assertion.span,
        tolerance: tolerance.epsilon(),
        passed,
        actuals,
    })
}

fn assertion_tolerance(assertion: &AssertStmt, base: Tolerance) -> Result<Tolerance, EvalError> {
    let Some(custom) = &assertion.tolerance else {
        return Ok(base);
    };
    Tolerance::with_epsilon(base.diameter(), custom.value).ok_or_else(|| {
        EvalError::new(
            "E209",
            custom.span,
            "assert tolerance must be a finite positive scalar literal",
        )
    })
}

struct AssertionContext<'a> {
    expressions: ExpressionEvaluator<'a>,
    tolerance: Tolerance,
    collections: &'a BTreeMap<String, Collection>,
    aliases: &'a BTreeMap<String, String>,
    padding: f64,
    metrics: crate::preset::LayoutMetrics,
    choices: &'a crate::EvaluationChoices,
}

impl AssertionContext<'_> {
    fn eval_boolean(&mut self, expression: &Expr) -> Result<bool, EvalError> {
        if let Expr::Group { expr, .. } = expression {
            return self.eval_boolean(expr);
        }
        if let Expr::Unary {
            op: UnaryOp::Not,
            expr,
            ..
        } = expression
        {
            return self.eval_boolean(expr).map(|value| !value);
        }
        if let Expr::Binary { op, lhs, rhs, .. } = expression
            && matches!(op, BinOp::And | BinOp::Or)
        {
            return self.eval_logical(*op, lhs, rhs);
        }
        if let Expr::Call { name, args, span } = expression
            && is_predicate(&name.name)
        {
            return self.eval_predicate_by_name(&name.name, args, *span);
        }
        match self.expressions.eval_as(expression, Expected::Bool)? {
            Value::Bool(value) => Ok(value),
            _ => Err(assertion_error(
                expression.span(),
                "assert expression must be boolean",
            )),
        }
    }

    fn eval_logical(&mut self, op: BinOp, lhs: &Expr, rhs: &Expr) -> Result<bool, EvalError> {
        let lhs = self.eval_boolean(lhs)?;
        let rhs = self.eval_boolean(rhs)?;
        Ok(if op == BinOp::And {
            lhs && rhs
        } else {
            lhs || rhs
        })
    }

    fn collect_actuals(&mut self, expression: &Expr) -> Result<Vec<SubtermActual>, EvalError> {
        let mut actuals = Vec::new();
        self.walk_actuals(expression, Some(Expected::Bool), &mut actuals)?;
        Ok(actuals)
    }

    fn walk_actuals(
        &mut self,
        expression: &Expr,
        expected: Option<Expected>,
        actuals: &mut Vec<SubtermActual>,
    ) -> Result<(), EvalError> {
        if self.is_collection(expression) {
            return Ok(());
        }
        visit_children(self, expression, expected, actuals)?;
        if let Some(actual) = self.actual_value(expression, expected)? {
            actuals.push(SubtermActual {
                span: expression.span(),
                actual,
            });
        }
        Ok(())
    }

    fn is_collection(&self, expression: &Expr) -> bool {
        match expression {
            Expr::Ref(id) => self.collections.contains_key(&id.name),
            Expr::Group { expr, .. } => self.is_collection(expr),
            _ => false,
        }
    }

    fn actual_value(
        &mut self,
        expression: &Expr,
        expected: Option<Expected>,
    ) -> Result<Option<ActualValue>, EvalError> {
        if is_boolean_expression(expression, expected) {
            return self
                .eval_boolean(expression)
                .map(|value| Some(ActualValue::Boolean { value }));
        }
        let value = if let Some(expected) = expected {
            self.expressions.eval_as(expression, expected)
        } else {
            self.expressions.eval(expression)
        };
        match value {
            Ok(Value::Scalar(number)) => Ok(Some(ActualValue::Number {
                value: number.value,
                dimension: number.dimension.into(),
            })),
            Ok(Value::Angle(radians)) => Ok(Some(ActualValue::Angle { radians })),
            Ok(Value::Bool(value)) => Ok(Some(ActualValue::Boolean { value })),
            Ok(Value::ObjectRef(_)) => Ok(None),
            Err(_error) if expression_may_need_context(expression) => Ok(None),
            Err(error) => Err(error),
        }
    }

    fn matching_expected(&mut self, lhs: &Expr, rhs: &Expr) -> Option<Expected> {
        let lhs_value = self.expressions.eval(lhs).ok();
        let rhs_value = self.expressions.eval(rhs).ok();
        if let Some(expected) = rhs_value.as_ref().map(expected_for_value)
            && self.expressions.eval_as(lhs, expected).is_ok()
        {
            return Some(expected);
        }
        lhs_value.as_ref().map(expected_for_value)
    }

    fn product_operand_expectations(
        &mut self,
        op: BinOp,
        lhs: &Expr,
        rhs: &Expr,
        expected: Option<Expected>,
    ) -> [Option<Expected>; 2] {
        let Some(expected) = expected else {
            return [None, None];
        };
        let candidates = match expected {
            Expected::Angle => angle_product_operand_expectations(op),
            Expected::Number(dimension) => product_operand_expectations(op, dimension),
            Expected::Bool | Expected::Object => return [None, None],
        };
        for &(lhs_expected, rhs_expected) in candidates {
            if self.expressions.eval_as(lhs, lhs_expected).is_ok()
                && self.expressions.eval_as(rhs, rhs_expected).is_ok()
            {
                return [Some(lhs_expected), Some(rhs_expected)];
            }
        }
        [None, None]
    }

    fn eval_predicate_by_name(
        &mut self,
        name: &str,
        args: &[Expr],
        span: Span,
    ) -> Result<bool, EvalError> {
        if layout::is_layout(name) {
            return self.eval_layout(name, args, span);
        }
        if name == "center" {
            return self.eval_center(args, span);
        }
        if is_alignment(name) {
            return self.eval_alignment(name, args, span);
        }
        if is_inset(name) {
            return self.eval_inset(name, args, span);
        }
        self.eval_geometric_predicate(name, args, span)
    }

    fn eval_geometric_predicate(
        &mut self,
        name: &str,
        args: &[Expr],
        span: Span,
    ) -> Result<bool, EvalError> {
        if name == "on" {
            return self.eval_on(args, span);
        }
        if name == "tangent" || name == "internally_tangent" {
            return self.eval_tangent(name, args, span);
        }
        if name == "parallel" || name == "perpendicular" {
            return self.eval_direction(name, args, span);
        }
        if name == "collinear" {
            return self.eval_collinear(args, span);
        }
        if name == "concyclic" {
            return self.eval_concyclic(args, span);
        }
        if name == "congruent" {
            return self.eval_congruent(args, span);
        }
        if name == "coincident" {
            return self.eval_coincident(args, span);
        }
        if name == "midpoint" {
            return self.eval_midpoint(args, span);
        }
        if name == "horizontal" || name == "vertical" {
            return self.eval_axis(name, args, span);
        }
        if name == "between" {
            return self.eval_between(args, span);
        }
        self.eval_inside(args, span)
    }

    fn eval_on(&mut self, args: &[Expr], span: Span) -> Result<bool, EvalError> {
        require_arity("on", args, 2, span)?;
        let point = self.expressions.eval_point(&args[0])?;
        let object = self.expressions.eval_resolved_object(&args[1])?;
        if !matches!(
            object,
            ResolvedObject::Line(_)
                | ResolvedObject::Ray(_)
                | ResolvedObject::Segment(_)
                | ResolvedObject::Circle(_)
                | ResolvedObject::Arc(_)
                | ResolvedObject::Ellipse(_)
                | ResolvedObject::RoundedRect(_)
        ) {
            return Err(assertion_error(
                args[1].span(),
                "on target must be linear, circle, arc, ellipse, or rounded rectangle",
            ));
        }
        point_on_object(point, &object, self.tolerance, span)
    }

    fn eval_tangent(&mut self, name: &str, args: &[Expr], span: Span) -> Result<bool, EvalError> {
        require_arity(name, args, 2, span)?;
        let lhs = self.expressions.eval_resolved_object(&args[0])?;
        let rhs = self.expressions.eval_resolved_object(&args[1])?;
        if name == "internally_tangent" {
            internally_tangent_objects(&lhs, &rhs, self.tolerance, span)
        } else {
            external_tangent_objects(&lhs, &rhs, self.tolerance, span)
        }
    }

    fn eval_direction(&mut self, name: &str, args: &[Expr], span: Span) -> Result<bool, EvalError> {
        require_arity(name, args, 2, span)?;
        let lhs_object = self.expressions.eval_resolved_object(&args[0])?;
        let rhs_object = self.expressions.eval_resolved_object(&args[1])?;
        let lhs = linear_direction(&lhs_object, args[0].span())?;
        let rhs = linear_direction(&rhs_object, args[1].span())?;
        if self.tolerance.construction_length_zero(lhs.norm())
            || self.tolerance.construction_length_zero(rhs.norm())
        {
            return Err(degenerate(
                span,
                "direction predicate has a degenerate carrier",
            ));
        }
        Ok(if name == "parallel" {
            self.tolerance.directions_parallel(lhs, rhs)
        } else {
            self.tolerance.directions_perpendicular(lhs, rhs)
        })
    }

    fn eval_collinear(&mut self, args: &[Expr], span: Span) -> Result<bool, EvalError> {
        if args.len() < 3 {
            return Err(assertion_error(
                span,
                "collinear requires at least three points",
            ));
        }
        let points = self.expressions.eval_points(args)?;
        let direction = points[1] - points[0];
        if self.tolerance.construction_length_zero(direction.norm()) {
            return Err(degenerate(span, "first two collinear points coincide"));
        }
        Ok(points[2..].iter().all(|point| {
            self.tolerance
                .vector_on_direction(*point - points[0], direction)
        }))
    }

    fn eval_concyclic(&mut self, args: &[Expr], span: Span) -> Result<bool, EvalError> {
        if args.len() < 4 {
            return Err(assertion_error(
                span,
                "concyclic requires at least four points",
            ));
        }
        let points = self.expressions.eval_points(args)?;
        let circle = construct::circumcircle(points[0], points[1], points[2], self.tolerance)
            .map_err(|error| EvalError::construction(span, error))?;
        Ok(points[3..].iter().all(|point| {
            self.tolerance
                .length_eq(measure::dist(circle.center, *point), circle.radius)
        }))
    }

    fn eval_congruent(&mut self, args: &[Expr], span: Span) -> Result<bool, EvalError> {
        require_arity("congruent", args, 2, span)?;
        let lhs_object = self.expressions.eval_resolved_object(&args[0])?;
        let rhs_object = self.expressions.eval_resolved_object(&args[1])?;
        let lhs = expect_segment(&lhs_object, args[0].span())?;
        let rhs = expect_segment(&rhs_object, args[1].span())?;
        Ok(self
            .tolerance
            .length_eq(measure::length(lhs), measure::length(rhs)))
    }

    fn eval_coincident(&mut self, args: &[Expr], span: Span) -> Result<bool, EvalError> {
        require_arity("coincident", args, 2, span)?;
        let lhs = self.expressions.eval_point(&args[0])?;
        let rhs = self.expressions.eval_point(&args[1])?;
        Ok(self.tolerance.points_eq(lhs, rhs))
    }

    fn eval_midpoint(&mut self, args: &[Expr], span: Span) -> Result<bool, EvalError> {
        require_arity("midpoint", args, 3, span)?;
        let points = self.expressions.eval_points(args)?;
        let expected = Point::new(
            points[1].x + (points[2].x - points[1].x) * 0.5,
            points[1].y + (points[2].y - points[1].y) * 0.5,
        );
        Ok(self.tolerance.points_eq(points[0], expected))
    }

    fn eval_axis(&mut self, name: &str, args: &[Expr], span: Span) -> Result<bool, EvalError> {
        require_arity(name, args, 2, span)?;
        let points = self.expressions.eval_points(args)?;
        let residual = if name == "horizontal" {
            points[1].y - points[0].y
        } else {
            points[1].x - points[0].x
        };
        Ok(self.tolerance.length_zero(residual))
    }

    fn eval_center(&mut self, args: &[Expr], span: Span) -> Result<bool, EvalError> {
        require_arity("center", args, 2, span)?;
        let lhs = self.measured_bounds(&args[0])?.center();
        let rhs = self.measured_bounds(&args[1])?.center();
        Ok(self.tolerance.points_eq(lhs, rhs))
    }

    fn eval_alignment(&mut self, name: &str, args: &[Expr], span: Span) -> Result<bool, EvalError> {
        if args.len() < 2 {
            return Err(assertion_error(
                span,
                format!("`{name}` requires at least two objects"),
            ));
        }
        let reference = alignment_component(self.measured_bounds(&args[0])?, name);
        let mut passed = true;
        for argument in &args[1..] {
            let component = alignment_component(self.measured_bounds(argument)?, name);
            passed &= self.tolerance.length_zero(component - reference);
        }
        Ok(passed)
    }

    fn measured_bounds(&mut self, expression: &Expr) -> Result<BBox, EvalError> {
        if let Expr::Group { expr, .. } = expression {
            return self.measured_bounds(expr);
        }
        if let Expr::Ref(id) = expression
            && let Some(collection) = self.collections.get(&id.name)
        {
            return self.collection_bounds(&collection.members, expression.span());
        }
        self.expressions
            .eval_resolved_object(expression)?
            .bbox()
            .ok_or_else(|| {
                assertion_error(expression.span(), "expected an object with measured bounds")
            })
    }

    fn collection_bounds(&mut self, members: &[String], span: Span) -> Result<BBox, EvalError> {
        let mut result: Option<BBox> = None;
        for member in members {
            let expression = Expr::Ref(Ident {
                name: member.clone(),
                span,
            });
            let bounds = self.measured_bounds(&expression)?;
            if let Some(current) = &mut result {
                current.include_bbox(bounds);
            } else {
                result = Some(bounds);
            }
        }
        result.ok_or_else(|| assertion_error(span, "collection has no measured members"))
    }

    fn eval_inset(&mut self, name: &str, args: &[Expr], span: Span) -> Result<bool, EvalError> {
        require_arity(name, args, 2, span)?;
        let alignment = inset_alignment(name);
        let content = alignment_component(self.measured_bounds(&args[0])?, alignment);
        let container = alignment_component(self.measured_bounds(&args[1])?, alignment);
        let direction = if matches!(name, "inset_left" | "inset_bottom") {
            1.0
        } else {
            -1.0
        };
        Ok(self
            .tolerance
            .length_zero(content - container - direction * self.padding))
    }

    fn eval_between(&mut self, args: &[Expr], span: Span) -> Result<bool, EvalError> {
        require_arity("between", args, 3, span)?;
        let points = self.expressions.eval_points(args)?;
        let direction = points[2] - points[0];
        if self.tolerance.construction_length_zero(direction.norm()) {
            return Err(degenerate(span, "between endpoints coincide"));
        }
        let offset = points[1] - points[0];
        if !self.tolerance.vector_on_direction(offset, direction) {
            return Ok(false);
        }
        let parameter = offset.dot(direction) / direction.norm_squared();
        Ok(self.tolerance.parameter_strictly_inside_unit(parameter))
    }

    fn eval_inside(&mut self, args: &[Expr], span: Span) -> Result<bool, EvalError> {
        require_arity("inside", args, 2, span)?;
        let point = self.expressions.eval_point(&args[0])?;
        let object = self.expressions.eval_resolved_object(&args[1])?;
        match object {
            ResolvedObject::Circle(circle) => Ok(self
                .tolerance
                .length_strictly_positive(circle.radius - measure::dist(circle.center, point))),
            ResolvedObject::Ellipse(ellipse) => Ok(self
                .tolerance
                .length_strictly_positive(-point_ellipse_residual(point, ellipse))),
            ResolvedObject::RoundedRect(rect) => Ok(self
                .tolerance
                .length_strictly_positive(-rect.signed_distance(point))),
            ResolvedObject::Polygon(polygon)
            | ResolvedObject::Rect(polygon)
            | ResolvedObject::Diamond(polygon) => Ok(point_strictly_inside_polygon(
                point,
                &polygon,
                self.tolerance,
            )),
            _ => Err(assertion_error(
                args[1].span(),
                "inside target must be a circle, ellipse, rounded rectangle, or polygon",
            )),
        }
    }
}

impl From<Dimension> for ReportDimension {
    fn from(value: Dimension) -> Self {
        match value {
            Dimension::Scalar => Self::Scalar,
            Dimension::Length => Self::Length,
            Dimension::Area => Self::Area,
        }
    }
}

fn visit_children(
    context: &mut AssertionContext<'_>,
    expression: &Expr,
    expected: Option<Expected>,
    actuals: &mut Vec<SubtermActual>,
) -> Result<(), EvalError> {
    match expression {
        Expr::Call { name, args, .. } => {
            visit_call_children(context, &name.name, args, expected, actuals)
        }
        Expr::Property { base, args, .. } => visit_property_children(context, base, args, actuals),
        Expr::Group { expr, .. } => context.walk_actuals(expr, expected, actuals),
        Expr::Unary { op, expr, .. } => visit_unary_child(context, *op, expr, expected, actuals),
        Expr::Binary { op, lhs, rhs, .. } => {
            visit_binary_children(context, *op, lhs, rhs, expected, actuals)
        }
        Expr::Number(_) | Expr::Boolean { .. } | Expr::String(_) | Expr::Tex(_) | Expr::Ref(_) => {
            Ok(())
        }
    }
}

fn visit_property_children(
    context: &mut AssertionContext<'_>,
    base: &Expr,
    args: &[Expr],
    actuals: &mut Vec<SubtermActual>,
) -> Result<(), EvalError> {
    context.walk_actuals(base, None, actuals)?;
    visit_list(context, args, actuals)
}

fn visit_unary_child(
    context: &mut AssertionContext<'_>,
    op: UnaryOp,
    expression: &Expr,
    expected: Option<Expected>,
    actuals: &mut Vec<SubtermActual>,
) -> Result<(), EvalError> {
    let operand_expected = match op {
        UnaryOp::Neg => expected,
        UnaryOp::Not => Some(Expected::Bool),
    };
    context.walk_actuals(expression, operand_expected, actuals)
}

fn visit_binary_children(
    context: &mut AssertionContext<'_>,
    op: BinOp,
    lhs: &Expr,
    rhs: &Expr,
    expected: Option<Expected>,
    actuals: &mut Vec<SubtermActual>,
) -> Result<(), EvalError> {
    let [lhs_expected, rhs_expected] = binary_operand_expectations(context, op, lhs, rhs, expected);
    context.walk_actuals(lhs, lhs_expected, actuals)?;
    context.walk_actuals(rhs, rhs_expected, actuals)
}

fn visit_list(
    context: &mut AssertionContext<'_>,
    expressions: &[Expr],
    actuals: &mut Vec<SubtermActual>,
) -> Result<(), EvalError> {
    for expression in expressions {
        context.walk_actuals(expression, None, actuals)?;
    }
    Ok(())
}

fn visit_call_children(
    context: &mut AssertionContext<'_>,
    name: &str,
    args: &[Expr],
    expected: Option<Expected>,
    actuals: &mut Vec<SubtermActual>,
) -> Result<(), EvalError> {
    if name == "edge" {
        return visit_list(context, args.get(..1).unwrap_or_default(), actuals);
    }
    if name == "abs" && args.len() == 1 {
        return context.walk_actuals(&args[0], expected, actuals);
    }
    if matches!(name, "min" | "max") && args.len() == 2 {
        let operand_expected = expected.or_else(|| context.matching_expected(&args[0], &args[1]));
        context.walk_actuals(&args[0], operand_expected, actuals)?;
        return context.walk_actuals(&args[1], operand_expected, actuals);
    }
    if name == "sqrt" && args.len() == 1 {
        return context.walk_actuals(&args[0], sqrt_operand_expected(expected), actuals);
    }
    if name == "ratio" && args.len() == 2 {
        let operand_expected = context.matching_expected(&args[0], &args[1]);
        context.walk_actuals(&args[0], operand_expected, actuals)?;
        return context.walk_actuals(&args[1], operand_expected, actuals);
    }
    for (index, argument) in args.iter().enumerate() {
        let expected = crate::eval::constructor_arg_dimension_for_arity(name, index, args.len())
            .map(expected_for_inferred);
        context.walk_actuals(argument, expected, actuals)?;
    }
    Ok(())
}

fn binary_operand_expectations(
    context: &mut AssertionContext<'_>,
    op: BinOp,
    lhs: &Expr,
    rhs: &Expr,
    expected: Option<Expected>,
) -> [Option<Expected>; 2] {
    match op {
        BinOp::Add | BinOp::Sub => {
            let expected = expected.or_else(|| context.matching_expected(lhs, rhs));
            [expected, expected]
        }
        BinOp::Eq | BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge => {
            let expected = context.matching_expected(lhs, rhs);
            [expected, expected]
        }
        BinOp::Mul | BinOp::Div => context.product_operand_expectations(op, lhs, rhs, expected),
        BinOp::And | BinOp::Or => [Some(Expected::Bool), Some(Expected::Bool)],
    }
}

const fn expected_for_inferred(dimension: InferredDimension) -> Expected {
    match dimension {
        InferredDimension::Number(dimension) => Expected::Number(dimension),
        InferredDimension::Angle => Expected::Angle,
    }
}

fn expected_for_value(value: &Value) -> Expected {
    match value {
        Value::Scalar(number) => Expected::Number(number.dimension),
        Value::Angle(_) => Expected::Angle,
        Value::Bool(_) => Expected::Bool,
        Value::ObjectRef(_) => Expected::Object,
    }
}

fn sqrt_operand_expected(expected: Option<Expected>) -> Option<Expected> {
    match expected {
        Some(Expected::Number(Dimension::Length)) => Some(Expected::Number(Dimension::Area)),
        Some(Expected::Number(Dimension::Scalar)) => Some(Expected::Number(Dimension::Scalar)),
        _ => None,
    }
}

fn expression_may_need_context(expression: &Expr) -> bool {
    matches!(
        expression,
        Expr::Number(_) | Expr::Group { .. } | Expr::String(_) | Expr::Tex(_)
    )
}

fn is_boolean_expression(expression: &Expr, expected: Option<Expected>) -> bool {
    match expression {
        Expr::Boolean { .. }
        | Expr::Unary {
            op: UnaryOp::Not, ..
        } => true,
        Expr::Call { name, .. } => {
            matches!(expected, Some(Expected::Bool)) && is_predicate(&name.name)
        }
        Expr::Binary { op, .. } => matches!(
            op,
            BinOp::Eq | BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge | BinOp::And | BinOp::Or
        ),
        Expr::Group { expr, .. } => is_boolean_expression(expr, expected),
        _ => false,
    }
}

fn is_predicate(name: &str) -> bool {
    layout::is_layout(name)
        || matches!(
            name,
            "on" | "tangent"
                | "internally_tangent"
                | "parallel"
                | "perpendicular"
                | "collinear"
                | "concyclic"
                | "congruent"
                | "coincident"
                | "center"
                | "align_center_x"
                | "align_center_y"
                | "align_left"
                | "align_right"
                | "align_top"
                | "align_bottom"
                | "inset_left"
                | "inset_right"
                | "inset_top"
                | "inset_bottom"
                | "midpoint"
                | "horizontal"
                | "vertical"
                | "between"
                | "inside"
        )
}

fn alignment_component(bounds: BBox, name: &str) -> f64 {
    match name {
        "align_center_x" => bounds.center().x,
        "align_center_y" => bounds.center().y,
        "align_left" => bounds.min.x,
        "align_right" => bounds.max.x,
        "align_bottom" => bounds.min.y,
        _ => bounds.max.y,
    }
}

fn is_alignment(name: &str) -> bool {
    matches!(
        name,
        "align_center_x"
            | "align_center_y"
            | "align_left"
            | "align_right"
            | "align_top"
            | "align_bottom"
    )
}

fn is_inset(name: &str) -> bool {
    matches!(
        name,
        "inset_left" | "inset_right" | "inset_top" | "inset_bottom"
    )
}

fn inset_alignment(name: &str) -> &str {
    match name {
        "inset_left" => "align_left",
        "inset_right" => "align_right",
        "inset_bottom" => "align_bottom",
        _ => "align_top",
    }
}

pub(crate) fn point_on_object(
    point: Point,
    object: &ResolvedObject,
    tolerance: Tolerance,
    span: Span,
) -> Result<bool, EvalError> {
    let residual = match object {
        ResolvedObject::Line(line) => {
            point_linear_distance(point, Linear::Line(*line), tolerance, span)?
        }
        ResolvedObject::Ray(ray) => {
            point_linear_distance(point, Linear::Ray(*ray), tolerance, span)?
        }
        ResolvedObject::Segment(segment) => {
            point_linear_distance(point, Linear::Segment(*segment), tolerance, span)?
        }
        ResolvedObject::Circle(circle) => {
            (measure::dist(point, circle.center) - circle.radius).abs()
        }
        ResolvedObject::Arc(arc) => point_arc_distance(point, arc.geometry, tolerance),
        ResolvedObject::Ellipse(ellipse) => point_ellipse_residual(point, *ellipse).abs(),
        object => return point_on_closed_shape(point, object, tolerance, span),
    };
    Ok(tolerance.length_zero(residual))
}

fn point_on_closed_shape(
    point: Point,
    object: &ResolvedObject,
    tolerance: Tolerance,
    span: Span,
) -> Result<bool, EvalError> {
    let residual = match object {
        ResolvedObject::RoundedRect(rect) => rect.signed_distance(point).abs(),
        ResolvedObject::Polygon(polygon)
        | ResolvedObject::Rect(polygon)
        | ResolvedObject::Diamond(polygon) => {
            point_polygon_edge_distance(point, polygon, tolerance, span)?
        }
        _ => {
            return Err(assertion_error(
                span,
                "on target must be linear, circle, arc, or polygon",
            ));
        }
    };
    Ok(tolerance.length_zero(residual))
}

fn point_ellipse_residual(point: Point, ellipse: crate::geom::Ellipse) -> f64 {
    let offset = point - ellipse.center;
    let local_x = offset.dot(ellipse.major_unit());
    let local_y = offset.dot(ellipse.minor_unit());
    let normalized_x = local_x / ellipse.semimajor;
    let normalized_y = local_y / ellipse.semiminor;
    ellipse.semiminor
        * (libm::sqrt(normalized_x * normalized_x + normalized_y * normalized_y) - 1.0)
}

fn external_tangent_objects(
    lhs: &ResolvedObject,
    rhs: &ResolvedObject,
    tolerance: Tolerance,
    span: Span,
) -> Result<bool, EvalError> {
    match (lhs, rhs) {
        (ResolvedObject::Circle(lhs), ResolvedObject::Circle(rhs)) => {
            Ok(tolerance
                .length_zero(measure::dist(lhs.center, rhs.center) - lhs.radius - rhs.radius))
        }
        _ => tangent_objects(lhs, rhs, tolerance, span),
    }
}

fn internally_tangent_objects(
    outer: &ResolvedObject,
    inner: &ResolvedObject,
    tolerance: Tolerance,
    span: Span,
) -> Result<bool, EvalError> {
    let (ResolvedObject::Circle(outer), ResolvedObject::Circle(inner)) = (outer, inner) else {
        return Err(assertion_error(
            span,
            "internally_tangent requires two circles",
        ));
    };
    if outer.radius <= inner.radius {
        return Err(degenerate(
            span,
            "internal tangency requires the outer radius to exceed the inner radius",
        ));
    }
    Ok(tolerance
        .length_zero(measure::dist(outer.center, inner.center) - (outer.radius - inner.radius)))
}

pub(crate) fn tangent_objects(
    lhs: &ResolvedObject,
    rhs: &ResolvedObject,
    tolerance: Tolerance,
    span: Span,
) -> Result<bool, EvalError> {
    match (lhs, rhs) {
        (ResolvedObject::Circle(lhs), ResolvedObject::Circle(rhs)) => {
            let distance = measure::dist(lhs.center, rhs.center);
            let external = (distance - lhs.radius - rhs.radius).abs();
            let radius_difference = (lhs.radius - rhs.radius).abs();
            let internal = (distance - radius_difference).abs();
            Ok(tolerance.length_zero(external)
                || radius_difference > 0.0 && tolerance.length_zero(internal))
        }
        (ResolvedObject::Circle(circle), linear) | (linear, ResolvedObject::Circle(circle)) => {
            let distance = circle_linear_distance(circle.center, linear, tolerance, span)?;
            Ok(tolerance.length_eq(distance, circle.radius))
        }
        _ => Err(assertion_error(
            span,
            "tangent requires circle-circle or circle-linear operands",
        )),
    }
}

fn circle_linear_distance(
    center: Point,
    object: &ResolvedObject,
    tolerance: Tolerance,
    span: Span,
) -> Result<f64, EvalError> {
    match object {
        ResolvedObject::Line(line) => {
            point_linear_distance(center, Linear::Line(*line), tolerance, span)
        }
        ResolvedObject::Ray(ray) => {
            point_linear_distance(center, Linear::Ray(*ray), tolerance, span)
        }
        ResolvedObject::Segment(segment) => {
            point_linear_distance(center, Linear::Segment(*segment), tolerance, span)
        }
        _ => Err(assertion_error(span, "expected a linear object")),
    }
}

#[derive(Clone, Copy)]
enum Linear {
    Line(Line),
    Ray(Ray),
    Segment(Segment),
}

fn point_linear_distance(
    point: Point,
    linear: Linear,
    tolerance: Tolerance,
    span: Span,
) -> Result<f64, EvalError> {
    let (start, through, domain) = match linear {
        Linear::Line(line) => (line.start, line.through, LinearDomain::Unbounded),
        Linear::Ray(ray) => (ray.start, ray.through, LinearDomain::Ray),
        Linear::Segment(segment) => (segment.start, segment.end, LinearDomain::Segment),
    };
    let direction = through - start;
    if tolerance.construction_length_zero(direction.norm()) {
        return Err(degenerate(span, "linear object is degenerate"));
    }
    let squared = direction.norm_squared();
    let parameter = (point - start).dot(direction) / squared;
    let clamped = match domain {
        LinearDomain::Unbounded => parameter,
        LinearDomain::Ray => parameter.max(0.0),
        LinearDomain::Segment => parameter.clamp(0.0, 1.0),
    };
    Ok(measure::dist(point, start + direction * clamped))
}

#[derive(Clone, Copy)]
enum LinearDomain {
    Unbounded,
    Ray,
    Segment,
}

fn point_arc_distance(point: Point, arc: Arc, tolerance: Tolerance) -> f64 {
    let angle = libm::atan2(point.y - arc.circle.center.y, point.x - arc.circle.center.x);
    if arc_contains_angle(arc, angle, tolerance) {
        (measure::dist(point, arc.circle.center) - arc.circle.radius).abs()
    } else {
        measure::dist(point, arc.start).min(measure::dist(point, arc.end))
    }
}

fn point_polygon_edge_distance(
    point: Point,
    polygon: &Polygon,
    tolerance: Tolerance,
    span: Span,
) -> Result<f64, EvalError> {
    polygon_edges(polygon)
        .map(|segment| point_linear_distance(point, Linear::Segment(segment), tolerance, span))
        .try_fold(f64::INFINITY, |closest, distance| {
            distance.map(|value| closest.min(value))
        })
}

fn point_strictly_inside_polygon(point: Point, polygon: &Polygon, tolerance: Tolerance) -> bool {
    if polygon_edges(polygon).any(|edge| {
        point_linear_distance(point, Linear::Segment(edge), tolerance, Span::default())
            .is_ok_and(|distance| tolerance.length_zero(distance))
    }) {
        return false;
    }
    winding_number(point, polygon) != 0
}

fn winding_number(point: Point, polygon: &Polygon) -> i32 {
    let mut winding = 0;
    for edge in polygon_edges(polygon) {
        let cross = (edge.end - edge.start).cross(point - edge.start);
        if edge.start.y <= point.y && edge.end.y > point.y && cross > 0.0 {
            winding += 1;
        } else if edge.start.y > point.y && edge.end.y <= point.y && cross < 0.0 {
            winding -= 1;
        }
    }
    winding
}

fn polygon_edges(polygon: &Polygon) -> impl Iterator<Item = Segment> + '_ {
    let vertices = polygon.vertices();
    vertices
        .iter()
        .copied()
        .zip(vertices.iter().copied().cycle().skip(1))
        .take(vertices.len())
        .map(|(start, end)| Segment { start, end })
}

fn linear_direction(object: &ResolvedObject, span: Span) -> Result<Vec2, EvalError> {
    match object {
        ResolvedObject::Line(line) => Ok(line.direction()),
        ResolvedObject::Ray(ray) => Ok(ray.direction()),
        ResolvedObject::Segment(segment) => Ok(segment.direction()),
        _ => Err(assertion_error(span, "expected a linear object")),
    }
}

fn expect_segment(object: &ResolvedObject, span: Span) -> Result<Segment, EvalError> {
    if let ResolvedObject::Segment(segment) = object {
        Ok(*segment)
    } else {
        Err(assertion_error(span, "expected a segment"))
    }
}

fn require_arity(name: &str, args: &[Expr], count: usize, span: Span) -> Result<(), EvalError> {
    if args.len() == count {
        Ok(())
    } else {
        Err(assertion_error(
            span,
            format!("`{name}` expects {count} arguments, found {}", args.len()),
        ))
    }
}

fn assertion_error(span: Span, message: impl Into<String>) -> EvalError {
    EvalError::new("E209", span, message)
}

fn degenerate(span: Span, message: impl Into<String>) -> EvalError {
    EvalError::new("E203", span, message)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic)]
mod tests {
    use std::collections::BTreeMap;

    use indexmap::IndexMap;
    use locus_syntax::parse;

    use super::*;
    use crate::geom::Circle;
    use crate::scene::{PointMark, ResolvedPoint};

    fn resolved(source: &str, values: Vec<(&str, ResolvedObject)>) -> (File, ResolvedScene) {
        let parsed = parse(source);
        assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
        let mut remaining = values
            .into_iter()
            .map(|(name, value)| (name.to_owned(), value))
            .collect::<BTreeMap<_, _>>();
        let mut objects = IndexMap::new();
        let mut bbox = None;
        for statement in &parsed.file.statements {
            let Statement::Op(op) = statement else {
                continue;
            };
            let Some(value) = remaining.remove(&op.id.name) else {
                continue;
            };
            if let Some(bounds) = value.bbox() {
                if let Some(current) = &mut bbox {
                    BBox::include_bbox(current, bounds);
                } else {
                    bbox = Some(bounds);
                }
            }
            objects.insert(op.id.clone(), value);
        }
        assert!(
            remaining.is_empty(),
            "unmatched fixture values: {remaining:?}"
        );
        let scene = ResolvedScene {
            objects,
            bbox: bbox.expect("fixture geometry bounds"),
        };
        (parsed.file, scene)
    }

    fn point(value: Point) -> ResolvedObject {
        ResolvedObject::Point(ResolvedPoint {
            geometry: value,
            mark: PointMark::Dot,
        })
    }

    #[test]
    fn measured_alignment_uses_bounds_and_supports_more_than_two_objects() {
        let tolerance = Tolerance::angular_default();
        let (file, scene) = resolved(
            "locus 0 beta 6\n\
             point P()\npoint Q()\npoint R()\npoint S()\n\
             rect a(4, 2)\ncircle b(1)\nrect c(2, 4)\nrounded_rect d(2, 4, 0.25)\n\
             assert center(P, b)\nassert center(a, b)\n\
             assert align_center_x(a, b, P, Q, R)\nassert align_center_y(a, b, P, S)\n\
             assert align_left(a, c)\nassert align_bottom(a, c)\n\
             assert align_right(a, d)\nassert align_top(a, d)\nassert a.width == 4\n",
            vec![
                ("P", point(Point::new(0.0, 0.0))),
                ("Q", point(Point::new(0.0, 2.0))),
                ("R", point(Point::new(0.0, -2.0))),
                ("S", point(Point::new(2.0, 0.0))),
                (
                    "a",
                    ResolvedObject::Rect(
                        construct::rect(Point::new(0.0, 0.0), 4.0, 2.0, tolerance).expect("a"),
                    ),
                ),
                (
                    "b",
                    ResolvedObject::Circle(Circle::new(Point::new(0.0, 0.0), 1.0).expect("b")),
                ),
                (
                    "c",
                    ResolvedObject::Rect(
                        construct::rect(Point::new(-1.0, 1.0), 2.0, 4.0, tolerance).expect("c"),
                    ),
                ),
                (
                    "d",
                    ResolvedObject::RoundedRect(
                        crate::geom::RoundedRect::new(Point::new(1.0, -1.0), 2.0, 4.0, 0.25)
                            .expect("d"),
                    ),
                ),
            ],
        );
        assert!(
            evaluate_assertions(&file, &scene)
                .expect("measured alignments")
                .passed
        );
    }

    #[test]
    fn failed_measured_alignment_reports_boolean_and_honors_assert_tolerance() {
        let (file, scene) = resolved(
            "locus 0 beta 6\npoint P()\npoint Q()\npoint R()\n\
             assert align_center_x(P, Q, R) tol 0.01\n\
             assert align_center_x(P, Q, R) tol 0.001\n",
            vec![
                ("P", point(Point::new(0.0, 0.0))),
                ("Q", point(Point::new(0.005, 1.0))),
                ("R", point(Point::new(0.0, 0.5))),
            ],
        );
        let report = evaluate_assertions(&file, &scene).expect("alignment report");
        assert!(report.assertions[0].passed);
        assert!(!report.assertions[1].passed);
        assert!(
            report.assertions[1]
                .actuals
                .iter()
                .any(|actual| matches!(actual.actual, ActualValue::Boolean { value: false }))
        );
    }
}
