#![allow(clippy::expect_used, reason = "fixture failures need their context")]

use super::compile;

const TRIANGLE: &str = include_str!("../../../examples/demonstrations/point_relations.locus");

#[test]
fn renders_geometry_math_and_assertions_deterministically() {
    let output = compile(TRIANGLE).expect("triangle renders");
    assert_eq!(output, compile(TRIANGLE).expect("second render"));
    assert_eq!(output["ok"], true);
    assert_eq!(output["assertions"]["passed"], true);
    assert!(
        output["svg"]
            .as_str()
            .is_some_and(|svg| svg.contains("<path"))
    );
}

#[test]
fn returns_source_anchored_syntax_and_evaluation_errors() {
    for source in ["not locus", "locus 0 beta 6\npoint A = midpoint(B, C)\n"] {
        let errors = compile(source).expect_err("invalid program");
        assert!(!errors.is_empty());
        assert!(errors[0].code.starts_with('E'));
        assert!(errors[0].span.line > 0);
    }
}

#[test]
fn renders_even_when_assertions_fail() {
    let output = compile(&TRIANGLE.replace("assert area(tri) == 6", "assert area(tri) == 7"))
        .expect("assertions do not prevent rendering");
    assert_eq!(output["assertions"]["passed"], false);
    assert!(output["svg"].is_string());
}

#[test]
fn renders_plain_cjk_text_and_reports_invalid_tex() {
    let source = "locus 0 beta 6\npoint A()\nlabel title(A, \"中文 日本語 한국어\")\n";
    let output = compile(source).expect("bundled CJK fonts");
    assert!(
        output["svg"]
            .as_str()
            .is_some_and(|svg| svg.contains("<path"))
    );
    let invalid = source.replace("\"中文 日本語 한국어\"", "$\\notalocuscommand$");
    assert_eq!(
        compile(&invalid).expect_err("unsupported TeX")[0].code,
        "E414"
    );
}

#[test]
fn renders_expanded_diagram_with_routes_and_text() {
    let source = include_str!("../../../examples/demonstrations/flowchart_agent_loop.locus");
    let output = compile(source).expect("diagram renders");
    assert_eq!(output["assertions"]["passed"], true);
    assert!(output["svg"].is_string());
}

#[test]
fn retains_groups_when_evaluating_assertions() {
    let source = "locus 0 beta 6\npoint A()\npoint B()\ngroup pair(A, B)\nrel row(pair)\nassert align_center_y(A, B)\n";
    let output = compile(source).expect("groups survive assertion evaluation");
    assert_eq!(output["assertions"]["passed"], true);
}
