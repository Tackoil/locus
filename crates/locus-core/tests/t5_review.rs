//! End-to-end regressions for the T5 solver review.
#![allow(clippy::expect_used)]

use std::fmt::Write as _;

use locus_core::{EvalError, Solved, evaluate_assertions, solve};
use locus_syntax::{File, parse};

fn parsed(source: &str) -> File {
    let parsed = parse(source);
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    parsed.file
}

fn solved_and_checked(source: &str) -> Solved {
    let file = parsed(source);
    let solved = solve(&file).expect("valid system solves");
    let assertions = evaluate_assertions(&file, &solved.scene).expect("valid assertions");
    assert!(assertions.passed, "{assertions:?}");
    solved
}

fn failure(source: &str) -> EvalError {
    solve(&parsed(source)).expect_err("invalid system must fail")
}

#[test]
fn lower_half_plane_directions_agree_with_assertions() {
    for (hint, angle) in [("0, -1", 270), ("1, -1", 315)] {
        let source = format!(
            "locus 0 beta 4\npoint A = point()\npoint B = offset(A, {hint})\n\
             num theta = dir_angle(A, B)\nrel theta == {angle}deg\n\
             rel dir_angle(A, B) == {angle}deg\n\
             assert theta == {angle}deg\nassert dir_angle(A, B) == {angle}deg\n"
        );
        assert_eq!(solved_and_checked(&source).summary.attempts, 0);
    }
}

#[test]
fn division_cancellation_has_identical_primal_values() {
    let solved = solved_and_checked(
        "locus 0 beta 4\nnum x = free ~1\nnum y = (3 / 5 - 0.6) * 100000000\n\
         rel y == 0\nrel (3 / 5 - 0.6) * 100000000 == 0\n\
         assert y == 0\nassert (3 / 5 - 0.6) * 100000000 == 0\n",
    );
    assert_eq!(solved.summary.attempts, 0);
}

#[test]
fn reflection_and_translation_agree_before_and_after_solving() {
    let solved = solved_and_checked(
        "locus 0 beta 4\npoint P = point()\npoint C = point()\n\
         num delta = hdist(reflect(P, C), translate(C, P, C)) * 100000000\n\
         rel delta == 0\nassert delta == 0\n",
    );
    assert_eq!(solved.summary.attempts, 0);
}

#[test]
fn inline_angle_rejects_degeneracy_and_segment_foot_clamps() {
    let angle = failure(
        "locus 0 beta 4\npoint A = point()\npoint B = offset(A, 1, 0)\n\
         point C = offset(A, 1, 0)\nrel angle(A, B, C) == 180deg\n",
    );
    assert_eq!(angle.code, "E404");
    assert_eq!(angle.span.line, 5);
    assert!(angle.message.contains("angle"));
    assert!(!angle.measured.is_empty());

    let foot = solved_and_checked(
        "locus 0 beta 4\npoint A = point()\npoint B = offset(A, 1, 0)\n\
         point P = offset(A, 2, 1)\nsegment s = seg(A, B)\n\
         rel coincident(foot(P, s), B)\nassert coincident(foot(P, s), B)\n",
    );
    assert_eq!(foot.summary.attempts, 0);
}

#[test]
fn conflicting_constant_targets_report_a_minimal_pair() {
    let error = failure("locus 0 beta 4\nnum x = free ~0\nrel x == 0\nrel x == 1\nrel x == 2\n");
    assert_eq!(error.code, "E402");
    assert!(!error.message.contains("candidate"), "{error}");
    let subset = error
        .message
        .split("minimal infeasible subset")
        .nth(1)
        .expect("a direct contradiction can be isolated")
        .split(';')
        .next()
        .expect("subset report");
    let lines = subset
        .split(|character: char| !character.is_ascii_digit())
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    assert_eq!(lines.len(), 2, "{error}");
}

#[test]
fn failure_report_keeps_every_relation_residual() {
    let mut source = "locus 0 beta 4\nnum x = free ~0\n".to_owned();
    for target in 0..10 {
        writeln!(source, "rel x == {target}").expect("write to string");
    }
    let error = failure(&source);
    for line in 3..=12 {
        assert!(
            error.message.contains(&format!("line {line} residual")),
            "{error}"
        );
    }
}

#[test]
fn stalled_feasible_nonlinear_system_does_not_claim_a_conflict_subset() {
    let error = failure("locus 0 beta 4\nnum x = free ~0\nrel x * x == 1\n");
    assert_eq!(error.code, "E403");
    assert!(!error.message.contains("candidate"), "{error}");
    assert!(!error.message.contains("subset: lines"), "{error}");
}

#[test]
fn unrelated_relation_cannot_turn_nonconvergence_into_a_proven_conflict() {
    let error =
        failure("locus 0 beta 4\nnum x = free ~0\nnum y = free ~0\nrel x * x == 1\nrel y == 0\n");
    assert_eq!(error.code, "E403");
    assert!(!error.message.contains("subset: lines"), "{error}");
    assert!(error.message.contains("non-convergent"), "{error}");
    solved_and_checked(
        "locus 0 beta 4\nnum x = free ~1\nnum y = free ~0\nrel x * x == 1\nrel y == 0\n",
    );
}

#[test]
fn contradictory_numeric_targets_retain_an_isolated_pair() {
    let error = failure("locus 0 beta 4\nnum x = free ~0\nrel x == 0\nrel x == 1\n");
    assert!(
        error
            .message
            .contains("minimal infeasible subset: lines 3, 4"),
        "{error}"
    );
}

#[test]
fn overlapping_numeric_targets_are_consistent_at_solve_tolerance() {
    solved_and_checked("locus 0 beta 4\nnum x = free ~0\nrel x == 0\nrel x == 0.0000000000005\n");
    solved_and_checked(
        "locus 0 beta 4\nnum x = free ~1000000000000\n\
         rel x == 1000000000000\nrel x == 1000000000000.5\n",
    );
    let stalled = failure(
        "locus 0 beta 4\nnum x = free ~0\nnum y = free ~0\n\
         rel x == 0\nrel x == 0.0000000000005\nrel y * y == 1\n",
    );
    assert_eq!(stalled.code, "E403");
    assert!(!stalled.message.contains("subset: lines"), "{stalled}");
}

#[test]
fn fixed_false_relation_is_a_proven_singleton_conflict() {
    let error = failure("locus 0 beta 4\nnum fixed = 1\nrel fixed == 2\n");
    assert_eq!(error.code, "E402");
    assert!(
        error.message.contains("minimal infeasible subset: lines 3"),
        "{error}"
    );
}
