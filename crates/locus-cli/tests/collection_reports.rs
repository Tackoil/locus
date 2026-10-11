//! Expanded collection metadata survives ordinary checking and assertion context.

use std::error::Error;
use std::fs;
use std::path::Path;
use std::process::Output;

use assert_cmd::Command;

const SOURCE: &str = "locus 0 beta 6\npoint anchor()\nfn leaf(at) {\n    rect shape(at,1,1)\n}\nfn pair(at) {\n    leaf first(at)\n    point right = on_circle(circle(at, 2), 0deg)\n    leaf second(right)\n    group items(first,second)\n    group repeated(items,first,second)\n    rel row(items)\n    assert center(repeated,items)\n}\npair panel(anchor)\nassert center(panel.items,panel.repeated)\n";

fn run(path: &Path, command: &str, args: &[&str]) -> Result<Output, Box<dyn Error>> {
    Ok(Command::cargo_bin("locus")?
        .arg(command)
        .arg(path)
        .args(args)
        .env(
            "LOCUS_REJECTED_REQUESTS_DIR",
            path.with_extension("requests"),
        )
        .output()?)
}

#[test]
fn nested_instance_groups_pass_ordinary_check_and_render() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("collections.locus");
    fs::write(&path, SOURCE)?;
    let checked = run(&path, "check", &["--json"])?;
    assert!(
        checked.status.success(),
        "{}",
        String::from_utf8_lossy(&checked.stdout)
    );
    let report: serde_json::Value = serde_json::from_slice(&checked.stdout)?;
    assert_eq!(report["passed"], true);
    assert_eq!(
        report["files"][0]["assertions"]["assertions"]
            .as_array()
            .ok_or("assertions")?
            .len(),
        2
    );
    let rendered = run(&path, "render", &["--stdout"])?;
    assert!(
        rendered.status.success(),
        "{}",
        String::from_utf8_lossy(&rendered.stderr)
    );
    assert!(String::from_utf8(rendered.stdout)?.contains("<svg"));
    Ok(())
}

#[test]
fn failed_collection_assertions_retain_ordered_upstream_definitions() -> Result<(), Box<dyn Error>>
{
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("failed-collection.locus");
    fs::write(
        &path,
        format!("{SOURCE}assert center(panel.items,panel.first)\n"),
    )?;
    let checked = run(&path, "check", &["--json"])?;
    assert_eq!(
        checked.status.code(),
        Some(1),
        "{}",
        String::from_utf8_lossy(&checked.stdout)
    );
    let report: serde_json::Value = serde_json::from_slice(&checked.stdout)?;
    let definitions =
        report["files"][0]["assertions"]["assertions"][2]["failure"]["upstream_definitions"]
            .as_array()
            .ok_or("upstream definitions")?;
    let names = definitions
        .iter()
        .map(|definition| definition["id"].as_str().ok_or("definition id"))
        .collect::<Result<Vec<_>, _>>()?;
    assert_eq!(
        names,
        [
            "anchor",
            "panel.first.shape",
            "panel.right",
            "panel.second.shape"
        ]
    );
    Ok(())
}

#[test]
fn describe_and_interchange_preserve_collections_and_source_chains() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("describe.locus");
    fs::write(&path, SOURCE)?;
    let output = run(&path, "describe", &["--json"])?;
    assert!(output.status.success());
    let report: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(
        report["collections"]["panel.items"]["members"],
        serde_json::json!(["panel.first", "panel.second"])
    );
    let chain = &report["collections"]["panel.first"]["origin"]["invocations"];
    assert_eq!(chain[0]["instance"], "panel");
    assert_eq!(chain[1]["instance"], "panel.first");
    assert!(
        !report["expansion_origins"]
            .as_array()
            .ok_or("origins")?
            .is_empty()
    );
    let diagram = report["diagram"].as_array().ok_or("diagram")?;
    assert_eq!(diagram[0]["accepted"]["automatic_dimensions"], false);
    let json = String::from_utf8(output.stdout)?;
    assert!(!json.contains("\"location\""));
    assert!(!json.contains("\"x\""));
    let output = run(&path, "fmt", &["--json"])?;
    assert!(output.status.success());
    let interchange: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(interchange["calls"][0]["id"]["name"], "panel");
    assert!(
        !interchange["expanded"]["ops"]
            .as_array()
            .ok_or("expanded ops")?
            .is_empty()
    );
    Ok(())
}

#[test]
fn describe_reports_accepted_measured_dimensions_and_path_attachment() -> Result<(), Box<dyn Error>>
{
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("diagram.locus");
    fs::write(
        &path,
        "locus 0 beta 6\nrect panel()\nlabel title(\"Measured title\") {role:title}\nrel center(title,panel)\nrel contain(panel,title)\npoint tip = on_circle(circle(panel.center, 4), 0deg)\narrow path(panel.center,tip)\nlabel caption(path,\"path\") {side:above}\n",
    )?;
    let output = run(&path, "describe", &["--json"])?;
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    let records = report["diagram"].as_array().ok_or("diagram")?;
    let accepted = |id| {
        records
            .iter()
            .find(|record| record["id"] == id)
            .map(|record| &record["accepted"])
    };
    let panel = accepted("panel").ok_or("panel")?;
    assert_eq!(panel["automatic_dimensions"], true);
    assert!(panel["width"].as_f64().is_some_and(|width| width > 0.0));
    let title = accepted("title").ok_or("title")?;
    assert_eq!(title["role"], "title");
    assert!(title["width"].as_f64().is_some_and(|width| width > 0.0));
    let route = accepted("path").ok_or("route")?;
    assert_eq!(route["route"], "direct");
    assert!((route["logical_length"].as_f64().ok_or("length")? - 4.0).abs() < 1e-9);
    assert_eq!(
        accepted("caption").ok_or("caption")?["attachment"]["side"],
        "above"
    );
    assert!(
        !report["relations"]
            .as_array()
            .ok_or("relations")?
            .is_empty()
    );
    Ok(())
}
