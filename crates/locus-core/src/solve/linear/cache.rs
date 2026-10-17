//! Solve-local tangent reuse with rank and null-residual checks.

use super::{ordered_dot, tangent_space_certified, transpose};
use crate::tol::RANK_FACTOR;

pub(crate) type TangentSpace = (Vec<Vec<f64>>, Vec<Vec<f64>>);

struct Entry {
    matrix: Vec<Vec<f64>>,
    space: TangentSpace,
    minimum: f64,
}

#[derive(Default)]
pub(crate) struct TangentCache {
    entries: Vec<Entry>,
}

impl TangentCache {
    pub(crate) fn space(&mut self, matrix: &[Vec<f64>]) -> Result<TangentSpace, ()> {
        self.space_with_bound(matrix).map(|(space, _)| space)
    }

    pub(crate) fn space_with_bound(
        &mut self,
        matrix: &[Vec<f64>],
    ) -> Result<(TangentSpace, f64), ()> {
        if let Some(entry) = self.entries.iter().find(|entry| entry.accepts(matrix)) {
            return Ok((
                entry.space.clone(),
                entry.minimum - libm::sqrt(entry.difference(matrix)),
            ));
        }
        let (space, minimum) = tangent_space_certified(matrix)?;
        if self.entries.len() == 2 {
            self.entries.remove(0);
        }
        self.entries.push(Entry {
            matrix: matrix.to_vec(),
            space: space.clone(),
            minimum,
        });
        Ok((space, minimum))
    }

    pub(crate) fn projector(&mut self, matrix: &[Vec<f64>]) -> Result<Vec<Vec<f64>>, ()> {
        self.space(matrix).map(|(projector, _)| projector)
    }
}

impl Entry {
    fn difference(&self, matrix: &[Vec<f64>]) -> f64 {
        matrix
            .iter()
            .flatten()
            .zip(self.matrix.iter().flatten())
            .map(|(a, b)| (a - b) * (a - b))
            .sum()
    }
    fn accepts(&self, matrix: &[Vec<f64>]) -> bool {
        if matrix.len() != self.matrix.len()
            || matrix.first().map(Vec::len) != self.matrix.first().map(Vec::len)
        {
            return false;
        }
        if matrix == self.matrix {
            return true;
        }
        let difference = self.difference(matrix);
        let scale =
            libm::sqrt(matrix.iter().map(|row| ordered_dot(row, row)).sum::<f64>()).max(1.0);
        // Weyl's inequality prevents a retained singular direction disappearing.
        if libm::sqrt(difference) + RANK_FACTOR * scale >= self.minimum {
            return false;
        }
        let null = transpose(&self.space.1);
        let residual = matrix
            .iter()
            .flat_map(|row| {
                null.iter().map(move |column| {
                    let value = ordered_dot(row, column);
                    value * value
                })
            })
            .sum::<f64>();
        // A rank increase or a changed tangent space invalidates reuse.
        residual <= 1e-26
    }
}

#[cfg(test)]
mod tests {
    use super::TangentCache;

    #[test]
    fn unchanged_tangents_reuse_a_certified_basis() {
        let mut cache = TangentCache::default();
        let original = cache.space(&[vec![1.0, 0.0]]).ok();
        let scaled = cache.space(&[vec![1.001, 0.0]]).ok();
        assert_eq!(original, scaled);
        assert_eq!(cache.entries.len(), 1);
    }

    #[test]
    fn rank_loss_and_new_coupling_invalidate_cached_bases() {
        for changed in [
            vec![vec![0.0, 0.0], vec![0.0, 0.0]],
            vec![vec![1.0, 0.0], vec![0.0, 1e-6]],
        ] {
            let mut cache = TangentCache::default();
            assert!(cache.space(&[vec![1.0, 0.0], vec![0.0, 0.0]]).is_ok());
            assert!(cache.space(&changed).is_ok());
            assert_eq!(cache.entries.len(), 2);
        }
    }
}
