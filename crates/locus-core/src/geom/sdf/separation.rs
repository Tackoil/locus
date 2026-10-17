//! Convex SAT classification and exact disjoint feature distances.
use super::{dot, edges, minimum, norm, polygon_distance, primal, segment_distance, sub};
use crate::geom::route::{RoutePoint, RouteScalar};

/// Minkowski core: point, segment, or polygon, expanded by a circular radius.
#[derive(Clone, Debug)]
pub struct Core<S> {
    /// Ordered vertices; one for a point, two for a finite segment.
    pub vertices: Vec<RoutePoint<S>>,
    /// Circle radius, rounded-corner radius, or stroke half width.
    pub radius: S,
}
impl<S: RouteScalar> Core<S> {
    /// Builds a core, reducing coincident vertices of rounded rectangle cores.
    #[must_use]
    #[allow(
        clippy::float_cmp,
        reason = "only exactly coincident core vertices may be collapsed"
    )]
    pub fn new(vertices: Vec<RoutePoint<S>>, radius: S) -> Self {
        let mut unique: Vec<RoutePoint<S>> = Vec::new();
        for p in vertices {
            if !unique
                .iter()
                .any(|q| p.x.value() == q.x.value() && p.y.value() == q.y.value())
            {
                unique.push(p);
            }
        }
        Self {
            vertices: unique,
            radius,
        }
    }
}
/// Signed shape separation; optional `2 * axis + side` pins an overlapping escape direction.
/// Side zero separates B above A; side one separates A above B on the selected axis.
#[must_use]
pub fn core_separation<S: RouteScalar>(a: &Core<S>, b: &Core<S>, forced_axis: Option<usize>) -> S {
    let actual = core_distance(&a.vertices, &b.vertices)
        .sub(&a.radius)
        .sub(&b.radius);
    if let Some(choice) = forced_axis.filter(|_| actual.value() < 0.0) {
        if !convex(&a.vertices) || !convex(&b.vertices) {
            if let Some(crossing) = crossing_distance(&a.vertices, &b.vertices, Some(choice)) {
                return crossing.sub(&a.radius).sub(&b.radius);
            }
        }
        return forced_distance(a, b, choice);
    }
    actual
}

fn forced_distance<S: RouteScalar>(a: &Core<S>, b: &Core<S>, choice: usize) -> S {
    let mut normals = axes(&a.vertices)
        .into_iter()
        .chain(axes(&b.vertices))
        .collect::<Vec<_>>();
    if normals.is_empty() {
        let scalar = &a.vertices[0].x;
        normals = vec![
            RoutePoint {
                x: scalar.constant(1.0),
                y: scalar.constant(0.0),
            },
            RoutePoint {
                x: scalar.constant(0.0),
                y: scalar.constant(1.0),
            },
        ];
    }
    let u = &normals[(choice / 2) % normals.len()];
    let (alo, ahi) = projection(&a.vertices, u);
    let (blo, bhi) = projection(&b.vertices, u);
    let gap = if choice % 2 == 0 {
        blo.sub(&ahi)
    } else {
        alo.sub(&bhi)
    };
    gap.sub(&a.radius).sub(&b.radius)
}
fn core_distance<S: RouteScalar>(a: &[RoutePoint<S>], b: &[RoutePoint<S>]) -> S {
    match (a.len(), b.len()) {
        (1, 1) => norm(&sub(&a[0], &b[0])),
        (1, 2) => segment_distance(&a[0], &b[0], &b[1]),
        (2, 1) => segment_distance(&b[0], &a[0], &a[1]),
        (1, _) => polygon_distance(b, &a[0]),
        (_, 1) => polygon_distance(a, &b[0]),
        _ => extended_distance(a, b),
    }
}
fn extended_distance<S: RouteScalar>(a: &[RoutePoint<S>], b: &[RoutePoint<S>]) -> S {
    if a.len() == 2 && b.len() == 2 && !segments_intersect(&a[0], &a[1], &b[0], &b[1]) {
        return feature_distance(a, b);
    }
    if !convex(a) || !convex(b) {
        return nonconvex_distance(a, b);
    }
    let first: Vec<_> = a.iter().map(primal).collect();
    let second: Vec<_> = b.iter().map(primal).collect();
    let normals = axes(&first).into_iter().chain(axes(&second));
    let distances: Vec<_> = normals.map(|u| escape(&first, &second, &u)).collect();
    if distances.iter().any(|e| *e < 0.0) {
        return feature_distance(a, b);
    }
    let selected = distances
        .iter()
        .enumerate()
        .reduce(|a, b| if a.1 <= b.1 { a } else { b });
    let Some((index, _)) = selected else {
        return a[0].x.constant(0.0);
    };
    let normal = axes(a).into_iter().chain(axes(b)).nth(index);
    normal.map_or_else(|| a[0].x.constant(0.0), |u| escape(a, b, &u).scale(-1.0))
}
fn axes<S: RouteScalar>(v: &[RoutePoint<S>]) -> Vec<RoutePoint<S>> {
    let mut result = Vec::new();
    for (a, b) in edges(v) {
        let d = sub(b, a);
        let length = norm(&d);
        if length.value() > 0.0 {
            result.push(RoutePoint {
                x: d.y.scale(-1.0).div(&length),
                y: d.x.div(&length),
            });
        }
    }
    if v.len() == 2 {
        let d = sub(&v[1], &v[0]);
        let length = norm(&d);
        if length.value() > 0.0 {
            result.push(RoutePoint {
                x: d.x.div(&length),
                y: d.y.div(&length),
            });
        }
    }
    result
}
fn projection<S: RouteScalar>(v: &[RoutePoint<S>], u: &RoutePoint<S>) -> (S, S) {
    let first = dot(&v[0], u);
    v.iter()
        .skip(1)
        .fold((first.clone(), first), |(lo, hi), p| {
            let d = dot(p, u);
            (minimum(lo, d.clone()), super::maximum(hi, d))
        })
}
fn escape<S: RouteScalar>(a: &[RoutePoint<S>], b: &[RoutePoint<S>], u: &RoutePoint<S>) -> S {
    let (alo, ahi) = projection(a, u);
    let (blo, bhi) = projection(b, u);
    minimum(ahi.sub(&blo), bhi.sub(&alo))
}
fn feature_distance<S: RouteScalar>(a: &[RoutePoint<S>], b: &[RoutePoint<S>]) -> S {
    let forward = a.iter().flat_map(|p| edges(b).map(move |(s, t)| (p, s, t)));
    let backward = b.iter().flat_map(|p| edges(a).map(move |(s, t)| (p, s, t)));
    let mut selected = None;
    let mut minimum = f64::INFINITY;
    for (p, s, t) in forward.chain(backward) {
        let distance = segment_distance(&primal(p), &primal(s), &primal(t));
        if selected.is_none()
            || minimum
                .partial_cmp(&distance)
                .is_none_or(std::cmp::Ordering::is_gt)
        {
            selected = Some((p, s, t));
            minimum = distance;
        }
    }
    selected.map_or_else(
        || a[0].x.constant(0.0),
        |(p, s, t)| segment_distance(p, s, t),
    )
}
fn cross<S: RouteScalar>(a: &RoutePoint<S>, b: &RoutePoint<S>, c: &RoutePoint<S>) -> f64 {
    (b.x.value() - a.x.value()) * (c.y.value() - a.y.value())
        - (b.y.value() - a.y.value()) * (c.x.value() - a.x.value())
}
fn convex<S: RouteScalar>(v: &[RoutePoint<S>]) -> bool {
    if v.len() < 4 {
        return true;
    }
    let mut sign = 0.0;
    for index in 0..v.len() {
        let turn = cross(
            &v[index],
            &v[(index + 1) % v.len()],
            &v[(index + 2) % v.len()],
        );
        if turn == 0.0 {
            continue;
        }
        if sign != 0.0 && turn * sign < 0.0 {
            return false;
        }
        sign = turn;
    }
    true
}
fn segments_intersect<S: RouteScalar>(
    a: &RoutePoint<S>,
    b: &RoutePoint<S>,
    c: &RoutePoint<S>,
    d: &RoutePoint<S>,
) -> bool {
    let turns = [
        cross(a, b, c),
        cross(a, b, d),
        cross(c, d, a),
        cross(c, d, b),
    ];
    if turns[0] * turns[1] > 0.0 || turns[2] * turns[3] > 0.0 {
        return false;
    }
    interval_overlap(a.x.value(), b.x.value(), c.x.value(), d.x.value())
        && interval_overlap(a.y.value(), b.y.value(), c.y.value(), d.y.value())
}
fn interval_overlap(a: f64, b: f64, c: f64, d: f64) -> bool {
    a.min(b) <= c.max(d) && c.min(d) <= a.max(b)
}
fn nonconvex_distance<S: RouteScalar>(a: &[RoutePoint<S>], b: &[RoutePoint<S>]) -> S {
    if let Some(crossing) = crossing_distance(a, b, None) {
        return crossing;
    }
    if b.len() > 2 && a.iter().all(|p| super::polygon_contains(b, p)) {
        return contained_corner_distance(a, b);
    }
    if a.len() > 2 && b.iter().all(|p| super::polygon_contains(a, p)) {
        return contained_corner_distance(b, a);
    }
    feature_distance(a, b)
}

fn crossing_distance<S: RouteScalar>(
    a: &[RoutePoint<S>],
    b: &[RoutePoint<S>],
    forced: Option<usize>,
) -> Option<S> {
    edges(a)
        .flat_map(|edge_a| edges(b).map(move |edge_b| (edge_a, edge_b)))
        .filter(|((start, end), (other_start, other_end))| {
            segments_intersect(start, end, other_start, other_end)
        })
        .map(|(edge_a, edge_b)| crossing_pair_distance(a, b, edge_a, edge_b, forced))
        .reduce(minimum)
}
fn crossing_pair_distance<S: RouteScalar>(
    a: &[RoutePoint<S>],
    b: &[RoutePoint<S>],
    edge_a: (&RoutePoint<S>, &RoutePoint<S>),
    edge_b: (&RoutePoint<S>, &RoutePoint<S>),
    forced: Option<usize>,
) -> S {
    let first = if convex(a) {
        a.to_vec()
    } else {
        vec![edge_a.0.clone(), edge_a.1.clone()]
    };
    let second = if convex(b) {
        b.to_vec()
    } else {
        vec![edge_b.0.clone(), edge_b.1.clone()]
    };
    let zero = a[0].x.constant(0.0);
    core_separation(
        &Core::new(first, zero.clone()),
        &Core::new(second, zero),
        forced,
    )
}

fn contained_corner_distance<S: RouteScalar>(
    subject: &[RoutePoint<S>],
    obstacle: &[RoutePoint<S>],
) -> S {
    subject
        .iter()
        .map(|p| polygon_distance(obstacle, p).scale(-1.0))
        .reduce(minimum)
        .unwrap_or_else(|| subject[0].x.constant(0.0))
        .scale(-1.0)
}

#[cfg(test)]
#[allow(
    clippy::float_cmp,
    reason = "exact feature distances and touching classifications are part of the contract"
)]
mod tests {
    use super::*;
    fn p(x: f64, y: f64) -> RoutePoint<f64> {
        RoutePoint { x, y }
    }
    #[test]
    fn crossing_segment_has_escape_depth() {
        let line = Core::new(vec![p(-2.0, 0.0), p(2.0, 0.0)], 0.0);
        let square = Core::new(super::super::rectangle(&p(0.0, 0.0), &2.0, &2.0), 0.0);
        assert_eq!(core_separation(&line, &square, None), -1.0);
    }
    fn concave() -> Core<f64> {
        Core::new(
            vec![
                p(-2.0, -2.0),
                p(2.0, -2.0),
                p(2.0, -1.0),
                p(-1.0, -1.0),
                p(-1.0, 2.0),
                p(-2.0, 2.0),
            ],
            0.0,
        )
    }
    #[test]
    fn concave_notch_is_disjoint_despite_overlapping_bounds() {
        let subject = Core::new(super::super::rectangle(&p(0.0, 0.0), &0.5, &0.5), 0.0);
        assert_eq!(core_separation(&subject, &concave(), None), 0.75);
    }
    #[test]
    fn concave_containment_uses_nearest_subject_corner() {
        let subject = Core::new(super::super::rectangle(&p(-1.5, 0.0), &0.5, &0.5), 0.0);
        assert_eq!(core_separation(&subject, &concave(), None), -0.25);
    }
    #[test]
    fn circle_clearance_uses_radial_distance() {
        let a = Core::new(vec![p(0.0, 0.0)], 1.0);
        let b = Core::new(vec![p(1.5, 1.5)], 1.0);
        assert!((core_separation(&a, &b, None) - (libm::sqrt(4.5) - 2.0)).abs() < 1e-12);
    }
    #[test]
    fn collinear_segments_disjoint_and_touching() {
        let a = Core::new(vec![p(0.0, 0.0), p(1.0, 0.0)], 0.0);
        let b = Core::new(vec![p(2.0, 0.0), p(3.0, 0.0)], 0.0);
        let c = Core::new(vec![p(1.0, 0.0), p(3.0, 0.0)], 0.0);
        assert_eq!(core_separation(&a, &b, None), 1.0);
        assert_eq!(core_separation(&a, &c, None), 0.0);
    }
    #[test]
    fn forced_choices_preserve_opposite_escape_sides() {
        let a = Core::new(vec![p(0.5, 0.0)], 0.0);
        let b = Core::new(super::super::rectangle(&p(0.0, 0.0), &2.0, &2.0), 0.0);
        assert_eq!(core_separation(&a, &b, None), -0.5);
        assert_eq!(core_separation(&a, &b, Some(2)), -0.5);
        assert_eq!(core_separation(&a, &b, Some(3)), -1.5);
        let disjoint = Core::new(vec![p(3.0, 0.0)], 0.0);
        assert_eq!(core_separation(&disjoint, &b, Some(2)), 2.0);
    }
    #[test]
    fn contained_box_escape_is_not_interval_intersection() {
        let a = Core::new(super::super::rectangle(&p(0.0, 0.0), &2.0, &2.0), 0.0);
        let b = Core::new(super::super::rectangle(&p(0.0, 0.0), &10.0, &10.0), 0.0);
        assert_eq!(core_separation(&a, &b, None), -6.0);
    }
}
