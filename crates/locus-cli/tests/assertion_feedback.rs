//! Regressions for assertion context and source-error classification.

use std::error::Error;
use std::fs;
use std::path::Path;
use std::process::Output;
use std::time::Duration;

use assert_cmd::Command;
use serde_json::{Value, json};

type Result<T = ()> = std::result::Result<T, Box<dyn Error>>;

fn command(file: &Path, args: &[&str]) -> Result<Output> {
    Ok(Command::cargo_bin("locus")?
        .arg("check")
        .arg(file)
        .args(args)
        .env("NO_COLOR", "1")
        .env(
            "LOCUS_REJECTED_REQUESTS_DIR",
            file.with_extension("requests"),
        )
        .timeout(Duration::from_secs(10))
        .output()?)
}

fn assertions(report: &Value) -> Result<&Vec<Value>> {
    report["files"][0]["assertions"]["assertions"]
        .as_array()
        .ok_or_else(|| "missing assertions".into())
}

fn close(actual: &Value, expected: f64) -> Result {
    assert!((actual.as_f64().ok_or("missing value")? - expected).abs() < 1e-12);
    Ok(())
}

#[test]
fn signed_offsets_show_named_values_units_and_transitive_definitions_in_every_format() -> Result {
    let directory = tempfile::tempdir()?;
    let file = directory.path().join("self-call.locus");
    let source = concat!(
        "locus 0 beta 5\n",
        "// Unicode before the failing expression: 回调\n",
        "point origin = point() { mark: none }\n",
        "point start = offset(origin, 2, 1) { mark: none }\n",
        "point end = offset(start, 0, -0.8) { mark: none }\n",
        "num expected = -0.7\n",
        "point unrelated = offset(origin, 100, 100) { mark: none }\n",
        "assert hdist(start, end) == 0\n",
        "assert vdist(start, end) == expected\n",
    );
    fs::write(&file, source)?;
    for args in [&["--json"][..], &["--json", "--compact"][..]] {
        let output = command(&file, args)?;
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stderr.is_empty());
        let report: Value = serde_json::from_slice(&output.stdout)?;
        let results = assertions(&report)?;
        let failed = results.last().ok_or("missing failure")?;
        let failure = &failed["failure"];
        assert_eq!(failure["expression"], "vdist(start, end) == expected");
        let comparison = &failure["comparisons"][0];
        assert_eq!(comparison["operator"], "==");
        assert_eq!(comparison["actual"]["expression"], "vdist(start, end)");
        close(&comparison["actual"]["value"], -0.8)?;
        assert_eq!(comparison["actual"]["unit"], "scene units");
        assert_eq!(comparison["expected"]["expression"], "expected");
        close(&comparison["expected"]["value"], -0.7)?;
        assert_eq!(comparison["expected"]["dimension"], "length");
        assert_eq!(comparison["expected"]["unit"], "scene units");
        let definitions = failure["upstream_definitions"]
            .as_array()
            .ok_or("definitions")?;
        assert_eq!(
            definitions
                .iter()
                .map(|definition| &definition["id"])
                .collect::<Vec<_>>(),
            vec![
                &json!("origin"),
                &json!("start"),
                &json!("end"),
                &json!("expected")
            ]
        );
        assert_eq!(
            definitions[2]["source"],
            "point end = offset(start, 0, -0.8) { mark: none }"
        );
        assert_eq!(definitions[2]["span"]["line"], 5);
        for term in failed["actuals"].as_array().ok_or("actuals")? {
            let span = &term["span"];
            let start = usize::try_from(span["byte_start"].as_u64().ok_or("start")?)?;
            let end = usize::try_from(span["byte_end"].as_u64().ok_or("end")?)?;
            assert_eq!(term["expression"], &source[start..end]);
        }
    }
    let output = command(&file, &[])?;
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8(output.stderr)?
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    assert!(stderr.contains("vdist(start, end) == expected"));
    assert!(stderr.contains("actual `vdist(start, end)` = -0.800000000000 scene units"));
    assert!(stderr.contains("expected `expected` = -0.700000000000 scene units"));
    assert!(stderr.contains("point end = offset(start, 0, -0.8)"));
    assert!(!stderr.contains("unrelated"));
    Ok(())
}

#[test]
fn comparisons_keep_dimensions_operators_and_boolean_meaning() -> Result {
    let directory = tempfile::tempdir()?;
    let file = directory.path().join("dimensions.locus");
    fs::write(
        &file,
        concat!(
            "locus 0 beta 5\n",
            "point A = point() { mark: none }\n",
            "point B = offset(A, 4, 0) { mark: none }\n",
            "rect box = rect(A, 2, 3)\n",
            "assert hdist(A, B) == 4\n",
            "assert area(box) >= 7\n",
            "assert dir_angle(A, B) == 90deg\n",
            "assert dist(A, B) / dist(A, B) < 0.5\n",
            "assert hdist(A, B) == 4 and (vdist(A, B) == 1 or dist(A, B) == 5)\n",
            "assert not horizontal(A, B)\n",
            "assert vertical(A, B)\n",
        ),
    )?;
    let output = command(&file, &["--json"])?;
    assert_eq!(output.status.code(), Some(1));
    let report: Value = serde_json::from_slice(&output.stdout)?;
    let results = assertions(&report)?;
    assert_eq!(results.len(), 7);
    assert!(results[0].get("failure").is_none());
    assert_eq!(results[0]["actuals"], json!([]));
    let area = &results[1]["failure"]["comparisons"][0];
    assert_eq!(area["operator"], ">=");
    close(&area["actual"]["value"], 6.0)?;
    assert_eq!(area["actual"]["unit"], "scene units squared");
    assert_eq!(area["expected"]["unit"], "scene units squared");
    let angle = &results[2]["failure"]["comparisons"][0];
    assert_eq!(angle["expected"]["expression"], "90deg");
    close(&angle["expected"]["radians"], std::f64::consts::FRAC_PI_2)?;
    assert_eq!(angle["expected"]["unit"], "rad");
    let scalar = &results[3]["failure"]["comparisons"][0];
    assert_eq!(scalar["operator"], "<");
    assert_eq!(scalar["actual"]["unit"], "unitless");
    let compound = results[4]["failure"]["comparisons"]
        .as_array()
        .ok_or("comparisons")?;
    assert_eq!(compound.len(), 2);
    assert_eq!(compound[0]["actual"]["expression"], "vdist(A, B)");
    assert_eq!(compound[1]["actual"]["expression"], "dist(A, B)");
    for result in &results[5..] {
        let comparison = &result["failure"]["comparisons"][0];
        assert_eq!(comparison["actual"]["value"], false);
        assert_eq!(comparison["expected"]["value"], true);
        assert!(comparison["expected"]["span"].is_null());
    }
    assert_eq!(
        results[5]["failure"]["comparisons"][0]["actual"]["expression"],
        "not horizontal(A, B)"
    );
    Ok(())
}

#[test]
fn assertion_context_includes_constraints_that_determine_free_values() -> Result {
    let directory = tempfile::tempdir()?;
    let file = directory.path().join("solved.locus");
    fs::write(
        &file,
        concat!(
            "locus 0 beta 5\nnum x = free 1\nnum y = free 2\n",
            "num unrelated = free 4\nrel x == y\nrel y == 3\nrel unrelated == 4\nassert x == 5\n",
        ),
    )?;
    let output = command(&file, &["--json", "--compact"])?;
    assert_eq!(output.status.code(), Some(1));
    let report: Value = serde_json::from_slice(&output.stdout)?;
    let failure = &assertions(&report)?[0]["failure"];
    let definitions = failure["upstream_definitions"]
        .as_array()
        .ok_or("definitions")?;
    assert_eq!(definitions.len(), 2);
    assert_eq!(definitions[0]["id"], "x");
    assert_eq!(definitions[1]["id"], "y");
    let relations = failure["upstream_relations"]
        .as_array()
        .ok_or("relations")?;
    assert_eq!(relations.len(), 2);
    assert_eq!(relations[0]["source"], "rel x == y");
    assert_eq!(relations[1]["source"], "rel y == 3");
    close(&failure["comparisons"][0]["actual"]["value"], 3.0)?;
    Ok(())
}

#[test]
fn invalid_properties_retain_source_errors_instead_of_solver_seed_advice() -> Result {
    let directory = tempfile::tempdir()?;
    let file = directory.path().join("properties.locus");
    let prefix =
        "locus 0 beta 5\npoint A = point()\npoint B = offset(A, 1, 0)\npoint C = offset(A, 0, 1)\n";
    for (declaration, code, message, fragment) in [
        (
            "text_block note = text_block(\"caption\") { place: below, anchor: A }",
            "E209",
            "cannot specify both",
            "{ place: below, anchor: A }",
        ),
        (
            "text_block note = text_block(\"caption\") { place: sideways }",
            "E209",
            "unknown placement",
            "sideways",
        ),
        (
            "label note = label(A, \"caption\") { dir: sideways }",
            "E209",
            "unknown label direction",
            "sideways",
        ),
        (
            "label note = label(A, \"caption\") { gap: -1 }",
            "E204",
            "gap must be nonnegative",
            "-1",
        ),
        (
            "right_angle note = right_angle(A, B, C) { size: 0 }",
            "E204",
            "size must be positive",
            "0",
        ),
        (
            "label note = label(A, \"caption\") { gap: 1 / 0 }",
            "E204",
            "division",
            "0",
        ),
    ] {
        let source = format!("{prefix}{declaration}\n");
        fs::write(&file, &source)?;
        let output = command(&file, &["--json", "--compact"])?;
        assert_eq!(output.status.code(), Some(2), "{declaration}");
        let report: Value = serde_json::from_slice(&output.stdout)?;
        let diagnostic = &report["files"][0]["diagnostics"][0];
        assert_eq!(diagnostic["code"], code, "{diagnostic}");
        assert!(
            diagnostic["message"]
                .as_str()
                .ok_or("message")?
                .contains(message)
        );
        assert!(
            !output
                .stdout
                .windows(10)
                .any(|window| window == b"degenerate")
        );
        let start = usize::try_from(diagnostic["span"]["byte_start"].as_u64().ok_or("start")?)?;
        let end = usize::try_from(diagnostic["span"]["byte_end"].as_u64().ok_or("end")?)?;
        assert_eq!(&source[start..end], fragment);
    }
    fs::write(
        &file,
        format!(
            "{prefix}text_block note = text_block(\"caption\") {{ place: below, anchor: A }}\n"
        ),
    )?;
    let human = command(&file, &[])?;
    let stderr = String::from_utf8(human.stderr)?;
    assert!(stderr.contains("E209"));
    assert!(stderr.contains("cannot specify both"));
    assert!(!stderr.contains("spread or correct hints"));
    Ok(())
}

#[test]
fn genuine_geometric_seed_degeneracy_keeps_its_classification() -> Result {
    let directory = tempfile::tempdir()?;
    let file = directory.path().join("degenerate.locus");
    fs::write(
        &file,
        "locus 0 beta 5\npoint A = point()\nline broken = line(A, A)\n",
    )?;
    let output = command(&file, &["--json"])?;
    assert_eq!(output.status.code(), Some(2));
    let report: Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(report["files"][0]["diagnostics"][0]["code"], "E404");
    Ok(())
}
