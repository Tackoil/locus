//! Continuous measured placement in path regions and on retained routes.
#![allow(clippy::expect_used, reason = "fixture failures need their context")]
use locus_core::geom::route::RouteSide;
use locus_core::scene::PathLabel;
use locus_core::{
    AnnotationSpec, EvaluationInputs, LabelChoice, MeasuredRole, MeasuredText, Scene,
    evaluate_assertions_with_inputs,
};
use locus_syntax::parse;

fn inputs() -> EvaluationInputs {
    let mut inputs = EvaluationInputs::default();
    inputs.measured.insert(
        "caption".to_owned(),
        MeasuredText {
            width: 1.0,
            height: 0.5,
            role: MeasuredRole::Normal,
        },
    );
    inputs
}

fn solve(source: &str, inputs: EvaluationInputs) -> Result<Scene, locus_core::EvalError> {
    let parsed = parse(source);
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    Scene::with_inputs(parsed.file, inputs)
}

fn label(scene: &Scene) -> (&PathLabel, locus_core::BBox) {
    let object = scene
        .resolved()
        .expect("resolved")
        .objects
        .iter()
        .find(|(id, _)| id.name == "caption")
        .map(|(_, object)| object)
        .expect("caption");
    let locus_core::ResolvedObject::Annotation(AnnotationSpec::Label {
        path: Some(path),
        bounds: Some(bounds),
        ..
    }) = object
    else {
        unreachable!("expected accepted path label")
    };
    (path, *bounds)
}

const DIRECT: &str = "locus 0 beta 6\npoint start()\npoint end = offset(start, 10, 0)\narrow path(start, end)\nlabel caption(path, \"caption\") { near: start, side: above }\n";

#[test]
fn label_parameters_solve_to_authored_center_and_keep_measured_extents() {
    let source = format!(
        "{DIRECT}point target = offset(start, 2.5, 2)\nrel center(caption, target)\nassert center(caption, target)\nassert caption.width == 1\nassert caption.height == 0.5\n"
    );
    let scene = solve(&source, inputs()).expect("continuous t/o");
    let (path, bounds) = label(&scene);
    assert!((path.parameter - 0.25).abs() < 1e-10);
    assert!((path.offset - 1.75).abs() < 1e-10);
    assert!((bounds.center().y - 2.0).abs() < 1e-10);
    assert!(
        evaluate_assertions_with_inputs(
            &scene.source_file,
            scene.resolved().expect("resolved"),
            &scene.inputs
        )
        .expect("assertions")
        .passed
    );
    let summary = scene.solve_summary().expect("summary");
    assert_eq!(summary.unknowns, 4);
    assert!(summary.hint_displacements.is_empty());
}

#[test]
fn near_region_and_clearance_are_hard_domains() {
    let source =
        format!("{DIRECT}point target = offset(start, 8, 2)\nrel center(caption, target)\n");
    assert!(solve(&source, inputs()).is_err());
    let source =
        format!("{DIRECT}point target = offset(start, 2, 0)\nrel center(caption, target)\n");
    assert!(solve(&source, inputs()).is_err());
}

#[test]
fn frozen_candidates_and_retained_intervals_change_the_same_accepted_bounds() {
    let mut inputs = inputs();
    inputs
        .choices
        .retained_routes
        .insert("path".to_owned(), (0.2, 0.8));
    inputs.choices.labels.insert(
        "caption".to_owned(),
        LabelChoice {
            side: RouteSide::Below,
            candidate: 2,
        },
    );
    let scene = solve(&DIRECT.replace("side: above", "side: below"), inputs)
        .expect("retained-route placement");
    let (path, bounds) = label(&scene);
    assert!((path.parameter - 0.3).abs() < 1e-10);
    assert!((bounds.center().x - 3.8).abs() < 1e-10);
    assert!(bounds.max.y < 0.0);
    assert_eq!(path.candidate, 2);
}

#[test]
fn plain_path_attachment_owns_two_internal_parameters() {
    let source = "locus 0 beta 6\npoint start()\npoint end = offset(start, 10, 0)\narrow path(start, end)\nlabel caption(path, \"caption\")\n";
    let scene = solve(source, inputs()).expect("default attachment");
    let (path, bounds) = label(&scene);
    assert!((path.parameter - 0.5).abs() < 1e-12);
    assert!((bounds.center().x - 5.0).abs() < 1e-12);
    assert_eq!(scene.solve_summary().expect("summary").unknowns, 4);
}

#[test]
fn invalid_measurements_and_placement_tokens_are_rejected() {
    let mut bad = inputs();
    bad.measured.get_mut("caption").expect("measurement").height = -0.5;
    assert_eq!(
        solve(DIRECT, bad).expect_err("bad measurement").code,
        "E412"
    );
    assert_eq!(
        solve(&DIRECT.replace("side: above", "side: sideways"), inputs())
            .expect_err("bad side")
            .code,
        "E209"
    );
    let mut bad = inputs();
    bad.choices
        .retained_distances
        .insert("path".to_owned(), (-1.0, 0.0));
    assert_eq!(
        solve(DIRECT, bad).expect_err("negative physical trim").code,
        "E413"
    );
}

#[test]
fn attached_label_bounds_drive_logical_path_height_without_changing_length() {
    let source = format!(
        "{DIRECT}rel path.height == 2\nassert path.height == 2\nassert path.length == 10\n"
    );
    let scene = solve(&source, inputs()).expect("path bounds coupling");
    let (path, bounds) = label(&scene);
    assert!((path.offset - 1.5).abs() < 1e-10);
    assert!((bounds.max.y - 2.0).abs() < 1e-10);
    assert!(
        evaluate_assertions_with_inputs(
            &scene.source_file,
            scene.resolved().expect("resolved"),
            &scene.inputs
        )
        .expect("assertions")
        .passed
    );
}

#[test]
fn frozen_label_choices_preserve_an_authored_side() {
    let mut inputs = inputs();
    inputs.choices.labels.insert(
        "caption".to_owned(),
        LabelChoice {
            side: RouteSide::Below,
            candidate: 0,
        },
    );
    let scene = solve(DIRECT, inputs).expect("authored side");
    assert_eq!(label(&scene).0.side, RouteSide::Above);
}

#[test]
fn continuous_failure_retries_automatic_label_sides() {
    let source = format!(
        "{}point target = offset(start, 5, -2)\nrel center(caption, target)\nassert center(caption, target)\n",
        DIRECT.replace("{ near: start, side: above }", "{ near: middle }")
    );
    let scene = solve(&source, inputs()).expect("below-side retry");
    assert_eq!(label(&scene).0.side, RouteSide::Below);
    let summary = scene.solve_summary().expect("summary");
    assert!(summary.outer_search_attempts > 1);
    assert!(
        summary
            .branch_resolutions
            .iter()
            .any(|choice| choice.choice.contains("label caption: Below"))
    );
    assert!(
        evaluate_assertions_with_inputs(
            &scene.source_file,
            scene.resolved().expect("resolved"),
            &scene.inputs
        )
        .expect("assertions")
        .passed
    );
}

#[test]
fn joint_label_search_retains_earlier_accepted_side_choices() {
    let source = "locus 0 beta 6\npoint start()\npoint end = offset(start, 10, 0)\narrow path(start,end)\nlabel caption(path, \"first\")\nlabel second(path, \"second\")\npoint below = offset(start,5,-2)\npoint right = offset(start,8,0)\nrel center(caption,below)\nrel center(second,right)\nassert center(caption,below)\nassert center(second,right)\n";
    let mut inputs = inputs();
    inputs.measured.insert(
        "second".to_owned(),
        MeasuredText {
            width: 1.0,
            height: 0.5,
            role: MeasuredRole::Normal,
        },
    );
    let scene = solve(source, inputs).expect("combined side search");
    assert_eq!(label(&scene).0.side, RouteSide::Below);
    assert!(
        scene
            .solve_summary()
            .expect("summary")
            .branch_resolutions
            .iter()
            .any(|choice| choice.choice.contains("label second: Right"))
    );
    assert!(
        evaluate_assertions_with_inputs(
            &scene.source_file,
            scene.resolved().expect("resolved"),
            &scene.inputs
        )
        .expect("assertions")
        .passed
    );
}

#[test]
fn ordered_finite_guide_incidence_solves_with_existing_geometric_pieces() {
    let source = "locus 0 beta 6\nrect source(2,1)\nrect target(2,1)\nrel row(source,target)\npoint source_end()\npoint target_end()\nrel align_center_x(source.s,source_end)\nrel align_center_x(target.s,target_end)\nrel gap_y(source.s,source_end)\nrel gap_y(target.s,target_end)\nsegment source_guide(source.s,source_end)\nsegment target_guide(target.s,target_end)\npoint first_tail()\npoint first_tip()\npoint second_tail()\npoint second_tip()\nrel on(first_tail,source_guide)\nrel on(first_tip,target_guide)\nrel on(second_tail,source_guide)\nrel on(second_tip,target_guide)\nrel align_center_y(first_tail,first_tip)\nrel align_center_y(second_tail,second_tip)\nrel gap_y(source,first_tail,second_tail,source_end)\nrel align_center_y(source_end,target_end)\nassert on(first_tail,source_guide) and on(first_tip,target_guide)\nassert on(second_tail,source_guide) and on(second_tip,target_guide)\nassert gap_y(first_tail,second_tail)\n";
    let scene = solve(source, EvaluationInputs::default()).expect("ordered finite incidence");
    assert!(
        evaluate_assertions_with_inputs(
            &scene.source_file,
            scene.resolved().expect("resolved"),
            &scene.inputs
        )
        .expect("assertions")
        .passed
    );
}

#[test]
fn arrow_aliases_share_attached_bounds_and_retained_label_placement() {
    let source = "locus 0 beta 6\npoint start()\npoint end = offset(start,10,0)\narrow path(start,end)\narrow alias = path\nlabel caption(alias,\"near target\") {near:end,side:above}\nassert path.width == alias.width\nassert path.height == alias.height\nassert path.height > 0\nassert path.length == 10\nassert alias.length == 10\n";
    let mut measured = inputs();
    measured
        .choices
        .retained_routes
        .insert("alias".to_owned(), (0.2, 0.6));
    let scene = solve(source, measured).expect("aliased retained label");
    let (path, bounds) = label(&scene);
    assert_eq!(path.path, "path");
    assert!((bounds.center().x - 5.2).abs() < 1e-10);
    assert!(
        evaluate_assertions_with_inputs(
            &scene.source_file,
            scene.resolved().expect("resolved"),
            &scene.inputs
        )
        .expect("assertions")
        .passed
    );
}

#[test]
fn physical_trims_follow_the_solved_route_length_in_label_bounds_and_assertions() {
    let source = "locus 0 beta 6\npoint start()\nnum extent = free 10\npoint end = offset(start,extent,0)\narrow path(start,end)\nlabel caption(path,\"physical retained\") {near:end,side:below}\npoint target = offset(start,14.6,-2)\nrel path.length == 20\nrel center(caption,target)\nassert path.length == 20\nassert center(caption,target)\n";
    let mut measured = inputs();
    measured
        .choices
        .retained_routes
        .insert("path".to_owned(), (0.1, 0.8));
    measured
        .choices
        .retained_distances
        .insert("path".to_owned(), (1.0, 2.0));
    let scene = solve(source, measured).expect("physical retained placement");
    let (path, bounds) = label(&scene);
    assert!((path.parameter - 0.8).abs() < 1e-10);
    assert!((bounds.center().x - 14.6).abs() < 1e-10);
    assert!(
        evaluate_assertions_with_inputs(
            &scene.source_file,
            scene.resolved().expect("resolved"),
            &scene.inputs
        )
        .expect("assertions")
        .passed
    );
}

#[test]
fn aliased_physical_and_scalar_retained_avoidance_share_assertion_semantics() {
    let source = "locus 0 beta 6\npoint start()\npoint end = offset(start,10,0)\narrow path(start,end)\narrow alias = path\npoint obstacle = offset(start,9,0)\nrel avoid(obstacle,path)\nassert avoid(obstacle,path)\nassert avoid(obstacle,alias)\n";
    for physical in [false, true] {
        let mut inputs = EvaluationInputs::default();
        if physical {
            inputs
                .choices
                .retained_distances
                .insert("alias".to_owned(), (2.0, 4.0));
        } else {
            inputs
                .choices
                .retained_routes
                .insert("alias".to_owned(), (0.2, 0.6));
        }
        let scene = solve(source, inputs).expect("canonical visible avoidance");
        assert!(
            evaluate_assertions_with_inputs(
                &scene.source_file,
                scene.resolved().expect("resolved"),
                &scene.inputs
            )
            .expect("assertions")
            .passed
        );
    }
}

#[test]
fn explicit_auto_near_and_side_match_default_label_search() {
    let source = format!(
        "{}point target = offset(start,5,-2)\nrel center(caption,target)\nassert center(caption,target)\n",
        DIRECT.replace("{ near: start, side: above }", "{ near: auto, side: auto }")
    );
    let scene = solve(&source, inputs()).expect("explicit automatic placement");
    assert_eq!(label(&scene).0.side, RouteSide::Below);
    assert!(
        evaluate_assertions_with_inputs(
            &scene.source_file,
            scene.resolved().expect("resolved"),
            &scene.inputs
        )
        .expect("assertions")
        .passed
    );
}

#[test]
fn label_overload_properties_are_source_anchored_errors() {
    let source = DIRECT.replace("near: start, side: above", "dir: center, gap: 1");
    let error = solve(&source, inputs()).expect_err("point props on path");
    assert_eq!(error.code, "E131");
    assert_eq!(error.span.line, 5);
    let source = DIRECT.replace("label caption(path,", "label caption(start,");
    let error = solve(&source, inputs()).expect_err("path props on point");
    assert_eq!(error.code, "E131");
    assert_eq!(error.span.line, 5);
}
