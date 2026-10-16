//! Measure expanded immutable text before scene evaluation.

use locus_core::{EvaluationInputs, MeasuredRole, MeasuredText, preset::LayoutMetrics};
use locus_syntax::{
    Constructor, Expr, File, OpStmt, PropValue, Statement, TextSegmentKind, TextValue,
};
use locus_tex::{FontSize, TexEngine, TextSegment, render_segments};

use crate::{LayoutError, TextBoxes};

/// Measures expanded text with actual font engines for both solving and layout.
///
/// # Errors
/// Returns a source-anchored text measurement error. Expansion diagnostics are
/// left to scene evaluation, which supplies the established diagnostic format.
pub fn measure_file(
    source: &File,
    plain: &dyn TexEngine,
    tex: &dyn TexEngine,
    metrics: LayoutMetrics,
) -> Result<(EvaluationInputs, TextBoxes), LayoutError> {
    let _measurement = locus_core::metrics::span("measure_text");
    let expanded = locus_core::metrics::measure("expand", || locus_syntax::expand(source));
    locus_core::trace::record("expand_text", || (serde_json::json!(expanded), None));
    let mut inputs = EvaluationInputs {
        metrics,
        ..EvaluationInputs::default()
    };
    let mut boxes = TextBoxes::new();
    if !expanded.diagnostics.is_empty() {
        return Ok((inputs, boxes));
    }
    for statement in &expanded.file.statements {
        let Statement::Op(op) = statement else {
            continue;
        };
        let Some(text) = content(op) else { continue };
        let _text = locus_core::metrics::span(op.id.name.as_str());
        let role = role(op);
        let size = match role {
            MeasuredRole::Normal => metrics.normal_font_size,
            MeasuredRole::Title => metrics.title_font_size,
        };
        let segments = segments(text);
        let measured =
            render_segments(&segments, plain, tex, FontSize::new(size)).map_err(|source| {
                LayoutError::Measurement {
                    id: op.id.name.clone(),
                    span: text.span,
                    source,
                }
            })?;
        inputs.measured.insert(
            op.id.name.clone(),
            MeasuredText {
                width: measured.metrics.width * metrics.em_size,
                height: (measured.metrics.height + measured.metrics.depth) * metrics.em_size,
                role,
            },
        );
        boxes.insert(op.id.name.clone(), measured);
    }
    Ok((inputs, boxes))
}

fn content(op: &OpStmt) -> Option<&TextValue> {
    let Constructor::Call { name, args, .. } = &op.ctor else {
        return None;
    };
    if !matches!(name.name.as_str(), "label" | "text_block") {
        return None;
    }
    args.iter().find_map(|arg| match arg {
        Expr::String(text) | Expr::Tex(text) => Some(text),
        _ => None,
    })
}

fn role(op: &OpStmt) -> MeasuredRole {
    if op.props.items.iter().any(|prop| {
        prop.name.name == "role"
            && matches!(&prop.value, PropValue::Expr(Expr::Ref(id)) if id.name == "title")
    }) {
        MeasuredRole::Title
    } else {
        MeasuredRole::Normal
    }
}

fn segments(text: &TextValue) -> Vec<TextSegment<'_>> {
    text.segments
        .iter()
        .map(|segment| match segment.kind {
            TextSegmentKind::Plain => TextSegment::Plain(&segment.source),
            TextSegmentKind::Tex => TextSegment::Tex(&segment.source),
        })
        .collect()
}
