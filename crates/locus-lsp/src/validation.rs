//! Validate with the same measured text and layout feedback as the CLI.

use locus_core::LayoutMetrics;
use locus_layout::{LayoutOptions, SolveLayoutError, measurement::measure_file, solve_layout};
use locus_syntax::File;
use locus_tex::{PlaintextEngine, RexEngine};
use lsp_types::Diagnostic;

use crate::{diagnostics, document::Document};

pub(crate) fn evaluate(document: &Document, file: &File) -> Option<Diagnostic> {
    let (inputs, measured) = match measure_file(
        file,
        &PlaintextEngine::new(),
        &RexEngine,
        LayoutMetrics::default(),
    ) {
        Ok(value) => value,
        Err(error) => return Some(diagnostics::layout(document, &error)),
    };
    let metrics = inputs.metrics;
    let options = LayoutOptions {
        em_size: metrics.em_size,
        gap: metrics.gap,
        margin: metrics.margin,
        stroke_width: metrics.stroke_width,
        mark_size: metrics.mark_size,
        label_connectors: false,
    };
    solve_layout(file, inputs, &measured, options)
        .err()
        .map(|error| match error {
            SolveLayoutError::Evaluation(error) => diagnostics::evaluation(document, &error),
            SolveLayoutError::Layout(error) => diagnostics::layout(document, &error),
        })
}
