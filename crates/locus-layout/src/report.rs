//! Residual collisions in core's inspection vocabulary.
//!
//! Label pairs are reported once in declaration order, including both spans.
//! Stroke/margin collisions are singleton `label_collision` diagnostics. This
//! final pass also catches strokes added after a label (for example an arrow
//! that depends on that label's center). Pinned labels are reported as well.
//! Each text connector is excluded only from its own label's collision check;
//! attachment to that box is intentional, even with a positive stroke width.

use crate::score::{Scorer, boxes_overlap};
use crate::{LayoutScene, PlacedAnnotation};
use indexmap::IndexMap;
use locus_core::{InspectDiagnostic, Tolerance};

/// Appends layout diagnostics to an existing geometry inspection report.
pub fn extend_inspection(report: &mut locus_core::InspectionReport, layout: &LayoutScene) {
    report
        .diagnostics
        .extend(layout.diagnostics.iter().cloned());
}

pub(crate) fn collisions(
    annotations: &IndexMap<String, PlacedAnnotation>,
    scorer: &Scorer,
    tolerance: Tolerance,
) -> Vec<InspectDiagnostic> {
    let labels: Vec<_> = annotations.values().filter(|a| a.text.is_some()).collect();
    let mut diagnostics = Vec::new();
    for (index, label) in labels.iter().enumerate() {
        if scorer.stroke_penalty(label.bounds, Some(&label.id.name))
            + scorer.margin_penalty(label.bounds)
            > 0.0
        {
            diagnostics.push(diagnostic(&[label]));
        }
        for other in &labels[index + 1..] {
            if boxes_overlap(label.bounds, other.bounds, tolerance) {
                diagnostics.push(diagnostic(&[label, other]));
            }
        }
    }
    diagnostics
}

fn diagnostic(labels: &[&PlacedAnnotation]) -> InspectDiagnostic {
    InspectDiagnostic::LabelCollision {
        labels: labels.iter().map(|a| a.id.name.clone()).collect(),
        spans: labels.iter().map(|a| a.id.span).collect(),
    }
}
