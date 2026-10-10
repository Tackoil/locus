//! Beta 5 syntax, recovery, and interchange regressions.
use locus_syntax::{Constructor, Statement, format, format_file, parse, to_interchange};

#[test]
fn metadata_seeds_and_alpha_round_trip() {
    let source = r#"locus 0 beta 5
// Metadata stays ordered, including repeated keys and interspersed objects.
meta title="Mixed $x$" // keep title
num seed = 1.5
meta title = "Next"
num r = free (seed + 2) * 3
class region { accent: #ab12ef80, fill: #123abc00 }
"#;
    let parsed = parse(source);
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    assert!(matches!(parsed.file.statements[0], Statement::Meta(_)));
    let Statement::Op(op) = &parsed.file.statements[3] else {
        unreachable!()
    };
    assert!(matches!(op.ctor, Constructor::FreeNum { .. }));
    let json = to_interchange(&parsed.file);
    assert_eq!(json.version, "0 beta 5");
    assert_eq!(json.meta.len(), 2);
    assert_eq!(json.meta[0].value.len(), 3);
    assert!(json.ops[1].hint.is_some());
    let formatted = format(source);
    assert!(formatted.diagnostics.is_empty());
    assert!(
        formatted
            .source
            .contains("meta title = \"Mixed $x$\"  // keep title")
    );
    assert!(
        formatted
            .source
            .contains("accent: #AB12EF80, fill: #123ABC00")
    );
    assert!(formatted.source.contains("free (seed + 2) * 3"));
    assert_eq!(format(&formatted.source).source, formatted.source);
    assert!(parse(&format_file(&parsed.file)).diagnostics.is_empty());
}

#[test]
fn rejects_previous_header_and_legacy_syntax() {
    for source in [
        "locus 0 beta 4\nnum n = 1\n",
        "locus 0 beta 5\nmeta {\n title \"old\"\n}\n",
        "locus 0 beta 5\nnum r = free ~1.5\n",
    ] {
        assert!(!parse(source).diagnostics.is_empty(), "{source}");
    }
}

#[test]
fn rejects_missing_and_coordinate_pair_seeds_locally() {
    for seed in ["", "(1, 2)"] {
        let parsed = parse(&format!(
            "locus 0 beta 5\nnum r = free {seed}\nnum valid = 2\n"
        ));
        assert_eq!(parsed.diagnostics.len(), 1);
        assert_eq!(parsed.diagnostics[0].code, "E124");
        assert_eq!(parsed.diagnostics[0].span.line, 2);
        assert_eq!(parsed.file.statements.len(), 1);
    }
}

#[test]
fn rejects_invalid_hex_lengths_and_digits_locally() {
    for color in [
        "#ABC",
        "#ABCD",
        "#12345",
        "#1234567",
        "#123456789",
        "#123456GG",
    ] {
        let parsed = parse(&format!(
            "locus 0 beta 5\nclass bad {{ accent: {color} }}\nnum valid = 1\n"
        ));
        assert_eq!(
            parsed.diagnostics.len(),
            1,
            "{color}: {:?}",
            parsed.diagnostics
        );
        assert_eq!(parsed.diagnostics[0].code, "E105");
        assert_eq!(parsed.diagnostics[0].span.line, 2);
        assert_eq!(parsed.file.statements.len(), 1);
    }
}
