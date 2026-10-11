//! Beta 6 source preservation, expansion, and collection contracts.
use locus_syntax::{
    CollectionKind, Constructor, Expr, File, Statement, expand, format, format_file, parse,
    to_interchange,
};

fn valid(source: &str) -> File {
    let parsed = parse(source);
    assert!(parsed.diagnostics.is_empty(), "{:#?}", parsed.diagnostics);
    parsed.file
}

fn op<'a>(file: &'a File, id: &str) -> &'a locus_syntax::OpStmt {
    file.statements
        .iter()
        .find_map(|statement| match statement {
            Statement::Op(op) if op.id.name == id => Some(op),
            _ => None,
        })
        .unwrap_or_else(|| unreachable!("missing operation {id}"))
}

#[test]
fn constructor_declarations_and_derived_postfix_preserve_source_forms() {
    let source = "locus 0 beta 6\npoint A()\npoint B()\nsegment edge(A, B)\npoint M = seg(A, B).midpoint\nrect box()\nlabel caption(\"hello $x$\") { role: title }\nrel contain(box, caption)\n";
    let file = valid(source);
    assert!(op(&file, "A").is_constructor_declaration());
    assert!(!op(&file, "M").is_constructor_declaration());
    let canonical = format(source);
    assert_eq!(canonical.source, format(&canonical.source).source);
    valid(&format_file(&file));
}

#[test]
fn calls_expand_independently_with_text_and_class_substitution() {
    let file = valid(
        "locus 0 beta 6\nfn box(text, appearance) {\n    rect shape()\n    label caption(text) [appearance]\n    rel contain(shape, caption)\n}\nbox first(\"one $x$\", auxiliary) [highlight]\nbox second(\"two\", main)\nrel column(first, second)\n",
    );
    assert!(matches!(file.statements[0], Statement::Fn(_)));
    let expanded = expand(&file);
    assert!(expanded.diagnostics.is_empty());
    assert_eq!(
        op(&expanded.file, "first.shape")
            .attr
            .as_ref()
            .map(|attr| attr.name.name.as_str()),
        Some("highlight")
    );
    assert_eq!(
        op(&expanded.file, "first.caption")
            .attr
            .as_ref()
            .map(|attr| attr.name.name.as_str()),
        Some("auxiliary")
    );
    let Constructor::Call { args, .. } = &op(&expanded.file, "first.caption").ctor else {
        unreachable!()
    };
    assert!(matches!(&args[0], Expr::String(text) if text.segments.len() == 3));
    assert_eq!(
        expanded.collections["first"].members,
        ["first.shape", "first.caption"]
    );
    assert_eq!(expanded.origins[0].invocations[0].instance, "first");
}

#[test]
fn nested_calls_resolve_namespaces_before_object_properties() {
    let file = valid(
        "locus 0 beta 6\nfn leaf() {\n    rect shape(2, 1)\n}\nfn branch() {\n    leaf child() [auxiliary]\n}\nbranch outer() [highlight]\npoint P = outer.child.shape.n\n",
    );
    let expanded = expand(&file);
    assert_eq!(expanded.collections["outer"].members, ["outer.child.shape"]);
    assert_eq!(
        expanded.collections["outer.child"].kind,
        CollectionKind::Instance
    );
    let Constructor::Expr { expr, .. } = &op(&expanded.file, "P").ctor else {
        unreachable!()
    };
    assert!(
        matches!(expr.as_ref(), Expr::Property { base, name, .. } if name.name == "n" && matches!(base.as_ref(), Expr::Ref(id) if id.name == "outer.child.shape"))
    );
    assert_eq!(expanded.origins[0].invocations.len(), 2);
    assert_eq!(
        op(&expanded.file, "outer.child.shape")
            .attr
            .as_ref()
            .map(|attr| attr.name.name.as_str()),
        Some("auxiliary")
    );
}

#[test]
fn substitution_retains_lexical_bindings_and_does_not_capture_caller_names() {
    let file = valid(
        "locus 0 beta 6\npoint external()\nfn pair(value) {\n    point external()\n    point alias = value\n}\npair first(external)\n",
    );
    let expanded = expand(&file);
    let Constructor::Expr { expr, .. } = &op(&expanded.file, "first.alias").ctor else {
        unreachable!()
    };
    assert!(matches!(expr.as_ref(), Expr::Ref(id) if id.name == "external"));
    assert_eq!(expanded.collections["first"].members, ["first.external"]);
}

#[test]
fn groups_flatten_deduplicate_aliases_and_keep_instances_atomic() {
    let file = valid(
        "locus 0 beta 6\npoint A() { mark: none }\npoint B()\npoint alias = A\nfn dot() {\n    point hidden() { mark: none }\n    point shown()\n}\ndot instance()\ngroup first(A, B)\ngroup all(first, alias, instance, B)\nrel row(all)\n",
    );
    let expanded = expand(&file);
    assert_eq!(expanded.collections["all"].members, ["A", "B", "instance"]);
    assert_eq!(expanded.collections["instance"].members, ["instance.shown"]);
}

#[test]
fn local_groups_do_not_add_external_members_to_instance_bounds() {
    let file = valid(
        "locus 0 beta 6\npoint external()\nfn decorate(target) {\n    label caption(target, \"caption\")\n    group members(target, caption)\n}\ndecorate instance(external)\n",
    );
    let expanded = expand(&file);
    assert_eq!(
        expanded.collections["instance"].members,
        ["instance.caption"]
    );
    assert_eq!(
        expanded.collections["instance.members"].members,
        ["external", "instance.caption"]
    );
}

#[test]
fn inset_padding_and_symbolic_edge_directions_survive_formatting() {
    let source = "locus 0 beta 6\nrect box(2, 1)\npoint E()\nsegment boundary = edge(box, E)\nrel inset_left(E, box) { padding: none }\nnum short = style_metric(\"short_length\")\n";
    let file = valid(source);
    assert!(format(source).source.contains("{ padding: none }"));
    let json = to_interchange(&file);
    assert_eq!(json.relation_properties[0].props[0].name, "padding");
    let expanded = expand(&file);
    let Constructor::Call { args, .. } = &op(&expanded.file, "boundary").ctor else {
        unreachable!()
    };
    assert!(matches!(&args[1], Expr::Ref(id) if id.name == "E"));
}

#[test]
fn json_retains_editable_sources_and_expanded_identities() {
    let file = valid(
        "locus 0 beta 6\nfn dot() {\n    point P()\n}\ndot instance()\ngroup points(instance.P)\n",
    );
    let json = to_interchange(&file);
    assert_eq!(json.version, "0 beta 6");
    assert_eq!(json.functions.len(), 1);
    assert_eq!(json.calls.len(), 1);
    assert_eq!(json.groups.len(), 1);
    assert_eq!(
        json.expanded.as_ref().map(|flat| flat.ops[0].id.as_str()),
        Some("instance.P")
    );
    assert_eq!(json.collections["points"].members, ["instance.P"]);
}

#[test]
fn beta6_rejects_removed_forms_and_invalid_expansion_inputs() {
    for source in [
        "fn unused() {\n    point P = missing\n}\n",
        "point P = point()\n",
        "point P(1)\n",
        "num n(1)\n",
        "point P()\nrel horizontal(P, P)\n",
        "num n = 1\ngroup invalid(n)\n",
        "group missing(future)\npoint future()\n",
        "fn dot() {\n    point P()\n}\ndot instance(1)\n",
        "fn dot() {\n    point P()\n}\ndot instance()\npoint P = instance.missing\n",
        "label caption(\"text\") { dir: N }\n",
        "point P()\nrel center(P, P) { padding: none }\n",
    ] {
        let parsed = parse(&format!("locus 0 beta 6\n{source}"));
        assert!(!parsed.diagnostics.is_empty(), "accepted {source}");
    }
}

#[test]
fn expansion_preserves_reference_occurrence_spans_and_body_origins() {
    let file = valid("locus 0 beta 6\nnum n = 2\nassert n == 2\n");
    let expanded = expand(&file);
    let Statement::Assert(assertion) = &expanded.file.statements[1] else {
        unreachable!()
    };
    assert!(matches!(&assertion.expr, Expr::Binary { lhs, .. } if lhs.span().line == 3));
    let parsed = parse("locus 0 beta 6\nfn invalid() {\n    point P(1)\n}\ninvalid instance()\n");
    assert!(
        parsed
            .diagnostics
            .iter()
            .any(|diag| diag.span.line == 3
                && diag.notes.iter().any(|note| note.contains("instance")))
    );
}

#[test]
fn dimensions_are_inferred_independently_for_each_instance() {
    let file = valid(
        "locus 0 beta 6\nfn value() {\n    num n = free 1\n}\nvalue length()\nvalue scalar()\ncircle c(length.n)\nnum angle = scalar.n * 2deg\n",
    );
    let expanded = expand(&file);
    let json = to_interchange(&expanded.file);
    let Some(locus_syntax::json::Hint::Scalar(locus_syntax::json::ExprNode::Number(first))) =
        &json.ops[0].hint
    else {
        unreachable!()
    };
    let Some(locus_syntax::json::Hint::Scalar(locus_syntax::json::ExprNode::Number(second))) =
        &json.ops[1].hint
    else {
        unreachable!()
    };
    assert_eq!(first.dimension, locus_syntax::json::Dimension::Length);
    assert_eq!(second.dimension, locus_syntax::json::Dimension::Scalar);
    let maps = locus_syntax::json::inferred_statement_expression_dimensions(&expanded.file);
    assert_eq!(
        maps[0].values().next(),
        Some(&locus_syntax::json::Dimension::Length)
    );
    assert_eq!(
        maps[1].values().next(),
        Some(&locus_syntax::json::Dimension::Scalar)
    );
}

#[test]
fn parentheses_preserve_relation_and_where_mode_validation() {
    let file = valid(
        "locus 0 beta 6\npoint A()\npoint B()\nrect shape(2,1)\npoint P() where { (on(P,seg(A,B))) }\nrel (center(A,shape)) and ((align_center_y(A,B)))\n",
    );
    assert!(expand(&file).diagnostics.is_empty());
    let rejected = parse("locus 0 beta 6\npoint A()\nrect shape(2,1)\nrel (inside(A,shape))\n");
    assert!(
        rejected
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "E146")
    );
}

#[test]
fn earlier_format_headers_are_rejected() {
    for revision in 0..6 {
        let output = parse(&format!("locus 0 beta {revision}\nnum n = 1\n"));
        assert!(
            output
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "E111")
        );
    }
}
