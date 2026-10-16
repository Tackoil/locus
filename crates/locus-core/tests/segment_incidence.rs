//! Finite-carrier convergence without authored placement hints.
#![allow(clippy::expect_used, reason = "regression diagnostics")]

use locus_core::{Scene, evaluate_assertions};

fn source(axis: &str, alignment: &str, carrier: &str, level: f64) -> String {
    format!(
        "locus 0 beta 6\nstyle textbook\n\
         point A()\n\
         point B() where {{ {alignment}(A, B), {axis}(B, A) == 40 }}\n\
         {carrier}\n\
         point L() where {{ {axis}(L, A) == {level} }}\n\
         point N() where {{ on(N, life), {alignment_level}(L, N) }}\n\
         assert {alignment}(A, B)\n\
         assert {axis}(B, A) == 40\n\
         assert {axis}(L, A) == {level}\n\
         assert on(N, life)\n\
         assert {alignment_level}(L, N)\n",
        alignment_level = if axis == "vdist" {
            "align_center_y"
        } else {
            "align_center_x"
        },
    )
}

fn assert_solves(source: &str) {
    let parsed = locus_syntax::parse(source);
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    let scene = Scene::new(parsed.file).expect("feasible finite incidence must converge");
    let summary = scene.solve_summary().expect("solve summary");
    assert!(summary.max_normalized_residual < 1e-12, "{summary:?}");
    assert!(
        evaluate_assertions(
            &scene.source_file,
            scene.resolved().expect("resolved scene")
        )
        .expect("assertion evaluation")
        .passed
    );
}

#[test]
fn below_segment_level_converges_without_coordinate_hints() {
    assert_solves(&source(
        "vdist",
        "align_center_x",
        "segment life(A, B)",
        3.0,
    ));
}

#[test]
fn segment_incidence_handles_both_directions_axes_and_endpoints() {
    for (axis, alignment) in [("vdist", "align_center_x"), ("hdist", "align_center_y")] {
        for carrier in ["segment life(A, B)", "segment life(B, A)"] {
            for level in [0.0, 3.0, 20.0, 37.0, 40.0] {
                assert_solves(&source(axis, alignment, carrier, level));
            }
        }
    }
}

#[test]
fn ray_incidence_handles_levels_past_the_through_point() {
    for level in [3.0, 40.0, 43.0] {
        assert_solves(&source("vdist", "align_center_x", "ray life(A, B)", level));
    }
}

#[test]
fn segment_incidence_rejects_levels_outside_both_endpoints() {
    for level in [-3.0, 43.0] {
        let parsed = locus_syntax::parse(&source(
            "vdist",
            "align_center_x",
            "segment life(A, B)",
            level,
        ));
        assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
        let error = Scene::new(parsed.file).expect_err("finite membership is infeasible");
        assert_eq!(error.code, "E403", "{error:?}");
    }
}
