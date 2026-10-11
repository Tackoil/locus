//! Differentiated measured bounds of accepted path-label placement.
use super::{
    AdEvaluator, DObject, DPoint, DValue, DifferentiatedFile, Jet, NumericDimension,
    RelationEvaluator, rect_vertices, solver_error,
};
use crate::eval::{EvalError, path_label};
use crate::geom::route::Route;
use locus_syntax::{Expr, OpStmt, Span};

fn reference_name(expression: &Expr) -> Option<&str> {
    match expression {
        Expr::Ref(id) => Some(&id.name),
        Expr::Group { expr, .. } => reference_name(expr),
        _ => None,
    }
}

fn attached_bounds(
    base: &Expr,
    route: &Route<Jet>,
    labels: &std::collections::BTreeMap<String, Vec<String>>,
    objects: &std::collections::BTreeMap<String, DObject>,
    count: usize,
) -> (DPoint, DPoint) {
    let (minimum, maximum) = route.bounds();
    let mut minimum: DPoint = minimum.into();
    let mut maximum: DPoint = maximum.into();
    for name in reference_name(base)
        .and_then(|name| labels.get(name))
        .into_iter()
        .flatten()
    {
        if let Some(DObject::Polygon(vertices, _, _)) = objects.get(name) {
            let (min, max) = super::polygon_bounds(vertices, count);
            minimum.x = minimum.x.min_live(&min.x);
            minimum.y = minimum.y.min_live(&min.y);
            maximum.x = maximum.x.max_live(&max.x);
            maximum.y = maximum.y.max_live(&max.y);
        }
    }
    (minimum, maximum)
}

impl DifferentiatedFile {
    /// Route geometry and captions keep distinct identities for hard bound faces.
    pub(crate) fn route_bound_contributors(
        &self,
        base: &Expr,
        route: &Route<Jet>,
    ) -> Vec<(String, DPoint, DPoint)> {
        let owner = reference_name(base).unwrap_or("<route>");
        let mut result: Vec<_> = route_bound_points(route)
            .into_iter()
            .enumerate()
            .map(|(index, point)| (format!("{owner}#route:{index}"), point.clone(), point))
            .collect();
        for name in reference_name(base)
            .and_then(|name| self.path_labels.get(name))
            .into_iter()
            .flatten()
        {
            if let Some(DObject::Polygon(vertices, _, _)) = self.objects.get(name) {
                let (minimum, maximum) = super::polygon_bounds(vertices, self.unknown_count);
                result.push((name.clone(), minimum, maximum));
            }
        }
        result
    }

    pub(crate) fn avoidance_object(&self, base: &Expr) -> Result<DObject, EvalError> {
        let object = self.object(base)?;
        if let DObject::Route(route) = &object
            && let Some(name) = reference_name(base)
        {
            let range = path_label::retained_parameters(
                route,
                &self.retained_routes,
                &self.retained_distances,
                &self.aliases,
                name,
            );
            return Ok(DObject::Route(
                route.retained_parameters((&range.0, &range.1)),
            ));
        }
        Ok(object)
    }

    pub(crate) fn route_bounds(&self, base: &Expr, route: &Route<Jet>) -> (DPoint, DPoint) {
        attached_bounds(
            base,
            route,
            &self.path_labels,
            &self.objects,
            self.unknown_count,
        )
    }
}

/// Endpoints, bends and two clamped quadratic extrema have fixed row identities.
/// Their union is exactly the route box, including when an extremum enters or
/// leaves the parameter interval. A clamped extremum becomes an endpoint.
fn route_bound_points(route: &Route<Jet>) -> Vec<DPoint> {
    let mut points: Vec<DPoint> = std::iter::once(route.from.clone())
        .chain(route.via.iter().cloned())
        .chain(std::iter::once(route.to.clone()))
        .map(Into::into)
        .collect();
    if let Some(control) = &route.control {
        for (from, control, to) in [
            (&route.from.x, &control.x, &route.to.x),
            (&route.from.y, &control.y, &route.to.y),
        ] {
            let denominator = from.sub(&control.scale(2.0)).add(to);
            let parameter = extremum_parameter(from, control, &denominator);
            points.push(route.sample(&parameter).into());
        }
    }
    points
}

fn extremum_parameter(from: &Jet, control: &Jet, denominator: &Jet) -> Jet {
    if denominator.value == 0.0 {
        return Jet::constant(0.0, from.gradient.len());
    }
    let parameter = from.sub(control).div(denominator);
    if parameter.value <= 0.0 {
        Jet::constant(0.0, from.gradient.len())
    } else if parameter.value >= 1.0 {
        Jet::constant(1.0, from.gradient.len())
    } else {
        parameter
    }
}

impl RelationEvaluator<'_> {
    pub(super) fn attached_route_property(
        &self,
        base: &Expr,
        route: &Route<Jet>,
        name: &str,
        span: Span,
    ) -> Result<DValue, EvalError> {
        if name == "length" {
            return Self::route_property(route, name, span);
        }
        let (min, max) = attached_bounds(base, route, &self.path_labels, self.objects, self.count);
        match name {
            "width" => Ok(DValue::Number(max.x.sub(&min.x), NumericDimension::Length)),
            "height" => Ok(DValue::Number(max.y.sub(&min.y), NumericDimension::Length)),
            _ => super::box_anchor(min, max, name, span)
                .map(|point| DValue::Object(DObject::Point(point))),
        }
    }
}

impl AdEvaluator<'_> {
    pub(super) fn path_label_object(&mut self, op: &OpStmt) -> Result<DObject, EvalError> {
        let path =
            path_label::path_name(op).ok_or_else(|| solver_error(op.span, "invalid path label"))?;
        let Some(DObject::Route(route)) = self.objects.get(path).cloned() else {
            return Err(solver_error(op.span, "path label requires a route"));
        };
        let parameter = self.variable(op.span)?;
        let offset = self.variable(op.span)?;
        let side = path_label::accepted_side(op, self.choices.labels.get(&op.id.name))?;
        let measured = self
            .measured
            .get(&op.id.name)
            .ok_or_else(|| solver_error(op.span, "missing path-label measurement"))?;
        let retained = path_label::retained_parameters(
            &route,
            &self.choices.retained_routes,
            &self.choices.retained_distances,
            &self.aliases,
            path,
        );
        let center = path_label::center(
            &route,
            &parameter,
            &offset,
            side,
            (measured.width, measured.height),
            &retained,
        )
        .into();
        let width = Jet::constant(measured.width, self.normalized.len());
        let height = Jet::constant(measured.height, self.normalized.len());
        Ok(DObject::Polygon(
            rect_vertices(&center, &width, &height),
            true,
            false,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geom::route::{RouteChoice, RouteKind, RoutePoint, RouteSide};

    fn placement(values: [f64; 7], kind: RouteKind, physical: bool) -> DPoint {
        let parameters: Vec<_> = values
            .iter()
            .enumerate()
            .map(|(index, value)| Jet::variable(*value, *value, index, 1.0, 7))
            .collect();
        let route = Route::automatic(
            RoutePoint {
                x: parameters[0].clone(),
                y: parameters[1].clone(),
            },
            RoutePoint {
                x: parameters[2].clone(),
                y: parameters[3].clone(),
            },
            kind,
            RouteChoice {
                side: RouteSide::Above,
                five_span: false,
            },
            parameters[6].clone(),
        );
        let retained = if physical {
            path_label::retained_parameters(
                &route,
                &std::collections::BTreeMap::new(),
                &std::collections::BTreeMap::from([("path".to_owned(), (0.3, 0.4))]),
                &std::collections::BTreeMap::new(),
                "path",
            )
        } else {
            (Jet::constant(0.1, 7), Jet::constant(0.9, 7))
        };
        path_label::center(
            &route,
            &parameters[4],
            &parameters[5],
            RouteSide::Right,
            (1.5, 0.5),
            &retained,
        )
        .into()
    }

    #[test]
    fn retained_label_jets_match_endpoint_route_and_label_finite_differences() {
        let values = [-1.0, -0.5, 4.0, 2.5, 0.45, 0.4, 1.2];
        for (kind, physical) in [
            (RouteKind::Curved, false),
            (RouteKind::Orthogonal, false),
            (RouteKind::Curved, true),
            (RouteKind::Orthogonal, true),
        ] {
            let point = placement(values, kind, physical);
            for index in 0..7 {
                let mut before = values;
                let mut after = values;
                before[index] -= 1e-6;
                after[index] += 1e-6;
                let before = placement(before, kind, physical);
                let after = placement(after, kind, physical);
                assert!(
                    (point.x.gradient[index] - (after.x.value - before.x.value) / 2e-6).abs()
                        < 1e-8
                );
                assert!(
                    (point.y.gradient[index] - (after.y.value - before.y.value) / 2e-6).abs()
                        < 1e-8
                );
            }
        }
    }
}
