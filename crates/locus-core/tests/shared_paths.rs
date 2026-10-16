//! Constructor compatibility and geometry invariants of decorated finite paths.
#![allow(clippy::expect_used, reason = "fixture failures need context")]
use locus_core::{
    AnnotationSpec, EvaluationInputs, MeasuredRole, MeasuredText, PathGeometry, ResolvedObject,
    Scene, evaluate_assertions_with_inputs,
};
use locus_syntax::{HeadKind, parse};

const POINTS: &str =
    "locus 0 beta 6\npoint start()\npoint end = on_circle(circle(start, 4), 0deg)\n";

fn scene(source: &str, inputs: EvaluationInputs) -> Scene {
    let parsed = parse(source);
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    Scene::with_inputs(parsed.file, inputs).expect("valid shared path scene")
}

fn object<'a>(scene: &'a Scene, name: &str) -> &'a ResolvedObject {
    scene
        .resolved()
        .expect("resolved scene")
        .objects
        .iter()
        .find(|(id, _)| id.name == name)
        .map(|(_, object)| object)
        .expect("named object")
}

#[test]
fn constructors_share_paths_and_preserve_defaults() {
    let source = format!(
        "{POINTS}segment plain(start,end)\narrow directed(start,end)\narrow bare(start,end) {{ head: - }}\nsegment double(start,end) {{ head: <-> }}\n"
    );
    let scene = scene(&source, EvaluationInputs::default());
    for (name, head) in [
        ("plain", HeadKind::None),
        ("directed", HeadKind::Forward),
        ("bare", HeadKind::None),
        ("double", HeadKind::Both),
    ] {
        let path = object(&scene, name).as_path().expect("shared path");
        assert_eq!(path.head, head);
        let route = path
            .route()
            .expect("valid centerline")
            .expect("geometric endpoints");
        assert!((route.length() - 4.0).abs() < 1e-12);
        assert!(route.control.is_none() && route.via.is_empty());
    }
    assert!(matches!(
        object(&scene, "plain").as_path().expect("path").geometry,
        PathGeometry::Segment(_)
    ));
    assert!(matches!(
        object(&scene, "directed").as_path().expect("path").geometry,
        PathGeometry::Routed { .. }
    ));
}

#[test]
fn segment_heads_preserve_solver_incidence_intersections_projection_and_ticks() {
    let source = format!(
        "{POINTS}segment vector(start,end) {{ head: <-> }}\npoint marker()\nrel on(marker,vector)\nrel dist(start,marker) == 2\npoint upper = on_circle(circle(marker,2),90deg)\npoint projected = foot(upper,vector)\nline cross_line(marker,upper)\npoint crossing = intersect(vector,cross_line)\ntick marks(vector,2)\nassert on(marker,vector)\nassert dist(projected,marker) == 0\nassert dist(crossing,marker) == 0\nassert vector.length == 4\nassert dist(vector.start,start) == 0\nassert dist(vector.end,end) == 0\nassert dist(vector.midpoint,marker) == 0\n"
    );
    let scene = scene(&source, EvaluationInputs::default());
    assert!(
        evaluate_assertions_with_inputs(
            &scene.source_file,
            scene.resolved().expect("resolved"),
            &scene.inputs
        )
        .expect("geometry assertions")
        .passed
    );
}

#[test]
fn aliases_and_boundary_segments_inherit_or_override_only_decoration() {
    let source = format!(
        "{POINTS}segment original(start,end) {{ head: -> }}\nsegment inherited = original\nsegment bare = original {{ head: - }}\nrect panel(start,4,2)\nsegment edge_path = edge(panel,N) {{ head: <-> }}\n"
    );
    let scene = scene(&source, EvaluationInputs::default());
    for (name, head) in [
        ("original", HeadKind::Forward),
        ("inherited", HeadKind::Forward),
        ("bare", HeadKind::None),
        ("edge_path", HeadKind::Both),
    ] {
        assert_eq!(object(&scene, name).as_path().expect("path").head, head);
    }
    assert_eq!(
        object(&scene, "original").as_path().expect("path").geometry,
        object(&scene, "bare").as_path().expect("path").geometry
    );
}

#[test]
fn segments_reject_bending_routing_and_invalid_heads() {
    for props in [
        "bend: 0deg",
        "route: direct",
        "route: orthogonal",
        "side: above",
        "head: sideways",
    ] {
        let source = format!("{POINTS}segment path(start,end) {{ {props} }}\n");
        let parsed = parse(&source);
        let rejected = !parsed.diagnostics.is_empty() || Scene::new(parsed.file).is_err();
        assert!(rejected, "accepted incompatible segment property: {props}");
    }
}

#[test]
fn measured_labels_attach_to_segments_without_changing_geometric_length() {
    let source = format!(
        "{POINTS}segment path(start,end) {{ head: -> }}\nlabel caption(path,\"Caption\") {{ near: middle, side: above }}\npoint target = on_circle(circle(path.midpoint,2),90deg)\nrel center(caption,target)\nassert center(caption,target)\nassert path.length == 4\n"
    );
    let mut inputs = EvaluationInputs::default();
    inputs.measured.insert(
        "caption".into(),
        MeasuredText {
            width: 1.0,
            height: 0.5,
            role: MeasuredRole::Normal,
        },
    );
    let scene = scene(&source, inputs);
    let ResolvedObject::Annotation(AnnotationSpec::Label {
        path: Some(attachment),
        bounds: Some(bounds),
        ..
    }) = object(&scene, "caption")
    else {
        unreachable!("path label");
    };
    assert_eq!(attachment.path, "path");
    let path = object(&scene, "path").as_path().expect("path");
    assert_eq!(path.label_bounds, Some(*bounds));
    assert!(
        path.layout_bbox().expect("layout bounds").height()
            > path.geometry_bbox().expect("centerline bounds").height()
    );
    assert!((path.route().expect("route").expect("centerline").length() - 4.0).abs() < 1e-12);
    assert!(
        evaluate_assertions_with_inputs(
            &scene.source_file,
            scene.resolved().expect("resolved"),
            &scene.inputs,
        )
        .expect("path label assertions")
        .passed
    );
}

#[test]
fn zero_length_segment_remains_valid_with_optional_heads() {
    let source = "locus 0 beta 6\npoint origin()\nsegment path(origin,origin) { head: <-> }\n";
    let scene = scene(source, EvaluationInputs::default());
    let path = object(&scene, "path").as_path().expect("path");
    assert!(matches!(path.geometry, PathGeometry::Segment(_)));
    assert!(path.route().expect("valid").expect("route").length().abs() < 1e-12);
}

#[test]
fn connectors_preserve_deferred_annotation_and_path_center_anchors() {
    let source = format!(
        "{POINTS}label caption(start,\"Caption\")\narrow first(caption.center,end)\narrow second(first.center,end)\n"
    );
    let scene = scene(&source, EvaluationInputs::default());
    let path = object(&scene, "second").as_path().expect("path");
    let PathGeometry::Routed {
        from: locus_core::AnchorSpec::AnnotationCenter { id, .. },
        ..
    } = &path.geometry
    else {
        unreachable!("deferred path center");
    };
    assert_eq!(id, "first");
    assert!(path.route().expect("deferred centerline").is_none());
}
