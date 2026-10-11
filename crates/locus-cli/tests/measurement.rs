//! Measurement happens before solving, including ordinary checks and expansion.

use std::error::Error;
use std::fs;
use std::path::Path;
use std::process::Output;

use assert_cmd::Command;
use serde_json::Value;

type Result<T = ()> = std::result::Result<T, Box<dyn Error>>;

fn check(path: &Path, extra: &[&str]) -> Result<Output> {
    Ok(Command::cargo_bin("locus")?
        .arg("check")
        .arg(path)
        .arg("--json")
        .args(extra)
        .env(
            "LOCUS_REJECTED_REQUESTS_DIR",
            path.with_extension("requests"),
        )
        .output()?)
}

#[test]
fn ordinary_check_measures_tex_and_honors_the_requested_engine() -> Result {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("invalid-tex.locus");
    fs::write(
        &path,
        "locus 0 beta 6\npoint A()\nlabel bad(A, $\\unknowncommand$)\n",
    )?;
    let output = check(&path, &[])?;
    assert_eq!(output.status.code(), Some(2));
    let report: Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(report["files"][0]["diagnostics"][0]["code"], "E414");
    assert_eq!(report["files"][0]["diagnostics"][0]["span"]["line"], 3);
    assert!(check(&path, &["--tex-engine", "mock"])?.status.success());
    Ok(())
}

#[test]
fn measured_roles_and_expanded_ids_are_available_to_solve_and_assertions() -> Result {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("roles.locus");
    fs::write(
        &path,
        "locus 0 beta 6\n\
fn card(text) {\n\
    rect box()\n\
    label caption(text)\n\
    rel center(caption, box)\n\
    rel box.width == 4\n\
    rel box.height == 2\n\
}\n\
card first(\"Ready\")\n\
card second(\"A longer caption\")\n\
label heading(\"Ready\") { role: title }\n\
rel center(heading, first.box)\n\
assert heading.width > first.caption.width\n\
assert second.caption.width > first.caption.width\n\
assert center(first.caption, first.box)\n\
assert center(second.caption, second.box)\n",
    )?;
    let output = check(&path, &[])?;
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    let report: Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(report["passed"], true);
    let svg = directory.path().join("roles.svg");
    Command::cargo_bin("locus")?
        .arg("render")
        .arg(&path)
        .arg("-o")
        .arg(&svg)
        .assert()
        .success();
    let rendered = fs::read_to_string(svg)?;
    assert!(rendered.contains("<path"));
    Ok(())
}

#[test]
fn ordinary_geometry_checks_do_not_require_text_inputs() -> Result {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("geometry.locus");
    fs::write(
        &path,
        "locus 0 beta 6\ncircle shape(2)\nassert radius(shape) == 2\n",
    )?;
    assert!(check(&path, &[])?.status.success());
    Ok(())
}
