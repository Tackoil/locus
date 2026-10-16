//! Deterministic relational solve and DOF reporting.

mod ad;
mod conflict;
mod geometric_projection;
mod init;
mod layout;
mod linear;
mod model;
mod search;
mod settling;
mod trace;
use model::{append_domains, equality_analysis, settle, unknown_preferences};
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
    /// Scalar unknown count across placed geometry, numeric parameters, and layout.
    pub unknowns: usize,
    /// Scalar residual equation count after relation expansion.
    pub equations: usize,
    /// Final normalized Jacobian rank.
    pub rank: usize,
    /// LM attempts, including rejected trials.
    pub attempts: usize,
    /// Continuous candidates visited by bounded outer discrete search.
    pub outer_search_attempts: usize,
    /// Continuous solve/layout passes used to reach measurement agreement.
    pub measurement_passes: usize,
    /// Discrete route candidates validated by the layout passes.
    pub routing_attempts: usize,
    /// Accepted LM steps.
    pub accepted_steps: usize,
    /// Largest absolute normalized final residual.
    pub max_normalized_residual: f64,
    /// Final LM damping value.
    pub final_damping: f64,
    /// Global and per-object null-space analysis.
    pub dof: DofReport,
    /// Explicit tangency branch decisions.
    pub branch_resolutions: Vec<BranchResolution>,
    /// Source and normalized displacements from hints.
    pub hint_displacements: Vec<HintDisplacement>,
    /// Preference settling statistics for layout systems.
    pub settling: Option<Box<SettlingSummary>>,
    /// Relations found redundant at the final Jacobian.
    pub redundant_relations: Vec<RelationReport>,
    /// Per-relation final residuals in source order.
    pub relation_residuals: Vec<RelationReport>,
}

/// Completion and rollback statistics for preference settling.
#[derive(Clone, Debug, PartialEq)]
pub struct SettlingSummary {
    /// True only after projected-gradient convergence.
    pub settling_complete: bool,
    /// Stable termination reason.
    pub exit_reason: String,
    /// Remaining normalized projected-gradient infinity norm.
    pub projected_gradient: f64,
    /// Preference iterations.
    pub steps: usize,
    /// Accepted feasible preference moves, excluding working-set pivots.
    pub accepted_moves: usize,
    /// Boundary releases and zero-distance blocking iterations.
    pub working_set_pivots: usize,
    /// Bounded local curvature-model rebuild attempts.
    pub model_rebuilds: usize,
    /// Total live state evaluations, including curvature probes and restoration.
    pub evaluations: usize,
    /// Trial evaluations during backtracking.
    pub line_search_evaluations: usize,
    /// Feasibility restoration attempts.
    pub restoration_steps: usize,
    /// Source spans of active one-sided layout requirements.
    pub active_layout_requirements: Vec<Span>,
    /// Null-space rank with active requirements included.
    pub active_remaining: usize,
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
    /// Semantic ownership of these unknown components.
    pub class: UnknownClass,
    /// Free declaration identifier.
    pub id: String,
    /// Declaration source span.
    pub span: Span,
    /// Independent global null directions that can move this object.
    pub hint_settled_dofs: usize,
    /// Stable suggestion suitable for an under-constraint diagnostic.
    pub suggestion: String,
}

/// Ownership classes for authored geometry and internal layout unknowns.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnknownClass {
    /// Authored placement or free numeric value.
    Geometric,
    /// Engine-sized box dimensions.
    AutomaticDimension,
    /// Independently placed measured text.
    LabelPlacement,
    /// Internal route or attached-label parameters.
    RouteLabelParameter,
}

impl UnknownClass {
    /// Stable report spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Geometric => "geometric",
            Self::AutomaticDimension => "automatic_dimension",
            Self::LabelPlacement => "label_placement",
            Self::RouteLabelParameter => "route_label_parameter",
        }
    }
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

/// An explicit tangency branch.
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
    class: UnknownClass,
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
    avoidance_axes: BTreeMap<usize, usize>,
}

struct SolveState {
    equations: Vec<Equation>,
    equalities: Vec<Equation>,
    inequalities: Vec<Equation>,
    preferences: Vec<ad::Jet>,
    residuals: Vec<f64>,
    jacobian: Vec<Vec<f64>>,
    analysis: Analysis,
}

/// Solves every free declaration and relation in a parsed file.
pub fn solve(file: &File) -> Result<Solved, EvalError> {
    solve_with_inputs(file, &crate::inputs::EvaluationInputs::default())
}

/// Solves a source program with renderer-supplied measurements.
pub fn solve_with_inputs(
    file: &File,
    inputs: &crate::inputs::EvaluationInputs,
) -> Result<Solved, EvalError> {
    let expanded = crate::program::expand_program(file)?;
    let result = PreparedEvaluation::with_inputs(&expanded.file, inputs, &expanded.collections)
        .and_then(|prepared| solve_prepared(&prepared));
    result.map_err(|error| crate::program::annotate_error(error, &expanded))
}

pub(crate) fn solve_prepared(prepared: &PreparedEvaluation<'_>) -> Result<Solved, EvalError> {
    let _solve = crate::metrics::span("solve");
    solve_prepared_with_attempt_limit(prepared, MAX_ATTEMPTS)
}

fn solve_prepared_with_attempt_limit(
    prepared: &PreparedEvaluation<'_>,
    max_attempts: usize,
) -> Result<Solved, EvalError> {
    let file = prepared.file();
    let sources = relations::collect(file);
    let system = initialize(prepared, &sources)?;
    let branches = relations::select_circle_branches(&sources);
    search::solve(prepared, &system, &sources, &branches, max_attempts)
}

pub(crate) fn layout_seed(prepared: &PreparedEvaluation<'_>) -> Result<ResolvedScene, EvalError> {
    let sources = relations::collect(prepared.file());
    let system = initialize(prepared, &sources)?;
    prepared
        .evaluate_final(&system.hints)
        .map(crate::eval::TrialEvaluation::into_scene)
}

fn initialize(
    prepared: &PreparedEvaluation<'_>,
    sources: &[SourceRelation<'_>],
) -> Result<UnknownSystem, EvalError> {
    let _initialize = crate::metrics::span("initialize");
    let mut result = initialize_at_scale(prepared, sources, 1.0);
    for scale in [0.25, 0.5, 2.0, 4.0, 0.125, 8.0, 16.0] {
        let Err(error) = &result else {
            break;
        };
        if !retryable_initialization(prepared.file(), error) {
            break;
        }
        result = initialize_at_scale(prepared, sources, scale);
    }
    result.map_err(|error| degenerate_seed(prepared.file(), error))
}

fn initialize_at_scale(
    prepared: &PreparedEvaluation<'_>,
    sources: &[SourceRelation<'_>],
    scale: f64,
) -> Result<UnknownSystem, EvalError> {
    let seed_trial = prepared.evaluate_seed(scale)?;
    let mut system = UnknownSystem::from_seed(prepared, &seed_trial)?;
    init::initialize(prepared, &mut system)?;
    let seed_trial = prepared.evaluate_trial(Some(&system.hints))?;
    if sources.iter().any(|s| layout::is_layout(s.expression)) {
        system.diameter = seed_trial.tolerance().diameter();
        system.scales = unknown_scales(&system.objects, &system.hints, system.diameter);
        system.normalized_hints = system
            .hints
            .iter()
            .zip(&system.scales)
            .map(|(value, scale)| value / scale)
            .collect();
    }
    validate_relation_primals(&seed_trial, prepared, sources)?;
    let seed_ad = DifferentiatedFile::evaluate(
        prepared,
        &system.normalized_hints,
        &system.normalized_hints,
        &system.scales,
        &system.dimensions,
        seed_trial.tolerance(),
    )?;
    seed_ad.validate_branch_sensitive_primal(prepared.file(), seed_trial.objects())?;
    Ok(system)
}

struct LmOutcome {
    normalized: Vec<f64>,
    state: SolveState,
    attempts: usize,
    accepted: usize,
    damping: f64,
    settling: Option<Box<SettlingSummary>>,
}

fn run_lm(
    prepared: &PreparedEvaluation<'_>,
    system: &UnknownSystem,
    sources: &[SourceRelation<'_>],
    branches: &[Option<CircleBranch>],
    max_attempts: usize,
    diagnose_failure: bool,
) -> Result<LmOutcome, EvalError> {
    let _solve = crate::metrics::span("levenberg_marquardt");
    let mut normalized = system.normalized_hints.clone();
    let mut state = evaluate_state(prepared, system, sources, branches, &normalized)?;
    let mut damping = INITIAL_DAMPING;
    let mut attempts = 0;
    let mut accepted = 0;
    trace::iteration("solver_start", prepared, system, &normalized, || {
        trace::state(&state)
    });
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
        let hint_null = if geometric_hint_projection(system, sources) {
            geometric_projection::null(system, &state)?
        } else {
            state.analysis.null.clone()
        };
        let trial_vector = trial_step(
            &normalized,
            &primary,
            &hint_null,
            &system.normalized_hints,
            damping,
        );
        attempts += 1;
        let trial = evaluate_state(prepared, system, sources, branches, &trial_vector);
        let trial_accepted = trial
            .as_ref()
            .is_ok_and(|trial| accepted_trial(&state, trial));
        trace::iteration("solver_iteration", prepared, system, &trial_vector, || {
            serde_json::json!({"iteration": attempts, "accepted": trial_accepted,
                "damping": damping, "state": trial.as_ref().ok().map(trace::state),
                "error": trial.as_ref().err().map(ToString::to_string)})
        });
        if trial_accepted {
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
        settling: None,
    })
}

fn geometric_hint_projection(system: &UnknownSystem, sources: &[SourceRelation<'_>]) -> bool {
    sources
        .iter()
        .any(|source| layout::is_layout(source.expression))
        || system
            .objects
            .iter()
            .any(|object| object.class == UnknownClass::RouteLabelParameter)
}

fn evaluate_state(
    prepared: &PreparedEvaluation<'_>,
    system: &UnknownSystem,
    sources: &[SourceRelation<'_>],
    branches: &[Option<CircleBranch>],
    normalized: &[f64],
) -> Result<SolveState, EvalError> {
    evaluate_state_for(prepared, system, sources, branches, normalized, false)
}

fn evaluate_state_for(
    prepared: &PreparedEvaluation<'_>,
    system: &UnknownSystem,
    sources: &[SourceRelation<'_>],
    branches: &[Option<CircleBranch>],
    normalized: &[f64],
    settling_faces: bool,
) -> Result<SolveState, EvalError> {
    let file = prepared.file();
    let actual = denormalize(normalized, &system.scales);
    let trial = prepared.evaluate_trial(Some(&actual))?;
    validate_relation_primals(&trial, prepared, sources)?;
    let differentiated = DifferentiatedFile::evaluate(
        prepared,
        normalized,
        &system.normalized_hints,
        &system.scales,
        &system.dimensions,
        trial.tolerance(),
    )?;
    differentiated.validate_branch_sensitive_primal(file, trial.objects())?;
    let mut equalities = relations::evaluate(&differentiated, sources, branches, system.diameter)?;
    let lower = if settling_faces {
        layout::lower_settling
    } else {
        layout::lower
    };
    let rows = lower(
        &differentiated,
        sources,
        system.diameter,
        &system.avoidance_axes,
    )?;
    equalities.extend(rows.equalities);
    let mut inequalities = rows.inequalities;
    append_domains(prepared, system, normalized, &mut inequalities)?;
    let mut preferences = rows.preferences;
    preferences.extend(unknown_preferences(prepared, system, normalized, sources));
    model::validate_rows(&equalities, &inequalities, &preferences, file.span)?;
    let mut equations = equalities.clone();
    equations.extend(inequalities.iter().map(|row| Equation {
        residual: row.residual.hinge(),
        relation: row.relation,
    }));
    let residuals = equations
        .iter()
        .map(|equation| equation.residual.value)
        .collect::<Vec<_>>();
    let jacobian = equations
        .iter()
        .map(|equation| equation.residual.gradient.clone())
        .collect::<Vec<_>>();
    // Layout Phase 1 computes its geometric null space separately. Computing a
    // second full projector here is unused; equality/working-set ranks are still
    // computed by the normative Jacobi procedure for settling and final reports.
    let analysis = if jacobian.is_empty() || geometric_hint_projection(system, sources) {
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
        equalities,
        inequalities,
        preferences,
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
    let _finish = crate::metrics::span("finalize_solution");
    let actual = denormalize(&outcome.normalized, &system.scales);
    let scene = prepared.evaluate_final(&actual)?.into_scene();
    let relation_residuals = relation_reports(sources, &outcome.state, &system.objects);
    let redundant_relations = redundant_reports(sources, &outcome.state, &system.objects)?;
    let equality_analysis = equality_analysis(&outcome.state.equalities, system.hints.len())?;
    let dof = dof_report(&system.objects, &equality_analysis)?;
    let summary = SolveSummary {
        unknowns: system.hints.len(),
        equations: outcome.state.equations.len(),
        rank: equality_analysis.rank,
        attempts: outcome.attempts,
        outer_search_attempts: 1,
        measurement_passes: 1,
        routing_attempts: 0,
        accepted_steps: outcome.accepted,
        max_normalized_residual: max_residual(&outcome.state.residuals),
        final_damping: outcome.damping,
        settling: outcome.settling.clone(),
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
        prepared: &PreparedEvaluation<'_>,
        seed: &crate::eval::TrialEvaluation,
    ) -> Result<Self, EvalError> {
        let dimensions = dimensions_from_scene(seed.objects());
        let diameter = seed.tolerance().diameter();
        let mut objects = Vec::new();
        let mut hints = Vec::new();
        for op in prepared.registry() {
            add_unknown(op, seed.objects(), &dimensions, &mut objects, &mut hints)?;
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
            avoidance_axes: BTreeMap::new(),
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
    if let ResolvedObject::Annotation(crate::AnnotationSpec::Label {
        path: Some(path), ..
    }) = object
    {
        add_label_parameters(op, path, objects, hints);
        return Ok(());
    }
    if let Some(parameter) = crate::eval::routing::route_parameter(object) {
        hints.push(parameter);
        objects.push(UnknownObject {
            class: UnknownClass::RouteLabelParameter,
            id: op.id.name.clone(),
            span: op.span,
            start,
            count: 1,
            dimension: NumericDimension::Length,
            report_hint: false,
        });
        return Ok(());
    }
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
        _ => {
            let center = placed_shape_center(object, op.span)?;
            hints.extend([center.x, center.y]);
            (2, NumericDimension::Length)
        }
    };
    objects.push(UnknownObject {
        class: if crate::eval::automatic::is_standalone_label(op) {
            UnknownClass::LabelPlacement
        } else {
            UnknownClass::Geometric
        },
        id: op.id.name.clone(),
        span: op.span,
        start,
        count,
        dimension,
        report_hint: matches!(op.ctor, Constructor::FreeNum { .. }),
    });
    if crate::eval::automatic::is_automatic_box(op) {
        let bounds = object
            .bbox()
            .ok_or_else(|| EvalError::new("E404", op.span, "automatic box has no bounds"))?;
        objects.push(UnknownObject {
            id: op.id.name.clone(),
            span: op.span,
            start: hints.len(),
            count: 2,
            dimension: NumericDimension::Length,
            report_hint: false,
            class: UnknownClass::AutomaticDimension,
        });
        hints.extend([bounds.width(), bounds.height()]);
    }
    Ok(())
}

fn add_label_parameters(
    op: &locus_syntax::OpStmt,
    path: &crate::scene::PathLabel,
    objects: &mut Vec<UnknownObject>,
    hints: &mut Vec<f64>,
) {
    for (value, dimension) in [
        (path.parameter, NumericDimension::Scalar),
        (path.offset, NumericDimension::Length),
    ] {
        objects.push(UnknownObject {
            class: UnknownClass::RouteLabelParameter,
            id: op.id.name.clone(),
            span: op.span,
            start: hints.len(),
            count: 1,
            dimension,
            report_hint: false,
        });
        hints.push(value);
    }
}

fn placed_shape_center(
    object: &ResolvedObject,
    span: locus_syntax::Span,
) -> Result<crate::Point, EvalError> {
    match object {
        ResolvedObject::Annotation(crate::AnnotationSpec::Label {
            bounds: Some(bounds),
            ..
        }) => Ok(bounds.center()),
        ResolvedObject::Circle(circle) => Ok(circle.center),
        ResolvedObject::Ellipse(ellipse) => Ok(ellipse.center),
        ResolvedObject::RoundedRect(rect) => Ok(rect.center),
        ResolvedObject::Rect(polygon) | ResolvedObject::Diamond(polygon) => {
            Ok(crate::BBox::for_polygon(polygon).center())
        }
        _ => Err(EvalError::new(
            "E404",
            span,
            "free declaration has an invalid resolved kind",
        )),
    }
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
            class: object.class,
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
    if object.class != UnknownClass::Geometric {
        return String::new();
    }
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
            .equalities
            .iter()
            .filter(|equation| equation.relation == index)
            .map(|equation| equation.residual.gradient.clone())
            .collect::<Vec<_>>();
        if additions.is_empty() {
            continue;
        }
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
    let mut seen = std::collections::BTreeSet::new();
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
        .filter(|id| seen.insert(id.clone()))
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
    search::annotate_failure(
        prepared,
        state,
        sources,
        EvalError::new(
            code,
            ordered
                .first()
                .map_or(Span::default(), |report| report.span),
            format!(
                "{category} relational system: {reason}; best-fit residuals worst first: {details}{infeasible_subset}; spread or correct hints"
            ),
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

fn degenerate_seed(file: &File, mut error: EvalError) -> EvalError {
    // Only geometric preconditions can describe a degenerate seed. Type,
    // property, dependency, and branch-selection errors need source repairs.
    if !retryable_initialization(file, &error) {
        return error;
    }
    error.code = "E404";
    error.message = format!(
        "degenerate solver seed: {}; spread or correct hints",
        error.message
    );
    error
}

fn retryable_initialization(file: &File, error: &EvalError) -> bool {
    !is_property_error(file, error.span)
        && matches!(
            error.code,
            "E201" | "E202" | "E203" | "E204" | "E205" | "E206"
        )
}

fn is_property_error(file: &File, span: Span) -> bool {
    file.statements.iter().any(|statement| {
        let Statement::Op(op) = statement else {
            return false;
        };
        !op.props.items.is_empty()
            && op.props.span.byte_start <= span.byte_start
            && span.byte_end <= op.props.span.byte_end
    })
}

fn dimension_from_core(dimension: Dimension) -> NumericDimension {
    match dimension {
        Dimension::Scalar => NumericDimension::Scalar,
        Dimension::Length => NumericDimension::Length,
        Dimension::Area => NumericDimension::Area,
    }
}
