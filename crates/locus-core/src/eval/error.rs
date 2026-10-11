//! Source-anchored evaluator and construction diagnostics.

use std::fmt;

use locus_syntax::Span;
use thiserror::Error;

use crate::geom::ConstructError;
use crate::geom::intersect::IntersectError;

/// One measured value explaining a failed geometric precondition.
#[derive(Clone, Debug, PartialEq)]
pub struct MeasuredValue {
    /// Human-readable quantity name.
    pub name: String,
    /// Finite measured magnitude.
    pub value: f64,
    /// Optional display unit.
    pub unit: Option<&'static str>,
}

/// A typed, source-anchored evaluation diagnostic.
#[derive(Clone, Debug, Error, PartialEq)]
#[error("{code} at {line}:{col}: {message}", line = .span.line, col = .span.col)]
pub struct EvalError {
    /// Stable E2xx code.
    pub code: &'static str,
    /// Most specific failing expression or constructor span.
    pub span: Span,
    /// Declared object being evaluated, when applicable.
    pub object_id: Option<Box<str>>,
    /// Human-readable failure summary.
    pub message: String,
    /// Offending measured values in deterministic display order.
    #[allow(
        clippy::box_collection,
        reason = "rare measured diagnostics are boxed to keep evaluator Result errors compact"
    )]
    pub measured: Box<Vec<MeasuredValue>>,
    /// Function calls that produced this declaration, from outermost to innermost.
    #[allow(
        clippy::box_collection,
        reason = "the rare invocation trace is boxed to keep Result error values compact"
    )]
    pub invocation_chain: Box<Vec<locus_syntax::InvocationOrigin>>,
    /// Rare outer-search details are boxed to keep common errors compact.
    search_failure: Option<Box<SearchFailureMetadata>>,
}

#[derive(Clone, Debug, Default, PartialEq)]
struct SearchFailureMetadata {
    inequalities: Vec<usize>,
    attempts: usize,
}

impl EvalError {
    /// Creates an evaluator error without measured geometry.
    #[must_use]
    pub fn new(code: &'static str, span: Span, message: impl Into<String>) -> Self {
        Self {
            code,
            span,
            object_id: None,
            message: message.into(),
            measured: Box::default(),
            invocation_chain: Box::default(),
            search_failure: None,
        }
    }

    /// Continuous candidates visited by a failed bounded outer search.
    #[must_use]
    pub fn outer_search_attempts(&self) -> usize {
        self.search_failure
            .as_ref()
            .map_or(0, |failure| failure.attempts)
    }

    pub(crate) fn failed_inequalities(&self) -> &[usize] {
        self.search_failure
            .as_ref()
            .map_or(&[], |failure| failure.inequalities.as_slice())
    }

    pub(crate) fn set_failed_inequalities(&mut self, inequalities: Vec<usize>) {
        self.search_failure
            .get_or_insert_with(Box::default)
            .inequalities = inequalities;
    }

    pub(crate) fn set_failed_search_attempts(&mut self, attempts: usize) {
        self.search_failure
            .get_or_insert_with(Box::default)
            .attempts = attempts;
    }

    /// Associates the failing declaration id.
    #[must_use]
    pub fn for_object(mut self, id: impl Into<String>) -> Self {
        self.object_id = Some(id.into().into_boxed_str());
        self
    }

    /// Adds one measured value.
    #[must_use]
    pub fn with_measurement(
        mut self,
        name: impl Into<String>,
        value: f64,
        unit: Option<&'static str>,
    ) -> Self {
        self.measured.push(MeasuredValue {
            name: name.into(),
            value,
            unit,
        });
        self
    }

    /// Wraps a geometry-kernel construction error.
    #[must_use]
    #[allow(
        clippy::needless_pass_by_value,
        reason = "consuming the transient kernel error is the diagnostic boundary"
    )]
    pub fn construction(span: Span, error: ConstructError) -> Self {
        let code = error.code();
        Self::new(code, span, message_without_code(code, &error.to_string()))
    }

    /// Wraps an intersection-kernel error.
    #[must_use]
    #[allow(
        clippy::needless_pass_by_value,
        reason = "consuming the transient kernel error is the diagnostic boundary"
    )]
    pub fn intersection(span: Span, error: IntersectError) -> Self {
        let code = error.code();
        Self::new(code, span, message_without_code(code, &error.to_string()))
    }

    /// Renders measured values as a compact diagnostic note.
    #[must_use]
    pub fn measured_note(&self) -> Option<String> {
        (!self.measured.is_empty()).then(|| {
            self.measured
                .iter()
                .map(MeasuredValue::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        })
    }
}

fn message_without_code<'a>(code: &str, message: &'a str) -> &'a str {
    message
        .strip_prefix(code)
        .and_then(|message| message.strip_prefix(':'))
        .map_or(message, str::trim_start)
}

impl fmt::Display for MeasuredValue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{} {:.6}", self.name, self.value)?;
        if let Some(unit) = self.unit {
            write!(formatter, " {unit}")?;
        }
        Ok(())
    }
}
