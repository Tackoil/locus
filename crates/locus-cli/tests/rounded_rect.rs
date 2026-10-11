//! End-to-end rounded rectangle checking, inspection, and export.
use assert_cmd::Command;
use serde_json::Value;
use std::error::Error;
use std::path::Path;

const SOURCE: &str = include_str!("../../../spec/test/demonstrations/rounded_rect.locus");

fn output(file: &Path, verb: &str, args: &[&str]) -> Result<Vec<u8>, Box<dyn Error>> {
    let result = Command::cargo_bin("locus")?
        .arg(verb)
        .arg(file)
        .args(args)
        .output()?;
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    Ok(result.stdout)
}

#[test]
fn rounded_rect_check_describe_and_vector_exports() -> Result<(), Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let file = dir.path().join("rounded.locus");
    std::fs::write(&file, SOURCE)?;
    output(&file, "check", &["--strict", "--layout"])?;
    let report: Value = serde_json::from_slice(&output(&file, "describe", &["--json"])?)?;
    let objects = report["objects"].as_array().ok_or("missing objects")?;
    let start = objects
        .iter()
        .find(|v| v["id"] == "start")
        .ok_or("missing start")?;
    assert_eq!(start["kind"], "rounded_rect");
    assert_eq!(start["resolved"]["corner_radius"], 0.5);
    assert_eq!(start["resolved"]["width"], 4.0);
    assert_eq!(start["resolved"]["height"], 2.0);
    let svg = String::from_utf8(output(&file, "render", &["--stdout"])?)?;
    assert_eq!(svg.matches("<rect ").count(), 2);
    assert!(svg.contains("rx=\"16.000000\""));
    assert!(svg.contains("rx=\"32.000000\""));
    let tikz = String::from_utf8(output(&file, "render", &["--tikz", "--stdout"])?)?;
    assert!(tikz.contains("rounded corners=12.000000pt"));
    assert!(tikz.contains("rounded corners=24.000000pt"));
    Ok(())
}

#[cfg(feature = "png")]
#[test]
fn rounded_rect_png_export() -> Result<(), Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let file = dir.path().join("rounded.locus");
    std::fs::write(&file, SOURCE)?;
    let bytes = output(&file, "render", &["--png", "--stdout"])?;
    assert!(bytes.starts_with(b"\x89PNG\r\n\x1a\n"));
    Ok(())
}
