//! Differentiation of shared logical routes and diagram measurement expressions.

use super::{
    AdEvaluator, DLinear, DObject, DPoint, DValue, Jet, LinearDomain, NumericDimension,
    RelationEvaluator, require_arity, solver_error,
};
use crate::eval::EvalError;
use crate::geom::route::{Route, RoutePoint, RouteScalar};
use locus_syntax::{Constructor, Expr, OpStmt, PropValue, Span};

impl RouteScalar for Jet {
    fn value(&self) -> f64 {
        self.value
    }
    fn constant(&self, value: f64) -> Self {
        Self::constant(value, self.gradient.len())
    }
    fn add(&self, rhs: &Self) -> Self {
        self.add(rhs)
    }
    fn sub(&self, rhs: &Self) -> Self {
        self.sub(rhs)
    }
    fn mul(&self, rhs: &Self) -> Self {
        self.mul(rhs)
    }
    fn div(&self, rhs: &Self) -> Self {
        self.div(rhs)
    }
    fn scale(&self, factor: f64) -> Self {
        self.scale(factor)
    }
    fn sqrt(&self) -> Self {
        self.sqrt()
            .unwrap_or_else(|()| Self::constant(f64::NAN, self.gradient.len()))
    }
    fn asinh(&self) -> Self {
        let factor = 1.0 / libm::sqrt(1.0 + self.value * self.value);
        Self {
            value: libm::asinh(self.value),
            seed_value: libm::asinh(self.seed_value),
            gradient: self.gradient.iter().map(|value| value * factor).collect(),
        }
    }
}

impl From<DPoint> for RoutePoint<Jet> {
    fn from(point: DPoint) -> Self {
        Self {
            x: point.x,
            y: point.y,
        }
    }
}

impl From<RoutePoint<Jet>> for DPoint {
    fn from(point: RoutePoint<Jet>) -> Self {
        Self {
            x: point.x,
            y: point.y,
        }
    }
}

impl AdEvaluator<'_> {
    pub(super) fn eval_arrow_op(&mut self, op: &OpStmt) -> Result<DObject, EvalError> {
        let Constructor::Call { args, .. } = &op.ctor else {
            return self.relation_evaluator().eval_object(match &op.ctor {
                Constructor::Expr { expr, .. } => expr,
                _ => return Err(solver_error(op.span, "invalid arrow constructor")),
            });
        };
        if let Some(kind) = crate::eval::routing::route_kind(op) {
            let offset = self.variable(op.span)?;
            let choice = crate::eval::routing::accepted_choice(op, self.route_choices);
            let mut evaluator = self.relation_evaluator();
            require_arity("arrow", args, 2, op.span)?;
            let from = evaluator.eval_point(&args[0])?;
            let to = evaluator.eval_point(&args[1])?;
            return Ok(DObject::Route(Route::automatic(
                from.into(),
                to.into(),
                kind,
                choice,
                offset,
            )));
        }
        let mut evaluator = self.relation_evaluator();
        require_arity("arrow", args, 2, op.span)?;
        let bend = match op
            .props
            .items
            .iter()
            .find(|prop| prop.name.name == "bend")
            .map(|prop| &prop.value)
        {
            Some(PropValue::Expr(expr)) => {
                evaluator.eval_number_as(expr, NumericDimension::Angle)?.0
            }
            Some(value) => return Err(solver_error(value.span(), "arrow bend requires an angle")),
            None => Jet::constant(0.0, self.normalized.len()),
        };
        evaluator.eval_direct_arrow(args, &bend, op.span)
    }
}

impl RelationEvaluator<'_> {
    pub(super) fn eval_direct_arrow(
        &mut self,
        args: &[Expr],
        bend: &Jet,
        span: Span,
    ) -> Result<DObject, EvalError> {
        require_arity("arrow", args, 2, span)?;
        let from = self.eval_point(&args[0])?;
        let to = self.eval_point(&args[1])?;
        crate::geom::arrow::bend_tangent(bend.value)
            .map_err(|error| EvalError::construction(span, error))?;
        let control = if bend.value == 0.0 && bend.gradient.iter().all(|value| *value == 0.0) {
            None
        } else {
            let chord = to.sub(&from);
            let tangent = bend.sin().div(&bend.cos());
            let offset = chord.perpendicular().scale(&tangent.scale(0.5));
            Some(
                from.add_vector(&chord.scale(&Jet::constant(0.5, self.count)).add(&offset))
                    .into(),
            )
        };
        Ok(DObject::Route(Route {
            from: from.into(),
            to: to.into(),
            control,
            via: Vec::new(),
            parameter: None,
        }))
    }
}

impl RelationEvaluator<'_> {
    pub(super) fn linear_measure_or_property(
        linear: DLinear,
        name: &str,
        span: Span,
    ) -> Result<DValue, EvalError> {
        if name == "length" && linear.domain == LinearDomain::Segment {
            let length = linear
                .direction()
                .norm()
                .map_err(|()| solver_error(span, "segment length is not finite"))?;
            return Ok(DValue::Number(length, NumericDimension::Length));
        }
        super::linear_property(linear, name, span)
            .map(|point| DValue::Object(DObject::Point(point)))
    }

    pub(super) fn eval_style_metric(&self, args: &[Expr], span: Span) -> Result<DValue, EvalError> {
        require_arity("style_metric", args, 1, span)?;
        let name = crate::eval::expr::plain_metric_name(&args[0])?;
        let value = self
            .metrics
            .metric(&name)
            .ok_or_else(|| solver_error(span, format!("unknown style metric `{name}`")))?;
        Ok(DValue::Number(
            Jet::constant(value, self.count),
            NumericDimension::Length,
        ))
    }

    pub(super) fn eval_edge(&mut self, args: &[Expr], span: Span) -> Result<DObject, EvalError> {
        require_arity("edge", args, 2, span)?;
        let DObject::Polygon(vertices, _, true) = self.eval_object(&args[0])? else {
            return Err(solver_error(
                args[0].span(),
                "edge requires an axis-aligned rect",
            ));
        };
        let direction = crate::eval::expr::edge_direction(&args[1])?;
        let (min, max) = super::polygon_bounds(&vertices, self.count);
        let nw = DPoint {
            x: min.x.clone(),
            y: max.y.clone(),
        };
        let se = DPoint {
            x: max.x.clone(),
            y: min.y.clone(),
        };
        let (start, through) = match direction {
            "N" => (nw, max),
            "E" => (max, se),
            "S" => (se, min),
            _ => (min, nw),
        };
        Ok(DObject::Linear(DLinear {
            start,
            through,
            domain: LinearDomain::Segment,
            layout_live: true,
        }))
    }

    pub(super) fn route_property(
        route: &Route<Jet>,
        name: &str,
        span: Span,
    ) -> Result<DValue, EvalError> {
        if name == "length" {
            return Ok(DValue::Number(route.length(), NumericDimension::Length));
        }
        let (min, max) = route.bounds();
        if name == "width" {
            return Ok(DValue::Number(max.x.sub(&min.x), NumericDimension::Length));
        }
        if name == "height" {
            return Ok(DValue::Number(max.y.sub(&min.y), NumericDimension::Length));
        }
        super::box_anchor(min.into(), max.into(), name, span)
            .map(|point| DValue::Object(DObject::Point(point)))
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, reason = "valid route test controls")]
mod tests {
    use super::*;

    fn route(parameters: &[f64; 6]) -> Route<Jet> {
        let values: Vec<_> = parameters
            .iter()
            .enumerate()
            .map(|(index, value)| Jet::variable(*value, *value, index, 1.0, 6))
            .collect();
        Route {
            from: RoutePoint {
                x: values[0].clone(),
                y: values[1].clone(),
            },
            to: RoutePoint {
                x: values[2].clone(),
                y: values[3].clone(),
            },
            control: Some(RoutePoint {
                x: values[4].clone(),
                y: values[5].clone(),
            }),
            via: Vec::new(),
            parameter: None,
        }
    }

    #[test]
    fn quadratic_length_gradient_matches_endpoint_and_control_finite_differences() {
        let parameters = [-0.7, 0.1, 3.8, 1.2, 0.8, 3.4];
        let length = route(&parameters).length();
        for index in 0..6 {
            let mut before = parameters;
            let mut after = parameters;
            before[index] -= 1e-6;
            after[index] += 1e-6;
            let finite_difference =
                (route(&after).length().value - route(&before).length().value) / 2e-6;
            assert!(
                (length.gradient[index] - finite_difference).abs() < 1e-8,
                "gradient {index}: {} vs {finite_difference}",
                length.gradient[index]
            );
        }
    }

    #[test]
    fn quadratic_extreme_uses_current_piece_and_gradient() {
        let mut curve = route(&[0.0, 0.0, 4.0, 0.0, 2.0, 2.0]);
        curve
            .control
            .as_mut()
            .expect("quadratic control")
            .y
            .seed_value = -2.0;
        let (_, max) = curve.bounds();
        assert!((max.y.value - 1.0).abs() < 1e-12);
        assert!((max.y.gradient[5] - 0.5).abs() < 1e-12);
    }
    #[test]
    fn orthogonal_live_extreme_and_detour_gradients_match_finite_differences() {
        use crate::geom::route::{RouteChoice, RouteKind};
        let mut from_x = Jet::variable(4.0, -4.0, 0, 1.0, 2);
        from_x.seed_value = -4.0;
        let offset = Jet::variable(2.0, 0.5, 1, 1.0, 2);
        let route = Route::automatic(
            RoutePoint {
                x: from_x,
                y: Jet::constant(0.0, 2),
            },
            RoutePoint {
                x: Jet::constant(1.0, 2),
                y: Jet::constant(3.0, 2),
            },
            RouteKind::Orthogonal,
            RouteChoice::default(),
            offset,
        );
        let (_, max) = route.bounds();
        assert!((max.x.value - 6.0).abs() < 1e-12);
        assert_eq!(max.x.gradient, [1.0, 1.0]);
        assert_eq!(route.length().gradient, [1.0, 2.0]);
    }

    #[test]
    fn curved_parameter_gradient_matches_finite_difference() {
        use crate::geom::route::{RouteChoice, RouteKind};
        let make = |offset| {
            Route::automatic(
                RoutePoint {
                    x: Jet::constant(0.0, 1),
                    y: Jet::constant(0.0, 1),
                },
                RoutePoint {
                    x: Jet::constant(1.0, 1),
                    y: Jet::constant(3.0, 1),
                },
                RouteKind::Curved,
                RouteChoice::default(),
                Jet::variable(offset, 0.5, 0, 1.0, 1),
            )
        };
        let derivative = (make(2.0 + 1e-6).length().value - make(2.0 - 1e-6).length().value) / 2e-6;
        assert!((make(2.0).length().gradient[0] - derivative).abs() < 1e-8);
    }
    #[test]
    fn physical_trim_distances_follow_live_endpoint_length_with_exact_gradients() {
        let constant = |value| Jet::constant(value, 1);
        let mut endpoint = constant(10.0);
        endpoint.gradient[0] = 1.0;
        let route = Route {
            from: RoutePoint {
                x: constant(0.0),
                y: constant(0.0),
            },
            to: RoutePoint {
                x: endpoint,
                y: constant(0.0),
            },
            control: None,
            via: Vec::new(),
            parameter: None,
        };
        let start = route.parameter_for_distance(&constant(1.0));
        let end = route.parameter_for_distance(&route.length().sub(&constant(2.0)));
        let visible = route.retained_parameters((&start, &end));
        assert!((visible.from.x.value - 1.0).abs() < 1e-12);
        assert!(visible.from.x.gradient[0].abs() < 1e-12);
        assert!((visible.to.x.value - 8.0).abs() < 1e-12);
        assert!((visible.to.x.gradient[0] - 1.0).abs() < 1e-12);
    }

    #[test]
    fn implicit_quadratic_distance_derivative_matches_finite_difference() {
        let constant = |value| Jet::constant(value, 1);
        let mut endpoint = constant(4.0);
        endpoint.gradient[0] = 1.0;
        let route = Route {
            from: RoutePoint {
                x: constant(0.0),
                y: constant(0.0),
            },
            to: RoutePoint {
                x: endpoint,
                y: constant(0.0),
            },
            control: Some(RoutePoint {
                x: constant(2.0),
                y: constant(2.0),
            }),
            via: Vec::new(),
            parameter: None,
        };
        let parameter = route.parameter_for_distance(&constant(1.0));
        let at = |endpoint| {
            let route = Route {
                from: RoutePoint { x: 0.0, y: 0.0 },
                to: RoutePoint {
                    x: endpoint,
                    y: 0.0,
                },
                control: Some(RoutePoint { x: 2.0, y: 2.0 }),
                via: Vec::new(),
                parameter: None,
            };
            route.parameter_for_distance(&1.0)
        };
        let numerical = (at(4.0 + 1e-5) - at(4.0 - 1e-5)) / 2e-5;
        assert!((parameter.gradient[0] - numerical).abs() < 1e-7);
    }
}
