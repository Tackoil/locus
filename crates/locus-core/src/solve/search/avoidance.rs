//! Per-pair forced escape choices, preserving choices for other pairs.
use std::collections::{BTreeMap, BTreeSet};

#[derive(Default)]
pub(super) struct Schedule {
    pub(super) current: BTreeMap<usize, usize>,
    rows: BTreeSet<usize>,
    visited: BTreeSet<Vec<(usize, usize)>>,
    lexicographic: Vec<usize>,
}

impl Schedule {
    pub(super) fn next(&mut self, failed: &[usize]) -> Option<BTreeMap<usize, usize>> {
        self.visited.insert(key(&self.current));
        for row in failed {
            self.rows.insert(*row);
        }
        let candidate = self.targeted(failed).or_else(|| self.fallback())?;
        self.current.clone_from(&candidate);
        self.visited.insert(key(&candidate));
        Some(candidate)
    }

    pub(super) fn new_route(&mut self) {
        self.visited.clear();
        self.lexicographic = vec![0; self.rows.len()];
    }

    fn targeted(&self, failed: &[usize]) -> Option<BTreeMap<usize, usize>> {
        let row = *failed.first()?;
        let start = self.current.get(&row).map_or(0, |choice| choice + 1);
        for direction in start..crate::preset::AVOIDANCE_DIRECTION_CAP {
            let mut candidate = self.current.clone();
            candidate.insert(row, direction);
            if !self.visited.contains(&key(&candidate)) {
                return Some(candidate);
            }
        }
        None
    }

    fn fallback(&mut self) -> Option<BTreeMap<usize, usize>> {
        if self.lexicographic.len() != self.rows.len() {
            self.lexicographic = vec![0; self.rows.len()];
        }
        while advance(&mut self.lexicographic) {
            let candidate = self
                .rows
                .iter()
                .zip(&self.lexicographic)
                .filter_map(|(row, choice)| {
                    choice.checked_sub(1).map(|direction| (*row, direction))
                })
                .collect();
            if !self.visited.contains(&key(&candidate)) {
                return Some(candidate);
            }
        }
        None
    }
}

fn key(choices: &BTreeMap<usize, usize>) -> Vec<(usize, usize)> {
    choices
        .iter()
        .map(|(row, choice)| (*row, *choice))
        .collect()
}

fn advance(indices: &mut [usize]) -> bool {
    for index in (0..indices.len()).rev() {
        indices[index] += 1;
        if indices[index] <= crate::preset::AVOIDANCE_DIRECTION_CAP {
            return true;
        }
        indices[index] = 0;
    }
    false
}
