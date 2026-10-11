//! Shared logical centerline geometry for solving, measurement, and rendering.

use super::{BBox, ConstructError, Point, arrow};

/// Arithmetic required by the shared route, implemented by numbers and solver jets.
pub trait RouteScalar: Clone {
    /// Current scalar value, used for live piece selection.
    fn value(&self) -> f64;
    /// A constant with the same differentiation context.
    #[must_use]
    fn constant(&self, value: f64) -> Self;
    /// Addition.
    #[must_use]
    fn add(&self, rhs: &Self) -> Self;
    /// Subtraction.
    #[must_use]
    fn sub(&self, rhs: &Self) -> Self;
    /// Multiplication.
    #[must_use]
    fn mul(&self, rhs: &Self) -> Self;
    /// Division.
    #[must_use]
    fn div(&self, rhs: &Self) -> Self;
    /// Constant scaling.
    #[must_use]
    fn scale(&self, factor: f64) -> Self;
    /// Nonnegative square root.
    #[must_use]
    fn sqrt(&self) -> Self;
    /// Inverse hyperbolic sine.
    #[must_use]
    fn asinh(&self) -> Self;
}

impl RouteScalar for f64 {
    fn value(&self) -> f64 {
        *self
    }
    fn constant(&self, value: f64) -> Self {
        value
    }
    fn add(&self, rhs: &Self) -> Self {
        self + rhs
    }
    fn sub(&self, rhs: &Self) -> Self {
        self - rhs
    }
    fn mul(&self, rhs: &Self) -> Self {
        self * rhs
    }
    fn div(&self, rhs: &Self) -> Self {
        self / rhs
    }
    fn scale(&self, factor: f64) -> Self {
        self * factor
    }
    fn sqrt(&self) -> Self {
        libm::sqrt(*self)
    }
    fn asinh(&self) -> Self {
        libm::asinh(*self)
    }
}

/// Route coordinate, independent of the numeric representation.
#[derive(Clone, Debug, PartialEq)]
pub struct RoutePoint<S> {
    /// Horizontal coordinate.
    pub x: S,
    /// Vertical coordinate.
    pub y: S,
}

/// Continuous routing construction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RouteKind {
    /// Axis-aligned detour.
    Orthogonal,
    /// Quadratic control offset.
    Curved,
}

/// Frozen page-side routing choice.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RouteSide {
    /// Detour toward positive x.
    Right,
    /// Detour toward negative x.
    Left,
    /// Detour toward positive y.
    Above,
    /// Detour toward negative y.
    Below,
}

/// Frozen side and orthogonal topology.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RouteChoice {
    /// Page side of the route.
    pub side: RouteSide,
    /// Whether to use the five-span detour.
    pub five_span: bool,
}

impl Default for RouteChoice {
    fn default() -> Self {
        Self {
            side: RouteSide::Right,
            five_span: false,
        }
    }
}

/// Accepted logical segment, quadratic Bézier, or orthogonal centerline.
#[derive(Clone, Debug, PartialEq)]
pub struct Route<S = f64> {
    /// Tail endpoint.
    pub from: RoutePoint<S>,
    /// Tip endpoint.
    pub to: RoutePoint<S>,
    /// Quadratic control point; absent for a straight segment.
    pub control: Option<RoutePoint<S>>,
    /// Intermediate vertices for an orthogonal route.
    pub via: Vec<RoutePoint<S>>,
    /// Solved detour or control-offset parameter for an automatic route.
    pub parameter: Option<S>,
}

impl Route<f64> {
    /// Constructs a direct route with the authored bend angle.
    pub fn direct(from: Point, to: Point, bend: f64) -> Result<Self, ConstructError> {
        let control = arrow::control(from, to, bend)?.map(RoutePoint::from);
        Ok(Self {
            from: from.into(),
            to: to.into(),
            control,
            via: Vec::new(),
            parameter: None,
        })
    }

    /// Upper bound on centerline speed in the route's sampling parameter.
    #[must_use]
    pub fn parameter_speed(&self) -> f64 {
        self.control.as_ref().map_or_else(
            || self.length(),
            |control| {
                let start: Point = self.from.clone().into();
                let end: Point = self.to.clone().into();
                let control: Point = control.clone().into();
                2.0 * (control - start).norm().max((end - control).norm())
            },
        )
    }

    /// Exact logical bounds, excluding render-only clipping and arrowheads.
    #[must_use]
    pub fn bbox(&self) -> BBox {
        let (min, max) = self.bounds();
        BBox {
            min: min.into(),
            max: max.into(),
        }
    }
}

fn retained_vertices<S: RouteScalar>(route: &Route<S>, range: (f64, f64)) -> Vec<RoutePoint<S>> {
    if route.via.is_empty() {
        return Vec::new();
    }
    let points = vertices(route);
    let length = route.length();
    let mut distance = route.from.x.constant(0.0);
    let mut result = Vec::new();
    for pair in points.windows(2) {
        distance = distance.add(&norm(
            &pair[1].x.sub(&pair[0].x),
            &pair[1].y.sub(&pair[0].y),
        ));
        let t = distance.div(&length).value();
        if t > range.0 && t < range.1 {
            result.push(pair[1].clone());
        }
    }
    result
}

impl From<Point> for RoutePoint<f64> {
    fn from(point: Point) -> Self {
        Self {
            x: point.x,
            y: point.y,
        }
    }
}

impl From<RoutePoint<f64>> for Point {
    fn from(point: RoutePoint<f64>) -> Self {
        Self::new(point.x, point.y)
    }
}

impl<S: RouteScalar> Route<S> {
    /// Restricts the visible centerline while retaining the solved routing parameter.
    #[must_use]
    pub fn retained(&self, range: (f64, f64)) -> Self {
        self.retained_parameters((
            &self.from.x.constant(range.0),
            &self.from.x.constant(range.1),
        ))
    }

    /// Restricts the visible centerline using live, differentiated native parameters.
    #[must_use]
    pub fn retained_parameters(&self, range: (&S, &S)) -> Self {
        let from = self.sample(range.0);
        let to = self.sample(range.1);
        let control = self
            .control
            .as_ref()
            .map(|control| retained_control(self, control, &from, (range.0, range.1)));
        Self {
            from,
            to,
            control,
            via: retained_vertices(self, (range.0.value(), range.1.value())),
            parameter: self.parameter.clone(),
        }
    }

    /// Logical arc length from the start to a native sampling parameter.
    #[must_use]
    pub fn distance_at_parameter(&self, parameter: &S) -> S {
        if parameter.value() <= 0.0 {
            return parameter.constant(0.0);
        }
        if parameter.value() >= 1.0 {
            return self.length();
        }
        let Some(control) = &self.control else {
            return self.length().mul(parameter);
        };
        let prefix_control = RoutePoint {
            x: self.from.x.add(&control.x.sub(&self.from.x).mul(parameter)),
            y: self.from.y.add(&control.y.sub(&self.from.y).mul(parameter)),
        };
        Self {
            from: self.from.clone(),
            to: self.sample(parameter),
            control: Some(prefix_control),
            via: Vec::new(),
            parameter: None,
        }
        .length()
    }

    /// Native sampling parameter at a logical arc distance, with implicit derivatives.
    #[must_use]
    pub fn parameter_for_distance(&self, distance: &S) -> S {
        let length = self.length();
        if distance.value() <= 0.0 {
            return distance.constant(0.0);
        }
        if distance.value() >= length.value() {
            return distance.constant(1.0);
        }
        let Some(control) = &self.control else {
            return distance.div(&length);
        };
        let parameter = distance.constant(distance_parameter(self, distance.value()));
        let speed = quadratic_speed(self, control, &parameter);
        if speed.value() == 0.0 {
            return parameter;
        }
        // The root is primal-only; differentiating this correction applies the
        // implicit-function derivative of prefix_length(u) = distance.
        parameter.add(
            &distance
                .sub(&self.distance_at_parameter(&parameter))
                .div(&speed),
        )
    }

    /// Constructs an automatic route from current endpoints and offset.
    #[must_use]
    pub fn automatic(
        from: RoutePoint<S>,
        to: RoutePoint<S>,
        kind: RouteKind,
        choice: RouteChoice,
        offset: S,
    ) -> Self {
        let (control, via) = match kind {
            RouteKind::Orthogonal => (None, orthogonal_vertices(&from, &to, choice, &offset)),
            RouteKind::Curved => (
                Some(curved_control(&from, &to, choice.side, &offset)),
                Vec::new(),
            ),
        };
        Self {
            from,
            to,
            control,
            via,
            parameter: Some(offset),
        }
    }

    /// Samples the logical centerline at a parameter in `[0, 1]`.
    #[must_use]
    pub fn sample(&self, t: &S) -> RoutePoint<S> {
        if !self.via.is_empty() {
            return polyline_sample(self, t);
        }
        RoutePoint {
            x: sample_axis(
                &self.from.x,
                self.control.as_ref().map(|p| &p.x),
                &self.to.x,
                t,
            ),
            y: sample_axis(
                &self.from.y,
                self.control.as_ref().map(|p| &p.y),
                &self.to.y,
                t,
            ),
        }
    }

    /// Exact axis extremes with piece selection based on current coordinates.
    #[must_use]
    pub fn bounds(&self) -> (RoutePoint<S>, RoutePoint<S>) {
        if !self.via.is_empty() {
            return polyline_bounds(self);
        }
        let (left, right) = axis_bounds(
            &self.from.x,
            self.control.as_ref().map(|p| &p.x),
            &self.to.x,
        );
        let (bottom, top) = axis_bounds(
            &self.from.y,
            self.control.as_ref().map(|p| &p.y),
            &self.to.y,
        );
        (
            RoutePoint { x: left, y: bottom },
            RoutePoint { x: right, y: top },
        )
    }

    /// Closed-form quadratic arc length, or endpoint distance for a segment.
    #[must_use]
    pub fn length(&self) -> S {
        if !self.via.is_empty() {
            return polyline_length(self);
        }
        let dx = self.to.x.sub(&self.from.x);
        let dy = self.to.y.sub(&self.from.y);
        let Some(control) = &self.control else {
            return norm(&dx, &dy);
        };
        let ax = dx.sub(&control.x.sub(&self.from.x).scale(2.0)).scale(2.0);
        let ay = dy.sub(&control.y.sub(&self.from.y).scale(2.0)).scale(2.0);
        let bx = control.x.sub(&self.from.x).scale(2.0);
        let by = control.y.sub(&self.from.y).scale(2.0);
        quadratic_length(&ax, &ay, &bx, &by)
    }
}

fn distance_parameter<S: RouteScalar>(route: &Route<S>, distance: f64) -> f64 {
    let (mut lower, mut upper) = (0.0_f64, 1.0_f64);
    for _ in 0..crate::preset::CLIPPING_BISECTIONS {
        let middle = lower.midpoint(upper);
        if route
            .distance_at_parameter(&route.from.x.constant(middle))
            .value()
            < distance
        {
            lower = middle;
        } else {
            upper = middle;
        }
    }
    lower.midpoint(upper)
}

fn quadratic_speed<S: RouteScalar>(route: &Route<S>, control: &RoutePoint<S>, parameter: &S) -> S {
    let axis = |from: &S, control: &S, to: &S| {
        control
            .sub(from)
            .mul(&parameter.constant(1.0).sub(parameter))
            .add(&to.sub(control).mul(parameter))
            .scale(2.0)
    };
    norm(
        &axis(&route.from.x, &control.x, &route.to.x),
        &axis(&route.from.y, &control.y, &route.to.y),
    )
}

fn retained_control<S: RouteScalar>(
    route: &Route<S>,
    control: &RoutePoint<S>,
    from: &RoutePoint<S>,
    range: (&S, &S),
) -> RoutePoint<S> {
    let axis = |start: &S, control: &S, end: &S, origin: &S| {
        origin.add(
            &control
                .sub(start)
                .mul(&range.0.constant(1.0).sub(range.0))
                .add(&end.sub(control).mul(range.0))
                .mul(&range.1.sub(range.0)),
        )
    };
    RoutePoint {
        x: axis(&route.from.x, &control.x, &route.to.x, &from.x),
        y: axis(&route.from.y, &control.y, &route.to.y, &from.y),
    }
}

fn select<S: RouteScalar>(a: &S, b: &S, maximum: bool) -> S {
    let retain_first = if maximum {
        a.value() >= b.value()
    } else {
        a.value() <= b.value()
    };
    if retain_first { a.clone() } else { b.clone() }
}

fn orthogonal_vertices<S: RouteScalar>(
    from: &RoutePoint<S>,
    to: &RoutePoint<S>,
    choice: RouteChoice,
    offset: &S,
) -> Vec<RoutePoint<S>> {
    let horizontal = matches!(choice.side, RouteSide::Right | RouteSide::Left);
    let positive = matches!(choice.side, RouteSide::Right | RouteSide::Above);
    let (a, b, cross_a, cross_b) = if horizontal {
        (&from.x, &to.x, &from.y, &to.y)
    } else {
        (&from.y, &to.y, &from.x, &to.x)
    };
    let detour = select(a, b, positive).add(&offset.scale(if positive { 1.0 } else { -1.0 }));
    let coordinates = if choice.five_span {
        let outside = select(cross_a, cross_b, true).add(offset);
        vec![
            (
                a.add(&offset.scale(if positive { 1.0 } else { -1.0 })),
                cross_a.clone(),
            ),
            (
                a.add(&offset.scale(if positive { 1.0 } else { -1.0 })),
                outside.clone(),
            ),
            (detour.clone(), outside),
            (detour, cross_b.clone()),
        ]
    } else {
        vec![(detour.clone(), cross_a.clone()), (detour, cross_b.clone())]
    };
    coordinates
        .into_iter()
        .map(|(along, cross)| {
            if horizontal {
                RoutePoint { x: along, y: cross }
            } else {
                RoutePoint { x: cross, y: along }
            }
        })
        .collect()
}

fn curved_control<S: RouteScalar>(
    from: &RoutePoint<S>,
    to: &RoutePoint<S>,
    side: RouteSide,
    offset: &S,
) -> RoutePoint<S> {
    let dx = to.x.sub(&from.x);
    let dy = to.y.sub(&from.y);
    let length = norm(&dx, &dy);
    let (nx, ny) = curve_normal(&dx, &dy, &length, side);
    let component = match side {
        RouteSide::Right => nx.value(),
        RouteSide::Left => -nx.value(),
        RouteSide::Above => ny.value(),
        RouteSide::Below => -ny.value(),
    };
    let signed = offset.scale(if component < 0.0 { -1.0 } else { 1.0 });
    RoutePoint {
        x: from.x.add(&to.x).scale(0.5).add(&nx.mul(&signed)),
        y: from.y.add(&to.y).scale(0.5).add(&ny.mul(&signed)),
    }
}

fn curve_normal<S: RouteScalar>(dx: &S, dy: &S, length: &S, side: RouteSide) -> (S, S) {
    if length.value() > 0.0 {
        return (dy.scale(-1.0).div(length), dx.div(length));
    }
    match side {
        RouteSide::Right | RouteSide::Left => (dx.constant(1.0), dy.constant(0.0)),
        RouteSide::Above | RouteSide::Below => (dx.constant(0.0), dy.constant(1.0)),
    }
}

fn vertices<S>(route: &Route<S>) -> Vec<&RoutePoint<S>> {
    core::iter::once(&route.from)
        .chain(route.via.iter())
        .chain(core::iter::once(&route.to))
        .collect()
}

fn polyline_length<S: RouteScalar>(route: &Route<S>) -> S {
    vertices(route)
        .windows(2)
        .fold(route.from.x.constant(0.0), |total, pair| {
            total.add(&norm(
                &pair[1].x.sub(&pair[0].x),
                &pair[1].y.sub(&pair[0].y),
            ))
        })
}

fn polyline_bounds<S: RouteScalar>(route: &Route<S>) -> (RoutePoint<S>, RoutePoint<S>) {
    vertices(route).into_iter().fold(
        (route.from.clone(), route.from.clone()),
        |(min, max), point| {
            (
                RoutePoint {
                    x: select(&min.x, &point.x, false),
                    y: select(&min.y, &point.y, false),
                },
                RoutePoint {
                    x: select(&max.x, &point.x, true),
                    y: select(&max.y, &point.y, true),
                },
            )
        },
    )
}

fn polyline_sample<S: RouteScalar>(route: &Route<S>, t: &S) -> RoutePoint<S> {
    let points = vertices(route);
    let mut remaining = polyline_length(route).mul(t);
    for pair in points.windows(2) {
        let length = norm(&pair[1].x.sub(&pair[0].x), &pair[1].y.sub(&pair[0].y));
        if length.value() > 0.0 && remaining.value() <= length.value() {
            let local = remaining.div(&length);
            return RoutePoint {
                x: sample_axis(&pair[0].x, None, &pair[1].x, &local),
                y: sample_axis(&pair[0].y, None, &pair[1].y, &local),
            };
        }
        remaining = remaining.sub(&length);
    }
    route.to.clone()
}

fn norm<S: RouteScalar>(x: &S, y: &S) -> S {
    x.mul(x).add(&y.mul(y)).sqrt()
}

fn sample_axis<S: RouteScalar>(from: &S, control: Option<&S>, to: &S, t: &S) -> S {
    let u = t.constant(1.0).sub(t);
    match control {
        None => from.mul(&u).add(&to.mul(t)),
        Some(control) => from
            .mul(&u.mul(&u))
            .add(&control.mul(&u.mul(t)).scale(2.0))
            .add(&to.mul(&t.mul(t))),
    }
}

fn axis_bounds<S: RouteScalar>(from: &S, control: Option<&S>, to: &S) -> (S, S) {
    let (mut min, mut max) = if from.value() <= to.value() {
        (from.clone(), to.clone())
    } else {
        (to.clone(), from.clone())
    };
    let Some(control) = control else {
        return (min, max);
    };
    let denominator = from.sub(&control.scale(2.0)).add(to);
    if denominator.value() == 0.0 {
        return (min, max);
    }
    let t = from.sub(control).div(&denominator);
    if t.value() > 0.0 && t.value() < 1.0 {
        let extreme = sample_axis(from, Some(control), to, &t);
        if extreme.value() < min.value() {
            min = extreme.clone();
        }
        if extreme.value() > max.value() {
            max = extreme;
        }
    }
    (min, max)
}

fn quadratic_length<S: RouteScalar>(ax: &S, ay: &S, bx: &S, by: &S) -> S {
    let a = ax.mul(ax).add(&ay.mul(ay));
    let b = ax.mul(bx).add(&ay.mul(by));
    let c = bx.mul(bx).add(&by.mul(by));
    if a.value() <= c.value() * 1e-24 {
        return c.sqrt();
    }
    let h = b.div(&a);
    // The squared perpendicular component avoids cancellation for nearly collinear controls.
    let cross = ax.mul(by).sub(&ay.mul(bx));
    let k2 = cross.mul(&cross).div(&a.mul(&a));
    let start = quadratic_primitive(&h, &k2);
    let end = quadratic_primitive(&h.add(&h.constant(1.0)), &k2);
    a.sqrt().mul(&end.sub(&start))
}

fn quadratic_primitive<S: RouteScalar>(u: &S, k2: &S) -> S {
    if k2.value() == 0.0 {
        let absolute = u.scale(if u.value() < 0.0 { -1.0 } else { 1.0 });
        return u.mul(&absolute).scale(0.5);
    }
    let radius = u.mul(u).add(k2).sqrt();
    u.mul(&radius)
        .add(&k2.mul(&u.div(&k2.sqrt()).asinh()))
        .scale(0.5)
}

#[cfg(test)]
#[allow(clippy::expect_used, reason = "valid route test constructors")]
mod tests {
    use super::*;

    #[test]
    fn direct_and_bent_length_and_exact_bounds() {
        let straight = Route::direct(Point::new(0.0, 0.0), Point::new(4.0, 0.0), 0.0)
            .expect("valid direct route");
        assert!((straight.length() - 4.0).abs() < 1e-12);
        let bent = Route::direct(
            Point::new(0.0, 0.0),
            Point::new(4.0, 0.0),
            core::f64::consts::FRAC_PI_4,
        )
        .expect("valid direct route");
        assert!((bent.length() - 4.591_174_298_785_276).abs() < 1e-12);
        assert!((bent.bbox().max.y - 1.0).abs() < 1e-12);
        assert!((bent.sample(&0.5).y - 1.0).abs() < 1e-12);
    }

    #[test]
    fn collinear_and_degenerate_controls_have_finite_lengths() {
        let route = Route {
            from: Point::new(0.0, 0.0).into(),
            to: Point::new(1.0, 0.0).into(),
            control: Some(Point::new(2.0, 0.0).into()),
            via: Vec::new(),
            parameter: None,
        };
        assert!((route.length() - 5.0 / 3.0).abs() < 1e-12);
        let zero = Route::direct(Point::new(1.0, 2.0), Point::new(1.0, 2.0), 0.7)
            .expect("valid direct route");
        assert!(zero.length().abs() < 1e-12);
    }
    #[test]
    fn orthogonal_lengths_use_differing_endpoint_coordinates_and_all_spans() {
        let from = RoutePoint { x: 1.0, y: -1.0 };
        let to = RoutePoint { x: 4.0, y: 3.0 };
        for side in [
            RouteSide::Right,
            RouteSide::Left,
            RouteSide::Above,
            RouteSide::Below,
        ] {
            let route = Route::automatic(
                from.clone(),
                to.clone(),
                RouteKind::Orthogonal,
                RouteChoice {
                    side,
                    five_span: false,
                },
                2.0,
            );
            assert!((route.length() - 11.0).abs() < 1e-12);
            assert_eq!(route.sample(&0.0), from);
            assert_eq!(route.sample(&1.0), to);
            assert_eq!(route.via.len(), 2);
            let five = Route::automatic(
                from.clone(),
                to.clone(),
                RouteKind::Orthogonal,
                RouteChoice {
                    side,
                    five_span: true,
                },
                2.0,
            );
            assert_eq!(five.via.len(), 4);
            assert!(five.length() >= route.length());
        }
    }

    #[test]
    fn curved_offset_has_exact_extrema_and_length_independent_of_side() {
        let right = Route::automatic(
            RoutePoint { x: 0.0, y: 0.0 },
            RoutePoint { x: 0.0, y: 4.0 },
            RouteKind::Curved,
            RouteChoice::default(),
            2.0,
        );
        let left = Route::automatic(
            right.from.clone(),
            right.to.clone(),
            RouteKind::Curved,
            RouteChoice {
                side: RouteSide::Left,
                five_span: false,
            },
            2.0,
        );
        assert!((right.bbox().max.x - 1.0).abs() < 1e-12);
        assert!((left.bbox().min.x + 1.0).abs() < 1e-12);
        assert!((left.length() - right.length()).abs() < 1e-12);
        assert!((right.length() - 4.591_174_298_785_276).abs() < 1e-12);
    }
    #[test]
    fn arc_distance_inversion_round_trips_straight_orthogonal_and_quadratic_routes() {
        let straight =
            Route::direct(Point::new(0.0, 0.0), Point::new(4.0, 0.0), 0.0).expect("straight");
        let curved = Route::direct(
            Point::new(0.0, 0.0),
            Point::new(4.0, 0.0),
            core::f64::consts::FRAC_PI_4,
        )
        .expect("quadratic");
        let orthogonal = Route::automatic(
            straight.from.clone(),
            RoutePoint { x: 1.0, y: 3.0 },
            RouteKind::Orthogonal,
            RouteChoice::default(),
            0.5,
        );
        for route in [straight, curved, orthogonal] {
            for factor in [0.1, 0.4, 0.9] {
                let distance = route.length() * factor;
                let parameter = route.parameter_for_distance(&distance);
                assert!((route.distance_at_parameter(&parameter) - distance).abs() < 1e-12);
            }
            let start = route.parameter_for_distance(&0.15);
            let end = route.parameter_for_distance(&(route.length() - 0.25));
            assert!(
                (route.retained_parameters((&start, &end)).length() - route.length() + 0.4).abs()
                    < 1e-12
            );
        }
    }
}
