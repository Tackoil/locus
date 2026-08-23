//! Syntax frontend for the Locus language.
//!
//! This crate deliberately stops at syntax and source-level validation. Geometry evaluation,
//! dimensional inference, formatting, and JSON interchange belong to downstream phases.

pub mod ast;
pub mod diag;
pub mod lexer;
pub mod parser;
pub mod span;
pub mod vocabulary;

pub use ast::*;
pub use diag::Diagnostic;
pub use lexer::{LexOutput, Token, TokenKind, lex};
pub use parser::{ParseOutput, parse};
pub use span::Span;
pub use vocabulary::{
    BUILTIN_ATTR_NAMES, CONSTRUCTOR_NAMES, KIND_NAMES, NUMERIC_FUNCTION_NAMES, PREDICATE_NAMES,
    PROP_NAMES, PROPERTY_NAMES, STYLE_PROP_NAMES, UNIT_NAMES,
};
