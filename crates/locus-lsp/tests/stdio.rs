//! Real-process tests for the stdio transport lifecycle.
#![allow(clippy::expect_used, clippy::panic, reason = "test assertions")]

use std::io::{BufRead as _, BufReader, Read as _, Write as _};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

#[test]
fn stdio_shutdown_exits_successfully() {
    let mut child = spawn_server();
    initialize(&mut child);
    send(
        &mut child,
        &json!({"jsonrpc": "2.0", "method": "initialized", "params": {}}),
    );
    send(
        &mut child,
        &json!({"jsonrpc": "2.0", "id": 2, "method": "shutdown", "params": null}),
    );
    let response = receive(&mut child);
    assert_eq!(response["id"], 2);
    assert_eq!(response["result"], Value::Null);
    send(
        &mut child,
        &json!({"jsonrpc": "2.0", "method": "exit", "params": null}),
    );
    drop(child.stdin.take());
    assert!(wait_for_exit(&mut child).success());
}

#[test]
fn stdio_exit_without_shutdown_is_unsuccessful() {
    let mut child = spawn_server();
    initialize(&mut child);
    send(
        &mut child,
        &json!({"jsonrpc": "2.0", "method": "initialized", "params": {}}),
    );
    send(
        &mut child,
        &json!({"jsonrpc": "2.0", "method": "exit", "params": null}),
    );
    drop(child.stdin.take());
    assert!(!wait_for_exit(&mut child).success());
}

#[test]
fn deeply_nested_document_reports_a_diagnostic_and_keeps_the_server_alive() {
    let mut child = spawn_server();
    initialize(&mut child);
    send(
        &mut child,
        &json!({"jsonrpc": "2.0", "method": "initialized", "params": {}}),
    );
    let source = format!(
        "locus 0 beta 6\nassert {}true{}\n",
        "(".repeat(5000),
        ")".repeat(5000)
    );
    send(
        &mut child,
        &json!({"jsonrpc": "2.0", "method": "textDocument/didOpen", "params": {
            "textDocument": {"uri": "file:///depth-limit.locus", "languageId": "locus", "version": 1, "text": source}
        }}),
    );
    let response = receive(&mut child);
    assert_eq!(response["method"], "textDocument/publishDiagnostics");
    assert_eq!(response["params"]["diagnostics"][0]["code"], "E161");
    send(
        &mut child,
        &json!({"jsonrpc": "2.0", "id": 2, "method": "shutdown", "params": null}),
    );
    let response = receive(&mut child);
    assert_eq!(response["id"], 2);
    send(
        &mut child,
        &json!({"jsonrpc": "2.0", "method": "exit", "params": null}),
    );
    drop(child.stdin.take());
    assert!(wait_for_exit(&mut child).success());
}

fn spawn_server() -> Child {
    Command::new(env!("CARGO_BIN_EXE_locus-lsp"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn locus-lsp")
}

fn initialize(child: &mut Child) {
    send(
        child,
        &json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {"capabilities": {}}
        }),
    );
    let response = receive(child);
    assert_eq!(response["id"], 1);
    assert!(response["result"]["capabilities"].is_object());
}

fn send(child: &mut Child, message: &Value) {
    let body = serde_json::to_vec(message).expect("serialize request");
    let input = child.stdin.as_mut().expect("server stdin");
    write!(input, "Content-Length: {}\r\n\r\n", body.len()).expect("write header");
    input.write_all(&body).expect("write body");
    input.flush().expect("flush request");
}

fn receive(child: &mut Child) -> Value {
    let output = child.stdout.as_mut().expect("server stdout");
    let mut reader = BufReader::new(output);
    let mut length = None;
    let mut line = String::new();
    loop {
        line.clear();
        assert!(reader.read_line(&mut line).expect("read header") > 0);
        if line == "\r\n" {
            break;
        }
        if let Some(value) = line.strip_prefix("Content-Length: ") {
            length = Some(value.trim().parse::<usize>().expect("content length"));
        }
    }
    let mut body = vec![0; length.expect("Content-Length header")];
    reader.read_exact(&mut body).expect("read response body");
    serde_json::from_slice(&body).expect("parse response")
}

fn wait_for_exit(child: &mut Child) -> ExitStatus {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        if let Some(status) = child.try_wait().expect("poll child") {
            return status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("locus-lsp did not exit within two seconds");
        }
        thread::sleep(Duration::from_millis(10));
    }
}
