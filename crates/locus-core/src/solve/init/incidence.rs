//! Interior finite-carrier seeds for diagrams with layout ordering requirements.
use super::{BTreeMap, EvalError, Expr, PreparedEvaluation, UnknownClass, UnknownSystem};
use crate::eval::{env::Environment, expr::ExpressionEvaluator};
use crate::scene::{PathGeometry, ResolvedPath};
use crate::{BBox, Point, ResolvedObject, Segment};
use locus_syntax::{Constructor, Kind, Statement};

type Boxes = BTreeMap<String, BBox>;

pub(super) fn seed(
    prepared: &PreparedEvaluation<'_>,
    system: &mut UnknownSystem,
    sources: &[crate::solve::SourceRelation<'_>],
    boxes: &mut Boxes,
) -> Result<(), EvalError> {
    if !sources
        .iter()
        .any(|source| on_arguments(source.expression).is_some())
    {
        return Ok(());
    }
    for pass in 0..2 {
        align_expression_anchors(prepared, system, sources, boxes)?;
        extend_carriers(prepared, system, sources, boxes)?;
        super::captions::seed(prepared, system, sources, boxes)?;
        project_points(prepared, system, sources, boxes)?;
        if pass == 0 {
            refresh(prepared, system, boxes)?;
            for horizontal in [true, false] {
                super::seed_order_axis(prepared, system, sources, boxes, horizontal);
            }
            super::seed_containers(prepared, system, sources, boxes);
            super::seed_shared_coordinates(prepared, system, sources, boxes);
        }
    }
    Ok(())
}

fn refresh(
    prepared: &PreparedEvaluation<'_>,
    system: &UnknownSystem,
    boxes: &mut Boxes,
) -> Result<Environment, EvalError> {
    let trial = prepared.evaluate_trial(Some(&system.hints))?;
    boxes.clear();
    let mut env = Environment::new();
    for (id, object) in trial.objects() {
        if let Some(bounds) = object.bbox() {
            boxes.insert(id.name.clone(), bounds);
        }
        env.insert(id.clone(), object.clone())
            .map_err(|error| EvalError::new("E302", id.span, error.to_string()))?;
    }
    Ok(env)
}

fn evaluator<'a>(
    prepared: &'a PreparedEvaluation<'_>,
    env: &'a Environment,
) -> ExpressionEvaluator<'a> {
    ExpressionEvaluator::new(
        env,
        crate::Tolerance::angular_default(),
        prepared.dimensions(),
    )
    .with_metrics(prepared.inputs().metrics)
}

fn align_expression_anchors(
    prepared: &PreparedEvaluation<'_>,
    system: &mut UnknownSystem,
    sources: &[crate::solve::SourceRelation<'_>],
    boxes: &mut Boxes,
) -> Result<(), EvalError> {
    let env = refresh(prepared, system, boxes)?;
    let mut expressions = evaluator(prepared, &env);
    for source in sources {
        let Expr::Call { name, args, .. } = source.expression else {
            continue;
        };
        if !matches!(
            name.name.as_str(),
            "align_center_x" | "align_center_y" | "gap_x" | "gap_y"
        ) || args.len() != 2
        {
            continue;
        }
        let [anchor, Expr::Ref(target)] = args.as_slice() else {
            continue;
        };
        if matches!(anchor, Expr::Ref(_)) {
            continue;
        }
        let Some(reference) = expressions.eval_resolved_object(anchor)?.bbox() else {
            continue;
        };
        let Some(bounds) = boxes.get(&target.name) else {
            continue;
        };
        let center = anchored_center(
            &name.name,
            reference,
            *bounds,
            prepared.inputs().metrics.gap,
        );
        super::move_center(system, &target.name, center, boxes);
    }
    Ok(())
}

fn anchored_center(name: &str, reference: BBox, bounds: BBox, gap: f64) -> Point {
    let mut center = bounds.center();
    match name {
        "align_center_x" => center.x = reference.center().x,
        "align_center_y" => center.y = reference.center().y,
        "gap_x" => center.x = center.x.max(reference.max.x + gap + bounds.width() / 2.0),
        _ => center.y = center.y.min(reference.min.y - gap - bounds.height() / 2.0),
    }
    center
}

fn on_arguments(expression: &Expr) -> Option<(&str, &Expr)> {
    let Expr::Call { name, args, .. } = expression else {
        return None;
    };
    let [Expr::Ref(point), target] = args.as_slice() else {
        return None;
    };
    (name.name == "on").then_some((point.name.as_str(), target))
}

fn point_is_free(prepared: &PreparedEvaluation<'_>, system: &UnknownSystem, name: &str) -> bool {
    prepared
        .registry()
        .iter()
        .any(|op| op.id.name == name && op.kind == Kind::Point)
        && system.objects.iter().any(|object| {
            object.id == name && object.class == UnknownClass::Geometric && object.count == 2
        })
}

fn extend_carriers(
    prepared: &PreparedEvaluation<'_>,
    system: &mut UnknownSystem,
    sources: &[crate::solve::SourceRelation<'_>],
    boxes: &mut Boxes,
) -> Result<(), EvalError> {
    let env = refresh(prepared, system, boxes)?;
    let mut expressions = evaluator(prepared, &env);
    for source in sources {
        let Some((point, target)) = on_arguments(source.expression) else {
            continue;
        };
        let Some(bounds) = boxes.get(point).copied() else {
            continue;
        };
        let ResolvedObject::Path(ResolvedPath {
            geometry: PathGeometry::Segment(mut segment),
            ..
        }) = expressions.eval_resolved_object(target)?
        else {
            continue;
        };
        let Some(finish) = carrier_finish(prepared, target) else {
            continue;
        };
        if !point_is_free(prepared, system, finish) {
            continue;
        }
        if let Some(bounds) = boxes.get(finish) {
            segment.end = bounds.center();
        }
        let endpoint = extended_endpoint(segment, bounds.center(), prepared.inputs().metrics.gap);
        super::move_center(system, finish, endpoint, boxes);
    }
    Ok(())
}

fn carrier_finish<'a>(prepared: &'a PreparedEvaluation<'_>, target: &Expr) -> Option<&'a str> {
    let Expr::Ref(id) = target else {
        return None;
    };
    let op = prepared
        .file()
        .statements
        .iter()
        .find_map(|statement| match statement {
            Statement::Op(op) if op.id.name == id.name => Some(op),
            _ => None,
        })?;
    let Constructor::Call { args, .. } = &op.ctor else {
        return None;
    };
    let [_, Expr::Ref(finish)] = args.as_slice() else {
        return None;
    };
    Some(&finish.name)
}

fn extended_endpoint(segment: Segment, point: Point, gap: f64) -> Point {
    let direction = segment.end - segment.start;
    let length_squared = direction.dot(direction);
    if length_squared == 0.0 {
        return segment.end;
    }
    let parameter = (point - segment.start).dot(direction) / length_squared;
    if parameter < 1.0 {
        return segment.end;
    }
    let factor = parameter + gap / libm::sqrt(length_squared);
    segment.start + direction * factor
}

fn project_points(
    prepared: &PreparedEvaluation<'_>,
    system: &mut UnknownSystem,
    sources: &[crate::solve::SourceRelation<'_>],
    boxes: &mut Boxes,
) -> Result<(), EvalError> {
    let env = refresh(prepared, system, boxes)?;
    let mut expressions = evaluator(prepared, &env);
    for source in sources {
        let Some((point, target)) = on_arguments(source.expression) else {
            continue;
        };
        if !point_is_free(prepared, system, point) {
            continue;
        }
        let Some(bounds) = boxes.get(point).copied() else {
            continue;
        };
        let ResolvedObject::Path(ResolvedPath {
            geometry: PathGeometry::Segment(segment),
            ..
        }) = expressions.eval_resolved_object(target)?
        else {
            continue;
        };
        super::move_center(
            system,
            point,
            interior_projection(segment, bounds.center()),
            boxes,
        );
    }
    Ok(())
}

fn interior_projection(segment: Segment, point: Point) -> Point {
    let direction = segment.end - segment.start;
    let length_squared = direction.dot(direction);
    if length_squared == 0.0 {
        return point;
    }
    let parameter = ((point - segment.start).dot(direction) / length_squared).clamp(0.01, 0.99);
    segment.start + direction * parameter
}
