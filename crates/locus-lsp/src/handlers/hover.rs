//! Hover documentation for identifiers and constructors.

use std::fmt::Write as _;

use locus_syntax::{CONSTRUCTOR_SIGNATURES, File, Statement};
use lsp_types::{Hover, HoverContents, HoverParams, MarkupContent, MarkupKind};

use super::scope;
use super::target::{Target, at, class_definition};
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
            let value = scope::resolve(&analysis.file, &id.name, byte)?;
            (binding_hover(document, value, markdown), id.span)
        }
        Target::Member(expr, id) => {
            let value = scope::resolve(&analysis.file, &scope::qualified_name(expr)?, byte)?;
            (binding_hover(document, value, markdown), id.span)
        }
        Target::Class(id) => (
            style_hover(document, &analysis.file, id, byte, markdown)?,
            id.span,
        ),
        Target::Callable(id) => {
            let signature = callable_signature(&analysis.file, &id.name)?;
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

fn style_hover(
    document: &crate::document::Document,
    file: &File,
    id: &locus_syntax::Ident,
    byte: usize,
    markdown: bool,
) -> Option<String> {
    if let Some(value @ scope::Binding::Parameter(_)) = scope::resolve(file, &id.name, byte) {
        return Some(binding_hover(document, value, markdown));
    }
    let class = class_definition(file, &id.name)?;
    Some(class_hover(
        &id.name,
        document.source_for_span(class.span).trim(),
        markdown,
    ))
}

fn binding_hover(
    document: &crate::document::Document,
    value: scope::Binding<'_>,
    markdown: bool,
) -> String {
    if let scope::Binding::Object(op) = value {
        return object_hover(document, op, markdown);
    }
    let signature = format!("{} {}", value.detail(), value.id().name);
    let source = document.source_for_span(value.span()).trim();
    if markdown {
        format!("`{signature}`\n\n```locus\n{source}\n```")
    } else {
        format!("{signature}\n\n{source}")
    }
}

fn callable_signature(file: &File, name: &str) -> Option<String> {
    if let Some(function) = file
        .statements
        .iter()
        .find_map(|statement| match statement {
            Statement::Fn(function) if function.name.name == name => Some(function),
            _ => None,
        })
    {
        let params = function
            .params
            .iter()
            .map(|id| id.name.as_str())
            .collect::<Vec<_>>();
        return Some(format!("fn {name}({})", params.join(", ")));
    }
    CONSTRUCTOR_SIGNATURES
        .iter()
        .find_map(|(candidate, signature)| (*candidate == name).then(|| (*signature).to_owned()))
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
