//! Compact preset spacing must not compete with internal placement seeds.
#![allow(
    clippy::expect_used,
    reason = "regressions need explicit failure context"
)]

use locus_core::{BBox, EvaluationInputs, MeasuredRole, MeasuredText, Scene};
use locus_syntax::parse;

fn scene(source: &str, inputs: EvaluationInputs) -> Scene {
    let parsed = parse(source);
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    Scene::with_inputs(parsed.file, inputs).expect("compact spacing scene")
}

fn bounds(scene: &Scene, name: &str) -> BBox {
    scene
        .resolved()
        .expect("resolved spacing scene")
        .objects
        .iter()
        .find(|(id, _)| id.name == name)
        .and_then(|(_, object)| object.bbox())
        .expect("spacing object bounds")
}

#[test]
fn gaps_compact_after_a_hard_anchor_moves_their_initial_placement() {
    for (relation, edge) in [("gap_y", "n"), ("gap_x", "w")] {
        let source = format!(
            "locus 0 beta 6\npoint origin()\nrect anchor(origin, 20, 20)\nrect first(2, 1)\nrect second(2, 1)\nrel center(first, anchor.{edge})\nrel {relation}(first, second)\n"
        );
        let mut inputs = EvaluationInputs::default();
        inputs.metrics.gap = 0.375;
        let expected = inputs.metrics.gap;
        let scene = scene(&source, inputs);
        let first = bounds(&scene, "first");
        let second = bounds(&scene, "second");
        let actual = if relation == "gap_y" {
            first.min.y - second.max.y
        } else {
            second.min.x - first.max.x
        };
        assert!(
            (actual - expected).abs() < 1.0e-8,
            "{relation} retained excess spacing: {actual}; expected {expected}"
        );
        assert!(
            scene
                .solve_summary()
                .expect("spacing summary")
                .settling
                .as_ref()
                .expect("settling report")
                .settling_complete
        );
    }
}

#[test]
fn gaps_allow_extra_space_required_by_hard_geometry() {
    let source = "locus 0 beta 6\npoint origin()\nrect first(origin, 2, 1)\nrect second(2, 1)\nrel align_center_x(first,second)\nrel vdist(second.center,first.center) == 4\nrel gap_y(first, second)\n";
    let scene = scene(source, EvaluationInputs::default());
    let actual = bounds(&scene, "first").min.y - bounds(&scene, "second").max.y;
    assert!((actual - 3.0).abs() < 1.0e-10);
    assert!(
        scene
            .solve_summary()
            .expect("fixed spacing summary")
            .settling
            .as_ref()
            .expect("settling report")
            .settling_complete
    );
}

#[test]
fn rows_adapt_to_changed_dimensions_without_manual_positions() {
    for width in [2, 8] {
        let source = format!(
            "locus 0 beta 6\nrect first({width}, 1)\nrect second(2, 1)\nrel row(first, second)\n"
        );
        let inputs = EvaluationInputs::default();
        let expected_gap = inputs.metrics.gap;
        let scene = scene(&source, inputs);
        let first = bounds(&scene, "first");
        let second = bounds(&scene, "second");
        assert!((second.min.x - first.max.x - expected_gap).abs() < 1e-9);
        assert!((first.center().y - second.center().y).abs() < 1e-9);
    }
}

#[test]
fn checkout_legend_and_headers_compact_without_inset_workarounds() {
    let source = include_str!("../../../examples/composition/checkout_sequence.locus");
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/text_measurements.json"
    ))
    .expect("bundled measurements");
    let measurements = fixture["measurements"]["examples/composition/checkout_sequence.locus"]
        .as_object()
        .expect("checkout measurements");
    let mut inputs = EvaluationInputs::default();
    for (name, value) in measurements {
        inputs.measured.insert(
            name.clone(),
            MeasuredText {
                width: f64::from_bits(value["width_bits"].as_u64().expect("measured width")),
                height: f64::from_bits(value["height_bits"].as_u64().expect("measured height")),
                role: if value["role"] == "title" {
                    MeasuredRole::Title
                } else {
                    MeasuredRole::Normal
                },
            },
        );
    }
    let expected = inputs.metrics.gap;
    let scene = scene(source, inputs);
    let actual = bounds(&scene, "key.frame").min.y - bounds(&scene, "customer.shape").max.y;
    assert!(
        (actual - expected).abs() < 1e-8,
        "checkout gap {actual}; expected {expected}; {:?}",
        scene.solve_summary().expect("checkout summary").settling
    );
    assert!(
        scene
            .solve_summary()
            .expect("checkout summary")
            .max_normalized_residual
            < 1e-12,
        "compaction must preserve every hard relation",
    );
}
