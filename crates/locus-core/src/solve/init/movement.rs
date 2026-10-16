//! Translation ownership for derived paths, shared by ordering and placement.
use super::{BTreeMap, BTreeSet, Expr, PreparedEvaluation, UnknownClass, UnknownSystem};
use locus_syntax::{Constructor, Kind, Statement};

pub(super) fn index(
    prepared: &PreparedEvaluation<'_>,
    system: &UnknownSystem,
) -> BTreeMap<String, BTreeSet<String>> {
    let mut result = BTreeMap::new();
    let mut visiting = BTreeSet::new();
    for statement in &prepared.file().statements {
        if let Statement::Op(op) = statement {
            owners(prepared, system, &op.id.name, &mut visiting, &mut result);
        }
    }
    result
}

fn owners(
    prepared: &PreparedEvaluation<'_>,
    system: &UnknownSystem,
    name: &str,
    visiting: &mut BTreeSet<String>,
    cache: &mut BTreeMap<String, BTreeSet<String>>,
) -> BTreeSet<String> {
    if let Some(found) = cache.get(name) {
        return found.clone();
    }
    if !visiting.insert(name.to_owned()) {
        return BTreeSet::new();
    }
    let found = if system.objects.iter().any(|o| {
        o.id == name
            && o.count == 2
            && matches!(
                o.class,
                UnknownClass::Geometric | UnknownClass::LabelPlacement
            )
    }) {
        BTreeSet::from([name.to_owned()])
    } else {
        derived(prepared, system, name, visiting, cache)
    };
    visiting.remove(name);
    cache.insert(name.to_owned(), found.clone());
    found
}

fn derived(
    prepared: &PreparedEvaluation<'_>,
    system: &UnknownSystem,
    name: &str,
    visiting: &mut BTreeSet<String>,
    cache: &mut BTreeMap<String, BTreeSet<String>>,
) -> BTreeSet<String> {
    let Some(op) = prepared.file().statements.iter().find_map(|s| match s {
        Statement::Op(op) if op.id.name == name => Some(op),
        _ => None,
    }) else {
        return BTreeSet::new();
    };
    match &op.ctor {
        Constructor::Expr { expr, .. } => expression(prepared, system, expr, visiting, cache),
        Constructor::Call { args, .. }
            if op.kind == Kind::Segment && args.iter().all(|arg| matches!(arg, Expr::Ref(_))) =>
        {
            let parts: Vec<_> = args
                .iter()
                .map(|arg| expression(prepared, system, arg, visiting, cache))
                .collect();
            if parts.iter().any(BTreeSet::is_empty) {
                BTreeSet::new()
            } else {
                parts.into_iter().flatten().collect()
            }
        }
        _ => BTreeSet::new(),
    }
}

fn expression(
    prepared: &PreparedEvaluation<'_>,
    system: &UnknownSystem,
    expression: &Expr,
    visiting: &mut BTreeSet<String>,
    cache: &mut BTreeMap<String, BTreeSet<String>>,
) -> BTreeSet<String> {
    match expression {
        Expr::Ref(id) => owners(prepared, system, &id.name, visiting, cache),
        Expr::Property { base, .. } | Expr::Group { expr: base, .. } => {
            expression_owners(prepared, system, base, visiting, cache)
        }
        _ => BTreeSet::new(),
    }
}

fn expression_owners(
    prepared: &PreparedEvaluation<'_>,
    system: &UnknownSystem,
    expr: &Expr,
    visiting: &mut BTreeSet<String>,
    cache: &mut BTreeMap<String, BTreeSet<String>>,
) -> BTreeSet<String> {
    expression(prepared, system, expr, visiting, cache)
}

#[cfg(test)]
mod tests {
    use super::super::tests::seeded;

    #[test]
    fn ordering_translates_a_derived_divider_through_both_endpoints() {
        let (_, b) = seeded(
            "locus 0 beta 6\nrect upper(2,1)\npoint left()\npoint right()\nrel align_center_y(left,right)\nsegment divider(left,right)\nrect lower(2,1)\nrel gap_y(upper,divider,lower)\n",
        );
        assert!(b["upper"].min.y - b["divider"].max.y >= 0.25 - 1e-12);
        assert!(b["divider"].min.y - b["lower"].max.y >= 0.25 - 1e-12);
        assert!((b["left"].center().y - b["right"].center().y).abs() < 1e-12);
    }
}
