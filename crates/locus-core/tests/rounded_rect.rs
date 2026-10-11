//! Rounded rectangles retain exact geometry through evaluation and solving.
use locus_core::{Point, RoundedRect, evaluate, evaluate_assertions, solve};
use std::error::Error;

fn checked(source: &str) -> Result<locus_core::Solved, Box<dyn Error>> {
    let parsed = locus_syntax::parse(source);
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    let solved = solve(&parsed.file)?;
    let report = evaluate_assertions(&parsed.file, &solved.scene)?;
    assert!(report.passed, "{report:?}");
    Ok(solved)
}

#[test]
fn placed_derived_alias_and_inline_forms_share_anchors_and_measurements()
-> Result<(), Box<dyn Error>> {
    let solved = checked(concat!(
        "locus 0 beta 5\n",
        "rounded_rect box = rounded_rect(6, 4, 1)\n",
        "rounded_rect copy = rounded_rect(box.center, 6, 4, 1)\n",
        "rounded_rect alias = copy\n",
        "point corner = offset(box.center, 2 + sqrt(0.5), 1 + sqrt(0.5))\n",
        "assert coincident(box.center, box.centroid)\n",
        "assert coincident(alias.ne, box.ne)\n",
        "assert hdist(box.w, box.e) == 6\n",
        "assert vdist(box.s, box.n) == 4\n",
        "assert box.width == 6\n",
        "assert box.height == 4\n",
        "assert box.corner_radius == 1\n",
        "assert rounded_rect(box.center, 6, 4, 1).corner_radius == 1\n",
        "assert area(box) == 23.141592653589793\n",
        "assert perimeter(copy) == 18.283185307179586\n",
        "assert inside(box.center, box)\n",
        "assert not inside(box.n, box)\n",
        "assert not inside(box.ne, box)\n",
        "assert not on(box.ne, box)\n",
        "assert on(box.n, box)\n",
        "assert on(corner, box)\n",
    ))?;
    assert_eq!(solved.summary.unknowns, 2);
    assert_eq!(solved.summary.equations, 0);
    Ok(())
}

#[test]
fn relations_solve_placement_and_radius_from_area() -> Result<(), Box<dyn Error>> {
    let solved = checked(concat!(
        "locus 0 beta 5\n",
        "point O = point()\n",
        "num r = free 0.8\n",
        "rounded_rect box = rounded_rect(6, 4, r)\n",
        "rel coincident(box.center, offset(O, 5, 2))\n",
        "rel area(box) == 23.141592653589793\n",
        "assert r == 1\n",
        "assert box.corner_radius == 1\n",
        "assert hdist(O, box.center) == 5\n",
        "assert vdist(O, box.center) == 2\n",
    ))?;
    assert_eq!(solved.summary.unknowns, 5);
    assert_eq!(solved.summary.rank, 3);
    Ok(())
}

#[test]
fn relations_measure_derived_shapes_and_follow_the_curved_boundary() -> Result<(), Box<dyn Error>> {
    checked(concat!(
        "locus 0 beta 5\n",
        "point O = point()\n",
        "num r = free 0.8\n",
        "rounded_rect box = rounded_rect(O, 6, 4, r)\n",
        "rel perimeter(rounded_rect(O, 6, 4, r)) == 18.283185307179586\n",
        "rel box.width == 6\n",
        "rel box.corner_radius == 1\n",
        "rel coincident(box.centroid, O)\n",
        "num y = free 1.5\n",
        "point P = offset(O, 2.7071067811865475, y)\n",
        "rel on(P, box)\n",
        "assert on(P, box)\n",
        "assert abs(vdist(O, P)) == 1.7071067811865475\n",
    ))?;
    Ok(())
}

#[test]
fn zero_radius_capsules_and_circle_limit_are_valid() -> Result<(), Box<dyn Error>> {
    checked(concat!(
        "locus 0 beta 5\n",
        "rounded_rect square = rounded_rect(6, 4, 0)\n",
        "rounded_rect capsule = rounded_rect(square.center, 6, 4, 2)\n",
        "rounded_rect circle = rounded_rect(square.center, 4, 4, 2)\n",
        "assert area(square) == 24\n",
        "assert perimeter(square) == 20\n",
        "assert on(square.ne, square)\n",
        "assert area(capsule) == 20.566370614359172\n",
        "assert perimeter(capsule) == 16.566370614359172\n",
        "assert area(circle) == 12.566370614359172\n",
        "assert perimeter(circle) == 12.566370614359172\n",
    ))?;
    Ok(())
}

#[test]
fn invalid_dimensions_and_radius_report_domain_errors() {
    for dimensions in ["0, 4, 1", "6, -4, 1", "6, 4, -1", "6, 4, 2.1"] {
        let parsed = locus_syntax::parse(&format!(
            "locus 0 beta 5\nrounded_rect box = rounded_rect({dimensions})\n"
        ));
        assert!(parsed.diagnostics.is_empty());
        let result = evaluate(&parsed.file);
        assert!(
            matches!(&result, Err(error) if error.code == "E404" && error.span.line == 2 && error.message.contains("rounded_rect domain error")),
            "{dimensions}: {result:?}"
        );
    }
}

#[test]
fn geometry_rejects_nonfinite_input_and_uses_true_corner_distance() -> Result<(), Box<dyn Error>> {
    let center = Point::new(0.0, 0.0);
    for radius in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(
            matches!(RoundedRect::new(center, 6.0, 4.0, radius), Err(error) if error.code() == "E202")
        );
    }
    let rect = RoundedRect::new(center, 6.0, 4.0, 1.0)?;
    assert!((rect.signed_distance(Point::new(3.0, 2.0)) - (libm::sqrt(2.0) - 1.0)).abs() < 1e-12);
    assert!(rect.signed_distance(center) < 0.0);
    assert!(rect.signed_distance(Point::new(0.0, 2.0)).abs() < 1e-12);
    for arc in rect.corner_arcs() {
        assert!(rect.signed_distance(arc.start).abs() < 1e-12);
        assert!(rect.signed_distance(arc.end).abs() < 1e-12);
    }
    Ok(())
}
