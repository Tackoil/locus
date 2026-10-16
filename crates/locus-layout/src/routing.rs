//! Deterministic bounded side/topology/detour enumeration for routed subjects.

use crate::LayoutError;
use crate::discrete::{LayoutFeedback, Requirement, conflict, members, object, route};
use indexmap::IndexMap;
use locus_core::Scene;
use locus_core::geom::route::{Route, RouteChoice, RouteKind, RouteSide};
use locus_core::geom::sdf::attached_route_separation;
use locus_core::{PathGeometry, ResolvedPath};
use locus_syntax::{Expr, OpStmt, PropValue, Statement};

pub(crate) fn search(
    scene: &Scene,
    requirements: &[Requirement<'_>],
    feedback: &mut LayoutFeedback,
) -> Result<(), LayoutError> {
    let mut routes: IndexMap<String, (Vec<String>, locus_syntax::Span)> = IndexMap::new();
    for requirement in requirements.iter().filter(|r| r.name == "avoid") {
        collect_routes(scene, requirement, &mut routes);
    }
    let mut attempts = feedback.route_search_attempts;
    for (name, (obstacles, span)) in routes {
        select(scene, &name, &obstacles, span, feedback, &mut attempts)?;
    }
    feedback.route_search_attempts = attempts;
    Ok(())
}

pub(crate) fn freeze_accepted(scene: &Scene, feedback: &mut LayoutFeedback) {
    feedback.route_choices.extend(accepted_choices(scene));
}

pub(crate) fn accepted_choices(scene: &Scene) -> std::collections::BTreeMap<String, RouteChoice> {
    let mut result = std::collections::BTreeMap::new();
    for statement in &scene.file.statements {
        let Statement::Op(op) = statement else {
            continue;
        };
        let Some((_, kind)) = automatic_op(scene, &op.id.name) else {
            continue;
        };
        let Some(logical) = object(scene, &op.id.name).and_then(route) else {
            continue;
        };
        let Some(offset) = logical.parameter else {
            continue;
        };
        let accepted = candidates(op, kind).into_iter().find(|choice| {
            Route::automatic(
                logical.from.clone(),
                logical.to.clone(),
                kind,
                *choice,
                offset,
            ) == logical
        });
        if let Some(choice) = accepted {
            result.insert(op.id.name.clone(), choice);
        }
    }
    result
}

fn collect_routes(
    scene: &Scene,
    requirement: &Requirement<'_>,
    routes: &mut IndexMap<String, (Vec<String>, locus_syntax::Span)>,
) {
    let Some(subject) = requirement.args.first() else {
        return;
    };
    let obstacles: Vec<_> = requirement.args[1..]
        .iter()
        .flat_map(|expr| members(scene, expr))
        .collect();
    for name in members(scene, subject) {
        if matches!(
            object(scene, &name),
            Some(locus_core::ResolvedObject::Path(ResolvedPath {
                geometry: PathGeometry::Routed { .. },
                ..
            }))
        ) {
            let entry = routes.entry(name).or_insert((Vec::new(), requirement.span));
            for obstacle in &obstacles {
                if !entry.0.contains(obstacle) {
                    entry.0.push(obstacle.clone());
                }
            }
        }
    }
}

fn select(
    scene: &Scene,
    name: &str,
    obstacles: &[String],
    span: locus_syntax::Span,
    feedback: &mut LayoutFeedback,
    attempts: &mut usize,
) -> Result<(), LayoutError> {
    let Some(logical) = planned_route(scene, name, feedback) else {
        return Ok(());
    };
    if clear(scene, name, &logical, obstacles, feedback) {
        return Ok(());
    }
    let Some((op, kind)) = automatic_op(scene, name) else {
        return Err(conflict(
            span,
            &[name.to_owned()],
            "direct route conflicts with avoidance obstacles",
        ));
    };
    let offsets = route_offsets(
        logical.parameter.unwrap_or(route_base(kind)),
        route_base(kind),
    );
    let choices = candidates(op, kind);
    for offset in offsets {
        for choice in &choices {
            let candidate = Route::automatic(
                logical.from.clone(),
                logical.to.clone(),
                kind,
                *choice,
                offset,
            );
            if candidate == logical {
                continue;
            }
            if *attempts >= feedback.route_search_cap {
                return Err(conflict(
                    span,
                    &[name.to_owned()],
                    "route search exhausted its deterministic candidate cap",
                ));
            }
            *attempts += 1;
            if clear(scene, name, &candidate, obstacles, feedback) {
                feedback.route_choices.insert(name.to_owned(), *choice);
                feedback.route_min_offsets.insert(name.to_owned(), offset);
                return Ok(());
            }
        }
    }
    Err(conflict(
        span,
        &[name.to_owned()],
        "route search exhausted its deterministic candidate cap",
    ))
}

fn automatic_op<'a>(scene: &'a Scene, name: &str) -> Option<(&'a OpStmt, RouteKind)> {
    scene.file.statements.iter().find_map(|statement| {
        let Statement::Op(op) = statement else {
            return None;
        };
        if op.id.name != name || op.kind != locus_syntax::Kind::Arrow {
            return None;
        }
        let kind = match token(op, "route")? {
            "orthogonal" => RouteKind::Orthogonal,
            "curved" => RouteKind::Curved,
            _ => return None,
        };
        Some((op, kind))
    })
}

pub(crate) fn token<'a>(op: &'a OpStmt, name: &str) -> Option<&'a str> {
    op.props
        .items
        .iter()
        .find(|p| p.name.name == name)
        .and_then(|p| match &p.value {
            PropValue::Expr(Expr::Ref(id)) => Some(id.name.as_str()),
            _ => None,
        })
}

pub(crate) fn authored_side(op: &OpStmt) -> Option<RouteSide> {
    match token(op, "side") {
        Some("right") => Some(RouteSide::Right),
        Some("left") => Some(RouteSide::Left),
        Some("above") => Some(RouteSide::Above),
        Some("below") => Some(RouteSide::Below),
        _ => None,
    }
}

fn candidates(op: &OpStmt, kind: RouteKind) -> Vec<RouteChoice> {
    let sides = authored_side(op).map_or_else(
        || locus_core::preset::ROUTE_PAGE_SIDE_ORDER.to_vec(),
        |side| vec![side],
    );
    sides
        .into_iter()
        .flat_map(|side| {
            let mut result = vec![RouteChoice {
                side,
                five_span: false,
            }];
            if kind == RouteKind::Orthogonal {
                result.push(RouteChoice {
                    side,
                    five_span: true,
                });
            }
            result
        })
        .collect()
}

fn clear(
    scene: &Scene,
    name: &str,
    logical: &Route,
    obstacles: &[String],
    feedback: &LayoutFeedback,
) -> bool {
    let visible = retain_visible(logical, name, feedback);
    obstacles
        .iter()
        .filter(|other| other.as_str() != name)
        .all(|other| {
            let Some(obstacle) = visible_obstacle(scene, other, feedback) else {
                return false;
            };
            attached_route_separation(
                &visible,
                &obstacle,
                scene.inputs.metrics.stroke_width / 2.0,
                scene.inputs.metrics.clearance,
            )
            .is_some_and(|distance| distance + 1e-12 >= scene.inputs.metrics.clearance)
        })
}

pub(crate) fn planned_route(scene: &Scene, name: &str, feedback: &LayoutFeedback) -> Option<Route> {
    let mut logical = object(scene, name).and_then(route)?;
    if let Some((_, kind)) = automatic_op(scene, name) {
        if let Some(choice) = feedback.route_choices.get(name) {
            let offset = logical
                .parameter
                .unwrap_or(locus_core::preset::ROUTE_DETOUR)
                .max(feedback.route_min_offsets.get(name).copied().unwrap_or(0.0));
            logical = Route::automatic(
                logical.from.clone(),
                logical.to.clone(),
                kind,
                *choice,
                offset,
            );
        }
    }
    Some(logical)
}

pub(crate) fn clipping_candidate(
    scene: &Scene,
    name: &str,
    feedback: &mut LayoutFeedback,
    acceptable: impl Fn(&Route) -> bool,
) -> Option<Route> {
    let (op, kind) = automatic_op(scene, name)?;
    let logical = planned_route(scene, name, feedback)?;
    let offset = logical.parameter?;
    let offsets = route_offsets(offset, route_base(kind));
    let choices = candidates(op, kind);
    for offset in offsets {
        for choice in &choices {
            let candidate = Route::automatic(
                logical.from.clone(),
                logical.to.clone(),
                kind,
                *choice,
                offset,
            );
            if candidate == logical {
                continue;
            }
            if feedback.route_search_attempts >= feedback.route_search_cap {
                return None;
            }
            feedback.route_search_attempts += 1;
            if acceptable(&candidate) {
                feedback.route_choices.insert(name.to_owned(), *choice);
                feedback.route_min_offsets.insert(name.to_owned(), offset);
                return Some(candidate);
            }
        }
    }
    None
}

fn route_base(kind: RouteKind) -> f64 {
    match kind {
        RouteKind::Orthogonal => locus_core::preset::ROUTE_DETOUR,
        RouteKind::Curved => locus_core::preset::ROUTE_CONTROL_OFFSET,
    }
}

fn route_offsets(current: f64, default: f64) -> Vec<f64> {
    let mut offsets = vec![current];
    for factor in locus_core::preset::ROUTE_OFFSET_FACTORS {
        let offset = current.max(default) * factor;
        if !offsets.contains(&offset) {
            offsets.push(offset);
        }
    }
    offsets
}

pub(crate) fn visible_obstacle(
    scene: &Scene,
    name: &str,
    feedback: &LayoutFeedback,
) -> Option<locus_core::ResolvedObject> {
    let original = object(scene, name)?;
    let Some(logical) = planned_route(scene, name, feedback) else {
        return Some(original.clone());
    };
    Some(visible_object(
        original.clone(),
        retain_visible(&logical, name, feedback),
    ))
}

fn retain_visible(logical: &Route, name: &str, feedback: &LayoutFeedback) -> Route {
    crate::clipping::retain(logical, retained_range(logical, name, feedback))
}

pub(crate) fn retained_range(logical: &Route, name: &str, feedback: &LayoutFeedback) -> (f64, f64) {
    feedback.retained_distances.get(name).map_or_else(
        || {
            feedback
                .retained_routes
                .get(name)
                .copied()
                .unwrap_or((0.0, 1.0))
        },
        |(prefix, suffix)| {
            (
                logical.parameter_for_distance(prefix),
                logical.parameter_for_distance(&(logical.length() - suffix)),
            )
        },
    )
}

fn visible_object(
    mut object: locus_core::ResolvedObject,
    visible: Route,
) -> locus_core::ResolvedObject {
    match &mut object {
        locus_core::ResolvedObject::Path(ResolvedPath {
            geometry: PathGeometry::Routed { route, .. },
            ..
        }) => *route = Some(visible),
        locus_core::ResolvedObject::Path(ResolvedPath {
            geometry: PathGeometry::Segment(segment),
            ..
        }) => {
            segment.start = visible.from.into();
            segment.end = visible.to.into();
        }
        _ => {}
    }
    object
}
