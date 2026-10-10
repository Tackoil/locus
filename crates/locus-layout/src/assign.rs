//! Greedy placement in deterministic, most-constrained-first order.
//!
//! Only annotations whose anchors are resolved are eligible. Fixed marks and
//! pinned text are assigned before automatic text. Within each tier, the fewest
//! collision-free candidates wins, then the fewest total candidates. Equal
//! scores select the lowest candidate index; remaining assignment ties use that
//! candidate index, then lexicographic identifier order. Constraints are
//! recomputed after each assignment. Output is restored to declaration order.

use indexmap::IndexMap;
use locus_core::scene::{CompassDirection, LabelDirection, TextBlockPlacement};
use locus_core::{AnchorSpec, AnnotationSpec, BBox, Point, ResolvedObject, ResolvedScene};
use locus_syntax::Ident;
use locus_tex::TexBox;

use crate::boxes::{connector, dimensions, mark, text_box, valid_bounds};
use crate::candidates::{Candidate, generate};
use crate::score::Scorer;
use crate::{LayoutError, LayoutOptions, LayoutScene, PlacedAnnotation, TextBoxes};

struct Pending<'a> {
    id: &'a Ident,
    spec: &'a AnnotationSpec,
    measured: Option<&'a TexBox>,
}

struct Choice {
    pending: usize,
    candidates: Vec<Candidate>,
    anchors: Vec<Point>,
    best: usize,
    free: usize,
    automatic: bool,
}

pub(crate) fn assign(
    scene: &ResolvedScene,
    measured: &TextBoxes,
    options: LayoutOptions,
) -> Result<LayoutScene, LayoutError> {
    let mut pending = prepare(scene, measured, options)?;
    let mut assigned = IndexMap::new();
    let mut scorer = Scorer::new(scene, options);
    while !pending.is_empty() {
        let choice = choose(&pending, scene, &assigned, &scorer, options)?;
        let item = pending.remove(choice.pending);
        let placed = place(&item, &choice, options)?;
        scorer.add_annotation(&placed);
        assigned.insert(item.id.name.clone(), placed);
    }
    finish(scene, assigned, &scorer)
}

fn prepare<'a>(
    scene: &'a ResolvedScene,
    measured: &'a TextBoxes,
    options: LayoutOptions,
) -> Result<Vec<Pending<'a>>, LayoutError> {
    let mut pending = Vec::new();
    for (id, object) in &scene.objects {
        let ResolvedObject::Annotation(spec) = object else {
            continue;
        };
        let text = required_measurement(id, object, measured)?;
        if let Some(text) = text {
            dimensions(id, text, options.em_size)?;
        }
        pending.push(Pending {
            id,
            spec,
            measured: text,
        });
    }
    Ok(pending)
}

fn required_measurement<'a>(
    id: &Ident,
    object: &ResolvedObject,
    measured: &'a TextBoxes,
) -> Result<Option<&'a TexBox>, LayoutError> {
    if crate::boxes::annotation_text(object).is_none() {
        return Ok(None);
    }
    measured
        .get(&id.name)
        .map(Some)
        .ok_or_else(|| LayoutError::MissingMeasurement {
            id: id.name.clone(),
            span: id.span,
        })
}

fn choose(
    pending: &[Pending<'_>],
    scene: &ResolvedScene,
    assigned: &IndexMap<String, PlacedAnnotation>,
    scorer: &Scorer,
    options: LayoutOptions,
) -> Result<Choice, LayoutError> {
    let mut choices = Vec::new();
    for (index, item) in pending.iter().enumerate() {
        if let Some(mut choice) = eligible(item, scene, assigned, options)? {
            choice.pending = index;
            rank_candidates(&mut choice, assigned, scorer);
            choices.push(choice);
        }
    }
    choices
        .into_iter()
        .min_by(|a, b| {
            (
                a.automatic,
                a.free,
                a.candidates.len(),
                a.best,
                &pending[a.pending].id.name,
            )
                .cmp(&(
                    b.automatic,
                    b.free,
                    b.candidates.len(),
                    b.best,
                    &pending[b.pending].id.name,
                ))
        })
        .ok_or_else(|| unresolved_error(&pending[0], assigned))
}

fn rank_candidates(
    choice: &mut Choice,
    assigned: &IndexMap<String, PlacedAnnotation>,
    scorer: &Scorer,
) {
    let mut best_score = f64::INFINITY;
    for (index, candidate) in choice.candidates.iter().enumerate() {
        let score = scorer.score(candidate.bounds, assigned.values()).total();
        if score == 0.0 {
            choice.free += 1;
        }
        if score.total_cmp(&best_score).is_lt() {
            best_score = score;
            choice.best = index;
        }
    }
}

fn eligible(
    item: &Pending<'_>,
    scene: &ResolvedScene,
    assigned: &IndexMap<String, PlacedAnnotation>,
    options: LayoutOptions,
) -> Result<Option<Choice>, LayoutError> {
    let Some(anchors) = anchors(item.spec)
        .into_iter()
        .map(|a| resolve(a, assigned))
        .collect::<Option<Vec<_>>>()
    else {
        return Ok(None);
    };
    let (candidates, automatic) = if let Some(measured) = item.measured {
        let (anchor, direction, gap) = text_request(item.spec, scene.bbox, &anchors, options.gap);
        if !anchor.is_finite() || !gap.is_finite() || gap < 0.0 {
            return Err(LayoutError::invalid(item.id));
        }
        let size = dimensions(item.id, measured, options.em_size)?;
        (
            generate(scene, anchor, size, direction, gap),
            direction == LabelDirection::Auto,
        )
    } else {
        (Vec::new(), false)
    };
    if candidates.iter().any(|c| !valid_bounds(c.bounds)) {
        return Err(LayoutError::invalid(item.id));
    }
    Ok(Some(Choice {
        pending: 0,
        candidates,
        anchors,
        best: 0,
        free: 0,
        automatic,
    }))
}

fn text_request(
    spec: &AnnotationSpec,
    bbox: BBox,
    anchors: &[Point],
    default_gap: f64,
) -> (Point, LabelDirection, f64) {
    match spec {
        AnnotationSpec::Label { direction, gap, .. } => {
            (anchors[0], *direction, gap.unwrap_or(default_gap))
        }
        AnnotationSpec::TextBlock { placement, .. } => {
            let (anchor, direction) = block_request(placement, bbox, anchors);
            (anchor, direction, default_gap)
        }
        _ => (bbox.center(), LabelDirection::Center, 0.0),
    }
}

fn block_request(
    placement: &TextBlockPlacement,
    bbox: BBox,
    anchors: &[Point],
) -> (Point, LabelDirection) {
    let (anchor, compass) = match placement {
        TextBlockPlacement::Below => (bbox.south(), CompassDirection::S),
        TextBlockPlacement::Above => (bbox.north(), CompassDirection::N),
        TextBlockPlacement::Left => (bbox.west(), CompassDirection::W),
        TextBlockPlacement::Right => (bbox.east(), CompassDirection::E),
        TextBlockPlacement::Anchor { direction, .. } => return (anchors[0], *direction),
    };
    (anchor, LabelDirection::Compass(compass))
}

fn anchors(spec: &AnnotationSpec) -> Vec<&AnchorSpec> {
    match spec {
        AnnotationSpec::Arrow { from, to, .. } => vec![from, to],
        AnnotationSpec::TextBlock {
            placement: TextBlockPlacement::Anchor { anchor, .. },
            ..
        }
        | AnnotationSpec::Label { anchor, .. } => vec![anchor],
        _ => Vec::new(),
    }
}

fn resolve(anchor: &AnchorSpec, assigned: &IndexMap<String, PlacedAnnotation>) -> Option<Point> {
    match anchor {
        AnchorSpec::Point(point) => Some(*point),
        AnchorSpec::AnnotationCenter { id, .. } => assigned.get(id).map(|a| a.bounds.center()),
    }
}

fn unresolved_error(
    item: &Pending<'_>,
    assigned: &IndexMap<String, PlacedAnnotation>,
) -> LayoutError {
    for anchor in anchors(item.spec) {
        if let AnchorSpec::AnnotationCenter { id, span } = anchor
            && !assigned.contains_key(id)
        {
            return LayoutError::Anchor {
                id: id.clone(),
                span: *span,
            };
        }
    }
    LayoutError::invalid(item.id)
}

fn place(
    item: &Pending<'_>,
    choice: &Choice,
    options: LayoutOptions,
) -> Result<PlacedAnnotation, LayoutError> {
    let Some(measured) = item.measured else {
        return mark(item.id, item.spec, &choice.anchors, options);
    };
    let bounds = choice.candidates[choice.best].bounds;
    let text = text_box(bounds, measured, options.em_size);
    if !text.baseline.is_finite() {
        return Err(LayoutError::invalid(item.id));
    }
    let leader = choice
        .anchors
        .first()
        .filter(|_| options.label_connectors || !matches!(item.spec, AnnotationSpec::Label { .. }))
        .map_or(crate::Leader::None, |a| connector(*a, bounds));
    Ok(PlacedAnnotation {
        id: item.id.clone(),
        spec: item.spec.clone(),
        bounds,
        text: Some(text),
        leader,
        strokes: Vec::new(),
        candidate_index: choice.best,
    })
}

fn finish(
    scene: &ResolvedScene,
    mut assigned: IndexMap<String, PlacedAnnotation>,
    scorer: &Scorer,
) -> Result<LayoutScene, LayoutError> {
    let mut annotations = IndexMap::new();
    let mut bbox = scene.bbox;
    for id in scene.objects.keys() {
        if let Some(annotation) = assigned.shift_remove(&id.name) {
            bbox.include_bbox(annotation.bounds);
            if !valid_bounds(bbox) {
                return Err(LayoutError::invalid(id));
            }
            annotations.insert(id.name.clone(), annotation);
        }
    }
    let diagnostics = crate::report::collisions(
        &annotations,
        scorer,
        locus_core::Tolerance::from_bbox(&scene.bbox),
    );
    Ok(LayoutScene {
        annotations,
        geometry_bbox: scene.bbox,
        bbox,
        diagnostics,
    })
}
