//! Source identities and relations shared by all snapshots.
use locus_syntax::{Constructor, Expr, Span, Statement};
use serde_json::{Value, json};
use std::collections::BTreeSet;

pub(super) fn build(source: &str) -> (Vec<Value>, Vec<Value>) {
    let parsed = locus_syntax::parse(source);
    let expanded = locus_syntax::expand(&parsed.file);
    let mut entities = Vec::new();
    let mut relations = Vec::new();
    for (index, statement) in expanded.file.statements.iter().enumerate() {
        let span = expanded
            .origins
            .get(index)
            .map_or(expanded.file.span, |origin| origin.span);
        append_statement(
            statement,
            source,
            span,
            expanded.origins.get(index),
            &mut entities,
            &mut relations,
        );
    }
    for collection in expanded.collections.values() {
        entities.push(
            json!({"id": collection.id, "source_name": collection.id.rsplit('.').next(),
            "kind": collection.kind,
            "members": collection.members, "span": collection.span,
            "source": snippet(source, collection.span), "origin": collection.origin}),
        );
        for member in &collection.members {
            relations.push(json!({"kind": "membership", "ids": [collection.id, member],
                "source": format!("{} contains {member}", collection.id), "span": collection.span}));
        }
    }
    (entities, relations)
}

fn append_statement(
    statement: &Statement,
    source: &str,
    span: Span,
    origin: Option<&locus_syntax::ExpansionOrigin>,
    entities: &mut Vec<Value>,
    relations: &mut Vec<Value>,
) {
    match statement {
        Statement::Op(op) => {
            entities.push(
                json!({"id": op.id.name, "source_name": snippet(source, op.id.span),
                "kind": op.kind.as_str(), "span": span,
                "source": snippet(source, span), "origin": origin}),
            );
            relations.extend(constructor_refs(&op.ctor).into_iter().map(|dependency| {
                json!({"kind": "dependency", "ids": [op.id.name, dependency],
                    "source": snippet(source, span), "span": span})
            }));
            relations.extend(op.where_clauses.iter().map(|expr| relation(expr, source)));
        }
        Statement::Rel(rel) => relations.push(relation(&rel.expr, source)),
        _ => {}
    }
}

fn constructor_refs(constructor: &Constructor) -> BTreeSet<String> {
    match constructor {
        Constructor::Call { args, .. } => args.iter().flat_map(references).collect(),
        Constructor::Expr { expr, .. } => references(expr),
        Constructor::FreeNum { hint, .. } => references(hint),
    }
}

fn relation(expr: &Expr, source: &str) -> Value {
    json!({"kind": "constraint", "ids": references(expr),
        "source": snippet(source, expr.span()), "span": expr.span(), "expression": expr})
}

fn references(expr: &Expr) -> BTreeSet<String> {
    if let Expr::Ref(id) = expr {
        return BTreeSet::from([id.name.clone()]);
    }
    children(expr).into_iter().flat_map(references).collect()
}

fn children(expr: &Expr) -> Vec<&Expr> {
    match expr {
        Expr::Call { args, .. } => args.iter().collect(),
        Expr::Property { base, args, .. } => std::iter::once(base.as_ref()).chain(args).collect(),
        Expr::Binary { lhs, rhs, .. } => vec![lhs, rhs],
        Expr::Unary { expr, .. } | Expr::Group { expr, .. } => vec![expr],
        _ => Vec::new(),
    }
}

fn snippet(source: &str, span: Span) -> &str {
    source.get(span.byte_start..span.byte_end).unwrap_or("")
}
