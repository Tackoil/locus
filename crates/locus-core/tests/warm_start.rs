//! Feedback iterates must preserve hard constraints and authored preference targets.
#![allow(
    clippy::expect_used,
    reason = "regressions need explicit failure context"
)]

use locus_core::{EvaluationInputs, Scene};
use std::sync::Arc;

fn scene(source: &str, inputs: EvaluationInputs) -> Scene {
    let parsed = locus_syntax::parse(source);
    assert!(parsed.diagnostics.is_empty());
    Scene::with_inputs(parsed.file, inputs).expect("solved scene")
}

#[test]
fn geometry_ignores_feedback_iterates() {
    let source = "locus 0 beta 6\nnum r = free 2\ncircle c(r)\n";
    let cold = scene(source, EvaluationInputs::default());
    let previous = scene(
        &source.replace("free 2", "free 5"),
        EvaluationInputs::default(),
    );
    let warm = scene(
        source,
        EvaluationInputs {
            warm_start: Some(Arc::new(
                previous.resolved().expect("previous scene").clone(),
            )),
            ..EvaluationInputs::default()
        },
    );
    assert_eq!(warm.resolved(), cold.resolved());
}

#[test]
fn mismatched_feedback_registry_falls_back_to_normal_initialization() {
    let source = "locus 0 beta 6\nrect a(2, 1)\nrect b(2, 1)\nrel row(a, b)\n";
    let cold = scene(source, EvaluationInputs::default());
    let unrelated = scene("locus 0 beta 6\npoint p()\n", EvaluationInputs::default());
    let warm = scene(
        source,
        EvaluationInputs {
            warm_start: Some(Arc::new(
                unrelated.resolved().expect("unrelated scene").clone(),
            )),
            ..EvaluationInputs::default()
        },
    );
    assert_eq!(warm.resolved(), cold.resolved());
}

#[test]
fn feedback_reuses_geometry_without_replacing_numeric_preference_targets() {
    let source =
        "locus 0 beta 6\nnum width = free 2\nrect a(width, 1)\nrect b(2, 1)\nrel row(a, b)\n";
    let previous = scene(
        &source.replace("free 2", "free 5"),
        EvaluationInputs::default(),
    );
    let warm = scene(
        source,
        EvaluationInputs {
            warm_start: Some(Arc::new(
                previous.resolved().expect("previous scene").clone(),
            )),
            ..EvaluationInputs::default()
        },
    );
    let summary = warm.solve_summary().expect("summary");
    assert!(summary.max_normalized_residual < 1e-12);
    assert!(
        summary
            .settling
            .as_ref()
            .expect("settling")
            .settling_complete
    );
    let width = warm
        .resolved()
        .expect("resolved")
        .objects
        .iter()
        .find_map(|(id, object)| match object {
            locus_core::ResolvedObject::Num(value) if id.name == "width" => Some(value.value),
            _ => None,
        })
        .expect("numeric width");
    assert!((width - 2.0).abs() < 1e-8);
}
