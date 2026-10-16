//! One initial retry using clipping choices from explicitly unsolved geometry.

use super::{Counters, merge_choices};
use crate::{LayoutOptions, TextBoxes};
use locus_core::{EvalError, EvaluationInputs, Scene};
use locus_syntax::{Expr, File, Statement};

pub(super) fn scene(
    file: &File,
    inputs: &mut EvaluationInputs,
    measured: &TextBoxes,
    options: LayoutOptions,
    first_pass: bool,
    counters: &mut Counters,
) -> Result<Scene, EvalError> {
    match Scene::with_inputs(file.clone(), inputs.clone()) {
        Ok(scene) => Ok(scene),
        Err(error) => {
            if !first_pass || error.code != "E413" || !has_trim(&file.statements) {
                return Err(error);
            }
            counters.searches += error.outer_search_attempts();
            recover(file, inputs, measured, options, counters).ok_or(error)
        }
    }
}

fn recover(
    file: &File,
    inputs: &mut EvaluationInputs,
    measured: &TextBoxes,
    options: LayoutOptions,
    counters: &mut Counters,
) -> Option<Scene> {
    let seed = Scene::layout_seed_with_inputs(file.clone(), inputs.clone()).ok()?;
    locus_core::trace::record("layout_recovery_seed", || {
        (
            serde_json::json!({"solved": false}),
            seed.resolved().cloned(),
        )
    });
    let feedback = crate::discrete::layout_program_with_budget(
        &seed,
        measured,
        options,
        counters.remaining_routes(),
    )
    .ok()?;
    locus_core::trace::record("layout_recovery", || {
        (
            serde_json::json!({
                "solved": false, "layout": format!("{:?}", feedback.layout),
                "route_search_attempts": feedback.route_search_attempts,
            }),
            seed.resolved().cloned(),
        )
    });
    counters.routes += feedback.route_search_attempts;
    if !feedback
        .retained_distances
        .values()
        .any(|(prefix, suffix)| *prefix > 0.0 || *suffix > 0.0)
    {
        return None;
    }
    let mut proposed = inputs.clone();
    proposed.choices = merge_choices(&inputs.choices, &feedback);
    let scene = Scene::with_inputs(file.clone(), proposed.clone()).ok()?;
    *inputs = proposed;
    Some(scene)
}

fn has_trim(statements: &[Statement]) -> bool {
    statements.iter().any(|statement| match statement {
        Statement::Rel(rel) => trim_expression(&rel.expr),
        Statement::Op(op) => op.where_clauses.iter().any(trim_expression),
        Statement::Fn(function) => has_trim(&function.body),
        _ => false,
    })
}

fn trim_expression(expr: &Expr) -> bool {
    match expr {
        Expr::Call { name, .. } => name.name == "trim_endpoints",
        Expr::Group { expr, .. } => trim_expression(expr),
        Expr::Binary {
            op: locus_syntax::BinOp::And,
            lhs,
            rhs,
            ..
        } => trim_expression(lhs) || trim_expression(rhs),
        _ => false,
    }
}
