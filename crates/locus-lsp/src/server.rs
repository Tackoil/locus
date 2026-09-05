//! Synchronous `lsp-server` transport and request dispatch.

use std::collections::BTreeMap;
use std::error::Error;
use std::time::{Duration, Instant};

use crossbeam_channel::RecvTimeoutError;
use lsp_server::{Connection, ErrorCode, Message, Notification, Request, Response};
use lsp_types::notification::Notification as _;
use lsp_types::request::Request as _;
use lsp_types::{
    DidChangeTextDocumentParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams,
    DidSaveTextDocumentParams, PublishDiagnosticsParams,
};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

use crate::capabilities::server_capabilities;
use crate::handlers::{completion, definition, formatting, hover, symbols};
use crate::state::{Analysis, State};

type ServerResult<T> = Result<T, Box<dyn Error + Send + Sync>>;
const CHANGE_DEBOUNCE: Duration = Duration::from_millis(40);

pub(crate) fn run_stdio() -> ServerResult<()> {
    let (connection, io_threads) = Connection::stdio();
    let result = run(&connection);
    io_threads.join()?;
    result
}

pub(crate) fn run(connection: &Connection) -> ServerResult<()> {
    initialize(connection)?;
    let mut state = State::default();
    let mut pending = PendingAnalyses::default();
    loop {
        flush_due(connection, &mut state, &mut pending)?;
        if handle_receive(connection, &mut state, &mut pending)? {
            return Ok(());
        }
    }
}

fn handle_receive(
    connection: &Connection,
    state: &mut State,
    pending: &mut PendingAnalyses,
) -> ServerResult<bool> {
    match receive(connection, pending) {
        Receive::Message(message) => dispatch_message(connection, state, pending, message),
        Receive::Timeout => Ok(false),
        Receive::Disconnected => Ok(true),
    }
}

enum Receive {
    Message(Message),
    Timeout,
    Disconnected,
}

fn receive(connection: &Connection, pending: &PendingAnalyses) -> Receive {
    match connection.receiver.recv_timeout(pending.wait_duration()) {
        Ok(message) => Receive::Message(message),
        Err(RecvTimeoutError::Timeout) => Receive::Timeout,
        Err(RecvTimeoutError::Disconnected) => Receive::Disconnected,
    }
}

fn dispatch_message(
    connection: &Connection,
    state: &mut State,
    pending: &mut PendingAnalyses,
    message: Message,
) -> ServerResult<bool> {
    match message {
        Message::Request(request) => {
            flush_all(connection, state, pending)?;
            if connection.handle_shutdown(&request)? {
                return Ok(true);
            }
            handle_request(connection, state, request)?;
        }
        Message::Notification(notification) => {
            if notification.method == "exit" {
                return Ok(true);
            }
            handle_notification(connection, state, pending, notification)?;
        }
        Message::Response(_) => {}
    }
    Ok(false)
}

fn initialize(connection: &Connection) -> ServerResult<()> {
    let (id, _) = connection.initialize_start()?;
    connection.initialize_finish(
        id,
        json!({
            "capabilities": server_capabilities(),
            "serverInfo": {
                "name": "locus-lsp",
                "version": env!("CARGO_PKG_VERSION"),
            }
        }),
    )?;
    Ok(())
}

fn handle_request(connection: &Connection, state: &State, request: Request) -> ServerResult<()> {
    let id = request.id.clone();
    let result = dispatch_request(state, request);
    let response = match result {
        Ok(value) => Response::new_ok(id, value),
        Err((code, message)) => Response::new_err(id, code, message),
    };
    connection.sender.send(Message::Response(response))?;
    Ok(())
}

fn dispatch_request(state: &State, request: Request) -> Result<Value, (i32, String)> {
    let method = request.method.as_str();
    if method == lsp_types::request::Completion::METHOD {
        let params = params::<lsp_types::CompletionParams>(request.params)?;
        return serialize(completion::handle(state, &params));
    }
    if method == lsp_types::request::HoverRequest::METHOD {
        let params = params::<lsp_types::HoverParams>(request.params)?;
        return serialize(hover::handle(state, &params));
    }
    if method == lsp_types::request::GotoDefinition::METHOD {
        let params = params::<lsp_types::GotoDefinitionParams>(request.params)?;
        return serialize(definition::handle(state, &params));
    }
    if method == lsp_types::request::Formatting::METHOD {
        let params = params::<lsp_types::DocumentFormattingParams>(request.params)?;
        return serialize(formatting::handle(state, &params));
    }
    if method == lsp_types::request::DocumentSymbolRequest::METHOD {
        let params = params::<lsp_types::DocumentSymbolParams>(request.params)?;
        return serialize(symbols::handle(state, &params));
    }
    Err((
        ErrorCode::MethodNotFound as i32,
        format!("unsupported request `{method}`"),
    ))
}

fn handle_notification(
    connection: &Connection,
    state: &mut State,
    pending: &mut PendingAnalyses,
    notification: Notification,
) -> ServerResult<()> {
    let method = notification.method.as_str();
    if method == lsp_types::notification::DidOpenTextDocument::METHOD {
        let params: DidOpenTextDocumentParams = deserialize(notification.params)?;
        let item = params.text_document;
        pending.remove(&item.uri);
        let analysis = state.open(item.uri, item.version, item.text);
        publish_analysis(connection, state, &analysis)?;
    } else if method == lsp_types::notification::DidChangeTextDocument::METHOD {
        let params: DidChangeTextDocumentParams = deserialize(notification.params)?;
        if state.change(
            &params.text_document.uri,
            params.text_document.version,
            &params.content_changes,
        ) {
            pending.schedule(params.text_document.uri);
        }
    } else if method == lsp_types::notification::DidSaveTextDocument::METHOD {
        let params: DidSaveTextDocumentParams = deserialize(notification.params)?;
        pending.remove(&params.text_document.uri);
        if let Some(analysis) = state.analyze_current(&params.text_document.uri) {
            publish_analysis(connection, state, &analysis)?;
        }
    } else if method == lsp_types::notification::DidCloseTextDocument::METHOD {
        let params: DidCloseTextDocumentParams = deserialize(notification.params)?;
        pending.remove(&params.text_document.uri);
        if let Some(document) = state.close(&params.text_document.uri) {
            publish(
                connection,
                PublishDiagnosticsParams::new(document.uri().clone(), Vec::new(), None),
            )?;
        }
    }
    Ok(())
}

#[derive(Default)]
struct PendingAnalyses {
    deadlines: BTreeMap<lsp_types::Uri, Instant>,
}

impl PendingAnalyses {
    fn schedule(&mut self, uri: lsp_types::Uri) {
        self.deadlines.insert(uri, Instant::now() + CHANGE_DEBOUNCE);
    }

    fn remove(&mut self, uri: &lsp_types::Uri) {
        self.deadlines.remove(uri);
    }

    fn wait_duration(&self) -> Duration {
        self.deadlines
            .values()
            .min()
            .map_or(Duration::from_secs(60), |deadline| {
                deadline.saturating_duration_since(Instant::now())
            })
    }

    fn take_due(&mut self) -> Vec<lsp_types::Uri> {
        let now = Instant::now();
        let due = self
            .deadlines
            .iter()
            .filter_map(|(uri, deadline)| (*deadline <= now).then_some(uri.clone()))
            .collect::<Vec<_>>();
        for uri in &due {
            self.deadlines.remove(uri);
        }
        due
    }

    fn take_all(&mut self) -> Vec<lsp_types::Uri> {
        std::mem::take(&mut self.deadlines).into_keys().collect()
    }
}

fn flush_due(
    connection: &Connection,
    state: &mut State,
    pending: &mut PendingAnalyses,
) -> ServerResult<()> {
    analyze_and_publish(connection, state, pending.take_due())
}

fn flush_all(
    connection: &Connection,
    state: &mut State,
    pending: &mut PendingAnalyses,
) -> ServerResult<()> {
    analyze_and_publish(connection, state, pending.take_all())
}

fn analyze_and_publish(
    connection: &Connection,
    state: &mut State,
    uris: Vec<lsp_types::Uri>,
) -> ServerResult<()> {
    for uri in uris {
        if let Some(analysis) = state.analyze_current(&uri) {
            publish_analysis(connection, state, &analysis)?;
        }
    }
    Ok(())
}

pub(crate) fn publish_analysis(
    connection: &Connection,
    state: &State,
    analysis: &Analysis,
) -> ServerResult<bool> {
    if !state.is_current(analysis) {
        return Ok(false);
    }
    publish(
        connection,
        PublishDiagnosticsParams::new(
            analysis.uri().clone(),
            analysis.diagnostics.clone(),
            Some(analysis.version()),
        ),
    )?;
    Ok(true)
}

fn publish(connection: &Connection, params: PublishDiagnosticsParams) -> ServerResult<()> {
    connection
        .sender
        .send(Message::Notification(Notification::new(
            lsp_types::notification::PublishDiagnostics::METHOD.to_owned(),
            params,
        )))?;
    Ok(())
}

fn params<T: DeserializeOwned>(value: Value) -> Result<T, (i32, String)> {
    serde_json::from_value(value).map_err(|error| {
        (
            ErrorCode::InvalidParams as i32,
            format!("invalid request parameters: {error}"),
        )
    })
}

fn deserialize<T: DeserializeOwned>(value: Value) -> ServerResult<T> {
    Ok(serde_json::from_value(value)?)
}

fn serialize<T: serde::Serialize>(value: T) -> Result<Value, (i32, String)> {
    serde_json::to_value(value).map_err(|error| {
        (
            ErrorCode::InternalError as i32,
            format!("could not serialize response: {error}"),
        )
    })
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
mod tests {
    use std::str::FromStr as _;
    use std::thread;
    use std::time::Duration;

    use lsp_server::{Message, RequestId};
    use lsp_types::Uri;

    use super::*;
    use crate::document::Document;

    const URI: &str = "file:///memory.locus";

    struct Transcript {
        client: Connection,
        server_thread: Option<thread::JoinHandle<ServerResult<()>>>,
        next_id: i32,
    }

    impl Transcript {
        fn start() -> Self {
            let (server, client) = Connection::memory();
            let server_thread = thread::spawn(move || run(&server));
            let mut transcript = Self {
                client,
                server_thread: Some(server_thread),
                next_id: 1,
            };
            let response = transcript.request("initialize", json!({"capabilities": {}}));
            assert!(response.get("capabilities").is_some());
            transcript.notify("initialized", json!({}));
            transcript
        }

        fn request(&mut self, method: &str, params: Value) -> Value {
            let id = RequestId::from(self.next_id);
            self.next_id += 1;
            self.client
                .sender
                .send(Message::Request(Request {
                    id: id.clone(),
                    method: method.to_owned(),
                    params,
                }))
                .expect("send request");
            loop {
                let message = self
                    .client
                    .receiver
                    .recv_timeout(Duration::from_secs(2))
                    .expect("receive response");
                if let Message::Response(response) = message
                    && response.id == id
                {
                    return response.response_result.expect("successful response");
                }
            }
        }

        fn notify(&self, method: &str, params: Value) {
            self.client
                .sender
                .send(Message::Notification(Notification {
                    method: method.to_owned(),
                    params,
                }))
                .expect("send notification");
        }

        fn diagnostics(&self) -> PublishDiagnosticsParams {
            loop {
                let message = self
                    .client
                    .receiver
                    .recv_timeout(Duration::from_secs(2))
                    .expect("receive diagnostics");
                if let Message::Notification(notification) = message
                    && notification.method == lsp_types::notification::PublishDiagnostics::METHOD
                {
                    return serde_json::from_value(notification.params)
                        .expect("diagnostic parameters");
                }
            }
        }

        fn finish(mut self) {
            let response = self.request("shutdown", Value::Null);
            assert_eq!(response, Value::Null);
            self.notify("exit", Value::Null);
            let Some(server_thread) = self.server_thread.take() else {
                return;
            };
            server_thread
                .join()
                .expect("join server")
                .expect("server run");
        }
    }

    #[test]
    fn initialization_open_change_save_utf16_and_shutdown_transcript() {
        let transcript = Transcript::start();
        transcript.notify(
            "textDocument/didOpen",
            json!({
                "textDocument": {
                    "uri": URI,
                    "languageId": "locus",
                    "version": 1,
                    "text": "locus 2\nnum a = 1\n"
                }
            }),
        );
        let opened = transcript.diagnostics();
        assert_eq!(opened.version, Some(1));
        assert!(opened.diagnostics.is_empty());

        transcript.notify(
            "textDocument/didChange",
            json!({
                "textDocument": {"uri": URI, "version": 2},
                "contentChanges": [{"text": "locus 2\nnum a = \"😀\" +\n"}]
            }),
        );
        let changed = transcript.diagnostics();
        assert_eq!(changed.version, Some(2));
        assert!(!changed.diagnostics.is_empty());
        assert!(changed.diagnostics[0].range.start.character >= 11);

        transcript.notify(
            "textDocument/didSave",
            json!({"textDocument": {"uri": URI}}),
        );
        let saved = transcript.diagnostics();
        assert_eq!(saved.version, Some(2));
        transcript.finish();
    }

    #[test]
    fn completion_hover_definition_formatting_and_symbols_transcript() {
        let mut transcript = Transcript::start();
        transcript.notify(
            "textDocument/didOpen",
            json!({
                "textDocument": {
                    "uri": URI,
                    "languageId": "locus",
                    "version": 1,
                    "text": "locus 2\npoint A=free ~(0,0)\npoint M = midpoint(A,A)\npoint Z = midpoint(M,M)\n"
                }
            }),
        );
        let _ = transcript.diagnostics();

        let completion = transcript.request("textDocument/completion", text_position(URI, 2, 19));
        assert!(
            completion
                .as_array()
                .is_some_and(|items| { items.iter().any(|item| item["label"] == "A") })
        );
        assert!(completion.as_array().is_some_and(|items| {
            items
                .iter()
                .all(|item| item["label"] != "M" && item["label"] != "Z")
        }));

        let style_completion =
            transcript.request("textDocument/completion", text_position(URI, 4, 0));
        let style_values = [
            "solid", "dashed", "dotted", "thin", "normal", "bold", "ink", "gray", "blue", "orange",
            "teal", "green", "red", "purple", "tint", "butt", "round", "square", "miter", "bevel",
        ];
        assert!(style_completion.as_array().is_some_and(|items| {
            style_values.iter().all(|expected| {
                items
                    .iter()
                    .any(|item| item["label"].as_str() == Some(expected))
            })
        }));

        let hover = transcript.request("textDocument/hover", text_position(URI, 2, 11));
        assert!(
            hover["contents"]["value"]
                .as_str()
                .is_some_and(|value| value.contains("midpoint(A, B) -> point"))
        );

        let definition = transcript.request("textDocument/definition", text_position(URI, 2, 19));
        assert_eq!(definition["range"]["start"]["line"], 1);

        let formatting = transcript.request(
            "textDocument/formatting",
            json!({"textDocument": {"uri": URI}, "options": {"tabSize": 2, "insertSpaces": true}}),
        );
        assert!(
            formatting[0]["newText"]
                .as_str()
                .is_some_and(|value| value.contains("point A"))
        );

        let symbols = transcript.request(
            "textDocument/documentSymbol",
            json!({"textDocument": {"uri": URI}}),
        );
        assert_eq!(symbols.as_array().map(Vec::len), Some(3));
        transcript.finish();
    }

    #[test]
    fn navigation_uses_ast_reference_and_callable_spans() {
        let mut transcript = Transcript::start();
        transcript.notify(
            "textDocument/didOpen",
            json!({
                "textDocument": {
                    "uri": URI,
                    "languageId": "locus",
                    "version": 1,
                    "text": concat!(
                        "locus 2\n",
                        "point midpoint = free ~(0, 0)\n",
                        "point P = midpoint(midpoint, midpoint) // midpoint\n",
                        "label note = label(P, \"midpoint\")\n",
                    )
                }
            }),
        );
        let _ = transcript.diagnostics();

        let constructor_hover = transcript.request("textDocument/hover", text_position(URI, 2, 11));
        assert!(
            constructor_hover["contents"]["value"]
                .as_str()
                .is_some_and(|value| value.contains("midpoint(A, B) -> point"))
        );
        let constructor_definition =
            transcript.request("textDocument/definition", text_position(URI, 2, 11));
        assert_eq!(constructor_definition, Value::Null);

        let reference_definition =
            transcript.request("textDocument/definition", text_position(URI, 2, 20));
        assert_eq!(reference_definition["range"]["start"]["line"], 1);
        let comment_hover = transcript.request("textDocument/hover", text_position(URI, 2, 45));
        assert_eq!(comment_hover, Value::Null);
        let text_definition =
            transcript.request("textDocument/definition", text_position(URI, 3, 25));
        assert_eq!(text_definition, Value::Null);
        transcript.finish();
    }

    #[test]
    fn completion_includes_self_in_where_and_prior_style_classes() {
        let mut transcript = Transcript::start();
        transcript.notify(
            "textDocument/didOpen",
            json!({
                "textDocument": {
                    "uri": URI,
                    "languageId": "locus",
                    "version": 1,
                    "text": concat!(
                        "locus 2\n",
                        "class faint { stroke: dashed }\n",
                        "point A = free ~(0, 0)\n",
                        "point M = free ~(1, 1) where { coincident(M, A) } [faint]\n",
                        "point Z = midpoint(M, A)\n",
                    )
                }
            }),
        );
        let _ = transcript.diagnostics();

        let where_completion =
            transcript.request("textDocument/completion", text_position(URI, 3, 43));
        assert!(completion_contains(&where_completion, "M"));
        assert!(!completion_contains(&where_completion, "Z"));

        let class_completion =
            transcript.request("textDocument/completion", text_position(URI, 3, 56));
        assert!(completion_contains(&class_completion, "faint"));
        transcript.finish();
    }

    #[test]
    fn completion_requires_one_header_in_the_first_statement() {
        let mut transcript = Transcript::start();
        transcript.notify(
            "textDocument/didOpen",
            json!({
                "textDocument": {
                    "uri": URI,
                    "languageId": "locus",
                    "version": 1,
                    "text": ""
                }
            }),
        );
        let _ = transcript.diagnostics();

        let before_header = transcript.request("textDocument/completion", text_position(URI, 0, 0));
        assert_eq!(
            before_header.as_array().map(Vec::len),
            Some(1),
            "only the mandatory header should be offered"
        );
        assert!(completion_contains(&before_header, "locus"));

        transcript.notify(
            "textDocument/didChange",
            json!({
                "textDocument": {"uri": URI, "version": 2},
                "contentChanges": [{"text": "locus 2\n\n"}]
            }),
        );
        let after_header = transcript.request("textDocument/completion", text_position(URI, 1, 0));
        assert!(completion_contains(&after_header, "point"));
        assert!(!completion_contains(&after_header, "locus"));
        transcript.finish();
    }

    #[test]
    fn bare_cr_positions_work_for_diagnostics_and_navigation() {
        let mut transcript = Transcript::start();
        transcript.notify(
            "textDocument/didOpen",
            json!({
                "textDocument": {
                    "uri": URI,
                    "languageId": "locus",
                    "version": 1,
                    "text": "locus 2\rpoint A = free ~(0, 0)\rpoint M = midpoint(A, A)\r"
                }
            }),
        );
        let opened = transcript.diagnostics();
        assert!(opened.diagnostics.is_empty());

        let definition = transcript.request("textDocument/definition", text_position(URI, 2, 19));
        assert_eq!(definition["range"]["start"]["line"], 1);

        transcript.notify(
            "textDocument/didChange",
            json!({
                "textDocument": {"uri": URI, "version": 2},
                "contentChanges": [{"text": "locus 2\rnum a = \"😀\" +\r"}]
            }),
        );
        let changed = transcript.diagnostics();
        assert_eq!(changed.diagnostics[0].range.start.line, 1);
        assert!(changed.diagnostics[0].range.start.character >= 11);
        transcript.finish();
    }

    #[test]
    fn stale_analysis_is_suppressed_over_memory_transport() {
        let Ok(uri) = Uri::from_str(URI) else {
            return;
        };
        let stale_document = Document::new(uri.clone(), 1, "locus 2\nnum a = 1\n".to_owned());
        let stale = Analysis::new(&stale_document);
        let mut server_state = State::default();
        let current = server_state.open(uri, 2, "locus 2\nnum a = 2\n".to_owned());
        let (server, client) = Connection::memory();
        assert!(!publish_analysis(&server, &server_state, &stale).expect("stale check"));
        assert!(
            client
                .receiver
                .recv_timeout(Duration::from_millis(20))
                .is_err()
        );
        assert!(publish_analysis(&server, &server_state, &current).expect("current publication"));
        assert!(matches!(
            client.receiver.recv_timeout(Duration::from_secs(1)),
            Ok(Message::Notification(_))
        ));
    }

    fn text_position(uri: &str, line: u32, character: u32) -> Value {
        json!({
            "textDocument": {"uri": uri},
            "position": {"line": line, "character": character}
        })
    }

    fn completion_contains(completion: &Value, label: &str) -> bool {
        completion.as_array().is_some_and(|items| {
            items
                .iter()
                .any(|item| item["label"].as_str() == Some(label))
        })
    }
}
