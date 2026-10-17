//! Column-contiguous Jacobi sweeps with unchanged normative arithmetic order.

use super::{JacobiResult, identity, ordered_dot, transpose};
use crate::tol::JACOBI_SKIP_FACTOR;

pub(super) fn decompose(
    input: &[Vec<f64>],
    columns: usize,
    compute_vectors: bool,
) -> Result<JacobiResult, ()> {
    let pairs = super::components::column_pairs(input, columns);
    let mut matrix = transpose(input);
    let mut vectors = if compute_vectors {
        identity(columns)
    } else {
        Vec::new()
    };
    let mut squares: Vec<_> = matrix
        .iter()
        .map(|column| ordered_dot(column, column))
        .collect();
    for _ in 0..64 {
        if !sweep(&mut matrix, &mut vectors, &mut squares, &pairs) {
            let singulars = squares.into_iter().map(libm::sqrt).collect();
            return Ok((transpose(&matrix), transpose(&vectors), singulars));
        }
    }
    Err(())
}

fn sweep(
    matrix: &mut [Vec<f64>],
    vectors: &mut [Vec<f64>],
    squares: &mut [f64],
    pairs: &[(usize, usize)],
) -> bool {
    let mut rotated = false;
    for &(first, second) in pairs {
        rotated |= rotate_pair(matrix, vectors, squares, first, second);
    }
    rotated
}

fn rotate_pair(
    matrix: &mut [Vec<f64>],
    vectors: &mut [Vec<f64>],
    squares: &mut [f64],
    first: usize,
    second: usize,
) -> bool {
    let alpha = squares[first];
    let beta = squares[second];
    let gamma = ordered_dot(&matrix[first], &matrix[second]);
    if alpha == 0.0 || beta == 0.0 || gamma.abs() <= JACOBI_SKIP_FACTOR * libm::sqrt(alpha * beta) {
        return false;
    }
    let t = tangent((beta - alpha) / (2.0 * gamma));
    let cosine = 1.0 / libm::sqrt(1.0 + t * t);
    let sine = cosine * t;
    rotate(matrix, first, second, cosine, sine);
    if !vectors.is_empty() {
        rotate(vectors, first, second, cosine, sine);
    }
    // Recompute in ascending row order, exactly as the next reference pair
    // would. Norms of untouched columns stay valid without another traversal.
    squares[first] = ordered_dot(&matrix[first], &matrix[first]);
    squares[second] = ordered_dot(&matrix[second], &matrix[second]);
    true
}

fn tangent(zeta: f64) -> f64 {
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

fn rotate(matrix: &mut [Vec<f64>], first: usize, second: usize, cosine: f64, sine: f64) {
    let (left, right) = matrix.split_at_mut(second);
    for (a, b) in left[first].iter_mut().zip(&mut right[0]) {
        let old_a = *a;
        let old_b = *b;
        *a = cosine * old_a - sine * old_b;
        *b = sine * old_a + cosine * old_b;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn column_storage_preserves_reference_bits() {
        for (rows, columns) in [(1, 1), (2, 4), (7, 3), (12, 12), (40, 25)] {
            let matrix: Vec<Vec<_>> = (0..rows)
                .map(|row| {
                    (0..columns)
                        .map(|column| {
                            let value = f64::from(
                                u32::try_from((row * 13 + column * 7 + row * column) % 19)
                                    .unwrap_or_else(|_| unreachable!("modulo 19 fits u32")),
                            ) - 9.0;
                            if column % 3 == 0 {
                                value * 1e-12
                            } else {
                                value
                            }
                        })
                        .collect()
                })
                .collect();
            for vectors in [false, true] {
                let actual = decompose(&matrix, columns, vectors);
                let reference = super::super::jacobi_reference::jacobi(&matrix, columns, vectors);
                assert_eq!(actual.is_ok(), reference.is_ok());
                if let (Ok(actual), Ok(reference)) = (actual, reference) {
                    assert_eq!(bits(actual), bits(reference));
                }
            }
        }
    }

    fn bits((matrix, vectors, singulars): JacobiResult) -> Vec<u64> {
        matrix
            .into_iter()
            .chain(vectors)
            .flatten()
            .chain(singulars)
            .map(f64::to_bits)
            .collect()
    }
}
