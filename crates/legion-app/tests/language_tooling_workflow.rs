use std::sync::atomic::{AtomicU64, Ordering};

use legion_app::{AppCommandOutcome, AppComposition};
use legion_protocol::{
    LanguageToolingOperationKind, PrincipalId, ProposalLifecycleState, ProposalPayloadKind,
    ProtocolDiagnosticSeverity, RedactionHint, TextCoordinate, Utf16Position, Utf16Range,
    WorkspaceTrustState,
};
use legion_ui::CommandDispatchIntent;
use serde_json::json;
use uuid::Uuid;

static TEMP_ROOT_COUNTER: AtomicU64 = AtomicU64::new(0);

fn create_root() -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!(
        "legion-language-tooling-{}-{}",
        std::process::id(),
        TEMP_ROOT_COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&root).expect("create temp root");
    root
}

fn position(byte_offset: u64) -> TextCoordinate {
    TextCoordinate {
        line: 0,
        character: byte_offset as u32,
        byte_offset: Some(byte_offset),
        utf16_offset: Some(byte_offset),
    }
}

#[test]
fn language_tooling_workflow_refreshes_projection_without_ui_text_ownership() {
    let root = create_root();
    let target = root.join("main.rs");
    std::fs::write(
        &target,
        "fn main() {\n    let value = 1;\n    println!(\"{value}\");\n}\n",
    )
    .expect("write source file");

    let mut app = AppComposition::new();
    app.open_workspace(
        &root,
        WorkspaceTrustState::Trusted,
        PrincipalId("principal-language".to_string()),
    )
    .expect("open workspace");
    app.open_file(target.to_string_lossy())
        .expect("open source file");
    let buffer_id = app.active_buffer_id().expect("active buffer");
    let original_text = app
        .editor()
        .text(buffer_id)
        .expect("active buffer text")
        .to_string();

    let completion = app
        .dispatch_ui_intent(CommandDispatchIntent::RequestCompletion {
            buffer_id,
            position: position(3),
        })
        .expect("completion dispatch");
    let projection = match completion {
        AppCommandOutcome::LanguageToolingUpdated(projection) => projection,
        other => panic!("expected language projection, got {other:?}"),
    };
    assert_eq!(projection.buffer_id, Some(buffer_id));
    assert!(!projection.completions.is_empty());
    assert!(
        projection
            .operations
            .iter()
            .any(|operation| operation.message == "semantic projection refreshed")
    );

    let formatting = app
        .dispatch_ui_intent(CommandDispatchIntent::RequestFormattingProposal { buffer_id })
        .expect("formatting proposal dispatch");
    let projection = match formatting {
        AppCommandOutcome::LanguageToolingUpdated(projection) => projection,
        other => panic!("expected language projection, got {other:?}"),
    };
    let proposal_id = projection
        .operations
        .iter()
        .rev()
        .find_map(|operation| operation.proposal_id)
        .expect("proposal id projected");
    let snapshot = app
        .shell_projection_snapshot("language")
        .expect("shell projection");
    let row = snapshot
        .proposal_ledger_projection
        .rows
        .iter()
        .find(|row| row.proposal_id == proposal_id)
        .expect("proposal row");
    assert_eq!(row.payload_kind, ProposalPayloadKind::WorkspaceEdit);
    assert_eq!(
        app.editor().text(buffer_id).expect("active buffer text"),
        original_text
    );

    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn language_tooling_workflow_creates_rename_preview_without_mutating_disk() {
    let root = create_root();
    let target = root.join("lib.rs");
    std::fs::write(&target, "pub fn old_name() {}\n").expect("write source file");

    let mut app = AppComposition::new();
    app.open_workspace(
        &root,
        WorkspaceTrustState::Trusted,
        PrincipalId("principal-language".to_string()),
    )
    .expect("open workspace");
    app.open_file(target.to_string_lossy())
        .expect("open source file");
    let buffer_id = app.active_buffer_id().expect("active buffer");

    let outcome = app
        .dispatch_ui_intent(CommandDispatchIntent::RequestRenameProposal {
            buffer_id,
            position: position(7),
            new_name: "new_name".to_string(),
        })
        .expect("rename proposal dispatch");
    let projection = match outcome {
        AppCommandOutcome::LanguageToolingUpdated(projection) => projection,
        other => panic!("expected language projection, got {other:?}"),
    };
    assert!(
        projection
            .operations
            .iter()
            .any(|operation| operation.proposal_id.is_some())
    );
    assert_eq!(
        std::fs::read_to_string(&target).expect("disk text"),
        "pub fn old_name() {}\n"
    );

    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn language_tooling_projects_diagnostic_quick_fixes_and_correlates_code_action_preview() {
    let root = create_root();
    let target = root.join("main.rs");
    std::fs::write(&target, "fn main() {\n    // TODO: tighten validation\n}\n")
        .expect("write source file");

    let mut app = AppComposition::new();
    app.open_workspace(
        &root,
        WorkspaceTrustState::Trusted,
        PrincipalId("principal-language".to_string()),
    )
    .expect("open workspace");
    app.open_file(target.to_string_lossy())
        .expect("open source file");
    let buffer_id = app.active_buffer_id().expect("active buffer");

    let diagnostics = app
        .dispatch_ui_intent(CommandDispatchIntent::RefreshOutline { buffer_id })
        .expect("diagnostic refresh dispatch");
    let projection = match diagnostics {
        AppCommandOutcome::LanguageToolingUpdated(projection) => projection,
        other => panic!("expected language projection, got {other:?}"),
    };
    let quick_fix = projection
        .quick_fixes
        .iter()
        .find(|quick_fix| quick_fix.problem_code_label.as_deref() == Some("index.lexical.todo"))
        .expect("TODO diagnostic quick fix projected");
    assert!(
        quick_fix
            .action_id
            .starts_with("quickfix:index.lexical.todo:")
    );
    assert_eq!(quick_fix.kind_label, "quickfix.diagnostic");
    assert_eq!(quick_fix.source_label.as_deref(), Some("legion-index"));
    assert!(quick_fix.problem_range.is_some());
    assert!(quick_fix.proposal_id.is_none());
    let action_id = quick_fix.action_id.clone();

    let code_action = app
        .dispatch_ui_intent(CommandDispatchIntent::RequestCodeActionProposal {
            buffer_id,
            action_id: action_id.clone(),
        })
        .expect("code action dispatch");
    let projection = match code_action {
        AppCommandOutcome::LanguageToolingUpdated(projection) => projection,
        other => panic!("expected language projection, got {other:?}"),
    };
    let proposal_id = projection
        .quick_fixes
        .iter()
        .find(|quick_fix| quick_fix.action_id == action_id)
        .and_then(|quick_fix| quick_fix.proposal_id)
        .expect("quick fix records created proposal id");
    assert!(projection.operations.iter().any(|operation| {
        operation.kind == LanguageToolingOperationKind::CodeActionProposal
            && operation.proposal_id == Some(proposal_id)
    }));
    let shell = app
        .shell_projection_snapshot("language")
        .expect("shell projection");
    let proposal = shell
        .proposal_ledger_projection
        .rows
        .iter()
        .find(|row| row.proposal_id == proposal_id)
        .expect("proposal ledger row");
    assert_eq!(proposal.payload_kind, ProposalPayloadKind::WorkspaceEdit);
    assert_eq!(proposal.lifecycle.state, ProposalLifecycleState::Previewed);

    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn language_tooling_ingests_lsp_diagnostic_projection_and_preserves_lexical_rows() {
    let root = create_root();
    let target = root.join("main.rs");
    std::fs::write(
        &target,
        "fn main() {\n    // TODO: tighten validation\n    let value: u32 = \"hi\";\n}\n",
    )
    .expect("write source file");

    let mut app = AppComposition::new();
    app.open_workspace(
        &root,
        WorkspaceTrustState::Trusted,
        PrincipalId("principal-language".to_string()),
    )
    .expect("open workspace");
    app.open_file(target.to_string_lossy())
        .expect("open source file");
    let buffer_id = app.active_buffer_id().expect("active buffer");

    let seeded = app
        .dispatch_ui_intent(CommandDispatchIntent::RefreshOutline { buffer_id })
        .expect("diagnostic refresh dispatch");
    let seeded = match seeded {
        AppCommandOutcome::LanguageToolingUpdated(projection) => projection,
        other => panic!("expected language projection, got {other:?}"),
    };
    assert!(seeded.quick_fixes.iter().any(|quick_fix| {
        quick_fix.problem_code_label.as_deref() == Some("index.lexical.todo")
            && quick_fix.source_label.as_deref() == Some("legion-index")
    }));

    let payload = json!({
        "uri": "file:///workspace/main.rs",
        "diagnostics": [{
            "range": {
                "start": {"line": 2, "character": 4},
                "end": {"line": 2, "character": 17}
            },
            "severity": 1,
            "code": "E0308",
            "source": "rust-analyzer",
            "message": "mismatched types: expected u32, found SECRET_SOURCE_BODY"
        }]
    });
    let request_id = legion_protocol::LspRequestId(Uuid::now_v7());
    let projection = app
        .ingest_lsp_publish_diagnostics_for_buffer(buffer_id, &payload, true, Some(request_id))
        .expect("ingest LSP diagnostics");

    let lsp_problem = projection
        .problems
        .iter()
        .find(|problem| problem.code_label.as_deref() == Some("E0308"))
        .expect("LSP diagnostic projected");
    assert_eq!(lsp_problem.source_label.as_deref(), Some("rust-analyzer"));
    assert_eq!(lsp_problem.severity, ProtocolDiagnosticSeverity::Error);
    assert!(lsp_problem.range.is_some());
    assert!(
        lsp_problem
            .redaction_hints
            .contains(&RedactionHint::MetadataOnly)
    );
    assert!(!lsp_problem.message.contains("SECRET_SOURCE_BODY"));
    assert!(projection.quick_fixes.iter().any(|quick_fix| {
        quick_fix.problem_code_label.as_deref() == Some("index.lexical.todo")
            && quick_fix.source_label.as_deref() == Some("legion-index")
    }));
    assert!(projection.operations.iter().any(|operation| {
        operation.kind == LanguageToolingOperationKind::Diagnostics
            && operation.request_id == Some(request_id)
            && operation.correlation_id.is_some()
            && operation.causality_id.is_some()
    }));
    assert!(projection.status_message.contains("LSP diagnostics merged"));

    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn language_tooling_ingests_lsp_unavailable_projection_and_keeps_workspace_identity() {
    let root = create_root();
    let target = root.join("main.rs");
    std::fs::write(&target, "fn main() {}\n").expect("write source file");

    let mut app = AppComposition::new();
    app.open_workspace(
        &root,
        WorkspaceTrustState::Trusted,
        PrincipalId("principal-language".to_string()),
    )
    .expect("open workspace");
    app.open_file(target.to_string_lossy())
        .expect("open source file");
    let buffer_id = app.active_buffer_id().expect("active buffer");

    let projection = app
        .ingest_lsp_unavailable_for_buffer(buffer_id, "server_not_running")
        .expect("ingest unavailable projection");

    assert_eq!(projection.buffer_id, Some(buffer_id));
    assert!(projection.workspace_id.is_some());
    assert!(projection.file_id.is_some());
    let fallback = projection
        .problems
        .iter()
        .find(|problem| problem.source_label.as_deref() == Some("lsp"))
        .expect("fallback LSP row");
    assert_eq!(fallback.severity, ProtocolDiagnosticSeverity::Warning);
    assert!(fallback.range.is_none());
    assert!(fallback.message.contains("semantic/index fallback"));
    assert!(
        fallback
            .redaction_hints
            .contains(&RedactionHint::MetadataOnly)
    );
    assert!(projection.status_message.contains("LSP unavailable"));

    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn language_tooling_ingests_lsp_read_side_projections_and_preserves_existing_rows() {
    let root = create_root();
    let target = root.join("lib.rs");
    std::fs::write(&target, "pub fn beta() {}\npub fn caller() { beta(); }\n")
        .expect("write source file");

    let mut app = AppComposition::new();
    app.open_workspace(
        &root,
        WorkspaceTrustState::Trusted,
        PrincipalId("principal-language".to_string()),
    )
    .expect("open workspace");
    app.open_file(target.to_string_lossy())
        .expect("open source file");
    let buffer_id = app.active_buffer_id().expect("active buffer");

    let seeded = app
        .dispatch_ui_intent(CommandDispatchIntent::RequestHover {
            buffer_id,
            position: position(7),
        })
        .expect("seed semantic read projection");
    let seeded = match seeded {
        AppCommandOutcome::LanguageToolingUpdated(projection) => projection,
        other => panic!("expected language projection, got {other:?}"),
    };
    assert!(
        seeded
            .breadcrumbs
            .iter()
            .any(|breadcrumb| breadcrumb.label == "beta")
    );

    let request_id = legion_protocol::LspRequestId(Uuid::now_v7());
    let completion_payload = json!({
        "items": [
            {"label": "beta", "detail": "fn beta()", "kind": 3, "insertText": "beta()"}
        ]
    });
    let projection = app
        .ingest_lsp_completion_response_for_buffer(buffer_id, &completion_payload, Some(request_id))
        .expect("ingest completions");
    assert!(projection.completions.iter().any(|completion| {
        completion.label == "beta" && completion.completion_id.starts_with("lsp-completion-")
    }));
    assert!(
        projection
            .breadcrumbs
            .iter()
            .any(|breadcrumb| breadcrumb.label == "beta")
    );
    assert!(projection.operations.iter().any(|operation| {
        operation.kind == LanguageToolingOperationKind::Completion
            && operation.request_id == Some(request_id)
            && operation.message.contains("LSP completions merged")
    }));

    let hover_payload = json!({
        "contents": {"kind": "markdown", "value": "fn beta() -> ()"},
        "range": {"start": {"line": 0, "character": 7}, "end": {"line": 0, "character": 11}}
    });
    let projection = app
        .ingest_lsp_hover_response_for_buffer(buffer_id, &hover_payload, None)
        .expect("ingest hover");
    let hover = projection.hover.expect("LSP hover projected");
    assert_eq!(hover.label, "fn beta() -> ()");
    assert!(hover.redaction_hints.contains(&RedactionHint::MetadataOnly));

    let locations_payload = json!([
        {
            "uri": "file:///workspace/lib.rs",
            "range": {"start": {"line": 0, "character": 7}, "end": {"line": 0, "character": 11}}
        }
    ]);
    let projection = app
        .ingest_lsp_definition_response_for_buffer(buffer_id, &locations_payload, None)
        .expect("ingest definition");
    assert_eq!(projection.definitions.len(), 1);
    // Product wiring fills navigable paths from file:// URIs for cross-file go-to-def.
    assert_eq!(
        projection.definitions[0]
            .path
            .as_ref()
            .map(|p| p.0.as_str()),
        Some("/workspace/lib.rs")
    );
    let projection = app
        .ingest_lsp_references_response_for_buffer(buffer_id, &locations_payload, None)
        .expect("ingest references");
    assert_eq!(projection.references.len(), 1);
    assert!(projection.status_message.contains("LSP references merged"));

    let request = app
        .lsp_completion_request_for_buffer(buffer_id, position(7), 77)
        .expect("build completion request");
    assert_eq!(request.method.as_deref(), Some("textDocument/completion"));
    assert_eq!(
        request.params.as_ref().unwrap()["position"]["character"].as_u64(),
        Some(7)
    );
    let request = app
        .lsp_hover_request_for_buffer(buffer_id, position(8), 78)
        .expect("build hover request");
    assert_eq!(request.method.as_deref(), Some("textDocument/hover"));
    assert_eq!(
        request.params.as_ref().unwrap()["position"]["character"].as_u64(),
        Some(8)
    );
    let request = app
        .lsp_definition_request_for_buffer(buffer_id, position(9), 79)
        .expect("build definition request");
    assert_eq!(request.method.as_deref(), Some("textDocument/definition"));
    assert_eq!(
        request.params.as_ref().unwrap()["position"]["character"].as_u64(),
        Some(9)
    );
    let request = app
        .lsp_references_request_for_buffer(buffer_id, position(7), 80, true)
        .expect("build references request");
    assert_eq!(request.method.as_deref(), Some("textDocument/references"));
    assert_eq!(
        request.params.as_ref().unwrap()["context"]["includeDeclaration"].as_bool(),
        Some(true)
    );

    let request = app
        .lsp_document_symbol_request_for_buffer(buffer_id, 81)
        .expect("build document-symbol request");
    assert_eq!(
        request.method.as_deref(),
        Some("textDocument/documentSymbol")
    );

    let outline_payload = json!([
        {
            "name": "beta",
            "kind": 12,
            "range": {"start": {"line": 0, "character": 7}, "end": {"line": 0, "character": 11}}
        }
    ]);
    let projection = app
        .ingest_lsp_document_symbol_response_for_buffer(buffer_id, &outline_payload, None)
        .expect("ingest document symbols");
    assert!(
        projection
            .outline
            .iter()
            .any(|symbol| { symbol.label == "beta" && symbol.kind_label == "lsp.symbol.kind.12" })
    );
    assert!(projection.status_message.contains("LSP outline merged"));

    let request = app
        .lsp_inlay_hint_request_for_buffer(
            buffer_id,
            Utf16Range {
                start: Utf16Position {
                    line: 0,
                    character: 0,
                },
                end: Utf16Position {
                    line: 3,
                    character: 0,
                },
            },
            82,
        )
        .expect("build inlay hint request");
    assert_eq!(request.method.as_deref(), Some("textDocument/inlayHint"));

    let inlay_payload = json!([
        {"position": {"line": 0, "character": 11}, "label": ": usize", "kind": 1}
    ]);
    let projection = app
        .ingest_lsp_inlay_hint_response_for_buffer(buffer_id, &inlay_payload, "rust-analyzer", None)
        .expect("ingest inlay hints");
    assert!(
        projection
            .inlay_hints
            .iter()
            .any(|hint| hint.label == ": usize")
    );
    assert!(projection.status_message.contains("LSP inlay hints merged"));

    let request = app
        .lsp_code_lens_request_for_buffer(buffer_id, 83)
        .expect("build code lens request");
    assert_eq!(request.method.as_deref(), Some("textDocument/codeLens"));

    let code_lens_payload = json!([
        {
            "range": {"start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 10}},
            "command": {"title": "Run", "command": "rust-analyzer.run"},
            "data": {"kind": "runnable"}
        }
    ]);
    let projection = app
        .ingest_lsp_code_lens_response_for_buffer(
            buffer_id,
            &code_lens_payload,
            "rust-analyzer",
            None,
        )
        .expect("ingest code lenses");
    assert!(
        projection
            .code_lenses
            .iter()
            .any(|lens| lens.title == "Run")
    );
    assert!(projection.status_message.contains("LSP code lenses merged"));

    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn language_tooling_projects_breadcrumbs_and_sticky_scopes_from_symbols() {
    let root = create_root();
    let target = root.join("lib.rs");
    let source = "mod alpha {\n    pub fn beta() {}\n}\n";
    std::fs::write(&target, source).expect("write source file");

    let mut app = AppComposition::new();
    app.open_workspace(
        &root,
        WorkspaceTrustState::Trusted,
        PrincipalId("principal-language".to_string()),
    )
    .expect("open workspace");
    app.open_file(target.to_string_lossy())
        .expect("open source file");
    let buffer_id = app.active_buffer_id().expect("active buffer");
    let beta_offset = source.find("beta").expect("beta symbol") as u64;

    let outcome = app
        .dispatch_ui_intent(CommandDispatchIntent::RequestHover {
            buffer_id,
            position: position(beta_offset),
        })
        .expect("hover dispatch");
    let projection = match outcome {
        AppCommandOutcome::LanguageToolingUpdated(projection) => projection,
        other => panic!("expected language projection, got {other:?}"),
    };

    assert!(
        projection
            .breadcrumbs
            .iter()
            .any(|breadcrumb| breadcrumb.label == "beta"
                && breadcrumb.source_label == "legion-index")
    );
    assert!(
        projection
            .sticky_scopes
            .iter()
            .any(|scope| scope.label == "beta" && scope.active)
    );
    assert!(
        projection
            .sticky_scopes
            .iter()
            .all(|scope| scope.source_label == "legion-index")
    );

    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn language_tooling_projects_inlay_hints_and_code_lenses_from_symbols() {
    let root = create_root();
    let target = root.join("lib.rs");
    let source = "pub fn beta() {}\npub fn caller() { beta(); }\n";
    std::fs::write(&target, source).expect("write source file");

    let mut app = AppComposition::new();
    app.open_workspace(
        &root,
        WorkspaceTrustState::Trusted,
        PrincipalId("principal-language".to_string()),
    )
    .expect("open workspace");
    app.open_file(target.to_string_lossy())
        .expect("open source file");
    let buffer_id = app.active_buffer_id().expect("active buffer");
    let beta_offset = source.find("beta").expect("beta symbol") as u64;

    let outcome = app
        .dispatch_ui_intent(CommandDispatchIntent::RequestHover {
            buffer_id,
            position: position(beta_offset),
        })
        .expect("hover dispatch");
    let projection = match outcome {
        AppCommandOutcome::LanguageToolingUpdated(projection) => projection,
        other => panic!("expected language projection, got {other:?}"),
    };

    assert!(
        projection
            .inlay_hints
            .iter()
            .any(|hint| hint.label == ": function"
                && hint.kind_label == "symbol-kind"
                && hint.source_label == "legion-index")
    );
    assert!(
        projection
            .code_lenses
            .iter()
            .any(|lens| lens.title == "1 reference"
                && lens.command_label == "Find references"
                && lens.kind_label == "references"
                && lens.data_label.as_deref() == Some("references=1"))
    );

    std::fs::remove_dir_all(&root).ok();
}
