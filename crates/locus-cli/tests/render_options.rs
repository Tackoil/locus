//! CLI export options, including the encoded PNG artifact.

use std::{error::Error, fs};

use assert_cmd::Command;

#[test]
fn background_is_available_for_vector_formats() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let source = directory.path().join("empty.locus");
    fs::write(&source, "locus 0 beta 4\nnum n = 1\n")?;
    for (flags, expected) in [
        (vec![], "fill=\"#FFFFFF\""),
        (vec!["--tikz"], "fill={rgb,255:red,255;green,255;blue,255}"),
    ] {
        let output = Command::cargo_bin("locus")?
            .arg("render")
            .arg(&source)
            .args(["--stdout", "--background", "white"])
            .args(flags)
            .output()?;
        assert!(output.status.success());
        assert!(String::from_utf8(output.stdout)?.contains(expected));
    }
    Ok(())
}

#[cfg(feature = "png")]
#[test]
fn png_background_dpi_and_stdout_match_the_saved_artifact() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let source = directory.path().join("empty.locus");
    let destination = directory.path().join("empty.png");
    fs::write(&source, "locus 0 beta 4\nnum n = 1\n")?;
    for (flags, size, density, pixel) in [
        (vec![], 64, 3780, [0, 0, 0, 0]),
        (vec!["--background", "transparent"], 64, 3780, [0, 0, 0, 0]),
        (
            vec!["--background", "white", "--dpi", "300"],
            200,
            11811,
            [255, 255, 255, 255],
        ),
    ] {
        let output = Command::cargo_bin("locus")?
            .arg("render")
            .arg(&source)
            .arg("-o")
            .arg(&destination)
            .args(["--png", "--stdout"])
            .args(flags)
            .output()?;
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(output.stdout, fs::read(&destination)?);
        let mut reader = png::Decoder::new(output.stdout.as_slice()).read_info()?;
        assert_eq!((reader.info().width, reader.info().height), (size, size));
        let physical = reader.info().pixel_dims.ok_or("missing density")?;
        assert_eq!((physical.xppu, physical.yppu), (density, density));
        assert_eq!(physical.unit, png::Unit::Meter);
        let mut pixels = vec![0; reader.output_buffer_size()];
        reader.next_frame(&mut pixels)?;
        assert!(pixels.chunks_exact(4).all(|rgba| rgba == pixel));
    }
    Ok(())
}

#[cfg(feature = "png")]
#[test]
fn invalid_export_flags_report_json_usage_errors() -> Result<(), Box<dyn Error>> {
    for flags in [
        vec!["--dpi", "300"],
        vec!["--png", "--dpi", "0"],
        vec!["--png", "--dpi=-1"],
        vec!["--png", "--dpi", "NaN"],
        vec!["--png", "--dpi", "1.5"],
        vec!["--png", "--background", "blue"],
        vec!["--png", "--tikz", "--dpi", "300"],
    ] {
        let output = Command::cargo_bin("locus")?
            .args(["render", "unused.locus", "-o", "unused.png", "--json"])
            .args(flags)
            .output()?;
        assert_eq!(output.status.code(), Some(3));
        let value: serde_json::Value = serde_json::from_slice(&output.stdout)?;
        assert_eq!(value["diagnostics"][0]["code"], "usage");
    }
    Ok(())
}

#[cfg(not(feature = "png"))]
#[test]
fn dpi_requires_a_png_enabled_build() -> Result<(), Box<dyn Error>> {
    Command::cargo_bin("locus")?
        .args(["render", "unused.locus", "-o", "unused.png", "--dpi", "300"])
        .assert()
        .code(3);
    Ok(())
}
