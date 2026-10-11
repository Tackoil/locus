//! Deterministic collision penalties against actual strokes and measured boxes.
//!
//! Stroke hits cost 1, label overlaps cost 10 plus overlap area normalized by
//! the label area, and margin violations cost 1 plus normalized excess distance.
//! Stroke width is represented by expanding the tested box by half that width.
//! Curves are tested analytically, without tessellation or platform math calls.

use crate::boxes::expand;
use crate::{LayoutOptions, Leader, PlacedAnnotation};
use locus_core::{
    Arc, BBox, Circle, LayoutCollision, Point, PointMark, ResolvedObject, ResolvedScene, Tolerance,
    Vec2,
};

/// Components of a candidate's penalty. Zero means collision-free.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Score {
    /// Geometry or annotation stroke hits.
    pub strokes: f64,
    /// Overlap with assigned text boxes.
    pub labels: f64,
    /// Exceeding the permitted figure margin.
    pub margin: f64,
}

impl Score {
    /// Total penalty; lower is preferred.
    #[must_use]
    pub fn total(self) -> f64 {
        self.strokes + self.labels + self.margin
    }
}

pub(crate) enum Stroke {
    Linear {
        start: Point,
        direction: Vec2,
        lower: f64,
        upper: f64,
    },
    Circle(Circle),
    Arc(Arc),
    Quadratic {
        from: Point,
        control: Point,
        to: Point,
    },
}

struct Obstacle {
    id: locus_syntax::Ident,
    bounds: Option<BBox>,
    stroke: Stroke,
}

impl Obstacle {
    fn new(id: &locus_syntax::Ident, bounds: Option<BBox>, stroke: Stroke) -> Self {
        Self {
            id: id.clone(),
            bounds,
            stroke,
        }
    }
}

pub(crate) struct Scorer {
    strokes: Vec<Obstacle>,
    // Only connectors carry an owner: attachment to their own text box is
    // intentional, but intersections with every other label must be scored.
    connectors: Vec<Obstacle>,
    viewport: BBox,
    options: LayoutOptions,
    tolerance: Tolerance,
}

impl Scorer {
    pub(crate) fn new(scene: &ResolvedScene, options: LayoutOptions) -> Self {
        let mut strokes = Vec::new();
        for (id, object) in &scene.objects {
            let mut paths = Vec::new();
            object_strokes(object, &mut paths);
            let bounds = match object {
                ResolvedObject::Line(_) | ResolvedObject::Ray(_) => None,
                _ => object.bbox(),
            };
            strokes.extend(
                paths
                    .into_iter()
                    .map(|stroke| Obstacle::new(id, bounds, stroke)),
            );
        }
        Self {
            strokes,
            connectors: Vec::new(),
            viewport: expand(scene.bbox, options.margin),
            options,
            tolerance: Tolerance::from_bbox(&scene.bbox),
        }
    }

    pub(crate) fn score<'a>(
        &self,
        bounds: BBox,
        assigned: impl Iterator<Item = &'a PlacedAnnotation>,
    ) -> Score {
        let strokes = self.stroke_penalty(bounds, None);
        let labels = assigned
            .filter(|a| a.text.is_some())
            .map(|a| overlap_penalty(bounds, a.bounds, self.tolerance))
            .sum();
        let margin = self.margin_penalty(bounds);
        Score {
            strokes,
            labels,
            margin,
        }
    }

    pub(crate) fn stroke_penalty(&self, bounds: BBox, own_label: Option<&str>) -> f64 {
        let padded = expand(bounds, self.options.stroke_width / 2.0);
        self.obstacles(own_label)
            .filter(|obstacle| stroke_hits(&obstacle.stroke, padded))
            .map(|_| 1.0)
            .sum()
    }

    fn obstacles<'a>(&'a self, own_label: Option<&'a str>) -> impl Iterator<Item = &'a Obstacle> {
        self.strokes.iter().chain(
            self.connectors
                .iter()
                .filter(move |obstacle| Some(obstacle.id.name.as_str()) != own_label),
        )
    }

    pub(crate) fn collision_details(&self, bounds: BBox, own_label: &str) -> Vec<LayoutCollision> {
        let padded = expand(bounds, self.options.stroke_width / 2.0);
        let mut hits = indexmap::IndexMap::new();
        for obstacle in self.obstacles(Some(own_label)) {
            if stroke_hits(&obstacle.stroke, padded) {
                hits.entry(&obstacle.id.name).or_insert_with(|| {
                    LayoutCollision::StrokeIntersection {
                        label: own_label.to_owned(),
                        object: obstacle.id.name.clone(),
                        span: obstacle.id.span,
                        label_bounds: bounds.into(),
                        tested_bounds: padded.into(),
                        object_bounds: obstacle.bounds.map(Into::into),
                    }
                });
            }
        }
        let mut details: Vec<_> = hits.into_values().collect();
        if self.margin_penalty(bounds) > 0.0 {
            details.push(LayoutCollision::MarginViolation {
                label: own_label.to_owned(),
                label_bounds: bounds.into(),
                allowed_bounds: self.viewport.into(),
            });
        }
        details
    }

    pub(crate) fn margin_penalty(&self, bounds: BBox) -> f64 {
        if self.tolerance.bbox_contains(self.viewport, bounds) {
            return 0.0;
        }
        let excess = (self.viewport.min.x - bounds.min.x).max(0.0)
            + (self.viewport.min.y - bounds.min.y).max(0.0)
            + (bounds.max.x - self.viewport.max.x).max(0.0)
            + (bounds.max.y - self.viewport.max.y).max(0.0);
        1.0 + excess / self.tolerance.diameter()
    }

    pub(crate) fn add_annotation(&mut self, mark: &PlacedAnnotation) {
        if let Leader::Routed { route, .. } = &mark.leader {
            let points: Vec<Point> = core::iter::once(&route.from)
                .chain(route.via.iter())
                .chain(core::iter::once(&route.to))
                .cloned()
                .map(Into::into)
                .collect();
            self.strokes.extend(points.windows(2).map(|pair| {
                Obstacle::new(
                    &mark.id,
                    Some(mark.bounds),
                    linear(pair[0], pair[1] - pair[0], 0.0, 1.0),
                )
            }));
        }
        self.strokes.extend(mark.strokes.iter().map(|s| {
            Obstacle::new(
                &mark.id,
                Some(mark.bounds),
                linear(s.start, s.end - s.start, 0.0, 1.0),
            )
        }));
        if let Leader::Segment(segment) = mark.leader {
            self.connectors.push(Obstacle::new(
                &mark.id,
                Some(locus_core::BBox::for_segment(segment)),
                linear(segment.start, segment.direction(), 0.0, 1.0),
            ));
        }
        if let Leader::Arrow {
            from, to, control, ..
        } = mark.leader
        {
            self.strokes.push(Obstacle::new(
                &mark.id,
                Some(mark.bounds),
                control.map_or_else(
                    || linear(from, to - from, 0.0, 1.0),
                    |control| Stroke::Quadratic { from, control, to },
                ),
            ));
        }
    }
}

/// Tests positive-area label overlap using the shared core tolerance.
#[must_use]
pub fn boxes_overlap(first: BBox, second: BBox, tolerance: Tolerance) -> bool {
    tolerance
        .length_strictly_positive(first.max.x.min(second.max.x) - first.min.x.max(second.min.x))
        && tolerance
            .length_strictly_positive(first.max.y.min(second.max.y) - first.min.y.max(second.min.y))
}

fn overlap_penalty(first: BBox, second: BBox, tolerance: Tolerance) -> f64 {
    if !boxes_overlap(first, second, tolerance) {
        return 0.0;
    }
    let width = first.max.x.min(second.max.x) - first.min.x.max(second.min.x);
    let height = first.max.y.min(second.max.y) - first.min.y.max(second.min.y);
    10.0 + (width / first.width()) * (height / first.height())
}

fn linear(start: Point, direction: Vec2, lower: f64, upper: f64) -> Stroke {
    Stroke::Linear {
        start,
        direction,
        lower,
        upper,
    }
}

fn object_strokes(object: &ResolvedObject, strokes: &mut Vec<Stroke>) {
    match object {
        ResolvedObject::Point(point) if point.mark != PointMark::None => {
            strokes.push(linear(point.geometry, Vec2::new(0.0, 0.0), 0.0, 1.0));
        }
        ResolvedObject::Line(line) => strokes.push(linear(
            line.start,
            line.direction(),
            f64::NEG_INFINITY,
            f64::INFINITY,
        )),
        ResolvedObject::Ray(ray) => {
            strokes.push(linear(ray.start, ray.direction(), 0.0, f64::INFINITY));
        }
        ResolvedObject::Segment(segment) => {
            strokes.push(linear(segment.start, segment.direction(), 0.0, 1.0));
        }
        _ => curved_strokes(object, strokes),
    }
}

fn curved_strokes(object: &ResolvedObject, strokes: &mut Vec<Stroke>) {
    match object {
        ResolvedObject::RoundedRect(rect) => {
            strokes.extend(
                rect.sides()
                    .map(|side| linear(side.start, side.direction(), 0.0, 1.0)),
            );
            strokes.extend(rect.corner_arcs().into_iter().map(Stroke::Arc));
        }
        ResolvedObject::Circle(circle) => strokes.push(Stroke::Circle(*circle)),
        ResolvedObject::Arc(arc) => strokes.push(Stroke::Arc(arc.geometry)),
        ResolvedObject::Polygon(polygon)
        | ResolvedObject::Rect(polygon)
        | ResolvedObject::Diamond(polygon) => {
            let vertices = polygon.vertices();
            for index in 0..vertices.len() {
                let start = vertices[index];
                strokes.push(linear(
                    start,
                    vertices[(index + 1) % vertices.len()] - start,
                    0.0,
                    1.0,
                ));
            }
        }
        _ => {}
    }
}

fn stroke_hits(stroke: &Stroke, bounds: BBox) -> bool {
    match *stroke {
        Stroke::Linear {
            start,
            direction,
            lower,
            upper,
        } => line_hits(start, direction, lower, upper, bounds),
        Stroke::Circle(circle) => circle_hits(circle, bounds),
        Stroke::Arc(arc) => arc_hits(arc, bounds),
        Stroke::Quadratic { from, control, to } => quadratic_hits(from, control, to, bounds),
    }
}

fn line_hits(start: Point, direction: Vec2, mut lower: f64, mut upper: f64, bounds: BBox) -> bool {
    for (origin, delta, min, max) in [
        (start.x, direction.x, bounds.min.x, bounds.max.x),
        (start.y, direction.y, bounds.min.y, bounds.max.y),
    ] {
        if delta == 0.0 {
            if origin < min || origin > max {
                return false;
            }
        } else {
            let first = (min - origin) / delta;
            let second = (max - origin) / delta;
            lower = lower.max(first.min(second));
            upper = upper.min(first.max(second));
        }
    }
    lower <= upper
}

fn inside(point: Point, bounds: BBox) -> bool {
    point.x >= bounds.min.x
        && point.x <= bounds.max.x
        && point.y >= bounds.min.y
        && point.y <= bounds.max.y
}

fn circle_hits(circle: Circle, bounds: BBox) -> bool {
    let closest = Point::new(
        circle.center.x.clamp(bounds.min.x, bounds.max.x),
        circle.center.y.clamp(bounds.min.y, bounds.max.y),
    );
    let farthest = corners(bounds)
        .into_iter()
        .map(|p| (p - circle.center).norm())
        .fold(0.0, f64::max);
    (closest - circle.center).norm() <= circle.radius && farthest >= circle.radius
}

fn corners(bounds: BBox) -> [Point; 4] {
    [
        bounds.min,
        Point::new(bounds.max.x, bounds.min.y),
        bounds.max,
        Point::new(bounds.min.x, bounds.max.y),
    ]
}

fn arc_hits(arc: Arc, bounds: BBox) -> bool {
    if inside(arc.start, bounds) || inside(arc.end, bounds) {
        return true;
    }
    circle_box_intersections(arc.circle, bounds)
        .into_iter()
        .any(|point| on_sweep(arc, point))
}

fn circle_box_intersections(circle: Circle, bounds: BBox) -> Vec<Point> {
    let mut points = Vec::new();
    for x in [bounds.min.x, bounds.max.x] {
        let offset = (x - circle.center.x) / circle.radius;
        if offset.abs() <= 1.0 {
            let height = circle.radius * libm::sqrt((1.0 - offset * offset).max(0.0));
            points.extend([
                Point::new(x, circle.center.y - height),
                Point::new(x, circle.center.y + height),
            ]);
        }
    }
    for y in [bounds.min.y, bounds.max.y] {
        let offset = (y - circle.center.y) / circle.radius;
        if offset.abs() <= 1.0 {
            let width = circle.radius * libm::sqrt((1.0 - offset * offset).max(0.0));
            points.extend([
                Point::new(circle.center.x - width, y),
                Point::new(circle.center.x + width, y),
            ]);
        }
    }
    points.into_iter().filter(|p| inside(*p, bounds)).collect()
}

fn on_sweep(arc: Arc, point: Point) -> bool {
    let angle =
        |point: Point| libm::atan2(point.y - arc.circle.center.y, point.x - arc.circle.center.x);
    let sign = if arc.ccw { 1.0 } else { -1.0 };
    let sweep = ((angle(arc.end) - angle(arc.start)) * sign).rem_euclid(std::f64::consts::TAU);
    let position = ((angle(point) - angle(arc.start)) * sign).rem_euclid(std::f64::consts::TAU);
    position <= sweep
}

fn quadratic_hits(from: Point, control: Point, to: Point, bounds: BBox) -> bool {
    if inside(from, bounds) || inside(to, bounds) {
        return true;
    }
    let a = (to - control) - (control - from);
    let b = (control - from) * 2.0;
    [bounds.min.x, bounds.max.x].into_iter().any(|edge| {
        boundary_hit(
            a.x,
            b.x,
            from.x - edge,
            |t| from.y + b.y * t + a.y * t * t,
            bounds.min.y,
            bounds.max.y,
        )
    }) || [bounds.min.y, bounds.max.y].into_iter().any(|edge| {
        boundary_hit(
            a.y,
            b.y,
            from.y - edge,
            |t| from.x + b.x * t + a.x * t * t,
            bounds.min.x,
            bounds.max.x,
        )
    })
}

fn boundary_hit(
    a: f64,
    b: f64,
    c: f64,
    other_coordinate: impl Fn(f64) -> f64,
    min: f64,
    max: f64,
) -> bool {
    roots(a, b, c)
        .into_iter()
        .filter(|t| (0.0..=1.0).contains(t))
        .any(|t| (min..=max).contains(&other_coordinate(t)))
}

fn roots(a: f64, b: f64, c: f64) -> Vec<f64> {
    // Scaling prevents intermediate products from overflowing/underflowing
    // solely because all coefficients share a very large/small magnitude.
    let scale = a.abs().max(b.abs()).max(c.abs());
    if scale == 0.0 {
        return Vec::new();
    }
    let (a, b, c) = (a / scale, b / scale, c / scale);
    if a == 0.0 {
        return if b == 0.0 { Vec::new() } else { vec![-c / b] };
    }
    let discriminant = libm::fma(-4.0 * a, c, b * b);
    if discriminant < 0.0 {
        return Vec::new();
    }
    // Choose the sign that adds magnitudes instead of subtracting nearly
    // equal values. Recover the second root from their product c/a.
    let q = -0.5 * (b + libm::copysign(libm::sqrt(discriminant), b));
    if q == 0.0 {
        return vec![-b / (2.0 * a)];
    }
    vec![q / a, c / q]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn box_at(x: f64, y: f64, half: f64) -> BBox {
        BBox {
            min: Point::new(x - half, y - half),
            max: Point::new(x + half, y + half),
        }
    }

    #[test]
    fn quadratic_roots_preserve_small_roots_and_common_coefficient_scales() {
        let tolerance = Tolerance::from_bbox(&box_at(0.0, 0.0, 1.0));
        let small_root = roots(-0.000_000_000_000_000_8, 4.0, -0.4);
        assert!(
            small_root
                .iter()
                .any(|root| tolerance.scalar_zero(*root - 0.1, 0.0, 1.0))
        );
        for scale in [1.0, 1.0e200, 1.0e-200] {
            let mut actual = roots(scale, -3.0 * scale, 2.0 * scale);
            actual.sort_by(f64::total_cmp);
            assert_eq!(actual.len(), 2);
            assert!(tolerance.scalar_zero(actual[0] - 1.0, 0.0, 1.0));
            assert!(tolerance.scalar_zero(actual[1] - 2.0, 0.0, 1.0));
        }
        assert_eq!(roots(1.0, 0.0, 0.0), vec![0.0]);
        assert!(roots(1.0, 0.0, 1.0).is_empty());
        assert_eq!(roots(0.0, 2.0, -1.0), vec![0.5]);
    }

    #[test]
    fn linear_strokes_use_carriers_and_domains_instead_of_definition_boxes() {
        let start = Point::new(0.0, 0.0);
        let direction = Vec2::new(1.0, 1.0);
        assert!(line_hits(
            start,
            direction,
            f64::NEG_INFINITY,
            f64::INFINITY,
            box_at(100.0, 100.0, 0.1)
        ));
        assert!(!line_hits(
            start,
            direction,
            0.0,
            1.0,
            box_at(100.0, 100.0, 0.1)
        ));
        assert!(!line_hits(
            start,
            direction,
            0.0,
            f64::INFINITY,
            box_at(-10.0, -10.0, 0.1)
        ));
        assert!(!line_hits(
            start,
            direction,
            0.0,
            1.0,
            box_at(0.8, 0.2, 0.1)
        ));
        assert!(line_hits(
            start,
            Vec2::new(0.0, 0.0),
            0.0,
            1.0,
            box_at(0.0, 0.0, 0.1)
        ));
    }

    #[test]
    fn circle_interior_is_clear_and_enclosed_circumference_collides() {
        let circle = Circle {
            center: Point::new(0.0, 0.0),
            radius: 2.0,
        };
        assert!(!circle_hits(circle, box_at(0.0, 0.0, 0.5)));
        assert!(circle_hits(circle, box_at(0.0, 0.0, 3.0)));
        assert!(circle_hits(circle, box_at(2.0, 0.0, 0.1)));
        assert!(!circle_hits(circle, box_at(3.0, 0.0, 0.1)));
    }

    #[test]
    fn directed_arcs_only_collide_on_their_actual_sweep() {
        let arc = Arc {
            circle: Circle {
                center: Point::new(0.0, 0.0),
                radius: 2.0,
            },
            start: Point::new(2.0, 0.0),
            end: Point::new(0.0, 2.0),
            ccw: true,
        };
        let diagonal = libm::sqrt(2.0);
        assert!(arc_hits(arc, box_at(diagonal, diagonal, 0.1)));
        assert!(!arc_hits(arc, box_at(-diagonal, -diagonal, 0.1)));
        assert!(!arc_hits(arc, box_at(0.0, 0.0, 0.1)));
        let clockwise = Arc { ccw: false, ..arc };
        assert!(!arc_hits(clockwise, box_at(diagonal, diagonal, 0.1)));
        assert!(arc_hits(clockwise, box_at(-diagonal, -diagonal, 0.1)));
    }

    #[test]
    fn quadratic_arrow_checks_curve_instead_of_chord_or_control_hull() {
        let from = Point::new(-2.0, 0.0);
        let to = Point::new(2.0, 0.0);
        let control = Point::new(0.0, 4.0);
        assert!(quadratic_hits(from, control, to, box_at(0.0, 2.0, 0.1)));
        assert!(!quadratic_hits(from, control, to, box_at(0.0, 0.0, 0.1)));
        assert!(!quadratic_hits(from, control, to, box_at(0.0, 3.0, 0.1)));
    }

    #[test]
    fn touching_boxes_have_no_positive_area_collision() {
        let first = box_at(0.0, 0.0, 1.0);
        let tolerance = Tolerance::from_bbox(&first);
        assert!(!boxes_overlap(first, box_at(2.0, 0.0, 1.0), tolerance));
        assert!(boxes_overlap(first, box_at(1.5, 0.0, 1.0), tolerance));
        assert!(!boxes_overlap(first, box_at(0.0, 0.0, 0.0), tolerance));
    }
    #[test]
    fn rounded_rect_collisions_follow_arcs_and_leave_cutout_corners_clear()
    -> Result<(), Box<dyn std::error::Error>> {
        let rect = locus_core::RoundedRect::new(Point::new(0.0, 0.0), 6.0, 4.0, 1.0)?;
        let mut strokes = Vec::new();
        super::object_strokes(&locus_core::ResolvedObject::RoundedRect(rect), &mut strokes);
        let hits = |bounds| {
            strokes
                .iter()
                .any(|stroke| super::stroke_hits(stroke, bounds))
        };
        assert!(hits(box_at(0.0, 2.0, 0.01)));
        assert!(hits(box_at(
            2.0 + libm::sqrt(0.5),
            1.0 + libm::sqrt(0.5),
            0.01
        )));
        assert!(!hits(box_at(3.0, 2.0, 0.01)));
        assert!(!hits(box_at(0.0, 0.0, 0.1)));
        Ok(())
    }
}
