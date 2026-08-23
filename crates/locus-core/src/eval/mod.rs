//! Top-to-bottom derived-tier evaluation.

pub mod deps;
pub mod env;
pub mod error;
pub mod expr;

pub use deps::{
    DependencyError, DependencyGraph, RemovalImpact, StatementDependency, StatementDependencyKind,
};
pub use error::{EvalError, MeasuredValue};
pub use expr::Value;

use std::collections::BTreeSet;

use locus_syntax::json::{
    Dimension as SyntaxDimension, ExprNode, Hint, inferred_expression_dimensions,
};
use locus_syntax::{
    BinOp, Constructor, Expr, File, HeadKind, Kind, OpStmt, PropValue, Span, Statement, TextValue,
    to_interchange,
};

use crate::Tolerance;
use crate::geom::construct;
use crate::geom::intersect::intersect;
use crate::geom::measure;
use crate::geom::{Arc, BBox, Circle, Line, Point, Ray, Segment};
use crate::scene::{
    AnnotationSpec, CompassDirection, Dimension, Evaluation, LabelDirection, NumericValue,
    PointMark, ResolvedArc, ResolvedObject, ResolvedPoint, ResolvedScene, TextBlockPlacement,
};

use self::env::Environment;
use self::expr::{
    DimensionMap, Expected, ExpressionEvaluator, InferredDimension, ObjectRef, expect_arity,
    type_error,
};

/// Evaluates a parsed file in source order.
pub fn evaluate(file: &File) -> Result<Evaluation, EvalError> {
    let prepared = PreparedEvaluation::new(file)?;
    if has_solver_work(file) {
        let solved = crate::solve::solve_prepared(&prepared)?;
        return Ok(Evaluation::Solved {
            scene: solved.scene,
            summary: solved.summary,
        });
    }
    let mut evaluator = FileEvaluator::new(prepared.dimensions.clone());
    // `rel` statements belong to the solver tier and `assert` statements to the post-solve
    // checker. Dimension inference still visits both, while this direct pass resolves only ops.
    for statement in &file.statements {
        let Statement::Op(op) = statement else {
            continue;
        };
        evaluator.evaluate_op(op)?;
    }
    Ok(Evaluation::Resolved(evaluator.finish_scene()))
}

pub(crate) struct PreparedEvaluation<'a> {
    file: &'a File,
    dimensions: DimensionMap,
}

impl<'a> PreparedEvaluation<'a> {
    pub(crate) fn new(file: &'a File) -> Result<Self, EvalError> {
        DependencyGraph::extract(file).map_err(dependency_eval_error)?;
        Ok(Self {
            file,
            dimensions: infer_dimensions(file)?,
        })
    }

    pub(crate) const fn file(&self) -> &'a File {
        self.file
    }

    pub(crate) const fn dimensions(&self) -> &DimensionMap {
        &self.dimensions
    }

    pub(crate) fn evaluate_trial(
        &self,
        unknowns: Option<&[f64]>,
    ) -> Result<TrialEvaluation, EvalError> {
        let mut evaluator = FileEvaluator::new(self.dimensions.clone());
        let mut cursor = UnknownCursor::new(unknowns);
        for statement in &self.file.statements {
            let Statement::Op(op) = statement else {
                continue;
            };
            evaluator.evaluate_op_with_unknowns(op, &mut cursor)?;
        }
        cursor.finish()?;
        Ok(evaluator.finish_trial())
    }
}

fn has_solver_work(file: &File) -> bool {
    file.statements.iter().any(|statement| match statement {
        Statement::Rel(_) => true,
        Statement::Op(op) => free_span(&op.ctor).is_some() || !op.where_clauses.is_empty(),
        Statement::Assert(_) | Statement::Meta(_) | Statement::Style(_) | Statement::Class(_) => {
            false
        }
    })
}

pub(crate) struct TrialEvaluation {
    env: Environment,
    bbox: BBox,
    length_fallbacks: Vec<f64>,
}

impl TrialEvaluation {
    pub(crate) fn tolerance(&self) -> Tolerance {
        Tolerance::from_bbox_with_fallbacks(&self.bbox, &self.length_fallbacks)
    }

    pub(crate) const fn objects(&self) -> &indexmap::IndexMap<locus_syntax::Ident, ResolvedObject> {
        self.env.objects()
    }

    pub(crate) fn into_scene(self) -> ResolvedScene {
        ResolvedScene {
            objects: self.env.into_objects(),
            bbox: self.bbox,
        }
    }
}

struct UnknownCursor<'a> {
    values: Option<&'a [f64]>,
    index: usize,
}

impl<'a> UnknownCursor<'a> {
    const fn new(values: Option<&'a [f64]>) -> Self {
        Self { values, index: 0 }
    }

    fn next(&mut self, span: Span) -> Result<Option<f64>, EvalError> {
        let Some(values) = self.values else {
            return Ok(None);
        };
        let value = values.get(self.index).copied().ok_or_else(|| {
            EvalError::new("E401", span, "solver supplied too few unknown values")
        })?;
        self.index += 1;
        if value.is_finite() {
            Ok(Some(value))
        } else {
            Err(EvalError::new(
                "E404",
                span,
                "solver trial contains a non-finite unknown",
            ))
        }
    }

    fn finish(self) -> Result<(), EvalError> {
        if self.values.is_some_and(|values| self.index != values.len()) {
            Err(EvalError::new(
                "E401",
                Span::default(),
                "solver supplied too many unknown values",
            ))
        } else {
            Ok(())
        }
    }
}

struct FileEvaluator {
    env: Environment,
    bbox: Option<BBox>,
    dimensions: DimensionMap,
    length_fallbacks: Vec<f64>,
}

impl FileEvaluator {
    fn new(dimensions: DimensionMap) -> Self {
        Self {
            env: Environment::new(),
            bbox: None,
            dimensions,
            length_fallbacks: Vec::new(),
        }
    }

    fn evaluate_op(&mut self, op: &OpStmt) -> Result<(), EvalError> {
        let tolerance = tolerance_for(self.bbox, &self.length_fallbacks);
        let additions = {
            let mut expressions = ExpressionEvaluator::new(&self.env, tolerance, &self.dimensions);
            evaluated_length_fallbacks(&mut expressions, op)?
        };
        self.length_fallbacks
            .extend(additions.into_iter().filter(|value| *value > 0.0));
        let tolerance = tolerance_for(self.bbox, &self.length_fallbacks);
        let mut expressions = ExpressionEvaluator::new(&self.env, tolerance, &self.dimensions);
        let object = evaluate_op_value(&mut expressions, op)
            .map_err(|error| attach_object(error, &op.id.name))?;
        validate_kind(op.kind, &object, op.ctor.span())?;
        if let Some(bounds) = object.bbox() {
            include_bbox(&mut self.bbox, bounds);
        }
        self.env
            .insert(op.id.clone(), object)
            .map_err(|error| EvalError::new("E302", op.id.span, error.to_string()))
    }

    fn evaluate_op_with_unknowns(
        &mut self,
        op: &OpStmt,
        unknowns: &mut UnknownCursor<'_>,
    ) -> Result<(), EvalError> {
        if matches!(
            op.ctor,
            Constructor::FreePoint { .. } | Constructor::FreeNum { .. }
        ) {
            return self.evaluate_free_op(op, unknowns);
        }
        self.evaluate_op(op)
    }

    fn evaluate_free_op(
        &mut self,
        op: &OpStmt,
        unknowns: &mut UnknownCursor<'_>,
    ) -> Result<(), EvalError> {
        let tolerance = tolerance_for(self.bbox, &self.length_fallbacks);
        let mut expressions = ExpressionEvaluator::new(&self.env, tolerance, &self.dimensions);
        let (object, fallbacks) = evaluate_free_value(
            &mut expressions,
            op,
            self.dimensions.get(&op.id.name).copied(),
            unknowns,
        )?;
        self.length_fallbacks.extend(fallbacks);
        validate_kind(op.kind, &object, op.ctor.span())?;
        if let Some(bounds) = object.bbox() {
            include_bbox(&mut self.bbox, bounds);
        }
        self.env
            .insert(op.id.clone(), object)
            .map_err(|error| EvalError::new("E302", op.id.span, error.to_string()))
    }

    fn finish_scene(self) -> ResolvedScene {
        ResolvedScene {
            objects: self.env.into_objects(),
            bbox: self.bbox.unwrap_or_else(origin_bbox),
        }
    }

    fn finish_trial(self) -> TrialEvaluation {
        TrialEvaluation {
            env: self.env,
            bbox: self.bbox.unwrap_or_else(origin_bbox),
            length_fallbacks: self.length_fallbacks,
        }
    }
}

fn evaluate_free_value(
    expressions: &mut ExpressionEvaluator<'_>,
    op: &OpStmt,
    inferred: Option<InferredDimension>,
    unknowns: &mut UnknownCursor<'_>,
) -> Result<(ResolvedObject, Vec<f64>), EvalError> {
    match &op.ctor {
        Constructor::FreePoint { x, y, .. } => evaluate_free_point(expressions, op, x, y, unknowns),
        Constructor::FreeNum { hint, .. } => {
            evaluate_free_num(expressions, hint, inferred, unknowns)
        }
        Constructor::Call { .. } | Constructor::Expr { .. } => Err(EvalError::new(
            "E401",
            op.span,
            "internal free-object dispatch error",
        )),
    }
}

fn evaluate_free_point(
    expressions: &mut ExpressionEvaluator<'_>,
    op: &OpStmt,
    x: &Expr,
    y: &Expr,
    unknowns: &mut UnknownCursor<'_>,
) -> Result<(ResolvedObject, Vec<f64>), EvalError> {
    let hinted_x = expressions.eval_number(x, Dimension::Length)?;
    let hinted_y = expressions.eval_number(y, Dimension::Length)?;
    let point = Point::new(
        unknowns.next(x.span())?.unwrap_or(hinted_x),
        unknowns.next(y.span())?.unwrap_or(hinted_y),
    );
    let object = apply_point_props(&resolved_point(point), &op.props)?;
    Ok((object, vec![hinted_x.abs(), hinted_y.abs()]))
}

fn evaluate_free_num(
    expressions: &mut ExpressionEvaluator<'_>,
    hint: &Expr,
    inferred: Option<InferredDimension>,
    unknowns: &mut UnknownCursor<'_>,
) -> Result<(ResolvedObject, Vec<f64>), EvalError> {
    let dimension = inferred.unwrap_or(InferredDimension::Number(Dimension::Scalar));
    let hinted = evaluate_typed_hint(expressions, hint, dimension)?;
    let value = unknowns.next(hint.span())?.unwrap_or(hinted);
    let object = match dimension {
        InferredDimension::Number(dimension) => {
            ResolvedObject::Num(NumericValue { value, dimension })
        }
        InferredDimension::Angle => ResolvedObject::Angle(value),
    };
    let fallbacks = (dimension == InferredDimension::Number(Dimension::Length))
        .then(|| hinted.abs())
        .into_iter()
        .collect();
    Ok((object, fallbacks))
}

fn evaluate_typed_hint(
    expressions: &mut ExpressionEvaluator<'_>,
    hint: &Expr,
    dimension: InferredDimension,
) -> Result<f64, EvalError> {
    match dimension {
        InferredDimension::Number(dimension) => expressions.eval_number(hint, dimension),
        InferredDimension::Angle => expressions.eval_angle_value(hint),
    }
}

fn evaluated_length_fallbacks(
    evaluator: &mut ExpressionEvaluator<'_>,
    op: &OpStmt,
) -> Result<Vec<f64>, EvalError> {
    let mut values = Vec::new();
    match &op.ctor {
        Constructor::Expr { expr, .. }
            if evaluator.dimension_of(&op.id.name)
                == Some(InferredDimension::Number(Dimension::Length)) =>
        {
            values.push(evaluator.eval_number(expr, Dimension::Length)?);
        }
        Constructor::Call { name, args, .. } => {
            collect_constructor_length_args(evaluator, &name.name, args, &mut values)?;
        }
        Constructor::Expr { .. } | Constructor::FreePoint { .. } | Constructor::FreeNum { .. } => {}
    }
    collect_length_props(evaluator, &op.props, &mut values)?;
    Ok(values)
}

fn collect_constructor_length_args(
    evaluator: &mut ExpressionEvaluator<'_>,
    name: &str,
    args: &[Expr],
    values: &mut Vec<f64>,
) -> Result<(), EvalError> {
    for (index, argument) in args.iter().enumerate() {
        if constructor_arg_dimension(name, index)
            == Some(InferredDimension::Number(Dimension::Length))
        {
            values.push(evaluator.eval_number(argument, Dimension::Length)?);
        }
    }
    Ok(())
}

fn collect_length_props(
    evaluator: &mut ExpressionEvaluator<'_>,
    props: &locus_syntax::Props,
    values: &mut Vec<f64>,
) -> Result<(), EvalError> {
    for property in &props.items {
        if property_dimension(&property.name.name)
            != Some(InferredDimension::Number(Dimension::Length))
        {
            continue;
        }
        if let PropValue::Expr(expression) = &property.value {
            values.push(evaluator.eval_number(expression, Dimension::Length)?);
        }
    }
    Ok(())
}

fn evaluate_op_value(
    evaluator: &mut ExpressionEvaluator<'_>,
    op: &OpStmt,
) -> Result<ResolvedObject, EvalError> {
    match &op.ctor {
        Constructor::Expr { expr, .. } => evaluate_named_num(evaluator, expr, &op.id.name),
        Constructor::Call {
            name, args, span, ..
        } => eval_constructor_object(evaluator, &name.name, args, &op.props, *span),
        Constructor::FreePoint { .. } | Constructor::FreeNum { .. } => Err(EvalError::new(
            "E210",
            op.ctor.span(),
            "free declarations require the relational solver",
        )),
    }
}

fn evaluate_named_num(
    evaluator: &mut ExpressionEvaluator<'_>,
    expression: &Expr,
    name: &str,
) -> Result<ResolvedObject, EvalError> {
    let expected = evaluator
        .dimension_of(name)
        .unwrap_or(InferredDimension::Number(Dimension::Scalar));
    let value = match expected {
        InferredDimension::Number(dimension) => {
            evaluator.eval_as(expression, Expected::Number(dimension))?
        }
        InferredDimension::Angle => evaluator.eval_as(expression, Expected::Angle)?,
    };
    match value {
        Value::Scalar(number) => Ok(ResolvedObject::Num(number)),
        Value::Angle(angle) => Ok(ResolvedObject::Angle(angle)),
        _ => Err(type_error(
            expression.span(),
            "a `num` expression must be numeric",
        )),
    }
}

pub(super) fn eval_constructor_call_value(
    evaluator: &mut ExpressionEvaluator<'_>,
    name: &str,
    args: &[Expr],
    span: Span,
) -> Result<Value, EvalError> {
    let empty = locus_syntax::Props {
        items: Vec::new(),
        span: Span::empty(span.line, span.col, span.byte_end),
    };
    let object = eval_constructor_object(evaluator, name, args, &empty, span)?;
    Ok(Value::ObjectRef(ObjectRef::Temporary(Box::new(object))))
}

fn eval_constructor_object(
    evaluator: &mut ExpressionEvaluator<'_>,
    name: &str,
    args: &[Expr],
    props: &locus_syntax::Props,
    span: Span,
) -> Result<ResolvedObject, EvalError> {
    if is_point_constructor(name) {
        let object = eval_point_constructor(evaluator, name, args, span)?;
        return apply_point_props(&object, props);
    }
    if is_linear_constructor(name) {
        return eval_linear_constructor(evaluator, name, args, span);
    }
    if is_circle_constructor(name) {
        return eval_circle_constructor(evaluator, name, args, props, span);
    }
    if is_polygon_constructor(name) {
        return eval_polygon_constructor(evaluator, name, args, span);
    }
    if is_annotation_constructor(name) {
        return eval_annotation_constructor(evaluator, name, args, props, span);
    }
    Err(type_error(span, format!("unknown constructor `{name}`")))
}

fn is_point_constructor(name: &str) -> bool {
    matches!(
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
    )
}

fn is_linear_constructor(name: &str) -> bool {
    matches!(
        name,
        "line"
            | "ray"
            | "seg"
            | "perpendicular"
            | "parallel"
            | "perp_bisector"
            | "angle_bisector"
            | "tangent_line"
    )
}

fn is_circle_constructor(name: &str) -> bool {
    matches!(
        name,
        "circle"
            | "circle_through"
            | "circle_diameter"
            | "circumcircle"
            | "incircle"
            | "arc"
            | "angle_arc"
    )
}

fn is_polygon_constructor(name: &str) -> bool {
    matches!(name, "polygon" | "rect" | "diamond")
}

fn is_annotation_constructor(name: &str) -> bool {
    matches!(
        name,
        "label" | "arrow" | "tick" | "right_angle" | "text_block"
    )
}

fn eval_point_constructor(
    evaluator: &mut ExpressionEvaluator<'_>,
    name: &str,
    args: &[Expr],
    span: Span,
) -> Result<ResolvedObject, EvalError> {
    if matches!(
        name,
        "midpoint" | "lerp" | "offset" | "translate" | "extend"
    ) {
        return eval_affine_point_constructor(evaluator, name, args, span);
    }
    if matches!(
        name,
        "project" | "rotate" | "reflect" | "on_circle" | "foot"
    ) {
        return eval_transform_point_constructor(evaluator, name, args, span);
    }
    if matches!(
        name,
        "centroid" | "circumcenter" | "incenter" | "orthocenter"
    ) {
        return eval_center_point_constructor(evaluator, name, args, span);
    }
    match name {
        "intersect" => eval_intersection(evaluator, args, span).map(resolved_point),
        "center" => eval_object_center(evaluator, args, span).map(resolved_point),
        _ => Err(type_error(
            span,
            format!("unknown point constructor `{name}`"),
        )),
    }
}

fn resolved_point(geometry: Point) -> ResolvedObject {
    ResolvedObject::Point(ResolvedPoint::new(geometry))
}

fn apply_point_props(
    object: &ResolvedObject,
    props: &locus_syntax::Props,
) -> Result<ResolvedObject, EvalError> {
    let ResolvedObject::Point(mut point) = *object else {
        return Err(type_error(
            props.span,
            "internal point-property dispatch error",
        ));
    };
    if let Some(value) = prop(props, "mark") {
        point.mark = parse_point_mark(value)?;
    }
    Ok(ResolvedObject::Point(point))
}

fn parse_point_mark(value: &PropValue) -> Result<PointMark, EvalError> {
    let PropValue::Expr(Expr::Ref(token)) = value else {
        return Err(type_error(
            value.span(),
            "point mark requires `dot`, `cross`, or `none`",
        ));
    };
    match token.name.as_str() {
        "dot" => Ok(PointMark::Dot),
        "cross" => Ok(PointMark::Cross),
        "none" => Ok(PointMark::None),
        _ => Err(type_error(
            token.span,
            format!("unknown point mark `{}`", token.name),
        )),
    }
}

fn eval_affine_point_constructor(
    evaluator: &mut ExpressionEvaluator<'_>,
    name: &str,
    args: &[Expr],
    span: Span,
) -> Result<ResolvedObject, EvalError> {
    match name {
        "midpoint" => {
            expect_arity(name, args, 2, span)?;
            construct::midpoint(
                evaluator.eval_point(&args[0])?,
                evaluator.eval_point(&args[1])?,
            )
            .map(resolved_point)
            .map_err(|error| EvalError::construction(span, error))
        }
        "lerp" => {
            expect_arity(name, args, 3, span)?;
            let scalar = evaluator.eval_number(&args[2], Dimension::Scalar)?;
            construct::lerp(
                evaluator.eval_point(&args[0])?,
                evaluator.eval_point(&args[1])?,
                scalar,
            )
            .map(resolved_point)
            .map_err(|error| EvalError::construction(span, error))
        }
        "offset" => eval_offset(evaluator, args, span).map(resolved_point),
        "translate" => eval_translate(evaluator, args, span).map(resolved_point),
        "extend" => eval_extend(evaluator, args, span).map(resolved_point),
        _ => Err(type_error(
            span,
            format!("unknown affine constructor `{name}`"),
        )),
    }
}

fn eval_offset(
    evaluator: &mut ExpressionEvaluator<'_>,
    args: &[Expr],
    span: Span,
) -> Result<Point, EvalError> {
    expect_arity("offset", args, 3, span)?;
    let point = evaluator.eval_point(&args[0])?;
    let dx = evaluator.eval_number(&args[1], Dimension::Length)?;
    let dy = evaluator.eval_number(&args[2], Dimension::Length)?;
    construct::offset(point, dx, dy).map_err(|error| EvalError::construction(span, error))
}

fn eval_translate(
    evaluator: &mut ExpressionEvaluator<'_>,
    args: &[Expr],
    span: Span,
) -> Result<Point, EvalError> {
    expect_arity("translate", args, 3, span)?;
    let points = evaluator.eval_points(args)?;
    construct::translate(points[0], points[1], points[2])
        .map_err(|error| EvalError::construction(span, error))
}

fn eval_extend(
    evaluator: &mut ExpressionEvaluator<'_>,
    args: &[Expr],
    span: Span,
) -> Result<Point, EvalError> {
    expect_arity("extend", args, 3, span)?;
    let first = evaluator.eval_point(&args[0])?;
    let second = evaluator.eval_point(&args[1])?;
    let distance = evaluator.eval_number(&args[2], Dimension::Length)?;
    construct::extend(first, second, distance, evaluator.tolerance)
        .map_err(|error| EvalError::construction(span, error))
}

fn eval_transform_point_constructor(
    evaluator: &mut ExpressionEvaluator<'_>,
    name: &str,
    args: &[Expr],
    span: Span,
) -> Result<ResolvedObject, EvalError> {
    match name {
        "project" => eval_project(evaluator, args, span),
        "rotate" => eval_rotate(evaluator, args, span),
        "reflect" => eval_reflect(evaluator, args, span),
        "on_circle" => eval_on_circle(evaluator, args, span),
        "foot" => eval_foot(evaluator, args, span),
        _ => Err(type_error(
            span,
            format!("unknown point transform `{name}`"),
        )),
    }
}

fn eval_project(
    evaluator: &mut ExpressionEvaluator<'_>,
    args: &[Expr],
    span: Span,
) -> Result<ResolvedObject, EvalError> {
    expect_arity("project", args, 2, span)?;
    let point = construct::project(
        evaluator.eval_point(&args[0])?,
        evaluator.eval_circle(&args[1])?,
        evaluator.tolerance,
    )
    .map_err(|error| EvalError::construction(span, error))?;
    Ok(resolved_point(point))
}

fn eval_rotate(
    evaluator: &mut ExpressionEvaluator<'_>,
    args: &[Expr],
    span: Span,
) -> Result<ResolvedObject, EvalError> {
    expect_arity("rotate", args, 3, span)?;
    let point = evaluator.eval_point(&args[0])?;
    let origin = evaluator.eval_point(&args[1])?;
    let angle = evaluator.eval_angle_value(&args[2])?;
    construct::rotate(point, origin, angle)
        .map(resolved_point)
        .map_err(|error| EvalError::construction(span, error))
}

fn eval_reflect(
    evaluator: &mut ExpressionEvaluator<'_>,
    args: &[Expr],
    span: Span,
) -> Result<ResolvedObject, EvalError> {
    expect_arity("reflect", args, 2, span)?;
    let point = evaluator.eval_point(&args[0])?;
    let axis = evaluator.eval_resolved_object(&args[1])?;
    let result = match axis {
        ResolvedObject::Line(line) => {
            construct::reflect_over_line(point, line, evaluator.tolerance)
        }
        ResolvedObject::Point(center) => construct::reflect_over_point(point, center.geometry),
        _ => {
            return Err(type_error(
                args[1].span(),
                "reflect axis must be a line or point",
            ));
        }
    };
    result
        .map(resolved_point)
        .map_err(|error| EvalError::construction(span, error))
}

fn eval_on_circle(
    evaluator: &mut ExpressionEvaluator<'_>,
    args: &[Expr],
    span: Span,
) -> Result<ResolvedObject, EvalError> {
    expect_arity("on_circle", args, 2, span)?;
    let circle = evaluator.eval_circle(&args[0])?;
    let angle = evaluator.eval_angle_value(&args[1])?;
    construct::on_circle(circle, angle)
        .map(resolved_point)
        .map_err(|error| EvalError::construction(span, error))
}

fn eval_foot(
    evaluator: &mut ExpressionEvaluator<'_>,
    args: &[Expr],
    span: Span,
) -> Result<ResolvedObject, EvalError> {
    expect_arity("foot", args, 2, span)?;
    let point = evaluator.eval_point(&args[0])?;
    let target = evaluator.eval_linear(&args[1])?;
    let result = match target {
        ResolvedObject::Line(line) => construct::foot(point, line, evaluator.tolerance),
        ResolvedObject::Ray(ray) => construct::foot(point, ray, evaluator.tolerance),
        ResolvedObject::Segment(segment) => construct::foot(point, segment, evaluator.tolerance),
        _ => return Err(type_error(args[1].span(), "foot target must be line-like")),
    };
    result
        .map(resolved_point)
        .map_err(|error| EvalError::construction(span, error))
}

fn eval_center_point_constructor(
    evaluator: &mut ExpressionEvaluator<'_>,
    name: &str,
    args: &[Expr],
    span: Span,
) -> Result<ResolvedObject, EvalError> {
    if name == "centroid" {
        let points = evaluator.eval_points(args)?;
        return construct::centroid(&points)
            .map(resolved_point)
            .map_err(|error| EvalError::construction(span, error));
    }
    expect_arity(name, args, 3, span)?;
    let points = evaluator.eval_points(args)?;
    let result = match name {
        "circumcenter" => {
            construct::circumcenter(points[0], points[1], points[2], evaluator.tolerance)
        }
        "incenter" => construct::incenter(points[0], points[1], points[2], evaluator.tolerance),
        "orthocenter" => {
            construct::orthocenter(points[0], points[1], points[2], evaluator.tolerance)
        }
        _ => {
            return Err(type_error(
                span,
                format!("unknown center constructor `{name}`"),
            ));
        }
    };
    result
        .map(resolved_point)
        .map_err(|error| EvalError::construction(span, error))
}

fn eval_object_center(
    evaluator: &mut ExpressionEvaluator<'_>,
    args: &[Expr],
    span: Span,
) -> Result<Point, EvalError> {
    expect_arity("center", args, 1, span)?;
    match evaluator.eval_resolved_object(&args[0])? {
        ResolvedObject::Circle(circle) => Ok(circle.center),
        ResolvedObject::Arc(arc) => Ok(arc.geometry.circle.center),
        _ => Err(type_error(args[0].span(), "center expects a circle or arc")),
    }
}

fn eval_linear_constructor(
    evaluator: &mut ExpressionEvaluator<'_>,
    name: &str,
    args: &[Expr],
    span: Span,
) -> Result<ResolvedObject, EvalError> {
    if matches!(name, "line" | "ray" | "seg") {
        return eval_basic_linear(evaluator, name, args, span);
    }
    if matches!(name, "perpendicular" | "parallel") {
        return eval_relative_line(evaluator, name, args, span);
    }
    eval_special_line(evaluator, name, args, span)
}

fn eval_basic_linear(
    evaluator: &mut ExpressionEvaluator<'_>,
    name: &str,
    args: &[Expr],
    span: Span,
) -> Result<ResolvedObject, EvalError> {
    expect_arity(name, args, 2, span)?;
    let points = evaluator.eval_points(args)?;
    let object = match name {
        "line" => Line::new(points[0], points[1]).map(ResolvedObject::Line),
        "ray" => Ray::new(points[0], points[1]).map(ResolvedObject::Ray),
        "seg" => Segment::new(points[0], points[1]).map(ResolvedObject::Segment),
        _ => {
            return Err(type_error(
                span,
                format!("unknown linear constructor `{name}`"),
            ));
        }
    };
    object.map_err(|error| EvalError::construction(span, error))
}

fn eval_relative_line(
    evaluator: &mut ExpressionEvaluator<'_>,
    name: &str,
    args: &[Expr],
    span: Span,
) -> Result<ResolvedObject, EvalError> {
    expect_arity(name, args, 2, span)?;
    let point = evaluator.eval_point(&args[0])?;
    let line = evaluator.eval_line(&args[1])?;
    let result = if name == "parallel" {
        construct::parallel(point, line)
    } else {
        construct::perpendicular(point, line)
    };
    result
        .map(ResolvedObject::Line)
        .map_err(|error| EvalError::construction(span, error))
}

fn eval_special_line(
    evaluator: &mut ExpressionEvaluator<'_>,
    name: &str,
    args: &[Expr],
    span: Span,
) -> Result<ResolvedObject, EvalError> {
    let result = match name {
        "perp_bisector" => {
            expect_arity(name, args, 2, span)?;
            construct::perp_bisector(
                evaluator.eval_point(&args[0])?,
                evaluator.eval_point(&args[1])?,
                evaluator.tolerance,
            )
        }
        "angle_bisector" => {
            expect_arity(name, args, 3, span)?;
            let points = evaluator.eval_points(args)?;
            construct::angle_bisector(points[0], points[1], points[2], evaluator.tolerance)
        }
        "tangent_line" => {
            return eval_tangent_line(evaluator, args, span).map(ResolvedObject::Line);
        }
        _ => {
            return Err(type_error(
                span,
                format!("unknown line constructor `{name}`"),
            ));
        }
    };
    result
        .map(ResolvedObject::Line)
        .map_err(|error| EvalError::construction(span, error))
}

fn eval_tangent_line(
    evaluator: &mut ExpressionEvaluator<'_>,
    args: &[Expr],
    span: Span,
) -> Result<Line, EvalError> {
    expect_arity("tangent_line", args, 3, span)?;
    let point = evaluator.eval_point(&args[0])?;
    let circle = evaluator.eval_circle(&args[1])?;
    let branch = eval_branch(evaluator, &args[2])?;
    construct::tangent_line(point, circle, branch, evaluator.tolerance).map_err(|error| {
        EvalError::construction(span, error)
            .with_measurement(
                "center distance",
                measure::dist(point, circle.center),
                Some("scene units"),
            )
            .with_measurement("radius", circle.radius, Some("scene units"))
    })
}

fn eval_circle_constructor(
    evaluator: &mut ExpressionEvaluator<'_>,
    name: &str,
    args: &[Expr],
    props: &locus_syntax::Props,
    span: Span,
) -> Result<ResolvedObject, EvalError> {
    if matches!(name, "arc" | "angle_arc") {
        return eval_arc_constructor(evaluator, name, args, props, span);
    }
    let circle = match name {
        "circle" => {
            expect_arity(name, args, 2, span)?;
            Circle::new(
                evaluator.eval_point(&args[0])?,
                evaluator.eval_number(&args[1], Dimension::Length)?,
            )
        }
        "circle_through" => {
            expect_arity(name, args, 2, span)?;
            construct::circle_through(
                evaluator.eval_point(&args[0])?,
                evaluator.eval_point(&args[1])?,
                evaluator.tolerance,
            )
        }
        "circle_diameter" => {
            expect_arity(name, args, 2, span)?;
            construct::circle_diameter(
                evaluator.eval_point(&args[0])?,
                evaluator.eval_point(&args[1])?,
                evaluator.tolerance,
            )
        }
        "circumcircle" | "incircle" => {
            return eval_triangle_circle(evaluator, name, args, span).map(ResolvedObject::Circle);
        }
        _ => {
            return Err(type_error(
                span,
                format!("unknown circle constructor `{name}`"),
            ));
        }
    };
    circle
        .map(ResolvedObject::Circle)
        .map_err(|error| EvalError::construction(span, error))
}

fn eval_triangle_circle(
    evaluator: &mut ExpressionEvaluator<'_>,
    name: &str,
    args: &[Expr],
    span: Span,
) -> Result<Circle, EvalError> {
    expect_arity(name, args, 3, span)?;
    let points = evaluator.eval_points(args)?;
    let result = if name == "incircle" {
        construct::incircle(points[0], points[1], points[2], evaluator.tolerance)
    } else {
        construct::circumcircle(points[0], points[1], points[2], evaluator.tolerance)
    };
    result.map_err(|error| {
        triangle_construction_error(error, [points[0], points[1], points[2]], span)
    })
}

fn triangle_construction_error(
    error: crate::ConstructError,
    points: [Point; 3],
    span: Span,
) -> EvalError {
    let twice_signed_area = (points[1] - points[0]).cross(points[2] - points[0]);
    EvalError::construction(span, error)
        .with_measurement(
            "first side",
            measure::dist(points[0], points[1]),
            Some("scene units"),
        )
        .with_measurement(
            "second side",
            measure::dist(points[1], points[2]),
            Some("scene units"),
        )
        .with_measurement(
            "third side",
            measure::dist(points[2], points[0]),
            Some("scene units"),
        )
        .with_measurement(
            "twice signed area",
            twice_signed_area,
            Some("square scene units"),
        )
}

fn eval_arc_constructor(
    evaluator: &mut ExpressionEvaluator<'_>,
    name: &str,
    args: &[Expr],
    props: &locus_syntax::Props,
    span: Span,
) -> Result<ResolvedObject, EvalError> {
    let arc = if name == "arc" {
        expect_arity(name, args, 4, span)?;
        construct::arc(
            evaluator.eval_circle(&args[0])?,
            evaluator.eval_point(&args[1])?,
            evaluator.eval_point(&args[2])?,
            evaluator.eval_bool(&args[3])?,
            evaluator.tolerance,
        )
    } else {
        expect_arity(name, args, 4, span)?;
        construct::angle_arc(
            evaluator.eval_point(&args[0])?,
            evaluator.eval_point(&args[1])?,
            evaluator.eval_point(&args[2])?,
            evaluator.eval_number(&args[3], Dimension::Length)?,
            evaluator.tolerance,
        )
    };
    let geometry = arc.map_err(|error| EvalError::construction(span, error))?;
    let angle_mark_count = if name == "angle_arc" {
        Some(eval_angle_arc_count(evaluator, props, span)?)
    } else {
        None
    };
    Ok(ResolvedObject::Arc(ResolvedArc {
        geometry,
        angle_mark_count,
    }))
}

fn eval_angle_arc_count(
    evaluator: &mut ExpressionEvaluator<'_>,
    props: &locus_syntax::Props,
    span: Span,
) -> Result<u8, EvalError> {
    let Some(value) = prop(props, "n") else {
        return Ok(1);
    };
    let PropValue::Expr(expression) = value else {
        return Err(type_error(
            value.span(),
            "angle-arc count requires a number",
        ));
    };
    eval_count(evaluator, expression, span)
}

fn eval_polygon_constructor(
    evaluator: &mut ExpressionEvaluator<'_>,
    name: &str,
    args: &[Expr],
    span: Span,
) -> Result<ResolvedObject, EvalError> {
    if name == "polygon" {
        let points = evaluator.eval_points(args)?;
        return construct::polygon(points, evaluator.tolerance)
            .map(ResolvedObject::Polygon)
            .map_err(|error| EvalError::construction(span, error));
    }
    expect_arity(name, args, 3, span)?;
    let center = evaluator.eval_point(&args[0])?;
    let width = evaluator.eval_number(&args[1], Dimension::Length)?;
    let height = evaluator.eval_number(&args[2], Dimension::Length)?;
    if name == "rect" {
        construct::rect(center, width, height, evaluator.tolerance)
            .map(ResolvedObject::Rect)
            .map_err(|error| EvalError::construction(span, error))
    } else {
        construct::diamond(center, width, height, evaluator.tolerance)
            .map(ResolvedObject::Diamond)
            .map_err(|error| EvalError::construction(span, error))
    }
}

fn eval_annotation_constructor(
    evaluator: &mut ExpressionEvaluator<'_>,
    name: &str,
    args: &[Expr],
    props: &locus_syntax::Props,
    span: Span,
) -> Result<ResolvedObject, EvalError> {
    let annotation = match name {
        "label" => eval_label(evaluator, args, props, span)?,
        "arrow" => eval_arrow(evaluator, args, props, span)?,
        "tick" => eval_tick(evaluator, args, span)?,
        "right_angle" => eval_right_angle(evaluator, args, props, span)?,
        "text_block" => eval_text_block(evaluator, args, props, span)?,
        _ => return Err(type_error(span, format!("unknown annotation `{name}`"))),
    };
    Ok(ResolvedObject::Annotation(annotation))
}

fn eval_label(
    evaluator: &mut ExpressionEvaluator<'_>,
    args: &[Expr],
    props: &locus_syntax::Props,
    span: Span,
) -> Result<AnnotationSpec, EvalError> {
    expect_arity("label", args, 2, span)?;
    let anchor = evaluator.eval_anchor(&args[0])?;
    let text = eval_text(&args[1])?;
    let direction = label_direction(props)?;
    let gap = optional_length_prop(evaluator, props, "gap")?;
    if gap.is_some_and(|value| value < 0.0) {
        return Err(EvalError::new(
            "E204",
            span,
            "label gap must be nonnegative",
        ));
    }
    Ok(AnnotationSpec::Label {
        anchor,
        text,
        direction,
        gap,
    })
}

fn eval_arrow(
    evaluator: &mut ExpressionEvaluator<'_>,
    args: &[Expr],
    props: &locus_syntax::Props,
    span: Span,
) -> Result<AnnotationSpec, EvalError> {
    expect_arity("arrow", args, 2, span)?;
    let from = evaluator.eval_anchor(&args[0])?;
    let to = evaluator.eval_anchor(&args[1])?;
    let head = prop(props, "head").map_or(Ok(HeadKind::Forward), |value| match value {
        PropValue::Head(head) => Ok(head.kind),
        _ => Err(type_error(
            value.span(),
            "arrow head requires `->`, `<->`, or `-`",
        )),
    })?;
    let bend = optional_angle_prop(evaluator, props, "bend")?.unwrap_or(0.0);
    Ok(AnnotationSpec::Arrow {
        from,
        to,
        head,
        bend,
    })
}

fn eval_tick(
    evaluator: &mut ExpressionEvaluator<'_>,
    args: &[Expr],
    span: Span,
) -> Result<AnnotationSpec, EvalError> {
    expect_arity("tick", args, 2, span)?;
    let segment = evaluator.eval_segment(&args[0])?;
    if evaluator
        .tolerance
        .construction_length_zero(measure::length(segment))
    {
        return Err(EvalError::new(
            "E203",
            span,
            "tick requires a nonzero segment",
        ));
    }
    let count = eval_count(evaluator, &args[1], span)?;
    Ok(AnnotationSpec::Tick { segment, count })
}

fn eval_right_angle(
    evaluator: &mut ExpressionEvaluator<'_>,
    args: &[Expr],
    props: &locus_syntax::Props,
    span: Span,
) -> Result<AnnotationSpec, EvalError> {
    expect_arity("right_angle", args, 3, span)?;
    let points = evaluator.eval_points(args)?;
    validate_annotation_rays(points[0], points[1], points[2], evaluator.tolerance, span)?;
    let size = optional_length_prop(evaluator, props, "size")?;
    if size.is_some_and(|value| value <= 0.0) {
        return Err(EvalError::new(
            "E204",
            span,
            "right-angle size must be positive",
        ));
    }
    Ok(AnnotationSpec::RightAngle {
        vertex: points[0],
        first: points[1],
        second: points[2],
        size,
    })
}

fn eval_text_block(
    evaluator: &mut ExpressionEvaluator<'_>,
    args: &[Expr],
    props: &locus_syntax::Props,
    span: Span,
) -> Result<AnnotationSpec, EvalError> {
    expect_arity("text_block", args, 1, span)?;
    let text = eval_text(&args[0])?;
    let placement = text_block_placement(evaluator, props)?;
    Ok(AnnotationSpec::TextBlock { text, placement })
}

fn eval_text(expression: &Expr) -> Result<TextValue, EvalError> {
    match expression {
        Expr::String(value) | Expr::Tex(value) => Ok(value.clone()),
        _ => Err(type_error(
            expression.span(),
            "expected quoted text or a TeX literal",
        )),
    }
}

fn label_direction(props: &locus_syntax::Props) -> Result<LabelDirection, EvalError> {
    let Some(value) = prop(props, "dir") else {
        return Ok(LabelDirection::Auto);
    };
    let PropValue::Expr(Expr::Ref(token)) = value else {
        return Err(type_error(
            value.span(),
            "label direction requires a direction token",
        ));
    };
    parse_label_direction(&token.name).ok_or_else(|| {
        type_error(
            token.span,
            format!("unknown label direction `{}`", token.name),
        )
    })
}

fn parse_label_direction(name: &str) -> Option<LabelDirection> {
    match name {
        "auto" => Some(LabelDirection::Auto),
        "center" => Some(LabelDirection::Center),
        _ => parse_compass(name).map(LabelDirection::Compass),
    }
}

fn parse_compass(name: &str) -> Option<CompassDirection> {
    match name {
        "N" => Some(CompassDirection::N),
        "E" => Some(CompassDirection::E),
        "S" => Some(CompassDirection::S),
        "W" => Some(CompassDirection::W),
        other => parse_diagonal_compass(other),
    }
}

fn parse_diagonal_compass(name: &str) -> Option<CompassDirection> {
    match name {
        "NE" => Some(CompassDirection::Ne),
        "SE" => Some(CompassDirection::Se),
        "SW" => Some(CompassDirection::Sw),
        "NW" => Some(CompassDirection::Nw),
        _ => None,
    }
}

fn text_block_placement(
    evaluator: &mut ExpressionEvaluator<'_>,
    props: &locus_syntax::Props,
) -> Result<TextBlockPlacement, EvalError> {
    if prop(props, "anchor").is_some() && prop(props, "place").is_some() {
        return Err(type_error(
            props.span,
            "text_block cannot specify both `place` and `anchor`",
        ));
    }
    if let Some(PropValue::Expr(anchor)) = prop(props, "anchor") {
        return Ok(TextBlockPlacement::Anchor {
            anchor: evaluator.eval_anchor(anchor)?,
            direction: label_direction(props)?,
        });
    }
    let Some(value) = prop(props, "place") else {
        return Ok(TextBlockPlacement::Below);
    };
    let PropValue::Expr(Expr::Ref(token)) = value else {
        return Err(type_error(
            value.span(),
            "text-block place requires a placement token",
        ));
    };
    match token.name.as_str() {
        "below" => Ok(TextBlockPlacement::Below),
        "above" => Ok(TextBlockPlacement::Above),
        "left" => Ok(TextBlockPlacement::Left),
        "right" => Ok(TextBlockPlacement::Right),
        _ => Err(type_error(
            token.span,
            format!("unknown placement `{}`", token.name),
        )),
    }
}

fn prop<'a>(props: &'a locus_syntax::Props, name: &str) -> Option<&'a PropValue> {
    props
        .items
        .iter()
        .find(|item| item.name.name == name)
        .map(|item| &item.value)
}

fn optional_length_prop(
    evaluator: &mut ExpressionEvaluator<'_>,
    props: &locus_syntax::Props,
    name: &str,
) -> Result<Option<f64>, EvalError> {
    optional_numeric_prop(evaluator, props, name, Dimension::Length)
}

fn optional_numeric_prop(
    evaluator: &mut ExpressionEvaluator<'_>,
    props: &locus_syntax::Props,
    name: &str,
    dimension: Dimension,
) -> Result<Option<f64>, EvalError> {
    prop(props, name)
        .map(|value| match value {
            PropValue::Expr(expression) => evaluator.eval_number(expression, dimension),
            _ => Err(type_error(
                value.span(),
                format!("property `{name}` requires a number"),
            )),
        })
        .transpose()
}

fn optional_angle_prop(
    evaluator: &mut ExpressionEvaluator<'_>,
    props: &locus_syntax::Props,
    name: &str,
) -> Result<Option<f64>, EvalError> {
    prop(props, name)
        .map(|value| match value {
            PropValue::Expr(expression) => evaluator.eval_angle_value(expression),
            _ => Err(type_error(
                value.span(),
                format!("property `{name}` requires an angle"),
            )),
        })
        .transpose()
}

fn eval_count(
    evaluator: &mut ExpressionEvaluator<'_>,
    expression: &Expr,
    span: Span,
) -> Result<u8, EvalError> {
    let value = evaluator.eval_number(expression, Dimension::Scalar)?;
    if value.fract() != 0.0 || !(1.0..=4.0).contains(&value) {
        return Err(EvalError::new(
            "E204",
            span,
            "count must be an integer in 1..=4",
        ));
    }
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    Ok(value as u8)
}

fn eval_branch(
    evaluator: &mut ExpressionEvaluator<'_>,
    expression: &Expr,
) -> Result<usize, EvalError> {
    let value = evaluator.eval_number(expression, Dimension::Scalar)?;
    if value.fract() != 0.0 || value < 0.0 {
        return Err(type_error(
            expression.span(),
            "branch index must be a nonnegative integer",
        ));
    }
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    Ok(value as usize)
}

fn validate_annotation_rays(
    vertex: Point,
    first: Point,
    second: Point,
    tolerance: Tolerance,
    span: Span,
) -> Result<(), EvalError> {
    if tolerance.construction_length_zero(measure::dist(vertex, first))
        || tolerance.construction_length_zero(measure::dist(vertex, second))
    {
        Err(EvalError::new(
            "E203",
            span,
            "annotation rays must be nonzero",
        ))
    } else {
        Ok(())
    }
}

fn eval_intersection(
    evaluator: &mut ExpressionEvaluator<'_>,
    args: &[Expr],
    span: Span,
) -> Result<Point, EvalError> {
    if !(2..=3).contains(&args.len()) {
        return Err(type_error(
            span,
            "intersect expects two objects and an optional branch",
        ));
    }
    let lhs = evaluator.eval_resolved_object(&args[0])?;
    let rhs = evaluator.eval_resolved_object(&args[1])?;
    let branch = args
        .get(2)
        .map(|expr| eval_branch(evaluator, expr))
        .transpose()?;
    intersect_objects(&lhs, &rhs, branch, evaluator.tolerance)
        .map_err(|error| enrich_intersection_error(error, &lhs, &rhs, span))
}

fn intersect_objects(
    lhs: &ResolvedObject,
    rhs: &ResolvedObject,
    branch: Option<usize>,
    tolerance: Tolerance,
) -> Result<Point, crate::geom::intersect::IntersectError> {
    match lhs {
        ResolvedObject::Line(value) => intersect_from_line(value, rhs, branch, tolerance),
        ResolvedObject::Ray(value) => intersect_from_ray(value, rhs, branch, tolerance),
        ResolvedObject::Segment(value) => intersect_from_segment(value, rhs, branch, tolerance),
        ResolvedObject::Circle(value) => intersect_from_circle(value, rhs, branch, tolerance),
        ResolvedObject::Arc(value) => intersect_from_arc(&value.geometry, rhs, branch, tolerance),
        _ => Err(crate::geom::intersect::IntersectError::Degenerate {
            object: "non-carrier",
        }),
    }
}

macro_rules! intersection_rhs {
    ($lhs:expr, $rhs:expr, $branch:expr, $tolerance:expr) => {
        match $rhs {
            ResolvedObject::Line(value) => intersect($lhs, value, $branch, $tolerance),
            ResolvedObject::Ray(value) => intersect($lhs, value, $branch, $tolerance),
            ResolvedObject::Segment(value) => intersect($lhs, value, $branch, $tolerance),
            ResolvedObject::Circle(value) => intersect($lhs, value, $branch, $tolerance),
            ResolvedObject::Arc(value) => intersect($lhs, &value.geometry, $branch, $tolerance),
            _ => Err(crate::geom::intersect::IntersectError::Degenerate {
                object: "non-carrier",
            }),
        }
    };
}

fn intersect_from_line(
    lhs: &Line,
    rhs: &ResolvedObject,
    branch: Option<usize>,
    tolerance: Tolerance,
) -> Result<Point, crate::geom::intersect::IntersectError> {
    intersection_rhs!(lhs, rhs, branch, tolerance)
}

fn intersect_from_ray(
    lhs: &Ray,
    rhs: &ResolvedObject,
    branch: Option<usize>,
    tolerance: Tolerance,
) -> Result<Point, crate::geom::intersect::IntersectError> {
    intersection_rhs!(lhs, rhs, branch, tolerance)
}

fn intersect_from_segment(
    lhs: &Segment,
    rhs: &ResolvedObject,
    branch: Option<usize>,
    tolerance: Tolerance,
) -> Result<Point, crate::geom::intersect::IntersectError> {
    intersection_rhs!(lhs, rhs, branch, tolerance)
}

fn intersect_from_circle(
    lhs: &Circle,
    rhs: &ResolvedObject,
    branch: Option<usize>,
    tolerance: Tolerance,
) -> Result<Point, crate::geom::intersect::IntersectError> {
    intersection_rhs!(lhs, rhs, branch, tolerance)
}

fn intersect_from_arc(
    lhs: &Arc,
    rhs: &ResolvedObject,
    branch: Option<usize>,
    tolerance: Tolerance,
) -> Result<Point, crate::geom::intersect::IntersectError> {
    intersection_rhs!(lhs, rhs, branch, tolerance)
}

fn enrich_intersection_error(
    error: crate::geom::intersect::IntersectError,
    lhs: &ResolvedObject,
    rhs: &ResolvedObject,
    span: Span,
) -> EvalError {
    let diagnostic = EvalError::intersection(span, error);
    match (lhs, rhs) {
        (ResolvedObject::Circle(lhs), ResolvedObject::Circle(rhs)) => diagnostic
            .with_measurement("first radius", lhs.radius, Some("scene units"))
            .with_measurement("second radius", rhs.radius, Some("scene units"))
            .with_measurement(
                "center distance",
                measure::dist(lhs.center, rhs.center),
                Some("scene units"),
            )
            .with_measurement("radii sum", lhs.radius + rhs.radius, Some("scene units")),
        (ResolvedObject::Line(line), ResolvedObject::Circle(circle))
        | (ResolvedObject::Circle(circle), ResolvedObject::Line(line)) => {
            circle_carrier_measurements(diagnostic, *circle, line.start, line.through, None)
        }
        (ResolvedObject::Ray(ray), ResolvedObject::Circle(circle))
        | (ResolvedObject::Circle(circle), ResolvedObject::Ray(ray)) => {
            circle_carrier_measurements(diagnostic, *circle, ray.start, ray.through, Some(false))
        }
        (ResolvedObject::Segment(segment), ResolvedObject::Circle(circle))
        | (ResolvedObject::Circle(circle), ResolvedObject::Segment(segment)) => {
            circle_carrier_measurements(diagnostic, *circle, segment.start, segment.end, Some(true))
        }
        _ => diagnostic,
    }
}

fn circle_carrier_measurements(
    diagnostic: EvalError,
    circle: Circle,
    start: Point,
    through: Point,
    bounded: Option<bool>,
) -> EvalError {
    let direction = through - start;
    let denominator = direction.norm_squared();
    let closest = if denominator == 0.0 {
        start
    } else {
        let raw_parameter = (circle.center - start).dot(direction) / denominator;
        let parameter = match bounded {
            None => raw_parameter,
            Some(false) => raw_parameter.max(0.0),
            Some(true) => raw_parameter.clamp(0.0, 1.0),
        };
        start + direction * parameter
    };
    diagnostic
        .with_measurement("circle radius", circle.radius, Some("scene units"))
        .with_measurement(
            "closest carrier distance",
            measure::dist(circle.center, closest),
            Some("scene units"),
        )
}

fn validate_kind(kind: Kind, object: &ResolvedObject, span: Span) -> Result<(), EvalError> {
    let matches = match object {
        ResolvedObject::Annotation(annotation) => annotation_kind_matches(kind, annotation),
        _ => geometry_kind_matches(kind, object),
    };
    if matches {
        Ok(())
    } else {
        Err(type_error(
            span,
            format!("constructor result does not match `{}`", kind.as_str()),
        ))
    }
}

fn annotation_kind_matches(kind: Kind, annotation: &AnnotationSpec) -> bool {
    matches!(
        (kind, annotation),
        (Kind::Label, AnnotationSpec::Label { .. })
            | (Kind::Arrow, AnnotationSpec::Arrow { .. })
            | (Kind::Tick, AnnotationSpec::Tick { .. })
            | (Kind::RightAngle, AnnotationSpec::RightAngle { .. })
            | (Kind::TextBlock, AnnotationSpec::TextBlock { .. })
    )
}

fn geometry_kind_matches(kind: Kind, object: &ResolvedObject) -> bool {
    match kind {
        Kind::Point => matches!(object, ResolvedObject::Point(_)),
        Kind::Num => matches!(object, ResolvedObject::Num(_) | ResolvedObject::Angle(_)),
        Kind::Line => matches!(object, ResolvedObject::Line(_)),
        Kind::Ray => matches!(object, ResolvedObject::Ray(_)),
        Kind::Segment => matches!(object, ResolvedObject::Segment(_)),
        _ => secondary_geometry_kind_matches(kind, object),
    }
}

fn secondary_geometry_kind_matches(kind: Kind, object: &ResolvedObject) -> bool {
    match kind {
        Kind::Circle => matches!(object, ResolvedObject::Circle(_)),
        Kind::Arc => matches!(object, ResolvedObject::Arc(_)),
        Kind::Polygon => matches!(object, ResolvedObject::Polygon(_)),
        Kind::Rect => matches!(object, ResolvedObject::Rect(_)),
        Kind::Diamond => matches!(object, ResolvedObject::Diamond(_)),
        _ => false,
    }
}

pub(crate) fn infer_dimensions(file: &File) -> Result<DimensionMap, EvalError> {
    let numeric_ids = numeric_ids(file);
    let mut dimensions = interchange_dimensions(file);
    for statement in &file.statements {
        infer_statement(statement, &numeric_ids, &mut dimensions)?;
    }
    Ok(dimensions)
}

fn interchange_dimensions(file: &File) -> DimensionMap {
    let mut dimensions = DimensionMap::new();
    for op in to_interchange(file)
        .ops
        .into_iter()
        .filter(|op| op.kind == Kind::Num)
    {
        let dimension = op
            .def
            .as_ref()
            .and_then(|definition| expr_node_dimension(&definition.tree))
            .or_else(|| op.hint.as_ref().and_then(hint_dimension))
            .unwrap_or(SyntaxDimension::Scalar);
        dimensions.insert(op.id, inferred_syntax_dimension(dimension));
    }
    for (range, dimension) in inferred_expression_dimensions(file) {
        dimensions.insert_expression_range(range, inferred_syntax_dimension(dimension));
    }
    dimensions
}

fn hint_dimension(hint: &Hint) -> Option<SyntaxDimension> {
    match hint {
        Hint::Scalar(expression) => expr_node_dimension(expression),
        Hint::Point(_) => None,
    }
}

fn expr_node_dimension(expression: &ExprNode) -> Option<SyntaxDimension> {
    match expression {
        ExprNode::Number(number) => Some(number.dimension),
        ExprNode::Ref { dimension, .. }
        | ExprNode::Call { dimension, .. }
        | ExprNode::Property { dimension, .. }
        | ExprNode::Group { dimension, .. }
        | ExprNode::Unary { dimension, .. }
        | ExprNode::Binary { dimension, .. } => *dimension,
        ExprNode::Boolean { .. } | ExprNode::Text { .. } => None,
    }
}

const fn inferred_syntax_dimension(dimension: SyntaxDimension) -> InferredDimension {
    match dimension {
        SyntaxDimension::Scalar => InferredDimension::Number(Dimension::Scalar),
        SyntaxDimension::Length => InferredDimension::Number(Dimension::Length),
        SyntaxDimension::Area => InferredDimension::Number(Dimension::Area),
        SyntaxDimension::Angle => InferredDimension::Angle,
    }
}

fn numeric_ids(file: &File) -> BTreeSet<String> {
    file.statements
        .iter()
        .filter_map(|statement| match statement {
            Statement::Op(op) if op.kind == Kind::Num => Some(op.id.name.clone()),
            _ => None,
        })
        .collect()
}

fn infer_statement(
    statement: &Statement,
    numeric_ids: &BTreeSet<String>,
    dimensions: &mut DimensionMap,
) -> Result<(), EvalError> {
    match statement {
        Statement::Op(op) => infer_op(op, numeric_ids, dimensions),
        Statement::Rel(rel) => infer_relation_expr(&rel.expr, numeric_ids, dimensions),
        Statement::Assert(assertion) => {
            infer_expr(&assertion.expr, None, numeric_ids, dimensions).map(|_| ())
        }
        Statement::Meta(_) | Statement::Style(_) | Statement::Class(_) => Ok(()),
    }
}

fn infer_op(
    op: &OpStmt,
    numeric_ids: &BTreeSet<String>,
    dimensions: &mut DimensionMap,
) -> Result<(), EvalError> {
    match &op.ctor {
        Constructor::Expr { expr, .. } => {
            let inferred = infer_expr(
                expr,
                dimensions.get(&op.id.name).copied(),
                numeric_ids,
                dimensions,
            )?;
            if let Some(dimension) = inferred {
                constrain_named(&op.id.name, dimension, expr.span(), dimensions)?;
            }
        }
        Constructor::Call { name, args, .. } => {
            infer_constructor_args(&name.name, args, numeric_ids, dimensions)?;
        }
        Constructor::FreePoint { x, y, .. } => {
            infer_expr(
                x,
                Some(InferredDimension::Number(Dimension::Length)),
                numeric_ids,
                dimensions,
            )?;
            infer_expr(
                y,
                Some(InferredDimension::Number(Dimension::Length)),
                numeric_ids,
                dimensions,
            )?;
        }
        Constructor::FreeNum { hint, .. } => {
            let expected = dimensions.get(&op.id.name).copied();
            infer_expr(hint, expected, numeric_ids, dimensions)?;
        }
    }
    infer_props(&op.props, numeric_ids, dimensions)?;
    for clause in &op.where_clauses {
        infer_relation_expr(clause, numeric_ids, dimensions)?;
    }
    Ok(())
}

fn infer_relation_expr(
    expression: &Expr,
    numeric_ids: &BTreeSet<String>,
    dimensions: &mut DimensionMap,
) -> Result<(), EvalError> {
    match expression {
        Expr::Binary {
            op: BinOp::And,
            lhs,
            rhs,
            ..
        } => {
            infer_relation_expr(lhs, numeric_ids, dimensions)?;
            infer_relation_expr(rhs, numeric_ids, dimensions)
        }
        Expr::Call { name, args, .. } if name.name == "pin" && args.len() == 3 => {
            infer_expr(&args[0], None, numeric_ids, dimensions)?;
            let length = Some(InferredDimension::Number(Dimension::Length));
            infer_expr(&args[1], length, numeric_ids, dimensions)?;
            infer_expr(&args[2], length, numeric_ids, dimensions).map(|_| ())
        }
        _ => infer_expr(expression, None, numeric_ids, dimensions).map(|_| ()),
    }
}

fn infer_constructor_args(
    name: &str,
    args: &[Expr],
    numeric_ids: &BTreeSet<String>,
    dimensions: &mut DimensionMap,
) -> Result<(), EvalError> {
    for (index, argument) in args.iter().enumerate() {
        let expected = constructor_arg_dimension(name, index);
        infer_expr(argument, expected, numeric_ids, dimensions)?;
    }
    Ok(())
}

fn constructor_arg_dimension(name: &str, index: usize) -> Option<InferredDimension> {
    if matches!(name, "offset") && matches!(index, 1 | 2)
        || matches!(name, "extend") && index == 2
        || matches!(name, "circle") && index == 1
        || matches!(name, "angle_arc") && index == 3
        || matches!(name, "rect" | "diamond") && matches!(index, 1 | 2)
    {
        return Some(InferredDimension::Number(Dimension::Length));
    }
    if matches!(name, "lerp" | "tangent_line" | "intersect") && index == 2
        || name == "tick" && index == 1
    {
        return Some(InferredDimension::Number(Dimension::Scalar));
    }
    if matches!(name, "rotate") && index == 2 || matches!(name, "on_circle") && index == 1 {
        return Some(InferredDimension::Angle);
    }
    None
}

fn infer_props(
    props: &locus_syntax::Props,
    numeric_ids: &BTreeSet<String>,
    dimensions: &mut DimensionMap,
) -> Result<(), EvalError> {
    for property in &props.items {
        let PropValue::Expr(expression) = &property.value else {
            continue;
        };
        let expected = property_dimension(&property.name.name);
        if expected.is_some() || property.name.name == "anchor" {
            infer_expr(expression, expected, numeric_ids, dimensions)?;
        }
    }
    Ok(())
}

fn property_dimension(name: &str) -> Option<InferredDimension> {
    match name {
        "gap" | "size" => Some(InferredDimension::Number(Dimension::Length)),
        "bend" => Some(InferredDimension::Angle),
        "n" => Some(InferredDimension::Number(Dimension::Scalar)),
        _ => None,
    }
}

fn infer_expr(
    expression: &Expr,
    expected: Option<InferredDimension>,
    numeric_ids: &BTreeSet<String>,
    dimensions: &mut DimensionMap,
) -> Result<Option<InferredDimension>, EvalError> {
    if let Some(result) = infer_leaf_expr(expression, expected, numeric_ids, dimensions) {
        return result;
    }
    infer_compound_expr(expression, expected, numeric_ids, dimensions)
}

fn infer_leaf_expr(
    expression: &Expr,
    expected: Option<InferredDimension>,
    numeric_ids: &BTreeSet<String>,
    dimensions: &mut DimensionMap,
) -> Option<Result<Option<InferredDimension>, EvalError>> {
    match expression {
        Expr::Number(number) => Some(infer_number_literal(number, expected)),
        Expr::Ref(id) if numeric_ids.contains(&id.name) => {
            Some(infer_numeric_ref(id, expected, dimensions))
        }
        Expr::Boolean { .. } | Expr::String(_) | Expr::Tex(_) | Expr::Ref(_) => Some(Ok(expected)),
        _ => None,
    }
}

fn infer_numeric_ref(
    id: &locus_syntax::Ident,
    expected: Option<InferredDimension>,
    dimensions: &mut DimensionMap,
) -> Result<Option<InferredDimension>, EvalError> {
    if let Some(dimension) = expected {
        constrain_named(&id.name, dimension, id.span, dimensions)?;
    }
    Ok(dimensions.get(&id.name).copied().or(expected))
}

fn infer_compound_expr(
    expression: &Expr,
    expected: Option<InferredDimension>,
    numeric_ids: &BTreeSet<String>,
    dimensions: &mut DimensionMap,
) -> Result<Option<InferredDimension>, EvalError> {
    match expression {
        Expr::Call { name, args, .. } => {
            infer_call(&name.name, args, expected, numeric_ids, dimensions)
        }
        Expr::Property {
            base, name, args, ..
        } => infer_property(base, &name.name, args, expected, numeric_ids, dimensions),
        Expr::Group { expr, .. } | Expr::Unary { expr, .. } => {
            infer_expr(expr, expected, numeric_ids, dimensions)
        }
        Expr::Binary { op, lhs, rhs, .. } => {
            infer_binary(*op, lhs, rhs, expected, numeric_ids, dimensions)
        }
        Expr::Number(_) | Expr::Boolean { .. } | Expr::String(_) | Expr::Tex(_) | Expr::Ref(_) => {
            Ok(expected)
        }
    }
}

fn infer_property(
    base: &Expr,
    name: &str,
    args: &[Expr],
    expected: Option<InferredDimension>,
    numeric_ids: &BTreeSet<String>,
    dimensions: &mut DimensionMap,
) -> Result<Option<InferredDimension>, EvalError> {
    infer_expr(base, None, numeric_ids, dimensions)?;
    for arg in args {
        infer_expr(arg, None, numeric_ids, dimensions)?;
    }
    if matches!(name, "width" | "height") {
        Ok(Some(InferredDimension::Number(Dimension::Length)))
    } else {
        Ok(expected)
    }
}

fn infer_number_literal(
    number: &locus_syntax::NumberExpr,
    expected: Option<InferredDimension>,
) -> Result<Option<InferredDimension>, EvalError> {
    if number.unit.is_some() {
        if expected.is_some_and(|value| value != InferredDimension::Angle) {
            return Err(type_error(
                number.span,
                "angle literal used in a non-angle context",
            ));
        }
        return Ok(Some(InferredDimension::Angle));
    }
    if expected == Some(InferredDimension::Angle) {
        return Err(type_error(
            number.span,
            "angle literals require an explicit `deg` or `rad` unit",
        ));
    }
    Ok(expected)
}

fn infer_call(
    name: &str,
    args: &[Expr],
    expected: Option<InferredDimension>,
    numeric_ids: &BTreeSet<String>,
    dimensions: &mut DimensionMap,
) -> Result<Option<InferredDimension>, EvalError> {
    if is_constructor_name(name) {
        infer_constructor_args(name, args, numeric_ids, dimensions)?;
        return Ok(expected);
    }
    if matches!(name, "sqrt" | "abs" | "min" | "max") {
        return infer_dimension_preserving_call(name, args, expected, numeric_ids, dimensions);
    }
    let result = numeric_function_dimension(name).or(expected);
    infer_numeric_function_args(name, args, result, numeric_ids, dimensions)?;
    Ok(result)
}

fn is_constructor_name(name: &str) -> bool {
    is_point_constructor(name)
        || is_linear_constructor(name)
        || is_circle_constructor(name)
        || is_polygon_constructor(name)
        || is_annotation_constructor(name)
}

fn infer_dimension_preserving_call(
    name: &str,
    args: &[Expr],
    expected: Option<InferredDimension>,
    numeric_ids: &BTreeSet<String>,
    dimensions: &mut DimensionMap,
) -> Result<Option<InferredDimension>, EvalError> {
    if name == "sqrt" {
        let span = args.first().map_or(Span::default(), Expr::span);
        let operand_expected = expected.and_then(sqrt_operand_dimension);
        let first = args
            .first()
            .map(|argument| infer_expr(argument, operand_expected, numeric_ids, dimensions))
            .transpose()?
            .flatten();
        return infer_sqrt_result(first, expected, span);
    }
    let first = args
        .first()
        .map(|argument| infer_expr(argument, expected, numeric_ids, dimensions))
        .transpose()?
        .flatten();
    let mut result = first.or(expected);
    for argument in args.iter().skip(1) {
        let inferred = infer_expr(argument, result, numeric_ids, dimensions)?;
        if result.is_none() {
            result = inferred;
        }
    }
    if first.is_none() {
        if let (Some(argument), Some(result)) = (args.first(), result) {
            infer_expr(argument, Some(result), numeric_ids, dimensions)?;
        }
    }
    Ok(result)
}

fn sqrt_operand_dimension(result: InferredDimension) -> Option<InferredDimension> {
    match result {
        InferredDimension::Number(Dimension::Scalar) => {
            Some(InferredDimension::Number(Dimension::Scalar))
        }
        InferredDimension::Number(Dimension::Length) => {
            Some(InferredDimension::Number(Dimension::Area))
        }
        InferredDimension::Number(Dimension::Area) | InferredDimension::Angle => None,
    }
}

fn infer_sqrt_result(
    operand: Option<InferredDimension>,
    expected: Option<InferredDimension>,
    span: Span,
) -> Result<Option<InferredDimension>, EvalError> {
    match operand {
        Some(InferredDimension::Number(Dimension::Scalar)) => {
            Ok(Some(InferredDimension::Number(Dimension::Scalar)))
        }
        Some(InferredDimension::Number(Dimension::Area)) => {
            Ok(Some(InferredDimension::Number(Dimension::Length)))
        }
        Some(InferredDimension::Number(Dimension::Length) | InferredDimension::Angle) => {
            Err(type_error(span, "sqrt accepts only scalar or area values"))
        }
        None => Ok(expected),
    }
}

fn numeric_function_dimension(name: &str) -> Option<InferredDimension> {
    match name {
        "dist" | "hdist" | "vdist" | "radius" | "length" | "perimeter" => {
            Some(InferredDimension::Number(Dimension::Length))
        }
        "area" => Some(InferredDimension::Number(Dimension::Area)),
        "angle" | "dir_angle" => Some(InferredDimension::Angle),
        "ratio" => Some(InferredDimension::Number(Dimension::Scalar)),
        _ => None,
    }
}

fn infer_numeric_function_args(
    name: &str,
    args: &[Expr],
    result: Option<InferredDimension>,
    numeric_ids: &BTreeSet<String>,
    dimensions: &mut DimensionMap,
) -> Result<(), EvalError> {
    let argument_expected = if matches!(name, "min" | "max" | "abs") {
        result
    } else {
        None
    };
    for argument in args {
        infer_expr(argument, argument_expected, numeric_ids, dimensions)?;
    }
    if name == "ratio" && args.len() == 2 {
        unify_exprs(&args[0], &args[1], numeric_ids, dimensions)?;
    }
    Ok(())
}

fn infer_binary(
    op: BinOp,
    lhs: &Expr,
    rhs: &Expr,
    expected: Option<InferredDimension>,
    numeric_ids: &BTreeSet<String>,
    dimensions: &mut DimensionMap,
) -> Result<Option<InferredDimension>, EvalError> {
    if matches!(op, BinOp::Add | BinOp::Sub) {
        let lhs_dim = infer_expr(lhs, expected, numeric_ids, dimensions)?;
        let rhs_dim = infer_expr(rhs, lhs_dim.or(expected), numeric_ids, dimensions)?;
        let result = lhs_dim.or(rhs_dim).or(expected);
        if lhs_dim.is_none() {
            infer_expr(lhs, result, numeric_ids, dimensions)?;
        }
        return Ok(result);
    }
    if matches!(
        op,
        BinOp::Eq | BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge
    ) {
        unify_exprs(lhs, rhs, numeric_ids, dimensions)?;
        return Ok(None);
    }
    if matches!(op, BinOp::And | BinOp::Or) {
        infer_expr(lhs, None, numeric_ids, dimensions)?;
        infer_expr(rhs, None, numeric_ids, dimensions)?;
        return Ok(None);
    }
    infer_product(op, lhs, rhs, expected, numeric_ids, dimensions)
}

fn infer_product(
    op: BinOp,
    lhs: &Expr,
    rhs: &Expr,
    expected: Option<InferredDimension>,
    numeric_ids: &BTreeSet<String>,
    dimensions: &mut DimensionMap,
) -> Result<Option<InferredDimension>, EvalError> {
    let lhs_dim = infer_expr(lhs, None, numeric_ids, dimensions)?;
    let rhs_dim = infer_expr(rhs, None, numeric_ids, dimensions)?;
    if expected == Some(InferredDimension::Angle) {
        infer_angle_product(op, lhs, rhs, lhs_dim, rhs_dim, numeric_ids, dimensions)?;
        return Ok(expected);
    }
    if op == BinOp::Div && expected == Some(InferredDimension::Number(Dimension::Scalar)) {
        unify_exprs(lhs, rhs, numeric_ids, dimensions)?;
        return Ok(expected);
    }
    let computed = infer_product_result(op, lhs_dim, rhs_dim, None, lhs.span().join(rhs.span()))?;
    if let Some(computed) = computed {
        if expected.is_some_and(|expected| expected != computed) {
            return Err(type_error(
                lhs.span().join(rhs.span()),
                format!("product has dimension {computed:?}, expected {expected:?}"),
            ));
        }
        return Ok(Some(computed));
    }
    if let Some(expected) = expected {
        let assignment = select_contextual_product_dimensions(
            op,
            expected,
            lhs_dim,
            rhs_dim,
            lhs.span().join(rhs.span()),
        )?;
        if let Some((lhs_expected, rhs_expected)) = assignment {
            infer_expr(lhs, Some(lhs_expected), numeric_ids, dimensions)?;
            infer_expr(rhs, Some(rhs_expected), numeric_ids, dimensions)?;
        }
    }
    Ok(expected)
}

fn select_contextual_product_dimensions(
    op: BinOp,
    expected: InferredDimension,
    lhs: Option<InferredDimension>,
    rhs: Option<InferredDimension>,
    span: Span,
) -> Result<Option<(InferredDimension, InferredDimension)>, EvalError> {
    let InferredDimension::Number(result) = expected else {
        return Err(type_error(span, "unsupported angle product context"));
    };
    let mut compatible = product_dimension_candidates(op, result)
        .iter()
        .copied()
        .map(|(lhs, rhs)| {
            (
                InferredDimension::Number(lhs),
                InferredDimension::Number(rhs),
            )
        })
        .filter(|(candidate_lhs, candidate_rhs)| {
            lhs.is_none_or(|lhs| lhs == *candidate_lhs)
                && rhs.is_none_or(|rhs| rhs == *candidate_rhs)
        });
    let first = compatible
        .next()
        .ok_or_else(|| type_error(span, "unsupported dimension product or quotient"))?;
    if compatible.next().is_some() {
        return Ok(None);
    }
    Ok(Some(first))
}

fn product_dimension_candidates(op: BinOp, result: Dimension) -> &'static [(Dimension, Dimension)] {
    use Dimension::{Area, Length, Scalar};

    match (op, result) {
        (BinOp::Mul, Scalar) => &[(Scalar, Scalar)],
        (BinOp::Div, Scalar) => &[(Scalar, Scalar), (Length, Length), (Area, Area)],
        (BinOp::Mul, Length) => &[(Length, Scalar), (Scalar, Length)],
        (BinOp::Div, Length) => &[(Length, Scalar), (Area, Length)],
        (BinOp::Mul, Area) => &[(Length, Length), (Area, Scalar), (Scalar, Area)],
        (BinOp::Div, Area) => &[(Area, Scalar)],
        _ => &[],
    }
}

fn infer_product_result(
    op: BinOp,
    lhs: Option<InferredDimension>,
    rhs: Option<InferredDimension>,
    expected: Option<InferredDimension>,
    span: Span,
) -> Result<Option<InferredDimension>, EvalError> {
    let (Some(lhs), Some(rhs)) = (lhs, rhs) else {
        return Ok(expected);
    };
    match (op, lhs, rhs) {
        (
            BinOp::Mul,
            InferredDimension::Number(Dimension::Length),
            InferredDimension::Number(Dimension::Length),
        ) => Ok(Some(InferredDimension::Number(Dimension::Area))),
        (BinOp::Div, same_lhs, same_rhs) if same_lhs == same_rhs => {
            Ok(Some(InferredDimension::Number(Dimension::Scalar)))
        }
        (
            BinOp::Div,
            InferredDimension::Number(Dimension::Area),
            InferredDimension::Number(Dimension::Length),
        ) => Ok(Some(InferredDimension::Number(Dimension::Length))),
        (_, other, InferredDimension::Number(Dimension::Scalar))
        | (BinOp::Mul, InferredDimension::Number(Dimension::Scalar), other) => Ok(Some(other)),
        _ => Err(type_error(
            span,
            "unsupported dimension product or quotient",
        )),
    }
}

fn infer_angle_product(
    op: BinOp,
    lhs: &Expr,
    rhs: &Expr,
    lhs_dim: Option<InferredDimension>,
    rhs_dim: Option<InferredDimension>,
    numeric_ids: &BTreeSet<String>,
    dimensions: &mut DimensionMap,
) -> Result<(), EvalError> {
    let candidates = [(lhs, rhs, lhs_dim, rhs_dim), (rhs, lhs, rhs_dim, lhs_dim)];
    let candidate_count = if op == BinOp::Div { 1 } else { 2 };
    let mut first_error = None;
    for &(angle, scalar, angle_dim, scalar_dim) in candidates.iter().take(candidate_count) {
        if !angle_dim.is_none_or(|dimension| dimension == InferredDimension::Angle)
            || !scalar_dim
                .is_none_or(|dimension| dimension == InferredDimension::Number(Dimension::Scalar))
        {
            continue;
        }
        let mut candidate_dimensions = dimensions.clone();
        let result = infer_expr(
            angle,
            Some(InferredDimension::Angle),
            numeric_ids,
            &mut candidate_dimensions,
        )
        .and_then(|_| {
            infer_expr(
                scalar,
                Some(InferredDimension::Number(Dimension::Scalar)),
                numeric_ids,
                &mut candidate_dimensions,
            )
        });
        match result {
            Ok(_) => {
                *dimensions = candidate_dimensions;
                return Ok(());
            }
            Err(error) => first_error.get_or_insert(error),
        };
    }
    Err(first_error.unwrap_or_else(|| {
        type_error(
            lhs.span().join(rhs.span()),
            "angle product requires one angle and one scalar",
        )
    }))
}

fn unify_exprs(
    lhs: &Expr,
    rhs: &Expr,
    numeric_ids: &BTreeSet<String>,
    dimensions: &mut DimensionMap,
) -> Result<(), EvalError> {
    let lhs_dim = infer_expr(lhs, None, numeric_ids, dimensions)?;
    let rhs_dim = infer_expr(rhs, lhs_dim, numeric_ids, dimensions)?;
    if lhs_dim.is_none() {
        infer_expr(lhs, rhs_dim, numeric_ids, dimensions)?;
    }
    Ok(())
}

fn constrain_named(
    name: &str,
    dimension: InferredDimension,
    span: Span,
    dimensions: &mut DimensionMap,
) -> Result<(), EvalError> {
    if let Some(existing) = dimensions.get(name) {
        if *existing != dimension {
            return Err(type_error(
                span,
                format!(
                    "numeric `{name}` has conflicting dimensions {existing:?} and {dimension:?}"
                ),
            ));
        }
    } else {
        dimensions.insert(name.to_owned(), dimension);
    }
    Ok(())
}

fn dependency_eval_error(error: DependencyError) -> EvalError {
    EvalError::new(error.code, error.span, error.message)
}

fn free_span(constructor: &Constructor) -> Option<Span> {
    match constructor {
        Constructor::FreePoint { span, .. } | Constructor::FreeNum { span, .. } => Some(*span),
        Constructor::Call { .. } | Constructor::Expr { .. } => None,
    }
}

fn tolerance_for(bbox: Option<BBox>, positive_lengths: &[f64]) -> Tolerance {
    Tolerance::from_bbox_with_fallbacks(&bbox.unwrap_or_else(origin_bbox), positive_lengths)
}

fn origin_bbox() -> BBox {
    BBox::from_points([Point::new(0.0, 0.0)]).unwrap_or(BBox {
        min: Point::new(0.0, 0.0),
        max: Point::new(0.0, 0.0),
    })
}

fn include_bbox(current: &mut Option<BBox>, bounds: BBox) {
    if let Some(current) = current {
        current.include_bbox(bounds);
    } else {
        *current = Some(bounds);
    }
}

fn attach_object(mut error: EvalError, id: &str) -> EvalError {
    if error.object_id.is_none() {
        error.object_id = Some(id.to_owned());
    }
    error
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use locus_syntax::{Ident, parse};

    use super::*;
    use crate::scene::NumericValue;

    fn op_from(source: &str) -> OpStmt {
        let parsed = parse(source);
        parsed
            .file
            .statements
            .into_iter()
            .find_map(|statement| match statement {
                Statement::Op(op) => Some(op),
                _ => None,
            })
            .expect("source contains an op")
    }

    fn test_evaluator(env: &Environment) -> ExpressionEvaluator<'_> {
        let bbox = BBox::from_points([Point::new(-10.0, -10.0), Point::new(10.0, 10.0)])
            .expect("finite bbox");
        let dimensions = Box::leak(Box::new(DimensionMap::new()));
        ExpressionEvaluator::new(env, Tolerance::from_bbox(&bbox), dimensions)
    }

    fn insert(env: &mut Environment, name: &str, object: ResolvedObject) {
        env.insert(
            Ident {
                name: name.to_owned(),
                span: Span::default(),
            },
            object,
        )
        .expect("unique test id");
    }

    #[test]
    fn evaluates_hand_computed_rotation_and_typed_angle_product() {
        let mut env = Environment::new();
        insert(&mut env, "P", resolved_point(Point::new(1.0, 0.0)));
        insert(&mut env, "O", resolved_point(Point::new(0.0, 0.0)));
        let op = op_from("locus 2\npoint R = rotate(P, O, 2 * 30deg)\n");
        let mut evaluator = test_evaluator(&env);
        let object = evaluate_op_value(&mut evaluator, &op).expect("typed rotation");
        let ResolvedObject::Point(point) = object else {
            unreachable!("rotate returns a point");
        };
        assert!((point.geometry.x - 0.5).abs() < 1.0e-12);
        assert!((point.geometry.y - libm::sqrt(3.0) * 0.5).abs() < 1.0e-12);
    }

    #[test]
    fn rejects_bare_scalar_in_angle_position() {
        let mut env = Environment::new();
        insert(&mut env, "P", resolved_point(Point::new(1.0, 0.0)));
        insert(&mut env, "O", resolved_point(Point::new(0.0, 0.0)));
        let op = op_from("locus 2\npoint R = rotate(P, O, 60)\n");
        let mut evaluator = test_evaluator(&env);
        let error = evaluate_op_value(&mut evaluator, &op).expect_err("unit is mandatory");
        assert_eq!(error.code, "E209");
        assert!(error.message.contains("explicit `deg` or `rad`"));
    }

    #[test]
    fn infers_named_length_from_a_later_constructor_use() {
        let parsed = parse(
            "locus 2\nnum radius = 3\npoint O = free ~(0, 0)\ncircle c = circle(O, radius)\n",
        );
        let Evaluation::Solved { scene, .. } = evaluate(&parsed.file).expect("free scene solves")
        else {
            unreachable!("free point is solved");
        };
        let (_, object) = scene.objects.first().expect("derived numeric value");
        assert_eq!(
            object,
            &ResolvedObject::Num(NumericValue {
                value: 3.0,
                dimension: Dimension::Length,
            })
        );
    }

    #[test]
    fn detects_conflicting_whole_file_numeric_dimensions() {
        let parsed = parse(
            "locus 2\nnum x = 2\npoint P = free ~(0, 0)\ncircle c = circle(P, x)\npoint Q = rotate(P, P, x)\n",
        );
        let error = evaluate(&parsed.file).expect_err("x cannot be both length and angle");
        assert_eq!(error.code, "E209");
        assert!(error.message.contains("conflicting dimensions"));
    }

    #[test]
    fn infers_area_and_sqrt_dimensions_across_named_values() {
        let parsed = parse(
            "locus 2\nnum w = 3\nnum h = 4\nnum a = w * h\nnum side = sqrt(a)\npoint P = free ~(0, 0)\nrect r = rect(P, w, h)\nassert area(r) == a\n",
        );
        let Evaluation::Solved { scene, .. } = evaluate(&parsed.file).expect("valid solve") else {
            unreachable!("free point is solved");
        };
        let dimensions = scene
            .objects
            .values()
            .filter_map(|object| match object {
                ResolvedObject::Num(number) => Some(number.dimension),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            dimensions,
            [
                Dimension::Length,
                Dimension::Length,
                Dimension::Area,
                Dimension::Length,
            ]
        );
    }

    #[test]
    fn infers_area_divided_by_length_as_length() {
        let parsed = parse(
            "locus 2\npoint A = free ~(0, 0)\npoint B = free ~(3, 0)\npoint C = free ~(0, 4)\npolygon t = polygon(A, B, C)\nnum h = area(t) / dist(A, B)\n",
        );
        let dimensions = infer_dimensions(&parsed.file).expect("spec-valid quotient");
        assert_eq!(
            dimensions.get("h"),
            Some(&InferredDimension::Number(Dimension::Length))
        );
    }

    #[test]
    fn contextual_literals_propagate_through_groups_sums_and_either_comparison_side() {
        for expression in ["(3)", "2 + 1", "sqrt(9)", "min(3, 4)", "abs(-3)"] {
            let source = format!(
                "locus 2\nnum w = {expression}\npoint P = free ~(0, 0)\ncircle c = circle(P, w)\n"
            );
            let parsed = parse(&source);
            let Evaluation::Solved { scene, .. } =
                evaluate(&parsed.file).expect("contextual length literal")
            else {
                unreachable!("free point is solved");
            };
            let (_, ResolvedObject::Num(value)) = scene.objects.first().expect("w") else {
                unreachable!("w is numeric");
            };
            assert_eq!(value.dimension, Dimension::Length);
            assert!((value.value - 3.0).abs() < f64::EPSILON);
        }

        let mut env = Environment::new();
        insert(&mut env, "A", resolved_point(Point::new(0.0, 0.0)));
        insert(&mut env, "B", resolved_point(Point::new(6.0, 0.0)));
        let parsed = parse("locus 2\nassert 6 == dist(A, B)\nassert dist(A, B) == 6\n");
        let mut evaluator = test_evaluator(&env);
        for statement in &parsed.file.statements {
            let Statement::Assert(assertion) = statement else {
                unreachable!("assertion expected");
            };
            assert_eq!(
                evaluator.eval(&assertion.expr).expect("unified comparison"),
                Value::Bool(true)
            );
        }
    }

    #[test]
    fn contextual_lengths_propagate_through_products_and_quotients() {
        let mut env = Environment::new();
        insert(&mut env, "O", resolved_point(Point::new(0.0, 0.0)));
        for expression in ["2 * 3", "12 / 2"] {
            let op = op_from(&format!("locus 2\ncircle c = circle(O, {expression})\n"));
            let mut evaluator = test_evaluator(&env);
            let ResolvedObject::Circle(circle) =
                evaluate_op_value(&mut evaluator, &op).expect("contextual length product")
            else {
                unreachable!("circle expected");
            };
            assert!((circle.radius - 6.0).abs() < f64::EPSILON);
        }
    }

    #[test]
    fn unconstrained_products_resolve_after_scalar_operand_defaults() {
        let angle_file = parse("locus 2\nnum t = 2 * 30deg\n").file;
        let dimensions = infer_dimensions(&angle_file).expect("unconstrained angle product");
        assert_eq!(dimensions.get("t"), Some(&InferredDimension::Angle));
        let Evaluation::Resolved(scene) = evaluate(&angle_file).expect("angle product evaluates")
        else {
            unreachable!("derived file resolves directly");
        };
        let ResolvedObject::Angle(angle) = scene.objects.first().expect("t exists").1 else {
            unreachable!("t is an angle");
        };
        assert!((*angle - core::f64::consts::PI / 3.0).abs() < 1.0e-12);

        let length_file = parse(
            "locus 2\npoint O = free ~(0, 0)\ncircle c = circle(O, 6)\nnum divisor = 2\nnum d = radius(c) / divisor\n",
        )
        .file;
        let dimensions = infer_dimensions(&length_file).expect("unconstrained length quotient");
        assert_eq!(
            dimensions.get("d"),
            Some(&InferredDimension::Number(Dimension::Length))
        );
        evaluate(&length_file).expect("length quotient evaluates");
    }

    #[test]
    fn deferred_completion_preserves_context_in_sums_and_extrema() {
        for expression in ["d + 1", "min(d, 1)"] {
            let parsed = parse(&format!(
                "locus 2\npoint O = free ~(0, 0)\ncircle c = circle(O, 4)\nnum d = radius(c) / 2\nnum e = {expression}\n"
            ));
            let dimensions = infer_dimensions(&parsed.file).expect("deferred length expression");
            for name in ["d", "e"] {
                assert_eq!(
                    dimensions.get(name),
                    Some(&InferredDimension::Number(Dimension::Length))
                );
            }
            evaluate(&parsed.file).expect("deferred expression evaluates");
        }
    }

    #[test]
    fn contextual_dimensions_propagate_through_compound_expressions() {
        for expression in ["radius(c) / 2 + (1 + 1)", "min(radius(c) / 2, abs(1))"] {
            let parsed = parse(&format!(
                "locus 2\npoint O = free ~(0, 0)\ncircle c = circle(O, 4)\nnum d = {expression}\n"
            ));
            let dimensions = infer_dimensions(&parsed.file).expect("compound length expression");
            assert_eq!(
                dimensions.get("d"),
                Some(&InferredDimension::Number(Dimension::Length))
            );
            evaluate(&parsed.file).expect("compound expression evaluates");
        }
    }

    #[test]
    fn repeated_product_operand_uses_later_area_constraint() {
        let parsed = parse(
            "locus 2\npoint O = free ~(0, 0)\nrect r = rect(O, 3, 3)\nnum side = 3\nnum a = side * side\nassert a == area(r)\n",
        );
        let dimensions = infer_dimensions(&parsed.file).expect("later area constraint");
        assert_eq!(
            dimensions.get("side"),
            Some(&InferredDimension::Number(Dimension::Length))
        );
        assert_eq!(
            dimensions.get("a"),
            Some(&InferredDimension::Number(Dimension::Area))
        );
        evaluate(&parsed.file).expect("later area constraint evaluates");
    }

    #[test]
    fn dimension_inference_scales_to_many_independent_numbers() {
        use std::fmt::Write as _;

        let mut source = String::from("locus 2\n");
        for index in 0..640 {
            writeln!(source, "num n{index} = {index}").expect("write to string");
        }
        let parsed = parse(&source);
        let dimensions = infer_dimensions(&parsed.file).expect("independent numbers infer");
        assert_eq!(dimensions.len(), 640);
        assert!(
            dimensions
                .values()
                .all(|dimension| { *dimension == InferredDimension::Number(Dimension::Scalar) })
        );
    }

    #[test]
    fn ambiguous_product_waits_for_later_operand_constraint() {
        let parsed = parse(
            "locus 2\nnum scale = 2\nnum width = 3\npoint O = free ~(0, 0)\ncircle c = circle(O, scale * width)\npoint P = lerp(O, O, scale)\n",
        );
        let dimensions = infer_dimensions(&parsed.file).expect("later constraint disambiguates");
        assert_eq!(
            dimensions.get("scale"),
            Some(&InferredDimension::Number(Dimension::Scalar))
        );
        assert_eq!(
            dimensions.get("width"),
            Some(&InferredDimension::Number(Dimension::Length))
        );
        evaluate(&parsed.file).expect("statement order does not change validity");
    }

    #[test]
    fn contextual_multiplication_uses_the_known_scalar_operand() {
        let parsed = parse(
            "locus 2\nnum scale = ratio(2, 1)\npoint O = free ~(0, 0)\ncircle lhs = circle(O, scale * 3)\ncircle rhs = circle(O, 3 * scale)\n",
        );
        let Evaluation::Solved { scene, .. } =
            evaluate(&parsed.file).expect("both operand orders evaluate")
        else {
            unreachable!("free point is solved");
        };
        let radii = scene
            .objects
            .values()
            .filter_map(|object| match object {
                ResolvedObject::Circle(circle) => Some(circle.radius),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(radii, [6.0, 6.0]);
    }

    #[test]
    fn scalar_quotients_accept_equal_length_and_area_dimensions() {
        let parsed = parse(
            "locus 2\npoint O = free ~(0, 0)\ncircle c = circle(O, 6)\nrect r = rect(O, 2, 3)\nnum length_scale = radius(c) / 3\nnum area_scale = area(r) / 3\npoint P = lerp(O, O, length_scale)\npoint Q = lerp(O, O, area_scale)\n",
        );
        let dimensions = infer_dimensions(&parsed.file).expect("equal-dimension quotients");
        for name in ["length_scale", "area_scale"] {
            assert_eq!(
                dimensions.get(name),
                Some(&InferredDimension::Number(Dimension::Scalar))
            );
        }
        evaluate(&parsed.file).expect("scalar quotients evaluate");
    }

    #[test]
    fn contextual_scalar_quotients_accept_nested_angle_operands() {
        let mut env = Environment::new();
        insert(
            &mut env,
            "c",
            ResolvedObject::Circle(Circle::new(Point::new(0.0, 0.0), 6.0).expect("circle")),
        );
        let op = op_from("locus 2\nnum q = (30deg * (radius(c) / 3)) / 60deg\n");
        let mut evaluator = test_evaluator(&env);
        let ResolvedObject::Num(value) =
            evaluate_op_value(&mut evaluator, &op).expect("angle quotient evaluates as scalar")
        else {
            unreachable!("scalar expected");
        };
        assert_eq!(value.dimension, Dimension::Scalar);
        assert!((value.value - 1.0).abs() < 1.0e-12);
    }

    #[test]
    fn inline_angle_relations_use_contextual_solver_evaluation() {
        let parsed = parse(
            "locus 2\npoint O = free ~(0, 0)\ncircle c = circle(O, 6)\nrel 30deg * (radius(c) / 3) == 60deg\nrel 30deg / (radius(c) / 3) == 15deg\n",
        );
        let Evaluation::Solved { summary, .. } =
            evaluate(&parsed.file).expect("inline angle relations solve")
        else {
            unreachable!("free point requires the solver");
        };
        assert_eq!(summary.equations, 2);
        assert!(summary.max_normalized_residual < 1.0e-12);
    }

    #[test]
    fn solver_relations_prefer_inferred_length_over_scalar_tolerance() {
        let parsed = parse(
            "locus 2\npoint O = free ~(0, 0)\npoint Far = free ~(1000000000000, 0)\ncircle c = circle(O, 6)\nrel radius(c) / 2 == 3.000001\n",
        );
        let Evaluation::Solved { summary, .. } =
            evaluate(&parsed.file).expect("length-scaled relation is within tolerance")
        else {
            unreachable!("free points require the solver");
        };
        assert!(summary.max_normalized_residual < 1.0e-12);
    }

    #[test]
    fn assertions_and_ratio_arguments_contextually_evaluate_nested_angles() {
        let parsed = parse(
            "locus 2\npoint O = free ~(0, 0)\ncircle c = circle(O, 6)\nnum q = ratio(30deg * (radius(c) / 3), 60deg)\nassert 30deg * (radius(c) / 3) == 60deg\n",
        );
        let Evaluation::Solved { scene, .. } =
            evaluate(&parsed.file).expect("ratio arguments evaluate")
        else {
            unreachable!("free point requires the solver");
        };
        let ResolvedObject::Num(value) = scene
            .objects
            .iter()
            .find_map(|(id, object)| (id.name == "q").then_some(object))
            .expect("q exists")
        else {
            unreachable!("q is scalar");
        };
        assert_eq!(value.dimension, Dimension::Scalar);
        assert!((value.value - 1.0).abs() < 1.0e-12);
        assert!(
            crate::evaluate_assertions(&parsed.file, &scene)
                .expect("assertion evaluates")
                .passed
        );
    }

    #[test]
    fn inferred_dimensions_contextualize_both_numeric_operands() {
        let parsed = parse(
            "locus 2\npoint O = free ~(0, 0)\ncircle c = circle(O, 6)\nnum q = ratio(30deg * (radius(c) / 3), 15deg * (radius(c) / 1.5))\nassert 30deg * (radius(c) / 3) == 15deg * (radius(c) / 1.5)\n",
        );
        let Evaluation::Solved { scene, .. } =
            evaluate(&parsed.file).expect("both ratio operands use inferred angle context")
        else {
            unreachable!("free point requires the solver");
        };
        let ResolvedObject::Num(value) = scene
            .objects
            .iter()
            .find_map(|(id, object)| (id.name == "q").then_some(object))
            .expect("q exists")
        else {
            unreachable!("q is scalar");
        };
        assert_eq!(value.dimension, Dimension::Scalar);
        assert!((value.value - 1.0).abs() < 1.0e-12);
        assert!(
            crate::evaluate_assertions(&parsed.file, &scene)
                .expect("both assertion operands use inferred angle context")
                .passed
        );
    }

    #[test]
    fn solver_relations_use_inferred_dimensions_for_both_sqrt_operands() {
        let parsed = parse(
            "locus 2\npoint O = free ~(0, 0)\ncircle c = circle(O, 6)\nrel sqrt(radius(c) * 3) == sqrt(18)\n",
        );
        let Evaluation::Solved { summary, .. } =
            evaluate(&parsed.file).expect("both square roots use inferred length context")
        else {
            unreachable!("free point requires the solver");
        };
        assert!(summary.max_normalized_residual < 1.0e-12);
    }

    #[test]
    fn pin_coordinates_use_contextual_length_evaluation() {
        let parsed = parse(
            "locus 2\npoint O = free ~(0, 0)\ncircle c = circle(O, 6)\npoint P = free ~(4, 0)\nrel pin(P, sqrt(radius(c) * 3), 0)\n",
        );
        let Evaluation::Solved { scene, summary } =
            evaluate(&parsed.file).expect("compound pin coordinate solves")
        else {
            unreachable!("free points require the solver");
        };
        let ResolvedObject::Point(point) = scene
            .objects
            .iter()
            .find_map(|(id, object)| (id.name == "P").then_some(object))
            .expect("P exists")
        else {
            unreachable!("P is a point");
        };
        assert!((point.geometry.x - libm::sqrt(18.0)).abs() < 1.0e-9);
        assert!(point.geometry.y.abs() < 1.0e-12);
        assert!(summary.max_normalized_residual < 1.0e-9);
    }

    #[test]
    fn product_inference_waits_for_both_operand_dimensions() {
        let parsed = parse(
            "locus 2\nnum w = 2\npoint O = free ~(0, 0)\ncircle c = circle(O, w)\nnum h = 3\nnum a = w * h\nrect r = rect(O, w, h)\n",
        );
        let dimensions = infer_dimensions(&parsed.file).expect("order-independent inference");
        assert_eq!(
            dimensions.get("a"),
            Some(&InferredDimension::Number(Dimension::Area))
        );
        let Evaluation::Solved { scene, .. } = evaluate(&parsed.file).expect("valid area product")
        else {
            unreachable!("free point is solved");
        };
        let ResolvedObject::Num(area) = scene
            .objects
            .iter()
            .find_map(|(id, object)| (id.name == "a").then_some(object))
            .expect("a exists")
        else {
            unreachable!("a is numeric");
        };
        assert_eq!(area.dimension, Dimension::Area);
        assert!((area.value - 6.0).abs() < f64::EPSILON);
    }

    #[test]
    fn text_block_anchor_constrains_nested_constructor_arguments() {
        let parsed = parse(
            "locus 2\nnum r = 2\npoint O = free ~(0, 0)\ntext_block note = text_block(\"test\") { anchor: circle(O, r).center }\n",
        );
        let dimensions = infer_dimensions(&parsed.file).expect("text-block anchor inference");
        assert_eq!(
            dimensions.get("r"),
            Some(&InferredDimension::Number(Dimension::Length))
        );
        evaluate(&parsed.file).expect("text-block anchor evaluates");
    }

    #[test]
    fn angle_products_infer_named_angle_in_either_operand_order() {
        let mut env = Environment::new();
        insert(&mut env, "P", resolved_point(Point::new(1.0, 0.0)));
        insert(&mut env, "O", resolved_point(Point::new(0.0, 0.0)));
        insert(
            &mut env,
            "t",
            ResolvedObject::Angle(core::f64::consts::PI / 6.0),
        );
        for expression in ["2 * t", "t * 2"] {
            let source = format!(
                "locus 2\nnum t = 30deg\npoint P = free ~(1, 0)\npoint O = free ~(0, 0)\npoint R = rotate(P, O, {expression})\n"
            );
            let parsed = parse(&source);
            let dimensions = infer_dimensions(&parsed.file).expect("angle product inference");
            assert_eq!(dimensions.get("t"), Some(&InferredDimension::Angle));

            let op = op_from(&format!("locus 2\npoint R = rotate(P, O, {expression})\n"));
            let mut evaluator = test_evaluator(&env);
            let ResolvedObject::Point(point) =
                evaluate_op_value(&mut evaluator, &op).expect("angle product evaluation")
            else {
                unreachable!("rotation returns a point");
            };
            assert!((point.geometry.x - 0.5).abs() < 1.0e-12);
            assert!((point.geometry.y - libm::sqrt(3.0) * 0.5).abs() < 1.0e-12);
        }
    }

    #[test]
    fn nested_angle_products_use_known_scalar_and_angle_sides() {
        for (expression, expected) in [
            (
                "ratio(2, 1) * (30deg * 2)",
                2.0 * core::f64::consts::PI / 3.0,
            ),
            (
                "(30deg * 2) * ratio(2, 1)",
                2.0 * core::f64::consts::PI / 3.0,
            ),
            ("(2 * 3) * (30deg * 2)", 2.0 * core::f64::consts::PI),
            ("(30deg * 2) * (2 * 3)", 2.0 * core::f64::consts::PI),
        ] {
            let parsed = parse(&format!("locus 2\nnum t = {expression}\n"));
            let dimensions = infer_dimensions(&parsed.file).expect("nested angle inference");
            assert_eq!(dimensions.get("t"), Some(&InferredDimension::Angle));

            let Evaluation::Resolved(scene) = evaluate(&parsed.file).expect("nested angle product")
            else {
                unreachable!("derived file resolves directly");
            };
            let ResolvedObject::Angle(angle) = scene.objects.first().expect("t exists").1 else {
                unreachable!("t is an angle");
            };
            assert!((*angle - expected).abs() < 1.0e-12);
        }
    }

    #[test]
    fn solver_contextually_evaluates_compound_scalar_angle_operands() {
        for (operator, expected) in [
            ("*", core::f64::consts::PI / 3.0),
            ("/", core::f64::consts::PI / 12.0),
        ] {
            let parsed = parse(&format!(
                "locus 2\npoint O = free ~(0, 0)\ncircle c = circle(O, 6)\nnum t = 30deg {operator} (radius(c) / 3)\n"
            ));
            let Evaluation::Solved { scene, .. } =
                evaluate(&parsed.file).expect("complete scene evaluates through solver")
            else {
                unreachable!("free point requires the solver");
            };
            let ResolvedObject::Angle(angle) = scene
                .objects
                .iter()
                .find_map(|(id, object)| (id.name == "t").then_some(object))
                .expect("t exists")
            else {
                unreachable!("t is an angle");
            };
            assert!((*angle - expected).abs() < 1.0e-12);
        }
    }

    #[test]
    fn nested_constructor_calls_impose_argument_dimensions() {
        let parsed = parse(
            "locus 2\nnum r = 2\npoint P = free ~(0, 0)\npoint O = free ~(0, 0)\nrel on(P, circle(O, r))\n",
        );
        let dimensions = infer_dimensions(&parsed.file).expect("nested circle inference");
        assert_eq!(
            dimensions.get("r"),
            Some(&InferredDimension::Number(Dimension::Length))
        );
    }

    #[test]
    fn resolved_points_and_angle_arcs_retain_validated_props() {
        let mut env = Environment::new();
        insert(&mut env, "A", resolved_point(Point::new(1.0, 0.0)));
        insert(&mut env, "B", resolved_point(Point::new(0.0, 0.0)));
        insert(&mut env, "C", resolved_point(Point::new(0.0, 1.0)));
        let point = op_from("locus 2\npoint M = midpoint(A, B) { mark: cross }\n");
        let arc = op_from("locus 2\narc a = angle_arc(A, B, C, 0.5) { n: 3 }\n");
        let mut evaluator = test_evaluator(&env);
        let ResolvedObject::Point(point) =
            evaluate_op_value(&mut evaluator, &point).expect("marked point")
        else {
            unreachable!("point expected");
        };
        assert_eq!(point.mark, PointMark::Cross);
        let ResolvedObject::Arc(arc) =
            evaluate_op_value(&mut evaluator, &arc).expect("marked angle arc")
        else {
            unreachable!("arc expected");
        };
        assert_eq!(arc.angle_mark_count, Some(3));

        let invalid = op_from("locus 2\narc a = angle_arc(A, B, C, 0.5) { n: 5 }\n");
        let error = evaluate_op_value(&mut evaluator, &invalid).expect_err("count range");
        assert_eq!(error.code, "E204");
        assert!(error.message.contains("1..=4"));
    }

    #[test]
    fn num_only_file_resolves_and_scene_recomputes() {
        let parsed = parse("locus 2\nnum phi = (1 + sqrt(5)) / 2\n");
        let mut scene = crate::Scene::new(parsed.file).expect("derived scene");
        let resolved = scene.resolved().expect("complete scene");
        let (_, ResolvedObject::Num(phi)) = resolved.objects.first().expect("phi") else {
            unreachable!("phi is numeric");
        };
        assert!((phi.value - (1.0 + libm::sqrt(5.0)) * 0.5).abs() < 1.0e-12);
        assert!(matches!(
            scene.recompute().expect("recompute"),
            Evaluation::Resolved(_)
        ));
    }

    #[test]
    fn resolves_annotation_spec_without_layout_position() {
        let mut env = Environment::new();
        insert(&mut env, "P", resolved_point(Point::new(2.0, 3.0)));
        let op = op_from("locus 2\nlabel note = label(P, \"hello\") { dir: NE, gap: 0.25 }\n");
        let mut evaluator = test_evaluator(&env);
        let object = evaluate_op_value(&mut evaluator, &op).expect("annotation spec");
        let ResolvedObject::Annotation(AnnotationSpec::Label {
            anchor,
            direction,
            gap,
            ..
        }) = object
        else {
            unreachable!("label resolves to an annotation");
        };
        assert_eq!(
            anchor,
            crate::scene::AnchorSpec::Point(Point::new(2.0, 3.0))
        );
        assert_eq!(direction, LabelDirection::Compass(CompassDirection::Ne));
        assert_eq!(gap, Some(0.25));
    }

    #[test]
    fn snapshots_measured_circle_intersection_error() {
        let mut env = Environment::new();
        insert(
            &mut env,
            "c1",
            ResolvedObject::Circle(Circle::new(Point::new(0.0, 0.0), 1.0).expect("circle")),
        );
        insert(
            &mut env,
            "c2",
            ResolvedObject::Circle(Circle::new(Point::new(4.0, 0.0), 0.9).expect("circle")),
        );
        let op = op_from("locus 2\npoint X = intersect(c1, c2)\n");
        let mut evaluator = test_evaluator(&env);
        let error = evaluate_op_value(&mut evaluator, &op)
            .map_err(|error| attach_object(error, "X"))
            .expect_err("circles are disjoint");
        insta::assert_debug_snapshot!(
            (
                error.code,
                error.span.line,
                error.object_id.as_deref(),
                error.message.as_str(),
                error.measured_note(),
            ),
            @r###"
        (
            "E201",
            2,
            Some(
                "X",
            ),
            "objects do not intersect",
            Some(
                "first radius 1.000000 scene units, second radius 0.900000 scene units, center distance 4.000000 scene units, radii sum 1.900000 scene units",
            ),
        )
        "###
        );
    }

    #[test]
    fn snapshots_measured_division_domain_error() {
        let parsed = parse("locus 2\nnum bad = 3 / 0\n");
        let error = evaluate(&parsed.file).expect_err("zero denominator");
        insta::assert_debug_snapshot!(
            (
                error.code,
                error.span.line,
                error.object_id.as_deref(),
                error.message.as_str(),
                error.measured_note(),
            ),
            @r###"
        (
            "E204",
            2,
            Some(
                "bad",
            ),
            "division denominator is zero under construction tolerance",
            Some(
                "denominator 0.000000",
            ),
        )
        "###
        );
    }

    #[test]
    fn every_spec_fixture_solves_relationally() -> Result<(), Box<dyn std::error::Error>> {
        let corpus = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../spec/test");
        let mut paths = fs::read_dir(corpus)?
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| {
                path.extension()
                    .is_some_and(|extension| extension == "locus")
            })
            .collect::<Vec<_>>();
        paths.sort();
        for path in paths {
            let source = fs::read_to_string(&path)?;
            let parsed = parse(&source);
            let evaluation = evaluate(&parsed.file)
                .map_err(|error| format!("{} failed relational solve: {error}", path.display()))?;
            assert!(
                matches!(evaluation, Evaluation::Solved { .. }),
                "{} should solve",
                path.display()
            );
        }
        Ok(())
    }
}
