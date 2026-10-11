//! Layout checks and render diagnostics must share precise, actionable feedback.

use std::error::Error;
use std::fs;
use std::path::Path;
use std::process::Output;

use assert_cmd::Command;
use serde_json::Value;

type Result<T = ()> = std::result::Result<T, Box<dyn Error>>;

const LIFELINE: &str = "locus 0 beta 5\npoint A = point() { mark: none }\npoint B = offset(A, 0, -4) { mark: none }\nsegment life = seg(A, B)\npoint row = midpoint(A, B) { mark: none }\nlabel message = label(row, \"approve\") { dir: center }\n";

fn run(file: &Path, command: &str, args: &[&str]) -> Result<Output> {
    Ok(Command::cargo_bin("locus")?
        .arg(command)
        .arg(file)
        .args(args)
        .env("NO_COLOR", "1")
        .env(
            "LOCUS_REJECTED_REQUESTS_DIR",
            file.with_extension("requests"),
        )
        .output()?)
}

fn check(file: &Path, args: &[&str]) -> Result<Value> {
    let output = run(file, "check", args)?;
    Ok(serde_json::from_slice(&output.stdout)?)
}

#[test]
fn layout_check_names_lifeline_and_matches_render_in_full_and_compact_json() -> Result {
    let directory = tempfile::tempdir()?;
    let file = directory.path().join("sequence.locus");
    let svg = directory.path().join("sequence.svg");
    fs::write(&file, LIFELINE)?;
    assert_eq!(check(&file, &["--strict", "--json"])?["passed"], true);
    let full = check(&file, &["--layout", "--strict", "--json"])?;
    let compact = check(&file, &["--layout", "--strict", "--json", "--compact"])?;
    assert_eq!(full["passed"], false);
    assert_eq!(
        full["files"][0]["diagnostics"],
        compact["files"][0]["diagnostics"]
    );
    let diagnostic = &full["files"][0]["diagnostics"][0];
    assert_eq!(diagnostic["code"], "label_collision");
    assert!(
        diagnostic["message"]
            .as_str()
            .ok_or("message")?
            .contains("life")
    );
    assert_eq!(diagnostic["span"]["line"], 6);
    let details = diagnostic["collision_details"]
        .as_array()
        .ok_or("details")?;
    assert_eq!(details.len(), 1);
    assert_eq!(details[0]["type"], "stroke_intersection");
    assert_eq!(details[0]["object"], "life");
    assert_eq!(details[0]["label"], "message");
    assert_eq!(details[0]["span"]["line"], 4);
    let bounds = &details[0]["label_bounds"];
    let tested = &details[0]["tested_bounds"];
    assert!(tested["min_x"].as_f64().ok_or("min")? < bounds["min_x"].as_f64().ok_or("min")?);
    assert!(tested["max_x"].as_f64().ok_or("max")? > bounds["max_x"].as_f64().ok_or("max")?);
    assert!(details[0]["object_bounds"].is_object());
    let rendered = Command::cargo_bin("locus")?
        .arg("render")
        .arg(&file)
        .arg("-o")
        .arg(&svg)
        .arg("--json")
        .output()?;
    assert!(rendered.status.success());
    let render: Value = serde_json::from_slice(&rendered.stdout)?;
    assert_eq!(render["diagnostics"], full["files"][0]["diagnostics"]);
    let human = run(&file, "check", &["--layout", "--strict"])?;
    assert_eq!(human.status.code(), Some(1));
    let text = String::from_utf8(human.stderr)?;
    assert!(text.contains("life") && text.contains("stroke") && text.contains("bounds"));
    assert_eq!(run(&file, "check", &["--layout"])?.status.code(), Some(0));
    Ok(())
}

#[test]
fn overlapping_labels_and_margin_violations_have_distinct_types_and_bounds() -> Result {
    let directory = tempfile::tempdir()?;
    let file = directory.path().join("labels.locus");
    fs::write(
        &file,
        "locus 0 beta 5\npoint A = point() { mark: none }\nlabel first = label(A, \"one\") { dir: center }\nlabel second = label(A, \"two\") { dir: center }\nlabel outside = label(A, \"far\") { dir: N, gap: 10 }\n",
    )?;
    let report = check(&file, &["--layout", "--strict", "--json"])?;
    let diagnostics = report["files"][0]["diagnostics"]
        .as_array()
        .ok_or("diagnostics")?;
    assert_eq!(diagnostics.len(), 2);
    let overlap = &diagnostics[0]["collision_details"][0];
    assert_eq!(overlap["type"], "label_overlap");
    assert_eq!(overlap["labels"], serde_json::json!(["first", "second"]));
    assert_eq!(overlap["spans"][1]["line"], 4);
    assert_eq!(overlap["bounds"].as_array().ok_or("bounds")?.len(), 2);
    assert!(
        diagnostics[0]["message"]
            .as_str()
            .ok_or("message")?
            .contains("first, second")
    );
    let margin = &diagnostics[1]["collision_details"][0];
    assert_eq!(margin["type"], "margin_violation");
    assert!(
        margin["label_bounds"]["min_y"].as_f64().ok_or("min")?
            > margin["allowed_bounds"]["max_y"].as_f64().ok_or("max")?
    );
    Ok(())
}

#[test]
fn layout_checks_preserve_batch_results_and_report_layout_failures() -> Result {
    let directory = tempfile::tempdir()?;
    let good = directory.path().join("good.locus");
    let collision = directory.path().join("collision.locus");
    let invalid = directory.path().join("invalid.locus");
    fs::write(
        &good,
        "locus 0 beta 5\ncircle c = circle(2)\nlabel name = label(c.center, \"inside\") { dir: center }\n",
    )?;
    fs::write(&collision, LIFELINE)?;
    fs::write(
        &invalid,
        "locus 0 beta 5\npoint A = point()\nlabel bad = label(A, $\\unknowncommand$)\n",
    )?;
    let output = Command::cargo_bin("locus")?
        .arg("check")
        .arg(&good)
        .arg(&collision)
        .arg(&invalid)
        .args(["--layout", "--strict", "--json"])
        .env(
            "LOCUS_REJECTED_REQUESTS_DIR",
            directory.path().join("requests"),
        )
        .output()?;
    assert_eq!(output.status.code(), Some(2));
    let report: Value = serde_json::from_slice(&output.stdout)?;
    let files = report["files"].as_array().ok_or("files")?;
    assert_eq!(files.len(), 3);
    assert_eq!(files[0]["passed"], true);
    assert_eq!(files[1]["passed"], false);
    assert_eq!(files[2]["diagnostics"][0]["code"], "E414");
    assert!(!directory.path().join("good.svg").exists());
    Ok(())
}
