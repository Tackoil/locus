//! Parse, expand, measure, agree on solve/layout, then render and deliver the result.

use locus_layout::LayoutScene;
use locus_render::{RenderError, RenderOptions, render_svg, render_tikz, style::StyleSheet};
use locus_syntax::Span;

use super::{
    LoadedScene, dependency_failure, inspection_diagnostics, io_failure, load_scene_with_engine,
    path_string, resolved_scene, success,
};
use crate::output::{CommandOutput, Execution, RenderResult};
use crate::{Background, RenderArgs};

pub(super) fn execute(args: &RenderArgs) -> Execution {
    match run_with_dump(args) {
        Ok(execution) => execution,
        Err(execution) => *execution,
    }
}

fn run_with_dump(args: &RenderArgs) -> Result<Execution, Box<Execution>> {
    let Some(path) = super::dump::path(args) else {
        return run(args);
    };
    super::dump::validate_paths(args, &path)?;
    let (mut result, steps) = locus_core::trace::collect(|| run(args));
    if let Ok(execution) = &mut result
        && let CommandOutput::Render(rendered) = &mut execution.output
    {
        rendered.dump = Some(path_string(&path));
    }
    super::dump::write(args, &path, steps, &result)?;
    result
}

fn run(args: &RenderArgs) -> Result<Execution, Box<Execution>> {
    let _file = locus_core::metrics::span(args.file.to_string_lossy());
    let loaded = load_scene_with_engine("render", &args.file, args.tex_engine)?;
    let placed = &loaded.placed;
    let bytes =
        render(&loaded, placed, args).map_err(|error| Box::new(render_failure(&loaded, &error)))?;
    locus_core::trace::record("render_output", || {
        (
            serde_json::json!({
                "format": format_name(args), "byte_length": bytes.len(),
                "svg": (format_name(args) == "svg").then(|| String::from_utf8_lossy(&bytes)),
                "content": if format_name(args) == "png" { serde_json::json!(bytes) }
                    else { serde_json::json!(String::from_utf8_lossy(&bytes)) },
                "background": format!("{:?}", args.background),
            }),
            None,
        )
    });
    if let Some(path) = &args.output {
        locus_core::metrics::measure("write_output", || std::fs::write(path, &bytes))
            .map_err(|error| Box::new(io_failure("render", path, &error)))?;
        locus_core::trace::record("write_output", || {
            (
                serde_json::json!({
                    "path": path_string(path), "byte_length": bytes.len(),
                }),
                None,
            )
        });
    }
    let result = RenderResult {
        command: "render",
        path: path_string(&args.file),
        format: format_name(args),
        output: args.output.as_deref().map(path_string),
        diagnostics: inspection_diagnostics(&args.file, &placed.diagnostics),
        stdout: args.stdout.then_some(bytes),
        dump: None,
    };
    Ok(success(CommandOutput::Render(result), loaded.document()))
}

fn render(
    loaded: &LoadedScene,
    placed: &LayoutScene,
    args: &RenderArgs,
) -> Result<Vec<u8>, RenderError> {
    let styles = locus_core::metrics::measure("styles", || StyleSheet::from_file(&loaded.file))?;
    locus_core::trace::record("styles", || {
        (serde_json::json!({"status": "resolved"}), None)
    });
    let scene = resolved_scene(&loaded.scene);
    let options = RenderOptions {
        background: match args.background {
            Background::Transparent => None,
            Background::White => Some(locus_render::style::Color(255, 255, 255, 255)),
        },
        ..RenderOptions::default()
    };
    super::dump::render_step(loaded, placed, &styles, options);
    #[cfg(feature = "png")]
    if args.png {
        let _render = locus_core::metrics::span("render_png");
        return locus_render::png::render_with_dpi(
            scene,
            placed,
            &styles,
            options,
            args.dpi.unwrap_or(96),
        );
    }
    let text = if args.tikz {
        locus_core::metrics::measure("render_tikz", || {
            render_tikz(scene, placed, &styles, options)
        })?
    } else {
        locus_core::metrics::measure("render_svg", || render_svg(scene, placed, &styles, options))?
    };
    Ok(text.into_bytes())
}

fn format_name(args: &RenderArgs) -> &'static str {
    #[cfg(feature = "png")]
    if args.png {
        return "png";
    }
    if args.tikz { "tikz" } else { "svg" }
}

fn render_failure(loaded: &LoadedScene, error: &RenderError) -> Execution {
    let span = match error {
        RenderError::Invalid { span, .. } => *span,
        RenderError::Options | RenderError::Raster { .. } => Span::empty(1, 1, 0),
    };
    dependency_failure(
        "render",
        &loaded.as_file(),
        error.code(),
        span,
        error.to_string(),
    )
}
