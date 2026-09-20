//! Parse and solve once, then measure, place, render, and deliver the result.

use locus_layout::{LayoutError, LayoutOptions, LayoutScene, boxes::measure_scene, layout};
use locus_render::{RenderError, RenderOptions, render_svg, render_tikz, style::StyleSheet};
use locus_syntax::Span;
use locus_tex::{FontSize, MockEngine, PlaintextEngine, RexEngine, TexEngine};

use super::{
    LoadedScene, dependency_failure, inspection_diagnostics, io_failure, load_scene, path_string,
    resolved_scene, success,
};
use crate::output::{CommandOutput, Execution, RenderResult};
use crate::{Background, RenderArgs, TextEngine};

pub(super) fn execute(args: &RenderArgs) -> Execution {
    match run(args) {
        Ok(execution) => execution,
        Err(execution) => *execution,
    }
}

fn run(args: &RenderArgs) -> Result<Execution, Box<Execution>> {
    let loaded = load_scene("render", &args.file)?;
    let placed = place(&loaded, args.tex_engine)
        .map_err(|error| Box::new(layout_failure(&loaded, &error)))?;
    let bytes = render(&loaded, &placed, args)
        .map_err(|error| Box::new(render_failure(&loaded, &error)))?;
    if let Some(path) = &args.output {
        std::fs::write(path, &bytes)
            .map_err(|error| Box::new(io_failure("render", path, &error)))?;
    }
    let result = RenderResult {
        command: "render",
        path: path_string(&args.file),
        format: format_name(args),
        output: args.output.as_deref().map(path_string),
        diagnostics: inspection_diagnostics(&args.file, &placed.diagnostics),
        stdout: args.stdout.then_some(bytes),
    };
    Ok(success(CommandOutput::Render(result), loaded.document()))
}

fn place(loaded: &LoadedScene, engine: TextEngine) -> Result<LayoutScene, LayoutError> {
    let tex: &dyn TexEngine = match engine {
        TextEngine::Rex => &RexEngine,
        TextEngine::Mock => &MockEngine,
    };
    let scene = resolved_scene(&loaded.scene);
    let measured = measure_scene(scene, &PlaintextEngine::new(), tex, FontSize::DEFAULT)?;
    layout(
        scene,
        &measured,
        LayoutOptions {
            em_size: 0.28,
            ..LayoutOptions::default()
        },
    )
}

fn render(
    loaded: &LoadedScene,
    placed: &LayoutScene,
    args: &RenderArgs,
) -> Result<Vec<u8>, RenderError> {
    let styles = StyleSheet::from_file(&loaded.file)?;
    let scene = resolved_scene(&loaded.scene);
    let options = RenderOptions {
        background: match args.background {
            Background::Transparent => None,
            Background::White => Some(locus_render::style::Color(255, 255, 255)),
        },
        ..RenderOptions::default()
    };
    #[cfg(feature = "png")]
    if args.png {
        return locus_render::png::render_with_dpi(
            scene,
            placed,
            &styles,
            options,
            args.dpi.unwrap_or(96),
        );
    }
    let text = if args.tikz {
        render_tikz(scene, placed, &styles, options)?
    } else {
        render_svg(scene, placed, &styles, options)?
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

fn layout_failure(loaded: &LoadedScene, error: &LayoutError) -> Execution {
    let span = match error {
        LayoutError::MissingMeasurement { span, .. }
        | LayoutError::Anchor { span, .. }
        | LayoutError::Invalid { span, .. }
        | LayoutError::Measurement { span, .. } => *span,
        LayoutError::Options => Span::empty(1, 1, 0),
    };
    dependency_failure(
        "render",
        &loaded.as_file(),
        error.code(),
        span,
        error.to_string(),
    )
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
