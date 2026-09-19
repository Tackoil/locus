//! Hand-written recursive-descent parser with physical-line recovery.

use crate::ast::{
    AssertStmt, BinOp, ClassStmt, Constructor, Expr, File, HeadKind, HeadToken, HexColor, Ident,
    Kind, MetaBlock, MetaEntry, NumberExpr, OpStmt, Prop, PropValue, Props, RelStmt, Statement,
    StyleAttr, StyleStmt, TextSegment, TextSegmentKind, TextValue, UnaryOp,
};
use crate::diag::did_you_mean;
use crate::lexer::{LexTextSegment, Token, TokenKind, lex};
use crate::vocabulary::{
    BUILTIN_ATTR_NAMES, CONSTRUCTOR_NAMES, KIND_NAMES, PREDICATE_NAMES, PROP_NAMES, PROPERTY_NAMES,
    STYLE_PROP_NAMES, all_call_names, is_known_call, prop_names_for_kind,
};
use crate::{Diagnostic, Span};
use std::collections::BTreeSet;

type OperatorSpec = (fn(&TokenKind) -> bool, BinOp, u8, u8);

const BINARY_OPERATORS: &[OperatorSpec] = &[
    (is_or, BinOp::Or, 1, 2),
    (is_and, BinOp::And, 2, 3),
    (is_eqeq, BinOp::Eq, 3, 4),
    (is_less, BinOp::Lt, 3, 4),
    (is_greater, BinOp::Gt, 3, 4),
    (is_less_eq, BinOp::Le, 3, 4),
    (is_greater_eq, BinOp::Ge, 3, 4),
    (is_plus, BinOp::Add, 4, 5),
    (is_minus, BinOp::Sub, 4, 5),
    (is_star, BinOp::Mul, 5, 6),
    (is_slash, BinOp::Div, 5, 6),
];

/// Parser result. A recovered AST is always returned, even when diagnostics are present.
#[derive(Clone, Debug, PartialEq)]
pub struct ParseOutput {
    /// Recovered file AST.
    pub file: File,
    /// Lexical, syntactic, and source-level validation diagnostics.
    pub diagnostics: Vec<Diagnostic>,
}

/// Parses a complete Locus source file.
#[must_use]
pub fn parse(source: &str) -> ParseOutput {
    let lexed = lex(source);
    let eof_span = lexed
        .tokens
        .last()
        .map_or(Span::empty(1, 1, source.len()), |token| token.span);
    let (lines, line_ends) = split_lines(lexed.tokens, source);
    let mut output = FileParser::new(lines, line_ends, lexed.diagnostics, source.len()).run();
    if !source.is_empty() && !source.ends_with(['\n', '\r']) {
        output.diagnostics.push(
            Diagnostic::new(
                "E149",
                eof_span,
                "the final statement must be terminated by a newline",
            )
            .with_suggestion("add a newline at end of file"),
        );
    }
    output
}

fn split_lines(tokens: Vec<Token>, source: &str) -> (Vec<Vec<Token>>, Vec<Span>) {
    let mut lines = vec![Vec::new()];
    let mut line_ends = Vec::new();
    for token in tokens {
        if matches!(token.kind, TokenKind::Newline) {
            let physical_end = Span::empty(token.span.line, token.span.col, token.span.byte_start);
            line_ends.push(logical_line_end(&lines, physical_end, source));
            lines.push(Vec::new());
            continue;
        }
        if matches!(token.kind, TokenKind::Eof) {
            line_ends.push(logical_line_end(&lines, token.span, source));
            break;
        }
        if let Some(line) = lines.last_mut() {
            line.push(token);
        }
    }
    debug_assert_eq!(lines.len(), line_ends.len());
    (lines, line_ends)
}

fn logical_line_end(lines: &[Vec<Token>], fallback: Span, source: &str) -> Span {
    let Some(token) = lines.last().and_then(|line| line.last()) else {
        return fallback;
    };
    let width = source[token.span.byte_start..token.span.byte_end]
        .chars()
        .count();
    Span::empty(token.span.line, token.span.col + width, token.span.byte_end)
}

struct FileParser {
    lines: Vec<Vec<Token>>,
    line_ends: Vec<Span>,
    line_index: usize,
    diagnostics: Vec<Diagnostic>,
    source_len: usize,
    defined: Vec<String>,
    classes: Vec<String>,
}

impl FileParser {
    fn new(
        lines: Vec<Vec<Token>>,
        line_ends: Vec<Span>,
        diagnostics: Vec<Diagnostic>,
        source_len: usize,
    ) -> Self {
        Self {
            lines,
            line_ends,
            line_index: 0,
            diagnostics,
            source_len,
            defined: Vec::new(),
            classes: Vec::new(),
        }
    }

    fn run(mut self) -> ParseOutput {
        let version = self.parse_header();
        let mut statements = Vec::new();
        while self.line_index < self.lines.len() {
            if self.lines[self.line_index].is_empty() {
                self.line_index += 1;
                continue;
            }
            if line_starts_with(&self.lines[self.line_index], "meta") {
                if let Some(statement) = self.parse_meta_block() {
                    statements.push(Statement::Meta(statement));
                }
                continue;
            }
            let line = self.lines[self.line_index].clone();
            if line
                .iter()
                .any(|token| matches!(token.kind, TokenKind::Invalid))
            {
                self.line_index += 1;
                continue;
            }
            let mut parser = LineParser::new(&line, self.line_ends[self.line_index]);
            let statement = parser.parse_statement();
            if statement.is_none() && parser.diagnostics.is_empty() {
                parser.error(
                    "E112",
                    parser.current_span(),
                    "expected a statement keyword or object kind",
                );
            }
            if let Some(statement) = statement {
                self.validate_and_record(version, &statement);
                statements.push(statement);
            }
            self.diagnostics.extend(parser.diagnostics);
            self.line_index += 1;
        }
        ParseOutput {
            file: File {
                version,
                statements,
                span: Span::new(1, 1, 0, self.source_len),
            },
            diagnostics: self.diagnostics,
        }
    }

    fn parse_header(&mut self) -> u32 {
        self.skip_empty_lines();
        if self.line_index >= self.lines.len() {
            self.diagnostics.push(Diagnostic::new(
                "E110",
                Span::empty(1, 1, 0),
                "missing mandatory `locus 3` header",
            ));
            return 0;
        }
        if !line_starts_with(&self.lines[self.line_index], "locus") {
            let span = line_span(&self.lines[self.line_index]);
            self.diagnostics.push(
                Diagnostic::new("E110", span, "the first statement must be a `locus` header")
                    .with_suggestion("locus 3"),
            );
            return 0;
        }
        let line = &self.lines[self.line_index];
        let mut parser = LineParser::new(line, self.line_ends[self.line_index]);
        parser.take_keyword("locus");
        let version = parser.parse_header_version();
        let _ = parser.expect_end();
        self.diagnostics.extend(parser.diagnostics);
        self.line_index += 1;
        version
    }

    fn parse_meta_block(&mut self) -> Option<MetaBlock> {
        let opening = &self.lines[self.line_index];
        let open_span = line_span(opening);
        let mut opening_parser = LineParser::new(opening, self.line_ends[self.line_index]);
        opening_parser.take_keyword("meta");
        if opening_parser
            .expect_simple(is_lbrace, "`{` after `meta`")
            .is_err()
            || opening_parser.expect_end().is_err()
        {
            self.diagnostics.extend(opening_parser.diagnostics);
            self.line_index += 1;
            return None;
        }
        self.line_index += 1;
        let mut entries = Vec::new();
        while self.line_index < self.lines.len() {
            let line = &self.lines[self.line_index];
            if line.is_empty() {
                self.line_index += 1;
                continue;
            }
            if matches!(
                line.first().map(|token| &token.kind),
                Some(TokenKind::RBrace)
            ) {
                if line.len() != 1 {
                    self.diagnostics.push(Diagnostic::new(
                        "E151",
                        line_span(line),
                        "metadata closing `}` must be the only token on its line",
                    ));
                }
                let span = open_span.join(line[0].span);
                self.line_index += 1;
                return Some(MetaBlock { entries, span });
            }
            if looks_like_top_level_statement(line) && !looks_like_meta_entry(line) {
                self.diagnostics.push(Diagnostic::new(
                    "E151",
                    open_span,
                    "metadata block is missing its closing `}` line",
                ));
                return Some(MetaBlock {
                    entries,
                    span: open_span,
                });
            }
            let mut parser = LineParser::new(line, self.line_ends[self.line_index]);
            if let Some(entry) = parser.parse_meta_entry() {
                entries.push(entry);
            }
            self.diagnostics.extend(parser.diagnostics);
            self.line_index += 1;
        }
        self.diagnostics.push(Diagnostic::new(
            "E151",
            open_span,
            "metadata block is missing its closing `}` line",
        ));
        Some(MetaBlock {
            entries,
            span: open_span,
        })
    }

    fn skip_empty_lines(&mut self) {
        while self.lines.get(self.line_index).is_some_and(Vec::is_empty) {
            self.line_index += 1;
        }
    }

    fn validate_and_record(&mut self, version: u32, statement: &Statement) {
        validate_version_semantics(version, statement, &mut self.diagnostics);
        self.validate_statement(statement);
        match statement {
            Statement::Op(op) => self.record_name(&op.id),
            Statement::Class(class) => {
                self.record_name(&class.name);
                self.classes.push(class.name.name.clone());
            }
            _ => {}
        }
    }

    fn validate_statement(&mut self, statement: &Statement) {
        match statement {
            Statement::Op(op) => self.validate_op(op),
            Statement::Rel(rel) => {
                self.validate_refs(&rel.expr, None);
                validate_boolean(&rel.expr, &mut self.diagnostics);
                validate_solvable(&rel.expr, &mut self.diagnostics);
                validate_angles(&rel.expr, &mut self.diagnostics);
            }
            Statement::Assert(assertion) => {
                self.validate_refs(&assertion.expr, None);
                validate_boolean(&assertion.expr, &mut self.diagnostics);
                validate_angles(&assertion.expr, &mut self.diagnostics);
            }
            Statement::Class(class) => validate_props(
                &class.props,
                STYLE_PROP_NAMES,
                STYLE_PROP_NAMES,
                &mut self.diagnostics,
            ),
            Statement::Meta(_) | Statement::Style(_) => {}
        }
    }

    fn validate_op(&mut self, op: &OpStmt) {
        validate_constructor_kind(op, &mut self.diagnostics);
        validate_props_for_op(op, &mut self.diagnostics);
        validate_constructor_angles(&op.ctor, &mut self.diagnostics);
        validate_constructor_exprs(&op.ctor, &mut self.diagnostics);
        self.validate_constructor_refs(&op.ctor);
        self.validate_prop_refs(&op.props);
        for clause in &op.where_clauses {
            self.validate_refs(clause, Some(&op.id.name));
            validate_boolean(clause, &mut self.diagnostics);
            validate_solvable(clause, &mut self.diagnostics);
            validate_angles(clause, &mut self.diagnostics);
        }
        if let Some(attr) = &op.attr {
            self.validate_attr(attr);
        }
    }

    fn validate_constructor_refs(&mut self, ctor: &Constructor) {
        match ctor {
            Constructor::Call { args, .. } => {
                for arg in args {
                    self.validate_refs(arg, None);
                }
            }
            Constructor::Expr { expr, .. } => self.validate_refs(expr, None),
            Constructor::FreePoint { x, y, .. } => {
                self.validate_refs(x, None);
                self.validate_refs(y, None);
            }
            Constructor::FreeNum { hint, .. } => self.validate_refs(hint, None),
        }
    }

    fn validate_prop_refs(&mut self, props: &Props) {
        for prop in &props.items {
            if prop_uses_references(&prop.name.name) {
                if let PropValue::Expr(expr) = &prop.value {
                    self.validate_refs(expr, None);
                }
            }
        }
    }

    fn validate_refs(&mut self, expr: &Expr, current: Option<&str>) {
        walk_refs(expr, &mut |ident| {
            if self.defined.iter().any(|name| name == &ident.name)
                || current == Some(ident.name.as_str())
            {
                return;
            }
            let mut diagnostic = Diagnostic::new(
                "E135",
                ident.span,
                format!("unknown or forward reference `{}`", ident.name),
            );
            let vocabulary: Vec<_> = self.defined.iter().map(String::as_str).collect();
            if let Some(suggestion) = did_you_mean(&ident.name, &vocabulary) {
                diagnostic = diagnostic.with_suggestion(suggestion);
            }
            self.diagnostics.push(diagnostic);
        });
    }

    fn validate_attr(&mut self, attr: &StyleAttr) {
        if BUILTIN_ATTR_NAMES.contains(&attr.name.name.as_str())
            || self.classes.iter().any(|name| name == &attr.name.name)
        {
            return;
        }
        let mut vocabulary = BUILTIN_ATTR_NAMES.to_vec();
        vocabulary.extend(self.classes.iter().map(String::as_str));
        let mut diagnostic = Diagnostic::new(
            "E133",
            attr.name.span,
            format!("unknown or forward style class `{}`", attr.name.name),
        );
        if let Some(suggestion) = did_you_mean(&attr.name.name, &vocabulary) {
            diagnostic = diagnostic.with_suggestion(suggestion);
        }
        self.diagnostics.push(diagnostic);
    }

    fn record_name(&mut self, ident: &Ident) {
        if self.defined.iter().any(|name| name == &ident.name) {
            self.diagnostics.push(Diagnostic::new(
                "E134",
                ident.span,
                format!("duplicate identifier `{}`", ident.name),
            ));
        } else {
            self.defined.push(ident.name.clone());
        }
    }
}

struct LineParser<'a> {
    tokens: &'a [Token],
    end_span: Span,
    position: usize,
    diagnostics: Vec<Diagnostic>,
}

impl<'a> LineParser<'a> {
    fn new(tokens: &'a [Token], end_span: Span) -> Self {
        Self {
            tokens,
            end_span,
            position: 0,
            diagnostics: Vec::new(),
        }
    }

    fn parse_statement(&mut self) -> Option<Statement> {
        let Some(first) = self.peek_ident().map(str::to_owned) else {
            self.error(
                "E112",
                self.current_span(),
                "expected a statement keyword or object kind",
            );
            return None;
        };
        match first.as_str() {
            "rel" => self.parse_rel().map(Statement::Rel),
            "assert" => self.parse_assert().map(Statement::Assert),
            "style" => self.parse_style().map(Statement::Style),
            "class" => self.parse_class().map(Statement::Class),
            "meta" => {
                self.error(
                    "E150",
                    self.current_span(),
                    "`meta` must open a multi-line block",
                );
                None
            }
            _ => self.parse_op_or_unknown(),
        }
    }

    fn parse_header_version(&mut self) -> u32 {
        let Some(token) = self.advance().cloned() else {
            self.error(
                "E110",
                self.current_span(),
                "header requires integer version `2` or `3`",
            );
            return 0;
        };
        let TokenKind::Number {
            raw, unit: None, ..
        } = &token.kind
        else {
            self.error(
                "E110",
                token.span,
                "header version must be the integer `2` or `3`",
            );
            return 0;
        };
        if !matches!(raw.as_str(), "2" | "3") {
            self.diagnostics.push(
                Diagnostic::new(
                    "E111",
                    token.span,
                    format!("unsupported Locus format version `{raw}`"),
                )
                .with_suggestion("3"),
            );
            return raw.parse().unwrap_or(0);
        }
        raw.parse().unwrap_or(0)
    }

    fn parse_op_or_unknown(&mut self) -> Option<Statement> {
        let token = self.peek()?.clone();
        let name = self.peek_ident()?.to_owned();
        let Some(kind) = Kind::from_name(&name) else {
            let mut diagnostic = Diagnostic::new(
                "E120",
                token.span,
                format!("unknown statement or object kind `{name}`"),
            );
            if let Some(suggestion) = did_you_mean(&name, KIND_NAMES) {
                diagnostic = diagnostic.with_suggestion(suggestion);
            }
            self.diagnostics.push(diagnostic);
            return None;
        };
        self.advance();
        self.parse_op(kind, token.span).map(Statement::Op)
    }

    fn parse_op(&mut self, kind: Kind, start: Span) -> Option<OpStmt> {
        let id = self.take_ident("object identifier")?;
        self.expect_simple(is_equal, "`=` after object identifier")
            .ok()?;
        let ctor = self.parse_constructor(kind)?;
        let where_clauses = self.parse_where_clause()?;
        let attr = self.parse_attr().ok()?;
        let props = self.parse_optional_props()?;
        self.expect_end().ok()?;
        Some(OpStmt {
            kind,
            id,
            ctor,
            where_clauses,
            attr,
            props,
            span: start.join(self.previous_span()),
        })
    }

    fn parse_constructor(&mut self, kind: Kind) -> Option<Constructor> {
        if self.at_keyword("free") {
            return self.parse_free_constructor(kind);
        }
        if kind == Kind::Num || !self.is_named_call() {
            let expr = self.parse_expr(0)?;
            let span = expr.span();
            return Some(Constructor::Expr {
                expr: Box::new(expr),
                span,
            });
        }
        self.parse_named_constructor()
    }

    fn is_named_call(&self) -> bool {
        matches!(
            self.peek().map(|token| &token.kind),
            Some(TokenKind::Ident(_))
        ) && self
            .tokens
            .get(self.position + 1)
            .is_some_and(|token| matches!(token.kind, TokenKind::LParen))
    }

    fn parse_free_constructor(&mut self, kind: Kind) -> Option<Constructor> {
        let free = self.advance()?.span;
        if !matches!(kind, Kind::Point | Kind::Num) {
            self.error(
                "E123",
                free,
                "only `point` and `num` declarations may use `free`",
            );
            return None;
        }
        if self
            .expect_simple(is_tilde, "mandatory `~` free hint")
            .is_err()
        {
            self.replace_last_code("E124");
            return None;
        }
        if kind == Kind::Point {
            self.parse_free_point(free)
        } else {
            self.parse_free_num(free)
        }
    }

    fn parse_free_point(&mut self, start: Span) -> Option<Constructor> {
        if self
            .expect_simple(is_lparen, "`(` starting point hint")
            .is_err()
        {
            self.replace_last_code("E124");
            return None;
        }
        let x = self.parse_expr(0)?;
        self.expect_simple(is_comma, "`,` between point hint coordinates")
            .ok()?;
        let y = self.parse_expr(0)?;
        let end = self
            .expect_simple(is_rparen, "`)` ending point hint")
            .ok()?;
        Some(Constructor::FreePoint {
            x: Box::new(x),
            y: Box::new(y),
            span: start.join(end),
        })
    }

    fn parse_free_num(&mut self, start: Span) -> Option<Constructor> {
        let starts_with_group = self.at_simple(is_lparen);
        let Some(hint) = self.parse_expr(0) else {
            if starts_with_group && self.at_simple(is_comma) {
                self.replace_last_diagnostic(
                    "E124",
                    "a free `num` requires a scalar hint, not a point-shaped hint",
                );
            }
            return None;
        };
        let span = start.join(hint.span());
        Some(Constructor::FreeNum {
            hint: Box::new(hint),
            span,
        })
    }

    fn parse_named_constructor(&mut self) -> Option<Constructor> {
        let name = self.take_ident("constructor name")?;
        let start = name.span;
        if self
            .expect_simple(is_lparen, "`(` after constructor name")
            .is_err()
        {
            self.replace_last_code("E125");
            return None;
        }
        let (args, end) = self.parse_args()?;
        Some(Constructor::Call {
            name,
            args,
            span: start.join(end),
        })
    }

    fn parse_where_clause(&mut self) -> Option<Vec<Expr>> {
        if !self.take_keyword("where") {
            return Some(Vec::new());
        }
        self.expect_simple(is_lbrace, "`{` after `where`").ok()?;
        let mut clauses = Vec::new();
        loop {
            clauses.push(self.parse_expr(0)?);
            if self.take_simple(is_comma).is_some() {
                continue;
            }
            self.expect_simple(is_rbrace, "`}` ending `where` clause")
                .ok()?;
            return Some(clauses);
        }
    }

    fn parse_attr(&mut self) -> Result<Option<StyleAttr>, ()> {
        let Some(start) = self.take_simple(is_lbracket) else {
            return Ok(None);
        };
        let name = self.take_ident("style class name").ok_or(())?;
        let end = self.expect_simple(is_rbracket, "`]` ending style attribute")?;
        Ok(Some(StyleAttr {
            name,
            span: start.join(end),
        }))
    }

    fn parse_optional_props(&mut self) -> Option<Props> {
        if self.at_simple(is_lbrace) {
            self.parse_props()
        } else {
            Some(Props {
                items: Vec::new(),
                span: Span::empty(
                    self.current_span().line,
                    self.current_span().col,
                    self.current_span().byte_start,
                ),
            })
        }
    }

    fn parse_props(&mut self) -> Option<Props> {
        let start = self.take_simple(is_lbrace)?;
        let mut items = Vec::new();
        if let Some(end) = self.take_simple(is_rbrace) {
            return Some(Props {
                items,
                span: start.join(end),
            });
        }
        loop {
            items.push(self.parse_prop()?);
            if self.take_simple(is_comma).is_some() {
                continue;
            }
            let end = self
                .expect_simple(is_rbrace, "`}` ending property block")
                .ok()?;
            return Some(Props {
                items,
                span: start.join(end),
            });
        }
    }

    fn parse_prop(&mut self) -> Option<Prop> {
        let name = self.take_ident("property name")?;
        self.expect_simple(is_colon, "`:` after property name")
            .ok()?;
        let value = self.parse_prop_value(&name.name)?;
        let span = name.span.join(value.span());
        Some(Prop { name, value, span })
    }

    fn parse_prop_value(&mut self, name: &str) -> Option<PropValue> {
        if name == "head" {
            if let Some(value) = self.parse_head_token() {
                return Some(PropValue::Head(value));
            }
        }
        let token = self.peek()?.clone();
        match token.kind {
            TokenKind::String(segments) => {
                self.advance();
                Some(PropValue::Text(text_value(segments, token.span)))
            }
            TokenKind::Tex(source) => {
                self.advance();
                Some(PropValue::Text(tex_value(source, token.span)))
            }
            TokenKind::HexColor(value) => {
                self.advance();
                Some(PropValue::HexColor(HexColor {
                    value,
                    span: token.span,
                }))
            }
            _ => self.parse_expr(0).map(PropValue::Expr),
        }
    }

    fn parse_head_token(&mut self) -> Option<HeadToken> {
        let token = self.peek()?;
        let kind = match token.kind {
            TokenKind::Arrow => HeadKind::Forward,
            TokenKind::DoubleArrow => HeadKind::Both,
            TokenKind::Minus => HeadKind::None,
            _ => return None,
        };
        let span = self.advance()?.span;
        Some(HeadToken { kind, span })
    }

    fn parse_rel(&mut self) -> Option<RelStmt> {
        let start = self.advance()?.span;
        let expr = self.parse_expr(0)?;
        self.expect_end().ok()?;
        Some(RelStmt {
            span: start.join(expr.span()),
            expr,
        })
    }

    fn parse_assert(&mut self) -> Option<AssertStmt> {
        let start = self.advance()?.span;
        let expr = self.parse_expr(0)?;
        let tolerance = if self.take_keyword("tol") {
            self.parse_tolerance()
        } else {
            None
        };
        self.expect_end().ok()?;
        Some(AssertStmt {
            span: start.join(self.previous_span()),
            expr,
            tolerance,
        })
    }

    fn parse_tolerance(&mut self) -> Option<NumberExpr> {
        let Some(token) = self.peek().cloned() else {
            self.error(
                "E147",
                self.current_span(),
                "assert tolerance must be a unitless number",
            );
            return None;
        };
        let TokenKind::Number {
            raw,
            value,
            unit: None,
        } = token.kind
        else {
            self.advance();
            self.error(
                "E147",
                token.span,
                "assert tolerance must be a unitless number",
            );
            return None;
        };
        self.advance();
        Some(NumberExpr {
            raw,
            value,
            unit: None,
            span: token.span,
        })
    }

    fn parse_style(&mut self) -> Option<StyleStmt> {
        let start = self.advance()?.span;
        let name = self.take_ident("style preset name")?;
        self.expect_end().ok()?;
        Some(StyleStmt {
            span: start.join(name.span),
            name,
        })
    }

    fn parse_class(&mut self) -> Option<ClassStmt> {
        let start = self.advance()?.span;
        let name = self.take_ident("style class name")?;
        let props = self.parse_props()?;
        self.expect_end().ok()?;
        Some(ClassStmt {
            span: start.join(props.span),
            name,
            props,
        })
    }

    fn parse_meta_entry(&mut self) -> Option<MetaEntry> {
        let key = self.take_ident("metadata key")?;
        let Some(token) = self.peek().cloned() else {
            self.error(
                "E150",
                self.current_span(),
                "metadata values must be quoted strings",
            );
            return None;
        };
        let TokenKind::String(segments) = token.kind else {
            self.error("E150", token.span, "metadata values must be quoted strings");
            return None;
        };
        self.advance();
        self.expect_end().ok()?;
        let value = text_value(segments, token.span);
        Some(MetaEntry {
            span: key.span.join(value.span),
            key,
            value,
        })
    }

    fn parse_expr(&mut self, min_binding_power: u8) -> Option<Expr> {
        let mut lhs = self.parse_prefix()?;
        loop {
            if self.at_simple(is_dot) {
                lhs = self.parse_property(lhs)?;
                continue;
            }
            let Some((op, left_power, right_power)) = self.binary_operator() else {
                break;
            };
            if left_power < min_binding_power {
                break;
            }
            if is_comparison(op) && expr_is_comparison(&lhs) {
                self.error(
                    "E145",
                    self.current_span(),
                    "comparison operators cannot be chained",
                );
                return None;
            }
            self.advance();
            let rhs = self.parse_expr(right_power)?;
            let span = lhs.span().join(rhs.span());
            lhs = Expr::Binary {
                op,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
                span,
            };
        }
        Some(lhs)
    }

    fn parse_prefix(&mut self) -> Option<Expr> {
        let token = self.peek()?.clone();
        if matches!(token.kind, TokenKind::Minus) {
            return self.parse_unary(UnaryOp::Neg, 6);
        }
        if matches!(&token.kind, TokenKind::Ident(name) if name == "not") {
            return self.parse_unary(UnaryOp::Not, 3);
        }
        self.parse_atom(&token)
    }

    fn parse_atom(&mut self, token: &Token) -> Option<Expr> {
        match &token.kind {
            TokenKind::Number { raw, value, unit } => {
                self.advance();
                Some(Expr::Number(NumberExpr {
                    value: *value,
                    raw: raw.clone(),
                    unit: *unit,
                    span: token.span,
                }))
            }
            TokenKind::String(segments) => {
                self.advance();
                Some(Expr::String(text_value(segments.clone(), token.span)))
            }
            TokenKind::Tex(source) => {
                self.advance();
                Some(Expr::Tex(tex_value(source.clone(), token.span)))
            }
            TokenKind::Ident(name) => self.parse_ident_expr(name.clone(), token.span),
            TokenKind::LParen => self.parse_group(),
            _ => {
                self.error("E113", token.span, "expected an expression");
                None
            }
        }
    }

    fn parse_unary(&mut self, op: UnaryOp, binding_power: u8) -> Option<Expr> {
        let start = self.advance()?.span;
        let expr = self.parse_expr(binding_power)?;
        let span = start.join(expr.span());
        Some(Expr::Unary {
            op,
            expr: Box::new(expr),
            span,
        })
    }

    fn parse_ident_expr(&mut self, name: String, span: Span) -> Option<Expr> {
        self.advance();
        if name == "free" {
            self.error(
                "E152",
                span,
                "reserved word `free` cannot be used as an identifier",
            );
            return None;
        }
        if name == "true" || name == "false" {
            return Some(Expr::Boolean {
                value: name == "true",
                span,
            });
        }
        let ident = Ident { name, span };
        if self.take_simple(is_lparen).is_none() {
            return Some(Expr::Ref(ident));
        }
        let (args, end) = self.parse_args()?;
        if !is_known_call(&ident.name) {
            let vocabulary = all_call_names();
            let mut diagnostic = Diagnostic::new(
                "E142",
                ident.span,
                format!("unknown function or predicate `{}`", ident.name),
            );
            if let Some(suggestion) = did_you_mean(&ident.name, &vocabulary) {
                diagnostic = diagnostic.with_suggestion(suggestion);
            }
            self.diagnostics.push(diagnostic);
        }
        Some(Expr::Call {
            span: span.join(end),
            name: ident,
            args,
        })
    }

    fn parse_group(&mut self) -> Option<Expr> {
        let start = self.advance()?.span;
        let expr = self.parse_expr(0)?;
        let end = self
            .expect_simple(is_rparen, "`)` ending grouped expression")
            .ok()?;
        Some(Expr::Group {
            expr: Box::new(expr),
            span: start.join(end),
        })
    }

    fn parse_property(&mut self, base: Expr) -> Option<Expr> {
        self.advance();
        let name = self.take_ident("property name")?;
        if !PROPERTY_NAMES.contains(&name.name.as_str()) {
            let mut diagnostic = Diagnostic::new(
                "E148",
                name.span,
                format!("unknown property or anchor `{}`", name.name),
            );
            if let Some(suggestion) = did_you_mean(&name.name, PROPERTY_NAMES) {
                diagnostic = diagnostic.with_suggestion(suggestion);
            }
            self.diagnostics.push(diagnostic);
        }
        let (args, end) = if let Some(start) = self.take_simple(is_lparen) {
            let (args, end) = self.parse_args()?;
            if args.is_empty() {
                self.error(
                    "E153",
                    start.join(end),
                    "property calls require at least one argument",
                );
            }
            (args, end)
        } else {
            (Vec::new(), name.span)
        };
        Some(Expr::Property {
            span: base.span().join(end),
            base: Box::new(base),
            name,
            args,
        })
    }

    fn parse_args(&mut self) -> Option<(Vec<Expr>, Span)> {
        let mut args = Vec::new();
        if let Some(end) = self.take_simple(is_rparen) {
            return Some((args, end));
        }
        loop {
            args.push(self.parse_expr(0)?);
            if self.take_simple(is_comma).is_some() {
                continue;
            }
            let end = self
                .expect_simple(is_rparen, "`)` ending argument list")
                .ok()?;
            return Some((args, end));
        }
    }

    fn binary_operator(&self) -> Option<(BinOp, u8, u8)> {
        let token = self.peek()?;
        BINARY_OPERATORS
            .iter()
            .find_map(|(predicate, op, left, right)| {
                predicate(&token.kind).then_some((*op, *left, *right))
            })
    }

    fn take_ident(&mut self, expected: &str) -> Option<Ident> {
        let token = self.peek()?.clone();
        let TokenKind::Ident(name) = token.kind else {
            self.error("E121", token.span, format!("expected {expected}"));
            return None;
        };
        self.advance();
        if name == "free" {
            self.error(
                "E152",
                token.span,
                "reserved word `free` cannot be used as an identifier",
            );
            return None;
        }
        Some(Ident {
            name,
            span: token.span,
        })
    }

    fn expect_simple(
        &mut self,
        predicate: fn(&TokenKind) -> bool,
        expected: &str,
    ) -> Result<Span, ()> {
        if let Some(span) = self.take_simple(predicate) {
            Ok(span)
        } else {
            self.error("E113", self.current_span(), format!("expected {expected}"));
            Err(())
        }
    }

    fn expect_end(&mut self) -> Result<(), ()> {
        if self.position == self.tokens.len() {
            Ok(())
        } else {
            self.error(
                "E114",
                self.current_span(),
                "unexpected trailing tokens; Locus allows one statement per line",
            );
            Err(())
        }
    }

    fn take_keyword(&mut self, keyword: &str) -> bool {
        if self.at_keyword(keyword) {
            self.position += 1;
            true
        } else {
            false
        }
    }

    fn at_keyword(&self, keyword: &str) -> bool {
        matches!(self.peek().map(|token| &token.kind), Some(TokenKind::Ident(name)) if name == keyword)
    }

    fn peek_ident(&self) -> Option<&str> {
        match &self.peek()?.kind {
            TokenKind::Ident(name) => Some(name),
            _ => None,
        }
    }

    fn take_simple(&mut self, predicate: fn(&TokenKind) -> bool) -> Option<Span> {
        if self.at_simple(predicate) {
            let span = self.tokens[self.position].span;
            self.position += 1;
            Some(span)
        } else {
            None
        }
    }

    fn at_simple(&self, predicate: fn(&TokenKind) -> bool) -> bool {
        self.peek().is_some_and(|token| predicate(&token.kind))
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.position)
    }

    fn advance(&mut self) -> Option<&Token> {
        let token = self.tokens.get(self.position);
        self.position += usize::from(token.is_some());
        token
    }

    fn current_span(&self) -> Span {
        self.peek().map_or(self.end_span, |token| token.span)
    }

    fn previous_span(&self) -> Span {
        self.position
            .checked_sub(1)
            .and_then(|index| self.tokens.get(index))
            .or_else(|| self.tokens.last())
            .map_or(Span::empty(1, 1, 0), |token| token.span)
    }

    fn error(&mut self, code: &'static str, span: Span, message: impl Into<String>) {
        self.diagnostics.push(Diagnostic::new(code, span, message));
    }

    fn replace_last_code(&mut self, code: &'static str) {
        if let Some(diagnostic) = self.diagnostics.last_mut() {
            diagnostic.code = code;
        }
    }

    fn replace_last_diagnostic(&mut self, code: &'static str, message: impl Into<String>) {
        if let Some(diagnostic) = self.diagnostics.last_mut() {
            diagnostic.code = code;
            diagnostic.message = message.into();
        }
    }
}

fn text_value(segments: Vec<LexTextSegment>, span: Span) -> TextValue {
    TextValue {
        segments: segments
            .into_iter()
            .map(|segment| TextSegment {
                kind: segment.kind,
                source: segment.source,
                span: segment.span,
            })
            .collect(),
        span,
    }
}

fn tex_value(source: String, span: Span) -> TextValue {
    TextValue {
        segments: vec![TextSegment {
            kind: TextSegmentKind::Tex,
            source,
            span,
        }],
        span,
    }
}

fn line_starts_with(line: &[Token], keyword: &str) -> bool {
    matches!(line.first().map(|token| &token.kind), Some(TokenKind::Ident(name)) if name == keyword)
}

fn looks_like_meta_entry(line: &[Token]) -> bool {
    matches!(
        line,
        [
            Token {
                kind: TokenKind::Ident(_),
                ..
            },
            Token {
                kind: TokenKind::String(_),
                ..
            },
            ..
        ]
    )
}

fn looks_like_top_level_statement(line: &[Token]) -> bool {
    let Some(Token {
        kind: TokenKind::Ident(first),
        ..
    }) = line.first()
    else {
        return false;
    };
    match first.as_str() {
        "locus" => matches!(
            line.get(1).map(|token| &token.kind),
            Some(TokenKind::Number { unit: None, .. })
        ),
        "meta" => line.get(1).is_some_and(|token| is_lbrace(&token.kind)),
        "style" => matches!(
            line.get(1).map(|token| &token.kind),
            Some(TokenKind::Ident(_))
        ),
        "class" => matches!(
            (line.get(1), line.get(2)),
            (
                Some(Token {
                    kind: TokenKind::Ident(_),
                    ..
                }),
                Some(Token {
                    kind: TokenKind::LBrace,
                    ..
                })
            )
        ),
        "rel" | "assert" => line
            .get(1)
            .is_some_and(|token| starts_expression(&token.kind)),
        _ => matches!(
            (line.get(1), line.get(2)),
            (
                Some(Token {
                    kind: TokenKind::Ident(_),
                    ..
                }),
                Some(Token {
                    kind: TokenKind::Equal,
                    ..
                })
            )
        ),
    }
}

fn starts_expression(kind: &TokenKind) -> bool {
    matches!(
        kind,
        TokenKind::Number { .. }
            | TokenKind::String(_)
            | TokenKind::Tex(_)
            | TokenKind::Ident(_)
            | TokenKind::LParen
            | TokenKind::Minus
    )
}

fn line_span(line: &[Token]) -> Span {
    line.first()
        .zip(line.last())
        .map_or(Span::empty(1, 1, 0), |(first, last)| {
            first.span.join(last.span)
        })
}

fn validate_constructor_kind(op: &OpStmt, diagnostics: &mut Vec<Diagnostic>) {
    let Constructor::Call { name, .. } = &op.ctor else {
        return;
    };
    if !CONSTRUCTOR_NAMES.contains(&name.name.as_str()) {
        let mut diagnostic = Diagnostic::new(
            "E140",
            name.span,
            format!("unknown constructor `{}`", name.name),
        );
        if let Some(suggestion) = did_you_mean(&name.name, CONSTRUCTOR_NAMES) {
            diagnostic = diagnostic.with_suggestion(suggestion);
        }
        diagnostics.push(diagnostic);
        return;
    }
    let allowed = constructors_for_kind(op.kind);
    if !allowed.contains(&name.name.as_str()) {
        diagnostics.push(
            Diagnostic::new(
                "E141",
                name.span,
                format!(
                    "constructor `{}` does not produce kind `{}`",
                    name.name,
                    op.kind.as_str()
                ),
            )
            .with_note(format!("expected one of: {}", allowed.join(", "))),
        );
    }
}

fn constructors_for_kind(kind: Kind) -> &'static [&'static str] {
    const BY_KIND: [&[&str]; 17] = [
        &[
            "point",
            "midpoint",
            "lerp",
            "offset",
            "intersect",
            "foot",
            "project",
            "rotate",
            "reflect",
            "translate",
            "on_circle",
            "extend",
            "centroid",
            "circumcenter",
            "incenter",
            "orthocenter",
            "center",
        ],
        &[],
        &[
            "line",
            "perpendicular",
            "parallel",
            "perp_bisector",
            "angle_bisector",
            "tangent_line",
        ],
        &["ray"],
        &["seg"],
        &[
            "circle",
            "circle_through",
            "circle_diameter",
            "circumcircle",
            "incircle",
        ],
        &["arc", "angle_arc"],
        &["ellipse"],
        &["bspline"],
        &["polygon"],
        &["rect"],
        &["diamond"],
        &["label"],
        &["arrow"],
        &["tick"],
        &["right_angle"],
        &["text_block"],
    ];
    BY_KIND[kind as usize]
}

fn validate_version_semantics(
    version: u32,
    statement: &Statement,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if version == 2 {
        validate_format_two_semantics(statement, diagnostics);
        return;
    }
    if version != 3 {
        return;
    }
    validate_format_three_semantics(statement, diagnostics);
}

fn validate_format_three_semantics(statement: &Statement, diagnostics: &mut Vec<Diagnostic>) {
    match statement {
        Statement::Op(op) if matches!(op.ctor, Constructor::FreePoint { .. }) => diagnostics.push(
            Diagnostic::new(
                "E123",
                op.ctor.span(),
                "format 3 points use parameterless `point()`, not coordinate hints",
            )
            .with_suggestion(format!("point {} = point()", op.id.name)),
        ),
        Statement::Rel(rel) => reject_pin(&rel.expr, diagnostics),
        Statement::Assert(assertion) => reject_pin(&assertion.expr, diagnostics),
        Statement::Op(op) => {
            for clause in &op.where_clauses {
                reject_pin(clause, diagnostics);
            }
        }
        _ => {}
    }
}

fn validate_format_two_semantics(statement: &Statement, diagnostics: &mut Vec<Diagnostic>) {
    match statement {
        Statement::Op(op) => {
            if is_format_three_op(op) {
                diagnostics.push(Diagnostic::new(
                    "E144",
                    op.ctor.span(),
                    "this constructor requires a `locus 3` header",
                ));
            }
            for clause in &op.where_clauses {
                reject_format_three_predicate(clause, diagnostics);
            }
        }
        Statement::Rel(relation) => reject_format_three_predicate(&relation.expr, diagnostics),
        Statement::Assert(assertion) => reject_format_three_predicate(&assertion.expr, diagnostics),
        _ => {}
    }
}

fn is_format_three_op(op: &OpStmt) -> bool {
    if matches!(op.kind, Kind::Ellipse | Kind::Bspline) {
        return true;
    }
    let Constructor::Call { name, args, .. } = &op.ctor else {
        return false;
    };
    matches!(
        (op.kind, name.name.as_str(), args.len()),
        (Kind::Point, "point", 0)
            | (Kind::Circle, "circle", 1)
            | (Kind::Rect | Kind::Diamond, "rect" | "diamond", 2)
    )
}

fn reject_format_three_predicate(expression: &Expr, diagnostics: &mut Vec<Diagnostic>) {
    match expression {
        Expr::Call { name, span, .. } if name.name == "internally_tangent" => {
            diagnostics.push(Diagnostic::new(
                "E144",
                *span,
                "`internally_tangent` requires a `locus 3` header",
            ));
        }
        Expr::Group { expr, .. } | Expr::Unary { expr, .. } => {
            reject_format_three_predicate(expr, diagnostics);
        }
        Expr::Binary { lhs, rhs, .. } => {
            reject_format_three_predicate(lhs, diagnostics);
            reject_format_three_predicate(rhs, diagnostics);
        }
        _ => {}
    }
}

fn reject_pin(expression: &Expr, diagnostics: &mut Vec<Diagnostic>) {
    match expression {
        Expr::Call { name, span, .. } if name.name == "pin" => diagnostics.push(Diagnostic::new(
            "E144",
            *span,
            "`pin` is not available in format 3; express relative geometry with relations",
        )),
        Expr::Group { expr, .. } | Expr::Unary { expr, .. } => reject_pin(expr, diagnostics),
        Expr::Binary { lhs, rhs, .. } => {
            reject_pin(lhs, diagnostics);
            reject_pin(rhs, diagnostics);
        }
        _ => {}
    }
}

fn validate_props_for_op(op: &OpStmt, diagnostics: &mut Vec<Diagnostic>) {
    let allowed = props_for_op(op);
    validate_props(&op.props, allowed, PROP_NAMES, diagnostics);
    for prop in &op.props.items {
        if prop.name.name == "bend" {
            if let PropValue::Expr(expr) = &prop.value {
                require_angle(expr, diagnostics);
            }
        }
    }
}

fn props_for_op(op: &OpStmt) -> &'static [&'static str] {
    prop_names_for_kind(op.kind, constructor_name(&op.ctor))
}

fn validate_props(
    props: &Props,
    allowed: &[&str],
    vocabulary: &[&str],
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut seen = BTreeSet::new();
    for prop in &props.items {
        if !seen.insert(prop.name.name.as_str()) {
            diagnostics.push(Diagnostic::new(
                "E132",
                prop.name.span,
                format!("duplicate property `{}`", prop.name.name),
            ));
        }
        if !vocabulary.contains(&prop.name.name.as_str()) {
            let mut diagnostic = Diagnostic::new(
                "E130",
                prop.name.span,
                format!("unknown property `{}`", prop.name.name),
            );
            if let Some(suggestion) = did_you_mean(&prop.name.name, vocabulary) {
                diagnostic = diagnostic.with_suggestion(suggestion);
            }
            diagnostics.push(diagnostic);
        } else if !allowed.contains(&prop.name.name.as_str()) {
            diagnostics.push(Diagnostic::new(
                "E131",
                prop.name.span,
                format!("property `{}` is not allowed here", prop.name.name),
            ));
        }
    }
}

fn validate_constructor_angles(ctor: &Constructor, diagnostics: &mut Vec<Diagnostic>) {
    let Constructor::Call { name, args, .. } = ctor else {
        return;
    };
    let angle_index = constructor_angle_index(&name.name, args.len());
    if let Some(expr) = angle_index.and_then(|index| args.get(index)) {
        require_angle(expr, diagnostics);
    }
}

fn constructor_angle_index(name: &str, arity: usize) -> Option<usize> {
    match (name, arity) {
        ("rotate", _) => Some(2),
        ("on_circle", _) => Some(1),
        ("ellipse", 4) => Some(3),
        _ => None,
    }
}

fn validate_constructor_exprs(ctor: &Constructor, diagnostics: &mut Vec<Diagnostic>) {
    match ctor {
        Constructor::Call { args, .. } => {
            for arg in args {
                validate_angles(arg, diagnostics);
            }
        }
        Constructor::Expr { expr, .. } => validate_angles(expr, diagnostics),
        Constructor::FreePoint { x, y, .. } => {
            validate_angles(x, diagnostics);
            validate_angles(y, diagnostics);
        }
        Constructor::FreeNum { hint, .. } => validate_angles(hint, diagnostics),
    }
}

fn validate_angles(expr: &Expr, diagnostics: &mut Vec<Diagnostic>) {
    if let Expr::Binary { op, lhs, rhs, .. } = expr {
        validate_binary_angles(*op, lhs, rhs, diagnostics);
        return;
    }
    if let Expr::Call { name, args, .. } = expr {
        validate_call_angles(&name.name, args, diagnostics);
        return;
    }
    if let Expr::Property {
        base, name, args, ..
    } = expr
    {
        validate_property_angles(base, &name.name, args, diagnostics);
        return;
    }
    if let Expr::Group { expr, .. } | Expr::Unary { expr, .. } = expr {
        validate_angles(expr, diagnostics);
    }
}

fn validate_binary_angles(op: BinOp, lhs: &Expr, rhs: &Expr, diagnostics: &mut Vec<Diagnostic>) {
    if matches!(
        op,
        BinOp::Eq | BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge
    ) {
        validate_angle_comparison(lhs, rhs, diagnostics);
    }
    validate_angles(lhs, diagnostics);
    validate_angles(rhs, diagnostics);
}

fn validate_angle_comparison(lhs: &Expr, rhs: &Expr, diagnostics: &mut Vec<Diagnostic>) {
    if is_definitely_angle(lhs) && is_pure_unitless_number(rhs) {
        require_angle(rhs, diagnostics);
    }
    if is_definitely_angle(rhs) && is_pure_unitless_number(lhs) {
        require_angle(lhs, diagnostics);
    }
}

fn validate_call_angles(name: &str, args: &[Expr], diagnostics: &mut Vec<Diagnostic>) {
    if let Some(arg) = angle_argument(name).and_then(|index| args.get(index)) {
        require_angle(arg, diagnostics);
    }
    for arg in args {
        validate_angles(arg, diagnostics);
    }
}

fn validate_property_angles(
    base: &Expr,
    name: &str,
    args: &[Expr],
    diagnostics: &mut Vec<Diagnostic>,
) {
    validate_angles(base, diagnostics);
    // The argument dimension of `.at` is receiver-dependent: angular for
    // circles and ellipses, scalar for B-splines. Typed evaluation diagnoses
    // a missing angle unit once the receiver kind is known.
    let _ = name;
    for arg in args {
        validate_angles(arg, diagnostics);
    }
}

fn angle_argument(name: &str) -> Option<usize> {
    match name {
        "rotate" => Some(2),
        "on_circle" => Some(1),
        _ => None,
    }
}

fn require_angle(expr: &Expr, diagnostics: &mut Vec<Diagnostic>) {
    if is_pure_unitless_number(expr) {
        diagnostics.push(
            Diagnostic::new(
                "E143",
                expr.span(),
                "angle literals require an explicit `deg` or `rad` unit",
            )
            .with_suggestion("add `deg` or `rad`"),
        );
    }
}

fn is_definitely_angle(expr: &Expr) -> bool {
    match expr {
        Expr::Number(number) => number.unit.is_some(),
        Expr::Call { name, .. } => matches!(name.name.as_str(), "angle" | "dir_angle"),
        Expr::Group { expr, .. } | Expr::Unary { expr, .. } => is_definitely_angle(expr),
        Expr::Binary { lhs, rhs, .. } => is_definitely_angle(lhs) || is_definitely_angle(rhs),
        _ => false,
    }
}

fn is_pure_unitless_number(expr: &Expr) -> bool {
    match expr {
        Expr::Number(number) => number.unit.is_none(),
        Expr::Group { expr, .. } | Expr::Unary { expr, .. } => is_pure_unitless_number(expr),
        Expr::Binary {
            op: BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div,
            lhs,
            rhs,
            ..
        } => is_pure_unitless_number(lhs) && is_pure_unitless_number(rhs),
        _ => false,
    }
}

fn validate_boolean(expr: &Expr, diagnostics: &mut Vec<Diagnostic>) {
    if let Expr::Binary { op, lhs, rhs, .. } = expr {
        validate_boolean_binary(*op, lhs, rhs, diagnostics);
        return;
    }
    if let Expr::Unary {
        op: UnaryOp::Not,
        expr,
        ..
    }
    | Expr::Group { expr, .. } = expr
    {
        validate_boolean(expr, diagnostics);
        return;
    }
    if let Expr::Call { name, args, span } = expr {
        validate_predicate_name(name, diagnostics);
        if args.is_empty() && PREDICATE_NAMES.contains(&name.name.as_str()) {
            diagnostics.push(Diagnostic::new(
                "E153",
                *span,
                "relation predicates require at least one argument",
            ));
        }
        return;
    }
    diagnostics.push(Diagnostic::new(
        "E144",
        expr.span(),
        "expected a comparison or relation predicate",
    ));
}

fn validate_boolean_binary(op: BinOp, lhs: &Expr, rhs: &Expr, diagnostics: &mut Vec<Diagnostic>) {
    if matches!(op, BinOp::And | BinOp::Or) {
        validate_boolean(lhs, diagnostics);
        validate_boolean(rhs, diagnostics);
    } else if !matches!(
        op,
        BinOp::Eq | BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge
    ) {
        diagnostics.push(Diagnostic::new(
            "E144",
            lhs.span().join(rhs.span()),
            "expected a comparison or relation predicate",
        ));
    }
}

fn validate_predicate_name(name: &Ident, diagnostics: &mut Vec<Diagnostic>) {
    if PREDICATE_NAMES.contains(&name.name.as_str()) {
        return;
    }
    let mut diagnostic = Diagnostic::new(
        "E144",
        name.span,
        format!("`{}` is not a relation predicate", name.name),
    );
    if let Some(suggestion) = did_you_mean(&name.name, PREDICATE_NAMES) {
        diagnostic = diagnostic.with_suggestion(suggestion);
    }
    diagnostics.push(diagnostic);
}

fn validate_solvable(expr: &Expr, diagnostics: &mut Vec<Diagnostic>) {
    match expr {
        Expr::Binary {
            op: BinOp::And,
            lhs,
            rhs,
            ..
        } => {
            validate_solvable(lhs, diagnostics);
            validate_solvable(rhs, diagnostics);
        }
        Expr::Binary { op: BinOp::Eq, .. } => {}
        Expr::Call { name, .. } if !matches!(name.name.as_str(), "between" | "inside") => {}
        _ => diagnostics.push(
            Diagnostic::new(
                "E146",
                expr.span(),
                "this assert-only boolean form cannot be used in `rel` or `where`",
            )
            .with_note("use it in an `assert` statement instead"),
        ),
    }
}

fn walk_refs(expr: &Expr, visit: &mut impl FnMut(&Ident)) {
    if let Expr::Ref(ident) = expr {
        visit(ident);
        return;
    }
    if let Expr::Call { args, .. } = expr {
        walk_ref_args(args, visit);
        return;
    }
    if let Expr::Property { base, args, .. } = expr {
        walk_refs(base, visit);
        walk_ref_args(args, visit);
        return;
    }
    if let Expr::Group { expr, .. } | Expr::Unary { expr, .. } = expr {
        walk_refs(expr, visit);
        return;
    }
    if let Expr::Binary { lhs, rhs, .. } = expr {
        walk_refs(lhs, visit);
        walk_refs(rhs, visit);
    }
}

fn walk_ref_args(args: &[Expr], visit: &mut impl FnMut(&Ident)) {
    for arg in args {
        walk_refs(arg, visit);
    }
}

fn constructor_name(ctor: &Constructor) -> Option<&str> {
    match ctor {
        Constructor::Call { name, .. } => Some(&name.name),
        _ => None,
    }
}

fn prop_uses_references(name: &str) -> bool {
    matches!(name, "gap" | "bend" | "n" | "size" | "anchor")
}

fn is_comparison(op: BinOp) -> bool {
    matches!(
        op,
        BinOp::Eq | BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge
    )
}

fn expr_is_comparison(expr: &Expr) -> bool {
    matches!(expr, Expr::Binary { op, .. } if is_comparison(*op))
}

fn is_lparen(kind: &TokenKind) -> bool {
    matches!(kind, TokenKind::LParen)
}
fn is_rparen(kind: &TokenKind) -> bool {
    matches!(kind, TokenKind::RParen)
}
fn is_lbrace(kind: &TokenKind) -> bool {
    matches!(kind, TokenKind::LBrace)
}
fn is_rbrace(kind: &TokenKind) -> bool {
    matches!(kind, TokenKind::RBrace)
}
fn is_lbracket(kind: &TokenKind) -> bool {
    matches!(kind, TokenKind::LBracket)
}
fn is_rbracket(kind: &TokenKind) -> bool {
    matches!(kind, TokenKind::RBracket)
}
fn is_comma(kind: &TokenKind) -> bool {
    matches!(kind, TokenKind::Comma)
}
fn is_colon(kind: &TokenKind) -> bool {
    matches!(kind, TokenKind::Colon)
}
fn is_dot(kind: &TokenKind) -> bool {
    matches!(kind, TokenKind::Dot)
}
fn is_equal(kind: &TokenKind) -> bool {
    matches!(kind, TokenKind::Equal)
}
fn is_tilde(kind: &TokenKind) -> bool {
    matches!(kind, TokenKind::Tilde)
}
fn is_or(kind: &TokenKind) -> bool {
    matches!(kind, TokenKind::Ident(name) if name == "or")
}
fn is_and(kind: &TokenKind) -> bool {
    matches!(kind, TokenKind::Ident(name) if name == "and")
}
fn is_eqeq(kind: &TokenKind) -> bool {
    matches!(kind, TokenKind::EqEq)
}
fn is_less(kind: &TokenKind) -> bool {
    matches!(kind, TokenKind::Less)
}
fn is_greater(kind: &TokenKind) -> bool {
    matches!(kind, TokenKind::Greater)
}
fn is_less_eq(kind: &TokenKind) -> bool {
    matches!(kind, TokenKind::LessEq)
}
fn is_greater_eq(kind: &TokenKind) -> bool {
    matches!(kind, TokenKind::GreaterEq)
}
fn is_plus(kind: &TokenKind) -> bool {
    matches!(kind, TokenKind::Plus)
}
fn is_minus(kind: &TokenKind) -> bool {
    matches!(kind, TokenKind::Minus)
}
fn is_star(kind: &TokenKind) -> bool {
    matches!(kind, TokenKind::Star)
}
fn is_slash(kind: &TokenKind) -> bool {
    matches!(kind, TokenKind::Slash)
}

#[cfg(test)]
mod tests {
    use super::parse;
    use crate::ast::{Constructor, Expr, Kind, Statement, TextSegmentKind};

    #[test]
    fn parses_free_where_props_and_mixed_text() {
        let source = r#"locus 2
point A = free ~(0, 0)
point B = free ~(2, 0) where { horizontal(A, B) } { mark: cross }
label text = label(B, "point $B$") { dir: NE }
assert dist(A, B) == 2
"#;
        let output = parse(source);
        assert!(output.diagnostics.is_empty(), "{:#?}", output.diagnostics);
        assert_eq!(output.file.statements.len(), 4);
        assert!(matches!(output.file.statements[1], Statement::Op(_)));
        if let Statement::Op(op) = &output.file.statements[1] {
            assert_eq!(op.kind, Kind::Point);
            assert!(matches!(op.ctor, Constructor::FreePoint { .. }));
            assert_eq!(op.where_clauses.len(), 1);
        }
        assert!(matches!(output.file.statements[2], Statement::Op(_)));
        if let Statement::Op(label) = &output.file.statements[2] {
            assert!(matches!(label.ctor, Constructor::Call { .. }));
            if let Constructor::Call { args, .. } = &label.ctor {
                assert!(matches!(args[1], Expr::String(_)));
                if let Expr::String(text) = &args[1] {
                    assert_eq!(text.segments.len(), 3);
                    assert_eq!(text.segments[1].kind, TextSegmentKind::Tex);
                    assert_eq!(text.segments[2].kind, TextSegmentKind::Plain);
                    assert_eq!(text.segments[2].source, "");
                }
            }
        }
    }

    #[test]
    fn format_versions_keep_their_placement_and_tangency_models_distinct() {
        let legacy = parse(
            "locus 2\npoint P = point()\ncircle c = circle(2)\nrel internally_tangent(c, c)\n",
        );
        assert!(
            legacy
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "E144")
        );

        let current = parse("locus 3\npoint P = free ~(0, 0)\nassert pin(P, 0, 0)\n");
        assert!(
            current
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "E123")
        );
        assert!(
            current
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "E144")
        );
    }

    #[test]
    fn recovers_at_every_bad_physical_line() {
        let source = r"locus 2
piont A = free ~(0, 0)
point A = free ~(0)
point A = free ~(0, 0)
circle c = incircel(A, 2)
point B = free ~(2, 0)
assert horizontal(A, B)
";
        let output = parse(source);
        assert_eq!(output.file.statements.len(), 4);
        let codes: Vec<_> = output.diagnostics.iter().map(|diag| diag.code).collect();
        assert!(codes.contains(&"E120"));
        assert!(codes.contains(&"E113"));
        assert!(codes.contains(&"E140"));
        assert!(matches!(
            output.file.statements.last(),
            Some(Statement::Assert(_))
        ));
    }

    #[test]
    fn enforces_angle_units_at_known_angle_sites() {
        let output = parse(
            "locus 2\npoint A = free ~(0, 0)\npoint O = free ~(1, 0)\npoint B = rotate(A, O, 60)\nassert angle(A, O, B) == 90\n",
        );
        assert_eq!(
            output
                .diagnostics
                .iter()
                .filter(|diagnostic| diagnostic.code == "E143")
                .count(),
            2
        );
    }

    #[test]
    fn diagnoses_bad_lines_and_missing_final_newline() {
        let output = parse("locus 2\n}\npoint A = free ~(0, 0)");
        let codes: Vec<_> = output
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic.code)
            .collect();
        assert_eq!(codes, ["E112", "E149"]);
        assert_eq!(output.file.statements.len(), 1);
    }

    #[test]
    fn not_binds_to_a_complete_boolean_primary() {
        let output = parse(
            "locus 2\npoint A = free ~(0, 0)\npoint B = free ~(1, 0)\nassert not dist(A, B) == 1\n",
        );
        assert!(output.diagnostics.is_empty(), "{:#?}", output.diagnostics);
        let Some(Statement::Assert(assertion)) = output.file.statements.last() else {
            assert!(matches!(
                output.file.statements.last(),
                Some(Statement::Assert(_))
            ));
            return;
        };
        assert!(matches!(
            assertion.expr,
            Expr::Unary {
                op: crate::ast::UnaryOp::Not,
                ref expr,
                ..
            } if matches!(expr.as_ref(), Expr::Binary { op: crate::ast::BinOp::Eq, .. })
        ));
    }

    #[test]
    fn rejects_chained_comparisons() {
        let output = parse("locus 2\nnum a = 1\nassert a == 1 == 1\n");
        assert_eq!(
            output
                .diagnostics
                .iter()
                .map(|diagnostic| diagnostic.code)
                .collect::<Vec<_>>(),
            ["E145"]
        );
    }

    #[test]
    fn accepts_grouped_scalar_hint_but_rejects_point_shape() {
        let valid = parse("locus 2\nnum r = free ~(1 + 2)\nnum s = free ~(1 + 2) * 3\n");
        assert!(valid.diagnostics.is_empty(), "{:#?}", valid.diagnostics);
        assert_eq!(valid.file.statements.len(), 2);

        let invalid = parse("locus 2\nnum r = free ~(1, 2)\n");
        assert_eq!(invalid.diagnostics.len(), 1);
        assert_eq!(invalid.diagnostics[0].code, "E124");
    }

    #[test]
    fn diagnoses_a_tolerance_keyword_without_a_value() {
        let output = parse("locus 2\nassert true == true tol // missing value\n");
        assert_eq!(output.diagnostics.len(), 1);
        assert_eq!(output.diagnostics[0].code, "E147");
        assert_eq!(output.diagnostics[0].span.line, 2);
        assert_eq!(output.diagnostics[0].span.col, 24);
    }

    #[test]
    fn meta_recovery_preserves_later_top_level_statements() {
        let source = r#"locus 2
meta {
title "missing close"
point A = free ~(0, 0)
meta {
note "malformed close"
} trailing
point B = free ~(1, 0)
assert dist(A, B) == 1
"#;
        let output = parse(source);
        assert_eq!(
            output
                .diagnostics
                .iter()
                .map(|diagnostic| diagnostic.code)
                .collect::<Vec<_>>(),
            ["E151", "E151"]
        );
        assert_eq!(output.file.statements.len(), 5);
        assert!(matches!(output.file.statements[0], Statement::Meta(_)));
        assert!(matches!(output.file.statements[1], Statement::Op(_)));
        assert!(matches!(output.file.statements[2], Statement::Meta(_)));
        assert!(matches!(output.file.statements[3], Statement::Op(_)));
        assert!(matches!(output.file.statements[4], Statement::Assert(_)));
    }

    #[test]
    fn malformed_meta_entry_with_trailing_tokens_stays_in_the_block() {
        let source = r#"locus 2
meta {
point "description" trailing
point 123
title "still metadata"
}
point A = free ~(0, 0)
"#;
        let output = parse(source);
        assert_eq!(
            output
                .diagnostics
                .iter()
                .map(|diagnostic| diagnostic.code)
                .collect::<Vec<_>>(),
            ["E114", "E150"]
        );
        assert_eq!(output.file.statements.len(), 2);
        assert!(matches!(output.file.statements[0], Statement::Meta(_)));
        if let Statement::Meta(meta) = &output.file.statements[0] {
            assert_eq!(meta.entries.len(), 1);
            assert_eq!(meta.entries[0].key.name, "title");
        }
        assert!(matches!(output.file.statements[1], Statement::Op(_)));
    }

    #[test]
    fn metadata_key_without_a_value_reports_at_logical_line_end() {
        let output = parse("locus 2\nmeta {\ntitle // missing value\n}\n");
        assert_eq!(output.diagnostics.len(), 1);
        assert_eq!(output.diagnostics[0].code, "E150");
        assert_eq!(output.diagnostics[0].span.line, 3);
        assert_eq!(output.diagnostics[0].span.col, 6);
    }

    #[test]
    fn rejects_empty_predicate_and_property_argument_lists() {
        let source = r"locus 2
point O = free ~(0, 0)
num r = free ~1
circle c = circle(O, r)
rel horizontal()
num n = c.at()
";
        let output = parse(source);
        assert_eq!(
            output
                .diagnostics
                .iter()
                .map(|diagnostic| diagnostic.code)
                .collect::<Vec<_>>(),
            ["E153", "E153"]
        );
    }

    #[test]
    fn rejects_free_in_every_identifier_path() {
        let source = r#"locus 2
point free = free ~(0, 0)
point A = free ~(0, 0)
point B = midpoint(free, A)
assert free(A)
style free
class free {}
point C = midpoint(A, A) [free]
point D = midpoint(A, A) { free: dot }
num n = A.free
meta {
free "value"
}
"#;
        let output = parse(source);
        assert_eq!(
            output
                .diagnostics
                .iter()
                .map(|diagnostic| diagnostic.code)
                .collect::<Vec<_>>(),
            ["E152"; 9]
        );
    }

    #[test]
    fn synthesized_columns_count_unicode_scalars_after_text_literals() {
        let output = parse("locus 2\nlabel L = label(A, \"é\"\nlabel T = label(A, $α$\n");
        assert_eq!(output.diagnostics.len(), 2);
        assert_eq!(output.diagnostics[0].code, "E113");
        assert_eq!(output.diagnostics[0].span.line, 2);
        assert_eq!(output.diagnostics[0].span.col, 23);
        assert_eq!(output.diagnostics[1].code, "E113");
        assert_eq!(output.diagnostics[1].span.line, 3);
        assert_eq!(output.diagnostics[1].span.col, 23);
    }
}
