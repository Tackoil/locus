//! Path-label candidate preferences remain inside the authored near region.

use crate::LayoutOptions;
use crate::discrete::{LayoutFeedback, route};
use crate::score::Scorer;
use locus_core::geom::route::RouteSide;
use locus_core::inputs::LabelChoice;
use locus_core::{AnnotationSpec, BBox, Point, ResolvedObject, Scene};
use locus_syntax::{OpStmt, Statement};

pub(crate) fn search(scene: &Scene, feedback: &mut LayoutFeedback, options: LayoutOptions) {
    let Some(resolved) = scene.resolved() else {
        return;
    };
    for (id, object) in &resolved.objects {
        let ResolvedObject::Annotation(AnnotationSpec::Label {
            path: Some(path),
            bounds: Some(bounds),
            ..
        }) = object
        else {
            continue;
        };
        if fixed_parameters(scene, &id.name) {
            feedback.label_choices.insert(
                id.name.clone(),
                LabelChoice {
                    side: path.side,
                    candidate: path.candidate,
                },
            );
            continue;
        }
        let Some(logical) = crate::discrete::object(scene, &path.path).and_then(route) else {
            continue;
        };
        let Some(op) = label_op(scene, &id.name) else {
            continue;
        };
        let sides = crate::routing::authored_side(op).map_or_else(
            || locus_core::preset::LABEL_PAGE_SIDE_ORDER.to_vec(),
            |side| vec![side],
        );
        let old_retained = previous_retained(scene, &path.path, &logical);
        let old_anchor: Point = logical
            .sample(&(old_retained.0 + (old_retained.1 - old_retained.0) * path.parameter))
            .into();
        let planned = crate::routing::planned_route(scene, &path.path, feedback).unwrap_or(logical);
        let retained = crate::routing::retained_range(&planned, &path.path, feedback);
        let scorer = visual_scorer(resolved, scene, feedback, &id.name, &path.path, options);
        let rejected = scene
            .inputs
            .choices
            .labels
            .get(&id.name)
            .copied()
            .filter(|choice| choice.side != path.side || choice.candidate != path.candidate);
        let choice = select(
            &id.name, path, *bounds, old_anchor, &planned, retained, &sides, rejected, feedback,
            &scorer,
        );
        feedback.label_choices.insert(id.name.clone(), choice);
    }
}

fn previous_retained(
    scene: &Scene,
    path: &str,
    logical: &locus_core::geom::route::Route,
) -> (f64, f64) {
    scene
        .inputs
        .choices
        .retained_distances
        .get(path)
        .map_or_else(
            || {
                scene
                    .inputs
                    .choices
                    .retained_routes
                    .get(path)
                    .copied()
                    .unwrap_or((0.0, 1.0))
            },
            |(prefix, suffix)| {
                (
                    logical.parameter_for_distance(prefix),
                    logical.parameter_for_distance(&(logical.length() - suffix)),
                )
            },
        )
}

fn label_op<'a>(scene: &'a Scene, name: &str) -> Option<&'a OpStmt> {
    scene
        .file
        .statements
        .iter()
        .find_map(|statement| match statement {
            Statement::Op(op) if op.id.name == name => Some(op),
            _ => None,
        })
}

#[allow(
    clippy::too_many_arguments,
    reason = "candidate evaluation retains all solved parameter context"
)]
fn select(
    name: &str,
    path: &locus_core::scene::PathLabel,
    bounds: BBox,
    old_anchor: Point,
    logical: &locus_core::geom::route::Route,
    retained: (f64, f64),
    sides: &[RouteSide],
    rejected: Option<LabelChoice>,
    feedback: &LayoutFeedback,
    scorer: &Scorer,
) -> LabelChoice {
    let mut best = LabelChoice {
        side: path.side,
        candidate: path.candidate,
    };
    let planned_anchor: Point = logical
        .sample(&(retained.0 + (retained.1 - retained.0) * path.parameter))
        .into();
    let mut score = penalty(
        name,
        shifted(bounds, planned_anchor - old_anchor),
        feedback,
        scorer,
    );
    for choice in candidates(sides, rejected) {
        let t = candidate_parameter(path.region, choice.candidate);
        let anchor: Point = logical
            .sample(&(retained.0 + (retained.1 - retained.0) * t))
            .into();
        let shift = anchor - old_anchor + offset(choice.side, bounds, path.offset)
            - offset(path.side, bounds, path.offset);
        let proposed = shifted(bounds, shift);
        let candidate_score = penalty(name, proposed, feedback, scorer);
        if candidate_score < score {
            score = candidate_score;
            best = choice;
        }
    }
    best
}

fn shifted(bounds: BBox, shift: locus_core::Vec2) -> BBox {
    BBox {
        min: bounds.min + shift,
        max: bounds.max + shift,
    }
}

fn candidate_parameter(region: (f64, f64), candidate: usize) -> f64 {
    match candidate {
        1 => region.0,
        2 => region.1,
        _ => region.0.midpoint(region.1),
    }
}

fn offset(side: RouteSide, bounds: BBox, gap: f64) -> locus_core::Vec2 {
    match side {
        RouteSide::Above => locus_core::Vec2::new(0.0, bounds.height() / 2.0 + gap),
        RouteSide::Below => locus_core::Vec2::new(0.0, -bounds.height() / 2.0 - gap),
        RouteSide::Right => locus_core::Vec2::new(bounds.width() / 2.0 + gap, 0.0),
        RouteSide::Left => locus_core::Vec2::new(-bounds.width() / 2.0 - gap, 0.0),
    }
}

fn penalty(name: &str, bounds: BBox, feedback: &LayoutFeedback, scorer: &Scorer) -> f64 {
    scorer
        .score(
            bounds,
            feedback
                .layout
                .annotations
                .values()
                .filter(|annotation| annotation.id.name != name),
        )
        .total()
}

fn visual_scorer(
    resolved: &locus_core::ResolvedScene,
    scene: &Scene,
    feedback: &LayoutFeedback,
    name: &str,
    path: &str,
    options: LayoutOptions,
) -> Scorer {
    let mut visible = resolved.clone();
    for (id, object) in &mut visible.objects {
        let identity = crate::discrete::alias_identity(scene, &id.name);
        if let Some(predicted) = crate::routing::visible_obstacle(scene, &identity, feedback) {
            *object = predicted;
        }
    }
    let mut scorer = Scorer::new(&visible, options);
    for annotation in feedback.layout.annotations.values() {
        let identity = crate::discrete::alias_identity(scene, &annotation.id.name);
        if identity == name || identity == path {
            continue;
        }
        let mut predicted = annotation.clone();
        if let Some(logical) = crate::routing::planned_route(scene, &identity, feedback) {
            let range = crate::routing::retained_range(&logical, &identity, feedback);
            predicted.leader =
                crate::clipping::visible_leader(&logical.retained(range), &predicted.leader);
        }
        scorer.add_annotation(&predicted);
    }
    scorer
}

// A core retry already attempted the input choice under this solve's constraints.
// Keep its accepted candidate as the baseline instead of immediately requesting
// the rejected input again. Every distinct proposal still requires another solve.
fn candidates(
    sides: &[RouteSide],
    rejected: Option<LabelChoice>,
) -> impl Iterator<Item = LabelChoice> + '_ {
    sides
        .iter()
        .flat_map(|side| {
            (0..3).map(move |candidate| LabelChoice {
                side: *side,
                candidate,
            })
        })
        .filter(move |choice| Some(*choice) != rejected)
}

// Equality-determined t/o cannot follow a nominal candidate's predicted shift.
// Retain the actual placement and its collision warnings instead of asking the
// continuous solve to undo a hard source constraint just to improve a preview.
fn fixed_parameters(scene: &Scene, id: &str) -> bool {
    let Some(summary) = scene.solve_summary() else {
        return false;
    };
    let parameters = summary
        .dof
        .objects
        .iter()
        .filter(|object| {
            object.id == id && object.class == locus_core::UnknownClass::RouteLabelParameter
        })
        .collect::<Vec<_>>();
    !parameters.is_empty()
        && parameters
            .iter()
            .all(|object| object.hint_settled_dofs == 0)
}
