//! Differentiated containment and actual-interior avoidance geometry.
use super::{DEllipse, DObject, DPoint, Jet, solver_error};
use crate::eval::EvalError;
use crate::geom::route::RoutePoint;
use crate::geom::sdf::{self, Core};
use locus_syntax::Span;

/// Exact signed container boundary distance, with live pieces.
pub(crate) fn signed_distance(
    object: &DObject,
    point: &DPoint,
    span: Span,
) -> Result<Jet, EvalError> {
    let p = RoutePoint::from(point.clone());
    match object {
        DObject::Circle(c) => Ok(point
            .sub(&c.center)
            .norm()
            .map_err(|()| solver_error(span, "undefined circle distance"))?
            .sub(&c.radius)),
        DObject::Ellipse(e) => Ok(ellipse_distance(e, &p)),
        DObject::RoundedRect(r) => r.signed_distance(point, span),
        DObject::Polygon(v, _, _) => Ok(sdf::polygon_distance(&points(v), &p)),
        _ => Err(solver_error(
            span,
            "contain requires a closed container shape",
        )),
    }
}
fn ellipse_distance(e: &DEllipse, p: &RoutePoint<Jet>) -> Jet {
    sdf::ellipse_distance(
        p,
        &e.center.clone().into(),
        &e.semimajor,
        &e.semiminor,
        &e.rotation.cos(),
        &e.rotation.sin(),
    )
}
/// Signed separation of shape cores, with a deterministic optional escape-axis choice.
pub(crate) fn separation(
    a: &DObject,
    b: &DObject,
    stroke_half_width: f64,
    forced_axis: Option<usize>,
    span: Span,
) -> Result<Jet, EvalError> {
    if matches!(a, DObject::Bspline(_)) || matches!(b, DObject::Bspline(_)) {
        return sdf::cores_separation(
            &collision_cores(a, stroke_half_width, span)?,
            &collision_cores(b, stroke_half_width, span)?,
            forced_axis,
        )
        .ok_or_else(|| solver_error(span, "empty path collision geometry"));
    }
    if let DObject::Route(route) = a {
        return route_pair(route, b, stroke_half_width, forced_axis, span);
    }
    if let DObject::Route(route) = b {
        return route_pair(route, a, stroke_half_width, forced_axis, span);
    }
    if let Some(distance) = ellipse_pair(a, b, forced_axis, true) {
        return Ok(distance);
    }
    if let Some(distance) = ellipse_pair(b, a, forced_axis, false) {
        return Ok(distance);
    }
    Ok(sdf::core_separation(
        &core(a, stroke_half_width, span)?,
        &core(b, stroke_half_width, span)?,
        forced_axis,
    ))
}
fn route_pair(
    route: &crate::geom::route::Route<Jet>,
    other: &DObject,
    stroke: f64,
    forced: Option<usize>,
    span: Span,
) -> Result<Jet, EvalError> {
    if let DObject::Route(other_route) = other {
        let width = Jet::constant(stroke, route.from.x.gradient.len());
        return sdf::cores_separation(
            &sdf::route_cores(route, &width),
            &sdf::route_cores(other_route, &width),
            forced,
        )
        .ok_or_else(|| solver_error(span, "empty routed collision geometry"));
    }
    sdf::route_core_separation(
        route,
        &core(other, stroke, span)?,
        &Jet::constant(stroke, route.from.x.gradient.len()),
        forced,
    )
    .ok_or_else(|| {
        solver_error(
            span,
            "curved obstacle avoidance requires retained route spans",
        )
    })
}

fn collision_cores(object: &DObject, stroke: f64, span: Span) -> Result<Vec<Core<Jet>>, EvalError> {
    match object {
        DObject::Route(route) => Ok(sdf::route_cores(
            route,
            &Jet::constant(stroke, route.from.x.gradient.len()),
        )),
        DObject::Bspline(spline) => spline_cores(&spline.controls, stroke, span),
        _ => Ok(vec![core(object, stroke, span)?]),
    }
}

#[allow(
    clippy::cast_precision_loss,
    reason = "allocated spline span count is exactly representable"
)]
fn spline_cores(controls: &[DPoint], stroke: f64, span: Span) -> Result<Vec<Core<Jet>>, EvalError> {
    let count = controls.len() - 3;
    let width = Jet::constant(stroke, controls[0].x.gradient.len());
    let mut result = Vec::new();
    for index in 0..count {
        let samples = [0.0, 1.0 / 3.0, 2.0 / 3.0, 1.0].map(|local| {
            let t = Jet::constant((index as f64 + local) / count as f64, width.gradient.len());
            super::bspline_at(controls, &t, span).map(RoutePoint::from)
        });
        let [a, b, c, d] = samples;
        let [a, b, c, d] = [a?, b?, c?, d?];
        let first = cubic_control(&a, &b, &c, &d);
        let second = cubic_control(&d, &c, &b, &a);
        result.extend(sdf::cubic_cores(&[a, first, second, d], &width));
    }
    Ok(result)
}

fn cubic_control(
    a: &RoutePoint<Jet>,
    b: &RoutePoint<Jet>,
    c: &RoutePoint<Jet>,
    d: &RoutePoint<Jet>,
) -> RoutePoint<Jet> {
    RoutePoint {
        x: a.x
            .scale(-5.0)
            .add(&b.x.scale(18.0))
            .sub(&c.x.scale(9.0))
            .add(&d.x.scale(2.0))
            .scale(1.0 / 6.0),
        y: a.y
            .scale(-5.0)
            .add(&b.y.scale(18.0))
            .sub(&c.y.scale(9.0))
            .add(&d.y.scale(2.0))
            .scale(1.0 / 6.0),
    }
}

#[cfg(test)]
mod path_tests {
    use super::*;

    fn curve(control_height: f64) -> DObject {
        let point = |x, y| RoutePoint {
            x: Jet::constant(x, 1),
            y: Jet::constant(y, 1),
        };
        DObject::Route(crate::geom::route::Route {
            from: point(-2.0, 0.0),
            to: point(2.0, 0.0),
            via: Vec::new(),
            parameter: None,
            control: Some(RoutePoint {
                x: Jet::constant(0.0, 1),
                y: Jet::variable(control_height, control_height, 0, 1.0, 1),
            }),
        })
    }

    #[test]
    #[allow(clippy::expect_used, reason = "finite shared avoidance test controls")]
    fn curve_collision_gradient_matches_the_same_primal_span_policy() {
        let obstacle = DObject::Point(DPoint {
            x: Jet::constant(0.0, 1),
            y: Jet::constant(0.0, 1),
        });
        let value = |height| {
            separation(&curve(height), &obstacle, 0.02, None, Span::default())
                .expect("shared curve separation")
        };
        let derivative = (value(2.0 + 1e-6).value - value(2.0 - 1e-6).value) / 2e-6;
        assert!((value(2.0).gradient[0] - derivative).abs() < 1e-8);
        assert!(value(2.0).value > 0.9);
    }
}
fn ellipse_pair(
    a: &DObject,
    b: &DObject,
    forced: Option<usize>,
    ellipse_first: bool,
) -> Option<Jet> {
    let DObject::Ellipse(ellipse) = a else {
        return None;
    };
    let (point, radius) = match b {
        DObject::Point(p) => (p, p.x.scale(0.0)),
        DObject::Circle(c) => (&c.center, c.radius.clone()),
        _ => return None,
    };
    let actual = ellipse_distance(ellipse, &point.clone().into()).sub(&radius);
    let Some(choice) = forced.filter(|_| actual.value < 0.0) else {
        return Some(actual);
    };
    Some(forced_ellipse_pair(
        ellipse,
        point,
        &radius,
        choice,
        ellipse_first,
    ))
}
fn forced_ellipse_pair(
    ellipse: &DEllipse,
    point: &DPoint,
    radius: &Jet,
    choice: usize,
    ellipse_first: bool,
) -> Jet {
    let horizontal = (choice / 2) % 2 == 0;
    let delta = point.sub(&ellipse.center);
    let offset = if horizontal { delta.x } else { delta.y };
    let sign = if (choice % 2 == 0) == ellipse_first {
        1.0
    } else {
        -1.0
    };
    let support = sdf::ellipse_cardinal_support(
        &ellipse.semimajor,
        &ellipse.semiminor,
        &ellipse.rotation.cos(),
        &ellipse.rotation.sin(),
        horizontal,
    );
    offset.scale(sign).sub(&support).sub(radius)
}

fn core(object: &DObject, stroke: f64, span: Span) -> Result<Core<Jet>, EvalError> {
    let (vertices, radius) = match object {
        DObject::Point(p) => point_core(p),
        DObject::Circle(c) => {
            let (vertices, _) = point_core(&c.center);
            (vertices, c.radius.clone())
        }
        DObject::Linear(l) => (
            vec![l.start.clone().into(), l.through.clone().into()],
            Jet::constant(stroke, l.start.x.gradient.len()),
        ),
        _ => closed_core(object, span)?,
    };
    Ok(Core::new(vertices, radius))
}
fn closed_core(object: &DObject, span: Span) -> Result<(Vec<RoutePoint<Jet>>, Jet), EvalError> {
    match object {
        DObject::Polygon(v, _, _) => Ok((points(v), v[0].x.scale(0.0))),
        DObject::RoundedRect(r) => Ok(rounded_core(r)),
        DObject::Ellipse(e) => Ok((ellipse_box(e, span)?, e.semimajor.scale(0.0))),
        DObject::Route(_) => Err(solver_error(
            span,
            "routed path avoidance requires discrete routing",
        )),
        _ => Err(solver_error(
            span,
            "avoid requires finite visible geometry or measured content",
        )),
    }
}
fn point_core(p: &DPoint) -> (Vec<RoutePoint<Jet>>, Jet) {
    (vec![p.clone().into()], p.x.scale(0.0))
}
fn rounded_core(r: &super::rounded_rect::DRoundedRect) -> (Vec<RoutePoint<Jet>>, Jet) {
    (
        sdf::rectangle(
            &r.center.clone().into(),
            &r.width.sub(&r.radius.scale(2.0)),
            &r.height.sub(&r.radius.scale(2.0)),
        ),
        r.radius.clone(),
    )
}
fn points(vertices: &[DPoint]) -> Vec<RoutePoint<Jet>> {
    vertices.iter().cloned().map(Into::into).collect()
}
fn ellipse_box(e: &DEllipse, span: Span) -> Result<Vec<RoutePoint<Jet>>, EvalError> {
    let cosine = e.rotation.cos();
    let sine = e.rotation.sin();
    let ac = e.semimajor.mul(&cosine);
    let bs = e.semiminor.mul(&sine);
    let as_ = e.semimajor.mul(&sine);
    let bc = e.semiminor.mul(&cosine);
    let width = ac
        .mul(&ac)
        .add(&bs.mul(&bs))
        .sqrt()
        .map_err(|()| solver_error(span, "non-finite ellipse extent"))?
        .scale(2.0);
    let height = as_
        .mul(&as_)
        .add(&bc.mul(&bc))
        .sqrt()
        .map_err(|()| solver_error(span, "non-finite ellipse extent"))?
        .scale(2.0);
    Ok(sdf::rectangle(&e.center.clone().into(), &width, &height))
}

#[cfg(test)]
#[allow(
    clippy::float_cmp,
    reason = "exact boundary values and selected piece gradients are part of the contract"
)]
mod tests {
    use super::*;
    fn jet(v: f64, index: usize) -> Jet {
        Jet::variable(v, v, index, 1.0, 5)
    }
    fn ellipse() -> DEllipse {
        DEllipse {
            center: DPoint {
                x: Jet::constant(0.0, 5),
                y: Jet::constant(0.0, 5),
            },
            semimajor: jet(4.0, 2),
            semiminor: jet(1.0, 3),
            rotation: jet(0.0, 4),
        }
    }
    fn distance(e: &DEllipse, x: f64, y: f64) -> Jet {
        ellipse_distance(
            e,
            &RoutePoint {
                x: jet(x, 0),
                y: jet(y, 1),
            },
        )
    }
    #[test]
    fn ellipse_boundary_gradients_match_both_sides() {
        let e = ellipse();
        for phi in [0.0, core::f64::consts::FRAC_PI_2, 0.7] {
            let (x, y) = (4.0 * libm::cos(phi), libm::sin(phi));
            let n = sdf::ellipse_normal(crate::geom::Point::new(x, y), 4.0, 1.0);
            let boundary = distance(&e, x, y);
            assert!(boundary.value.abs() < 1e-12);
            assert!((boundary.gradient[0] - n.x).abs() < 1e-10);
            assert!((boundary.gradient[1] - n.y).abs() < 1e-10);
            for offset in [-1e-7, 1e-7] {
                let near = distance(&e, x + offset * n.x, y + offset * n.y);
                assert!((near.value - offset).abs() < 1e-11);
                assert!((near.gradient[0] - n.x).abs() < 1e-6);
                assert!((near.gradient[1] - n.y).abs() < 1e-6);
            }
        }
    }
    #[test]
    fn ellipse_medial_axis_and_axis_parameters_are_live() {
        let e = ellipse();
        let center = distance(&e, 0.0, 0.0);
        assert_eq!(center.value, -1.0);
        assert_eq!(center.gradient[1], 1.0);
        assert_eq!(center.gradient[3], -1.0);
        let above = distance(&e, 1.0, 1e-8);
        let below = distance(&e, 1.0, -1e-8);
        assert!(above.gradient[1] > 0.0 && below.gradient[1] < 0.0);
    }
    #[test]
    fn coincident_circles_offer_opposite_finite_gradients() {
        let a = Core::new(
            vec![RoutePoint {
                x: jet(0.0, 0),
                y: jet(0.0, 1),
            }],
            Jet::constant(1.0, 5),
        );
        let b = Core::new(
            vec![RoutePoint {
                x: Jet::constant(0.0, 5),
                y: Jet::constant(0.0, 5),
            }],
            Jet::constant(1.0, 5),
        );
        let left = sdf::core_separation(&a, &b, Some(0));
        let right = sdf::core_separation(&a, &b, Some(1));
        assert_eq!(left.value, -2.0);
        assert_eq!(right.value, -2.0);
        assert_eq!(left.gradient[0], -1.0);
        assert_eq!(right.gradient[0], 1.0);
        assert_eq!(sdf::core_separation(&a, &b, None).gradient[0], 1.0);
    }
    #[test]
    fn polygon_boundary_uses_outward_normal() {
        let center = RoutePoint {
            x: Jet::constant(0.0, 5),
            y: Jet::constant(0.0, 5),
        };
        let v = sdf::rectangle(&center, &Jet::constant(2.0, 5), &Jet::constant(2.0, 5));
        let point = RoutePoint {
            x: jet(1.0, 0),
            y: jet(0.0, 1),
        };
        let distance = sdf::polygon_distance(&v, &point);
        assert_eq!(distance.value, 0.0);
        assert_eq!(distance.gradient[0], 1.0);
        assert_eq!(distance.gradient[1], 0.0);
    }
    fn perturb_axis(e: &mut DEllipse, index: usize, offset: f64) {
        let axis = match index {
            2 => &mut e.semimajor,
            3 => &mut e.semiminor,
            _ => &mut e.rotation,
        };
        axis.value += offset;
    }
    #[test]
    fn ellipse_overlap_offers_opposite_escape_gradients() -> Result<(), &'static str> {
        let object = DObject::Ellipse(ellipse());
        let point = DObject::Point(DPoint {
            x: jet(0.0, 0),
            y: jet(0.0, 1),
        });
        let east = ellipse_pair(&object, &point, Some(0), true).ok_or("ellipse pair required")?;
        let west = ellipse_pair(&object, &point, Some(1), true).ok_or("ellipse pair required")?;
        assert_eq!(east.value, -4.0);
        assert_eq!(west.value, -4.0);
        assert_eq!(east.gradient[0], 1.0);
        assert_eq!(west.gradient[0], -1.0);
        let circle = DObject::Circle(super::super::DCircle {
            center: DPoint {
                x: jet(0.0, 0),
                y: jet(0.0, 1),
            },
            radius: Jet::constant(0.5, 5),
        });
        let north = ellipse_pair(&object, &circle, Some(2), true).ok_or("ellipse pair required")?;
        assert_eq!(north.value, -1.5);
        assert_eq!(north.gradient[1], 1.0);
        assert_eq!(north.gradient[3], -1.0);
        Ok(())
    }
    #[test]
    fn ellipse_shape_parameter_gradients_match_finite_differences() {
        let e = ellipse();
        let base = distance(&e, 3.0, 1.0);
        for index in 2..5 {
            let mut plus = e.clone();
            let mut minus = e.clone();
            perturb_axis(&mut plus, index, 1e-6);
            perturb_axis(&mut minus, index, -1e-6);
            let finite =
                (distance(&plus, 3.0, 1.0).value - distance(&minus, 3.0, 1.0).value) / 2e-6;
            assert!((base.gradient[index] - finite).abs() < 1e-7);
        }
    }
    #[test]
    fn crossing_concave_obstacle_has_live_escape_gradient() {
        let subject = Core::new(
            vec![
                RoutePoint {
                    x: jet(-1.5, 0),
                    y: jet(0.0, 1),
                },
                RoutePoint {
                    x: jet(0.5, 0),
                    y: jet(0.0, 1),
                },
            ],
            Jet::constant(0.0, 5),
        );
        let obstacle = Core::new(
            [
                (-2.0, -2.0),
                (2.0, -2.0),
                (2.0, -1.0),
                (-1.0, -1.0),
                (-1.0, 2.0),
                (-2.0, 2.0),
            ]
            .into_iter()
            .map(|(x, y)| RoutePoint {
                x: Jet::constant(x, 5),
                y: Jet::constant(y, 5),
            })
            .collect(),
            Jet::constant(0.0, 5),
        );
        let actual = sdf::core_separation(&subject, &obstacle, None);
        assert_eq!(actual.value, -0.5);
        assert_eq!(actual.gradient[0], 1.0);
        let lower = sdf::core_separation(&subject, &obstacle, Some(0));
        let upper = sdf::core_separation(&subject, &obstacle, Some(1));
        assert_eq!(lower.gradient[1], -1.0);
        assert_eq!(upper.gradient[1], 1.0);
    }
    #[test]
    fn circle_separation_gradient_follows_actual_center_direction() {
        let a = Core::new(
            vec![RoutePoint {
                x: jet(0.0, 0),
                y: jet(0.0, 1),
            }],
            Jet::constant(1.0, 5),
        );
        let b = Core::new(
            vec![RoutePoint {
                x: Jet::constant(1.5, 5),
                y: Jet::constant(1.5, 5),
            }],
            Jet::constant(1.0, 5),
        );
        let sep = sdf::core_separation(&a, &b, None);
        assert!(sep.value > 0.1);
        assert!((sep.gradient[0] + core::f64::consts::FRAC_1_SQRT_2).abs() < 1e-12);
    }
}
