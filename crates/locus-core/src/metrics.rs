//! Opt-in wall-clock timings for synchronous pipeline stages.
//!
//! Wrap an operation in [`collect`] to enable recording on the current thread.
//! Nested collections and other threads are isolated. Timings are observational
//! and never enter deterministic scene or solver reports.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Instant;

use serde::Serialize;

thread_local! {
    static CURRENT: RefCell<Option<Rc<RefCell<Collector>>>> = const { RefCell::new(None) };
}

/// Wall-clock measurements for one collected operation.
#[derive(Clone, Debug, Serialize)]
pub struct TimingReport {
    /// Elapsed milliseconds, including uninstrumented work inside the operation.
    pub total_ms: f64,
    /// Individual stage invocations in start order, including repeated passes.
    pub steps: Vec<StepTiming>,
}

/// One inclusive stage duration; child time is included in its parent.
#[derive(Clone, Debug, Serialize)]
pub struct StepTiming {
    /// Stage name or file path for a per-file scope.
    pub name: String,
    /// Index of the enclosing step, or `None` for a root step.
    pub parent: Option<usize>,
    /// Milliseconds since collection began.
    pub start_ms: f64,
    /// Elapsed milliseconds, including nested stages.
    pub duration_ms: f64,
}

struct Collector {
    start: Instant,
    steps: Vec<StepTiming>,
    active: Vec<usize>,
}

struct Collection {
    previous: Option<Rc<RefCell<Collector>>>,
}

impl Drop for Collection {
    fn drop(&mut self) {
        CURRENT.with(|current| current.replace(self.previous.take()));
    }
}

/// Collects timings while preserving the operation's return value, including errors.
///
/// Recording is disabled again on return or unwinding. Child threads must start
/// their own collection. Keep stage guards inside the operation's scope.
pub fn collect<T>(operation: impl FnOnce() -> T) -> (T, TimingReport) {
    let collector = Rc::new(RefCell::new(Collector {
        start: Instant::now(),
        steps: Vec::new(),
        active: Vec::new(),
    }));
    let _collection = Collection {
        previous: CURRENT.with(|current| current.replace(Some(Rc::clone(&collector)))),
    };
    let result = operation();
    let collector = collector.borrow();
    let report = TimingReport {
        total_ms: collector.start.elapsed().as_secs_f64() * 1_000.0,
        steps: collector.steps.clone(),
    };
    (result, report)
}

/// Scope guard that records elapsed time when dropped, including on early return.
#[must_use = "keep the guard alive until the stage finishes"]
pub struct TimingSpan {
    recording: Option<(Rc<RefCell<Collector>>, usize, Instant)>,
}

/// Starts a stage on the current thread, with no clock read or allocation when disabled.
pub fn span(name: impl Into<String>) -> TimingSpan {
    let recording = CURRENT.with(|current| {
        let collector = current.borrow().as_ref().map(Rc::clone)?;
        let start = Instant::now();
        let mut state = collector.borrow_mut();
        let index = state.steps.len();
        let parent = state.active.last().copied();
        let start_ms = start.duration_since(state.start).as_secs_f64() * 1_000.0;
        state.steps.push(StepTiming {
            name: name.into(),
            parent,
            start_ms,
            duration_ms: 0.0,
        });
        state.active.push(index);
        drop(state);
        Some((collector, index, start))
    });
    TimingSpan { recording }
}

impl Drop for TimingSpan {
    fn drop(&mut self) {
        if let Some((collector, index, start)) = &self.recording {
            let duration_ms = start.elapsed().as_secs_f64() * 1_000.0;
            let mut state = collector.borrow_mut();
            state.steps[*index].duration_ms = duration_ms;
            state.active.retain(|active| active != index);
        }
    }
}

/// Times one expression or fallible operation without changing its result.
pub fn measure<T>(name: impl Into<String>, operation: impl FnOnce() -> T) -> T {
    let _stage = span(name);
    operation()
}
