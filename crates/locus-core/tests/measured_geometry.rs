//! Measured layout, edge, and route contract.
#![allow(
    clippy::expect_used,
    reason = "fixture failures need explicit test context"
)]

use locus_core::solve::UnknownClass;
use locus_core::{
    EvaluationInputs, MeasuredRole, MeasuredText, Scene, evaluate_assertions_with_inputs,
};
use locus_syntax::parse;

fn scene(source: &str, inputs: EvaluationInputs) -> Scene {
    let parsed = parse(source);
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    Scene::with_inputs(parsed.file, inputs).expect("measured geometry scene")
}

fn assert_scene(scene: &Scene) {
    let report = evaluate_assertions_with_inputs(
        &scene.source_file,
        scene.resolved().expect("resolved scene"),
        &scene.inputs,
    )
    .expect("milestone one assertions");
    assert!(report.passed, "{report:?}");
}

fn measured_caption() -> EvaluationInputs {
    let mut inputs = EvaluationInputs::default();
    inputs.measured.insert(
        "caption".to_owned(),
        MeasuredText {
            width: 3.0,
            height: 0.4,
            role: MeasuredRole::Normal,
        },
    );
    inputs
}

#[test]
fn standalone_label_is_centered_on_a_fixed_size_automatic_box() {
    let source = include_str!("../../../tests/fixtures/programs/measurement/automatic_label.locus");
    let mut scene = scene(source, measured_caption());
    assert_scene(&scene);
    assert_eq!(
        scene.solve_summary().expect("automatic box solve").unknowns,
        6
    );
    let resolved = scene.resolved().expect("resolved label");
    let bounds = resolved
        .objects
        .iter()
        .find(|(id, _)| id.name == "caption")
        .expect("caption")
        .1
        .bbox()
        .expect("measured caption bounds");
    assert!((bounds.max.x - bounds.min.x - 3.0).abs() < 1.0e-10);
    assert!((bounds.max.y - bounds.min.y - 0.4).abs() < 1.0e-10);
    let before = scene.evaluation.clone();
    scene.recompute().expect("repeat measured solve");
    assert_eq!(scene.evaluation, before);
}

#[test]
fn every_automatic_box_constructor_retains_authored_dimensions() {
    for kind in ["rect", "diamond", "rounded_rect"] {
        let source = format!(
            "locus 0 beta 6\n{kind} panel()\nlabel caption(\"Measured caption\")\nrel panel.width == 4\nrel panel.height == 2\nrel center(caption, panel)\nassert panel.width == 4\nassert panel.height == 2\nassert center(caption, panel)\n"
        );
        assert_scene(&scene(&source, measured_caption()));
    }
}

#[test]
fn edge_constraints_place_a_divider_across_the_box() {
    assert_scene(&scene(
        include_str!("../../../tests/fixtures/programs/measurement/edge_divider.locus"),
        EvaluationInputs::default(),
    ));
}

#[test]
fn aliased_edges_retain_live_piece_derivatives() {
    let source = include_str!("../../../tests/fixtures/programs/measurement/edge_divider.locus")
        .replace(
            "point start()",
            "segment west = edge(panel, W)\nsegment east = edge(panel, E)\npoint start()",
        )
        .replace("on(start, edge(panel, W))", "on(start, west)")
        .replace("on(finish, edge(panel, E))", "on(finish, east)");
    assert_scene(&scene(&source, EvaluationInputs::default()));
}

#[test]
fn direct_arrow_length_uses_the_active_preset_metric() {
    let mut inputs = EvaluationInputs::default();
    inputs.metrics.short_length = 1.75;
    assert_scene(&scene(
        include_str!("../../../tests/fixtures/programs/measurement/direct_metric.locus"),
        inputs,
    ));
}

#[test]
fn collection_union_bounds_and_inset_padding_agree_with_assertions() {
    let mut inputs = EvaluationInputs::default();
    inputs.metrics.padding = 0.375;
    assert_scene(&scene(
        include_str!("../../../tests/fixtures/programs/measurement/collection_insets.locus"),
        inputs,
    ));
}

#[test]
fn all_inset_sides_use_the_content_bounds_and_leave_the_other_axis_free() {
    let source = "locus 0 beta 6\nrect outer(4, 4)\nrect left(1, 1)\nrect right(1, 1)\nrect top(1, 1)\nrect bottom(1, 1)\nrel inset_left(left, outer)\nrel inset_right(right, outer)\nrel inset_top(top, outer)\nrel inset_bottom(bottom, outer)\nassert inset_left(left, outer)\nassert inset_right(right, outer)\nassert inset_top(top, outer)\nassert inset_bottom(bottom, outer)\n";
    let scene = scene(source, EvaluationInputs::default());
    assert_eq!(scene.solve_summary().expect("inset solve").equations, 4);
    assert_scene(&scene);
}

#[test]
fn instance_and_group_bounds_include_measured_standalone_labels() {
    let source = "locus 0 beta 6\nfn item() {\nrect shape(4, 2)\nlabel caption(\"Wide caption\")\nrel center(caption, shape)\n}\nitem widget()\ngroup content(widget.shape, widget.caption)\nrect marker(1, 1)\nrel align_right(marker, widget)\nrel align_center_y(marker, content)\nassert align_right(marker, widget)\nassert align_right(marker, content)\nassert hdist(widget.shape.center, marker.center) == 2.5\n";
    let mut inputs = EvaluationInputs::default();
    inputs.measured.insert(
        "widget.caption".to_owned(),
        MeasuredText {
            width: 6.0,
            height: 0.4,
            role: MeasuredRole::Normal,
        },
    );
    assert_scene(&scene(source, inputs));
}

#[test]
fn bent_arrow_length_and_bounds_participate_in_equalities() {
    let source = "locus 0 beta 6\npoint start()\npoint finish()\narrow path(start, finish) { bend: 30deg }\nrect marker(1, 1)\nrel align_center_y(start, finish)\nrel path.length == style_metric(\"short_length\")\nrel center(marker, path)\nassert path.length == style_metric(\"short_length\")\nassert center(marker, path)\n";
    assert_scene(&scene(source, EvaluationInputs::default()));
}

#[test]
fn unknown_reports_distinguish_geometry_dimensions_and_labels() {
    let scene = scene(
        include_str!("../../../tests/fixtures/programs/measurement/automatic_label.locus"),
        measured_caption(),
    );
    let summary = scene.solve_summary().expect("classified unknowns");
    let classes = summary
        .dof
        .objects
        .iter()
        .map(|object| (object.id.as_str(), object.class))
        .collect::<Vec<_>>();
    assert_eq!(
        classes,
        [
            ("panel", UnknownClass::Geometric),
            ("panel", UnknownClass::AutomaticDimension),
            ("caption", UnknownClass::LabelPlacement)
        ]
    );
    assert!(
        summary
            .dof
            .objects
            .iter()
            .filter(|object| object.class != UnknownClass::Geometric)
            .all(|object| object.suggestion.is_empty())
    );
    assert!(summary.hint_displacements.is_empty());
}

#[test]
fn unreferenced_automatic_box_and_label_keep_positive_preset_defaults() {
    let scene = scene(
        "locus 0 beta 6\nrect panel()\nlabel caption(\"Measured caption\")\n",
        measured_caption(),
    );
    let summary = scene
        .solve_summary()
        .expect("unreferenced automatic unknowns");
    assert_eq!(summary.unknowns, 6);
    assert_eq!(summary.dof.global_remaining, 6);
    let resolved = scene.resolved().expect("default extents");
    let bounds = resolved
        .objects
        .iter()
        .find(|(id, _)| id.name == "panel")
        .expect("panel")
        .1
        .bbox()
        .expect("panel bounds");
    assert!((bounds.width() - scene.inputs.metrics.default_box_width).abs() < 1.0e-10);
    assert!((bounds.height() - scene.inputs.metrics.default_box_height).abs() < 1.0e-10);
    assert!(
        resolved
            .objects
            .values()
            .filter_map(locus_core::ResolvedObject::bbox)
            .all(|bounds| bounds.min.is_finite()
                && bounds.max.is_finite()
                && bounds.max.y > bounds.min.y)
    );
}

#[test]
fn label_declared_before_its_center_partner_solves_deterministically() {
    let source = "locus 0 beta 6\nlabel caption(\"Measured caption\")\nrect panel()\nrel panel.width == 4\nrel panel.height == 2\nrel center(caption, panel)\nassert center(caption, panel)\n";
    let mut scene = scene(source, measured_caption());
    assert_scene(&scene);
    let before = scene.evaluation.clone();
    scene
        .recompute()
        .expect("repeat forward-partner measured solve");
    assert_eq!(scene.evaluation, before);
}

#[test]
fn missing_and_invalid_text_measurements_are_source_errors() {
    let source = "locus 0 beta 6\nlabel caption(\"Caption\")\n";
    let parsed = parse(source);
    assert_eq!(
        Scene::new(parsed.file.clone())
            .expect_err("measurement required")
            .code,
        "E410"
    );
    for (width, height) in [
        (1.0, 0.0),
        (-1.0, 0.4),
        (f64::NAN, 0.4),
        (1.0, f64::INFINITY),
    ] {
        let mut inputs = measured_caption();
        inputs.measured.insert(
            "caption".to_owned(),
            MeasuredText {
                width,
                height,
                role: MeasuredRole::Normal,
            },
        );
        assert_eq!(
            Scene::with_inputs(parsed.file.clone(), inputs)
                .expect_err("invalid measurement")
                .code,
            "E412"
        );
    }
}

#[test]
fn invalid_preset_is_rejected_before_solving() {
    let parsed = parse("locus 0 beta 6\nrect panel()\n");
    let mut inputs = EvaluationInputs::default();
    inputs.metrics.short_length = 0.0;
    assert_eq!(
        Scene::with_inputs(parsed.file, inputs)
            .expect_err("invalid preset")
            .code,
        "E413"
    );
}

#[test]
fn contradictory_automatic_dimensions_fail_with_finite_deterministic_feedback() {
    let parsed =
        parse("locus 0 beta 6\nrect panel()\nrel panel.width == 1\nrel panel.width == 2\n");
    let first = Scene::new(parsed.file.clone()).expect_err("contradictory widths");
    let second = Scene::new(parsed.file).expect_err("repeat contradictory widths");
    assert_eq!(first.code, "E403");
    assert_eq!(first.message, second.message);
    assert!(!first.message.contains("NaN"));
    assert!(!first.message.contains("Infinity"));
}

#[test]
fn automatic_dimensions_seed_from_centered_measured_content() {
    let mut inputs = measured_caption();
    inputs.measured.insert(
        "caption".to_owned(),
        MeasuredText {
            width: 6.0,
            height: 0.4,
            role: MeasuredRole::Normal,
        },
    );
    let scene = scene(
        "locus 0 beta 6\nlabel caption(\"Wide caption\")\nrect panel()\nrel center(caption, panel)\nassert center(caption, panel)\n",
        inputs,
    );
    assert_scene(&scene);
    let bounds = scene
        .resolved()
        .expect("seeded automatic box")
        .objects
        .iter()
        .find(|(id, _)| id.name == "panel")
        .expect("panel")
        .1
        .bbox()
        .expect("panel bounds");
    assert!((bounds.width() - 6.5).abs() < 1.0e-10);
    assert!((bounds.height() - 1.0).abs() < 1.0e-10);
}

#[test]
fn failed_layout_assertions_report_booleans_and_metrics_without_symbol_errors() {
    let source = "locus 0 beta 6\nrect panel(4, 2)\ngroup container(panel)\nrect shifted(offset(panel.center, 1, 0), 1, 1)\nassert center(shifted, container)\nassert on(panel.center, edge(panel, N))\nassert style_metric(\"short_length\") == 999\n";
    let scene = scene(source, EvaluationInputs::default());
    let report = evaluate_assertions_with_inputs(
        &scene.source_file,
        scene.resolved().expect("layout assertion scene"),
        &scene.inputs,
    )
    .expect("failed assertion feedback");
    assert!(!report.passed);
    assert_eq!(report.assertions.len(), 3);
    assert!(
        report
            .assertions
            .iter()
            .all(|assertion| !assertion.passed && !assertion.actuals.is_empty())
    );
}
