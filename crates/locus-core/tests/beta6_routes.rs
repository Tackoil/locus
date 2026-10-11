//! Shared route measurements and finite rectangular boundary expression checks.
#![allow(clippy::expect_used, reason = "fixture diagnostics need test context")]

use locus_core::{ResolvedObject, Scene, evaluate_assertions};
use locus_syntax::parse;

fn scene(source: &str) -> Result<Scene, locus_core::EvalError> {
    let parsed = parse(source);
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    Scene::new(parsed.file)
}

#[test]
fn frozen_route_context_cannot_override_an_authored_side() {
    use locus_core::geom::route::{RouteChoice, RouteSide};
    let parsed = parse(
        "locus 0 beta 6\npoint start()\npoint end = offset(start, 0, 3)\narrow path(start, end) { route: orthogonal, side: right }\n",
    );
    let mut inputs = locus_core::EvaluationInputs::default();
    inputs.choices.routes.insert(
        "path".to_owned(),
        RouteChoice {
            side: RouteSide::Left,
            five_span: false,
        },
    );
    let scene =
        Scene::with_inputs(parsed.file, inputs).expect("authored side survives stale choices");
    let route = scene
        .resolved()
        .expect("resolved route")
        .objects
        .values()
        .find_map(|object| match object {
            ResolvedObject::Annotation(locus_core::AnnotationSpec::Arrow {
                route: Some(route),
                ..
            }) => Some(route),
            _ => None,
        })
        .expect("automatic route");
    assert!(route.bbox().max.x > route.from.x);
    assert!((route.bbox().min.x - route.from.x).abs() < 1e-12);
    assert!(
        scene
            .solve_summary()
            .expect("route summary")
            .branch_resolutions
            .iter()
            .any(|branch| branch.choice.contains("Right"))
    );
}

#[test]
fn all_rectangular_edges_have_directed_closed_endpoints() {
    let scene = scene("locus 0 beta 6\npoint origin()\nrect panel(origin, 4, 2)\nsegment north = edge(panel, N)\nsegment east = edge(panel, E)\nsegment south = edge(panel, S)\nsegment west = edge(panel, W)\n").expect("edge values");
    let resolved = scene.resolved().expect("resolved edges");
    let center = resolved
        .objects
        .values()
        .find_map(|object| {
            if let ResolvedObject::Point(point) = object {
                Some(point.geometry)
            } else {
                None
            }
        })
        .expect("panel center");
    let segments: Vec<_> = resolved
        .objects
        .values()
        .filter_map(|object| {
            if let ResolvedObject::Segment(segment) = object {
                Some(*segment)
            } else {
                None
            }
        })
        .collect();
    let expected = [
        ((-2.0, 1.0), (2.0, 1.0)),
        ((2.0, 1.0), (2.0, -1.0)),
        ((2.0, -1.0), (-2.0, -1.0)),
        ((-2.0, -1.0), (-2.0, 1.0)),
    ];
    for (segment, (start, end)) in segments.iter().zip(expected) {
        assert!((segment.start.x - center.x - start.0).abs() < 1e-12);
        assert!((segment.start.y - center.y - start.1).abs() < 1e-12);
        assert!((segment.end.x - center.x - end.0).abs() < 1e-12);
        assert!((segment.end.y - center.y - end.1).abs() < 1e-12);
    }
    assert_eq!(segments.len(), 4);
}

#[test]
fn edge_rejects_other_shapes_even_when_used_only_by_a_relation() {
    for kind in ["diamond", "rounded_rect"] {
        let dimensions = if kind == "rounded_rect" {
            "4, 2, 0.2"
        } else {
            "4, 2"
        };
        let source = format!(
            "locus 0 beta 6\n{kind} panel({dimensions})\npoint marker()\nrel on(marker, edge(panel, W))\n"
        );
        let error = scene(&source).expect_err("rect-only edge");
        assert!(
            error.message.contains("edge requires an axis-aligned rect"),
            "{error:?}"
        );
    }
}

#[test]
fn style_metric_rejects_unknown_names_and_non_plain_strings() {
    for metric in ["\"unknown\"", "1", "$short_length$", "\"$short_length$\""] {
        let source = format!(
            "locus 0 beta 6\npoint start()\npoint finish()\narrow path(start, finish)\nrel path.length == style_metric({metric})\n"
        );
        let error = scene(&source).expect_err("invalid style metric");
        assert!(
            error.message.contains("style metric") || error.message.contains("style_metric"),
            "{error:?}"
        );
    }
}

#[test]
fn deferred_bent_arrow_assertions_share_exact_bounds_and_length() {
    let scene = scene("locus 0 beta 6\npoint start()\npoint finish = offset(start, 4, 0)\narrow path(start, finish) { bend: 45deg }\nassert path.height == 1\nassert path.width == 4\nassert path.length > 4.5\nassert path.length < 4.6\n").expect("bent route");
    let report = evaluate_assertions(
        &scene.source_file,
        scene.resolved().expect("resolved route"),
    )
    .expect("route assertions");
    assert!(report.passed, "{report:?}");
}

#[test]
fn temporary_direct_arrows_share_the_solver_route_measurement() {
    let scene = scene("locus 0 beta 6\npoint start()\npoint finish()\nrel arrow(start, finish).length == style_metric(\"short_length\")\nassert arrow(start, finish).length == style_metric(\"short_length\")\n").expect("temporary route solve");
    assert!(
        evaluate_assertions(
            &scene.source_file,
            scene.resolved().expect("resolved endpoints")
        )
        .expect("temporary route assertions")
        .passed
    );
}

#[test]
fn orthogonal_authored_length_solves_internal_detour_without_avoidance() {
    let scene = scene(include_str!(
        "../../../fixtures/beta6/m2/orthogonal_length.locus"
    ))
    .expect("internal route parameter solve");
    assert!(
        evaluate_assertions(&scene.source_file, scene.resolved().expect("solved route"))
            .expect("route assertions")
            .passed
    );
    let route = scene
        .resolved()
        .expect("solved route")
        .objects
        .values()
        .find_map(|object| match object {
            ResolvedObject::Annotation(locus_core::AnnotationSpec::Arrow {
                route: Some(route),
                ..
            }) => Some(route),
            _ => None,
        })
        .expect("persisted route");
    assert!((route.parameter.expect("internal offset") - 2.0).abs() < 1e-10);
    assert_eq!(route.via.len(), 2);
    let summary = scene.solve_summary().expect("route solve summary");
    let parameter = summary
        .dof
        .objects
        .iter()
        .find(|object| object.id == "path")
        .expect("internal parameter ownership");
    assert_eq!(
        parameter.class,
        locus_core::solve::UnknownClass::RouteLabelParameter
    );
}

#[test]
fn unused_route_parameter_keeps_preset_default_and_is_registered() {
    let scene = scene("locus 0 beta 6\npoint start()\npoint finish = offset(start, 0, 3)\narrow path(start, finish) { route: orthogonal }\nassert path.length == 4\n").expect("unreferenced internal route parameter");
    assert_eq!(scene.solve_summary().expect("solve summary").unknowns, 3);
    assert!(
        evaluate_assertions(&scene.source_file, scene.resolved().expect("solved route"))
            .expect("route assertions")
            .passed
    );
}

#[test]
fn curved_authored_length_solves_control_offset() {
    let scene = scene("locus 0 beta 6\npoint start()\npoint finish = offset(start, 0, 4)\narrow path(start, finish) { route: curved, side: right }\nrel path.length == 5\nassert path.length == 5\nassert path.width > 1\n").expect("curved internal parameter");
    assert!(
        evaluate_assertions(&scene.source_file, scene.resolved().expect("solved curve"))
            .expect("curve assertions")
            .passed
    );
}

#[test]
fn frozen_route_choice_follows_endpoints_moving_during_solving() {
    let scene = scene("locus 0 beta 6\npoint start()\npoint finish()\narrow path(start, finish) { route: orthogonal, side: right }\nrel hdist(start, finish) == 2\nrel vdist(start, finish) == 3\nrel path.length == 9\nassert path.length == 9\nassert path.width == 4\nassert path.height == 3\n").expect("joint endpoints and detour solve");
    assert!(
        evaluate_assertions(
            &scene.source_file,
            scene.resolved().expect("solved moving endpoints")
        )
        .expect("moving route assertions")
        .passed
    );
}

#[test]
fn outer_search_runs_when_default_route_side_cannot_meet_measurement_without_avoid() {
    let scene = scene("locus 0 beta 6\npoint start()\npoint finish = offset(start, 3, 2)\narrow path(start, finish) { route: orthogonal }\nrel path.height == 4\nrel path.width == 3\nrel path.length == 9\nassert path.height == 4\nassert path.length == 9\n").expect("outer route-side search");
    assert!(
        evaluate_assertions(
            &scene.source_file,
            scene.resolved().expect("accepted searched route")
        )
        .expect("searched route assertions")
        .passed
    );
}

#[test]
fn fixed_route_side_preserves_authored_restriction_and_domain_conflicts() {
    for constraints in [
        "rel path.height == 4\nrel path.width == 3",
        "rel path.length == 4",
    ] {
        let source = format!(
            "locus 0 beta 6\npoint start()\npoint finish = offset(start, 3, 2)\narrow path(start, finish) {{ route: orthogonal, side: right }}\n{constraints}\n"
        );
        assert!(scene(&source).is_err(), "{source}");
    }
}

#[test]
fn outer_search_combines_choices_for_multiple_routes() {
    let scene = scene("locus 0 beta 6\npoint first()\npoint first_tip = offset(first, 3, 2)\npoint second()\npoint second_tip = offset(second, 3, 2)\narrow first_path(first, first_tip) { route: orthogonal }\narrow second_path(second, second_tip) { route: orthogonal }\nrel first_path.length == 9\nrel hdist(first, first_path.w) == -2\nrel second_path.length == 9\nrel hdist(second, second_path.w) == -2\nassert first_path.length == 9\nassert second_path.length == 9\nassert hdist(first, first_path.w) == -2\nassert hdist(second, second_path.w) == -2\n").expect("bounded combined route search");
    assert!(
        evaluate_assertions(
            &scene.source_file,
            scene.resolved().expect("accepted combined routes")
        )
        .expect("combined route assertions")
        .passed
    );
}

#[test]
fn failing_row_search_holds_successful_choices_and_skips_unrelated_routes() {
    let scene = scene("locus 0 beta 6\npoint unused()\npoint unused_tip = offset(unused, 0, 3)\narrow unused_path(unused, unused_tip) { route: orthogonal }\npoint first()\npoint first_tip = offset(first, 3, 2)\npoint second()\npoint second_tip = offset(second, 3, 2)\narrow first_path(first, first_tip) { route: orthogonal }\narrow second_path(second, second_tip) { route: orthogonal }\nrel first_path.height == 4\nrel first_path.width == 3\nrel first_path.length == 9\nrel second_path.height == 4\nrel second_path.width == 3\nrel second_path.length == 9\nassert unused_path.length == 4\nassert first_path.height == 4\nassert second_path.height == 4\n").expect("failed-row guided combination within search cap");
    assert!(
        evaluate_assertions(
            &scene.source_file,
            scene.resolved().expect("accepted failed-row routes")
        )
        .expect("failed-row route assertions")
        .passed
    );
}
