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
    pub object_id: Option<String>,
    /// Human-readable failure summary.
    pub message: String,
    /// Offending measured values in deterministic display order.
    pub measured: Vec<MeasuredValue>,
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
            measured: Vec::new(),
        }
    }

    /// Associates the failing declaration id.
    #[must_use]
    pub fn for_object(mut self, id: impl Into<String>) -> Self {
        self.object_id = Some(id.into());
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
