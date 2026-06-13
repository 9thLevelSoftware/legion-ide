use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;

use legion_ai_providers::{
    McpClient, StdioMcpTransport, StdioMcpTransportConfig, StreamableHttpMcpTransport,
    StreamableHttpMcpTransportConfig,
};
use legion_protocol::{
    CapabilityId, DelegatedTaskToolPermissionDecision, DelegatedTaskToolPermissionProfile,
    FileFingerprint, McpListChangedKind, McpPromptDescriptor, McpPromptName, McpRegistrySnapshot,
    McpResourceDescriptor, McpResourceUri, McpServerDescriptor, McpServerId, McpToolDescriptor,
    McpToolName, McpTransportKind, PermissionBudgetActionClass, ProposalRiskLabel, RedactionHint,
    TimestampMillis,
};
use legion_security::{
    mcp_tool_permission_labels, mcp_tool_permission_request, mcp_tool_target_id,
};
use serde_json::{Value, json};

fn descriptor_registry(
    server_id: &str,
    transport_kind: McpTransportKind,
    endpoint_label: &str,
    tool_name: &str,
    resource_uri: &str,
    prompt_name: &str,
) -> McpRegistrySnapshot {
    let server_id_name = server_id.to_string();
    let server_id = McpServerId(server_id_name.clone());
    McpRegistrySnapshot {
        registry_id: format!("mcp-registry:{}:1", server_id_name),
        server: McpServerDescriptor {
            server_id: server_id.clone(),
            transport_kind,
            display_label: format!("{} server", server_id_name),
            endpoint_label: endpoint_label.to_string(),
            tools_list_changed: true,
            resources_list_changed: true,
            prompts_list_changed: true,
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        },
        tools: vec![McpToolDescriptor {
            server_id: server_id.clone(),
            name: McpToolName(tool_name.to_string()),
            description_label: format!("{tool_name} tool"),
            input_schema_hash: FileFingerprint {
                algorithm: "sha256".to_string(),
                value: format!("schema:{tool_name}"),
            },
            risk_label: ProposalRiskLabel::High,
            required_permission_profile: DelegatedTaskToolPermissionProfile::Write,
            action_class: PermissionBudgetActionClass::InvokeLocalTool,
            capability: CapabilityId("mcp.tool.call".to_string()),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        }],
        resources: vec![McpResourceDescriptor {
            server_id: server_id.clone(),
            uri: McpResourceUri(resource_uri.to_string()),
            name_label: format!("{resource_uri} resource"),
            mime_type_label: "application/json".to_string(),
            subscribable: true,
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        }],
        prompts: vec![McpPromptDescriptor {
            server_id,
            name: McpPromptName(prompt_name.to_string()),
            description_label: format!("{prompt_name} prompt"),
            argument_labels: vec!["scope".to_string()],
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        }],
        last_notification_kind: None,
        list_version: 1,
        generated_at: TimestampMillis(1),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    }
}

fn stdio_fixture_script() -> String {
    r#"
import json
import sys

spec = json.loads(sys.argv[1])
initial_tool = spec["tool"]
initial_resource = spec["resource"]
initial_prompt = spec["prompt"]
reloaded_tool = spec.get("reloaded_tool")
reloaded_resource = spec.get("reloaded_resource")
reloaded_prompt = spec.get("reloaded_prompt")
state = {"tools": 0, "resources": 0, "prompts": 0}

while True:
    line = sys.stdin.readline()
    if not line:
        break
    if not line.strip():
        continue
    request = json.loads(line)
    method = request["method"]
    if method == "tools/list":
        state["tools"] += 1
        if reloaded_tool is not None and state["tools"] >= 1:
            tools = reloaded_tool
        else:
            tools = [initial_tool]
        result = {"tools": tools}
    elif method == "resources/list":
        state["resources"] += 1
        if reloaded_resource is not None and state["resources"] >= 1:
            resources = reloaded_resource
        else:
            resources = [initial_resource]
        result = {"resources": resources}
    elif method == "prompts/list":
        state["prompts"] += 1
        if reloaded_prompt is not None and state["prompts"] >= 1:
            prompts = reloaded_prompt
        else:
            prompts = [initial_prompt]
        result = {"prompts": prompts}
    elif method == "tools/call":
        result = {
            "content": [
                {
                    "type": "text",
                    "text": "called:" + request["params"]["name"],
                }
            ]
        }
    elif method == "resources/read":
        result = {
            "contents": [
                {
                    "uri": request["params"]["uri"],
                    "mimeType": "application/json",
                    "text": "{\"ok\":true}",
                }
            ]
        }
    elif method == "prompts/get":
        result = {
            "messages": [
                {
                    "role": "assistant",
                    "content": {
                        "type": "text",
                        "text": "prompt:" + request["params"]["name"],
                    },
                }
            ]
        }
    else:
        result = {"echo_method": method}
    print(json.dumps({"jsonrpc": "2.0", "id": request["id"], "result": result}), flush=True)
"#
    .to_string()
}

fn stdio_transport(spec: &Value) -> StdioMcpTransport {
    StdioMcpTransport::new(StdioMcpTransportConfig {
        command: "python3".to_string(),
        args: vec![
            "-u".to_string(),
            "-c".to_string(),
            stdio_fixture_script(),
            spec.to_string(),
        ],
    })
}

fn read_http_request_json(stream: &mut TcpStream) -> Value {
    let mut buffer = Vec::new();
    let mut scratch = [0_u8; 1024];
    let header_end = loop {
        let read = stream.read(&mut scratch).expect("read request bytes");
        assert!(
            read > 0,
            "client closed HTTP connection before sending a request"
        );
        buffer.extend_from_slice(&scratch[..read]);
        if let Some(position) = buffer.windows(4).position(|window| window == b"\r\n\r\n") {
            break position + 4;
        }
    };
    let header_text = String::from_utf8(buffer[..header_end].to_vec()).expect("header utf8");
    let content_length = header_text
        .lines()
        .find_map(|line| {
            let lower = line.to_ascii_lowercase();
            if lower.starts_with("content-length:") {
                line.split_once(':')
                    .and_then(|(_, value)| value.trim().parse::<usize>().ok())
            } else {
                None
            }
        })
        .expect("content-length header");
    let mut body = buffer[header_end..].to_vec();
    while body.len() < content_length {
        let read = stream.read(&mut scratch).expect("read request body");
        assert!(
            read > 0,
            "client closed HTTP connection before sending request body"
        );
        body.extend_from_slice(&scratch[..read]);
    }
    serde_json::from_slice(&body[..content_length]).expect("parse HTTP JSON request")
}

fn write_http_response(stream: &mut TcpStream, response: &Value) {
    let body = response.to_string();
    let payload = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );
    stream
        .write_all(payload.as_bytes())
        .expect("write HTTP response");
    stream.flush().expect("flush HTTP response");
}

fn spawn_http_fixture() -> (String, Arc<Mutex<Vec<Value>>>, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind HTTP fixture");
    let endpoint = format!("http://{}", listener.local_addr().expect("local addr"));
    let requests = Arc::new(Mutex::new(Vec::new()));
    let requests_thread = Arc::clone(&requests);
    let handle = thread::spawn(move || {
        for _ in 0..4 {
            let (mut stream, _) = listener.accept().expect("accept HTTP request");
            let request = read_http_request_json(&mut stream);
            let method = request["method"].as_str().expect("method string");
            requests_thread
                .lock()
                .expect("lock requests")
                .push(request.clone());
            let result = match method {
                "tools/list" => json!({
                    "tools": [
                        {
                            "name": "fetch_url",
                            "description": "fetch URL",
                            "inputSchema": {"type": "object", "properties": {"url": {"type": "string"}}}
                        }
                    ]
                }),
                "resources/list" => json!({
                    "resources": [
                        {
                            "uri": "web://index",
                            "name": "web index",
                            "mimeType": "text/html",
                            "subscribable": false
                        }
                    ]
                }),
                "prompts/list" => json!({
                    "prompts": [
                        {
                            "name": "summarize_web",
                            "description": "summarize web content",
                            "arguments": [{"name": "url"}]
                        }
                    ]
                }),
                "tools/call" => json!({
                    "content": [{"type": "text", "text": format!("called:{}", request["params"]["name"].as_str().expect("tool name")) }]
                }),
                other => panic!("unexpected HTTP method {other}"),
            };
            let response = json!({
                "jsonrpc": "2.0",
                "id": request["id"].clone(),
                "result": result,
            });
            write_http_response(&mut stream, &response);
        }
    });
    (endpoint, requests, handle)
}

fn assert_permission_audit(tool: &McpToolDescriptor, session_label: &str) {
    let labels = mcp_tool_permission_labels(session_label, tool);
    assert_eq!(labels[0], "automate.permission.mcp_tool_call");
    assert!(labels.contains(&session_label.to_string()));
    assert!(labels.contains(&format!("mcp.server:{}", tool.server_id.0)));
    assert!(labels.contains(&format!("mcp.tool:{}", tool.name.0)));
    assert!(labels.contains(&format!("mcp.capability:{}", tool.capability.0)));

    let request = mcp_tool_permission_request(
        "permission:mcp:allow",
        tool,
        DelegatedTaskToolPermissionDecision::Allow,
        session_label,
        1,
    );
    let expected_target = mcp_tool_target_id(&tool.server_id, &tool.name);
    assert_eq!(request.target_id.as_deref(), Some(expected_target.as_str()));
    assert_eq!(request.capability.as_ref(), Some(&tool.capability));
    assert!(request.runtime_allowed);
    assert!(!request.deny_overrides);
    assert!(request.human_approval_recorded);
    assert!(request.labels.contains(&session_label.to_string()));
}

#[test]
fn filesystem_class_stdio_reference_server_passes_conformance() {
    let registry = descriptor_registry(
        "mcp:filesystem",
        McpTransportKind::Stdio,
        "stdio:filesystem",
        "read_file",
        "file:///workspace/README.md",
        "workspace_summary",
    );
    let tool = registry.tools[0].clone();
    let resource = registry.resources[0].clone();
    let prompt = registry.prompts[0].clone();
    let client = McpClient::new(
        registry,
        stdio_transport(&json!({
            "tool": {
                "name": tool.name.0.clone(),
                "description": tool.description_label.clone(),
                "inputSchema": {"type": "object", "properties": {"path": {"type": "string"}}}
            },
            "resource": {
                "uri": resource.uri.0.clone(),
                "name": resource.name_label.clone(),
                "mimeType": resource.mime_type_label.clone(),
                "subscribable": resource.subscribable
            },
            "prompt": {
                "name": prompt.name.0.clone(),
                "description": prompt.description_label.clone(),
                "arguments": prompt.argument_labels.iter().map(|name| json!({"name": name})).collect::<Vec<_>>()
            }
        })),
    )
    .expect("valid filesystem-class client");

    let list_tools = client
        .list_tools("filesystem:tools:list")
        .expect("list tools");
    assert_eq!(list_tools["result"]["tools"][0]["name"], "read_file");
    let list_resources = client
        .list_resources("filesystem:resources:list")
        .expect("list resources");
    assert_eq!(
        list_resources["result"]["resources"][0]["uri"],
        "file:///workspace/README.md"
    );
    let list_prompts = client
        .list_prompts("filesystem:prompts:list")
        .expect("list prompts");
    assert_eq!(
        list_prompts["result"]["prompts"][0]["name"],
        "workspace_summary"
    );

    let permission = mcp_tool_permission_request(
        "permission:filesystem:allow",
        &tool,
        DelegatedTaskToolPermissionDecision::Allow,
        "workspace:filesystem",
        1,
    );
    let response = client
        .call_tool_with_permission(
            "filesystem:tool:call",
            &tool.server_id,
            &tool.name,
            json!({"path": "README.md"}),
            &permission,
        )
        .expect("approved filesystem tool call");
    assert_eq!(response["result"]["content"][0]["text"], "called:read_file");

    assert_permission_audit(&tool, "workspace:filesystem");
}

#[test]
fn web_class_streamable_http_reference_server_passes_conformance() {
    let registry = descriptor_registry(
        "mcp:web",
        McpTransportKind::StreamableHttp,
        "http://127.0.0.1",
        "fetch_url",
        "web://index",
        "summarize_web",
    );
    let tool = registry.tools[0].clone();
    let resource = registry.resources[0].clone();
    let prompt = registry.prompts[0].clone();
    let (endpoint, requests, handle) = spawn_http_fixture();
    let client = McpClient::new(
        registry,
        StreamableHttpMcpTransport::new(StreamableHttpMcpTransportConfig { endpoint }),
    )
    .expect("valid web-class client");

    let list_tools = client.list_tools("web:tools:list").expect("list tools");
    assert_eq!(list_tools["result"]["tools"][0]["name"], "fetch_url");
    let list_resources = client
        .list_resources("web:resources:list")
        .expect("list resources");
    assert_eq!(
        list_resources["result"]["resources"][0]["uri"],
        "web://index"
    );
    let list_prompts = client
        .list_prompts("web:prompts:list")
        .expect("list prompts");
    assert_eq!(
        list_prompts["result"]["prompts"][0]["name"],
        "summarize_web"
    );

    let permission = mcp_tool_permission_request(
        "permission:web:allow",
        &tool,
        DelegatedTaskToolPermissionDecision::Allow,
        "workspace:web",
        1,
    );
    let response = client
        .call_tool_with_permission(
            "web:tool:call",
            &tool.server_id,
            &tool.name,
            json!({"url": "https://example.com"}),
            &permission,
        )
        .expect("approved web tool call");
    assert_eq!(response["result"]["content"][0]["text"], "called:fetch_url");

    handle
        .join()
        .expect("web fixture exits after four requests");
    let requests = requests.lock().expect("lock requests");
    assert_eq!(requests.len(), 4);
    assert_eq!(requests[0]["method"], "tools/list");
    assert_eq!(requests[3]["method"], "tools/call");
    assert_permission_audit(&tool, "workspace:web");
    assert_eq!(resource.name_label, "web://index resource");
    assert_eq!(prompt.description_label, "summarize_web prompt");
}

#[test]
fn custom_stdio_reference_server_reloads_after_list_changed() {
    let registry = descriptor_registry(
        "mcp:custom",
        McpTransportKind::Stdio,
        "stdio:custom",
        "write_file",
        "workspace://metadata",
        "review_workspace",
    );
    let tool = registry.tools[0].clone();
    let transport = stdio_transport(&json!({
        "tool": {
            "name": tool.name.0.clone(),
            "description": tool.description_label.clone(),
            "inputSchema": {"type": "object", "properties": {"path": {"type": "string"}}}
        },
        "resource": {
            "uri": registry.resources[0].uri.0.clone(),
            "name": registry.resources[0].name_label.clone(),
            "mimeType": registry.resources[0].mime_type_label.clone(),
            "subscribable": registry.resources[0].subscribable
        },
        "prompt": {
            "name": registry.prompts[0].name.0.clone(),
            "description": registry.prompts[0].description_label.clone(),
            "arguments": registry.prompts[0].argument_labels.iter().map(|name| json!({"name": name})).collect::<Vec<_>>()
        },
        "reloaded_tool": [
            {
                "name": "write_file",
                "description": "write file after reload",
                "inputSchema": {"type": "object"}
            },
            {
                "name": "read_metadata",
                "description": "read metadata",
                "inputSchema": {"type": "object"}
            }
        ],
        "reloaded_resource": [
            {
                "uri": "workspace://metadata",
                "name": "workspace metadata",
                "mimeType": "application/json",
                "subscribable": true
            },
            {
                "uri": "workspace://status",
                "name": "workspace status",
                "mimeType": "application/json",
                "subscribable": false
            }
        ],
        "reloaded_prompt": [
            {
                "name": "review_workspace",
                "description": "review workspace after reload",
                "arguments": [{"name": "scope"}, {"name": "risk"}]
            }
        ]
    }));
    let mut client = McpClient::new(registry, transport).expect("valid custom client");

    let reloaded = client
        .reload_after_list_changed(
            McpListChangedKind::Tools,
            "custom:reload:tools",
            TimestampMillis(2),
        )
        .expect("reload after list changed");
    assert_eq!(reloaded.list_version, 2);
    assert_eq!(reloaded.last_notification_kind, None);
    assert!(
        reloaded
            .tools
            .iter()
            .any(|tool| tool.name.0 == "read_metadata")
    );
    assert_eq!(reloaded.tools.len(), 2);

    let reloaded = client
        .reload_after_list_changed(
            McpListChangedKind::Resources,
            "custom:reload:resources",
            TimestampMillis(3),
        )
        .expect("resource reload after list changed");
    assert_eq!(reloaded.resources.len(), 2);
    assert!(
        reloaded
            .resources
            .iter()
            .any(|resource| resource.uri.0 == "workspace://status")
    );

    let reloaded = client
        .reload_after_list_changed(
            McpListChangedKind::Prompts,
            "custom:reload:prompts",
            TimestampMillis(4),
        )
        .expect("prompt reload after list changed");
    assert_eq!(
        reloaded.prompts[0].argument_labels,
        vec!["scope".to_string(), "risk".to_string()]
    );
    assert_permission_audit(&tool, "workspace:custom");
    assert_eq!(reloaded.server.transport_kind, McpTransportKind::Stdio);
}

fn stdio_pid_transport() -> StdioMcpTransport {
    let script = r#"
import json, os, sys
spec = json.loads(sys.argv[1])
for raw_line in sys.stdin:
    if not raw_line.strip():
        continue
    request = json.loads(raw_line)
    method = request["method"]
    if method == "tools/list":
        result = {"tools": spec["tools"]}
    elif method == "resources/list":
        result = {"resources": spec["resources"]}
    elif method == "prompts/list":
        result = {"prompts": spec["prompts"]}
    elif method == "tools/call":
        result = {"content": [{"type": "text", "text": f'pid:{os.getpid()}'}]}
    else:
        result = {"echo_method": method}
    print(json.dumps({"jsonrpc": "2.0", "id": request["id"], "result": result}), flush=True)
"#;
    StdioMcpTransport::new(StdioMcpTransportConfig {
        command: "python3".to_string(),
        args: vec![
            "-u".to_string(),
            "-c".to_string(),
            script.to_string(),
            json!({
                "tools": [{
                    "name": "fetch_url",
                    "description": "fetch URL",
                    "inputSchema": {"type": "object", "properties": {"url": {"type": "string"}}}
                }],
                "resources": [],
                "prompts": []
            })
            .to_string(),
        ],
    })
}

#[test]
fn stdio_transport_clone_opens_a_fresh_session_per_child() {
    let registry = descriptor_registry(
        "mcp:stdio:clone",
        McpTransportKind::Stdio,
        "stdio:clone",
        "fetch_url",
        "web://index",
        "summarize_web",
    );
    let tool = registry.tools[0].clone();
    let permission = mcp_tool_permission_request(
        "permission:stdio:clone",
        &tool,
        DelegatedTaskToolPermissionDecision::Allow,
        "workspace:stdio:clone",
        1,
    );

    let base_transport = stdio_pid_transport();
    let client_a =
        McpClient::new(registry.clone(), base_transport.clone()).expect("client A is valid");
    let client_b = McpClient::new(registry, base_transport).expect("client B is valid");

    let pid_a = client_a
        .call_tool_with_permission(
            "stdio:clone:a",
            &tool.server_id,
            &tool.name,
            json!({"url": "https://example.com/a"}),
            &permission,
        )
        .expect("client A call succeeds");
    let pid_b = client_b
        .call_tool_with_permission(
            "stdio:clone:b",
            &tool.server_id,
            &tool.name,
            json!({"url": "https://example.com/b"}),
            &permission,
        )
        .expect("client B call succeeds");

    let pid_a = pid_a["result"]["content"][0]["text"]
        .as_str()
        .expect("pid a text");
    let pid_b = pid_b["result"]["content"][0]["text"]
        .as_str()
        .expect("pid b text");
    assert!(pid_a.starts_with("pid:"));
    assert!(pid_b.starts_with("pid:"));
    assert_ne!(
        pid_a, pid_b,
        "cloned stdio transports must not share a child process"
    );
}
