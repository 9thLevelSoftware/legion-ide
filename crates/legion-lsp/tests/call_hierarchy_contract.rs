use legion_lsp::{
    LspTextDocumentIdentity, incoming_calls_request, outgoing_calls_request,
    prepare_call_hierarchy_request, project_incoming_calls_response,
    project_outgoing_calls_response, project_prepare_call_hierarchy_response,
};
use legion_protocol::{
    BufferVersion, FileFingerprint, FileId, LanguageId, LspCallHierarchyItem, ProtocolTextRange,
    SnapshotId, TextCoordinate, Utf16Position, WorkspaceId,
};
use serde_json::json;

fn document() -> LspTextDocumentIdentity {
    LspTextDocumentIdentity {
        uri: "file:///workspace/src/main.rs".to_string(),
        language_id: LanguageId("rust".to_string()),
        workspace_id: WorkspaceId(1),
        file_id: FileId(2),
        snapshot_id: SnapshotId(3),
        buffer_version: BufferVersion(4),
        content_hash: Some(FileFingerprint {
            algorithm: "sha256".to_string(),
            value: "abc".to_string(),
        }),
    }
}

fn sample_call_hierarchy_item() -> LspCallHierarchyItem {
    LspCallHierarchyItem {
        name: "main".to_string(),
        kind: 12,
        uri: "file:///workspace/src/main.rs".to_string(),
        range: ProtocolTextRange {
            start: TextCoordinate {
                line: 0,
                character: 0,
                byte_offset: None,
                utf16_offset: None,
            },
            end: TextCoordinate {
                line: 5,
                character: 1,
                byte_offset: None,
                utf16_offset: None,
            },
        },
        selection_range: ProtocolTextRange {
            start: TextCoordinate {
                line: 0,
                character: 3,
                byte_offset: None,
                utf16_offset: None,
            },
            end: TextCoordinate {
                line: 0,
                character: 7,
                byte_offset: None,
                utf16_offset: None,
            },
        },
        detail: Some("fn main()".to_string()),
    }
}

// ---------------------------------------------------------------------------
// Request builder tests
// ---------------------------------------------------------------------------

#[test]
fn prepare_call_hierarchy_request_uses_document_uri_and_position() {
    let request = prepare_call_hierarchy_request(
        70,
        &document(),
        Utf16Position {
            line: 10,
            character: 5,
        },
    );

    assert_eq!(request.id, Some(70));
    assert_eq!(
        request.method.as_deref(),
        Some("textDocument/prepareCallHierarchy")
    );
    let params = request.params.expect("params");
    assert_eq!(
        params["textDocument"]["uri"].as_str(),
        Some("file:///workspace/src/main.rs")
    );
    assert_eq!(params["position"]["line"].as_u64(), Some(10));
    assert_eq!(params["position"]["character"].as_u64(), Some(5));
}

#[test]
fn incoming_calls_request_uses_call_hierarchy_item() {
    let item = sample_call_hierarchy_item();
    let request = incoming_calls_request(71, &item);

    assert_eq!(request.id, Some(71));
    assert_eq!(
        request.method.as_deref(),
        Some("callHierarchy/incomingCalls")
    );
    let params = request.params.expect("params");
    assert_eq!(params["item"]["name"].as_str(), Some("main"));
    assert_eq!(params["item"]["kind"].as_u64(), Some(12));
    assert_eq!(
        params["item"]["uri"].as_str(),
        Some("file:///workspace/src/main.rs")
    );
    assert!(params["item"]["range"].is_object());
    assert!(params["item"]["selectionRange"].is_object());
    assert_eq!(params["item"]["detail"].as_str(), Some("fn main()"));
}

#[test]
fn outgoing_calls_request_uses_call_hierarchy_item() {
    let item = sample_call_hierarchy_item();
    let request = outgoing_calls_request(72, &item);

    assert_eq!(request.id, Some(72));
    assert_eq!(
        request.method.as_deref(),
        Some("callHierarchy/outgoingCalls")
    );
    let params = request.params.expect("params");
    assert_eq!(params["item"]["name"].as_str(), Some("main"));
    assert_eq!(params["item"]["kind"].as_u64(), Some(12));
}

#[test]
fn incoming_calls_request_omits_detail_when_none() {
    let mut item = sample_call_hierarchy_item();
    item.detail = None;
    let request = incoming_calls_request(73, &item);
    let params = request.params.expect("params");
    assert!(params["item"]["detail"].is_null());
}

// ---------------------------------------------------------------------------
// Prepare call hierarchy response projection tests
// ---------------------------------------------------------------------------

#[test]
fn prepare_call_hierarchy_response_projects_items() {
    let response = json!([
        {
            "name": "main",
            "kind": 12,
            "uri": "file:///workspace/src/main.rs",
            "range": {
                "start": {"line": 0, "character": 0},
                "end": {"line": 5, "character": 1}
            },
            "selectionRange": {
                "start": {"line": 0, "character": 3},
                "end": {"line": 0, "character": 7}
            },
            "detail": "fn main()"
        }
    ]);

    let items = project_prepare_call_hierarchy_response(&response).expect("items");
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].name, "main");
    assert_eq!(items[0].kind, 12);
    assert_eq!(items[0].uri, "file:///workspace/src/main.rs");
    assert_eq!(items[0].detail.as_deref(), Some("fn main()"));
    assert_eq!(items[0].range.start.line, 0);
    assert_eq!(items[0].selection_range.start.character, 3);
}

#[test]
fn prepare_call_hierarchy_null_response_returns_none() {
    assert!(project_prepare_call_hierarchy_response(&json!(null)).is_none());
}

#[test]
fn prepare_call_hierarchy_empty_array_returns_empty_vec() {
    let items = project_prepare_call_hierarchy_response(&json!([])).expect("items");
    assert!(items.is_empty());
}

#[test]
fn prepare_call_hierarchy_malformed_item_is_skipped() {
    let response = json!([
        {"name": "valid", "kind": 12, "uri": "file:///a.rs",
         "range": {"start": {"line": 0, "character": 0}, "end": {"line": 1, "character": 0}},
         "selectionRange": {"start": {"line": 0, "character": 3}, "end": {"line": 0, "character": 8}}},
        {"kind": 12}
    ]);

    let items = project_prepare_call_hierarchy_response(&response).expect("items");
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].name, "valid");
}

#[test]
fn prepare_call_hierarchy_non_array_returns_none() {
    assert!(project_prepare_call_hierarchy_response(&json!({"name": "x"})).is_none());
}

// ---------------------------------------------------------------------------
// Incoming calls response projection tests
// ---------------------------------------------------------------------------

#[test]
fn incoming_calls_response_projects_callers() {
    let response = json!([
        {
            "from": {
                "name": "caller_fn",
                "kind": 12,
                "uri": "file:///workspace/src/caller.rs",
                "range": {
                    "start": {"line": 10, "character": 0},
                    "end": {"line": 15, "character": 1}
                },
                "selectionRange": {
                    "start": {"line": 10, "character": 3},
                    "end": {"line": 10, "character": 12}
                }
            },
            "fromRanges": [
                {
                    "start": {"line": 12, "character": 4},
                    "end": {"line": 12, "character": 20}
                }
            ]
        }
    ]);

    let calls = project_incoming_calls_response(&response);
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].from.name, "caller_fn");
    assert_eq!(calls[0].from.kind, 12);
    assert_eq!(calls[0].from.uri, "file:///workspace/src/caller.rs");
    assert_eq!(calls[0].from_ranges.len(), 1);
    assert_eq!(calls[0].from_ranges[0].start.line, 12);
    assert_eq!(calls[0].from_ranges[0].start.character, 4);
}

#[test]
fn incoming_calls_null_response_returns_empty() {
    assert!(project_incoming_calls_response(&json!(null)).is_empty());
}

#[test]
fn incoming_calls_empty_array_returns_empty() {
    assert!(project_incoming_calls_response(&json!([])).is_empty());
}

#[test]
fn incoming_calls_malformed_from_is_skipped() {
    let response = json!([
        {"from": {"kind": 12}, "fromRanges": []},
        {
            "from": {
                "name": "good",
                "kind": 12,
                "uri": "file:///a.rs",
                "range": {"start": {"line": 0, "character": 0}, "end": {"line": 1, "character": 0}},
                "selectionRange": {"start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 4}}
            },
            "fromRanges": []
        }
    ]);

    let calls = project_incoming_calls_response(&response);
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].from.name, "good");
}

#[test]
fn incoming_calls_missing_from_ranges_defaults_to_empty() {
    let response = json!([
        {
            "from": {
                "name": "caller",
                "kind": 12,
                "uri": "file:///a.rs",
                "range": {"start": {"line": 0, "character": 0}, "end": {"line": 1, "character": 0}},
                "selectionRange": {"start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 6}}
            }
        }
    ]);

    let calls = project_incoming_calls_response(&response);
    assert_eq!(calls.len(), 1);
    assert!(calls[0].from_ranges.is_empty());
}

// ---------------------------------------------------------------------------
// Outgoing calls response projection tests
// ---------------------------------------------------------------------------

#[test]
fn outgoing_calls_response_projects_callees() {
    let response = json!([
        {
            "to": {
                "name": "callee_fn",
                "kind": 12,
                "uri": "file:///workspace/src/callee.rs",
                "range": {
                    "start": {"line": 5, "character": 0},
                    "end": {"line": 8, "character": 1}
                },
                "selectionRange": {
                    "start": {"line": 5, "character": 3},
                    "end": {"line": 5, "character": 12}
                },
                "detail": "fn callee_fn(x: i32)"
            },
            "fromRanges": [
                {
                    "start": {"line": 2, "character": 4},
                    "end": {"line": 2, "character": 15}
                },
                {
                    "start": {"line": 3, "character": 4},
                    "end": {"line": 3, "character": 15}
                }
            ]
        }
    ]);

    let calls = project_outgoing_calls_response(&response);
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].to.name, "callee_fn");
    assert_eq!(calls[0].to.kind, 12);
    assert_eq!(calls[0].to.detail.as_deref(), Some("fn callee_fn(x: i32)"));
    assert_eq!(calls[0].from_ranges.len(), 2);
    assert_eq!(calls[0].from_ranges[0].start.line, 2);
    assert_eq!(calls[0].from_ranges[1].start.line, 3);
}

#[test]
fn outgoing_calls_null_response_returns_empty() {
    assert!(project_outgoing_calls_response(&json!(null)).is_empty());
}

#[test]
fn outgoing_calls_empty_array_returns_empty() {
    assert!(project_outgoing_calls_response(&json!([])).is_empty());
}

#[test]
fn outgoing_calls_malformed_to_is_skipped() {
    let response = json!([
        {"to": {}, "fromRanges": []}
    ]);

    assert!(project_outgoing_calls_response(&response).is_empty());
}

#[test]
fn multiple_incoming_and_outgoing_calls_projected() {
    let incoming_response = json!([
        {
            "from": {
                "name": "alpha",
                "kind": 12,
                "uri": "file:///a.rs",
                "range": {"start": {"line": 0, "character": 0}, "end": {"line": 1, "character": 0}},
                "selectionRange": {"start": {"line": 0, "character": 3}, "end": {"line": 0, "character": 8}}
            },
            "fromRanges": [{"start": {"line": 0, "character": 10}, "end": {"line": 0, "character": 15}}]
        },
        {
            "from": {
                "name": "beta",
                "kind": 6,
                "uri": "file:///b.rs",
                "range": {"start": {"line": 5, "character": 0}, "end": {"line": 10, "character": 0}},
                "selectionRange": {"start": {"line": 5, "character": 4}, "end": {"line": 5, "character": 8}}
            },
            "fromRanges": []
        }
    ]);

    let calls = project_incoming_calls_response(&incoming_response);
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0].from.name, "alpha");
    assert_eq!(calls[0].from.kind, 12);
    assert_eq!(calls[1].from.name, "beta");
    assert_eq!(calls[1].from.kind, 6);
}
