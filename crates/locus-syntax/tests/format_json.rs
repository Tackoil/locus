//! Canonical formatting and interchange corpus/property tests.

use locus_syntax::{format, parse, to_json_pretty};
use std::fs;
use std::path::{Path, PathBuf};

fn corpus_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../spec/test")
}

fn fixture(name: &str) -> Result<String, std::io::Error> {
    fs::read_to_string(
        corpus_dir()
            .join("demonstrations")
            .join(format!("{name}.locus")),
    )
}

fn parsed(source: &str) -> locus_syntax::File {
    let output = parse(source);
    assert!(output.diagnostics.is_empty(), "{:#?}", output.diagnostics);
    output.file
}

fn assert_fixture_snapshot(name: &str) -> Result<(), Box<dyn std::error::Error>> {
    let source = fixture(name)?;
    let formatted = format(&source);
    assert!(
        formatted.diagnostics.is_empty(),
        "{:#?}",
        formatted.diagnostics
    );
    insta::assert_snapshot!(format!("{name}_formatted"), formatted.source);
    let json = to_json_pretty(&parsed(&source))?;
    insta::assert_snapshot!(format!("{name}_json"), json);
    Ok(())
}

fn assert_fixture_idempotent(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let source = fs::read_to_string(path)?;
    let once = format(&source);
    assert!(
        once.diagnostics.is_empty(),
        "{}: {:#?}",
        path.display(),
        once.diagnostics
    );
    let twice = format(&once.source);
    assert!(twice.diagnostics.is_empty(), "{:#?}", twice.diagnostics);
    assert_eq!(once.source, twice.source, "{}", path.display());
    Ok(())
}

#[test]
fn formatter_is_idempotent_on_every_spec_fixture() -> Result<(), Box<dyn std::error::Error>> {
    let mut paths = Vec::new();
    for group in ["demonstrations", "stress", "composition"] {
        let entries = fs::read_dir(corpus_dir().join(group))?;
        paths.extend(
            entries
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .map(|entry| entry.path())
                .filter(|path| {
                    path.extension()
                        .is_some_and(|extension| extension == "locus")
                }),
        );
    }
    paths.sort();
    for path in paths {
        assert_fixture_idempotent(&path)?;
    }
    Ok(())
}

#[test]
fn snapshots_angle_bisector_ratio() -> Result<(), Box<dyn std::error::Error>> {
    assert_fixture_snapshot("angle_bisector_ratio")
}

#[test]
fn snapshots_inscribed_angle() -> Result<(), Box<dyn std::error::Error>> {
    assert_fixture_snapshot("inscribed_angle")
}

#[test]
fn snapshots_pentagon_golden() -> Result<(), Box<dyn std::error::Error>> {
    assert_fixture_snapshot("pentagon_golden")
}
