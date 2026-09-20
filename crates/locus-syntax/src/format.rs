//! Canonical, comment-preserving source formatting.

use crate::ast::{
    AngleUnit, AssertStmt, BinOp, ClassStmt, Constructor, Expr, File, HeadKind, MetaBlock,
    MetaEntry, OpStmt, PropValue, Props, Statement, TextSegmentKind, TextValue, UnaryOp,
};
use crate::{Diagnostic, FILE_HEADER, parse};
use std::collections::BTreeMap;
use std::fmt::Write as _;

/// The result of parsing and formatting source text.
#[derive(Clone, Debug, PartialEq)]
pub struct FormatOutput {
    /// Canonical source, always terminated by one newline.
    pub source: String,
    /// Diagnostics produced while parsing the input.
    pub diagnostics: Vec<Diagnostic>,
}

/// Parses and canonically formats a complete source file.
#[must_use]
pub fn format(source: &str) -> FormatOutput {
    let parsed = parse(source);
    FormatOutput {
        source: format_parsed(&parsed.file, source),
        diagnostics: parsed.diagnostics,
    }
}

/// Formats an AST without source trivia.
///
/// Because blank lines and comments are not represented in [`File`], consecutive operation
/// statements form one alignment block here. Use [`format_parsed`] when source-trivia boundaries
/// must split alignment blocks.
#[must_use]
pub fn format_file(file: &File) -> String {
    let mut lines = vec![FILE_HEADER.to_owned()];
    let mut index = 0;
    while index < file.statements.len() {
        if matches!(file.statements[index], Statement::Op(_)) {
            index = push_aligned_ops(&file.statements, index, &mut lines);
        } else {
            lines.extend(format_statement(&file.statements[index], None));
            index += 1;
        }
    }
    finish_lines(lines)
}

/// Formats an AST while retaining comments and blank lines from its source.
#[must_use]
pub fn format_parsed(file: &File, source: &str) -> String {
    let source_lines = physical_lines(source);
    let mut replacements = statement_replacements(file, source, &source_lines);
    align_source_ops(&source_lines, &mut replacements);
    let lines = source_lines
        .iter()
        .enumerate()
        .map(|(index, line)| format_source_line(index + 1, line, &replacements))
        .collect();
    finish_lines(lines)
}

fn physical_lines(source: &str) -> Vec<&str> {
    let bytes = source.as_bytes();
    let mut lines = Vec::new();
    let mut start = 0;
    let mut cursor = 0;
    while cursor < bytes.len() {
        if !matches!(bytes[cursor], b'\r' | b'\n') {
            cursor += 1;
            continue;
        }
        lines.push(&source[start..cursor]);
        if bytes[cursor] == b'\r' && bytes.get(cursor + 1) == Some(&b'\n') {
            cursor += 1;
        }
        cursor += 1;
        start = cursor;
    }
    if start < source.len() {
        lines.push(&source[start..]);
    }
    lines
}

#[derive(Clone, Debug)]
enum Replacement {
    Code(String),
    Op { prefix: String, suffix: String },
}

fn statement_replacements(
    file: &File,
    source: &str,
    lines: &[&str],
) -> BTreeMap<usize, Replacement> {
    let mut replacements = BTreeMap::new();
    if let Some(line) = header_line(lines) {
        replacements.insert(line, Replacement::Code(FILE_HEADER.to_owned()));
    }
    for statement in &file.statements {
        add_statement_replacements(statement, source, &mut replacements);
    }
    replacements
}

fn header_line(lines: &[&str]) -> Option<usize> {
    lines.iter().enumerate().find_map(|(index, line)| {
        let code = split_comment(line).0.trim();
        (code.split_ascii_whitespace().next() == Some("locus")).then_some(index + 1)
    })
}

fn add_statement_replacements(
    statement: &Statement,
    source: &str,
    replacements: &mut BTreeMap<usize, Replacement>,
) {
    match statement {
        Statement::Op(op) => {
            let (prefix, suffix) = format_op_parts(op);
            replacements.insert(op.span.line, Replacement::Op { prefix, suffix });
        }
        Statement::Meta(meta) => add_meta_replacements(meta, source, replacements),
        _ => {
            if let Some(line) = format_statement(statement, None).into_iter().next() {
                replacements.insert(statement.span().line, Replacement::Code(line));
            }
        }
    }
}

fn add_meta_replacements(
    meta: &MetaBlock,
    source: &str,
    replacements: &mut BTreeMap<usize, Replacement>,
) {
    replacements.insert(meta.span.line, Replacement::Code("meta {".to_owned()));
    for entry in &meta.entries {
        replacements.insert(entry.span.line, Replacement::Code(format_meta_entry(entry)));
    }
    let end_line = line_at_byte(source, meta.span.byte_end.saturating_sub(1));
    replacements.insert(end_line, Replacement::Code("}".to_owned()));
}

fn line_at_byte(source: &str, byte: usize) -> usize {
    let bytes = source.as_bytes();
    let limit = byte.min(bytes.len());
    let mut line = 1;
    let mut cursor = 0;
    while cursor < limit {
        if bytes[cursor] == b'\r' {
            line += 1;
            cursor += usize::from(bytes.get(cursor + 1) == Some(&b'\n'));
        } else if bytes[cursor] == b'\n' {
            line += 1;
        }
        cursor += 1;
    }
    line
}

fn align_source_ops(lines: &[&str], replacements: &mut BTreeMap<usize, Replacement>) {
    let mut line = 1;
    while line <= lines.len() {
        if !matches!(replacements.get(&line), Some(Replacement::Op { .. })) {
            line += 1;
            continue;
        }
        let start = line;
        while matches!(replacements.get(&line), Some(Replacement::Op { .. })) {
            line += 1;
        }
        align_replacement_range(replacements, start, line);
    }
}

fn align_replacement_range(
    replacements: &mut BTreeMap<usize, Replacement>,
    start: usize,
    end: usize,
) {
    let width = (start..end)
        .filter_map(|line| match replacements.get(&line) {
            Some(Replacement::Op { prefix, .. }) => Some(prefix.chars().count()),
            _ => None,
        })
        .max()
        .unwrap_or_default();
    for line in start..end {
        let Some(Replacement::Op { prefix, suffix }) = replacements.get_mut(&line) else {
            continue;
        };
        let padding = width.saturating_sub(prefix.chars().count()) + 1;
        *suffix = format!("{}{}= {}", prefix, " ".repeat(padding), suffix);
        prefix.clear();
    }
}

fn format_source_line(
    line_number: usize,
    original: &str,
    replacements: &BTreeMap<usize, Replacement>,
) -> String {
    let (original_code, comment) = split_comment(original);
    let code = match replacements.get(&line_number) {
        Some(Replacement::Code(code)) => code.clone(),
        Some(Replacement::Op { prefix, suffix }) => format!("{prefix}{suffix}"),
        None => original_code.trim_end().to_owned(),
    };
    join_comment(code, comment)
}

fn join_comment(code: String, comment: Option<&str>) -> String {
    let Some(comment) = comment else {
        return code;
    };
    if code.trim().is_empty() {
        format!("{}{}", code, comment.trim_end())
    } else {
        format!("{}  {}", code.trim_end(), comment.trim_end())
    }
}

fn split_comment(line: &str) -> (&str, Option<&str>) {
    let bytes = line.as_bytes();
    let mut index = 0;
    let mut quoted = false;
    let mut tex = false;
    while index < bytes.len() {
        if bytes[index] == b'\\' && (quoted || tex) {
            index = (index + 2).min(bytes.len());
            continue;
        }
        if bytes[index] == b'"' && !tex {
            quoted = !quoted;
        } else if bytes[index] == b'$' {
            tex = !tex;
        } else if bytes[index] == b'/' && !quoted && !tex && bytes.get(index + 1) == Some(&b'/') {
            return (&line[..index], Some(&line[index..]));
        }
        index += 1;
    }
    (line, None)
}

fn push_aligned_ops(statements: &[Statement], start: usize, lines: &mut Vec<String>) -> usize {
    let end = statements[start..]
        .iter()
        .position(|statement| !matches!(statement, Statement::Op(_)))
        .map_or(statements.len(), |offset| start + offset);
    let width = statements[start..end]
        .iter()
        .filter_map(|statement| match statement {
            Statement::Op(op) => Some(format_op_prefix(op).chars().count()),
            _ => None,
        })
        .max();
    for statement in &statements[start..end] {
        lines.extend(format_statement(statement, width));
    }
    end
}

fn format_statement(statement: &Statement, alignment: Option<usize>) -> Vec<String> {
    match statement {
        Statement::Op(op) => vec![format_op(op, alignment)],
        Statement::Rel(rel) => vec![format!("rel {}", format_expr(&rel.expr))],
        Statement::Assert(value) => vec![format_assert(value)],
        Statement::Meta(meta) => format_meta(meta),
        Statement::Style(style) => vec![format!("style {}", style.name.name)],
        Statement::Class(class) => vec![format_class(class)],
    }
}

fn format_op(op: &OpStmt, alignment: Option<usize>) -> String {
    let (prefix, suffix) = format_op_parts(op);
    let padding = alignment.map_or(1, |width| width.saturating_sub(prefix.chars().count()) + 1);
    format!("{}{}= {}", prefix, " ".repeat(padding), suffix)
}

fn format_op_parts(op: &OpStmt) -> (String, String) {
    let mut suffix = format_constructor(&op.ctor);
    if !op.where_clauses.is_empty() {
        suffix.push_str(" where { ");
        suffix.push_str(&format_expr_list(&op.where_clauses));
        suffix.push_str(" }");
    }
    if let Some(attr) = &op.attr {
        let _ = write!(suffix, " [{}]", attr.name.name);
    }
    if !op.props.items.is_empty() {
        suffix.push(' ');
        suffix.push_str(&format_props(&op.props));
    }
    (format_op_prefix(op), suffix)
}

fn format_op_prefix(op: &OpStmt) -> String {
    format!("{} {}", op.kind.as_str(), op.id.name)
}

fn format_constructor(constructor: &Constructor) -> String {
    match constructor {
        Constructor::Call { name, args, .. } => format_call(&name.name, args),
        Constructor::Expr { expr, .. } => format_expr(expr),
        Constructor::FreeNum { hint, .. } => format!("free ~{}", format_expr(hint)),
    }
}

fn format_assert(assertion: &AssertStmt) -> String {
    let mut output = format!("assert {}", format_expr(&assertion.expr));
    if let Some(tolerance) = &assertion.tolerance {
        let _ = write!(output, " tol {}", format_number(tolerance));
    }
    output
}

fn format_meta(meta: &MetaBlock) -> Vec<String> {
    let mut lines = vec!["meta {".to_owned()];
    lines.extend(meta.entries.iter().map(format_meta_entry));
    lines.push("}".to_owned());
    lines
}

fn format_meta_entry(entry: &MetaEntry) -> String {
    format!("  {} {}", entry.key.name, format_quoted_text(&entry.value))
}

fn format_class(class: &ClassStmt) -> String {
    format!("class {} {}", class.name.name, format_props(&class.props))
}

fn format_props(props: &Props) -> String {
    let items = props
        .items
        .iter()
        .map(|prop| format!("{}: {}", prop.name.name, format_prop_value(&prop.value)))
        .collect::<Vec<_>>()
        .join(", ");
    format!("{{ {items} }}")
}

fn format_prop_value(value: &PropValue) -> String {
    if let PropValue::Expr(expr) = value {
        return format_expr(expr);
    }
    if let PropValue::Text(text) = value {
        return format_text_value(text);
    }
    if let PropValue::Head(head) = value {
        return format_head(head.kind).to_owned();
    }
    let PropValue::HexColor(color) = value else {
        unreachable!("all property variants handled")
    };
    format!("#{}", color.value)
}

fn format_head(kind: HeadKind) -> &'static str {
    const SPELLINGS: [&str; 3] = ["->", "<->", "-"];
    SPELLINGS[kind as usize]
}

fn format_text_value(value: &TextValue) -> String {
    match value.segments.as_slice() {
        [segment] if segment.kind == TextSegmentKind::Tex => format!("${}$", segment.source),
        _ => format_quoted_text(value),
    }
}

fn format_quoted_text(value: &TextValue) -> String {
    let mut output = String::from("\"");
    for segment in &value.segments {
        match segment.kind {
            TextSegmentKind::Plain => push_escaped_plain(&mut output, &segment.source),
            TextSegmentKind::Tex => {
                output.push('$');
                output.push_str(&segment.source);
                output.push('$');
            }
        }
    }
    output.push('"');
    output
}

fn push_escaped_plain(output: &mut String, source: &str) {
    output.push_str(
        &source
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('$', "\\$")
            .replace('\n', "\\n"),
    );
}

/// Formats one expression with canonical spacing and necessary parentheses.
#[must_use]
pub(crate) fn format_expr(expr: &Expr) -> String {
    format_expr_at(expr, 0)
}

fn format_expr_at(expr: &Expr, parent_power: u8) -> String {
    if let Expr::Number(number) = expr {
        return format_number(number);
    }
    if let Expr::Boolean { value, .. } = expr {
        return value.to_string();
    }
    if let Expr::String(value) = expr {
        return format_quoted_text(value);
    }
    if let Expr::Tex(value) = expr {
        return format_text_value(value);
    }
    if let Expr::Ref(ident) = expr {
        return ident.name.clone();
    }
    if let Expr::Call { name, args, .. } = expr {
        return format_call(&name.name, args);
    }
    if let Expr::Property {
        base, name, args, ..
    } = expr
    {
        return format_property(base, &name.name, args);
    }
    if let Expr::Group { expr, .. } = expr {
        return format!("({})", format_expr(expr));
    }
    if let Expr::Unary { op, expr, .. } = expr {
        return format_unary(*op, expr, parent_power);
    }
    let Expr::Binary { op, lhs, rhs, .. } = expr else {
        unreachable!("all expression variants handled")
    };
    format_binary(*op, lhs, rhs, parent_power)
}

fn format_call(name: &str, args: &[Expr]) -> String {
    format!("{}({})", name, format_expr_list(args))
}

fn format_expr_list(args: &[Expr]) -> String {
    args.iter().map(format_expr).collect::<Vec<_>>().join(", ")
}

fn format_property(base: &Expr, name: &str, args: &[Expr]) -> String {
    let mut output = format!("{}.{}", format_expr_at(base, 7), name);
    if !args.is_empty() {
        let _ = write!(output, "({})", format_expr_list(args));
    }
    output
}

fn format_unary(op: UnaryOp, expr: &Expr, parent_power: u8) -> String {
    let power = if op == UnaryOp::Not { 3 } else { 6 };
    let operator = if op == UnaryOp::Not { "not " } else { "-" };
    let output = format!("{operator}{}", format_expr_at(expr, power));
    parenthesize(output, power, parent_power)
}

fn format_binary(op: BinOp, lhs: &Expr, rhs: &Expr, parent_power: u8) -> String {
    let power = binary_power(op);
    let output = format!(
        "{} {} {}",
        format_expr_at(lhs, power),
        binary_spelling(op),
        format_expr_at(rhs, power + 1)
    );
    parenthesize(output, power, parent_power)
}

fn parenthesize(output: String, power: u8, parent_power: u8) -> String {
    if power < parent_power {
        format!("({output})")
    } else {
        output
    }
}

fn binary_power(op: BinOp) -> u8 {
    const POWERS: [u8; 11] = [4, 4, 5, 5, 3, 3, 3, 3, 3, 2, 1];
    POWERS[op as usize]
}

fn binary_spelling(op: BinOp) -> &'static str {
    const SPELLINGS: [&str; 11] = ["+", "-", "*", "/", "==", "<", ">", "<=", ">=", "and", "or"];
    SPELLINGS[op as usize]
}

fn format_number(number: &crate::ast::NumberExpr) -> String {
    let mut output = number.raw.clone();
    match number.unit {
        Some(AngleUnit::Deg) => output.push_str("deg"),
        Some(AngleUnit::Rad) => output.push_str("rad"),
        None => {}
    }
    output
}

fn finish_lines(mut lines: Vec<String>) -> String {
    while lines.last().is_some_and(String::is_empty) {
        lines.pop();
    }
    lines.push(String::new());
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::{format, header_line};
    use crate::parse;

    #[test]
    fn aligns_ops_and_preserves_comments() {
        let source = "// lead\nlocus 0 beta 4\npoint A=point()// hint\nsegment long_name=seg(A,A)\n\nassert true == true\n";
        let output = format(source);
        assert!(output.diagnostics.is_empty(), "{:#?}", output.diagnostics);
        assert_eq!(
            output.source,
            "// lead\nlocus 0 beta 4\npoint A           = point()  // hint\nsegment long_name = seg(A, A)\n\nassert true == true\n"
        );
    }

    #[test]
    fn is_idempotent_with_tex_and_comment_markers() {
        let source = "locus 0 beta 4\ntext_block t=text_block(\"https://x/$a$\")// keep\n";
        let once = format(source).source;
        let twice = format(&once).source;
        assert_eq!(once, twice);
    }

    #[test]
    fn header_lookup_requires_a_complete_keyword() {
        assert_eq!(header_line(&["locusfoo 2", "locus 0 beta 4"]), Some(2));
    }

    #[test]
    fn preserves_statements_from_bare_carriage_return_lines() {
        let source = "locus 0 beta 4\rpoint A = point()\r";
        let formatted = format(source);
        assert!(
            formatted.diagnostics.is_empty(),
            "{:#?}",
            formatted.diagnostics
        );
        assert_eq!(formatted.source, "locus 0 beta 4\npoint A = point()\n");
        let reparsed = parse(&formatted.source);
        assert!(
            reparsed.diagnostics.is_empty(),
            "{:#?}",
            reparsed.diagnostics
        );
        assert_eq!(reparsed.file.statements.len(), 1);
    }
}
