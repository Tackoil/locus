//! Stable, serializable frontend diagnostics.

use crate::Span;
use serde::Serialize;

/// A syntax diagnostic. Codes in this crate are stable members of the E1xx family.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Diagnostic {
    /// Stable machine-readable error code.
    pub code: &'static str,
    /// Primary source location.
    pub span: Span,
    /// Human-readable summary.
    pub message: String,
    /// Additional context, in display order.
    pub notes: Vec<String>,
    /// A replacement or action the caller may offer as a quick fix.
    pub suggestion: Option<String>,
}

impl Diagnostic {
    /// Creates a diagnostic without notes or a suggestion.
    #[must_use]
    pub fn new(code: &'static str, span: Span, message: impl Into<String>) -> Self {
        Self {
            code,
            span,
            message: message.into(),
            notes: Vec::new(),
            suggestion: None,
        }
    }

    /// Appends a note.
    #[must_use]
    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.notes.push(note.into());
        self
    }

    /// Adds a suggested replacement.
    #[must_use]
    pub fn with_suggestion(mut self, suggestion: impl Into<String>) -> Self {
        self.suggestion = Some(suggestion.into());
        self
    }
}

/// Finds a close vocabulary entry using deterministic Levenshtein distance.
#[must_use]
pub fn did_you_mean<'a>(value: &str, vocabulary: &'a [&str]) -> Option<&'a str> {
    let value = value.to_ascii_lowercase();
    vocabulary
        .iter()
        .map(|candidate| (*candidate, edit_distance(&value, candidate)))
        .filter(|(candidate, distance)| {
            let limit = if candidate.len().max(value.len()) <= 5 {
                1
            } else {
                3
            };
            *distance <= limit
        })
        .min_by_key(|(candidate, distance)| (*distance, *candidate))
        .map(|(candidate, _)| candidate)
}

fn edit_distance(left: &str, right: &str) -> usize {
    let mut previous: Vec<usize> = (0..=right.len()).collect();
    let mut current = vec![0; right.len() + 1];
    for (left_index, left_byte) in left.bytes().enumerate() {
        current[0] = left_index + 1;
        for (right_index, right_byte) in right.bytes().enumerate() {
            let substitution = usize::from(left_byte != right_byte);
            current[right_index + 1] = (current[right_index] + 1)
                .min(previous[right_index + 1] + 1)
                .min(previous[right_index] + substitution);
        }
        std::mem::swap(&mut previous, &mut current);
    }
    previous[right.len()]
}

#[cfg(test)]
mod tests {
    use super::did_you_mean;

    #[test]
    fn suggests_nearby_names() {
        assert_eq!(
            did_you_mean("incircel", &["circle", "incircle"]),
            Some("incircle")
        );
        assert_eq!(did_you_mean("zzz", &["circle", "incircle"]), None);
    }
}
