use legion_lsp::{
    LspTextDocumentIdentity, code_lens_request, completion_request, declaration_request,
    definition_request, document_symbol_request, folding_range_request, hover_request,
    implementation_request, inlay_hint_request, project_code_lens_response,
    project_completion_response, project_document_symbol_response, project_hover_response,
    project_inlay_hint_response, project_location_response, project_workspace_symbol_response,
    references_request, semantic_tokens_full_request, signature_help_request,
    type_definition_request, workspace_symbol_request,
};
use legion_protocol::{
    BufferVersion, FileFingerprint, FileId, LanguageId, SnapshotId, Utf16Position, Utf16Range,
    WorkspaceId,
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

#[test]
fn completion_request_uses_utf16_position_and_document_uri() {
    let request = completion_request(
        42,
        &document(),
        Utf16Position {
            line: 7,
            character: 11,
        },
    );

    assert_eq!(request.id, Some(42));
    assert_eq!(request.method.as_deref(), Some("textDocument/completion"));
    let params = request.params.expect("params");
    assert_eq!(
        params["textDocument"]["uri"].as_str(),
        Some("file:///workspace/src/main.rs")
    );
    assert_eq!(params["position"]["line"].as_u64(), Some(7));
    assert_eq!(params["position"]["character"].as_u64(), Some(11));
}

#[test]
fn hover_request_uses_utf16_position_and_document_uri() {
    let request = hover_request(
        43,
        &document(),
        Utf16Position {
            line: 3,
            character: 5,
        },
    );

    assert_eq!(request.id, Some(43));
    assert_eq!(request.method.as_deref(), Some("textDocument/hover"));
    let params = request.params.expect("params");
    assert_eq!(
        params["textDocument"]["uri"].as_str(),
        Some("file:///workspace/src/main.rs")
    );
    assert_eq!(params["position"]["line"].as_u64(), Some(3));
    assert_eq!(params["position"]["character"].as_u64(), Some(5));
}

#[test]
fn definition_request_uses_utf16_position_and_document_uri() {
    let request = definition_request(
        44,
        &document(),
        Utf16Position {
            line: 4,
            character: 9,
        },
    );

    assert_eq!(request.id, Some(44));
    assert_eq!(request.method.as_deref(), Some("textDocument/definition"));
    let params = request.params.expect("params");
    assert_eq!(
        params["textDocument"]["uri"].as_str(),
        Some("file:///workspace/src/main.rs")
    );
    assert_eq!(params["position"]["line"].as_u64(), Some(4));
    assert_eq!(params["position"]["character"].as_u64(), Some(9));
}

#[test]
fn references_request_uses_utf16_position_document_uri_and_context() {
    let request = references_request(
        45,
        &document(),
        Utf16Position {
            line: 8,
            character: 13,
        },
        true,
    );

    assert_eq!(request.id, Some(45));
    assert_eq!(request.method.as_deref(), Some("textDocument/references"));
    let params = request.params.expect("params");
    assert_eq!(
        params["textDocument"]["uri"].as_str(),
        Some("file:///workspace/src/main.rs")
    );
    assert_eq!(params["position"]["line"].as_u64(), Some(8));
    assert_eq!(params["position"]["character"].as_u64(), Some(13));
    assert_eq!(
        params["context"]["includeDeclaration"].as_bool(),
        Some(true)
    );
}

#[test]
fn document_symbol_request_uses_document_uri() {
    let request = document_symbol_request(46, &document());

    assert_eq!(request.id, Some(46));
    assert_eq!(
        request.method.as_deref(),
        Some("textDocument/documentSymbol")
    );
    let params = request.params.expect("params");
    assert_eq!(
        params["textDocument"]["uri"].as_str(),
        Some("file:///workspace/src/main.rs")
    );
}

#[test]
fn document_symbol_response_projects_nested_outline_rows() {
    let response = json!([
        {
            "name": "module",
            "kind": 2,
            "range": {"start": {"line": 0, "character": 0}, "end": {"line": 5, "character": 1}},
            "selectionRange": {"start": {"line": 0, "character": 4}, "end": {"line": 0, "character": 10}},
            "children": [
                {
                    "name": "beta",
                    "kind": 12,
                    "range": {"start": {"line": 1, "character": 4}, "end": {"line": 1, "character": 18}},
                    "selectionRange": {"start": {"line": 1, "character": 11}, "end": {"line": 1, "character": 15}}
                }
            ]
        }
    ]);

    let outline = project_document_symbol_response(&response, 10);
    assert_eq!(outline.len(), 2);
    assert_eq!(outline[0].label, "module");
    assert_eq!(outline[0].kind_label, "lsp.symbol.kind.2");
    assert_eq!(outline[0].depth, 0);
    assert_eq!(outline[1].label, "beta");
    assert_eq!(outline[1].depth, 1);
    assert!(outline.iter().all(|row| row.range.is_some()));
}

#[test]
fn document_symbol_response_projects_flat_symbol_information_and_caps() {
    let response = json!([
        {
            "name": "alpha",
            "kind": 12,
            "location": {
                "uri": "file:///workspace/src/main.rs",
                "range": {"start": {"line": 0, "character": 7}, "end": {"line": 0, "character": 12}}
            }
        },
        {
            "name": "beta",
            "kind": 12,
            "location": {
                "uri": "file:///workspace/src/main.rs",
                "range": {"start": {"line": 1, "character": 7}, "end": {"line": 1, "character": 11}}
            }
        }
    ]);

    let outline = project_document_symbol_response(&response, 1);
    assert_eq!(outline.len(), 1);
    assert_eq!(outline[0].label, "alpha");
    assert!(outline[0].children_omitted);
}

#[test]
fn workspace_symbol_request_uses_query_param() {
    let request = workspace_symbol_request(47, "beta");

    assert_eq!(request.id, Some(47));
    assert_eq!(request.method.as_deref(), Some("workspace/symbol"));
    let params = request.params.expect("params");
    assert_eq!(params["query"].as_str(), Some("beta"));
    let long_query = "x".repeat(400);
    let request = workspace_symbol_request(48, long_query);
    let params = request.params.expect("params");
    let query = params["query"].as_str().expect("bounded query");
    assert!(query.len() <= 240);
    assert!(query.ends_with('…'));
}

#[test]
fn workspace_symbol_response_projects_symbol_locations() {
    let response = json!([
        {
            "name": "beta",
            "kind": 12,
            "location": {
                "uri": "file:///workspace/src/main.rs",
                "range": {"start": {"line": 1, "character": 7}, "end": {"line": 1, "character": 11}}
            }
        },
        {
            "name": "gamma",
            "kind": 12,
            "location": {"uri": "file:///workspace/src/gamma.rs"}
        }
    ]);

    let locations = project_workspace_symbol_response(&response, 10);
    assert_eq!(locations.len(), 2);
    assert_eq!(locations[0].label, "beta");
    assert_eq!(
        locations[0].path.as_ref().map(|path| path.0.as_str()),
        Some("/workspace/src/main.rs")
    );
    assert!(locations[0].range.is_some());
    assert_eq!(
        locations[1].path.as_ref().map(|path| path.0.as_str()),
        Some("/workspace/src/gamma.rs")
    );
    assert!(locations[1].degraded);
    assert!(locations[1].range.is_none());
}

#[test]
fn inlay_hint_request_uses_document_uri_and_range() {
    let request = inlay_hint_request(
        49,
        &document(),
        Utf16Range {
            start: Utf16Position {
                line: 1,
                character: 0,
            },
            end: Utf16Position {
                line: 3,
                character: 8,
            },
        },
    );

    assert_eq!(request.id, Some(49));
    assert_eq!(request.method.as_deref(), Some("textDocument/inlayHint"));
    let params = request.params.expect("params");
    assert_eq!(
        params["textDocument"]["uri"].as_str(),
        Some("file:///workspace/src/main.rs")
    );
    assert_eq!(params["range"]["start"]["line"].as_u64(), Some(1));
    assert_eq!(params["range"]["end"]["character"].as_u64(), Some(8));
}

#[test]
fn inlay_hint_response_projects_metadata_rows() {
    let response = json!([
        {
            "position": {"line": 1, "character": 13},
            "label": ": u32",
            "kind": 1,
            "paddingLeft": true,
            "paddingRight": false,
            "textEdits": [{"newText": "SECRET_SOURCE_BODY"}]
        },
        {
            "position": {"line": 2, "character": 8},
            "label": [{"value": "param"}, {"value": ": &str"}],
            "kind": 2,
            "paddingLeft": false,
            "paddingRight": true
        }
    ]);

    let hints = project_inlay_hint_response(&response, "rust-analyzer", 10);
    assert_eq!(hints.len(), 2);
    assert_eq!(hints[0].label, ": u32");
    assert_eq!(hints[0].kind_label, "lsp.inlay.kind.1");
    assert!(hints[0].padding_left);
    assert!(!hints[0].padding_right);
    assert_eq!(hints[0].source_label, "rust-analyzer");
    assert_eq!(hints[1].label, "param: &str");
    assert!(
        hints
            .iter()
            .all(|hint| !hint.label.contains("SECRET_SOURCE_BODY"))
    );
}

#[test]
fn code_lens_request_uses_document_uri() {
    let request = code_lens_request(50, &document());

    assert_eq!(request.id, Some(50));
    assert_eq!(request.method.as_deref(), Some("textDocument/codeLens"));
    let params = request.params.expect("params");
    assert_eq!(
        params["textDocument"]["uri"].as_str(),
        Some("file:///workspace/src/main.rs")
    );
}

#[test]
fn code_lens_response_projects_metadata_rows() {
    let response = json!([
        {
            "range": {"start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 12}},
            "command": {
                "title": "Run test",
                "command": "rust-analyzer.runSingle",
                "arguments": [{"source": "SECRET_SOURCE_BODY"}]
            },
            "data": {"kind": "runnable", "target": "SECRET_SOURCE_BODY"}
        },
        {
            "range": {"start": {"line": 1, "character": 0}, "end": {"line": 1, "character": 8}},
            "data": {"kind": "references", "count": 2}
        }
    ]);

    let lenses = project_code_lens_response(&response, "rust-analyzer", 10);
    assert_eq!(lenses.len(), 2);
    assert_eq!(lenses[0].title, "Run test");
    assert_eq!(lenses[0].command_label, "rust-analyzer.runSingle");
    assert_eq!(lenses[0].kind_label, "lsp.codelens.runnable");
    assert_eq!(lenses[0].source_label, "rust-analyzer");
    assert!(lenses[0].range.is_some());
    assert!(
        lenses[0]
            .data_label
            .as_deref()
            .unwrap()
            .contains("kind=runnable")
    );
    assert!(!format!("{lenses:?}").contains("SECRET_SOURCE_BODY"));
    assert_eq!(lenses[1].title, "lsp code lens");
    assert_eq!(lenses[1].kind_label, "lsp.codelens.references");
}

#[test]
fn completion_response_projects_completion_list_with_bounded_rows() {
    let response = json!({
        "isIncomplete": false,
        "items": [
            {"label": "println!", "detail": "macro_rules! println", "kind": 3, "insertText": "println!(\"$0\")"},
            {"label": "very_long_completion_label_abcdefghijklmnopqrstuvwxyz_abcdefghijklmnopqrstuvwxyz_abcdefghijklmnopqrstuvwxyz_abcdefghijklmnopqrstuvwxyz", "detail": "source excerpt SECRET_SOURCE_BODY should be bounded only", "kind": 6},
            {"detail": "missing label is ignored", "kind": 1}
        ]
    });

    let completions = project_completion_response(&response, 10);
    assert_eq!(completions.len(), 2);
    assert_eq!(completions[0].label, "println!");
    assert_eq!(
        completions[0].detail_label.as_deref(),
        Some("macro_rules! println")
    );
    assert_eq!(completions[0].kind_label, "lsp.completion.kind.3");
    assert_eq!(completions[0].score_basis_points, 10_000);
    assert!(!completions[0].degraded);
    assert!(completions[1].label.ends_with('…'));
    assert!(completions[1].label.len() <= 123);
    assert_eq!(completions[1].kind_label, "lsp.completion.kind.6");
    assert!(completions[1].degraded);
    assert!(
        completions[0]
            .completion_id
            .starts_with("lsp-completion-0-")
    );
    assert!(
        completions[1]
            .completion_id
            .starts_with("lsp-completion-1-")
    );
}

#[test]
fn completion_response_projects_array_shape_and_limit() {
    let response = json!([
        {"label": "alpha", "kind": 1, "insertText": "alpha"},
        {"label": "beta", "kind": 2, "insertText": "beta"},
        {"label": "gamma", "kind": 3, "insertText": "gamma"}
    ]);

    let completions = project_completion_response(&response, 2);
    assert_eq!(completions.len(), 2);
    assert_eq!(completions[0].label, "alpha");
    assert_eq!(completions[1].label, "beta");
    assert!(completions[0].score_basis_points > completions[1].score_basis_points);
}

#[test]
fn malformed_completion_response_yields_no_rows() {
    assert!(project_completion_response(&json!({"items": "not-an-array"}), 10).is_empty());
    assert!(
        project_completion_response(&json!({"items": [{"detail": "missing label"}]}), 10)
            .is_empty()
    );
}

#[test]
fn hover_response_projects_markup_and_range_metadata() {
    let response = json!({
        "contents": {"kind": "markdown", "value": "fn alpha() -> u32\n\nSECRET_SOURCE_BODY should be bounded"},
        "range": {
            "start": {"line": 1, "character": 2},
            "end": {"line": 1, "character": 7}
        }
    });

    let hover = project_hover_response(&response, Some(FileId(2))).expect("hover row");
    assert_eq!(hover.file_id, Some(FileId(2)));
    assert_eq!(hover.label, "fn alpha() -> u32");
    assert!(hover.summary.contains("fn alpha"));
    assert!(hover.summary.len() <= 323);
    assert!(hover.hover_id.starts_with("lsp-hover-"));
    assert!(hover.range.is_some());
    assert!(!hover.degraded);
}

#[test]
fn hover_response_projects_marked_string_array_and_null() {
    let response = json!({
        "contents": ["alpha", {"language": "rust", "value": "beta"}]
    });

    let hover = project_hover_response(&response, None).expect("hover row");
    assert_eq!(hover.file_id, None);
    assert_eq!(hover.label, "alpha");
    assert!(hover.summary.contains("alpha"));
    assert!(hover.summary.contains("beta"));
    assert!(hover.degraded);
    assert!(project_hover_response(&json!(null), None).is_none());
}

#[test]
fn definition_response_projects_location_and_location_link_shapes() {
    let response = json!([
        {
            "uri": "file:///workspace/src/lib.rs",
            "range": {
                "start": {"line": 1, "character": 2},
                "end": {"line": 1, "character": 8}
            }
        },
        {
            "targetUri": "file:///workspace/src/other.rs",
            "targetSelectionRange": {
                "start": {"line": 4, "character": 1},
                "end": {"line": 4, "character": 5}
            }
        }
    ]);

    let locations = project_location_response(&response, 10);
    assert_eq!(locations.len(), 2);
    assert!(locations[0].location_id.starts_with("lsp-location-0-"));
    assert_eq!(
        locations[0].path.as_ref().map(|p| p.0.as_str()),
        Some("/workspace/src/lib.rs")
    );
    assert!(locations[0].range.is_some());
    assert!(!locations[0].degraded);
    assert!(locations[1].location_id.starts_with("lsp-location-1-"));
    assert_eq!(
        locations[1].path.as_ref().map(|p| p.0.as_str()),
        Some("/workspace/src/other.rs")
    );
    assert!(locations[1].range.is_some());
    assert!(locations[1].label.contains("other.rs"));
}

#[test]
fn definition_response_projects_single_location_and_malformed_as_empty() {
    let response = json!({
        "uri": "file:///workspace/src/lib.rs",
        "range": {
            "start": {"line": 0, "character": 0},
            "end": {"line": 0, "character": 3}
        }
    });
    let locations = project_location_response(&response, 5);
    assert_eq!(locations.len(), 1);
    assert!(locations[0].range.is_some());
    let degraded =
        project_location_response(&json!({"uri": "file:///workspace/src/no_range.rs"}), 5);
    assert_eq!(degraded.len(), 1);
    assert!(degraded[0].degraded);
    assert!(degraded[0].range.is_none());
    assert!(project_location_response(&json!(null), 5).is_empty());
    assert!(project_location_response(&json!([{"range": {}}]), 5).is_empty());
}

#[test]
fn declaration_implementation_and_type_definition_requests_use_document_uri_and_position() {
    let position = Utf16Position {
        line: 6,
        character: 14,
    };

    for request in [
        declaration_request(51, &document(), position),
        implementation_request(52, &document(), position),
        type_definition_request(53, &document(), position),
    ] {
        assert!(matches!(
            request.method.as_deref(),
            Some("textDocument/declaration")
                | Some("textDocument/implementation")
                | Some("textDocument/typeDefinition")
        ));
        let params = request.params.expect("params");
        assert_eq!(
            params["textDocument"]["uri"].as_str(),
            Some("file:///workspace/src/main.rs")
        );
        assert_eq!(params["position"]["line"].as_u64(), Some(6));
        assert_eq!(params["position"]["character"].as_u64(), Some(14));
    }
}

#[test]
fn signature_help_request_uses_document_uri_and_position() {
    let request = signature_help_request(
        54,
        &document(),
        Utf16Position {
            line: 2,
            character: 17,
        },
    );

    assert_eq!(request.id, Some(54));
    assert_eq!(
        request.method.as_deref(),
        Some("textDocument/signatureHelp")
    );
    let params = request.params.expect("params");
    assert_eq!(
        params["textDocument"]["uri"].as_str(),
        Some("file:///workspace/src/main.rs")
    );
    assert_eq!(params["position"]["line"].as_u64(), Some(2));
    assert_eq!(params["position"]["character"].as_u64(), Some(17));
}

#[test]
fn folding_range_request_uses_document_uri() {
    let request = folding_range_request(55, &document());

    assert_eq!(request.id, Some(55));
    assert_eq!(request.method.as_deref(), Some("textDocument/foldingRange"));
    let params = request.params.expect("params");
    assert_eq!(
        params["textDocument"]["uri"].as_str(),
        Some("file:///workspace/src/main.rs")
    );
}

#[test]
fn semantic_tokens_full_request_uses_document_uri() {
    let request = semantic_tokens_full_request(56, &document());

    assert_eq!(request.id, Some(56));
    assert_eq!(
        request.method.as_deref(),
        Some("textDocument/semanticTokens/full")
    );
    let params = request.params.expect("params");
    assert_eq!(
        params["textDocument"]["uri"].as_str(),
        Some("file:///workspace/src/main.rs")
    );
}
