//! Regression coverage for the TypeScript organize-imports adapter.

use std::sync::mpsc::{Receiver, SyncSender};
use std::time::Duration;

use crate::AppComposition;
use crate::language::{LspReadOutcome, LspWorkerRequest};
use legion_protocol::{
    BufferId, LanguageId, LanguageServerId, LspCapabilitySummary, LspResultStatus,
    LspServerBinaryProvenance, LspServerHealthRecord, PrincipalId, ProtocolTextRange,
    TextCoordinate, WorkspaceTrustState,
};
use legion_ui::CommandDispatchIntent;

fn health(language: &str) -> LspServerHealthRecord {
    LspServerHealthRecord {
        server_id: LanguageServerId(7),
        language_id: LanguageId(language.into()),
        binary_provenance: LspServerBinaryProvenance::Configured,
        binary_path_hash: None,
        artifact_hash: None,
        version: None,
        init_status: LspResultStatus::Fresh,
        capabilities: vec![
            LspCapabilitySummary {
                capability: "codeActionProvider".into(),
                supported: true,
                dynamic_registration: false,
                option_hash: None,
                redaction_hints: Vec::new(),
                schema_version: 1,
            },
            LspCapabilitySummary {
                capability: "executeCommandProvider".into(),
                supported: true,
                dynamic_registration: false,
                option_hash: None,
                redaction_hints: Vec::new(),
                schema_version: 1,
            },
        ],
        diagnostics_latency_ms: None,
        restart_count: 0,
        download_decision_id: None,
        schema_version: 1,
    }
}

struct Fixture {
    _root: tempfile::TempDir,
    app: AppComposition,
    buffer: BufferId,
    requests: Receiver<LspWorkerRequest>,
    _results: SyncSender<crate::language::LspWorkerResult>,
}

fn fixture(extension: &str, language: &str, advertised: bool) -> Fixture {
    let root = tempfile::tempdir().expect("workspace");
    let source = root.path().join(format!("app.{extension}"));
    std::fs::write(&source, "import { unused } from './lib';\n").expect("source");
    let mut app = AppComposition::new();
    app.open_workspace(
        root.path(),
        WorkspaceTrustState::Trusted,
        PrincipalId("ts-organize-test".into()),
    )
    .expect("workspace");
    app.open_file(source.to_string_lossy()).expect("source");
    let buffer = app.active_buffer_id().expect("buffer");
    let (requests, results) = app.set_lsp_request_harness_for_test(health(language));
    app.lsp_session
        .set_execute_command_ids_for_test(if advertised {
            vec!["_typescript.organizeImports".into()]
        } else {
            Vec::new()
        });
    app.notify_lsp_did_open(buffer);
    let _ = requests
        .recv_timeout(Duration::from_secs(1))
        .expect("didOpen");
    Fixture {
        _root: root,
        app,
        buffer,
        requests,
        _results: results,
    }
}

#[test]
fn typescript_organize_uses_file_and_all_mode_without_candidates() {
    let mut fixture = fixture("ts", "typescript", true);
    fixture
        .app
        .dispatch_ui_intent(CommandDispatchIntent::RequestOrganizeImportsProposal {
            buffer_id: fixture.buffer,
        })
        .expect("organize");
    let LspWorkerRequest::RequestRead { method, params, .. } = fixture
        .requests
        .recv_timeout(Duration::from_secs(1))
        .expect("command")
    else {
        panic!("expected command")
    };
    assert_eq!(method, "workspace/executeCommand");
    assert_eq!(params["command"], "_typescript.organizeImports");
    assert_eq!(
        params["arguments"][1],
        serde_json::json!({"mode":"All", "skipDestructiveCodeActions":false})
    );
    // The command argument must be the filesystem-canonical native path of the
    // opened file. Build that expectation independently of the URI round trip the
    // product uses (`canonical_path_to_uri` + `uri_to_canonical_path`): mirroring
    // that expression would make the assertion a tautology that passes even when
    // both sides are wrong together. `std::fs::canonicalize` is the independent
    // oracle, and `strip_unc_from_pathbuf` is the crate's existing helper for
    // dropping the Windows verbatim `\\?\` prefix it adds.
    //
    // The raw `TempDir` path is *not* usable as the expectation: on the GitHub
    // Windows runner the temp root arrives through the 8.3 short name `RUNNER~1`
    // (canonical: `runneradmin`), and on the GitHub macOS runner through the
    // `/var` -> `/private/var` symlink. The fixture deliberately keeps opening the
    // workspace and the file through that non-canonical spelling, so this assertion
    // is what proves the product normalises it.
    let expected_argument = crate::strip_unc_from_pathbuf(
        std::fs::canonicalize(fixture._root.path().join("app.ts"))
            .expect("fixture file must canonicalize"),
    );
    assert_eq!(
        params["arguments"][0],
        expected_argument.to_string_lossy().as_ref()
    );
    assert!(
        !params["arguments"][0]
            .as_str()
            .unwrap()
            .starts_with(r"\\?\")
    );
    assert!(
        fixture
            .app
            .language_tooling_projection()
            .code_action_candidates
            .is_empty()
    );
}

#[test]
fn unadvertised_typescript_command_keeps_generic_request() {
    let mut fixture = fixture("ts", "typescript", false);
    fixture
        .app
        .dispatch_ui_intent(CommandDispatchIntent::RequestOrganizeImportsProposal {
            buffer_id: fixture.buffer,
        })
        .expect("organize");
    let LspWorkerRequest::RequestRead { method, params, .. } = fixture
        .requests
        .recv_timeout(Duration::from_secs(1))
        .expect("code action")
    else {
        panic!("expected code action")
    };
    assert_eq!(method, "textDocument/codeAction");
    assert_eq!(
        params["context"]["only"],
        serde_json::json!(["source.organizeImports"])
    );
}

#[test]
fn foreign_language_with_command_keeps_generic_request() {
    let mut fixture = fixture("rs", "rust", true);
    fixture
        .app
        .dispatch_ui_intent(CommandDispatchIntent::RequestOrganizeImportsProposal {
            buffer_id: fixture.buffer,
        })
        .expect("organize");
    let LspWorkerRequest::RequestRead { method, params, .. } = fixture
        .requests
        .recv_timeout(Duration::from_secs(1))
        .expect("code action")
    else {
        panic!("expected code action")
    };
    assert_eq!(method, "textDocument/codeAction");
    assert_eq!(
        params["context"]["only"],
        serde_json::json!(["source.organizeImports"])
    );
}

#[test]
fn organize_callback_preserves_operation_and_requires_review_before_mutation() {
    let mut fixture = fixture("ts", "typescript", true);
    let before = fixture.app.buffer_text_for_input(fixture.buffer).unwrap();
    let path = fixture._root.path().join("app.ts");
    let disk_before = std::fs::read_to_string(&path).unwrap();
    fixture
        .app
        .dispatch_ui_intent(CommandDispatchIntent::RequestOrganizeImportsProposal {
            buffer_id: fixture.buffer,
        })
        .unwrap();
    let LspWorkerRequest::RequestRead { tag, .. } = fixture
        .requests
        .recv_timeout(Duration::from_secs(1))
        .unwrap()
    else {
        panic!("expected command")
    };
    let uri = fixture
        .app
        .document_uri_for_buffer_for_test(fixture.buffer)
        .unwrap();
    let (reply, response) = std::sync::mpsc::sync_channel(1);
    fixture
        ._results
        .send(crate::language::LspWorkerResult::ApplyEditRequested {
            request: legion_lsp::LspApplyWorkspaceEditRequest {
                json_rpc_id: 81,
                params: serde_json::json!({"edit":{"changes":{uri:[{
                    "range":{"start":{"line":0,"character":0},"end":{"line":1,"character":0}},
                    "newText":""
                }]}}}),
                context: tag.operation_context,
                deadline: None,
            },
            reply,
            decision: crate::language::ApplyEditDecision::new(),
        })
        .unwrap();
    fixture.app.drain_lsp_session();
    assert!(response.try_recv().is_err(), "the server must await review");
    assert_eq!(
        fixture.app.buffer_text_for_input(fixture.buffer).unwrap(),
        before
    );
    assert_eq!(std::fs::read_to_string(&path).unwrap(), disk_before);
    let projection = fixture.app.language_tooling_projection();
    assert!(
        projection.operations.iter().any(|row| {
            row.kind == legion_protocol::LanguageToolingOperationKind::OrganizeImportsProposal
                && row.proposal_id.is_some()
        }),
        "callback must retain organize operation: {:?}",
        projection.operations
    );
}

#[test]
fn timed_out_organize_rejects_a_late_callback() {
    let mut fixture = fixture("ts", "typescript", true);
    fixture
        .app
        .dispatch_ui_intent(CommandDispatchIntent::RequestOrganizeImportsProposal {
            buffer_id: fixture.buffer,
        })
        .unwrap();
    let LspWorkerRequest::RequestRead { tag, .. } = fixture
        .requests
        .recv_timeout(Duration::from_secs(1))
        .unwrap()
    else {
        panic!("expected command")
    };
    let context = tag.operation_context.clone().expect("command context");
    fixture
        ._results
        .send(crate::language::LspWorkerResult::ReadResult {
            outcome: Err(crate::language::LanguageSessionError::Unavailable),
            tag,
        })
        .unwrap();
    fixture.app.drain_lsp_session();

    let uri = fixture
        .app
        .document_uri_for_buffer_for_test(fixture.buffer)
        .unwrap();
    let (reply, response) = std::sync::mpsc::sync_channel(1);
    fixture
        ._results
        .send(crate::language::LspWorkerResult::ApplyEditRequested {
            request: legion_lsp::LspApplyWorkspaceEditRequest {
                json_rpc_id: 82,
                params: serde_json::json!({"edit":{"changes":{uri:[]}}}),
                context: Some(context),
                deadline: None,
            },
            reply,
            decision: crate::language::ApplyEditDecision::new(),
        })
        .unwrap();
    fixture.app.drain_lsp_session();
    let result = response.recv_timeout(Duration::from_secs(1)).unwrap();
    assert!(!result.applied);
    assert!(
        result
            .failure_reason
            .as_deref()
            .is_some_and(|reason| reason.contains("not an active selected command"))
    );
    assert!(
        fixture
            .app
            .language_tooling_projection()
            .operations
            .iter()
            .all(|row| row.proposal_id.is_none())
    );
}

#[test]
fn changed_editor_rejects_successful_old_organize_response() {
    let mut fixture = fixture("ts", "typescript", true);
    fixture
        .app
        .dispatch_ui_intent(CommandDispatchIntent::RequestOrganizeImportsProposal {
            buffer_id: fixture.buffer,
        })
        .unwrap();
    let LspWorkerRequest::RequestRead { tag, .. } = fixture
        .requests
        .recv_timeout(Duration::from_secs(1))
        .unwrap()
    else {
        panic!("expected command")
    };
    let request_id = tag
        .operation_context
        .as_ref()
        .expect("command context")
        .request_id
        .0
        .to_string();
    let operation_id = tag.operation_id.clone().expect("operation id");
    fixture
        .app
        .dispatch_ui_intent(CommandDispatchIntent::Replace {
            buffer_id: fixture.buffer,
            range: ProtocolTextRange {
                start: TextCoordinate {
                    line: 0,
                    character: 0,
                    byte_offset: None,
                    utf16_offset: None,
                },
                end: TextCoordinate {
                    line: 0,
                    character: 0,
                    byte_offset: None,
                    utf16_offset: None,
                },
            },
            replacement: "// changed\n".into(),
        })
        .unwrap();
    fixture
        ._results
        .send(crate::language::LspWorkerResult::ReadResult {
            outcome: Ok(LspReadOutcome {
                result: serde_json::Value::Null,
                issued_snapshot: tag.snapshot_id,
                status: LspResultStatus::Fresh,
            }),
            tag,
        })
        .unwrap();
    fixture.app.drain_lsp_session();
    assert!(
        !fixture
            .app
            .pending_code_action_contexts
            .contains_key(&request_id)
    );
    let operation = fixture
        .app
        .language_tooling_projection()
        .operations
        .into_iter()
        .find(|row| row.operation_id == operation_id)
        .expect("organize operation");
    assert_eq!(
        operation.status,
        legion_protocol::LanguageToolingStatusKind::Stale
    );
    assert!(operation.proposal_id.is_none());
    assert_eq!(
        fixture.app.buffer_text_for_input(fixture.buffer).unwrap(),
        "// changed\nimport { unused } from './lib';\n"
    );
}
