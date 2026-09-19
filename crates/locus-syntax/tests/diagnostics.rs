//! Stable frontend diagnostic snapshots.

use locus_syntax::parse;

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "the inline snapshot is intentionally explicit"
)]
fn snapshots_stable_e1xx_diagnostics() {
    let source = r"locus 3
piont A = free ~(0, 0)
point A = free ~0
line bad = free ~(0, 0)
point A = free ~(0, 0)
circle c = incircel(A, 2)
point P = rotate(A, A, 60)
point Q = midpoint(A, Later) [hilight] { mrak: dot, mark: dot, mark: cross }
rel inside(P, c)
assert dist(A, P)
assert dist(A, P) == 1 tol 1deg
point Later = midpoint(A, P) extra
meta {
  title $not-a-string$
}
";
    let output = parse(source);
    let summary: Vec<_> = output
        .diagnostics
        .iter()
        .map(|diagnostic| {
            (
                diagnostic.code,
                diagnostic.span.line,
                diagnostic.message.as_str(),
                diagnostic.suggestion.as_deref(),
            )
        })
        .collect();
    insta::assert_debug_snapshot!(summary, @r###"
    [
        (
            "E120",
            2,
            "unknown statement or object kind `piont`",
            None,
        ),
        (
            "E124",
            3,
            "expected `(` starting point hint",
            None,
        ),
        (
            "E123",
            4,
            "only `point` and `num` declarations may use `free`",
            None,
        ),
        (
            "E123",
            5,
            "format 3 points use parameterless `point()`, not coordinate hints",
            Some(
                "point A = point()",
            ),
        ),
        (
            "E140",
            6,
            "unknown constructor `incircel`",
            Some(
                "incircle",
            ),
        ),
        (
            "E143",
            7,
            "angle literals require an explicit `deg` or `rad` unit",
            Some(
                "add `deg` or `rad`",
            ),
        ),
        (
            "E130",
            8,
            "unknown property `mrak`",
            None,
        ),
        (
            "E132",
            8,
            "duplicate property `mark`",
            None,
        ),
        (
            "E135",
            8,
            "unknown or forward reference `Later`",
            None,
        ),
        (
            "E133",
            8,
            "unknown or forward style class `hilight`",
            Some(
                "highlight",
            ),
        ),
        (
            "E146",
            9,
            "this assert-only boolean form cannot be used in `rel` or `where`",
            None,
        ),
        (
            "E144",
            10,
            "`dist` is not a relation predicate",
            None,
        ),
        (
            "E147",
            11,
            "assert tolerance must be a unitless number",
            None,
        ),
        (
            "E114",
            12,
            "unexpected trailing tokens; Locus allows one statement per line",
            None,
        ),
        (
            "E150",
            14,
            "metadata values must be quoted strings",
            None,
        ),
    ]
    "###);
}
