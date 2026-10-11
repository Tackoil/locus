//! Typed expression evaluation and numeric dimension checking.

use std::collections::BTreeMap;

use locus_syntax::{AngleUnit, BinOp, Expr, Span, UnaryOp};

use super::env::Environment;
use super::error::EvalError;
use crate::Tolerance;
use crate::geom::construct;
use crate::geom::measure;
use crate::geom::{Arc, BBox, Bspline, Circle, Ellipse, Line, Point, Polygon, Segment};
use crate::scene::{AnchorSpec, Dimension, NumericValue, ResolvedObject, ResolvedPoint};

/// An object-valued expression, including composable temporary constructors.
#[derive(Clone, Debug, PartialEq)]
pub enum ObjectRef {
    /// A named declaration in the evaluator environment.
    Named(String),
    /// A nested constructor result.
    Temporary(Box<ResolvedObject>),
    /// A geometry or deferred-layout anchor.
    Anchor(AnchorSpec),
}

/// Runtime expression value.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    /// Unitless, length, or area numeric value.
    Scalar(NumericValue),
    /// Angle in radians; intentionally distinct from scalar values.
    Angle(f64),
    /// Named, temporary, or anchor object reference.
    ObjectRef(ObjectRef),
    /// Boolean expression result.
    Bool(bool),
}

/// A named numeric expression dimension, with angles kept distinct.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum InferredDimension {
    Number(Dimension),
    Angle,
}

/// Whole-file dimensions inferred for named declarations and expression nodes.
#[derive(Clone, Debug, Default)]
pub(crate) struct DimensionMap {
    named: BTreeMap<String, InferredDimension>,
    expressions: BTreeMap<(usize, usize), InferredDimension>,
}

impl DimensionMap {
    pub(crate) const fn new() -> Self {
        Self {
            named: BTreeMap::new(),
            expressions: BTreeMap::new(),
        }
    }

    pub(crate) fn get(&self, name: &str) -> Option<&InferredDimension> {
        self.named.get(name)
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.named.len()
    }

    #[cfg(test)]
    pub(crate) fn values(&self) -> impl Iterator<Item = &InferredDimension> {
        self.named.values()
    }

    pub(crate) fn insert(&mut self, name: String, dimension: InferredDimension) {
        self.named.insert(name, dimension);
    }

    pub(crate) fn insert_expression_range(
        &mut self,
        range: (usize, usize),
        dimension: InferredDimension,
    ) {
        self.expressions.insert(range, dimension);
    }

    pub(crate) fn expression_dimension(&self, expression: &Expr) -> Option<InferredDimension> {
        let span = expression.span();
        self.expressions
            .get(&(span.byte_start, span.byte_end))
            .copied()
    }
}

#[derive(Clone, Copy)]
pub(crate) enum Expected {
    Number(Dimension),
    Angle,
    Bool,
    Object,
}

/// Evaluates expressions against the already-resolved source prefix.
pub(crate) struct ExpressionEvaluator<'a> {
    env: &'a Environment,
    pub(super) tolerance: Tolerance,
    dimensions: &'a DimensionMap,
}

impl<'a> ExpressionEvaluator<'a> {
    pub(crate) const fn new(
        env: &'a Environment,
        tolerance: Tolerance,
        dimensions: &'a DimensionMap,
    ) -> Self {
        Self {
            env,
            tolerance,
            dimensions,
        }
    }

    pub(crate) fn dimension_of(&self, name: &str) -> Option<InferredDimension> {
        self.dimensions.get(name).copied()
    }

    pub(crate) fn expression_dimension(&self, expression: &Expr) -> Option<InferredDimension> {
        self.dimensions.expression_dimension(expression)
    }

    pub(crate) const fn dimensions(&self) -> &DimensionMap {
        self.dimensions
    }

    pub(crate) fn eval(&mut self, expression: &Expr) -> Result<Value, EvalError> {
        if let Some(value) = self.eval_leaf(expression) {
            return value;
        }
        self.eval_compound(expression)
    }

    fn eval_leaf(&self, expression: &Expr) -> Option<Result<Value, EvalError>> {
        match expression {
            Expr::Number(number) => Some(Self::eval_number_literal(number)),
            Expr::Boolean { value, .. } => Some(Ok(Value::Bool(*value))),
            Expr::Ref(id) => Some(self.eval_ref(&id.name, id.span)),
            Expr::String(value) | Expr::Tex(value) => Some(Err(EvalError::new(
                "E209",
                value.span,
                "text is only valid in an annotation text argument",
            ))),
            _ => None,
        }
    }

    fn eval_compound(&mut self, expression: &Expr) -> Result<Value, EvalError> {
        match expression {
            Expr::Call { name, args, span } => self.eval_call(&name.name, args, *span),
            Expr::Property {
                base,
                name,
                args,
                span,
            } => self.eval_property(base, &name.name, args, *span),
            Expr::Group { expr, .. } => self.eval(expr),
            Expr::Unary { op, expr, span } => self.eval_unary(*op, expr, *span),
            Expr::Binary { op, lhs, rhs, span } => self.eval_binary(*op, lhs, rhs, *span),
            Expr::Number(_)
            | Expr::Boolean { .. }
            | Expr::Ref(_)
            | Expr::String(_)
            | Expr::Tex(_) => Err(type_error(
                expression.span(),
                "internal expression dispatch error",
            )),
        }
    }

    pub(crate) fn eval_as(
        &mut self,
        expression: &Expr,
        expected: Expected,
    ) -> Result<Value, EvalError> {
        if let Expr::Group { expr, .. } = expression {
            return self.eval_as(expr, expected);
        }
        if let Some(value) = contextual_number_literal(expression, expected) {
            return require_expected(value?, expected, expression.span());
        }
        if let Some(value) = self.eval_contextual_operator(expression, expected) {
            return require_expected(value?, expected, expression.span());
        }
        if let Some(value) = self.eval_contextual_call(expression, expected) {
            return require_expected(value?, expected, expression.span());
        }
        let value = self.eval(expression)?;
        require_expected(value, expected, expression.span())
    }

    fn eval_contextual_operator(
        &mut self,
        expression: &Expr,
        expected: Expected,
    ) -> Option<Result<Value, EvalError>> {
        if !matches!(expected, Expected::Number(_) | Expected::Angle) {
            return None;
        }
        match expression {
            Expr::Unary { op, expr, span } if *op == UnaryOp::Neg => Some(
                self.eval_as(expr, expected)
                    .and_then(|value| negate_value(&value, *span)),
            ),
            Expr::Binary { op, lhs, rhs, span } if matches!(op, BinOp::Add | BinOp::Sub) => {
                Some(self.eval_contextual_sum(*op, lhs, rhs, expected, *span))
            }
            Expr::Binary { op, lhs, rhs, span } if matches!(op, BinOp::Mul | BinOp::Div) => {
                Some(self.eval_contextual_product(*op, lhs, rhs, expected, *span))
            }
            _ => None,
        }
    }

    fn eval_contextual_sum(
        &mut self,
        op: BinOp,
        lhs: &Expr,
        rhs: &Expr,
        expected: Expected,
        span: Span,
    ) -> Result<Value, EvalError> {
        let lhs = self.eval_as(lhs, expected)?;
        let rhs = self.eval_as(rhs, expected)?;
        sum_values(op, lhs, rhs, span)
    }

    fn eval_contextual_product(
        &mut self,
        op: BinOp,
        lhs: &Expr,
        rhs: &Expr,
        expected: Expected,
        span: Span,
    ) -> Result<Value, EvalError> {
        let untyped = self.eval_product(op, lhs, rhs, span);
        if let Expected::Angle = expected {
            for &(lhs_expected, rhs_expected) in angle_product_operand_expectations(op) {
                let candidate =
                    self.eval_product_as(op, lhs, rhs, lhs_expected, rhs_expected, span);
                if candidate
                    .as_ref()
                    .is_ok_and(|value| value_matches_expected(value, expected))
                {
                    return candidate;
                }
            }
            return untyped.and_then(|value| require_expected(value, expected, span));
        }
        if untyped
            .as_ref()
            .is_ok_and(|value| value_matches_expected(value, expected))
        {
            return untyped;
        }
        let Expected::Number(result_dimension) = expected else {
            return untyped.and_then(|value| require_expected(value, expected, span));
        };
        for &(lhs_expected, rhs_expected) in product_operand_expectations(op, result_dimension) {
            let candidate = self.eval_product_as(op, lhs, rhs, lhs_expected, rhs_expected, span);
            if candidate
                .as_ref()
                .is_ok_and(|value| value_matches_expected(value, expected))
            {
                return candidate;
            }
        }
        untyped.and_then(|value| require_expected(value, expected, span))
    }

    fn eval_product_as(
        &mut self,
        op: BinOp,
        lhs: &Expr,
        rhs: &Expr,
        lhs_expected: Expected,
        rhs_expected: Expected,
        span: Span,
    ) -> Result<Value, EvalError> {
        let lhs = self.eval_as(lhs, lhs_expected)?;
        let rhs_value = self.eval_as(rhs, rhs_expected)?;
        match op {
            BinOp::Mul => multiply(lhs, rhs_value, span),
            BinOp::Div => divide(lhs, rhs_value, self.tolerance, span, rhs.span()),
            _ => Err(type_error(span, "internal arithmetic dispatch error")),
        }
    }

    fn eval_contextual_call(
        &mut self,
        expression: &Expr,
        expected: Expected,
    ) -> Option<Result<Value, EvalError>> {
        let Expr::Call { name, args, span } = expression else {
            return None;
        };
        matches!(expected, Expected::Number(_) | Expected::Angle).then(|| {
            match name.name.as_str() {
                "sqrt" | "abs" | "min" | "max" => {
                    self.eval_contextual_numeric_call(&name.name, args, expected, *span)
                }
                _ => self.eval(expression),
            }
        })
    }

    fn eval_contextual_numeric_call(
        &mut self,
        name: &str,
        args: &[Expr],
        expected: Expected,
        span: Span,
    ) -> Result<Value, EvalError> {
        if name == "sqrt" {
            expect_arity(name, args, 1, span)?;
            let operand = sqrt_operand_expected(expected, span)?;
            let value = self.eval_as(&args[0], operand)?;
            return square_root(value, self.tolerance, args[0].span());
        }
        if name == "abs" {
            expect_arity(name, args, 1, span)?;
            let value = self.eval_as(&args[0], expected)?;
            return absolute_value(value, span);
        }
        expect_arity(name, args, 2, span)?;
        let lhs = self.eval_as(&args[0], expected)?;
        let rhs = self.eval_as(&args[1], expected)?;
        extremum(name, lhs, rhs, span)
    }

    fn eval_number_literal(number: &locus_syntax::NumberExpr) -> Result<Value, EvalError> {
        match number.unit {
            Some(AngleUnit::Deg) => {
                finite_angle(number.value * core::f64::consts::PI / 180.0, number.span)
            }
            Some(AngleUnit::Rad) => finite_angle(number.value, number.span),
            None => finite_scalar(number.value, Dimension::Scalar, number.span),
        }
    }

    fn eval_ref(&self, name: &str, span: Span) -> Result<Value, EvalError> {
        let object = self.env.get(name).ok_or_else(|| {
            EvalError::new(
                "E301",
                span,
                format!("unknown or forward reference `{name}`"),
            )
        })?;
        if let ResolvedObject::Num(number) = object {
            let dimension = match self.dimensions.get(name) {
                Some(InferredDimension::Number(dimension)) => *dimension,
                _ => number.dimension,
            };
            return Ok(Value::Scalar(NumericValue {
                value: number.value,
                dimension,
            }));
        }
        if let ResolvedObject::Angle(angle) = object {
            return Ok(Value::Angle(*angle));
        }
        Ok(Value::ObjectRef(ObjectRef::Named(name.to_owned())))
    }

    fn eval_call(&mut self, name: &str, args: &[Expr], span: Span) -> Result<Value, EvalError> {
        if is_numeric_function(name) {
            self.eval_numeric_call(name, args, span)
        } else {
            super::eval_constructor_call_value(self, name, args, span)
        }
    }

    fn eval_unary(
        &mut self,
        op: UnaryOp,
        expression: &Expr,
        span: Span,
    ) -> Result<Value, EvalError> {
        let value = self.eval(expression)?;
        match (op, value) {
            (UnaryOp::Neg, Value::Scalar(number)) => {
                finite_scalar(-number.value, number.dimension, span)
            }
            (UnaryOp::Neg, Value::Angle(angle)) => finite_angle(-angle, span),
            (UnaryOp::Not, Value::Bool(value)) => Ok(Value::Bool(!value)),
            (UnaryOp::Neg, _) => Err(type_error(span, "unary `-` requires a numeric value")),
            (UnaryOp::Not, _) => Err(type_error(span, "`not` requires a boolean value")),
        }
    }

    fn eval_binary(
        &mut self,
        op: BinOp,
        lhs: &Expr,
        rhs: &Expr,
        span: Span,
    ) -> Result<Value, EvalError> {
        match op {
            BinOp::Add | BinOp::Sub => self.eval_sum(op, lhs, rhs, span),
            BinOp::Mul | BinOp::Div => self.eval_product(op, lhs, rhs, span),
            BinOp::Eq | BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge => {
                self.eval_comparison(op, lhs, rhs, span)
            }
            BinOp::And | BinOp::Or => self.eval_boolean_binary(op, lhs, rhs, span),
        }
    }

    fn eval_sum(
        &mut self,
        op: BinOp,
        lhs: &Expr,
        rhs: &Expr,
        span: Span,
    ) -> Result<Value, EvalError> {
        let (lhs, rhs) = self.eval_matching_pair(lhs, rhs)?;
        sum_values(op, lhs, rhs, span)
    }

    fn eval_product(
        &mut self,
        op: BinOp,
        lhs: &Expr,
        rhs: &Expr,
        span: Span,
    ) -> Result<Value, EvalError> {
        let divisor_span = rhs.span();
        let lhs = self.eval(lhs)?;
        let rhs = self.eval(rhs)?;
        match op {
            BinOp::Mul => multiply(lhs, rhs, span),
            BinOp::Div => divide(lhs, rhs, self.tolerance, span, divisor_span),
            _ => Err(type_error(span, "internal arithmetic dispatch error")),
        }
    }

    fn eval_comparison(
        &mut self,
        op: BinOp,
        lhs: &Expr,
        rhs: &Expr,
        span: Span,
    ) -> Result<Value, EvalError> {
        let (lhs, rhs) = self.eval_matching_pair(lhs, rhs)?;
        compare_values(op, lhs, rhs, self.tolerance, span)
    }

    fn eval_matching_pair(&mut self, lhs: &Expr, rhs: &Expr) -> Result<(Value, Value), EvalError> {
        if let Some(expected) = self.inferred_pair_expected(lhs, rhs) {
            if let Some(pair) = self.match_pair_as(lhs, rhs, Some(expected)) {
                return Ok(pair);
            }
        }
        match (self.eval(lhs), self.eval(rhs)) {
            (Ok(lhs_value), Ok(rhs_value)) => {
                Ok(self.match_evaluated_pair(lhs, rhs, lhs_value, rhs_value))
            }
            (Err(error), Ok(rhs_value)) => self
                .match_pair_as(lhs, rhs, expected_for_value(&rhs_value))
                .ok_or(error),
            (Ok(lhs_value), Err(error)) => self
                .match_pair_as(lhs, rhs, expected_for_value(&lhs_value))
                .ok_or(error),
            (Err(error), Err(_)) => Err(error),
        }
    }

    fn inferred_pair_expected(&self, lhs: &Expr, rhs: &Expr) -> Option<Expected> {
        match (
            self.dimensions.expression_dimension(lhs),
            self.dimensions.expression_dimension(rhs),
        ) {
            (Some(lhs), Some(rhs)) if lhs == rhs => Some(expected_for_inferred(lhs)),
            (Some(dimension), None) | (None, Some(dimension)) => {
                Some(expected_for_inferred(dimension))
            }
            _ => None,
        }
    }

    fn match_evaluated_pair(
        &mut self,
        lhs: &Expr,
        rhs: &Expr,
        lhs_value: Value,
        rhs_value: Value,
    ) -> (Value, Value) {
        if values_have_matching_type(&lhs_value, &rhs_value) {
            return (lhs_value, rhs_value);
        }
        for expected in preferred_pair_expectations(&lhs_value, &rhs_value)
            .into_iter()
            .flatten()
        {
            if let Some(pair) = self.match_pair_as(lhs, rhs, Some(expected)) {
                return pair;
            }
        }
        (lhs_value, rhs_value)
    }

    fn match_pair_as(
        &mut self,
        lhs: &Expr,
        rhs: &Expr,
        expected: Option<Expected>,
    ) -> Option<(Value, Value)> {
        let expected = expected?;
        let lhs = self.eval_as(lhs, expected).ok()?;
        let rhs = self.eval_as(rhs, expected).ok()?;
        values_have_matching_type(&lhs, &rhs).then_some((lhs, rhs))
    }

    fn eval_boolean_binary(
        &mut self,
        op: BinOp,
        lhs: &Expr,
        rhs: &Expr,
        span: Span,
    ) -> Result<Value, EvalError> {
        let lhs_value = self.eval_as(lhs, Expected::Bool)?;
        let lhs = expect_bool(lhs_value, lhs.span())?;
        let rhs_value = self.eval_as(rhs, Expected::Bool)?;
        let rhs = expect_bool(rhs_value, rhs.span())?;
        Ok(Value::Bool(match op {
            BinOp::And => lhs && rhs,
            BinOp::Or => lhs || rhs,
            _ => return Err(type_error(span, "internal boolean dispatch error")),
        }))
    }

    fn eval_numeric_call(
        &mut self,
        name: &str,
        args: &[Expr],
        span: Span,
    ) -> Result<Value, EvalError> {
        if matches!(
            name,
            "dist"
                | "hdist"
                | "vdist"
                | "angle"
                | "dir_angle"
                | "radius"
                | "length"
                | "area"
                | "perimeter"
        ) {
            return self.eval_geometry_numeric_call(name, args, span);
        }
        self.eval_scalar_numeric_call(name, args, span)
    }

    fn eval_geometry_numeric_call(
        &mut self,
        name: &str,
        args: &[Expr],
        span: Span,
    ) -> Result<Value, EvalError> {
        match name {
            "dist" | "hdist" | "vdist" => self.eval_distance(name, args, span),
            "angle" => self.eval_angle(args, span),
            "dir_angle" => self.eval_direction_angle(args, span),
            "radius" => self.eval_radius(args, span),
            "length" => self.eval_length(args, span),
            "area" | "perimeter" => self.eval_shape_measure(name, args, span),
            _ => Err(type_error(
                span,
                format!("unknown geometry function `{name}`"),
            )),
        }
    }

    fn eval_scalar_numeric_call(
        &mut self,
        name: &str,
        args: &[Expr],
        span: Span,
    ) -> Result<Value, EvalError> {
        match name {
            "ratio" => self.eval_ratio(args, span),
            "sqrt" | "abs" => self.eval_unary_function(name, args, span),
            "min" | "max" => self.eval_extremum(name, args, span),
            _ => Err(type_error(
                span,
                format!("unknown numeric function `{name}`"),
            )),
        }
    }

    fn eval_distance(&mut self, name: &str, args: &[Expr], span: Span) -> Result<Value, EvalError> {
        expect_arity(name, args, 2, span)?;
        let lhs = self.eval_point(&args[0])?;
        let rhs = self.eval_point(&args[1])?;
        let value = match name {
            "hdist" => measure::hdist(lhs, rhs),
            "vdist" => measure::vdist(lhs, rhs),
            _ => measure::dist(lhs, rhs),
        };
        finite_scalar(value, Dimension::Length, span)
    }

    fn eval_angle(&mut self, args: &[Expr], span: Span) -> Result<Value, EvalError> {
        expect_arity("angle", args, 3, span)?;
        let points = self.eval_points(args)?;
        let angle =
            measure::angle(points[0], points[1], points[2], self.tolerance).map_err(|error| {
                EvalError::construction(span, error)
                    .with_measurement(
                        "first ray length",
                        measure::dist(points[1], points[0]),
                        Some("scene units"),
                    )
                    .with_measurement(
                        "second ray length",
                        measure::dist(points[1], points[2]),
                        Some("scene units"),
                    )
                    .with_measurement(
                        "ray cross product",
                        (points[0] - points[1]).cross(points[2] - points[1]),
                        Some("square scene units"),
                    )
            })?;
        finite_angle(angle, span)
    }

    fn eval_direction_angle(&mut self, args: &[Expr], span: Span) -> Result<Value, EvalError> {
        expect_arity("dir_angle", args, 2, span)?;
        let points = self.eval_points(args)?;
        let angle = measure::dir_angle(points[0], points[1], self.tolerance).map_err(|error| {
            EvalError::construction(span, error).with_measurement(
                "direction length",
                measure::dist(points[0], points[1]),
                Some("scene units"),
            )
        })?;
        finite_angle(angle, span)
    }

    fn eval_radius(&mut self, args: &[Expr], span: Span) -> Result<Value, EvalError> {
        expect_arity("radius", args, 1, span)?;
        let circle = self.eval_circle(&args[0])?;
        finite_scalar(circle.radius, Dimension::Length, span)
    }

    fn eval_length(&mut self, args: &[Expr], span: Span) -> Result<Value, EvalError> {
        expect_arity("length", args, 1, span)?;
        let segment = self.eval_segment(&args[0])?;
        finite_scalar(measure::length(segment), Dimension::Length, span)
    }

    fn eval_shape_measure(
        &mut self,
        name: &str,
        args: &[Expr],
        span: Span,
    ) -> Result<Value, EvalError> {
        expect_arity(name, args, 1, span)?;
        let object = self.eval_resolved_object(&args[0])?;
        match (name, object) {
            ("area", ResolvedObject::RoundedRect(rect)) => {
                finite_scalar(rect.area(), Dimension::Area, span)
            }
            ("perimeter", ResolvedObject::RoundedRect(rect)) => {
                finite_scalar(rect.perimeter(), Dimension::Length, span)
            }
            ("area", ResolvedObject::Ellipse(ellipse)) => finite_scalar(
                core::f64::consts::PI * ellipse.semimajor * ellipse.semiminor,
                Dimension::Area,
                span,
            ),
            (
                "area",
                ResolvedObject::Polygon(polygon)
                | ResolvedObject::Rect(polygon)
                | ResolvedObject::Diamond(polygon),
            ) => finite_scalar(measure::area(&polygon), Dimension::Area, span),
            (
                "perimeter",
                ResolvedObject::Polygon(polygon)
                | ResolvedObject::Rect(polygon)
                | ResolvedObject::Diamond(polygon),
            ) => finite_scalar(measure::perimeter(&polygon), Dimension::Length, span),
            ("perimeter", ResolvedObject::Ellipse(_)) => {
                Err(type_error(span, "ellipse perimeter is not supported"))
            }
            _ => Err(type_error(
                span,
                format!("{name} requires a polygon or supported closed shape"),
            )),
        }
    }

    fn eval_ratio(&mut self, args: &[Expr], span: Span) -> Result<Value, EvalError> {
        expect_arity("ratio", args, 2, span)?;
        let (lhs, rhs) = self.eval_matching_pair(&args[0], &args[1])?;
        let number = expect_number(
            divide(lhs, rhs, self.tolerance, span, args[1].span())?,
            span,
        )?;
        if number.dimension == Dimension::Scalar {
            Ok(Value::Scalar(number))
        } else {
            Err(type_error(
                span,
                "ratio operands must have the same dimension",
            ))
        }
    }

    fn eval_unary_function(
        &mut self,
        name: &str,
        args: &[Expr],
        span: Span,
    ) -> Result<Value, EvalError> {
        expect_arity(name, args, 1, span)?;
        let value = self.eval(&args[0])?;
        if name == "abs" {
            absolute_value(value, span)
        } else {
            square_root(value, self.tolerance, args[0].span())
        }
    }

    fn eval_extremum(&mut self, name: &str, args: &[Expr], span: Span) -> Result<Value, EvalError> {
        expect_arity(name, args, 2, span)?;
        let (lhs, rhs) = self.eval_matching_pair(&args[0], &args[1])?;
        extremum(name, lhs, rhs, span)
    }

    fn eval_property(
        &mut self,
        base: &Expr,
        name: &str,
        args: &[Expr],
        span: Span,
    ) -> Result<Value, EvalError> {
        let base_value = self.eval(base)?;
        let reference = expect_object(base_value, base.span())?;
        if let Some(anchor) = self.annotation_center(&reference, name, args, span)? {
            return Ok(Value::ObjectRef(ObjectRef::Anchor(anchor)));
        }
        let object = self.resolve_object(&reference, span)?;
        self.eval_geometry_property(&object, name, args, span)
    }

    fn annotation_center(
        &self,
        reference: &ObjectRef,
        name: &str,
        args: &[Expr],
        span: Span,
    ) -> Result<Option<AnchorSpec>, EvalError> {
        let ObjectRef::Named(id) = reference else {
            return Ok(None);
        };
        if !matches!(self.env.get(id), Some(ResolvedObject::Annotation(_))) || name != "center" {
            return Ok(None);
        }
        expect_arity("annotation.center", args, 0, span)?;
        Ok(Some(AnchorSpec::AnnotationCenter {
            id: id.clone(),
            span,
        }))
    }

    fn eval_geometry_property(
        &mut self,
        object: &ResolvedObject,
        name: &str,
        args: &[Expr],
        span: Span,
    ) -> Result<Value, EvalError> {
        if matches!(
            object,
            ResolvedObject::Ellipse(_)
                | ResolvedObject::Bspline(_)
                | ResolvedObject::RoundedRect(_)
        ) {
            return self.eval_curve_property(object, name, args, span);
        }
        match object {
            ResolvedObject::Segment(segment) => segment_property(*segment, name, args, span),
            ResolvedObject::Circle(circle) => self.circle_property(*circle, name, args, span),
            ResolvedObject::Arc(arc) => Self::arc_property(arc.geometry, name, args, span),
            ResolvedObject::Polygon(polygon) => polygon_property(polygon, name, args, span, false),
            ResolvedObject::Rect(polygon) | ResolvedObject::Diamond(polygon) => {
                polygon_property(polygon, name, args, span, true)
            }
            _ => Err(type_error(
                span,
                format!("property `{name}` is not defined for this object"),
            )),
        }
    }

    fn eval_curve_property(
        &mut self,
        object: &ResolvedObject,
        name: &str,
        args: &[Expr],
        span: Span,
    ) -> Result<Value, EvalError> {
        match object {
            ResolvedObject::Ellipse(ellipse) => self.ellipse_property(*ellipse, name, args, span),
            ResolvedObject::Bspline(spline) => self.bspline_property(spline, name, args, span),
            ResolvedObject::RoundedRect(rect) => rounded_rect_property(*rect, name, args, span),
            _ => Err(type_error(span, "expected a curve")),
        }
    }

    fn ellipse_property(
        &mut self,
        ellipse: Ellipse,
        name: &str,
        args: &[Expr],
        span: Span,
    ) -> Result<Value, EvalError> {
        let point = ellipse_point_property(ellipse, name);
        if let Some(point) = point {
            return point_property(point, args, span);
        }
        self.ellipse_nonpoint_property(ellipse, name, args, span)
    }

    fn ellipse_nonpoint_property(
        &mut self,
        ellipse: Ellipse,
        name: &str,
        args: &[Expr],
        span: Span,
    ) -> Result<Value, EvalError> {
        match name {
            "at" => {
                expect_arity("ellipse.at", args, 1, span)?;
                let angle = self.eval_angle_value(&args[0])?;
                ellipse
                    .at(angle)
                    .map(point_value)
                    .map_err(|error| EvalError::construction(span, error))
            }
            "semimajor" => scalar_property(ellipse.semimajor, Dimension::Length, args, span),
            "semiminor" => scalar_property(ellipse.semiminor, Dimension::Length, args, span),
            "rotation" => angle_property(ellipse.rotation, args, span),
            _ => Err(type_error(
                span,
                format!("ellipse has no property `{name}`"),
            )),
        }
    }

    fn bspline_property(
        &mut self,
        spline: &Bspline,
        name: &str,
        args: &[Expr],
        span: Span,
    ) -> Result<Value, EvalError> {
        let point = match name {
            "start" => Some(spline.controls()[0]),
            "end" => Some(spline.controls()[spline.controls().len() - 1]),
            "at" => {
                expect_arity("bspline.at", args, 1, span)?;
                let parameter = self.eval_number(&args[0], Dimension::Scalar)?;
                Some(
                    spline
                        .at(parameter)
                        .map_err(|error| EvalError::construction(span, error))?,
                )
            }
            _ => None,
        };
        point.map_or_else(
            || {
                Err(type_error(
                    span,
                    format!("B-spline has no property `{name}`"),
                ))
            },
            |point| point_property(point, &[], span),
        )
    }

    fn circle_property(
        &mut self,
        circle: Circle,
        name: &str,
        args: &[Expr],
        span: Span,
    ) -> Result<Value, EvalError> {
        match name {
            "center" => point_property(circle.center, args, span),
            "at" => {
                expect_arity("circle.at", args, 1, span)?;
                let value = self.eval_as(&args[0], Expected::Angle)?;
                let angle = expect_angle(value, args[0].span())?;
                let point = construct::on_circle(circle, angle)
                    .map_err(|error| EvalError::construction(span, error))?;
                Ok(point_value(point))
            }
            _ => Err(type_error(span, format!("circle has no property `{name}`"))),
        }
    }

    fn arc_property(arc: Arc, name: &str, args: &[Expr], span: Span) -> Result<Value, EvalError> {
        match name {
            "center" => point_property(arc.circle.center, args, span),
            "start" => point_property(arc.start, args, span),
            "end" => point_property(arc.end, args, span),
            _ => Err(type_error(span, format!("arc has no property `{name}`"))),
        }
    }

    pub(crate) fn eval_point(&mut self, expression: &Expr) -> Result<Point, EvalError> {
        let value = self.eval_as(expression, Expected::Object)?;
        let reference = expect_object(value, expression.span())?;
        self.reference_to_point(reference, expression.span())
    }

    fn reference_to_point(&self, reference: ObjectRef, span: Span) -> Result<Point, EvalError> {
        match reference {
            ObjectRef::Anchor(anchor) => anchor_to_point(anchor, span),
            other => self.object_reference_to_point(&other, span),
        }
    }

    fn object_reference_to_point(
        &self,
        reference: &ObjectRef,
        span: Span,
    ) -> Result<Point, EvalError> {
        match self.resolve_object(reference, span)? {
            ResolvedObject::Point(point) => Ok(point.geometry),
            _ => Err(type_error(span, "expected a point or geometry anchor")),
        }
    }

    pub(crate) fn eval_anchor(&mut self, expression: &Expr) -> Result<AnchorSpec, EvalError> {
        let value = self.eval_as(expression, Expected::Object)?;
        let reference = expect_object(value, expression.span())?;
        match reference {
            ObjectRef::Anchor(anchor) => Ok(anchor),
            other => self
                .reference_to_point(other, expression.span())
                .map(AnchorSpec::Point),
        }
    }

    pub(crate) fn eval_line(&mut self, expression: &Expr) -> Result<Line, EvalError> {
        match self.eval_resolved_object(expression)? {
            ResolvedObject::Line(value) => Ok(value),
            _ => Err(type_error(expression.span(), "expected a line")),
        }
    }

    pub(crate) fn eval_linear(&mut self, expression: &Expr) -> Result<ResolvedObject, EvalError> {
        let object = self.eval_resolved_object(expression)?;
        match object {
            ResolvedObject::Line(_) | ResolvedObject::Ray(_) | ResolvedObject::Segment(_) => {
                Ok(object)
            }
            _ => Err(type_error(
                expression.span(),
                "expected a line, ray, or segment",
            )),
        }
    }

    pub(crate) fn eval_circle(&mut self, expression: &Expr) -> Result<Circle, EvalError> {
        match self.eval_resolved_object(expression)? {
            ResolvedObject::Circle(value) => Ok(value),
            _ => Err(type_error(expression.span(), "expected a circle")),
        }
    }

    pub(crate) fn eval_segment(&mut self, expression: &Expr) -> Result<Segment, EvalError> {
        match self.eval_resolved_object(expression)? {
            ResolvedObject::Segment(value) => Ok(value),
            _ => Err(type_error(expression.span(), "expected a segment")),
        }
    }

    pub(crate) fn eval_number(
        &mut self,
        expression: &Expr,
        dimension: Dimension,
    ) -> Result<f64, EvalError> {
        let value = self.eval_as(expression, Expected::Number(dimension))?;
        Ok(expect_number(value, expression.span())?.value)
    }

    pub(crate) fn eval_angle_value(&mut self, expression: &Expr) -> Result<f64, EvalError> {
        let value = self.eval_as(expression, Expected::Angle)?;
        expect_angle(value, expression.span())
    }

    pub(crate) fn eval_bool(&mut self, expression: &Expr) -> Result<bool, EvalError> {
        let value = self.eval_as(expression, Expected::Bool)?;
        expect_bool(value, expression.span())
    }

    pub(crate) fn eval_points(&mut self, expressions: &[Expr]) -> Result<Vec<Point>, EvalError> {
        expressions
            .iter()
            .map(|expression| self.eval_point(expression))
            .collect()
    }

    pub(crate) fn eval_resolved_object(
        &mut self,
        expression: &Expr,
    ) -> Result<ResolvedObject, EvalError> {
        let value = self.eval_as(expression, Expected::Object)?;
        let reference = expect_object(value, expression.span())?;
        self.resolve_object(&reference, expression.span())
    }

    fn resolve_object(
        &self,
        reference: &ObjectRef,
        span: Span,
    ) -> Result<ResolvedObject, EvalError> {
        match reference {
            ObjectRef::Named(name) => self.env.get(name).cloned().ok_or_else(|| {
                EvalError::new(
                    "E301",
                    span,
                    format!("unknown or forward reference `{name}`"),
                )
            }),
            ObjectRef::Temporary(object) => Ok((**object).clone()),
            ObjectRef::Anchor(_) => Err(type_error(span, "expected an object, found an anchor")),
        }
    }
}

fn ellipse_point_property(ellipse: Ellipse, name: &str) -> Option<Point> {
    let major = ellipse.major_unit();
    match name {
        "center" => Some(ellipse.center),
        "major_start" => Some(ellipse.center - major * ellipse.semimajor),
        "major_end" => Some(ellipse.center + major * ellipse.semimajor),
        "focus1" => Some(ellipse.center - major * ellipse.focal_distance()),
        "focus2" => Some(ellipse.center + major * ellipse.focal_distance()),
        _ => ellipse_minor_point_property(ellipse, name),
    }
}

fn ellipse_minor_point_property(ellipse: Ellipse, name: &str) -> Option<Point> {
    let minor = ellipse.minor_unit();
    match name {
        "minor_start" => Some(ellipse.center - minor * ellipse.semiminor),
        "minor_end" => Some(ellipse.center + minor * ellipse.semiminor),
        _ => None,
    }
}

fn scalar_property(
    value: f64,
    dimension: Dimension,
    args: &[Expr],
    span: Span,
) -> Result<Value, EvalError> {
    expect_arity("numeric property", args, 0, span)?;
    finite_scalar(value, dimension, span)
}

fn angle_property(value: f64, args: &[Expr], span: Span) -> Result<Value, EvalError> {
    expect_arity("angle property", args, 0, span)?;
    finite_angle(value, span)
}

fn is_numeric_function(name: &str) -> bool {
    locus_syntax::NUMERIC_FUNCTION_NAMES.contains(&name)
}

fn finite_scalar(value: f64, dimension: Dimension, span: Span) -> Result<Value, EvalError> {
    if value.is_finite() {
        Ok(Value::Scalar(NumericValue { value, dimension }))
    } else {
        Err(EvalError::new(
            "E202",
            span,
            "expression produced a non-finite value",
        ))
    }
}

fn finite_angle(value: f64, span: Span) -> Result<Value, EvalError> {
    if value.is_finite() {
        Ok(Value::Angle(value))
    } else {
        Err(EvalError::new(
            "E202",
            span,
            "angle expression produced a non-finite value",
        ))
    }
}

fn require_expected(value: Value, expected: Expected, span: Span) -> Result<Value, EvalError> {
    match (expected, &value) {
        (Expected::Number(dimension), Value::Scalar(number)) if number.dimension == dimension => {
            Ok(value)
        }
        (Expected::Angle, Value::Angle(_))
        | (Expected::Bool, Value::Bool(_))
        | (Expected::Object, Value::ObjectRef(_)) => Ok(value),
        (Expected::Angle, Value::Scalar(_)) => Err(type_error(
            span,
            "expected an angle with an explicit `deg` or `rad` unit",
        )),
        (Expected::Number(dimension), Value::Scalar(number)) => Err(type_error(
            span,
            format!("expected {dimension:?}, found {:?}", number.dimension),
        )),
        _ => Err(type_error(span, "expression has the wrong semantic type")),
    }
}

fn expected_for_value(value: &Value) -> Option<Expected> {
    match value {
        Value::Scalar(number) => Some(Expected::Number(number.dimension)),
        Value::Angle(_) => Some(Expected::Angle),
        Value::ObjectRef(_) | Value::Bool(_) => None,
    }
}

const fn expected_for_inferred(dimension: InferredDimension) -> Expected {
    match dimension {
        InferredDimension::Number(dimension) => Expected::Number(dimension),
        InferredDimension::Angle => Expected::Angle,
    }
}

fn preferred_pair_expectations(lhs: &Value, rhs: &Value) -> [Option<Expected>; 2] {
    let lhs_expected = expected_for_value(lhs);
    let rhs_expected = expected_for_value(rhs);
    if is_unitless_scalar(lhs) && !is_unitless_scalar(rhs) {
        [rhs_expected, lhs_expected]
    } else {
        [lhs_expected, rhs_expected]
    }
}

fn is_unitless_scalar(value: &Value) -> bool {
    matches!(
        value,
        Value::Scalar(NumericValue {
            dimension: Dimension::Scalar,
            ..
        })
    )
}

fn contextual_number_literal(
    expression: &Expr,
    expected: Expected,
) -> Option<Result<Value, EvalError>> {
    let Expr::Number(number) = expression else {
        return None;
    };
    match expected {
        Expected::Number(dimension) if number.unit.is_none() => {
            Some(finite_scalar(number.value, dimension, number.span))
        }
        Expected::Number(_) | Expected::Angle | Expected::Bool | Expected::Object => None,
    }
}

fn sqrt_operand_expected(result: Expected, span: Span) -> Result<Expected, EvalError> {
    match result {
        Expected::Number(Dimension::Scalar) => Ok(Expected::Number(Dimension::Scalar)),
        Expected::Number(Dimension::Length) => Ok(Expected::Number(Dimension::Area)),
        Expected::Number(Dimension::Area) | Expected::Angle => {
            Err(type_error(span, "sqrt result must be scalar or length"))
        }
        Expected::Bool | Expected::Object => Err(type_error(span, "sqrt requires a number")),
    }
}

fn values_have_matching_type(lhs: &Value, rhs: &Value) -> bool {
    match (lhs, rhs) {
        (Value::Scalar(lhs), Value::Scalar(rhs)) => lhs.dimension == rhs.dimension,
        (Value::Angle(_), Value::Angle(_))
        | (Value::ObjectRef(_), Value::ObjectRef(_))
        | (Value::Bool(_), Value::Bool(_)) => true,
        _ => false,
    }
}

fn value_matches_expected(value: &Value, expected: Expected) -> bool {
    match (value, expected) {
        (Value::Scalar(number), Expected::Number(dimension)) => number.dimension == dimension,
        (Value::Angle(_), Expected::Angle)
        | (Value::ObjectRef(_), Expected::Object)
        | (Value::Bool(_), Expected::Bool) => true,
        _ => false,
    }
}

pub(crate) fn angle_product_operand_expectations(op: BinOp) -> &'static [(Expected, Expected)] {
    const SCALAR: Expected = Expected::Number(Dimension::Scalar);

    match op {
        BinOp::Mul => &[(Expected::Angle, SCALAR), (SCALAR, Expected::Angle)],
        BinOp::Div => &[(Expected::Angle, SCALAR)],
        _ => &[],
    }
}

pub(crate) fn product_operand_expectations(
    op: BinOp,
    result: Dimension,
) -> &'static [(Expected, Expected)] {
    use Dimension::{Area, Length, Scalar};
    const SCALAR: Expected = Expected::Number(Dimension::Scalar);
    const LENGTH: Expected = Expected::Number(Dimension::Length);
    const AREA: Expected = Expected::Number(Dimension::Area);

    match (op, result) {
        (BinOp::Mul, Scalar) => &[(SCALAR, SCALAR)],
        (BinOp::Div, Scalar) => &[
            (SCALAR, SCALAR),
            (LENGTH, LENGTH),
            (AREA, AREA),
            (Expected::Angle, Expected::Angle),
        ],
        (BinOp::Mul, Length) => &[(LENGTH, SCALAR), (SCALAR, LENGTH)],
        (BinOp::Div, Length) => &[(LENGTH, SCALAR), (AREA, LENGTH)],
        (BinOp::Mul, Area) => &[(LENGTH, LENGTH), (AREA, SCALAR), (SCALAR, AREA)],
        (BinOp::Div, Area) => &[(AREA, SCALAR)],
        _ => &[],
    }
}

fn negate_value(value: &Value, span: Span) -> Result<Value, EvalError> {
    match value {
        Value::Scalar(number) => finite_scalar(-number.value, number.dimension, span),
        Value::Angle(angle) => finite_angle(-*angle, span),
        _ => Err(type_error(span, "unary `-` requires a numeric value")),
    }
}

fn sum_values(op: BinOp, lhs: Value, rhs: Value, span: Span) -> Result<Value, EvalError> {
    match (lhs, rhs) {
        (Value::Scalar(lhs), Value::Scalar(rhs)) if lhs.dimension == rhs.dimension => {
            finite_scalar(apply_sum(op, lhs.value, rhs.value), lhs.dimension, span)
        }
        (Value::Angle(lhs), Value::Angle(rhs)) => finite_angle(apply_sum(op, lhs, rhs), span),
        _ => Err(type_error(
            span,
            "addition and subtraction require matching numeric dimensions",
        )),
    }
}

fn multiply(lhs: Value, rhs: Value, span: Span) -> Result<Value, EvalError> {
    match (lhs, rhs) {
        (Value::Scalar(lhs), Value::Scalar(rhs)) => {
            let dimension = multiply_dimensions(lhs.dimension, rhs.dimension, span)?;
            finite_scalar(lhs.value * rhs.value, dimension, span)
        }
        (Value::Scalar(scalar), Value::Angle(angle)) if scalar.dimension == Dimension::Scalar => {
            finite_angle(scalar.value * angle, span)
        }
        (Value::Angle(angle), Value::Scalar(scalar)) if scalar.dimension == Dimension::Scalar => {
            finite_angle(angle * scalar.value, span)
        }
        _ => Err(type_error(span, "unsupported dimension product")),
    }
}

fn multiply_dimensions(lhs: Dimension, rhs: Dimension, span: Span) -> Result<Dimension, EvalError> {
    match (lhs, rhs) {
        (Dimension::Scalar, other) | (other, Dimension::Scalar) => Ok(other),
        (Dimension::Length, Dimension::Length) => Ok(Dimension::Area),
        _ => Err(type_error(span, "unsupported dimension product")),
    }
}

fn divide(
    lhs: Value,
    rhs: Value,
    tolerance: Tolerance,
    span: Span,
    divisor_span: Span,
) -> Result<Value, EvalError> {
    reject_zero_divisor(&rhs, tolerance, divisor_span)?;
    match (lhs, rhs) {
        (Value::Scalar(lhs), Value::Scalar(rhs)) => {
            let dimension = divide_dimensions(lhs.dimension, rhs.dimension, span)?;
            finite_scalar(lhs.value / rhs.value, dimension, span)
        }
        (Value::Angle(lhs), Value::Angle(rhs)) => finite_scalar(lhs / rhs, Dimension::Scalar, span),
        (Value::Angle(lhs), Value::Scalar(rhs)) if rhs.dimension == Dimension::Scalar => {
            finite_angle(lhs / rhs.value, span)
        }
        _ => Err(type_error(span, "unsupported dimension quotient")),
    }
}

fn divide_dimensions(lhs: Dimension, rhs: Dimension, span: Span) -> Result<Dimension, EvalError> {
    match (lhs, rhs) {
        (same_lhs, same_rhs) if same_lhs == same_rhs => Ok(Dimension::Scalar),
        (other, Dimension::Scalar) => Ok(other),
        (Dimension::Area, Dimension::Length) => Ok(Dimension::Length),
        _ => Err(type_error(span, "unsupported dimension quotient")),
    }
}

fn reject_zero_divisor(value: &Value, tolerance: Tolerance, span: Span) -> Result<(), EvalError> {
    let is_zero = match value {
        Value::Scalar(number) => scalar_is_zero(*number, tolerance),
        Value::Angle(angle) => tolerance.construction_angle_zero(*angle),
        _ => return Err(type_error(span, "division requires numeric operands")),
    };
    if is_zero {
        let (measurement, unit) = divisor_measurement(value);
        Err(EvalError::new(
            "E204",
            span,
            "division denominator is zero under construction tolerance",
        )
        .with_measurement("denominator", measurement, unit))
    } else {
        Ok(())
    }
}

fn divisor_measurement(value: &Value) -> (f64, Option<&'static str>) {
    match value {
        Value::Scalar(number) => (number.value, dimension_unit(number.dimension)),
        Value::Angle(angle) => (*angle, Some("rad")),
        Value::ObjectRef(_) | Value::Bool(_) => (f64::NAN, None),
    }
}

const fn dimension_unit(dimension: Dimension) -> Option<&'static str> {
    match dimension {
        Dimension::Scalar => None,
        Dimension::Length => Some("scene units"),
        Dimension::Area => Some("scene units²"),
    }
}

fn scalar_is_zero(number: NumericValue, tolerance: Tolerance) -> bool {
    match number.dimension {
        Dimension::Scalar => tolerance.construction_scalar_zero(number.value),
        Dimension::Length => tolerance.construction_length_zero(number.value),
        Dimension::Area => tolerance.construction_area_zero(number.value),
    }
}

fn compare_values(
    op: BinOp,
    lhs: Value,
    rhs: Value,
    tolerance: Tolerance,
    span: Span,
) -> Result<Value, EvalError> {
    let (lhs, rhs, equal) = comparable_values(lhs, rhs, tolerance, span)?;
    let result = comparison_result(op, lhs, rhs, equal)
        .ok_or_else(|| type_error(span, "internal comparison dispatch error"))?;
    Ok(Value::Bool(result))
}

fn comparison_result(op: BinOp, lhs: f64, rhs: f64, equal: bool) -> Option<bool> {
    Some(match op {
        BinOp::Eq => equal,
        BinOp::Lt => lhs < rhs && !equal,
        BinOp::Gt => lhs > rhs && !equal,
        _ => return inclusive_comparison_result(op, lhs, rhs, equal),
    })
}

fn inclusive_comparison_result(op: BinOp, lhs: f64, rhs: f64, equal: bool) -> Option<bool> {
    match op {
        BinOp::Le => Some(lhs < rhs || equal),
        BinOp::Ge => Some(lhs > rhs || equal),
        _ => None,
    }
}

fn comparable_values(
    lhs: Value,
    rhs: Value,
    tolerance: Tolerance,
    span: Span,
) -> Result<(f64, f64, bool), EvalError> {
    match (lhs, rhs) {
        (Value::Scalar(lhs), Value::Scalar(rhs)) if lhs.dimension == rhs.dimension => {
            let equal = numeric_equal(lhs, rhs, tolerance);
            Ok((lhs.value, rhs.value, equal))
        }
        (Value::Angle(lhs), Value::Angle(rhs)) => Ok((lhs, rhs, tolerance.angle_zero(lhs - rhs))),
        _ => Err(type_error(
            span,
            "comparison operands must have matching dimensions",
        )),
    }
}

fn numeric_equal(lhs: NumericValue, rhs: NumericValue, tolerance: Tolerance) -> bool {
    match lhs.dimension {
        Dimension::Scalar => tolerance.scalar_zero(lhs.value - rhs.value, lhs.value, rhs.value),
        Dimension::Length => tolerance.length_eq(lhs.value, rhs.value),
        Dimension::Area => tolerance.area_eq(lhs.value, rhs.value),
    }
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "Value is consumed by expression dispatch"
)]
fn absolute_value(value: Value, span: Span) -> Result<Value, EvalError> {
    match value {
        Value::Scalar(number) => finite_scalar(number.value.abs(), number.dimension, span),
        Value::Angle(angle) => finite_angle(angle.abs(), span),
        _ => Err(type_error(span, "abs requires a numeric argument")),
    }
}

fn square_root(value: Value, tolerance: Tolerance, span: Span) -> Result<Value, EvalError> {
    let number = expect_number(value, span)?;
    let dimension = sqrt_dimension(number.dimension, span)?;
    let near_zero = scalar_is_zero(number, tolerance);
    if number.value < 0.0 && !near_zero {
        return Err(
            EvalError::new("E204", span, "sqrt operand must be nonnegative").with_measurement(
                "operand",
                number.value,
                dimension_unit(number.dimension),
            ),
        );
    }
    finite_scalar(libm::sqrt(number.value.max(0.0)), dimension, span)
}

fn sqrt_dimension(dimension: Dimension, span: Span) -> Result<Dimension, EvalError> {
    Ok(match dimension {
        Dimension::Scalar => Dimension::Scalar,
        Dimension::Area => Dimension::Length,
        Dimension::Length => {
            return Err(type_error(span, "sqrt accepts only scalar or area values"));
        }
    })
}

fn extremum(name: &str, lhs: Value, rhs: Value, span: Span) -> Result<Value, EvalError> {
    match (lhs, rhs) {
        (Value::Scalar(lhs), Value::Scalar(rhs)) if lhs.dimension == rhs.dimension => {
            let value = if name == "min" {
                lhs.value.min(rhs.value)
            } else {
                lhs.value.max(rhs.value)
            };
            finite_scalar(value, lhs.dimension, span)
        }
        (Value::Angle(lhs), Value::Angle(rhs)) => {
            let value = if name == "min" {
                lhs.min(rhs)
            } else {
                lhs.max(rhs)
            };
            finite_angle(value, span)
        }
        _ => Err(type_error(
            span,
            format!("{name} operands must have matching dimensions"),
        )),
    }
}

fn segment_property(
    segment: Segment,
    name: &str,
    args: &[Expr],
    span: Span,
) -> Result<Value, EvalError> {
    let point = match name {
        "start" => segment.start,
        "end" => segment.end,
        "midpoint" => segment.midpoint(),
        _ => {
            return Err(type_error(
                span,
                format!("segment has no property `{name}`"),
            ));
        }
    };
    point_property(point, args, span)
}

fn rounded_rect_property(
    rect: crate::geom::RoundedRect,
    name: &str,
    args: &[Expr],
    span: Span,
) -> Result<Value, EvalError> {
    expect_arity(name, args, 0, span)?;
    match name {
        "corner_radius" => finite_scalar(rect.corner_radius, Dimension::Length, span),
        "centroid" => Ok(point_value(rect.center)),
        _ => box_property(rect.bbox(), name, span),
    }
}

fn polygon_property(
    polygon: &Polygon,
    name: &str,
    args: &[Expr],
    span: Span,
    box_anchors: bool,
) -> Result<Value, EvalError> {
    expect_arity(name, args, 0, span)?;
    if name == "centroid" {
        let point = polygon
            .centroid()
            .map_err(|error| EvalError::construction(span, error))?;
        return Ok(point_value(point));
    }
    if !box_anchors {
        return Err(type_error(
            span,
            format!("polygon has no property `{name}`"),
        ));
    }
    box_property(BBox::for_polygon(polygon), name, span)
}

fn box_property(bbox: BBox, name: &str, span: Span) -> Result<Value, EvalError> {
    if let Some(value) = box_numeric_property(bbox, name, span) {
        return value;
    }
    let point = box_point_property(bbox, name);
    point.map_or_else(
        || Err(type_error(span, format!("box has no property `{name}`"))),
        |value| Ok(point_value(value)),
    )
}

fn box_point_property(bbox: BBox, name: &str) -> Option<Point> {
    match name {
        "center" => Some(bbox.center()),
        "n" => Some(bbox.north()),
        "s" => Some(bbox.south()),
        "e" => Some(bbox.east()),
        "w" => Some(bbox.west()),
        _ => box_diagonal_property(bbox, name),
    }
}

fn box_diagonal_property(bbox: BBox, name: &str) -> Option<Point> {
    match name {
        "ne" => Some(bbox.northeast()),
        "nw" => Some(bbox.northwest()),
        "se" => Some(bbox.southeast()),
        "sw" => Some(bbox.southwest()),
        _ => None,
    }
}

fn box_numeric_property(bbox: BBox, name: &str, span: Span) -> Option<Result<Value, EvalError>> {
    match name {
        "width" => Some(finite_scalar(bbox.width(), Dimension::Length, span)),
        "height" => Some(finite_scalar(bbox.height(), Dimension::Length, span)),
        _ => None,
    }
}

fn point_property(point: Point, args: &[Expr], span: Span) -> Result<Value, EvalError> {
    expect_arity("point property", args, 0, span)?;
    Ok(point_value(point))
}

fn point_value(point: Point) -> Value {
    Value::ObjectRef(ObjectRef::Temporary(Box::new(ResolvedObject::Point(
        ResolvedPoint::new(point),
    ))))
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "anchors are consumed while resolving expression values"
)]
fn anchor_to_point(anchor: AnchorSpec, span: Span) -> Result<Point, EvalError> {
    match anchor {
        AnchorSpec::Point(point) => Ok(point),
        AnchorSpec::AnnotationCenter { .. } => Err(type_error(
            span,
            "layout-layer annotation anchors cannot feed geometry",
        )),
    }
}

fn apply_sum(op: BinOp, lhs: f64, rhs: f64) -> f64 {
    if op == BinOp::Add {
        lhs + rhs
    } else {
        lhs - rhs
    }
}

pub(crate) fn expect_arity(
    name: &str,
    args: &[Expr],
    count: usize,
    span: Span,
) -> Result<(), EvalError> {
    if args.len() == count {
        Ok(())
    } else {
        Err(type_error(
            span,
            format!("`{name}` expects {count} arguments, found {}", args.len()),
        ))
    }
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "typed extraction consumes the expression value"
)]
pub(crate) fn expect_number(value: Value, span: Span) -> Result<NumericValue, EvalError> {
    match value {
        Value::Scalar(number) => Ok(number),
        _ => Err(type_error(span, "expected a scalar, length, or area value")),
    }
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "typed extraction consumes the expression value"
)]
pub(crate) fn expect_angle(value: Value, span: Span) -> Result<f64, EvalError> {
    match value {
        Value::Angle(angle) => Ok(angle),
        _ => Err(type_error(
            span,
            "expected an angle with an explicit `deg` or `rad` unit",
        )),
    }
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "typed extraction consumes the expression value"
)]
pub(crate) fn expect_bool(value: Value, span: Span) -> Result<bool, EvalError> {
    match value {
        Value::Bool(value) => Ok(value),
        _ => Err(type_error(span, "expected a boolean value")),
    }
}

pub(crate) fn expect_object(value: Value, span: Span) -> Result<ObjectRef, EvalError> {
    match value {
        Value::ObjectRef(reference) => Ok(reference),
        _ => Err(type_error(span, "expected an object reference")),
    }
}

pub(crate) fn type_error(span: Span, message: impl Into<String>) -> EvalError {
    EvalError::new("E209", span, message)
}
