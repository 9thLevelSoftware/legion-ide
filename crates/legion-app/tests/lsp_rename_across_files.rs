//! P2.F1.T5 acceptance: rename across files is reviewable and reversible.
//!
//! The write-side translation has existed since PKT-LSP-C, and single-file
//! apply and rollback have been proven in `checkpoint_restore`. What had never
//! been shown end to end is the thing the task actually promises: a rename that
//! spans more than one file, reviewed before it lands, applied, and then taken
//! back — with every file it touched restored, not just the one that happened
//! to be active.
//!
//! A rename that reverses only the buffer you were looking at is worse than one
//! that cannot be reversed at all, because it leaves the workspace half-renamed
//! and looks like it worked.

use legion_app::AppComposition;
use legion_app::language::{LspReadKind, LspReadOutcome, LspRequestTag, LspWorkerResult};
use legion_protocol::{
    BufferId, CausalityId, LspCapabilitySummary, LspResultStatus, LspServerBinaryProvenance,
    LspServerHealthRecord, PrincipalId, ProposalLifecycleAction, ProposalLifecycleCommand,
    ProposalLifecycleCommandReason, ProposalLifecycleState, ProposalPayload, ProposalRequest,
    ProposalResponse, ProposalRollbackReason, TimestampMillis, WorkspaceTrustState,
};
use legion_ui::CommandDispatchIntent;

fn health() -> LspServerHealthRecord {
    LspServerHealthRecord {
        server_id: legion_protocol::LanguageServerId(1),
        language_id: legion_protocol::LanguageId("rust".to_string()),
        binary_provenance: LspServerBinaryProvenance::Configured,
        binary_path_hash: None,
        artifact_hash: None,
        version: None,
        init_status: LspResultStatus::Fresh,
        capabilities: vec![LspCapabilitySummary {
            capability: "renameProvider".to_string(),
            supported: true,
            dynamic_registration: false,
            option_hash: None,
            redaction_hints: Vec::new(),
            schema_version: 1,
        }],
        diagnostics_latency_ms: None,
        restart_count: 0,
        download_decision_id: None,
        schema_version: 1,
    }
}

/// Two files, both open, both mentioning `widget`.
struct Fixture {
    _root: tempfile::TempDir,
    app: AppComposition,
    lib_path: std::path::PathBuf,
    main_path: std::path::PathBuf,
    active_buffer: BufferId,
    lib_buffer: BufferId,
    /// The URIs the app itself would use for these two buffers. Guessing them
    /// from the temp paths makes the test assert against its own URI-building
    /// rather than the app's resolver.
    lib_uri: String,
    main_uri: String,
}

const LIB_BEFORE: &str = "pub fn widget() {}\n";
const MAIN_BEFORE: &str = "fn main() { widget(); }\n";

fn fixture() -> Fixture {
    let root = tempfile::tempdir().expect("tempdir");
    std::fs::write(
        root.path().join("Cargo.toml"),
        "[package]\nname = \"rename-across\"\n",
    )
    .expect("manifest");
    let lib_path = root.path().join("lib.rs");
    let main_path = root.path().join("main.rs");
    std::fs::write(&lib_path, LIB_BEFORE).expect("lib.rs");
    std::fs::write(&main_path, MAIN_BEFORE).expect("main.rs");

    let mut app = AppComposition::new();
    app.open_workspace(
        root.path(),
        WorkspaceTrustState::Trusted,
        PrincipalId("rename-across".to_string()),
    )
    .expect("open workspace");

    // Both files must be open: the translator resolves each document the edit
    // names, and an unresolvable document is a translation failure rather than
    // a silent partial rename.
    app.open_file(lib_path.to_string_lossy()).expect("open lib");
    let lib_buffer = app.active_buffer_id().expect("lib buffer");
    app.open_file(main_path.to_string_lossy())
        .expect("open main");
    let active_buffer = app.active_buffer_id().expect("active buffer");

    let lib_uri = app
        .document_uri_for_buffer_for_test(lib_buffer)
        .expect("lib uri");
    let main_uri = app
        .document_uri_for_buffer_for_test(active_buffer)
        .expect("main uri");

    Fixture {
        _root: root,
        app,
        lib_path,
        main_path,
        active_buffer,
        lib_buffer,
        lib_uri,
        main_uri,
    }
}

/// A `WorkspaceEdit` renaming `widget` to `gadget` in both files, shaped the
/// way a language server sends one.
fn rename_edit(lib_uri: &str, main_uri: &str) -> serde_json::Value {
    serde_json::json!({
        "changes": {
            lib_uri: [
                {
                    "range": {
                        "start": { "line": 0, "character": 7 },
                        "end": { "line": 0, "character": 13 }
                    },
                    "newText": "gadget"
                }
            ],
            main_uri: [
                {
                    "range": {
                        "start": { "line": 0, "character": 12 },
                        "end": { "line": 0, "character": 18 }
                    },
                    "newText": "gadget"
                }
            ]
        }
    })
}

/// Drive the rename result through the real drain path and return the proposal
/// it produced.
fn rename_proposal(fx: &mut Fixture) -> legion_protocol::WorkspaceProposal {
    let sender = fx.app.inject_lsp_result_sender_for_test(health());
    let snapshot_id = fx
        .app
        .current_snapshot_id_for_test(fx.active_buffer)
        .expect("snapshot id");
    sender
        .send(LspWorkerResult::ReadResult {
            outcome: Ok(LspReadOutcome {
                result: rename_edit(&fx.lib_uri, &fx.main_uri),
                issued_snapshot: snapshot_id,
                status: LspResultStatus::Fresh,
            }),
            tag: LspRequestTag {
                buffer_id: fx.active_buffer,
                kind: LspReadKind::Rename {
                    new_name: "gadget".to_string(),
                },
                snapshot_id,
            },
        })
        .expect("send rename result");
    fx.app.drain_lsp_session();

    let operation = fx
        .app
        .language_tooling_projection()
        .operations
        .into_iter()
        .find(|op| op.proposal_id.is_some())
        .unwrap_or_else(|| {
            panic!(
                "the rename must produce a proposal, not a bare edit; operations were {:?}",
                fx.app.language_tooling_projection().operations
            )
        });
    let proposal_id = operation.proposal_id.expect("proposal id");
    fx.app
        .workspace_proposal_for_id(proposal_id)
        .expect("the proposal must be registered with the coordinator")
}

/// The proposal covers both files, and it is only a proposal — nothing on disk
/// has moved.
#[test]
fn a_cross_file_rename_is_reviewable_before_anything_changes() {
    let mut fx = fixture();
    let proposal = rename_proposal(&mut fx);

    let ProposalPayload::WorkspaceEdit(payload) = &proposal.payload else {
        panic!(
            "a rename must arrive as a workspace edit, got {:?}",
            proposal.payload
        );
    };
    assert_eq!(
        payload.file_edits.len(),
        2,
        "both files the rename touches must be in the proposal, got {:?}",
        payload
            .file_edits
            .iter()
            .map(|edit| edit.file.canonical_path.0.clone())
            .collect::<Vec<_>>()
    );
    let state = fx
        .app
        .shell_projection_snapshot("rename review")
        .expect("snapshot")
        .proposal_ledger_projection
        .rows
        .iter()
        .find(|row| row.proposal_id == proposal.proposal_id)
        .map(|row| row.lifecycle.state)
        .expect("the proposal must appear in the ledger to be reviewable");
    assert_eq!(
        state,
        ProposalLifecycleState::Previewed,
        "a rename must stop at Previewed and wait to be looked at"
    );

    assert_eq!(
        std::fs::read_to_string(&fx.lib_path).expect("lib"),
        LIB_BEFORE,
        "review must not have written anything"
    );
    assert_eq!(
        std::fs::read_to_string(&fx.main_path).expect("main"),
        MAIN_BEFORE
    );
}

/// Approving applies the rename to every file it named, and undo reverses it in
/// every file it named.
///
/// The reversal mechanism is editor undo, not the lifecycle `Rollback` command.
/// `rollback_action_matches_route` declares `EditorUndoGroup` as the reversal
/// for a text-edit route, and text edits produce no durable checkpoint because
/// they never reach disk — the lifecycle command records that a reversal
/// happened, it does not perform one.
#[test]
fn a_cross_file_rename_applies_and_reverses_in_both_files() {
    let mut fx = fixture();
    let proposal = rename_proposal(&mut fx);
    let proposal_id = proposal.proposal_id;

    let applied = fx
        .app
        .approve_and_apply_rename_proposal(proposal_id)
        .expect("approve and apply");
    assert!(
        matches!(applied, ProposalResponse::Applied(_)),
        "expected Applied, got {applied:?}"
    );

    // Apply lands in the editor, not on disk — writing is a separate,
    // separately-authorized step. Both buffers must carry the rename; a rename
    // that stops at the active buffer leaves the workspace half-renamed.
    for (buffer, name) in [(fx.lib_buffer, "lib.rs"), (fx.active_buffer, "main.rs")] {
        let text = fx
            .app
            .editor()
            .text(buffer)
            .expect("buffer text")
            .to_string();
        assert!(
            text.contains("gadget") && !text.contains("widget"),
            "{name} buffer must be renamed, got {text:?}"
        );
    }
    assert_eq!(
        std::fs::read_to_string(&fx.lib_path).expect("lib on disk"),
        LIB_BEFORE,
        "apply alone must not write to disk"
    );

    // Undo has to reach every buffer the rename touched, not only the one that
    // was focused when it ran.
    for (path, buffer, name, before) in [
        (fx.lib_path.clone(), fx.lib_buffer, "lib.rs", LIB_BEFORE),
        (
            fx.main_path.clone(),
            fx.active_buffer,
            "main.rs",
            MAIN_BEFORE,
        ),
    ] {
        fx.app
            .open_file(path.to_string_lossy())
            .expect("focus the file being undone");
        fx.app
            .dispatch_ui_intent(CommandDispatchIntent::Undo { buffer_id: buffer })
            .expect("undo dispatches through app authority");
        let text = fx
            .app
            .editor()
            .text(buffer)
            .expect("buffer text")
            .to_string();
        assert_eq!(
            text, before,
            "undo must reverse the rename in {name} — a rename reversible only in \
             the active buffer leaves the workspace half-renamed and looks like \
             it worked"
        );
    }

    let rollback = ProposalLifecycleCommand {
        proposal_id,
        action: ProposalLifecycleAction::Rollback,
        principal: proposal.principal.clone(),
        capability: proposal.capability.clone(),
        correlation_id: proposal.correlation_id,
        causality_id: CausalityId(uuid::Uuid::now_v7()),
        reason: Some(ProposalLifecycleCommandReason::Rollback(
            ProposalRollbackReason::UserRequested,
        )),
        diagnostics: Vec::new(),
        requested_at: TimestampMillis(0),
        schema_version: 1,
    };
    let response = fx
        .app
        .handle_proposal_request(ProposalRequest::Rollback(rollback))
        .expect("rollback");
    assert!(
        matches!(response, ProposalResponse::RolledBack { .. }),
        "expected RolledBack, got {response:?}"
    );

    let state = fx
        .app
        .shell_projection_snapshot("rename rollback")
        .expect("snapshot")
        .proposal_ledger_projection
        .rows
        .iter()
        .find(|row| row.proposal_id == proposal_id)
        .map(|row| row.lifecycle.state)
        .expect("ledger row");
    assert_eq!(
        state,
        ProposalLifecycleState::RolledBack,
        "the ledger must show the reversal so a reviewer can see it happened"
    );

    assert_eq!(
        std::fs::read_to_string(&fx.lib_path).expect("lib on disk"),
        LIB_BEFORE,
        "the rename never wrote to disk, so there is nothing there to unwind"
    );
    assert_eq!(
        std::fs::read_to_string(&fx.main_path).expect("main on disk"),
        MAIN_BEFORE
    );
}

// ─── The other write-side actions on the same path ──────────────────────────

/// Feed a write-side result through the real drain and return the projection.
fn drain_write_side(
    fx: &mut Fixture,
    kind: LspReadKind,
    result: serde_json::Value,
) -> legion_protocol::LanguageToolingProjection {
    let sender = fx.app.inject_lsp_result_sender_for_test(write_health());
    let snapshot_id = fx
        .app
        .current_snapshot_id_for_test(fx.active_buffer)
        .expect("snapshot id");
    sender
        .send(LspWorkerResult::ReadResult {
            outcome: Ok(LspReadOutcome {
                result,
                issued_snapshot: snapshot_id,
                status: LspResultStatus::Fresh,
            }),
            tag: LspRequestTag {
                buffer_id: fx.active_buffer,
                kind,
                snapshot_id,
            },
        })
        .expect("send result");
    fx.app.drain_lsp_session();
    fx.app.language_tooling_projection()
}

fn write_health() -> LspServerHealthRecord {
    let mut record = health();
    record.capabilities = [
        "renameProvider",
        "documentFormattingProvider",
        "codeActionProvider",
    ]
    .iter()
    .map(|name| LspCapabilitySummary {
        capability: name.to_string(),
        supported: true,
        dynamic_registration: false,
        option_hash: None,
        redaction_hints: Vec::new(),
        schema_version: 1,
    })
    .collect();
    record
}

/// A formatting response is a bare `TextEdit[]`, not a `WorkspaceEdit`. It has
/// to reach the same proposal pipeline anyway — that is the stop condition.
#[test]
fn a_formatting_response_becomes_a_reviewable_proposal() {
    let mut fx = fixture();
    let edits = serde_json::json!([
        {
            "range": {
                "start": { "line": 0, "character": 0 },
                "end": { "line": 0, "character": 3 }
            },
            "newText": "PUB"
        }
    ]);
    let projection = drain_write_side(&mut fx, LspReadKind::Formatting, edits);

    assert!(
        projection.operations.iter().any(|op| op.kind
            == legion_protocol::LanguageToolingOperationKind::FormattingProposal
            && op.proposal_id.is_some()),
        "formatting must produce a proposal, got {:?}",
        projection.operations
    );
    assert_eq!(
        std::fs::read_to_string(&fx.main_path).expect("main on disk"),
        MAIN_BEFORE,
        "generating a formatting proposal must not write"
    );
}

/// Organize-imports is a code action narrowed to one kind; its edit goes
/// through the same pipeline.
#[test]
fn an_organize_imports_code_action_becomes_a_reviewable_proposal() {
    let mut fx = fixture();
    let actions = serde_json::json!([
        {
            "title": "Organize imports",
            "kind": "source.organizeImports",
            "edit": {
                "changes": {
                    fx.main_uri.clone(): [
                        {
                            "range": {
                                "start": { "line": 0, "character": 0 },
                                "end": { "line": 0, "character": 2 }
                            },
                            "newText": "FN"
                        }
                    ]
                }
            }
        }
    ]);
    let projection = drain_write_side(
        &mut fx,
        LspReadKind::CodeAction {
            organize_imports: true,
        },
        actions,
    );

    assert!(
        projection.operations.iter().any(|op| op.kind
            == legion_protocol::LanguageToolingOperationKind::OrganizeImportsProposal
            && op.proposal_id.is_some()),
        "organize-imports must produce a proposal, got {:?}",
        projection.operations
    );
}

/// A code action that carries only a command is refused out loud.
///
/// Running it would need `workspace/executeCommand`, which lets a server mutate
/// the workspace outside the proposal pipeline — the one thing this task's stop
/// condition forbids. Producing nothing quietly would look like the action did
/// not exist.
#[test]
fn a_command_only_code_action_is_refused_rather_than_executed() {
    let mut fx = fixture();
    let actions = serde_json::json!([
        {
            "title": "Run rustfmt",
            "kind": "source.fixAll",
            "command": { "title": "fmt", "command": "rust-analyzer.rustfmt" }
        }
    ]);
    let projection = drain_write_side(
        &mut fx,
        LspReadKind::CodeAction {
            organize_imports: false,
        },
        actions,
    );

    let failure = projection
        .operations
        .iter()
        .find(|op| op.kind == legion_protocol::LanguageToolingOperationKind::CodeActionProposal)
        .expect("the refusal must be recorded as an operation");
    assert!(
        failure.proposal_id.is_none(),
        "a command-only action must not produce a proposal"
    );
    assert!(
        failure.message.contains("command-only"),
        "the refusal must say why, got {:?}",
        failure.message
    );
}

/// The write-side requests fail closed on capability, like the read side.
#[test]
fn write_side_requests_skip_when_the_server_does_not_advertise_them() {
    let mut fx = fixture();
    let mut record = health();
    record.capabilities = ["documentFormattingProvider", "codeActionProvider"]
        .iter()
        .map(|name| LspCapabilitySummary {
            capability: name.to_string(),
            supported: false,
            dynamic_registration: false,
            option_hash: None,
            redaction_hints: Vec::new(),
            schema_version: 1,
        })
        .collect();
    fx.app.set_lsp_health_for_test(record);

    let range = fx
        .app
        .whole_document_utf16_range_for_test(fx.active_buffer)
        .expect("range");
    assert!(
        !fx.app.issue_lsp_formatting_request(fx.active_buffer),
        "formatting must skip when documentFormattingProvider=false"
    );
    assert!(
        !fx.app
            .issue_lsp_code_action_request(fx.active_buffer, range, true),
        "organize-imports must skip when codeActionProvider=false"
    );
}
