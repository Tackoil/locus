//! CLI rendering destinations, feature selection, and source-aware failures.

use std::error::Error;
use std::fs;
use std::path::Path;

use assert_cmd::Command;

type Result<T = ()> = std::result::Result<T, Box<dyn Error>>;

const PLAIN: &str = "locus 3\ncircle c = circle(2)\nlabel name = label(c.center, \"circle\")\n";
const TEX: &str = "locus 3\ncircle c = circle(2)\nlabel name = label(c.center, $x^2$)\n";

fn render(path: &Path) -> Result<Command> {
    let mut command = Command::cargo_bin("locus")?;
    command.arg("render").arg(path);
    Ok(command)
}

fn destination_case(options: &[&str], signature: &[u8]) -> Result {
    let directory = tempfile::tempdir()?;
    let source = directory.path().join("figure.locus");
    let target = directory.path().join("output");
    fs::write(&source, PLAIN)?;
    let output = render(&source)?
        .args(options)
        .arg("--stdout")
        .arg("-o")
        .arg(&target)
        .output()?;
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stdout.starts_with(signature));
    assert_eq!(output.stdout, fs::read(&target)?);
    let report = render(&source)?
        .args(options)
        .arg("-o")
        .arg(&target)
        .arg("--json")
        .output()?;
    assert!(report.status.success());
    let json: serde_json::Value = serde_json::from_slice(&report.stdout)?;
    assert_eq!(json["command"], "render");
    assert_eq!(json["output"], target.to_string_lossy().as_ref());
    assert!(json["diagnostics"].is_array());
    Ok(())
}

#[test]
fn svg_and_tikz_use_real_plaintext_and_clean_stdout() -> Result {
    destination_case(&[], b"<svg")?;
    destination_case(&["--tikz"], b"\\begin{tikzpicture}")?;
    let directory = tempfile::tempdir()?;
    let source = directory.path().join("figure.locus");
    fs::write(&source, PLAIN)?;
    let output = render(&source)?.arg("--stdout").output()?;
    let svg = String::from_utf8(output.stdout)?;
    assert!(
        svg.lines()
            .any(|line| line.contains("<path") && line.contains("fill=\"#111827\"")),
        "real text must contain filled glyph outlines"
    );
    assert!(!svg.contains("data-engine=\"mock\""));
    Ok(())
}

#[cfg(feature = "png")]
#[test]
fn png_feature_delivers_binary_bytes() -> Result {
    destination_case(&["--png"], b"\x89PNG\r\n\x1a\n")
}

#[test]
fn usage_errors_precede_reading_or_writing() -> Result {
    let directory = tempfile::tempdir()?;
    let missing = directory.path().join("missing.locus");
    render(&missing)?.assert().code(3);
    render(&missing)?
        .args(["--stdout", "--json"])
        .assert()
        .code(3);
    render(&missing)?
        .args(["--stdout", "--tex-engine=unknown"])
        .assert()
        .code(3);
    #[cfg(feature = "png")]
    render(&missing)?
        .args(["--stdout", "--png", "--tikz"])
        .assert()
        .code(3);
    #[cfg(not(feature = "png"))]
    render(&missing)?
        .args(["--stdout", "--png"])
        .assert()
        .code(3);
    Ok(())
}

fn failure_case(source: &str, code: &str, exit: i32, line: u64) -> Result {
    let directory = tempfile::tempdir()?;
    let input = directory.path().join("bad.locus");
    let output = directory.path().join("existing.svg");
    fs::write(&input, source)?;
    fs::write(&output, "keep existing artifact")?;
    let result = render(&input)?
        .arg("-o")
        .arg(&output)
        .arg("--json")
        .output()?;
    assert_eq!(result.status.code(), Some(exit));
    let json: serde_json::Value = serde_json::from_slice(&result.stdout)?;
    assert_eq!(json["diagnostics"][0]["code"], code);
    assert_eq!(json["diagnostics"][0]["span"]["line"], line);
    assert_eq!(
        json["diagnostics"][0]["path"],
        input.to_string_lossy().as_ref()
    );
    assert_eq!(fs::read_to_string(&output)?, "keep existing artifact");
    Ok(())
}

#[test]
fn failures_preserve_codes_spans_and_existing_artifacts() -> Result {
    failure_case("num a = 1\n", "E110", 3, 1)?;
    failure_case("locus 3\nnum a = 1 / 0\n", "E204", 2, 2)?;
    // The existing ReX seam is unavailable: never silently substitute mock output.
    failure_case(TEX, "E414", 2, 3)?;
    let directory = tempfile::tempdir()?;
    let input = directory.path().join("good.locus");
    fs::write(&input, PLAIN)?;
    let result = render(&input)?
        .arg("-o")
        .arg(directory.path())
        .arg("--json")
        .output()?;
    assert_eq!(result.status.code(), Some(3));
    let json: serde_json::Value = serde_json::from_slice(&result.stdout)?;
    assert_eq!(json["diagnostics"][0]["code"], "io");
    Ok(())
}

#[test]
fn mock_is_explicit_and_assertions_do_not_gate_render() -> Result {
    let directory = tempfile::tempdir()?;
    let input = directory.path().join("figure.locus");
    fs::write(&input, format!("{TEX}assert 1 == 2\n"))?;
    // Deliberately exercise the standalone render contract even after a failed check.
    Command::cargo_bin("locus")?
        .arg("check")
        .arg(&input)
        .assert()
        .code(1);
    let result = render(&input)?
        .args(["--tex-engine=mock", "--stdout"])
        .output()?;
    assert!(result.status.success());
    let svg = String::from_utf8(result.stdout)?;
    assert!(svg.starts_with("<svg"));
    assert!(svg.contains("<g id=\"name\""));
    Ok(())
}
