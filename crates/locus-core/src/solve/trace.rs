//! Observational numerical snapshots; evaluation errors remain visible in the dump.
use super::{PreparedEvaluation, SolveState, UnknownSystem, denormalize};
use serde_json::{Value, json};

pub(super) fn iteration(
    name: &str,
    prepared: &PreparedEvaluation<'_>,
    system: &UnknownSystem,
    normalized: &[f64],
    details: impl FnOnce() -> Value,
) {
    crate::trace::record(name, || {
        let actual = denormalize(normalized, &system.scales);
        let geometry = prepared.evaluate_trial(Some(&actual));
        let error = geometry.as_ref().err().map(ToString::to_string);
        (
            json!({"normalized": normalized, "values": actual,
            "details": details(), "geometry_error": error,
            "unknowns": system.objects.iter().map(|object| json!({
                "id": object.id, "start": object.start, "count": object.count,
                "class": format!("{:?}", object.class),
            })).collect::<Vec<_>>()}),
            geometry.ok().map(crate::eval::TrialEvaluation::into_scene),
        )
    });
}

pub(super) fn state(state: &SolveState) -> Value {
    json!({"residuals": state.residuals,
        "max_residual": super::max_residual(&state.residuals)})
}
