//! Clipping recovery supplies choices but never accepts unsolved seed geometry.
#![allow(
    clippy::expect_used,
    reason = "regression fixtures require failure context"
)]

use locus_core::{EvaluationInputs, Scene, evaluate_assertions_with_inputs};
use locus_layout::{LayoutOptions, SolveLayoutError, TextBoxes, layout_program, solve_layout};

fn file(obstacle: f64, width: f64) -> locus_syntax::File {
    let source = format!(
        "locus 0 beta 6\npoint origin()\nrect source(origin,{width},2)\npoint end = on_circle(circle(origin, 10), 0deg)\narrow path(origin,end)\npoint obstacle = lerp(origin, on_circle(circle(origin, style_metric(\"short_length\")), 0deg), ({obstacle}) / style_metric(\"short_length\"))\nrel trim_endpoints(path,source)\nrel avoid(obstacle,path)\nassert avoid(obstacle,path)\nassert path.length == 10\nassert path.width == 10\n"
    );
    let parsed = locus_syntax::parse(&source);
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    parsed.file
}

#[test]
fn initial_logical_collision_recovers_using_visible_trimmed_obstacle() {
    let file = file(1.0, 4.0);
    let initial = Scene::new(file.clone()).expect_err("logical stroke intersects point");
    assert_eq!(initial.code, "E413");
    assert!(initial.outer_search_attempts() > 0);
    let seed = Scene::layout_seed_with_inputs(file.clone(), EvaluationInputs::default())
        .expect("relation-aware seed");
    assert!(seed.solve_summary().is_none());
    let (accepted, _) = solve_layout(
        &file,
        EvaluationInputs::default(),
        &TextBoxes::new(),
        LayoutOptions::default(),
    )
    .expect("ordinary solve with clipped obstacle");
    assert_eq!(
        accepted.inputs.choices.retained_distances["path"],
        (2.0, 0.0)
    );
    let summary = accepted.solve_summary().expect("accepted solve report");
    assert_eq!(
        summary.outer_search_attempts,
        initial.outer_search_attempts() + 1
    );
    assert_eq!(summary.measurement_passes, 1);
    assert_eq!(summary.routing_attempts, 0);
    let report = evaluate_assertions_with_inputs(
        &file,
        accepted.resolved().expect("accepted geometry"),
        &accepted.inputs,
    )
    .expect("visible avoidance and logical bounds assertions");
    assert!(report.passed, "{:?}", report.assertions);
}

#[test]
fn truly_infeasible_visible_collision_preserves_original_diagnostic() {
    let file = file(5.0, 4.0);
    let initial = Scene::new(file.clone()).expect_err("logical collision");
    let error = solve_layout(
        &file,
        EvaluationInputs::default(),
        &TextBoxes::new(),
        LayoutOptions::default(),
    )
    .expect_err("visible collision remains infeasible");
    assert_eq!(error, SolveLayoutError::Evaluation(initial));
}

#[test]
fn unsupported_fully_enclosed_seed_preserves_original_diagnostic() {
    let file = file(1.0, 24.0);
    let initial = Scene::new(file.clone()).expect_err("logical collision");
    let error = solve_layout(
        &file,
        EvaluationInputs::default(),
        &TextBoxes::new(),
        LayoutOptions::default(),
    )
    .expect_err("fully enclosed direct path cannot bootstrap");
    assert_eq!(error, SolveLayoutError::Evaluation(initial));
}

#[test]
fn bootstrap_route_probes_share_the_normal_feedback_budget_and_report() {
    let parsed = locus_syntax::parse(
        "locus 0 beta 6\npoint origin()\nrect source(origin,24,6)\npoint end = on_circle(circle(origin, 10), 0deg)\narrow path(origin,end) { route: orthogonal, side: right }\nrel trim_endpoints(path,source)\nrel avoid(origin,path)\nassert avoid(origin,path)\n",
    );
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    let initial = Scene::new(parsed.file.clone()).expect_err("logical endpoint collision");
    let seed = Scene::layout_seed_with_inputs(parsed.file.clone(), EvaluationInputs::default())
        .expect("unsolved automatic route");
    let bootstrap = layout_program(&seed, &TextBoxes::new(), LayoutOptions::default())
        .expect("clipping route probes");
    assert!(bootstrap.route_search_attempts > 0);
    let (accepted, _) = solve_layout(
        &parsed.file,
        EvaluationInputs::default(),
        &TextBoxes::new(),
        LayoutOptions::default(),
    )
    .expect("ordinary solve after bootstrap route choice");
    let summary = accepted.solve_summary().expect("accepted solve report");
    assert_eq!(summary.routing_attempts, bootstrap.route_search_attempts);
    assert!(summary.routing_attempts <= locus_core::preset::SEARCH_CAP);
    assert_eq!(
        summary.outer_search_attempts,
        initial.outer_search_attempts() + 1
    );
    assert_eq!(summary.measurement_passes, 1);
    assert!(
        evaluate_assertions_with_inputs(
            &parsed.file,
            accepted.resolved().expect("accepted geometry"),
            &accepted.inputs,
        )
        .expect("accepted visible clearance")
        .passed
    );
}
