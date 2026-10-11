//! Residual collisions in core's inspection vocabulary.
//!
//! Label pairs are reported once in declaration order, including both spans.
//! Stroke/margin collisions retain obstacle ids, causes, and bounds. This
//! final pass also catches strokes added after a label (for example an arrow
//! that depends on that label's center). Pinned labels are reported as well.
//! Each text connector is excluded only from its own label's collision check;
//! attachment to that box is intentional, even with a positive stroke width.

use crate::score::{Scorer, boxes_overlap};
use crate::{LayoutScene, PlacedAnnotation};
use indexmap::IndexMap;
use locus_core::{InspectDiagnostic, LayoutCollision, Tolerance};

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
        let details = scorer.collision_details(label.bounds, &label.id.name);
        if !details.is_empty() {
            diagnostics.push(diagnostic(&[label], details));
        }
        for other in &labels[index + 1..] {
            if boxes_overlap(label.bounds, other.bounds, tolerance) {
                diagnostics.push(diagnostic(
                    &[label, other],
                    vec![LayoutCollision::LabelOverlap {
                        labels: [label.id.name.clone(), other.id.name.clone()],
                        spans: [label.id.span, other.id.span],
                        bounds: [label.bounds.into(), other.bounds.into()],
                    }],
                ));
            }
        }
    }
    diagnostics
}

fn diagnostic(labels: &[&PlacedAnnotation], details: Vec<LayoutCollision>) -> InspectDiagnostic {
    InspectDiagnostic::LabelCollision {
        labels: labels.iter().map(|a| a.id.name.clone()).collect(),
        spans: labels.iter().map(|a| a.id.span).collect(),
        details,
    }
}

/// Recomputes diagnostics from accepted visible routes and measured label boxes.
pub(crate) fn refresh_collisions(
    scene: &locus_core::ResolvedScene,
    layout: &mut LayoutScene,
    options: crate::LayoutOptions,
) {
    let mut visible = scene.clone();
    for (id, object) in &mut visible.objects {
        if let Some(route) = layout.visible_paths.get(&id.name) {
            *object = locus_core::ResolvedObject::Segment(locus_core::Segment {
                start: route.from.clone().into(),
                end: route.to.clone().into(),
            });
        }
    }
    let mut scorer = Scorer::new(&visible, options);
    for annotation in layout.annotations.values() {
        scorer.add_annotation(annotation);
    }
    layout.diagnostics = collisions(
        &layout.annotations,
        &scorer,
        Tolerance::from_bbox(&scene.bbox),
    );
}
