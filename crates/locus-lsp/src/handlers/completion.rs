//! Context-aware completion from the shared syntax vocabulary.

use std::collections::BTreeSet;

use locus_syntax::{
    BOOLEAN_LITERAL_NAMES, BOOLEAN_OPERATOR_NAMES, BUILTIN_ATTR_NAMES, CONSTRUCTOR_NAMES,
    CONSTRUCTOR_SIGNATURES, FREE_CONSTRUCTOR_NAME, KIND_NAMES, Kind, NUMERIC_FUNCTION_NAMES,
    PREDICATE_NAMES, PROPERTY_NAMES, STATEMENT_NAMES, STYLE_PRESET_NAMES, STYLE_PROP_NAMES,
    Statement, Token, TokenKind, UNIT_NAMES, call_names_for_property, constructor_kind,
    enum_values_for_property, lex,
};
use lsp_types::{
    CompletionItem, CompletionItemKind, CompletionParams, CompletionResponse, CompletionTextEdit,
    TextEdit,
};

use crate::document::Document;
use crate::state::{Analysis, State, defined_ops};

pub(crate) fn handle(state: &State, params: &CompletionParams) -> Option<CompletionResponse> {
    let position = params.text_document_position.position;
    let uri = &params.text_document_position.text_document.uri;
    let document = state.document(uri)?;
    let analysis = state.analysis(uri)?;
    let cursor = document.position_to_byte(position)?;
    let site = completion_site(document, analysis, cursor);
    let mut items = CompletionItems::new(document, site.prefix_start, cursor, site.replace_end);
    if !extend_basic_context(&mut items, &site.context)
        && !extend_structural_context(&mut items, analysis, cursor, &site.context)
    {
        extend_semantic_context(&mut items, analysis, cursor, site.context);
    }
    Some(CompletionResponse::Array(items.finish()))
}

fn extend_basic_context(items: &mut CompletionItems<'_>, context: &Context) -> bool {
    match context {
        Context::Suppressed | Context::None => {}
        Context::Header => extend_header(items),
        Context::Statement => extend_statements(items),
        Context::AngleUnit => items.extend(UNIT_NAMES, CompletionItemKind::UNIT, "angle unit"),
        _ => return false,
    }
    true
}

fn extend_structural_context(
    items: &mut CompletionItems<'_>,
    analysis: &Analysis,
    cursor: usize,
    context: &Context,
) -> bool {
    match context {
        Context::Attribute => extend_attributes(items, analysis, cursor),
        Context::PropertyAccess => items.extend(
            PROPERTY_NAMES,
            CompletionItemKind::PROPERTY,
            "geometry property",
        ),
        Context::StylePreset => items.extend(
            STYLE_PRESET_NAMES,
            CompletionItemKind::ENUM_MEMBER,
            "style preset",
        ),
        Context::PropertyName {
            style,
            kind,
            constructor,
        } => {
            let names = property_names(*style, *kind, constructor.as_deref());
            items.extend(names, CompletionItemKind::PROPERTY, "property");
        }
        Context::Suppressed
        | Context::None
        | Context::Header
        | Context::Statement
        | Context::AngleUnit
        | Context::PropertyValue(_)
        | Context::RootRhs(_)
        | Context::BooleanExpression { .. }
        | Context::Expression => return false,
    }
    true
}

fn property_names(
    style: bool,
    kind: Option<Kind>,
    constructor: Option<&str>,
) -> &'static [&'static str] {
    if style {
        STYLE_PROP_NAMES
    } else {
        kind.map_or(&[], |kind| {
            locus_syntax::prop_names_for_kind(kind, constructor)
        })
    }
}

fn extend_semantic_context(
    items: &mut CompletionItems<'_>,
    analysis: &Analysis,
    cursor: usize,
    context: Context,
) {
    match context {
        Context::PropertyValue(property) => {
            extend_property_values(items, analysis, cursor, &property);
        }
        Context::RootRhs(kind) => extend_root_rhs(items, analysis, cursor, kind),
        Context::BooleanExpression { self_reference } => {
            extend_boolean_expression(items, analysis, cursor, self_reference.as_ref());
        }
        Context::Expression => extend_expression(items, analysis, cursor),
        Context::Suppressed
        | Context::None
        | Context::Header
        | Context::Statement
        | Context::AngleUnit
        | Context::Attribute
        | Context::PropertyAccess
        | Context::StylePreset
        | Context::PropertyName { .. } => {}
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Context {
    Suppressed,
    None,
    Header,
    Statement,
    AngleUnit,
    Attribute,
    PropertyAccess,
    StylePreset,
    PropertyName {
        style: bool,
        kind: Option<Kind>,
        constructor: Option<String>,
    },
    PropertyValue(String),
    RootRhs(Kind),
    BooleanExpression {
        self_reference: Option<(String, Kind)>,
    },
    Expression,
}

struct CompletionSite {
    context: Context,
    prefix_start: usize,
    replace_end: usize,
}

fn completion_site(document: &Document, analysis: &Analysis, cursor: usize) -> CompletionSite {
    let text = document.text();
    let line_start = document.line_start_at(cursor);
    if cursor_is_suppressed(text, line_start, cursor) {
        return site(Context::Suppressed, cursor, cursor);
    }
    let prefix_start = identifier_prefix_start(text, cursor);
    let identifier_end = identifier_suffix_end(text, cursor);
    if !header_precedes_cursor(document, analysis, cursor) {
        return site(Context::Header, prefix_start, identifier_end);
    }
    if let Some(unit_start) = angle_unit_prefix_start(text, line_start, cursor) {
        return site(
            Context::AngleUnit,
            unit_start,
            alphabetic_suffix_end(text, cursor),
        );
    }

    let head = &text[line_start..prefix_start];
    let lexed = lex(head);
    if last_unmatched(&lexed.tokens, &TokenKind::LBracket, &TokenKind::RBracket).is_some() {
        return site(Context::Attribute, prefix_start, identifier_end);
    }
    if text[..prefix_start].ends_with('.') {
        return site(Context::PropertyAccess, prefix_start, identifier_end);
    }
    if head.trim().is_empty() {
        return site(Context::Statement, prefix_start, identifier_end);
    }

    let mut result = classify_line(head, &lexed.tokens, prefix_start, identifier_end);
    if matches!(&result.context, Context::PropertyValue(property) if property == "head") {
        result.prefix_start = punctuation_prefix_start(text, cursor);
        result.replace_end = punctuation_suffix_end(text, cursor);
    }
    result
}

fn classify_line(
    head: &str,
    tokens: &[Token],
    prefix_start: usize,
    replace_end: usize,
) -> CompletionSite {
    let Some(keyword) = first_identifier(tokens) else {
        return site(Context::None, prefix_start, replace_end);
    };
    if keyword == "style" {
        return site(Context::StylePreset, prefix_start, replace_end);
    }
    if let Some(open) = last_unmatched(tokens, &TokenKind::LBrace, &TokenKind::RBrace) {
        return classify_brace(head, tokens, open, keyword, prefix_start, replace_end);
    }
    if matches!(keyword, "rel" | "assert") {
        return site(
            Context::BooleanExpression {
                self_reference: None,
            },
            prefix_start,
            replace_end,
        );
    }
    let Some(kind) = Kind::from_name(keyword) else {
        return site(Context::None, prefix_start, replace_end);
    };
    classify_operation(head, tokens, kind, prefix_start, replace_end)
}

fn classify_brace(
    head: &str,
    tokens: &[Token],
    open: usize,
    keyword: &str,
    prefix_start: usize,
    replace_end: usize,
) -> CompletionSite {
    if identifier_before(tokens, open) == Some("where") {
        return site(
            Context::BooleanExpression {
                self_reference: operation_identity(tokens),
            },
            prefix_start,
            replace_end,
        );
    }
    let property = property_cursor(tokens, open);
    if property.parentheses > 0 {
        return site(Context::Expression, prefix_start, replace_end);
    }
    let Some(colon) = property.colon else {
        return site(
            Context::PropertyName {
                style: keyword == "class",
                kind: Kind::from_name(keyword),
                constructor: operation_constructor(tokens).map(str::to_owned),
            },
            prefix_start,
            replace_end,
        );
    };
    let name = head[property.start..colon].trim().to_owned();
    site(Context::PropertyValue(name), prefix_start, replace_end)
}

fn classify_operation(
    head: &str,
    tokens: &[Token],
    kind: Kind,
    prefix_start: usize,
    replace_end: usize,
) -> CompletionSite {
    let Some(equal) = tokens.iter().find(|token| token.kind == TokenKind::Equal) else {
        return site(Context::None, prefix_start, replace_end);
    };
    let rhs = head[equal.span.byte_end..].trim();
    if rhs.is_empty() {
        site(Context::RootRhs(kind), prefix_start, replace_end)
    } else {
        site(Context::Expression, prefix_start, replace_end)
    }
}

const fn site(context: Context, prefix_start: usize, replace_end: usize) -> CompletionSite {
    CompletionSite {
        context,
        prefix_start,
        replace_end,
    }
}

fn extend_header(items: &mut CompletionItems<'_>) {
    if let Some(header) = STATEMENT_NAMES.first() {
        items.push(header, CompletionItemKind::KEYWORD, "mandatory file header");
    }
}

fn extend_statements(items: &mut CompletionItems<'_>) {
    items.extend(
        STATEMENT_NAMES.get(1..).unwrap_or_default(),
        CompletionItemKind::KEYWORD,
        "statement",
    );
    items.extend(KIND_NAMES, CompletionItemKind::CLASS, "object kind");
}

fn extend_attributes(items: &mut CompletionItems<'_>, analysis: &Analysis, cursor: usize) {
    items.extend(
        BUILTIN_ATTR_NAMES,
        CompletionItemKind::CLASS,
        "style attribute",
    );
    for class in analysis.file.statements.iter().filter_map(|statement| {
        let Statement::Class(class) = statement else {
            return None;
        };
        (class.span.byte_end <= cursor).then_some(class)
    }) {
        items.push(&class.name.name, CompletionItemKind::CLASS, "style class");
    }
}

fn extend_property_values(
    items: &mut CompletionItems<'_>,
    analysis: &Analysis,
    cursor: usize,
    property: &str,
) {
    let values = enum_values_for_property(property);
    let calls = call_names_for_property(property);
    items.extend(values, CompletionItemKind::ENUM_MEMBER, "property value");
    items.extend(calls, CompletionItemKind::FUNCTION, "property value");
    if values.is_empty() && calls.is_empty() {
        extend_expression(items, analysis, cursor);
    }
}

fn extend_root_rhs(
    items: &mut CompletionItems<'_>,
    analysis: &Analysis,
    cursor: usize,
    kind: Kind,
) {
    if matches!(kind, Kind::Point | Kind::Num) {
        items.push(
            FREE_CONSTRUCTOR_NAME,
            CompletionItemKind::CONSTRUCTOR,
            "free declaration",
        );
    }
    if kind == Kind::Num {
        items.extend(
            NUMERIC_FUNCTION_NAMES,
            CompletionItemKind::FUNCTION,
            "numeric function",
        );
        extend_references(items, analysis, cursor);
    } else {
        extend_constructors(items, Some(kind));
    }
}

fn extend_boolean_expression(
    items: &mut CompletionItems<'_>,
    analysis: &Analysis,
    cursor: usize,
    self_reference: Option<&(String, Kind)>,
) {
    items.extend(PREDICATE_NAMES, CompletionItemKind::FUNCTION, "predicate");
    items.extend(
        BOOLEAN_OPERATOR_NAMES,
        CompletionItemKind::KEYWORD,
        "boolean operator",
    );
    extend_expression(items, analysis, cursor);
    if let Some((name, kind)) = self_reference {
        items.push(name, CompletionItemKind::VARIABLE, kind.as_str());
    }
}

fn extend_expression(items: &mut CompletionItems<'_>, analysis: &Analysis, cursor: usize) {
    extend_constructors(items, None);
    items.extend(
        NUMERIC_FUNCTION_NAMES,
        CompletionItemKind::FUNCTION,
        "numeric function",
    );
    items.extend(
        BOOLEAN_LITERAL_NAMES,
        CompletionItemKind::ENUM_MEMBER,
        "boolean literal",
    );
    extend_references(items, analysis, cursor);
}

fn extend_constructors(items: &mut CompletionItems<'_>, result_kind: Option<Kind>) {
    for name in CONSTRUCTOR_NAMES
        .iter()
        .filter(|name| result_kind.is_none_or(|kind| constructor_kind(name) == Some(kind)))
    {
        let detail = CONSTRUCTOR_SIGNATURES
            .iter()
            .find_map(|(candidate, signature)| (*candidate == *name).then_some(*signature))
            .unwrap_or("constructor");
        items.push(name, CompletionItemKind::CONSTRUCTOR, detail);
    }
}

fn extend_references(items: &mut CompletionItems<'_>, analysis: &Analysis, cursor: usize) {
    for op in defined_ops(&analysis.file).filter(|op| operation_available(op, cursor)) {
        items.push(&op.id.name, CompletionItemKind::VARIABLE, op.kind.as_str());
    }
}

fn operation_available(op: &locus_syntax::OpStmt, cursor: usize) -> bool {
    op.span.byte_end <= cursor
        || op
            .where_clauses
            .iter()
            .any(|clause| clause.span().byte_start <= cursor && cursor <= clause.span().byte_end)
}

struct CompletionItems<'a> {
    document: &'a Document,
    prefix: &'a str,
    prefix_start: usize,
    replace_end: usize,
    values: Vec<CompletionItem>,
    seen: BTreeSet<String>,
}

impl<'a> CompletionItems<'a> {
    fn new(document: &'a Document, prefix_start: usize, cursor: usize, replace_end: usize) -> Self {
        Self {
            document,
            prefix: &document.text()[prefix_start..cursor],
            prefix_start,
            replace_end,
            values: Vec::new(),
            seen: BTreeSet::new(),
        }
    }

    fn extend(&mut self, names: &[&str], kind: CompletionItemKind, detail: &str) {
        for name in names {
            self.push(name, kind, detail);
        }
    }

    fn push(&mut self, name: &str, kind: CompletionItemKind, detail: &str) {
        if !name.starts_with(self.prefix) || !self.seen.insert(name.to_owned()) {
            return;
        }
        self.values.push(CompletionItem {
            label: name.to_owned(),
            kind: Some(kind),
            detail: Some(detail.to_owned()),
            text_edit: Some(CompletionTextEdit::Edit(TextEdit {
                range: lsp_types::Range::new(
                    self.document.byte_to_position(self.prefix_start),
                    self.document.byte_to_position(self.replace_end),
                ),
                new_text: name.to_owned(),
            })),
            ..CompletionItem::default()
        });
    }

    fn finish(self) -> Vec<CompletionItem> {
        self.values
    }
}

fn header_precedes_cursor(document: &Document, _analysis: &Analysis, cursor: usize) -> bool {
    let cursor_line = document.byte_to_position(cursor).line as usize;
    first_code_line(document).is_some_and(|header_line| header_line < cursor_line)
}

fn first_code_line(document: &Document) -> Option<usize> {
    (0..document.line_count()).find(|line| {
        document
            .line_text(*line)
            .and_then(|text| text.split("//").next())
            .is_some_and(|code| !code.trim().is_empty())
    })
}

fn first_identifier(tokens: &[Token]) -> Option<&str> {
    tokens.iter().find_map(|token| match &token.kind {
        TokenKind::Ident(name) => Some(name.as_str()),
        _ => None,
    })
}

fn identifier_before(tokens: &[Token], index: usize) -> Option<&str> {
    tokens[..index]
        .iter()
        .rev()
        .find_map(|token| match &token.kind {
            TokenKind::Ident(name) => Some(name.as_str()),
            _ => None,
        })
}

fn operation_identity(tokens: &[Token]) -> Option<(String, Kind)> {
    let kind = first_identifier(tokens).and_then(Kind::from_name)?;
    let name = tokens.iter().skip(1).find_map(|token| match &token.kind {
        TokenKind::Ident(name) => Some(name.clone()),
        _ => None,
    })?;
    Some((name, kind))
}

fn operation_constructor(tokens: &[Token]) -> Option<&str> {
    let equal = tokens
        .iter()
        .position(|token| token.kind == TokenKind::Equal)?;
    tokens[equal + 1..]
        .iter()
        .find_map(|token| match &token.kind {
            TokenKind::Ident(name) => Some(name.as_str()),
            _ => None,
        })
}

fn last_unmatched(tokens: &[Token], open: &TokenKind, close: &TokenKind) -> Option<usize> {
    let mut stack = Vec::new();
    for (index, token) in tokens.iter().enumerate() {
        if &token.kind == open {
            stack.push(index);
        } else if &token.kind == close {
            stack.pop();
        }
    }
    stack.last().copied()
}

struct PropertyCursor {
    start: usize,
    colon: Option<usize>,
    parentheses: usize,
}

fn property_cursor(tokens: &[Token], open: usize) -> PropertyCursor {
    let mut result = PropertyCursor {
        start: tokens[open].span.byte_end,
        colon: None,
        parentheses: 0,
    };
    for token in &tokens[open + 1..] {
        update_property_cursor(&mut result, token);
    }
    result
}

fn update_property_cursor(result: &mut PropertyCursor, token: &Token) {
    match token.kind {
        TokenKind::LParen => result.parentheses += 1,
        TokenKind::RParen => result.parentheses = result.parentheses.saturating_sub(1),
        TokenKind::Comma if result.parentheses == 0 => {
            result.start = token.span.byte_end;
            result.colon = None;
        }
        TokenKind::Colon if result.parentheses == 0 => result.colon = Some(token.span.byte_start),
        _ => {}
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum LexicalMode {
    Code,
    String,
    StringTex,
    Tex,
}

fn cursor_is_suppressed(text: &str, line_start: usize, cursor: usize) -> bool {
    let mut mode = LexicalMode::Code;
    let mut values = text[line_start..cursor].chars().peekable();
    while let Some(value) = values.next() {
        if mode == LexicalMode::Code && value == '/' && values.peek() == Some(&'/') {
            return true;
        }
        if mode != LexicalMode::Code && value == '\\' {
            values.next();
            continue;
        }
        mode = transition_mode(mode, value);
    }
    mode != LexicalMode::Code
}

const fn transition_mode(mode: LexicalMode, value: char) -> LexicalMode {
    match (mode, value) {
        (LexicalMode::Code, '"') | (LexicalMode::StringTex, '$') => LexicalMode::String,
        (LexicalMode::Code, '$') => LexicalMode::Tex,
        (LexicalMode::String, '$') => LexicalMode::StringTex,
        (LexicalMode::String, '"') | (LexicalMode::Tex, '$') => LexicalMode::Code,
        _ => mode,
    }
}

fn angle_unit_prefix_start(text: &str, line_start: usize, cursor: usize) -> Option<usize> {
    let line = &text[line_start..cursor];
    line.char_indices().find_map(|(offset, value)| {
        let start = line_start + offset;
        let boundary = text[..start]
            .chars()
            .next_back()
            .is_none_or(|previous| !is_identifier_continue(previous));
        (boundary && (value.is_ascii_digit() || value == '.'))
            .then(|| numeric_suffix_offset(&text[start..cursor]).map(|suffix| start + suffix))
            .flatten()
    })
}

fn numeric_suffix_offset(value: &str) -> Option<usize> {
    let bytes = value.as_bytes();
    let mut index = consume_digits(bytes, 0);
    let integer_digits = index;
    if bytes.get(index) == Some(&b'.') {
        index += 1;
        index = consume_digits(bytes, index);
    }
    if integer_digits == 0 && index <= 1 {
        return None;
    }
    if matches!(bytes.get(index), Some(b'e' | b'E')) {
        index = consume_exponent(bytes, index)?;
    }
    while matches!(bytes.get(index), Some(b' ' | b'\t')) {
        index += 1;
    }
    let suffix = value.get(index..)?;
    (suffix.chars().all(|ch| ch.is_ascii_alphabetic())
        && UNIT_NAMES.iter().any(|unit| unit.starts_with(suffix)))
    .then_some(index)
}

fn consume_digits(bytes: &[u8], mut index: usize) -> usize {
    while bytes.get(index).is_some_and(u8::is_ascii_digit) {
        index += 1;
    }
    index
}

fn consume_exponent(bytes: &[u8], mut index: usize) -> Option<usize> {
    index += 1;
    if matches!(bytes.get(index), Some(b'+' | b'-')) {
        index += 1;
    }
    let digits = consume_digits(bytes, index);
    (digits > index).then_some(digits)
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

fn identifier_suffix_end(text: &str, mut byte: usize) -> usize {
    while text
        .get(byte..)
        .and_then(|rest| rest.chars().next())
        .is_some_and(is_identifier_continue)
    {
        byte += 1;
    }
    byte
}

fn alphabetic_suffix_end(text: &str, mut byte: usize) -> usize {
    while text
        .as_bytes()
        .get(byte)
        .is_some_and(u8::is_ascii_alphabetic)
    {
        byte += 1;
    }
    byte
}

fn punctuation_prefix_start(text: &str, mut byte: usize) -> usize {
    while byte > 0 && text.as_bytes()[byte - 1].is_ascii_punctuation() {
        let value = text.as_bytes()[byte - 1];
        if !matches!(value, b'<' | b'-' | b'>') {
            break;
        }
        byte -= 1;
    }
    byte
}

fn punctuation_suffix_end(text: &str, mut byte: usize) -> usize {
    while text
        .as_bytes()
        .get(byte)
        .is_some_and(|value| matches!(value, b'<' | b'-' | b'>'))
    {
        byte += 1;
    }
    byte
}

fn is_identifier_continue(value: char) -> bool {
    value.is_ascii_alphanumeric() || matches!(value, '_' | '\'')
}
