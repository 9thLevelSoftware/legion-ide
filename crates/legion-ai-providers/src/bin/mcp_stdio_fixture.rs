//! Minimal stdio MCP JSON-RPC server used as a deterministic test fixture.
//!
//! This replaces the previous inline `python3` script so the stdio conformance
//! tests have no external runtime dependency. It is built by Cargo and located
//! by integration tests via `CARGO_BIN_EXE_mcp_stdio_fixture`.
//!
//! Usage: `mcp_stdio_fixture <mode> <spec-json>` where `<mode>` is either
//! `conformance` or `pid`. The process reads one JSON-RPC request per line on
//! stdin and writes one JSON-RPC response per line on stdout.

use std::io::{self, BufRead, Write};

use serde_json::{Value, json};

fn main() {
    let mode = std::env::args().nth(1).unwrap_or_default();
    let spec: Value = std::env::args()
        .nth(2)
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or(Value::Null);

    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut out = stdout.lock();

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(line) => line,
            Err(_) => break,
        };
        if line.trim().is_empty() {
            continue;
        }
        let request: Value = match serde_json::from_str(&line) {
            Ok(value) => value,
            Err(_) => continue,
        };
        let method = request["method"].as_str().unwrap_or_default();
        let result = match mode.as_str() {
            "pid" => pid_result(&spec, method),
            _ => conformance_result(&spec, method, &request),
        };
        let response = json!({
            "jsonrpc": "2.0",
            "id": request["id"].clone(),
            "result": result,
        });
        writeln!(out, "{response}").expect("write response");
        out.flush().expect("flush response");
    }
}

fn conformance_result(spec: &Value, method: &str, request: &Value) -> Value {
    match method {
        "tools/list" => {
            let tools = spec
                .get("reloaded_tool")
                .cloned()
                .unwrap_or_else(|| json!([spec["tool"].clone()]));
            json!({ "tools": tools })
        }
        "resources/list" => {
            let resources = spec
                .get("reloaded_resource")
                .cloned()
                .unwrap_or_else(|| json!([spec["resource"].clone()]));
            json!({ "resources": resources })
        }
        "prompts/list" => {
            let prompts = spec
                .get("reloaded_prompt")
                .cloned()
                .unwrap_or_else(|| json!([spec["prompt"].clone()]));
            json!({ "prompts": prompts })
        }
        "tools/call" => json!({
            "content": [{
                "type": "text",
                "text": format!("called:{}", request["params"]["name"].as_str().unwrap_or_default()),
            }]
        }),
        "resources/read" => json!({
            "contents": [{
                "uri": request["params"]["uri"].clone(),
                "mimeType": "application/json",
                "text": "{\"ok\":true}",
            }]
        }),
        "prompts/get" => json!({
            "messages": [{
                "role": "assistant",
                "content": {
                    "type": "text",
                    "text": format!("prompt:{}", request["params"]["name"].as_str().unwrap_or_default()),
                },
            }]
        }),
        other => json!({ "echo_method": other }),
    }
}

fn pid_result(spec: &Value, method: &str) -> Value {
    match method {
        "tools/list" => json!({ "tools": spec["tools"].clone() }),
        "resources/list" => json!({ "resources": spec["resources"].clone() }),
        "prompts/list" => json!({ "prompts": spec["prompts"].clone() }),
        "tools/call" => json!({
            "content": [{
                "type": "text",
                "text": format!("pid:{}", std::process::id()),
            }]
        }),
        other => json!({ "echo_method": other }),
    }
}
