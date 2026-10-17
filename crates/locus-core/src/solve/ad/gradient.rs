//! Sparse forward derivatives, expanded only when assembling solver rows.

use std::ops::{Index, IndexMut};

#[derive(Clone, Debug)]
pub(crate) struct Gradient {
    count: usize,
    entries: Vec<(usize, f64)>,
}

impl Gradient {
    pub(super) fn zeros(count: usize) -> Self {
        Self {
            count,
            entries: Vec::new(),
        }
    }

    pub(crate) fn len(&self) -> usize {
        self.count
    }

    pub(crate) fn dense(&self) -> Vec<f64> {
        let mut values = vec![0.0; self.count];
        for &(index, value) in &self.entries {
            values[index] = value;
        }
        values
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = &f64> {
        self.entries.iter().map(|(_, value)| value)
    }

    pub(crate) fn map(&self, transform: impl Fn(f64) -> f64) -> Self {
        // Preserve non-finite propagation even at implicit zero coordinates.
        if !transform(0.0).is_finite() {
            return self
                .dense()
                .into_iter()
                .map(transform)
                .collect::<Vec<_>>()
                .into();
        }
        Self {
            count: self.count,
            entries: self
                .entries
                .iter()
                .filter_map(|&(index, value)| {
                    let value = transform(value);
                    (value != 0.0).then_some((index, value))
                })
                .collect(),
        }
    }

    pub(super) fn combine(&self, rhs: &Self, a: f64, b: f64) -> Self {
        debug_assert_eq!(self.count, rhs.count);
        if !a.is_finite() || !b.is_finite() {
            return (0..self.count)
                .map(|i| self[i] * a + rhs[i] * b)
                .collect::<Vec<_>>()
                .into();
        }
        let mut entries = Vec::with_capacity(self.entries.len() + rhs.entries.len());
        let mut left = self.entries.iter().peekable();
        let mut right = rhs.entries.iter().peekable();
        while left.peek().is_some() || right.peek().is_some() {
            let index = left
                .peek()
                .map_or(usize::MAX, |entry| entry.0)
                .min(right.peek().map_or(usize::MAX, |entry| entry.0));
            let x = take(&mut left, index);
            let y = take(&mut right, index);
            let value = x * a + y * b;
            if value != 0.0 {
                entries.push((index, value));
            }
        }
        Self {
            count: self.count,
            entries,
        }
    }
}

fn take<'a>(
    entries: &mut std::iter::Peekable<impl Iterator<Item = &'a (usize, f64)>>,
    index: usize,
) -> f64 {
    if entries.peek().is_some_and(|entry| entry.0 == index) {
        entries.next().map_or(0.0, |entry| entry.1)
    } else {
        0.0
    }
}

impl From<Vec<f64>> for Gradient {
    fn from(values: Vec<f64>) -> Self {
        Self {
            count: values.len(),
            entries: values
                .into_iter()
                .enumerate()
                .filter(|(_, v)| *v != 0.0)
                .collect(),
        }
    }
}

impl Index<usize> for Gradient {
    type Output = f64;
    fn index(&self, index: usize) -> &Self::Output {
        assert!(index < self.count);
        match self.entries.binary_search_by_key(&index, |entry| entry.0) {
            Ok(position) => &self.entries[position].1,
            Err(_) => &0.0,
        }
    }
}

impl IndexMut<usize> for Gradient {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        assert!(index < self.count);
        let position = self
            .entries
            .binary_search_by_key(&index, |entry| entry.0)
            .unwrap_or_else(|position| {
                self.entries.insert(position, (index, 0.0));
                position
            });
        &mut self.entries[position].1
    }
}

#[cfg(test)]
mod tests {
    use super::Gradient;

    #[test]
    fn sparse_arithmetic_matches_dense_coordinates() {
        let a = vec![0.0, 2.0, 0.0, -4.0, 3.0];
        let b = vec![5.0, -2.0, 0.0, 2.0, 0.0];
        let expected: Vec<_> = a.iter().zip(&b).map(|(a, b)| a * 0.5 + b).collect();
        assert_eq!(
            Gradient::from(a).combine(&b.into(), 0.5, 1.0).dense(),
            expected
        );
    }

    #[test]
    fn implicit_zeros_preserve_non_finite_failures() {
        let zero = Gradient::zeros(3);
        assert!(zero.map(|x| x * f64::INFINITY).iter().all(|x| x.is_nan()));
        assert!(
            zero.combine(&zero, f64::INFINITY, 0.0)
                .iter()
                .all(|x| x.is_nan())
        );
    }
}
