//! LSP transport and request dispatch with background validation.

use std::collections::BTreeMap;
use std::error::Error;
use std::time::{Duration, Instant};

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
    drop(connection);
    let io_result = io_threads.join();
    result?;
    io_result?;
    Ok(())
}

pub(crate) fn run(connection: &Connection) -> ServerResult<()> {
    let client_capabilities = initialize(connection)?;
    let mut state = State::with_client_capabilities(&client_capabilities);
    let mut pending = PendingAnalyses::default();
    loop {
        flush_due(&mut state, &mut pending)?;
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
        Receive::Analysis(analysis) => {
            publish_analysis(connection, state, &analysis)?;
            Ok(false)
        }
        Receive::Timeout => Ok(false),
        Receive::Disconnected => {
            Err(std::io::Error::other("client disconnected before shutdown").into())
        }
    }
}

enum Receive {
    Message(Message),
    Analysis(Analysis),
    Timeout,
    Disconnected,
}

fn receive(connection: &Connection, pending: &PendingAnalyses) -> Receive {
    crossbeam_channel::select! {
        recv(connection.receiver) -> message => message.map_or(Receive::Disconnected, Receive::Message),
        recv(pending.worker.completed) -> analysis => analysis.map_or(Receive::Disconnected, Receive::Analysis),
        default(pending.wait_duration()) => Receive::Timeout,
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
            flush_all(state, pending)?;
            if connection.handle_shutdown(&request)? {
                return Ok(true);
            }
            handle_request(connection, state, request)?;
        }
        Message::Notification(notification) => {
            if notification.method == "exit" {
                return Err(std::io::Error::other("received exit before shutdown").into());
            }
            handle_notification(connection, state, pending, notification)?;
        }
        Message::Response(_) => {}
    }
    Ok(false)
}

fn initialize(connection: &Connection) -> ServerResult<lsp_types::ClientCapabilities> {
    let (id, params) = connection.initialize_start()?;
    let client_capabilities = params.get("capabilities").cloned().map_or_else(
        || Ok(lsp_types::ClientCapabilities::default()),
        serde_json::from_value,
    )?;
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
    Ok(client_capabilities)
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
        pending.worker.submit(state, analysis)?;
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
            pending.worker.submit(state, analysis)?;
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
    worker: crate::worker::Worker,
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

fn flush_due(state: &mut State, pending: &mut PendingAnalyses) -> ServerResult<()> {
    let uris = pending.take_due();
    analyze_and_schedule(state, pending, uris)
}

fn flush_all(state: &mut State, pending: &mut PendingAnalyses) -> ServerResult<()> {
    let uris = pending.take_all();
    analyze_and_schedule(state, pending, uris)
}

fn analyze_and_schedule(
    state: &mut State,
    pending: &PendingAnalyses,
    uris: Vec<lsp_types::Uri>,
) -> ServerResult<()> {
    for uri in uris {
        if let Some(analysis) = state.analyze_current(&uri) {
            pending.worker.submit(state, analysis)?;
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
    mod source_navigation;
    use std::thread;
    use std::time::Duration;

    use lsp_server::{Message, RequestId};

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
            Self::start_with_capabilities(&json!({}))
        }

        fn start_with_capabilities(capabilities: &Value) -> Self {
            let (server, client) = Connection::memory();
            let server_thread = thread::spawn(move || run(&server));
            let mut transcript = Self {
                client,
                server_thread: Some(server_thread),
                next_id: 1,
            };
            let response = transcript.request("initialize", json!({"capabilities": capabilities}));
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
    fn beta6_functions_keep_source_navigation_and_vocabulary() {
        let mut transcript = Transcript::start();
        transcript.notify(
            "textDocument/didOpen",
            json!({
                "textDocument": {
                    "uri": URI, "languageId": "locus", "version": 1,
                    "text": "locus 0 beta 6\nfn dot() {\n    point P()\n}\ndot example()\n\nrel \n"
                }
            }),
        );
        let _ = transcript.diagnostics();
        let definition = transcript.request("textDocument/definition", text_position(URI, 4, 1));
        assert_eq!(definition["range"]["start"]["line"], 1);
        let hover = transcript.request("textDocument/hover", text_position(URI, 4, 1));
        assert_eq!(hover["contents"]["value"], "fn dot()");
        let statements = transcript.request("textDocument/completion", text_position(URI, 5, 0));
        for name in ["fn", "group", "dot"] {
            assert!(completion_contains(&statements, name), "missing {name}");
        }
        let predicates = transcript.request("textDocument/completion", text_position(URI, 6, 4));
        for name in ["center", "align_center_x", "row", "contain", "avoid"] {
            assert!(completion_contains(&predicates, name), "missing {name}");
        }
        for name in ["coincident", "horizontal", "vertical", "congruent"] {
            assert!(!completion_contains(&predicates, name), "removed {name}");
        }
        let symbols = transcript.request(
            "textDocument/documentSymbol",
            json!({"textDocument": {"uri": URI}}),
        );
        assert!(
            symbols
                .as_array()
                .is_some_and(|items| items.iter().any(|item| item["name"] == "dot"))
        );
        transcript.finish();
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

    fn diagnostic_codes(params: &PublishDiagnosticsParams) -> Vec<&str> {
        params
            .diagnostics
            .iter()
            .filter_map(|diagnostic| match diagnostic.code.as_ref() {
                Some(lsp_types::NumberOrString::String(code)) => Some(code.as_str()),
                Some(lsp_types::NumberOrString::Number(_)) | None => None,
            })
            .collect()
    }
}
