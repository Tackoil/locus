//! Textbook classes, exact overrides, and validation spans.
#![allow(clippy::expect_used, clippy::float_cmp)]
use locus_render::{
    RenderError,
    style::{Color, StyleSheet},
};
use locus_syntax::{Expr, PropValue, Statement};

#[test]
fn textbook_semantics_and_exact_overrides() {
    let file = locus_syntax::parse(
        "locus 2\nclass exact { stroke: dash(6, 3, 2), weight: 1.5, accent: #2563EB, fill: #CCFBF1, opacity: 0.85, cap: square, join: bevel }\nclass colored { fill: tint, accent: red }\nclass minimal { accent: teal }\npoint a = free ~(0, 0) [exact]\n",
    );
    assert!(file.diagnostics.is_empty(), "{:?}", file.diagnostics);
    let sheet = StyleSheet::from_file(&file.file).expect("styles");
    let main = sheet.class("main").expect("main");
    assert!(main.dash.is_empty());
    assert_eq!(main.fill, None);
    assert_eq!(
        sheet.class("auxiliary").expect("auxiliary").dash,
        [5.0, 3.0]
    );
    assert_eq!(
        sheet.class("construction").expect("construction").accent,
        Color(107, 114, 128)
    );
    assert_eq!(
        sheet.class("highlight").expect("highlight").accent,
        Color(37, 99, 235)
    );
    let exact = sheet.for_object("a");
    assert_eq!(exact.weight, 1.5);
    assert_eq!(exact.dash, [6.0, 3.0, 2.0, 6.0, 3.0, 2.0]);
    assert_eq!(exact.accent.hex(), "#2563EB");
    assert_eq!(exact.fill, Some(Color(204, 251, 241)));
    assert_eq!(exact.opacity, 0.85);
    assert_eq!(exact.cap, "square");
    assert_eq!(exact.join, "bevel");
    let minimal = sheet.class("minimal").expect("minimal");
    assert_eq!(minimal.weight, main.weight);
    assert_eq!(minimal.dash, main.dash);
    assert_eq!(
        sheet.class("colored").expect("tint").fill,
        Some(Color(251, 233, 233))
    );
    assert_eq!(sheet.for_object("unassigned"), *main);
}

#[test]
fn invalid_exact_values_keep_property_spans() {
    for (property, invalid) in [
        ("weight", 0.0),
        ("weight", f64::INFINITY),
        ("opacity", 1.1),
        ("opacity", -0.1),
        ("opacity", f64::NAN),
    ] {
        let mut file =
            locus_syntax::parse(&format!("locus 2\nclass exact {{ {property}: 1 }}\n")).file;
        let Statement::Class(class) = &mut file.statements[0] else {
            unreachable!()
        };
        let prop = &mut class.props.items[0];
        let PropValue::Expr(Expr::Number(number)) = &mut prop.value else {
            unreachable!()
        };
        let span = number.span;
        number.value = invalid;
        assert!(
            matches!(StyleSheet::from_file(&file),Err(RenderError::Invalid { span: actual,.. }) if actual==span)
        );
    }
}

#[test]
fn invalid_dash_length_keeps_argument_span() {
    let mut file = locus_syntax::parse("locus 2\nclass exact { stroke: dash(6, 3) }\n").file;
    let Statement::Class(class) = &mut file.statements[0] else {
        unreachable!()
    };
    let PropValue::Expr(Expr::Call { args, .. }) = &mut class.props.items[0].value else {
        unreachable!()
    };
    let Expr::Number(number) = &mut args[1] else {
        unreachable!()
    };
    let span = number.span;
    number.value = 0.0;
    assert!(
        matches!(StyleSheet::from_file(&file),Err(RenderError::Invalid { span: actual,.. }) if actual==span)
    );
}
