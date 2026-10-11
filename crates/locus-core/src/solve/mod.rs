//! Deterministic relational solve and DOF reporting.

mod ad;
mod conflict;
mod linear;
mod relations;

use std::collections::BTreeMap;

use locus_syntax::{Constructor, File, Span, Statement};

use crate::eval::{EvalError, PreparedEvaluation};
use crate::scene::{Dimension, ResolvedObject, ResolvedScene};
use crate::tol::{HINT_FAR_THRESHOLD, SOLVE_EPSILON};

use self::ad::{DifferentiatedFile, NumericDimension, dimensions_from_scene};
use self::linear::Analysis;
use self::relations::{CircleBranch, Equation, SourceRelation};

const INITIAL_DAMPING: f64 = 1.0e-3;
const MIN_DAMPING: f64 = 1.0e-15;
const MAX_DAMPING: f64 = 1.0e15;
const MAX_ATTEMPTS: usize = 200;

/// A solved scene and its deterministic solve report.
#[derive(Clone, Debug, PartialEq)]
pub struct Solved {
    /// Fully resolved source-order scene.
    pub scene: ResolvedScene,
    /// Iteration, DOF, branch, and hint-displacement report.
    pub summary: SolveSummary,
}

/// Normative solve statistics exposed to `describe` and `check`.
#[derive(Clone, Debug, PartialEq)]
pub struct SolveSummary {
    /// Scalar unknown count after expanding free points.
    pub unknowns: usize,
    /// Scalar residual equation count after relation expansion.
    pub equations: usize,
    /// Final normalized Jacobian rank.
    pub rank: usize,
    /// LM attempts, including rejected trials.
    pub attempts: usize,
    /// Accepted LM steps.
    pub accepted_steps: usize,
    /// Largest absolute normalized final residual.
    pub max_normalized_residual: f64,
    /// Final LM damping value.
    pub final_damping: f64,
    /// Global and per-object null-space analysis.
    pub dof: DofReport,
    /// Seed-fixed discrete branch decisions.
    pub branch_resolutions: Vec<BranchResolution>,
    /// Source and normalized displacements from hints.
    pub hint_displacements: Vec<HintDisplacement>,
    /// Relations found redundant at the final Jacobian.
    pub redundant_relations: Vec<RelationReport>,
    /// Per-relation final residuals in source order.
    pub relation_residuals: Vec<RelationReport>,
}

/// Global and local hint-settled degrees of freedom.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DofReport {
    /// `unknowns - rank` at the solution.
    pub global_remaining: usize,
    /// One source-order entry per free object.
    pub objects: Vec<ObjectDof>,
}

/// Local null-space rank for one free declaration.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ObjectDof {
    /// Free declaration identifier.
    pub id: String,
    /// Declaration source span.
    pub span: Span,
    /// Independent global null directions that can move this object.
    pub hint_settled_dofs: usize,
    /// Stable suggestion suitable for an under-constraint diagnostic.
    pub suggestion: String,
}

/// One hint-to-solution displacement report.
#[derive(Clone, Debug, PartialEq)]
pub struct HintDisplacement {
    /// Free declaration identifier.
    pub id: String,
    /// Declaration source span.
    pub span: Span,
    /// Per-component displacement in source dimensions.
    pub source_components: Vec<f64>,
    /// Per-component displacement in normalized unknown coordinates.
    pub normalized_components: Vec<f64>,
    /// Largest absolute normalized component displacement.
    pub max_normalized_component: f64,
    /// Whether the normative `0.25` `hint_far` threshold is exceeded.
    pub hint_far: bool,
    /// Normative normalized threshold reported alongside the actual value.
    pub hint_far_threshold: f64,
}

/// A seed-selected discrete branch.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BranchResolution {
    /// Relation source span.
    pub span: Span,
    /// Human-readable stable branch name.
    pub choice: String,
}

/// Source-anchored relation residual and implicated unknowns.
#[derive(Clone, Debug, PartialEq)]
pub struct RelationReport {
    /// Atomic relation source span.
    pub span: Span,
    /// Largest absolute normalized component residual.
    pub max_normalized_residual: f64,
    /// Free objects whose final Jacobian columns participate.
    pub implicated_unknowns: Vec<String>,
}

#[derive(Clone, Debug)]
struct UnknownObject {
    id: String,
    span: Span,
    start: usize,
    count: usize,
    dimension: NumericDimension,
    report_hint: bool,
}

#[derive(Clone, Debug)]
struct UnknownSystem {
    objects: Vec<UnknownObject>,
    hints: Vec<f64>,
    scales: Vec<f64>,
    normalized_hints: Vec<f64>,
    dimensions: BTreeMap<String, NumericDimension>,
    diameter: f64,
}

struct SolveState {
    equations: Vec<Equation>,
    residuals: Vec<f64>,
    jacobian: Vec<Vec<f64>>,
    analysis: Analysis,
}

/// Solves every free declaration and relation in a parsed file.
pub fn solve(file: &File) -> Result<Solved, EvalError> {
    let prepared = PreparedEvaluation::new(file)?;
    solve_prepared(&prepared)
}

pub(crate) fn solve_prepared(prepared: &PreparedEvaluation<'_>) -> Result<Solved, EvalError> {
    solve_prepared_with_attempt_limit(prepared, MAX_ATTEMPTS)
}

fn solve_prepared_with_attempt_limit(
    prepared: &PreparedEvaluation<'_>,
    max_attempts: usize,
) -> Result<Solved, EvalError> {
    let file = prepared.file();
    let seed_trial = prepared.evaluate_trial(None).map_err(degenerate_seed)?;
    let system = UnknownSystem::from_seed(file, prepared.dimensions(), &seed_trial)?;
    let sources = relations::collect(file);
    validate_relation_primals(&seed_trial, prepared, &sources).map_err(degenerate_seed)?;
    let seed_ad = DifferentiatedFile::evaluate(
        file,
        &system.normalized_hints,
        &system.normalized_hints,
        &system.scales,
        &system.dimensions,
        prepared.dimensions(),
        seed_trial.tolerance(),
    )
    .map_err(degenerate_seed)?;
    seed_ad
        .validate_branch_sensitive_primal(file, seed_trial.objects())
        .map_err(degenerate_seed)?;
    let branches = relations::select_circle_branches(&sources);
    let outcome = run_lm(prepared, &system, &sources, &branches, max_attempts, true)?;
    finish_solve(prepared, &system, &sources, &branches, &outcome)
}

struct LmOutcome {
    normalized: Vec<f64>,
    state: SolveState,
    attempts: usize,
    accepted: usize,
    damping: f64,
}

fn run_lm(
    prepared: &PreparedEvaluation<'_>,
    system: &UnknownSystem,
    sources: &[SourceRelation<'_>],
    branches: &[Option<CircleBranch>],
    max_attempts: usize,
    diagnose_failure: bool,
) -> Result<LmOutcome, EvalError> {
    let mut normalized = system.normalized_hints.clone();
    let mut state = evaluate_state(prepared, system, sources, branches, &normalized)?;
    let mut damping = INITIAL_DAMPING;
    let mut attempts = 0;
    let mut accepted = 0;
    while max_residual(&state.residuals) >= SOLVE_EPSILON && attempts < max_attempts {
        let primary = linear::lm_primary_step(&state.jacobian, &state.residuals, damping)
            .ok_or_else(|| {
                nonconvergence_error(
                    prepared,
                    system,
                    &state,
                    sources,
                    branches,
                    "LM Cholesky factorization failed",
                    diagnose_failure,
                )
            })?;
        let trial_vector = trial_step(
            &normalized,
            &primary,
            &state.analysis.null,
            &system.normalized_hints,
            damping,
        );
        attempts += 1;
        let trial = evaluate_state(prepared, system, sources, branches, &trial_vector);
        if trial
            .as_ref()
            .is_ok_and(|trial| accepted_trial(&state, trial))
        {
            normalized = trial_vector;
            state = trial.unwrap_or_else(|_| unreachable!());
            accepted += 1;
            damping = (damping / 10.0).max(MIN_DAMPING);
        } else {
            if damping == MAX_DAMPING {
                return Err(failed_solve_error(
                    prepared,
                    system,
                    &state,
                    sources,
                    branches,
                    "LM damping ceiling reached",
                    diagnose_failure,
                ));
            }
            damping = (damping * 10.0).min(MAX_DAMPING);
        }
    }
    if max_residual(&state.residuals) >= SOLVE_EPSILON {
        return Err(failed_solve_error(
            prepared,
            system,
            &state,
            sources,
            branches,
            "LM iteration cap reached",
            diagnose_failure,
        ));
    }
    Ok(LmOutcome {
        normalized,
        state,
        attempts,
        accepted,
        damping,
    })
}

fn evaluate_state(
    prepared: &PreparedEvaluation<'_>,
    system: &UnknownSystem,
    sources: &[SourceRelation<'_>],
    branches: &[Option<CircleBranch>],
    normalized: &[f64],
) -> Result<SolveState, EvalError> {
    let file = prepared.file();
    let actual = denormalize(normalized, &system.scales);
    let trial = prepared.evaluate_trial(Some(&actual))?;
    validate_relation_primals(&trial, prepared, sources)?;
    let differentiated = DifferentiatedFile::evaluate(
        file,
        normalized,
        &system.normalized_hints,
        &system.scales,
        &system.dimensions,
        prepared.dimensions(),
        trial.tolerance(),
    )?;
    differentiated.validate_branch_sensitive_primal(file, trial.objects())?;
    let equations = relations::evaluate(&differentiated, sources, branches, system.diameter)?;
    let residuals = equations
        .iter()
        .map(|equation| equation.residual.value)
        .collect::<Vec<_>>();
    let jacobian = equations
        .iter()
        .map(|equation| equation.residual.gradient.clone())
        .collect::<Vec<_>>();
    let analysis = if jacobian.is_empty() {
        linear::unconstrained(system.hints.len())
    } else {
        linear::analyze(&jacobian).map_err(|()| {
            EvalError::new(
                "E403",
                file.span,
                "Jacobi SVD did not converge in 64 sweeps",
            )
        })?
    };
    Ok(SolveState {
        equations,
        residuals,
        jacobian,
        analysis,
    })
}

fn validate_relation_primals(
    trial: &crate::eval::TrialEvaluation,
    prepared: &PreparedEvaluation<'_>,
    sources: &[SourceRelation<'_>],
) -> Result<(), EvalError> {
    for source in sources {
        trial.validate_relation_expression(source.expression, prepared.dimensions())?;
    }
    Ok(())
}

fn trial_step(
    current: &[f64],
    primary: &[f64],
    null: &[Vec<f64>],
    hints: &[f64],
    damping: f64,
) -> Vec<f64> {
    let after_primary = current
        .iter()
        .zip(primary)
        .map(|(value, step)| value + step)
        .collect::<Vec<_>>();
    let toward_hint = hints
        .iter()
        .zip(&after_primary)
        .map(|(hint, value)| hint - value)
        .collect::<Vec<_>>();
    let settled = linear::multiply(null, &toward_hint);
    after_primary
        .iter()
        .zip(settled)
        .map(|(value, hint_step)| value + hint_step / (1.0 + damping))
        .collect()
}

fn accepted_trial(current: &SolveState, trial: &SolveState) -> bool {
    let maximum = max_residual(&trial.residuals);
    maximum.is_finite()
        && (maximum < SOLVE_EPSILON || objective(&trial.residuals) < objective(&current.residuals))
}

fn objective(residuals: &[f64]) -> f64 {
    let mut sum = 0.0;
    for residual in residuals {
        sum += residual * residual;
    }
    0.5 * sum
}

fn max_residual(residuals: &[f64]) -> f64 {
    debug_assert!(residuals.iter().all(|value| value.is_finite()));
    let mut maximum = 0.0;
    for residual in residuals {
        let magnitude = residual.abs();
        if magnitude.total_cmp(&maximum).is_gt() {
            maximum = magnitude;
        }
    }
    maximum
}

fn finish_solve(
    prepared: &PreparedEvaluation<'_>,
    system: &UnknownSystem,
    sources: &[SourceRelation<'_>],
    branches: &[Option<CircleBranch>],
    outcome: &LmOutcome,
) -> Result<Solved, EvalError> {
    let actual = denormalize(&outcome.normalized, &system.scales);
    let scene = prepared.evaluate_trial(Some(&actual))?.into_scene();
    let relation_residuals = relation_reports(sources, &outcome.state, &system.objects);
    let redundant_relations = redundant_reports(sources, &outcome.state, &system.objects)?;
    let dof = dof_report(&system.objects, &outcome.state.analysis)?;
    let summary = SolveSummary {
        unknowns: system.hints.len(),
        equations: outcome.state.equations.len(),
        rank: outcome.state.analysis.rank,
        attempts: outcome.attempts,
        accepted_steps: outcome.accepted,
        max_normalized_residual: max_residual(&outcome.state.residuals),
        final_damping: outcome.damping,
        dof,
        branch_resolutions: branch_reports(sources, branches),
        hint_displacements: hint_reports(system, &actual, &outcome.normalized),
        redundant_relations,
        relation_residuals,
    };
    Ok(Solved { scene, summary })
}

impl UnknownSystem {
    fn from_seed(
        file: &File,
        inferred_dimensions: &crate::eval::expr::DimensionMap,
        seed: &crate::eval::TrialEvaluation,
    ) -> Result<Self, EvalError> {
        let dimensions = dimensions_from_scene(seed.objects());
        let diameter = seed.tolerance().diameter();
        let mut objects = Vec::new();
        let mut hints = Vec::new();
        for statement in &file.statements {
            let Statement::Op(op) = statement else {
                continue;
            };
            if matches!(op.ctor, Constructor::FreeNum { .. })
                || crate::eval::is_placed_op(op, inferred_dimensions)
            {
                add_unknown(op, seed.objects(), &dimensions, &mut objects, &mut hints)?;
            }
        }
        let scales = unknown_scales(&objects, &hints, diameter);
        let normalized_hints = hints
            .iter()
            .zip(&scales)
            .map(|(value, scale)| value / scale)
            .collect();
        Ok(Self {
            objects,
            hints,
            scales,
            normalized_hints,
            dimensions,
            diameter,
        })
    }
}

fn add_unknown(
    op: &locus_syntax::OpStmt,
    resolved: &indexmap::IndexMap<locus_syntax::Ident, ResolvedObject>,
    dimensions: &BTreeMap<String, NumericDimension>,
    objects: &mut Vec<UnknownObject>,
    hints: &mut Vec<f64>,
) -> Result<(), EvalError> {
    let object = resolved
        .iter()
        .find(|(id, _)| id.name == op.id.name)
        .map(|(_, object)| object)
        .ok_or_else(|| {
            EvalError::new("E404", op.span, "free object missing from seed evaluation")
        })?;
    let start = hints.len();
    let (count, dimension) = match object {
        ResolvedObject::Point(point) => {
            hints.extend([point.geometry.x, point.geometry.y]);
            (2, NumericDimension::Length)
        }
        ResolvedObject::Num(value) => {
            hints.push(value.value);
            (
                1,
                dimensions
                    .get(&op.id.name)
                    .copied()
                    .unwrap_or(dimension_from_core(value.dimension)),
            )
        }
        ResolvedObject::Angle(value) => {
            hints.push(*value);
            (1, NumericDimension::Angle)
        }
        ResolvedObject::Circle(circle) => {
            hints.extend([circle.center.x, circle.center.y]);
            (2, NumericDimension::Length)
        }
        ResolvedObject::Ellipse(ellipse) => {
            hints.extend([ellipse.center.x, ellipse.center.y]);
            (2, NumericDimension::Length)
        }
        ResolvedObject::Rect(polygon) | ResolvedObject::Diamond(polygon) => {
            let center = crate::geom::BBox::for_polygon(polygon).center();
            hints.extend([center.x, center.y]);
            (2, NumericDimension::Length)
        }
        _ => {
            return Err(EvalError::new(
                "E404",
                op.span,
                "free declaration has an invalid resolved kind",
            ));
        }
    };
    objects.push(UnknownObject {
        id: op.id.name.clone(),
        span: op.span,
        start,
        count,
        dimension,
        report_hint: matches!(op.ctor, Constructor::FreeNum { .. }),
    });
    Ok(())
}

fn unknown_scales(objects: &[UnknownObject], hints: &[f64], diameter: f64) -> Vec<f64> {
    let mut result = vec![1.0; hints.len()];
    for object in objects {
        let scale = unknown_scale(object, hints, diameter);
        for value in &mut result[object.start..object.start + object.count] {
            *value = scale;
        }
    }
    result
}

fn unknown_scale(object: &UnknownObject, hints: &[f64], diameter: f64) -> f64 {
    match object.dimension {
        NumericDimension::Length => diameter,
        NumericDimension::Area => diameter * diameter,
        NumericDimension::Angle => 1.0,
        NumericDimension::Scalar => hints[object.start].abs().max(1.0),
    }
}

fn denormalize(normalized: &[f64], scales: &[f64]) -> Vec<f64> {
    normalized
        .iter()
        .zip(scales)
        .map(|(value, scale)| value * scale)
        .collect()
}

fn dof_report(objects: &[UnknownObject], analysis: &Analysis) -> Result<DofReport, EvalError> {
    let mut reports = Vec::new();
    for object in objects {
        let block = analysis.null[object.start..object.start + object.count].to_vec();
        let local = linear::rank(&block)
            .map_err(|()| EvalError::new("E403", object.span, "local DOF SVD did not converge"))?;
        reports.push(ObjectDof {
            id: object.id.clone(),
            span: object.span,
            hint_settled_dofs: local,
            suggestion: suggestion(object),
        });
    }
    Ok(DofReport {
        global_remaining: analysis.null.len().saturating_sub(analysis.rank),
        objects: reports,
    })
}

fn suggestion(object: &UnknownObject) -> String {
    if object.count == 2 {
        format!(
            "add a relation if unintended, e.g. on({}, …) or vdist(…) == …",
            object.id
        )
    } else {
        format!(
            "add an equality involving `{}` if this free numeric value should be fixed",
            object.id
        )
    }
}

fn hint_reports(
    system: &UnknownSystem,
    actual: &[f64],
    normalized: &[f64],
) -> Vec<HintDisplacement> {
    system
        .objects
        .iter()
        .filter(|object| object.report_hint)
        .map(|object| {
            let range = object.start..object.start + object.count;
            let source_components = range
                .clone()
                .map(|index| actual[index] - system.hints[index])
                .collect::<Vec<_>>();
            let normalized_components = range
                .map(|index| normalized[index] - system.normalized_hints[index])
                .collect::<Vec<_>>();
            let maximum = normalized_components
                .iter()
                .map(|value| value.abs())
                .fold(0.0, f64::max);
            HintDisplacement {
                id: object.id.clone(),
                span: object.span,
                source_components,
                normalized_components,
                max_normalized_component: maximum,
                hint_far: maximum > HINT_FAR_THRESHOLD,
                hint_far_threshold: HINT_FAR_THRESHOLD,
            }
        })
        .collect()
}

fn branch_reports(
    sources: &[SourceRelation<'_>],
    branches: &[Option<CircleBranch>],
) -> Vec<BranchResolution> {
    sources
        .iter()
        .zip(branches)
        .filter_map(|(source, branch)| {
            branch.map(|branch| BranchResolution {
                span: source.span,
                choice: match branch {
                    CircleBranch::External => "external circle tangency".to_owned(),
                    CircleBranch::InternalFirstOuter => {
                        "internal circle tangency (first circle contains second)".to_owned()
                    }
                },
            })
        })
        .collect()
}

fn relation_reports(
    sources: &[SourceRelation<'_>],
    state: &SolveState,
    objects: &[UnknownObject],
) -> Vec<RelationReport> {
    sources
        .iter()
        .enumerate()
        .map(|(index, source)| {
            let rows = state
                .equations
                .iter()
                .filter(|equation| equation.relation == index)
                .collect::<Vec<_>>();
            RelationReport {
                span: source.span,
                max_normalized_residual: rows
                    .iter()
                    .map(|equation| equation.residual.value.abs())
                    .fold(0.0, f64::max),
                implicated_unknowns: implicated(&rows, objects),
            }
        })
        .collect()
}

fn redundant_reports(
    sources: &[SourceRelation<'_>],
    state: &SolveState,
    objects: &[UnknownObject],
) -> Result<Vec<RelationReport>, EvalError> {
    let all = relation_reports(sources, state, objects);
    let mut rows = Vec::new();
    let mut previous_rank = 0;
    let mut redundant = Vec::new();
    for (index, report) in all.into_iter().enumerate() {
        let additions = state
            .equations
            .iter()
            .filter(|equation| equation.relation == index)
            .map(|equation| equation.residual.gradient.clone())
            .collect::<Vec<_>>();
        rows.extend(additions.iter().cloned());
        let rank = linear::rank(&rows)
            .map_err(|()| EvalError::new("E403", report.span, "redundancy SVD did not converge"))?;
        if rank < previous_rank + additions.len() {
            redundant.push(report);
        }
        previous_rank = rank;
    }
    Ok(redundant)
}

fn implicated(rows: &[&Equation], objects: &[UnknownObject]) -> Vec<String> {
    objects
        .iter()
        .filter(|object| {
            rows.iter().any(|equation| {
                equation.residual.gradient[object.start..object.start + object.count]
                    .iter()
                    .any(|value| *value != 0.0)
            })
        })
        .map(|object| object.id.clone())
        .collect()
}

fn failed_solve_error(
    prepared: &PreparedEvaluation<'_>,
    system: &UnknownSystem,
    state: &SolveState,
    sources: &[SourceRelation<'_>],
    branches: &[Option<CircleBranch>],
    reason: &str,
    diagnose_failure: bool,
) -> EvalError {
    let reports = relation_reports(sources, state, &system.objects);
    let mut ordered = reports.iter().collect::<Vec<_>>();
    ordered.sort_by(|lhs, rhs| {
        rhs.max_normalized_residual
            .total_cmp(&lhs.max_normalized_residual)
            .then_with(|| lhs.span.byte_start.cmp(&rhs.span.byte_start))
    });
    let details = ordered
        .iter()
        .map(|report| {
            format!(
                "line {} residual {:.6e} [{}]",
                report.span.line,
                report.max_normalized_residual,
                report.implicated_unknowns.join(", ")
            )
        })
        .collect::<Vec<_>>()
        .join("; ");
    let indices = (0..sources.len()).collect::<Vec<_>>();
    let conflicting = system.hints.is_empty() || conflict::certified(sources, system, &indices);
    let code = if conflicting { "E402" } else { "E403" };
    let category = if conflicting {
        "over-constrained/conflicting"
    } else {
        "non-convergent"
    };
    let infeasible_subset = infeasible_subset_note(
        prepared,
        system,
        sources,
        branches,
        conflicting,
        diagnose_failure,
    );
    EvalError::new(
        code,
        ordered
            .first()
            .map_or(Span::default(), |report| report.span),
        format!(
            "{category} relational system: {reason}; best-fit residuals worst first: {details}{infeasible_subset}; spread or correct hints"
        ),
    )
}

fn infeasible_subset_note(
    prepared: &PreparedEvaluation<'_>,
    system: &UnknownSystem,
    sources: &[SourceRelation<'_>],
    branches: &[Option<CircleBranch>],
    conflicting: bool,
    diagnose_failure: bool,
) -> String {
    if !conflicting {
        return String::new();
    }
    if !diagnose_failure {
        return "; minimal infeasible subset was not isolable".to_owned();
    }
    let Some(indices) = isolate_infeasible_subset(prepared, system, sources, branches) else {
        return "; minimal infeasible subset was not isolable".to_owned();
    };
    let lines = indices
        .iter()
        .map(|index| sources[*index].span.line.to_string())
        .collect::<Vec<_>>();
    format!("; minimal infeasible subset: lines {}", lines.join(", "))
}

fn isolate_infeasible_subset(
    prepared: &PreparedEvaluation<'_>,
    system: &UnknownSystem,
    sources: &[SourceRelation<'_>],
    branches: &[Option<CircleBranch>],
) -> Option<Vec<usize>> {
    let mut retained = (0..sources.len()).collect::<Vec<_>>();
    // A stalled nonlinear solve is not evidence that its equations are impossible.
    // Start deletion only after an algebraic contradiction has been established.
    if !system.hints.is_empty() && !conflict::certified(sources, system, &retained) {
        return None;
    }
    for candidate in 0..sources.len() {
        if retained.len() <= 1 || !retained.contains(&candidate) {
            continue;
        }
        let trial = retained
            .iter()
            .copied()
            .filter(|index| *index != candidate)
            .collect::<Vec<_>>();
        if subset_is_infeasible(prepared, system, sources, branches, &trial)? {
            retained = trial;
        }
    }
    Some(retained)
}

fn subset_is_infeasible(
    prepared: &PreparedEvaluation<'_>,
    system: &UnknownSystem,
    sources: &[SourceRelation<'_>],
    branches: &[Option<CircleBranch>],
    indices: &[usize],
) -> Option<bool> {
    let subset_sources = indices
        .iter()
        .map(|index| sources[*index].clone())
        .collect::<Vec<_>>();
    let subset_branches = indices
        .iter()
        .map(|index| branches[*index])
        .collect::<Vec<_>>();
    match run_lm(
        prepared,
        system,
        &subset_sources,
        &subset_branches,
        MAX_ATTEMPTS,
        false,
    ) {
        Ok(_) => Some(false),
        Err(_) if system.hints.is_empty() || conflict::certified(sources, system, indices) => {
            Some(true)
        }
        Err(_) => None,
    }
}

fn nonconvergence_error(
    prepared: &PreparedEvaluation<'_>,
    system: &UnknownSystem,
    state: &SolveState,
    sources: &[SourceRelation<'_>],
    branches: &[Option<CircleBranch>],
    reason: &str,
    diagnose_failure: bool,
) -> EvalError {
    failed_solve_error(
        prepared,
        system,
        state,
        sources,
        branches,
        reason,
        diagnose_failure,
    )
}

fn degenerate_seed(mut error: EvalError) -> EvalError {
    // Arrow-bend domains cannot be repaired by changing placement hints.
    if error.code == "E210" {
        return error;
    }
    error.code = "E404";
    error.message = format!(
        "degenerate solver seed: {}; spread or correct hints",
        error.message
    );
    error
}

fn dimension_from_core(dimension: Dimension) -> NumericDimension {
    match dimension {
        Dimension::Scalar => NumericDimension::Scalar,
        Dimension::Length => NumericDimension::Length,
        Dimension::Area => NumericDimension::Area,
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::float_cmp, clippy::panic)]
mod tests {
    use std::fmt::Write as _;

    use locus_syntax::parse;

    use super::*;

    fn solve_source(source: &str) -> Solved {
        let parsed = parse(source);
        assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
        solve(&parsed.file).expect("system solves")
    }

    #[test]
    fn alignment_has_known_relative_answer_and_dof_report() {
        let solved = solve_source(
            "locus 0 beta 5\npoint A = point()\npoint B = point()\nrel horizontal(A, B)\nrel hdist(A, B) == 5\n",
        );
        let delta = scene_point(&solved, "B") - scene_point(&solved, "A");
        assert!((delta.x - 5.0).abs() < 1.0e-10);
        assert!(delta.y.abs() < 1.0e-10);
        assert_eq!(solved.summary.rank, 2);
        assert_eq!(solved.summary.dof.global_remaining, 2);
    }

    #[test]
    fn current_format_circle_tangency_is_explicit() {
        let external = solve_source(
            "locus 0 beta 5\ncircle outer = circle(3)\ncircle inner = circle(2)\nrel tangent(outer, inner)\n",
        );
        let internal = solve_source(
            "locus 0 beta 5\ncircle outer = circle(3)\ncircle inner = circle(2)\nrel internally_tangent(outer, inner)\n",
        );
        assert!((circle_distance(&external, "outer", "inner") - 5.0).abs() < 1.0e-12);
        assert!((circle_distance(&internal, "outer", "inner") - 1.0).abs() < 1.0e-12);
    }

    fn circle_distance(solved: &Solved, lhs: &str, rhs: &str) -> f64 {
        let circle = |name: &str| {
            let object = solved
                .scene
                .objects
                .iter()
                .find(|(id, _)| id.name == name)
                .map(|(_, value)| value)
                .expect("circle");
            let ResolvedObject::Circle(circle) = object else {
                panic!("circle")
            };
            *circle
        };
        crate::geom::measure::dist(circle(lhs).center, circle(rhs).center)
    }

    fn scene_point(solved: &Solved, name: &str) -> crate::geom::Point {
        let object = solved
            .scene
            .objects
            .iter()
            .find(|(id, _)| id.name == name)
            .map(|(_, object)| object);
        let Some(ResolvedObject::Point(point)) = object else {
            panic!("missing point {name}");
        };
        point.geometry
    }

    #[test]
    fn every_relation_vocabulary_entry_expands_without_a_solver_panic() {
        let solved = solve_source(
            "locus 0 beta 5\npoint A = point()\npoint B = offset(A, 2, 0)\npoint C = offset(A, 1, 0)\npoint D = offset(A, 0, 2)\npoint E = offset(A, 2, 2)\npoint L = offset(A, -2, 1)\npoint R = offset(A, 2, 1)\npoint Q = offset(A, 0, 1)\nnum r = 1\nline ab = line(A, B)\nline de = line(D, E)\nline ad = line(A, D)\nline lr = line(L, R)\nsegment ac = seg(A, C)\nsegment de_seg = seg(D, E)\nray ar = ray(A, B)\ncircle c1 = circle(A, r)\ncircle c2 = circle(B, r)\narc quarter = arc(c1, C, Q, true)\nrel dist(A, B) == 2\nrel on(C, ab)\nrel on(C, ar)\nrel on(C, ac)\nrel on(C, c1)\nrel on(C, quarter)\nrel tangent(c1, lr)\nrel tangent(c1, c2)\nrel parallel(ab, de)\nrel perpendicular(ab, ad)\nrel collinear(A, C, B)\nrel concyclic(A, B, E, D)\nrel congruent(seg(A, B), de_seg)\nrel coincident(C, C)\nrel midpoint(C, A, B)\nrel horizontal(A, B)\nrel vertical(A, D)\n",
        );
        assert!(solved.summary.max_normalized_residual < SOLVE_EPSILON);
        assert_eq!(solved.summary.relation_residuals.len(), 17);
    }

    #[test]
    fn automatic_differentiation_covers_intersection_and_tangent_line_constructors() {
        let solved = solve_source(
            "locus 0 beta 5\npoint A = point()\npoint B = offset(A, 2, 0)\npoint C = offset(A, 1, -1)\npoint D = offset(A, 1, 1)\nline h = line(A, B)\nline v = line(C, D)\npoint X = intersect(h, v)\nnum r = 0.5\ncircle c = circle(A, r)\npoint P = offset(A, 2, 1)\nline t = tangent_line(P, c, 1)\nrel tangent(c, t)\n",
        );
        assert!(solved.summary.max_normalized_residual < SOLVE_EPSILON);
    }

    #[test]
    fn near_tangent_intersection_uses_the_geometry_kernel_branch_count() {
        let solved = solve_source(
            "locus 0 beta 5\npoint O = point()\npoint A = offset(O, -2, 1.0000000001)\npoint B = offset(O, 2, 1.0000000001)\nline l = line(A, B)\nnum r = 1\ncircle c = circle(O, r)\npoint X = intersect(l, c, 0)\n",
        );
        assert!(solved.summary.max_normalized_residual < SOLVE_EPSILON);
    }

    #[test]
    fn small_circle_second_branch_remains_available_during_solving() {
        let solved = solve_source(
            "locus 0 beta 5\npoint A = point()\npoint B = offset(A, 0.00001, 0)\nnum r = 0.00001\ncircle a = circle(A, r)\ncircle b = circle(B, r)\npoint X = intersect(a, b, 1)\n",
        );
        let intersection = scene_point(&solved, "X");
        let relative = intersection - scene_point(&solved, "A");
        assert!((relative.x - 5.0e-6).abs() < 1.0e-15);
        assert!(relative.y < 0.0);
        assert!(solved.summary.max_normalized_residual < SOLVE_EPSILON);
    }

    #[test]
    fn touching_collinear_segments_intersect_during_solving() {
        let solved = solve_source(
            "locus 0 beta 5\npoint A = point()\npoint B = offset(A, 1, 0)\npoint C = offset(A, 2, 0)\nsegment ab = seg(A, B)\nsegment bc = seg(B, C)\npoint X = intersect(ab, bc)\n",
        );
        assert_eq!(scene_point(&solved, "X"), scene_point(&solved, "B"));
        assert!(solved.summary.max_normalized_residual < SOLVE_EPSILON);
    }

    #[test]
    fn translated_circumcenter_remains_accurate_during_solving() {
        let solved = solve_source(
            "locus 0 beta 5\npoint A = point()\npoint B = offset(A, 4, 0)\npoint C = offset(A, 0, 3)\npoint O = circumcenter(A, B, C)\n",
        );
        assert_eq!(
            scene_point(&solved, "O") - scene_point(&solved, "A"),
            crate::geom::Vec2::new(2.0, 1.5)
        );
        assert!(solved.summary.max_normalized_residual < SOLVE_EPSILON);
    }

    #[test]
    fn large_angle_bisector_supports_projection_during_solving() {
        let solved = solve_source(
            "locus 0 beta 5\npoint B = point()\npoint A = offset(B, 10000000000000, 0)\npoint C = offset(B, 0, 10000000000000)\nline bisector = angle_bisector(A, B, C)\npoint F = foot(A, bisector)\n",
        );
        let tolerance = crate::Tolerance::from_bbox(&solved.scene.bbox);
        let relative = scene_point(&solved, "F") - scene_point(&solved, "B");
        assert!(tolerance.length_zero(relative.x - 5.0e12));
        assert!(tolerance.length_zero(relative.y - 5.0e12));
        assert!(solved.summary.max_normalized_residual < SOLVE_EPSILON);
    }

    #[test]
    fn repeated_angle_bisectors_do_not_inflate_scene_tolerance() {
        let mut source = String::from(
            "locus 0 beta 5\npoint B = point()\npoint A = offset(B, 1, 0)\npoint C = offset(B, 0, 1)\n",
        );
        for index in 0..60 {
            writeln!(source, "line bisector_{index} = angle_bisector(A, B, C)")
                .expect("write source");
        }
        source.push_str("assert coincident(A, B)\n");
        let parsed = parse(&source);
        assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
        let solved = solve(&parsed.file).expect("repeated bisectors solve");
        assert!(solved.scene.bbox.diameter() < 2.0);
        let assertions =
            crate::evaluate_assertions(&parsed.file, &solved.scene).expect("assertion evaluates");
        assert!(!assertions.passed);
    }

    #[test]
    fn redundant_and_underconstrained_systems_are_reported() {
        let solved = solve_source(
            "locus 0 beta 5\npoint A = point()\npoint B = point()\nrel horizontal(A, B)\nrel horizontal(A, B)\n",
        );
        assert_eq!(solved.summary.dof.global_remaining, 3);
        assert_eq!(solved.summary.redundant_relations.len(), 1);
    }

    #[test]
    fn repeated_solves_are_bit_identical_in_process() {
        let source = "locus 0 beta 5\npoint A = point()\npoint B = point()\nrel horizontal(A, B)\nrel dist(A, B) == 5\n";
        let first = solve_source(source);
        let second = solve_source(source);
        assert_eq!(first, second);
    }

    #[test]
    fn repeated_solves_are_bit_identical_across_processes() {
        const CHILD_ENV: &str = "LOCUS_SOLVE_DETERMINISM_CHILD";
        const MARKER: &str = "LOCUS_SOLVE_FINGERPRINT=";
        if std::env::var_os(CHILD_ENV).is_some() {
            let source = "locus 0 beta 5\npoint A = point()\npoint B = point()\nrel horizontal(A, B)\nrel dist(A, B) == 5\n";
            println!("{MARKER}{:?}", solve_source(source));
            return;
        }

        let run_child = || {
            let executable = std::env::current_exe().expect("current test executable");
            let output = std::process::Command::new(executable)
                .args([
                    "--exact",
                    "solve::tests::repeated_solves_are_bit_identical_across_processes",
                    "--nocapture",
                ])
                .env(CHILD_ENV, "1")
                .output()
                .expect("determinism child runs");
            assert!(output.status.success(), "child failed: {output:?}");
            let stdout = String::from_utf8(output.stdout).expect("test output is UTF-8");
            stdout
                .lines()
                .find_map(|line| line.strip_prefix(MARKER))
                .map(str::to_owned)
                .expect("child emitted solve fingerprint")
        };

        assert_eq!(run_child(), run_child());
    }

    #[test]
    fn conflicting_system_returns_report_instead_of_panicking() {
        let parsed = parse("locus 0 beta 5\nnum a = free 0\nrel a == 0\nrel a == 1\n");
        let error = solve(&parsed.file).expect_err("values conflict");
        assert_eq!(error.code, "E402");
        assert!(error.message.contains("best-fit residuals worst first"));
    }

    #[test]
    fn derived_only_relations_are_solved_and_conflicts_are_reported() {
        let satisfied = parse("locus 0 beta 5\nnum a = 1\nrel a == 1\n");
        let evaluation = crate::eval::evaluate(&satisfied.file).expect("relation holds");
        let crate::scene::Evaluation::Solved { summary, .. } = evaluation else {
            panic!("relation-bearing file must enter the solver")
        };
        assert_eq!(summary.unknowns, 0);
        assert_eq!(summary.equations, 1);

        let conflicting = parse("locus 0 beta 5\nnum a = 1\nrel a == 2\n");
        let error = crate::eval::evaluate(&conflicting.file).expect_err("relation conflicts");
        assert_eq!(error.code, "E402");
    }

    #[test]
    fn assert_only_relations_and_invalid_relation_domains_are_targeted() {
        let assert_only = parse("locus 0 beta 5\nnum a = 1\nrel a < 2\n");
        let error = solve(&assert_only.file).expect_err("comparison is assert-only");
        assert_eq!(error.code, "E146");
        assert!(error.message.contains("assert-only"));

        let non_segment = parse(
            "locus 0 beta 5\npoint A = point()\npoint B = point()\nline ab = line(A, B)\nrel congruent(ab, ab)\n",
        );
        let error = solve(&non_segment.file).expect_err("congruent requires segments");
        assert!(error.message.contains("expected a segment"));
    }

    #[test]
    fn iteration_cap_returns_nonconvergence_report() {
        let parsed =
            parse("locus 0 beta 5\npoint A = point()\npoint B = point()\nrel vertical(A, B)\n");
        let prepared = PreparedEvaluation::new(&parsed.file).expect("valid program");
        let error = solve_prepared_with_attempt_limit(&prepared, 0).expect_err("cap is zero");
        assert_eq!(error.code, "E403");
        assert!(error.message.contains("LM iteration cap reached"));
        assert!(error.message.contains("best-fit residuals worst first"));
    }

    #[test]
    fn direction_angles_match_kernel_normalization_in_relations() {
        let solved = solve_source(
            "locus 0 beta 5\npoint A = point()\npoint B = offset(A, 0, -1)\nnum theta = dir_angle(A, B)\nrel theta == 270deg\nrel dir_angle(A, B) == 270deg\n",
        );
        assert_eq!(
            scene_point(&solved, "B") - scene_point(&solved, "A"),
            crate::geom::Vec2::new(0.0, -1.0)
        );
        assert_eq!(solved.summary.max_normalized_residual, 0.0);
    }

    #[test]
    fn inline_relation_constructors_use_current_domain_rules() {
        let angle = parse(
            "locus 0 beta 5\npoint A = point()\npoint B = offset(A, 1, 0)\npoint C = offset(A, 1, 0)\nrel angle(A, B, C) == 180deg\n",
        );
        let error = solve(&angle.file).expect_err("collinear angle is degenerate");
        assert_eq!(error.code, "E404");
        assert!(error.message.contains("angle"));
        assert!(!error.measured.is_empty());

        let foot = solve_source(
            "locus 0 beta 5\npoint A = point()\npoint B = offset(A, 1, 0)\npoint P = offset(A, 2, 1)\nsegment ab = seg(A, B)\nrel coincident(foot(P, ab), B)\n",
        );
        assert_eq!(
            scene_point(&foot, "B"),
            scene_point(&foot, "A") + crate::geom::Vec2::new(1.0, 0.0)
        );
    }

    #[test]
    fn differentiated_primals_preserve_kernel_arithmetic_order() {
        let solved = solve_source(
            "locus 0 beta 5\npoint P = point()\npoint C = offset(P, 1.1, 0)\npoint reflected = reflect(P, C)\npoint translated = translate(C, P, C)\nnum quotient_delta = (3 / 5 - 0.6) * 100000000\nnum reflect_delta = hdist(reflected, translated) * 100000000\nrel quotient_delta == 0\nrel (3 / 5 - 0.6) * 100000000 == 0\nrel reflect_delta == 0\n",
        );
        assert_eq!(solved.summary.max_normalized_residual, 0.0);
    }

    #[test]
    fn nonsmooth_derivatives_remain_seed_selected_after_crossing() {
        let crossing = ad::Jet {
            value: -2.0,
            seed_value: 2.0,
            gradient: vec![1.0],
        };
        let zero = ad::Jet::constant(0.0, 1);
        let absolute = crossing.abs();
        assert_eq!(absolute.value, 2.0);
        assert_eq!(absolute.gradient, vec![1.0]);
        let maximum = crossing.max(&zero);
        assert_eq!(maximum.value, 0.0);
        assert_eq!(maximum.gradient, vec![1.0]);
        let minimum = crossing.min(&zero);
        assert_eq!(minimum.value, -2.0);
        assert_eq!(minimum.gradient, vec![0.0]);
    }

    #[test]
    fn conflicting_subset_is_isolated_and_all_residuals_are_reported() {
        let conflicting =
            parse("locus 0 beta 5\nnum x = free 0\nrel x == 0\nrel x == 1\nrel x == 2\n");
        let error = solve(&conflicting.file).expect_err("three values conflict");
        assert_eq!(error.code, "E402");
        assert!(
            error
                .message
                .contains("minimal infeasible subset: lines 4, 5"),
            "{}",
            error.message
        );

        let mut source = String::from("locus 0 beta 5\nnum x = free 0\n");
        for value in 0..10 {
            writeln!(source, "rel x == {value}").expect("write relation");
        }
        let parsed = parse(&source);
        let error = solve(&parsed.file).expect_err("ten values conflict");
        assert_eq!(error.code, "E402");
        assert!(error.message.matches(" residual ").count() >= 10);
    }
}
