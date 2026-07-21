//! Minimal fake DAP adapter for CI (WS-A-D Phase 2 B1/B2).
//!
//! Speaks enough DAP over stdio for:
//! - `initialize` → response + `initialized` event
//! - `setBreakpoints` → verified breakpoints
//! - `launch` / `configurationDone` → `stopped` (entry)
//! - `threads` / `stackTrace` / `scopes` / `variables`
//! - `next` / `stepIn` / `stepOut` / `continue` / `pause` → `stopped` or `continued`
//! - `disconnect` → response + exit
//!
//! Not a real debugger. Real CodeLLDB / lldb-dap is B3.

use std::io::{self, BufRead, BufReader, Write};

use serde_json::{Value, json};

fn main() {
    let stdin = io::stdin();
    let mut reader = BufReader::new(stdin.lock());
    let mut stdout = io::stdout().lock();
    let mut stopped = false;

    while let Ok(msg) = read_message(&mut reader) {
        let method = msg
            .get("method")
            .and_then(|m| m.as_str())
            .unwrap_or("")
            .to_string();
        let id = msg.get("id").cloned();
        let params = msg.get("params").cloned().unwrap_or(json!({}));

        match method.as_str() {
            "initialize" => {
                if let Some(id) = id {
                    write_message(
                        &mut stdout,
                        &json!({
                            "jsonrpc": "2.0",
                            "id": id,
                            "result": {
                                "supportsConfigurationDoneRequest": true,
                                "supportsSetVariable": false
                            }
                        }),
                    );
                }
                write_message(
                    &mut stdout,
                    &json!({
                        "jsonrpc": "2.0",
                        "method": "initialized",
                        "params": {}
                    }),
                );
            }
            "setBreakpoints" => {
                let source = params.get("source").cloned().unwrap_or(json!({}));
                let lines = params
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
                if let Some(id) = id {
                    write_message(
                        &mut stdout,
                        &json!({
                            "jsonrpc": "2.0",
                            "id": id,
                            "result": { "breakpoints": breakpoints }
                        }),
                    );
                }
            }
            "launch" | "attach" => {
                if let Some(id) = id {
                    write_message(
                        &mut stdout,
                        &json!({
                            "jsonrpc": "2.0",
                            "id": id,
                            "result": {}
                        }),
                    );
                }
            }
            "configurationDone" => {
                if let Some(id) = id {
                    write_message(
                        &mut stdout,
                        &json!({
                            "jsonrpc": "2.0",
                            "id": id,
                            "result": {}
                        }),
                    );
                }
                stopped = true;
                write_message(
                    &mut stdout,
                    &json!({
                        "jsonrpc": "2.0",
                        "method": "stopped",
                        "params": {
                            "reason": "entry",
                            "threadId": 1,
                            "allThreadsStopped": true
                        }
                    }),
                );
            }
            "threads" => {
                if let Some(id) = id {
                    write_message(
                        &mut stdout,
                        &json!({
                            "jsonrpc": "2.0",
                            "id": id,
                            "result": {
                                "threads": [{ "id": 1, "name": "main" }]
                            }
                        }),
                    );
                }
            }
            "stackTrace" => {
                if let Some(id) = id {
                    write_message(
                        &mut stdout,
                        &json!({
                            "jsonrpc": "2.0",
                            "id": id,
                            "result": {
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
                            }
                        }),
                    );
                }
            }
            "scopes" => {
                if let Some(id) = id {
                    write_message(
                        &mut stdout,
                        &json!({
                            "jsonrpc": "2.0",
                            "id": id,
                            "result": {
                                "scopes": [{
                                    "name": "Locals",
                                    "variablesReference": 1,
                                    "expensive": false
                                }]
                            }
                        }),
                    );
                }
            }
            "variables" => {
                if let Some(id) = id {
                    write_message(
                        &mut stdout,
                        &json!({
                            "jsonrpc": "2.0",
                            "id": id,
                            "result": {
                                "variables": [{
                                    "name": "count",
                                    "value": "42",
                                    "type": "i32",
                                    "variablesReference": 0
                                }]
                            }
                        }),
                    );
                }
            }
            "next" | "stepIn" | "stepOut" => {
                if let Some(id) = id {
                    write_message(
                        &mut stdout,
                        &json!({
                            "jsonrpc": "2.0",
                            "id": id,
                            "result": {}
                        }),
                    );
                }
                stopped = true;
                write_message(
                    &mut stdout,
                    &json!({
                        "jsonrpc": "2.0",
                        "method": "stopped",
                        "params": {
                            "reason": "step",
                            "threadId": 1,
                            "allThreadsStopped": true
                        }
                    }),
                );
            }
            "continue" => {
                if let Some(id) = id {
                    write_message(
                        &mut stdout,
                        &json!({
                            "jsonrpc": "2.0",
                            "id": id,
                            "result": { "allThreadsContinued": true }
                        }),
                    );
                }
                stopped = false;
                write_message(
                    &mut stdout,
                    &json!({
                        "jsonrpc": "2.0",
                        "method": "continued",
                        "params": {
                            "threadId": 1,
                            "allThreadsContinued": true
                        }
                    }),
                );
            }
            "pause" => {
                if let Some(id) = id {
                    write_message(
                        &mut stdout,
                        &json!({
                            "jsonrpc": "2.0",
                            "id": id,
                            "result": {}
                        }),
                    );
                }
                stopped = true;
                write_message(
                    &mut stdout,
                    &json!({
                        "jsonrpc": "2.0",
                        "method": "stopped",
                        "params": {
                            "reason": "pause",
                            "threadId": 1,
                            "allThreadsStopped": true
                        }
                    }),
                );
            }
            "disconnect" | "terminate" => {
                let _ = stopped;
                if let Some(id) = id {
                    write_message(
                        &mut stdout,
                        &json!({
                            "jsonrpc": "2.0",
                            "id": id,
                            "result": {}
                        }),
                    );
                }
                break;
            }
            "" => {
                // Response from client — ignore.
            }
            other => {
                if let Some(id) = id {
                    write_message(
                        &mut stdout,
                        &json!({
                            "jsonrpc": "2.0",
                            "id": id,
                            "error": {
                                "code": -32601,
                                "message": format!("method not found: {other}")
                            }
                        }),
                    );
                }
            }
        }
    }
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
