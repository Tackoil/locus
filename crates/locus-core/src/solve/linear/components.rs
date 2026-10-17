//! Exact structural column components; disjoint supports never rotate together.

pub(super) fn column_pairs(matrix: &[Vec<f64>], columns: usize) -> Vec<(usize, usize)> {
    let mut roots: Vec<_> = (0..columns).collect();
    let mut nonzero = vec![false; columns];
    for row in matrix {
        join_row(row, &mut roots, &mut nonzero);
    }
    for column in 0..columns {
        roots[column] = root(&roots, column);
    }
    let mut pairs = Vec::new();
    for first in 0..columns {
        if nonzero[first] {
            pairs.extend(
                ((first + 1)..columns)
                    .filter(|second| roots[*second] == roots[first])
                    .map(|second| (first, second)),
            );
        }
    }
    pairs
}

fn join_row(row: &[f64], roots: &mut [usize], nonzero: &mut [bool]) {
    let mut first = None;
    for (column, value) in row.iter().enumerate() {
        if *value == 0.0 {
            continue;
        }
        nonzero[column] = true;
        let current = root(roots, column);
        let previous = *first.get_or_insert(current);
        let previous = root(roots, previous);
        roots[current.max(previous)] = current.min(previous);
    }
}

fn root(roots: &[usize], mut column: usize) -> usize {
    while roots[column] != column {
        column = roots[column];
    }
    column
}
