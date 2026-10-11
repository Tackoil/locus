//! Feedback compares proposals with the continuous solver's accepted branches.
#![allow(
    clippy::expect_used,
    reason = "regression fixtures require failure context"
)]

use locus_core::{EvaluationInputs, Scene, evaluate_assertions_with_inputs};
use locus_layout::{LayoutOptions, TextBoxes, solve_layout};

fn measured_caption() -> (EvaluationInputs, TextBoxes) {
    let mut inputs = EvaluationInputs::default();
    inputs.measured.insert(
        "caption".into(),
        locus_core::MeasuredText {
            width: 0.3,
            height: 0.2,
            role: locus_core::MeasuredRole::Normal,
        },
    );
    let mut measured = TextBoxes::new();
    measured.insert(
        "caption".into(),
        locus_tex::TexBox {
            metrics: locus_tex::BoxMetrics::new(0.3, 0.2, 0.0),
            advance: 0.3,
            ..locus_tex::TexBox::default()
        },
    );
    (inputs, measured)
}

#[test]
fn accepted_internal_label_retry_does_not_require_another_measurement_pass() {
    let parsed = locus_syntax::parse(
        "locus 0 beta 6\npoint start()\npoint end = offset(start,10,0)\narrow path(start,end)\nlabel caption(path,\"caption\") { near: middle }\npoint target = offset(start,5,-2)\nrel center(caption,target)\nassert center(caption,target)\n",
    );
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    let (mut inputs, measured) = measured_caption();
    inputs.choices.labels.insert(
        "caption".into(),
        locus_core::LabelChoice {
            side: locus_core::geom::route::RouteSide::Above,
            candidate: 1,
        },
    );
    let initial = Scene::with_inputs(parsed.file.clone(), inputs.clone()).expect("internal retry");
    let accepted_choice = initial
        .resolved()
        .expect("resolved")
        .objects
        .values()
        .find_map(|object| {
            if let locus_core::ResolvedObject::Annotation(locus_core::AnnotationSpec::Label {
                path: Some(path),
                ..
            }) = object
            {
                Some(locus_core::LabelChoice {
                    side: path.side,
                    candidate: path.candidate,
                })
            } else {
                None
            }
        })
        .expect("accepted label metadata");
    assert_eq!(accepted_choice.candidate, 0);
    assert_eq!(
        accepted_choice.side,
        locus_core::geom::route::RouteSide::Below
    );
    assert!(
        initial
            .solve_summary()
            .expect("report")
            .outer_search_attempts
            > 1
    );
    let (accepted, _) = solve_layout(&parsed.file, inputs, &measured, LayoutOptions::default())
        .expect("accepted branch already agrees with layout");
    assert_eq!(accepted.inputs.choices.labels["caption"], accepted_choice);
    assert_eq!(
        accepted.solve_summary().expect("report").measurement_passes,
        1
    );
    assert!(
        evaluate_assertions_with_inputs(
            &parsed.file,
            accepted.resolved().expect("resolved"),
            &accepted.inputs
        )
        .expect("accepted center assertion")
        .passed
    );
}

#[test]
fn new_physical_trim_still_requires_a_solve_with_visible_label_bounds() {
    let parsed = locus_syntax::parse(
        "locus 0 beta 6\npoint start()\npoint end = offset(start,4,0)\nrect source(start,2,2)\narrow path(start,end)\nlabel caption(path,\"caption\") { near: end, side: above }\nrel trim_endpoints(path,source)\n",
    );
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    let (inputs, measured) = measured_caption();
    let (accepted, layout) =
        solve_layout(&parsed.file, inputs, &measured, LayoutOptions::default())
            .expect("physical trim feedback");
    assert_eq!(
        accepted.solve_summary().expect("report").measurement_passes,
        2
    );
    assert_eq!(
        accepted.inputs.choices.retained_distances["path"],
        (1.0, 0.0)
    );
    let start = accepted
        .resolved()
        .expect("resolved")
        .objects
        .iter()
        .find_map(|(id, object)| {
            if id.name == "start"
                && let locus_core::ResolvedObject::Point(point) = object
            {
                Some(point)
            } else {
                None
            }
        })
        .expect("start point");
    assert!(
        (layout.annotations["caption"].bounds.center().x - start.geometry.x - 3.4).abs() < 1e-10
    );
}

#[test]
fn genuinely_new_label_candidate_requires_another_solve() {
    let parsed = locus_syntax::parse(
        "locus 0 beta 6\npoint start()\npoint end = offset(start,4,0)\npoint center = offset(start,2,0.25)\ncircle obstacle(center,0.12)\narrow path(start,end)\nlabel caption(path,\"caption\") { near: middle, side: above }\n",
    );
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    let (inputs, measured) = measured_caption();
    let (accepted, _) = solve_layout(&parsed.file, inputs, &measured, LayoutOptions::default())
        .expect("new candidate is solved before acceptance");
    assert_eq!(accepted.inputs.choices.labels["caption"].candidate, 1);
    assert_eq!(
        accepted.solve_summary().expect("report").measurement_passes,
        2
    );
}
