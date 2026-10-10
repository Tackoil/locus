//! Hand-written, recovery-friendly lexer for the line-oriented Locus grammar.
#![allow(missing_docs, reason = "token variant spellings are self-describing")]

use crate::ast::{AngleUnit, TextSegmentKind};
use crate::{Diagnostic, Span};

/// A token and its source span.
#[derive(Clone, Debug, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

/// A source text segment carried by a string token.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LexTextSegment {
    pub kind: TextSegmentKind,
    pub source: String,
    pub span: Span,
}

/// Lexical token kinds. Keywords intentionally remain identifiers.
#[derive(Clone, Debug, PartialEq)]
pub enum TokenKind {
    Ident(String),
    Number {
        raw: String,
        value: f64,
        unit: Option<AngleUnit>,
    },
    String(Vec<LexTextSegment>),
    Tex(String),
    HexColor(String),
    LParen,
    RParen,
    LBrace,
    RBrace,
    LBracket,
    RBracket,
    Comma,
    Colon,
    Dot,
    Equal,
    EqEq,
    Less,
    Greater,
    LessEq,
    GreaterEq,
    Plus,
    Minus,
    Star,
    Slash,
    Arrow,
    DoubleArrow,
    Newline,
    Invalid,
    Eof,
}

/// Lexer result containing recoverable diagnostics.
#[derive(Clone, Debug, PartialEq)]
pub struct LexOutput {
    pub tokens: Vec<Token>,
    pub diagnostics: Vec<Diagnostic>,
}

/// Lexes a complete UTF-8 Locus source file.
#[must_use]
pub fn lex(source: &str) -> LexOutput {
    Lexer::new(source).run()
}

struct Lexer<'a> {
    source: &'a str,
    cursor: usize,
    line: usize,
    col: usize,
    tokens: Vec<Token>,
    diagnostics: Vec<Diagnostic>,
}

impl<'a> Lexer<'a> {
    fn new(source: &'a str) -> Self {
        Self {
            source,
            cursor: 0,
            line: 1,
            col: 1,
            tokens: Vec::new(),
            diagnostics: Vec::new(),
        }
    }

    fn run(mut self) -> LexOutput {
        while let Some(ch) = self.peek() {
            self.lex_next(ch);
        }
        let span = Span::empty(self.line, self.col, self.cursor);
        self.tokens.push(Token {
            kind: TokenKind::Eof,
            span,
        });
        LexOutput {
            tokens: self.tokens,
            diagnostics: self.diagnostics,
        }
    }

    fn lex_next(&mut self, ch: char) {
        if matches!(ch, ' ' | '\t') {
            self.advance();
        } else if matches!(ch, '\n' | '\r') {
            self.lex_newline();
        } else if ch == '/' && self.peek_next() == Some('/') {
            self.skip_comment();
        } else if ch.is_ascii_alphabetic() {
            self.lex_ident();
        } else if ch.is_ascii_digit()
            || (ch == '.' && self.peek_next().is_some_and(|next| next.is_ascii_digit()))
        {
            self.lex_number();
        } else if ch == '"' {
            self.lex_string();
        } else if ch == '$' {
            self.lex_tex();
        } else if ch == '#' {
            self.lex_hex_color();
        } else {
            self.lex_punctuation_or_invalid();
        }
    }

    fn lex_newline(&mut self) {
        let start = self.mark();
        if self.peek() == Some('\r') {
            self.advance();
            if self.peek() == Some('\n') {
                self.advance_raw();
            }
        } else {
            self.advance();
        }
        self.tokens.push(Token {
            kind: TokenKind::Newline,
            span: self.span_from(start),
        });
        self.line += 1;
        self.col = 1;
    }

    fn skip_comment(&mut self) {
        while !matches!(self.peek(), None | Some('\n' | '\r')) {
            self.advance();
        }
    }

    fn lex_ident(&mut self) {
        let start = self.mark();
        self.advance();
        while self.peek().is_some_and(is_ident_continue) {
            self.advance();
        }
        self.push_slice(TokenKind::Ident, start);
    }

    fn lex_number(&mut self) {
        let start = self.mark();
        self.consume_digits();
        if self.peek() == Some('.') {
            self.advance();
            self.consume_digits();
        }
        if matches!(self.peek(), Some('e' | 'E')) && !self.lex_exponent(start) {
            self.push_invalid(start);
            return;
        }
        let numeric_end = self.cursor;
        let unit = self.lex_angle_unit();
        let raw = self.source[start.2..numeric_end].to_owned();
        match raw.parse::<f64>() {
            Ok(value) if value.is_finite() => self.tokens.push(Token {
                kind: TokenKind::Number { raw, value, unit },
                span: self.span_from(start),
            }),
            _ => {
                let span = self.span_from(start);
                self.diagnostics.push(Diagnostic::new(
                    "E104",
                    span,
                    "invalid or non-finite number literal",
                ));
                self.tokens.push(Token {
                    kind: TokenKind::Invalid,
                    span,
                });
            }
        }
    }

    fn lex_exponent(&mut self, start: (usize, usize, usize)) -> bool {
        self.advance();
        if matches!(self.peek(), Some('+' | '-')) {
            self.advance();
        }
        if !self.peek().is_some_and(|ch| ch.is_ascii_digit()) {
            while self.peek().is_some_and(is_ident_continue) {
                self.advance();
            }
            let span = self.span_from(start);
            self.diagnostics.push(Diagnostic::new(
                "E104",
                span,
                "exponent requires at least one digit",
            ));
            return false;
        }
        self.consume_digits();
        true
    }

    fn lex_angle_unit(&mut self) -> Option<AngleUnit> {
        let rest = &self.source[self.cursor..];
        if rest.starts_with("deg") && !rest[3..].chars().next().is_some_and(is_ident_continue) {
            self.advance_n_ascii(3);
            Some(AngleUnit::Deg)
        } else if rest.starts_with("rad")
            && !rest[3..].chars().next().is_some_and(is_ident_continue)
        {
            self.advance_n_ascii(3);
            Some(AngleUnit::Rad)
        } else {
            None
        }
    }

    fn lex_string(&mut self) {
        let start = self.mark();
        self.advance();
        let mut segments = Vec::new();
        let mut plain = String::new();
        let mut plain_start = self.mark();
        while self.lex_string_step(start, &mut segments, &mut plain, &mut plain_start) {}
    }

    fn lex_string_step(
        &mut self,
        start: (usize, usize, usize),
        segments: &mut Vec<LexTextSegment>,
        plain: &mut String,
        plain_start: &mut (usize, usize, usize),
    ) -> bool {
        match self.string_event() {
            StringEvent::Close => {
                self.finish_string(
                    start,
                    std::mem::take(segments),
                    std::mem::take(plain),
                    *plain_start,
                );
                false
            }
            StringEvent::Tex => self.lex_inline_tex_or_finish(start, segments, plain, plain_start),
            StringEvent::Escape => {
                self.lex_string_escape(plain);
                true
            }
            StringEvent::Unterminated => {
                self.report_unterminated_string(start);
                false
            }
            StringEvent::Plain(ch) => {
                plain.push(ch);
                self.advance();
                true
            }
        }
    }

    fn lex_inline_tex_or_finish(
        &mut self,
        start: (usize, usize, usize),
        segments: &mut Vec<LexTextSegment>,
        plain: &mut String,
        plain_start: &mut (usize, usize, usize),
    ) -> bool {
        if self.lex_inline_tex(segments, plain, plain_start) {
            true
        } else {
            self.push_invalid(start);
            false
        }
    }

    fn string_event(&self) -> StringEvent {
        match self.peek() {
            Some('"') => StringEvent::Close,
            Some('$') => StringEvent::Tex,
            Some('\\') => StringEvent::Escape,
            None | Some('\n' | '\r') => StringEvent::Unterminated,
            Some(ch) => StringEvent::Plain(ch),
        }
    }

    fn finish_string(
        &mut self,
        start: (usize, usize, usize),
        mut segments: Vec<LexTextSegment>,
        mut plain: String,
        plain_start: (usize, usize, usize),
    ) {
        self.push_text_segment(
            &mut segments,
            TextSegmentKind::Plain,
            &mut plain,
            plain_start,
        );
        self.advance();
        self.tokens.push(Token {
            kind: TokenKind::String(segments),
            span: self.span_from(start),
        });
    }

    fn lex_inline_tex(
        &mut self,
        segments: &mut Vec<LexTextSegment>,
        plain: &mut String,
        plain_start: &mut (usize, usize, usize),
    ) -> bool {
        self.push_text_segment(segments, TextSegmentKind::Plain, plain, *plain_start);
        let tex_start = self.mark();
        self.advance();
        let Some(source) = self.consume_tex_body(tex_start, "E103") else {
            return false;
        };
        segments.push(LexTextSegment {
            kind: TextSegmentKind::Tex,
            source,
            span: self.span_from(tex_start),
        });
        *plain_start = self.mark();
        true
    }

    fn report_unterminated_string(&mut self, start: (usize, usize, usize)) {
        self.diagnostics.push(Diagnostic::new(
            "E101",
            self.span_from(start),
            "unterminated string literal",
        ));
        self.push_invalid(start);
    }

    fn lex_string_escape(&mut self, plain: &mut String) {
        let escape_start = self.mark();
        self.advance();
        match self.peek() {
            Some('"') => plain.push('"'),
            Some('\\') => plain.push('\\'),
            Some('$') => plain.push('$'),
            Some('n') => plain.push('\n'),
            Some(ch) => {
                plain.push(ch);
                self.diagnostics.push(Diagnostic::new(
                    "E102",
                    self.span_from(escape_start),
                    format!("unsupported string escape `\\{ch}`"),
                ));
            }
            None => return,
        }
        self.advance();
    }

    fn lex_tex(&mut self) {
        let start = self.mark();
        self.advance();
        if let Some(source) = self.consume_tex_body(start, "E103") {
            self.tokens.push(Token {
                kind: TokenKind::Tex(source),
                span: self.span_from(start),
            });
        } else {
            self.push_invalid(start);
        }
    }

    fn consume_tex_body(
        &mut self,
        start: (usize, usize, usize),
        code: &'static str,
    ) -> Option<String> {
        let mut source = String::new();
        loop {
            match self.consume_tex_step(&mut source) {
                TexControl::Continue => {}
                TexControl::Close => return Some(source),
                TexControl::Unterminated => {
                    self.diagnostics.push(Diagnostic::new(
                        code,
                        self.span_from(start),
                        "unterminated TeX literal",
                    ));
                    return None;
                }
            }
        }
    }

    fn consume_tex_step(&mut self, source: &mut String) -> TexControl {
        match self.tex_event() {
            TexEvent::Close => {
                self.advance();
                TexControl::Close
            }
            TexEvent::Escape => {
                self.consume_tex_escape(source);
                TexControl::Continue
            }
            TexEvent::Unterminated => TexControl::Unterminated,
            TexEvent::Plain(ch) => {
                source.push(ch);
                self.advance();
                TexControl::Continue
            }
        }
    }

    fn consume_tex_escape(&mut self, source: &mut String) {
        source.push('\\');
        self.advance();
        if let Some(next) = self.peek().filter(|next| !matches!(next, '\n' | '\r')) {
            source.push(next);
            self.advance();
        }
    }

    fn tex_event(&self) -> TexEvent {
        match self.peek() {
            Some('$') => TexEvent::Close,
            Some('\\') => TexEvent::Escape,
            None | Some('\n' | '\r') => TexEvent::Unterminated,
            Some(ch) => TexEvent::Plain(ch),
        }
    }

    fn lex_hex_color(&mut self) {
        let start = self.mark();
        self.advance();
        while self.peek().is_some_and(|ch| ch.is_ascii_alphanumeric()) {
            self.advance();
        }
        let value = &self.source[start.2 + 1..self.cursor];
        if matches!(value.len(), 6 | 8) && value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            self.tokens.push(Token {
                kind: TokenKind::HexColor(value.to_ascii_uppercase()),
                span: self.span_from(start),
            });
        } else {
            let span = self.span_from(start);
            self.diagnostics.push(Diagnostic::new(
                "E105",
                span,
                "hex color must contain exactly six or eight hexadecimal digits",
            ));
            self.tokens.push(Token {
                kind: TokenKind::Invalid,
                span,
            });
        }
    }

    fn lex_punctuation_or_invalid(&mut self) {
        let start = self.mark();
        let kind = self
            .compound_punctuation()
            .or_else(|| self.single_punctuation());
        let Some(kind) = kind else {
            self.push_unexpected_character(start);
            return;
        };
        self.tokens.push(Token {
            kind,
            span: self.span_from(start),
        });
    }

    fn compound_punctuation(&mut self) -> Option<TokenKind> {
        const COMPOUND: &[(&str, TokenKind)] = &[
            ("<->", TokenKind::DoubleArrow),
            ("->", TokenKind::Arrow),
            ("==", TokenKind::EqEq),
            ("<=", TokenKind::LessEq),
            (">=", TokenKind::GreaterEq),
        ];
        for (source, kind) in COMPOUND {
            if self.source[self.cursor..].starts_with(source) {
                self.advance_n_ascii(source.len());
                return Some(kind.clone());
            }
        }
        None
    }

    fn single_punctuation(&mut self) -> Option<TokenKind> {
        const SINGLE: &[(char, TokenKind)] = &[
            ('(', TokenKind::LParen),
            (')', TokenKind::RParen),
            ('{', TokenKind::LBrace),
            ('}', TokenKind::RBrace),
            ('[', TokenKind::LBracket),
            (']', TokenKind::RBracket),
            (',', TokenKind::Comma),
            (':', TokenKind::Colon),
            ('.', TokenKind::Dot),
            ('=', TokenKind::Equal),
            ('<', TokenKind::Less),
            ('>', TokenKind::Greater),
            ('+', TokenKind::Plus),
            ('-', TokenKind::Minus),
            ('*', TokenKind::Star),
            ('/', TokenKind::Slash),
        ];
        let ch = self.peek()?;
        let kind = SINGLE
            .iter()
            .find_map(|(candidate, kind)| (*candidate == ch).then(|| kind.clone()))?;
        self.advance();
        Some(kind)
    }

    fn push_unexpected_character(&mut self, start: (usize, usize, usize)) {
        let Some(ch) = self.peek() else {
            return;
        };
        self.advance();
        let span = self.span_from(start);
        self.diagnostics.push(Diagnostic::new(
            "E100",
            span,
            format!("unexpected character `{ch}`"),
        ));
        self.tokens.push(Token {
            kind: TokenKind::Invalid,
            span,
        });
    }

    fn push_text_segment(
        &self,
        segments: &mut Vec<LexTextSegment>,
        kind: TextSegmentKind,
        source: &mut String,
        start: (usize, usize, usize),
    ) {
        segments.push(LexTextSegment {
            kind,
            source: std::mem::take(source),
            span: self.span_from(start),
        });
    }

    fn push_slice(&mut self, make: impl FnOnce(String) -> TokenKind, start: (usize, usize, usize)) {
        self.tokens.push(Token {
            kind: make(self.source[start.2..self.cursor].to_owned()),
            span: self.span_from(start),
        });
    }

    fn push_invalid(&mut self, start: (usize, usize, usize)) {
        self.tokens.push(Token {
            kind: TokenKind::Invalid,
            span: self.span_from(start),
        });
    }

    fn consume_digits(&mut self) {
        while self.peek().is_some_and(|ch| ch.is_ascii_digit()) {
            self.advance();
        }
    }

    fn advance_n_ascii(&mut self, count: usize) {
        for _ in 0..count {
            self.advance();
        }
    }

    fn mark(&self) -> (usize, usize, usize) {
        (self.line, self.col, self.cursor)
    }

    fn span_from(&self, start: (usize, usize, usize)) -> Span {
        Span::new(start.0, start.1, start.2, self.cursor)
    }

    fn peek(&self) -> Option<char> {
        self.source[self.cursor..].chars().next()
    }

    fn peek_next(&self) -> Option<char> {
        self.source[self.cursor..].chars().nth(1)
    }

    fn advance(&mut self) {
        if let Some(ch) = self.peek() {
            self.cursor += ch.len_utf8();
            self.col += 1;
        }
    }

    fn advance_raw(&mut self) {
        if let Some(ch) = self.peek() {
            self.cursor += ch.len_utf8();
        }
    }
}

enum StringEvent {
    Close,
    Tex,
    Escape,
    Unterminated,
    Plain(char),
}

enum TexEvent {
    Close,
    Escape,
    Unterminated,
    Plain(char),
}

enum TexControl {
    Continue,
    Close,
    Unterminated,
}

fn is_ident_continue(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '_' | '\'')
}

#[cfg(test)]
mod tests {
    use super::{TokenKind, lex};
    use crate::ast::{AngleUnit, TextSegmentKind};

    #[test]
    fn lexes_numbers_units_comments_and_punctuation() {
        let output = lex("rotate(P, O, -30deg) // side\n.5rad 1e-9 <-> ->\n");
        assert!(output.diagnostics.is_empty(), "{:#?}", output.diagnostics);
        assert!(output.tokens.iter().any(|token| matches!(
            token.kind,
            TokenKind::Number {
                unit: Some(AngleUnit::Deg),
                ..
            }
        )));
        assert!(output.tokens.iter().any(|token| matches!(
            token.kind,
            TokenKind::Number {
                unit: Some(AngleUnit::Rad),
                ..
            }
        )));
        assert_eq!(
            output
                .tokens
                .iter()
                .filter(|token| matches!(token.kind, TokenKind::Newline))
                .count(),
            2
        );
    }

    #[test]
    fn segments_mixed_text_and_preserves_tex_backslashes() {
        let output = lex("\"angle $\\alpha$ and \\$5\" $\\triangle ABC$\n");
        assert!(output.diagnostics.is_empty(), "{:#?}", output.diagnostics);
        assert!(matches!(output.tokens[0].kind, TokenKind::String(_)));
        if let TokenKind::String(segments) = &output.tokens[0].kind {
            assert_eq!(segments.len(), 3);
            assert_eq!(segments[0].kind, TextSegmentKind::Plain);
            assert_eq!(segments[1].kind, TextSegmentKind::Tex);
            assert_eq!(segments[1].source, "\\alpha");
            assert_eq!(segments[2].source, " and $5");
        }
        assert!(
            matches!(&output.tokens[1].kind, TokenKind::Tex(value) if value == "\\triangle ABC")
        );
    }

    #[test]
    fn preserves_empty_plain_segments_at_exact_string_boundaries() {
        let output = lex("\"\" \"filter offset $O_2$\"\n");
        assert!(output.diagnostics.is_empty(), "{:#?}", output.diagnostics);

        if let TokenKind::String(empty) = &output.tokens[0].kind {
            assert_eq!(empty.len(), 1);
            assert_eq!(empty[0].kind, TextSegmentKind::Plain);
            assert_eq!(empty[0].source, "");
        } else {
            assert!(matches!(output.tokens[0].kind, TokenKind::String(_)));
        }

        if let TokenKind::String(mixed) = &output.tokens[1].kind {
            assert_eq!(mixed.len(), 3);
            assert_eq!(mixed[0].kind, TextSegmentKind::Plain);
            assert_eq!(mixed[0].source, "filter offset ");
            assert_eq!(mixed[1].kind, TextSegmentKind::Tex);
            assert_eq!(mixed[1].source, "O_2");
            assert_eq!(mixed[2].kind, TextSegmentKind::Plain);
            assert_eq!(mixed[2].source, "");
        } else {
            assert!(matches!(output.tokens[1].kind, TokenKind::String(_)));
        }
    }

    #[test]
    fn reports_independent_lexical_errors() {
        let output = lex("\"bad\\q\"\n$unterminated\n#12345Z\n@");
        let codes: Vec<_> = output.diagnostics.iter().map(|diag| diag.code).collect();
        assert_eq!(codes, ["E102", "E103", "E105", "E100"]);
    }

    #[test]
    fn malformed_exponent_reports_once() {
        let output = lex("1e\n");
        assert_eq!(output.diagnostics.len(), 1);
        assert_eq!(output.diagnostics[0].code, "E104");
    }
}
