//! Minimal local MCP server dispatch for app-owned workbench capabilities.

use std::io::{BufRead, Write};

use legion_protocol::{McpServerId, McpToolDescriptor};
use serde_json::{Value, json};
use thiserror::Error;

/// Errors returned while serving the local MCP stdio stream.
#[derive(Debug, Error)]
pub enum McpServerError {
    /// The input or output stream failed.
    #[error("stdio transport failed: {0}")]
    Io(#[from] std::io::Error),
    /// A JSON message could not be decoded or encoded.
    #[error("invalid JSON message: {0}")]
    Json(#[from] serde_json::Error),
    /// An app-owned workbench callback rejected or failed an invocation.
    #[error("workbench callback failed: {0}")]
    Workbench(String),
}

/// Capability decision supplied by app-owned composition.
pub trait McpCapabilityGate {
    /// Return whether this tool call is approved for the supplied arguments.
    fn allows(&self, tool: &McpToolDescriptor, arguments: &Value) -> bool;
}

/// Conservative default gate; every tool call is denied.
#[derive(Debug, Clone, Copy, Default)]
pub struct DenyAllCapabilityGate;

impl McpCapabilityGate for DenyAllCapabilityGate {
    fn allows(&self, _tool: &McpToolDescriptor, _arguments: &Value) -> bool {
        false
    }
}

#[derive(Debug)]
struct Request {
    jsonrpc: String,
    id: Option<Value>,
    method: String,
    params: Value,
}

#[derive(Debug)]
struct Response {
    jsonrpc: &'static str,
    id: Option<Value>,
    result: Option<Value>,
    error: Option<ErrorObject>,
}

#[derive(Debug)]
struct ErrorObject {
    code: i32,
    message: String,
}

impl Response {
    fn result(id: Option<Value>, result: Value) -> Self {
        Self {
            jsonrpc: "2.0",
            id,
            result: Some(result),
            error: None,
        }
    }

    fn error(id: Option<Value>, code: i32, message: impl Into<String>) -> Self {
        Self {
            jsonrpc: "2.0",
            id,
            result: None,
            error: Some(ErrorObject {
                code,
                message: message.into(),
            }),
        }
    }

    fn into_value(self) -> Value {
        let mut value = json!({ "jsonrpc": self.jsonrpc, "id": self.id });
        if let Some(result) = self.result {
            value["result"] = result;
        }
        if let Some(error) = self.error {
            value["error"] = json!({ "code": error.code, "message": error.message });
        }
        value
    }
}

/// Local stdio MCP server with app-owned capability and proposal callbacks.
pub struct McpStdioServer<G, F> {
    server_id: McpServerId,
    tools: Vec<McpToolDescriptor>,
    gate: G,
    invoke: F,
}

impl<G, F> McpStdioServer<G, F>
where
    G: McpCapabilityGate,
    F: Fn(&McpToolDescriptor, Value) -> Result<Value, McpServerError>,
{
    /// Construct a server. No tool call is permitted unless `gate` allows it.
    pub fn new(server_id: McpServerId, tools: Vec<McpToolDescriptor>, gate: G, invoke: F) -> Self {
        Self {
            server_id,
            tools,
            gate,
            invoke,
        }
    }

    /// Serve newline-delimited JSON-RPC messages over the supplied stdio streams.
    pub fn serve<R: BufRead, W: Write>(
        &self,
        input: R,
        mut output: W,
    ) -> Result<(), McpServerError> {
        for line in input.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            let request: Value = match serde_json::from_str(&line) {
                Ok(request) => request,
                Err(error) => {
                    let response = Response::error(None, -32600, error.to_string());
                    writeln!(output, "{}", response.into_value())?;
                    continue;
                }
            };
            let request = match parse_request(request) {
                Ok(request) => request,
                Err(error) => {
                    writeln!(
                        output,
                        "{}",
                        Response::error(None, -32600, error).into_value()
                    )?;
                    continue;
                }
            };
            if request.jsonrpc != "2.0" {
                if request.id.is_some() {
                    let response = Response::error(request.id, -32600, "jsonrpc must be 2.0");
                    writeln!(output, "{}", response.into_value())?;
                }
                continue;
            }
            if let Some(response) = self.dispatch(request) {
                writeln!(output, "{}", response.into_value())?;
            }
        }
        Ok(())
    }

    fn dispatch(&self, request: Request) -> Option<Response> {
        let id = request.id.clone();
        match request.method.as_str() {
            "initialize" => Some(Response::result(
                id,
                json!({
                    "protocolVersion": "2025-11-25",
                    "capabilities": { "tools": { "listChanged": false } },
                    "serverInfo": { "name": self.server_id.0, "version": "0.1" }
                }),
            )),
            "tools/list" => Some(Response::result(
                id,
                json!({ "tools": self.tools.iter().map(tool_metadata).collect::<Vec<_>>() }),
            )),
            "tools/call" => Some(self.call_tool(id, request.params)),
            _ if id.is_some() => Some(Response::error(id, -32601, "method not found")),
            _ => None,
        }
    }

    fn call_tool(&self, id: Option<Value>, params: Value) -> Response {
        let Some(name) = params.get("name").and_then(Value::as_str) else {
            return Response::error(id, -32602, "tools/call requires a tool name");
        };
        let Some(tool) = self.tools.iter().find(|tool| tool.name.0 == name) else {
            return Response::error(id, -32602, "unknown tool");
        };
        let arguments = params
            .get("arguments")
            .cloned()
            .unwrap_or_else(|| json!({}));
        if !self.gate.allows(tool, &arguments) {
            return Response::error(id, -32001, "capability denied");
        }
        match (self.invoke)(tool, arguments) {
            Ok(result) => Response::result(id, result),
            Err(error) => Response::error(id, -32603, error.to_string()),
        }
    }
}

fn parse_request(value: Value) -> Result<Request, String> {
    let object = value
        .as_object()
        .ok_or_else(|| "request must be a JSON object".to_string())?;
    let jsonrpc = object
        .get("jsonrpc")
        .and_then(Value::as_str)
        .ok_or_else(|| "request missing jsonrpc".to_string())?
        .to_string();
    let method = object
        .get("method")
        .and_then(Value::as_str)
        .ok_or_else(|| "request missing method".to_string())?
        .to_string();
    Ok(Request {
        jsonrpc,
        id: object.get("id").cloned(),
        method,
        params: object.get("params").cloned().unwrap_or_else(|| json!({})),
    })
}

fn tool_metadata(tool: &McpToolDescriptor) -> Value {
    json!({
        "name": tool.name.0,
        "description": tool.description_label,
        "inputSchema": { "type": "object" }
    })
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;
    use std::sync::{Arc, Mutex};

    use legion_protocol::{
        CapabilityId, DelegatedTaskToolPermissionProfile, FileFingerprint, McpToolName,
        PermissionBudgetActionClass, ProposalRiskLabel, RedactionHint,
    };
    use serde_json::json;

    use super::{DenyAllCapabilityGate, McpCapabilityGate, McpServerError, McpStdioServer};

    #[derive(Debug, Clone, Copy)]
    struct AllowGate;

    impl McpCapabilityGate for AllowGate {
        fn allows(
            &self,
            _tool: &legion_protocol::McpToolDescriptor,
            _arguments: &serde_json::Value,
        ) -> bool {
            true
        }
    }

    fn tool() -> legion_protocol::McpToolDescriptor {
        legion_protocol::McpToolDescriptor {
            server_id: legion_protocol::McpServerId("legion-local".to_string()),
            name: McpToolName("workspace.propose_edit".to_string()),
            description_label: "Create an app-owned edit proposal".to_string(),
            input_schema_hash: FileFingerprint {
                algorithm: "sha256".to_string(),
                value: "schema".to_string(),
            },
            risk_label: ProposalRiskLabel::Medium,
            required_permission_profile: DelegatedTaskToolPermissionProfile::Write,
            action_class: PermissionBudgetActionClass::ProposeEdits,
            capability: CapabilityId("workspace.proposal".to_string()),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        }
    }

    fn server<G, F>(gate: G, invoke: F) -> McpStdioServer<G, F>
    where
        G: McpCapabilityGate,
        F: Fn(
            &legion_protocol::McpToolDescriptor,
            serde_json::Value,
        ) -> Result<serde_json::Value, McpServerError>,
    {
        McpStdioServer::new(
            legion_protocol::McpServerId("legion-local".to_string()),
            vec![tool()],
            gate,
            invoke,
        )
    }

    #[test]
    fn initialize_and_list_are_metadata_only() {
        let input = concat!(
            "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\"}\n",
            "{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/list\"}\n"
        );
        let mut output = Vec::new();
        server(DenyAllCapabilityGate, |_tool, _args| Ok(json!("unused")))
            .serve(Cursor::new(input), &mut output)
            .unwrap();
        let text = String::from_utf8(output).unwrap();
        assert!(text.contains("2025-11-25"));
        assert!(text.contains("workspace.propose_edit"));
        assert!(text.contains("inputSchema"));
    }

    #[test]
    fn default_gate_denies_without_invoking_callback() {
        let calls = Arc::new(Mutex::new(0));
        let callback_calls = Arc::clone(&calls);
        let input = "{\"jsonrpc\":\"2.0\",\"id\":3,\"method\":\"tools/call\",\"params\":{\"name\":\"workspace.propose_edit\"}}\n";
        let mut output = Vec::new();
        server(DenyAllCapabilityGate, move |_tool, _args| {
            *callback_calls.lock().unwrap() += 1;
            Ok(json!("should not run"))
        })
        .serve(Cursor::new(input), &mut output)
        .unwrap();
        assert_eq!(*calls.lock().unwrap(), 0);
        assert!(
            String::from_utf8(output)
                .unwrap()
                .contains("capability denied")
        );
    }

    #[test]
    fn approved_call_routes_arguments_to_callback() {
        let input = "{\"jsonrpc\":\"2.0\",\"id\":4,\"method\":\"tools/call\",\"params\":{\"name\":\"workspace.propose_edit\",\"arguments\":{\"path\":\"src/lib.rs\"}}}\n";
        let mut output = Vec::new();
        server(AllowGate, |tool, args| {
            assert_eq!(tool.name.0, "workspace.propose_edit");
            assert_eq!(args["path"], "src/lib.rs");
            Ok(json!({"proposal": "proposal-1"}))
        })
        .serve(Cursor::new(input), &mut output)
        .unwrap();
        let text = String::from_utf8(output).unwrap();
        assert!(text.contains("proposal-1"));
    }

    #[test]
    fn unknown_method_returns_json_rpc_error() {
        let input = "{\"jsonrpc\":\"2.0\",\"id\":5,\"method\":\"completion/complete\"}\n";
        let mut output = Vec::new();
        server(DenyAllCapabilityGate, |_tool, _args| Ok(json!(null)))
            .serve(Cursor::new(input), &mut output)
            .unwrap();
        let text = String::from_utf8(output).unwrap();
        assert!(text.contains("method not found"));
        assert!(!text.contains("completion"));
    }
}
