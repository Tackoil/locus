//! Semantic leaf lookup for navigation and hover requests.

use locus_syntax::{Constructor, Expr, File, Ident, PropValue, Statement};

pub(super) enum Target<'a> {
    Object(&'a Ident),
    Class(&'a Ident),
    Callable(&'a Ident),
}

pub(super) fn at(file: &File, byte: usize) -> Option<Target<'_>> {
    file.statements
        .iter()
        .find_map(|statement| statement_target(statement, byte))
}

pub(super) fn object_definition<'a>(
    file: &'a File,
    name: &str,
) -> Option<&'a locus_syntax::OpStmt> {
    file.statements
        .iter()
        .find_map(|statement| match statement {
            Statement::Op(op) if op.id.name == name => Some(op),
            _ => None,
        })
}

pub(super) fn class_definition<'a>(
    file: &'a File,
    name: &str,
) -> Option<&'a locus_syntax::ClassStmt> {
    file.statements
        .iter()
        .find_map(|statement| match statement {
            Statement::Class(class) if class.name.name == name => Some(class),
            _ => None,
        })
}

fn statement_target(statement: &Statement, byte: usize) -> Option<Target<'_>> {
    match statement {
        Statement::Op(op) => op_target(op, byte),
        Statement::Rel(value) => expr_target(&value.expr, byte),
        Statement::Assert(value) => expr_target(&value.expr, byte),
        Statement::Class(value) => ident_target(&value.name, byte, Target::Class)
            .or_else(|| props_target(&value.props, byte)),
        Statement::Meta(_) | Statement::Style(_) => None,
    }
}

fn op_target(op: &locus_syntax::OpStmt, byte: usize) -> Option<Target<'_>> {
    ident_target(&op.id, byte, Target::Object)
        .or_else(|| constructor_target(&op.ctor, byte))
        .or_else(|| {
            op.where_clauses
                .iter()
                .find_map(|clause| expr_target(clause, byte))
        })
        .or_else(|| {
            op.attr
                .as_ref()
                .and_then(|attr| ident_target(&attr.name, byte, Target::Class))
        })
        .or_else(|| props_target(&op.props, byte))
}

fn constructor_target(constructor: &Constructor, byte: usize) -> Option<Target<'_>> {
    match constructor {
        Constructor::Call { name, args, .. } => ident_target(name, byte, Target::Callable)
            .or_else(|| args.iter().find_map(|arg| expr_target(arg, byte))),
        Constructor::Expr { expr, .. } | Constructor::FreeNum { hint: expr, .. } => {
            expr_target(expr, byte)
        }
        Constructor::FreePoint { x, y, .. } => {
            expr_target(x, byte).or_else(|| expr_target(y, byte))
        }
    }
}

fn props_target(props: &locus_syntax::Props, byte: usize) -> Option<Target<'_>> {
    props.items.iter().find_map(|prop| match &prop.value {
        PropValue::Expr(expr) => expr_target(expr, byte),
        PropValue::Text(_) | PropValue::Head(_) | PropValue::HexColor(_) => None,
    })
}

fn expr_target(expression: &Expr, byte: usize) -> Option<Target<'_>> {
    if let Some(target) = leaf_target(expression, byte) {
        return Some(target);
    }
    compound_target(expression, byte)
}

fn leaf_target(expression: &Expr, byte: usize) -> Option<Target<'_>> {
    match expression {
        Expr::Ref(id) => ident_target(id, byte, Target::Object),
        Expr::Number(_)
        | Expr::Boolean { .. }
        | Expr::String(_)
        | Expr::Tex(_)
        | Expr::Call { .. }
        | Expr::Property { .. }
        | Expr::Group { .. }
        | Expr::Unary { .. }
        | Expr::Binary { .. } => None,
    }
}

fn compound_target(expression: &Expr, byte: usize) -> Option<Target<'_>> {
    match expression {
        Expr::Call { name, args, .. } => ident_target(name, byte, Target::Callable)
            .or_else(|| args.iter().find_map(|arg| expr_target(arg, byte))),
        Expr::Property { base, args, .. } => {
            expr_target(base, byte).or_else(|| args.iter().find_map(|arg| expr_target(arg, byte)))
        }
        Expr::Group { expr, .. } | Expr::Unary { expr, .. } => expr_target(expr, byte),
        Expr::Binary { lhs, rhs, .. } => expr_target(lhs, byte).or_else(|| expr_target(rhs, byte)),
        Expr::Number(_) | Expr::Boolean { .. } | Expr::String(_) | Expr::Tex(_) | Expr::Ref(_) => {
            None
        }
    }
}

fn ident_target<'a>(
    ident: &'a Ident,
    byte: usize,
    make: impl FnOnce(&'a Ident) -> Target<'a>,
) -> Option<Target<'a>> {
    (ident.span.byte_start <= byte && byte < ident.span.byte_end).then(|| make(ident))
}
