//! Bounded curvature recovery on the current feasible boundary manifold.

use super::{
    ConstraintState, Direction, RANK_FACTOR, SETTLING_TOLERANCE, SettlingReport, add_step, dot,
    gradient, infinity_norm, linear, orthogonalize, restore, working_jacobian,
};
use crate::preset::{
    SETTLING_RECOVERY_DIFFERENCE, SETTLING_RECOVERY_DIMENSIONS, SETTLING_RECOVERY_GRADIENT,
};

pub(super) fn eligible(state: &ConstraintState, direction: &Direction) -> bool {
    let scale = infinity_norm(&state.preferences.values).max(1.0);
    direction.release.is_none()
        && direction.projected_gradient > SETTLING_TOLERANCE * scale
        && direction.projected_gradient <= SETTLING_RECOVERY_GRADIENT * scale
        && direction.slope < 0.0
        && infinity_norm(&direction.delta) > 0.0
}

pub(super) fn rebuild(
    z: &[f64],
    state: &ConstraintState,
    active: &[usize],
    original: &Direction,
    evaluate: &mut impl FnMut(&[f64]) -> Option<ConstraintState>,
    report: &mut SettlingReport,
) -> Option<Direction> {
    let basis = tangent_basis(&original.null);
    if basis.is_empty() || basis.len() > SETTLING_RECOVERY_DIMENSIONS {
        return None;
    }
    let columns = basis
        .iter()
        .map(|vector| {
            let plus = probe_gradient(
                z,
                vector,
                SETTLING_RECOVERY_DIFFERENCE,
                active,
                evaluate,
                report,
            )?;
            let minus = probe_gradient(
                z,
                vector,
                -SETTLING_RECOVERY_DIFFERENCE,
                active,
                evaluate,
                report,
            )?;
            let change: Vec<_> = plus
                .iter()
                .zip(minus)
                .map(|(a, b)| (a - b) / (2.0 * SETTLING_RECOVERY_DIFFERENCE))
                .collect();
            Some(basis.iter().map(|v| dot(v, &change)).collect::<Vec<_>>())
        })
        .collect::<Option<Vec<_>>>()?;
    let hessian = symmetric(&linear::transpose(&columns));
    let tangent_gradient = linear::multiply(&original.null, &gradient(&state.preferences, z.len()));
    let rhs: Vec<_> = basis.iter().map(|v| -dot(v, &tangent_gradient)).collect();
    let reduced = linear::least_squares(&hessian, &rhs).ok()?;
    let delta = linear::multiply(&linear::transpose(&basis), &reduced);
    let slope = dot(&tangent_gradient, &delta);
    if slope >= 0.0 || infinity_norm(&delta) > 8.0 * infinity_norm(&original.delta) {
        return None;
    }
    Some(Direction {
        delta,
        slope,
        projected_gradient: original.projected_gradient,
        release: None,
        null: original.null.clone(),
    })
}

fn tangent_basis(null: &[Vec<f64>]) -> Vec<Vec<f64>> {
    let mut result = Vec::new();
    for mut column in linear::transpose(null) {
        orthogonalize(&mut column, &result);
        let norm = libm::sqrt(dot(&column, &column));
        if norm > RANK_FACTOR {
            result.push(column.iter().map(|v| v / norm).collect());
        }
    }
    result
}

fn probe_gradient(
    z: &[f64],
    vector: &[f64],
    step: f64,
    active: &[usize],
    evaluate: &mut impl FnMut(&[f64]) -> Option<ConstraintState>,
    report: &mut SettlingReport,
) -> Option<Vec<f64>> {
    let trial = add_step(z, vector, step);
    let (restored, state) = restore(trial.clone(), active, evaluate, report)?;
    let correction: Vec<_> = restored.iter().zip(&trial).map(|(a, b)| a - b).collect();
    if libm::sqrt(dot(&correction, &correction)) > 0.1 * step.abs() {
        return None;
    }
    let working = working_jacobian(&state, active);
    let null = if working.is_empty() {
        linear::unconstrained(z.len()).null
    } else {
        linear::tangent_projector(&working).ok()?
    };
    Some(linear::multiply(
        &null,
        &gradient(&state.preferences, z.len()),
    ))
}

fn symmetric(matrix: &[Vec<f64>]) -> Vec<Vec<f64>> {
    matrix
        .iter()
        .enumerate()
        .map(|(i, row)| {
            row.iter()
                .enumerate()
                .map(|(j, value)| 0.5 * (value + matrix[j][i]))
                .collect()
        })
        .collect()
}
