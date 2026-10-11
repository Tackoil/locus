//! Preference convergence, honest incomplete warnings, and process determinism.
use assert_cmd::Command;
use serde_json::Value;
use std::{error::Error, fs, path::Path};
type Result<T = ()> = std::result::Result<T, Box<dyn Error>>;

fn check(path: &Path, strict: bool) -> Result<std::process::Output> {
    let mut command = Command::cargo_bin("locus")?;
    command.arg("check").arg(path).arg("--json").env(
        "LOCUS_REJECTED_REQUESTS_DIR",
        path.with_extension("requests"),
    );
    if strict {
        command.arg("--strict");
    }
    Ok(command.output()?)
}

#[test]
fn armijo_cycle_converges_and_serializes_settling_stats() -> Result {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("cycle.locus");
    fs::write(
        &path,
        include_str!("../../../fixtures/beta6/m2/settling_cycle.locus"),
    )?;
    let normal = check(&path, false)?;
    assert!(
        normal.status.success(),
        "{}",
        String::from_utf8_lossy(&normal.stdout)
    );
    let report: Value = serde_json::from_slice(&normal.stdout)?;
    let settling = &report["files"][0]["solve"]["settling"];
    assert!(
        settling["line_search_evaluations"]
            .as_u64()
            .is_some_and(|n| n > 0)
    );
    assert_eq!(settling["settling_complete"], true);
    assert!(
        report["files"][0]["diagnostics"]
            .as_array()
            .is_some_and(|ds| ds.iter().all(|d| d["code"] != "settling_incomplete"))
    );
    Ok(())
}

#[test]
fn bounded_recovery_retains_honest_incomplete_strict_warnings() -> Result {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("wide-cycle.locus");
    let mut source = include_str!("../../../fixtures/beta6/m2/settling_cycle.locus").to_owned();
    // Extra independent freedoms exceed the pinned curvature-rebuild dimension cap.
    for index in 0..33 {
        use std::fmt::Write;
        writeln!(source, "num spare{index} = free 0")?;
    }
    fs::write(&path, source)?;
    let normal = check(&path, false)?;
    assert!(normal.status.success());
    let report: Value = serde_json::from_slice(&normal.stdout)?;
    assert_eq!(
        report["files"][0]["solve"]["settling"]["settling_complete"],
        false
    );
    assert!(
        report["files"][0]["diagnostics"]
            .as_array()
            .is_some_and(|ds| ds.iter().any(|d| d["code"] == "settling_incomplete"))
    );
    assert_eq!(check(&path, true)?.status.code(), Some(1));
    Ok(())
}

#[test]
fn fixed_crossing_stroke_fails_as_layout_conflict_with_exit_two() -> Result {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("crossing.locus");
    fs::write(
        &path,
        include_str!("../../../fixtures/beta6/m2/crossing_stroke.locus"),
    )?;
    let result = check(&path, false)?;
    assert_eq!(result.status.code(), Some(2));
    let report: Value = serde_json::from_slice(&result.stdout)?;
    assert_eq!(report["files"][0]["diagnostics"][0]["code"], "E413");
    Ok(())
}

#[test]
fn unchanged_checkout_completes_settling_in_repeatable_fresh_processes() -> Result {
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../spec/test/composition/checkout_sequence.locus");
    let first = check(&source, false)?;
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stdout)
    );
    let report: Value = serde_json::from_slice(&first.stdout)?;
    let solve = &report["files"][0]["solve"];
    assert_eq!(solve["settling"]["settling_complete"], true);
    assert!(
        solve["max_normalized_residual"]
            .as_f64()
            .is_some_and(|r| r < 1e-12)
    );
    assert!(
        solve["settling"]["projected_gradient"]
            .as_f64()
            .is_some_and(|g| g < 1e-9)
    );
    let second = check(&source, false)?;
    assert!(second.status.success());
    assert_eq!(first.stdout, second.stdout);
    Ok(())
}
