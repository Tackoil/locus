//! Cheap propagation of automatic dimensions and straight-carrier lengths.
use super::{
    BTreeMap, BTreeSet, EvalError, Expr, PreparedEvaluation, UnknownClass, UnknownSystem,
    movable_names, move_center, operand_names, refresh_derived_boxes,
};
use crate::eval::{env::Environment, expr::ExpressionEvaluator};
use locus_syntax::{BinOp, Constructor};

pub(super) fn seed(
    prepared: &PreparedEvaluation<'_>,
    system: &mut UnknownSystem,
    sources: &[super::super::SourceRelation<'_>],
    boxes: &mut BTreeMap<String, crate::BBox>,
) -> Result<(), EvalError> {
    let trial = prepared.evaluate_trial(Some(&system.hints))?;
    let mut env = Environment::new();
    for (id, object) in trial.objects() {
        env.insert(id.clone(), object.clone())
            .map_err(|e| EvalError::new("E302", id.span, e.to_string()))?;
    }
    let mut evaluator = ExpressionEvaluator::new(&env, trial.tolerance(), prepared.dimensions())
        .with_metrics(prepared.inputs().metrics);
    let mut graph = BTreeMap::<usize, Vec<usize>>::new();
    let mut pins = BTreeMap::<usize, Vec<f64>>::new();
    for source in sources {
        if let Expr::Binary {
            op: BinOp::Eq,
            lhs,
            rhs,
            ..
        } = source.expression
        {
            dimension_equation(system, lhs, rhs, &mut evaluator, &mut graph, &mut pins);
            seed_length(prepared, system, lhs, rhs, &mut evaluator, boxes);
            seed_length(prepared, system, rhs, lhs, &mut evaluator, boxes);
        }
    }
    propagate(system, &graph, &pins);
    refresh_derived_boxes(prepared, system, boxes);
    Ok(())
}

fn dimension_index(system: &UnknownSystem, expression: &Expr) -> Option<usize> {
    let Expr::Property { base, name, .. } = expression else {
        return None;
    };
    let Expr::Ref(id) = base.as_ref() else {
        return None;
    };
    let axis = match name.name.as_str() {
        "width" => 0,
        "height" => 1,
        _ => return None,
    };
    system
        .objects
        .iter()
        .find(|o| o.id == id.name && o.class == UnknownClass::AutomaticDimension)
        .map(|o| o.start + axis)
}

fn constant(expression: &Expr) -> bool {
    match expression {
        Expr::Number(_) => true,
        Expr::Call { name, .. } => name.name == "style_metric",
        Expr::Group { expr, .. } | Expr::Unary { expr, .. } => constant(expr),
        Expr::Binary { lhs, rhs, .. } => constant(lhs) && constant(rhs),
        _ => false,
    }
}

fn dimension_equation(
    system: &UnknownSystem,
    lhs: &Expr,
    rhs: &Expr,
    evaluator: &mut ExpressionEvaluator<'_>,
    graph: &mut BTreeMap<usize, Vec<usize>>,
    pins: &mut BTreeMap<usize, Vec<f64>>,
) {
    match (dimension_index(system, lhs), dimension_index(system, rhs)) {
        (Some(a), Some(b)) => {
            graph.entry(a).or_default().push(b);
            graph.entry(b).or_default().push(a);
        }
        (Some(a), None) => pin(a, rhs, evaluator, graph, pins),
        (None, Some(b)) => pin(b, lhs, evaluator, graph, pins),
        _ => {}
    }
}

fn pin(
    index: usize,
    expression: &Expr,
    evaluator: &mut ExpressionEvaluator<'_>,
    graph: &mut BTreeMap<usize, Vec<usize>>,
    pins: &mut BTreeMap<usize, Vec<f64>>,
) {
    if constant(expression)
        && let Ok(value) = evaluator.eval_number(expression, crate::Dimension::Length)
        && value.is_finite()
        && value > 0.0
    {
        graph.entry(index).or_default();
        pins.entry(index).or_default().push(value);
    }
}

fn propagate(
    system: &mut UnknownSystem,
    graph: &BTreeMap<usize, Vec<usize>>,
    pins: &BTreeMap<usize, Vec<f64>>,
) {
    let mut visited = BTreeSet::<usize>::new();
    for &start in graph.keys() {
        if visited.contains(&start) {
            continue;
        }
        let members = component(graph, start);
        visited.extend(&members);
        let fixed: Vec<_> = members
            .iter()
            .flat_map(|i| pins.get(i).into_iter().flatten())
            .copied()
            .collect();
        let target = fixed
            .first()
            .copied()
            .unwrap_or_else(|| members.iter().map(|i| system.hints[*i]).fold(0.0, f64::max));
        if fixed.iter().any(|v| (*v - target).abs() > 1e-12) {
            continue;
        }
        for index in members {
            system.hints[index] = target;
        }
    }
}

fn component(graph: &BTreeMap<usize, Vec<usize>>, start: usize) -> BTreeSet<usize> {
    let mut members = BTreeSet::new();
    let mut pending = vec![start];
    while let Some(index) = pending.pop() {
        if members.insert(index) {
            pending.extend(&graph[&index]);
        }
    }
    members
}

fn seed_length(
    prepared: &PreparedEvaluation<'_>,
    system: &mut UnknownSystem,
    lhs: &Expr,
    rhs: &Expr,
    evaluator: &mut ExpressionEvaluator<'_>,
    boxes: &mut BTreeMap<String, crate::BBox>,
) {
    let Expr::Property { base, name, .. } = lhs else {
        return;
    };
    if name.name != "length" || !constant(rhs) {
        return;
    }
    let Expr::Ref(id) = base.as_ref() else { return };
    let Ok(crate::ResolvedObject::Path(path)) = evaluator.eval_resolved_object(base) else {
        return;
    };
    if path
        .route()
        .ok()
        .flatten()
        .is_none_or(|route| route.control.is_some() || !route.via.is_empty())
    {
        return;
    }
    let Some(op) = prepared
        .registry()
        .iter()
        .find(|op| op.id.name == id.name)
        .copied()
        .or_else(|| {
            prepared.file().statements.iter().find_map(|s| match s {
                locus_syntax::Statement::Op(op) if op.id.name == id.name => Some(op),
                _ => None,
            })
        })
    else {
        return;
    };
    let Constructor::Call { args, .. } = &op.ctor else {
        return;
    };
    let [Expr::Ref(a), Expr::Ref(b)] = args.as_slice() else {
        return;
    };
    if !movable_names(system).contains(&b.name) {
        return;
    }
    let (Some(abox), Some(bbox)) = (boxes.get(&a.name), boxes.get(&b.name)) else {
        return;
    };
    let Ok(length) = evaluator.eval_number(rhs, crate::Dimension::Length) else {
        return;
    };
    let delta =
        ordered_direction(prepared, &a.name, &b.name).unwrap_or(bbox.center() - abox.center());
    if length > 0.0 && delta.norm() > 1e-12 {
        let offset = delta * (length / delta.norm());
        let target = abox.center() + offset;
        system
            .seed_lengths
            .push((a.name.clone(), b.name.clone(), offset));
        move_center(system, &b.name, target, boxes);
    }
}

fn ordered_direction(
    prepared: &PreparedEvaluation<'_>,
    first: &str,
    second: &str,
) -> Option<crate::Vec2> {
    super::super::relations::collect(prepared.file())
        .iter()
        .find_map(|source| {
            let Expr::Call { name, args, .. } = source.expression else {
                return None;
            };
            let direction = match name.name.as_str() {
                "gap_x" | "row" => crate::Vec2::new(1.0, 0.0),
                "gap_y" | "column" => crate::Vec2::new(0.0, -1.0),
                _ => return None,
            };
            operand_names(args).windows(2).find_map(|pair| {
                if pair[0] == first && pair[1] == second {
                    Some(direction)
                } else if pair[0] == second && pair[1] == first {
                    Some(direction * -1.0)
                } else {
                    None
                }
            })
        })
}

#[cfg(test)]
mod tests {
    use super::super::tests::seeded;

    #[test]
    fn dimension_pins_propagate_across_reversed_equalities() {
        let (_, boxes) = seeded(
            "locus 0 beta 6\nrect a()\nrect b()\nrect c()\nrel c.width == b.width\nrel b.width == a.width\nrel a.width == 3\nrel row(a,b,c)\n",
        );
        for name in ["a", "b", "c"] {
            assert!((boxes[name].width() - 3.0).abs() < 1e-12);
        }
    }

    #[test]
    fn straight_length_seed_preserves_horizontal_alignment() {
        let (_, boxes) = seeded(
            "locus 0 beta 6\npoint a()\npoint b()\narrow sample(a,b)\nrel align_center_y(a,b)\nrel gap_x(a,b)\nrel sample.length == 1\n",
        );
        assert!((boxes["b"].center().x - boxes["a"].center().x - 1.0).abs() < 1e-12);
        assert!((boxes["b"].center().y - boxes["a"].center().y).abs() < 1e-12);
    }
}
