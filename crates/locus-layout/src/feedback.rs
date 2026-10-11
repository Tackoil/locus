//! Bounded continuous solve and discrete layout agreement.

use locus_core::{EvaluationInputs, Scene};
use locus_syntax::File;
use thiserror::Error;

use crate::{LayoutError, LayoutOptions, LayoutScene, TextBoxes};
mod recovery;

#[derive(Default)]
struct Counters {
    searches: usize,
    routes: usize,
}

impl Counters {
    fn remaining_routes(&self) -> usize {
        locus_core::preset::SEARCH_CAP.saturating_sub(self.routes)
    }
}

/// Failure while bringing continuous measurements and discrete layout into agreement.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum SolveLayoutError {
    /// Continuous geometry or measured layout requirements are infeasible.
    #[error("{0}")]
    Evaluation(#[from] locus_core::EvalError),
    /// Routing, clipping, or the bounded feedback pass failed.
    #[error("{0}")]
    Layout(#[from] LayoutError),
}

/// Solves and places a measured program with a bounded measurement feedback loop.
///
/// Every discrete change is frozen before the next solve, so constraints and
/// assertions observe the same logical routes and label boxes as rendering.
/// An initial clipping-related continuous failure may use one explicitly
/// unsolved seed to obtain trim choices before retrying the ordinary solve.
///
/// # Errors
/// Returns the original continuous failure or a source-anchored layout conflict
/// when routing, clipping, or measurement feedback cannot converge.
pub fn solve_layout(
    file: &File,
    mut inputs: EvaluationInputs,
    measured: &TextBoxes,
    options: LayoutOptions,
) -> Result<(Scene, LayoutScene), SolveLayoutError> {
    let _layout = locus_core::metrics::span("solve_layout");
    let mut changed = Vec::new();
    let mut counters = Counters::default();
    let mut conflict_span = file.span;
    let mut relations = Vec::new();
    for pass in 0..locus_core::preset::MEASUREMENT_PASS_CAP {
        let _pass = locus_core::metrics::span("measurement_pass");
        let mut scene = recovery::scene(
            file,
            &mut inputs,
            measured,
            options,
            pass == 0,
            &mut counters,
        )?;
        counters.searches += scene
            .solve_summary()
            .map_or(0, |summary| summary.outer_search_attempts);
        let feedback = crate::discrete::layout_program_with_budget(
            &scene,
            measured,
            options,
            counters.remaining_routes(),
        )?;
        counters.routes += feedback.route_search_attempts;
        let accepted = accepted_choices(&scene);
        let next = merge_choices(&accepted, &feedback);
        let scale = scene.resolved().map_or(1.0, |resolved| {
            locus_core::Tolerance::from_bbox(&resolved.bbox).diameter()
        });
        changed = changed_choices(&accepted, &next, scale);
        if changed.is_empty() {
            scene.inputs.choices = next;
            record_passes(&mut scene, pass + 1, counters.searches, counters.routes);
            return Ok((scene, feedback.layout));
        }
        relations = relevant_relations(&scene.file, &changed);
        conflict_span = relations.first().copied().unwrap_or(scene.file.span);
        inputs.choices = next;
    }
    Err(LayoutError::Conflict {
        span: conflict_span,
        measurements: changed.clone(),
        message: format!(
            "measurement feedback did not converge after {} passes: {}; related relations at {}",
            locus_core::preset::MEASUREMENT_PASS_CAP,
            changed.join(", "),
            relations
                .iter()
                .map(|span| format!("{}:{}", span.line, span.col))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
    .into())
}

fn accepted_choices(scene: &Scene) -> locus_core::EvaluationChoices {
    let mut choices = scene.inputs.choices.clone();
    choices
        .routes
        .extend(crate::routing::accepted_choices(scene));
    if let Some(resolved) = scene.resolved() {
        for (id, object) in &resolved.objects {
            if let locus_core::ResolvedObject::Annotation(locus_core::AnnotationSpec::Label {
                path: Some(path),
                ..
            }) = object
            {
                choices.labels.insert(
                    id.name.clone(),
                    locus_core::LabelChoice {
                        side: path.side,
                        candidate: path.candidate,
                    },
                );
            }
        }
    }
    choices
}

fn merge_choices(
    current: &locus_core::EvaluationChoices,
    feedback: &crate::discrete::LayoutFeedback,
) -> locus_core::EvaluationChoices {
    let mut next = current.clone();
    next.routes.extend(feedback.route_choices.clone());
    next.route_min_offsets
        .extend(feedback.route_min_offsets.clone());
    next.retained_routes
        .extend(feedback.retained_routes.clone());
    next.retained_distances
        .extend(feedback.retained_distances.clone());
    next.labels.extend(feedback.label_choices.clone());
    next
}

fn record_passes(scene: &mut Scene, passes: usize, searches: usize, routes: usize) {
    if let locus_core::Evaluation::Solved { summary, .. } = &mut scene.evaluation {
        summary.measurement_passes = passes;
        summary.outer_search_attempts = searches;
        summary.routing_attempts = routes;
    }
}

fn relevant_relations(file: &File, changed: &[String]) -> Vec<locus_syntax::Span> {
    let names = changed
        .iter()
        .filter_map(|value| value.split_whitespace().next())
        .collect::<std::collections::BTreeSet<_>>();
    file.statements
        .iter()
        .flat_map(|statement| match statement {
            locus_syntax::Statement::Rel(rel) => vec![&rel.expr],
            locus_syntax::Statement::Op(op) => op.where_clauses.iter().collect(),
            _ => Vec::new(),
        })
        .filter(|expr| mentions(expr, &names))
        .map(locus_syntax::Expr::span)
        .collect()
}

fn mentions(expr: &locus_syntax::Expr, names: &std::collections::BTreeSet<&str>) -> bool {
    use locus_syntax::Expr;
    match expr {
        Expr::Ref(id) => names.contains(id.name.as_str()),
        Expr::Call { args, .. } => args.iter().any(|arg| mentions(arg, names)),
        Expr::Property { base, args, .. } => std::iter::once(base.as_ref())
            .chain(args)
            .any(|arg| mentions(arg, names)),
        Expr::Binary { lhs, rhs, .. } => mentions(lhs, names) || mentions(rhs, names),
        Expr::Unary { expr, .. } | Expr::Group { expr, .. } => mentions(expr, names),
        _ => false,
    }
}

fn changed_choices(
    current: &locus_core::EvaluationChoices,
    next: &locus_core::EvaluationChoices,
    scale: f64,
) -> Vec<String> {
    let mut changed = Vec::new();
    changed.extend(
        next.routes
            .iter()
            .filter(|(id, v)| current.routes.get(*id) != Some(*v))
            .map(|(id, _)| format!("{id} route")),
    );
    changed.extend(
        next.labels
            .iter()
            .filter(|(id, v)| current.labels.get(*id) != Some(*v))
            .map(|(id, _)| format!("{id} label")),
    );
    changed.extend(
        next.route_min_offsets
            .iter()
            .filter(|(id, v)| {
                current
                    .route_min_offsets
                    .get(*id)
                    .is_none_or(|old| !close_length(*old, **v, scale))
            })
            .map(|(id, _)| format!("{id} detour")),
    );
    changed.extend(
        next.retained_routes
            .iter()
            .filter(|(id, v)| {
                !next.retained_distances.contains_key(*id)
                    && current
                        .retained_routes
                        .get(*id)
                        .is_none_or(|old| !close(old.0, v.0) || !close(old.1, v.1))
            })
            .map(|(id, _)| format!("{id} retained route")),
    );
    changed.extend(
        next.retained_distances
            .iter()
            .filter(|(id, v)| {
                current.retained_distances.get(*id).is_none_or(|old| {
                    !close_length(old.0, v.0, scale) || !close_length(old.1, v.1, scale)
                })
            })
            .map(|(id, _)| format!("{id} retained route")),
    );
    changed
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-12 * a.abs().max(b.abs()).max(1.0)
}

fn close_length(a: f64, b: f64, scale: f64) -> bool {
    (a - b).abs() <= 1e-12 * scale.max(a.abs()).max(b.abs())
}
