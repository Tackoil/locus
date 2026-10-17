//! Rank-certified orthogonal bases for well-conditioned working sets.

use super::{ordered_dot, transpose};
use crate::tol::RANK_FACTOR;

#[cfg(test)]
pub(super) fn basis(matrix: &[Vec<f64>]) -> Option<Vec<Vec<f64>>> {
    basis_with_bound(matrix).map(|(basis, _)| basis)
}

#[cfg(test)]
pub(super) fn basis_with_bound(matrix: &[Vec<f64>]) -> Option<(Vec<Vec<f64>>, f64)> {
    let columns = matrix.first()?.len();
    let (normals, condition) = row_basis(matrix)?;
    let scale = libm::sqrt(matrix.iter().map(|row| ordered_dot(row, row)).sum::<f64>()).max(1.0);
    Some((complement(&normals, columns)?, scale / condition))
}

pub(super) fn row_basis(matrix: &[Vec<f64>]) -> Option<(Vec<Vec<f64>>, f64)> {
    let scale = libm::sqrt(matrix.iter().map(|row| ordered_dot(row, row)).sum::<f64>()).max(1.0);
    if !scale.is_finite() {
        return None;
    }
    let mut normals = Vec::new();
    let mut triangular = Vec::new();
    let mut discarded_square = 0.0;
    for row in matrix {
        let mut residual = row.clone();
        let mut coefficients = remove(&mut residual, &normals);
        let norm = libm::sqrt(ordered_dot(&residual, &residual));
        if norm <= RANK_FACTOR {
            discarded_square += norm * norm;
            continue;
        }
        coefficients.push(norm);
        triangular.push(coefficients);
        normals.push(residual.into_iter().map(|value| value / norm).collect());
    }
    let error = libm::sqrt(discarded_square);
    // sigma_min(J) >= sigma_min(selected rows) >= 1 / ||L^-1||_F.
    // Weyl's bound accounts for the roundoff-sized discarded residuals.
    let minimum = minimum_bound(&triangular);
    if error > RANK_FACTOR || minimum <= RANK_FACTOR * scale + error {
        return None;
    }
    Some((normals, scale / minimum))
}

pub(super) fn minimum_bound(lower: &[Vec<f64>]) -> f64 {
    let mut square = 0.0;
    for column in 0..lower.len() {
        let mut inverse = vec![0.0; lower.len()];
        for row in column..lower.len() {
            let rhs = f64::from(row == column);
            inverse[row] =
                (rhs - ordered_dot(&lower[row][..row], &inverse[..row])) / lower[row][row];
            square += inverse[row] * inverse[row];
        }
    }
    1.0 / libm::sqrt(square)
}

fn remove(vector: &mut [f64], basis: &[Vec<f64>]) -> Vec<f64> {
    let mut coefficients = vec![0.0; basis.len()];
    for _ in 0..2 {
        for (index, row) in basis.iter().enumerate() {
            let projection = ordered_dot(vector, row);
            coefficients[index] += projection;
            for (value, component) in vector.iter_mut().zip(row) {
                *value -= projection * component;
            }
        }
    }
    coefficients
}

fn complement(normals: &[Vec<f64>], columns: usize) -> Option<Vec<Vec<f64>>> {
    let count = columns.checked_sub(normals.len())?;
    if count == 0 {
        return Some(vec![Vec::new(); columns]);
    }
    let mut reflectors: Vec<Vec<f64>> = Vec::new();
    for (offset, normal) in normals.iter().enumerate() {
        let mut vector = normal.clone();
        for (start, reflector) in reflectors.iter().enumerate() {
            reflect(&mut vector[start..], reflector);
        }
        let mut vector = vector[offset..].to_vec();
        let norm = libm::sqrt(ordered_dot(&vector, &vector));
        if norm < 1e-7 {
            return None;
        }
        vector[0] += norm.copysign(vector[0]);
        let norm = libm::sqrt(ordered_dot(&vector, &vector));
        vector.iter_mut().for_each(|value| *value /= norm);
        reflectors.push(vector);
    }
    let mut null = Vec::with_capacity(count);
    for column in normals.len()..columns {
        let mut vector = vec![0.0; columns];
        vector[column] = 1.0;
        for (start, reflector) in reflectors.iter().enumerate().rev() {
            reflect(&mut vector[start..], reflector);
        }
        null.push(vector);
    }
    Some(transpose(&null))
}

fn reflect(vector: &mut [f64], reflector: &[f64]) {
    let projection = 2.0 * ordered_dot(vector, reflector);
    for (value, axis) in vector.iter_mut().zip(reflector) {
        *value -= projection * axis;
    }
}

pub(super) fn projector(basis: &[Vec<f64>]) -> Vec<Vec<f64>> {
    basis
        .iter()
        .map(|a| basis.iter().map(|b| ordered_dot(a, b)).collect())
        .collect()
}

pub(super) fn intersect(
    basis: &[Vec<f64>],
    minimum: f64,
    active: &[Vec<f64>],
    working: &[Vec<f64>],
) -> Option<(Vec<Vec<f64>>, f64)> {
    if active.is_empty() {
        return Some((basis.to_vec(), minimum));
    }
    let transposed = transpose(basis);
    let projected: Vec<_> = active
        .iter()
        .map(|row| super::multiply(&transposed, row))
        .collect();
    let (normals, condition) = row_basis(&projected)?;
    if normals.len() != active.len() {
        return None;
    }
    let active_minimum = norm(&projected).max(1.0) / condition;
    let bound =
        1.0 / (1.0 / minimum + 1.0 / active_minimum + norm(active) / (minimum * active_minimum));
    if bound <= RANK_FACTOR * norm(working).max(1.0) {
        return None;
    }
    let reduced = complement(&normals, transposed.len())?;
    let reduced = transpose(&reduced);
    let result = basis
        .iter()
        .map(|row| super::multiply(&reduced, row))
        .collect();
    Some((result, bound))
}

fn norm(matrix: &[Vec<f64>]) -> f64 {
    libm::sqrt(matrix.iter().map(|row| ordered_dot(row, row)).sum::<f64>())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uncertain_rank_uses_the_svd_fallback() {
        assert!(basis(&[vec![1e11, 0.0], vec![0.0, 1.0]]).is_none());
    }

    #[test]
    fn certified_discarded_rows_keep_the_normative_rank_cutoff() {
        let Some(null) = basis(&[vec![1.0, 0.0], vec![0.0, 1e-11]]) else {
            unreachable!("certified rank-one matrix")
        };
        assert_eq!(null[0].len(), 1);
        assert!(null[0][0].abs() < 1e-12);
        assert!((null[1][0].abs() - 1.0).abs() < 1e-12);
    }

    #[test]
    fn certified_basis_is_tangent_and_orthonormal() {
        let matrix = vec![
            vec![1.0, 2.0, 3.0],
            vec![1.0 + 1e-8, 2.0 - 1e-8, 3.0 + 2e-8],
        ];
        let Some(basis) = basis(&matrix) else {
            unreachable!("well-conditioned retained rank")
        };
        let columns = transpose(&basis);
        assert_eq!(columns.len(), 1);
        assert!((ordered_dot(&columns[0], &columns[0]) - 1.0).abs() < 1e-12);
        for row in matrix {
            assert!(ordered_dot(&row, &columns[0]).abs() < 1e-12);
        }
    }
}
