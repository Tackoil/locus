//! Every consumer evaluates accepted logical routing and retained visible labels.
use assert_cmd::Command;
use serde_json::Value;
use std::{error::Error, fs, path::Path};

type Result<T = ()> = std::result::Result<T, Box<dyn Error>>;

fn check(path: &Path) -> Result<std::process::Output> {
    Ok(Command::cargo_bin("locus")?
        .args(["check"])
        .arg(path)
        .arg("--json")
        .env(
            "LOCUS_REJECTED_REQUESTS_DIR",
            path.with_extension("requests"),
        )
        .output()?)
}

#[test]
fn ordinary_check_observes_accepted_route_clearance_and_length() -> Result {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("routing.locus");
    fs::write(
        &path,
        "locus 0 beta 6\npoint start()\npoint finish = on_circle(circle(start, 4), 90deg)\npoint middle = on_circle(circle(start, 2), 90deg)\nrect obstacle(middle, 2.5, 1)\narrow path(start, finish) { route: orthogonal, side: right }\nrel avoid(path, obstacle)\nassert avoid(path, obstacle)\nassert path.length >= 8\n",
    )?;
    let output = check(&path)?;
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    let report: Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(report["files"][0]["assertions"]["passed"], true);
    assert!(
        report["files"][0]["solve"]["measurement_passes"]
            .as_u64()
            .is_some_and(|passes| (2..=3).contains(&passes))
    );
    assert!(
        report["files"][0]["solve"]["routing_attempts"]
            .as_u64()
            .is_some_and(|attempts| attempts > 0)
    );
    for backend in [Vec::new(), vec!["--tikz"]] {
        Command::cargo_bin("locus")?
            .arg("render")
            .arg(&path)
            .args(backend)
            .arg("--stdout")
            .assert()
            .success();
    }
    Ok(())
}

#[test]
fn attached_endpoint_contacts_pass_the_same_avoidance_assertion() -> Result {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("attachment.locus");
    fs::write(
        &path,
        "locus 0 beta 6\npoint center()\nrect first(center, 2, 2)\npoint second_center = on_circle(circle(center, 4), 90deg)\nrect second(second_center, 2, 2)\narrow path(first.e, second.e) { route: orthogonal, side: right }\nrel avoid(path, first, second)\nassert avoid(path, first, second)\n",
    )?;
    let output = check(&path)?;
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    Ok(())
}

#[test]
fn required_clipping_failure_blocks_check_and_preserves_render_output() -> Result {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("enclosed.locus");
    fs::write(
        &path,
        "locus 0 beta 6\npoint start()\npoint end = on_circle(circle(start, 0.5), 0deg)\nrect boundary(start, 2, 2)\narrow path(start, end)\nrel trim_endpoints(path, boundary)\n",
    )?;
    let output = check(&path)?;
    assert_eq!(output.status.code(), Some(2));
    let report: Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(report["files"][0]["diagnostics"][0]["code"], "E413");
    let svg = directory.path().join("existing.svg");
    fs::write(&svg, "preserve")?;
    Command::cargo_bin("locus")?
        .arg("render")
        .arg(&path)
        .arg("--output")
        .arg(&svg)
        .assert()
        .code(2);
    assert_eq!(fs::read_to_string(svg)?, "preserve");
    Ok(())
}
