//! Source-aware discrete route selection and visible endpoint clipping.

use crate::{LayoutError, LayoutOptions, LayoutScene, TextBoxes};
use locus_core::geom::route::{Route, RouteChoice};
use locus_core::{ResolvedObject, Scene};
use locus_syntax::{Expr, Span, Statement};
use std::collections::BTreeMap;

/// Choices to freeze before repeating the continuous solve.
#[derive(Clone, Debug)]
pub struct LayoutFeedback {
    /// Accepted drawing using currently solved geometry.
    pub layout: LayoutScene,
    /// Actual route candidate validations in this bounded discrete pass.
    pub route_search_attempts: usize,
    /// Remaining route candidate budget for this pass.
    pub(crate) route_search_cap: usize,
    /// Proposed frozen route sides and topologies.
    pub route_choices: BTreeMap<String, RouteChoice>,
    /// Minimum detour required by accepted route candidates.
    pub route_min_offsets: BTreeMap<String, f64>,
    /// Retained logical parameter intervals for clipped labels.
    pub retained_routes: BTreeMap<String, (f64, f64)>,
    /// Physical prefix/suffix arc distances retained through live route changes.
    pub retained_distances: BTreeMap<String, (f64, f64)>,
    /// Proposed path-label region candidates and sides.
    pub label_choices: BTreeMap<String, locus_core::inputs::LabelChoice>,
}

/// Performs source-aware discrete layout without mutating logical geometry.
///
/// # Errors
/// Returns source-anchored conflicts for impossible routing or clipping.
pub fn layout_program(
    scene: &Scene,
    measured: &TextBoxes,
    options: LayoutOptions,
) -> Result<LayoutFeedback, LayoutError> {
    layout_program_with_budget(scene, measured, options, locus_core::preset::SEARCH_CAP)
}

pub(crate) fn layout_program_with_budget(
    scene: &Scene,
    measured: &TextBoxes,
    options: LayoutOptions,
    cap: usize,
) -> Result<LayoutFeedback, LayoutError> {
    let _layout = locus_core::metrics::span("discrete_layout");
    let resolved = scene
        .resolved()
        .ok_or_else(|| conflict(Span::default(), &[], "missing resolved scene"))?;
    let mut result = LayoutFeedback {
        layout: locus_core::metrics::measure("place_annotations", || {
            crate::layout(resolved, measured, options)
        })?,
        route_search_attempts: 0,
        route_search_cap: cap,
        route_choices: scene.inputs.choices.routes.clone(),
        route_min_offsets: scene.inputs.choices.route_min_offsets.clone(),
        retained_routes: BTreeMap::new(),
        retained_distances: BTreeMap::new(),
        label_choices: scene.inputs.choices.labels.clone(),
    };
    let requirements = requirements(scene);
    crate::routing::freeze_accepted(scene, &mut result);
    locus_core::metrics::measure("clip_endpoints", || {
        crate::clipping::apply(scene, &requirements, &mut result)
    })?;
    locus_core::metrics::measure("route_search", || {
        crate::routing::search(scene, &requirements, &mut result)
    })?;
    locus_core::metrics::measure("place_path_labels", || {
        crate::path_labels::search(scene, &mut result, options);
    });
    locus_core::metrics::measure("collision_report", || {
        crate::report::refresh_collisions(resolved, &mut result.layout, options);
    });
    Ok(result)
}

pub(crate) struct Requirement<'a> {
    pub name: &'a str,
    pub args: &'a [Expr],
    pub span: Span,
}

fn requirements(scene: &Scene) -> Vec<Requirement<'_>> {
    let mut result = Vec::new();
    for statement in &scene.file.statements {
        match statement {
            Statement::Rel(rel) => append(&rel.expr, &mut result),
            Statement::Op(op) => append_where(&op.where_clauses, &mut result),
            _ => {}
        }
    }
    result
}

fn append_where<'a>(expressions: &'a [Expr], result: &mut Vec<Requirement<'a>>) {
    for expr in expressions {
        append(expr, result);
    }
}

fn append<'a>(expr: &'a Expr, result: &mut Vec<Requirement<'a>>) {
    match expr {
        Expr::Call { name, args, span } => result.push(Requirement {
            name: &name.name,
            args,
            span: *span,
        }),
        Expr::Group { expr, .. } => append(expr, result),
        Expr::Binary {
            op: locus_syntax::BinOp::And,
            lhs,
            rhs,
            ..
        } => {
            append(lhs, result);
            append(rhs, result);
        }
        _ => {}
    }
}

pub(crate) fn members(scene: &Scene, expr: &Expr) -> Vec<String> {
    let mut result = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    match expr {
        Expr::Ref(id) => append_member(scene, &id.name, &mut seen, &mut result),
        Expr::Group { expr, .. } => return members(scene, expr),
        _ => {}
    }
    result
}

fn append_member(
    scene: &Scene,
    name: &str,
    seen: &mut std::collections::BTreeSet<String>,
    result: &mut Vec<String>,
) {
    if let Some(collection) = scene.collections.get(name) {
        for member in &collection.members {
            append_member(scene, member, seen, result);
        }
        return;
    }
    let identity = alias_identity(scene, name);
    if seen.insert(identity.clone()) {
        result.push(identity);
    }
}

pub(crate) fn alias_identity(scene: &Scene, name: &str) -> String {
    let mut identity = name.to_owned();
    for _ in 0..scene.file.statements.len() {
        let target = scene
            .file
            .statements
            .iter()
            .find_map(|statement| alias_target(statement, &identity));
        let Some(target) = target else {
            break;
        };
        identity = target.to_owned();
    }
    identity
}

fn alias_target<'a>(statement: &'a Statement, name: &str) -> Option<&'a str> {
    let Statement::Op(op) = statement else {
        return None;
    };
    if op.id.name != name {
        return None;
    }
    let locus_syntax::Constructor::Expr { expr, .. } = &op.ctor else {
        return None;
    };
    match expr.as_ref() {
        Expr::Ref(id) => Some(&id.name),
        _ => None,
    }
}

pub(crate) fn object<'a>(scene: &'a Scene, name: &str) -> Option<&'a ResolvedObject> {
    scene
        .resolved()?
        .objects
        .iter()
        .find(|(id, _)| id.name == name)
        .map(|(_, object)| object)
}

pub(crate) fn route(object: &ResolvedObject) -> Option<Route> {
    object.as_path()?.route().ok().flatten()
}

pub(crate) fn conflict(span: Span, measurements: &[String], message: &str) -> LayoutError {
    LayoutError::Conflict {
        span,
        measurements: measurements.to_vec(),
        message: message.to_owned(),
    }
}
