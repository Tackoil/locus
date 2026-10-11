//! Forward-mode automatic differentiation through derived geometry.

use std::collections::BTreeMap;
mod rounded_rect;
use rounded_rect::DRoundedRect;

use locus_syntax::{
    AngleUnit, BinOp, Constructor, Expr, File, Kind, OpStmt, Span, Statement, UnaryOp,
};

use crate::Tolerance;
use crate::eval::EvalError;
use crate::eval::expr::{DimensionMap, InferredDimension};
use crate::scene::{Dimension, ResolvedObject};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NumericDimension {
    Scalar,
    Length,
    Area,
    Angle,
}

#[derive(Clone, Debug)]
pub(crate) struct Jet {
    pub(crate) value: f64,
    pub(crate) seed_value: f64,
    pub(crate) gradient: Vec<f64>,
}

impl Jet {
    pub(crate) fn constant(value: f64, count: usize) -> Self {
        Self {
            value,
            seed_value: value,
            gradient: vec![0.0; count],
        }
    }

    fn variable(value: f64, seed_value: f64, index: usize, scale: f64, count: usize) -> Self {
        let mut result = Self::constant(value, count);
        result.seed_value = seed_value;
        result.gradient[index] = scale;
        result
    }

    pub(crate) fn add(&self, rhs: &Self) -> Self {
        Self::from_binary(
            self.value + rhs.value,
            self.seed_value + rhs.seed_value,
            self,
            rhs,
            1.0,
            1.0,
        )
    }

    pub(crate) fn sub(&self, rhs: &Self) -> Self {
        Self::from_binary(
            self.value - rhs.value,
            self.seed_value - rhs.seed_value,
            self,
            rhs,
            1.0,
            -1.0,
        )
    }

    pub(crate) fn mul(&self, rhs: &Self) -> Self {
        Self::from_binary(
            self.value * rhs.value,
            self.seed_value * rhs.seed_value,
            self,
            rhs,
            rhs.value,
            self.value,
        )
    }

    pub(crate) fn div(&self, rhs: &Self) -> Self {
        let inverse = 1.0 / rhs.value;
        Self::from_binary(
            self.value / rhs.value,
            self.seed_value / rhs.seed_value,
            self,
            rhs,
            inverse,
            -self.value * inverse * inverse,
        )
    }

    pub(crate) fn scale(&self, factor: f64) -> Self {
        Self {
            value: self.value * factor,
            seed_value: self.seed_value * factor,
            gradient: self.gradient.iter().map(|value| value * factor).collect(),
        }
    }

    fn neg(&self) -> Self {
        self.scale(-1.0)
    }

    pub(crate) fn sqrt(&self) -> Result<Self, ()> {
        if self.value < 0.0 || !self.value.is_finite() {
            return Err(());
        }
        if self.value == 0.0 {
            let mut result = Self::constant(0.0, self.gradient.len());
            result.seed_value = libm::sqrt(self.seed_value);
            return Ok(result);
        }
        let value = libm::sqrt(self.value);
        Ok(Self {
            value,
            seed_value: libm::sqrt(self.seed_value),
            gradient: self
                .gradient
                .iter()
                .map(|derivative| derivative / (2.0 * value))
                .collect(),
        })
    }

    pub(crate) fn sin(&self) -> Self {
        Self {
            value: libm::sin(self.value),
            seed_value: libm::sin(self.seed_value),
            gradient: self
                .gradient
                .iter()
                .map(|derivative| derivative * libm::cos(self.value))
                .collect(),
        }
    }

    pub(crate) fn cos(&self) -> Self {
        Self {
            value: libm::cos(self.value),
            seed_value: libm::cos(self.seed_value),
            gradient: self
                .gradient
                .iter()
                .map(|derivative| -derivative * libm::sin(self.value))
                .collect(),
        }
    }

    fn atan2(&self, x: &Self) -> Result<Self, ()> {
        let denominator = x.value * x.value + self.value * self.value;
        if denominator == 0.0 || !denominator.is_finite() {
            return Err(());
        }
        Ok(Self::from_binary(
            libm::atan2(self.value, x.value),
            libm::atan2(self.seed_value, x.seed_value),
            self,
            x,
            x.value / denominator,
            -self.value / denominator,
        ))
    }

    fn acos(&self) -> Result<Self, ()> {
        if !(-1.0..=1.0).contains(&self.value) {
            return Err(());
        }
        let denominator = libm::sqrt(1.0 - self.value * self.value);
        if denominator == 0.0 {
            return Err(());
        }
        Ok(Self {
            value: libm::acos(self.value),
            seed_value: libm::acos(self.seed_value),
            gradient: self
                .gradient
                .iter()
                .map(|derivative| -derivative / denominator)
                .collect(),
        })
    }

    pub(crate) fn abs(&self) -> Self {
        let mut result = if self.seed_value < 0.0 {
            self.neg()
        } else {
            self.clone()
        };
        result.value = self.value.abs();
        result.seed_value = self.seed_value.abs();
        result
    }

    pub(crate) fn max(&self, rhs: &Self) -> Self {
        let mut result = if self.seed_value >= rhs.seed_value {
            self.clone()
        } else {
            rhs.clone()
        };
        result.value = self.value.max(rhs.value);
        result.seed_value = self.seed_value.max(rhs.seed_value);
        result
    }

    pub(crate) fn min(&self, rhs: &Self) -> Self {
        let mut result = if self.seed_value <= rhs.seed_value {
            self.clone()
        } else {
            rhs.clone()
        };
        result.value = self.value.min(rhs.value);
        result.seed_value = self.seed_value.min(rhs.seed_value);
        result
    }

    fn from_binary(
        value: f64,
        seed_value: f64,
        lhs: &Self,
        rhs: &Self,
        lhs_factor: f64,
        rhs_factor: f64,
    ) -> Self {
        let mut gradient = Vec::with_capacity(lhs.gradient.len());
        for index in 0..lhs.gradient.len() {
            gradient.push(lhs.gradient[index] * lhs_factor + rhs.gradient[index] * rhs_factor);
        }
        Self {
            value,
            seed_value,
            gradient,
        }
    }

    fn normalize_angle(&self) -> Self {
        Self {
            value: normalize_angle(self.value),
            seed_value: normalize_angle(self.seed_value),
            gradient: self.gradient.clone(),
        }
    }

    pub(crate) fn with_live_value(mut self, live: &Self) -> Self {
        self.value = live.value;
        self
    }
}

#[derive(Clone, Debug)]
pub(crate) struct DPoint {
    pub(crate) x: Jet,
    pub(crate) y: Jet,
}

impl DPoint {
    pub(crate) fn sub(&self, rhs: &Self) -> DVector {
        DVector {
            x: self.x.sub(&rhs.x),
            y: self.y.sub(&rhs.y),
        }
    }

    fn add_vector(&self, rhs: &DVector) -> Self {
        Self {
            x: self.x.add(&rhs.x),
            y: self.y.add(&rhs.y),
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct DVector {
    pub(crate) x: Jet,
    pub(crate) y: Jet,
}

impl DVector {
    pub(crate) fn dot(&self, rhs: &Self) -> Jet {
        self.x.mul(&rhs.x).add(&self.y.mul(&rhs.y))
    }

    pub(crate) fn cross(&self, rhs: &Self) -> Jet {
        self.x.mul(&rhs.y).sub(&self.y.mul(&rhs.x))
    }

    pub(crate) fn norm(&self) -> Result<Jet, ()> {
        self.dot(self).sqrt()
    }

    fn add(&self, rhs: &Self) -> Self {
        Self {
            x: self.x.add(&rhs.x),
            y: self.y.add(&rhs.y),
        }
    }

    fn scale(&self, factor: &Jet) -> Self {
        Self {
            x: self.x.mul(factor),
            y: self.y.mul(factor),
        }
    }

    fn perpendicular(&self) -> Self {
        Self {
            x: self.y.neg(),
            y: self.x.clone(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LinearDomain {
    Line,
    Ray,
    Segment,
}

#[derive(Clone, Copy)]
struct ParameterInterval {
    lower: f64,
    upper: f64,
}

#[derive(Clone, Debug)]
pub(crate) struct DLinear {
    pub(crate) start: DPoint,
    pub(crate) through: DPoint,
    pub(crate) domain: LinearDomain,
}

impl DLinear {
    pub(crate) fn direction(&self) -> DVector {
        self.through.sub(&self.start)
    }
}

#[derive(Clone, Debug)]
pub(crate) struct DCircle {
    pub(crate) center: DPoint,
    pub(crate) radius: Jet,
}

#[derive(Clone, Debug)]
pub(crate) struct DArc {
    pub(crate) circle: DCircle,
    pub(crate) start: DPoint,
    pub(crate) end: DPoint,
    pub(crate) ccw: bool,
    pub(crate) seed_ccw: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct DEllipse {
    pub(crate) center: DPoint,
    pub(crate) semimajor: Jet,
    pub(crate) semiminor: Jet,
    pub(crate) rotation: Jet,
}

#[derive(Clone, Debug)]
pub(crate) struct DBspline {
    pub(crate) controls: Vec<DPoint>,
}

#[derive(Clone, Debug)]
pub(crate) enum DObject {
    Number(Jet, NumericDimension),
    Point(DPoint),
    Linear(DLinear),
    Circle(DCircle),
    Arc(DArc),
    Ellipse(DEllipse),
    Bspline(DBspline),
    Polygon(Vec<DPoint>, bool),
    RoundedRect(DRoundedRect),
    Annotation,
}

#[derive(Clone, Debug)]
#[allow(
    clippy::large_enum_variant,
    reason = "boxing differentiated objects would add allocation to every expression evaluation"
)]
enum DValue {
    Number(Jet, NumericDimension),
    Object(DObject),
    Bool(bool),
}

pub(crate) struct DifferentiatedFile {
    objects: BTreeMap<String, DObject>,
    unknown_count: usize,
    tolerance: Tolerance,
    inferred_dimensions: DimensionMap,
}

impl DifferentiatedFile {
    pub(crate) fn evaluate(
        file: &File,
        normalized: &[f64],
        normalized_seed: &[f64],
        scales: &[f64],
        dimensions: &BTreeMap<String, NumericDimension>,
        inferred_dimensions: &DimensionMap,
        tolerance: Tolerance,
    ) -> Result<Self, EvalError> {
        let mut evaluator = AdEvaluator {
            objects: BTreeMap::new(),
            normalized,
            normalized_seed,
            scales,
            dimensions,
            inferred_dimensions,
            tolerance,
            cursor: 0,
        };
        for statement in &file.statements {
            if let Statement::Op(op) = statement {
                evaluator.eval_op(op)?;
            }
        }
        if evaluator.cursor != normalized.len() {
            return Err(solver_error(file.span, "unknown vector length mismatch"));
        }
        Ok(Self {
            objects: evaluator.objects,
            unknown_count: normalized.len(),
            tolerance,
            inferred_dimensions: inferred_dimensions.clone(),
        })
    }

    pub(crate) fn object(&self, expression: &Expr) -> Result<DObject, EvalError> {
        let mut evaluator = RelationEvaluator::new(
            &self.objects,
            self.unknown_count,
            self.tolerance,
            &self.inferred_dimensions,
        );
        evaluator.eval_object(expression)
    }

    pub(crate) fn matching_numbers(
        &self,
        lhs: &Expr,
        rhs: &Expr,
    ) -> Result<(Jet, Jet, NumericDimension), EvalError> {
        let mut evaluator = RelationEvaluator::new(
            &self.objects,
            self.unknown_count,
            self.tolerance,
            &self.inferred_dimensions,
        );
        evaluator.eval_matching_numbers(lhs, rhs)
    }

    pub(crate) fn validate_branch_sensitive_primal(
        &self,
        file: &File,
        resolved: &indexmap::IndexMap<locus_syntax::Ident, ResolvedObject>,
    ) -> Result<(), EvalError> {
        for statement in &file.statements {
            let Statement::Op(op) = statement else {
                continue;
            };
            let Constructor::Call { name, .. } = &op.ctor else {
                continue;
            };
            if !matches!(name.name.as_str(), "intersect" | "tangent_line") {
                continue;
            }
            let differentiated = self
                .objects
                .get(&op.id.name)
                .ok_or_else(|| solver_error(op.id.span, "AD object is missing"))?;
            let actual = resolved
                .get(&op.id)
                .ok_or_else(|| solver_error(op.id.span, "resolved object is missing"))?;
            if !branch_sensitive_primal_matches(differentiated, actual, self.tolerance) {
                return Err(solver_error(
                    op.span,
                    format!(
                        "AD and geometry evaluators selected different `{}` branches",
                        name.name
                    ),
                ));
            }
        }
        Ok(())
    }
}

fn branch_sensitive_primal_matches(
    differentiated: &DObject,
    actual: &ResolvedObject,
    tolerance: Tolerance,
) -> bool {
    match (differentiated, actual) {
        (DObject::Point(lhs), ResolvedObject::Point(rhs)) => {
            point_primal_matches(lhs, rhs.geometry, tolerance)
        }
        (DObject::Linear(lhs), ResolvedObject::Line(rhs)) => {
            point_primal_matches(&lhs.start, rhs.start, tolerance)
                && point_primal_matches(&lhs.through, rhs.through, tolerance)
        }
        _ => false,
    }
}

fn point_primal_matches(lhs: &DPoint, rhs: crate::geom::Point, tolerance: Tolerance) -> bool {
    tolerance.length_eq(lhs.x.value, rhs.x) && tolerance.length_eq(lhs.y.value, rhs.y)
}

struct AdEvaluator<'a> {
    objects: BTreeMap<String, DObject>,
    normalized: &'a [f64],
    normalized_seed: &'a [f64],
    scales: &'a [f64],
    dimensions: &'a BTreeMap<String, NumericDimension>,
    inferred_dimensions: &'a DimensionMap,
    tolerance: Tolerance,
    cursor: usize,
}

impl AdEvaluator<'_> {
    fn eval_op(&mut self, op: &OpStmt) -> Result<(), EvalError> {
        let object = if crate::eval::is_placed_op(op, self.inferred_dimensions) {
            self.placed_object(op)?
        } else {
            self.eval_regular_op(op)?
        };
        self.objects.insert(op.id.name.clone(), object);
        Ok(())
    }

    fn eval_regular_op(&mut self, op: &OpStmt) -> Result<DObject, EvalError> {
        match &op.ctor {
            Constructor::FreeNum { .. } => self.free_number(op),
            Constructor::Expr { expr, .. } if op.kind == Kind::Num => {
                let expected = self
                    .dimensions
                    .get(&op.id.name)
                    .copied()
                    .unwrap_or(NumericDimension::Scalar);
                let mut evaluator = RelationEvaluator::new(
                    &self.objects,
                    self.normalized.len(),
                    self.tolerance,
                    self.inferred_dimensions,
                );
                let (value, dimension) = evaluator.eval_number_as(expr, expected)?;
                Ok(DObject::Number(value, dimension))
            }
            Constructor::Expr { expr, .. } => self.relation_evaluator().eval_object(expr),
            Constructor::Call { name, args, .. } => {
                let mut evaluator = self.relation_evaluator();
                evaluator.eval_constructor(&name.name, args, op.kind, op.span)
            }
        }
    }

    fn relation_evaluator(&self) -> RelationEvaluator<'_> {
        RelationEvaluator::new(
            &self.objects,
            self.normalized.len(),
            self.tolerance,
            self.inferred_dimensions,
        )
    }

    fn placed_object(&mut self, op: &OpStmt) -> Result<DObject, EvalError> {
        let center = DPoint {
            x: self.variable(op.span)?,
            y: self.variable(op.span)?,
        };
        let Constructor::Call { name, args, .. } = &op.ctor else {
            return Err(solver_error(op.span, "invalid placed declaration"));
        };
        let mut evaluator = self.relation_evaluator();
        match (op.kind, name.name.as_str()) {
            (Kind::Point, "point") => Ok(DObject::Point(center)),
            (Kind::Circle, "circle") => Ok(DObject::Circle(DCircle {
                center,
                radius: evaluator
                    .eval_number_as(&args[0], NumericDimension::Length)?
                    .0,
            })),
            (Kind::Ellipse, "ellipse") => evaluator
                .ellipse_with_center(center, args, op.span)
                .map(DObject::Ellipse),
            (Kind::RoundedRect, "rounded_rect") => evaluator.rounded_rect_at(center, args, op.span),
            (Kind::Rect | Kind::Diamond, "rect" | "diamond") => {
                evaluator.placed_polygon(&center, args, op.kind, op.span)
            }
            _ => Err(solver_error(op.span, "invalid placed declaration")),
        }
    }

    fn free_number(&mut self, op: &OpStmt) -> Result<DObject, EvalError> {
        let dimension = self
            .dimensions
            .get(&op.id.name)
            .copied()
            .unwrap_or(NumericDimension::Scalar);
        self.variable(op.span)
            .map(|value| DObject::Number(value, dimension))
    }

    fn variable(&mut self, span: Span) -> Result<Jet, EvalError> {
        let index = self.cursor;
        let normalized = self
            .normalized
            .get(index)
            .copied()
            .ok_or_else(|| solver_error(span, "unknown vector is too short"))?;
        let normalized_seed = self.normalized_seed[index];
        let scale = self.scales[index];
        self.cursor += 1;
        Ok(Jet::variable(
            normalized * scale,
            normalized_seed * scale,
            index,
            scale,
            self.normalized.len(),
        ))
    }
}

struct RelationEvaluator<'a> {
    objects: &'a BTreeMap<String, DObject>,
    count: usize,
    tolerance: Tolerance,
    inferred_dimensions: &'a DimensionMap,
}

impl<'a> RelationEvaluator<'a> {
    const fn new(
        objects: &'a BTreeMap<String, DObject>,
        count: usize,
        tolerance: Tolerance,
        inferred_dimensions: &'a DimensionMap,
    ) -> Self {
        Self {
            objects,
            count,
            tolerance,
            inferred_dimensions,
        }
    }

    fn eval(&mut self, expression: &Expr) -> Result<DValue, EvalError> {
        if let Some(value) = self.eval_leaf(expression) {
            return value;
        }
        self.eval_compound(expression)
    }

    fn eval_leaf(&self, expression: &Expr) -> Option<Result<DValue, EvalError>> {
        match expression {
            Expr::Number(number) => Some(Ok(self.number_literal(number))),
            Expr::Boolean { value, .. } => Some(Ok(DValue::Bool(*value))),
            Expr::Ref(id) => Some(
                self.objects
                    .get(&id.name)
                    .cloned()
                    .map(DValue::Object)
                    .ok_or_else(|| solver_error(id.span, format!("unknown object `{}`", id.name))),
            ),
            Expr::String(value) | Expr::Tex(value) => {
                Some(Err(solver_error(value.span, "text is not solver-valued")))
            }
            _ => None,
        }
    }

    fn eval_compound(&mut self, expression: &Expr) -> Result<DValue, EvalError> {
        match expression {
            Expr::Group { expr, .. } => self.eval(expr),
            Expr::Unary { op, expr, span } => self.eval_unary(*op, expr, *span),
            Expr::Binary { op, lhs, rhs, span } => self.eval_binary(*op, lhs, rhs, *span),
            Expr::Call { name, args, span } => self.eval_call(&name.name, args, *span),
            Expr::Property {
                base,
                name,
                args,
                span,
            } => self.eval_property(base, &name.name, args, *span),
            Expr::Number(_)
            | Expr::Boolean { .. }
            | Expr::Ref(_)
            | Expr::String(_)
            | Expr::Tex(_) => Err(solver_error(
                expression.span(),
                "internal expression dispatch error",
            )),
        }
    }

    fn number_literal(&self, number: &locus_syntax::NumberExpr) -> DValue {
        let (value, dimension) = match number.unit {
            Some(AngleUnit::Deg) => (
                number.value * core::f64::consts::PI / 180.0,
                NumericDimension::Angle,
            ),
            Some(AngleUnit::Rad) => (number.value, NumericDimension::Angle),
            None => (number.value, NumericDimension::Scalar),
        };
        DValue::Number(Jet::constant(value, self.count), dimension)
    }

    fn eval_unary(
        &mut self,
        op: UnaryOp,
        expression: &Expr,
        span: Span,
    ) -> Result<DValue, EvalError> {
        match (op, self.eval(expression)?) {
            (
                UnaryOp::Neg,
                DValue::Number(value, dimension)
                | DValue::Object(DObject::Number(value, dimension)),
            ) => Ok(DValue::Number(value.neg(), dimension)),
            (UnaryOp::Not, DValue::Bool(value)) => Ok(DValue::Bool(!value)),
            _ => Err(solver_error(span, "invalid unary solver expression")),
        }
    }

    fn eval_binary(
        &mut self,
        op: BinOp,
        lhs: &Expr,
        rhs: &Expr,
        span: Span,
    ) -> Result<DValue, EvalError> {
        if matches!(
            op,
            BinOp::Eq | BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge | BinOp::And | BinOp::Or
        ) {
            return Err(solver_error(
                span,
                "boolean expression must be expanded as a relation",
            ));
        }
        let (lhs, lhs_dimension) = self.eval_number(lhs)?;
        let (rhs, rhs_dimension) = self.eval_number(rhs)?;
        match op {
            BinOp::Add | BinOp::Sub => {
                Self::eval_sum(op, &lhs, lhs_dimension, &rhs, rhs_dimension, span)
            }
            BinOp::Mul => Ok(DValue::Number(
                lhs.mul(&rhs),
                multiply_dimension(lhs_dimension, rhs_dimension, span)?,
            )),
            BinOp::Div => Ok(DValue::Number(
                lhs.div(&rhs),
                divide_dimension(lhs_dimension, rhs_dimension, span)?,
            )),
            _ => Err(solver_error(span, "invalid arithmetic solver expression")),
        }
    }

    fn eval_sum(
        op: BinOp,
        lhs: &Jet,
        lhs_dimension: NumericDimension,
        rhs: &Jet,
        rhs_dimension: NumericDimension,
        span: Span,
    ) -> Result<DValue, EvalError> {
        let dimension = contextual_dimension(lhs_dimension, rhs_dimension, span)?;
        let value = if op == BinOp::Add {
            lhs.add(rhs)
        } else {
            lhs.sub(rhs)
        };
        Ok(DValue::Number(value, dimension))
    }

    fn eval_call(&mut self, name: &str, args: &[Expr], span: Span) -> Result<DValue, EvalError> {
        if is_numeric_function(name) {
            return self.eval_numeric_call(name, args, span);
        }
        self.eval_constructor(name, args, constructor_kind(name, span)?, span)
            .map(DValue::Object)
    }

    fn eval_numeric_call(
        &mut self,
        name: &str,
        args: &[Expr],
        span: Span,
    ) -> Result<DValue, EvalError> {
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
    ) -> Result<DValue, EvalError> {
        match name {
            "dist" | "hdist" | "vdist" => self.eval_distance(name, args, span),
            "angle" | "dir_angle" => self.eval_angle_function(name, args, span),
            "radius" => self.eval_radius(args, span),
            "length" => self.eval_length(args, span),
            "area" | "perimeter" => self.eval_shape_measure(name, args, span),
            _ => Err(solver_error(
                span,
                format!("unsupported geometry function `{name}`"),
            )),
        }
    }

    fn eval_scalar_numeric_call(
        &mut self,
        name: &str,
        args: &[Expr],
        span: Span,
    ) -> Result<DValue, EvalError> {
        match name {
            "ratio" => self.eval_ratio(args, span),
            "sqrt" | "abs" => self.eval_unary_numeric(name, args, span),
            "min" | "max" => self.eval_extremum(name, args, span),
            _ => Err(solver_error(
                span,
                format!("unsupported numeric function `{name}`"),
            )),
        }
    }

    fn eval_distance(
        &mut self,
        name: &str,
        args: &[Expr],
        span: Span,
    ) -> Result<DValue, EvalError> {
        require_arity(name, args, 2, span)?;
        let lhs = self.eval_point(&args[0])?;
        let rhs = self.eval_point(&args[1])?;
        let delta = rhs.sub(&lhs);
        let result = match name {
            "hdist" => delta.x,
            "vdist" => delta.y,
            _ => delta
                .norm()
                .map_err(|()| solver_error(span, "distance is undefined"))?,
        };
        Ok(DValue::Number(result, NumericDimension::Length))
    }

    fn eval_angle_function(
        &mut self,
        name: &str,
        args: &[Expr],
        span: Span,
    ) -> Result<DValue, EvalError> {
        let angle = if name == "dir_angle" {
            require_arity(name, args, 2, span)?;
            let direction = self.eval_point(&args[1])?.sub(&self.eval_point(&args[0])?);
            direction
                .y
                .atan2(&direction.x)
                .map(|value| value.normalize_angle())
        } else {
            require_arity(name, args, 3, span)?;
            let a = self.eval_point(&args[0])?;
            let b = self.eval_point(&args[1])?;
            let c = self.eval_point(&args[2])?;
            let first = a.sub(&b);
            let second = c.sub(&b);
            first.cross(&second).abs().atan2(&first.dot(&second))
        };
        angle
            .map(|value| DValue::Number(value, NumericDimension::Angle))
            .map_err(|()| solver_error(span, "angle is undefined for degenerate geometry"))
    }

    fn eval_radius(&mut self, args: &[Expr], span: Span) -> Result<DValue, EvalError> {
        require_arity("radius", args, 1, span)?;
        let circle = self.eval_circle(&args[0])?;
        Ok(DValue::Number(circle.radius, NumericDimension::Length))
    }

    fn eval_length(&mut self, args: &[Expr], span: Span) -> Result<DValue, EvalError> {
        require_arity("length", args, 1, span)?;
        let linear = self.eval_linear(&args[0])?;
        let length = linear
            .direction()
            .norm()
            .map_err(|()| solver_error(span, "zero segment"))?;
        Ok(DValue::Number(length, NumericDimension::Length))
    }

    fn eval_shape_measure(
        &mut self,
        name: &str,
        args: &[Expr],
        span: Span,
    ) -> Result<DValue, EvalError> {
        require_arity(name, args, 1, span)?;
        if let DObject::RoundedRect(rect) = self.eval_object(&args[0])? {
            return Ok(rect.measure(name));
        }
        if name == "area" {
            if let Ok(ellipse) = self.eval_ellipse(&args[0]) {
                let area = ellipse
                    .semimajor
                    .mul(&ellipse.semiminor)
                    .scale(core::f64::consts::PI);
                return Ok(DValue::Number(area, NumericDimension::Area));
            }
        }
        let vertices = self.eval_polygon(&args[0])?;
        let (value, dimension) = if name == "area" {
            (polygon_area(&vertices).abs(), NumericDimension::Area)
        } else {
            (
                polygon_perimeter(&vertices, span)?,
                NumericDimension::Length,
            )
        };
        Ok(DValue::Number(value, dimension))
    }

    fn eval_ratio(&mut self, args: &[Expr], span: Span) -> Result<DValue, EvalError> {
        require_arity("ratio", args, 2, span)?;
        let (lhs, rhs, _) = self.eval_matching_numbers(&args[0], &args[1])?;
        Ok(DValue::Number(lhs.div(&rhs), NumericDimension::Scalar))
    }

    fn eval_unary_numeric(
        &mut self,
        name: &str,
        args: &[Expr],
        span: Span,
    ) -> Result<DValue, EvalError> {
        require_arity(name, args, 1, span)?;
        let (value, dimension) = self.eval_number(&args[0])?;
        if name == "abs" {
            return Ok(DValue::Number(value.abs(), dimension));
        }
        let result = value
            .sqrt()
            .map_err(|()| solver_error(span, "sqrt domain error"))?;
        let dimension = match dimension {
            NumericDimension::Area => NumericDimension::Length,
            NumericDimension::Scalar => NumericDimension::Scalar,
            _ => return Err(solver_error(span, "sqrt requires scalar or area")),
        };
        Ok(DValue::Number(result, dimension))
    }

    fn eval_extremum(
        &mut self,
        name: &str,
        args: &[Expr],
        span: Span,
    ) -> Result<DValue, EvalError> {
        require_arity(name, args, 2, span)?;
        let (lhs, lhs_dimension) = self.eval_number(&args[0])?;
        let (rhs, rhs_dimension) = self.eval_number(&args[1])?;
        let dimension = contextual_dimension(lhs_dimension, rhs_dimension, span)?;
        let result = if name == "min" {
            lhs.min(&rhs)
        } else {
            lhs.max(&rhs)
        };
        Ok(DValue::Number(result, dimension))
    }

    pub(crate) fn eval_number(
        &mut self,
        expression: &Expr,
    ) -> Result<(Jet, NumericDimension), EvalError> {
        match self.eval(expression)? {
            DValue::Number(value, dimension)
            | DValue::Object(DObject::Number(value, dimension)) => Ok((value, dimension)),
            _ => Err(solver_error(
                expression.span(),
                "expected a numeric expression",
            )),
        }
    }

    fn eval_number_as(
        &mut self,
        expression: &Expr,
        expected: NumericDimension,
    ) -> Result<(Jet, NumericDimension), EvalError> {
        match expression {
            Expr::Number(number) => self.number_literal_as(number, expected),
            Expr::Group { expr, .. } => self.eval_number_as(expr, expected),
            Expr::Unary {
                op: UnaryOp::Neg,
                expr,
                ..
            } => self
                .eval_number_as(expr, expected)
                .map(|(value, dimension)| (value.neg(), dimension)),
            Expr::Binary { op, lhs, rhs, span } => {
                self.eval_number_binary_as(*op, lhs, rhs, expected, *span)
            }
            Expr::Call { name, args, span } => self
                .eval_numeric_call_as(&name.name, args, expected, *span)
                .unwrap_or_else(|| self.eval_number_then_require(expression, expected)),
            _ => self.eval_number_then_require(expression, expected),
        }
    }

    fn eval_matching_numbers(
        &mut self,
        lhs: &Expr,
        rhs: &Expr,
    ) -> Result<(Jet, Jet, NumericDimension), EvalError> {
        if let Some(result) = self.eval_inferred_pair(lhs, rhs) {
            return result;
        }
        match (self.eval_number(lhs), self.eval_number(rhs)) {
            (Ok(lhs_value), Ok(rhs_value)) => {
                self.match_evaluated_numbers(lhs, rhs, lhs_value, rhs_value)
            }
            (Err(error), Ok((_, dimension))) | (Ok((_, dimension)), Err(error)) => {
                self.eval_number_pair_as(lhs, rhs, dimension).ok_or(error)
            }
            (Err(error), Err(_)) => {
                for dimension in numeric_dimensions() {
                    if let Some(pair) = self.eval_number_pair_as(lhs, rhs, dimension) {
                        return Ok(pair);
                    }
                }
                Err(error)
            }
        }
    }

    fn eval_inferred_pair(
        &mut self,
        lhs: &Expr,
        rhs: &Expr,
    ) -> Option<Result<(Jet, Jet, NumericDimension), EvalError>> {
        let dimension = self.inferred_pair_dimension(lhs, rhs)?;
        Some(self.eval_number_pair_as_result(lhs, rhs, dimension))
    }

    fn inferred_pair_dimension(&self, lhs: &Expr, rhs: &Expr) -> Option<NumericDimension> {
        match (
            self.inferred_dimensions.expression_dimension(lhs),
            self.inferred_dimensions.expression_dimension(rhs),
        ) {
            (Some(lhs), Some(rhs)) if lhs == rhs => Some(numeric_dimension(lhs)),
            (Some(dimension), None) | (None, Some(dimension)) => Some(numeric_dimension(dimension)),
            _ => None,
        }
    }

    fn match_evaluated_numbers(
        &mut self,
        lhs: &Expr,
        rhs: &Expr,
        lhs_value: (Jet, NumericDimension),
        rhs_value: (Jet, NumericDimension),
    ) -> Result<(Jet, Jet, NumericDimension), EvalError> {
        if lhs_value.1 == rhs_value.1 {
            return Ok((lhs_value.0, rhs_value.0, lhs_value.1));
        }
        for dimension in preferred_numeric_dimensions(lhs_value.1, rhs_value.1) {
            if let Some(pair) = self.eval_number_pair_as(lhs, rhs, dimension) {
                return Ok(pair);
            }
        }
        let span = lhs.span().join(rhs.span());
        contextual_dimension(lhs_value.1, rhs_value.1, span)
            .map(|dimension| (lhs_value.0, rhs_value.0, dimension))
    }

    fn eval_number_pair_as(
        &mut self,
        lhs: &Expr,
        rhs: &Expr,
        dimension: NumericDimension,
    ) -> Option<(Jet, Jet, NumericDimension)> {
        self.eval_number_pair_as_result(lhs, rhs, dimension).ok()
    }

    fn eval_number_pair_as_result(
        &mut self,
        lhs: &Expr,
        rhs: &Expr,
        dimension: NumericDimension,
    ) -> Result<(Jet, Jet, NumericDimension), EvalError> {
        let (lhs, _) = self.eval_number_as(lhs, dimension)?;
        let (rhs, _) = self.eval_number_as(rhs, dimension)?;
        Ok((lhs, rhs, dimension))
    }

    fn eval_number_binary_as(
        &mut self,
        op: BinOp,
        lhs: &Expr,
        rhs: &Expr,
        expected: NumericDimension,
        span: Span,
    ) -> Result<(Jet, NumericDimension), EvalError> {
        match op {
            BinOp::Add | BinOp::Sub => self.eval_sum_as(op, lhs, rhs, expected, span),
            BinOp::Mul | BinOp::Div => self.eval_product_as(op, lhs, rhs, expected, span),
            _ => self
                .eval_number_binary(op, lhs, rhs, span)
                .and_then(|(value, dimension)| {
                    require_numeric_dimension(value, dimension, expected, span)
                }),
        }
    }

    fn eval_numeric_call_as(
        &mut self,
        name: &str,
        args: &[Expr],
        expected: NumericDimension,
        span: Span,
    ) -> Option<Result<(Jet, NumericDimension), EvalError>> {
        match name {
            "min" | "max" => Some(self.eval_extremum_as(name, args, expected, span)),
            "abs" => Some(self.eval_abs_as(args, expected, span)),
            "sqrt" => Some(self.eval_sqrt_as(args, expected, span)),
            _ => None,
        }
    }

    fn eval_number_then_require(
        &mut self,
        expression: &Expr,
        expected: NumericDimension,
    ) -> Result<(Jet, NumericDimension), EvalError> {
        let (value, dimension) = self.eval_number(expression)?;
        require_numeric_dimension(value, dimension, expected, expression.span())
    }

    fn number_literal_as(
        &self,
        number: &locus_syntax::NumberExpr,
        expected: NumericDimension,
    ) -> Result<(Jet, NumericDimension), EvalError> {
        match (number.unit, expected) {
            (None, NumericDimension::Angle) => Err(solver_error(
                number.span,
                "angle literals require an explicit `deg` or `rad` unit",
            )),
            (None, dimension) => Ok((Jet::constant(number.value, self.count), dimension)),
            (Some(_), NumericDimension::Angle) => {
                let DValue::Number(value, dimension) = self.number_literal(number) else {
                    unreachable!("numeric literal expected");
                };
                Ok((value, dimension))
            }
            (Some(_), _) => Err(solver_error(
                number.span,
                "angle literal used in a non-angle context",
            )),
        }
    }

    fn eval_sum_as(
        &mut self,
        op: BinOp,
        lhs: &Expr,
        rhs: &Expr,
        expected: NumericDimension,
        span: Span,
    ) -> Result<(Jet, NumericDimension), EvalError> {
        let (lhs, lhs_dimension) = self.eval_number_as(lhs, expected)?;
        let (rhs, rhs_dimension) = self.eval_number_as(rhs, expected)?;
        let DValue::Number(value, dimension) =
            Self::eval_sum(op, &lhs, lhs_dimension, &rhs, rhs_dimension, span)?
        else {
            unreachable!("numeric sum expected");
        };
        Ok((value, dimension))
    }

    fn eval_product_as(
        &mut self,
        op: BinOp,
        lhs: &Expr,
        rhs: &Expr,
        expected: NumericDimension,
        span: Span,
    ) -> Result<(Jet, NumericDimension), EvalError> {
        for &(lhs_expected, rhs_expected) in numeric_product_operand_dimensions(op, expected) {
            let Ok((lhs, lhs_dimension)) = self.eval_number_as(lhs, lhs_expected) else {
                continue;
            };
            let Ok((rhs, rhs_dimension)) = self.eval_number_as(rhs, rhs_expected) else {
                continue;
            };
            let dimension = if op == BinOp::Mul {
                multiply_dimension(lhs_dimension, rhs_dimension, span)?
            } else {
                divide_dimension(lhs_dimension, rhs_dimension, span)?
            };
            let value = if op == BinOp::Mul {
                lhs.mul(&rhs)
            } else {
                lhs.div(&rhs)
            };
            return require_numeric_dimension(value, dimension, expected, span);
        }
        let (value, dimension) = self.eval_number_binary(op, lhs, rhs, span)?;
        require_numeric_dimension(value, dimension, expected, span)
    }

    fn eval_number_binary(
        &mut self,
        op: BinOp,
        lhs: &Expr,
        rhs: &Expr,
        span: Span,
    ) -> Result<(Jet, NumericDimension), EvalError> {
        let DValue::Number(value, dimension) = self.eval_binary(op, lhs, rhs, span)? else {
            unreachable!("numeric product expected");
        };
        Ok((value, dimension))
    }

    fn eval_extremum_as(
        &mut self,
        name: &str,
        args: &[Expr],
        expected: NumericDimension,
        span: Span,
    ) -> Result<(Jet, NumericDimension), EvalError> {
        require_arity(name, args, 2, span)?;
        let (lhs, _) = self.eval_number_as(&args[0], expected)?;
        let (rhs, _) = self.eval_number_as(&args[1], expected)?;
        let value = if name == "min" {
            lhs.min(&rhs)
        } else {
            lhs.max(&rhs)
        };
        Ok((value, expected))
    }

    fn eval_abs_as(
        &mut self,
        args: &[Expr],
        expected: NumericDimension,
        span: Span,
    ) -> Result<(Jet, NumericDimension), EvalError> {
        require_arity("abs", args, 1, span)?;
        self.eval_number_as(&args[0], expected)
            .map(|(value, dimension)| (value.abs(), dimension))
    }

    fn eval_sqrt_as(
        &mut self,
        args: &[Expr],
        expected: NumericDimension,
        span: Span,
    ) -> Result<(Jet, NumericDimension), EvalError> {
        require_arity("sqrt", args, 1, span)?;
        let operand_dimension = match expected {
            NumericDimension::Scalar => NumericDimension::Scalar,
            NumericDimension::Length => NumericDimension::Area,
            NumericDimension::Area | NumericDimension::Angle => {
                return Err(solver_error(span, "sqrt requires scalar or area"));
            }
        };
        let (value, _) = self.eval_number_as(&args[0], operand_dimension)?;
        value
            .sqrt()
            .map(|value| (value, expected))
            .map_err(|()| solver_error(span, "sqrt domain error"))
    }

    fn eval_object(&mut self, expression: &Expr) -> Result<DObject, EvalError> {
        match self.eval(expression)? {
            DValue::Object(object) => Ok(object),
            _ => Err(solver_error(
                expression.span(),
                "expected a geometry object",
            )),
        }
    }

    fn eval_point(&mut self, expression: &Expr) -> Result<DPoint, EvalError> {
        match self.eval_object(expression)? {
            DObject::Point(point) => Ok(point),
            _ => Err(solver_error(expression.span(), "expected a point")),
        }
    }

    fn eval_linear(&mut self, expression: &Expr) -> Result<DLinear, EvalError> {
        match self.eval_object(expression)? {
            DObject::Linear(linear) => Ok(linear),
            _ => Err(solver_error(expression.span(), "expected a linear object")),
        }
    }

    fn eval_circle(&mut self, expression: &Expr) -> Result<DCircle, EvalError> {
        match self.eval_object(expression)? {
            DObject::Circle(circle) => Ok(circle),
            _ => Err(solver_error(expression.span(), "expected a circle")),
        }
    }

    fn eval_polygon(&mut self, expression: &Expr) -> Result<Vec<DPoint>, EvalError> {
        match self.eval_object(expression)? {
            DObject::Polygon(vertices, _) => Ok(vertices),
            _ => Err(solver_error(expression.span(), "expected a polygon")),
        }
    }

    fn eval_ellipse(&mut self, expression: &Expr) -> Result<DEllipse, EvalError> {
        match self.eval_object(expression)? {
            DObject::Ellipse(ellipse) => Ok(ellipse),
            _ => Err(solver_error(expression.span(), "expected an ellipse")),
        }
    }

    fn ellipse_with_center(
        &mut self,
        center: DPoint,
        args: &[Expr],
        span: Span,
    ) -> Result<DEllipse, EvalError> {
        if !matches!(args.len(), 2 | 3) {
            return Err(solver_error(
                span,
                "ellipse expects two semiaxes and optional rotation",
            ));
        }
        let semimajor = self.eval_number_as(&args[0], NumericDimension::Length)?.0;
        let semiminor = self.eval_number_as(&args[1], NumericDimension::Length)?.0;
        if semiminor.value <= 0.0 || semimajor.value < semiminor.value {
            return Err(solver_error(
                span,
                "ellipse semiaxes must satisfy a >= b > 0",
            ));
        }
        let rotation = if let Some(value) = args.get(2) {
            self.eval_number_as(value, NumericDimension::Angle)?.0
        } else {
            Jet::constant(0.0, self.count)
        };
        Ok(DEllipse {
            center,
            semimajor,
            semiminor,
            rotation,
        })
    }

    fn placed_polygon(
        &mut self,
        center: &DPoint,
        args: &[Expr],
        kind: Kind,
        span: Span,
    ) -> Result<DObject, EvalError> {
        require_arity(kind.as_str(), args, 2, span)?;
        let width = self.eval_number_as(&args[0], NumericDimension::Length)?.0;
        let height = self.eval_number_as(&args[1], NumericDimension::Length)?.0;
        let vertices = if kind == Kind::Rect {
            rect_vertices(center, &width, &height)
        } else {
            diamond_vertices(center, &width, &height)
        };
        Ok(DObject::Polygon(vertices, true))
    }

    fn eval_constructor(
        &mut self,
        name: &str,
        args: &[Expr],
        kind: Kind,
        span: Span,
    ) -> Result<DObject, EvalError> {
        match kind {
            Kind::Point => self
                .eval_point_constructor(name, args, span)
                .map(DObject::Point),
            Kind::Line | Kind::Ray | Kind::Segment => self
                .eval_linear_constructor(name, args, span)
                .map(DObject::Linear),
            Kind::Circle => self
                .eval_circle_constructor(name, args, span)
                .map(DObject::Circle),
            Kind::Arc => self
                .eval_arc_constructor(name, args, span)
                .map(DObject::Arc),
            Kind::Polygon | Kind::Rect | Kind::RoundedRect | Kind::Diamond => {
                self.eval_polygon_constructor(name, args, span)
            }
            Kind::Label | Kind::Arrow | Kind::Tick | Kind::RightAngle | Kind::TextBlock => {
                Ok(DObject::Annotation)
            }
            _ => self.eval_remaining_constructor(name, args, kind, span),
        }
    }

    fn eval_remaining_constructor(
        &mut self,
        name: &str,
        args: &[Expr],
        kind: Kind,
        span: Span,
    ) -> Result<DObject, EvalError> {
        if matches!(kind, Kind::Ellipse | Kind::Bspline) {
            self.eval_curve_constructor(name, args, kind, span)
        } else {
            Err(solver_error(span, "numeric constructor kind mismatch"))
        }
    }

    fn eval_curve_constructor(
        &mut self,
        name: &str,
        args: &[Expr],
        kind: Kind,
        span: Span,
    ) -> Result<DObject, EvalError> {
        match (kind, name) {
            (Kind::Ellipse, "ellipse") => {
                self.eval_derived_ellipse(args, span).map(DObject::Ellipse)
            }
            (Kind::Bspline, "bspline") => self.eval_bspline(args, span).map(DObject::Bspline),
            _ => Err(solver_error(span, "invalid curve constructor")),
        }
    }

    fn eval_derived_ellipse(&mut self, args: &[Expr], span: Span) -> Result<DEllipse, EvalError> {
        if !matches!(args.len(), 3 | 4) {
            return Err(solver_error(
                span,
                "derived ellipse expects center, a, b [, rotation]",
            ));
        }
        let center = self.eval_point(&args[0])?;
        self.ellipse_with_center(center, &args[1..], span)
    }

    fn eval_bspline(&mut self, args: &[Expr], span: Span) -> Result<DBspline, EvalError> {
        if args.len() < 4 {
            return Err(solver_error(
                span,
                "B-spline requires at least four controls",
            ));
        }
        let controls = args
            .iter()
            .map(|argument| self.eval_point(argument))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(DBspline { controls })
    }

    fn eval_point_constructor(
        &mut self,
        name: &str,
        args: &[Expr],
        span: Span,
    ) -> Result<DPoint, EvalError> {
        if matches!(
            name,
            "midpoint" | "lerp" | "offset" | "translate" | "extend"
        ) {
            return self.eval_affine_point(name, args, span);
        }
        if matches!(
            name,
            "foot" | "project" | "rotate" | "reflect" | "on_circle"
        ) {
            return self.eval_transform_point(name, args, span);
        }
        self.eval_center_point(name, args, span)
    }

    fn eval_affine_point(
        &mut self,
        name: &str,
        args: &[Expr],
        span: Span,
    ) -> Result<DPoint, EvalError> {
        match name {
            "midpoint" => self.affine_pair(args, span, 0.5),
            "lerp" => self.lerp(args, span),
            "offset" => self.offset(args, span),
            "translate" => self.translate(args, span),
            "extend" => self.extend(args, span),
            _ => Err(solver_error(
                span,
                format!("unsupported affine point constructor `{name}`"),
            )),
        }
    }

    fn eval_transform_point(
        &mut self,
        name: &str,
        args: &[Expr],
        span: Span,
    ) -> Result<DPoint, EvalError> {
        match name {
            "foot" => self.foot(args, span),
            "project" => self.project(args, span),
            "rotate" => self.rotate(args, span),
            "reflect" => self.reflect(args, span),
            "on_circle" => self.on_circle(args, span),
            _ => Err(solver_error(
                span,
                format!("unsupported point transform `{name}`"),
            )),
        }
    }

    fn eval_center_point(
        &mut self,
        name: &str,
        args: &[Expr],
        span: Span,
    ) -> Result<DPoint, EvalError> {
        match name {
            "centroid" => self.centroid(args, span),
            "circumcenter" | "incenter" | "orthocenter" => self.triangle_center(name, args, span),
            "center" => self.object_center(args, span),
            "intersect" => self.intersection(args, span),
            _ => Err(solver_error(
                span,
                format!("unsupported point constructor `{name}`"),
            )),
        }
    }

    fn affine_pair(&mut self, args: &[Expr], span: Span, factor: f64) -> Result<DPoint, EvalError> {
        require_arity("midpoint", args, 2, span)?;
        let first = self.eval_point(&args[0])?;
        let second = self.eval_point(&args[1])?;
        Ok(first.add_vector(&second.sub(&first).scale(&Jet::constant(factor, self.count))))
    }

    fn lerp(&mut self, args: &[Expr], span: Span) -> Result<DPoint, EvalError> {
        require_arity("lerp", args, 3, span)?;
        let first = self.eval_point(&args[0])?;
        let second = self.eval_point(&args[1])?;
        let factor = self.eval_number_as(&args[2], NumericDimension::Scalar)?.0;
        Ok(first.add_vector(&second.sub(&first).scale(&factor)))
    }

    fn offset(&mut self, args: &[Expr], span: Span) -> Result<DPoint, EvalError> {
        require_arity("offset", args, 3, span)?;
        let point = self.eval_point(&args[0])?;
        Ok(DPoint {
            x: point
                .x
                .add(&self.eval_number_as(&args[1], NumericDimension::Length)?.0),
            y: point
                .y
                .add(&self.eval_number_as(&args[2], NumericDimension::Length)?.0),
        })
    }

    fn translate(&mut self, args: &[Expr], span: Span) -> Result<DPoint, EvalError> {
        require_arity("translate", args, 3, span)?;
        let point = self.eval_point(&args[0])?;
        let from = self.eval_point(&args[1])?;
        let to = self.eval_point(&args[2])?;
        Ok(point.add_vector(&to.sub(&from)))
    }

    fn extend(&mut self, args: &[Expr], span: Span) -> Result<DPoint, EvalError> {
        require_arity("extend", args, 3, span)?;
        let first = self.eval_point(&args[0])?;
        let second = self.eval_point(&args[1])?;
        let extension = self.eval_number_as(&args[2], NumericDimension::Length)?.0;
        let direction = second.sub(&first);
        let length = direction
            .norm()
            .map_err(|()| solver_error(span, "extend direction is zero"))?;
        let factor = length.add(&extension).div(&length);
        Ok(first.add_vector(&direction.scale(&factor)))
    }

    fn foot(&mut self, args: &[Expr], span: Span) -> Result<DPoint, EvalError> {
        require_arity("foot", args, 2, span)?;
        let point = self.eval_point(&args[0])?;
        let linear = self.eval_linear(&args[1])?;
        closest_point(&point, &linear, span)
    }

    fn project(&mut self, args: &[Expr], span: Span) -> Result<DPoint, EvalError> {
        require_arity("project", args, 2, span)?;
        let point = self.eval_point(&args[0])?;
        let circle = self.eval_circle(&args[1])?;
        let radial = point.sub(&circle.center);
        let norm = radial
            .norm()
            .map_err(|()| solver_error(span, "project at circle center"))?;
        Ok(circle
            .center
            .add_vector(&radial.scale(&circle.radius.div(&norm))))
    }

    fn rotate(&mut self, args: &[Expr], span: Span) -> Result<DPoint, EvalError> {
        require_arity("rotate", args, 3, span)?;
        let point = self.eval_point(&args[0])?;
        let origin = self.eval_point(&args[1])?;
        let angle = self.eval_number_as(&args[2], NumericDimension::Angle)?.0;
        let relative = point.sub(&origin);
        let cosine = angle.cos();
        let sine = angle.sin();
        Ok(DPoint {
            x: origin
                .x
                .add(&relative.x.mul(&cosine).sub(&relative.y.mul(&sine))),
            y: origin
                .y
                .add(&relative.x.mul(&sine).add(&relative.y.mul(&cosine))),
        })
    }

    fn reflect(&mut self, args: &[Expr], span: Span) -> Result<DPoint, EvalError> {
        require_arity("reflect", args, 2, span)?;
        let point = self.eval_point(&args[0])?;
        match self.eval_object(&args[1])? {
            DObject::Point(center) => Ok(center.add_vector(&center.sub(&point))),
            DObject::Linear(linear) => {
                let projection = closest_point(&point, &linear, span)?;
                Ok(projection.add_vector(&projection.sub(&point)))
            }
            _ => Err(solver_error(span, "reflect requires a point or line")),
        }
    }

    fn on_circle(&mut self, args: &[Expr], span: Span) -> Result<DPoint, EvalError> {
        require_arity("on_circle", args, 2, span)?;
        let circle = self.eval_circle(&args[0])?;
        let angle = self.eval_number_as(&args[1], NumericDimension::Angle)?.0;
        Ok(DPoint {
            x: circle.center.x.add(&circle.radius.mul(&angle.cos())),
            y: circle.center.y.add(&circle.radius.mul(&angle.sin())),
        })
    }

    #[allow(
        clippy::cast_precision_loss,
        reason = "geometry arity is far below the f64 exact-integer range"
    )]
    fn centroid(&mut self, args: &[Expr], span: Span) -> Result<DPoint, EvalError> {
        if args.is_empty() {
            return Err(solver_error(span, "centroid requires points"));
        }
        let mut x = Jet::constant(0.0, self.count);
        let mut y = Jet::constant(0.0, self.count);
        for argument in args {
            let point = self.eval_point(argument)?;
            x = x.add(&point.x);
            y = y.add(&point.y);
        }
        let inverse = 1.0 / args.len() as f64;
        Ok(DPoint {
            x: x.scale(inverse),
            y: y.scale(inverse),
        })
    }

    fn triangle_center(
        &mut self,
        name: &str,
        args: &[Expr],
        span: Span,
    ) -> Result<DPoint, EvalError> {
        require_arity(name, args, 3, span)?;
        let a = self.eval_point(&args[0])?;
        let b = self.eval_point(&args[1])?;
        let c = self.eval_point(&args[2])?;
        let circumcenter = || circumcenter(&a, &b, &c, span);
        match name {
            "circumcenter" => circumcenter(),
            "incenter" => incenter(&a, &b, &c, span),
            "orthocenter" => {
                let center = circumcenter()?;
                let ab = b.sub(&a);
                let ac = c.sub(&a);
                let local_center = center.sub(&a);
                Ok(DPoint {
                    x: a.x.add(&ab.x.add(&ac.x).sub(&local_center.x.scale(2.0))),
                    y: a.y.add(&ab.y.add(&ac.y).sub(&local_center.y.scale(2.0))),
                })
            }
            _ => Err(solver_error(span, "unknown triangle center")),
        }
    }

    fn object_center(&mut self, args: &[Expr], span: Span) -> Result<DPoint, EvalError> {
        require_arity("center", args, 1, span)?;
        match self.eval_object(&args[0])? {
            DObject::Circle(circle) => Ok(circle.center),
            DObject::Arc(arc) => Ok(arc.circle.center),
            _ => Err(solver_error(span, "center expects circle or arc")),
        }
    }

    fn intersection(&mut self, args: &[Expr], span: Span) -> Result<DPoint, EvalError> {
        if !(2..=3).contains(&args.len()) {
            return Err(solver_error(
                span,
                "intersect expects two or three arguments",
            ));
        }
        let lhs = self.eval_object(&args[0])?;
        let rhs = self.eval_object(&args[1])?;
        let candidates = intersection_candidates(&lhs, &rhs, span, self.tolerance)?;
        let branch = args
            .get(2)
            .map(|expression| self.branch_index(expression))
            .transpose()?;
        select_intersection(candidates, branch, span)
    }

    fn branch_index(&mut self, expression: &Expr) -> Result<usize, EvalError> {
        let value = self
            .eval_number_as(expression, NumericDimension::Scalar)?
            .0
            .value;
        if value < 0.0 || value.fract() != 0.0 {
            return Err(solver_error(
                expression.span(),
                "branch index must be a nonnegative integer",
            ));
        }
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        Ok(value as usize)
    }

    fn eval_linear_constructor(
        &mut self,
        name: &str,
        args: &[Expr],
        span: Span,
    ) -> Result<DLinear, EvalError> {
        if matches!(name, "line" | "ray" | "seg") {
            require_arity(name, args, 2, span)?;
            let domain = match name {
                "line" => LinearDomain::Line,
                "ray" => LinearDomain::Ray,
                _ => LinearDomain::Segment,
            };
            return Ok(DLinear {
                start: self.eval_point(&args[0])?,
                through: self.eval_point(&args[1])?,
                domain,
            });
        }
        self.eval_derived_line(name, args, span)
    }

    fn eval_derived_line(
        &mut self,
        name: &str,
        args: &[Expr],
        span: Span,
    ) -> Result<DLinear, EvalError> {
        match name {
            "parallel" | "perpendicular" => self.relative_line(name, args, span),
            "perp_bisector" => self.perp_bisector(args, span),
            "angle_bisector" => self.angle_bisector(args, span),
            "tangent_line" => self.tangent_line(args, span),
            _ => Err(solver_error(
                span,
                format!("unsupported line constructor `{name}`"),
            )),
        }
    }

    fn relative_line(
        &mut self,
        name: &str,
        args: &[Expr],
        span: Span,
    ) -> Result<DLinear, EvalError> {
        require_arity(name, args, 2, span)?;
        let point = self.eval_point(&args[0])?;
        let source = self.eval_linear(&args[1])?;
        let direction = if name == "parallel" {
            source.direction()
        } else {
            source.direction().perpendicular()
        };
        Ok(DLinear {
            start: point.clone(),
            through: point.add_vector(&direction),
            domain: LinearDomain::Line,
        })
    }

    fn perp_bisector(&mut self, args: &[Expr], span: Span) -> Result<DLinear, EvalError> {
        require_arity("perp_bisector", args, 2, span)?;
        let first = self.eval_point(&args[0])?;
        let second = self.eval_point(&args[1])?;
        let midpoint = first.add_vector(&second.sub(&first).scale(&Jet::constant(0.5, self.count)));
        let direction = second.sub(&first).perpendicular();
        Ok(DLinear {
            start: midpoint.clone(),
            through: midpoint.add_vector(&direction),
            domain: LinearDomain::Line,
        })
    }

    fn angle_bisector(&mut self, args: &[Expr], span: Span) -> Result<DLinear, EvalError> {
        require_arity("angle_bisector", args, 3, span)?;
        let a = self.eval_point(&args[0])?;
        let b = self.eval_point(&args[1])?;
        let c = self.eval_point(&args[2])?;
        let first_vector = a.sub(&b);
        let second_vector = c.sub(&b);
        let first_length = first_vector
            .norm()
            .map_err(|()| solver_error(span, "zero direction"))?;
        let second_length = second_vector
            .norm()
            .map_err(|()| solver_error(span, "zero direction"))?;
        let one = Jet::constant(1.0, self.count);
        let first = first_vector.scale(&one.div(&first_length));
        let second = second_vector.scale(&one.div(&second_length));
        let direction = first.add(&second);
        let direction_length = direction
            .norm()
            .map_err(|()| solver_error(span, "straight angle has no unique interior bisector"))?;
        if self
            .tolerance
            .construction_scalar_zero(direction_length.value)
        {
            return Err(solver_error(
                span,
                "straight angle has no unique interior bisector",
            ));
        }
        let carrier_length = first_length.max(&second_length);
        let carrier_direction = direction.scale(&carrier_length.div(&direction_length));
        Ok(DLinear {
            start: b.clone(),
            through: b.add_vector(&carrier_direction),
            domain: LinearDomain::Line,
        })
    }

    fn tangent_line(&mut self, args: &[Expr], span: Span) -> Result<DLinear, EvalError> {
        require_arity("tangent_line", args, 3, span)?;
        let point = self.eval_point(&args[0])?;
        let circle = self.eval_circle(&args[1])?;
        let branch = self.branch_index(&args[2])?;
        if branch > 1 {
            return Err(solver_error(span, "tangent_line branch must be 0 or 1"));
        }
        let radial = point.sub(&circle.center);
        let distance = radial
            .norm()
            .map_err(|()| solver_error(span, "tangent point equals circle center"))?;
        if distance.value < circle.radius.value
            || self
                .tolerance
                .construction_length_eq(distance.value, circle.radius.value)
        {
            return Err(solver_error(
                span,
                "tangent_line point must be outside the circle",
            ));
        }
        let base = radial
            .y
            .atan2(&radial.x)
            .map_err(|()| solver_error(span, "tangent direction is undefined"))?;
        let offset = circle
            .radius
            .div(&distance)
            .acos()
            .map_err(|()| solver_error(span, "tangent angle is undefined"))?;
        let mut contacts = [
            point_on_circle(&circle, &base.sub(&offset)),
            point_on_circle(&circle, &base.add(&offset)),
        ];
        contacts.sort_by(|lhs, rhs| {
            normalized_point_angle(lhs, &circle.center)
                .total_cmp(&normalized_point_angle(rhs, &circle.center))
        });
        Ok(DLinear {
            start: point,
            through: contacts[branch].clone(),
            domain: LinearDomain::Line,
        })
    }

    fn eval_circle_constructor(
        &mut self,
        name: &str,
        args: &[Expr],
        span: Span,
    ) -> Result<DCircle, EvalError> {
        match name {
            "circle" => {
                require_arity(name, args, 2, span)?;
                Ok(DCircle {
                    center: self.eval_point(&args[0])?,
                    radius: self.eval_number_as(&args[1], NumericDimension::Length)?.0,
                })
            }
            "circle_through" => self.circle_through(args, span),
            "circle_diameter" => self.circle_diameter(args, span),
            "circumcircle" | "incircle" => self.triangle_circle(name, args, span),
            _ => Err(solver_error(
                span,
                format!("unsupported circle constructor `{name}`"),
            )),
        }
    }

    fn circle_through(&mut self, args: &[Expr], span: Span) -> Result<DCircle, EvalError> {
        require_arity("circle_through", args, 2, span)?;
        let center = self.eval_point(&args[0])?;
        let point = self.eval_point(&args[1])?;
        let radius = point
            .sub(&center)
            .norm()
            .map_err(|()| solver_error(span, "zero circle radius"))?;
        Ok(DCircle { center, radius })
    }

    fn circle_diameter(&mut self, args: &[Expr], span: Span) -> Result<DCircle, EvalError> {
        require_arity("circle_diameter", args, 2, span)?;
        let first = self.eval_point(&args[0])?;
        let second = self.eval_point(&args[1])?;
        let radius = second
            .sub(&first)
            .norm()
            .map_err(|()| solver_error(span, "zero diameter"))?
            .scale(0.5);
        let center = first.add_vector(&second.sub(&first).scale(&Jet::constant(0.5, self.count)));
        Ok(DCircle { center, radius })
    }

    fn triangle_circle(
        &mut self,
        name: &str,
        args: &[Expr],
        span: Span,
    ) -> Result<DCircle, EvalError> {
        require_arity(name, args, 3, span)?;
        let a = self.eval_point(&args[0])?;
        let b = self.eval_point(&args[1])?;
        let c = self.eval_point(&args[2])?;
        let center = if name == "incircle" {
            incenter(&a, &b, &c, span)?
        } else {
            circumcenter(&a, &b, &c, span)?
        };
        let radius = if name == "incircle" {
            let side = b.sub(&a);
            center.sub(&a).cross(&side).abs().div(
                &side
                    .norm()
                    .map_err(|()| solver_error(span, "degenerate triangle"))?,
            )
        } else {
            a.sub(&center)
                .norm()
                .map_err(|()| solver_error(span, "degenerate triangle"))?
        };
        Ok(DCircle { center, radius })
    }

    fn eval_arc_constructor(
        &mut self,
        name: &str,
        args: &[Expr],
        span: Span,
    ) -> Result<DArc, EvalError> {
        if name == "arc" {
            require_arity(name, args, 4, span)?;
            let circle = self.eval_circle(&args[0])?;
            let start = self.eval_point(&args[1])?;
            let end = self.eval_point(&args[2])?;
            let DValue::Bool(ccw) = self.eval(&args[3])? else {
                return Err(solver_error(span, "arc direction must be boolean"));
            };
            return Ok(DArc {
                circle,
                start,
                end,
                ccw,
                seed_ccw: ccw,
            });
        }
        require_arity(name, args, 4, span)?;
        let a = self.eval_point(&args[0])?;
        let center = self.eval_point(&args[1])?;
        let c = self.eval_point(&args[2])?;
        let radius = self.eval_number_as(&args[3], NumericDimension::Length)?.0;
        let first = unit(&a.sub(&center), span)?;
        let second = unit(&c.sub(&center), span)?;
        let start = center.add_vector(&first.scale(&radius));
        let end = center.add_vector(&second.scale(&radius));
        Ok(DArc {
            circle: DCircle { center, radius },
            start,
            end,
            ccw: first.cross(&second).value >= 0.0,
            seed_ccw: first.cross(&second).seed_value >= 0.0,
        })
    }

    fn eval_polygon_constructor(
        &mut self,
        name: &str,
        args: &[Expr],
        span: Span,
    ) -> Result<DObject, EvalError> {
        if name == "rounded_rect" {
            require_arity(name, args, 4, span)?;
            let center = self.eval_point(&args[0])?;
            return self.rounded_rect_at(center, &args[1..], span);
        }
        if name == "polygon" {
            let vertices = args
                .iter()
                .map(|arg| self.eval_point(arg))
                .collect::<Result<Vec<_>, _>>()?;
            return Ok(DObject::Polygon(vertices, false));
        }
        require_arity(name, args, 3, span)?;
        let center = self.eval_point(&args[0])?;
        let width = self.eval_number_as(&args[1], NumericDimension::Length)?.0;
        let height = self.eval_number_as(&args[2], NumericDimension::Length)?.0;
        let vertices = if name == "rect" {
            rect_vertices(&center, &width, &height)
        } else {
            diamond_vertices(&center, &width, &height)
        };
        Ok(DObject::Polygon(vertices, true))
    }

    fn eval_property(
        &mut self,
        base: &Expr,
        name: &str,
        args: &[Expr],
        span: Span,
    ) -> Result<DValue, EvalError> {
        require_arity(name, args, usize::from(name == "at"), span)?;
        match self.eval_object(base)? {
            DObject::Point(point) => Ok(DValue::Object(DObject::Point(point))),
            DObject::Linear(linear) => linear_property(linear, name, span)
                .map(|point| DValue::Object(DObject::Point(point))),
            DObject::Circle(circle) => self.circle_property(circle, name, args, span),
            object => self.eval_curve_or_shape_property(object, name, args, span),
        }
    }

    fn eval_curve_or_shape_property(
        &mut self,
        object: DObject,
        name: &str,
        args: &[Expr],
        span: Span,
    ) -> Result<DValue, EvalError> {
        match object {
            DObject::Arc(arc) => {
                arc_property(arc, name, span).map(|point| DValue::Object(DObject::Point(point)))
            }
            DObject::RoundedRect(rect) => rect.property(name, span, self.count),
            DObject::Polygon(vertices, boxed) => {
                polygon_property(&vertices, boxed, name, span, self.count)
            }
            DObject::Ellipse(ellipse) => self.ellipse_property(ellipse, name, args, span),
            DObject::Bspline(spline) => self.bspline_property(&spline, name, args, span),
            _ => Err(solver_error(
                span,
                format!("property `{name}` is unavailable"),
            )),
        }
    }

    fn ellipse_property(
        &mut self,
        ellipse: DEllipse,
        name: &str,
        args: &[Expr],
        span: Span,
    ) -> Result<DValue, EvalError> {
        if name == "at" {
            let angle = self.eval_number_as(&args[0], NumericDimension::Angle)?.0;
            return Ok(DValue::Object(DObject::Point(ellipse_at(&ellipse, &angle))));
        }
        if let Some(point) = ellipse_anchor(&ellipse, name) {
            return Ok(DValue::Object(DObject::Point(point?)));
        }
        ellipse_numeric_property(ellipse, name, span)
    }

    fn bspline_property(
        &mut self,
        spline: &DBspline,
        name: &str,
        args: &[Expr],
        span: Span,
    ) -> Result<DValue, EvalError> {
        let point = match name {
            "start" => spline.controls.first().cloned(),
            "end" => spline.controls.last().cloned(),
            "at" => {
                let parameter = self.eval_number_as(&args[0], NumericDimension::Scalar)?.0;
                Some(bspline_at(&spline.controls, &parameter, span)?)
            }
            _ => None,
        };
        point.map_or_else(
            || {
                Err(solver_error(
                    span,
                    format!("B-spline has no property `{name}`"),
                ))
            },
            |point| Ok(DValue::Object(DObject::Point(point))),
        )
    }

    fn circle_property(
        &mut self,
        circle: DCircle,
        name: &str,
        args: &[Expr],
        span: Span,
    ) -> Result<DValue, EvalError> {
        match name {
            "center" => Ok(DValue::Object(DObject::Point(circle.center))),
            "at" => {
                let angle = self.eval_number_as(&args[0], NumericDimension::Angle)?.0;
                Ok(DValue::Object(DObject::Point(DPoint {
                    x: circle.center.x.add(&circle.radius.mul(&angle.cos())),
                    y: circle.center.y.add(&circle.radius.mul(&angle.sin())),
                })))
            }
            _ => Err(solver_error(
                span,
                format!("circle has no property `{name}`"),
            )),
        }
    }
}

fn ellipse_at(ellipse: &DEllipse, angle: &Jet) -> DPoint {
    let cosine = ellipse.rotation.cos();
    let sine = ellipse.rotation.sin();
    let major = DVector {
        x: cosine.clone(),
        y: sine.clone(),
    };
    let minor = DVector {
        x: sine.neg(),
        y: cosine,
    };
    ellipse
        .center
        .add_vector(&major.scale(&ellipse.semimajor.mul(&angle.cos())))
        .add_vector(&minor.scale(&ellipse.semiminor.mul(&angle.sin())))
}

fn ellipse_anchor(ellipse: &DEllipse, name: &str) -> Option<Result<DPoint, EvalError>> {
    let zero = Jet::constant(0.0, ellipse.semimajor.gradient.len());
    let pi = Jet::constant(core::f64::consts::PI, ellipse.semimajor.gradient.len());
    let half_pi = Jet::constant(
        core::f64::consts::FRAC_PI_2,
        ellipse.semimajor.gradient.len(),
    );
    let angle = match name {
        "center" => return Some(Ok(ellipse.center.clone())),
        "major_end" => Some(zero),
        "major_start" => Some(pi),
        "minor_end" => Some(half_pi),
        "minor_start" => Some(half_pi.neg()),
        _ => None,
    };
    if let Some(angle) = angle {
        return Some(Ok(ellipse_at(ellipse, &angle)));
    }
    ellipse_focus(ellipse, name)
}

fn ellipse_focus(ellipse: &DEllipse, name: &str) -> Option<Result<DPoint, EvalError>> {
    let sign = match name {
        "focus1" => -1.0,
        "focus2" => 1.0,
        _ => return None,
    };
    let focal = ellipse
        .semimajor
        .sub(&ellipse.semiminor)
        .mul(&ellipse.semimajor.add(&ellipse.semiminor))
        .sqrt()
        .map_err(|()| solver_error(Span::default(), "ellipse focal distance is undefined"));
    Some(focal.map(|focal| {
        let direction = DVector {
            x: ellipse.rotation.cos(),
            y: ellipse.rotation.sin(),
        };
        ellipse
            .center
            .add_vector(&direction.scale(&focal.scale(sign)))
    }))
}

fn ellipse_numeric_property(
    ellipse: DEllipse,
    name: &str,
    span: Span,
) -> Result<DValue, EvalError> {
    match name {
        "semimajor" => Ok(DValue::Number(ellipse.semimajor, NumericDimension::Length)),
        "semiminor" => Ok(DValue::Number(ellipse.semiminor, NumericDimension::Length)),
        "rotation" => Ok(DValue::Number(ellipse.rotation, NumericDimension::Angle)),
        _ => Err(solver_error(
            span,
            format!("ellipse has no property `{name}`"),
        )),
    }
}

fn bspline_at(controls: &[DPoint], parameter: &Jet, span: Span) -> Result<DPoint, EvalError> {
    if !(0.0..=1.0).contains(&parameter.value) {
        return Err(solver_error(span, "B-spline parameter must be in [0, 1]"));
    }
    if parameter.value == 0.0 {
        return Ok(controls[0].clone());
    }
    if parameter.value >= 1.0 {
        return Ok(controls[controls.len() - 1].clone());
    }
    let knots = bspline_knots(controls.len());
    let knot_span = (3..controls.len())
        .find(|index| knots[*index] <= parameter.value && parameter.value < knots[*index + 1])
        .ok_or_else(|| solver_error(span, "B-spline span selection failed"))?;
    Ok(de_boor_points(controls, &knots, knot_span, parameter))
}

#[allow(
    clippy::cast_precision_loss,
    reason = "an allocated control-point count is exactly representable at every feasible size"
)]
fn bspline_knots(control_count: usize) -> Vec<f64> {
    let spans = control_count - 3;
    let mut knots = vec![0.0; 4];
    knots.extend((1..spans).map(|index| index as f64 / spans as f64));
    knots.extend([1.0; 4]);
    knots
}

fn de_boor_points(controls: &[DPoint], knots: &[f64], span: usize, parameter: &Jet) -> DPoint {
    let mut values = (0..=3)
        .map(|index| controls[span - 3 + index].clone())
        .collect::<Vec<_>>();
    for level in 1..=3 {
        for index in (level..=3).rev() {
            let knot_index = span - 3 + index;
            let denominator = knots[knot_index + 4 - level] - knots[knot_index];
            let alpha = parameter
                .sub(&Jet::constant(knots[knot_index], parameter.gradient.len()))
                .scale(1.0 / denominator);
            values[index] = lerp_dpoint(&values[index - 1], &values[index], &alpha);
        }
    }
    values[3].clone()
}

fn lerp_dpoint(first: &DPoint, second: &DPoint, amount: &Jet) -> DPoint {
    DPoint {
        x: first.x.add(&second.x.sub(&first.x).mul(amount)),
        y: first.y.add(&second.y.sub(&first.y).mul(amount)),
    }
}

fn normalize_angle(angle: f64) -> f64 {
    let tau = 2.0 * core::f64::consts::PI;
    let reduced = angle % tau;
    let normalized = if reduced < 0.0 {
        reduced + tau
    } else {
        reduced
    };
    if normalized >= tau || normalized == 0.0 {
        0.0
    } else {
        normalized
    }
}

fn intersection_candidates(
    lhs: &DObject,
    rhs: &DObject,
    span: Span,
    tolerance: Tolerance,
) -> Result<Vec<DPoint>, EvalError> {
    if matches!(lhs, DObject::Arc(_)) || matches!(rhs, DObject::Arc(_)) {
        return arc_intersection_candidates(lhs, rhs, span, tolerance);
    }
    match (lhs, rhs) {
        (DObject::Linear(lhs), DObject::Linear(rhs)) => {
            linear_linear_intersection(lhs, rhs, span, tolerance)
        }
        (DObject::Linear(linear), DObject::Circle(circle)) => {
            linear_circle_intersections(linear, circle, span, tolerance)
        }
        (DObject::Circle(circle), DObject::Linear(linear)) => {
            let mut points = linear_circle_intersections(linear, circle, span, tolerance)?;
            sort_by_circle_angle(&mut points, circle);
            Ok(points)
        }
        (DObject::Circle(lhs), DObject::Circle(rhs)) => {
            circle_circle_intersections(lhs, rhs, span, tolerance)
        }
        _ => Err(solver_error(span, "unsupported intersection object pair")),
    }
}

fn arc_intersection_candidates(
    lhs: &DObject,
    rhs: &DObject,
    span: Span,
    tolerance: Tolerance,
) -> Result<Vec<DPoint>, EvalError> {
    match (lhs, rhs) {
        (DObject::Linear(linear), DObject::Arc(arc)) => filter_arc(
            linear_circle_intersections(linear, &arc.circle, span, tolerance)?,
            arc,
            span,
            tolerance,
        ),
        (DObject::Arc(arc), DObject::Linear(linear)) => {
            let mut points = filter_arc(
                linear_circle_intersections(linear, &arc.circle, span, tolerance)?,
                arc,
                span,
                tolerance,
            )?;
            sort_by_circle_angle(&mut points, &arc.circle);
            Ok(points)
        }
        (DObject::Circle(circle), DObject::Arc(arc)) => filter_arc(
            circle_circle_intersections(circle, &arc.circle, span, tolerance)?,
            arc,
            span,
            tolerance,
        ),
        (DObject::Arc(arc), DObject::Circle(circle)) => {
            let mut points = filter_arc(
                circle_circle_intersections(&arc.circle, circle, span, tolerance)?,
                arc,
                span,
                tolerance,
            )?;
            sort_by_circle_angle(&mut points, &arc.circle);
            Ok(points)
        }
        (DObject::Arc(lhs), DObject::Arc(rhs)) => {
            let points = circle_circle_intersections(&lhs.circle, &rhs.circle, span, tolerance)?;
            filter_arc(
                filter_arc(points, lhs, span, tolerance)?,
                rhs,
                span,
                tolerance,
            )
        }
        _ => Err(solver_error(
            span,
            "unsupported arc intersection object pair",
        )),
    }
}

fn linear_linear_intersection(
    lhs: &DLinear,
    rhs: &DLinear,
    span: Span,
    tolerance: Tolerance,
) -> Result<Vec<DPoint>, EvalError> {
    let first_direction = lhs.direction();
    let second_direction = rhs.direction();
    validate_linear_direction(&first_direction, span, tolerance)?;
    validate_linear_direction(&second_direction, span, tolerance)?;
    let offset = rhs.start.sub(&lhs.start);
    if tolerance.directions_parallel(
        direction_primal(&first_direction),
        direction_primal(&second_direction),
    ) {
        return if tolerance.vector_on_direction(
            direction_primal(&offset),
            direction_primal(&first_direction),
        ) {
            collinear_linear_intersection(lhs, rhs, span, tolerance)
        } else {
            Err(solver_error(
                span,
                "parallel linear carriers do not intersect",
            ))
        };
    }
    let denominator = first_direction.cross(&second_direction);
    let first_parameter = offset.cross(&second_direction).div(&denominator);
    let second_parameter = offset.cross(&first_direction).div(&denominator);
    if !parameter_in_domain(first_parameter.value, lhs.domain, tolerance)
        || !parameter_in_domain(second_parameter.value, rhs.domain, tolerance)
    {
        return Err(solver_error(
            span,
            "intersection lies outside the closed linear object",
        ));
    }
    Ok(vec![
        lhs.start
            .add_vector(&first_direction.scale(&first_parameter)),
    ])
}

fn linear_circle_intersections(
    linear: &DLinear,
    circle: &DCircle,
    span: Span,
    tolerance: Tolerance,
) -> Result<Vec<DPoint>, EvalError> {
    let direction = linear.direction();
    let norm_squared = direction.dot(&direction);
    validate_linear_direction(&direction, span, tolerance)?;
    let to_center = circle.center.sub(&linear.start);
    let center_parameter = to_center.dot(&direction).div(&norm_squared);
    let closest = linear.start.add_vector(&direction.scale(&center_parameter));
    let closest_offset = closest.sub(&circle.center);
    let radial_square = circle
        .radius
        .mul(&circle.radius)
        .sub(&closest_offset.dot(&closest_offset));
    let center_distance = closest_offset
        .norm()
        .map_err(|()| solver_error(span, "linear-circle distance is undefined"))?;
    if center_distance.value > circle.radius.value
        && !tolerance.length_eq(center_distance.value, circle.radius.value)
    {
        return Err(solver_error(
            span,
            "linear object and circle do not intersect",
        ));
    }
    let zero = Jet::constant(0.0, radial_square.gradient.len());
    let parameter_offset = radial_square
        .max(&zero)
        .div(&norm_squared)
        .sqrt()
        .map_err(|()| solver_error(span, "linear-circle root is undefined"))?;
    let mut parameters = if tolerance.length_eq(center_distance.value, circle.radius.value) {
        vec![center_parameter]
    } else {
        vec![
            center_parameter.sub(&parameter_offset),
            center_parameter.add(&parameter_offset),
        ]
    };
    parameters.retain(|parameter| parameter_in_domain(parameter.value, linear.domain, tolerance));
    if parameters.is_empty() {
        return Err(solver_error(
            span,
            "circle roots lie outside the closed linear object",
        ));
    }
    parameters.sort_by(|lhs, rhs| lhs.value.total_cmp(&rhs.value));
    Ok(parameters
        .iter()
        .map(|parameter| linear.start.add_vector(&direction.scale(parameter)))
        .collect())
}

fn circle_circle_intersections(
    lhs: &DCircle,
    rhs: &DCircle,
    span: Span,
    tolerance: Tolerance,
) -> Result<Vec<DPoint>, EvalError> {
    let centers = rhs.center.sub(&lhs.center);
    let distance = centers
        .norm()
        .map_err(|()| solver_error(span, "concentric circles do not have finite branches"))?;
    if tolerance.length_zero(distance.value) {
        return Err(solver_error(
            span,
            "concentric circles do not have finite branches",
        ));
    }
    let sum = lhs.radius.value + rhs.radius.value;
    let difference = (lhs.radius.value - rhs.radius.value).abs();
    if (distance.value > sum && !tolerance.length_eq(distance.value, sum))
        || (distance.value < difference && !tolerance.length_eq(distance.value, difference))
    {
        return Err(solver_error(span, "circles do not intersect"));
    }
    // Mirror the kernel's scale-normalized chord calculation. Treating the
    // primal scale as locally constant preserves the algebraic derivative
    // while keeping all intermediate products dimensionless and representable.
    let scale_value = lhs.radius.value.max(rhs.radius.value).max(distance.value);
    let scale = Jet::constant(scale_value, distance.gradient.len());
    let lhs_radius = lhs.radius.div(&scale);
    let rhs_radius = rhs.radius.div(&scale);
    let normalized_distance = distance.div(&scale);
    let normalized_sum = lhs_radius.add(&rhs_radius);
    let normalized_difference = lhs_radius.sub(&rhs_radius).abs();
    let along = normalized_distance
        .add(
            &lhs_radius
                .sub(&rhs_radius)
                .mul(&normalized_sum)
                .div(&normalized_distance),
        )
        .scale(0.5 * scale_value);
    let base = lhs.center.add_vector(&centers.scale(&along.div(&distance)));
    let height_product = normalized_sum
        .add(&normalized_distance)
        .mul(&normalized_sum.sub(&normalized_distance))
        .mul(&normalized_distance.add(&normalized_difference))
        .mul(&normalized_distance.sub(&normalized_difference));
    let height = height_product
        .max(&Jet::constant(0.0, height_product.gradient.len()))
        .sqrt()
        .map_err(|()| solver_error(span, "circle-circle root is undefined"))?
        .div(&normalized_distance.scale(2.0))
        .scale(scale_value);
    let perpendicular = centers.perpendicular().scale(&height.div(&distance));
    let tangent =
        tolerance.length_eq(distance.value, sum) || tolerance.length_eq(distance.value, difference);
    let mut points = if tangent {
        vec![base]
    } else {
        vec![
            base.add_vector(&perpendicular),
            base.add_vector(&perpendicular.scale(&Jet::constant(-1.0, height.gradient.len()))),
        ]
    };
    sort_by_circle_angle(&mut points, lhs);
    Ok(points)
}

fn select_intersection(
    candidates: Vec<DPoint>,
    branch: Option<usize>,
    span: Span,
) -> Result<DPoint, EvalError> {
    match (candidates.len(), branch) {
        (1, None | Some(0)) => candidates
            .into_iter()
            .next()
            .ok_or_else(|| solver_error(span, "intersection disappeared")),
        (count, None) => Err(solver_error(
            span,
            format!("intersection has {count} branches; an explicit index is required"),
        )),
        (count, Some(branch)) => candidates.get(branch).cloned().ok_or_else(|| {
            solver_error(
                span,
                format!("intersection branch {branch} is out of range for {count} roots"),
            )
        }),
    }
}

fn parameter_in_domain(parameter: f64, domain: LinearDomain, tolerance: Tolerance) -> bool {
    match domain {
        LinearDomain::Line => true,
        LinearDomain::Ray => tolerance.parameter_nonnegative(parameter),
        LinearDomain::Segment => tolerance.parameter_in_closed_interval(parameter, 0.0, 1.0),
    }
}

fn collinear_linear_intersection(
    lhs: &DLinear,
    rhs: &DLinear,
    span: Span,
    tolerance: Tolerance,
) -> Result<Vec<DPoint>, EvalError> {
    let lhs_interval = mapped_parameter_interval(lhs, lhs);
    let rhs_interval = mapped_parameter_interval(rhs, lhs);
    let lower = lhs_interval.lower.max(rhs_interval.lower);
    let upper = lhs_interval.upper.min(rhs_interval.upper);
    let finite_bounds = lower.is_finite() && upper.is_finite();
    let direction = lhs.direction();
    let separation = (upper - lower).abs() * direction_primal(&direction).norm();
    if finite_bounds && tolerance.length_zero(separation) {
        let parameter = Jet::constant(lower + (upper - lower) * 0.5, direction.x.gradient.len());
        Ok(vec![lhs.start.add_vector(&direction.scale(&parameter))])
    } else if lower < upper {
        Err(solver_error(
            span,
            "collinear linear objects have overlapping intersections",
        ))
    } else {
        Err(solver_error(
            span,
            "intersection lies outside the closed linear object",
        ))
    }
}

fn mapped_parameter_interval(carrier: &DLinear, reference: &DLinear) -> ParameterInterval {
    let reference_direction = direction_primal(&reference.direction());
    let carrier_direction = direction_primal(&carrier.direction());
    let offset = crate::geom::Vec2::new(
        carrier.start.x.value - reference.start.x.value,
        carrier.start.y.value - reference.start.y.value,
    );
    let denominator = reference_direction.norm_squared();
    let origin = offset.dot(reference_direction) / denominator;
    let step = carrier_direction.dot(reference_direction) / denominator;
    match carrier.domain {
        LinearDomain::Line => ParameterInterval {
            lower: f64::NEG_INFINITY,
            upper: f64::INFINITY,
        },
        LinearDomain::Ray if step >= 0.0 => ParameterInterval {
            lower: origin,
            upper: f64::INFINITY,
        },
        LinearDomain::Ray => ParameterInterval {
            lower: f64::NEG_INFINITY,
            upper: origin,
        },
        LinearDomain::Segment => {
            let end = origin + step;
            ParameterInterval {
                lower: origin.min(end),
                upper: origin.max(end),
            }
        }
    }
}

fn filter_arc(
    points: Vec<DPoint>,
    arc: &DArc,
    span: Span,
    tolerance: Tolerance,
) -> Result<Vec<DPoint>, EvalError> {
    let points = points
        .into_iter()
        .filter(|point| point_in_arc(point, arc, tolerance))
        .collect::<Vec<_>>();
    if points.is_empty() {
        Err(solver_error(
            span,
            "supporting-circle roots lie outside the closed arc",
        ))
    } else {
        Ok(points)
    }
}

fn point_in_arc(point: &DPoint, arc: &DArc, tolerance: Tolerance) -> bool {
    let point_angle = normalized_point_angle(point, &arc.circle.center);
    let start = normalized_point_angle(&arc.start, &arc.circle.center);
    let end = normalized_point_angle(&arc.end, &arc.circle.center);
    let tau = 2.0 * core::f64::consts::PI;
    if arc.ccw {
        tolerance.angle_in_closed_sweep(
            (point_angle - start).rem_euclid(tau),
            (end - start).rem_euclid(tau),
        )
    } else {
        tolerance.angle_in_closed_sweep(
            (start - point_angle).rem_euclid(tau),
            (start - end).rem_euclid(tau),
        )
    }
}

fn validate_linear_direction(
    direction: &DVector,
    span: Span,
    tolerance: Tolerance,
) -> Result<(), EvalError> {
    let length =
        libm::sqrt(direction.x.value * direction.x.value + direction.y.value * direction.y.value);
    if tolerance.construction_length_zero(length) {
        Err(solver_error(span, "linear carrier is degenerate"))
    } else {
        Ok(())
    }
}

fn direction_primal(direction: &DVector) -> crate::geom::Vec2 {
    crate::geom::Vec2::new(direction.x.value, direction.y.value)
}

fn sort_by_circle_angle(points: &mut [DPoint], circle: &DCircle) {
    points.sort_by(|lhs, rhs| {
        normalized_point_angle(lhs, &circle.center)
            .total_cmp(&normalized_point_angle(rhs, &circle.center))
    });
}

fn normalized_point_angle(point: &DPoint, center: &DPoint) -> f64 {
    let angle = libm::atan2(
        point.y.value - center.y.value,
        point.x.value - center.x.value,
    );
    angle.rem_euclid(2.0 * core::f64::consts::PI)
}

fn point_on_circle(circle: &DCircle, angle: &Jet) -> DPoint {
    DPoint {
        x: circle.center.x.add(&circle.radius.mul(&angle.cos())),
        y: circle.center.y.add(&circle.radius.mul(&angle.sin())),
    }
}

fn closest_point(point: &DPoint, linear: &DLinear, span: Span) -> Result<DPoint, EvalError> {
    let direction = linear.direction();
    let denominator = direction.dot(&direction);
    if denominator.value == 0.0 {
        return Err(solver_error(span, "linear carrier is degenerate"));
    }
    let mut parameter = point.sub(&linear.start).dot(&direction).div(&denominator);
    parameter = match linear.domain {
        LinearDomain::Line => parameter,
        LinearDomain::Ray => parameter.max(&Jet::constant(0.0, parameter.gradient.len())),
        LinearDomain::Segment => parameter
            .max(&Jet::constant(0.0, parameter.gradient.len()))
            .min(&Jet::constant(1.0, parameter.gradient.len())),
    };
    Ok(linear.start.add_vector(&direction.scale(&parameter)))
}

fn unit(vector: &DVector, span: Span) -> Result<DVector, EvalError> {
    let norm = vector
        .norm()
        .map_err(|()| solver_error(span, "zero direction"))?;
    Ok(vector.scale(&Jet::constant(1.0, norm.gradient.len()).div(&norm)))
}

pub(crate) fn circumcenter(
    a: &DPoint,
    b: &DPoint,
    c: &DPoint,
    span: Span,
) -> Result<DPoint, EvalError> {
    let ab = b.sub(a);
    let ac = c.sub(a);
    let denominator = ab.cross(&ac).scale(2.0);
    if denominator.value == 0.0 {
        return Err(solver_error(
            span,
            "circumcenter requires a non-collinear triangle",
        ));
    }
    let base_squared = ab.dot(&ab);
    let side_squared = ac.dot(&ac);
    let local_x =
        ac.y.mul(&base_squared)
            .sub(&ab.y.mul(&side_squared))
            .div(&denominator);
    let local_y =
        ab.x.mul(&side_squared)
            .sub(&ac.x.mul(&base_squared))
            .div(&denominator);
    Ok(DPoint {
        x: a.x.add(&local_x),
        y: a.y.add(&local_y),
    })
}

fn incenter(a: &DPoint, b: &DPoint, c: &DPoint, span: Span) -> Result<DPoint, EvalError> {
    let wa = c
        .sub(b)
        .norm()
        .map_err(|()| solver_error(span, "degenerate triangle"))?;
    let wb = a
        .sub(c)
        .norm()
        .map_err(|()| solver_error(span, "degenerate triangle"))?;
    let wc = b
        .sub(a)
        .norm()
        .map_err(|()| solver_error(span, "degenerate triangle"))?;
    let total = wa.add(&wb).add(&wc);
    Ok(DPoint {
        x: wa
            .mul(&a.x)
            .add(&wb.mul(&b.x))
            .add(&wc.mul(&c.x))
            .div(&total),
        y: wa
            .mul(&a.y)
            .add(&wb.mul(&b.y))
            .add(&wc.mul(&c.y))
            .div(&total),
    })
}

fn polygon_area(vertices: &[DPoint]) -> Jet {
    let mut result = Jet::constant(0.0, vertices[0].x.gradient.len());
    let origin = &vertices[0];
    for index in 0..vertices.len() {
        let next = (index + 1) % vertices.len();
        result = result.add(
            &vertices[index]
                .sub(origin)
                .cross(&vertices[next].sub(origin)),
        );
    }
    result.scale(0.5)
}

fn polygon_perimeter(vertices: &[DPoint], span: Span) -> Result<Jet, EvalError> {
    let mut result = Jet::constant(0.0, vertices[0].x.gradient.len());
    for index in 0..vertices.len() {
        let next = (index + 1) % vertices.len();
        result = result.add(
            &vertices[next]
                .sub(&vertices[index])
                .norm()
                .map_err(|()| solver_error(span, "degenerate polygon edge"))?,
        );
    }
    Ok(result)
}

fn rect_vertices(center: &DPoint, width: &Jet, height: &Jet) -> Vec<DPoint> {
    let half_width = width.scale(0.5);
    let half_height = height.scale(0.5);
    vec![
        DPoint {
            x: center.x.sub(&half_width),
            y: center.y.sub(&half_height),
        },
        DPoint {
            x: center.x.add(&half_width),
            y: center.y.sub(&half_height),
        },
        DPoint {
            x: center.x.add(&half_width),
            y: center.y.add(&half_height),
        },
        DPoint {
            x: center.x.sub(&half_width),
            y: center.y.add(&half_height),
        },
    ]
}

fn diamond_vertices(center: &DPoint, width: &Jet, height: &Jet) -> Vec<DPoint> {
    let half_width = width.scale(0.5);
    let half_height = height.scale(0.5);
    vec![
        DPoint {
            x: center.x.clone(),
            y: center.y.sub(&half_height),
        },
        DPoint {
            x: center.x.add(&half_width),
            y: center.y.clone(),
        },
        DPoint {
            x: center.x.clone(),
            y: center.y.add(&half_height),
        },
        DPoint {
            x: center.x.sub(&half_width),
            y: center.y.clone(),
        },
    ]
}

fn linear_property(linear: DLinear, name: &str, span: Span) -> Result<DPoint, EvalError> {
    match name {
        "start" => Ok(linear.start),
        "end" => Ok(linear.through),
        "midpoint" if linear.domain == LinearDomain::Segment => Ok(linear.start.add_vector(
            &linear
                .direction()
                .scale(&Jet::constant(0.5, linear.start.x.gradient.len())),
        )),
        _ => Err(solver_error(
            span,
            format!("linear object has no property `{name}`"),
        )),
    }
}

fn arc_property(arc: DArc, name: &str, span: Span) -> Result<DPoint, EvalError> {
    match name {
        "center" => Ok(arc.circle.center),
        "start" => Ok(arc.start),
        "end" => Ok(arc.end),
        _ => Err(solver_error(span, format!("arc has no property `{name}`"))),
    }
}

fn polygon_property(
    vertices: &[DPoint],
    boxed: bool,
    name: &str,
    span: Span,
    count: usize,
) -> Result<DValue, EvalError> {
    if name == "centroid" {
        let point = polygon_centroid(vertices, span)?;
        return Ok(DValue::Object(DObject::Point(point)));
    }
    if !boxed {
        return Err(solver_error(
            span,
            format!("polygon has no property `{name}`"),
        ));
    }
    let (minimum, maximum) = polygon_bounds(vertices, count);
    match name {
        "width" => Ok(DValue::Number(
            maximum.x.sub(&minimum.x),
            NumericDimension::Length,
        )),
        "height" => Ok(DValue::Number(
            maximum.y.sub(&minimum.y),
            NumericDimension::Length,
        )),
        _ => box_anchor(minimum, maximum, name, span)
            .map(|point| DValue::Object(DObject::Point(point))),
    }
}

fn polygon_centroid(vertices: &[DPoint], span: Span) -> Result<DPoint, EvalError> {
    let mut weighted_x = Jet::constant(0.0, vertices[0].x.gradient.len());
    let mut weighted_y = weighted_x.clone();
    let mut twice_area = weighted_x.clone();
    let origin = &vertices[0];
    for index in 0..vertices.len() {
        let next = (index + 1) % vertices.len();
        let start = vertices[index].sub(origin);
        let end = vertices[next].sub(origin);
        let cross = start.cross(&end);
        twice_area = twice_area.add(&cross);
        weighted_x = weighted_x.add(&start.x.add(&end.x).mul(&cross));
        weighted_y = weighted_y.add(&start.y.add(&end.y).mul(&cross));
    }
    if twice_area.value == 0.0 {
        return Err(solver_error(span, "polygon centroid is undefined"));
    }
    let denominator = twice_area.scale(3.0);
    Ok(DPoint {
        x: origin.x.add(&weighted_x.div(&denominator)),
        y: origin.y.add(&weighted_y.div(&denominator)),
    })
}

fn polygon_bounds(vertices: &[DPoint], count: usize) -> (DPoint, DPoint) {
    let mut minimum = DPoint {
        x: Jet::constant(f64::INFINITY, count),
        y: Jet::constant(f64::INFINITY, count),
    };
    let mut maximum = DPoint {
        x: Jet::constant(f64::NEG_INFINITY, count),
        y: Jet::constant(f64::NEG_INFINITY, count),
    };
    for point in vertices {
        minimum.x = minimum.x.min(&point.x);
        minimum.y = minimum.y.min(&point.y);
        maximum.x = maximum.x.max(&point.x);
        maximum.y = maximum.y.max(&point.y);
    }
    (minimum, maximum)
}

fn box_anchor(
    minimum: DPoint,
    maximum: DPoint,
    name: &str,
    span: Span,
) -> Result<DPoint, EvalError> {
    let half = Jet::constant(0.5, minimum.x.gradient.len());
    let center = DPoint {
        x: minimum.x.add(&maximum.x).mul(&half),
        y: minimum.y.add(&maximum.y).mul(&half),
    };
    if name == "center" {
        return Ok(center);
    }
    if let Some(point) = cardinal_anchor(&minimum, &maximum, &center, name) {
        return Ok(point);
    }
    diagonal_anchor(minimum, maximum, name)
        .ok_or_else(|| solver_error(span, format!("box has no property `{name}`")))
}

fn cardinal_anchor(
    minimum: &DPoint,
    maximum: &DPoint,
    center: &DPoint,
    name: &str,
) -> Option<DPoint> {
    match name {
        "n" => DPoint {
            x: center.x.clone(),
            y: maximum.y.clone(),
        },
        "s" => DPoint {
            x: center.x.clone(),
            y: minimum.y.clone(),
        },
        "e" => DPoint {
            x: maximum.x.clone(),
            y: center.y.clone(),
        },
        "w" => DPoint {
            x: minimum.x.clone(),
            y: center.y.clone(),
        },
        _ => return None,
    }
    .into()
}

fn diagonal_anchor(minimum: DPoint, maximum: DPoint, name: &str) -> Option<DPoint> {
    Some(match name {
        "ne" => maximum,
        "nw" => DPoint {
            x: minimum.x,
            y: maximum.y,
        },
        "se" => DPoint {
            x: maximum.x,
            y: minimum.y,
        },
        "sw" => minimum,
        _ => return None,
    })
}

fn is_numeric_function(name: &str) -> bool {
    matches!(
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
            | "ratio"
            | "sqrt"
            | "abs"
            | "min"
            | "max"
    )
}

fn constructor_kind(name: &str, span: Span) -> Result<Kind, EvalError> {
    if matches!(
        name,
        "line" | "perpendicular" | "parallel" | "perp_bisector" | "angle_bisector" | "tangent_line"
    ) {
        return Ok(Kind::Line);
    }
    if name == "ray" {
        return Ok(Kind::Ray);
    }
    if name == "seg" {
        return Ok(Kind::Segment);
    }
    if matches!(
        name,
        "circle" | "circle_through" | "circle_diameter" | "circumcircle" | "incircle"
    ) {
        return Ok(Kind::Circle);
    }
    if matches!(name, "arc" | "angle_arc") {
        return Ok(Kind::Arc);
    }
    if matches!(name, "polygon" | "rect" | "rounded_rect" | "diamond") {
        return Ok(Kind::Polygon);
    }
    if matches!(
        name,
        "midpoint"
            | "lerp"
            | "offset"
            | "intersect"
            | "foot"
            | "project"
            | "rotate"
            | "reflect"
            | "translate"
            | "on_circle"
            | "extend"
            | "centroid"
            | "circumcenter"
            | "incenter"
            | "orthocenter"
            | "center"
    ) {
        return Ok(Kind::Point);
    }
    Err(solver_error(span, format!("unknown constructor `{name}`")))
}

fn contextual_dimension(
    lhs: NumericDimension,
    rhs: NumericDimension,
    span: Span,
) -> Result<NumericDimension, EvalError> {
    if lhs == rhs {
        Ok(lhs)
    } else if lhs == NumericDimension::Scalar {
        Ok(rhs)
    } else if rhs == NumericDimension::Scalar {
        Ok(lhs)
    } else {
        Err(solver_error(span, "numeric dimensions do not match"))
    }
}

fn multiply_dimension(
    lhs: NumericDimension,
    rhs: NumericDimension,
    span: Span,
) -> Result<NumericDimension, EvalError> {
    match (lhs, rhs) {
        (NumericDimension::Scalar, other) | (other, NumericDimension::Scalar) => Ok(other),
        (NumericDimension::Length, NumericDimension::Length) => Ok(NumericDimension::Area),
        _ => Err(solver_error(span, "unsupported dimensional product")),
    }
}

fn divide_dimension(
    lhs: NumericDimension,
    rhs: NumericDimension,
    span: Span,
) -> Result<NumericDimension, EvalError> {
    match (lhs, rhs) {
        (other, NumericDimension::Scalar) => Ok(other),
        (left, right) if left == right => Ok(NumericDimension::Scalar),
        (NumericDimension::Area, NumericDimension::Length) => Ok(NumericDimension::Length),
        _ => Err(solver_error(span, "unsupported dimensional quotient")),
    }
}

fn numeric_product_operand_dimensions(
    op: BinOp,
    result: NumericDimension,
) -> &'static [(NumericDimension, NumericDimension)] {
    match op {
        BinOp::Mul => multiplication_operand_dimensions(result),
        BinOp::Div => division_operand_dimensions(result),
        _ => &[],
    }
}

const fn numeric_dimensions() -> [NumericDimension; 4] {
    [
        NumericDimension::Scalar,
        NumericDimension::Length,
        NumericDimension::Area,
        NumericDimension::Angle,
    ]
}

fn preferred_numeric_dimensions(
    lhs: NumericDimension,
    rhs: NumericDimension,
) -> [NumericDimension; 2] {
    if lhs == NumericDimension::Scalar && rhs != NumericDimension::Scalar {
        [rhs, lhs]
    } else {
        [lhs, rhs]
    }
}

fn multiplication_operand_dimensions(
    result: NumericDimension,
) -> &'static [(NumericDimension, NumericDimension)] {
    use NumericDimension::{Angle, Area, Length, Scalar};

    match result {
        Scalar => &[(Scalar, Scalar)],
        Length => &[(Length, Scalar), (Scalar, Length)],
        Area => &[(Length, Length), (Area, Scalar), (Scalar, Area)],
        Angle => &[(Angle, Scalar), (Scalar, Angle)],
    }
}

fn division_operand_dimensions(
    result: NumericDimension,
) -> &'static [(NumericDimension, NumericDimension)] {
    use NumericDimension::{Angle, Area, Length, Scalar};

    match result {
        Scalar => &[
            (Scalar, Scalar),
            (Length, Length),
            (Area, Area),
            (Angle, Angle),
        ],
        Length => &[(Length, Scalar), (Area, Length)],
        Area => &[(Area, Scalar)],
        Angle => &[(Angle, Scalar)],
    }
}

fn require_numeric_dimension(
    value: Jet,
    actual: NumericDimension,
    expected: NumericDimension,
    span: Span,
) -> Result<(Jet, NumericDimension), EvalError> {
    if actual == expected {
        Ok((value, actual))
    } else {
        Err(solver_error(
            span,
            format!("expected {expected:?}, found {actual:?}"),
        ))
    }
}

fn require_arity(name: &str, args: &[Expr], expected: usize, span: Span) -> Result<(), EvalError> {
    if args.len() == expected {
        Ok(())
    } else {
        Err(solver_error(
            span,
            format!("`{name}` expects {expected} arguments"),
        ))
    }
}

fn solver_error(span: Span, message: impl Into<String>) -> EvalError {
    EvalError::new("E404", span, message)
}

pub(crate) fn dimensions_from_scene(
    objects: &indexmap::IndexMap<locus_syntax::Ident, ResolvedObject>,
) -> BTreeMap<String, NumericDimension> {
    objects
        .iter()
        .filter_map(|(id, object)| match object {
            ResolvedObject::Num(value) => Some((id.name.clone(), dimension(value.dimension))),
            ResolvedObject::Angle(_) => Some((id.name.clone(), NumericDimension::Angle)),
            _ => None,
        })
        .collect()
}

fn dimension(value: Dimension) -> NumericDimension {
    match value {
        Dimension::Scalar => NumericDimension::Scalar,
        Dimension::Length => NumericDimension::Length,
        Dimension::Area => NumericDimension::Area,
    }
}

const fn numeric_dimension(value: InferredDimension) -> NumericDimension {
    match value {
        InferredDimension::Number(Dimension::Scalar) => NumericDimension::Scalar,
        InferredDimension::Number(Dimension::Length) => NumericDimension::Length,
        InferredDimension::Number(Dimension::Area) => NumericDimension::Area,
        InferredDimension::Angle => NumericDimension::Angle,
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::float_cmp)]
mod tests {
    use super::*;

    fn jet(value: f64, seed_value: f64, derivative: f64) -> Jet {
        Jet {
            value,
            seed_value,
            gradient: vec![derivative],
        }
    }

    #[test]
    fn closest_point_keeps_the_seed_selected_projection_derivative() {
        let point = DPoint {
            x: jet(-1.0, 0.5, 1.0),
            y: Jet::constant(1.0, 1),
        };
        let segment = DLinear {
            start: DPoint {
                x: Jet::constant(0.0, 1),
                y: Jet::constant(0.0, 1),
            },
            through: DPoint {
                x: Jet::constant(1.0, 1),
                y: Jet::constant(0.0, 1),
            },
            domain: LinearDomain::Segment,
        };
        let projection = closest_point(&point, &segment, Span::default()).expect("projection");
        assert_eq!(projection.x.value, 0.0);
        assert_eq!(projection.x.seed_value, 0.5);
        assert_eq!(projection.x.gradient, vec![1.0]);
    }
}
