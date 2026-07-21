//! Minimal fake DAP adapter for CI (WS-A-D Phase 2 B1).
//!
//! Speaks enough DAP over stdio for:
//! - `initialize` → response + `initialized` event
//! - `disconnect` → response + exit
//!
//! Not a real debugger. Real CodeLLDB / lldb-dap is B3.

use std::io::{self, BufRead, BufReader, Write};

use serde_json::{Value, json};

fn main() {
    let stdin = io::stdin();
    let mut reader = BufReader::new(stdin.lock());
    let mut stdout = io::stdout().lock();

    while let Ok(msg) = read_message(&mut reader) {
        let method = msg
            .get("method")
            .and_then(|m| m.as_str())
            .unwrap_or("")
            .to_string();
        let id = msg.get("id").cloned();

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
            "disconnect" | "terminate" => {
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
