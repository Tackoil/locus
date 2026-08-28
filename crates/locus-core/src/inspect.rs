//! Deterministic, machine-readable measurements and scene diagnostics.

use std::collections::{BTreeMap, BTreeSet};

use locus_syntax::{Constructor, Expr, File, Ident, PropValue, Span, Statement};
use serde::Serialize;

use crate::Tolerance;
use crate::assert::{point_on_object, tangent_objects};
use crate::geom::measure;
use crate::geom::{BBox, Point};
use crate::scene::{Dimension, Evaluation, ResolvedObject, ResolvedScene, Scene};
use crate::solve::SolveSummary;

/// Complete `inspect` output.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct InspectionReport {
    /// Numeric tables in deterministic source order.
    pub measurements: MeasurementTables,
    /// Automatically detected geometric facts.
    pub incidences: Vec<Incidence>,
    /// Geometry, solve, visibility, and reference diagnostics.
    pub diagnostics: Vec<InspectDiagnostic>,
}

/// All measurement-table families required by the interchange contract.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct MeasurementTables {
    /// Pairwise distances among named points.
    pub distances: Vec<DistanceMeasurement>,
    /// Three angles for every nondegenerate named-point triangle.
    pub triangle_angles: Vec<TriangleAngleMeasurement>,
    /// Ratios made by named points lying strictly between two other named points.
    pub line_ratios: Vec<LineRatioMeasurement>,
}

/// A pairwise named-point distance.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct DistanceMeasurement {
    /// First point in file order.
    pub first: String,
    /// Second point in file order.
    pub second: String,
    /// Distance in scene units.
    pub distance: f64,
}

/// One interior angle of a named-point triangle.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct TriangleAngleMeasurement {
    /// First angle ray point.
    pub first: String,
    /// Angle vertex.
    pub vertex: String,
    /// Second angle ray point.
    pub second: String,
    /// Unsigned interior angle in radians.
    pub radians: f64,
}

/// A named point dividing the line segment between two other named points.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct LineRatioMeasurement {
    /// First endpoint.
    pub start: String,
    /// Strictly interior point.
    pub point: String,
    /// Second endpoint.
    pub end: String,
    /// `dist(start, point) / dist(point, end)`.
    pub ratio: f64,
    /// Small integer description when detected at incidence tolerance.
    pub integer_ratio: Option<IntegerRatio>,
}

/// Reduced positive integer ratio.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct IntegerRatio {
    /// Left ratio term.
    pub left: u32,
    /// Right ratio term.
    pub right: u32,
}

/// A geometry fact detected with the same tolerance model as assertions.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Incidence {
    /// A named point lies on a named closed/open geometry object.
    PointOnObject {
        /// Point id.
        point: String,
        /// Target object id.
        object: String,
    },
    /// A named circle is tangent to a named circle or linear object.
    Tangent {
        /// First object in file order.
        first: String,
        /// Second object in file order.
        second: String,
    },
    /// A named point divides a line between named endpoints in a small integer ratio.
    RatioAlongLine {
        /// First endpoint.
        start: String,
        /// Interior point.
        point: String,
        /// Second endpoint.
        end: String,
        /// Reduced ratio term for `start-point`.
        left: u32,
        /// Reduced ratio term for `point-end`.
        right: u32,
    },
}

/// An inspect diagnostic. Layout appends `LabelCollision` after placement.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum InspectDiagnostic {
    /// An object stores geometry that cannot supply a required direction.
    Degenerate {
        /// Object id.
        id: String,
        /// Declaration span.
        span: Span,
        /// Human-readable failed property.
        reason: String,
    },
    /// A solved free value moved farther than the normative hint threshold.
    HintFar {
        /// Object id.
        id: String,
        /// Declaration span.
        span: Span,
        /// Largest normalized component displacement.
        actual: f64,
        /// Normative threshold.
        threshold: f64,
    },
    /// Reserved for `locus-layout` to populate after label placement.
    LabelCollision {
        /// Colliding label ids in deterministic order.
        labels: Vec<String>,
        /// Corresponding source spans.
        spans: Vec<Span>,
    },
    /// An object's geometry bounds lie outside the resolved geometry viewport bounds.
    OutOfView {
        /// Object id.
        id: String,
        /// Declaration span.
        span: Span,
        /// Object geometry bounds.
        bounds: GeometryBounds,
    },
    /// Two named points are too close for comfortable visual separation.
    Crowding {
        /// First point in file order.
        first: String,
        /// Second point in file order.
        second: String,
        /// Point distance in scene units.
        distance: f64,
    },
    /// A declaration has no incoming source reference.
    Unreferenced {
        /// Declaration id.
        id: String,
        /// Declaration span.
        span: Span,
    },
}

/// Serializable axis-aligned geometry bounds.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct GeometryBounds {
    /// Minimum x coordinate.
    pub min_x: f64,
    /// Minimum y coordinate.
    pub min_y: f64,
    /// Maximum x coordinate.
    pub max_x: f64,
    /// Maximum y coordinate.
    pub max_y: f64,
}

/// Inspects an already evaluated `Scene`, including solve-only diagnostics when present.
#[must_use]
pub fn inspect_scene(scene: &Scene) -> InspectionReport {
    match &scene.evaluation {
        Evaluation::Resolved(resolved) => inspect(&scene.file, resolved, None),
        Evaluation::Solved {
            scene: resolved,
            summary,
        } => inspect(&scene.file, resolved, Some(summary)),
    }
}

/// Builds a deterministic inspection report for a resolved scene.
#[must_use]
pub fn inspect(
    file: &File,
    scene: &ResolvedScene,
    solve: Option<&SolveSummary>,
) -> InspectionReport {
    let tolerance = inspection_tolerance(scene);
    let viewport = visible_geometry_bbox(scene);
    let points = named_points(scene);
    InspectionReport {
        measurements: measurement_tables(&points, tolerance),
        incidences: detected_incidences(scene, &points, tolerance),
        diagnostics: diagnostics(file, scene, &points, solve, tolerance, viewport),
    }
}

fn inspection_tolerance(scene: &ResolvedScene) -> Tolerance {
    let length_fallbacks = scene
        .objects
        .values()
        .filter_map(|object| match object {
            ResolvedObject::Num(number) if number.dimension == Dimension::Length => {
                Some(number.value)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    Tolerance::from_bbox_with_fallbacks(&scene.bbox, &length_fallbacks)
}

fn visible_geometry_bbox(scene: &ResolvedScene) -> Option<BBox> {
    let mut viewport = None;
    for object in scene.objects.values().filter(|object| is_visible(object)) {
        let Some(bounds) = object.bbox() else {
            continue;
        };
        if let Some(current) = &mut viewport {
            BBox::include_bbox(current, bounds);
        } else {
            viewport = Some(bounds);
        }
    }
    viewport
}

fn is_visible(object: &ResolvedObject) -> bool {
    !matches!(
        object,
        ResolvedObject::Num(_)
            | ResolvedObject::Angle(_)
            | ResolvedObject::Annotation(_)
            | ResolvedObject::Point(crate::scene::ResolvedPoint {
                mark: crate::scene::PointMark::None,
                ..
            })
    )
}

#[derive(Clone, Copy)]
struct NamedPoint<'a> {
    id: &'a Ident,
    point: Point,
    index: usize,
}

fn named_points(scene: &ResolvedScene) -> Vec<NamedPoint<'_>> {
    scene
        .objects
        .iter()
        .enumerate()
        .filter_map(|(index, (id, object))| match object {
            ResolvedObject::Point(point) => Some(NamedPoint {
                id,
                point: point.geometry,
                index,
            }),
            _ => None,
        })
        .collect()
}

fn measurement_tables(points: &[NamedPoint<'_>], tolerance: Tolerance) -> MeasurementTables {
    MeasurementTables {
        distances: distance_measurements(points),
        triangle_angles: triangle_angle_measurements(points, tolerance),
        line_ratios: line_ratio_measurements(points, tolerance),
    }
}

fn distance_measurements(points: &[NamedPoint<'_>]) -> Vec<DistanceMeasurement> {
    let mut output = Vec::new();
    for first in 0..points.len() {
        for second in first + 1..points.len() {
            output.push(DistanceMeasurement {
                first: points[first].id.name.clone(),
                second: points[second].id.name.clone(),
                distance: measure::dist(points[first].point, points[second].point),
            });
        }
    }
    output
}

fn triangle_angle_measurements(
    points: &[NamedPoint<'_>],
    tolerance: Tolerance,
) -> Vec<TriangleAngleMeasurement> {
    let mut output = Vec::new();
    for first in 0..points.len() {
        for second in first + 1..points.len() {
            for third in second + 1..points.len() {
                push_triangle_angles(
                    &mut output,
                    [points[first], points[second], points[third]],
                    tolerance,
                );
            }
        }
    }
    output
}

fn push_triangle_angles(
    output: &mut Vec<TriangleAngleMeasurement>,
    points: [NamedPoint<'_>; 3],
    tolerance: Tolerance,
) {
    for [first, vertex, second] in [
        [points[1], points[0], points[2]],
        [points[0], points[1], points[2]],
        [points[0], points[2], points[1]],
    ] {
        if let Ok(radians) = measure::angle(first.point, vertex.point, second.point, tolerance) {
            output.push(TriangleAngleMeasurement {
                first: first.id.name.clone(),
                vertex: vertex.id.name.clone(),
                second: second.id.name.clone(),
                radians,
            });
        }
    }
}

fn line_ratio_measurements(
    points: &[NamedPoint<'_>],
    tolerance: Tolerance,
) -> Vec<LineRatioMeasurement> {
    let mut output = Vec::new();
    for start in 0..points.len() {
        for end in start + 1..points.len() {
            for point in 0..points.len() {
                if point != start
                    && point != end
                    && let Some(measurement) =
                        line_ratio(points[start], points[point], points[end], tolerance)
                {
                    output.push(measurement);
                }
            }
        }
    }
    output
}

fn line_ratio(
    start: NamedPoint<'_>,
    point: NamedPoint<'_>,
    end: NamedPoint<'_>,
    tolerance: Tolerance,
) -> Option<LineRatioMeasurement> {
    let direction = end.point - start.point;
    if tolerance.construction_length_zero(direction.norm())
        || !tolerance.vector_on_direction(point.point - start.point, direction)
    {
        return None;
    }
    let parameter = (point.point - start.point).dot(direction) / direction.norm_squared();
    if !tolerance.parameter_strictly_inside_unit(parameter) {
        return None;
    }
    let left_length = measure::dist(start.point, point.point);
    let right_length = measure::dist(point.point, end.point);
    let integer_ratio = tolerance
        .small_integer_ratio(left_length, right_length)
        .map(|(left, right)| IntegerRatio { left, right });
    Some(LineRatioMeasurement {
        start: start.id.name.clone(),
        point: point.id.name.clone(),
        end: end.id.name.clone(),
        ratio: left_length / right_length,
        integer_ratio,
    })
}

fn detected_incidences(
    scene: &ResolvedScene,
    points: &[NamedPoint<'_>],
    tolerance: Tolerance,
) -> Vec<Incidence> {
    let mut staged = Vec::new();
    detect_point_on_object(scene, points, tolerance, &mut staged);
    detect_tangencies(scene, tolerance, &mut staged);
    detect_ratios(points, tolerance, &mut staged);
    staged.sort_by(|lhs, rhs| (&lhs.0, &lhs.1, &lhs.2).cmp(&(&rhs.0, &rhs.1, &rhs.2)));
    staged.into_iter().map(|(_, _, _, value)| value).collect()
}

type StagedIncidence = (usize, usize, String, Incidence);

fn detect_point_on_object(
    scene: &ResolvedScene,
    points: &[NamedPoint<'_>],
    tolerance: Tolerance,
    output: &mut Vec<StagedIncidence>,
) {
    for point in points {
        for (object_index, (id, object)) in scene.objects.iter().enumerate() {
            if is_incidence_target(object)
                && point_on_object(point.point, object, tolerance, id.span).unwrap_or(false)
            {
                output.push((
                    point.index,
                    object_index,
                    format!("point_on_object:{}:{}", point.id.name, id.name),
                    Incidence::PointOnObject {
                        point: point.id.name.clone(),
                        object: id.name.clone(),
                    },
                ));
            }
        }
    }
}

fn is_incidence_target(object: &ResolvedObject) -> bool {
    matches!(
        object,
        ResolvedObject::Line(_)
            | ResolvedObject::Ray(_)
            | ResolvedObject::Segment(_)
            | ResolvedObject::Circle(_)
            | ResolvedObject::Arc(_)
            | ResolvedObject::Polygon(_)
            | ResolvedObject::Rect(_)
            | ResolvedObject::Diamond(_)
    )
}

fn detect_tangencies(
    scene: &ResolvedScene,
    tolerance: Tolerance,
    output: &mut Vec<StagedIncidence>,
) {
    for first in 0..scene.objects.len() {
        for second in first + 1..scene.objects.len() {
            let Some((first_id, first_object)) = scene.objects.get_index(first) else {
                continue;
            };
            let Some((second_id, second_object)) = scene.objects.get_index(second) else {
                continue;
            };
            if tangent_candidate(first_object, second_object)
                && tangent_objects(first_object, second_object, tolerance, first_id.span)
                    .unwrap_or(false)
            {
                output.push((
                    first,
                    second,
                    format!("tangent:{}:{}", first_id.name, second_id.name),
                    Incidence::Tangent {
                        first: first_id.name.clone(),
                        second: second_id.name.clone(),
                    },
                ));
            }
        }
    }
}

fn tangent_candidate(lhs: &ResolvedObject, rhs: &ResolvedObject) -> bool {
    matches!(lhs, ResolvedObject::Circle(_)) && is_circle_or_linear(rhs)
        || matches!(rhs, ResolvedObject::Circle(_)) && is_circle_or_linear(lhs)
}

fn is_circle_or_linear(object: &ResolvedObject) -> bool {
    matches!(
        object,
        ResolvedObject::Circle(_)
            | ResolvedObject::Line(_)
            | ResolvedObject::Ray(_)
            | ResolvedObject::Segment(_)
    )
}

fn detect_ratios(
    points: &[NamedPoint<'_>],
    tolerance: Tolerance,
    output: &mut Vec<StagedIncidence>,
) {
    for start in 0..points.len() {
        for end in start + 1..points.len() {
            for point in 0..points.len() {
                let Some(measurement) = (point != start && point != end)
                    .then(|| line_ratio(points[start], points[point], points[end], tolerance))
                    .flatten()
                else {
                    continue;
                };
                let Some(ratio) = measurement.integer_ratio else {
                    continue;
                };
                output.push((
                    points[start].index,
                    points[point].index,
                    format!(
                        "ratio:{}:{}:{}",
                        measurement.start, measurement.point, measurement.end
                    ),
                    Incidence::RatioAlongLine {
                        start: measurement.start,
                        point: measurement.point,
                        end: measurement.end,
                        left: ratio.left,
                        right: ratio.right,
                    },
                ));
            }
        }
    }
}

fn diagnostics(
    file: &File,
    scene: &ResolvedScene,
    points: &[NamedPoint<'_>],
    solve: Option<&SolveSummary>,
    tolerance: Tolerance,
    viewport: Option<BBox>,
) -> Vec<InspectDiagnostic> {
    let referenced = referenced_ids(file);
    let hints = hint_diagnostics(solve);
    let mut staged = Vec::new();
    for (index, (id, object)) in scene.objects.iter().enumerate() {
        stage_object_diagnostics(
            &mut staged,
            index,
            id,
            object,
            viewport,
            tolerance,
            &referenced,
            &hints,
        );
    }
    stage_crowding(&mut staged, points, tolerance);
    staged.sort_by(|lhs, rhs| (&lhs.0, &lhs.1).cmp(&(&rhs.0, &rhs.1)));
    staged.into_iter().map(|(_, _, value)| value).collect()
}

type StagedDiagnostic = (usize, String, InspectDiagnostic);

#[allow(clippy::too_many_arguments)]
fn stage_object_diagnostics(
    output: &mut Vec<StagedDiagnostic>,
    index: usize,
    id: &Ident,
    object: &ResolvedObject,
    viewport: Option<BBox>,
    tolerance: Tolerance,
    referenced: &BTreeSet<String>,
    hints: &BTreeMap<String, InspectDiagnostic>,
) {
    if is_degenerate(object, tolerance) {
        output.push((
            index,
            "degenerate".to_owned(),
            InspectDiagnostic::Degenerate {
                id: id.name.clone(),
                span: id.span,
                reason: "zero-length linear direction".to_owned(),
            },
        ));
    }
    if let Some(diagnostic) = hints.get(&id.name) {
        output.push((index, "hint_far".to_owned(), diagnostic.clone()));
    }
    if let (Some(viewport), Some(bounds)) = (viewport, object.bbox())
        && !tolerance.bbox_contains(viewport, bounds)
    {
        output.push((
            index,
            "out_of_view".to_owned(),
            InspectDiagnostic::OutOfView {
                id: id.name.clone(),
                span: id.span,
                bounds: bounds.into(),
            },
        ));
    }
    if !referenced.contains(&id.name) {
        output.push((
            index,
            "unreferenced".to_owned(),
            InspectDiagnostic::Unreferenced {
                id: id.name.clone(),
                span: id.span,
            },
        ));
    }
}

fn is_degenerate(object: &ResolvedObject, tolerance: Tolerance) -> bool {
    matches!(object, ResolvedObject::Segment(segment) if tolerance.construction_length_zero(segment.direction().norm()))
}

fn hint_diagnostics(solve: Option<&SolveSummary>) -> BTreeMap<String, InspectDiagnostic> {
    solve
        .into_iter()
        .flat_map(|summary| &summary.hint_displacements)
        .filter(|hint| hint.hint_far)
        .map(|hint| {
            (
                hint.id.clone(),
                InspectDiagnostic::HintFar {
                    id: hint.id.clone(),
                    span: hint.span,
                    actual: hint.max_normalized_component,
                    threshold: hint.hint_far_threshold,
                },
            )
        })
        .collect()
}

fn stage_crowding(
    output: &mut Vec<StagedDiagnostic>,
    points: &[NamedPoint<'_>],
    tolerance: Tolerance,
) {
    for first in 0..points.len() {
        for second in first + 1..points.len() {
            if tolerance.points_crowded(points[first].point, points[second].point) {
                output.push((
                    points[first].index,
                    format!(
                        "crowding:{}:{}",
                        points[first].id.name, points[second].id.name
                    ),
                    InspectDiagnostic::Crowding {
                        first: points[first].id.name.clone(),
                        second: points[second].id.name.clone(),
                        distance: measure::dist(points[first].point, points[second].point),
                    },
                ));
            }
        }
    }
}

fn referenced_ids(file: &File) -> BTreeSet<String> {
    let mut output = BTreeSet::new();
    for statement in &file.statements {
        collect_statement_references(statement, &mut output);
    }
    output
}

fn collect_statement_references(statement: &Statement, output: &mut BTreeSet<String>) {
    match statement {
        Statement::Op(op) => collect_op_references(op, output),
        Statement::Rel(relation) => collect_expr_references(&relation.expr, output),
        Statement::Assert(assertion) => collect_expr_references(&assertion.expr, output),
        Statement::Meta(_) | Statement::Style(_) | Statement::Class(_) => {}
    }
}

fn collect_op_references(op: &locus_syntax::OpStmt, output: &mut BTreeSet<String>) {
    collect_constructor_references(&op.ctor, output);
    for expression in &op.where_clauses {
        collect_expr_references(expression, output);
    }
    for property in &op.props.items {
        if let PropValue::Expr(expression) = &property.value {
            collect_expr_references(expression, output);
        }
    }
}

fn collect_constructor_references(constructor: &Constructor, output: &mut BTreeSet<String>) {
    match constructor {
        Constructor::Call { args, .. } => collect_expr_list_references(args, output),
        Constructor::Expr { expr, .. } | Constructor::FreeNum { hint: expr, .. } => {
            collect_expr_references(expr, output);
        }
        Constructor::FreePoint { x, y, .. } => {
            collect_expr_references(x, output);
            collect_expr_references(y, output);
        }
    }
}

fn collect_expr_references(expression: &Expr, output: &mut BTreeSet<String>) {
    if let Expr::Ref(id) = expression {
        output.insert(id.name.clone());
        return;
    }
    collect_compound_expr_references(expression, output);
}

fn collect_compound_expr_references(expression: &Expr, output: &mut BTreeSet<String>) {
    match expression {
        Expr::Call { args, .. } => collect_expr_list_references(args, output),
        Expr::Property { base, args, .. } => {
            collect_expr_references(base, output);
            collect_expr_list_references(args, output);
        }
        Expr::Group { expr, .. } | Expr::Unary { expr, .. } => {
            collect_expr_references(expr, output);
        }
        Expr::Binary { lhs, rhs, .. } => {
            collect_expr_references(lhs, output);
            collect_expr_references(rhs, output);
        }
        Expr::Number(_) | Expr::Boolean { .. } | Expr::String(_) | Expr::Tex(_) | Expr::Ref(_) => {}
    }
}

fn collect_expr_list_references(expressions: &[Expr], output: &mut BTreeSet<String>) {
    for expression in expressions {
        collect_expr_references(expression, output);
    }
}

impl From<BBox> for GeometryBounds {
    fn from(value: BBox) -> Self {
        Self {
            min_x: value.min.x,
            min_y: value.min.y,
            max_x: value.max.x,
            max_y: value.max.y,
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use indexmap::IndexMap;

    use super::*;
    use crate::geom::{Circle, Point, Segment};
    use crate::scene::{PointMark, ResolvedPoint};

    fn ident(name: &str, line: usize) -> Ident {
        Ident {
            name: name.to_owned(),
            span: Span::new(line, 1, line, line + 1),
        }
    }

    fn incidence_fixture(offset: f64) -> ResolvedScene {
        let mut objects = IndexMap::new();
        let a = Point::new(0.0, 0.0);
        let m = Point::new(1.0, offset);
        let b = Point::new(2.0, 0.0);
        objects.insert(ident("A", 1), point(a));
        objects.insert(ident("M", 2), point(m));
        objects.insert(ident("B", 3), point(b));
        objects.insert(
            ident("AB", 4),
            ResolvedObject::Segment(Segment::new(a, b).expect("segment")),
        );
        objects.insert(
            ident("inc", 5),
            ResolvedObject::Circle(Circle::new(Point::new(1.0, 1.0), 1.0).expect("circle")),
        );
        ResolvedScene {
            objects,
            bbox: BBox::from_points([Point::new(0.0, 0.0), Point::new(2.0, 2.0)]).expect("bbox"),
        }
    }

    fn point(value: Point) -> ResolvedObject {
        ResolvedObject::Point(ResolvedPoint {
            geometry: value,
            mark: PointMark::Dot,
        })
    }

    #[test]
    fn fixture_detects_point_on_object_tangency_and_ratio() {
        let scene = incidence_fixture(0.0);
        let report = inspect(&empty_file(), &scene, None);
        assert!(report.incidences.iter().any(|incidence| matches!(
            incidence,
            Incidence::PointOnObject { point, object } if point == "M" && object == "AB"
        )));
        assert!(report.incidences.iter().any(|incidence| matches!(
            incidence,
            Incidence::Tangent { first, second } if first == "AB" && second == "inc"
        )));
        assert!(report.incidences.iter().any(|incidence| matches!(
            incidence,
            Incidence::RatioAlongLine { point, left: 1, right: 1, .. } if point == "M"
        )));
    }

    #[test]
    fn incidence_respects_just_inside_and_outside_epsilon() {
        let inside = inspect(&empty_file(), &incidence_fixture(2.7e-9), None);
        let outside = inspect(&empty_file(), &incidence_fixture(2.9e-9), None);
        let has_midpoint_on_segment = |report: &InspectionReport| {
            report.incidences.iter().any(|incidence| {
                matches!(
                    incidence,
                    Incidence::PointOnObject { point, object } if point == "M" && object == "AB"
                )
            })
        };
        assert!(has_midpoint_on_segment(&inside));
        assert!(!has_midpoint_on_segment(&outside));
    }

    #[test]
    fn measurement_tables_are_source_ordered_and_complete() {
        let scene = incidence_fixture(0.0);
        let measurements = inspect(&empty_file(), &scene, None).measurements;
        assert_eq!(measurements.distances[0].first, "A");
        assert_eq!(measurements.distances[0].second, "M");
        assert_eq!(measurements.triangle_angles.len(), 0);
        assert_eq!(
            measurements.line_ratios[0].integer_ratio,
            Some(IntegerRatio { left: 1, right: 1 })
        );
    }

    #[test]
    fn label_collision_variant_is_serializable_before_layout_exists() {
        let diagnostic = InspectDiagnostic::LabelCollision {
            labels: vec!["a".to_owned(), "b".to_owned()],
            spans: vec![Span::default(), Span::default()],
        };
        let json = serde_json::to_string(&diagnostic).expect("serialize diagnostic");
        assert!(json.contains("label_collision"));
    }

    #[test]
    fn out_of_view_uses_visible_geometry_bounds() {
        let mut scene = incidence_fixture(0.0);
        scene.objects.insert(
            ident("hidden", 6),
            ResolvedObject::Point(ResolvedPoint {
                geometry: Point::new(100.0, 100.0),
                mark: PointMark::None,
            }),
        );
        scene.bbox.include_point(Point::new(100.0, 100.0));
        let report = inspect(&empty_file(), &scene, None);
        assert!(report.diagnostics.iter().any(|diagnostic| matches!(
            diagnostic,
            InspectDiagnostic::OutOfView { id, .. } if id == "hidden"
        )));
    }

    fn empty_file() -> File {
        File {
            version: 2,
            statements: Vec::new(),
            span: Span::default(),
        }
    }
}
