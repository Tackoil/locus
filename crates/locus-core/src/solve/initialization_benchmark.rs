//! Opt-in, frozen-measurement initialization audit and release benchmark.
#![allow(
    clippy::expect_used,
    reason = "benchmark fixtures fail with explicit context"
)]

use super::*;
use serde_json::{Value, json};
use std::time::Instant;

fn checkout_inputs() -> crate::EvaluationInputs {
    measured_inputs("examples/demonstrations/checkout_sequence.locus")
}

fn measured_inputs(source: &str) -> crate::EvaluationInputs {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../../tests/fixtures/text_measurements.json"
    ))
    .expect("measurement fixture");
    let measurements = fixture["measurements"][source]
        .as_object()
        .expect("checkout measurements");
    let mut inputs = crate::EvaluationInputs::default();
    for (name, text) in measurements {
        inputs.measured.insert(
            name.clone(),
            crate::MeasuredText {
                width: f64::from_bits(text["width_bits"].as_u64().expect("width")),
                height: f64::from_bits(text["height_bits"].as_u64().expect("height")),
                role: if text["role"] == "title" {
                    crate::MeasuredRole::Title
                } else {
                    crate::MeasuredRole::Normal
                },
            },
        );
    }
    inputs
}

#[test]
fn stress_sequence_initial_relations_are_feasible() {
    let parsed = locus_syntax::parse(include_str!(
        "../../../../examples/stress/sequence_check.locus"
    ));
    let expanded = crate::program::expand_program(&parsed.file).expect("expanded stress sequence");
    let inputs = measured_inputs("examples/stress/sequence_check.locus");
    let prepared = PreparedEvaluation::with_inputs(&expanded.file, &inputs, &expanded.collections)
        .expect("prepared stress sequence");
    let audit = relation_audit(&prepared);
    assert_eq!(audit["failed_relations"], 0, "{audit}");
    assert!(audit["max_residual"].as_f64().expect("finite residual") < SOLVE_EPSILON);
}

fn relation_audit(prepared: &PreparedEvaluation<'_>) -> Value {
    let sources = relations::collect(prepared.file());
    let (system, phases) = crate::trace::collect(|| initialize(prepared, &sources));
    let system = system.expect("initialization");
    let branches = relations::select_circle_branches(&sources);
    let state = evaluate_state(
        prepared,
        &system,
        &sources,
        &branches,
        &system.normalized_hints,
    )
    .expect("initial residuals");
    let rows: Vec<_> = sources
        .iter()
        .enumerate()
        .map(|(index, source)| {
            let maximum = state
                .equations
                .iter()
                .filter(|row| row.relation == index)
                .map(|row| row.residual.value.abs())
                .fold(0.0, f64::max);
            json!({"index":index, "expression":source.expression, "line":source.span.line,
            "failed": maximum >= SOLVE_EPSILON, "max_normalized_residual":maximum,
            "scene_units":maximum * system.diameter})
        })
        .collect();
    let failed = rows.iter().filter(|row| row["failed"] == true).count();
    let objects: Vec<_> = system.objects.iter().map(|o| json!({"id":o.id,"class":o.class.as_str(),"values":system.hints[o.start..o.start+o.count]})).collect();
    json!({"failed_relations":failed, "max_residual":max_residual(&state.residuals),
        "relations":rows,"objects":objects,"seed":phases.into_iter().find(|s| s.name == "initialization_start").map(|s| s.data)})
}

fn measured_solve(prepared: &PreparedEvaluation<'_>) -> Value {
    let start = Instant::now();
    let (result, timing) = crate::metrics::collect(|| solve_prepared(prepared));
    let elapsed = start.elapsed().as_secs_f64() * 1000.0;
    let solved = match result {
        Ok(solved) => solved,
        Err(error) => return json!({"total_ms":elapsed,"error":error.to_string()}),
    };
    let summary = &solved.summary;
    let settling = summary.settling.as_ref().expect("settling summary");
    assert!(summary.max_normalized_residual < SOLVE_EPSILON);
    json!({"total_ms":elapsed, "attempts":summary.attempts,
        "settling_complete":settling.settling_complete,
        "seed_repair_steps":timing.steps.iter().filter(|s| s.name == "seed_repair_step").count(),
        "accepted_steps":summary.accepted_steps, "candidates":summary.outer_search_attempts,
        "settling_evaluations":settling.evaluations, "settling_steps":settling.steps,
        "initialize_ms":timing.steps.iter().filter(|s| s.name == "initialize").map(|s| s.duration_ms).sum::<f64>(),
        "feasibility_ms":timing.steps.iter().filter(|s| s.name == "levenberg_marquardt").map(|s| s.duration_ms).sum::<f64>(),
        "settling_ms":timing.steps.iter().filter(|s| s.name == "settle_preferences").map(|s| s.duration_ms).sum::<f64>()})
}

#[test]
#[ignore = "opt-in release benchmark; writes LOCUS_INIT_BENCH_OUT"]
fn checkout_initialization_stages() {
    let parsed = locus_syntax::parse(include_str!(
        "../../../../examples/demonstrations/checkout_sequence.locus"
    ));
    let expanded = crate::program::expand_program(&parsed.file).expect("expanded checkout");
    let prepared =
        PreparedEvaluation::with_inputs(&expanded.file, &checkout_inputs(), &expanded.collections)
            .expect("prepared checkout");
    let audit = relation_audit(&prepared);
    let stage = std::env::var("LOCUS_INIT_BENCH_STAGE").expect("stage name");
    let output = std::env::var("LOCUS_INIT_BENCH_OUT").expect("output path");
    let count = std::env::var("LOCUS_INIT_BENCH_RUNS")
        .ok()
        .and_then(|n| n.parse().ok())
        .unwrap_or(3);
    let mut report = json!({"stage":stage,"audit":audit,"runs":[]});
    write_report(&output, &report);
    for _ in 0..count {
        let run = measured_solve(&prepared);
        eprintln!("{stage}: {run}");
        report["runs"].as_array_mut().expect("runs").push(run);
        write_report(&output, &report);
    }
    println!(
        "{stage}: initial failures={}, runs={}",
        report["audit"]["failed_relations"],
        report["runs"].as_array().expect("runs").len()
    );
}

fn write_report(output: &str, report: &Value) {
    std::fs::write(
        output,
        serde_json::to_vec_pretty(report).expect("report JSON"),
    )
    .expect("write report");
}

#[test]
fn checkout_initial_relations_are_feasible() {
    let parsed = locus_syntax::parse(include_str!(
        "../../../../examples/demonstrations/checkout_sequence.locus"
    ));
    let expanded = crate::program::expand_program(&parsed.file).expect("expanded checkout");
    let prepared =
        PreparedEvaluation::with_inputs(&expanded.file, &checkout_inputs(), &expanded.collections)
            .expect("prepared checkout");
    let audit = relation_audit(&prepared);
    let failed: Vec<_> = audit["relations"]
        .as_array()
        .expect("relations")
        .iter()
        .filter(|row| row["failed"] == true)
        .collect();
    assert!(failed.is_empty(), "initial checkout relations: {failed:?}");
}
