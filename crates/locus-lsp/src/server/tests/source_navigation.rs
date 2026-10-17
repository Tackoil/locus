//! Beta 6 editing and source navigation through the LSP transport.

use super::*;

const SOURCE: &str = concat!(
    "locus 0 beta 6\n",
    "point outer()\n",
    "point shared()\n",
    "fn dot(text) {\n",
    "  point shared()\n",
    "  label caption(shared, text)\n",
    "  group marks(shared, caption)\n",
    "}\n",
    "fn pair(text) {\n",
    "  dot inner(text)\n",
    "}\n",
    "pair example(\"hi\")\n",
    "group all(example.inner.shared, example)\n",
    "rel center(example.inner.shared, outer)\n",
    "point later()\n",
);

fn open(source: &str) -> Transcript {
    let transcript = Transcript::start();
    transcript.notify(
        "textDocument/didOpen",
        json!({"textDocument": {
            "uri": URI, "languageId": "locus", "version": 1, "text": source,
        }}),
    );
    let _ = transcript.diagnostics();
    transcript
}

fn position(source: &str, needle: &str) -> Value {
    let byte = source.find(needle).expect("source contains test cursor");
    let document = Document::new(URI.parse().expect("test URI"), 1, source.to_owned());
    let cursor = document.byte_to_position(byte);
    text_position(URI, cursor.line, cursor.character)
}

#[test]
fn large_diagrams_do_not_block_navigation_or_shutdown() {
    let source = include_str!("../../../../../examples/demonstrations/checkout_sequence.locus");
    let mut transcript = Transcript::start();
    transcript.notify(
        "textDocument/didOpen",
        json!({"textDocument": {
            "uri": URI, "languageId": "locus", "version": 1, "text": source,
        }}),
    );
    let definition = transcript.request(
        "textDocument/definition",
        position(source, "connector checkout("),
    );
    assert_eq!(
        definition["range"]["start"]["line"],
        position(source, "fn connector(")["position"]["line"]
    );
    transcript.finish();
}

#[test]
fn measured_function_labels_do_not_report_missing_inputs() {
    let source = concat!(
        "locus 0 beta 6\n",
        "fn node(text) {\n",
        "  rect shape()\n",
        "  label caption(text)\n",
        "  rel center(caption, shape)\n",
        "  rel contain(shape, caption)\n",
        "}\n",
        "node example(\"Measured $x^2$\")\n",
    );
    let transcript = Transcript::start();
    transcript.notify(
        "textDocument/didOpen",
        json!({"textDocument": {
            "uri": URI, "languageId": "locus", "version": 1, "text": source,
        }}),
    );
    let diagnostics = transcript.diagnostics();
    assert!(
        diagnostics.diagnostics.is_empty(),
        "{:?}",
        diagnostics.diagnostics
    );
    transcript.finish();
}

#[test]
fn text_measurement_failures_have_source_diagnostics() {
    let transcript = Transcript::start();
    transcript.notify(
        "textDocument/didOpen",
        json!({"textDocument": {
            "uri": URI, "languageId": "locus", "version": 1,
            "text": "locus 0 beta 6\nlabel caption($\\unknowncommand$)\n",
        }}),
    );
    let diagnostics = transcript.diagnostics();
    assert!(diagnostic_codes(&diagnostics).contains(&"E414"));
    assert_eq!(diagnostics.diagnostics[0].range.start.line, 1);
    transcript.finish();
}

#[test]
fn local_bindings_shadow_globals_and_do_not_escape() {
    let mut transcript = open(SOURCE);
    let params = position(SOURCE, "shared, text");
    let completion = transcript.request("textDocument/completion", params.clone());
    for name in ["shared", "text", "outer"] {
        assert!(completion_contains(&completion, name), "missing {name}");
    }
    for name in ["later", "example", "caption"] {
        assert!(
            !completion_contains(&completion, name),
            "out of scope: {name}"
        );
    }
    let definition = transcript.request("textDocument/definition", params);
    assert_eq!(definition["range"]["start"]["line"], 4);
    let parameter = transcript.request("textDocument/definition", position(SOURCE, "text)\n"));
    assert_eq!(parameter["range"]["start"]["line"], 3);
    let completion = transcript.request(
        "textDocument/completion",
        position(SOURCE, "example.inner.shared, example"),
    );
    assert!(completion_contains(&completion, "example"));
    assert!(!completion_contains(&completion, "caption"));
    assert!(!completion_contains(&completion, "text"));
    transcript.finish();
}

#[test]
fn nested_instance_members_complete_and_navigate_to_source() {
    let mut transcript = open(SOURCE);
    for (needle, line) in [
        ("example.inner.shared, outer", 11),
        ("inner.shared, outer", 9),
        ("shared, outer", 4),
    ] {
        let definition = transcript.request("textDocument/definition", position(SOURCE, needle));
        assert_eq!(definition["range"]["start"]["line"], line, "{needle}");
    }
    let params = position(SOURCE, "shared, outer");
    let completion = transcript.request("textDocument/completion", params.clone());
    for name in ["shared", "caption", "marks"] {
        assert!(
            completion_contains(&completion, name),
            "missing member {name}"
        );
    }
    for name in ["center", "text", "outer"] {
        assert!(
            !completion_contains(&completion, name),
            "not a member: {name}"
        );
    }
    let hover = transcript.request("textDocument/hover", params);
    assert!(
        hover["contents"]["value"]
            .as_str()
            .is_some_and(|value| value.contains("point shared()"))
    );
    let hover = transcript.request("textDocument/hover", position(SOURCE, "all(example"));
    assert!(
        hover["contents"]["value"]
            .as_str()
            .is_some_and(|value| value.contains("group all"))
    );
    transcript.finish();
}

#[test]
fn style_parameters_and_invocation_classes_navigate() {
    let source = concat!(
        "locus 0 beta 6\n",
        "class emphasis { stroke: red }\n",
        "fn dot(text, appearance) {\n",
        "  label caption(text) [appearance]\n",
        "}\n",
        "dot sample(\"hi\", emphasis) [emphasis]\n",
    );
    let mut transcript = open(source);
    let params = position(source, "appearance]\n");
    let definition = transcript.request("textDocument/definition", params.clone());
    assert_eq!(definition["range"]["start"]["line"], 2);
    let completion = transcript.request("textDocument/completion", params);
    assert!(completion_contains(&completion, "appearance"));
    let hover = transcript.request("textDocument/hover", position(source, "appearance]\n"));
    assert!(
        hover["contents"]["value"]
            .as_str()
            .is_some_and(|value| value.contains("parameter appearance"))
    );
    let definition = transcript.request("textDocument/definition", position(source, "emphasis]\n"));
    assert_eq!(definition["range"]["start"]["line"], 1);
    transcript.finish();
}

#[test]
fn constructor_declarations_and_named_calls_offer_beta6_values() {
    let source = [
        SOURCE,
        concat!(
            "label note(\"hi\") { dir: auto }\n",
            "arrow path(outer, shared) { route: curved }\n",
            "pair other(outer)\n",
        ),
    ]
    .concat();
    let mut transcript = open(&source);
    let values = transcript.request("textDocument/completion", position(&source, "auto }"));
    assert!(completion_contains(&values, "auto"));
    let values = transcript.request("textDocument/completion", position(&source, "curved }"));
    for name in ["direct", "curved", "orthogonal"] {
        assert!(completion_contains(&values, name));
    }
    let args = transcript.request("textDocument/completion", position(&source, "outer)\n"));
    for name in ["outer", "example", "all"] {
        assert!(completion_contains(&args, name));
    }
    transcript.finish();
}
