//! Curved retained spans share primal and differentiated avoidance geometry.
#![allow(clippy::expect_used, reason = "fixed collision fixture geometry")]

use locus_core::geom::Bspline;
use locus_core::geom::{route::Route, sdf::object_separation};
use locus_core::{AnchorSpec, BBox, Point, ResolvedObject};
use locus_core::{PathGeometry, ResolvedPath};

fn arrow(route: Route) -> ResolvedObject {
    ResolvedObject::Path(ResolvedPath {
        geometry: PathGeometry::Routed {
            from: AnchorSpec::Point(route.from.clone().into()),
            to: AnchorSpec::Point(route.to.clone().into()),
            route: Some(route),
            bend: 0.0,
        },
        label_bounds: None,
        head: locus_syntax::HeadKind::None,
    })
}

fn box_at(x: f64, y: f64) -> ResolvedObject {
    let bounds = BBox {
        min: Point::new(x - 0.1, y - 0.1),
        max: Point::new(x + 0.1, y + 0.1),
    };
    ResolvedObject::Rect(
        locus_core::geom::construct::rect(
            bounds.center(),
            bounds.width(),
            bounds.height(),
            locus_core::Tolerance::from_bbox(&bounds),
        )
        .expect("valid box"),
    )
}

#[test]
fn quadratic_collision_uses_centerline_instead_of_its_bounds() {
    let curve = arrow(
        Route::direct(
            Point::new(-2.0, 0.0),
            Point::new(2.0, 0.0),
            core::f64::consts::FRAC_PI_4,
        )
        .expect("curve"),
    );
    assert!(object_separation(&curve, &box_at(0.0, 1.0), 0.02, None).expect("curve contact") < 0.0);
    assert!(
        object_separation(&curve, &box_at(0.0, 0.0), 0.02, None).expect("inside curve bounds")
            > 0.7
    );
}

#[test]
fn path_pair_crossing_is_symmetric_and_clear_paths_stay_clear() {
    let horizontal =
        arrow(Route::direct(Point::new(-2.0, 0.0), Point::new(2.0, 0.0), 0.0).expect("horizontal"));
    let crossing =
        arrow(Route::direct(Point::new(0.0, -2.0), Point::new(0.0, 2.0), 0.0).expect("vertical"));
    let first = object_separation(&horizontal, &crossing, 0.02, None).expect("crossing");
    let second = object_separation(&crossing, &horizontal, 0.02, None).expect("crossing reversed");
    assert!(first < 0.0);
    assert!((first - second).abs() < 1e-12);
    let separate =
        arrow(Route::direct(Point::new(-2.0, 1.0), Point::new(2.0, 1.0), 0.0).expect("separate"));
    assert!(
        (object_separation(&horizontal, &separate, 0.02, None).expect("clear") - 0.96).abs()
            < 1e-12
    );
}

#[test]
fn bspline_collision_uses_exact_bezier_spans_with_bounded_chord_error() {
    let spline = ResolvedObject::Bspline(
        Bspline::new(vec![
            Point::new(-2.0, 0.0),
            Point::new(-1.0, 2.0),
            Point::new(1.0, 2.0),
            Point::new(2.0, 0.0),
        ])
        .expect("spline"),
    );
    assert!(
        object_separation(&spline, &box_at(0.0, 1.5), 0.02, None).expect("spline intersection")
            < 0.0
    );
    assert!(
        object_separation(&spline, &box_at(0.0, 0.0), 0.02, None).expect("spline bounds interior")
            > 1.0
    );
}
