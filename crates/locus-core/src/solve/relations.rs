//! Source-ordered relation expansion and normalized residual semantics.

use locus_syntax::{BinOp, Expr, File, Span, Statement};

use crate::eval::EvalError;

use super::ad::{
    DArc, DCircle, DLinear, DObject, DPoint, DifferentiatedFile, Jet, LinearDomain,
    NumericDimension,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CircleBranch {
    External,
    InternalFirstOuter,
    InternalSecondOuter,
}

#[derive(Clone, Debug)]
pub(crate) struct SourceRelation<'a> {
    pub(crate) expression: &'a Expr,
    pub(crate) span: Span,
}

#[derive(Clone, Debug)]
pub(crate) struct Equation {
    pub(crate) residual: Jet,
    pub(crate) relation: usize,
}

pub(crate) fn collect(file: &File) -> Vec<SourceRelation<'_>> {
    let mut result = Vec::new();
    for statement in &file.statements {
        collect_statement(statement, &mut result);
    }
    result
}

fn collect_statement<'a>(statement: &'a Statement, result: &mut Vec<SourceRelation<'a>>) {
    match statement {
        Statement::Op(op) => {
            for expression in &op.where_clauses {
                flatten(expression, result);
            }
        }
        Statement::Rel(relation) => flatten(&relation.expr, result),
        Statement::Assert(_) | Statement::Meta(_) | Statement::Style(_) | Statement::Class(_) => {}
    }
}

fn flatten<'a>(expression: &'a Expr, result: &mut Vec<SourceRelation<'a>>) {
    if let Expr::Binary {
        op: BinOp::And,
        lhs,
        rhs,
        ..
    } = expression
    {
        flatten(lhs, result);
        flatten(rhs, result);
    } else {
        result.push(SourceRelation {
            expression,
            span: expression.span(),
        });
    }
}

pub(crate) fn select_circle_branches(
    differentiated: &DifferentiatedFile,
    relations: &[SourceRelation<'_>],
) -> Result<Vec<Option<CircleBranch>>, EvalError> {
    relations
        .iter()
        .map(|relation| select_circle_branch(differentiated, relation.expression))
        .collect()
}

fn select_circle_branch(
    differentiated: &DifferentiatedFile,
    expression: &Expr,
) -> Result<Option<CircleBranch>, EvalError> {
    let Expr::Call { name, args, span } = expression else {
        return Ok(None);
    };
    if name.name != "tangent" || args.len() != 2 {
        return Ok(None);
    }
    let lhs = differentiated.object(&args[0])?;
    let rhs = differentiated.object(&args[1])?;
    let (DObject::Circle(lhs), DObject::Circle(rhs)) = (lhs, rhs) else {
        return Ok(None);
    };
    choose_circle_branch(&lhs, &rhs, *span).map(Some)
}

fn choose_circle_branch(
    lhs: &DCircle,
    rhs: &DCircle,
    span: Span,
) -> Result<CircleBranch, EvalError> {
    if lhs.radius.value <= 0.0 || rhs.radius.value <= 0.0 {
        return Err(relation_error(
            span,
            "circle tangency seed radii must be positive",
        ));
    }
    let distance = rhs
        .center
        .sub(&lhs.center)
        .norm()
        .map_err(|()| relation_error(span, "circle tangency centers coincide"))?
        .value;
    let external_error = (distance - (lhs.radius.value + rhs.radius.value)).abs();
    let internal_error = (distance - (lhs.radius.value - rhs.radius.value).abs()).abs();
    if external_error <= internal_error {
        Ok(CircleBranch::External)
    } else if lhs.radius.value > rhs.radius.value {
        Ok(CircleBranch::InternalFirstOuter)
    } else if rhs.radius.value > lhs.radius.value {
        Ok(CircleBranch::InternalSecondOuter)
    } else {
        Err(relation_error(
            span,
            "equal seed radii cannot select internal tangency",
        ))
    }
}

pub(crate) fn evaluate(
    differentiated: &DifferentiatedFile,
    relations: &[SourceRelation<'_>],
    branches: &[Option<CircleBranch>],
    diameter: f64,
) -> Result<Vec<Equation>, EvalError> {
    let mut equations = Vec::new();
    for (index, relation) in relations.iter().enumerate() {
        evaluate_atomic(
            differentiated,
            relation.expression,
            index,
            branches[index],
            diameter,
            &mut equations,
        )?;
    }
    Ok(equations)
}

fn evaluate_atomic(
    differentiated: &DifferentiatedFile,
    expression: &Expr,
    relation: usize,
    branch: Option<CircleBranch>,
    diameter: f64,
    equations: &mut Vec<Equation>,
) -> Result<(), EvalError> {
    if let Expr::Binary {
        op: BinOp::Eq,
        lhs,
        rhs,
        span,
    } = expression
    {
        return numeric_equality(
            differentiated,
            lhs,
            rhs,
            relation,
            *span,
            diameter,
            equations,
        );
    }
    let Expr::Call { name, args, span } = expression else {
        return Err(assert_only_relation_error(expression.span()));
    };
    if matches!(name.name.as_str(), "between" | "inside") {
        return Err(assert_only_relation_error(*span));
    }
    evaluate_predicate(
        differentiated,
        &name.name,
        args,
        relation,
        branch,
        *span,
        diameter,
        equations,
    )
}

#[allow(
    clippy::too_many_arguments,
    reason = "source-ordered residual sink remains explicit"
)]
fn evaluate_predicate(
    differentiated: &DifferentiatedFile,
    name: &str,
    args: &[Expr],
    relation: usize,
    branch: Option<CircleBranch>,
    span: Span,
    diameter: f64,
    equations: &mut Vec<Equation>,
) -> Result<(), EvalError> {
    if matches!(name, "on" | "tangent" | "parallel" | "perpendicular") {
        return evaluate_object_predicate(
            differentiated,
            name,
            args,
            relation,
            branch,
            span,
            diameter,
            equations,
        );
    }
    evaluate_point_predicate(
        differentiated,
        name,
        args,
        relation,
        span,
        diameter,
        equations,
    )
}

#[allow(
    clippy::too_many_arguments,
    reason = "source-ordered residual sink remains explicit"
)]
fn evaluate_object_predicate(
    differentiated: &DifferentiatedFile,
    name: &str,
    args: &[Expr],
    relation: usize,
    branch: Option<CircleBranch>,
    span: Span,
    diameter: f64,
    equations: &mut Vec<Equation>,
) -> Result<(), EvalError> {
    match name {
        "on" => on_relation(differentiated, args, relation, span, diameter, equations),
        "tangent" => tangent_relation(
            differentiated,
            args,
            relation,
            branch,
            span,
            diameter,
            equations,
        ),
        "parallel" | "perpendicular" => {
            direction_relation(differentiated, name, args, relation, span, equations)
        }
        _ => Err(relation_error(
            span,
            format!("unsupported object relation `{name}`"),
        )),
    }
}

fn evaluate_point_predicate(
    differentiated: &DifferentiatedFile,
    name: &str,
    args: &[Expr],
    relation: usize,
    span: Span,
    diameter: f64,
    equations: &mut Vec<Equation>,
) -> Result<(), EvalError> {
    if matches!(name, "collinear" | "concyclic" | "congruent") {
        return evaluate_multi_point_predicate(
            differentiated,
            name,
            args,
            relation,
            span,
            diameter,
            equations,
        );
    }
    match name {
        "coincident" => {
            coincident_relation(differentiated, args, relation, span, diameter, equations)
        }
        "midpoint" => midpoint_relation(differentiated, args, relation, span, diameter, equations),
        "horizontal" | "vertical" => axis_relation(
            differentiated,
            name,
            args,
            relation,
            span,
            diameter,
            equations,
        ),
        "pin" => pin_relation(differentiated, args, relation, span, diameter, equations),
        _ => Err(relation_error(
            span,
            format!("unsupported relation `{name}`"),
        )),
    }
}

fn evaluate_multi_point_predicate(
    differentiated: &DifferentiatedFile,
    name: &str,
    args: &[Expr],
    relation: usize,
    span: Span,
    diameter: f64,
    equations: &mut Vec<Equation>,
) -> Result<(), EvalError> {
    match name {
        "collinear" => {
            collinear_relation(differentiated, args, relation, span, diameter, equations)
        }
        "concyclic" => {
            concyclic_relation(differentiated, args, relation, span, diameter, equations)
        }
        "congruent" => {
            congruent_relation(differentiated, args, relation, span, diameter, equations)
        }
        _ => Err(relation_error(
            span,
            format!("unsupported multi-point relation `{name}`"),
        )),
    }
}

fn numeric_equality(
    differentiated: &DifferentiatedFile,
    lhs: &Expr,
    rhs: &Expr,
    relation: usize,
    span: Span,
    diameter: f64,
    equations: &mut Vec<Equation>,
) -> Result<(), EvalError> {
    let (lhs, rhs, dimension) = differentiated.matching_numbers(lhs, rhs)?;
    let scale = numeric_scale(&lhs, &rhs, dimension, diameter);
    push(equations, lhs.sub(&rhs).div(&scale), relation, span)
}

fn on_relation(
    differentiated: &DifferentiatedFile,
    args: &[Expr],
    relation: usize,
    span: Span,
    diameter: f64,
    equations: &mut Vec<Equation>,
) -> Result<(), EvalError> {
    require_arity("on", args, 2, span)?;
    let point = expect_point(differentiated.object(&args[0])?, args[0].span())?;
    let residual = match differentiated.object(&args[1])? {
        DObject::Linear(linear) => point_linear_residual(&point, &linear, span)?,
        DObject::Circle(circle) => point_circle_residual(&point, &circle, span)?,
        DObject::Arc(arc) => point_arc_residual(&point, &arc, span)?,
        _ => {
            return Err(relation_error(
                args[1].span(),
                "on target must be linear, circle, or arc",
            ));
        }
    };
    push(equations, residual.scale(1.0 / diameter), relation, span)
}

fn tangent_relation(
    differentiated: &DifferentiatedFile,
    args: &[Expr],
    relation: usize,
    branch: Option<CircleBranch>,
    span: Span,
    diameter: f64,
    equations: &mut Vec<Equation>,
) -> Result<(), EvalError> {
    require_arity("tangent", args, 2, span)?;
    let lhs = differentiated.object(&args[0])?;
    let rhs = differentiated.object(&args[1])?;
    let residual = match (lhs, rhs) {
        (DObject::Circle(lhs), DObject::Circle(rhs)) => {
            circle_tangent_residual(&lhs, &rhs, branch, span)?
        }
        (DObject::Circle(circle), DObject::Linear(linear))
        | (DObject::Linear(linear), DObject::Circle(circle)) => {
            point_linear_residual(&circle.center, &linear, span)?
                .abs()
                .sub(&circle.radius)
        }
        _ => {
            return Err(relation_error(
                span,
                "tangent requires circle-circle or circle-linear operands",
            ));
        }
    };
    push(equations, residual.scale(1.0 / diameter), relation, span)
}

fn circle_tangent_residual(
    lhs: &DCircle,
    rhs: &DCircle,
    branch: Option<CircleBranch>,
    span: Span,
) -> Result<Jet, EvalError> {
    let distance = rhs
        .center
        .sub(&lhs.center)
        .norm()
        .map_err(|()| relation_error(span, "circle centers coincide"))?;
    let target = match branch.unwrap_or(CircleBranch::External) {
        CircleBranch::External => lhs.radius.add(&rhs.radius),
        CircleBranch::InternalFirstOuter => {
            if lhs.radius.value < rhs.radius.value {
                return Err(relation_error(
                    span,
                    "internal tangency radius ordering reversed",
                ));
            }
            lhs.radius.sub(&rhs.radius)
        }
        CircleBranch::InternalSecondOuter => {
            if rhs.radius.value < lhs.radius.value {
                return Err(relation_error(
                    span,
                    "internal tangency radius ordering reversed",
                ));
            }
            rhs.radius.sub(&lhs.radius)
        }
    };
    Ok(distance.sub(&target))
}

fn direction_relation(
    differentiated: &DifferentiatedFile,
    name: &str,
    args: &[Expr],
    relation: usize,
    span: Span,
    equations: &mut Vec<Equation>,
) -> Result<(), EvalError> {
    require_arity(name, args, 2, span)?;
    let lhs = expect_linear(differentiated.object(&args[0])?, args[0].span())?.direction();
    let rhs = expect_linear(differentiated.object(&args[1])?, args[1].span())?.direction();
    let denominator = lhs
        .norm()
        .and_then(|left| rhs.norm().map(|right| left.mul(&right)))
        .map_err(|()| relation_error(span, "direction relation has a degenerate carrier"))?;
    let numerator = if name == "parallel" {
        lhs.cross(&rhs)
    } else {
        lhs.dot(&rhs)
    };
    push(equations, numerator.div(&denominator), relation, span)
}

fn collinear_relation(
    differentiated: &DifferentiatedFile,
    args: &[Expr],
    relation: usize,
    span: Span,
    diameter: f64,
    equations: &mut Vec<Equation>,
) -> Result<(), EvalError> {
    if args.len() < 3 {
        return Err(relation_error(
            span,
            "collinear requires at least three points",
        ));
    }
    let first = expect_point(differentiated.object(&args[0])?, args[0].span())?;
    let second = expect_point(differentiated.object(&args[1])?, args[1].span())?;
    let direction = second.sub(&first);
    let norm = direction
        .norm()
        .map_err(|()| relation_error(span, "first two collinear points coincide"))?;
    for argument in &args[2..] {
        let point = expect_point(differentiated.object(argument)?, argument.span())?;
        let residual = direction
            .cross(&point.sub(&first))
            .div(&norm)
            .scale(1.0 / diameter);
        push(equations, residual, relation, span)?;
    }
    Ok(())
}

fn concyclic_relation(
    differentiated: &DifferentiatedFile,
    args: &[Expr],
    relation: usize,
    span: Span,
    diameter: f64,
    equations: &mut Vec<Equation>,
) -> Result<(), EvalError> {
    if args.len() < 4 {
        return Err(relation_error(
            span,
            "concyclic requires at least four points",
        ));
    }
    let first = expect_point(differentiated.object(&args[0])?, args[0].span())?;
    let second = expect_point(differentiated.object(&args[1])?, args[1].span())?;
    let third = expect_point(differentiated.object(&args[2])?, args[2].span())?;
    let circle = circumcircle(&first, &second, &third, span)?;
    for argument in &args[3..] {
        let point = expect_point(differentiated.object(argument)?, argument.span())?;
        push(
            equations,
            point_circle_residual(&point, &circle, span)?.scale(1.0 / diameter),
            relation,
            span,
        )?;
    }
    Ok(())
}

fn congruent_relation(
    differentiated: &DifferentiatedFile,
    args: &[Expr],
    relation: usize,
    span: Span,
    diameter: f64,
    equations: &mut Vec<Equation>,
) -> Result<(), EvalError> {
    require_arity("congruent", args, 2, span)?;
    let lhs = expect_segment(differentiated.object(&args[0])?, args[0].span())?;
    let rhs = expect_segment(differentiated.object(&args[1])?, args[1].span())?;
    let residual = lhs
        .direction()
        .norm()
        .and_then(|left| rhs.direction().norm().map(|right| left.sub(&right)))
        .map_err(|()| relation_error(span, "congruent segment is degenerate"))?;
    push(equations, residual.scale(1.0 / diameter), relation, span)
}

fn coincident_relation(
    differentiated: &DifferentiatedFile,
    args: &[Expr],
    relation: usize,
    span: Span,
    diameter: f64,
    equations: &mut Vec<Equation>,
) -> Result<(), EvalError> {
    require_arity("coincident", args, 2, span)?;
    let lhs = expect_point(differentiated.object(&args[0])?, args[0].span())?;
    let rhs = expect_point(differentiated.object(&args[1])?, args[1].span())?;
    push(
        equations,
        lhs.x.sub(&rhs.x).scale(1.0 / diameter),
        relation,
        span,
    )?;
    push(
        equations,
        lhs.y.sub(&rhs.y).scale(1.0 / diameter),
        relation,
        span,
    )
}

fn midpoint_relation(
    differentiated: &DifferentiatedFile,
    args: &[Expr],
    relation: usize,
    span: Span,
    diameter: f64,
    equations: &mut Vec<Equation>,
) -> Result<(), EvalError> {
    require_arity("midpoint", args, 3, span)?;
    let midpoint = expect_point(differentiated.object(&args[0])?, args[0].span())?;
    let first = expect_point(differentiated.object(&args[1])?, args[1].span())?;
    let second = expect_point(differentiated.object(&args[2])?, args[2].span())?;
    let half = Jet::constant(0.5, midpoint.x.gradient.len());
    let target_x = first.x.add(&second.x).mul(&half);
    let target_y = first.y.add(&second.y).mul(&half);
    push(
        equations,
        midpoint.x.sub(&target_x).scale(1.0 / diameter),
        relation,
        span,
    )?;
    push(
        equations,
        midpoint.y.sub(&target_y).scale(1.0 / diameter),
        relation,
        span,
    )
}

fn axis_relation(
    differentiated: &DifferentiatedFile,
    name: &str,
    args: &[Expr],
    relation: usize,
    span: Span,
    diameter: f64,
    equations: &mut Vec<Equation>,
) -> Result<(), EvalError> {
    require_arity(name, args, 2, span)?;
    let lhs = expect_point(differentiated.object(&args[0])?, args[0].span())?;
    let rhs = expect_point(differentiated.object(&args[1])?, args[1].span())?;
    let residual = if name == "horizontal" {
        rhs.y.sub(&lhs.y)
    } else {
        rhs.x.sub(&lhs.x)
    };
    push(equations, residual.scale(1.0 / diameter), relation, span)
}

fn pin_relation(
    differentiated: &DifferentiatedFile,
    args: &[Expr],
    relation: usize,
    span: Span,
    diameter: f64,
    equations: &mut Vec<Equation>,
) -> Result<(), EvalError> {
    require_arity("pin", args, 3, span)?;
    let point = expect_point(differentiated.object(&args[0])?, args[0].span())?;
    let x = differentiated
        .number_as(&args[1], NumericDimension::Length)?
        .0;
    let y = differentiated
        .number_as(&args[2], NumericDimension::Length)?
        .0;
    push(
        equations,
        point.x.sub(&x).scale(1.0 / diameter),
        relation,
        span,
    )?;
    push(
        equations,
        point.y.sub(&y).scale(1.0 / diameter),
        relation,
        span,
    )
}

fn point_circle_residual(point: &DPoint, circle: &DCircle, span: Span) -> Result<Jet, EvalError> {
    point
        .sub(&circle.center)
        .norm()
        .map(|distance| distance.sub(&circle.radius))
        .map_err(|()| relation_error(span, "circle center and point coincide"))
}

fn point_linear_residual(point: &DPoint, linear: &DLinear, span: Span) -> Result<Jet, EvalError> {
    let direction = linear.direction();
    let length = direction
        .norm()
        .map_err(|()| relation_error(span, "linear object is degenerate"))?;
    let parameter = point
        .sub(&linear.start)
        .dot(&direction)
        .div(&direction.dot(&direction));
    let signed = direction.cross(&point.sub(&linear.start)).div(&length);
    if linear.domain == LinearDomain::Line {
        return Ok(signed);
    }
    let carrier = signed.abs();
    let start = point
        .sub(&linear.start)
        .norm()
        .map_err(|()| relation_error(span, "distance to linear start is not finite"))?;
    let end = point
        .sub(&linear.through)
        .norm()
        .map_err(|()| relation_error(span, "distance to linear end is not finite"))?;
    let live = select_linear_piece(parameter.value, linear.domain, &carrier, &start, &end);
    let seed = select_linear_piece(parameter.seed_value, linear.domain, &carrier, &start, &end);
    Ok(seed.clone().with_live_value(live))
}

fn select_linear_piece<'a>(
    parameter: f64,
    domain: LinearDomain,
    carrier: &'a Jet,
    start: &'a Jet,
    end: &'a Jet,
) -> &'a Jet {
    match domain {
        LinearDomain::Line => carrier,
        LinearDomain::Ray => {
            if parameter >= 0.0 {
                carrier
            } else {
                start
            }
        }
        LinearDomain::Segment if parameter < 0.0 => start,
        LinearDomain::Segment if parameter <= 1.0 => carrier,
        LinearDomain::Segment => end,
    }
}

fn point_arc_residual(point: &DPoint, arc: &DArc, span: Span) -> Result<Jet, EvalError> {
    let radial = point_circle_residual(point, &arc.circle, span)?.abs();
    let start = point
        .sub(&arc.start)
        .norm()
        .map_err(|()| relation_error(span, "distance to arc start is not finite"))?;
    let end = point
        .sub(&arc.end)
        .norm()
        .map_err(|()| relation_error(span, "distance to arc end is not finite"))?;
    let endpoint = start.min(&end);
    let live = if point_angle_in_arc(point, arc, false) {
        &radial
    } else {
        &endpoint
    };
    let seed = if point_angle_in_arc(point, arc, true) {
        radial.clone()
    } else {
        endpoint.clone()
    };
    Ok(seed.with_live_value(live))
}

fn point_angle_in_arc(point: &DPoint, arc: &DArc, seed: bool) -> bool {
    let angle = angle_value(point, &arc.circle.center, seed);
    let start = angle_value(&arc.start, &arc.circle.center, seed);
    let end = angle_value(&arc.end, &arc.circle.center, seed);
    let tau = 2.0 * core::f64::consts::PI;
    let ccw_offset = (angle - start).rem_euclid(tau);
    let ccw_sweep = (end - start).rem_euclid(tau);
    if if seed { arc.seed_ccw } else { arc.ccw } {
        ccw_offset <= ccw_sweep
    } else {
        (start - angle).rem_euclid(tau) <= (start - end).rem_euclid(tau)
    }
}

fn angle_value(point: &DPoint, center: &DPoint, seed: bool) -> f64 {
    let component = |value: &Jet| {
        if seed { value.seed_value } else { value.value }
    };
    libm::atan2(
        component(&point.y) - component(&center.y),
        component(&point.x) - component(&center.x),
    )
}

fn circumcircle(
    first: &DPoint,
    second: &DPoint,
    third: &DPoint,
    span: Span,
) -> Result<DCircle, EvalError> {
    let center = super::ad::circumcenter(first, second, third, span)?;
    let radius = first
        .sub(&center)
        .norm()
        .map_err(|()| relation_error(span, "concyclic seed is degenerate"))?;
    Ok(DCircle { center, radius })
}

fn numeric_scale(lhs: &Jet, rhs: &Jet, dimension: NumericDimension, diameter: f64) -> Jet {
    let count = lhs.gradient.len();
    match dimension {
        NumericDimension::Scalar => Jet::constant(1.0, count).max(&lhs.abs()).max(&rhs.abs()),
        NumericDimension::Length => Jet::constant(diameter, count)
            .max(&lhs.abs())
            .max(&rhs.abs()),
        NumericDimension::Area => Jet::constant(diameter * diameter, count)
            .max(&lhs.abs())
            .max(&rhs.abs()),
        NumericDimension::Angle => Jet::constant(1.0, count),
    }
}

fn expect_point(object: DObject, span: Span) -> Result<DPoint, EvalError> {
    match object {
        DObject::Point(point) => Ok(point),
        _ => Err(relation_error(span, "expected a point")),
    }
}

fn expect_linear(object: DObject, span: Span) -> Result<DLinear, EvalError> {
    match object {
        DObject::Linear(linear) => Ok(linear),
        _ => Err(relation_error(span, "expected a line, ray, or segment")),
    }
}

fn expect_segment(object: DObject, span: Span) -> Result<DLinear, EvalError> {
    match object {
        DObject::Linear(linear) if linear.domain == LinearDomain::Segment => Ok(linear),
        _ => Err(relation_error(span, "expected a segment")),
    }
}

fn push(
    equations: &mut Vec<Equation>,
    residual: Jet,
    relation: usize,
    span: Span,
) -> Result<(), EvalError> {
    if residual.value.is_finite() && residual.gradient.iter().all(|value| value.is_finite()) {
        equations.push(Equation { residual, relation });
        Ok(())
    } else {
        Err(relation_error(span, "relation residual is not finite"))
    }
}

fn require_arity(name: &str, args: &[Expr], expected: usize, span: Span) -> Result<(), EvalError> {
    if args.len() == expected {
        Ok(())
    } else {
        Err(relation_error(
            span,
            format!("`{name}` expects {expected} arguments"),
        ))
    }
}

fn relation_error(span: Span, message: impl Into<String>) -> EvalError {
    EvalError::new("E404", span, message)
}

fn assert_only_relation_error(span: Span) -> EvalError {
    EvalError::new(
        "E146",
        span,
        "this assert-only boolean form cannot be used in `rel` or `where`",
    )
}
