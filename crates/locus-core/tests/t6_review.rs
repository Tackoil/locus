//! Regressions from the independent T6 review.
#![allow(clippy::expect_used)]

use locus_core::{
    ActualValue, BBox, Circle, Incidence, Point, ReportDimension, ResolvedObject, ResolvedPoint,
    ResolvedScene, evaluate_assertions, inspect,
};
use locus_syntax::{File, Statement, parse};

fn fixture(assertion: &str) -> (File, ResolvedScene) {
    let source = format!(
        "locus 0 beta 5\npoint A = point()\npoint B = point()\ncircle c = circle(A, 1)\ncircle d = circle(A, 1)\nassert {assertion}\n"
    );
    let parsed = parse(&source);
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    let values = [
        ResolvedObject::Point(ResolvedPoint::new(Point::new(0.0, 0.0))),
        ResolvedObject::Point(ResolvedPoint::new(Point::new(4.0, 0.0))),
        ResolvedObject::Circle(Circle::new(Point::new(0.0, 0.0), 1.0).expect("circle")),
        ResolvedObject::Circle(Circle::new(Point::new(0.0, 0.0), 1.0).expect("circle")),
    ];
    let ids = parsed
        .file
        .statements
        .iter()
        .filter_map(|statement| match statement {
            Statement::Op(op) => Some(op.id.clone()),
            _ => None,
        });
    let objects = ids.zip(values).collect();
    let scene = ResolvedScene {
        objects,
        bbox: BBox::from_points([Point::new(-1.0, -1.0), Point::new(4.0, 1.0)]).expect("bbox"),
    };
    (parsed.file, scene)
}

#[test]
fn failed_assert_with_midpoint_constructor_returns_report() {
    let (file, scene) = fixture("dist(A, midpoint(A, B)) == 3");
    let report =
        evaluate_assertions(&file, &scene).expect("valid false assertion must return a report");
    assert!(!report.passed);
}

#[test]
fn identical_circles_are_not_tangent() {
    let (file, scene) = fixture("tangent(c, d)");
    assert!(!evaluate_assertions(&file, &scene).expect("report").passed);
    assert!(!inspect(&file, &scene, None).incidences.iter().any(|incidence| matches!(incidence, Incidence::Tangent { first, second } if first == "c" && second == "d")));
}

#[test]
fn near_equal_circles_can_be_internally_tangent() {
    let (file, mut scene) = fixture("internally_tangent(d, c)");
    let radius_delta = 1.0e-13;
    *scene.objects.get_index_mut(2).expect("first circle").1 =
        ResolvedObject::Circle(Circle::new(Point::new(0.0, 0.0), 1.0).expect("first circle"));
    *scene.objects.get_index_mut(3).expect("second circle").1 = ResolvedObject::Circle(
        Circle::new(Point::new(radius_delta, 0.0), 1.0 + radius_delta).expect("second circle"),
    );
    assert!(evaluate_assertions(&file, &scene).expect("report").passed);
    assert!(inspect(&file, &scene, None).incidences.iter().any(|incidence| matches!(incidence, Incidence::Tangent { first, second } if first == "c" && second == "d")));
}

#[test]
fn near_concentric_equal_radius_circles_are_not_tangent() {
    let (file, mut scene) = fixture("tangent(c, d)");
    *scene.objects.get_index_mut(3).expect("second circle").1 =
        ResolvedObject::Circle(Circle::new(Point::new(1.0e-10, 0.0), 1.0).expect("second circle"));
    assert!(!evaluate_assertions(&file, &scene).expect("report").passed);
    assert!(!inspect(&file, &scene, None).incidences.iter().any(|incidence| matches!(incidence, Incidence::Tangent { first, second } if first == "c" && second == "d")));
}

#[test]
fn equal_radius_circles_can_be_externally_tangent() {
    let (file, mut scene) = fixture("tangent(c, d)");
    *scene.objects.get_index_mut(3).expect("second circle").1 =
        ResolvedObject::Circle(Circle::new(Point::new(2.0, 0.0), 1.0).expect("second circle"));
    assert!(evaluate_assertions(&file, &scene).expect("report").passed);
    assert!(inspect(&file, &scene, None).incidences.iter().any(|incidence| matches!(incidence, Incidence::Tangent { first, second } if first == "c" && second == "d")));
}

#[test]
fn constructor_subterms_keep_contextual_dimensions() {
    let (file, scene) = fixture("coincident(offset(A, sqrt(4), 0), B)");
    let report = evaluate_assertions(&file, &scene).expect("report");
    assert!(!report.passed);
    assert!(report.assertions[0].actuals.iter().any(|term| matches!(term.actual, ActualValue::Number { value, dimension: ReportDimension::Area } if (value - 4.0).abs() < f64::EPSILON)));
    assert!(report.assertions[0].actuals.iter().any(|term| matches!(term.actual, ActualValue::Number { value, dimension: ReportDimension::Length } if (value - 2.0).abs() < f64::EPSILON)));
}

#[test]
fn contextual_product_under_sqrt_returns_a_failure_report() {
    let (file, scene) = fixture("coincident(offset(A, sqrt(dist(A, B) * 2), 0), B)");
    let report = evaluate_assertions(&file, &scene).expect("valid false assertion must report");
    assert!(!report.passed);
    assert!(report.assertions[0].actuals.iter().any(|term| matches!(term.actual, ActualValue::Number { value, dimension: ReportDimension::Area } if (value - 8.0).abs() < f64::EPSILON)));
    assert!(report.assertions[0].actuals.iter().any(|term| matches!(term.actual, ActualValue::Number { value, dimension: ReportDimension::Length } if (value - libm::sqrt(8.0)).abs() < f64::EPSILON)));
}
