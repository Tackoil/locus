//! Timings are opt-in and preserve command output and exit behavior.

use std::error::Error;
use std::fs;
use std::path::Path;
use std::process::Output;

use assert_cmd::Command;
use serde_json::Value;
use tempfile::TempDir;

type Result<T = ()> = std::result::Result<T, Box<dyn Error>>;

fn fixture() -> Result<TempDir> {
    let directory = tempfile::tempdir()?;
    fs::write(
        directory.path().join("scene.locus"),
        "locus 0 beta 6\ncircle shape(2)\nlabel caption(shape.center, \"Circle\")\nassert radius(shape) == 2\n",
    )?;
    Ok(directory)
}

fn invoke(directory: &Path, args: &[&str]) -> Result<Output> {
    Ok(Command::cargo_bin("locus")?
        .current_dir(directory)
        .args(args)
        .output()?)
}

fn names(report: &Value) -> Vec<&str> {
    report["steps"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|step| step["name"].as_str())
        .collect()
}

#[test]
fn json_timings_preserve_normal_and_compact_stdout() -> Result {
    let directory = fixture()?;
    for output_flag in ["--json", "--compact"] {
        let mut args = vec!["check", "scene.locus", "--json"];
        if output_flag == "--compact" {
            args.push(output_flag);
        }
        let plain = invoke(directory.path(), &args)?;
        args.push("--timings=json");
        let timed = invoke(directory.path(), &args)?;
        assert!(timed.status.success(), "{timed:?}");
        assert!(plain.stderr.is_empty());
        assert_eq!(plain.stdout, timed.stdout);
        let report: Value = serde_json::from_slice(&timed.stderr)?;
        let steps = names(&report);
        for expected in [
            "command",
            "scene.locus",
            "read_source",
            "parse",
            "measure_text",
            "solve_layout",
            "measurement_pass",
            "evaluate_scene",
            "prepare_evaluation",
            "solve",
            "initialize",
            "solve_candidate",
            "levenberg_marquardt",
            "settle_preferences",
            "finalize_solution",
            "discrete_layout",
            "assertions",
            "output",
        ] {
            assert!(steps.contains(&expected), "missing {expected}: {report}");
        }
        assert!(report["total_ms"].as_f64().is_some_and(|ms| ms >= 0.0));
    }
    Ok(())
}

#[test]
fn human_timings_leave_rendered_stdout_unchanged() -> Result {
    let directory = fixture()?;
    let args = ["render", "scene.locus", "--stdout", "--tex-engine", "mock"];
    let plain = invoke(directory.path(), &args)?;
    let mut timed_args = args.to_vec();
    timed_args.push("--timings");
    let timed = invoke(directory.path(), &timed_args)?;
    assert!(timed.status.success(), "{timed:?}");
    assert_eq!(timed.stdout, plain.stdout);
    let stderr = String::from_utf8(timed.stderr)?;
    assert!(stderr.contains("Timings (inclusive wall time):"));
    assert!(stderr.contains("render_svg:"));
    assert!(stderr.contains("total:"));
    Ok(())
}

#[test]
fn failed_batch_keeps_per_file_steps_and_exit_code() -> Result {
    let directory = fixture()?;
    fs::write(directory.path().join("invalid.locus"), "invalid source\n")?;
    let args = ["check", "scene.locus", "invalid.locus", "--json"];
    let plain = invoke(directory.path(), &args)?;
    let mut timed_args = args.to_vec();
    timed_args.push("--timings=json");
    let timed = invoke(directory.path(), &timed_args)?;
    assert_eq!(timed.status.code(), Some(3));
    assert_eq!(plain.status.code(), timed.status.code());
    assert_eq!(plain.stdout, timed.stdout);
    let report: Value = serde_json::from_slice(&timed.stderr)?;
    let steps = report["steps"].as_array().ok_or("missing steps")?;
    let invalid_index = steps
        .iter()
        .position(|step| step["name"] == "invalid.locus")
        .ok_or("missing invalid file scope")?;
    assert!(
        steps
            .iter()
            .any(|step| step["name"] == "parse" && step["parent"] == invalid_index)
    );
    assert!(names(&report).contains(&"scene.locus"));
    Ok(())
}

#[test]
fn render_to_file_reports_delivery_and_allows_global_flag_first() -> Result {
    let directory = fixture()?;
    let output = invoke(
        directory.path(),
        &[
            "--timings=json",
            "render",
            "scene.locus",
            "-o",
            "scene.svg",
            "--json",
        ],
    )?;
    assert!(output.status.success(), "{output:?}");
    let report: Value = serde_json::from_slice(&output.stderr)?;
    assert!(names(&report).contains(&"write_output"));
    assert!(fs::read_to_string(directory.path().join("scene.svg"))?.contains("<svg"));
    Ok(())
}

#[test]
fn failed_read_is_timed_and_formatting_is_supported() -> Result {
    let directory = fixture()?;
    let failed = invoke(
        directory.path(),
        &["describe", "missing.locus", "--json", "--timings=json"],
    )?;
    assert!(!failed.status.success());
    let report: Value = serde_json::from_slice(&failed.stderr)?;
    assert!(names(&report).contains(&"read_source"));
    assert!(!names(&report).contains(&"parse"));
    let formatted = invoke(directory.path(), &["fmt", "scene.locus", "--timings=json"])?;
    assert!(formatted.status.success());
    let report: Value = serde_json::from_slice(&formatted.stderr)?;
    assert!(names(&report).contains(&"format"));
    Ok(())
}

#[test]
fn evaluation_and_assertion_failures_still_produce_metrics() -> Result {
    let directory = fixture()?;
    for (source, status, expected_stage) in [
        (
            "locus 0 beta 6\npoint A()\nlabel bad(A, $\\unknowncommand$)\n",
            2,
            "measure_text",
        ),
        (
            "locus 0 beta 6\ncircle shape(2)\nrel radius(shape) == 3\n",
            2,
            "solve_candidate",
        ),
        (
            "locus 0 beta 6\ncircle shape(2)\nassert radius(shape) == 3\n",
            1,
            "assertions",
        ),
    ] {
        fs::write(directory.path().join("failure.locus"), source)?;
        let args = ["check", "failure.locus", "--json"];
        let plain = invoke(directory.path(), &args)?;
        let mut timed_args = args.to_vec();
        timed_args.push("--timings=json");
        let timed = invoke(directory.path(), &timed_args)?;
        assert_eq!(timed.status.code(), Some(status), "{timed:?}");
        assert_eq!(timed.stdout, plain.stdout);
        assert_eq!(timed.status.code(), plain.status.code());
        let report: Value = serde_json::from_slice(&timed.stderr)?;
        assert!(names(&report).contains(&expected_stage));
    }
    Ok(())
}

#[test]
fn repeated_layout_passes_are_reported_individually() -> Result {
    let directory = fixture()?;
    fs::write(
        directory.path().join("routing.locus"),
        "locus 0 beta 6\n\
point start()\n\
point finish = offset(start, 0, 4)\n\
point middle = offset(start, 0, 2)\n\
rect obstacle(middle, 2.5, 1)\n\
arrow path(start, finish) { route: orthogonal, side: right }\n\
rel avoid(path, obstacle)\n\
assert avoid(path, obstacle)\n",
    )?;
    let timed = invoke(
        directory.path(),
        &["check", "routing.locus", "--json", "--timings=json"],
    )?;
    assert!(timed.status.success(), "{timed:?}");
    let result: Value = serde_json::from_slice(&timed.stdout)?;
    let report: Value = serde_json::from_slice(&timed.stderr)?;
    let passes = names(&report)
        .iter()
        .filter(|name| **name == "measurement_pass")
        .count();
    assert!(passes > 1);
    assert_eq!(result["files"][0]["solve"]["measurement_passes"], passes);
    Ok(())
}
