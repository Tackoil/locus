//! Preference model and working-set reporting adapters.
use super::ad::Jet;
use super::{
    Analysis, CircleBranch, Equation, EvalError, LmOutcome, PreparedEvaluation, SettlingSummary,
    SourceRelation, Span, UnknownClass, UnknownObject, UnknownSystem, evaluate_state_for, layout,
    linear, settling,
};

pub(super) fn append_domains(
    prepared: &PreparedEvaluation<'_>,
    system: &UnknownSystem,
    z: &[f64],
    rows: &mut Vec<Equation>,
) -> Result<(), EvalError> {
    for object in &system.objects {
        if object.class != UnknownClass::RouteLabelParameter {
            continue;
        }
        let index = object.start;
        let op = prepared
            .registry()
            .iter()
            .find(|op| op.id.name == object.id)
            .ok_or_else(|| EvalError::new("E404", object.span, "parameter owner missing"))?;
        let lower = if op.kind == locus_syntax::Kind::Label {
            if object.dimension == super::ad::NumericDimension::Scalar {
                let region = crate::eval::path_label::region(op)?;
                domain_row(
                    index,
                    region.1 / system.scales[index] - z[index],
                    -1.0,
                    z.len(),
                    rows,
                );
                region.0
            } else {
                prepared.inputs().metrics.clearance
            }
        } else {
            prepared
                .inputs()
                .choices
                .route_min_offsets
                .get(&object.id)
                .copied()
                .unwrap_or(crate::preset::ROUTE_MIN_OFFSET)
                .max(crate::preset::ROUTE_MIN_OFFSET)
        };
        domain_row(
            index,
            z[index] - lower / system.scales[index],
            1.0,
            z.len(),
            rows,
        );
    }
    Ok(())
}

fn domain_row(
    index: usize,
    residual: f64,
    gradient: f64,
    columns: usize,
    rows: &mut Vec<Equation>,
) {
    let mut value = Jet::constant(residual, columns);
    value.gradient[index] = gradient;
    rows.push(Equation {
        residual: value,
        relation: usize::MAX - index,
    });
}

pub(super) fn unknown_preferences(
    prepared: &PreparedEvaluation<'_>,
    system: &UnknownSystem,
    z: &[f64],
    sources: &[SourceRelation<'_>],
) -> Vec<Jet> {
    let has_layout = sources
        .iter()
        .any(|source| layout::is_layout(source.expression));
    let mut values = Vec::new();
    for object in &system.objects {
        // Initial positions are a starting point, not authored layout intent.
        // Keeping them in the objective trades compact gaps for stale seeds.
        if has_layout && is_internal_placement(object) {
            continue;
        }
        for index in object.start..object.start + object.count {
            let target = preference_target(prepared, system, object, index);
            let mut jet = Jet::constant(z[index] - target, z.len());
            jet.gradient[index] = 1.0;
            values.push(jet);
        }
    }
    values
}

fn is_internal_placement(object: &UnknownObject) -> bool {
    object.class == UnknownClass::LabelPlacement
        || (object.class == UnknownClass::Geometric && !object.report_hint)
}

fn preference_target(
    prepared: &PreparedEvaluation<'_>,
    system: &UnknownSystem,
    object: &UnknownObject,
    index: usize,
) -> f64 {
    if object.class == UnknownClass::AutomaticDimension {
        let metrics = prepared.inputs().metrics;
        let size = if index == object.start {
            metrics.default_box_width
        } else {
            metrics.default_box_height
        };
        size / system.scales[index]
    } else {
        system.normalized_hints[index]
    }
}

pub(super) fn equality_analysis(rows: &[Equation], columns: usize) -> Result<Analysis, EvalError> {
    if rows.is_empty() {
        return Ok(linear::unconstrained(columns));
    }
    linear::analyze(
        &rows
            .iter()
            .map(|row| row.residual.gradient.clone())
            .collect::<Vec<_>>(),
    )
    .map_err(|()| EvalError::new("E403", Span::default(), "equality DOF SVD did not converge"))
}

fn constraint_rows(jets: impl Iterator<Item = Jet>) -> settling::Rows {
    let jets = jets.collect::<Vec<_>>();
    settling::Rows {
        values: jets.iter().map(|j| j.value).collect(),
        jacobian: jets.into_iter().map(|j| j.gradient).collect(),
    }
}

pub(super) fn settle(
    prepared: &PreparedEvaluation<'_>,
    system: &UnknownSystem,
    sources: &[SourceRelation<'_>],
    branches: &[Option<CircleBranch>],
    outcome: &mut LmOutcome,
) -> Result<(), EvalError> {
    if !sources.iter().any(|s| layout::is_layout(s.expression))
        && !system
            .objects
            .iter()
            .any(|o| o.class == UnknownClass::RouteLabelParameter)
    {
        return Ok(());
    }
    let result = settling::optimize(&outcome.normalized, |z| {
        let state = evaluate_state_for(prepared, system, sources, branches, z, true).ok()?;
        Some(settling::ConstraintState {
            equalities: constraint_rows(state.equalities.into_iter().map(|r| r.residual)),
            inequalities: constraint_rows(state.inequalities.into_iter().map(|r| r.residual)),
            preferences: constraint_rows(state.preferences.into_iter()),
        })
    });
    outcome.normalized = result.z;
    outcome.state = evaluate_state_for(
        prepared,
        system,
        sources,
        branches,
        &outcome.normalized,
        true,
    )?;
    let report = result.report;
    let active_layout_requirements = report
        .active_inequalities
        .iter()
        .filter_map(|index| {
            let row = outcome.state.inequalities.get(*index)?;
            sources.get(row.relation).map(|s| s.span).or_else(|| {
                system
                    .objects
                    .iter()
                    .find(|object| object.start == usize::MAX - row.relation)
                    .map(|object| object.span)
            })
        })
        .collect();
    let active_rank = linear::rank(&report.active_null).unwrap_or(0);
    outcome.settling = Some(Box::new(SettlingSummary {
        settling_complete: report.complete,
        exit_reason: report.reason.to_owned(),
        projected_gradient: report.projected_gradient,
        steps: report.steps,
        accepted_moves: report.accepted_moves,
        working_set_pivots: report.working_set_pivots,
        model_rebuilds: report.model_rebuilds,
        evaluations: report.evaluations,
        line_search_evaluations: report.line_search_evaluations,
        restoration_steps: report.restoration_steps,
        active_layout_requirements,
        active_remaining: active_rank,
    }));
    Ok(())
}

/// Reject overflowing rows before a live hinge could mask a NaN as zero.
pub(super) fn validate_rows(
    equalities: &[Equation],
    inequalities: &[Equation],
    preferences: &[Jet],
    span: Span,
) -> Result<(), EvalError> {
    let hard = equalities
        .iter()
        .chain(inequalities)
        .map(|row| &row.residual);
    if hard
        .chain(preferences.iter())
        .all(|row| row.value.is_finite() && row.gradient.iter().all(|value| value.is_finite()))
    {
        Ok(())
    } else {
        Err(EvalError::new(
            "E404",
            span,
            "non-finite normalized layout requirement or preference",
        ))
    }
}
