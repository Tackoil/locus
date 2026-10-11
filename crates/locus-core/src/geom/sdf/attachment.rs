//! Contact exemptions never hide an inward excursion between sample points.

use super::{cores_separation, object_cores, object_signed_distance, route_cores};
use crate::{ResolvedObject, geom::route::Route};

pub(super) fn prefix_clear(
    route: &Route,
    obstacle: &ResolvedObject,
    t: f64,
    reverse: bool,
) -> bool {
    if route.control.is_none() {
        let interval = if reverse { (1.0 - t, 1.0) } else { (0.0, t) };
        let prefix = route.retained(interval);
        return object_cores(obstacle, 0.0)
            .and_then(|cores| cores_separation(&route_cores(&prefix, &0.0), &cores, None))
            .is_some_and(|distance| distance >= -1e-12);
    }
    let distance = |parameter| {
        let parameter = if reverse { 1.0 - parameter } else { parameter };
        object_signed_distance(obstacle, route.sample(&parameter).into())
            .unwrap_or(f64::NEG_INFINITY)
    };
    let mut remaining = crate::preset::ATTACHMENT_PREFIX_CAP;
    clear_interval(
        &distance,
        route.parameter_speed(),
        (0.0, t),
        (distance(0.0), distance(t)),
        &mut remaining,
    )
}

fn clear_interval(
    distance: &impl Fn(f64) -> f64,
    speed: f64,
    interval: (f64, f64),
    values: (f64, f64),
    remaining: &mut usize,
) -> bool {
    if values.0.min(values.1) < -1e-12 {
        return false;
    }
    if values.0.min(values.1) - speed * (interval.1 - interval.0) / 2.0 >= -1e-12 {
        return true;
    }
    if *remaining == 0 {
        return false;
    }
    *remaining -= 1;
    let middle = interval.0.midpoint(interval.1);
    let value = distance(middle);
    clear_interval(
        distance,
        speed,
        (interval.0, middle),
        (values.0, value),
        remaining,
    ) && clear_interval(
        distance,
        speed,
        (middle, interval.1),
        (value, values.1),
        remaining,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Circle, Point};

    #[test]
    fn inward_quadratic_excursion_between_contact_samples_is_not_exempted() {
        let route = Route {
            from: Point::new(1.0, 0.0).into(),
            to: Point::new(4.0, 0.0).into(),
            control: Some(Point::new(0.9999, 0.0).into()),
            via: Vec::new(),
            parameter: None,
        };
        let obstacle = ResolvedObject::Circle(Circle {
            center: Point::new(0.0, 0.0),
            radius: 1.0,
        });
        assert!(!prefix_clear(&route, &obstacle, 0.25, false));
    }
}
