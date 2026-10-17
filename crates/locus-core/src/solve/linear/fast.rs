//! SIMD-backed QR paths, guarded by the solver's singular-value rank cutoff.

use super::ordered_dot;
use crate::tol::RANK_FACTOR;
use faer::{
    Mat,
    linalg::solvers::{ColPivQr, SolveLstsq},
};

struct Certified {
    qr: ColPivQr<f64>,
    rank: usize,
    minimum: f64,
}

fn factor(matrix: &[Vec<f64>]) -> Option<Certified> {
    let columns = matrix.first()?.len();
    let scale = libm::sqrt(matrix.iter().map(|row| ordered_dot(row, row)).sum::<f64>()).max(1.0);
    if !scale.is_finite() {
        return None;
    }
    let transposed = Mat::from_fn(columns, matrix.len(), |row, column| matrix[column][row]);
    let qr = transposed.col_piv_qr();
    let r = qr.R();
    let rank = (0..columns.min(matrix.len()))
        .take_while(|index| r[(*index, *index)].abs() > RANK_FACTOR)
        .count();
    let mut discarded = 0.0;
    for row in rank..r.nrows() {
        for column in rank..r.ncols() {
            let value = r[(row, column)];
            discarded += value * value;
        }
    }
    let mut inverse = Mat::<f64>::zeros(rank, rank);
    faer::linalg::triangular_inverse::invert_upper_triangular(
        inverse.as_mut(),
        r.get(..rank, ..rank),
        faer::Par::Seq,
    );
    let minimum = 1.0 / inverse.norm_l2();
    let error = libm::sqrt(discarded);
    // The selected triangular block bounds every retained singular direction.
    // Its discarded block must be below the absolute cutoff even before scaling.
    if !error.is_finite()
        || minimum.is_nan()
        || error > RANK_FACTOR
        || minimum <= RANK_FACTOR * scale + error
    {
        return None;
    }
    Some(Certified { qr, rank, minimum })
}

pub(super) fn rank(matrix: &[Vec<f64>]) -> Option<usize> {
    factor(matrix).map(|factor| factor.rank)
}

pub(super) fn null_basis(matrix: &[Vec<f64>]) -> Option<(Vec<Vec<f64>>, f64)> {
    let factor = factor(matrix)?;
    let q = factor.qr.compute_Q();
    let basis = (0..q.nrows())
        .map(|row| {
            (factor.rank..q.ncols())
                .map(|column| q[(row, column)])
                .collect()
        })
        .collect();
    Some((basis, factor.minimum))
}

pub(super) fn solve(matrix: &[Vec<f64>], rhs: &[f64]) -> Option<Vec<f64>> {
    let factor = factor(matrix)?;
    let columns = matrix.first()?.len();
    if factor.rank == 0 {
        return Some(vec![0.0; columns]);
    }
    let q = factor.qr.compute_Q();
    let basis = q.as_ref().get(.., ..factor.rank);
    let a = Mat::from_fn(matrix.len(), columns, |row, column| matrix[row][column]);
    let reduced = &a * basis;
    let rhs = Mat::from_fn(rhs.len(), 1, |row, _| rhs[row]);
    let solution = reduced.qr().solve_lstsq(&rhs);
    let result = basis * solution;
    let values: Vec<_> = (0..columns).map(|row| result[(row, 0)]).collect();
    values
        .iter()
        .all(|value| value.is_finite())
        .then_some(values)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn certified_rank_matches_svd_for_rectangular_and_redundant_systems() {
        for matrix in [
            vec![vec![1.0, 0.0, 1.0], vec![0.0, 2.0, 0.0]],
            vec![vec![1.0, 2.0], vec![2.0, 4.0], vec![0.0, 1.0]],
        ] {
            let expected = super::super::analyze(&matrix)
                .ok()
                .map(|analysis| analysis.rank);
            assert_eq!(rank(&matrix), expected);
            let (basis, _) =
                null_basis(&matrix).unwrap_or_else(|| unreachable!("certified matrix"));
            for direction in super::super::transpose(&basis) {
                assert!((ordered_dot(&direction, &direction) - 1.0).abs() < 1e-12);
                assert!(
                    super::super::multiply(&matrix, &direction)
                        .iter()
                        .all(|value| value.abs() < 1e-12)
                );
            }
        }
    }

    #[test]
    fn uncertain_scaled_rank_requests_svd() {
        assert!(rank(&[vec![1e11, 0.0], vec![0.0, 1.0]]).is_none());
    }

    #[test]
    fn non_finite_qr_for_zero_rows_uses_the_finite_jacobi_basis() {
        let matrix = vec![vec![0.0; 3]; 2];
        assert!(factor(&matrix).is_none());
        assert_eq!(super::super::rank(&matrix), Ok(0));
        let expected = super::super::identity(3);
        assert_eq!(
            super::super::tangent_space(&matrix),
            Ok((expected.clone(), expected))
        );
    }

    #[test]
    fn redundant_columns_return_the_minimum_norm_least_squares_solution() {
        let matrix = [vec![1.0, 1.0], vec![1.0, 1.0]];
        let solution =
            solve(&matrix, &[1.0, 3.0]).unwrap_or_else(|| unreachable!("certified matrix"));
        assert!(solution.iter().all(|value| (value - 1.0).abs() < 1e-12));
    }
}
