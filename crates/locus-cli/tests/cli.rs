//! End-to-end CLI contract and snapshot tests.

use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use assert_cmd::Command;

const DEMONSTRATIONS: &[&str] = &[
    "aec3_filter_alignment",
    "angle_bisector_ratio",
    "euler_nine_point",
    "flowchart_agent_loop",
    "inscribed_angle",
    "pentagon_golden",
    "sequence_check",
    "tangent_circles",
];

const ERRORS: &[&str] = &[
    "missing_header",
    "bad_version",
    "unknown_kind",
    "unknown_constructor",
    "duplicate_id",
    "forward_reference",
    "malformed_property",
    "division_by_zero",
];

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn locus() -> Result<Command, Box<dyn Error>> {
    let mut command = Command::cargo_bin("locus")?;
    command.current_dir(workspace_root());
    command.env("NO_COLOR", "1");
    Ok(command)
}

fn output_text(output: Vec<u8>) -> Result<String, Box<dyn Error>> {
    Ok(String::from_utf8(output)?)
}

#[test]
fn intended_agent_loop() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let figure = directory.path().join("agent-loop.locus");

    locus()?
        .args(["init", figure.to_str().ok_or("non-UTF-8 path")?])
        .assert()
        .success();
    locus()?
        .args(["add", figure.to_str().ok_or("non-UTF-8 path")?])
        .write_stdin("point A()\nassert dist(A, A) == 0\n")
        .assert()
        .success();
    let check = locus()?
        .args(["check", figure.to_str().ok_or("non-UTF-8 path")?, "--json"])
        .output()?;
    assert_eq!(check.status.code(), Some(0));
    let check_json: serde_json::Value = serde_json::from_slice(&check.stdout)?;
    assert_eq!(check_json["passed"], true);
    let render = locus()?
        .args([
            "render",
            figure.to_str().ok_or("non-UTF-8 path")?,
            "-o",
            directory
                .path()
                .join("agent-loop.svg")
                .to_str()
                .ok_or("non-UTF-8 path")?,
        ])
        .output()?;
    assert_eq!(render.status.code(), Some(0));
    assert!(fs::read_to_string(directory.path().join("agent-loop.svg"))?.starts_with("<svg"));
    Ok(())
}

#[test]
fn help_and_version_are_successful_displays() -> Result<(), Box<dyn Error>> {
    for argument in ["--help", "--version"] {
        let output = locus()?.arg(argument).output()?;
        assert_eq!(output.status.code(), Some(0), "{argument}");
        assert!(!output.stdout.is_empty(), "{argument}");
        assert!(output.stderr.is_empty(), "{argument}");
    }
    Ok(())
}

#[test]
fn usage_errors_honor_json_output() -> Result<(), Box<dyn Error>> {
    let output = locus()?.args(["--json", "check"]).output()?;
    assert_eq!(output.status.code(), Some(3));
    assert!(output.stderr.is_empty());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(value["command"], "usage");
    assert_eq!(value["diagnostics"][0]["code"], "usage");
    assert_eq!(value["diagnostics"][0]["severity"], "error");
    Ok(())
}

#[test]
fn solved_demonstration_reports_are_stable() -> Result<(), Box<dyn Error>> {
    for name in DEMONSTRATIONS {
        let fixture = format!("examples/demonstrations/{name}.locus");
        for command_name in ["describe", "inspect"] {
            let mut command = locus()?;
            command.args([command_name, &fixture, "--json"]);
            if command_name == "inspect" {
                command.args(["--measurements", "--incidences"]);
            }
            let output = command.output()?;
            assert_eq!(output.status.code(), Some(0), "{command_name} {name}");
            let stdout = output_text(output.stdout)?;
            insta::assert_snapshot!(format!("{name}_{command_name}_json"), stdout);
        }
    }
    Ok(())
}

#[test]
fn invalid_file_diagnostics_are_stable_in_both_formats() -> Result<(), Box<dyn Error>> {
    for name in ERRORS {
        let fixture = format!("tests/fixtures/errors/{name}.locus");
        let human = locus()?.args(["check", &fixture]).output()?;
        assert_ne!(human.status.code(), Some(0), "human {name}");
        insta::assert_snapshot!(format!("{name}_human"), output_text(human.stderr)?);

        let json = locus()?.args(["check", &fixture, "--json"]).output()?;
        assert_ne!(json.status.code(), Some(0), "json {name}");
        insta::assert_snapshot!(format!("{name}_json"), output_text(json.stdout)?);
    }
    Ok(())
}
