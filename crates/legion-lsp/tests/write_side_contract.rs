use legion_lsp::{
    LspTextDocumentIdentity, code_action_request, formatting_request, organize_imports_request,
    prepare_rename_request, range_formatting_request, rename_request,
};
use legion_protocol::{
    BufferVersion, FileFingerprint, FileId, LanguageId, LspFormattingOptions, SnapshotId,
    Utf16Position, Utf16Range, WorkspaceId,
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

fn formatting_options() -> LspFormattingOptions {
    LspFormattingOptions {
        tab_size: 4,
        insert_spaces: true,
        trim_trailing_whitespace: true,
        insert_final_newline: true,
        custom_options: vec![("rustfmt.wrap_comments".to_string(), "true".to_string())],
    }
}

#[test]
fn prepare_rename_and_rename_requests_use_document_uri_position_and_name() {
    let position = Utf16Position {
        line: 2,
        character: 9,
    };

    let prepare = prepare_rename_request(61, &document(), position);
    assert_eq!(prepare.id, Some(61));
    assert_eq!(
        prepare.method.as_deref(),
        Some("textDocument/prepareRename")
    );
    let params = prepare.params.expect("params");
    assert_eq!(
        params["textDocument"]["uri"].as_str(),
        Some("file:///workspace/src/main.rs")
    );
    assert_eq!(params["position"]["line"].as_u64(), Some(2));
    assert_eq!(params["position"]["character"].as_u64(), Some(9));

    let rename = rename_request(62, &document(), position, "updated_name");
    assert_eq!(rename.id, Some(62));
    assert_eq!(rename.method.as_deref(), Some("textDocument/rename"));
    let params = rename.params.expect("params");
    assert_eq!(params["newName"].as_str(), Some("updated_name"));
    assert_eq!(params["position"]["character"].as_u64(), Some(9));
}

#[test]
fn formatting_requests_use_document_uri_and_options() {
    let options = formatting_options();
    let request = formatting_request(63, &document(), &options);

    assert_eq!(request.id, Some(63));
    assert_eq!(request.method.as_deref(), Some("textDocument/formatting"));
    let params = request.params.expect("params");
    assert_eq!(
        params["textDocument"]["uri"].as_str(),
        Some("file:///workspace/src/main.rs")
    );
    assert_eq!(params["options"]["tab_size"].as_u64(), Some(4));
    assert_eq!(params["options"]["insert_spaces"].as_bool(), Some(true));
    assert_eq!(
        params["options"]["custom_options"][0][0].as_str(),
        Some("rustfmt.wrap_comments")
    );

    let range_request = range_formatting_request(
        64,
        &document(),
        Utf16Range {
            start: Utf16Position {
                line: 1,
                character: 0,
            },
            end: Utf16Position {
                line: 4,
                character: 2,
            },
        },
        &options,
    );
    assert_eq!(
        range_request.method.as_deref(),
        Some("textDocument/rangeFormatting")
    );
    let params = range_request.params.expect("params");
    assert_eq!(params["range"]["start"]["line"].as_u64(), Some(1));
    assert_eq!(params["range"]["end"]["character"].as_u64(), Some(2));
}

#[test]
fn code_action_and_organize_imports_requests_include_context() {
    let diagnostics = vec![json!({
        "range": {
            "start": {"line": 1, "character": 4},
            "end": {"line": 1, "character": 12}
        },
        "severity": 2,
        "code": "unused_import",
        "source": "rust-analyzer",
        "message": "unused import"
    })];
    let range = Utf16Range {
        start: Utf16Position {
            line: 0,
            character: 0,
        },
        end: Utf16Position {
            line: 8,
            character: 0,
        },
    };

    let request = code_action_request(
        65,
        &document(),
        range,
        diagnostics.clone(),
        Some(vec!["source.fixAll".to_string()]),
    );
    assert_eq!(request.id, Some(65));
    assert_eq!(request.method.as_deref(), Some("textDocument/codeAction"));
    let params = request.params.expect("params");
    assert_eq!(
        params["context"]["diagnostics"].as_array().map(Vec::len),
        Some(1)
    );
    assert_eq!(params["context"]["only"][0].as_str(), Some("source.fixAll"));
    assert_eq!(params["range"]["start"]["line"].as_u64(), Some(0));

    let organize_imports = organize_imports_request(66, &document(), range, diagnostics);
    assert_eq!(
        organize_imports.method.as_deref(),
        Some("textDocument/codeAction")
    );
    let params = organize_imports.params.expect("params");
    assert_eq!(
        params["context"]["only"][0].as_str(),
        Some("source.organizeImports")
    );
    assert_eq!(
        params["context"]["diagnostics"].as_array().map(Vec::len),
        Some(1)
    );
}
