//! Document symbols for every recovered top-level statement.

use locus_syntax::{Kind, Statement};
use lsp_types::{
    DocumentSymbol, DocumentSymbolParams, DocumentSymbolResponse, Location, SymbolInformation,
    SymbolKind,
};

use crate::state::State;

#[allow(
    deprecated,
    reason = "LSP retains this field for protocol compatibility"
)]
pub(crate) fn handle(
    state: &State,
    params: &DocumentSymbolParams,
) -> Option<DocumentSymbolResponse> {
    let document = state.document(&params.text_document.uri)?;
    let analysis = state.analysis(&params.text_document.uri)?;
    if state.hierarchical_symbols() {
        let symbols = analysis
            .file
            .statements
            .iter()
            .map(|statement| nested_symbol(document, statement))
            .collect();
        Some(DocumentSymbolResponse::Nested(symbols))
    } else {
        let symbols = analysis
            .file
            .statements
            .iter()
            .map(|statement| flat_symbol(document, statement))
            .collect();
        Some(DocumentSymbolResponse::Flat(symbols))
    }
}

#[allow(
    deprecated,
    reason = "LSP retains this field for protocol compatibility"
)]
fn nested_symbol(document: &crate::document::Document, statement: &Statement) -> DocumentSymbol {
    let (name, detail, kind, selection) = symbol_parts(statement);
    DocumentSymbol {
        name,
        detail: Some(detail),
        kind,
        tags: None,
        deprecated: None,
        range: document.span_to_range(statement.span()),
        selection_range: document.span_to_range(selection),
        children: None,
    }
}

#[allow(
    deprecated,
    reason = "LSP retains this field for protocol compatibility"
)]
fn flat_symbol(document: &crate::document::Document, statement: &Statement) -> SymbolInformation {
    let (name, _, kind, _) = symbol_parts(statement);
    SymbolInformation {
        name,
        kind,
        tags: None,
        deprecated: None,
        location: Location::new(
            document.uri().clone(),
            document.span_to_range(statement.span()),
        ),
        container_name: None,
    }
}

fn symbol_parts(statement: &Statement) -> (String, String, SymbolKind, locus_syntax::Span) {
    match statement {
        Statement::Op(op) => (
            op.id.name.clone(),
            op.kind.as_str().to_owned(),
            symbol_kind(op.kind),
            op.id.span,
        ),
        Statement::Rel(value) => (
            "relation".to_owned(),
            "rel".to_owned(),
            SymbolKind::OPERATOR,
            value.span,
        ),
        Statement::Assert(value) => (
            "assertion".to_owned(),
            "assert".to_owned(),
            SymbolKind::BOOLEAN,
            value.span,
        ),
        Statement::Meta(value) => (
            "meta".to_owned(),
            "metadata".to_owned(),
            SymbolKind::NAMESPACE,
            value.span,
        ),
        Statement::Style(value) => (
            value.name.name.clone(),
            "style".to_owned(),
            SymbolKind::KEY,
            value.name.span,
        ),
        Statement::Class(value) => (
            value.name.name.clone(),
            "style class".to_owned(),
            SymbolKind::CLASS,
            value.name.span,
        ),
    }
}

const fn symbol_kind(kind: Kind) -> SymbolKind {
    match kind {
        Kind::Num => SymbolKind::NUMBER,
        Kind::Point => SymbolKind::CONSTANT,
        Kind::Label | Kind::TextBlock => SymbolKind::STRING,
        Kind::Line
        | Kind::Ray
        | Kind::Segment
        | Kind::Circle
        | Kind::Arc
        | Kind::Polygon
        | Kind::Rect
        | Kind::Diamond
        | Kind::Arrow
        | Kind::Tick
        | Kind::RightAngle => SymbolKind::OBJECT,
    }
}
