//! Run the actual spec stress examples through `check`, including derived assertions.

use std::error::Error;
use std::fs;
use std::path::Path;
use std::time::Duration;

use assert_cmd::Command;

#[path = "../../../tests/support/stress.rs"]
mod support;

fn integer(value: &serde_json::Value) -> Result<usize, Box<dyn Error>> {
    Ok(usize::try_from(
        value.as_u64().ok_or("missing integer in solve report")?,
    )?)
}

#[test]
fn every_stress_example_passes_its_assertions() -> Result<(), Box<dyn Error>> {
    let cases = support::cases()?;
    for case in &cases {
        case.parse()?;
    }
    let output = Command::cargo_bin("locus")?
        .arg("check")
        .args(cases.iter().map(support::Case::path))
        .arg("--json")
        // This batches all measured layout systems, each with bounded solve feedback.
        .timeout(Duration::from_secs(1200))
        .output()?;
    assert!(
        output.status.success(),
        "spec check failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    let files = report["files"].as_array().ok_or("missing file reports")?;
    assert_eq!(files.len(), cases.len());
    for (file, case) in files.iter().zip(&cases) {
        assert_eq!(file["path"], case.path().to_string_lossy().as_ref());
        assert_eq!(file["passed"], true, "{}", case.name);
        assert_eq!(file["assertions"]["passed"], true, "{}", case.name);
        let assertions = file["assertions"]["assertions"]
            .as_array()
            .ok_or("missing assertion results")?;
        let solve = &file["solve"];
        case.check_report(
            assertions.len(),
            integer(&solve["equations"])?,
            integer(&solve["attempts"])?,
            solve["max_normalized_residual"]
                .as_f64()
                .ok_or("missing residual")?,
        );
    }
    Ok(())
}

#[test]
fn closed_segment_bisectors_converge_with_finite_membership() -> Result<(), Box<dyn Error>> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/solver/closed_segment_bisectors.locus");
    let output = Command::cargo_bin("locus")?
        .arg("check")
        .arg(&path)
        .arg("--json")
        .output()?;
    assert!(output.status.success());
    let report: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(report["passed"], true);
    let file = &report["files"][0];
    assert_eq!(file["assertions"]["passed"], true);
    assert!(
        file["solve"]["max_normalized_residual"]
            .as_f64()
            .ok_or("missing solve residual")?
            < 1e-12
    );

    // Preserve the historical source; also check its supporting-line equivalent.
    let source = fs::read_to_string(path)?
        .replace("on(D, seg(B, C))", "on(D, line(B, C))")
        .replace("on(E, seg(A, C))", "on(E, line(A, C))")
        .replace("on(F, seg(A, B))", "on(F, line(A, B))");
    let directory = tempfile::tempdir()?;
    let passing = directory.path().join("supporting-lines.locus");
    fs::write(&passing, source)?;
    Command::cargo_bin("locus")?
        .arg("check")
        .arg(passing)
        .assert()
        .success();
    Ok(())
}
