//! Concise JSON views that retain all failures and diagnostics.

use std::io::Write;

use locus_core::{AssertionResult, Incidence, InspectDiagnostic, MeasurementTables};
use serde::Serialize;

use crate::output::{CheckFileResult, CheckResult, CliDiagnostic, CommandOutput, InspectResult};

#[derive(Serialize)]
struct Check<'a> {
    command: &'static str,
    strict: bool,
    passed: bool,
    files: Vec<CheckFile<'a>>,
}

#[derive(Serialize)]
struct CheckFile<'a> {
    path: &'a str,
    passed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    solve: Option<Solve>,
    diagnostics: &'a [CliDiagnostic],
    #[serde(skip_serializing_if = "Option::is_none")]
    assertions: Option<AssertionSummary<'a>>,
}

#[derive(Serialize)]
struct Solve {
    unknowns: usize,
    equations: usize,
    rank: usize,
    global_remaining: usize,
    max_normalized_residual: f64,
}

#[derive(Serialize)]
struct AssertionSummary<'a> {
    passed: bool,
    total: usize,
    failed: usize,
    assertions: Vec<&'a AssertionResult>,
}

#[derive(Serialize)]
struct Inspect<'a> {
    command: &'static str,
    path: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    measurements: Option<&'a MeasurementTables>,
    #[serde(skip_serializing_if = "Option::is_none")]
    incidences: Option<&'a [Incidence]>,
    diagnostics: &'a [InspectDiagnostic],
}

pub(crate) fn write(writer: impl Write, output: &CommandOutput) -> serde_json::Result<()> {
    match output {
        CommandOutput::Check(result) => serde_json::to_writer(writer, &check(result)),
        CommandOutput::Inspect(result) => serde_json::to_writer(writer, &inspect(result)),
        _ => serde_json::to_writer(writer, output),
    }
}

fn check(result: &CheckResult) -> Check<'_> {
    Check {
        command: result.command,
        strict: result.strict,
        passed: result.passed,
        files: result.files.iter().map(check_file).collect(),
    }
}

fn check_file(result: &CheckFileResult) -> CheckFile<'_> {
    CheckFile {
        path: &result.path,
        passed: result.passed,
        solve: result.solve.as_ref().map(|solve| Solve {
            unknowns: solve.unknowns,
            equations: solve.equations,
            rank: solve.rank,
            global_remaining: solve.dof.global_remaining,
            max_normalized_residual: solve.max_normalized_residual,
        }),
        diagnostics: &result.diagnostics,
        assertions: result.assertions.as_ref().map(|report| {
            let assertions: Vec<_> = report
                .assertions
                .iter()
                .filter(|item| !item.passed)
                .collect();
            AssertionSummary {
                passed: report.passed,
                total: report.assertions.len(),
                failed: assertions.len(),
                assertions,
            }
        }),
    }
}

fn inspect(result: &InspectResult) -> Inspect<'_> {
    let measurements = &result.inspection.measurements;
    let has_measurements = !measurements.distances.is_empty()
        || !measurements.triangle_angles.is_empty()
        || !measurements.line_ratios.is_empty();
    Inspect {
        command: result.command,
        path: &result.path,
        measurements: has_measurements.then_some(measurements),
        incidences: (!result.inspection.incidences.is_empty())
            .then_some(result.inspection.incidences.as_slice()),
        diagnostics: &result.inspection.diagnostics,
    }
}
