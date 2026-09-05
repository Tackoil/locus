//! Context-aware completion from the shared syntax vocabulary.

use std::collections::BTreeSet;

use locus_syntax::{
    BUILTIN_ATTR_NAMES, CONSTRUCTOR_NAMES, CONSTRUCTOR_SIGNATURES, ENUM_VALUE_NAMES, KIND_NAMES,
    NUMERIC_FUNCTION_NAMES, PREDICATE_NAMES, PROP_NAMES, PROPERTY_NAMES, STATEMENT_NAMES,
    STYLE_PROP_NAMES, Statement, UNIT_NAMES,
};
use lsp_types::{CompletionItem, CompletionItemKind, CompletionParams, CompletionResponse};

use crate::state::{State, defined_ops};

pub(crate) fn handle(state: &State, params: &CompletionParams) -> Option<CompletionResponse> {
    let position = params.text_document_position.position;
    let uri = &params.text_document_position.text_document.uri;
    let document = state.document(uri)?;
    let analysis = state.analysis(uri)?;
    let byte = document.position_to_byte(position)?;
    let prefix_start = identifier_prefix_start(document.text(), byte);
    let prefix = &document.text()[prefix_start..byte];
    let before = document.text()[..prefix_start].chars().next_back();
    let line_start = document.line_start_at(prefix_start);
    let at_statement_start = document.text()[line_start..prefix_start].trim().is_empty();
    let in_attribute = attribute_context(document.text(), line_start, prefix_start);
    let mut items = Vec::new();
    let mut seen = BTreeSet::new();

    if !header_precedes_cursor(document, analysis, byte) {
        extend_header(&mut items, &mut seen, prefix);
    } else if in_attribute {
        extend_attributes(&mut items, &mut seen, prefix, analysis, byte);
    } else if before == Some('.') {
        extend_items(
            &mut items,
            &mut seen,
            PROPERTY_NAMES,
            prefix,
            CompletionItemKind::PROPERTY,
            "geometry property",
        );
    } else if before.is_some_and(|value| value.is_ascii_digit() || value == '.') {
        extend_items(
            &mut items,
            &mut seen,
            UNIT_NAMES,
            prefix,
            CompletionItemKind::UNIT,
            "angle unit",
        );
    } else {
        extend_general(
            &mut items,
            &mut seen,
            prefix,
            at_statement_start,
            analysis,
            byte,
        );
    }
    Some(CompletionResponse::Array(items))
}

fn extend_general(
    items: &mut Vec<CompletionItem>,
    seen: &mut BTreeSet<String>,
    prefix: &str,
    at_statement_start: bool,
    analysis: &crate::state::Analysis,
    cursor_byte: usize,
) {
    if at_statement_start {
        extend_statement_names(items, seen, prefix);
        extend_items(
            items,
            seen,
            KIND_NAMES,
            prefix,
            CompletionItemKind::CLASS,
            "object kind",
        );
    }
    extend_callables(items, seen, prefix);
    extend_values(items, seen, prefix);
    for op in defined_ops(&analysis.file).filter(|op| operation_available(op, cursor_byte)) {
        push_item(
            items,
            seen,
            &op.id.name,
            prefix,
            CompletionItemKind::VARIABLE,
            op.kind.as_str(),
        );
    }
}

fn extend_attributes(
    items: &mut Vec<CompletionItem>,
    seen: &mut BTreeSet<String>,
    prefix: &str,
    analysis: &crate::state::Analysis,
    cursor_byte: usize,
) {
    extend_items(
        items,
        seen,
        BUILTIN_ATTR_NAMES,
        prefix,
        CompletionItemKind::CLASS,
        "style attribute",
    );
    for class in analysis.file.statements.iter().filter_map(|statement| {
        let Statement::Class(class) = statement else {
            return None;
        };
        (class.span.byte_end <= cursor_byte).then_some(class)
    }) {
        push_item(
            items,
            seen,
            &class.name.name,
            prefix,
            CompletionItemKind::CLASS,
            "style class",
        );
    }
}

fn operation_available(op: &locus_syntax::OpStmt, cursor_byte: usize) -> bool {
    op.span.byte_end <= cursor_byte
        || op.where_clauses.iter().any(|clause| {
            clause.span().byte_start <= cursor_byte && cursor_byte <= clause.span().byte_end
        })
}

fn attribute_context(text: &str, line_start: usize, prefix_start: usize) -> bool {
    let before = &text[line_start..prefix_start];
    before.rfind('[') > before.rfind(']')
}

fn header_precedes_cursor(
    document: &crate::document::Document,
    analysis: &crate::state::Analysis,
    cursor_byte: usize,
) -> bool {
    if analysis.file.version != 2 {
        return false;
    }
    let cursor_line = document.byte_to_position(cursor_byte).line as usize;
    first_code_line(document).is_some_and(|header_line| header_line < cursor_line)
}

fn first_code_line(document: &crate::document::Document) -> Option<usize> {
    (0..document.line_count()).find(|line| {
        document
            .line_text(*line)
            .and_then(|text| text.split("//").next())
            .is_some_and(|code| !code.trim().is_empty())
    })
}

fn extend_header(items: &mut Vec<CompletionItem>, seen: &mut BTreeSet<String>, prefix: &str) {
    if let Some(header) = STATEMENT_NAMES.first() {
        push_item(
            items,
            seen,
            header,
            prefix,
            CompletionItemKind::KEYWORD,
            "mandatory file header",
        );
    }
}

fn extend_statement_names(
    items: &mut Vec<CompletionItem>,
    seen: &mut BTreeSet<String>,
    prefix: &str,
) {
    for name in STATEMENT_NAMES.iter().skip(1) {
        push_item(
            items,
            seen,
            name,
            prefix,
            CompletionItemKind::KEYWORD,
            "statement",
        );
    }
}

fn extend_callables(items: &mut Vec<CompletionItem>, seen: &mut BTreeSet<String>, prefix: &str) {
    extend_constructors(items, seen, prefix);
    extend_items(
        items,
        seen,
        NUMERIC_FUNCTION_NAMES,
        prefix,
        CompletionItemKind::FUNCTION,
        "numeric function",
    );
    extend_items(
        items,
        seen,
        PREDICATE_NAMES,
        prefix,
        CompletionItemKind::FUNCTION,
        "predicate",
    );
}

fn extend_values(items: &mut Vec<CompletionItem>, seen: &mut BTreeSet<String>, prefix: &str) {
    extend_items(
        items,
        seen,
        PROP_NAMES,
        prefix,
        CompletionItemKind::PROPERTY,
        "operation property",
    );
    extend_items(
        items,
        seen,
        STYLE_PROP_NAMES,
        prefix,
        CompletionItemKind::PROPERTY,
        "style property",
    );
    extend_items(
        items,
        seen,
        ENUM_VALUE_NAMES,
        prefix,
        CompletionItemKind::ENUM_MEMBER,
        "enum value",
    );
    extend_items(
        items,
        seen,
        BUILTIN_ATTR_NAMES,
        prefix,
        CompletionItemKind::ENUM_MEMBER,
        "style attribute",
    );
    extend_items(
        items,
        seen,
        UNIT_NAMES,
        prefix,
        CompletionItemKind::UNIT,
        "angle unit",
    );
}

fn extend_constructors(items: &mut Vec<CompletionItem>, seen: &mut BTreeSet<String>, prefix: &str) {
    for name in CONSTRUCTOR_NAMES {
        let detail = CONSTRUCTOR_SIGNATURES
            .iter()
            .find_map(|(candidate, signature)| (*candidate == *name).then_some(*signature))
            .unwrap_or("constructor");
        push_item(
            items,
            seen,
            name,
            prefix,
            CompletionItemKind::CONSTRUCTOR,
            detail,
        );
    }
}

fn extend_items(
    items: &mut Vec<CompletionItem>,
    seen: &mut BTreeSet<String>,
    names: &[&str],
    prefix: &str,
    kind: CompletionItemKind,
    detail: &str,
) {
    for name in names {
        push_item(items, seen, name, prefix, kind, detail);
    }
}

fn push_item(
    items: &mut Vec<CompletionItem>,
    seen: &mut BTreeSet<String>,
    name: &str,
    prefix: &str,
    kind: CompletionItemKind,
    detail: &str,
) {
    if name.starts_with(prefix) && seen.insert(name.to_owned()) {
        items.push(CompletionItem {
            label: name.to_owned(),
            kind: Some(kind),
            detail: Some(detail.to_owned()),
            ..CompletionItem::default()
        });
    }
}

fn identifier_prefix_start(text: &str, mut byte: usize) -> usize {
    while byte > 0 {
        let value = text.as_bytes()[byte - 1];
        if !(value.is_ascii_alphanumeric() || matches!(value, b'_' | b'\'')) {
            break;
        }
        byte -= 1;
    }
    byte
}
