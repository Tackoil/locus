//! Measured containment, collections, shape-aware avoidance, and feasible settling.
#![allow(
    clippy::expect_used,
    reason = "integration failures need fixture context"
)]
use locus_core::{
    EvaluationInputs, MeasuredRole, MeasuredText, Scene, evaluate_assertions_with_inputs,
};
use locus_syntax::parse;

fn inputs(width: f64) -> EvaluationInputs {
    let mut inputs = EvaluationInputs::default();
    inputs.measured.insert(
        "caption".to_owned(),
        MeasuredText {
            width,
            height: 0.5,
            role: MeasuredRole::Normal,
        },
    );
    inputs
}
fn scene(source: &str, inputs: EvaluationInputs) -> Result<Scene, locus_core::EvalError> {
    let parsed = parse(source);
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    Scene::with_inputs(parsed.file, inputs)
}
fn validated(scene: &Scene) {
    let assertions = evaluate_assertions_with_inputs(
        &scene.source_file,
        scene.resolved().expect("resolved scene"),
        &scene.inputs,
    )
    .expect("layout assertions");
    assert!(assertions.passed, "{assertions:?}");
    assert!(
        scene
            .solve_summary()
            .expect("solve summary")
            .max_normalized_residual
            < 1e-12
    );
}
fn bounds(scene: &Scene, name: &str) -> locus_core::BBox {
    scene
        .resolved()
        .expect("scene")
        .objects
        .iter()
        .find(|(id, _)| id.name == name)
        .expect("object")
        .1
        .bbox()
        .expect("bounds")
}

#[test]
fn long_caption_sizes_rect_diamond_and_ellipse_with_exact_padding() {
    for source in [
        include_str!("../../../fixtures/beta6/m2/contain_rect.locus"),
        include_str!("../../../fixtures/beta6/m2/contain_diamond.locus"),
        include_str!("../../../fixtures/beta6/m2/contain_ellipse.locus"),
    ] {
        let scene = scene(source, inputs(8.0)).expect("contained long caption");
        validated(&scene);
        assert!(bounds(&scene, "panel").width() >= 8.5 - 1e-9);
        assert!(scene.solve_summary().expect("summary").settling.is_some());
    }
}

#[test]
fn groups_expand_into_row_and_column_chains() {
    let scene = scene(
        include_str!("../../../fixtures/beta6/m2/group_chains.locus"),
        EvaluationInputs::default(),
    )
    .expect("group chains");
    validated(&scene);
    let first = bounds(&scene, "first");
    let second = bounds(&scene, "second");
    let third = bounds(&scene, "third");
    assert!(second.min.x - first.max.x >= 0.25 - 1e-9);
    assert!(third.min.x - second.max.x >= 0.25 - 1e-9);
    assert!((first.center().y - second.center().y).abs() < 1e-9);
}

#[test]
fn caption_widens_two_guide_row_and_keeps_feasible_settling() {
    let source = include_str!("../../../fixtures/beta6/m2/guide_caption.locus");
    let short = scene(source, inputs(3.0)).expect("short guide caption");
    validated(&short);
    let long = scene(source, inputs(9.0)).expect("long guide caption");
    validated(&long);
    assert!(bounds(&long, "panel").width() > bounds(&short, "panel").width() + 5.0);
    let repeated = scene(source, inputs(9.0)).expect("deterministic guide row");
    assert_eq!(long.solve_summary(), repeated.solve_summary());
    assert_eq!(long.resolved(), repeated.resolved());
}

#[test]
fn offset_circles_clear_actual_outlines_despite_overlapping_boxes() {
    let mut inputs = EvaluationInputs::default();
    inputs.metrics.clearance = 0.1;
    let scene = scene(
        include_str!("../../../fixtures/beta6/m2/circle_clearance.locus"),
        inputs,
    )
    .expect("radial separation");
    validated(&scene);
    assert!(bounds(&scene, "first").max.x > bounds(&scene, "second").min.x);
}

#[test]
fn stroke_crossing_box_is_a_layout_conflict() {
    let error = scene(
        include_str!("../../../fixtures/beta6/m2/crossing_stroke.locus"),
        EvaluationInputs::default(),
    )
    .expect_err("crossing interiors");
    assert_eq!(error.code, "E413");
    assert!(error.message.contains("layout conflict"));
}

#[test]
fn exact_inradius_keeps_rectangular_deepest_segment() {
    let source = "locus 0 beta 6\npoint origin()\npoint start = offset(origin,-4,0)\npoint end = offset(origin,4,0)\nrect panel(origin,10,2)\nsegment line(start,end)\nrel contain(panel,line)\nassert contain(panel,line)\n";
    let mut inputs = EvaluationInputs::default();
    inputs.metrics.padding = 1.0;
    let scene = scene(source, inputs).expect("nonempty deepest-point segment");
    validated(&scene);
}

#[test]
fn excess_padding_on_fixed_shape_is_infeasible() {
    let source =
        "locus 0 beta 6\npoint origin()\nrect panel(origin,10,2)\nrel contain(panel,origin)\n";
    let mut inputs = EvaluationInputs::default();
    inputs.metrics.padding = 1.1;
    let error = scene(source, inputs).expect_err("empty inset");
    assert_eq!(error.code, "E413");
}

#[test]
fn avoidance_expands_label_and_obstacle_collections_memberwise() {
    let source = "locus 0 beta 6\nlabel caption(\"Measured caption\")\nrect obstacle(2,2)\ncircle disk(1)\ngroup obstacles(obstacle,disk)\nrel avoid(caption,obstacles)\nassert avoid(caption,obstacles)\n";
    let scene = scene(source, inputs(3.0)).expect("label avoidance");
    validated(&scene);
}

#[test]
fn nonlinear_preference_settling_backtracks_and_retains_feasibility() {
    let scene = scene(
        include_str!("../../../fixtures/beta6/m2/settling_cycle.locus"),
        EvaluationInputs::default(),
    )
    .expect("nonlinear preference settling");
    validated(&scene);
    let settling = scene
        .solve_summary()
        .expect("summary")
        .settling
        .as_ref()
        .expect("settling report");
    assert!(settling.line_search_evaluations > settling.steps.saturating_sub(1));
    assert!(settling.projected_gradient.is_finite());
}

#[test]
fn cover_relations_use_collection_extrema_and_compact_automatic_dimensions() {
    let source = "locus 0 beta 6\npoint origin()\npoint other = offset(origin,5,2)\nrect panel()\ngroup content(origin,other)\nrel cover_x(panel,content)\nrel cover_y(panel,content)\nassert cover_x(panel,content)\nassert cover_y(panel,content)\n";
    let scene = scene(source, EvaluationInputs::default()).expect("cover collections");
    validated(&scene);
    let panel = bounds(&scene, "panel");
    assert!((panel.width() - 5.5).abs() < 1e-8);
    assert!((panel.height() - 2.5).abs() < 1e-8);
}

#[test]
fn row_groups_deduplicate_aliases_and_keep_instances_as_whole_items() {
    let source = "locus 0 beta 6\nfn card() {\n rect lower(2,1)\n rect upper(offset(lower.center,0,2),2,1)\n}\ncard first()\ncard second()\ngroup cards(first,second)\nrel column(cards)\nassert column(cards)\n";
    let scene = scene(source, EvaluationInputs::default()).expect("whole composed instances");
    validated(&scene);
    let first = bounds(&scene, "first.lower");
    let second = bounds(&scene, "second.upper");
    assert!(first.min.y - second.max.y >= 0.25 - 1e-9);
    let alias = "locus 0 beta 6\nrect a(1,1)\nrect alias = a\nrect b(1,1)\ngroup items(alias,b)\nrel row(a,items)\nassert row(a,items)\n";
    validated(
        &self::scene(alias, EvaluationInputs::default()).expect("alias identity deduplication"),
    );
}

#[test]
fn gaps_separate_collection_union_without_chaining_its_members() {
    let source = "locus 0 beta 6\nrect a(2,1)\nrect b(2,1)\nrect c(2,1)\nrel row(a,b)\ngroup headers(a,b)\nrel gap_y(headers,c)\nassert row(a,b)\nassert gap_y(headers,c)\n";
    let scene = scene(source, EvaluationInputs::default()).expect("combined collection gap");
    validated(&scene);
    assert!((bounds(&scene, "a").center().y - bounds(&scene, "b").center().y).abs() < 1e-9);
}

#[test]
fn containment_uses_all_corners_of_collection_combined_bounds() {
    let source = "locus 0 beta 6\npoint origin()\npoint a = offset(origin,-1.5,0)\npoint b = offset(origin,0,1.5)\ndiamond panel(origin,4,4)\ngroup content(a,b)\nrel contain(panel,content)\n";
    let mut inputs = EvaluationInputs::default();
    inputs.metrics.padding = 0.0;
    let error = scene(source, inputs).expect_err("combined corner lies outside diamond");
    assert_eq!(error.code, "E413");
}

#[test]
fn overflowing_layout_normalization_is_an_error_instead_of_a_panic_or_feasible_hinge() {
    let source = "locus 0 beta 6\npoint origin()\npoint nearby = offset(origin,0.01,0)\nrel gap_x(origin,nearby)\n";
    let mut inputs = EvaluationInputs::default();
    inputs.metrics.gap = 1e308;
    let error = scene(source, inputs).expect_err("overflowing normalized gap");
    assert!(error.message.contains("non-finite"), "{error:?}");
}

#[test]
fn avoidance_recursively_expands_instances_and_deduplicates_aliases() {
    let declarations = "locus 0 beta 6\nfn pair(anchor) {\n rect one(anchor,1,1)\n rect two(offset(anchor,3,0),1,1)\n rect alias = one\n group nested(one,two,alias)\n}\nfn assembly(anchor) {\n pair child(anchor)\n group parts(child.nested,child)\n}\npoint origin()\nassembly subject(origin)\nassembly obstacle(offset(origin,0,5))\ngroup subjects(subject.parts,subject.child.alias)\ngroup obstacles(obstacle.parts,obstacle.child.alias)\n";
    let nested = scene(
        &format!("{declarations}rel avoid(subjects,obstacles)\nassert avoid(subjects,obstacles)\n"),
        EvaluationInputs::default(),
    )
    .expect("recursive nested instance avoidance");
    validated(&nested);
    let direct = scene(&format!("{declarations}rel avoid(subject.child.one,obstacle.child.one,obstacle.child.two)\nrel avoid(subject.child.two,obstacle.child.one,obstacle.child.two)\nassert avoid(subjects,obstacles)\n"), EvaluationInputs::default())
        .expect("equivalent individual visible members");
    validated(&direct);
    assert_eq!(
        nested.solve_summary().expect("summary").equations,
        direct.solve_summary().expect("summary").equations,
        "alias identities must not duplicate member-pair rows"
    );
}

#[test]
fn nested_collection_rows_keep_instances_whole_and_deduplicate_references() {
    let source = "locus 0 beta 6\nfn card() {\n rect lower(2,1)\n rect upper(offset(lower.center,0,2),2,1)\n}\ncard first()\ncard second()\ngroup cards(first,second)\ngroup nested(cards,first)\nrel column(nested)\nassert column(nested)\n";
    let scene = scene(source, EvaluationInputs::default()).expect("nested whole-instance column");
    validated(&scene);
    assert!(
        bounds(&scene, "first.lower").min.y - bounds(&scene, "second.upper").max.y >= 0.25 - 1e-9
    );
}

#[test]
fn containment_keeps_nested_instance_aggregate_corner_semantics() {
    let source = "locus 0 beta 6\nfn pair(anchor) {\n point a = offset(anchor,-1.5,0)\n point b = offset(anchor,0,1.5)\n}\npoint origin()\npair items(origin)\ndiamond panel(origin,4,4)\ngroup content(items)\nrel contain(panel,content)\n";
    let mut inputs = EvaluationInputs::default();
    inputs.metrics.padding = 0.0;
    let error =
        scene(source, inputs).expect_err("aggregate nested-instance corner outside diamond");
    assert_eq!(error.code, "E413");
}
