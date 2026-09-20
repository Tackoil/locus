//! Hover documentation for identifiers and constructors.

use std::fmt::Write as _;

use locus_syntax::CONSTRUCTOR_SIGNATURES;
use lsp_types::{Hover, HoverContents, HoverParams, MarkupContent, MarkupKind};

use super::target::{Target, at, class_definition, object_definition};
use crate::state::State;

pub(crate) fn handle(state: &State, params: &HoverParams) -> Option<Hover> {
    let uri = &params.text_document_position_params.text_document.uri;
    let document = state.document(uri)?;
    let analysis = state.analysis(uri)?;
    let byte = document.position_to_byte(params.text_document_position_params.position)?;
    let target = at(&analysis.file, byte)?;
    let markdown = state.hover_markup() == &MarkupKind::Markdown;
    let (contents, span) = match target {
        Target::Object(id) => {
            let op = object_definition(&analysis.file, &id.name)?;
            (object_hover(document, op, markdown), id.span)
        }
        Target::Class(id) => {
            let class = class_definition(&analysis.file, &id.name)?;
            let source = document.source_for_span(class.span).trim();
            (class_hover(&id.name, source, markdown), id.span)
        }
        Target::Callable(id) => {
            let signature = CONSTRUCTOR_SIGNATURES
                .iter()
                .find_map(|(name, signature)| (*name == id.name).then_some(*signature))?;
            let value = if markdown {
                format!("`{signature}`")
            } else {
                signature.to_string()
            };
            (value, id.span)
        }
    };
    Some(Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: state.hover_markup().clone(),
            value: contents,
        }),
        range: Some(document.span_to_range(span)),
    })
}

fn object_hover(
    document: &crate::document::Document,
    op: &locus_syntax::OpStmt,
    markdown: bool,
) -> String {
    let mut text = if markdown {
        format!("`{}` **{}**", op.kind.as_str(), op.id.name)
    } else {
        format!("{} {}", op.kind.as_str(), op.id.name)
    };
    let definition = document.source_for_span(op.span).trim();
    if !definition.is_empty() {
        if markdown {
            let _ = write!(text, "\n\n```locus\n{definition}\n```");
        } else {
            let _ = write!(text, "\n\n{definition}");
        }
    }
    text
}

fn class_hover(name: &str, source: &str, markdown: bool) -> String {
    if markdown {
        format!("`class {name}`\n\n```locus\n{source}\n```")
    } else {
        format!("class {name}\n\n{source}")
    }
}
