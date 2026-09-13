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
        .write_stdin("point A = free ~(0, 0)\nassert dist(A, A) == 0\n")
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
    assert_eq!(render.status.code(), Some(2));
    assert!(output_text(render.stderr)?.contains("render is not implemented in this build"));
    Ok(())
}

#[test]
fn exit_codes_match_the_specification() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let success = directory.path().join("success.locus");
    let assertion = directory.path().join("assertion.locus");
    let evaluation = directory.path().join("evaluation.locus");
    fs::write(&success, "locus 2\nnum n = 1\n")?;
    fs::write(&assertion, "locus 2\nnum n = 1\nassert n == 2\n")?;
    fs::write(&evaluation, "locus 2\nnum n = 1 / 0\n")?;

    locus()?
        .args(["check", success.to_str().ok_or("non-UTF-8 path")?])
        .assert()
        .code(0);
    locus()?
        .args(["check", assertion.to_str().ok_or("non-UTF-8 path")?])
        .assert()
        .code(1);
    locus()?
        .args(["check", evaluation.to_str().ok_or("non-UTF-8 path")?])
        .assert()
        .code(2);
    locus()?.arg("check").assert().code(3);
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
fn human_warnings_use_warning_severity() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let figure = directory.path().join("warning.locus");
    fs::write(&figure, "locus 2\npoint A = free ~(0, 0)\n")?;
    let output = locus()?.arg("inspect").arg(&figure).output()?;
    assert_eq!(output.status.code(), Some(0));
    let stderr = output_text(output.stderr)?;
    assert!(stderr.contains('⚠'));
    assert!(!stderr.contains('×'));
    Ok(())
}

#[test]
fn update_and_remove_preserve_dependency_safety() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let figure = directory.path().join("edits.locus");
    fs::write(
        &figure,
        "locus 2\npoint A = free ~(0, 0)\npoint B = free ~(1, 0)\nsegment AB = seg(A, B)\n",
    )?;

    locus()?
        .args([
            "update",
            figure.to_str().ok_or("non-UTF-8 path")?,
            "--id",
            "A",
            "--def",
            "free ~(2, 0)",
        ])
        .assert()
        .success();
    locus()?
        .args([
            "update",
            figure.to_str().ok_or("non-UTF-8 path")?,
            "--id",
            "A",
            "--set",
            "mark=cross",
        ])
        .assert()
        .success();
    let edited = fs::read_to_string(&figure)?;
    assert!(edited.contains("point A = free ~(2, 0) { mark: cross }"));

    locus()?
        .args([
            "remove",
            figure.to_str().ok_or("non-UTF-8 path")?,
            "--id",
            "A",
        ])
        .assert()
        .code(2);
    locus()?
        .args([
            "remove",
            figure.to_str().ok_or("non-UTF-8 path")?,
            "--id",
            "AB",
        ])
        .assert()
        .success();
    locus()?
        .args([
            "remove",
            figure.to_str().ok_or("non-UTF-8 path")?,
            "--id",
            "A",
        ])
        .assert()
        .success();
    Ok(())
}

#[test]
fn update_rejects_fragments_that_escape_their_target() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let figure = directory.path().join("surgical-update.locus");
    let source = concat!(
        "locus 2\n",
        "point A = free ~(0, 0) where { pin(A, 0, 0) } [main] { mark: dot } // keep\n",
    );
    fs::write(&figure, source)?;

    for (flag, fragment) in [
        ("--def", "free ~(1, 1) //"),
        ("--def", "free ~(1, 1)\nnum injected = 9"),
        ("--set", "mark=dot, dir: N"),
    ] {
        let output = locus()?
            .arg("update")
            .arg(&figure)
            .args(["--id", "A", flag, fragment, "--json"])
            .output()?;
        assert_eq!(output.status.code(), Some(3), "{flag} {fragment}");
        let value: serde_json::Value = serde_json::from_slice(&output.stdout)?;
        assert_eq!(value["diagnostics"][0]["code"], "usage");
        assert_eq!(fs::read_to_string(&figure)?, source);
    }
    Ok(())
}

#[test]
fn update_accepts_one_complex_fragment_and_preserves_surrounding_source()
-> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let point = directory.path().join("point.locus");
    fs::write(
        &point,
        concat!(
            "locus 2\n",
            "point A = free ~(0, 0) where { pin(A, 3, 2) } [main] { mark: dot } // keep\n",
        ),
    )?;
    locus()?
        .arg("update")
        .arg(&point)
        .args(["--id", "A", "--def", "free ~(1 + 2, 4 / 2)"])
        .assert()
        .success();
    assert_eq!(
        fs::read_to_string(&point)?,
        concat!(
            "locus 2\n",
            "point A = free ~(1 + 2, 4 / 2) where { pin(A, 3, 2) } [main] { mark: dot } // keep\n",
        )
    );

    let text = directory.path().join("text.locus");
    fs::write(
        &text,
        "locus 2\npoint A = free ~(0, 0)\ntext_block note = text_block(\"old\")\n",
    )?;
    locus()?
        .arg("update")
        .arg(&text)
        .args([
            "--id",
            "note",
            "--def",
            "text_block(\"a, b = c // literal\")",
        ])
        .assert()
        .success();
    locus()?
        .arg("update")
        .arg(&text)
        .args(["--id", "note", "--set", "anchor=offset(A, 1, 2)"])
        .assert()
        .success();
    let edited = fs::read_to_string(&text)?;
    assert!(edited.contains("text_block(\"a, b = c // literal\")"));
    assert!(edited.contains("{ anchor: offset(A, 1, 2) }"));

    let referenced = directory.path().join("referenced.locus");
    fs::write(
        &referenced,
        concat!(
            "locus 2\n",
            "point A = free ~(0, 0)\n",
            "point B = free ~(2, 0)\n",
            "point M = free ~(1, 0)\n",
        ),
    )?;
    locus()?
        .arg("update")
        .arg(&referenced)
        .args(["--id", "M", "--def", "midpoint(A, B)"])
        .assert()
        .success();
    assert!(fs::read_to_string(&referenced)?.contains("point M = midpoint(A, B)"));
    Ok(())
}

#[test]
fn update_accepts_hyphen_leading_definition_values() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let figure = directory.path().join("negative.locus");
    let source = "locus 2\nnum a = 1\nnum b = 2\n";

    for (definition, expected) in [("-1", -1.0), ("-a", -1.0), ("-(a + 1)", -2.0)] {
        fs::write(&figure, source)?;
        locus()?
            .arg("update")
            .arg(&figure)
            .args(["--id", "b", "--def", definition])
            .assert()
            .success();
        let describe = locus()?
            .args([
                "describe",
                figure.to_str().ok_or("non-UTF-8 path")?,
                "--json",
            ])
            .output()?;
        assert_eq!(describe.status.code(), Some(0));
        let value: serde_json::Value = serde_json::from_slice(&describe.stdout)?;
        assert_eq!(value["objects"][1]["resolved"]["value"], expected);
    }
    Ok(())
}

#[test]
fn remove_rejects_relation_and_assertion_dependents_as_dependency_errors()
-> Result<(), Box<dyn Error>> {
    for (name, dependent) in [
        ("relation", "rel pin(A, 0, 0)"),
        ("assertion", "assert dist(A, A) == 0"),
    ] {
        let directory = tempfile::tempdir()?;
        let figure = directory.path().join(format!("{name}.locus"));
        let source = format!("locus 2\npoint A = free ~(0, 0)\n{dependent}\n");
        fs::write(&figure, &source)?;
        let output = locus()?
            .arg("remove")
            .arg(&figure)
            .args(["--id", "A", "--json"])
            .output()?;
        assert_eq!(output.status.code(), Some(2), "{name}");
        let value: serde_json::Value = serde_json::from_slice(&output.stdout)?;
        assert_eq!(value["diagnostics"][0]["code"], "E303", "{name}");
        assert_eq!(fs::read_to_string(&figure)?, source, "{name}");
    }
    Ok(())
}

#[test]
fn human_reports_include_resolved_geometry_measurements_and_incidences()
-> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let figure = directory.path().join("report.locus");
    fs::write(
        &figure,
        concat!(
            "locus 2\n",
            "point A = free ~(0, 0)\n",
            "point B = free ~(3, 4)\n",
            "segment AB = seg(A, B)\n",
        ),
    )?;

    let describe = locus()?.arg("describe").arg(&figure).output()?;
    assert_eq!(describe.status.code(), Some(0));
    let describe = output_text(describe.stdout)?;
    assert!(describe.contains("segment AB @4:1"));
    assert!(describe.contains("start (0.000000, 0.000000), end (3.000000, 4.000000)"));

    let inspect = locus()?.arg("inspect").arg(&figure).output()?;
    assert_eq!(inspect.status.code(), Some(0));
    let inspect = output_text(inspect.stdout)?;
    assert!(inspect.contains("distance A@2:7--B@3:7 = 5.000000"));
    assert!(inspect.contains("incidence A@2:7 on AB@4:9"));
    assert!(inspect.contains("incidence B@3:7 on AB@4:9"));
    Ok(())
}

#[test]
fn describe_retains_anchored_text_block_placement() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let figure = directory.path().join("anchored-text.locus");
    fs::write(
        &figure,
        concat!(
            "locus 2\n",
            "point A = free ~(3, 4)\n",
            "text_block note = text_block(\"hello\") { anchor: A, dir: NE }\n",
        ),
    )?;

    let json = locus()?
        .args([
            "describe",
            figure.to_str().ok_or("non-UTF-8 path")?,
            "--json",
        ])
        .output()?;
    assert_eq!(json.status.code(), Some(0));
    let value: serde_json::Value = serde_json::from_slice(&json.stdout)?;
    let placement = &value["objects"][1]["resolved"]["value"]["placement"];
    assert_eq!(placement["kind"], "anchor");
    assert_eq!(placement["anchor"]["kind"], "point");
    assert_eq!(placement["anchor"]["point"]["x"], 3.0);
    assert_eq!(placement["anchor"]["point"]["y"], 4.0);
    assert_eq!(placement["direction"], "ne");

    let human = locus()?.arg("describe").arg(&figure).output()?;
    assert_eq!(human.status.code(), Some(0));
    let human = output_text(human.stdout)?;
    assert!(human.contains("text=\"hello\""));
    assert!(human.contains("anchor (3.000000, 4.000000), direction ne"));
    Ok(())
}

#[test]
fn check_reports_every_input_with_deterministic_exit_precedence() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let assertion = directory.path().join("failed-assert.locus");
    let evaluation = directory.path().join("evaluation-error.locus");
    let parse_error = directory.path().join("parse-error.locus");
    let good = directory.path().join("good.locus");
    fs::write(&assertion, "locus 2\nnum n = 1\nassert n == 2\n")?;
    fs::write(&evaluation, "locus 2\nnum broken = 1 / 0\n")?;
    fs::write(&parse_error, "num broken = 1\n")?;
    fs::write(&good, "locus 2\nnum ok = 1\n")?;

    let json = locus()?
        .arg("check")
        .args([&assertion, &parse_error, &good])
        .arg("--json")
        .output()?;
    assert_eq!(json.status.code(), Some(3));
    let value: serde_json::Value = serde_json::from_slice(&json.stdout)?;
    let files = value["files"].as_array().ok_or("missing files")?;
    assert_eq!(files.len(), 3);
    assert_eq!(files[0]["path"], assertion.to_string_lossy().as_ref());
    assert_eq!(files[0]["assertions"]["passed"], false);
    assert!(
        !files[0]["assertions"]["assertions"][0]["actuals"]
            .as_array()
            .ok_or("missing assertion actuals")?
            .is_empty()
    );
    assert_eq!(files[1]["path"], parse_error.to_string_lossy().as_ref());
    assert_eq!(files[1]["diagnostics"][0]["code"], "E110");
    assert_eq!(files[2]["path"], good.to_string_lossy().as_ref());
    assert_eq!(files[2]["passed"], true);

    let human = locus()?
        .arg("check")
        .args([&assertion, &parse_error, &good])
        .output()?;
    assert_eq!(human.status.code(), Some(3));
    let stdout = output_text(human.stdout)?;
    for path in [&assertion, &parse_error, &good] {
        assert!(stdout.contains(path.to_string_lossy().as_ref()));
    }
    let stderr = output_text(human.stderr)?;
    assert!(stderr.contains("failed-assert.locus"));
    assert!(stderr.contains("parse-error.locus"));

    let evaluation_precedence = locus()?
        .arg("check")
        .args([&assertion, &evaluation, &good])
        .arg("--json")
        .output()?;
    assert_eq!(evaluation_precedence.status.code(), Some(2));
    let value: serde_json::Value = serde_json::from_slice(&evaluation_precedence.stdout)?;
    assert_eq!(value["files"].as_array().ok_or("missing files")?.len(), 3);
    assert_eq!(value["files"][0]["assertions"]["passed"], false);
    assert_eq!(value["files"][1]["diagnostics"][0]["code"], "E204");
    assert_eq!(value["files"][2]["passed"], true);
    Ok(())
}

#[test]
fn solved_demonstration_reports_are_stable() -> Result<(), Box<dyn Error>> {
    for name in DEMONSTRATIONS {
        let fixture = format!("spec/test/demonstrations/{name}.locus");
        for command_name in ["describe", "inspect"] {
            let output = locus()?.args([command_name, &fixture, "--json"]).output()?;
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
        let fixture = format!("fixtures/errors/{name}.locus");
        let human = locus()?.args(["check", &fixture]).output()?;
        assert_ne!(human.status.code(), Some(0), "human {name}");
        insta::assert_snapshot!(format!("{name}_human"), output_text(human.stderr)?);

        let json = locus()?.args(["check", &fixture, "--json"]).output()?;
        assert_ne!(json.status.code(), Some(0), "json {name}");
        insta::assert_snapshot!(format!("{name}_json"), output_text(json.stdout)?);
    }
    Ok(())
}
