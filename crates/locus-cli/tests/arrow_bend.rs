//! Captured benchmark regression: checking and exporting reject unsafe bends.

use std::error::Error;
use std::fs;

use assert_cmd::Command;
use serde_json::Value;

type Result = std::result::Result<(), Box<dyn Error>>;
const CAPTURED: &str = include_str!("fixtures/arrow-bend-singularity.locus");

#[test]
fn captured_singular_bend_fails_check_and_render_without_overwriting_output() -> Result {
    let directory = tempfile::tempdir()?;
    let source = directory.path().join("figure.locus");
    let output_path = directory.path().join("figure.svg");
    fs::write(&source, CAPTURED)?;
    fs::write(&output_path, "existing output")?;
    for command in ["check", "render"] {
        let mut cli = Command::cargo_bin("locus")?;
        cli.current_dir(directory.path())
            .arg(command)
            .arg(&source)
            .arg("--json");
        if command == "render" {
            cli.arg("-o").arg(&output_path);
        }
        let output = cli.output()?;
        assert_eq!(output.status.code(), Some(2));
        let report: Value = serde_json::from_slice(&output.stdout)?;
        let diagnostics = if command == "check" {
            &report["files"][0]["diagnostics"]
        } else {
            &report["diagnostics"]
        };
        assert_eq!(diagnostics[0]["code"], "E210");
        assert_eq!(diagnostics[0]["span"]["line"], 15);
        assert!(
            diagnostics[0]["message"]
                .as_str()
                .is_some_and(|s| s.contains("45deg"))
        );
        assert_eq!(fs::read_to_string(&output_path)?, "existing output");
    }
    Ok(())
}

#[test]
fn captured_source_with_a_gentler_bend_exports_a_normal_viewport() -> Result {
    let directory = tempfile::tempdir()?;
    let source = directory.path().join("figure.locus");
    fs::write(&source, CAPTURED.replace("bend: 90deg", "bend: 45deg"))?;
    Command::cargo_bin("locus")?
        .current_dir(directory.path())
        .arg("check")
        .arg(&source)
        .assert()
        .success();
    let output = Command::cargo_bin("locus")?
        .current_dir(directory.path())
        .arg("render")
        .arg(&source)
        .arg("--stdout")
        .output()?;
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let svg = String::from_utf8(output.stdout)?;
    for dimension in ["width", "height"] {
        let (_, tail) = svg
            .split_once(&format!("{dimension}=\""))
            .ok_or("missing dimension")?;
        let (value, _) = tail.split_once('"').ok_or("missing dimension terminator")?;
        let value: f64 = value.parse()?;
        assert!(value > 100.0 && value < 1000.0, "{dimension}: {value}");
    }
    Ok(())
}
