//! T6 assertion tolerance regressions.
#![allow(clippy::expect_used)]

use locus_core::{BBox, Point, ResolvedObject, ResolvedPoint, ResolvedScene, evaluate_assertions};
use locus_syntax::{Statement, parse};

#[test]
fn zero_bbox_uses_positive_point_hint() {
    let parsed = parse(
        "locus 2\npoint A = free ~(100, 0)\nrel pin(A, 0, 0)\nassert pin(A, 0.00000005, 0)\n",
    );
    assert!(parsed.diagnostics.is_empty());
    let op = parsed
        .file
        .statements
        .first()
        .and_then(|statement| match statement {
            Statement::Op(op) => Some(op),
            _ => None,
        })
        .expect("point declaration");
    let origin = Point::new(0.0, 0.0);
    let scene = ResolvedScene {
        objects: [(
            op.id.clone(),
            ResolvedObject::Point(ResolvedPoint::new(origin)),
        )]
        .into_iter()
        .collect(),
        bbox: BBox::from_points([origin]).expect("bbox"),
    };
    assert!(
        evaluate_assertions(&parsed.file, &scene)
            .expect("valid assertion")
            .passed
    );
}

#[test]
fn zero_bbox_uses_nested_positive_length_literals() {
    let parsed = parse(
        "locus 2\npoint A = free ~(0, 0)\npoint B = offset(A, 100 - 100, 0)\nassert pin(A, 0.00000005, 0)\n",
    );
    assert!(parsed.diagnostics.is_empty());
    let origin = Point::new(0.0, 0.0);
    let ids = parsed
        .file
        .statements
        .iter()
        .filter_map(|statement| match statement {
            Statement::Op(op) => Some(op.id.clone()),
            _ => None,
        });
    let objects = ids
        .map(|id| (id, ResolvedObject::Point(ResolvedPoint::new(origin))))
        .collect();
    let scene = ResolvedScene {
        objects,
        bbox: BBox::from_points([origin]).expect("bbox"),
    };
    assert!(
        evaluate_assertions(&parsed.file, &scene)
            .expect("valid assertion")
            .passed
    );
}

#[test]
fn angle_tolerance_is_always_in_radians() {
    let parsed = parse("locus 2\nassert 0rad == 3rad tol 2\n");
    assert!(parsed.diagnostics.is_empty());
    let scene = ResolvedScene {
        objects: indexmap::IndexMap::new(),
        bbox: BBox::from_points([Point::new(0.0, 0.0)]).expect("bbox"),
    };
    assert!(
        !evaluate_assertions(&parsed.file, &scene)
            .expect("valid assertion")
            .passed
    );
}
