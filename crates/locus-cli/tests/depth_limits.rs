//! Malicious source receives structured diagnostics without terminating batch checks.

use std::{error::Error, fs};

use assert_cmd::Command;

#[test]
fn deeply_nested_input_reports_a_diagnostic_and_preserves_batch_processing()
-> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let bad = directory.path().join("deep.locus");
    let good = directory.path().join("ordinary.locus");
    fs::write(
        &bad,
        format!(
            "locus 0 beta 6\nassert {}true{}\n",
            "(".repeat(5000),
            ")".repeat(5000)
        ),
    )?;
    fs::write(&good, "locus 0 beta 6\npoint P()\n")?;
    let output = Command::cargo_bin("locus")?
        .current_dir(directory.path())
        .arg("check")
        .args([&bad, &good])
        .arg("--json")
        .output()?;
    assert_eq!(output.status.code(), Some(3));
    let report: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(report["files"][0]["diagnostics"][0]["code"], "E161");
    assert_eq!(report["files"][1]["passed"], true);
    Ok(())
}
