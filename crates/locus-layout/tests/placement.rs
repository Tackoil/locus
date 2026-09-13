//! Known-scene and contract regressions using actual composed `MockEngine` boxes.
#![allow(clippy::expect_used, clippy::float_cmp)]

use locus_core::scene::{CompassDirection, LabelDirection};
use locus_core::{
    AnchorSpec, AnnotationSpec, BBox, InspectDiagnostic, Point, ResolvedObject, ResolvedScene,
    Scene, Tolerance, Vec2,
};
use locus_layout::{
    LayoutError, LayoutOptions, LayoutScene, Leader, TextBoxes, boxes::measure_scene,
    candidates::generate, layout, report::extend_inspection,
};
use locus_syntax::{Ident, Span};
use locus_tex::{BoxMetrics, FontSize, MockEngine, TexBox};

fn resolved(source: &str) -> ResolvedScene {
    let parsed = locus_syntax::parse(source);
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    Scene::new(parsed.file)
        .expect("evaluates")
        .resolved()
        .expect("resolved scene")
        .clone()
}

fn measured(scene: &ResolvedScene) -> TextBoxes {
    measure_scene(scene, &MockEngine, &MockEngine, FontSize::DEFAULT).expect("measured")
}

fn placed(scene: &ResolvedScene) -> LayoutScene {
    layout(scene, &measured(scene), LayoutOptions::default()).expect("layout")
}

fn triangle() -> ResolvedScene {
    resolved(
        "locus 2\npoint A = free ~(0, 0)\npoint B = free ~(6, 0)\npoint C = free ~(0, 6)\nsegment AB = seg(A, B)\nsegment BC = seg(B, C)\nsegment CA = seg(C, A)\nlabel a = label(A, \"A\")\nlabel b = label(B, \"B\")\nlabel c = label(C, \"C\")\n",
    )
}

#[test]
fn triangle_labels_use_hand_checkable_external_bisectors() {
    let scene = triangle();
    let before = scene.clone();
    let output = placed(&scene);
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let a = &output.annotations["a"];
    let b = &output.annotations["b"];
    let c = &output.annotations["c"];
    assert!(a.bounds.max.x < 0.0 && a.bounds.max.y < 0.0);
    assert!(b.bounds.min.x > 6.0 && b.bounds.center().y < 0.0);
    assert!(c.bounds.center().x < 0.0 && c.bounds.min.y > 6.0);
    let tolerance = Tolerance::from_bbox(&scene.bbox);
    let b_offset = b.bounds.center() - Point::new(6.0, 0.0);
    let c_offset = c.bounds.center() - Point::new(0.0, 6.0);
    let slope = -(libm::sqrt(2.0) - 1.0);
    assert!(tolerance.scalar_zero(b_offset.y / b_offset.x - slope, 0.0, 1.0));
    assert!(tolerance.scalar_zero(c_offset.x / c_offset.y - slope, 0.0, 1.0));
    assert_eq!(
        [a.candidate_index, b.candidate_index, c.candidate_index],
        [0, 0, 0]
    );
    assert_eq!(scene, before);
    assert_eq!(output.geometry_bbox, scene.bbox);
}

#[test]
fn repeated_layout_is_identical_including_diagnostics_and_leaders() {
    let scene = triangle();
    assert_eq!(placed(&scene), placed(&scene));
    assert_eq!(
        placed(&scene).annotations.keys().collect::<Vec<_>>(),
        ["a", "b", "c"]
    );
}

#[test]
fn compass_has_sixteen_candidates_and_explicit_directions_are_pinned() {
    let scene = resolved("locus 2\npoint A = free ~(0, 0)\n");
    let anchor = Point::new(0.0, 0.0);
    let size = Vec2::new(2.0, 1.0);
    let automatic = generate(&scene, anchor, size, LabelDirection::Auto, 0.25);
    assert_eq!(automatic.len(), 16);
    assert_eq!(automatic[0].bounds.min.y, 0.25);
    assert!(automatic[8].bounds.min.y > automatic[0].bounds.min.y);
    let pinned = generate(
        &scene,
        anchor,
        size,
        LabelDirection::Compass(CompassDirection::Ne),
        0.25,
    );
    assert_eq!(pinned.len(), 1);
    assert_eq!(pinned[0], automatic[1]);
    let center = generate(&scene, anchor, size, LabelDirection::Center, 100.0);
    assert_eq!(center.len(), 1);
    assert_eq!(center[0].bounds.center(), anchor);
}

#[test]
fn segment_and_circle_preferences_precede_compass_order() {
    let scene = resolved(
        "locus 2\npoint A = free ~(0, 0)\npoint B = free ~(8, 0)\nsegment s = seg(A, B)\ncircle c = circle(A, 2)\n",
    );
    let size = Vec2::new(1.0, 1.0);
    let midpoint = generate(
        &scene,
        Point::new(4.0, 0.0),
        size,
        LabelDirection::Auto,
        0.25,
    );
    assert_eq!(midpoint.len(), 20);
    assert_eq!(midpoint[0].bounds.min.y, 0.25);
    assert_eq!(midpoint[1].bounds.max.y, -0.25);
    let radial = generate(
        &scene,
        Point::new(-2.0, 0.0),
        size,
        LabelDirection::Auto,
        0.25,
    );
    assert_eq!(radial.len(), 18);
    assert_eq!(radial[0].bounds.max.x, -2.25);
}

#[test]
fn unavoidable_crowding_reports_both_labels_and_source_spans() {
    let scene = resolved(
        "locus 2\npoint A = free ~(0, 0) { mark: none }\nlabel first = label(A, \"X\") { dir: center }\nlabel second = label(A, \"Y\") { dir: center }\n",
    );
    let output = placed(&scene);
    assert_eq!(
        output.annotations["first"].bounds.center(),
        Point::new(0.0, 0.0)
    );
    assert_eq!(
        output.diagnostics,
        vec![InspectDiagnostic::LabelCollision {
            labels: vec!["first".into(), "second".into()],
            spans: vec![
                output.annotations["first"].id.span,
                output.annotations["second"].id.span
            ],
        }]
    );
    assert_eq!(output, placed(&scene));
}

#[test]
fn automatic_crowding_and_margin_violations_are_reported() {
    let scene = resolved(
        "locus 2\npoint A = free ~(0, 0)\nlabel a = label(A, \"wide wide wide\")\nlabel b = label(A, \"wide wide wide\")\nlabel c = label(A, \"wide wide wide\")\n",
    );
    let output = layout(
        &scene,
        &measured(&scene),
        LayoutOptions {
            margin: 0.0,
            ..LayoutOptions::default()
        },
    )
    .expect("crowded layout succeeds");
    assert_eq!(output.annotations.len(), 3);
    assert!(output.diagnostics.len() >= 3);
}

#[test]
fn pinned_box_is_reserved_before_automatic_box_regardless_of_id_order() {
    let scene = resolved(
        "locus 2\npoint A = free ~(0, 0) { mark: none }\nlabel auto = label(A, \"X\")\nlabel pinned = label(A, \"Y\") { dir: N }\n",
    );
    let output = placed(&scene);
    assert_eq!(output.annotations["pinned"].bounds.min.y, 0.25);
    assert_ne!(output.annotations["auto"].candidate_index, 0);
    assert!(output.diagnostics.is_empty());
}

#[test]
fn identifier_order_breaks_equal_candidate_assignment_ties() {
    let scene = resolved(
        "locus 2\npoint A = free ~(0, 0) { mark: none }\nlabel z = label(A, \"X\")\nlabel a = label(A, \"X\")\n",
    );
    let output = placed(&scene);
    assert_eq!(output.annotations["a"].candidate_index, 0);
    assert_ne!(output.annotations["z"].candidate_index, 0);
    assert_eq!(output.annotations.keys().collect::<Vec<_>>(), ["z", "a"]);
}

#[test]
fn mixed_multiline_measurements_keep_baselines_and_actual_extents() {
    let scene = resolved(
        "locus 2\npoint A = free ~(0, 0) { mark: none }\nlabel a = label(A, \"A$x$\\nWW\") { dir: center }\n",
    );
    let measurements = measured(&scene);
    let options = LayoutOptions {
        em_size: 2.0,
        margin: 10.0,
        ..LayoutOptions::default()
    };
    let output = layout(&scene, &measurements, options).expect("layout");
    let text = output.annotations["a"].text.as_ref().expect("text box");
    let tolerance = Tolerance::from_bbox(&output.bbox);
    assert!(tolerance.length_zero(text.bounds.width() - measurements["a"].metrics.width * 2.0));
    assert!(tolerance.length_zero(
        text.bounds.height()
            - (measurements["a"].metrics.height + measurements["a"].metrics.depth) * 2.0
    ));
    assert_eq!(text.measured, measurements["a"]);
    assert_eq!(
        text.baseline.y,
        text.bounds.max.y - text.measured.metrics.height * 2.0
    );
    assert_eq!(
        text.measured
            .svg_body
            .matches("data-engine=\"mock\"")
            .count(),
        4
    );
}

#[test]
fn all_annotation_kinds_resolve_deferred_centers_and_leader_info() {
    let scene = resolved(
        "locus 2\npoint A = free ~(0, 0)\npoint B = free ~(6, 0)\npoint C = free ~(0, 6)\nsegment s = seg(A, B)\nlabel a = label(A, \"A\") { dir: W }\nlabel b = label(a.center, \"B\") { dir: N }\narrow link = arrow(a.center, B) { head: <->, bend: 20deg }\ntick equal = tick(s, 3)\nright_angle square = right_angle(A, B, C) { size: 0.5 }\ntext_block caption = text_block(\"caption\")\ntext_block note = text_block(\"note\") { anchor: b.center, dir: E }\n",
    );
    let output = placed(&scene);
    assert_eq!(output.annotations.len(), 7);
    assert_eq!(output.annotations["equal"].strokes.len(), 3);
    assert_eq!(output.annotations["square"].strokes.len(), 2);
    assert!(output.annotations["caption"].bounds.max.y < scene.bbox.min.y);
    assert!(
        matches!(output.annotations["link"].leader, Leader::Arrow { from, to, control: Some(_), .. } if from == output.annotations["a"].bounds.center() && to == Point::new(6.0, 0.0))
    );
    assert!(
        matches!(output.annotations["b"].leader, Leader::Segment(s) if s.start == output.annotations["a"].bounds.center())
    );
    assert_eq!(output, placed(&scene));
}

#[test]
fn missing_measurement_and_invalid_metrics_are_source_anchored() {
    let scene = triangle();
    let missing = layout(&scene, &TextBoxes::new(), LayoutOptions::default())
        .expect_err("missing measurement");
    assert!(
        matches!(missing, LayoutError::MissingMeasurement { ref id, span } if id == "a" && span.line > 0)
    );
    assert_eq!(missing.code(), "E410");
    let mut measurements = measured(&scene);
    measurements.insert(
        "a".into(),
        TexBox::new("", BoxMetrics::new(f64::NAN, 1.0, 0.0)),
    );
    assert_eq!(
        layout(&scene, &measurements, LayoutOptions::default())
            .expect_err("invalid metrics")
            .code(),
        "E412"
    );
}

#[test]
fn forward_layout_dependencies_and_cycles_are_handled_without_recursion() {
    let mut scene = resolved(
        "locus 2\npoint A = free ~(0, 0)\nlabel first = label(A, \"A\")\nlabel next = label(first.center, \"B\")\n",
    );
    scene.objects.reverse();
    assert_eq!(placed(&scene).annotations.len(), 2);
    let id = scene
        .objects
        .keys()
        .find(|id| id.name == "first")
        .expect("first")
        .clone();
    let ResolvedObject::Annotation(AnnotationSpec::Label { anchor, .. }) =
        scene.objects.get_mut(&id).expect("annotation")
    else {
        unreachable!()
    };
    let span = Span::new(99, 3, 20, 31);
    *anchor = AnchorSpec::AnnotationCenter {
        id: "next".into(),
        span,
    };
    assert!(matches!(
        layout(&scene, &measured(&scene), LayoutOptions::default()),
        Err(LayoutError::Anchor { .. })
    ));
}

#[test]
fn inspect_accepts_layout_diagnostics_without_core_dependency_changes() {
    let parsed = locus_syntax::parse(
        "locus 2\npoint A = free ~(0, 0)\nlabel a = label(A, \"A\") { dir: center }\n",
    );
    let scene = Scene::new(parsed.file.clone()).expect("scene");
    let resolved = scene.resolved().expect("resolved");
    let output = placed(resolved);
    let mut report = locus_core::inspect(&parsed.file, resolved, None);
    let previous = report.diagnostics.len();
    extend_inspection(&mut report, &output);
    assert_eq!(&report.diagnostics[previous..], output.diagnostics);
    assert!(!output.diagnostics.is_empty());
}

#[test]
fn invalid_options_and_overflow_are_rejected() {
    let scene = triangle();
    assert_eq!(
        layout(
            &scene,
            &measured(&scene),
            LayoutOptions {
                gap: -1.0,
                ..LayoutOptions::default()
            }
        )
        .expect_err("invalid gap")
        .code(),
        "E413"
    );
    let mut boxes = measured(&scene);
    boxes.insert(
        "a".into(),
        TexBox::new("", BoxMetrics::new(1.0, f64::MAX, f64::MAX)),
    );
    assert_eq!(
        layout(&scene, &boxes, LayoutOptions::default())
            .expect_err("overflow")
            .code(),
        "E412"
    );
}

#[test]
fn empty_scene_has_no_annotations_or_diagnostics() {
    let scene = ResolvedScene {
        objects: indexmap::IndexMap::<Ident, ResolvedObject>::new(),
        bbox: BBox {
            min: Point::new(0.0, 0.0),
            max: Point::new(0.0, 0.0),
        },
    };
    let output = placed(&scene);
    assert!(output.annotations.is_empty() && output.diagnostics.is_empty());
    assert_eq!(output.bbox, scene.bbox);
}

#[test]
fn angle_arc_supplies_external_bisector_without_drawn_segments() {
    let scene = resolved(
        "locus 2\npoint A = free ~(2, 0)\npoint V = free ~(0, 0)\npoint B = free ~(0, 2)\narc angle = angle_arc(A, V, B, 0.5)\nlabel a = label(V, \"A\")\n",
    );
    let output = placed(&scene);
    assert_eq!(output.annotations["a"].candidate_index, 0);
    assert!(output.annotations["a"].bounds.max.x < 0.0);
    assert!(output.annotations["a"].bounds.max.y < 0.0);
    assert!(output.diagnostics.is_empty());
}

#[test]
fn residual_pass_detects_arrow_added_after_its_anchor_label() {
    let scene = resolved(
        "locus 2\npoint A = free ~(0, 0) { mark: none }\npoint B = free ~(5, 0) { mark: none }\nlabel a = label(A, \"X\") { dir: center }\narrow link = arrow(a.center, B)\n",
    );
    let output = placed(&scene);
    assert_eq!(output.diagnostics.len(), 1);
    assert!(
        matches!(&output.diagnostics[0], InspectDiagnostic::LabelCollision { labels, .. } if labels == &["a"])
    );
}

#[test]
fn missing_anchor_reports_reference_span() {
    let mut scene = triangle();
    let (_, object) = scene
        .objects
        .iter_mut()
        .find(|(id, _)| id.name == "a")
        .expect("label");
    let ResolvedObject::Annotation(AnnotationSpec::Label { anchor, .. }) = object else {
        unreachable!()
    };
    let reference_span = Span::new(42, 10, 300, 314);
    *anchor = AnchorSpec::AnnotationCenter {
        id: "missing".into(),
        span: reference_span,
    };
    assert_eq!(
        layout(&scene, &measured(&scene), LayoutOptions::default()),
        Err(LayoutError::Anchor {
            id: "missing".into(),
            span: reference_span
        })
    );
}

#[test]
fn fewer_available_candidates_take_priority_over_identifier_order() {
    let scene = resolved(
        "locus 2\npoint A = free ~(0, 0) { mark: none }\nlabel a = label(A, \"X\")\nlabel z = label(A, \"WWWW\")\n",
    );
    // The wide label only fits north/south within the two-unit margin. The
    // small label fits in more directions, so z must reserve north first.
    let output = placed(&scene);
    assert_eq!(output.annotations["z"].candidate_index, 0);
    assert_ne!(output.annotations["a"].candidate_index, 0);
    assert!(output.diagnostics.is_empty());
}

fn connector_fixture(connector_id: &str, direction: &str, connector_first: bool) -> ResolvedScene {
    let connector = format!("label {connector_id} = label(A, \"X\") {{ dir: N, gap: 4 }}\n");
    let crossed = format!("label b = label(B, \"X\") {{ dir: {direction} }}\n");
    let labels = if connector_first {
        connector + &crossed
    } else {
        crossed + &connector
    };
    resolved(&format!(
        "locus 2\npoint A = free ~(0, 0) {{ mark: none }}\npoint B = free ~(0, 2) {{ mark: none }}\n{labels}"
    ))
}

#[test]
fn pinned_connector_collisions_are_reported_in_any_placement_or_declaration_order() {
    for connector_id in ["a", "z"] {
        // Identifier ties place a before b, and z after b. Neither source
        // declaration order nor assignment order may hide the final collision.
        for connector_first in [true, false] {
            for stroke_width in [0.0, 0.04] {
                let scene = connector_fixture(connector_id, "center", connector_first);
                let options = LayoutOptions {
                    margin: 10.0,
                    stroke_width,
                    ..LayoutOptions::default()
                };
                let output = layout(&scene, &measured(&scene), options).expect("layout");
                assert_eq!(
                    output.diagnostics,
                    vec![InspectDiagnostic::LabelCollision {
                        labels: vec!["b".into()],
                        spans: vec![output.annotations["b"].id.span],
                    }]
                );
                assert_eq!(
                    output,
                    layout(&scene, &measured(&scene), options).expect("repeat")
                );
            }
        }
    }
}

#[test]
fn automatic_labels_avoid_previously_placed_text_connectors() {
    for stroke_width in [0.0, 0.04] {
        let scene = connector_fixture("a", "auto", true);
        let output = layout(
            &scene,
            &measured(&scene),
            LayoutOptions {
                margin: 10.0,
                stroke_width,
                ..LayoutOptions::default()
            },
        )
        .expect("layout");
        // The first (north) candidate intersects a's vertical connector.
        assert_ne!(output.annotations["b"].candidate_index, 0);
        assert!(output.annotations["b"].bounds.min.x > stroke_width / 2.0);
        assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    }
}

#[test]
fn isolated_labels_do_not_collide_with_their_own_connector_attachment() {
    let scene = resolved(
        "locus 2\npoint A = free ~(0, 0) { mark: none }\nlabel a = label(A, \"X\") { dir: N, gap: 4 }\n",
    );
    for stroke_width in [0.0, 0.04] {
        let output = layout(
            &scene,
            &measured(&scene),
            LayoutOptions {
                margin: 10.0,
                stroke_width,
                ..LayoutOptions::default()
            },
        )
        .expect("layout");
        assert!(matches!(output.annotations["a"].leader, Leader::Segment(_)));
        assert!(output.diagnostics.is_empty());
    }
}

#[test]
fn nearly_straight_arrows_report_crossings_and_leave_nearby_boxes_clear() {
    for bend in ["0", "0.00000000000001", "-0.00000000000001"] {
        for position in [0.4, 1.2, 3.6] {
            for offset in [0.0, 0.1] {
                let scene = resolved(&format!(
                    "locus 2\npoint A = free ~(0, 0) {{ mark: none }}\npoint B = free ~(4, 4) {{ mark: none }}\npoint C = free ~({position}, {}) {{ mark: none }}\narrow a = arrow(A, B) {{ bend: {bend}deg }}\nlabel b = label(C, \"X\") {{ dir: center }}\n",
                    position + offset
                ));
                let mut boxes = measured(&scene);
                // Exercise the public premeasured-box input with the review's
                // small box, where inaccurate curve parameters miss an edge.
                boxes.get_mut("b").expect("box").metrics = BoxMetrics::new(0.01, 0.005, 0.005);
                let output = layout(
                    &scene,
                    &boxes,
                    LayoutOptions {
                        margin: 10.0,
                        ..LayoutOptions::default()
                    },
                )
                .expect("layout");
                let expected = if offset == 0.0 {
                    vec![InspectDiagnostic::LabelCollision {
                        labels: vec!["b".into()],
                        spans: vec![output.annotations["b"].id.span],
                    }]
                } else {
                    Vec::new()
                };
                assert_eq!(
                    output.diagnostics, expected,
                    "bend={bend}, position={position}, offset={offset}"
                );
            }
        }
    }
}
