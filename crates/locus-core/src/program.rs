//! Expansion boundary shared by direct evaluation and relational solving.

use locus_syntax::{ExpandOutput, File, Statement, expand};

use crate::EvalError;

pub(crate) fn expand_program(file: &File) -> Result<ExpandOutput, EvalError> {
    let _expand = crate::metrics::span("expand");
    let expanded = expand(file);
    if let Some(diagnostic) = expanded.diagnostics.first() {
        let mut error = EvalError::new(diagnostic.code, diagnostic.span, &diagnostic.message);
        if !diagnostic.notes.is_empty() {
            error.message.push_str(": ");
            error.message.push_str(&diagnostic.notes.join("; "));
        }
        return Err(error);
    }
    Ok(expanded)
}

pub(crate) fn annotate_error(mut error: EvalError, expanded: &ExpandOutput) -> EvalError {
    let index = error.object_id.as_ref().and_then(|id| {
        expanded.file.statements.iter().position(
            |statement| matches!(statement, Statement::Op(op) if op.id.name == id.as_ref()),
        )
    });
    let index = index.or_else(|| {
        expanded.origins.iter().position(|origin| {
            origin.span.byte_start <= error.span.byte_start
                && error.span.byte_end <= origin.span.byte_end
        })
    });
    if let Some(origin) = index.and_then(|index| expanded.origins.get(index)) {
        error
            .invocation_chain
            .as_mut()
            .clone_from(&origin.invocations);
    }
    error
}

pub(crate) fn alias_identities(file: &File) -> std::collections::BTreeMap<String, String> {
    let mut aliases = std::collections::BTreeMap::new();
    for statement in &file.statements {
        if let Statement::Op(op) = statement
            && let locus_syntax::Constructor::Expr { expr, .. } = &op.ctor
            && let locus_syntax::Expr::Ref(id) = expr.as_ref()
        {
            let identity = aliases
                .get(&id.name)
                .cloned()
                .unwrap_or_else(|| id.name.clone());
            aliases.insert(op.id.name.clone(), identity);
        }
    }
    aliases
}

/// Rows/columns expand groups, retain composed instances, and deduplicate identities.
pub(crate) fn chain_operands(
    collections: &std::collections::BTreeMap<String, locus_syntax::Collection>,
    aliases: &std::collections::BTreeMap<String, String>,
    args: &[locus_syntax::Expr],
) -> Vec<locus_syntax::Expr> {
    let mut result = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for argument in args {
        for mut item in chain_argument(collections, argument) {
            if let locus_syntax::Expr::Ref(id) = &mut item {
                if let Some(identity) = aliases.get(&id.name) {
                    id.name.clone_from(identity);
                }
                if !seen.insert(id.name.clone()) {
                    continue;
                }
            }
            result.push(item);
        }
    }
    result
}

fn chain_argument(
    collections: &std::collections::BTreeMap<String, locus_syntax::Collection>,
    argument: &locus_syntax::Expr,
) -> Vec<locus_syntax::Expr> {
    use locus_syntax::{CollectionKind, Expr, Ident};
    if let Expr::Group { expr, .. } = argument {
        return chain_argument(collections, expr);
    }
    if let Expr::Ref(id) = argument
        && let Some(collection) = collections.get(&id.name)
        && collection.kind == CollectionKind::Group
    {
        return collection
            .members
            .iter()
            .map(|name| {
                Expr::Ref(Ident {
                    name: name.clone(),
                    span: argument.span(),
                })
            })
            .collect();
    }
    vec![argument.clone()]
}

/// Avoidance expands every nested collection into visible member identities.
pub(crate) fn avoidance_operands(
    collections: &std::collections::BTreeMap<String, locus_syntax::Collection>,
    aliases: &std::collections::BTreeMap<String, String>,
    args: &[locus_syntax::Expr],
) -> Vec<locus_syntax::Expr> {
    let mut result = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for argument in args {
        append_avoidance_operand(collections, aliases, argument, &mut seen, &mut result);
    }
    result
}

fn append_avoidance_operand(
    collections: &std::collections::BTreeMap<String, locus_syntax::Collection>,
    aliases: &std::collections::BTreeMap<String, String>,
    argument: &locus_syntax::Expr,
    seen: &mut std::collections::BTreeSet<String>,
    result: &mut Vec<locus_syntax::Expr>,
) {
    use locus_syntax::{Expr, Ident};
    if let Expr::Group { expr, .. } = argument {
        append_avoidance_operand(collections, aliases, expr, seen, result);
        return;
    }
    let Expr::Ref(id) = argument else {
        result.push(argument.clone());
        return;
    };
    let identity = avoidance_identity(aliases, &id.name);
    if !seen.insert(identity.to_owned()) {
        return;
    }
    if let Some(collection) = collections.get(identity) {
        for member in &collection.members {
            let member = Expr::Ref(Ident {
                name: member.clone(),
                span: argument.span(),
            });
            append_avoidance_operand(collections, aliases, &member, seen, result);
        }
    } else {
        result.push(Expr::Ref(Ident {
            name: identity.to_owned(),
            span: id.span,
        }));
    }
}

fn avoidance_identity<'a>(
    aliases: &'a std::collections::BTreeMap<String, String>,
    name: &'a str,
) -> &'a str {
    let mut identity = name;
    for _ in 0..aliases.len() {
        let Some(alias) = aliases.get(identity) else {
            break;
        };
        identity = alias;
    }
    identity
}
