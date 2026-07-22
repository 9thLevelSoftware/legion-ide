//! Minimal fake DAP adapter for CI (WS-A-D Phase 2 B1/B2/B4).
//!
//! Speaks **Microsoft DAP** over stdio (`seq` / `type` / `command` / `arguments`)
//! for:
//! - `initialize` → response + `initialized` event
//! - `setBreakpoints` → verified breakpoints
//! - `launch` / `configurationDone` → `stopped` (entry)
//! - `threads` / `stackTrace` / `scopes` / `variables`
//! - `next` / `stepIn` / `stepOut` / `continue` / `pause` → `stopped` or `continued`
//! - `disconnect` → response + exit
//!
//! Contract stand-in for real CodeLLDB / `lldb-dap` wire shape.

use std::io::{self, BufRead, BufReader, Write};

use serde_json::{Value, json};

fn main() {
    let stdin = io::stdin();
    let mut reader = BufReader::new(stdin.lock());
    let mut stdout = io::stdout().lock();
    let mut out_seq = 1u64;
    let mut stopped = false;

    while let Ok(msg) = read_message(&mut reader) {
        let msg_type = msg
            .get("type")
            .and_then(|t| t.as_str())
            .unwrap_or("")
            .to_string();
        if msg_type != "request" {
            continue;
        }
        let command = msg
            .get("command")
            .and_then(|c| c.as_str())
            .unwrap_or("")
            .to_string();
        let request_seq = msg.get("seq").and_then(|s| s.as_u64()).unwrap_or(0);
        let arguments = msg.get("arguments").cloned().unwrap_or(json!({}));

        match command.as_str() {
            "initialize" => {
                write_response(
                    &mut stdout,
                    &mut out_seq,
                    request_seq,
                    "initialize",
                    true,
                    json!({
                        "supportsConfigurationDoneRequest": true,
                        "supportsSetVariable": false
                    }),
                );
                write_event(&mut stdout, &mut out_seq, "initialized", json!({}));
            }
            "setBreakpoints" => {
                let source = arguments.get("source").cloned().unwrap_or(json!({}));
                let lines = arguments
                    .get("breakpoints")
                    .and_then(|b| b.as_array())
                    .cloned()
                    .unwrap_or_default();
                let breakpoints: Vec<Value> = lines
                    .iter()
                    .enumerate()
                    .map(|(i, bp)| {
                        let line = bp.get("line").and_then(|l| l.as_u64()).unwrap_or(1);
                        json!({
                            "id": i + 1,
                            "verified": true,
                            "line": line,
                            "source": source,
                            "message": "fake adapter verified"
                        })
                    })
                    .collect();
                write_response(
                    &mut stdout,
                    &mut out_seq,
                    request_seq,
                    "setBreakpoints",
                    true,
                    json!({ "breakpoints": breakpoints }),
                );
            }
            "launch" | "attach" => {
                write_response(
                    &mut stdout,
                    &mut out_seq,
                    request_seq,
                    &command,
                    true,
                    json!({}),
                );
            }
            "configurationDone" => {
                write_response(
                    &mut stdout,
                    &mut out_seq,
                    request_seq,
                    "configurationDone",
                    true,
                    json!({}),
                );
                stopped = true;
                write_event(
                    &mut stdout,
                    &mut out_seq,
                    "stopped",
                    json!({
                        "reason": "entry",
                        "threadId": 1,
                        "allThreadsStopped": true
                    }),
                );
            }
            "threads" => {
                write_response(
                    &mut stdout,
                    &mut out_seq,
                    request_seq,
                    "threads",
                    true,
                    json!({
                        "threads": [{ "id": 1, "name": "main" }]
                    }),
                );
            }
            "stackTrace" => {
                write_response(
                    &mut stdout,
                    &mut out_seq,
                    request_seq,
                    "stackTrace",
                    true,
                    json!({
                        "stackFrames": [{
                            "id": 1,
                            "name": "main",
                            "line": 10,
                            "column": 1,
                            "source": {
                                "name": "main.rs",
                                "path": "src/main.rs"
                            }
                        }],
                        "totalFrames": 1
                    }),
                );
            }
            "scopes" => {
                write_response(
                    &mut stdout,
                    &mut out_seq,
                    request_seq,
                    "scopes",
                    true,
                    json!({
                        "scopes": [{
                            "name": "Locals",
                            "variablesReference": 1,
                            "expensive": false
                        }]
                    }),
                );
            }
            "variables" => {
                write_response(
                    &mut stdout,
                    &mut out_seq,
                    request_seq,
                    "variables",
                    true,
                    json!({
                        "variables": [{
                            "name": "count",
                            "value": "42",
                            "type": "i32",
                            "variablesReference": 0
                        }]
                    }),
                );
            }
            "next" | "stepIn" | "stepOut" => {
                write_response(
                    &mut stdout,
                    &mut out_seq,
                    request_seq,
                    &command,
                    true,
                    json!({}),
                );
                stopped = true;
                write_event(
                    &mut stdout,
                    &mut out_seq,
                    "stopped",
                    json!({
                        "reason": "step",
                        "threadId": 1,
                        "allThreadsStopped": true
                    }),
                );
            }
            "continue" => {
                write_response(
                    &mut stdout,
                    &mut out_seq,
                    request_seq,
                    "continue",
                    true,
                    json!({ "allThreadsContinued": true }),
                );
                stopped = false;
                write_event(
                    &mut stdout,
                    &mut out_seq,
                    "continued",
                    json!({
                        "threadId": 1,
                        "allThreadsContinued": true
                    }),
                );
                // B6 contract: simulate hitting the next breakpoint so product
                // continue-until-stopped can re-project a paused stack in CI.
                stopped = true;
                write_event(
                    &mut stdout,
                    &mut out_seq,
                    "stopped",
                    json!({
                        "reason": "breakpoint",
                        "threadId": 1,
                        "allThreadsStopped": true
                    }),
                );
            }
            "pause" => {
                write_response(
                    &mut stdout,
                    &mut out_seq,
                    request_seq,
                    "pause",
                    true,
                    json!({}),
                );
                stopped = true;
                write_event(
                    &mut stdout,
                    &mut out_seq,
                    "stopped",
                    json!({
                        "reason": "pause",
                        "threadId": 1,
                        "allThreadsStopped": true
                    }),
                );
            }
            "disconnect" | "terminate" => {
                let _ = stopped;
                write_response(
                    &mut stdout,
                    &mut out_seq,
                    request_seq,
                    &command,
                    true,
                    json!({}),
                );
                break;
            }
            other => {
                write_response(
                    &mut stdout,
                    &mut out_seq,
                    request_seq,
                    other,
                    false,
                    json!({}),
                );
                // success=false responses should use message field; rewrite:
            }
        }
    }
}

fn write_response<W: Write>(
    writer: &mut W,
    out_seq: &mut u64,
    request_seq: u64,
    command: &str,
    success: bool,
    body: Value,
) {
    let seq = *out_seq;
    *out_seq = out_seq.saturating_add(1);
    let mut msg = json!({
        "seq": seq,
        "type": "response",
        "request_seq": request_seq,
        "success": success,
        "command": command,
    });
    if success {
        msg["body"] = body;
    } else {
        msg["message"] = json!(format!("method not found: {command}"));
    }
    write_message(writer, &msg);
}

fn write_event<W: Write>(writer: &mut W, out_seq: &mut u64, event: &str, body: Value) {
    let seq = *out_seq;
    *out_seq = out_seq.saturating_add(1);
    write_message(
        writer,
        &json!({
            "seq": seq,
            "type": "event",
            "event": event,
            "body": body
        }),
    );
}

fn read_message<R: BufRead>(reader: &mut R) -> io::Result<Value> {
    let mut headers = String::new();
    loop {
        let mut line = String::new();
        let n = reader.read_line(&mut line)?;
        if n == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "EOF in headers",
            ));
        }
        if line == "\r\n" || line == "\n" {
            break;
        }
        headers.push_str(&line);
    }
    let length = headers
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("Content-Length").then_some(value)
        })
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing Content-Length"))?
        .trim()
        .parse::<usize>()
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
    let mut body = vec![0u8; length];
    reader.read_exact(&mut body)?;
    serde_json::from_slice(&body).map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))
}

fn write_message<W: Write>(writer: &mut W, value: &Value) {
    let body = serde_json::to_vec(value).expect("serialize");
    let _ = write!(writer, "Content-Length: {}\r\n\r\n", body.len());
    let _ = writer.write_all(&body);
    let _ = writer.flush();
}
