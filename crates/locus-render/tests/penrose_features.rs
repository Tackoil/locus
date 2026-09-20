//! Executable contract for the Penrose public-gallery feature audit.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use locus_core::{evaluate_assertions, solve};
use locus_layout::{LayoutOptions, boxes::measure_scene, layout};
use locus_render::{RenderOptions, render_svg, style::StyleSheet};
use locus_tex::{FontSize, PlaintextEngine, RexEngine};

const MANIFEST: &str = include_str!("../../../benchmarks/penrose/gallery.tsv");

#[derive(Clone, Copy)]
struct Row<'a> {
    penrose_id: &'a str,
    title: &'a str,
    outcome: &'a str,
    feature_area: &'a str,
    evidence: &'a str,
    limitation: &'a str,
}

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn row(line: &str, line_number: usize) -> Row<'_> {
    let fields = line.split('\t').collect::<Vec<_>>();
    assert_eq!(
        fields.len(),
        6,
        "gallery.tsv line {line_number} must contain six tab-separated fields"
    );
    Row {
        penrose_id: fields[0],
        title: fields[1],
        outcome: fields[2],
        feature_area: fields[3],
        evidence: fields[4],
        limitation: fields[5],
    }
}

fn rows() -> Vec<Row<'static>> {
    let mut lines = MANIFEST.lines();
    assert_eq!(
        lines.next(),
        Some("penrose_id\ttitle\toutcome\tfeature_area\tevidence\tlimitation")
    );
    lines
        .enumerate()
        .filter(|(_, line)| !line.trim().is_empty())
        .map(|(index, line)| row(line, index + 2))
        .collect()
}

fn parse_witness(path: &Path) -> Result<locus_syntax::File, Box<dyn Error>> {
    let source = fs::read_to_string(path)?;
    let parsed = locus_syntax::parse(&source);
    assert!(
        parsed.diagnostics.is_empty(),
        "{}: {:?}",
        path.display(),
        parsed.diagnostics
    );
    Ok(parsed.file)
}

fn render_witness(path: &Path) -> Result<(), Box<dyn Error>> {
    let file = parse_witness(path)?;
    let solved = solve(&file)?;
    let assertions = evaluate_assertions(&file, &solved.scene)?;
    assert!(assertions.passed, "{}: {assertions:?}", path.display());
    let measured = measure_scene(
        &solved.scene,
        &PlaintextEngine::new(),
        &RexEngine,
        FontSize::DEFAULT,
    )?;
    let placed = layout(
        &solved.scene,
        &measured,
        LayoutOptions {
            em_size: 0.28,
            ..LayoutOptions::default()
        },
    )?;
    let styles = StyleSheet::from_file(&file)?;
    let svg = render_svg(&solved.scene, &placed, &styles, RenderOptions::default())?;
    assert_eq!(
        roxmltree::Document::parse(&svg)?
            .root_element()
            .tag_name()
            .name(),
        "svg"
    );
    Ok(())
}

#[test]
fn manifest_covers_the_pinned_public_gallery() {
    let rows = rows();
    assert_eq!(rows.len(), 53);
    let mut ids = BTreeSet::new();
    let mut outcomes = BTreeMap::new();
    for row in &rows {
        assert!(
            ids.insert(row.penrose_id),
            "duplicate ID: {}",
            row.penrose_id
        );
        assert!(!row.title.is_empty());
        assert!(!row.feature_area.is_empty());
        assert!(matches!(
            row.outcome,
            "native" | "approximation" | "blocked"
        ));
        *outcomes.entry(row.outcome).or_insert(0) += 1;
        if row.outcome == "blocked" {
            assert_eq!(row.evidence, "-", "{}", row.penrose_id);
        } else {
            assert_ne!(row.evidence, "-", "{}", row.penrose_id);
        }
        if row.outcome == "native" {
            assert_eq!(row.limitation, "-", "{}", row.penrose_id);
        } else {
            assert_ne!(row.limitation, "-", "{}", row.penrose_id);
        }
    }
    assert_eq!(outcomes.get("native"), Some(&8));
    assert_eq!(outcomes.get("approximation"), Some(&28));
    assert_eq!(outcomes.get("blocked"), Some(&17));
}

#[test]
fn cited_witnesses_parse_and_native_witnesses_render() -> Result<(), Box<dyn Error>> {
    let root = repository_root();
    let rows = rows();
    let evidence = rows
        .iter()
        .filter(|row| row.evidence != "-")
        .map(|row| root.join(row.evidence))
        .collect::<BTreeSet<_>>();
    let native = rows
        .iter()
        .filter(|row| row.outcome == "native")
        .map(|row| root.join(row.evidence))
        .collect::<BTreeSet<_>>();

    for path in evidence {
        assert!(path.is_file(), "missing witness: {}", path.display());
        let _ = parse_witness(&path)?;
    }
    for path in native {
        render_witness(&path)?;
    }
    Ok(())
}
