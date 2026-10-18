//! Browser bindings over the same measured solve/layout and SVG pipeline as the CLI.

use locus_core::{Evaluation, evaluate_assertions_with_inputs};
use locus_layout::{LayoutError, LayoutOptions, SolveLayoutError, measurement::measure_file};
use locus_render::{RenderError, RenderOptions, style::StyleSheet};
use locus_syntax::{Diagnostic, Span};
use locus_tex::{PlaintextEngine, RexEngine};
use serde_json::{Value, json};
use wasm_bindgen::prelude::*;

/// Renders complete `.locus` source and returns a JSON response string.
///
/// Success has `ok: true`, `svg`, `warnings`, and `assertions`. Failed assertions
/// do not suppress SVG output, matching CLI rendering. Pipeline failures have
/// `ok: false`, `svg: null`, and source-anchored `diagnostics`.
/// No filesystem, network, clocks, or DOM access is required by this operation.
#[wasm_bindgen]
#[must_use]
pub fn render(source: &str) -> String {
    match compile(source) {
        Ok(output) => output,
        Err(diagnostics) => json!({"ok": false, "svg": null, "diagnostics": diagnostics}),
    }
    .to_string()
}

fn compile(source: &str) -> Result<Value, Vec<Diagnostic>> {
    let parsed = locus_syntax::parse(source);
    if !parsed.diagnostics.is_empty() {
        return Err(parsed.diagnostics);
    }
    let (inputs, measured) = measure_file(
        &parsed.file,
        &PlaintextEngine::new(),
        &RexEngine,
        locus_core::LayoutMetrics::default(),
    )
    .map_err(|error| layout_error(&error))?;
    let metrics = inputs.metrics;
    let options = LayoutOptions {
        em_size: metrics.em_size,
        gap: metrics.gap,
        margin: metrics.margin,
        stroke_width: metrics.stroke_width,
        mark_size: metrics.mark_size,
        label_connectors: false,
    };
    let (scene, placed) = locus_layout::solve_layout(&parsed.file, inputs, &measured, options)
        .map_err(solve_error)?;
    let resolved = match &scene.evaluation {
        Evaluation::Resolved(resolved)
        | Evaluation::Solved {
            scene: resolved, ..
        } => resolved,
    };
    let styles = StyleSheet::from_file(&scene.file).map_err(|error| render_error(&error))?;
    let svg = locus_render::render_svg(resolved, &placed, &styles, RenderOptions::default())
        .map_err(|error| render_error(&error))?;
    let assertions = evaluate_assertions_with_inputs(&scene.source_file, resolved, &scene.inputs)
        .map_err(eval_error)?;
    Ok(json!({
        "ok": true, "svg": svg, "diagnostics": [],
        "warnings": placed.diagnostics, "assertions": assertions,
    }))
}

fn eval_error(error: locus_core::EvalError) -> Vec<Diagnostic> {
    vec![Diagnostic::new(error.code, error.span, error.message)]
}

fn solve_error(error: SolveLayoutError) -> Vec<Diagnostic> {
    match error {
        SolveLayoutError::Evaluation(error) => eval_error(error),
        SolveLayoutError::Layout(error) => layout_error(&error),
    }
}

fn layout_error(error: &LayoutError) -> Vec<Diagnostic> {
    let span = match error {
        LayoutError::MissingMeasurement { span, .. }
        | LayoutError::Anchor { span, .. }
        | LayoutError::Invalid { span, .. }
        | LayoutError::Conflict { span, .. }
        | LayoutError::Measurement { span, .. } => *span,
        LayoutError::Options => Span::empty(1, 1, 0),
    };
    vec![Diagnostic::new(error.code(), span, error.to_string())]
}

fn render_error(error: &RenderError) -> Vec<Diagnostic> {
    let span = match error {
        RenderError::Invalid { span, .. } => *span,
        RenderError::Options | RenderError::Raster { .. } => Span::empty(1, 1, 0),
    };
    vec![Diagnostic::new(error.code(), span, error.to_string())]
}

#[cfg(test)]
mod tests;
