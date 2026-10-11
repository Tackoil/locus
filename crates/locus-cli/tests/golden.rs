//! Versioned demonstration goldens and independent-process pipeline determinism.

use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use assert_cmd::Command;

type Result<T = ()> = std::result::Result<T, Box<dyn Error>>;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn fixtures() -> Result<Vec<PathBuf>> {
    let directory = Path::new("examples/demonstrations");
    let mut paths = Vec::new();
    for entry in fs::read_dir(root().join(directory))? {
        let name = entry?.file_name();
        if Path::new(&name)
            .extension()
            .is_some_and(|ext| ext == "locus")
        {
            paths.push(directory.join(name));
        }
    }
    paths.sort();
    assert!(!paths.is_empty(), "no demonstrations discovered");
    Ok(paths)
}

fn run(command: &str, path: &Path, options: &[&str]) -> Result<Vec<u8>> {
    let output = Command::cargo_bin("locus")?
        .current_dir(root())
        .arg(command)
        .arg(path)
        .args(options)
        .timeout(Duration::from_secs(120))
        .output()?;
    assert!(
        output.status.success(),
        "{command} {} failed:\n{}\n{}",
        path.display(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(output.stdout)
}

fn checked_svg(path: &Path) -> Result<Vec<u8>> {
    run("check", path, &["--json"])?;
    // A new process rereads the original file and repeats parsing, expansion,
    // measurement, solve/layout feedback, and rendering independently.
    run("render", path, &["--tex-engine=rex", "--stdout"])
}

fn compare(path: &Path, extension: &str, actual: &[u8]) -> Result {
    let name = path.file_stem().ok_or("missing fixture name")?;
    let expected_path = root()
        .join("tests/fixtures/golden")
        .join(name)
        .with_extension(extension);
    if std::env::var_os("LOCUS_UPDATE_GOLDENS").as_deref() == Some(std::ffi::OsStr::new("1")) {
        fs::write(&expected_path, actual)?;
    }
    let expected = fs::read(&expected_path)?;
    assert!(
        actual == expected,
        "golden mismatch: {}; review the change before running LOCUS_UPDATE_GOLDENS=1 cargo test -p locus-cli --test golden demonstration_goldens",
        expected_path.display()
    );
    Ok(())
}

#[test]
fn demonstration_goldens() -> Result {
    for path in fixtures()? {
        for command in ["describe", "inspect"] {
            let args: &[&str] = if command == "inspect" {
                &["--json", "--measurements", "--incidences"]
            } else {
                &["--json"]
            };
            compare(
                &path,
                &format!("{command}.json"),
                &run(command, &path, args)?,
            )?;
        }
        compare(&path, "svg", &checked_svg(&path)?)?;
    }
    Ok(())
}

#[test]
fn full_pipeline_is_byte_deterministic() -> Result {
    for path in fixtures()? {
        let first = checked_svg(&path)?;
        let second = checked_svg(&path)?;
        assert!(
            first == second,
            "full pipeline changed SVG bytes: {}",
            path.display()
        );
    }
    Ok(())
}
