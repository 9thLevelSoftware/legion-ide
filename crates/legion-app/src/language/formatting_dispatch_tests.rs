//! Dispatch-level formatting tests: a live request owns the eventual proposal.

use std::sync::mpsc::{Receiver, SyncSender};
use std::time::Duration;

use crate::AppComposition;
use crate::language::{LspReadOutcome, LspWorkerRequest, LspWorkerResult};
use legion_protocol::{
    BufferId, LanguageId, LanguageServerId, LanguageToolingOperationKind,
    LanguageToolingStatusKind, LspCapabilitySummary, LspResultStatus, LspServerBinaryProvenance,
    LspServerHealthRecord, PrincipalId, ProposalPayload, TextCoordinate, WorkspaceTrustState,
};
use legion_ui::CommandDispatchIntent;

fn health(capabilities: &[&str]) -> LspServerHealthRecord {
    LspServerHealthRecord {
        server_id: LanguageServerId(7),
        language_id: LanguageId("typescript".into()),
        binary_provenance: LspServerBinaryProvenance::Configured,
        binary_path_hash: None,
        artifact_hash: None,
        version: None,
        init_status: LspResultStatus::Fresh,
        capabilities: capabilities
            .iter()
            .map(|capability| LspCapabilitySummary {
                capability: (*capability).into(),
                supported: true,
                dynamic_registration: false,
                option_hash: None,
                redaction_hints: Vec::new(),
                schema_version: 1,
            })
            .collect(),
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
    results: SyncSender<LspWorkerResult>,
}

fn fixture(capabilities: &[&str]) -> Fixture {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("format.ts");
    std::fs::write(&source, "export const value={answer:42};\n").unwrap();
    let mut app = AppComposition::new();
    app.open_workspace(
        root.path(),
        WorkspaceTrustState::Trusted,
        PrincipalId("format-dispatch-tests".into()),
    )
    .unwrap();
    app.open_file(source.to_string_lossy()).unwrap();
    let buffer = app.active_buffer_id().unwrap();
    let (requests, results) = app.set_lsp_request_harness_for_test(health(capabilities));
    app.notify_lsp_did_open(buffer);
    let _ = requests.recv_timeout(Duration::from_secs(1)).unwrap();
    Fixture {
        _root: root,
        app,
        buffer,
        requests,
        results,
    }
}

#[test]
fn live_formatting_dispatch_waits_for_real_response() {
    let mut fixture = fixture(&["documentFormattingProvider"]);
    fixture
        .app
        .dispatch_ui_intent(CommandDispatchIntent::RequestFormattingProposal {
            buffer_id: fixture.buffer,
        })
        .unwrap();
    let tag = match fixture
        .requests
        .recv_timeout(Duration::from_secs(1))
        .unwrap()
    {
        LspWorkerRequest::RequestRead { tag, .. } => tag,
        _ => panic!("expected formatting request"),
    };
    assert!(
        fixture
            .app
            .language_tooling_projection()
            .operations
            .iter()
            .filter(|row| row.kind == LanguageToolingOperationKind::FormattingProposal)
            .all(|row| row.proposal_id.is_none())
    );
    fixture
        .results
        .send(LspWorkerResult::ReadResult {
            outcome: Ok(LspReadOutcome {
                result: serde_json::json!([{
                    "range": {"start":{"line":0,"character":0},"end":{"line":0,"character":31}},
                    "newText": "export const value = { answer: 42 };\n"
                }]),
                issued_snapshot: tag.snapshot_id,
                status: LspResultStatus::Fresh,
            }),
            tag,
        })
        .unwrap();
    fixture.app.drain_lsp_session();
    let operation = fixture
        .app
        .language_tooling_projection()
        .operations
        .into_iter()
        .find(|row| row.kind == LanguageToolingOperationKind::FormattingProposal)
        .unwrap();
    let proposal = fixture
        .app
        .workspace_proposal_for_id(operation.proposal_id.unwrap())
        .unwrap();
    let ProposalPayload::WorkspaceEdit(edit) = proposal.payload else {
        panic!("expected workspace edit")
    };
    assert!(!edit.file_edits[0].edits.edits.is_empty());
}

#[test]
fn formatting_without_capability_records_failure_without_proposal() {
    let mut fixture = fixture(&[]);
    fixture
        .app
        .dispatch_ui_intent(CommandDispatchIntent::RequestFormattingProposal {
            buffer_id: fixture.buffer,
        })
        .unwrap();
    assert!(fixture.requests.try_recv().is_err());
    let projection = fixture.app.language_tooling_projection();
    assert_eq!(projection.status, LanguageToolingStatusKind::Failed);
    assert!(
        projection
            .operations
            .iter()
            .filter(|row| row.kind == LanguageToolingOperationKind::FormattingProposal)
            .all(|row| row.proposal_id.is_none())
    );
}

#[test]
fn rename_dispatch_waits_for_real_response_without_fake_preview() {
    let mut fixture = fixture(&["renameProvider"]);
    fixture
        .app
        .dispatch_ui_intent(CommandDispatchIntent::RequestRenameProposal {
            buffer_id: fixture.buffer,
            position: TextCoordinate {
                line: 0,
                character: 13,
                byte_offset: None,
                utf16_offset: None,
            },
            new_name: "answer".into(),
        })
        .unwrap();
    let tag = match fixture
        .requests
        .recv_timeout(Duration::from_secs(1))
        .unwrap()
    {
        LspWorkerRequest::RequestRead { tag, .. } => tag,
        _ => panic!("expected rename request"),
    };
    assert!(
        fixture
            .app
            .language_tooling_projection()
            .operations
            .iter()
            .filter(|row| row.kind == LanguageToolingOperationKind::RenameProposal)
            .all(|row| row.proposal_id.is_none())
    );
    let uri = fixture
        .app
        .document_uri_for_buffer_for_test(fixture.buffer)
        .unwrap();
    fixture
        .results
        .send(LspWorkerResult::ReadResult {
            outcome: Ok(LspReadOutcome {
                result: serde_json::json!({"changes":{uri:[{
                    "range":{"start":{"line":0,"character":13},"end":{"line":0,"character":19}},
                    "newText":"answer"
                }]}}),
                issued_snapshot: tag.snapshot_id,
                status: LspResultStatus::Fresh,
            }),
            tag,
        })
        .unwrap();
    fixture.app.drain_lsp_session();
    let operation = fixture
        .app
        .language_tooling_projection()
        .operations
        .into_iter()
        .find(|row| row.kind == LanguageToolingOperationKind::RenameProposal)
        .unwrap();
    assert!(operation.proposal_id.is_some());
}
