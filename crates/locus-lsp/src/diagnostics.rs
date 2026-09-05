//! Stable Locus diagnostic conversion to LSP diagnostics.

use locus_core::EvalError;
use locus_syntax::Diagnostic as SyntaxDiagnostic;
use lsp_types::{Diagnostic, DiagnosticSeverity, NumberOrString};
use serde_json::json;

use crate::document::Document;

pub(crate) fn syntax(document: &Document, value: &SyntaxDiagnostic) -> Diagnostic {
    Diagnostic {
        range: document.span_to_range(value.span),
        severity: Some(DiagnosticSeverity::ERROR),
        code: Some(NumberOrString::String(value.code.to_owned())),
        code_description: None,
        source: Some("locus".to_owned()),
        message: value.message.clone(),
        related_information: None,
        tags: None,
        data: Some(json!({
            "code": value.code,
            "severity": "error",
            "span": value.span,
            "message": value.message,
            "notes": value.notes,
            "suggestion": value.suggestion,
        })),
    }
}

pub(crate) fn evaluation(document: &Document, value: &EvalError) -> Diagnostic {
    let message = value.object_id.as_ref().map_or_else(
        || value.message.clone(),
        |object| format!("{object}: {}", value.message),
    );
    Diagnostic {
        range: document.span_to_range(value.span),
        severity: Some(DiagnosticSeverity::ERROR),
        code: Some(NumberOrString::String(value.code.to_owned())),
        code_description: None,
        source: Some("locus".to_owned()),
        message,
        related_information: None,
        tags: None,
        data: Some(json!({
            "code": value.code,
            "severity": "error",
            "span": value.span,
            "message": value.message,
            "object_id": value.object_id,
            "measured": value.measured.iter().map(|measurement| json!({
                "name": measurement.name,
                "value": measurement.value,
                "unit": measurement.unit,
            })).collect::<Vec<_>>(),
        })),
    }
}
