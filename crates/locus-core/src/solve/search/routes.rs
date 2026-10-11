//! Failed-row route targeting and bounded deterministic choice combinations.
use std::collections::BTreeSet;

use locus_syntax::Expr;

use super::{BTreeMap, PreparedEvaluation, RouteChoice, SourceRelation};
use crate::eval::DependencyGraph;

pub(super) struct Schedule {
    elements: Vec<(String, Vec<RouteChoice>)>,
    current: Vec<usize>,
    lexicographic: Vec<usize>,
    visited: BTreeSet<Vec<usize>>,
}

impl Schedule {
    pub(super) fn new(prepared: &PreparedEvaluation<'_>, sources: &[SourceRelation<'_>]) -> Self {
        let related = source_dependencies(prepared, sources.iter().map(|source| source.expression));
        let elements = prepared
            .registry()
            .iter()
            .filter(|op| related.contains(&op.id.name))
            .filter(|op| crate::eval::routing::route_kind(op).is_some())
            .map(|op| {
                (
                    op.id.name.clone(),
                    crate::eval::routing::route_candidates(op),
                )
            })
            .collect::<Vec<_>>();
        let current = elements
            .iter()
            .map(|(id, choices)| {
                prepared
                    .route_choices()
                    .get(id)
                    .and_then(|choice| choices.iter().position(|candidate| candidate == choice))
                    .unwrap_or(0)
            })
            .collect::<Vec<_>>();
        Self {
            lexicographic: vec![0; elements.len()],
            visited: BTreeSet::from([current.clone()]),
            elements,
            current,
        }
    }

    pub(super) fn next(&mut self, failing: Option<&str>) -> Option<BTreeMap<String, RouteChoice>> {
        let indices = self.targeted(failing).or_else(|| self.lexicographic())?;
        self.visited.insert(indices.clone());
        self.current.clone_from(&indices);
        Some(
            self.elements
                .iter()
                .zip(indices)
                .map(|((id, choices), index)| (id.clone(), choices[index]))
                .collect(),
        )
    }

    fn targeted(&self, failing: Option<&str>) -> Option<Vec<usize>> {
        let index = self
            .elements
            .iter()
            .position(|(id, _)| Some(id.as_str()) == failing)?;
        let mut indices = self.current.clone();
        for choice in indices[index] + 1..self.elements[index].1.len() {
            indices[index] = choice;
            if !self.visited.contains(&indices) {
                return Some(indices);
            }
        }
        None
    }

    fn lexicographic(&mut self) -> Option<Vec<usize>> {
        while advance(&mut self.lexicographic, &self.elements) {
            if !self.visited.contains(&self.lexicographic) {
                return Some(self.lexicographic.clone());
            }
        }
        None
    }
}

fn advance(indices: &mut [usize], elements: &[(String, Vec<RouteChoice>)]) -> bool {
    for index in (0..indices.len()).rev() {
        indices[index] += 1;
        if indices[index] < elements[index].1.len() {
            return true;
        }
        indices[index] = 0;
    }
    false
}

pub(super) fn failing_element(
    prepared: &PreparedEvaluation<'_>,
    state: &super::super::SolveState,
    sources: &[SourceRelation<'_>],
) -> Option<String> {
    // This relative tolerance selects search priorities only; final feasibility keeps
    // SOLVE_EPSILON. It prevents roundoff in a solved route from hiding a conflicting one.
    let threshold = priority_threshold(state);
    for (index, source) in sources.iter().enumerate() {
        if !state
            .equations
            .iter()
            .any(|row| row.relation == index && row.residual.value.abs() >= threshold)
        {
            continue;
        }
        if matches!(source.expression, Expr::Call {name,..} if name.name == "avoid") {
            return None;
        }
        let related = source_dependencies(prepared, core::iter::once(source.expression));
        if let Some(op) = prepared.registry().iter().find(|op| {
            related.contains(&op.id.name) && crate::eval::routing::route_kind(op).is_some()
        }) {
            return Some(op.id.name.clone());
        }
    }
    None
}

pub(super) fn priority_threshold(state: &super::super::SolveState) -> f64 {
    let worst = state
        .equations
        .iter()
        .map(|row| row.residual.value.abs())
        .fold(0.0, f64::max);
    (worst * crate::preset::SEARCH_ROW_PRIORITY_FACTOR).max(super::SOLVE_EPSILON)
}

pub(super) fn source_dependencies<'a>(
    prepared: &PreparedEvaluation<'_>,
    expressions: impl Iterator<Item = &'a Expr>,
) -> BTreeSet<String> {
    let mut result = BTreeSet::new();
    for expression in expressions {
        references(expression, &mut result);
    }
    let Ok(graph) =
        DependencyGraph::extract_with_collections(prepared.file(), prepared.collections())
    else {
        return result;
    };
    let mut pending = result.iter().cloned().collect::<Vec<_>>();
    while let Some(id) = pending.pop() {
        for dependency in graph.dependencies_of(&id).into_iter().flatten() {
            if result.insert(dependency.clone()) {
                pending.push(dependency.clone());
            }
        }
    }
    result
}

fn references(expression: &Expr, result: &mut BTreeSet<String>) {
    if let Expr::Ref(id) = expression {
        result.insert(id.name.clone());
    }
    for child in expression_children(expression) {
        references(child, result);
    }
}

fn expression_children(expression: &Expr) -> Vec<&Expr> {
    match expression {
        Expr::Call { args, .. } => args.iter().collect(),
        Expr::Property { base, args, .. } => {
            core::iter::once(base.as_ref()).chain(args.iter()).collect()
        }
        Expr::Group { expr, .. } | Expr::Unary { expr, .. } => vec![expr],
        Expr::Binary { lhs, rhs, .. } => vec![lhs, rhs],
        _ => Vec::new(),
    }
}

#[cfg(test)]
#[allow(
    clippy::expect_used,
    reason = "valid source for search priority regression"
)]
mod tests {
    use super::*;
    use crate::solve::{SolveState, ad::Jet, linear, relations};

    #[test]
    fn roundoff_above_acceptance_tolerance_does_not_hide_a_material_route_failure() {
        let parsed = locus_syntax::parse(
            "locus 0 beta 6\npoint origin()\npoint tip()\nrel hdist(origin, tip) == 3\nrel vdist(origin, tip) == 2\narrow first(origin,tip) {route:orthogonal}\narrow second(origin,tip) {route:orthogonal}\nrel first.height == 4\nrel second.height == 4\n",
        );
        assert!(parsed.diagnostics.is_empty());
        let prepared = PreparedEvaluation::new(&parsed.file).expect("registered routes");
        let sources = relations::collect(&parsed.file);
        let state = SolveState {
            equations: vec![
                relations::Equation {
                    residual: Jet::constant(crate::tol::SOLVE_EPSILON * 10.0, 4),
                    relation: 2,
                },
                relations::Equation {
                    residual: Jet::constant(0.1, 4),
                    relation: 3,
                },
            ],
            equalities: Vec::new(),
            inequalities: Vec::new(),
            preferences: Vec::new(),
            residuals: Vec::new(),
            jacobian: Vec::new(),
            analysis: linear::unconstrained(4),
        };
        assert_eq!(
            failing_element(&prepared, &state, &sources).as_deref(),
            Some("second")
        );
    }
}
