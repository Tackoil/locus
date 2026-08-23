//! Source locations used by tokens, AST nodes, and diagnostics.

use serde::Serialize;

/// A half-open UTF-8 byte range with a one-based starting line and column.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize)]
pub struct Span {
    /// One-based physical source line.
    pub line: usize,
    /// One-based Unicode-scalar source column.
    pub col: usize,
    /// Inclusive UTF-8 byte offset.
    pub byte_start: usize,
    /// Exclusive UTF-8 byte offset.
    pub byte_end: usize,
}

impl Span {
    /// Creates a source span.
    #[must_use]
    pub const fn new(line: usize, col: usize, byte_start: usize, byte_end: usize) -> Self {
        Self {
            line,
            col,
            byte_start,
            byte_end,
        }
    }

    /// Creates an empty span at a source position.
    #[must_use]
    pub const fn empty(line: usize, col: usize, byte: usize) -> Self {
        Self::new(line, col, byte, byte)
    }

    /// Returns the smallest span covering `self` and `other`.
    #[must_use]
    pub fn join(self, other: Self) -> Self {
        if self.byte_start <= other.byte_start {
            Self::new(
                self.line,
                self.col,
                self.byte_start,
                self.byte_end.max(other.byte_end),
            )
        } else {
            other.join(self)
        }
    }
}
