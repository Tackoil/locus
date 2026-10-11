//! Bounded path-label side and near-region candidate retries.
use super::{BTreeMap, PreparedEvaluation, SourceRelation};
use crate::eval::path_label;
use crate::geom::route::RouteSide;
use crate::inputs::LabelChoice;
use std::collections::BTreeSet;

pub(super) struct Schedule {
    elements: Vec<(String, Vec<LabelChoice>)>,
    current: Vec<usize>,
    fallback: Vec<usize>,
    visited: BTreeSet<Vec<usize>>,
}

impl Schedule {
    pub(super) fn new(prepared: &PreparedEvaluation<'_>, sources: &[SourceRelation<'_>]) -> Self {
        let related = related(prepared, sources.iter().map(|source| source.expression));
        let elements = prepared
            .registry()
            .iter()
            .filter(|op| {
                path_label::owns_parameters(op, prepared.file()) && related.contains(&op.id.name)
            })
            .map(|op| (op.id.name.clone(), candidates(op)))
            .collect::<Vec<_>>();
        let current = elements
            .iter()
            .map(|(id, choices)| {
                prepared
                    .inputs()
                    .choices
                    .labels
                    .get(id)
                    .and_then(|choice| choices.iter().position(|candidate| choice == candidate))
                    .unwrap_or(0)
            })
            .collect::<Vec<_>>();
        Self {
            fallback: vec![0; elements.len()],
            visited: BTreeSet::from([current.clone()]),
            elements,
            current,
        }
    }

    pub(super) fn next(&mut self, failing: Option<&str>) -> Option<BTreeMap<String, LabelChoice>> {
        let indices = self.targeted(failing).or_else(|| self.fallback())?;
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

    pub(super) fn new_route(&mut self) {
        self.visited = BTreeSet::from([self.current.clone()]);
        self.fallback.fill(0);
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

    fn fallback(&mut self) -> Option<Vec<usize>> {
        while advance(&mut self.fallback, &self.elements) {
            if !self.visited.contains(&self.fallback) {
                return Some(self.fallback.clone());
            }
        }
        None
    }
}

fn candidates(op: &locus_syntax::OpStmt) -> Vec<LabelChoice> {
    let authored = path_label::side(op).unwrap_or(RouteSide::Above);
    let mut sides = vec![authored];
    if !path_label::has_fixed_side(op) {
        sides.extend(
            crate::preset::ROUTE_PAGE_SIDE_ORDER
                .into_iter()
                .filter(|side| *side != authored),
        );
    }
    sides
        .into_iter()
        .flat_map(|side| (0..3).map(move |candidate| LabelChoice { side, candidate }))
        .collect()
}

fn advance(indices: &mut [usize], elements: &[(String, Vec<LabelChoice>)]) -> bool {
    for index in (0..indices.len()).rev() {
        indices[index] += 1;
        if indices[index] < elements[index].1.len() {
            return true;
        }
        indices[index] = 0;
    }
    false
}

fn related<'a>(
    prepared: &PreparedEvaluation<'_>,
    expressions: impl Iterator<Item = &'a locus_syntax::Expr>,
) -> BTreeSet<String> {
    let mut dependencies = super::routes::source_dependencies(prepared, expressions);
    for (path, labels) in path_label::attachments(prepared.file()) {
        if dependencies.contains(&path) && !labels.iter().any(|label| dependencies.contains(label))
        {
            dependencies.extend(labels);
        }
    }
    dependencies
}

pub(super) fn failing_element(
    prepared: &PreparedEvaluation<'_>,
    state: &super::super::SolveState,
    sources: &[SourceRelation<'_>],
) -> Option<String> {
    let threshold = super::routes::priority_threshold(state);
    for (index, source) in sources.iter().enumerate() {
        if !state
            .equations
            .iter()
            .any(|row| row.relation == index && row.residual.value.abs() >= threshold)
        {
            continue;
        }
        let dependencies = related(prepared, core::iter::once(source.expression));
        if let Some(op) = prepared.registry().iter().find(|op| {
            path_label::owns_parameters(op, prepared.file()) && dependencies.contains(&op.id.name)
        }) {
            return Some(op.id.name.clone());
        }
    }
    None
}

pub(super) fn reports(prepared: &PreparedEvaluation<'_>) -> Vec<super::super::BranchResolution> {
    prepared
        .registry()
        .iter()
        .filter(|op| path_label::owns_parameters(op, prepared.file()))
        .map(|op| {
            let mut choice = prepared
                .inputs()
                .choices
                .labels
                .get(&op.id.name)
                .copied()
                .unwrap_or(LabelChoice {
                    side: path_label::side(op).unwrap_or(RouteSide::Above),
                    candidate: 0,
                });
            choice.side = path_label::accepted_side(op, Some(&choice)).unwrap_or(choice.side);
            super::super::BranchResolution {
                span: op.span,
                choice: format!(
                    "label {}: {:?}, candidate {}",
                    op.id.name, choice.side, choice.candidate
                ),
            }
        })
        .collect()
}
