//! Hand-written recursive-descent parser with physical-line recovery.

use crate::ast::{
    AssertStmt, BinOp, CallStmt, ClassStmt, Constructor, Expr, File, FnStmt, GroupStmt, HeadKind,
    HeadToken, HexColor, Ident, Kind, MetaEntry, NumberExpr, OpStmt, Prop, PropValue, Props,
    RelStmt, Statement, StyleAttr, StyleStmt, TextSegment, TextSegmentKind, TextValue, UnaryOp,
};
use crate::diag::did_you_mean;
use crate::lexer::{LexTextSegment, Token, TokenKind, lex};
use crate::vocabulary::{
    BUILTIN_ATTR_NAMES, CONSTRUCTOR_NAMES, KIND_NAMES, PREDICATE_NAMES, PROP_NAMES, PROPERTY_NAMES,
    STYLE_PROP_NAMES, all_call_names, is_known_call, prop_names_for_kind,
};
use crate::{Diagnostic, FILE_HEADER, Span};
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
    beta6: bool,
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
            beta6: false,
        }
    }

    fn run(mut self) -> ParseOutput {
        self.parse_header();
        let mut statements = Vec::new();
        while self.line_index < self.lines.len() {
            if let Some(statement) = self.parse_next_statement() {
                if !self.beta6 {
                    self.validate_and_record(&statement);
                }
                statements.push(statement);
            }
        }
        if self.beta6 {
            let source_file = File {
                statements: statements.clone(),
                span: Span::new(1, 1, 0, self.source_len),
            };
            let expanded = crate::expand(&source_file);
            self.diagnostics.extend(expanded.diagnostics);
            for (statement, origin) in expanded.file.statements.iter().zip(&expanded.origins) {
                let start = self.diagnostics.len();
                self.validate_and_record(statement);
                add_invocation_notes(&mut self.diagnostics[start..], &origin.invocations);
            }
        }
        ParseOutput {
            file: File {
                statements,
                span: Span::new(1, 1, 0, self.source_len),
            },
            diagnostics: self.diagnostics,
        }
    }

    fn parse_next_statement(&mut self) -> Option<Statement> {
        let index = self.line_index;
        self.line_index += 1;
        let line = self.lines[index].clone();
        if line.is_empty()
            || line
                .iter()
                .any(|token| matches!(token.kind, TokenKind::Invalid))
        {
            return None;
        }
        let mut parser = LineParser::new(&line, self.line_ends[index]);
        parser.beta6 = self.beta6;
        let mut statement = parser.parse_statement();
        if statement.is_none() && parser.diagnostics.is_empty() {
            parser.error(
                "E112",
                parser.current_span(),
                "expected a statement keyword or object kind",
            );
        }
        self.diagnostics.extend(parser.diagnostics);
        if let Some(Statement::Fn(function)) = &mut statement {
            self.parse_function_body(function);
        }
        statement
    }

    fn parse_function_body(&mut self, function: &mut FnStmt) {
        while self.line_index < self.lines.len() {
            let line = &self.lines[self.line_index];
            if matches!(
                line.first().map(|token| &token.kind),
                Some(TokenKind::RBrace)
            ) {
                let mut parser = LineParser::new(line, self.line_ends[self.line_index]);
                parser.advance();
                let _ = parser.expect_end();
                self.diagnostics.extend(parser.diagnostics);
                function.span = function.span.join(line_span(line));
                self.line_index += 1;
                return;
            }
            if let Some(statement) = self.parse_next_statement() {
                if matches!(
                    statement,
                    Statement::Fn(_)
                        | Statement::Meta(_)
                        | Statement::Style(_)
                        | Statement::Class(_)
                ) {
                    self.diagnostics.push(Diagnostic::new("E155", statement.span(), "function bodies allow only declarations, groups, calls, relations, and assertions"));
                } else {
                    function.body.push(statement);
                }
            }
        }
        self.diagnostics.push(Diagnostic::new(
            "E155",
            function.span,
            "missing `}` ending function definition",
        ));
    }

    fn parse_header(&mut self) {
        self.skip_empty_lines();
        if self.line_index >= self.lines.len() {
            self.diagnostics.push(Diagnostic::new(
                "E110",
                Span::empty(1, 1, 0),
                format!("missing mandatory `{FILE_HEADER}` header"),
            ));
            return;
        }
        if !line_starts_with(&self.lines[self.line_index], "locus") {
            let span = line_span(&self.lines[self.line_index]);
            self.diagnostics.push(
                Diagnostic::new("E110", span, "the first statement must be a `locus` header")
                    .with_suggestion(FILE_HEADER),
            );
            return;
        }
        let line = &self.lines[self.line_index];
        self.beta6 = matches!(line.last().map(|token| &token.kind), Some(TokenKind::Number { raw, .. }) if raw == "6");
        let mut parser = LineParser::new(line, self.line_ends[self.line_index]);
        parser.take_keyword("locus");
        parser.parse_header_version();
        let _ = parser.expect_end();
        self.diagnostics.extend(parser.diagnostics);
        self.line_index += 1;
    }

    fn skip_empty_lines(&mut self) {
        while self.lines.get(self.line_index).is_some_and(Vec::is_empty) {
            self.line_index += 1;
        }
    }

    fn validate_and_record(&mut self, statement: &Statement) {
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
        if self.beta6 {
            crate::expand::validate_beta6_statement(statement, &mut self.diagnostics);
        }
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
            Statement::Meta(_)
            | Statement::Style(_)
            | Statement::Fn(_)
            | Statement::Group(_)
            | Statement::Call(_) => {}
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
        if self.beta6 {
            return;
        }
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
    beta6: bool,
}

impl<'a> LineParser<'a> {
    fn new(tokens: &'a [Token], end_span: Span) -> Self {
        Self {
            tokens,
            end_span,
            position: 0,
            diagnostics: Vec::new(),
            beta6: false,
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
        if first == "fn" {
            return self.parse_fn().map(Statement::Fn);
        }
        if first == "group" {
            return self.parse_group_statement().map(Statement::Group);
        }
        match first.as_str() {
            "rel" => self.parse_rel().map(Statement::Rel),
            "assert" => self.parse_assert().map(Statement::Assert),
            "style" => self.parse_style().map(Statement::Style),
            "class" => self.parse_class().map(Statement::Class),
            "meta" => self.parse_meta_entry().map(Statement::Meta),
            _ => self.parse_op_or_unknown(),
        }
    }

    fn parse_header_version(&mut self) {
        let Some(major) = self.advance().cloned() else {
            self.error(
                "E110",
                self.current_span(),
                format!("header must be `{FILE_HEADER}`"),
            );
            return;
        };
        let beta = self.advance().cloned();
        let revision = self.advance().cloned();
        let valid = matches!(major.kind, TokenKind::Number { ref raw, unit: None, .. } if raw == "0")
            && matches!(beta.as_ref().map(|token| &token.kind), Some(TokenKind::Ident(value)) if value == "beta")
            && matches!(revision.as_ref().map(|token| &token.kind), Some(TokenKind::Number { raw, unit: None, .. }) if raw == "6");
        if !valid {
            let span = revision
                .as_ref()
                .or(beta.as_ref())
                .map_or(major.span, |token| major.span.join(token.span));
            self.diagnostics.push(
                Diagnostic::new("E111", span, "unsupported Locus file header")
                    .with_suggestion(FILE_HEADER),
            );
        }
    }

    fn parse_op_or_unknown(&mut self) -> Option<Statement> {
        let token = self.peek()?.clone();
        let name = self.peek_ident()?.to_owned();
        let Some(kind) = Kind::from_name(&name) else {
            if self.beta6
                && matches!(
                    self.tokens.get(self.position + 1).map(|token| &token.kind),
                    Some(TokenKind::Ident(_))
                )
            {
                return self.parse_call_statement().map(Statement::Call);
            }
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
        let ctor = if self.take_simple(is_lparen).is_some() {
            if kind == Kind::Num {
                self.error(
                    "E156",
                    id.span,
                    "`num` has no constructor declaration; use `num id = expression`",
                );
                return None;
            }
            let (args, end) = self.parse_args()?;
            let name = Ident {
                name: if kind == Kind::Segment {
                    "seg"
                } else {
                    kind.as_str()
                }
                .to_owned(),
                span: start,
            };
            Constructor::Call {
                name,
                args,
                span: start.join(end),
            }
        } else {
            self.expect_simple(
                is_equal,
                "`=` or constructor arguments after object identifier",
            )
            .ok()?;
            let ctor = self.parse_constructor(kind)?;
            self.validate_rhs_constructor(kind, &id, &ctor);
            ctor
        };
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

    fn validate_rhs_constructor(&mut self, kind: Kind, id: &Ident, ctor: &Constructor) {
        if !self.beta6 {
            return;
        }
        let Constructor::Call { name, .. } = ctor else {
            return;
        };
        let expected = if kind == Kind::Segment {
            "seg"
        } else {
            kind.as_str()
        };
        if name.name == expected {
            self.diagnostics.push(
                Diagnostic::new(
                    "E156",
                    name.span,
                    "same-kind constructors use a constructor declaration",
                )
                .with_suggestion(format!("{} {}(...)", kind.as_str(), id.name)),
            );
        }
    }

    fn parse_fn(&mut self) -> Option<FnStmt> {
        let start = self.advance()?.span;
        let name = self.take_ident("function name")?;
        self.expect_simple(is_lparen, "`(` after function name")
            .ok()?;
        let mut params = Vec::new();
        if self.take_simple(is_rparen).is_none() {
            loop {
                params.push(self.take_ident("parameter name")?);
                if self.take_simple(is_comma).is_some() {
                    continue;
                }
                self.expect_simple(is_rparen, "`)` ending parameters")
                    .ok()?;
                break;
            }
        }
        let end = self
            .expect_simple(is_lbrace, "`{` opening function body")
            .ok()?;
        self.expect_end().ok()?;
        Some(FnStmt {
            name,
            params,
            body: Vec::new(),
            span: start.join(end),
        })
    }

    fn parse_group_statement(&mut self) -> Option<GroupStmt> {
        let start = self.advance()?.span;
        let name = self.take_ident("group name")?;
        self.expect_simple(is_lparen, "`(` after group name").ok()?;
        let (members, end) = self.parse_args()?;
        self.expect_end().ok()?;
        if members.is_empty() {
            self.error(
                "E157",
                start.join(end),
                "a group requires at least one member",
            );
        }
        Some(GroupStmt {
            name,
            members,
            span: start.join(end),
        })
    }

    fn parse_call_statement(&mut self) -> Option<CallStmt> {
        let function = self.take_ident("function name")?;
        let id = self.take_ident("instance name")?;
        self.expect_simple(is_lparen, "`(` after instance name")
            .ok()?;
        let (args, _) = self.parse_args()?;
        let attr = self.parse_attr().ok()?;
        self.expect_end().ok()?;
        let span = function.span.join(self.previous_span());
        Some(CallStmt {
            function,
            id,
            args,
            attr,
            span,
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
        let ctor = self.parse_named_constructor()?;
        if !self.at_simple(is_dot) {
            return Some(ctor);
        }
        let Constructor::Call { name, args, span } = ctor else {
            unreachable!()
        };
        let mut expr = Expr::Call { name, args, span };
        while self.at_simple(is_dot) {
            expr = self.parse_property(expr)?;
        }
        Some(Constructor::Expr {
            span: expr.span(),
            expr: Box::new(expr),
        })
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
        if kind != Kind::Num {
            self.error("E123", free, "only `num` declarations may use `free`");
            return None;
        }
        self.parse_free_num(free)
    }

    fn parse_free_num(&mut self, start: Span) -> Option<Constructor> {
        if !self
            .peek()
            .is_some_and(|token| starts_expression(&token.kind))
        {
            self.error(
                "E124",
                self.current_span(),
                "a free `num` requires a scalar seed expression after `free`",
            );
            return None;
        }
        let starts_with_group = self.at_simple(is_lparen);
        let Some(hint) = self.parse_expr(0) else {
            if starts_with_group && self.at_simple(is_comma) {
                self.replace_last_diagnostic(
                    "E124",
                    "a free `num` requires a scalar seed, not a coordinate pair",
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
        if name.name == "offset" {
            self.diagnostics.push(offset_migration(Diagnostic::new(
                "E140",
                name.span,
                "unknown constructor `offset`",
            )));
            return None;
        }
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
        let props = self.parse_optional_props()?;
        self.expect_end().ok()?;
        Some(RelStmt {
            span: start.join(self.previous_span()),
            expr,
            props,
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
        let start = self.advance()?.span;
        let key = self.take_ident("metadata key")?;
        self.expect_simple(is_equal, "`=` after metadata key")
            .ok()?;
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
            span: start.join(value.span),
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
            if ident.name == "offset" {
                diagnostic = offset_migration(diagnostic);
            } else if let Some(suggestion) = did_you_mean(&ident.name, &vocabulary) {
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
        if !self.beta6 && !PROPERTY_NAMES.contains(&name.name.as_str()) {
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

fn add_invocation_notes(diagnostics: &mut [Diagnostic], invocations: &[crate::InvocationOrigin]) {
    for diagnostic in diagnostics {
        for invocation in invocations {
            diagnostic.notes.push(format!(
                "in `{}` invoked as `{}` at line {}",
                invocation.function, invocation.instance, invocation.span.line
            ));
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

fn offset_migration(diagnostic: Diagnostic) -> Diagnostic {
    diagnostic
        .with_note("`offset` has been removed from the public language")
        .with_suggestion(
            "use anchors, row/column, alignment, containment, or geometric constructions; use hdist/vdist equalities for actual measured dimensions",
        )
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
        if name.name == "offset" {
            diagnostic = offset_migration(diagnostic);
        } else if let Some(suggestion) = did_you_mean(&name.name, CONSTRUCTOR_NAMES) {
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
    const BY_KIND: [&[&str]; 18] = [
        &[
            "point",
            "midpoint",
            "lerp",
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
        &["seg", "edge"],
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
        &["rounded_rect"],
        &["diamond"],
        &["label"],
        &["arrow"],
        &["tick"],
        &["right_angle"],
        &["text_block"],
    ];
    BY_KIND[kind as usize]
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
        if args.is_empty()
            && (PREDICATE_NAMES.contains(&name.name.as_str())
                || crate::vocabulary::LEGACY_PREDICATE_NAMES.contains(&name.name.as_str()))
        {
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
    if PREDICATE_NAMES.contains(&name.name.as_str())
        || crate::vocabulary::LEGACY_PREDICATE_NAMES.contains(&name.name.as_str())
    {
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
        Expr::Group { expr, .. } => validate_solvable(expr, diagnostics),
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

pub(crate) fn prop_uses_references(name: &str) -> bool {
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
