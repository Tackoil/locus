//! Restrict Phase-1 null-space hint motion to authored geometric unknowns.

use super::{EvalError, SolveState, UnknownClass, UnknownSystem, linear};
use locus_syntax::Span;

pub(super) fn null(system: &UnknownSystem, state: &SolveState) -> Result<Vec<Vec<f64>>, EvalError> {
    geometric_null(system, &state.jacobian)
}

fn geometric_null(
    system: &UnknownSystem,
    jacobian: &[Vec<f64>],
) -> Result<Vec<Vec<f64>>, EvalError> {
    let columns = system.hints.len();
    let geometric = system
        .objects
        .iter()
        .filter(|object| object.class == UnknownClass::Geometric)
        .flat_map(|object| object.start..object.start + object.count)
        .collect::<Vec<_>>();
    let restricted = jacobian
        .iter()
        .map(|row| geometric.iter().map(|index| row[*index]).collect())
        .collect::<Vec<_>>();
    let analysis = if restricted.is_empty() {
        linear::unconstrained(geometric.len())
    } else {
        linear::analyze(&restricted).map_err(|()| {
            EvalError::new(
                "E403",
                Span::default(),
                "geometric hint SVD did not converge",
            )
        })?
    };
    let mut null = vec![vec![0.0; columns]; columns];
    for (row, full_row) in geometric.iter().enumerate() {
        for (column, full_column) in geometric.iter().enumerate() {
            null[*full_row][*full_column] = analysis.null[row][column];
        }
    }
    Ok(null)
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use crate::solve::{UnknownObject, ad::NumericDimension};
    use std::collections::BTreeMap;

    fn system(classes: &[UnknownClass]) -> UnknownSystem {
        let objects = classes
            .iter()
            .enumerate()
            .map(|(index, class)| UnknownObject {
                class: *class,
                id: format!("component_{index}"),
                span: Span::default(),
                start: index,
                count: 1,
                dimension: NumericDimension::Length,
                report_hint: false,
            })
            .collect();
        UnknownSystem {
            objects,
            hints: vec![0.0; classes.len()],
            normalized_hints: vec![0.0; classes.len()],
            scales: vec![1.0; classes.len()],
            dimensions: BTreeMap::new(),
            diameter: 1.0,
            avoidance_axes: BTreeMap::new(),
        }
    }

    #[test]
    fn layout_hint_motion_is_zero_without_primary_constraints() {
        let system = system(&[
            UnknownClass::Geometric,
            UnknownClass::AutomaticDimension,
            UnknownClass::LabelPlacement,
            UnknownClass::RouteLabelParameter,
        ]);
        let null = geometric_null(&system, &[]).expect("identity constraints converge");
        let motion = linear::multiply(&null, &[2.0, 3.0, 4.0, 5.0]);
        assert_eq!(motion, [2.0, 0.0, 0.0, 0.0]);
    }

    #[test]
    fn coupled_geometry_cannot_drag_an_internal_dimension_toward_hints() {
        let system = system(&[UnknownClass::Geometric, UnknownClass::AutomaticDimension]);
        let null =
            geometric_null(&system, &[vec![1.0, -1.0]]).expect("coupled constraints converge");
        let motion = linear::multiply(&null, &[2.0, 3.0]);
        assert!(motion.iter().all(|value| value.abs() < 1.0e-12));
    }

    #[test]
    fn geometric_null_space_retains_legacy_projection() {
        let system = system(&[UnknownClass::Geometric, UnknownClass::Geometric]);
        let jacobian = vec![vec![1.0, -1.0]];
        let actual = geometric_null(&system, &jacobian).expect("Jacobi converges");
        let legacy = linear::analyze(&jacobian).expect("Jacobi converges").null;
        assert_eq!(actual, legacy);
    }

    #[test]
    fn rank_truncation_cannot_reintroduce_internal_hint_motion() {
        let system = system(&[UnknownClass::Geometric, UnknownClass::AutomaticDimension]);
        let null =
            geometric_null(&system, &[vec![1.0e11, 1.0]]).expect("restricted Jacobi converges");
        let motion = linear::multiply(&null, &[2.0, 3.0]);
        assert!(motion.iter().all(|value| value.abs() < 1.0e-12));
    }
}
