use legion_lsp::{
    LspDiagnosticProjectionContext, LspTextDocumentChange, LspTextDocumentIdentity,
    did_change_notification, did_open_notification, lsp_unavailable_problem_projection,
    project_publish_diagnostics,
};
use legion_protocol::{
    BufferVersion, FileFingerprint, FileId, LanguageId, ProtocolDiagnosticSeverity, RedactionHint,
    SemanticPrivacyScope, SnapshotId, Utf16Position as ProtocolUtf16Position,
    Utf16Range as ProtocolUtf16Range, WorkspaceId,
};
use legion_text::LineIndex;
use serde_json::json;

fn fingerprint(value: &str) -> FileFingerprint {
    FileFingerprint {
        algorithm: "test".to_string(),
        value: value.to_string(),
    }
}

fn document_identity() -> LspTextDocumentIdentity {
    LspTextDocumentIdentity {
        uri: "file:///workspace/src/main.rs".to_string(),
        language_id: LanguageId("rust".to_string()),
        workspace_id: WorkspaceId(7),
        file_id: FileId(9),
        snapshot_id: SnapshotId(11),
        buffer_version: BufferVersion(3),
        content_hash: Some(fingerprint("content")),
    }
}

fn diagnostic_context() -> LspDiagnosticProjectionContext {
    LspDiagnosticProjectionContext {
        workspace_id: WorkspaceId(7),
        file_id: FileId(9),
        snapshot_id: SnapshotId(11),
        buffer_version: BufferVersion(3),
        content_hash: Some(fingerprint("content")),
        privacy_scope: SemanticPrivacyScope::Workspace,
        disclose_ranges: true,
    }
}

fn protocol_range(range: legion_text::Utf16Range) -> ProtocolUtf16Range {
    ProtocolUtf16Range {
        start: ProtocolUtf16Position {
            line: range.start.line as u32,
            character: range.start.character as u32,
        },
        end: ProtocolUtf16Position {
            line: range.end.line as u32,
            character: range.end.character as u32,
        },
    }
}

#[test]
fn document_sync_builds_did_open_and_incremental_did_change_with_utf16_ranges() {
    let text = "fn main() {\n    let crab = \"🦀\";\n}\n";
    let line_index = LineIndex::new(text);
    let crab_start = text.find('🦀').expect("crab emoji present");
    let crab_end = crab_start + "🦀".len();
    let utf16_range = protocol_range(line_index.utf16_range(crab_start, crab_end).unwrap());
    assert_eq!(utf16_range.start.line, 1);
    assert_eq!(utf16_range.end.character - utf16_range.start.character, 2);

    let document = document_identity();
    let did_open = did_open_notification(&document, text);
    assert_eq!(did_open.method.as_deref(), Some("textDocument/didOpen"));
    assert_eq!(did_open.id, None);
    assert_eq!(
        did_open.params.as_ref().unwrap()["textDocument"]["languageId"],
        "rust"
    );
    assert_eq!(
        did_open.params.as_ref().unwrap()["textDocument"]["version"],
        3
    );
    assert_eq!(
        did_open.params.as_ref().unwrap()["textDocument"]["text"],
        text
    );

    let did_change = did_change_notification(
        &document,
        vec![LspTextDocumentChange::ranged(utf16_range, "crab()")],
    );
    let params = did_change.params.as_ref().unwrap();
    assert_eq!(did_change.method.as_deref(), Some("textDocument/didChange"));
    assert_eq!(params["textDocument"]["uri"], document.uri);
    assert_eq!(params["textDocument"]["version"], 3);
    assert_eq!(params["contentChanges"][0]["text"], "crab()");
    assert_eq!(
        params["contentChanges"][0]["range"]["start"]["character"],
        utf16_range.start.character
    );
    assert_eq!(
        params["contentChanges"][0]["range"]["end"]["character"],
        utf16_range.end.character
    );
}

#[test]
fn publish_diagnostics_projects_metadata_without_source_bodies() {
    let payload = json!({
        "uri": "file:///workspace/src/main.rs",
        "diagnostics": [{
            "range": {
                "start": {"line": 1, "character": 4},
                "end": {"line": 1, "character": 9}
            },
            "severity": 1,
            "code": "E0425",
            "source": "rust-analyzer",
            "message": "cannot find value `SECRET_SOURCE_BODY` in this scope"
        }]
    });

    let projected = project_publish_diagnostics(&payload, diagnostic_context()).unwrap();

    assert_eq!(projected.problems.len(), 1);
    let problem = &projected.problems[0];
    assert_eq!(problem.file_id, Some(FileId(9)));
    assert_eq!(problem.severity, ProtocolDiagnosticSeverity::Error);
    assert_eq!(problem.code_label.as_deref(), Some("E0425"));
    assert_eq!(problem.source_label.as_deref(), Some("rust-analyzer"));
    assert!(problem.range.is_some());
    assert!(
        problem
            .redaction_hints
            .contains(&RedactionHint::MetadataOnly)
    );
    assert!(!problem.message.contains("SECRET_SOURCE_BODY"));

    assert_eq!(projected.summary.diagnostic_count, 1);
    assert_eq!(projected.summary.error_count, 1);
    assert_eq!(projected.summary.warning_count, 0);
    assert_eq!(projected.summary.diagnostic_hashes.len(), 1);
    assert_eq!(projected.summary.source_hashes.len(), 1);
    assert!(
        projected
            .summary
            .redaction_hints
            .contains(&RedactionHint::MetadataOnly)
    );
    assert!(!format!("{:?}", projected.summary).contains("SECRET_SOURCE_BODY"));
}

#[test]
fn unavailable_lsp_projection_preserves_fallback_semantic_path() {
    let fallback = lsp_unavailable_problem_projection(diagnostic_context(), "server_not_running");

    assert_eq!(fallback.file_id, Some(FileId(9)));
    assert_eq!(fallback.severity, ProtocolDiagnosticSeverity::Warning);
    assert_eq!(fallback.source_label.as_deref(), Some("lsp"));
    assert!(fallback.range.is_none());
    assert!(fallback.message.contains("semantic/index fallback"));
    assert!(
        fallback
            .redaction_hints
            .contains(&RedactionHint::MetadataOnly)
    );
    assert!(!fallback.message.contains("src/main.rs"));
}
