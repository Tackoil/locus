//! Carrier identities implied by explicit incidence and point constructions.

use super::{BTreeMap, Expr, PreparedEvaluation};
use locus_syntax::{Constructor, Kind, Statement};

pub(super) fn index(
    prepared: &PreparedEvaluation<'_>,
    sources: &[crate::solve::SourceRelation<'_>],
) -> BTreeMap<String, String> {
    let aliases = crate::program::alias_identities(prepared.file());
    let mut carriers = BTreeMap::new();
    for source in sources {
        if let Some((point, carrier)) = explicit(source.expression) {
            carriers
                .entry(identity(point, &aliases))
                .or_insert_with(|| identity(carrier, &aliases));
        }
    }
    for statement in &prepared.file().statements {
        let Statement::Op(op) = statement else {
            continue;
        };
        if op.kind != Kind::Point {
            continue;
        }
        if let Some(carrier) = constructed(&op.ctor, &carriers, &aliases) {
            carriers
                .entry(identity(&op.id.name, &aliases))
                .or_insert(carrier);
        }
    }
    carriers
}

fn constructed(
    constructor: &Constructor,
    carriers: &BTreeMap<String, String>,
    aliases: &BTreeMap<String, String>,
) -> Option<String> {
    match constructor {
        Constructor::Expr { expr, .. } => inferred(expr, carriers, aliases),
        Constructor::Call { name, args, .. } => infer_call(&name.name, args, carriers, aliases),
        Constructor::FreeNum { .. } => None,
    }
}

fn identity(name: &str, aliases: &BTreeMap<String, String>) -> String {
    aliases.get(name).map_or(name, String::as_str).to_owned()
}

fn explicit(expression: &Expr) -> Option<(&str, &str)> {
    let Expr::Call { name, args, .. } = expression else {
        return None;
    };
    let [Expr::Ref(point), Expr::Ref(carrier)] = args.as_slice() else {
        return None;
    };
    (name.name == "on").then_some((&point.name, &carrier.name))
}

fn inferred(
    expression: &Expr,
    carriers: &BTreeMap<String, String>,
    aliases: &BTreeMap<String, String>,
) -> Option<String> {
    match expression {
        Expr::Ref(id) => carriers.get(&identity(&id.name, aliases)).cloned(),
        Expr::Group { expr, .. } => inferred(expr, carriers, aliases),
        Expr::Call { name, args, .. } => infer_call(&name.name, args, carriers, aliases),
        _ => None,
    }
}

fn infer_call(
    name: &str,
    args: &[Expr],
    carriers: &BTreeMap<String, String>,
    aliases: &BTreeMap<String, String>,
) -> Option<String> {
    match name {
        "foot" => {
            let [_, Expr::Ref(carrier)] = args else {
                return None;
            };
            Some(identity(&carrier.name, aliases))
        }
        "translate" => {
            let [point, from, to] = args else {
                return None;
            };
            let carrier = inferred(point, carriers, aliases)?;
            (inferred(from, carriers, aliases).as_ref() == Some(&carrier)
                && inferred(to, carriers, aliases).as_ref() == Some(&carrier))
            .then_some(carrier)
        }
        _ => None,
    }
}
