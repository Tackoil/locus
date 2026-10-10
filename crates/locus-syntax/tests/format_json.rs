//! Canonical formatting and interchange corpus/property tests.

use locus_syntax::{format, parse, to_json_pretty};
use proptest::prelude::*;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

fn corpus_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../spec/test")
}

fn fixture(name: &str) -> Result<String, std::io::Error> {
    fs::read_to_string(
        corpus_dir()
            .join("demonstrations")
            .join(format!("{name}.locus")),
    )
}

fn parsed(source: &str) -> locus_syntax::File {
    let output = parse(source);
    assert!(output.diagnostics.is_empty(), "{:#?}", output.diagnostics);
    output.file
}

fn spanless_ast(file: &locus_syntax::File) -> Result<Value, serde_json::Error> {
    let mut value = serde_json::to_value(file)?;
    strip_spans(&mut value);
    Ok(value)
}

fn strip_spans(value: &mut Value) {
    if let Value::Object(fields) = value {
        fields.remove("span");
        for child in fields.values_mut() {
            strip_spans(child);
        }
    } else if let Value::Array(items) = value {
        for item in items {
            strip_spans(item);
        }
    }
}

fn assert_fixture_snapshot(name: &str) -> Result<(), Box<dyn std::error::Error>> {
    let source = fixture(name)?;
    let formatted = format(&source);
    assert!(
        formatted.diagnostics.is_empty(),
        "{:#?}",
        formatted.diagnostics
    );
    insta::assert_snapshot!(format!("{name}_formatted"), formatted.source);
    let json = to_json_pretty(&parsed(&source))?;
    insta::assert_snapshot!(format!("{name}_json"), json);
    Ok(())
}

fn assert_fixture_idempotent(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let source = fs::read_to_string(path)?;
    let once = format(&source);
    assert!(
        once.diagnostics.is_empty(),
        "{}: {:#?}",
        path.display(),
        once.diagnostics
    );
    let twice = format(&once.source);
    assert!(twice.diagnostics.is_empty(), "{:#?}", twice.diagnostics);
    assert_eq!(once.source, twice.source, "{}", path.display());
    Ok(())
}

#[test]
fn formatter_is_idempotent_on_every_spec_fixture() -> Result<(), Box<dyn std::error::Error>> {
    let mut paths = Vec::new();
    for group in ["demonstrations", "stress"] {
        let entries = fs::read_dir(corpus_dir().join(group))?;
        paths.extend(
            entries
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .map(|entry| entry.path())
                .filter(|path| {
                    path.extension()
                        .is_some_and(|extension| extension == "locus")
                }),
        );
    }
    paths.sort();
    for path in paths {
        assert_fixture_idempotent(&path)?;
    }
    Ok(())
}

#[test]
fn snapshots_angle_bisector_ratio() -> Result<(), Box<dyn std::error::Error>> {
    assert_fixture_snapshot("angle_bisector_ratio")
}

#[test]
fn snapshots_inscribed_angle() -> Result<(), Box<dyn std::error::Error>> {
    assert_fixture_snapshot("inscribed_angle")
}

#[test]
fn snapshots_pentagon_golden() -> Result<(), Box<dyn std::error::Error>> {
    assert_fixture_snapshot("pentagon_golden")
}

#[derive(Clone, Debug)]
enum GeneratedStatement {
    Num(String),
    Point(String, String),
    Segment,
    Circle(String),
    Label(String),
    Arrow(i16),
    Assert(String),
    Rel(String),
}

impl GeneratedStatement {
    fn source(&self, index: usize, padding: &str) -> String {
        if let Self::Num(expr) = self {
            return format!("num n{index}{padding}={padding}{expr}");
        }
        if let Self::Point(x, y) = self {
            return format!("point P{index}{padding}={padding}offset(A, {x}, {y})");
        }
        if matches!(self, Self::Segment) {
            return format!("segment s{index}{padding}={padding}seg(A, B)");
        }
        if let Self::Circle(radius) = self {
            return format!("circle c{index}{padding}={padding}circle(A, {radius}) [ink]");
        }
        if let Self::Label(text) = self {
            return format!(
                "label l{index}{padding}={padding}label(A, {text}) {{ dir: NE, gap: r / 10 }}"
            );
        }
        if let Self::Arrow(angle) = self {
            return format!(
                "arrow a{index}{padding}={padding}arrow(A, B) {{ head: <->, bend: {angle}deg }}"
            );
        }
        if let Self::Assert(expr) = self {
            return format!("assert {expr}");
        }
        let Self::Rel(expr) = self else {
            unreachable!("all generated statement variants handled")
        };
        format!("rel {expr}")
    }
}

fn numeric_expr() -> BoxedStrategy<String> {
    let leaf = prop_oneof![
        (0_u16..100).prop_map(|value| value.to_string()),
        Just("r".to_owned()),
        Just("dist(A, B)".to_owned()),
        Just("hdist(A, B)".to_owned()),
        Just("box.width".to_owned()),
    ];
    leaf.prop_recursive(4, 96, 3, |inner| {
        prop_oneof![
            inner.clone().prop_map(|expr| format!("-{expr}")),
            inner.clone().prop_map(|expr| format!("({expr})")),
            inner.clone().prop_map(|expr| format!("abs({expr})")),
            inner.clone().prop_map(|expr| format!("sqrt({expr})")),
            (inner.clone(), inner.clone()).prop_map(|(lhs, rhs)| format!("min({lhs}, {rhs})")),
            (inner.clone(), inner.clone()).prop_map(|(lhs, rhs)| format!("ratio({lhs}, {rhs})")),
            (
                inner.clone(),
                prop::sample::select(vec!["+", "-", "*", "/"]),
                inner,
            )
                .prop_map(|(lhs, op, rhs)| format!("{lhs} {op} {rhs}")),
        ]
    })
    .boxed()
}

fn boolean_expr() -> BoxedStrategy<String> {
    let leaf = prop_oneof![
        (numeric_expr(), numeric_expr()).prop_map(|(lhs, rhs)| format!("{lhs} == {rhs}")),
        Just("horizontal(A, B)".to_owned()),
        Just("on(A, base_circle)".to_owned()),
        Just("parallel(seg(A, B), seg(A, B))".to_owned()),
    ];
    leaf.prop_recursive(3, 64, 2, |inner| {
        prop_oneof![
            inner.clone().prop_map(|expr| format!("not ({expr})")),
            (inner.clone(), inner.clone()).prop_map(|(lhs, rhs)| format!("({lhs}) and ({rhs})")),
            (inner.clone(), inner).prop_map(|(lhs, rhs)| format!("({lhs}) or ({rhs})")),
        ]
    })
    .boxed()
}

fn rel_expr() -> BoxedStrategy<String> {
    prop_oneof![
        (numeric_expr(), numeric_expr()).prop_map(|(lhs, rhs)| format!("{lhs} == {rhs}")),
        Just("horizontal(A, B)".to_owned()),
        Just("on(A, base_circle)".to_owned()),
    ]
    .boxed()
}

fn text_value() -> BoxedStrategy<String> {
    prop::sample::select(vec![
        "\"plain text\"".to_owned(),
        "$\\alpha$".to_owned(),
        "\"mixed $x$ text\"".to_owned(),
        "\"https://x/$a$\"".to_owned(),
        "\"line\\nnext\"".to_owned(),
    ])
    .boxed()
}

fn generated_statement() -> BoxedStrategy<GeneratedStatement> {
    prop_oneof![
        numeric_expr().prop_map(GeneratedStatement::Num),
        (numeric_expr(), numeric_expr()).prop_map(|(x, y)| GeneratedStatement::Point(x, y)),
        Just(GeneratedStatement::Segment),
        numeric_expr().prop_map(GeneratedStatement::Circle),
        text_value().prop_map(GeneratedStatement::Label),
        (-170_i16..170).prop_map(GeneratedStatement::Arrow),
        boolean_expr().prop_map(GeneratedStatement::Assert),
        rel_expr().prop_map(GeneratedStatement::Rel),
    ]
    .boxed()
}

prop_compose! {
    fn valid_file()
        (bx in 1_i16..40, radius in 1_u16..20, padding in 0_usize..4, comment in any::<bool>(),
         meta_after_style in any::<bool>(), statements in prop::collection::vec(generated_statement(), 0..12))
        -> String
    {
        let pad = " ".repeat(padding);
        let trailing = if comment { "  // generated hint" } else { "" };
        let meta = "meta title = \"Generated $c$ file\"\n";
        let prelude = if meta_after_style {
            format!("style textbook\n{meta}")
        } else {
            format!("{meta}style textbook\n")
        };
        let separator = if comment { "\n// generated statement\n" } else { "\n" };
        let generated = statements
            .iter()
            .enumerate()
            .map(|(index, statement)| statement.source(index, &pad))
            .collect::<Vec<_>>()
            .join(separator);
        format!(
            "locus 0 beta 5\n\n{prelude}class ink {{ accent: #123ABC, weight: 1 }}\n\n\
             point A{pad}={pad}point(){trailing}\n\
             point B{pad}={pad}offset(A, {bx}, 0) where {{ horizontal(A, B) }}\n\
             num r{pad}={pad}{radius}\n\
             rect box{pad}={pad}rect(A, r, r)\n\
             circle base_circle{pad}={pad}circle(A, r) [ink]\n\n\
             {generated}\n"
        )
    }
}

proptest! {
    #[test]
    fn parse_format_parse_round_trip(source in valid_file()) {
        let before = parse(&source);
        prop_assert!(before.diagnostics.is_empty(), "{:#?}", before.diagnostics);
        let formatted = format(&source);
        prop_assert!(formatted.diagnostics.is_empty(), "{:#?}", formatted.diagnostics);
        let after = parse(&formatted.source);
        prop_assert!(after.diagnostics.is_empty(), "{:#?}", after.diagnostics);
        let before_ast = spanless_ast(&before.file)
            .map_err(|error| TestCaseError::fail(error.to_string()))?;
        let after_ast = spanless_ast(&after.file)
            .map_err(|error| TestCaseError::fail(error.to_string()))?;
        prop_assert_eq!(after_ast, before_ast, "{}", formatted.source);
    }

    #[test]
    fn generated_formatter_is_idempotent(source in valid_file()) {
        let once = format(&source);
        prop_assert!(once.diagnostics.is_empty(), "{:#?}", once.diagnostics);
        let twice = format(&once.source);
        prop_assert!(twice.diagnostics.is_empty(), "{:#?}", twice.diagnostics);
        prop_assert_eq!(once.source, twice.source);
    }
}
