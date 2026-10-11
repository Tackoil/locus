//! Milestone-zero integration contract for expansion and solve preparation.
#![allow(
    clippy::expect_used,
    reason = "fixture failures need explicit test context"
)]

use locus_core::{ResolvedObject, Scene, evaluate_assertions};
use locus_syntax::parse;

fn scene(source: &str) -> Scene {
    let parsed = parse(source);
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    Scene::new(parsed.file).expect("beta 6 scene")
}

#[test]
fn expanded_geometry_shares_one_solve_and_retains_source() {
    let source = include_str!("../../../fixtures/beta6/m0/expanded_geometry.locus");
    let mut scene = scene(source);
    let summary = scene.solve_summary().expect("one shared solve");
    assert_eq!(summary.unknowns, 8);
    assert_eq!(summary.equations, 4);
    let ids = summary
        .dof
        .objects
        .iter()
        .map(|object| object.id.as_str())
        .collect::<Vec<_>>();
    assert_eq!(ids, ["first.A", "first.B", "second.A", "second.B"]);
    assert_eq!(
        scene.collections["paths"].members,
        ["first.path", "second.path"]
    );
    let resolved = scene.resolved().expect("resolved geometry");
    assert!(
        evaluate_assertions(&scene.source_file, resolved)
            .expect("assertions")
            .passed
    );
    let before = scene.evaluation.clone();
    scene.recompute().expect("repeat expansion and solve");
    assert_eq!(scene.evaluation, before);
}

#[test]
fn deferred_intersection_runs_after_the_solve() {
    let source = include_str!("../../../fixtures/beta6/m0/deferred_intersection.locus");
    let scene = scene(source);
    let resolved = scene.resolved().expect("resolved circles");
    assert!(resolved.objects.keys().any(|id| id.name == "P"));
    assert!(
        evaluate_assertions(&scene.file, resolved)
            .expect("incidence assertions")
            .passed
    );
}

#[test]
fn unused_numeric_unknown_retains_hint_and_dof() {
    let source = include_str!("../../../fixtures/beta6/m0/unreferenced_unknown.locus");
    let scene = scene(source);
    let summary = scene.solve_summary().expect("free number solve");
    assert_eq!(summary.unknowns, 1);
    assert_eq!(summary.dof.global_remaining, 1);
    assert_eq!(summary.dof.objects[0].id, "spare");
    assert_eq!(summary.hint_displacements[0].source_components, [0.0]);
    assert!(
        matches!(scene.resolved().expect("resolved number").objects.values().next(),
        Some(ResolvedObject::Num(value)) if value.value.to_bits() == 7.0_f64.to_bits())
    );
}

#[test]
fn construction_error_identifies_the_invoked_instance() {
    let parsed =
        parse("locus 0 beta 6\nfn bad(radius) {\n    circle disk(radius)\n}\nbad example(-1)\n");
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    let error = Scene::new(parsed.file).expect_err("negative radius");
    assert_eq!(error.object_id.as_deref(), Some("example.disk"));
    assert_eq!(error.span.line, 3);
    assert_eq!(error.invocation_chain.len(), 1);
    assert_eq!(error.invocation_chain[0].instance, "example");
    assert_eq!(error.invocation_chain[0].span.line, 5);
}
