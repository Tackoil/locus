//! Deterministic dense linear algebra for the normative solver.

use crate::tol::{JACOBI_SKIP_FACTOR, RANK_FACTOR};

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
    let columns = jacobian.first().map_or(0, Vec::len);
    if columns == 0 {
        return Ok(Analysis {
            rank: 0,
            null: Vec::new(),
        });
    }
    let (rotated, vectors, singulars) = jacobi(jacobian, columns)?;
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

pub(crate) fn rank(matrix: &[Vec<f64>]) -> Result<usize, ()> {
    analyze(matrix).map(|analysis| analysis.rank)
}

pub(crate) fn lm_primary_step(
    jacobian: &[Vec<f64>],
    residuals: &[f64],
    damping: f64,
) -> Option<Vec<f64>> {
    let count = jacobian.first().map_or(0, Vec::len);
    let mut normal = vec![vec![0.0; count]; count];
    let mut rhs = vec![0.0; count];
    for row in 0..jacobian.len() {
        for column in 0..count {
            rhs[column] -= jacobian[row][column] * residuals[row];
            for other in 0..count {
                normal[column][other] += jacobian[row][column] * jacobian[row][other];
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

fn jacobi(input: &[Vec<f64>], columns: usize) -> Result<JacobiResult, ()> {
    let mut matrix = input.to_vec();
    let mut vectors = identity(columns);
    for _ in 0..64 {
        let mut rotated = false;
        for first in 0..columns {
            for second in (first + 1)..columns {
                rotated |= rotate_pair(&mut matrix, &mut vectors, first, second);
            }
        }
        if !rotated {
            let singulars = column_norms(&matrix, columns);
            return Ok((matrix, vectors, singulars));
        }
    }
    Err(())
}

fn rotate_pair(
    matrix: &mut [Vec<f64>],
    vectors: &mut [Vec<f64>],
    first: usize,
    second: usize,
) -> bool {
    let alpha = column_dot(matrix, first, first);
    let beta = column_dot(matrix, second, second);
    let gamma = column_dot(matrix, first, second);
    if alpha == 0.0 || beta == 0.0 || gamma.abs() <= JACOBI_SKIP_FACTOR * libm::sqrt(alpha * beta) {
        return false;
    }
    let zeta = (beta - alpha) / (2.0 * gamma);
    let t = jacobi_tangent(zeta);
    let cosine = 1.0 / libm::sqrt(1.0 + t * t);
    let sine = cosine * t;
    rotate_columns(matrix, first, second, cosine, sine);
    rotate_columns(vectors, first, second, cosine, sine);
    true
}

fn jacobi_tangent(zeta: f64) -> f64 {
    if zeta == 0.0 {
        return 1.0;
    }
    let absolute = zeta.abs();
    let square = zeta * zeta;
    let root = if square.is_infinite() && absolute.is_finite() {
        let inverse = 1.0 / absolute;
        absolute * libm::sqrt(1.0 + inverse * inverse)
    } else {
        libm::sqrt(1.0 + square)
    };
    zeta.signum() / (absolute + root)
}

fn rotate_columns(matrix: &mut [Vec<f64>], first: usize, second: usize, cosine: f64, sine: f64) {
    for row in matrix {
        let left = row[first];
        let right = row[second];
        row[first] = cosine * left - sine * right;
        row[second] = sine * left + cosine * right;
    }
}

fn column_dot(matrix: &[Vec<f64>], first: usize, second: usize) -> f64 {
    let mut result = 0.0;
    for row in matrix {
        result += row[first] * row[second];
    }
    result
}

fn column_norms(matrix: &[Vec<f64>], columns: usize) -> Vec<f64> {
    (0..columns)
        .map(|column| libm::sqrt(column_dot(matrix, column, column)))
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
        let mut matrix = vec![vec![1.0, 1.0e-12], vec![0.0, 1.0e-12]];
        let mut vectors = identity(2);
        assert!(rotate_pair(&mut matrix, &mut vectors, 0, 1));
    }

    #[test]
    fn singular_order_is_descending_with_original_index_ties() {
        assert_eq!(
            descending_singular_order(&[1.0, 3.0, 3.0, 2.0]),
            [1, 2, 3, 0]
        );
    }
}
