//! Discrete route and clipping acceptance retains logical geometry.
#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "fixture failures require context"
)]
use locus_core::{AnnotationSpec, EvaluationInputs, ResolvedObject, Scene};
use locus_layout::{LayoutError, LayoutOptions, Leader, TextBoxes, layout_program, solve_layout};

fn scene(source: &str) -> Scene {
    let parsed = locus_syntax::parse(source);
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    Scene::new(parsed.file).expect("evaluated fixture")
}

fn path(scene: &Scene) -> locus_core::geom::route::Route {
    scene
        .resolved()
        .expect("resolved")
        .objects
        .values()
        .find_map(|object| match object {
            ResolvedObject::Annotation(AnnotationSpec::Arrow {
                route: Some(route), ..
            }) => Some(route.clone()),
            ResolvedObject::Annotation(AnnotationSpec::Arrow {
                from: locus_core::AnchorSpec::Point(from),
                to: locus_core::AnchorSpec::Point(to),
                bend,
                ..
            }) => locus_core::geom::route::Route::direct(*from, *to, *bend).ok(),
            _ => None,
        })
        .expect("logical route")
}

#[test]
fn clipping_preserves_logical_bounds_and_moves_arrowhead_to_boundary() {
    let scene = scene(
        "locus 0 beta 6\npoint start()\npoint finish = on_circle(circle(start, 4), 0deg)\nrect boundary(start, 2, 2)\narrow path(start, finish)\nrel trim_endpoints(path, boundary)\n",
    );
    let logical = path(&scene).clone();
    let feedback =
        layout_program(&scene, &TextBoxes::new(), LayoutOptions::default()).expect("clipped");
    assert!((feedback.retained_routes["path"].0 - 0.25).abs() < 1e-12);
    let Leader::Arrow { from, to, .. } = feedback.layout.annotations["path"].leader else {
        panic!("arrow leader")
    };
    assert!((from.x - logical.from.x - 1.0).abs() < 1e-12);
    assert_eq!(to, logical.to.clone().into());
    assert_eq!(path(&scene), logical);
    assert!((logical.length() - 4.0).abs() < 1e-12);
}

#[test]
fn clipping_collection_visits_arrow_and_segment_paths() {
    let scene = scene(
        "locus 0 beta 6\npoint start()\npoint finish = on_circle(circle(start, 4), 0deg)\nrect boundary(start, 2, 2)\narrow path(start, finish)\nsegment line(start, finish)\ngroup paths(path, line)\nrel trim_endpoints(paths, boundary)\n",
    );
    let feedback =
        layout_program(&scene, &TextBoxes::new(), LayoutOptions::default()).expect("clipped");
    assert_eq!(feedback.retained_routes.len(), 2);
    let visible = &feedback.layout.visible_paths["line"];
    assert!((visible.length() - 3.0).abs() < 1e-12);
}

#[test]
fn fully_enclosed_paths_have_source_anchored_conflicts() {
    let scene = scene(
        "locus 0 beta 6\npoint start()\npoint finish = on_circle(circle(start, 0.5), 0deg)\nrect boundary(start, 2, 2)\narrow path(start, finish)\nrel trim_endpoints(path, boundary)\n",
    );
    let error = layout_program(&scene, &TextBoxes::new(), LayoutOptions::default())
        .expect_err("fully enclosed");
    assert_eq!(error.code(), "E413");
    let LayoutError::Conflict {
        measurements, span, ..
    } = error
    else {
        panic!("conflict")
    };
    assert_eq!(measurements, vec!["path"]);
    assert!(span.byte_start > 0);
}

#[test]
fn incompatible_boundaries_are_diagnosed() {
    let scene = scene(
        "locus 0 beta 6\npoint start()\npoint finish = on_circle(circle(start, 4), 0deg)\nrect first(start, 7, 2)\nrect second(finish, 7, 2)\narrow path(start, finish)\nrel trim_endpoints(path, first)\nrel trim_endpoints(path, second)\n",
    );
    let error = layout_program(&scene, &TextBoxes::new(), LayoutOptions::default())
        .expect_err("incompatible trims");
    assert!(error.to_string().contains("incompatible"));
}

#[test]
fn curved_clipping_keeps_exact_quadratic_geometry() {
    let scene = scene(
        "locus 0 beta 6\npoint start()\npoint finish = on_circle(circle(start, 4), 0deg)\ncircle boundary(start, 1)\narrow path(start, finish) { bend: 45deg }\nrel trim_endpoints(path, boundary)\n",
    );
    let feedback = layout_program(&scene, &TextBoxes::new(), LayoutOptions::default())
        .expect("clipped quadratic");
    let Leader::Arrow {
        from,
        control: Some(control),
        to,
        ..
    } = feedback.layout.annotations["path"].leader
    else {
        panic!("quadratic leader")
    };
    assert!(((from - locus_core::Point::from(path(&scene).from)).norm() - 1.0).abs() < 1e-12);
    let retained = locus_core::geom::route::Route {
        from: from.into(),
        to: to.into(),
        control: Some(control.into()),
        via: Vec::new(),
        parameter: None,
    };
    let range = feedback.retained_routes["path"];
    for t in [0.0, 0.3, 0.7, 1.0] {
        let accepted = retained.sample(&t);
        let original = path(&scene).sample(&(range.0 + (range.1 - range.0) * t));
        assert!((accepted.x - original.x).abs() < 1e-12);
        assert!((accepted.y - original.y).abs() < 1e-12);
    }
}

#[test]
fn routed_avoidance_increases_offset_and_retains_authored_side() {
    let parsed = locus_syntax::parse(
        "locus 0 beta 6\npoint start()\npoint finish = on_circle(circle(start, 4), 90deg)\npoint center = on_circle(circle(start, 2), 90deg)\nrect obstacle(center, 2.5, 1)\narrow path(start, finish) { route: orthogonal, side: right }\nrel avoid(path, obstacle)\n",
    );
    let initial = Scene::new(parsed.file.clone()).expect("initial routed scene");
    let feedback = layout_program(&initial, &TextBoxes::new(), LayoutOptions::default())
        .expect("detour selection");
    assert_eq!(feedback.route_search_attempts, 4);
    let (scene, layout) = solve_layout(
        &parsed.file,
        EvaluationInputs::default(),
        &TextBoxes::new(),
        LayoutOptions::default(),
    )
    .expect("detour resolves obstacle");
    assert!(path(&scene).parameter.expect("offset") >= 2.0);
    assert_eq!(
        scene.inputs.choices.routes["path"].side,
        locus_core::geom::route::RouteSide::Right
    );
    let Leader::Routed { route, .. } = &layout.annotations["path"].leader else {
        panic!("routed leader")
    };
    assert_eq!(*route, path(&scene));
}

#[test]
fn route_clearance_does_not_replace_authored_length() {
    let parsed = locus_syntax::parse(
        "locus 0 beta 6\npoint start()\npoint finish = on_circle(circle(start, 4), 90deg)\npoint center = on_circle(circle(start, 2), 90deg)\nrect obstacle(center, 2.5, 1)\narrow path(start, finish) { route: orthogonal, side: right }\nrel path.length == 5\nrel avoid(path, obstacle)\n",
    );
    let error = solve_layout(
        &parsed.file,
        EvaluationInputs::default(),
        &TextBoxes::new(),
        LayoutOptions::default(),
    )
    .expect_err("length/clearance conflict");
    assert!(error.to_string().contains("E413"), "{error}");
}

#[test]
fn route_can_leave_an_attached_shape_without_moving_it() {
    let parsed = locus_syntax::parse(
        "locus 0 beta 6\npoint center()\nrect first(center, 2, 2)\npoint second_center = on_circle(circle(center, 4), 90deg)\nrect second(second_center, 2, 2)\narrow path(first.e, second.e) { route: orthogonal, side: right }\nrel avoid(path, first, second)\n",
    );
    let (scene, _) = solve_layout(
        &parsed.file,
        EvaluationInputs::default(),
        &TextBoxes::new(),
        LayoutOptions::default(),
    )
    .expect("attached outward route");
    assert!((path(&scene).from.x - path(&scene).to.x).abs() < 1e-12);
    assert!((path(&scene).to.y - path(&scene).from.y - 4.0).abs() < 1e-12);
}

fn measured_label() -> (EvaluationInputs, TextBoxes) {
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
fn clipped_label_uses_retained_route_while_logical_length_stays_fixed() {
    let parsed = locus_syntax::parse(
        "locus 0 beta 6\npoint start()\npoint finish = on_circle(circle(start, 4), 0deg)\ncircle source(start, 1)\ncircle target(finish, 1)\narrow path(start, finish)\nlabel caption(path, \"next\") { near: end, side: above }\nrel trim_endpoints(path, source)\nrel trim_endpoints(path, target)\n",
    );
    let (inputs, measured) = measured_label();
    let (scene, layout) = solve_layout(&parsed.file, inputs, &measured, LayoutOptions::default())
        .expect("clipped label feedback");
    let logical = path(&scene);
    let position = layout.annotations["caption"].bounds.center();
    assert!((position.x - logical.from.x - 2.6).abs() < 1e-12);
    assert!((logical.length() - 4.0).abs() < 1e-12);
    assert_eq!(scene.inputs.choices.retained_routes["path"], (0.25, 0.75));
}

#[test]
fn path_label_candidate_is_deterministic_and_honors_authored_near_and_side() {
    let parsed = locus_syntax::parse(
        "locus 0 beta 6\npoint start()\npoint finish = on_circle(circle(start, 4), 0deg)\npoint center = translate(on_circle(circle(start, 2), 0deg), start, on_circle(circle(start, 0.25), 90deg))\ncircle obstacle(center, 0.12)\narrow path(start, finish)\nlabel caption(path, \"next\") { near: middle, side: above }\n",
    );
    let (inputs, measured) = measured_label();
    let (scene, layout) = solve_layout(
        &parsed.file,
        inputs.clone(),
        &measured,
        LayoutOptions::default(),
    )
    .expect("candidate feedback");
    let choice = scene.inputs.choices.labels["caption"];
    assert_eq!(choice.side, locus_core::geom::route::RouteSide::Above);
    assert_eq!(choice.candidate, 1);
    let logical = path(&scene);
    let center = layout.annotations["caption"].bounds.center();
    assert!((center.x - logical.from.x - 1.6).abs() < 1e-12);
    assert!(center.y > logical.from.y);
    let (_, repeated) =
        solve_layout(&parsed.file, inputs, &measured, LayoutOptions::default()).expect("repeat");
    assert_eq!(layout, repeated);
}

#[test]
fn nested_instance_collections_visit_owned_paths_and_deduplicate_alias_identity() {
    let scene = scene(
        "locus 0 beta 6\npoint start()\npoint finish = on_circle(circle(start, 4), 0deg)\nrect boundary(start, 2, 2)\nfn link(from, to) {\n    arrow path(from, to)\n}\nlink first(start, finish)\nlink second(start, finish)\narrow alias = first.path\ngroup paths(first, alias, second)\nrel trim_endpoints(paths, boundary)\n",
    );
    let feedback = layout_program(&scene, &TextBoxes::new(), LayoutOptions::default())
        .expect("nested clipped paths");
    assert_eq!(feedback.retained_routes.len(), 2);
    assert!(feedback.retained_routes.contains_key("first.path"));
    assert!(feedback.retained_routes.contains_key("second.path"));
    assert!(!feedback.retained_routes.contains_key("alias"));
    assert_eq!(
        feedback.layout.annotations["alias"].leader,
        feedback.layout.annotations["first.path"].leader
    );
}

#[test]
fn clipping_rejects_collection_boundaries_even_when_they_have_one_member() {
    let scene = scene(
        "locus 0 beta 6\npoint start()\npoint finish = on_circle(circle(start, 4), 0deg)\nrect boundary(start, 2, 2)\ngroup boundaries(boundary)\narrow path(start, finish)\nrel trim_endpoints(path, boundaries)\n",
    );
    let error = layout_program(&scene, &TextBoxes::new(), LayoutOptions::default())
        .expect_err("collection has no outline");
    assert!(error.to_string().contains("collection"));
}

#[test]
fn oscillating_retained_label_measurements_exhaust_the_feedback_cap_honestly() {
    let parsed = locus_syntax::parse(
        "locus 0 beta 6\npoint start()\npoint finish = on_circle(circle(start, 4), 0deg)\narrow path(start, finish)\nlabel caption(path, \"next\") { near: start, side: above }\nrect boundary(caption.center, 2, 1)\nrel trim_endpoints(path, boundary)\n",
    );
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    let (inputs, measured) = measured_label();
    let error = solve_layout(&parsed.file, inputs, &measured, LayoutOptions::default())
        .expect_err("oscillating visible parameter");
    assert!(error.to_string().contains("did not converge"), "{error}");
    assert!(error.to_string().contains("path retained route"), "{error}");
}

#[test]
fn clipped_diagnostics_check_visible_strokes_and_exclude_removed_endpoint_spans() {
    let parsed = locus_syntax::parse(
        "locus 0 beta 6\npoint start() { mark: none }\npoint finish = on_circle(circle(start, 4), 0deg)\ncircle source(start, 1)\narrow path(start, finish)\nsegment stroke(start, finish)\ngroup paths(path, stroke)\nlabel caption(\"origin\")\nrel center(caption, start)\nrel trim_endpoints(paths, source)\n",
    );
    let (inputs, measured) = measured_label();
    let scene = Scene::with_inputs(parsed.file, inputs).expect("centered caption");
    let original = locus_layout::layout(
        scene.resolved().expect("resolved"),
        &measured,
        LayoutOptions::default(),
    )
    .expect("original layout");
    assert!(!original.diagnostics.is_empty());
    let feedback =
        layout_program(&scene, &measured, LayoutOptions::default()).expect("visible diagnostics");
    assert!(
        feedback.layout.diagnostics.is_empty(),
        "{:?}",
        feedback.layout.diagnostics
    );
}

#[test]
fn clipping_leaves_middle_crossings_and_exterior_endpoints_intact() {
    let scene = scene(
        "locus 0 beta 6\npoint start()\npoint finish = on_circle(circle(start, 4), 0deg)\npoint center = on_circle(circle(start, 2), 0deg)\ncircle boundary(center, 0.5)\narrow path(start, finish)\nrel trim_endpoints(path, boundary)\n",
    );
    let feedback = layout_program(&scene, &TextBoxes::new(), LayoutOptions::default())
        .expect("unchanged endpoints");
    assert_eq!(feedback.retained_routes["path"], (0.0, 1.0));
    let Leader::Arrow { from, to, .. } = feedback.layout.annotations["path"].leader else {
        panic!("visible arrow")
    };
    assert_eq!(from, path(&scene).from.into());
    assert_eq!(to, path(&scene).to.into());
}

#[test]
fn route_conflicts_follow_relation_order_when_route_declarations_are_reversed() {
    let scene = scene(
        "locus 0 beta 6\npoint start()\npoint finish = on_circle(circle(start, 4), 90deg)\npoint center = on_circle(circle(start, 2), 90deg)\nrect obstacle(center, 2000, 2000)\narrow first(start, finish) { route: orthogonal, side: right }\narrow second(start, finish) { route: orthogonal, side: right }\nrel avoid(second, obstacle)\nrel avoid(first, obstacle)\n",
    );
    let error = layout_program(&scene, &TextBoxes::new(), LayoutOptions::default())
        .expect_err("enclosed routes fail");
    let LayoutError::Conflict {
        measurements, span, ..
    } = error
    else {
        panic!("routing conflict")
    };
    assert_eq!(measurements, vec!["second"]);
    assert_eq!(span.line, 8);
}

#[test]
fn grouped_and_conjoined_clipping_requirements_are_applied_in_source_order() {
    let scene = scene(
        "locus 0 beta 6\npoint start()\npoint finish = on_circle(circle(start, 4), 0deg)\ncircle source(start,1)\ncircle target(finish,1)\narrow path(start,finish)\nrel (trim_endpoints(path,source)) and (trim_endpoints(path,target))\n",
    );
    let feedback = layout_program(&scene, &TextBoxes::new(), LayoutOptions::default())
        .expect("wrapped clipping");
    assert_eq!(feedback.retained_distances["path"], (1.0, 1.0));
    assert_eq!(feedback.retained_routes["path"], (0.25, 0.75));
}

#[test]
fn direct_route_avoidance_rejects_intersections_and_accepts_actual_clearance() {
    for (width, accepted) in [(2.0, false), (0.1, true)] {
        let source = format!(
            "locus 0 beta 6\npoint start()\npoint finish = on_circle(circle(start, 4), 90deg)\npoint center = translate(on_circle(circle(start, 0.5), 0deg), start, on_circle(circle(start, 2), 90deg))\nrect obstacle(center,{width},1)\narrow path(start,finish)\nrel avoid(path,obstacle)\n"
        );
        let scene = scene(&source);
        let result = layout_program(&scene, &TextBoxes::new(), LayoutOptions::default());
        assert_eq!(result.is_ok(), accepted, "{result:?}");
        if let Err(error) = result {
            assert_eq!(error.code(), "E413");
        }
    }
}

#[test]
fn fully_enclosed_automatic_route_searches_for_visible_detour_before_acceptance() {
    let parsed = locus_syntax::parse(
        "locus 0 beta 6\npoint start()\npoint finish = on_circle(circle(start, 2), 90deg)\npoint center = on_circle(circle(start, 1), 90deg)\nrect boundary(center,0.3,4)\narrow path(start,finish) { route: orthogonal, side: right }\nnum span = free 2.1\nrel path.length == span\nrel trim_endpoints(path,boundary)\n",
    );
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    let initial = Scene::new(parsed.file.clone()).expect("short feasible route");
    assert!(path(&initial).parameter.expect("short detour") < 0.15);
    let proposed = layout_program(&initial, &TextBoxes::new(), LayoutOptions::default())
        .expect("visible route candidate");
    assert!(proposed.route_search_attempts > 0);
    assert!(proposed.route_min_offsets["path"] > 0.15);
    let (scene, layout) = solve_layout(
        &parsed.file,
        EvaluationInputs::default(),
        &TextBoxes::new(),
        LayoutOptions::default(),
    )
    .expect("visible automatic route");
    assert!(layout.annotations["path"].leader != Leader::None);
    assert!(path(&scene).parameter.expect("accepted detour") > 0.15);
    assert!((scene.inputs.choices.retained_distances["path"].0 - 0.15).abs() < 1e-12);
}

#[test]
fn minimum_settled_offsets_recover_clipping_and_avoidance_within_budget() {
    for relation in ["trim_endpoints", "avoid"] {
        let height = if relation == "avoid" { 1 } else { 4 };
        let source = format!(
            "locus 0 beta 6\npoint start()\npoint finish = on_circle(circle(start, 2), 90deg)\npoint center = on_circle(circle(start, 1), 90deg)\nrect boundary(center,0.3,{height})\narrow path(start,finish) {{ route: orthogonal, side: right }}\nnum span = free 1\nrel path.length == span\nrel {relation}(path,boundary)\n"
        );
        let parsed = locus_syntax::parse(&source);
        assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
        let initial = Scene::new(parsed.file.clone()).expect("minimum detour");
        assert!(path(&initial).parameter.expect("offset") < 1e-5);
        let proposed = layout_program(&initial, &TextBoxes::new(), LayoutOptions::default())
            .expect("baseline candidate");
        assert!(proposed.route_min_offsets["path"] >= locus_core::preset::ROUTE_DETOUR);
        assert!(proposed.route_search_attempts <= locus_core::preset::SEARCH_CAP);
        let (accepted, _) = solve_layout(
            &parsed.file,
            EvaluationInputs::default(),
            &TextBoxes::new(),
            LayoutOptions::default(),
        )
        .expect("accepted visible route");
        let offset = path(&accepted).parameter.expect("offset");
        assert!(
            offset + 1e-9 >= locus_core::preset::ROUTE_DETOUR,
            "{relation}: {offset}"
        );
    }
}

#[test]
fn path_label_selection_predicts_the_proposed_clipped_route() {
    let parsed = locus_syntax::parse(
        "locus 0 beta 6\npoint start()\npoint finish = on_circle(circle(start, 2), 90deg)\npoint center = on_circle(circle(start, 1), 90deg)\nrect boundary(center,0.3,4)\npoint obstacle_center = translate(on_circle(circle(start, 1.7), 0deg), start, on_circle(circle(start, 1), 90deg))\nrect obstacle(obstacle_center,0.4,0.2)\narrow path(start,finish) { route: orthogonal, side: right }\nlabel caption(path,\"next\") { near: middle, side: right }\nnum span = free 1\nrel path.length == span\nrel trim_endpoints(path,boundary)\n",
    );
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    let (mut inputs, mut measured) = measured_label();
    inputs
        .measured
        .get_mut("caption")
        .expect("measurement")
        .width = 1.0;
    measured.get_mut("caption").expect("measurement").metrics =
        locus_tex::BoxMetrics::new(1.0, 0.2, 0.0);
    let initial = Scene::with_inputs(parsed.file.clone(), inputs.clone()).expect("initial label");
    assert!(path(&initial).parameter.expect("initial offset") < 1e-5);
    let feedback = layout_program(&initial, &measured, LayoutOptions::default())
        .expect("proposed clipped route");
    assert!(feedback.route_min_offsets["path"] >= 0.5);
    assert_eq!(feedback.label_choices["caption"].candidate, 1);
    let (accepted, _) = solve_layout(&parsed.file, inputs, &measured, LayoutOptions::default())
        .expect("stable label choice");
    assert!(
        accepted
            .solve_summary()
            .expect("summary")
            .measurement_passes
            <= locus_core::preset::MEASUREMENT_PASS_CAP
    );
}

#[test]
fn contained_path_labels_preserve_the_solved_candidate_inside_clear_outlines() {
    let parsed = locus_syntax::parse(
        "locus 0 beta 6\npoint start() { mark: none }\npoint finish = on_circle(circle(start, 8), 0deg)\npoint center = on_circle(circle(start, 4), 0deg)\nrect frame(center,10,20)\narrow path(start,finish)\nlabel caption(path,\"next\") { near: middle, side: above }\nrel contain(frame,path,caption)\n",
    );
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    for candidate in 0..3 {
        let (mut inputs, measured) = measured_label();
        inputs.choices.labels.insert(
            "caption".into(),
            locus_core::inputs::LabelChoice {
                side: locus_core::geom::route::RouteSide::Above,
                candidate,
            },
        );
        let scene =
            Scene::with_inputs(parsed.file.clone(), inputs.clone()).expect("contained label");
        let feedback =
            layout_program(&scene, &measured, LayoutOptions::default()).expect("outline scoring");
        assert_eq!(feedback.label_choices["caption"].candidate, candidate);
        let (accepted, _) = solve_layout(&parsed.file, inputs, &measured, LayoutOptions::default())
            .expect("stable contained caption");
        assert_eq!(
            accepted
                .solve_summary()
                .expect("summary")
                .measurement_passes,
            1
        );
    }
}

#[test]
fn visual_outline_scoring_preserves_explicit_solid_avoidance_failures() {
    let parsed = locus_syntax::parse(
        "locus 0 beta 6\npoint start()\npoint finish = on_circle(circle(start, 8), 0deg)\npoint center = on_circle(circle(start, 4), 0deg)\nrect frame(center,10,20)\narrow path(start,finish)\nlabel caption(path,\"next\") { side: above }\nrel contain(frame,caption)\nrel avoid(caption,frame)\n",
    );
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    let (inputs, measured) = measured_label();
    let error = solve_layout(&parsed.file, inputs, &measured, LayoutOptions::default())
        .expect_err("a contained label cannot avoid the solid interior");
    assert!(error.to_string().contains("E413"), "{error}");
}
