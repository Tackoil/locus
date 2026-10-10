//! CLI rendering destinations, feature selection, and source-aware failures.

use std::collections::BTreeSet;
use std::error::Error;
use std::fs;
use std::path::Path;

use assert_cmd::Command;

type Result<T = ()> = std::result::Result<T, Box<dyn Error>>;

const PLAIN: &str =
    "locus 0 beta 4\ncircle c = circle(2)\nlabel name = label(c.center, \"circle\")\n";
const TEX: &str = "locus 0 beta 4\ncircle c = circle(2)\nlabel name = label(c.center, $x^2$)\n";

fn render(path: &Path) -> Result<Command> {
    let mut command = Command::cargo_bin("locus")?;
    command.arg("render").arg(path);
    Ok(command)
}

fn destination_case(source_text: &str, options: &[&str], signature: &[u8]) -> Result {
    let directory = tempfile::tempdir()?;
    let source = directory.path().join("figure.locus");
    let target = directory.path().join("output");
    fs::write(&source, source_text)?;
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

fn render_label(text: &str, options: &[&str]) -> Result<Vec<u8>> {
    let directory = tempfile::tempdir()?;
    let source = directory.path().join("label.locus");
    fs::write(
        &source,
        format!(
            "locus 0 beta 4\npoint anchor = point() {{ mark: none }}\nlabel name = label(anchor, \"{text}\") {{ dir: center }}\n"
        ),
    )?;
    let output = render(&source)?.args(options).arg("--stdout").output()?;
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(output.stdout)
}

fn svg_attribute<'a>(element: &'a str, name: &str) -> Result<&'a str> {
    element
        .split_once(&format!("{name}=\""))
        .and_then(|(_, tail)| tail.split_once('"'))
        .map(|(value, _)| value)
        .ok_or_else(|| format!("missing SVG attribute {name}").into())
}

fn svg_paths(svg: &str) -> Result<Vec<&str>> {
    svg.split("<path ")
        .skip(1)
        .map(|element| svg_attribute(element, "d"))
        .collect()
}

#[test]
fn offset_labels_render_without_connectors_by_default() -> Result {
    let directory = tempfile::tempdir()?;
    let source = directory.path().join("label.locus");
    for properties in ["", " { dir: N, gap: 4 }"] {
        fs::write(
            &source,
            format!(
                "locus 0 beta 4\npoint anchor = point() {{ mark: none }}\nlabel name = label(anchor, \"X\"){properties}\n"
            ),
        )?;
        let output = render(&source)?.arg("--stdout").output()?;
        assert!(output.status.success());
        let svg = String::from_utf8(output.stdout)?;
        assert_eq!(svg_paths(&svg)?.len(), 1, "only the glyph is drawn");
        assert!(!svg.contains("<line"));
    }
    Ok(())
}

#[test]
fn cjk_scripts_render_distinct_glyph_outlines() -> Result {
    let mut outlines = BTreeSet::new();
    for text in ["中", "文", "あ", "カ", "한", "글"] {
        let svg = String::from_utf8(render_label(text, &[])?)?;
        assert!(!svg.contains("<text"));
        assert!(!svg.contains("<circle"));
        let paths = svg_paths(&svg)?;
        assert_eq!(paths.len(), 1, "{text} must have a glyph outline");
        // Compare outline topology independent of viewport translation. Missing
        // glyph boxes have identical commands even when their positions differ.
        let commands: String = paths[0].chars().filter(char::is_ascii_uppercase).collect();
        assert!(!commands.is_empty());
        assert!(
            outlines.insert(commands),
            "{text} reused another glyph's outline"
        );
    }
    Ok(())
}

#[test]
fn mixed_cjk_ascii_math_and_multiline_labels_render() -> Result {
    let text = r"中文 CJK $x^2$\nかな 한글 123";
    let svg = String::from_utf8(render_label(text, &[])?)?;
    assert_eq!(svg_paths(&svg)?.len(), 14);
    assert!(!svg.contains("<text"));
    let single_line = String::from_utf8(render_label(&text.replace(r"\n", " "), &[])?)?;
    let height: f64 = svg_attribute(&svg, "height")?.parse()?;
    let single_height: f64 = svg_attribute(&single_line, "height")?.parse()?;
    assert!(
        height > single_height,
        "a second line must expand the viewport"
    );
    let tikz = String::from_utf8(render_label(text, &["--tikz"])?)?;
    assert!(tikz.contains(".. controls "));
    assert!(!tikz.contains("\\node"));
    Ok(())
}

#[cfg(feature = "png")]
#[test]
fn cjk_png_labels_contain_visible_ink() -> Result {
    for text in ["中文", "かな", "한글", r"中文 CJK $x^2$\nかな 한글 123"] {
        let bytes = render_label(text, &["--png"])?;
        let mut reader = png::Decoder::new(bytes.as_slice()).read_info()?;
        assert_eq!(reader.info().color_type, png::ColorType::Rgba);
        let mut pixels = vec![0; reader.output_buffer_size()];
        let frame = reader.next_frame(&mut pixels)?;
        let rgba = &pixels[..frame.buffer_size()];
        // The anchor has no mark, so all visible pixels belong to the label.
        assert!(
            rgba.chunks_exact(4).any(|pixel| pixel[3] > 0),
            "{text} is blank"
        );
        assert!(rgba.chunks_exact(4).any(|pixel| pixel[3] == 0));
    }
    Ok(())
}

#[test]
fn svg_and_tikz_use_real_plaintext_and_clean_stdout() -> Result {
    destination_case(PLAIN, &[], b"<svg")?;
    destination_case(PLAIN, &["--tikz"], b"\\begin{tikzpicture}")?;
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
    destination_case(PLAIN, &["--png"], b"\x89PNG\r\n\x1a\n")?;
    destination_case(TEX, &["--png"], b"\x89PNG\r\n\x1a\n")
}

#[test]
fn default_rex_engine_renders_math_to_svg_and_tikz() -> Result {
    let source = TEX.replace("x^2", r"\frac{-b \pm \sqrt{b^2-4ac}}{2a}");
    destination_case(&source, &[], b"<svg")?;
    destination_case(&source, &["--tikz"], b"\\begin{tikzpicture}")?;
    let directory = tempfile::tempdir()?;
    let input = directory.path().join("math.locus");
    fs::write(&input, &source)?;
    let result = render(&input)?
        .args(["--tex-engine=rex", "--stdout"])
        .output()?;
    assert!(result.status.success());
    let svg = String::from_utf8(result.stdout)?;
    assert!(svg.contains("<path"));
    assert!(svg.contains("fill=\"#111827\""));
    assert!(!svg.contains("<text"));
    assert!(!svg.contains("data-engine=\"mock\""));
    Ok(())
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
    failure_case("locus 0 beta 4\nnum a = 1 / 0\n", "E204", 2, 2)?;
    failure_case(&TEX.replace("x^2", r"\notacommand{x}"), "E414", 2, 3)?;
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
    assert!(svg.contains("<g id=\"c\""));
    // Mock text has no glyph outlines; labels no longer leave a connector behind.
    assert!(!svg.contains("<g id=\"name\""));
    Ok(())
}
