//! Original row-major implementation, retained as a bitwise regression oracle.

use super::*;
use crate::tol::JACOBI_SKIP_FACTOR;

pub(super) fn jacobi(
    input: &[Vec<f64>],
    columns: usize,
    compute_vectors: bool,
) -> Result<JacobiResult, ()> {
    let mut matrix = input.to_vec();
    let mut vectors = if compute_vectors {
        identity(columns)
    } else {
        Vec::new()
    };
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
