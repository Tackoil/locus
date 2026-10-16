//! Offline, dependency-free render dump visualization.
use super::{
    dump::{Dump, same_path},
    io_failure, path_string, success,
};
use crate::{
    VisualizeArgs,
    output::{CommandOutput, Execution, MutationResult, SourceDocument},
};
use std::io;

pub(super) fn execute(args: &VisualizeArgs) -> Execution {
    let output = args
        .output
        .clone()
        .unwrap_or_else(|| args.file.with_extension("html"));
    if same_path(&args.file, &output) {
        return io_failure(
            "visualize",
            &output,
            &io::Error::other("dump and HTML output must have different paths"),
        );
    }
    match generate(args, &output) {
        Ok(()) => success(
            CommandOutput::Mutation(MutationResult {
                command: "visualize",
                path: path_string(&args.file),
                changed: true,
                id: None,
                message: format!("created render visualizer at {}", output.display()),
            }),
            SourceDocument {
                path: path_string(&args.file),
                source: String::new(),
            },
        ),
        Err(error) => io_failure("visualize", &args.file, &error),
    }
}

fn generate(args: &VisualizeArgs, output: &std::path::Path) -> io::Result<()> {
    let bytes = std::fs::read(&args.file)?;
    let dump: Dump = serde_json::from_slice(&bytes)?;
    if dump.schema != "locus-render-dump"
        || dump.version != 1
        || dump.steps.is_empty()
        || !valid_metadata(&dump)
    {
        return Err(io::Error::other(
            "unsupported or empty render dump (expected locus-render-dump version 1)",
        ));
    }
    let data = serde_json::to_string(&dump)?
        .replace('&', "\\u0026")
        .replace('<', "\\u003c")
        .replace('>', "\\u003e")
        .replace('\u{2028}', "\\u2028")
        .replace('\u{2029}', "\\u2029");
    let html = include_str!("visualizer.html")
        .replace("__LOCUS_SCRIPT__", include_str!("visualizer.js"))
        .replace(
            "__LOCUS_RELATIONS__",
            include_str!("visualizer_relations.js"),
        )
        .replace("__LOCUS_DUMP__", &data);
    std::fs::write(output, html)
}

fn valid_metadata(dump: &Dump) -> bool {
    dump.result
        .get("exit_code")
        .and_then(serde_json::Value::as_u64)
        .is_some()
        && dump.entities.iter().all(valid_entity)
        && dump.relations.iter().all(|relation| {
            relation["kind"].is_string()
                && relation["source"].is_string()
                && string_array(&relation["ids"])
        })
}

fn valid_entity(entity: &serde_json::Value) -> bool {
    entity["id"].is_string()
        && entity["kind"].is_string()
        && entity.get("members").is_none_or(string_array)
}

fn string_array(value: &serde_json::Value) -> bool {
    value
        .as_array()
        .is_some_and(|items| items.iter().all(serde_json::Value::is_string))
}
