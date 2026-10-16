//! Internal continuous route parameters and frozen discrete choices.

use crate::scene::{PathGeometry, ResolvedPath};
use locus_syntax::{Kind, OpStmt, PropValue};

use super::{EvalError, FileEvaluator, UnknownCursor, include_bbox};
use crate::geom::route::{Route, RouteChoice, RouteKind, RouteSide};
use crate::{AnchorSpec, ResolvedObject};

pub(crate) fn route_kind(op: &OpStmt) -> Option<RouteKind> {
    if op.kind != Kind::Arrow {
        return None;
    }
    match token(op, "route") {
        Some("orthogonal") => Some(RouteKind::Orthogonal),
        Some("curved") => Some(RouteKind::Curved),
        _ => None,
    }
}

fn token<'a>(op: &'a OpStmt, name: &str) -> Option<&'a str> {
    let value = super::prop(&op.props, name)?;
    match value {
        PropValue::Expr(locus_syntax::Expr::Ref(id)) => Some(&id.name),
        _ => None,
    }
}

pub(crate) const fn default_parameter(kind: RouteKind) -> f64 {
    match kind {
        RouteKind::Orthogonal => crate::preset::ROUTE_DETOUR,
        RouteKind::Curved => crate::preset::ROUTE_CONTROL_OFFSET,
    }
}

pub(crate) fn route_parameter(object: &ResolvedObject) -> Option<f64> {
    match object {
        ResolvedObject::Path(ResolvedPath {
            geometry: PathGeometry::Routed {
                route: Some(route), ..
            },
            ..
        }) => route.parameter,
        _ => None,
    }
}

pub(crate) fn route_candidates(op: &OpStmt) -> Vec<RouteChoice> {
    let sides = match token(op, "side") {
        Some("right") => vec![RouteSide::Right],
        Some("left") => vec![RouteSide::Left],
        Some("above") => vec![RouteSide::Above],
        Some("below") => vec![RouteSide::Below],
        _ => crate::preset::ROUTE_PAGE_SIDE_ORDER.to_vec(),
    };
    sides
        .into_iter()
        .flat_map(|side| {
            let mut choices = vec![RouteChoice {
                side,
                five_span: false,
            }];
            if route_kind(op) == Some(RouteKind::Orthogonal) {
                choices.push(RouteChoice {
                    side,
                    five_span: true,
                });
            }
            choices
        })
        .collect()
}

pub(crate) fn accepted_choice(
    op: &OpStmt,
    choices: &std::collections::BTreeMap<String, RouteChoice>,
) -> RouteChoice {
    let candidates = route_candidates(op);
    choices
        .get(&op.id.name)
        .copied()
        .filter(|choice| candidates.contains(choice))
        .unwrap_or(candidates[0])
}

impl FileEvaluator {
    pub(super) fn evaluate_routed_op(
        &mut self,
        op: &OpStmt,
        unknowns: &mut UnknownCursor<'_>,
    ) -> Result<(), EvalError> {
        let kind = route_kind(op)
            .ok_or_else(|| EvalError::new("E401", op.span, "invalid routed arrow"))?;
        let minimum = self
            .inputs
            .choices
            .route_min_offsets
            .get(&op.id.name)
            .copied()
            .unwrap_or(0.0);
        let offset = unknowns
            .next(op.span)?
            .unwrap_or(default_parameter(kind).max(minimum));
        let tolerance = super::tolerance_for(self.bbox, &self.length_fallbacks);
        let mut expressions =
            super::ExpressionEvaluator::new(&self.env, tolerance, &self.dimensions)
                .with_metrics(self.inputs.metrics);
        let mut object = super::evaluate_op_value(&mut expressions, op)?;
        let ResolvedObject::Path(ResolvedPath {
            geometry:
                PathGeometry::Routed {
                    from: AnchorSpec::Point(from),
                    to: AnchorSpec::Point(to),
                    route,
                    ..
                },
            ..
        }) = &mut object
        else {
            return Err(EvalError::new(
                "E410",
                op.span,
                "routed endpoints require measured geometry anchors",
            ));
        };
        let choice = accepted_choice(op, &self.route_choices);
        *route = Some(Route::automatic(
            (*from).into(),
            (*to).into(),
            kind,
            choice,
            offset,
        ));
        if let Some(bounds) = object.bbox() {
            include_bbox(&mut self.bbox, bounds);
        }
        self.env
            .insert(op.id.clone(), object)
            .map_err(|error| EvalError::new("E302", op.id.span, error.to_string()))
    }
}
