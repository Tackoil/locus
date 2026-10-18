//! Shared stack-safety bounds for parsing and function expansion.

use crate::{Diagnostic, Expr, Span};

pub(crate) const MAX_SYNTAX_DEPTH: usize = 64;

pub(crate) fn depth_diagnostic(span: Span) -> Diagnostic {
    Diagnostic::new(
        "E161",
        span,
        format!("syntax nesting exceeds the limit of {MAX_SYNTAX_DEPTH}"),
    )
    .with_suggestion("split the expression or function composition into smaller definitions")
}

pub(crate) fn expression_depth_exceeded(expr: &Expr) -> bool {
    let mut pending = vec![(expr, 1)];
    while let Some((expr, depth)) = pending.pop() {
        if depth > MAX_SYNTAX_DEPTH {
            return true;
        }
        push_children(expr, depth + 1, &mut pending);
    }
    false
}

fn push_children<'a>(expr: &'a Expr, depth: usize, pending: &mut Vec<(&'a Expr, usize)>) {
    match expr {
        Expr::Call { args, .. } => pending.extend(args.iter().map(|arg| (arg, depth))),
        Expr::Property { base, args, .. } => {
            pending.push((base.as_ref(), depth));
            pending.extend(args.iter().map(|arg| (arg, depth)));
        }
        Expr::Group { expr, .. } | Expr::Unary { expr, .. } => {
            pending.push((expr.as_ref(), depth));
        }
        Expr::Binary { lhs, rhs, .. } => {
            pending.extend([(lhs.as_ref(), depth), (rhs.as_ref(), depth)]);
        }
        _ => {}
    }
}
