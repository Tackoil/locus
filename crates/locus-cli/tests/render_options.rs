//! CLI export options, including the encoded PNG artifact.

use std::error::Error;

use assert_cmd::Command;

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
