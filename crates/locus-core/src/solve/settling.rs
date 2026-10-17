//! Feasible preference settling with a deterministic primal working set.

use super::linear;
use crate::preset::{
    ACTIVE_TOLERANCE, DIRECTION_TOLERANCE, LINE_SEARCH_ARMIJO, LINE_SEARCH_MIN_ALPHA,
    RESTORATION_CAP, SETTLING_PIVOT_CAP, SETTLING_STEP_CAP, SETTLING_TOLERANCE, SETTLING_WORK_CAP,
};
use crate::tol::{RANK_FACTOR, SOLVE_EPSILON};
mod recovery;

#[derive(Clone, Debug, Default)]
pub(crate) struct Rows {
    pub(crate) values: Vec<f64>,
    pub(crate) jacobian: Vec<Vec<f64>>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct ConstraintState {
    pub(crate) equalities: Rows,
    /// Raw normalized inequalities with feasible sign `g >= 0`.
    pub(crate) inequalities: Rows,
    pub(crate) preferences: Rows,
}

#[derive(Clone, Debug)]
pub(crate) struct SettlingReport {
    pub(crate) complete: bool,
    pub(crate) reason: &'static str,
    pub(crate) projected_gradient: f64,
    pub(crate) active_inequalities: Vec<usize>,
    pub(crate) active_null: Vec<Vec<f64>>,
    pub(crate) steps: usize,
    pub(crate) accepted_moves: usize,
    pub(crate) working_set_pivots: usize,
    pub(crate) model_rebuilds: usize,
    pub(crate) evaluations: usize,
    pub(crate) line_search_evaluations: usize,
    pub(crate) restoration_steps: usize,
}

#[derive(Clone, Debug)]
pub(crate) struct SettlingResult {
    pub(crate) z: Vec<f64>,
    pub(crate) report: SettlingReport,
}

struct Direction {
    delta: Vec<f64>,
    slope: f64,
    projected_gradient: f64,
    release: Option<usize>,
    null: Vec<Vec<f64>>,
}

pub(crate) fn optimize(
    initial: &[f64],
    mut evaluate: impl FnMut(&[f64]) -> Option<ConstraintState>,
) -> SettlingResult {
    let mut evaluations = 0;
    let mut result = optimize_counted(initial, |z| {
        evaluations += 1;
        evaluate(z)
    });
    result.report.evaluations = evaluations;
    result
}

fn optimize_counted(
    initial: &[f64],
    mut evaluate: impl FnMut(&[f64]) -> Option<ConstraintState>,
) -> SettlingResult {
    let mut result = initial_result(initial);
    let Some(mut state) = evaluate(initial) else {
        return result;
    };
    let mut active = initially_active(&state);
    let mut consecutive_pivots = 0;
    let mut rebuilt = false;
    let mut radius = crate::preset::SETTLING_TRUST_RADIUS;
    let mut tangent_cache = linear::TangentCache::default();
    for _ in 0..SETTLING_WORK_CAP {
        if consecutive_pivots == SETTLING_PIVOT_CAP {
            break;
        }
        result.report.steps += 1;
        crate::trace::record("settling_iteration", || {
            (
                serde_json::json!({
                    "iteration": result.report.steps, "normalized": result.z,
                    "active": active, "equalities": state.equalities.values,
                    "inequalities": state.inequalities.values,
                    "preferences": state.preferences.values, "trust_radius": radius,
                    "accepted_moves": result.report.accepted_moves,
                }),
                None,
            )
        });
        let Some(independent) =
            independent_boundaries(&state, &active, initial.len(), &mut tangent_cache)
        else {
            result.report.reason = "stalled";
            break;
        };
        active = independent;
        let Some(direction) =
            direction_with_radius(&state, &active, initial.len(), radius, &mut tangent_cache)
        else {
            result.report.reason = "stalled";
            break;
        };
        let direction = recover_direction(
            &mut result,
            &state,
            &active,
            direction,
            &mut evaluate,
            &mut rebuilt,
        );
        update_direction_report(&mut result.report, &direction, &active);
        if release(&mut active, direction.release) {
            consecutive_pivots += 1;
            result.report.working_set_pivots += 1;
            continue;
        }
        if finish_direction(&mut result.report, &state, &direction) {
            return result;
        }
        if result.report.accepted_moves == SETTLING_STEP_CAP {
            break;
        }
        let (ratio, blocker) = blocking_ratio(&state, &active, &direction.delta);
        if ratio <= 0.0 {
            activate(&mut active, blocker);
            consecutive_pivots += 1;
            result.report.working_set_pivots += 1;
            continue;
        }
        let accepted = line_search(
            &result.z,
            &state,
            &active,
            &direction,
            ratio,
            &mut evaluate,
            &mut result.report,
        );
        let Some((z, accepted_state)) = accepted else {
            if shrink_radius(&mut radius) {
                continue;
            }
            result.report.reason = "line_search_failed";
            return result;
        };
        result.z = z;
        state = accepted_state;
        result.report.accepted_moves += 1;
        radius = (2.0 * radius).min(crate::preset::SETTLING_TRUST_RADIUS);
        consecutive_pivots = 0;
        activate_boundary(&mut active, blocker, &state);
    }
    finalize_report(&mut result.report, &state, &active, initial.len());
    result
}

fn recover_direction(
    result: &mut SettlingResult,
    state: &ConstraintState,
    active: &[usize],
    direction: Direction,
    evaluate: &mut impl FnMut(&[f64]) -> Option<ConstraintState>,
    rebuilt: &mut bool,
) -> Direction {
    if !*rebuilt && result.report.accepted_moves > 0 && recovery::eligible(state, &direction) {
        *rebuilt = true;
        result.report.model_rebuilds += 1;
        return recovery::rebuild(
            &result.z,
            state,
            active,
            &direction,
            evaluate,
            &mut result.report,
        )
        .unwrap_or(direction);
    }
    direction
}

fn shrink_radius(radius: &mut f64) -> bool {
    let smaller = *radius * crate::preset::SETTLING_TRUST_SHRINK;
    if smaller < crate::preset::SETTLING_TRUST_MIN_RADIUS {
        return false;
    }
    *radius = smaller;
    true
}

fn initial_result(initial: &[f64]) -> SettlingResult {
    SettlingResult {
        z: initial.to_vec(),
        report: SettlingReport {
            complete: false,
            reason: "step_cap",
            projected_gradient: f64::INFINITY,
            active_inequalities: Vec::new(),
            active_null: linear::unconstrained(initial.len()).null,
            steps: 0,
            accepted_moves: 0,
            working_set_pivots: 0,
            model_rebuilds: 0,
            evaluations: 0,
            line_search_evaluations: 0,
            restoration_steps: 0,
        },
    }
}

fn initially_active(state: &ConstraintState) -> Vec<usize> {
    state
        .inequalities
        .values
        .iter()
        .enumerate()
        .filter_map(|(index, value)| (*value <= ACTIVE_TOLERANCE).then_some(index))
        .collect()
}

fn working_jacobian(state: &ConstraintState, active: &[usize]) -> Vec<Vec<f64>> {
    let mut result = state.equalities.jacobian.clone();
    result.extend(
        active
            .iter()
            .map(|index| state.inequalities.jacobian[*index].clone()),
    );
    result
}

/// Keep inequality multipliers unique modulo the equality row space. Redundant
/// rows still participate in blocking, restoration and every feasibility check.
fn independent_boundaries(
    state: &ConstraintState,
    active: &[usize],
    columns: usize,
    cache: &mut linear::TangentCache,
) -> Option<Vec<usize>> {
    let null = if state.equalities.jacobian.is_empty() {
        linear::unconstrained(columns).null
    } else {
        cache.projector(&state.equalities.jacobian).ok()?
    };
    let mut basis: Vec<Vec<f64>> = Vec::new();
    let mut result = Vec::new();
    for index in active {
        // A near-boundary nomination is not a binding equality. Keeping a
        // genuinely slack row would falsely certify a nonstationary interior.
        if state.inequalities.values[*index] >= SOLVE_EPSILON {
            continue;
        }
        let row = &state.inequalities.jacobian[*index];
        let mut residual = linear::multiply(&null, row);
        orthogonalize(&mut residual, &basis);
        let norm = libm::sqrt(dot(&residual, &residual));
        if norm > RANK_FACTOR * libm::sqrt(dot(row, row)).max(1.0) {
            basis.push(residual.iter().map(|v| v / norm).collect());
            result.push(*index);
        }
    }
    Some(result)
}

fn orthogonalize(row: &mut [f64], basis: &[Vec<f64>]) {
    // Reorthogonalization prevents nearly dependent rows acquiring a false rank.
    for _ in 0..2 {
        for vector in basis {
            let projection = dot(row, vector);
            for (value, component) in row.iter_mut().zip(vector) {
                *value -= projection * component;
            }
        }
    }
}

fn direction(state: &ConstraintState, active: &[usize], columns: usize) -> Option<Direction> {
    direction_with_radius(
        state,
        active,
        columns,
        crate::preset::SETTLING_TRUST_RADIUS,
        &mut linear::TangentCache::default(),
    )
}

fn direction_with_radius(
    state: &ConstraintState,
    active: &[usize],
    columns: usize,
    radius: f64,
    cache: &mut linear::TangentCache,
) -> Option<Direction> {
    let working = working_jacobian(state, active);
    let (null, basis) = if working.is_empty() {
        let null = linear::unconstrained(columns).null;
        (null.clone(), null)
    } else {
        linear::working_tangent(&state.equalities.jacobian, &working, columns, cache).ok()?
    };
    let projected = project_rows(&state.preferences.jacobian, &basis);
    let delta = preference_step(&projected, &state.preferences.values, &basis, radius)?;
    let gradient = gradient(&state.preferences, columns);
    let tangent_gradient = linear::multiply(&null, &gradient);
    let projected_gradient = infinity_norm(&tangent_gradient);
    let slope = dot(&tangent_gradient, &delta);
    let stationary = projected_gradient
        <= SETTLING_TOLERANCE * infinity_norm(&state.preferences.values).max(1.0);
    let release = releasing_row(state, active, &basis, &delta, stationary, cache).ok()?;
    Some(Direction {
        delta,
        slope,
        projected_gradient,
        release,
        null,
    })
}

fn project_rows(rows: &[Vec<f64>], null: &[Vec<f64>]) -> Vec<Vec<f64>> {
    let transposed = linear::transpose(null);
    rows.iter()
        .map(|row| linear::multiply(&transposed, row))
        .collect()
}

fn preference_step(
    projected: &[Vec<f64>],
    values: &[f64],
    null: &[Vec<f64>],
    radius: f64,
) -> Option<Vec<f64>> {
    if projected.is_empty() {
        return Some(vec![0.0; null.len()]);
    }
    let rhs = values.iter().map(|value| -value).collect::<Vec<_>>();
    let step = linear::bounded_least_squares(projected, &rhs, radius).ok()?;
    Some(linear::multiply(null, &step))
}

fn gradient(rows: &Rows, columns: usize) -> Vec<f64> {
    if rows.jacobian.is_empty() {
        return vec![0.0; columns];
    }
    linear::multiply(&linear::transpose(&rows.jacobian), &rows.values)
}

fn releasing_row(
    state: &ConstraintState,
    active: &[usize],
    basis: &[Vec<f64>],
    delta: &[f64],
    stationary: bool,
    cache: &mut linear::TangentCache,
) -> Result<Option<usize>, ()> {
    if active.is_empty() {
        return Ok(None);
    }
    let mut model = state.preferences.clone();
    if !stationary {
        let change = linear::multiply(&model.jacobian, delta);
        for (value, change) in model.values.iter_mut().zip(change) {
            *value += change;
        }
    }
    let rhs = gradient(&model, delta.len())
        .iter()
        .map(|value| -value)
        .collect::<Vec<_>>();
    let multipliers = boundary_multipliers(state, active, basis, &rhs, cache)?;
    Ok(active.iter().enumerate().find_map(|(position, index)| {
        (multipliers[position] > SETTLING_TOLERANCE).then_some(*index)
    }))
}

fn boundary_multipliers(
    state: &ConstraintState,
    active: &[usize],
    working_basis: &[Vec<f64>],
    rhs: &[f64],
    cache: &mut linear::TangentCache,
) -> Result<Vec<f64>, ()> {
    let equality_basis = if state.equalities.jacobian.is_empty() {
        linear::unconstrained(rhs.len()).null
    } else {
        cache.space(&state.equalities.jacobian)?.1
    };
    let equality_remaining = equality_basis.first().map_or(0, Vec::len);
    let working_remaining = working_basis.first().map_or(0, Vec::len);
    if equality_remaining == working_remaining + active.len() {
        let transpose = linear::transpose(&equality_basis);
        let projected: Vec<_> = active
            .iter()
            .map(|index| linear::multiply(&transpose, &state.inequalities.jacobian[*index]))
            .collect();
        return linear::least_squares(
            &linear::transpose(&projected),
            &linear::multiply(&transpose, rhs),
        );
    }
    let working = working_jacobian(state, active);
    let multipliers = linear::least_squares(&linear::transpose(&working), rhs)?;
    Ok(multipliers[state.equalities.values.len()..].to_vec())
}

fn release(active: &mut Vec<usize>, row: Option<usize>) -> bool {
    let Some(row) = row else {
        return false;
    };
    active.retain(|index| *index != row);
    true
}

fn finish_direction(
    report: &mut SettlingReport,
    state: &ConstraintState,
    direction: &Direction,
) -> bool {
    let threshold = SETTLING_TOLERANCE * infinity_norm(&state.preferences.values).max(1.0);
    if direction.projected_gradient <= threshold {
        report.complete = true;
        report.reason = "complete";
        return true;
    }
    if direction.slope >= -DIRECTION_TOLERANCE * libm::sqrt(dot(&direction.delta, &direction.delta))
    {
        report.reason = "stalled";
        return true;
    }
    false
}

fn blocking_ratio(
    state: &ConstraintState,
    active: &[usize],
    delta: &[f64],
) -> (f64, Option<usize>) {
    let changes = linear::multiply(&state.inequalities.jacobian, delta);
    let mut result = (1.0, None);
    for (index, change) in changes.iter().enumerate() {
        let roundoff = DIRECTION_TOLERANCE
            * libm::sqrt(dot(
                &state.inequalities.jacobian[index],
                &state.inequalities.jacobian[index],
            ))
            .max(1.0)
            * libm::sqrt(dot(delta, delta)).max(1.0);
        if active.contains(&index) || *change >= -roundoff {
            continue;
        }
        let ratio = state.inequalities.values[index].max(0.0) / -*change;
        if ratio < result.0 || (ratio.total_cmp(&result.0).is_eq() && result.1.is_none()) {
            result = (ratio, Some(index));
        }
    }
    result
}

fn activate(active: &mut Vec<usize>, blocker: Option<usize>) {
    if let Some(index) = blocker {
        active.push(index);
        active.sort_unstable();
        active.dedup();
    }
}

fn activate_boundary(active: &mut Vec<usize>, blocker: Option<usize>, state: &ConstraintState) {
    activate(
        active,
        blocker.filter(|index| state.inequalities.values[*index] <= ACTIVE_TOLERANCE),
    );
}

fn line_search(
    z: &[f64],
    state: &ConstraintState,
    active: &[usize],
    direction: &Direction,
    ratio: f64,
    evaluate: &mut impl FnMut(&[f64]) -> Option<ConstraintState>,
    report: &mut SettlingReport,
) -> Option<(Vec<f64>, ConstraintState)> {
    let current_merit = merit(&state.preferences.values);
    let mut alpha = 1.0;
    while alpha >= LINE_SEARCH_MIN_ALPHA {
        report.line_search_evaluations += 1;
        crate::trace::record("settling_line_search", || {
            (
                serde_json::json!({
                    "iteration": report.line_search_evaluations, "alpha": alpha, "ratio": ratio,
                    "normalized": z, "direction": direction.delta,
                }),
                None,
            )
        });
        let trial = add_step(z, &direction.delta, alpha * ratio);
        let restored = restore(trial, active, evaluate, report);
        if let Some((trial, trial_state)) = restored {
            let bound = current_merit + LINE_SEARCH_ARMIJO * alpha * ratio * direction.slope;
            if merit(&trial_state.preferences.values) <= bound {
                return Some((trial, trial_state));
            }
        }
        alpha *= 0.5;
    }
    None
}

fn restore(
    mut z: Vec<f64>,
    active: &[usize],
    evaluate: &mut impl FnMut(&[f64]) -> Option<ConstraintState>,
    report: &mut SettlingReport,
) -> Option<(Vec<f64>, ConstraintState)> {
    let mut state = evaluate(&z)?;
    let mut damping = 1.0e-12;
    for _ in 0..RESTORATION_CAP {
        if working_feasible(&state, active) {
            return Some((z, state));
        }
        report.restoration_steps += 1;
        crate::trace::record("settling_restoration", || {
            (
                serde_json::json!({
                    "iteration": report.restoration_steps, "normalized": z,
                    "equalities": state.equalities.values, "inequalities": state.inequalities.values,
                    "damping": damping,
                }),
                None,
            )
        });
        let rows = restoration_rows(&state, active);
        let step = linear::restoration_step(&rows.jacobian, &rows.values, damping)?;
        let accepted = restoration_search(&z, &step, active, merit(&rows.values), evaluate);
        if let Some((trial, trial_state)) = accepted {
            z = trial;
            state = trial_state;
            damping = (damping / 10.0).max(1.0e-15);
        } else {
            damping *= 10.0;
        }
    }
    working_feasible(&state, active).then_some((z, state))
}

fn restoration_search(
    z: &[f64],
    step: &[f64],
    active: &[usize],
    previous: f64,
    evaluate: &mut impl FnMut(&[f64]) -> Option<ConstraintState>,
) -> Option<(Vec<f64>, ConstraintState)> {
    // A full correction can cross a live geometric piece or violate a previously
    // slack face. Shorten that same correction before rebuilding it with damping.
    for alpha in [1.0, 0.5, 0.25] {
        let trial = add_step(z, step, alpha);
        if let Some(state) = restoration_trial(&trial, active, previous, alpha, evaluate) {
            return Some((trial, state));
        }
    }
    None
}

fn restoration_rows(state: &ConstraintState, active: &[usize]) -> Rows {
    let mut rows = state.equalities.clone();
    for (index, value) in state.inequalities.values.iter().enumerate() {
        if *value <= -SOLVE_EPSILON || active.contains(&index) {
            rows.values.push(*value);
            rows.jacobian
                .push(state.inequalities.jacobian[index].clone());
        }
    }
    rows
}

fn restoration_trial(
    z: &[f64],
    active: &[usize],
    previous: f64,
    alpha: f64,
    evaluate: &mut impl FnMut(&[f64]) -> Option<ConstraintState>,
) -> Option<ConstraintState> {
    let state = evaluate(z)?;
    let feasible = working_feasible(&state, active);
    let trial_merit = merit(&restoration_rows(&state, active).values);
    let accepted = feasible || trial_merit < previous;
    crate::trace::record("settling_restoration_trial", || {
        (
            serde_json::json!({
                "normalized": z, "alpha": alpha,
                "previous_merit": previous, "merit": trial_merit,
                "feasible": feasible, "accepted": accepted, "active": active,
                "equalities": state.equalities.values,
                "inequalities": state.inequalities.values,
            }),
            None,
        )
    });
    accepted.then_some(state)
}

fn working_feasible(state: &ConstraintState, active: &[usize]) -> bool {
    feasible(state)
        && active
            .iter()
            .all(|index| state.inequalities.values[*index].abs() < SOLVE_EPSILON)
}

fn feasible(state: &ConstraintState) -> bool {
    state
        .equalities
        .values
        .iter()
        .all(|value| value.abs() < SOLVE_EPSILON)
        && state
            .inequalities
            .values
            .iter()
            .all(|value| value.is_finite() && *value > -SOLVE_EPSILON)
        && state
            .preferences
            .values
            .iter()
            .all(|value| value.is_finite())
}

fn update_direction_report(report: &mut SettlingReport, direction: &Direction, active: &[usize]) {
    report.projected_gradient = direction.projected_gradient;
    report.active_inequalities = active.to_vec();
    report.active_null.clone_from(&direction.null);
}

fn finalize_report(
    report: &mut SettlingReport,
    state: &ConstraintState,
    active: &[usize],
    columns: usize,
) {
    if let Some(direction) = direction(state, active, columns) {
        update_direction_report(report, &direction, active);
    }
}

fn add_step(z: &[f64], delta: &[f64], factor: f64) -> Vec<f64> {
    z.iter()
        .zip(delta)
        .map(|(value, step)| value + factor * step)
        .collect()
}

fn dot(lhs: &[f64], rhs: &[f64]) -> f64 {
    lhs.iter().zip(rhs).map(|(left, right)| left * right).sum()
}

fn infinity_norm(values: &[f64]) -> f64 {
    values.iter().map(|value| value.abs()).fold(0.0, f64::max)
}

fn merit(values: &[f64]) -> f64 {
    0.5 * dot(values, values)
}

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests {
    use super::*;

    fn rows(values: &[f64], jacobian: &[&[f64]]) -> Rows {
        Rows {
            values: values.to_vec(),
            jacobian: jacobian.iter().map(|row| row.to_vec()).collect(),
        }
    }

    fn one_dimension(x: f64, target: f64) -> ConstraintState {
        ConstraintState {
            inequalities: rows(&[x], &[&[1.0]]),
            preferences: rows(&[x - target], &[&[1.0]]),
            ..ConstraintState::default()
        }
    }

    #[test]
    fn releases_boundary_when_preference_points_into_interior() {
        let result = optimize(&[0.0], |z| Some(one_dimension(z[0], 1.0)));
        assert_eq!(result.z, [1.0]);
        assert!(result.report.complete);
        assert!(result.report.active_inequalities.is_empty());
        assert_eq!(result.report.line_search_evaluations, 1);
    }

    #[test]
    fn blocks_at_boundary_and_retains_active_requirement() {
        let result = optimize(&[2.0], |z| Some(one_dimension(z[0], -1.0)));
        assert_eq!(result.z, [0.0]);
        assert!(result.report.complete);
        assert_eq!(result.report.active_inequalities, [0]);
        assert_eq!(result.report.projected_gradient, 0.0);
    }

    #[test]
    fn chooses_lowest_index_on_blocking_ties() {
        let result = optimize(&[0.0], |z| {
            Some(ConstraintState {
                inequalities: rows(&[1.0 - z[0], 1.0 - z[0]], &[&[-1.0], &[-1.0]]),
                preferences: rows(&[z[0] - 2.0], &[&[1.0]]),
                ..ConstraintState::default()
            })
        });
        assert_eq!(result.z, [1.0]);
        assert_eq!(result.report.active_inequalities, [0]);
        assert!(result.report.complete);
    }

    #[test]
    fn chooses_lowest_index_when_multiple_boundaries_release() {
        let state = ConstraintState {
            inequalities: rows(&[0.0, 0.0], &[&[1.0, 0.0], &[0.0, 1.0]]),
            preferences: rows(&[-1.0, -1.0], &[&[1.0, 0.0], &[0.0, 1.0]]),
            ..ConstraintState::default()
        };
        let computed = direction(&state, &[0, 1], 2);
        assert_eq!(computed.and_then(|direction| direction.release), Some(0));
    }

    #[test]
    fn scaled_preference_accepts_exact_gauss_newton_step() {
        let result = optimize(&[0.0], |z| {
            Some(ConstraintState {
                preferences: rows(&[0.001 * (z[0] - 1.0)], &[&[0.001]]),
                ..ConstraintState::default()
            })
        });
        assert!((result.z[0] - 1.0).abs() < 1.0e-12);
        assert_eq!(result.report.line_search_evaluations, 1);
        assert!(result.report.complete);
    }

    #[test]
    fn rank_truncation_reports_stall_instead_of_false_convergence() {
        let result = optimize(&[0.0, 0.0], |z| {
            Some(ConstraintState {
                preferences: rows(&[1.0e11 * z[0], z[1] - 1.0], &[&[1.0e11, 0.0], &[0.0, 1.0]]),
                ..ConstraintState::default()
            })
        });
        assert_eq!(result.z, [0.0, 0.0]);
        assert!(!result.report.complete);
        assert_eq!(result.report.reason, "stalled");
        assert_eq!(result.report.projected_gradient, 1.0);
        assert_eq!(result.report.line_search_evaluations, 0);
    }

    #[test]
    fn armijo_backtracking_prevents_the_zero_one_cycle() {
        let mut accepted_merits = Vec::new();
        let result = optimize(&[0.0], |z| {
            let x = z[0];
            let q = cycle_polynomial(x);
            accepted_merits.push((x, 0.5 * (x * x + q * q)));
            Some(ConstraintState {
                preferences: rows(&[x, q], &[&[1.0], &[6.0 * x * x - 3.0 * x - 2.0]]),
                ..ConstraintState::default()
            })
        });
        assert!(result.z[0] > 0.7 && result.z[0] < 1.0);
        assert!(result.report.line_search_evaluations > result.report.steps - 1);
        assert!(merit(&[result.z[0], cycle_polynomial(result.z[0])]) < 1.0);
        assert!(
            accepted_merits
                .iter()
                .any(|(x, _)| (*x - 1.0).abs() < 1.0e-12)
        );
        assert!(
            accepted_merits
                .iter()
                .any(|(x, _)| (*x - 0.5).abs() < 1.0e-12)
        );
    }

    fn cycle_polynomial(x: f64) -> f64 {
        2.0 * x * x * x - 1.5 * x * x - 2.0 * x + 2.5
    }

    #[test]
    fn restores_curved_equalities_before_acceptance() {
        let result = optimize(&[1.0, 0.0], |z| {
            Some(ConstraintState {
                equalities: rows(
                    &[z[0] * z[0] + z[1] * z[1] - 1.0],
                    &[&[2.0 * z[0], 2.0 * z[1]]],
                ),
                preferences: rows(&[z[0], z[1] - 1.0], &[&[1.0, 0.0], &[0.0, 1.0]]),
                ..ConstraintState::default()
            })
        });
        assert!(result.z[0].abs() < 1.0e-6);
        assert!(
            (result.z[0] * result.z[0] + result.z[1] * result.z[1] - 1.0).abs() < SOLVE_EPSILON
        );
        assert!(result.report.restoration_steps > 0);
        assert!(result.report.complete, "{:?}", result.report);
        assert!(result.report.projected_gradient <= SETTLING_TOLERANCE);
    }

    #[test]
    fn restores_nonlinear_inequality_and_blocks_at_its_boundary() {
        let result = optimize(&[0.0], |z| {
            Some(ConstraintState {
                inequalities: rows(&[1.0 - z[0] * z[0]], &[&[-2.0 * z[0]]]),
                preferences: rows(&[z[0] - 2.0], &[&[1.0]]),
                ..ConstraintState::default()
            })
        });
        assert!((result.z[0] - 1.0).abs() < SOLVE_EPSILON);
        assert!(1.0 - result.z[0] * result.z[0] > -SOLVE_EPSILON);
        assert!(result.report.restoration_steps > 0);
        assert!(result.report.complete);
        assert_eq!(result.report.active_inequalities, [0]);
    }

    #[test]
    fn restores_active_boundary_even_when_tangent_trial_is_feasible_interior() {
        let result = optimize(&[1.0, 0.0], |z| {
            Some(ConstraintState {
                inequalities: rows(
                    &[z[0] * z[0] + z[1] * z[1] - 1.0],
                    &[&[2.0 * z[0], 2.0 * z[1]]],
                ),
                preferences: rows(&[z[0], z[1] - 1.0], &[&[1.0, 0.0], &[0.0, 1.0]]),
                ..ConstraintState::default()
            })
        });
        assert!(result.report.restoration_steps > 0);
        assert!(
            (result.z[0] * result.z[0] + result.z[1] * result.z[1] - 1.0).abs() < SOLVE_EPSILON
        );
        assert!(result.z[0].abs() < 1.0e-6);
        assert_eq!(result.report.active_inequalities, [0]);
    }

    #[test]
    fn failed_line_search_rolls_back_last_feasible_iterate() {
        let result = optimize(&[0.0], |z| {
            if z[0] == 0.0 {
                return Some(ConstraintState {
                    preferences: rows(&[-1.0], &[&[1.0]]),
                    ..ConstraintState::default()
                });
            }
            Some(ConstraintState {
                preferences: rows(&[2.0], &[&[1.0]]),
                ..ConstraintState::default()
            })
        });
        assert_eq!(result.z, [0.0]);
        assert!(!result.report.complete);
        assert_eq!(result.report.reason, "line_search_failed");
        assert!(
            result.report.steps > 1,
            "smaller trust radii must be retried"
        );
        assert!(result.report.steps <= SETTLING_WORK_CAP);
        assert_eq!(
            result.report.line_search_evaluations,
            11 * result.report.steps
        );
    }

    #[test]
    fn restoration_failures_are_bounded_and_do_not_escape_as_solve_failure() {
        let result = optimize(&[0.0], |z| {
            Some(ConstraintState {
                equalities: rows(&[if z[0] == 0.0 { 0.0 } else { 1.0 }], &[&[0.0]]),
                preferences: rows(&[z[0] - 1.0], &[&[1.0]]),
                ..ConstraintState::default()
            })
        });
        assert_eq!(result.z, [0.0]);
        assert_eq!(result.report.reason, "line_search_failed");
        assert!(
            result.report.steps > 1,
            "restoration retries must stay bounded"
        );
        assert_eq!(
            result.report.restoration_steps,
            RESTORATION_CAP * result.report.line_search_evaluations,
        );
        assert!(result.report.steps <= SETTLING_WORK_CAP);
    }

    #[test]
    fn pivot_cap_is_not_reported_as_completion() {
        let count = SETTLING_PIVOT_CAP + 1;
        let identity = linear::unconstrained(count).null;
        let result = optimize(&vec![0.0; count], |z| {
            Some(ConstraintState {
                inequalities: Rows {
                    values: z.to_vec(),
                    jacobian: identity.clone(),
                },
                preferences: Rows {
                    values: z.iter().map(|x| x - 1.0).collect(),
                    jacobian: identity.clone(),
                },
                ..ConstraintState::default()
            })
        });
        assert!(!result.report.complete);
        assert_eq!(result.report.reason, "step_cap");
        assert_eq!(result.report.steps, SETTLING_PIVOT_CAP);
        assert_eq!(result.z, vec![0.0; count]);
        assert_eq!(result.report.active_inequalities, [count - 1]);
    }

    #[test]
    fn separate_corner_faces_converge_and_remain_feasible() {
        let result = optimize(&[-1.0e-7, 1.0e-7], |z| {
            Some(ConstraintState {
                inequalities: rows(&[z[1] - z[0], z[1] + z[0]], &[&[-1.0, 1.0], &[1.0, 1.0]]),
                preferences: rows(&[z[0] - 1.0, z[1] + 2.0], &[&[1.0, 0.0], &[0.0, 1.0]]),
                ..ConstraintState::default()
            })
        });
        assert!(result.report.complete, "{:?}", result.report);
        assert!(result.z[1] - result.z[0].abs() > -SOLVE_EPSILON);
        assert!(infinity_norm(&result.z) < 1.0e-12);
    }

    #[test]
    fn duplicate_boundaries_do_not_consume_the_work_budget() {
        for equality in [false, true] {
            let result = optimize(&[0.0], |z| {
                Some(ConstraintState {
                    equalities: if equality {
                        rows(&[z[0]], &[&[1.0]])
                    } else {
                        Rows::default()
                    },
                    inequalities: Rows {
                        values: vec![z[0]; 64],
                        jacobian: vec![vec![1.0]; 64],
                    },
                    preferences: rows(&[z[0] - 1.0], &[&[1.0]]),
                })
            });
            assert!(result.report.complete, "{:?}", result.report);
            assert!((result.z[0] - if equality { 0.0 } else { 1.0 }).abs() < SOLVE_EPSILON);
            assert!(result.report.steps < 5);
        }
    }

    #[test]
    fn dependent_cone_has_a_sign_valid_stationarity_certificate() {
        for order in [[0, 1, 2], [2, 1, 0], [1, 2, 0]] {
            for scale in [0.01, 1.0, 100.0] {
                let result = optimize(&[0.0, 0.0], |z| {
                    let values = [z[0], z[1], scale * (z[1] - z[0])];
                    let jacobian = [vec![1.0, 0.0], vec![0.0, 1.0], vec![-scale, scale]];
                    Some(ConstraintState {
                        inequalities: Rows {
                            values: order.iter().map(|i| values[*i]).collect(),
                            jacobian: order.iter().map(|i| jacobian[*i].clone()).collect(),
                        },
                        preferences: rows(&[z[0] + 1.0, z[1]], &[&[1.0, 0.0], &[0.0, 1.0]]),
                        ..ConstraintState::default()
                    })
                });
                assert!(result.report.complete, "{:?}", result.report);
                assert_eq!(result.z, [0.0, 0.0]);
                assert!(result.report.steps < 10, "{:?}", result.report);
            }
        }
    }

    #[test]
    fn redundant_oblique_boundaries_do_not_block_tangent_roundoff() {
        for scale in [0.1, 1.0, 3.0, 100.0] {
            let result = optimize(&[0.0, 0.0, 0.0], |z| {
                let value = z[0] + 2.0 * z[1] + 3.0 * z[2];
                Some(ConstraintState {
                    equalities: rows(&[value], &[&[1.0, 2.0, 3.0]]),
                    inequalities: rows(&[scale * value], &[&[scale, 2.0 * scale, 3.0 * scale]]),
                    preferences: rows(
                        &[z[0] - 1.0, z[1] - 2.0, z[2] + 2.0],
                        &[&[1.0, 0.0, 0.0], &[0.0, 1.0, 0.0], &[0.0, 0.0, 1.0]],
                    ),
                })
            });
            assert!(result.report.complete, "{:?}", result.report);
            assert!(scale * (result.z[0] + 2.0 * result.z[1] + 3.0 * result.z[2]) > -SOLVE_EPSILON);
            assert!(result.report.steps < 5);
        }
    }

    #[test]
    fn curvature_recovery_certifies_a_nonzero_residual_optimum() {
        let result = optimize(&[0.0], |z| {
            Some(ConstraintState {
                preferences: rows(&[z[0] - 1.0, z[0] * z[0]], &[&[1.0], &[2.0 * z[0]]]),
                ..ConstraintState::default()
            })
        });
        assert!(result.report.complete, "{:?}", result.report);
        assert_eq!(result.report.model_rebuilds, 1);
        let x = result.z[0];
        assert!((x + 2.0 * x * x * x - 1.0).abs() < SETTLING_TOLERANCE);
        assert!(merit(&[result.z[0] - 1.0, result.z[0] * result.z[0]]) < 0.5);
    }

    #[test]
    fn convergence_requires_sign_valid_multipliers_of_the_current_gradient() {
        let result = optimize(&[0.0, 0.0], |z| {
            Some(ConstraintState {
                inequalities: rows(&[z[0]], &[&[1.0, 0.0]]),
                preferences: rows(&[1.0 - z[0] + 2e-10 * z[1]], &[&[-1.0, 2e-10]]),
                ..ConstraintState::default()
            })
        });
        assert!(result.report.complete, "{:?}", result.report);
        assert!(
            result.z[0] > 0.9,
            "the current normal gradient requires releasing x"
        );
        assert_eq!(result.report.accepted_moves, 1);
    }

    #[test]
    fn a_near_boundary_interior_point_is_not_falsely_stationary() {
        let result = optimize(&[0.5 * ACTIVE_TOLERANCE], |z| {
            Some(one_dimension(z[0], -1.0))
        });
        assert!(result.report.complete, "{:?}", result.report);
        assert!(result.z[0].abs() < SOLVE_EPSILON);
        assert_eq!(result.report.accepted_moves, 1);
    }

    #[test]
    fn inactive_feasible_faces_do_not_pin_restoration_at_their_boundary() {
        let mut report = initial_result(&[0.0]).report;
        let restored = restore(
            vec![0.0],
            &[],
            &mut |z| {
                Some(ConstraintState {
                    equalities: rows(&[z[0] - 1.0], &[&[1.0]]),
                    inequalities: rows(&[1e12 * z[0]], &[&[1e12]]),
                    ..ConstraintState::default()
                })
            },
            &mut report,
        );
        assert!(
            restored.is_some(),
            "an inactive, feasible inequality must allow inward restoration"
        );
        if let Some((z, state)) = restored {
            assert!((z[0] - 1.0).abs() < SOLVE_EPSILON);
            assert!(feasible(&state));
        }
    }

    #[test]
    fn restoration_shortens_an_overshooting_correction_before_retrying_damping() {
        for reject_outside_domain in [false, true] {
            let mut report = initial_result(&[0.1]).report;
            let (restored, trace) = crate::trace::collect(|| {
                restore(
                    vec![0.1],
                    &[],
                    &mut |z| {
                        if reject_outside_domain && z[0] > 2.0 {
                            return None;
                        }
                        Some(ConstraintState {
                            equalities: rows(&[z[0] * z[0] - 1.0], &[&[2.0 * z[0]]]),
                            inequalities: rows(&[z[0], 2.0 - z[0]], &[&[1.0], &[-1.0]]),
                            ..ConstraintState::default()
                        })
                    },
                    &mut report,
                )
            });
            let Some((z, state)) = restored else {
                unreachable!("shortened corrections must restore the bounded positive root");
            };
            assert!((z[0] - 1.0).abs() < SOLVE_EPSILON);
            assert!(feasible(&state));
            assert!(report.restoration_steps < RESTORATION_CAP);
            assert!(trace.iter().any(|step| {
                step.name == "settling_restoration_trial"
                    && step.data["accepted"] == true
                    && step.data["alpha"] == 0.25
            }));
        }
    }

    #[test]
    fn a_redundant_boundary_does_not_erase_descent_on_a_curved_equality() {
        let result = optimize(&[1.0, 0.0], |z| {
            Some(ConstraintState {
                equalities: rows(&[z[0] - z[1] * z[1] - 1.0], &[&[1.0, -2.0 * z[1]]]),
                inequalities: rows(&[1e12 * (z[0] - 1.0)], &[&[1e12, 0.0]]),
                preferences: rows(&[z[0] - 2.0, z[1] - 1.0], &[&[1.0, 0.0], &[0.0, 1.0]]),
            })
        });
        assert!(result.report.complete, "{:?}", result.report);
        assert!((result.z[0] - 2.0).abs() < 1e-8);
        assert!((result.z[1] - 1.0).abs() < 1e-8);
        assert!((result.z[0] - result.z[1] * result.z[1] - 1.0).abs() < SOLVE_EPSILON);
        assert!(result.z[0] >= 1.0);
        assert!(result.report.restoration_steps > 0);
    }
    #[test]
    fn nearly_parallel_equalities_do_not_create_spurious_boundary_pivots() {
        for difference in [1.0e-6, 1.0e-8] {
            let result = optimize(&[0.0, 0.0, 0.0], |z| {
                Some(ConstraintState {
                    equalities: rows(
                        &[z[0] + z[1], z[0] + (1.0 + difference) * z[1]],
                        &[&[1.0, 1.0, 0.0], &[1.0, 1.0 + difference, 0.0]],
                    ),
                    inequalities: rows(&[z[0]], &[&[1.0, 0.0, 0.0]]),
                    preferences: rows(
                        &[z[0] + 1.0, z[1] - 1.0, z[2] - 2.0],
                        &[&[1.0, 0.0, 0.0], &[0.0, 1.0, 0.0], &[0.0, 0.0, 1.0]],
                    ),
                })
            });
            assert!(result.report.complete, "{:?}", result.report);
            assert!(result.z[0].abs() < SOLVE_EPSILON);
            assert!(result.z[1].abs() < SOLVE_EPSILON);
            assert!((result.z[2] - 2.0).abs() < SOLVE_EPSILON);
            assert!(result.report.working_set_pivots < 5);
        }
    }
    #[test]
    fn accepted_move_cap_is_not_reported_as_completion() {
        let result = optimize(&[0.0], |z| {
            Some(ConstraintState {
                preferences: rows(&[z[0] - 10_000.0], &[&[1.0]]),
                ..ConstraintState::default()
            })
        });
        assert!(!result.report.complete);
        assert_eq!(result.report.reason, "step_cap");
        assert_eq!(result.report.accepted_moves, SETTLING_STEP_CAP);
        assert!(result.report.steps <= SETTLING_WORK_CAP);
        assert!(result.report.projected_gradient > 1.0);
        assert!(result.z[0] > 0.0 && result.z[0] < 10_000.0);
    }
}
