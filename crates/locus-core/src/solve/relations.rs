//! Source-ordered relation expansion and normalized residual semantics.

use locus_syntax::{BinOp, Expr, File, Ident, Span, Statement};

use crate::eval::EvalError;

use super::ad::{
    DArc, DCircle, DEllipse, DLinear, DObject, DPoint, DifferentiatedFile, Jet, LinearDomain,
    NumericDimension,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CircleBranch {
    External,
    InternalFirstOuter,
}

#[derive(Clone, Debug)]
pub(crate) struct SourceRelation<'a> {
    pub(crate) expression: &'a Expr,
    pub(crate) span: Span,
    pub(crate) padding_none: bool,
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
        Statement::Rel(relation) => collect_relation(relation, result),
        Statement::Assert(_)
        | Statement::Meta(_)
        | Statement::Style(_)
        | Statement::Class(_)
        | Statement::Fn(_)
        | Statement::Group(_)
        | Statement::Call(_) => {}
    }
}

fn collect_relation<'a>(relation: &'a locus_syntax::RelStmt, result: &mut Vec<SourceRelation<'a>>) {
    let start = result.len();
    flatten(&relation.expr, result);
    for source in &mut result[start..] {
        source.padding_none = !relation.props.items.is_empty();
    }
}

fn flatten<'a>(expression: &'a Expr, result: &mut Vec<SourceRelation<'a>>) {
    if let Expr::Group { expr, .. } = expression {
        flatten(expr, result);
        return;
    }
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
            padding_none: false,
        });
    }
}

pub(crate) fn select_circle_branches(
    relations: &[SourceRelation<'_>],
) -> Vec<Option<CircleBranch>> {
    relations
        .iter()
        .map(|relation| select_circle_branch(relation.expression))
        .collect()
}

fn select_circle_branch(expression: &Expr) -> Option<CircleBranch> {
    let Expr::Call { name, args, .. } = expression else {
        return None;
    };
    if args.len() != 2 || !matches!(name.name.as_str(), "tangent" | "internally_tangent") {
        return None;
    }
    if name.name == "internally_tangent" {
        return Some(CircleBranch::InternalFirstOuter);
    }
    Some(CircleBranch::External)
}

pub(crate) fn evaluate(
    differentiated: &DifferentiatedFile,
    relations: &[SourceRelation<'_>],
    branches: &[Option<CircleBranch>],
    diameter: f64,
) -> Result<Vec<Equation>, EvalError> {
    let mut equations = Vec::new();
    for (index, relation) in relations.iter().enumerate() {
        if super::layout::is_layout(relation.expression) {
            continue;
        }
        if let Expr::Call { name, args, span } = relation.expression
            && is_inset(&name.name)
        {
            inset_relation(
                differentiated,
                &name.name,
                args,
                relation.padding_none,
                index,
                *span,
                diameter,
                &mut equations,
            )?;
            continue;
        }
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
    if matches!(
        name,
        "on" | "tangent" | "internally_tangent" | "parallel" | "perpendicular"
    ) {
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
        "tangent" | "internally_tangent" => tangent_relation(
            differentiated,
            name,
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
        "center" => center_relation(differentiated, args, relation, span, diameter, equations),
        "align_center_x" | "align_center_y" | "align_left" | "align_right" | "align_top"
        | "align_bottom" => alignment_relation(
            differentiated,
            name,
            args,
            relation,
            span,
            diameter,
            equations,
        ),
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
        DObject::Ellipse(ellipse) => point_ellipse_residual(&point, &ellipse, span)?,
        DObject::RoundedRect(rect) => rect.signed_distance(&point, span)?,
        _ => {
            return Err(relation_error(
                args[1].span(),
                "on target must be linear, circle, arc, ellipse, or rounded rectangle",
            ));
        }
    };
    push(equations, residual.scale(1.0 / diameter), relation, span)
}

#[allow(
    clippy::too_many_arguments,
    reason = "source-ordered residual metadata and sink remain explicit"
)]
fn tangent_relation(
    differentiated: &DifferentiatedFile,
    name: &str,
    args: &[Expr],
    relation: usize,
    branch: Option<CircleBranch>,
    span: Span,
    diameter: f64,
    equations: &mut Vec<Equation>,
) -> Result<(), EvalError> {
    require_arity(name, args, 2, span)?;
    let lhs = differentiated.object(&args[0])?;
    let rhs = differentiated.object(&args[1])?;
    let residual = match (lhs, rhs) {
        (DObject::Circle(lhs), DObject::Circle(rhs)) => {
            circle_tangent_residual(&lhs, &rhs, branch, span)?
        }
        (DObject::Circle(circle), DObject::Linear(linear))
        | (DObject::Linear(linear), DObject::Circle(circle)) => {
            if name == "internally_tangent" {
                return Err(relation_error(
                    span,
                    "internally_tangent requires two circles",
                ));
            }
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

fn point_ellipse_residual(
    point: &DPoint,
    ellipse: &DEllipse,
    span: Span,
) -> Result<Jet, EvalError> {
    let offset = point.sub(&ellipse.center);
    let cosine = ellipse.rotation.cos();
    let sine = ellipse.rotation.sin();
    let local_x = offset.x.mul(&cosine).add(&offset.y.mul(&sine));
    let local_y = offset.y.mul(&cosine).sub(&offset.x.mul(&sine));
    let normalized_x = local_x.div(&ellipse.semimajor);
    let normalized_y = local_y.div(&ellipse.semiminor);
    let radius = normalized_x
        .mul(&normalized_x)
        .add(&normalized_y.mul(&normalized_y))
        .sqrt()
        .map_err(|()| relation_error(span, "ellipse relation is degenerate"))?;
    Ok(radius
        .sub(&Jet::constant(1.0, radius.gradient.len()))
        .mul(&ellipse.semiminor))
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

fn center_relation(
    differentiated: &DifferentiatedFile,
    args: &[Expr],
    relation: usize,
    span: Span,
    diameter: f64,
    equations: &mut Vec<Equation>,
) -> Result<(), EvalError> {
    require_arity("center", args, 2, span)?;
    let lhs = measured_center(differentiated, &args[0])?;
    let rhs = measured_center(differentiated, &args[1])?;
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

#[allow(
    clippy::too_many_arguments,
    reason = "source-ordered residual sink remains explicit"
)]
fn inset_relation(
    differentiated: &DifferentiatedFile,
    name: &str,
    args: &[Expr],
    padding_none: bool,
    relation: usize,
    span: Span,
    diameter: f64,
    equations: &mut Vec<Equation>,
) -> Result<(), EvalError> {
    require_arity(name, args, 2, span)?;
    let alignment = inset_alignment(name);
    let content = alignment_component(differentiated, &args[0], alignment)?;
    let container = alignment_component(differentiated, &args[1], alignment)?;
    let padding = if padding_none {
        0.0
    } else {
        differentiated.metrics.padding
    };
    let direction = if matches!(name, "inset_left" | "inset_bottom") {
        1.0
    } else {
        -1.0
    };
    let residual = content
        .sub(&container)
        .sub(&Jet::constant(direction * padding, content.gradient.len()));
    push(equations, residual.scale(1.0 / diameter), relation, span)
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

fn alignment_relation(
    differentiated: &DifferentiatedFile,
    name: &str,
    args: &[Expr],
    relation: usize,
    span: Span,
    diameter: f64,
    equations: &mut Vec<Equation>,
) -> Result<(), EvalError> {
    if args.len() < 2 {
        return Err(relation_error(
            span,
            format!("`{name}` requires at least two objects"),
        ));
    }
    let reference = alignment_component(differentiated, &args[0], name)?;
    for argument in &args[1..] {
        let component = alignment_component(differentiated, argument, name)?;
        push(
            equations,
            component.sub(&reference).scale(1.0 / diameter),
            relation,
            span,
        )?;
    }
    Ok(())
}

fn alignment_component(
    differentiated: &DifferentiatedFile,
    expression: &Expr,
    name: &str,
) -> Result<Jet, EvalError> {
    if matches!(name, "align_center_x" | "align_center_y") {
        let center = measured_center(differentiated, expression)?;
        return Ok(if name == "align_center_x" {
            center.x
        } else {
            center.y
        });
    }
    let bounds = measured_bounds(differentiated, expression)?;
    Ok(match name {
        "align_left" => bounds.minimum.x,
        "align_right" => bounds.maximum.x,
        "align_bottom" => bounds.minimum.y,
        _ => bounds.maximum.y,
    })
}

#[derive(Clone)]
pub(super) struct MeasuredBounds {
    pub(super) minimum: DPoint,
    pub(super) maximum: DPoint,
}

impl MeasuredBounds {
    fn from_point(point: DPoint) -> Self {
        Self {
            minimum: point.clone(),
            maximum: point,
        }
    }

    fn include(&mut self, point: &DPoint) {
        self.minimum.x = live_min(&self.minimum.x, &point.x);
        self.minimum.y = live_min(&self.minimum.y, &point.y);
        self.maximum.x = live_max(&self.maximum.x, &point.x);
        self.maximum.y = live_max(&self.maximum.y, &point.y);
    }

    pub(super) fn center(self) -> DPoint {
        DPoint {
            x: self.minimum.x.add(&self.maximum.x).scale(0.5),
            y: self.minimum.y.add(&self.maximum.y).scale(0.5),
        }
    }
}

fn measured_center(
    differentiated: &DifferentiatedFile,
    expression: &Expr,
) -> Result<DPoint, EvalError> {
    if collection_members(differentiated, expression).is_some() {
        return measured_bounds(differentiated, expression).map(MeasuredBounds::center);
    }
    match differentiated.object(expression)? {
        DObject::Point(point) => Ok(point),
        DObject::Circle(circle) => Ok(circle.center),
        DObject::Ellipse(ellipse) => Ok(ellipse.center),
        DObject::RoundedRect(_) => measured_anchor(differentiated, expression, "center"),
        _ => measured_bounds(differentiated, expression).map(MeasuredBounds::center),
    }
}

pub(super) fn measured_bounds(
    differentiated: &DifferentiatedFile,
    expression: &Expr,
) -> Result<MeasuredBounds, EvalError> {
    if let Some(members) = collection_members(differentiated, expression) {
        return collection_bounds(differentiated, members, expression.span());
    }
    let object = differentiated.object(expression)?;
    if let Some(route) = differentiated.layout_route(expression, &object) {
        let (minimum, maximum) = differentiated.route_bounds(expression, &route);
        return Ok(MeasuredBounds { minimum, maximum });
    }
    match object {
        DObject::Point(point) => Ok(MeasuredBounds::from_point(point)),
        DObject::Linear(linear) => {
            bounds_of_points(&[linear.start, linear.through], expression.span())
        }
        DObject::Polygon(vertices, _, _) => bounds_of_points(&vertices, expression.span()),
        DObject::Bspline(spline) => bounds_of_points(&spline.controls, expression.span()),
        DObject::Route(route) => {
            let (minimum, maximum) = differentiated.route_bounds(expression, &route);
            Ok(MeasuredBounds { minimum, maximum })
        }
        object => curved_bounds(differentiated, expression, object),
    }
}

/// Stable member/route/caption identities, independent of the current extreme.
/// Aggregate measurements remain available through `measured_bounds`.
pub(super) fn measured_contributors(
    differentiated: &DifferentiatedFile,
    arguments: &[Expr],
) -> Result<Vec<MeasuredBounds>, EvalError> {
    let members = crate::program::avoidance_operands(
        &differentiated.collections,
        &differentiated.aliases,
        arguments,
    );
    let mut seen = std::collections::BTreeSet::new();
    let mut result = Vec::new();
    for member in members {
        if let Some(route) = differentiated.layout_route(&member, &differentiated.object(&member)?)
        {
            append_route_contributors(differentiated, &member, &route, &mut seen, &mut result);
        } else if contributor_identity(&member).is_none_or(|id| seen.insert(id.to_owned())) {
            result.extend(primitive_contributors(differentiated, &member)?);
        }
    }
    Ok(result)
}

fn primitive_contributors(
    d: &DifferentiatedFile,
    member: &Expr,
) -> Result<Vec<MeasuredBounds>, EvalError> {
    let points = match d.object(member)? {
        DObject::Linear(linear) => vec![linear.start, linear.through],
        DObject::Polygon(vertices, false, _) => vertices,
        DObject::Bspline(spline) => spline.controls,
        _ => return Ok(vec![measured_bounds(d, member)?]),
    };
    Ok(points.into_iter().map(MeasuredBounds::from_point).collect())
}

fn contributor_identity(expression: &Expr) -> Option<&str> {
    match expression {
        Expr::Ref(id) => Some(&id.name),
        Expr::Group { expr, .. } => contributor_identity(expr),
        _ => None,
    }
}

fn append_route_contributors(
    d: &DifferentiatedFile,
    member: &Expr,
    route: &crate::geom::route::Route<Jet>,
    seen: &mut std::collections::BTreeSet<String>,
    result: &mut Vec<MeasuredBounds>,
) {
    for (identity, minimum, maximum) in d.route_bound_contributors(member, route) {
        let identity = if identity.is_empty() {
            contributor_identity(member).map(str::to_owned)
        } else {
            Some(identity)
        };
        if identity.is_none_or(|id| seen.insert(id)) {
            result.push(MeasuredBounds { minimum, maximum });
        }
    }
}

pub(super) fn collection_members<'a>(
    differentiated: &'a DifferentiatedFile,
    expression: &Expr,
) -> Option<&'a [String]> {
    match expression {
        Expr::Ref(id) => differentiated
            .collections
            .get(&id.name)
            .map(|collection| collection.members.as_slice()),
        Expr::Group { expr, .. } => collection_members(differentiated, expr),
        _ => None,
    }
}

fn collection_bounds(
    differentiated: &DifferentiatedFile,
    members: &[String],
    span: Span,
) -> Result<MeasuredBounds, EvalError> {
    let mut result: Option<MeasuredBounds> = None;
    for member in members {
        let expression = Expr::Ref(Ident {
            name: member.clone(),
            span,
        });
        let bounds = measured_bounds(differentiated, &expression)?;
        if let Some(current) = &mut result {
            current.include(&bounds.minimum);
            current.include(&bounds.maximum);
        } else {
            result = Some(bounds);
        }
    }
    result.ok_or_else(|| relation_error(span, "collection has no measured members"))
}

fn curved_bounds(
    differentiated: &DifferentiatedFile,
    expression: &Expr,
    object: DObject,
) -> Result<MeasuredBounds, EvalError> {
    match object {
        DObject::Circle(circle) => Ok(bounds_with_extents(
            &circle.center,
            &circle.radius,
            &circle.radius,
        )),
        DObject::Ellipse(ellipse) => ellipse_bounds(&ellipse, expression.span()),
        DObject::Arc(arc) => Ok(arc_bounds(&arc)),
        DObject::RoundedRect(_) => Ok(MeasuredBounds {
            minimum: measured_anchor(differentiated, expression, "sw")?,
            maximum: measured_anchor(differentiated, expression, "ne")?,
        }),
        _ => Err(relation_error(
            expression.span(),
            "expected an object with measured bounds",
        )),
    }
}

fn measured_anchor(
    differentiated: &DifferentiatedFile,
    expression: &Expr,
    name: &str,
) -> Result<DPoint, EvalError> {
    let span = expression.span();
    let property = Expr::Property {
        base: Box::new(expression.clone()),
        name: Ident {
            name: name.to_owned(),
            span,
        },
        args: Vec::new(),
        span,
    };
    expect_point(differentiated.object(&property)?, span)
}

fn bounds_of_points(points: &[DPoint], span: Span) -> Result<MeasuredBounds, EvalError> {
    let first = points
        .first()
        .ok_or_else(|| relation_error(span, "object has no measured points"))?;
    let mut bounds = MeasuredBounds::from_point(first.clone());
    for point in &points[1..] {
        bounds.include(point);
    }
    Ok(bounds)
}

fn bounds_with_extents(center: &DPoint, horizontal: &Jet, vertical: &Jet) -> MeasuredBounds {
    MeasuredBounds {
        minimum: DPoint {
            x: center.x.sub(horizontal),
            y: center.y.sub(vertical),
        },
        maximum: DPoint {
            x: center.x.add(horizontal),
            y: center.y.add(vertical),
        },
    }
}

fn ellipse_bounds(ellipse: &DEllipse, span: Span) -> Result<MeasuredBounds, EvalError> {
    let cosine = ellipse.rotation.cos();
    let sine = ellipse.rotation.sin();
    let horizontal = squared_sum_root(
        &ellipse.semimajor.mul(&cosine),
        &ellipse.semiminor.mul(&sine),
        span,
    )?;
    let vertical = squared_sum_root(
        &ellipse.semimajor.mul(&sine),
        &ellipse.semiminor.mul(&cosine),
        span,
    )?;
    Ok(bounds_with_extents(&ellipse.center, &horizontal, &vertical))
}

fn squared_sum_root(first: &Jet, second: &Jet, span: Span) -> Result<Jet, EvalError> {
    first
        .mul(first)
        .add(&second.mul(second))
        .sqrt()
        .map_err(|()| relation_error(span, "ellipse extent is undefined"))
}

fn arc_bounds(arc: &DArc) -> MeasuredBounds {
    let mut bounds = MeasuredBounds::from_point(arc.start.clone());
    bounds.include(&arc.end);
    for angle in [
        0.0,
        core::f64::consts::FRAC_PI_2,
        core::f64::consts::PI,
        3.0 * core::f64::consts::FRAC_PI_2,
    ] {
        let point = DPoint {
            x: arc
                .circle
                .center
                .x
                .add(&arc.circle.radius.scale(libm::cos(angle))),
            y: arc
                .circle
                .center
                .y
                .add(&arc.circle.radius.scale(libm::sin(angle))),
        };
        if point_angle_in_arc(&point, arc, false) {
            bounds.include(&point);
        }
    }
    bounds
}

fn live_min(lhs: &Jet, rhs: &Jet) -> Jet {
    if lhs.value <= rhs.value {
        lhs.clone()
    } else {
        rhs.clone()
    }
}

fn live_max(lhs: &Jet, rhs: &Jet) -> Jet {
    if lhs.value >= rhs.value {
        lhs.clone()
    } else {
        rhs.clone()
    }
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
    let carrier = if linear.layout_live {
        if signed.value < 0.0 {
            signed.scale(-1.0)
        } else {
            signed
        }
    } else {
        signed.abs()
    };
    let start = point
        .sub(&linear.start)
        .norm()
        .map_err(|()| relation_error(span, "distance to linear start is not finite"))?;
    let end = point
        .sub(&linear.through)
        .norm()
        .map_err(|()| relation_error(span, "distance to linear end is not finite"))?;
    let live = select_linear_piece(parameter.value, linear.domain, &carrier, &start, &end);
    if linear.layout_live {
        return Ok(live.clone());
    }
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

#[cfg(test)]
#[allow(clippy::expect_used, clippy::float_cmp, clippy::panic)]
mod tests {
    use super::*;
    use crate::solve::{Solved, solve};

    fn solve_source(source: &str) -> Solved {
        let parsed = locus_syntax::parse(source);
        assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
        solve(&parsed.file).expect("alignment system solves")
    }

    #[test]
    fn measured_equalities_solve_shape_bounds_without_matching_dimensions() {
        let source = "locus 0 beta 6\n\
            rect reference(4, 2)\nrect left(2, 4)\nrounded_rect right(2, 4, 0.25)\n\
            circle c(1)\npoint P()\npoint Q()\n\
            rel align_left(reference, left)\nrel align_bottom(reference, left)\n\
            rel align_right(reference, right)\nrel align_top(reference, right)\n\
            rel center(c, reference)\nrel align_center_x(reference, c, P, Q)\n\
            rel align_center_y(reference, c, P, Q)\n\
            assert align_left(reference, left)\nassert align_bottom(reference, left)\n\
            assert align_right(reference, right)\nassert align_top(reference, right)\n\
            assert center(c, reference)\nassert align_center_x(reference, c, P, Q)\n\
            assert align_center_y(reference, c, P, Q)\n\
            assert reference.width == 4\nassert left.width == 2\n";
        let solved = solve_source(source);
        let parsed = locus_syntax::parse(source);
        assert!(
            crate::assert::evaluate_assertions(&parsed.file, &solved.scene)
                .expect("solved assertions")
                .passed
        );
        assert_eq!(solved.summary.rank, 10);
    }

    #[test]
    fn measured_bound_derivatives_follow_the_current_extreme() {
        let first = DPoint {
            x: Jet {
                value: 2.0,
                seed_value: -2.0,
                gradient: vec![1.0],
            },
            y: Jet::constant(0.0, 1),
        };
        let second = DPoint {
            x: Jet {
                value: -1.0,
                seed_value: 1.0,
                gradient: vec![0.0],
            },
            y: Jet::constant(0.0, 1),
        };
        let bounds = bounds_of_points(&[first, second], Span::default()).expect("segment bounds");
        assert_eq!(bounds.minimum.x.value, -1.0);
        assert_eq!(bounds.minimum.x.gradient, vec![0.0]);
        assert_eq!(bounds.maximum.x.gradient, vec![1.0]);
    }

    #[test]
    fn finite_layout_incidence_switches_endpoint_and_interior_gradients_live() {
        let mut linear = DLinear {
            start: DPoint {
                x: Jet::constant(0.0, 1),
                y: Jet::constant(0.0, 1),
            },
            through: DPoint {
                x: Jet::constant(1.0, 1),
                y: Jet::constant(0.0, 1),
            },
            domain: LinearDomain::Segment,
            layout_live: true,
        };
        let point = DPoint {
            x: Jet {
                value: 3.0,
                seed_value: 0.5,
                gradient: vec![1.0],
            },
            y: Jet::constant(1.0, 1),
        };
        let live = point_linear_residual(&point, &linear, Span::default()).expect("live endpoint");
        assert!((live.gradient[0] - 2.0 / libm::sqrt(5.0)).abs() < 1e-12);
        linear.layout_live = false;
        let geometric =
            point_linear_residual(&point, &linear, Span::default()).expect("frozen geometry");
        assert_eq!(geometric.value, live.value);
        assert_eq!(geometric.gradient, vec![0.0]);
        linear.layout_live = true;
        let point = DPoint {
            x: Jet {
                value: 0.5,
                seed_value: 3.0,
                gradient: vec![1.0],
            },
            y: Jet::constant(1.0, 1),
        };
        let interior =
            point_linear_residual(&point, &linear, Span::default()).expect("live interior");
        assert_eq!(interior.gradient, vec![0.0]);
        assert_eq!(interior.value, 1.0);
    }
}
