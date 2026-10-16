//! Opt-in render snapshots, isolated to the collecting thread and scope.
use crate::ResolvedScene;
use serde_json::Value;
use std::cell::RefCell;

thread_local! {
    static STEPS: RefCell<Option<Vec<Step>>> = const { RefCell::new(None) };
}

/// One completed pipeline operation or numerical trial.
#[derive(Debug)]
pub struct Step {
    /// Stable stage name.
    pub name: String,
    /// Stage-specific result, including errors and numerical diagnostics.
    pub data: Value,
    /// Geometry at this step, when available.
    pub scene: Option<ResolvedScene>,
}

struct Collection(Option<Vec<Step>>);
impl Drop for Collection {
    fn drop(&mut self) {
        STEPS.with(|steps| *steps.borrow_mut() = self.0.take());
    }
}

/// Collects snapshots, restoring any enclosing collection even on unwinding.
pub fn collect<T>(operation: impl FnOnce() -> T) -> (T, Vec<Step>) {
    let _guard = Collection(STEPS.with(|steps| steps.replace(Some(Vec::new()))));
    let result = operation();
    let steps = STEPS.with(|steps| steps.take().unwrap_or_default());
    (result, steps)
}

/// Builds a snapshot only when collection is enabled. Snapshot work must be observational.
pub fn record(name: &str, snapshot: impl FnOnce() -> (Value, Option<ResolvedScene>)) {
    if !STEPS.with(|steps| steps.borrow().is_some()) {
        return;
    }
    let (data, scene) = snapshot();
    STEPS.with(|steps| {
        if let Some(steps) = steps.borrow_mut().as_mut() {
            steps.push(Step {
                name: name.to_owned(),
                data,
                scene,
            });
        }
    });
}

#[cfg(test)]
mod tests {
    use super::{collect, record};
    use serde_json::json;

    #[test]
    fn lazy_and_nested_collections() {
        record("disabled", || {
            unreachable!("disabled snapshots must be lazy")
        });
        let ((), outer) = collect(|| {
            record("before", || (json!(1), None));
            let ((), inner) = collect(|| record("inner", || (json!(2), None)));
            assert_eq!(inner.len(), 1);
            record("after", || (json!(3), None));
        });
        assert_eq!(
            outer.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(),
            ["before", "after"]
        );
    }
}
