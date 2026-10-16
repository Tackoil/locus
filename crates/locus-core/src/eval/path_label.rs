//! Measured path text with continuous route-region and page-side offsets.
use locus_syntax::{Constructor, Expr, File, Kind, OpStmt, PropValue, Statement};

use super::{EvalError, FileEvaluator, UnknownCursor, eval_text, include_bbox};
use crate::geom::route::{Route, RoutePoint, RouteScalar, RouteSide};
use crate::scene::{LabelDirection, PathLabel};
use crate::{AnchorSpec, AnnotationSpec, BBox, Point, ResolvedObject};

pub(crate) fn path_name(op: &OpStmt) -> Option<&str> {
    if op.kind != Kind::Label {
        return None;
    }
    let Constructor::Call { args, .. } = &op.ctor else {
        return None;
    };
    let [path, _] = args.as_slice() else {
        return None;
    };
    reference_name(path)
}

fn reference_name(expression: &Expr) -> Option<&str> {
    match expression {
        Expr::Ref(id) => Some(&id.name),
        Expr::Group { expr, .. } => reference_name(expr),
        _ => None,
    }
}

pub(crate) fn owns_parameters(op: &OpStmt, file: &File) -> bool {
    let Some(path) = path_name(op) else {
        return false;
    };
    file.statements.iter().any(|statement| {
        matches!(statement, Statement::Op(target) if target.id.name == path && matches!(target.kind, Kind::Arrow | Kind::Segment))
    })
}

pub(crate) fn is_route(object: &ResolvedObject) -> bool {
    object.as_path().is_some()
}

pub(crate) fn attachments(file: &File) -> std::collections::BTreeMap<String, Vec<String>> {
    let aliases = crate::program::alias_identities(file);
    let mut attachments = std::collections::BTreeMap::<String, Vec<String>>::new();
    for statement in &file.statements {
        if let Statement::Op(op) = statement
            && owns_parameters(op, file)
            && let Some(path) = path_name(op)
        {
            attachments
                .entry(
                    aliases
                        .get(path)
                        .cloned()
                        .unwrap_or_else(|| path.to_owned()),
                )
                .or_default()
                .push(op.id.name.clone());
        }
    }
    for (alias, identity) in aliases {
        if let Some(labels) = attachments.get(&identity).cloned() {
            attachments.insert(alias, labels);
        }
    }
    attachments
}

pub(crate) fn retained_range(
    ranges: &std::collections::BTreeMap<String, (f64, f64)>,
    aliases: &std::collections::BTreeMap<String, String>,
    path: &str,
) -> (f64, f64) {
    let identity = aliases.get(path).map_or(path, String::as_str);
    ranges
        .iter()
        .filter(|(id, _)| aliases.get(*id).map_or(id.as_str(), String::as_str) == identity)
        .map(|(_, range)| *range)
        .reduce(|a, b| (a.0.max(b.0), a.1.min(b.1)))
        .unwrap_or((0.0, 1.0))
}

pub(crate) fn retained_parameters<S: RouteScalar>(
    route: &Route<S>,
    ranges: &std::collections::BTreeMap<String, (f64, f64)>,
    distances: &std::collections::BTreeMap<String, (f64, f64)>,
    aliases: &std::collections::BTreeMap<String, String>,
    path: &str,
) -> (S, S) {
    let identity = aliases.get(path).map_or(path, String::as_str);
    let physical = distances
        .iter()
        .filter(|(id, _)| aliases.get(*id).map_or(id.as_str(), String::as_str) == identity)
        .map(|(_, range)| *range)
        .reduce(|a, b| (a.0.max(b.0), a.1.max(b.1)));
    if let Some((prefix, suffix)) = physical {
        let start = route.from.x.constant(prefix);
        let end = route.length().sub(&route.from.x.constant(suffix));
        return (
            route.parameter_for_distance(&start),
            route.parameter_for_distance(&end),
        );
    }
    let range = retained_range(ranges, aliases, path);
    (
        route.from.x.constant(range.0),
        route.from.x.constant(range.1),
    )
}

fn token<'a>(op: &'a OpStmt, name: &str) -> Option<&'a str> {
    match super::prop(&op.props, name)? {
        PropValue::Expr(Expr::Ref(id)) => Some(&id.name),
        _ => None,
    }
}

pub(crate) fn region(op: &OpStmt) -> Result<(f64, f64), EvalError> {
    validate_token(op, "near")?;
    let index = match token(op, "near") {
        Some("start" | "source") => 0,
        None | Some("auto" | "middle") => 1,
        Some("end" | "target") => 2,
        _ => {
            return Err(EvalError::new(
                "E209",
                op.span,
                "label near requires start, middle, or end",
            ));
        }
    };
    Ok(crate::preset::LABEL_REGIONS[index])
}

pub(crate) fn side(op: &OpStmt) -> Result<RouteSide, EvalError> {
    validate_token(op, "side")?;
    match token(op, "side") {
        None | Some("auto" | "above") => Ok(RouteSide::Above),
        Some("below") => Ok(RouteSide::Below),
        Some("left") => Ok(RouteSide::Left),
        Some("right") => Ok(RouteSide::Right),
        _ => Err(EvalError::new(
            "E209",
            op.span,
            "label side requires above, below, left, or right",
        )),
    }
}

fn validate_token(op: &OpStmt, name: &str) -> Result<(), EvalError> {
    if super::prop(&op.props, name).is_some() && token(op, name).is_none() {
        Err(EvalError::new(
            "E209",
            op.span,
            format!("label {name} requires a direction token"),
        ))
    } else {
        Ok(())
    }
}

pub(crate) fn accepted_side(
    op: &OpStmt,
    choice: Option<&crate::inputs::LabelChoice>,
) -> Result<RouteSide, EvalError> {
    let authored = side(op)?;
    if has_fixed_side(op) {
        return Ok(authored);
    }
    Ok(choice.map_or(authored, |choice| choice.side))
}

pub(crate) fn has_fixed_side(op: &OpStmt) -> bool {
    token(op, "side").is_some_and(|value| value != "auto")
}

pub(crate) fn preferred_parameter(region: (f64, f64), candidate: usize) -> f64 {
    match candidate {
        1 => region.0,
        2 => region.1,
        _ => region.0.midpoint(region.1),
    }
}

pub(crate) fn center<S: RouteScalar>(
    route: &Route<S>,
    parameter: &S,
    offset: &S,
    side: RouteSide,
    extents: (f64, f64),
    retained: &(S, S),
) -> RoutePoint<S> {
    let visible_parameter = parameter.mul(&retained.1.sub(&retained.0)).add(&retained.0);
    let mut point = route.sample(&visible_parameter);
    match side {
        RouteSide::Right => point.x = point.x.add(&offset.add(&offset.constant(extents.0 / 2.0))),
        RouteSide::Left => point.x = point.x.sub(&offset.add(&offset.constant(extents.0 / 2.0))),
        RouteSide::Above => point.y = point.y.add(&offset.add(&offset.constant(extents.1 / 2.0))),
        RouteSide::Below => point.y = point.y.sub(&offset.add(&offset.constant(extents.1 / 2.0))),
    }
    point
}

impl FileEvaluator {
    pub(super) fn evaluate_path_label(
        &mut self,
        op: &OpStmt,
        unknowns: &mut UnknownCursor<'_>,
    ) -> Result<(), EvalError> {
        validate_overload_props(
            &op.props,
            &["dir", "gap"],
            "path labels accept near/side, not dir/gap",
        )?;
        let path =
            path_name(op).ok_or_else(|| EvalError::new("E401", op.span, "invalid path label"))?;
        let route = path_route(self.env.get(path), op.span)?;
        let region = region(op)?;
        let choice = self.inputs.choices.labels.get(&op.id.name);
        let side = accepted_side(op, choice)?;
        let candidate = choice.map_or(0, |choice| choice.candidate);
        let parameter = unknowns
            .next(op.span)?
            .unwrap_or(preferred_parameter(region, candidate));
        let offset = unknowns
            .next(op.span)?
            .unwrap_or(self.inputs.metrics.clearance);
        let measured = self.inputs.measured.get(&op.id.name).ok_or_else(|| {
            EvalError::new(
                "E410",
                op.span,
                format!("missing text measurement for {}", op.id.name),
            )
        })?;
        validate_measurement(*measured, op.span)?;
        let retained = retained_parameters(
            &route,
            &self.inputs.choices.retained_routes,
            &self.inputs.choices.retained_distances,
            &self.aliases,
            path,
        );
        let center: Point = center(
            &route,
            &parameter,
            &offset,
            side,
            (measured.width, measured.height),
            &retained,
        )
        .into();
        let bounds = BBox {
            min: Point::new(
                center.x - measured.width / 2.0,
                center.y - measured.height / 2.0,
            ),
            max: Point::new(
                center.x + measured.width / 2.0,
                center.y + measured.height / 2.0,
            ),
        };
        let Constructor::Call { args, .. } = &op.ctor else {
            unreachable!()
        };
        let object = ResolvedObject::Annotation(AnnotationSpec::Label {
            path: Some(PathLabel {
                path: self
                    .aliases
                    .get(path)
                    .cloned()
                    .unwrap_or_else(|| path.to_owned()),
                parameter,
                offset,
                side,
                region,
                candidate,
            }),
            bounds: Some(bounds),
            anchor: AnchorSpec::Point(center),
            text: eval_text(&args[1])?,
            direction: LabelDirection::Center,
            gap: None,
        });
        include_bbox(&mut self.bbox, bounds);
        self.env
            .insert(op.id.clone(), object)
            .map_err(|error| EvalError::new("E302", op.id.span, error.to_string()))
    }
}

pub(crate) fn validate_overload_props(
    props: &locus_syntax::Props,
    rejected: &[&str],
    message: &str,
) -> Result<(), EvalError> {
    if let Some(property) = props
        .items
        .iter()
        .find(|property| rejected.contains(&property.name.name.as_str()))
    {
        Err(EvalError::new("E131", property.span, message))
    } else {
        Ok(())
    }
}

fn validate_measurement(
    measured: crate::inputs::MeasuredText,
    span: locus_syntax::Span,
) -> Result<(), EvalError> {
    if measured.width.is_finite()
        && measured.height.is_finite()
        && measured.width >= 0.0
        && measured.height > 0.0
    {
        Ok(())
    } else {
        Err(EvalError::new(
            "E412",
            span,
            "invalid measured text dimensions",
        ))
    }
}

fn path_route(
    object: Option<&ResolvedObject>,
    span: locus_syntax::Span,
) -> Result<Route, EvalError> {
    object
        .and_then(ResolvedObject::as_path)
        .map(|path| {
            path.route()
                .map_err(|error| EvalError::construction(span, error))
        })
        .transpose()?
        .flatten()
        .ok_or_else(|| EvalError::new("E410", span, "path label requires a measured logical route"))
}
