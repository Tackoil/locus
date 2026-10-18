//! Reject hostile nesting before recursive parsing, expansion, or AST drops exhaust the stack.
#![allow(
    clippy::expect_used,
    reason = "test thread failures must fail the regression test"
)]

use locus_syntax::{Statement, parse};
use std::fmt::Write;

fn rejects_depth(source: &str) {
    let parsed = parse(source);
    assert!(
        parsed
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "E161"),
        "expected a depth diagnostic: {:?}",
        parsed.diagnostics
    );
}

#[test]
fn hostile_expression_shapes_are_rejected_on_a_small_stack_and_recover() {
    std::thread::Builder::new()
        .stack_size(512 * 1024)
        .spawn(|| {
            for expression in [
                format!("{}1{}", "(".repeat(5000), ")".repeat(5000)),
                format!("{}1", "-".repeat(5000)),
                format!("{}true", "not ".repeat(5000)),
                format!("1{}", " + 1".repeat(5000)),
                format!("P{}", ".center".repeat(5000)),
                format!("{}1{}", "sqrt(".repeat(5000), ")".repeat(5000)),
            ] {
                let source =
                    format!("locus 0 beta 6\nnum value = free {expression}\npoint recovered()\n");
                rejects_depth(&source);
                let parsed = parse(&source);
                assert!(parsed.file.statements.iter().any(|statement| {
                    matches!(statement, Statement::Op(op) if op.id.name == "recovered")
                }));
            }
        })
        .expect("start bounded-stack parser test")
        .join()
        .expect("parser must reject deep input without aborting");
}

#[test]
fn expression_depth_boundary_keeps_normal_grouping_and_wide_calls() {
    let grouped = format!("{}1{}", "(".repeat(63), ")".repeat(63));
    let flat = format!("1{}", " + 1".repeat(63));
    let wide = format!("min({})", vec!["1"; 5000].join(", "));
    for expression in [grouped, flat, wide] {
        let parsed = parse(&format!("locus 0 beta 6\nnum value = free {expression}\n"));
        assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    }
    rejects_depth(&format!(
        "locus 0 beta 6\nnum value = free {}1{}\n",
        "(".repeat(64),
        ")".repeat(64)
    ));
    rejects_depth(&format!(
        "locus 0 beta 6\nnum value = free 1{}\n",
        " + 1".repeat(64)
    ));
}

#[test]
fn invalid_nested_function_definitions_do_not_exhaust_the_stack() {
    let source = format!(
        "locus 0 beta 6\n{}{}",
        "fn nested() {\n".repeat(5000),
        "}\n".repeat(5000)
    );
    rejects_depth(&source);
}

#[test]
fn named_function_expansion_and_substituted_arguments_are_bounded() -> Result<(), std::fmt::Error> {
    let mut source = "locus 0 beta 6\nfn f0() {\npoint P()\n}\n".to_owned();
    for index in 1..70 {
        writeln!(source, "fn f{index}() {{\nf{} child()\n}}", index - 1)?;
    }
    source.push_str("f69 instance()\n");
    rejects_depth(&source);

    let mut source = "locus 0 beta 6\nfn f0(value) {\nassert value == 1\n}\n".to_owned();
    for index in 1..12 {
        writeln!(
            source,
            "fn f{index}(value) {{\nf{} child((((((((value))))))))\n}}",
            index - 1
        )?;
    }
    source.push_str("f11 instance(1)\n");
    rejects_depth(&source);
    Ok(())
}
