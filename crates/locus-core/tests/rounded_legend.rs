//! A freely sized rounded legend must compact without repeated failed corrections.
#![allow(clippy::expect_used, reason = "regressions need fixture context")]

use locus_core::{
    EvaluationInputs, MeasuredRole, MeasuredText, Scene, evaluate_assertions_with_inputs,
};

const LEGEND: &str = include_str!("../../../tests/fixtures/programs/layout/rounded_legend.locus");

fn inputs(width_scale: f64, padding: f64, radius_fraction: f64) -> EvaluationInputs {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/text_measurements.json"
    ))
    .expect("bundled font measurements");
    let measurements = &fixture["measurements"]["examples/demonstrations/checkout_sequence.locus"];
    let mut inputs = EvaluationInputs::default();
    inputs.metrics.padding = padding;
    inputs.metrics.rounded_radius_fraction = radius_fraction;
    for id in ["key.requests.caption", "key.responses.caption"] {
        let value = &measurements[id];
        inputs.measured.insert(
            id.to_owned(),
            MeasuredText {
                width: f64::from_bits(value["width_bits"].as_u64().expect("width")) * width_scale,
                height: f64::from_bits(value["height_bits"].as_u64().expect("height")),
                role: MeasuredRole::Normal,
            },
        );
    }
    inputs
}

fn solve(source: &str, inputs: EvaluationInputs) -> Scene {
    let parsed = locus_syntax::parse(source);
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    let scene = Scene::with_inputs(parsed.file, inputs).expect("feasible rounded legend");
    let assertions = evaluate_assertions_with_inputs(
        &scene.source_file,
        scene.resolved().expect("resolved legend"),
        &scene.inputs,
    )
    .expect("legend assertions");
    assert!(assertions.passed, "{assertions:?}");
    let summary = scene.solve_summary().expect("legend summary");
    assert!(summary.max_normalized_residual < 1e-12);
    assert!(
        summary
            .settling
            .as_ref()
            .expect("settling")
            .settling_complete,
        "padding {}, radius fraction {}: {:?}",
        scene.inputs.metrics.padding,
        scene.inputs.metrics.rounded_radius_fraction,
        summary.settling
    );
    scene
}

#[test]
fn isolated_legend_compacts_with_bounded_restoration_and_is_repeatable() {
    let scene = solve(LEGEND, inputs(1.0, 0.25, 0.15));
    let report = scene
        .solve_summary()
        .expect("summary")
        .settling
        .as_ref()
        .expect("settling");
    assert_eq!(report.restoration_steps, 0, "{report:?}");
    assert!(report.accepted_moves <= 8, "{report:?}");
    assert_eq!(report.line_search_evaluations, report.accepted_moves);
    let repeated = solve(LEGEND, inputs(1.0, 0.25, 0.15));
    assert_eq!(scene.resolved(), repeated.resolved());
    assert_eq!(scene.solve_summary(), repeated.solve_summary());
}

#[test]
fn legend_adapts_to_text_padding_rounding_and_neighbor_constraints() {
    for (width, padding, radius) in [
        (2.0, 0.25, 0.15),
        (1.0, 0.0, 0.15),
        (1.0, 0.1, 0.15),
        (1.0, 0.5, 0.15),
        (1.0, 0.25, 0.05),
        (1.0, 0.25, 0.45),
    ] {
        solve(LEGEND, inputs(width, padding, radius));
    }
    let surrounded = format!(
        "{LEGEND}\nrect above(6,1)\nrect below(6,1)\nrel gap_y(above,key.frame,below)\nrel align_left(key.frame,below)\nassert gap_y(above,key.frame,below)\n"
    );
    solve(&surrounded, inputs(1.0, 0.25, 0.15));
}

#[test]
fn fixed_legend_dimensions_remain_binding() {
    let fixed = LEGEND.replace("rounded_rect frame()", "rounded_rect frame(8,4,0.6)");
    solve(&fixed, inputs(1.0, 0.25, 0.15));
    let too_small = LEGEND.replace("rounded_rect frame()", "rounded_rect frame(1,1,0.1)");
    let parsed = locus_syntax::parse(&too_small);
    assert!(parsed.diagnostics.is_empty());
    let error = Scene::with_inputs(parsed.file, inputs(1.0, 0.25, 0.15))
        .expect_err("fixed frame cannot grow to fit its captions");
    assert_eq!(error.code, "E413");
}
