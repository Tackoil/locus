//! Shared layout preparation and actionable collision feedback.

use locus_core::{GeometryBounds, InspectDiagnostic, LayoutCollision};
use locus_layout::{LayoutError, LayoutOptions};
use locus_syntax::Span;

use super::{LoadedFile, dependency_failure};
use crate::output::Execution;

pub(super) fn options(metrics: locus_core::preset::LayoutMetrics) -> LayoutOptions {
    LayoutOptions {
        em_size: metrics.em_size,
        gap: metrics.gap,
        margin: metrics.margin,
        stroke_width: metrics.stroke_width,
        mark_size: metrics.mark_size,
        label_connectors: false,
    }
}

pub(super) fn file_failure(
    command: &'static str,
    loaded: &LoadedFile,
    error: &LayoutError,
) -> Execution {
    let span = match error {
        LayoutError::MissingMeasurement { span, .. }
        | LayoutError::Anchor { span, .. }
        | LayoutError::Invalid { span, .. }
        | LayoutError::Conflict { span, .. }
        | LayoutError::Measurement { span, .. } => *span,
        LayoutError::Options => Span::empty(1, 1, 0),
    };
    dependency_failure(command, loaded, error.code(), span, error.to_string())
}

pub(super) fn details(diagnostic: &InspectDiagnostic) -> Vec<LayoutCollision> {
    match diagnostic {
        InspectDiagnostic::LabelCollision { details, .. } => details.clone(),
        _ => Vec::new(),
    }
}

pub(super) fn message(diagnostic: &InspectDiagnostic, labels: &[String]) -> String {
    let causes = details(diagnostic)
        .iter()
        .map(cause)
        .collect::<Vec<_>>()
        .join("; ");
    format!("layout collision for {}: {causes}", labels.join(", "))
}

fn cause(collision: &LayoutCollision) -> String {
    match collision {
        LayoutCollision::LabelOverlap { .. } => "text boxes overlap".to_owned(),
        LayoutCollision::StrokeIntersection { object, span, .. } => {
            format!("intersects stroke `{object}` at {}:{}", span.line, span.col)
        }
        LayoutCollision::MarginViolation { .. } => "exceeds permitted figure margin".to_owned(),
    }
}

pub(super) fn notes(diagnostic: &InspectDiagnostic) -> Vec<String> {
    details(diagnostic).iter().flat_map(bounds_notes).collect()
}

fn bounds_notes(collision: &LayoutCollision) -> Vec<String> {
    match collision {
        LayoutCollision::LabelOverlap { bounds, .. } => vec![
            format!("first label bounds: {}", bounds_text(bounds[0])),
            format!("second label bounds: {}", bounds_text(bounds[1])),
        ],
        LayoutCollision::StrokeIntersection {
            object,
            label_bounds,
            tested_bounds,
            object_bounds,
            ..
        } => {
            let mut notes = vec![
                format!("label bounds: {}", bounds_text(*label_bounds)),
                format!(
                    "stroke test bounds (including half stroke width): {}",
                    bounds_text(*tested_bounds)
                ),
            ];
            if let Some(bounds) = object_bounds {
                notes.push(format!("`{object}` bounds: {}", bounds_text(*bounds)));
            }
            notes
        }
        LayoutCollision::MarginViolation {
            label_bounds,
            allowed_bounds,
            ..
        } => vec![
            format!("label bounds: {}", bounds_text(*label_bounds)),
            format!("permitted bounds: {}", bounds_text(*allowed_bounds)),
        ],
    }
}

fn bounds_text(bounds: GeometryBounds) -> String {
    format!(
        "x [{:.6}, {:.6}], y [{:.6}, {:.6}] (scene units)",
        bounds.min_x, bounds.max_x, bounds.min_y, bounds.max_y
    )
}

pub(super) fn suggestion(diagnostic: &InspectDiagnostic) -> Option<String> {
    matches!(diagnostic, InspectDiagnostic::LabelCollision { .. }).then(||
        "adjust the label's dir/gap or anchor, or increase spacing around the named obstacle; for margin violations move text closer to the figure".to_owned())
}
