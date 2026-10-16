//! Render traces and the offline viewer's CLI contract.
use assert_cmd::Command;
use serde_json::Value;
use std::{error::Error, fs, path::Path};

const SOURCE: &str = "locus 0 beta 6\nnum radius = free 0.1\nrel radius * radius == 4\npoint origin()\ncircle disk(origin, radius)\nlabel caption(origin, \"Center\")\ngroup shapes(disk, caption)\n";

fn render_plain(source: &Path, output: &Path) -> Result<Command, Box<dyn Error>> {
    let mut command = Command::cargo_bin("locus")?;
    command
        .arg("render")
        .arg(source)
        .arg("-o")
        .arg(output)
        .arg("--tex-engine=mock");
    Ok(command)
}

fn render(source: &Path, output: &Path) -> Result<Command, Box<dyn Error>> {
    let mut command = render_plain(source, output)?;
    command
        .arg("--dump")
        .arg(output.with_extension("dump.json"));
    Ok(command)
}

fn read(path: &Path) -> Result<Value, Box<dyn Error>> {
    Ok(serde_json::from_slice(&fs::read(path)?)?)
}

#[test]
fn explicit_dump_captures_every_lm_trial_and_preserves_rendering() -> Result<(), Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let source = dir.path().join("scene.locus");
    let output = dir.path().join("scene.svg");
    fs::write(&source, SOURCE)?;
    render(&source, &output)?.assert().success();
    let dump = read(&output.with_extension("dump.json"))?;
    assert_eq!(dump["schema"], "locus-render-dump");
    assert_eq!(dump["source"], SOURCE);
    let steps = dump["steps"].as_array().ok_or("missing steps")?;
    let iterations: Vec<_> = steps
        .iter()
        .filter(|s| s["name"] == "solver_iteration")
        .collect();
    let solved = steps
        .iter()
        .find(|s| s["name"] == "solve_pass")
        .ok_or("missing solve pass")?;
    assert_eq!(
        iterations.len() as u64,
        solved["data"]["solve"]["attempts"]
            .as_u64()
            .ok_or("missing attempts")?
    );
    assert!(iterations.iter().all(|s| s["svg"].is_string()));
    assert_eq!(dump["entities"][4]["id"], "shapes");
    assert!(
        dump["relations"]
            .as_array()
            .ok_or("missing relations")?
            .iter()
            .any(|r| r["kind"] == "membership")
    );
    let rendered = fs::read_to_string(&output)?;
    let export = steps
        .iter()
        .find(|s| s["name"] == "render_output")
        .ok_or("missing export")?;
    assert_eq!(export["svg"], rendered);
    let other = dir.path().join("without.svg");
    render_plain(&source, &other)?
        .arg("--no-dump")
        .assert()
        .success();
    assert_eq!(fs::read(&output)?, fs::read(&other)?);
    assert!(!other.with_extension("dump.json").exists());
    Ok(())
}

#[test]
fn stdout_stays_clean_and_dump_path_can_be_overridden() -> Result<(), Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let source = dir.path().join("scene.locus");
    let dump = dir.path().join("trace.json");
    fs::write(&source, SOURCE)?;
    let output = Command::cargo_bin("locus")?
        .arg("render")
        .arg(&source)
        .args(["--stdout", "--tex-engine=mock", "--dump"])
        .arg(&dump)
        .output()?;
    assert!(output.status.success());
    assert!(output.stdout.starts_with(b"<svg"));
    assert_eq!(read(&dump)?["result"]["exit_code"], 0);
    assert!(!source.with_extension("dump.json").exists());
    Ok(())
}

#[test]
fn failures_preserve_completed_steps_and_diagnostics() -> Result<(), Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let source = dir.path().join("invalid.locus");
    let output = dir.path().join("invalid.svg");
    for text in ["point broken()\n", "locus 0 beta 6\nnum n = 1 / 0\n"] {
        fs::write(&source, text)?;
        let result = render(&source, &output)?.arg("--json").output()?;
        assert!(!result.status.success());
        let dump = read(&output.with_extension("dump.json"))?;
        assert_eq!(dump["source"], text);
        assert_ne!(dump["result"]["exit_code"], 0);
        assert!(
            dump["result"]["output"]["diagnostics"]
                .as_array()
                .is_some_and(|d| !d.is_empty())
        );
        assert_eq!(dump["steps"][0]["name"], "read_source");
        assert!(!output.exists());
    }
    Ok(())
}

#[test]
fn layout_and_settling_snapshots_retain_labels_and_groups() -> Result<(), Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let source = dir.path().join("group.locus");
    let output = dir.path().join("group.svg");
    fs::write(
        &source,
        "locus 0 beta 6\nfn badge(text) {\nrect shape()\nlabel caption(text)\nrel center(caption, shape)\nrel contain(shape, caption)\n}\nbadge first(\"One\")\ngroup all(first)\n",
    )?;
    render(&source, &output)?.assert().success();
    let dump = read(&output.with_extension("dump.json"))?;
    let steps = dump["steps"].as_array().ok_or("missing steps")?;
    assert!(steps.iter().any(|s| s["name"] == "settling_iteration"));
    assert!(
        steps
            .iter()
            .any(|s| s["name"] == "settling_evaluation" && s["svg"].is_string())
    );
    let layout = steps
        .iter()
        .find(|s| s["name"] == "layout_pass")
        .ok_or("missing layout")?;
    assert!(
        layout["svg"]
            .as_str()
            .is_some_and(|s| s.contains("first.caption"))
    );
    assert_eq!(layout["data"]["annotations"][0]["id"], "first.caption");
    let shape = dump["entities"]
        .as_array()
        .ok_or("missing entities")?
        .iter()
        .find(|e| e["id"] == "first.shape")
        .ok_or("missing expanded shape")?;
    assert_eq!(shape["source_name"], "shape");
    assert_eq!(shape["origin"]["invocations"][0]["instance"], "first");
    Ok(())
}

#[test]
fn dump_cannot_overwrite_source_or_render_output() -> Result<(), Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let source = dir.path().join("scene.locus");
    let output = dir.path().join("scene.svg");
    fs::write(&source, SOURCE)?;
    for dump in [&source, &output] {
        render_plain(&source, &output)?
            .arg("--dump")
            .arg(dump)
            .assert()
            .code(3);
        assert_eq!(fs::read_to_string(&source)?, SOURCE);
        assert!(!output.exists());
    }
    Ok(())
}

#[test]
fn viewer_is_self_contained_and_escapes_embedded_source() -> Result<(), Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let source = dir.path().join("scene.locus");
    let output = dir.path().join("scene.svg");
    fs::write(
        &source,
        format!("{SOURCE}// </script><script>alert('unsafe')</script>\n"),
    )?;
    render(&source, &output)?.assert().success();
    let dump = output.with_extension("dump.json");
    let html = dir.path().join("viewer.html");
    Command::cargo_bin("locus")?
        .arg("visualize")
        .arg(&dump)
        .arg("-o")
        .arg(&html)
        .assert()
        .success();
    let text = fs::read_to_string(&html)?;
    assert!(text.contains("Locus / render explorer"));
    assert!(text.contains("\\u003c/script\\u003e"));
    assert!(!text.contains("<script>alert('unsafe')"));
    assert!(!text.contains("__LOCUS_DUMP__"));
    assert!(!text.contains("__LOCUS_SCRIPT__"));
    let original = fs::read(&dump)?;
    Command::cargo_bin("locus")?
        .arg("visualize")
        .arg(&dump)
        .arg("-o")
        .arg(&dump)
        .assert()
        .code(3);
    assert_eq!(fs::read(&dump)?, original);
    fs::remove_file(&source)?;
    Command::cargo_bin("locus")?
        .arg("visualize")
        .arg(&dump)
        .assert()
        .success();
    assert!(dump.with_extension("html").exists());
    Ok(())
}

#[test]
fn viewer_rejects_invalid_and_unsupported_dumps() -> Result<(), Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let dump = dir.path().join("dump.json");
    for source in [
        "{}",
        "{invalid}",
        r#"{"schema":"other","version":9,"path":"x","source":"","entities":[],"relations":[],"steps":[],"result":{}}"#,
    ] {
        fs::write(&dump, source)?;
        Command::cargo_bin("locus")?
            .arg("visualize")
            .arg(&dump)
            .assert()
            .code(3);
        assert!(!dump.with_extension("html").exists());
    }
    Ok(())
}

#[test]
fn failed_solves_capture_rejected_numerical_trials() -> Result<(), Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let source = dir.path().join("rejected.locus");
    let output = dir.path().join("rejected.svg");
    fs::write(
        &source,
        "locus 0 beta 6\nnum x = free 1\nrel x * x * x - 2 * x + 2 == 0\npoint origin()\ncircle disk(origin, 1)\n",
    )?;
    let result = render(&source, &output)?.output()?;
    assert!(!result.status.success());
    let dump = read(&output.with_extension("dump.json"))?;
    let steps = dump["steps"].as_array().ok_or("missing steps")?;
    assert!(
        steps.iter().any(|step| step["name"] == "solver_iteration"
            && step["data"]["details"]["accepted"] == false)
    );
    assert_ne!(dump["result"]["exit_code"], 0);
    Ok(())
}

#[test]
fn every_feedback_pass_has_its_own_geometry_and_layout_snapshot() -> Result<(), Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let source = dir.path().join("routing.locus");
    let output = dir.path().join("routing.svg");
    fs::write(
        &source,
        "locus 0 beta 6\npoint start()\npoint finish = on_circle(circle(start, 4), 90deg)\npoint middle = on_circle(circle(start, 2), 90deg)\nrect obstacle(middle, 2.5, 1)\narrow path(start, finish) { route: orthogonal, side: right }\nrel avoid(path, obstacle)\n",
    )?;
    render(&source, &output)?.assert().success();
    let dump = read(&output.with_extension("dump.json"))?;
    let steps = dump["steps"].as_array().ok_or("missing steps")?;
    let solves: Vec<_> = steps.iter().filter(|s| s["name"] == "solve_pass").collect();
    let layouts: Vec<_> = steps
        .iter()
        .filter(|s| s["name"] == "layout_pass")
        .collect();
    assert!(layouts.len() >= 2);
    assert_eq!(solves.len(), layouts.len());
    for (index, (solve, layout)) in solves.iter().zip(layouts).enumerate() {
        assert_eq!(solve["data"]["pass"], index + 1);
        assert_eq!(layout["data"]["pass"], index + 1);
        assert!(solve["svg"].is_string());
        assert!(layout["svg"].is_string());
    }
    Ok(())
}

#[cfg(feature = "png")]
#[test]
fn png_dump_keeps_the_encoded_export_and_an_svg_preview() -> Result<(), Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let source = dir.path().join("scene.locus");
    let output = dir.path().join("scene.png");
    fs::write(&source, SOURCE)?;
    render(&source, &output)?
        .args(["--png", "--background", "white"])
        .assert()
        .success();
    let dump = read(&output.with_extension("dump.json"))?;
    let steps = dump["steps"].as_array().ok_or("missing steps")?;
    let export = steps
        .iter()
        .find(|s| s["name"] == "render_output")
        .ok_or("missing export")?;
    assert_eq!(export["data"]["format"], "png");
    assert_eq!(
        export["data"]["content"],
        serde_json::json!(fs::read(&output)?)
    );
    assert!(
        steps
            .iter()
            .any(|s| s["name"] == "render_svg_preview" && s["svg"].is_string())
    );
    Ok(())
}

#[test]
fn rendering_does_not_capture_a_dump_by_default() -> Result<(), Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let source = dir.path().join("scene.locus");
    let output = dir.path().join("scene.svg");
    fs::write(&source, SOURCE)?;
    let result = render_plain(&source, &output)?.arg("--json").output()?;
    assert!(result.status.success());
    let report: Value = serde_json::from_slice(&result.stdout)?;
    assert!(report.get("dump").is_none());
    assert!(output.exists());
    assert!(!output.with_extension("dump.json").exists());
    let stdout = Command::cargo_bin("locus")?
        .arg("render")
        .arg(&source)
        .args(["--stdout", "--tex-engine=mock"])
        .output()?;
    assert!(stdout.status.success());
    assert!(stdout.stdout.starts_with(b"<svg"));
    assert!(!source.with_extension("dump.json").exists());
    Ok(())
}

#[test]
fn measurement_and_solver_previews_keep_measured_labels_and_arrows() -> Result<(), Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let source = dir.path().join("labels.locus");
    let output = dir.path().join("labels.svg");
    fs::write(
        &source,
        "locus 0 beta 6\nrect box()\nlabel caption(\"Measured label\")\nrel center(caption, box)\nrel contain(box, caption)\npoint start()\npoint end = on_circle(circle(start, 4), 0deg)\narrow path(start, end) { route: orthogonal }\nlabel attached(path, \"Attached caption\") { near: middle, side: above }\n",
    )?;
    render(&source, &output)?.assert().success();
    let dump = read(&output.with_extension("dump.json"))?;
    let steps = dump["steps"].as_array().ok_or("missing steps")?;
    let measured = steps
        .iter()
        .find(|s| s["name"] == "measure_text")
        .ok_or("missing measurement")?;
    let measurements = measured["data"]["measurements"]
        .as_array()
        .ok_or("missing labels")?;
    assert_eq!(measurements.len(), 2);
    assert!(measurements.iter().all(|label| {
        label["outline"]["svg"]
            .as_str()
            .is_some_and(|svg| svg.contains("<path"))
    }));
    assert!(
        measured["svg"]
            .as_str()
            .is_some_and(|svg| svg.contains("data-measurement-gallery")
                && svg.contains("data-measured-label=\"caption\""))
    );
    let solver_steps: Vec<_> = steps
        .iter()
        .filter(|s| {
            matches!(
                s["name"].as_str(),
                Some("solver_start" | "solver_iteration" | "settling_evaluation" | "solve_pass")
            )
        })
        .collect();
    assert!(!solver_steps.is_empty());
    for snapshot in solver_steps {
        let svg = snapshot["svg"]
            .as_str()
            .ok_or("missing numerical preview")?;
        assert!(svg.contains("data-measured-label=\"caption\""));
        assert!(svg.contains("data-measured-label=\"attached\""));
        assert!(svg.contains("<g id=\"path\""));
        // Glyph paths are stored once in the measurement step, not every trial.
        assert!(!snapshot["data"].to_string().contains("svg_body"));
    }
    Ok(())
}
