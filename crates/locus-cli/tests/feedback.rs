//! Regression coverage for bounded feedback and direct inspection queries.

use std::error::Error;
use std::fmt::Write as _;
use std::fs;
use std::path::Path;
use std::time::Duration;

use assert_cmd::Command;
use serde_json::{Value, json};

type Result<T = ()> = std::result::Result<T, Box<dyn Error>>;

fn command(name: &str, file: &Path, args: &[&str]) -> Result<std::process::Output> {
    Ok(Command::cargo_bin("locus")?
        .arg(name)
        .arg(file)
        .args(args)
        .arg("--json")
        .env(
            "LOCUS_REJECTED_REQUESTS_DIR",
            file.parent().ok_or("missing parent")?.join("requests"),
        )
        .timeout(Duration::from_secs(10))
        .output()?)
}

fn fixture(file: &Path) -> Result {
    fs::write(
        file,
        concat!(
            "locus 0 beta 5\n",
            "point A = point() { mark: none }\n",
            "point B = offset(A, 4, 0) { mark: none }\n",
            "point C = offset(B, 0, 3) { mark: none }\n",
            "point M = midpoint(A, B) { mark: none }\n",
            "segment AB = seg(A, B)\n",
        ),
    )?;
    Ok(())
}

#[test]
fn helper_heavy_scenes_have_small_default_feedback_and_pass_strict_check() -> Result {
    let directory = tempfile::tempdir()?;
    let file = directory.path().join("helpers.locus");
    let mut source = "locus 0 beta 5\npoint A = point()\n".to_owned();
    for index in 0..200 {
        writeln!(source, "point H{index} = offset(A, 0, 0) {{ mark: none }}")?;
    }
    source.push_str("point far = offset(A, 100, 100) { mark: none }\n");
    fs::write(&file, source)?;
    let inspect = command("inspect", &file, &["--compact"])?;
    assert_eq!(inspect.status.code(), Some(0));
    assert!(inspect.stdout.len() < 512);
    let report: Value = serde_json::from_slice(&inspect.stdout)?;
    assert_eq!(report["diagnostics"], json!([]));
    assert!(report.get("measurements").is_none());
    assert!(report.get("incidences").is_none());

    let check = command("check", &file, &["--strict", "--compact"])?;
    assert_eq!(check.status.code(), Some(0));
    assert!(check.stdout.len() < 1024);
    let report: Value = serde_json::from_slice(&check.stdout)?;
    assert_eq!(report["passed"], true);
    assert_eq!(report["files"][0]["diagnostics"], json!([]));
    Ok(())
}

#[test]
fn direct_queries_preserve_order_and_only_return_requested_measurements() -> Result {
    let directory = tempfile::tempdir()?;
    let file = directory.path().join("queries.locus");
    fixture(&file)?;
    let output = command(
        "inspect",
        &file,
        &[
            "--distance",
            "A",
            "C",
            "--distance",
            "C",
            "B",
            "--angle",
            "A",
            "B",
            "C",
            "--ratio",
            "B",
            "M",
            "A",
            "--compact",
        ],
    )?;
    assert_eq!(output.status.code(), Some(0));
    let report: Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(
        report["measurements"]["distances"],
        json!([
            {"first": "A", "second": "C", "distance": 5.0},
            {"first": "C", "second": "B", "distance": 3.0},
        ])
    );
    assert_eq!(
        report["measurements"]["triangle_angles"]
            .as_array()
            .ok_or("angles")?
            .len(),
        1
    );
    assert_eq!(report["measurements"]["triangle_angles"][0]["vertex"], "B");
    let angle = report["measurements"]["triangle_angles"][0]["radians"]
        .as_f64()
        .ok_or("radians")?;
    assert!((angle - std::f64::consts::FRAC_PI_2).abs() < 1e-12);
    assert_eq!(
        report["measurements"]["line_ratios"]
            .as_array()
            .ok_or("ratios")?
            .len(),
        1
    );
    assert_eq!(report["measurements"]["line_ratios"][0]["start"], "B");
    assert_eq!(
        report["measurements"]["line_ratios"][0]["integer_ratio"],
        json!({"left": 1, "right": 1})
    );
    assert!(report.get("incidences").is_none());
    Ok(())
}

#[test]
fn exhaustive_tables_and_incidences_are_independent_opt_ins() -> Result {
    let directory = tempfile::tempdir()?;
    let file = directory.path().join("opt-in.locus");
    fixture(&file)?;
    let default: Value = serde_json::from_slice(&command("inspect", &file, &[])?.stdout)?;
    assert_eq!(default["measurements"]["distances"], json!([]));
    assert_eq!(default["incidences"], json!([]));
    let tables: Value =
        serde_json::from_slice(&command("inspect", &file, &["--measurements"])?.stdout)?;
    assert_eq!(
        tables["measurements"]["distances"]
            .as_array()
            .ok_or("distances")?
            .len(),
        6
    );
    assert_eq!(tables["incidences"], json!([]));
    let facts: Value =
        serde_json::from_slice(&command("inspect", &file, &["--incidences"])?.stdout)?;
    assert_eq!(facts["measurements"]["distances"], json!([]));
    assert!(
        facts["incidences"]
            .as_array()
            .ok_or("incidences")?
            .iter()
            .any(|item| {
                item["kind"] == "point_on_object" && item["point"] == "M" && item["object"] == "AB"
            })
    );
    Ok(())
}

#[test]
fn invalid_queries_return_structured_usage_errors() -> Result {
    let directory = tempfile::tempdir()?;
    let file = directory.path().join("invalid-query.locus");
    fixture(&file)?;
    for args in [
        vec!["--distance", "A", "missing"],
        vec!["--distance", "A", "AB"],
        vec!["--angle", "A", "A", "B"],
        vec!["--ratio", "A", "C", "B"],
        vec!["--ratio", "A", "B", "M"],
    ] {
        let output = command("inspect", &file, &args)?;
        assert_eq!(output.status.code(), Some(3), "{args:?}");
        let report: Value = serde_json::from_slice(&output.stdout)?;
        assert_eq!(report["diagnostics"][0]["code"], "inspect_query");
        assert_eq!(report["diagnostics"][0]["severity"], "error");
    }
    for args in [
        vec!["--angle", "A", "B"],
        vec!["--distance", "A", "B", "--measurements"],
    ] {
        let output = command("inspect", &file, &args)?;
        assert_eq!(output.status.code(), Some(3));
        let report: Value = serde_json::from_slice(&output.stdout)?;
        assert_eq!(report["diagnostics"][0]["code"], "usage");
    }
    Ok(())
}

#[test]
fn compact_checks_summarize_successes_and_retain_every_failure_actual() -> Result {
    let directory = tempfile::tempdir()?;
    let file = directory.path().join("assertions.locus");
    fs::write(
        &file,
        "locus 0 beta 5\nnum n = 1\nassert n == 1\nassert n == 2\nassert n > 0\n",
    )?;
    let full = command("check", &file, &[])?;
    let compact = command("check", &file, &["--compact"])?;
    assert_eq!(full.status.code(), Some(1));
    assert_eq!(compact.status.code(), Some(1));
    assert_eq!(std::str::from_utf8(&compact.stdout)?.lines().count(), 1);
    let full: Value = serde_json::from_slice(&full.stdout)?;
    let compact: Value = serde_json::from_slice(&compact.stdout)?;
    let report = &compact["files"][0]["assertions"];
    assert_eq!(report["total"], 3);
    assert_eq!(report["failed"], 1);
    assert_eq!(report["passed"], false);
    assert_eq!(
        report["assertions"],
        json!([full["files"][0]["assertions"]["assertions"][1]])
    );
    assert_eq!(
        compact["files"][0]["diagnostics"],
        full["files"][0]["diagnostics"]
    );
    assert_eq!(
        compact["files"][0]["solve"]["rank"],
        full["files"][0]["solve"]["rank"]
    );
    assert_eq!(
        compact["files"][0]["solve"]["global_remaining"],
        full["files"][0]["solve"]["dof"]["global_remaining"]
    );
    assert!(
        compact["files"][0]["solve"]
            .get("relation_residuals")
            .is_none()
    );
    Ok(())
}

#[test]
fn compact_checks_retain_visible_warnings_and_evaluation_errors() -> Result {
    let directory = tempfile::tempdir()?;
    let file = directory.path().join("warnings.locus");
    fs::write(
        &file,
        "locus 0 beta 5\npoint A = point()\npoint B = offset(A, 0, 0)\n",
    )?;
    let output = command("check", &file, &["--strict", "--compact"])?;
    assert_eq!(output.status.code(), Some(1));
    let report: Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(report["files"][0]["diagnostics"][0]["code"], "crowding");
    fs::write(&file, "locus 0 beta 5\nnum n = 1 / 0\n")?;
    let output = command("check", &file, &["--compact"])?;
    assert_eq!(output.status.code(), Some(2));
    let report: Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(report["files"][0]["diagnostics"][0]["code"], "E204");
    assert!(report["files"][0].get("assertions").is_none());
    Ok(())
}
