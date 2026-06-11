//! Scripted mock Language Server Protocol (LSP) server for legion-lsp contract tests.
//!
//! Reads Content-Length framed JSON-RPC 2.0 requests from stdin and writes framed
//! responses/notification events to stdout. The mock is intentionally minimal: it only
//! implements the subset required by the WS03 framer / correlation / process
//! supervision and read-side projection slices. It does not perform real document
//! analysis, rename, or any edit-producing feature.
//!
//! Behavior is driven by request methods:
//!
//! - `initialize` -> respond with a `ServerCapabilities` value whose shape matches
//!   the LSP `initialize` result so the caller can verify the correlation slice.
//! - `shutdown` -> respond with `null` and continue to accept `exit`.
//! - `exit` -> process exits with status code 0.
//! - any other method -> respond with a `Mock{op}` JSON object so the integration
//!   test can assert the out-of-order correlation behavior using distinct payloads
//!   per JSON-RPC id without re-using the same response content.
//!
//! The mock also emits optional startup notifications:
//! - `MOCK_LSP_EMIT_PROGRESS=1` sends one `$/progress` notification.
//! - `MOCK_LSP_EMIT_DIAGNOSTICS=1` sends one `textDocument/publishDiagnostics`
//!   notification containing a sentinel message used by tests to prove the session
//!   stores metadata only, not raw diagnostic/source text.
//!
//! No raw source payloads are ever sent. Only the metadata-only request id,
//! method, and a fixed-shape response body appear in mock output.

#![forbid(unsafe_code)]

use std::io::{self, BufRead, Read, Write};

use serde_json::{Value, json};

/// LSP `Content-Length` frame header terminator.
const HEADER_SEPARATOR: &str = "\r\n\r\n";

fn main() {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut input = stdin.lock();
    let mut output = stdout.lock();

    if std::env::var("MOCK_LSP_EMIT_PROGRESS").as_deref() == Ok("1") {
        // Notification (no `id`) framed as a single message so the consumer can
        // observe a notification pre-arrival without depending on it.
        let progress = json!({
            "jsonrpc": "2.0",
            "method": "$/progress",
            "params": {
                "token": "mock-init",
                "value": {"kind": "begin", "title": "mock"},
            },
        });
        let _ = write_frame(&mut output, &progress);
        let _ = output.flush();
    }

    if std::env::var("MOCK_LSP_EMIT_DIAGNOSTICS").as_deref() == Ok("1") {
        let diagnostics = json!({
            "jsonrpc": "2.0",
            "method": "textDocument/publishDiagnostics",
            "params": {
                "uri": "file:///workspace/src/main.rs",
                "diagnostics": [{
                    "range": {
                        "start": {"line": 1, "character": 4},
                        "end": {"line": 1, "character": 9}
                    },
                    "severity": 1,
                    "code": "E9999",
                    "source": "mock-lsp",
                    "message": "SECRET_DIAGNOSTIC_BODY must not be stored"
                }]
            }
        });
        let _ = write_frame(&mut output, &diagnostics);
        let _ = output.flush();
    }

    loop {
        let frame = match read_frame(&mut input) {
            Ok(frame) => frame,
            Err(MockIoError::Eof) => {
                // Parent closed stdin; exit cleanly so the test harness sees
                // a successful shutdown rather than a panic.
                return;
            }
            Err(err) => {
                eprintln!("mock_lsp_server: read error: {err}");
                std::process::exit(2);
            }
        };
        let envelope: Value = match serde_json::from_slice(&frame) {
            Ok(value) => value,
            Err(err) => {
                eprintln!("mock_lsp_server: invalid JSON: {err}");
                std::process::exit(2);
            }
        };
        let id = envelope.get("id").and_then(Value::as_u64);
        let method = envelope.get("method").and_then(Value::as_str).unwrap_or("");

        let response = match method {
            "initialize" => Some(json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "capabilities": {
                        "textDocumentSync": {"openClose": true, "change": 1},
                        "hoverProvider": true,
                        "definitionProvider": true,
                    },
                    "serverInfo": {
                        "name": "mock-lsp-server",
                        "version": "0.1.0",
                    },
                },
            })),
            "shutdown" => Some(json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": Value::Null,
            })),
            "exit" => {
                // Acknowledge `exit` only if the test sent a numeric id; the
                // LSP spec says `exit` is a notification, but the mock mirrors
                // both shapes so the harness can call `exit` with a stray id
                // in negative tests.
                if let Some(id) = id {
                    let _ = write_frame(
                        &mut output,
                        &json!({"jsonrpc": "2.0", "id": id, "result": Value::Null}),
                    );
                    let _ = output.flush();
                }
                return;
            }
            "$/cancelRequest" => None,
            "mock.echo" => {
                let params = envelope.get("params").cloned().unwrap_or(Value::Null);
                Some(json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {"echo": params},
                }))
            }
            "mock.noise" => {
                // Inject a notification before the response to exercise the
                // consumer's framing/buffering of intermediate messages.
                let _ = write_frame(
                    &mut output,
                    &json!({
                        "jsonrpc": "2.0",
                        "method": "mock/noise",
                        "params": {"ack": id},
                    }),
                );
                let _ = output.flush();
                Some(json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {"noise": id},
                }))
            }
            "textDocument/completion" => Some(json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "isIncomplete": false,
                    "items": [{
                        "label": "mockCompletion",
                        "detail": "fn mockCompletion() -> ()",
                        "kind": 3,
                        "insertText": "mockCompletion()"
                    }]
                }
            })),
            "textDocument/hover" => Some(json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "contents": {"kind": "markdown", "value": "fn mockCompletion() -> ()"},
                    "range": {
                        "start": {"line": 0, "character": 7},
                        "end": {"line": 0, "character": 21}
                    }
                }
            })),
            "textDocument/definition" => Some(json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "uri": "file:///workspace/src/main.rs",
                    "range": {
                        "start": {"line": 0, "character": 7},
                        "end": {"line": 0, "character": 21}
                    }
                }
            })),
            "textDocument/references" => Some(json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": [
                    {
                        "uri": "file:///workspace/src/main.rs",
                        "range": {
                            "start": {"line": 0, "character": 7},
                            "end": {"line": 0, "character": 21}
                        }
                    },
                    {
                        "targetUri": "file:///workspace/src/caller.rs",
                        "targetSelectionRange": {
                            "start": {"line": 2, "character": 4},
                            "end": {"line": 2, "character": 18}
                        }
                    }
                ]
            })),
            other => {
                // Surface a JSON-RPC error for unknown methods so the
                // consumer can map it through the standard error path.
                Some(json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "error": {"code": -32601, "message": format!("mock: unknown method: {other}")},
                }))
            }
        };

        if let Some(response) = response {
            if write_frame(&mut output, &response).is_err() {
                return;
            }
            let _ = output.flush();
        }
    }
}

#[derive(Debug)]
enum MockIoError {
    Io(io::Error),
    /// Peer closed the stream cleanly.
    Eof,
    /// Header was malformed.
    Protocol(String),
}

impl std::fmt::Display for MockIoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MockIoError::Io(err) => write!(f, "io: {err}"),
            MockIoError::Eof => write!(f, "eof"),
            MockIoError::Protocol(message) => write!(f, "protocol: {message}"),
        }
    }
}

impl From<io::Error> for MockIoError {
    fn from(err: io::Error) -> Self {
        if err.kind() == io::ErrorKind::UnexpectedEof {
            MockIoError::Eof
        } else {
            MockIoError::Io(err)
        }
    }
}

fn read_frame<R: Read + BufRead>(reader: &mut R) -> Result<Vec<u8>, MockIoError> {
    // Read headers byte-by-byte until we see the separator.
    let mut header = Vec::with_capacity(128);
    let mut byte = [0u8; 1];
    loop {
        let read = reader.read(&mut byte)?;
        match read {
            0 => return Err(MockIoError::Eof),
            1 => {
                header.push(byte[0]);
                if header.ends_with(HEADER_SEPARATOR.as_bytes()) {
                    break;
                }
                if header.len() > 16 * 1024 {
                    return Err(MockIoError::Protocol("header too large".to_string()));
                }
            }
            _ => return Err(MockIoError::Protocol("short read returned >1".to_string())),
        }
    }
    let header_str = std::str::from_utf8(&header[..header.len() - HEADER_SEPARATOR.len()])
        .map_err(|err| MockIoError::Protocol(format!("header utf-8: {err}")))?;
    let length: usize = header_str
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("Content-Length").then_some(value)
        })
        .ok_or_else(|| MockIoError::Protocol("missing Content-Length".to_string()))?
        .trim()
        .parse()
        .map_err(|err| MockIoError::Protocol(format!("invalid Content-Length: {err}")))?;

    let mut payload = vec![0u8; length];
    reader.read_exact(&mut payload)?;
    Ok(payload)
}

fn write_frame<W: Write>(writer: &mut W, value: &Value) -> io::Result<()> {
    let payload =
        serde_json::to_vec(value).map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
    writer.write_all(format!("Content-Length: {}\r\n\r\n", payload.len()).as_bytes())?;
    writer.write_all(&payload)?;
    Ok(())
}
