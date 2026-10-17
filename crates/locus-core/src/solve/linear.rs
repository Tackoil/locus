//! Deterministic dense linear algebra for the normative solver.

use crate::tol::RANK_FACTOR;

mod cache;
mod components;
pub(crate) use cache::TangentCache;
mod fast;
mod jacobi;
mod tangent;
use jacobi::decompose as jacobi;
#[cfg(test)]
mod jacobi_reference;

#[derive(Clone, Debug)]
pub(crate) struct Analysis {
    pub(crate) rank: usize,
    pub(crate) null: Vec<Vec<f64>>,
}

type JacobiResult = (Vec<Vec<f64>>, Vec<Vec<f64>>, Vec<f64>);

pub(crate) fn unconstrained(columns: usize) -> Analysis {
    Analysis {
        rank: 0,
        null: identity(columns),
    }
}

pub(crate) fn analyze(jacobian: &[Vec<f64>]) -> Result<Analysis, ()> {
    let _timing = crate::metrics::span("linear_analysis");
    let columns = jacobian.first().map_or(0, Vec::len);
    if columns == 0 {
        return Ok(Analysis {
            rank: 0,
            null: Vec::new(),
        });
    }
    let (rotated, vectors, singulars) = jacobi(jacobian, columns, true)?;
    let order = descending_singular_order(&singulars);
    let maximum = order.first().map_or(0.0, |index| singulars[*index]);
    let threshold = RANK_FACTOR * maximum.max(1.0);
    let rank = order
        .iter()
        .filter(|index| singulars[**index] > threshold)
        .count();
    let pseudoinverse = pseudoinverse(&rotated, &vectors, &singulars, &order, threshold);
    Ok(Analysis {
        rank,
        null: null_projector(&pseudoinverse, jacobian, columns),
    })
}

/// Orthogonal tangent projection for layout settling, without inverse-product cancellation.
/// The legacy geometric projector retains its established arithmetic order.
pub(crate) fn tangent_projector(jacobian: &[Vec<f64>]) -> Result<Vec<Vec<f64>>, ()> {
    tangent_space(jacobian).map(|(projector, _)| projector)
}

pub(crate) fn tangent_space(jacobian: &[Vec<f64>]) -> Result<cache::TangentSpace, ()> {
    tangent_space_certified(jacobian).map(|(space, _)| space)
}

pub(crate) fn working_tangent(
    equalities: &[Vec<f64>],
    working: &[Vec<f64>],
    columns: usize,
    cache: &mut TangentCache,
) -> Result<cache::TangentSpace, ()> {
    let ((_, basis), minimum) = if equalities.is_empty() {
        let identity = identity(columns);
        ((identity.clone(), identity), f64::INFINITY)
    } else {
        cache.space_with_bound(equalities)?
    };
    let active = &working[equalities.len()..];
    if let Some((basis, _)) = tangent::intersect(&basis, minimum, active, working) {
        return Ok((tangent::projector(&basis), basis));
    }
    cache.space(working)
}

fn tangent_space_certified(jacobian: &[Vec<f64>]) -> Result<(cache::TangentSpace, f64), ()> {
    let _timing = crate::metrics::span("tangent_projector");
    let columns = jacobian.first().map_or(0, Vec::len);
    if columns == 0 {
        return Ok(((Vec::new(), Vec::new()), f64::INFINITY));
    }
    if let Some((basis, minimum)) = fast::null_basis(jacobian) {
        return Ok(((tangent::projector(&basis), basis), minimum));
    }
    let _fallback = crate::metrics::span("tangent_svd_fallback");
    let (_, vectors, singulars) = jacobi(jacobian, columns, true)?;
    let order = descending_singular_order(&singulars);
    let maximum = order.first().map_or(0.0, |index| singulars[*index]);
    let threshold = RANK_FACTOR * maximum.max(1.0);
    let discarded: Vec<_> = order
        .iter()
        .copied()
        .filter(|index| singulars[*index] <= threshold)
        .collect();
    let basis = vectors
        .iter()
        .map(|row| discarded.iter().map(|index| row[*index]).collect())
        .collect();
    let mut result = vec![vec![0.0; columns]; columns];
    for component in order
        .into_iter()
        .filter(|index| singulars[*index] <= threshold)
    {
        for (row, values) in result.iter_mut().enumerate() {
            for (column, value) in values.iter_mut().enumerate() {
                *value += vectors[row][component] * vectors[column][component];
            }
        }
    }
    let minimum = singulars
        .iter()
        .copied()
        .filter(|value| *value > threshold)
        .fold(f64::INFINITY, f64::min);
    Ok(((result, basis), minimum))
}

pub(crate) fn rank(matrix: &[Vec<f64>]) -> Result<usize, ()> {
    let _timing = crate::metrics::span("linear_rank");
    let columns = matrix.first().map_or(0, Vec::len);
    if columns == 0 {
        return Ok(0);
    }
    if let Some(rank) = fast::rank(matrix) {
        return Ok(rank);
    }
    let (_, _, singulars) = jacobi(matrix, columns, false)?;
    let order = descending_singular_order(&singulars);
    let maximum = order.first().map_or(0.0, |index| singulars[*index]);
    let threshold = RANK_FACTOR * maximum.max(1.0);
    Ok(order
        .iter()
        .filter(|index| singulars[**index] > threshold)
        .count())
}

pub(crate) fn lm_primary_step(
    jacobian: &[Vec<f64>],
    residuals: &[f64],
    damping: f64,
) -> Option<Vec<f64>> {
    let _timing = crate::metrics::span("lm_primary_step");
    let count = jacobian.first().map_or(0, Vec::len);
    let mut normal = vec![vec![0.0; count]; count];
    let mut rhs = vec![0.0; count];
    for (row, residual) in jacobian.iter().zip(residuals) {
        let nonzero: Vec<_> = row
            .iter()
            .enumerate()
            .filter(|(_, value)| **value != 0.0)
            .collect();
        for &(column, value) in &nonzero {
            rhs[column] -= value * residual;
            for &(other, other_value) in &nonzero {
                normal[column][other] += value * other_value;
            }
        }
    }
    for (index, row) in normal.iter_mut().enumerate() {
        row[index] += damping;
    }
    cholesky_solve(&normal, &rhs)
}

pub(crate) fn multiply(matrix: &[Vec<f64>], vector: &[f64]) -> Vec<f64> {
    matrix.iter().map(|row| ordered_dot(row, vector)).collect()
}

pub(crate) fn restoration_step(
    matrix: &[Vec<f64>],
    residuals: &[f64],
    damping: f64,
) -> Option<Vec<f64>> {
    let _timing = crate::metrics::span("restoration_step");
    if damping > 1e-12 {
        return lm_primary_step(matrix, residuals, damping);
    }
    let rhs: Vec<_> = residuals.iter().map(|value| -value).collect();
    least_squares(matrix, &rhs)
        .ok()
        .or_else(|| lm_primary_step(matrix, residuals, damping))
}

/// Minimum-norm least-squares solution using the normative rank cutoff.
pub(crate) fn least_squares(matrix: &[Vec<f64>], rhs: &[f64]) -> Result<Vec<f64>, ()> {
    let _timing = crate::metrics::span("least_squares");
    let columns = matrix.first().map_or(0, Vec::len);
    if columns == 0 {
        return Ok(Vec::new());
    }
    if let Some(solution) = fast::solve(matrix, rhs) {
        return Ok(solution);
    }
    let _fallback = crate::metrics::span("least_squares_svd_fallback");
    let (rotated, vectors, singulars) = jacobi(matrix, columns, true)?;
    let order = descending_singular_order(&singulars);
    let maximum = order.first().map_or(0.0, |index| singulars[*index]);
    let threshold = RANK_FACTOR * maximum.max(1.0);
    let inverse = pseudoinverse(&rotated, &vectors, &singulars, &order, threshold);
    Ok(multiply(&inverse, rhs))
}

/// A trust-region preference step over the same retained singular subspace.
pub(crate) fn bounded_least_squares(
    matrix: &[Vec<f64>],
    rhs: &[f64],
    radius: f64,
) -> Result<Vec<f64>, ()> {
    let _timing = crate::metrics::span("bounded_least_squares");
    let columns = matrix.first().map_or(0, Vec::len);
    if columns == 0 {
        return Ok(Vec::new());
    }
    let (rotated, vectors, singulars) = jacobi(matrix, columns, true)?;
    let order = descending_singular_order(&singulars);
    let maximum = order.first().map_or(0.0, |index| singulars[*index]);
    let threshold = RANK_FACTOR * maximum.max(1.0);
    let inverse = pseudoinverse(&rotated, &vectors, &singulars, &order, threshold);
    let step = multiply(&inverse, rhs);
    if ordered_dot(&step, &step) <= radius * radius {
        return Ok(step);
    }
    let projected_rhs = multiply(&transpose(&rotated), rhs);
    let modes = order
        .into_iter()
        .filter(|index| singulars[*index] > threshold)
        .map(|index| {
            (
                index,
                singulars[index] * singulars[index],
                projected_rhs[index],
            )
        })
        .collect::<Vec<_>>();
    trust_step(&vectors, &modes, radius, maximum * maximum)
}

fn trust_step(
    vectors: &[Vec<f64>],
    modes: &[(usize, f64, f64)],
    radius: f64,
    maximum_square: f64,
) -> Result<Vec<f64>, ()> {
    let mut damping = maximum_square * crate::preset::SETTLING_TRUST_DAMPING;
    let mut lower = 0.0;
    let mut upper = None;
    let mut accepted = None;
    for _ in 0..crate::preset::SETTLING_TRUST_TRIALS {
        let step = regularized_step(vectors, modes, damping);
        let norm_square = ordered_dot(&step, &step);
        if norm_square <= radius * radius {
            if norm_square >= radius * radius * (1.0 - crate::preset::SETTLING_TRUST_TOLERANCE) {
                return Ok(step);
            }
            accepted = Some(step);
            upper = Some(damping);
        } else {
            lower = damping;
        }
        damping = upper.map_or(damping * 10.0, |bound| lower.midpoint(bound));
    }
    accepted.ok_or(())
}

fn regularized_step(vectors: &[Vec<f64>], modes: &[(usize, f64, f64)], damping: f64) -> Vec<f64> {
    let mut step = vec![0.0; vectors.len()];
    for (component, square, projection) in modes {
        let weight = projection / (square + damping);
        for (row, value) in step.iter_mut().enumerate() {
            *value += vectors[row][*component] * weight;
        }
    }
    step
}

pub(crate) fn transpose(matrix: &[Vec<f64>]) -> Vec<Vec<f64>> {
    let columns = matrix.first().map_or(0, Vec::len);
    (0..columns)
        .map(|column| matrix.iter().map(|row| row[column]).collect())
        .collect()
}

fn pseudoinverse(
    rotated: &[Vec<f64>],
    vectors: &[Vec<f64>],
    singulars: &[f64],
    order: &[usize],
    threshold: f64,
) -> Vec<Vec<f64>> {
    let rows = rotated.len();
    let columns = vectors.len();
    let mut result = vec![vec![0.0; rows]; columns];
    for component in order {
        if singulars[*component] <= threshold {
            continue;
        }
        let inverse_square = 1.0 / (singulars[*component] * singulars[*component]);
        for row in 0..columns {
            for (column, rotated_row) in rotated.iter().enumerate().take(rows) {
                result[row][column] +=
                    vectors[row][*component] * rotated_row[*component] * inverse_square;
            }
        }
    }
    result
}

fn descending_singular_order(singulars: &[f64]) -> Vec<usize> {
    let mut order = (0..singulars.len()).collect::<Vec<_>>();
    order.sort_by(|lhs, rhs| {
        singulars[*rhs]
            .total_cmp(&singulars[*lhs])
            .then_with(|| lhs.cmp(rhs))
    });
    order
}

fn null_projector(
    pseudoinverse: &[Vec<f64>],
    jacobian: &[Vec<f64>],
    columns: usize,
) -> Vec<Vec<f64>> {
    let mut result = identity(columns);
    for row in 0..columns {
        for column in 0..columns {
            let mut product = 0.0;
            for (equation, jacobian_row) in jacobian.iter().enumerate() {
                product += pseudoinverse[row][equation] * jacobian_row[column];
            }
            result[row][column] -= product;
        }
    }
    result
}

fn cholesky_solve(matrix: &[Vec<f64>], rhs: &[f64]) -> Option<Vec<f64>> {
    let lower = cholesky_factor(matrix)?;
    let forward = forward_substitute(&lower, rhs);
    Some(back_substitute(&lower, &forward))
}

fn cholesky_factor(matrix: &[Vec<f64>]) -> Option<Vec<Vec<f64>>> {
    let count = matrix.len();
    let mut lower = vec![vec![0.0; count]; count];
    for row in 0..count {
        for column in 0..=row {
            let mut value = matrix[row][column];
            for inner in 0..column {
                value -= lower[row][inner] * lower[column][inner];
            }
            if row == column {
                if value <= 0.0 || !value.is_finite() {
                    return None;
                }
                lower[row][column] = libm::sqrt(value);
            } else {
                lower[row][column] = value / lower[column][column];
            }
        }
    }
    Some(lower)
}

fn forward_substitute(lower: &[Vec<f64>], rhs: &[f64]) -> Vec<f64> {
    let count = lower.len();
    let mut forward = vec![0.0; count];
    for row in 0..count {
        let mut value = rhs[row];
        for (column, forward_value) in forward.iter().enumerate().take(row) {
            value -= lower[row][column] * forward_value;
        }
        forward[row] = value / lower[row][row];
    }
    forward
}

fn back_substitute(lower: &[Vec<f64>], forward: &[f64]) -> Vec<f64> {
    let count = lower.len();
    let mut result = vec![0.0; count];
    for row in (0..count).rev() {
        let mut value = forward[row];
        for column in (row + 1)..count {
            value -= lower[column][row] * result[column];
        }
        result[row] = value / lower[row][row];
    }
    result
}

fn ordered_dot(lhs: &[f64], rhs: &[f64]) -> f64 {
    let mut result = 0.0;
    for index in 0..lhs.len() {
        result += lhs[index] * rhs[index];
    }
    result
}

fn identity(size: usize) -> Vec<Vec<f64>> {
    let mut result = vec![vec![0.0; size]; size];
    for (index, row) in result.iter_mut().enumerate() {
        row[index] = 1.0;
    }
    result
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn rejected_restoration_corrections_respect_increased_damping() {
        let step = restoration_step(&[vec![1.0]], &[1.0], 1.0).expect("damped correction");
        assert!((step[0] + 0.5).abs() < 1e-12);
    }

    #[test]
    fn rank_and_null_space_match_a_redundant_system() {
        let matrix = vec![vec![1.0, 0.0, 1.0], vec![2.0, 0.0, 2.0]];
        let analysis = analyze(&matrix).expect("Jacobi converges");
        assert_eq!(analysis.rank, 1);
        let projected = multiply(&analysis.null, &[1.0, 2.0, -1.0]);
        assert!((projected[0] - 1.0).abs() < 1.0e-10);
        assert!((projected[1] - 2.0).abs() < 1.0e-10);
        assert!((projected[2] + 1.0).abs() < 1.0e-10);
    }

    #[test]
    fn ordered_cholesky_solves_positive_definite_system() {
        let result = cholesky_solve(&[vec![4.0, 2.0], vec![2.0, 3.0]], &[6.0, 5.0])
            .expect("positive definite");
        assert!((result[0] - 1.0).abs() < 1.0e-12);
        assert!((result[1] - 1.0).abs() < 1.0e-12);
    }

    #[test]
    fn jacobi_rotates_nonzero_columns_below_the_rank_threshold() {
        let matrix = vec![vec![1.0, 1.0e-12], vec![0.0, 1.0e-12]];
        let (_, vectors, _) = jacobi(&matrix, 2, true).expect("Jacobi converges");
        assert_ne!(vectors, identity(2));
    }

    #[test]
    fn rank_only_jacobi_retains_exact_singular_value_bits() {
        for matrix in [
            vec![vec![1.0, 1.0e-12, -2.0, 0.0], vec![0.0, 1.0e-12, 3.0, 0.0]],
            vec![vec![1.0, 2.0, 3.0], vec![2.0, 4.0, 6.0]],
        ] {
            let columns = matrix[0].len();
            let (_, _, full) = jacobi(&matrix, columns, true).expect("full Jacobi");
            let (_, _, minimal) = jacobi(&matrix, columns, false).expect("rank Jacobi");
            assert_eq!(
                full.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
                minimal.iter().map(|v| v.to_bits()).collect::<Vec<_>>()
            );
            assert_eq!(
                rank(&matrix).expect("rank"),
                analyze(&matrix).expect("analysis").rank
            );
        }
    }

    #[test]
    fn singular_order_is_descending_with_original_index_ties() {
        assert_eq!(
            descending_singular_order(&[1.0, 3.0, 3.0, 2.0]),
            [1, 2, 3, 0]
        );
    }
    #[test]
    fn tangent_projection_has_no_motion_normal_to_nearly_parallel_equalities() {
        let matrix = vec![
            vec![1.0, 2.0, 3.0],
            vec![1.0 + 1.0e-8, 2.0 - 1.0e-8, 3.0 + 2.0e-8],
        ];
        let null = tangent_projector(&matrix).expect("tangent projector");
        let projected = multiply(&null, &[1.0, -1.0, 1.0]);
        for residual in multiply(&matrix, &projected) {
            assert!(
                residual.abs() < 1e-12,
                "normal leakage: {residual}; {projected:?}"
            );
        }
        let repeated = multiply(&null, &projected);
        for (first, second) in projected.iter().zip(repeated) {
            assert!(
                (first - second).abs() < 1e-12,
                "projection must be idempotent"
            );
        }
        assert!(ordered_dot(&projected, &projected) > 0.1);
    }
    #[test]
    fn bounded_preference_step_preserves_progress_in_strong_directions() {
        let step = bounded_least_squares(&[vec![1.0, 0.0], vec![0.0, 1e-8]], &[1.0, 1.0], 1.0)
            .expect("bounded preference step");
        assert!(ordered_dot(&step, &step) <= 1.0);
        assert!(step[0] > 0.99, "strong direction lost progress: {step:?}");
        assert!(
            step[1].abs() < 0.02,
            "weak direction dominated the step: {step:?}"
        );
    }
    #[test]
    fn bounded_step_reaches_the_trust_boundary_without_excessive_damping() {
        let step = bounded_least_squares(&[vec![1.0, 0.0], vec![0.0, 1e-8]], &[1.0, 1.0], 8.0)
            .expect("accurate trust step");
        let norm_square = ordered_dot(&step, &step);
        assert!(norm_square <= 64.0);
        assert!(
            norm_square > 64.0 * (1.0 - 1e-6),
            "excessive damping: {step:?}"
        );
        assert!(step[0] > 0.99);
        assert!(step[1] > 7.9);
    }
}
