//! Arrow bend domains apply equally to literals, expressions, and solver scenes.
#![allow(clippy::expect_used)]

use locus_core::{Point, evaluate, geom::arrow};

fn source(bend: &str) -> String {
    format!(
        "locus 0 beta 5\npoint A = point()\npoint B = offset(A, 4, 0)\n\
         arrow link = arrow(A, B) {{ bend: {bend} }}\n"
    )
}

#[test]
fn singular_and_near_singular_bends_report_the_property_and_repair() {
    for bend in [
        "90deg",
        "-90deg",
        "270deg",
        "-270deg",
        "89.99deg",
        "90.01deg",
        "-89.99deg",
        "269.99deg",
        "450deg",
        "(45deg + 45deg)",
    ] {
        let parsed = locus_syntax::parse(&source(bend));
        assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
        let error = evaluate(&parsed.file).expect_err("unsafe bend must fail checking");
        assert_eq!(error.code, "E210", "{bend}: {error}");
        assert_eq!(error.span.line, 4);
        assert_eq!(error.span.col, 34);
        assert_eq!(error.object_id.as_deref(), Some("link"));
        assert!(error.message.contains("45deg"));
        assert!(!error.message.contains("hints"));
        assert_eq!(error.measured[0].unit, Some("deg"));
    }
}

#[test]
fn ordinary_bends_and_both_sides_of_the_domain_boundary_are_supported() {
    for bend in ["0deg", "45deg", "-45deg", "89.9deg", "90.1deg", "180deg"] {
        let parsed = locus_syntax::parse(&source(bend));
        assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
        evaluate(&parsed.file).expect("bounded bend");
    }
    let boundary = libm::atan(1000.0);
    for sign in [-1.0, 1.0] {
        assert!(arrow::bend_tangent(sign * (boundary - 1e-8)).is_ok());
        assert!(arrow::bend_tangent(sign * (boundary + 1e-8)).is_err());
    }
    for bend in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert_eq!(
            arrow::bend_tangent(bend).expect_err("non-finite").code(),
            "E202"
        );
    }
}

#[test]
fn named_bends_and_deferred_annotation_anchors_are_checked_before_layout() {
    let source = "locus 0 beta 5\npoint A = point()\nnum theta = 90deg\n\
                  label name = label(A, \"A\")\n\
                  arrow link = arrow(A, name.center) { bend: theta }\n";
    let parsed = locus_syntax::parse(source);
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    let error = evaluate(&parsed.file).expect_err("named bend must fail");
    assert_eq!(error.code, "E210");
    assert_eq!(error.span.line, 5);
}

#[test]
fn control_bounds_limits_are_independent_of_scene_scale() {
    for scale in [1e-200, 1.0, 1e200] {
        let from = Point::new(0.0, 0.0);
        let to = Point::new(4.0 * scale, 0.0);
        let control = arrow::control(from, to, core::f64::consts::FRAC_PI_4)
            .expect("finite arrow at any scale")
            .expect("curved arrow");
        arrow::validate_control(from, to, control).expect("bounded control");
        let excessive = Point::new(2.0 * scale, 1e6 * scale);
        assert_eq!(
            arrow::validate_control(from, to, excessive)
                .expect_err("excessive relative bounds")
                .code(),
            "E204"
        );
    }
}
