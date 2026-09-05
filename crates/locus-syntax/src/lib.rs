//! Syntax frontend for the Locus language.
//!
//! This crate deliberately stops at syntax and source-level validation. Geometry evaluation
//! belongs to downstream phases; canonical source formatting and syntax-level JSON interchange
//! live here so every caller shares one representation.

pub mod ast;
pub mod diag;
pub mod format;
pub mod json;
pub mod lexer;
pub mod parser;
pub mod span;
pub mod vocabulary;

pub use ast::*;
pub use diag::Diagnostic;
pub use format::{FormatOutput, format, format_file, format_parsed};
pub use json::{Interchange, to_interchange, to_json, to_json_pretty};
pub use lexer::{LexOutput, Token, TokenKind, lex};
pub use parser::{ParseOutput, parse};
pub use span::Span;
pub use vocabulary::{
    BUILTIN_ATTR_NAMES, CONSTRUCTOR_NAMES, CONSTRUCTOR_SIGNATURES, ENUM_VALUE_NAMES, KIND_NAMES,
    NUMERIC_FUNCTION_NAMES, PREDICATE_NAMES, PROP_NAMES, PROPERTY_NAMES, STATEMENT_NAMES,
    STYLE_PROP_NAMES, UNIT_NAMES,
};
