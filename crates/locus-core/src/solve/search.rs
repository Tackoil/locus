//! Bounded source-ordered retries after continuous layout failure.
use super::{
    BTreeMap, CircleBranch, EvalError, PreparedEvaluation, SOLVE_EPSILON, Solved, SourceRelation,
    UnknownSystem, evaluate_state, finish_solve, initialize, layout, max_residual, run_lm, settle,
};
use crate::geom::route::RouteChoice;
mod avoidance;
mod labels;
mod routes;
pub(super) fn annotate_failure(
    prepared: &PreparedEvaluation<'_>,
    state: &super::SolveState,
    sources: &[SourceRelation<'_>],
    mut error: EvalError,
) -> EvalError {
    error.set_failed_inequalities(state.inequalities.iter().enumerate().filter_map(|(index, row)| {
        let source = sources.get(row.relation)?;
        (row.residual.value < -SOLVE_EPSILON && matches!(source.expression, locus_syntax::Expr::Call {name,..} if name.name == "avoid")).then_some(index)
    }).collect::<Vec<_>>());
    let threshold = routes::priority_threshold(state);
    if state.equations.iter().any(|row| {
        row.residual.value.abs() >= threshold
            && sources
                .get(row.relation)
                .is_some_and(|source| layout::is_layout(source.expression))
    }) {
        error.code = "E413";
    }

    match labels::failing_element(prepared, state, sources)
        .or_else(|| routes::failing_element(prepared, state, sources))
    {
        Some(id) => {
            error.code = "E413";
            error.for_object(id)
        }
        None => error,
    }
}

pub(super) fn solve(
    prepared: &PreparedEvaluation<'_>,
    system: &UnknownSystem,
    sources: &[SourceRelation<'_>],
    branches: &[Option<CircleBranch>],
    cap: usize,
) -> Result<Solved, EvalError> {
    let mut count = 0;
    let first =
        attempt(prepared, system, sources, branches, cap, &mut count).map_err(|mut error| {
            error.set_failed_search_attempts(count);
            error
        });
    if first.is_ok() || !has_discrete_work(prepared, sources) {
        return first;
    }
    let original_error = first.err().unwrap_or_else(|| unreachable!());
    let mut failure = SearchFailure {
        count,
        error: original_error.clone(),
    };
    if let Some(solved) = retry_discrete(prepared, sources, branches, cap, &mut failure) {
        return Ok(solved);
    }
    let count = failure.count;
    let mut best_error = original_error;
    best_error.set_failed_search_attempts(count);
    if best_error.code == "E413" {
        best_error.message = format!(
            "layout conflict after {count} continuous candidates: {}",
            best_error.message
        );
    }
    Err(best_error)
}

struct SearchFailure {
    count: usize,
    error: EvalError,
}

fn retry_discrete(
    prepared: &PreparedEvaluation<'_>,
    sources: &[SourceRelation<'_>],
    branches: &[Option<CircleBranch>],
    cap: usize,
    failure: &mut SearchFailure,
) -> Option<Solved> {
    let mut schedules = Schedules::new(prepared, sources);
    while failure.count < crate::preset::SEARCH_CAP {
        if !schedules.next_choice(prepared, &failure.error) {
            break;
        }
        let candidate = prepared.with_layout_choices(schedules.choices.clone());
        match candidate_attempt(
            &candidate,
            sources,
            branches,
            cap,
            &schedules.avoidance.current,
            failure,
        ) {
            Ok(solved) => return Some(solved),
            Err(error) => failure.error = error,
        }
    }
    None
}

struct Schedules {
    routes: routes::Schedule,
    labels: labels::Schedule,
    avoidance: avoidance::Schedule,
    choices: crate::inputs::EvaluationChoices,
}

impl Schedules {
    fn new(prepared: &PreparedEvaluation<'_>, sources: &[SourceRelation<'_>]) -> Self {
        let mut choices = prepared.inputs().choices.clone();
        choices.routes.clone_from(prepared.route_choices());
        Self {
            routes: routes::Schedule::new(prepared, sources),
            labels: labels::Schedule::new(prepared, sources),
            avoidance: avoidance::Schedule::default(),
            choices,
        }
    }

    fn next_choice(&mut self, prepared: &PreparedEvaluation<'_>, failure: &EvalError) -> bool {
        if let Some(id) = failure.object_id.as_deref()
            && let Some(op) = prepared.registry().iter().find(|op| op.id.name == id)
            && self.targeted_choice(op)
        {
            return true;
        }
        let failed = failure.failed_inequalities();
        if self.avoidance.next(failed).is_some() {
            return true;
        }
        if let Some(next) = self.labels.next(None) {
            self.choices.labels.extend(next);
            self.avoidance.new_route();
            return true;
        }
        self.route_choice(None)
    }

    fn targeted_choice(&mut self, op: &locus_syntax::OpStmt) -> bool {
        if op.kind == locus_syntax::Kind::Label {
            if let Some(next) = self.labels.next(Some(&op.id.name)) {
                self.choices.labels.extend(next);
                self.avoidance.new_route();
                return true;
            }
            return false;
        }
        self.route_choice(Some(&op.id.name))
    }

    fn route_choice(&mut self, id: Option<&str>) -> bool {
        if let Some(next) = self.routes.next(id) {
            self.choices.routes.extend(next);
            self.avoidance.new_route();
            self.labels.new_route();
            return true;
        }
        false
    }
}

fn candidate_attempt(
    prepared: &PreparedEvaluation<'_>,
    sources: &[SourceRelation<'_>],
    branches: &[Option<CircleBranch>],
    cap: usize,
    axes: &BTreeMap<usize, usize>,
    failure: &mut SearchFailure,
) -> Result<Solved, EvalError> {
    let mut initialized = initialize(prepared, sources).inspect_err(|_| {
        failure.count += 1;
    })?;
    initialized.avoidance_axes.clone_from(axes);
    attempt(
        prepared,
        &initialized,
        sources,
        branches,
        cap,
        &mut failure.count,
    )
}

fn attempt(
    prepared: &PreparedEvaluation<'_>,
    system: &UnknownSystem,
    sources: &[SourceRelation<'_>],
    branches: &[Option<CircleBranch>],
    cap: usize,
    count: &mut usize,
) -> Result<Solved, EvalError> {
    let _candidate = crate::metrics::span("solve_candidate");
    *count += 1;
    let mut outcome = run_lm(
        prepared,
        system,
        sources,
        branches,
        cap,
        !has_discrete_work(prepared, sources),
    )?;
    // Restore true signed-separation semantics before settling or accepting a candidate.
    let mut accepted_system = system.clone();
    accepted_system.avoidance_axes.clear();
    outcome.state = crate::metrics::measure("validate_candidate", || {
        evaluate_state(
            prepared,
            &accepted_system,
            sources,
            branches,
            &outcome.normalized,
        )
    })?;
    if max_residual(&outcome.state.residuals) >= SOLVE_EPSILON {
        let error = EvalError::new(
            "E413",
            prepared.file().span,
            "candidate violates actual separation",
        );
        return Err(annotate_failure(prepared, &outcome.state, sources, error));
    }
    crate::metrics::measure("settle_preferences", || {
        settle(prepared, &accepted_system, sources, branches, &mut outcome)
    })?;
    let mut solved = finish_solve(prepared, &accepted_system, sources, branches, &outcome)?;
    solved.summary.outer_search_attempts = *count;
    solved
        .summary
        .branch_resolutions
        .extend(route_reports(prepared));
    solved
        .summary
        .branch_resolutions
        .extend(labels::reports(prepared));
    solved
        .summary
        .branch_resolutions
        .extend(avoidance_reports(system, &outcome.state, sources));
    Ok(solved)
}

fn has_discrete_work(prepared: &PreparedEvaluation<'_>, sources: &[SourceRelation<'_>]) -> bool {
    prepared
        .registry()
        .iter()
        .any(|op| crate::eval::routing::route_kind(op).is_some())
        || prepared
            .registry()
            .iter()
            .any(|op| crate::eval::path_label::owns_parameters(op, prepared.file()))
        || sources.iter().any(|s| layout::is_layout(s.expression))
}

fn route_reports(prepared: &PreparedEvaluation<'_>) -> Vec<super::BranchResolution> {
    prepared
        .registry()
        .iter()
        .filter(|op| crate::eval::routing::route_kind(op).is_some())
        .map(|op| {
            let choice = crate::eval::routing::accepted_choice(op, prepared.route_choices());
            let topology = route_topology(op, choice);
            super::BranchResolution {
                span: op.span,
                choice: format!("route {}: {:?}, {topology}", op.id.name, choice.side),
            }
        })
        .collect()
}

fn route_topology(op: &locus_syntax::OpStmt, choice: RouteChoice) -> &'static str {
    if crate::eval::routing::route_kind(op) == Some(crate::geom::route::RouteKind::Curved) {
        "quadratic"
    } else if choice.five_span {
        "five-span"
    } else {
        "three-span"
    }
}

fn avoidance_reports(
    system: &UnknownSystem,
    state: &super::SolveState,
    sources: &[SourceRelation<'_>],
) -> Vec<super::BranchResolution> {
    system
        .avoidance_axes
        .iter()
        .filter_map(|(index, choice)| {
            let row = state.inequalities.get(*index)?;
            let source = sources.get(row.relation)?;
            Some(super::BranchResolution {
                span: source.span,
                choice: format!("avoidance escape direction {choice}"),
            })
        })
        .collect()
}
