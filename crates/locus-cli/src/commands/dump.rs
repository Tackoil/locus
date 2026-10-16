//! Versioned render dump construction and exact layout previews.
use std::path::{Path, PathBuf};

use locus_core::{Scene, trace};
use locus_layout::LayoutScene;
use locus_render::{RenderOptions, render_svg, style::StyleSheet};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::{LoadedScene, io_failure, path_string, resolved_scene};
use crate::{
    RenderArgs,
    output::{Execution, SolveReport},
};
mod arrows;
mod labels;
mod metadata;

#[derive(Serialize, Deserialize)]
pub(super) struct Dump {
    pub schema: String,
    pub version: u32,
    pub path: String,
    pub source: String,
    pub entities: Vec<Value>,
    pub relations: Vec<Value>,
    pub steps: Vec<Step>,
    pub result: Value,
}

#[derive(Serialize, Deserialize)]
pub(super) struct Step {
    pub name: String,
    pub data: Value,
    pub svg: Option<String>,
}

pub(super) fn path(args: &RenderArgs) -> Option<PathBuf> {
    if args.no_dump {
        None
    } else {
        args.dump.clone()
    }
}

pub(super) fn same_path(first: &Path, second: &Path) -> bool {
    normalized_path(first) == normalized_path(second)
}

fn normalized_path(path: &Path) -> PathBuf {
    if let Ok(path) = path.canonicalize() {
        return path;
    }
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    parent
        .canonicalize()
        .unwrap_or_else(|_| parent.to_owned())
        .join(path.file_name().unwrap_or_default())
}

pub(super) fn validate_paths(args: &RenderArgs, dump: &Path) -> Result<(), Box<Execution>> {
    let conflicts = same_path(dump, &args.file)
        || args
            .output
            .as_ref()
            .is_some_and(|output| same_path(dump, output) || same_path(output, &args.file));
    if conflicts {
        return Err(Box::new(io_failure(
            "render",
            dump,
            &std::io::Error::other("source, render output, and dump must have different paths"),
        )));
    }
    Ok(())
}

pub(super) fn write(
    args: &RenderArgs,
    path: &Path,
    steps: Vec<trace::Step>,
    result: &Result<Execution, Box<Execution>>,
) -> Result<(), Box<Execution>> {
    let source = steps
        .iter()
        .find(|step| step.name == "read_source")
        .and_then(|step| step.data["source"].as_str())
        .unwrap_or("")
        .to_owned();
    let (entities, relations) = metadata::build(&source);
    let execution = match result {
        Ok(value) => value,
        Err(value) => value,
    };
    let previews = labels::Previews::new(&source, &steps);
    let mut steps: Vec<_> = steps
        .into_iter()
        .map(|event| step(event, &previews))
        .collect();
    let result = json!({"exit_code": execution.exit.code(), "output": execution.output});
    steps.push(Step {
        name: "result".to_owned(),
        data: result.clone(),
        svg: None,
    });
    let dump = Dump {
        schema: "locus-render-dump".to_owned(),
        version: 1,
        path: path_string(&args.file),
        source,
        entities,
        relations,
        steps,
        result,
    };
    let bytes = serde_json::to_vec_pretty(&dump)
        .map_err(std::io::Error::other)
        .and_then(|bytes| std::fs::write(path, bytes));
    bytes.map_err(|error| Box::new(io_failure("render", path, &error)))
}

fn step(step: trace::Step, previews: &labels::Previews) -> Step {
    let mut data = step.data;
    let supplied = data
        .get_mut("svg")
        .and_then(|value| value.take().as_str().map(str::to_owned));
    let svg = supplied.or_else(|| step.scene.as_ref().and_then(|scene| previews.render(scene)));
    if let Some(scene) = &step.scene {
        data["objects"] = json!(
            scene
                .objects
                .iter()
                .map(|(id, object)| json!({
                    "id": id.name, "span": id.span, "bounds": object.bbox().map(bounds),
                    "value": format!("{object:?}"),
                }))
                .collect::<Vec<_>>()
        );
    }
    Step {
        name: step.name,
        data,
        svg,
    }
}

pub(super) fn measured_step(
    inputs: &locus_core::EvaluationInputs,
    boxes: &locus_layout::TextBoxes,
) {
    trace::record("measure_text", || {
        let measurements = labels::measurements(inputs, boxes);
        let svg = labels::gallery(&measurements);
        (
            json!({"measurements": measurements, "svg": svg, "preview_kind": "measurements"}),
            None,
        )
    });
}

pub(super) fn layout_step(pass: usize, scene: &Scene, layout: Option<&LayoutScene>) {
    let name = if layout.is_some() {
        "layout_pass"
    } else {
        "solve_pass"
    };
    trace::record(name, || {
        let mut data = layout.map_or_else(|| json!({}), |placed| preview(scene, placed));
        data["pass"] = json!(pass);
        data["solve"] = json!(scene.solve_summary().map(SolveReport::from));
        (data, Some(resolved_scene(scene).clone()))
    });
}

fn preview(scene: &Scene, placed: &LayoutScene) -> Value {
    let rendered = StyleSheet::from_file(&scene.file).and_then(|styles| {
        render_svg(
            resolved_scene(scene),
            placed,
            &styles,
            RenderOptions::default(),
        )
    });
    json!({"svg": rendered.as_ref().ok(), "preview_error": rendered.as_ref().err().map(ToString::to_string),
        "bounds": bounds(placed.bbox), "diagnostics": placed.diagnostics,
        "visible_paths": format!("{:?}", placed.visible_paths),
        "annotations": placed.annotations.values().map(|a| json!({
            "id": a.id.name, "bounds": bounds(a.bounds), "candidate": a.candidate_index,
            "leader": format!("{:?}", a.leader),
        })).collect::<Vec<_>>()})
}

pub(super) fn render_step(
    loaded: &LoadedScene,
    placed: &LayoutScene,
    styles: &StyleSheet,
    options: RenderOptions,
) {
    trace::record("render_svg_preview", || {
        let rendered = render_svg(resolved_scene(&loaded.scene), placed, styles, options);
        (
            json!({"svg": rendered.as_ref().ok(),
            "error": rendered.as_ref().err().map(ToString::to_string)}),
            None,
        )
    });
}

fn bounds(bounds: locus_core::BBox) -> Value {
    json!({"min": {"x": bounds.min.x, "y": bounds.min.y},
        "max": {"x": bounds.max.x, "y": bounds.max.y}})
}
