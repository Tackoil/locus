//! Bounded repair of coupled seed dependencies using the normative live rows.
//! Failed or stalled trials keep the best valid seed and defer to the full solver.
use crate::solve::{self, PreparedEvaluation, SourceRelation, UnknownSystem};

const MAX_STEPS: usize = 24;

pub(in crate::solve) fn repair(
    prepared: &PreparedEvaluation<'_>,
    system: &mut UnknownSystem,
    sources: &[SourceRelation<'_>],
) {
    let _timing = crate::metrics::span("seed_repair");
    let branches = solve::relations::select_circle_branches(sources);
    let mut normalized = system.normalized_hints.clone();
    let Ok(mut state) = solve::evaluate_state(prepared, system, sources, &branches, &normalized)
    else {
        return;
    };
    solve::trace::iteration(
        "initialization_start",
        prepared,
        system,
        &normalized,
        || solve::trace::state(&state),
    );
    let mut damping = 1e-3;
    for _ in 0..MAX_STEPS {
        if solve::max_residual(&state.residuals) < solve::SOLVE_EPSILON {
            break;
        }
        let _step = crate::metrics::span("seed_repair_step");
        freeze_authored_hints(system, &mut state.jacobian);
        let Some(delta) =
            solve::linear::lm_primary_step(&state.jacobian, &state.residuals, damping)
        else {
            break;
        };
        if let Some((trial, next)) = improving_trial(
            prepared,
            system,
            sources,
            &branches,
            &normalized,
            &delta,
            &state,
        ) {
            normalized = trial;
            state = next;
            damping = (damping / 10.0).max(1e-12);
        } else {
            damping *= 10.0;
            if damping > 1e3 {
                break;
            }
        }
    }
    system.hints = solve::denormalize(&normalized, &system.scales);
    system.normalized_hints = normalized;
}

fn freeze_authored_hints(system: &UnknownSystem, jacobian: &mut [Vec<f64>]) {
    for object in system.objects.iter().filter(|o| o.report_hint) {
        for row in jacobian.iter_mut() {
            row[object.start..object.start + object.count].fill(0.0);
        }
    }
}

fn improving_trial(
    prepared: &PreparedEvaluation<'_>,
    system: &UnknownSystem,
    sources: &[SourceRelation<'_>],
    branches: &[Option<solve::CircleBranch>],
    normalized: &[f64],
    delta: &[f64],
    state: &solve::SolveState,
) -> Option<(Vec<f64>, solve::SolveState)> {
    for factor in [1.0, 0.5, 0.25] {
        let trial: Vec<_> = normalized
            .iter()
            .zip(delta)
            .map(|(x, d)| x + factor * d)
            .collect();
        if let Ok(next) = solve::evaluate_state(prepared, system, sources, branches, &trial)
            && solve::objective(&next.residuals) < solve::objective(&state.residuals)
        {
            return Some((trial, next));
        }
    }
    None
}
