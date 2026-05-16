use std::{
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
};

static TEMP_ROOT_COUNTER: AtomicU64 = AtomicU64::new(0);
const FULL_CACHE_BUDGET_BYTES: usize = 5 * 1024 * 1024;

use devil_app::{
    AppCommandExecutionState, AppCommandOutcome, AppCommandRequest, AppComposition,
    AppEditorCommandPort, AppSaveOutcome, AppWorkspaceCommandPort, CommandDispatcher,
    CommandExecutionService, OpenFileIntent,
};
use devil_editor::{TextEdit, TextPosition};
use devil_observability::{InMemoryEventSink, SharedEventSink};
use devil_project::OpenedFileText;
use devil_protocol::{
    BatchProposalPayload, BufferId, BufferVersion, CanonicalPath, CapabilityId, CausalityId,
    ChangedTextRange, CorrelationId, EditBatch, EventEnvelope, FileConflictLifecycleState,
    FileContentVersion, FileId, FileIdentity, FileMetadata, FileTreeNode, PreviewSummary,
    PrincipalId, ProposalAffectedTarget, ProposalBatchAtomicity, ProposalBatchItem,
    ProposalBatchRollbackPolicy, ProposalDenialReason, ProposalId, ProposalPayload,
    ProposalRejectionReason, ProposalRequest, ProposalResponse, ProposalStaleReason,
    ProposalTargetCoverage, ProposalTargetCoverageKind, ProposalTargetKind,
    ProposalVersionPreconditions, SnapshotId, TextOffset, TextRange, TextTransactionDescriptor,
    TimestampMillis, TransactionSource, ViewportProjectionMode, WorkspaceGeneration, WorkspaceId,
    WorkspacePort, WorkspaceProposal, WorkspaceRequest, WorkspaceResponse, WorkspaceTrustState,
};
use devil_ui::{CommandDispatchIntent, ShellLayoutProjection};
use uuid::Uuid;

fn create_root() -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!(
        "devil-app-integration-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |value| value.as_millis() as u64)
            + TEMP_ROOT_COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&root).expect("create temp root");
    root
}

fn app_with_events() -> (AppComposition, InMemoryEventSink) {
    let sink = InMemoryEventSink::new();
    let app = AppComposition::with_event_sink(SharedEventSink::new(sink.clone()));
    (app, sink)
}

fn assert_non_zero_core_ids(event: &EventEnvelope) {
    assert_ne!(event.correlation_id.0, 0, "correlation id must be non-zero");
    assert_ne!(
        event.causality_id.0,
        Uuid::nil(),
        "causality id must be non-zero"
    );
    assert_ne!(event.sequence.0, 0, "event sequence must be non-zero");
}

fn event_names(events: &[EventEnvelope]) -> Vec<&str> {
    events.iter().map(|event| event.event.as_str()).collect()
}

fn assert_events_include_order(events: &[EventEnvelope], expected: &[&str]) {
    let mut cursor = 0;
    for event in events {
        if cursor < expected.len() && event.event == expected[cursor] {
            cursor += 1;
        }
    }
    assert_eq!(
        cursor,
        expected.len(),
        "expected event order {expected:?}, got {:?}",
        event_names(events)
    );
}

fn deterministic_large_text(byte_len: usize) -> String {
    let line = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef\n";
    let mut text = String::with_capacity(byte_len);
    while text.len() + line.len() <= byte_len {
        text.push_str(line);
    }
    while text.len() < byte_len {
        text.push('z');
    }
    text
}

fn empty_preconditions() -> ProposalVersionPreconditions {
    ProposalVersionPreconditions {
        file_version: None,
        buffer_version: None,
        snapshot_id: None,
        generation: None,
        file_content_version: None,
        workspace_generation: None,
        expected_fingerprint: None,
        expected_file_length: None,
        expected_modified_at: None,
    }
}

fn proposal_envelope(payload: ProposalPayload) -> WorkspaceProposal {
    proposal_envelope_with(
        ProposalId(700),
        "editor.write",
        payload,
        empty_preconditions(),
    )
}

fn proposal_envelope_with(
    proposal_id: ProposalId,
    capability: &str,
    payload: ProposalPayload,
    preconditions: ProposalVersionPreconditions,
) -> WorkspaceProposal {
    WorkspaceProposal {
        proposal_id,
        principal: PrincipalId("trusted".to_string()),
        capability: CapabilityId(capability.to_string()),
        correlation_id: CorrelationId(42),
        payload,
        preconditions,
        preview: PreviewSummary {
            summary: "test proposal".to_string(),
            details: Vec::new(),
        },
        expires_at: None,
        created_at: TimestampMillis(1),
    }
}

fn register_validate_preview(app: &mut AppComposition, proposal: &WorkspaceProposal) {
    assert!(matches!(
        app.register_proposal_lifecycle(proposal)
            .expect("register proposal lifecycle"),
        ProposalResponse::Created(_)
    ));
    assert!(matches!(
        app.handle_proposal_request(ProposalRequest::Validate(proposal.clone()))
            .expect("validate proposal"),
        ProposalResponse::Validated(_)
    ));
    assert!(matches!(
        app.handle_proposal_request(ProposalRequest::Preview(proposal.clone()))
            .expect("preview proposal"),
        ProposalResponse::Previewed { .. }
    ));
}

fn workspace_tree(app: &AppComposition, workspace_id: WorkspaceId) -> Vec<FileTreeNode> {
    match app
        .workspace()
        .handle(WorkspaceRequest::ReadTree(workspace_id))
        .expect("read workspace tree")
    {
        WorkspaceResponse::Tree(tree) => tree,
        other => panic!("expected workspace tree, got {other:?}"),
    }
}

fn workspace_node_by_name(
    app: &AppComposition,
    workspace_id: WorkspaceId,
    name: &str,
) -> FileTreeNode {
    workspace_tree(app, workspace_id)
        .into_iter()
        .find(|node| node.name == name)
        .unwrap_or_else(|| panic!("workspace node {name} not found"))
}

fn file_preconditions(
    node: &FileTreeNode,
    workspace_generation: WorkspaceGeneration,
) -> ProposalVersionPreconditions {
    let fingerprint = node
        .metadata
        .as_ref()
        .and_then(|metadata| metadata.fingerprint.clone())
        .expect("file node fingerprint");
    ProposalVersionPreconditions {
        file_version: Some(node.identity.content_version),
        buffer_version: None,
        snapshot_id: None,
        generation: Some(workspace_generation),
        file_content_version: Some(node.identity.content_version),
        workspace_generation: Some(workspace_generation),
        expected_fingerprint: Some(fingerprint),
        expected_file_length: node
            .metadata
            .as_ref()
            .and_then(|metadata| metadata.size_bytes),
        expected_modified_at: node
            .metadata
            .as_ref()
            .and_then(|metadata| metadata.modified_at),
    }
}

fn workspace_preconditions(
    workspace_generation: WorkspaceGeneration,
) -> ProposalVersionPreconditions {
    ProposalVersionPreconditions {
        generation: Some(workspace_generation),
        workspace_generation: Some(workspace_generation),
        ..empty_preconditions()
    }
}

fn target(
    target_id: &str,
    order_file_id: u128,
    path: &str,
    kind: ProposalTargetKind,
) -> ProposalAffectedTarget {
    ProposalAffectedTarget {
        target_id: target_id.to_string(),
        kind,
        workspace_id: Some(WorkspaceId(11)),
        file_id: Some(FileId(order_file_id)),
        buffer_id: None,
        path: Some(CanonicalPath(path.to_string())),
        terminal_session_id: None,
        plugin_id: None,
        remote_authority: None,
        collaboration_session_id: None,
        byte_ranges: Vec::new(),
        redaction_hints: Vec::new(),
    }
}

fn text_edit_payload(file_id: FileId, start: u64, end: u64) -> ProposalPayload {
    ProposalPayload::TextEdit(devil_protocol::TextEditProposal {
        file_id,
        edits: EditBatch {
            edits: vec![devil_protocol::TextEdit {
                range: TextRange::new(TextOffset::byte(start), TextOffset::byte(end)),
                replacement: "replacement".to_string(),
            }],
        },
    })
}

#[test]
fn workspace_vfs_integration_untrusted_save_is_denied_without_disk_mutation() {
    let root = create_root();
    let target = root.join("blocked.txt");
    std::fs::write(&target, "seed").expect("seed file");

    let (mut app, sink) = app_with_events();
    app.open_workspace(
        &root,
        WorkspaceTrustState::Untrusted,
        PrincipalId("untrusted".to_string()),
    )
    .expect("open workspace");
    app.open_file(target.to_string_lossy())
        .expect("open target file");

    app.edit_active_buffer(TextEdit::insert(TextPosition::new(0, 4), "!"))
        .expect("dirty blocked buffer");
    let buffer_id = app.active_buffer_id().expect("active buffer id");

    let save_err = app
        .save_active_buffer()
        .expect("save should return outcome");
    assert!(matches!(save_err, AppSaveOutcome::Rejected(_)));

    assert_eq!(
        std::fs::read_to_string(&target).expect("read blocked file"),
        "seed"
    );
    assert!(app.editor().is_dirty(buffer_id).expect("dirty state"));
    assert_eq!(
        app.editor()
            .buffer_save_state(buffer_id)
            .expect("save state"),
        FileConflictLifecycleState::SaveFailed
    );

    let events = sink.events().expect("captured save-denied events");
    for event in &events {
        assert_non_zero_core_ids(event);
    }
    assert_events_include_order(
        &events,
        &[
            "editor.transaction_applied",
            "proposal.created",
            "proposal.validated",
            "proposal.previewed",
            "security.denial",
            "proposal.rejected",
            "workspace.save_denied",
        ],
    );

    let _ = std::fs::remove_dir_all(&root);
    let _ = save_err;
}

#[test]
fn workspace_vfs_integration_oversized_save_is_rejected_and_preserves_dirty_text() {
    let root = create_root();
    let target = root.join("oversized-save.txt");
    std::fs::write(&target, "seed").expect("seed file");

    let (mut app, sink) = app_with_events();
    app.open_workspace(
        &root,
        WorkspaceTrustState::Trusted,
        PrincipalId("trusted".to_string()),
    )
    .expect("open workspace");
    app.open_file(target.to_string_lossy())
        .expect("open target file");

    let oversized_insert = deterministic_large_text(512 * 1024 + 1);
    app.edit_active_buffer(TextEdit::insert(
        TextPosition::new(0, 4),
        oversized_insert.clone(),
    ))
    .expect("dirty oversized buffer");
    let buffer_id = app.active_buffer_id().expect("active buffer id");

    let save = app
        .save_active_buffer()
        .expect("oversized save should return outcome");
    assert!(matches!(
        &save,
        AppSaveOutcome::Rejected(response) if matches!(response.as_ref(), ProposalResponse::Denied { .. })
    ));

    assert_eq!(
        std::fs::read_to_string(&target).expect("disk content preserved"),
        "seed"
    );
    assert_eq!(
        app.editor().text(buffer_id).expect("dirty text preserved"),
        format!("seed{oversized_insert}")
    );
    assert!(app.editor().is_dirty(buffer_id).expect("dirty retained"));
    assert_eq!(
        app.editor()
            .buffer_save_state(buffer_id)
            .expect("save state"),
        FileConflictLifecycleState::SaveFailed
    );

    let events = sink.events().expect("captured oversized save events");
    for event in &events {
        assert_non_zero_core_ids(event);
    }
    assert_events_include_order(
        &events,
        &[
            "editor.transaction_applied",
            "proposal.created",
            "proposal.validated",
            "proposal.previewed",
            "security.denial",
            "proposal.rejected",
            "workspace.save_denied",
        ],
    );

    let _ = save;
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn workspace_vfs_integration_failed_existing_read_does_not_create_empty_buffer() {
    let root = create_root();
    let missing = root.join("missing.txt");

    let (mut app, sink) = app_with_events();
    app.open_workspace(
        &root,
        WorkspaceTrustState::Trusted,
        PrincipalId("trusted".to_string()),
    )
    .expect("open workspace");

    let open_err = app
        .open_file(missing.to_string_lossy())
        .expect_err("missing existing file must fail");

    assert!(app.active_buffer_id().is_none());
    assert!(!missing.exists());

    let events = sink.events().expect("captured open failure events");
    assert_eq!(event_names(&events), vec!["workspace.open_read_failed"]);
    assert_non_zero_core_ids(&events[0]);

    let _ = open_err;
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn workspace_vfs_integration_explicit_new_file_open_creates_empty_buffer_only_on_create_intent() {
    let root = create_root();
    let target = root.join("new-file.txt");

    let mut app = AppComposition::new();
    app.open_workspace(
        &root,
        WorkspaceTrustState::Trusted,
        PrincipalId("trusted".to_string()),
    )
    .expect("open workspace");

    app.open_file(target.to_string_lossy())
        .expect_err("plain existing open should not create an empty buffer");
    assert!(app.active_buffer_id().is_none());

    app.open_new_file(target.to_string_lossy())
        .expect("explicit create intent should open empty buffer");
    let buffer_id = app.active_buffer_id().expect("active new buffer");
    assert_eq!(app.editor().text(buffer_id).expect("new buffer text"), "");
    assert!(!target.exists());

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn workspace_vfs_integration_open_edit_save_use_engine_and_workspace_ids() {
    let root = create_root();
    let target = root.join("editable.txt");
    std::fs::write(&target, "seed").expect("seed file");

    let (mut app, sink) = app_with_events();
    let opened = app
        .open_workspace(
            &root,
            WorkspaceTrustState::Trusted,
            PrincipalId("trusted".to_string()),
        )
        .expect("open workspace");
    let file_id = app
        .open_file(target.to_string_lossy())
        .expect("open target file");
    let buffer_id = app.active_buffer_id().expect("active buffer id");

    let edit = app
        .edit_active_buffer(TextEdit::insert(TextPosition::new(0, 4), "!"))
        .expect("edit through editor engine");
    assert_eq!(edit.workspace_id, opened.workspace_id);
    assert_eq!(edit.file_id, file_id);
    assert_eq!(edit.buffer_id, buffer_id);

    let save = app
        .save_active_buffer()
        .expect("save through workspace actor");
    let AppSaveOutcome::Saved(save) = save else {
        panic!("expected saved outcome, got {save:?}");
    };
    assert_eq!(save.workspace_id, opened.workspace_id);
    assert_eq!(save.file_id, file_id);
    assert_eq!(save.buffer_id, buffer_id);
    assert_eq!(
        std::fs::read_to_string(&target).expect("read saved file"),
        "seed!"
    );
    assert!(!app.editor().is_dirty(buffer_id).expect("dirty cleared"));
    assert_eq!(
        app.editor()
            .buffer_save_state(buffer_id)
            .expect("save state"),
        FileConflictLifecycleState::Clean
    );

    let after_save_fingerprint = app
        .active_file_fingerprint()
        .expect("active fingerprint after save")
        .clone();
    app.edit_active_buffer(TextEdit::insert(TextPosition::new(0, 5), "?"))
        .expect("dirty again");
    assert!(app.editor().is_dirty(buffer_id).expect("dirty after edit"));
    let second_save = app.save_active_buffer().expect("second save");
    let AppSaveOutcome::Saved(second_save) = second_save else {
        panic!("expected second saved outcome, got {second_save:?}");
    };
    assert_ne!(save.snapshot_id, second_save.snapshot_id);
    assert_ne!(
        after_save_fingerprint,
        app.active_file_fingerprint()
            .expect("updated fingerprint after second save")
            .clone()
    );
    assert!(
        !app.editor()
            .is_dirty(buffer_id)
            .expect("dirty cleared again")
    );

    let events = sink.events().expect("captured edit-save events");
    for event in &events {
        assert_non_zero_core_ids(event);
    }
    assert_events_include_order(
        &events,
        &[
            "editor.transaction_applied",
            "proposal.created",
            "proposal.validated",
            "proposal.previewed",
            "proposal.applied",
            "editor.transaction_applied",
            "proposal.created",
            "proposal.validated",
            "proposal.previewed",
            "proposal.applied",
        ],
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn workspace_vfs_integration_large_file_projection_omits_full_source_text() {
    let root = create_root();
    let target = root.join("large.txt");
    let text = deterministic_large_text(FULL_CACHE_BUDGET_BYTES + 128 * 1024);
    std::fs::write(&target, text).expect("large file");

    let (mut app, _sink) = app_with_events();
    app.open_workspace(
        &root,
        WorkspaceTrustState::Trusted,
        PrincipalId("trusted".to_string()),
    )
    .expect("open workspace");
    app.open_file(target.to_string_lossy())
        .expect("open large target file");

    let projection = app
        .active_buffer_projection(&ShellLayoutProjection::plain("large"))
        .expect("active projection");
    let viewport = projection.viewport.as_ref().expect("viewport projection");
    let payload_bytes = viewport
        .line_slices
        .iter()
        .map(|slice| slice.visible_text.len())
        .sum::<usize>();

    assert!(projection.degraded);
    assert!(projection.small_buffer_text().is_none());
    assert_eq!(viewport.mode, ViewportProjectionMode::DegradedLargeFile);
    assert!(viewport.large_file_status.is_some());
    assert!(payload_bytes < FULL_CACHE_BUDGET_BYTES / 32);
    assert!(viewport.decoration_spans.is_empty());
    assert!(viewport.fold_ranges.is_empty());
    assert!(viewport.semantic_token_overlays.is_empty());

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn workspace_vfs_integration_external_overwrite_between_open_and_save_yields_conflict() {
    let root = create_root();
    let target = root.join("stale.txt");
    std::fs::write(&target, "seed").expect("seed file");

    let (mut app, sink) = app_with_events();
    app.open_workspace(
        &root,
        WorkspaceTrustState::Trusted,
        PrincipalId("trusted".to_string()),
    )
    .expect("open workspace");
    app.open_file(target.to_string_lossy())
        .expect("open target file");
    let buffer_id = app.active_buffer_id().expect("active buffer id");
    app.edit_active_buffer(TextEdit::insert(TextPosition::new(0, 4), "!"))
        .expect("edit buffer");

    std::fs::write(&target, "external").expect("external overwrite");

    let save_err = app
        .save_active_buffer()
        .expect("external overwrite should return save outcome");
    assert!(matches!(
        &save_err,
        AppSaveOutcome::Rejected(response) if matches!(response.as_ref(), ProposalResponse::Stale { .. })
    ));

    assert_eq!(
        std::fs::read_to_string(&target).expect("disk content preserved"),
        "external"
    );
    assert_eq!(app.editor().text(buffer_id).expect("dirty text"), "seed!");
    assert!(app.editor().is_dirty(buffer_id).expect("dirty retained"));
    let conflict = app
        .editor()
        .conflict_state(buffer_id)
        .expect("conflict query")
        .expect("queryable stale/conflict state");
    assert_eq!(
        conflict.context.expected_fingerprint,
        app.active_file_fingerprint().cloned()
    );
    assert_eq!(
        app.editor()
            .buffer_save_state(buffer_id)
            .expect("save state"),
        FileConflictLifecycleState::ConflictDirty
    );

    let events = sink.events().expect("captured stale save events");
    for event in &events {
        assert_non_zero_core_ids(event);
    }
    assert_events_include_order(
        &events,
        &[
            "editor.transaction_applied",
            "proposal.created",
            "proposal.validated",
            "proposal.previewed",
            "proposal.stale_rejected",
        ],
    );

    let _ = save_err;
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn workspace_vfs_integration_non_save_proposals_are_structurally_rejected_without_panic() {
    let mut app = AppComposition::new();
    let payloads = vec![
        text_edit_payload(FileId(9), 0, 4),
        ProposalPayload::CreateFile(devil_protocol::CreateFileProposal {
            path: CanonicalPath("C:/repo/new.rs".to_string()),
            initial_content: Some("fn main() {}".to_string()),
        }),
        ProposalPayload::TerminalCommand(devil_protocol::TerminalCommandProposal {
            session_id: None,
            command: "cargo test".to_string(),
            cwd: Some(CanonicalPath("C:/repo".to_string())),
            env: std::collections::HashMap::new(),
        }),
    ];

    for payload in payloads {
        let proposal = proposal_envelope(payload.clone());
        let validation = app
            .handle_proposal_request(ProposalRequest::Validate(proposal.clone()))
            .expect("validate non-save proposal");
        if matches!(payload, ProposalPayload::TerminalCommand(_)) {
            assert!(matches!(
                validation,
                ProposalResponse::Rejected {
                    reason: ProposalRejectionReason::Unsupported,
                    ..
                }
            ));
        } else {
            let ProposalResponse::Rejected { transition, reason } = validation else {
                panic!("stateless non-save validation should reject, got {validation:?}");
            };
            assert_eq!(reason, ProposalRejectionReason::ValidationFailed);
            assert!(
                transition
                    .diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.code == "proposal.missing_lifecycle_context")
            );
        }

        let apply = app
            .handle_proposal_request(ProposalRequest::Apply(proposal))
            .expect("apply non-save proposal");
        assert!(matches!(
            apply,
            ProposalResponse::Rejected {
                reason: ProposalRejectionReason::ValidationFailed,
                ..
            }
        ));
    }
}

#[test]
fn workspace_vfs_integration_unknown_lifecycle_context_preview_is_rejected() {
    let mut app = AppComposition::new();
    let proposal = proposal_envelope(text_edit_payload(FileId(9), 0, 4));

    let preview = app
        .handle_proposal_request(ProposalRequest::Preview(proposal))
        .expect("preview unknown proposal");

    let ProposalResponse::Rejected { transition, reason } = preview else {
        panic!("unknown lifecycle context should reject preview, got {preview:?}");
    };
    assert_eq!(reason, ProposalRejectionReason::ValidationFailed);
    assert!(
        transition
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "proposal.missing_lifecycle_context")
    );
}

#[test]
fn workspace_vfs_integration_registered_text_edit_apply_mutates_open_buffer() {
    let root = create_root();
    let target = root.join("edit-apply.txt");
    std::fs::write(&target, "seed").expect("seed file");

    let (mut app, sink) = app_with_events();
    let opened = app
        .open_workspace(
            &root,
            WorkspaceTrustState::Trusted,
            PrincipalId("trusted".to_string()),
        )
        .expect("open workspace");
    let file_id = app
        .open_file(target.to_string_lossy())
        .expect("open target file");
    let buffer_id = app.active_buffer_id().expect("active buffer id");
    let node = workspace_node_by_name(&app, opened.workspace_id, "edit-apply.txt");
    let mut preconditions = file_preconditions(&node, opened.generation);
    preconditions.buffer_version = Some(
        app.editor()
            .buffer_version(buffer_id)
            .expect("buffer version"),
    );
    preconditions.snapshot_id = Some(
        app.editor()
            .current_snapshot(buffer_id)
            .expect("current snapshot")
            .snapshot_id,
    );
    let proposal = proposal_envelope_with(
        ProposalId(701),
        "editor.write",
        ProposalPayload::TextEdit(devil_protocol::TextEditProposal {
            file_id,
            edits: EditBatch {
                edits: vec![devil_protocol::TextEdit {
                    range: TextRange::new(TextOffset::byte(0), TextOffset::byte(4)),
                    replacement: "sprout".to_string(),
                }],
            },
        }),
        preconditions,
    );

    register_validate_preview(&mut app, &proposal);
    let response = app
        .handle_proposal_request(ProposalRequest::Apply(proposal))
        .expect("apply text edit proposal");

    assert!(matches!(response, ProposalResponse::Applied(_)));
    assert_eq!(app.editor().text(buffer_id).expect("buffer text"), "sprout");
    assert!(app.editor().is_dirty(buffer_id).expect("dirty state"));

    let events = sink.events().expect("captured proposal apply events");
    assert_events_include_order(
        &events,
        &[
            "proposal.created",
            "proposal.validated",
            "proposal.previewed",
            "editor.transaction_applied",
            "proposal.applied",
        ],
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn workspace_vfs_integration_stale_text_edit_precondition_does_not_apply() {
    let root = create_root();
    let target = root.join("stale-edit.txt");
    std::fs::write(&target, "seed").expect("seed file");

    let mut app = AppComposition::new();
    let opened = app
        .open_workspace(
            &root,
            WorkspaceTrustState::Trusted,
            PrincipalId("trusted".to_string()),
        )
        .expect("open workspace");
    let file_id = app
        .open_file(target.to_string_lossy())
        .expect("open target file");
    let buffer_id = app.active_buffer_id().expect("active buffer id");
    let node = workspace_node_by_name(&app, opened.workspace_id, "stale-edit.txt");
    let mut preconditions = file_preconditions(&node, opened.generation);
    preconditions.buffer_version = Some(
        app.editor()
            .buffer_version(buffer_id)
            .expect("buffer version"),
    );
    preconditions.snapshot_id = Some(
        app.editor()
            .current_snapshot(buffer_id)
            .expect("current snapshot")
            .snapshot_id,
    );
    let proposal = proposal_envelope_with(
        ProposalId(702),
        "editor.write",
        ProposalPayload::TextEdit(devil_protocol::TextEditProposal {
            file_id,
            edits: EditBatch {
                edits: vec![devil_protocol::TextEdit {
                    range: TextRange::new(TextOffset::byte(0), TextOffset::byte(4)),
                    replacement: "sprout".to_string(),
                }],
            },
        }),
        preconditions,
    );

    register_validate_preview(&mut app, &proposal);
    app.edit_active_buffer(TextEdit::insert(TextPosition::new(0, 4), "!"))
        .expect("make proposal stale");
    let response = app
        .handle_proposal_request(ProposalRequest::Apply(proposal))
        .expect("apply stale text edit proposal");

    let ProposalResponse::Stale { stale, .. } = response else {
        panic!("expected stale text edit response, got {response:?}");
    };
    assert_eq!(stale.reason, ProposalStaleReason::BufferVersionMismatch);
    assert_eq!(app.editor().text(buffer_id).expect("buffer text"), "seed!");

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn workspace_vfs_integration_closed_file_create_delete_rename_apply_through_workspace() {
    let root = create_root();
    let delete_target = root.join("delete-me.txt");
    let rename_source = root.join("rename-me.txt");
    let rename_destination = root.join("renamed.txt");
    let create_target = root.join("created.txt");
    std::fs::write(&delete_target, "gone").expect("seed delete file");
    std::fs::write(&rename_source, "move").expect("seed rename file");

    let mut app = AppComposition::new();
    let opened = app
        .open_workspace(
            &root,
            WorkspaceTrustState::Trusted,
            PrincipalId("trusted".to_string()),
        )
        .expect("open workspace");
    let mut generation = opened.generation;

    let create = proposal_envelope_with(
        ProposalId(710),
        "fs.write",
        ProposalPayload::CreateFile(devil_protocol::CreateFileProposal {
            path: CanonicalPath(create_target.to_string_lossy().into_owned()),
            initial_content: Some("new content".to_string()),
        }),
        workspace_preconditions(generation),
    );
    register_validate_preview(&mut app, &create);
    assert!(matches!(
        app.handle_proposal_request(ProposalRequest::Apply(create))
            .expect("apply create proposal"),
        ProposalResponse::Applied(_)
    ));
    assert_eq!(
        std::fs::read_to_string(&create_target).expect("read created file"),
        "new content"
    );
    generation = WorkspaceGeneration(generation.0 + 1);

    let delete_node = workspace_node_by_name(&app, opened.workspace_id, "delete-me.txt");
    let delete = proposal_envelope_with(
        ProposalId(711),
        "fs.write",
        ProposalPayload::DeleteFile(devil_protocol::DeleteFileProposal {
            file: delete_node.identity.clone(),
        }),
        file_preconditions(&delete_node, generation),
    );
    register_validate_preview(&mut app, &delete);
    assert!(matches!(
        app.handle_proposal_request(ProposalRequest::Apply(delete))
            .expect("apply delete proposal"),
        ProposalResponse::Applied(_)
    ));
    assert!(!delete_target.exists());
    generation = WorkspaceGeneration(generation.0 + 1);

    let rename_node = workspace_node_by_name(&app, opened.workspace_id, "rename-me.txt");
    let rename = proposal_envelope_with(
        ProposalId(712),
        "fs.write",
        ProposalPayload::RenameFile(devil_protocol::RenameFileProposal {
            file: rename_node.identity.clone(),
            destination: CanonicalPath(rename_destination.to_string_lossy().into_owned()),
        }),
        file_preconditions(&rename_node, generation),
    );
    register_validate_preview(&mut app, &rename);
    assert!(matches!(
        app.handle_proposal_request(ProposalRequest::Apply(rename))
            .expect("apply rename proposal"),
        ProposalResponse::Applied(_)
    ));
    assert!(!rename_source.exists());
    assert_eq!(
        std::fs::read_to_string(&rename_destination).expect("read renamed file"),
        "move"
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn workspace_vfs_integration_open_file_delete_and_rename_are_denied() {
    let root = create_root();
    let target = root.join("open.txt");
    let destination = root.join("open-renamed.txt");
    std::fs::write(&target, "seed").expect("seed file");

    let mut app = AppComposition::new();
    let opened = app
        .open_workspace(
            &root,
            WorkspaceTrustState::Trusted,
            PrincipalId("trusted".to_string()),
        )
        .expect("open workspace");
    app.open_file(target.to_string_lossy())
        .expect("open target file");
    let node = workspace_node_by_name(&app, opened.workspace_id, "open.txt");
    let preconditions = file_preconditions(&node, opened.generation);

    let delete = proposal_envelope_with(
        ProposalId(720),
        "fs.write",
        ProposalPayload::DeleteFile(devil_protocol::DeleteFileProposal {
            file: node.identity.clone(),
        }),
        preconditions.clone(),
    );
    register_validate_preview(&mut app, &delete);
    let delete_response = app
        .handle_proposal_request(ProposalRequest::Apply(delete))
        .expect("apply open-file delete proposal");
    let ProposalResponse::Denied { transition, reason } = delete_response else {
        panic!("expected open-file delete denial, got {delete_response:?}");
    };
    assert_eq!(reason, ProposalDenialReason::PolicyDenied);
    assert!(
        transition.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "proposal.open_file_workspace_mutation_denied"
        })
    );
    assert!(target.exists());

    let rename = proposal_envelope_with(
        ProposalId(721),
        "fs.write",
        ProposalPayload::RenameFile(devil_protocol::RenameFileProposal {
            file: node.identity.clone(),
            destination: CanonicalPath(destination.to_string_lossy().into_owned()),
        }),
        preconditions,
    );
    register_validate_preview(&mut app, &rename);
    let rename_response = app
        .handle_proposal_request(ProposalRequest::Apply(rename))
        .expect("apply open-file rename proposal");
    assert!(matches!(
        rename_response,
        ProposalResponse::Denied {
            reason: ProposalDenialReason::PolicyDenied,
            ..
        }
    ));
    assert!(target.exists());
    assert!(!destination.exists());

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn workspace_vfs_integration_batch_apply_remains_fail_closed_after_preview() {
    let root = create_root();
    let target = root.join("batch-created.txt");

    let mut app = AppComposition::new();
    app.open_workspace(
        &root,
        WorkspaceTrustState::Trusted,
        PrincipalId("trusted".to_string()),
    )
    .expect("open workspace");
    let batch = BatchProposalPayload {
        batch_id: Uuid::parse_str("77777777-7777-7777-7777-777777777777").unwrap(),
        atomicity: ProposalBatchAtomicity::OrderedNonAtomic,
        rollback_policy: ProposalBatchRollbackPolicy::NotSupported,
        target_coverage: ProposalTargetCoverage {
            coverage_kind: ProposalTargetCoverageKind::Complete,
            targets: vec![ProposalAffectedTarget {
                target_id: "create-target".to_string(),
                kind: ProposalTargetKind::PathOnly,
                workspace_id: app.workspace_id(),
                file_id: None,
                buffer_id: None,
                path: Some(CanonicalPath(target.to_string_lossy().into_owned())),
                terminal_session_id: None,
                plugin_id: None,
                remote_authority: None,
                collaboration_session_id: None,
                byte_ranges: Vec::new(),
                redaction_hints: Vec::new(),
            }],
            omitted_target_count: 0,
            redaction_hints: Vec::new(),
        },
        items: vec![ProposalBatchItem {
            order: 0,
            item_id: "create-target".to_string(),
            payload: Box::new(ProposalPayload::CreateFile(
                devil_protocol::CreateFileProposal {
                    path: CanonicalPath(target.to_string_lossy().into_owned()),
                    initial_content: Some("blocked".to_string()),
                },
            )),
            target_ids: vec!["create-target".to_string()],
            required_capability: CapabilityId("fs.write".to_string()),
            rollback_step_ids: Vec::new(),
        }],
        dependency_edges: Vec::new(),
        rollback_steps: Vec::new(),
        partial_failures: Vec::new(),
        preview_warnings: Vec::new(),
        schema_version: 1,
    };
    let proposal = proposal_envelope_with(
        ProposalId(730),
        "fs.write",
        ProposalPayload::Batch(batch),
        empty_preconditions(),
    );

    register_validate_preview(&mut app, &proposal);
    let response = app
        .handle_proposal_request(ProposalRequest::Apply(proposal))
        .expect("apply batch proposal");

    assert!(matches!(
        response,
        ProposalResponse::Rejected {
            reason: ProposalRejectionReason::Unsupported,
            ..
        }
    ));
    assert!(!target.exists());

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn workspace_vfs_integration_batch_affected_targets_are_visited_in_item_order() {
    let app = AppComposition::new();
    let batch = BatchProposalPayload {
        batch_id: Uuid::parse_str("55555555-5555-5555-5555-555555555555").unwrap(),
        atomicity: ProposalBatchAtomicity::PrepareAllBeforeMutate,
        rollback_policy: ProposalBatchRollbackPolicy::Required,
        target_coverage: ProposalTargetCoverage {
            coverage_kind: ProposalTargetCoverageKind::Complete,
            targets: Vec::new(),
            omitted_target_count: 0,
            redaction_hints: Vec::new(),
        },
        items: vec![
            ProposalBatchItem {
                order: 2,
                item_id: "item-third".to_string(),
                payload: Box::new(ProposalPayload::CreateFile(
                    devil_protocol::CreateFileProposal {
                        path: CanonicalPath("C:/repo/third.rs".to_string()),
                        initial_content: None,
                    },
                )),
                target_ids: Vec::new(),
                required_capability: CapabilityId("fs.write".to_string()),
                rollback_step_ids: Vec::new(),
            },
            ProposalBatchItem {
                order: 0,
                item_id: "item-first".to_string(),
                payload: Box::new(text_edit_payload(FileId(101), 10, 14)),
                target_ids: Vec::new(),
                required_capability: CapabilityId("editor.write".to_string()),
                rollback_step_ids: Vec::new(),
            },
            ProposalBatchItem {
                order: 1,
                item_id: "item-second".to_string(),
                payload: Box::new(ProposalPayload::DeleteFile(
                    devil_protocol::DeleteFileProposal {
                        file: FileIdentity {
                            file_id: FileId(202),
                            workspace_id: WorkspaceId(11),
                            canonical_path: CanonicalPath("C:/repo/second.rs".to_string()),
                            content_version: FileContentVersion(1),
                            content_hash: None,
                        },
                    },
                )),
                target_ids: Vec::new(),
                required_capability: CapabilityId("fs.write".to_string()),
                rollback_step_ids: Vec::new(),
            },
        ],
        dependency_edges: Vec::new(),
        rollback_steps: Vec::new(),
        partial_failures: Vec::new(),
        preview_warnings: Vec::new(),
        schema_version: 1,
    };
    let proposal = proposal_envelope(ProposalPayload::Batch(batch));

    let coverage = app.proposal_target_coverage(&proposal);
    let target_ids = coverage
        .targets
        .iter()
        .map(|target| target.target_id.as_str())
        .collect::<Vec<_>>();

    assert_eq!(
        target_ids,
        vec![
            "text-edit:file:101",
            "delete-file:file:202",
            "create-file:path:C:/repo/third.rs"
        ]
    );
    assert_eq!(coverage.targets[0].byte_ranges.len(), 1);
    assert!(matches!(
        coverage.targets[2].kind,
        ProposalTargetKind::PathOnly
    ));
}

#[test]
fn workspace_vfs_integration_batch_uses_explicit_target_coverage_order() {
    let app = AppComposition::new();
    let explicit_targets = vec![
        target(
            "target-explicit-z",
            303,
            "C:/repo/z.rs",
            ProposalTargetKind::ClosedFile,
        ),
        target(
            "target-explicit-a",
            101,
            "C:/repo/a.rs",
            ProposalTargetKind::OpenBuffer,
        ),
    ];
    let batch = BatchProposalPayload {
        batch_id: Uuid::parse_str("66666666-6666-6666-6666-666666666666").unwrap(),
        atomicity: ProposalBatchAtomicity::PrepareAllBeforeMutate,
        rollback_policy: ProposalBatchRollbackPolicy::NotRequired,
        target_coverage: ProposalTargetCoverage {
            coverage_kind: ProposalTargetCoverageKind::Complete,
            targets: explicit_targets,
            omitted_target_count: 0,
            redaction_hints: Vec::new(),
        },
        items: vec![ProposalBatchItem {
            order: 0,
            item_id: "item-unused-for-order".to_string(),
            payload: Box::new(text_edit_payload(FileId(404), 0, 1)),
            target_ids: vec![
                "target-explicit-z".to_string(),
                "target-explicit-a".to_string(),
            ],
            required_capability: CapabilityId("editor.write".to_string()),
            rollback_step_ids: Vec::new(),
        }],
        dependency_edges: Vec::new(),
        rollback_steps: Vec::new(),
        partial_failures: Vec::new(),
        preview_warnings: Vec::new(),
        schema_version: 1,
    };
    let proposal = proposal_envelope(ProposalPayload::Batch(batch));

    let coverage = app.proposal_target_coverage(&proposal);
    let target_ids = coverage
        .targets
        .iter()
        .map(|target| target.target_id.as_str())
        .collect::<Vec<_>>();

    assert_eq!(target_ids, vec!["target-explicit-z", "target-explicit-a"]);
}

#[test]
fn workspace_vfs_integration_failed_save_preserves_pending_dirty_text() {
    let root = create_root();
    let target = root.join("failed.txt");
    std::fs::write(&target, "seed").expect("seed file");

    let mut app = AppComposition::new();
    app.open_workspace(
        &root,
        WorkspaceTrustState::Trusted,
        PrincipalId("trusted".to_string()),
    )
    .expect("open workspace");
    app.open_file(target.to_string_lossy())
        .expect("open target file");
    let buffer_id = app.active_buffer_id().expect("active buffer id");
    app.edit_active_buffer(TextEdit::insert(TextPosition::new(0, 4), "!"))
        .expect("edit buffer");

    std::fs::remove_file(&target).expect("delete file before save");

    let save_err = app
        .save_active_buffer()
        .expect("deleted file should return save outcome");
    assert!(matches!(save_err, AppSaveOutcome::Rejected(_)));

    assert!(!target.exists());
    assert_eq!(app.editor().text(buffer_id).expect("dirty text"), "seed!");
    assert!(app.editor().is_dirty(buffer_id).expect("dirty retained"));
    assert_eq!(
        app.editor()
            .buffer_save_state(buffer_id)
            .expect("save state"),
        FileConflictLifecycleState::ConflictDirty
    );

    let _ = save_err;
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn workspace_vfs_integration_ui_command_intent_routes_to_engine_apply_edit() {
    let root = create_root();
    let target = root.join("intent.txt");
    std::fs::write(&target, "seed").expect("seed file");

    let (mut app, sink) = app_with_events();
    app.open_workspace(
        &root,
        WorkspaceTrustState::Trusted,
        PrincipalId("trusted".to_string()),
    )
    .expect("open workspace");
    let file_id = app
        .open_file(target.to_string_lossy())
        .expect("open target file");
    let buffer_id = app.active_buffer_id().expect("active buffer id");

    let outcome = app
        .dispatch_ui_intent(CommandDispatchIntent::Insert {
            buffer_id,
            at: TextPosition::new(0, 0),
            text: "x".to_string(),
        })
        .expect("dispatch edit intent");

    match outcome {
        AppCommandOutcome::Edited(descriptor) => {
            assert_eq!(descriptor.file_id, file_id);
            assert_eq!(descriptor.buffer_id, buffer_id);
        }
        other => panic!("expected edited outcome, got {other:?}"),
    }

    let events = sink.events().expect("captured UI edit events");
    assert_eq!(event_names(&events), vec!["editor.transaction_applied"]);
    assert_non_zero_core_ids(&events[0]);

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn workspace_vfs_integration_large_buffer_projection_does_not_populate_full_text() {
    let root = create_root();
    let target = root.join("large.txt");

    // Create a 6MB file to force degraded mode
    let mut large_content = String::with_capacity(6 * 1024 * 1024);
    let line = "padding padding padding padding padding padding padding padding padding padding padding padding padding padding padding padding padding padding padding padding padding padding padding padding padding padding padding padding padding padding padding padding padding padding padding padding padding padding padding padding padding padding padding padding padding padding padding padding padding padding padding padding padding padding padding padding padding padding padding padding padding padding padding padding padding padding padding padding padding padding padding padding padding padding padding padding padding padding padding padding padding padding padding padding padding padding padding padding padding padding padding padding padding padding padding padding padding padding padding padding padding\n";
    while large_content.len() < 6 * 1024 * 1024 {
        large_content.push_str(line);
    }
    std::fs::write(&target, &large_content).expect("large seed file");

    let (mut app, _sink) = app_with_events();
    app.open_workspace(
        &root,
        WorkspaceTrustState::Trusted,
        PrincipalId("trusted".to_string()),
    )
    .expect("open workspace");

    app.open_file(target.to_string_lossy())
        .expect("open target large file");

    let snapshot = app
        .shell_projection_snapshot("title")
        .expect("get projection");
    let active = snapshot.active_buffer_projection;

    // Proving large-buffer projection does not call/populate full-source projection
    assert!(active.degraded, "should be degraded");
    assert!(
        active.small_buffer_text().is_none(),
        "should not have unbounded full text"
    );

    let viewport = active.viewport.expect("should have viewport projection");
    assert!(
        !viewport.line_slices.is_empty(),
        "viewport should have line slices"
    );
    // Ensure viewport is bounded (e.g. only 24 lines based on default height)
    assert!(
        viewport.line_slices.len() <= 100,
        "viewport slices should be bounded, not unbounded"
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[derive(Debug, Default)]
struct MockEditorPort {
    applied: Vec<(BufferId, TextEdit)>,
    undone: Vec<BufferId>,
    redone: Vec<BufferId>,
}

impl AppEditorCommandPort for MockEditorPort {
    fn apply_edit(
        &mut self,
        buffer_id: BufferId,
        edit: TextEdit,
    ) -> Result<TextTransactionDescriptor, devil_app::AppCompositionError> {
        self.applied.push((buffer_id, edit));
        Ok(mock_descriptor(buffer_id, FileId(7)))
    }

    fn undo(
        &mut self,
        buffer_id: BufferId,
    ) -> Result<TextTransactionDescriptor, devil_app::AppCompositionError> {
        self.undone.push(buffer_id);
        Ok(mock_descriptor(buffer_id, FileId(7)))
    }

    fn redo(
        &mut self,
        buffer_id: BufferId,
    ) -> Result<TextTransactionDescriptor, devil_app::AppCompositionError> {
        self.redone.push(buffer_id);
        Ok(mock_descriptor(buffer_id, FileId(7)))
    }
}

#[derive(Debug, Default)]
struct MockWorkspacePort {
    opened: Vec<(WorkspaceId, String, OpenFileIntent)>,
    tree_requests: Vec<WorkspaceId>,
    tree: Vec<FileTreeNode>,
}

impl AppWorkspaceCommandPort for MockWorkspacePort {
    fn open_file_text(
        &self,
        workspace_id: WorkspaceId,
        path: &str,
        intent: OpenFileIntent,
        event_context: Option<devil_app::EventContext>,
    ) -> Result<OpenedFileText, devil_app::AppCompositionError> {
        let _ = (workspace_id, path, intent, event_context);
        unimplemented!("mock command-routing tests do not exercise open execution")
    }

    fn tree_snapshot(
        &self,
        workspace_id: WorkspaceId,
    ) -> Result<Vec<FileTreeNode>, devil_app::AppCompositionError> {
        let mut clone = Self {
            opened: self.opened.clone(),
            tree_requests: self.tree_requests.clone(),
            tree: self.tree.clone(),
        };
        clone.tree_requests.push(workspace_id);
        Ok(clone.tree)
    }
}

fn mock_descriptor(buffer_id: BufferId, file_id: FileId) -> TextTransactionDescriptor {
    TextTransactionDescriptor {
        workspace_id: WorkspaceId(11),
        buffer_id,
        file_id,
        transaction_id: Uuid::nil(),
        correlation_id: devil_protocol::CorrelationId(99),
        source: TransactionSource::User,
        pre_snapshot_id: SnapshotId(1),
        post_snapshot_id: SnapshotId(2),
        pre_buffer_version: BufferVersion(0),
        post_buffer_version: BufferVersion(1),
        changed_ranges: Vec::<ChangedTextRange>::new(),
        causality_id: CausalityId(Uuid::nil()),
        parent_transaction_id: None,
        schema_version: 1,
        undo_group_id: None,
        occurred_at: TimestampMillis(0),
    }
}

fn mock_file_node(file_id: FileId, workspace_id: WorkspaceId, path: &str) -> FileTreeNode {
    FileTreeNode {
        identity: FileIdentity {
            file_id,
            workspace_id,
            canonical_path: CanonicalPath(path.to_string()),
            content_version: FileContentVersion(0),
            content_hash: None,
        },
        name: path.to_string(),
        children: Vec::new(),
        metadata: None::<FileMetadata>,
    }
}

#[test]
fn workspace_vfs_integration_command_dispatcher_routes_without_concrete_ports() {
    let active = devil_app::AppCommandRouteContext {
        workspace_id: Some(WorkspaceId(11)),
        buffer_id: Some(BufferId(5)),
        file_id: Some(FileId(7)),
    };

    let routed = CommandDispatcher::route_intent(
        CommandDispatchIntent::Insert {
            buffer_id: BufferId(5),
            at: TextPosition::new(0, 0),
            text: "mock".to_string(),
        },
        active,
        devil_protocol::CorrelationId(1),
    )
    .expect("route insert intent");
    assert!(matches!(
        routed,
        AppCommandRequest::ApplyEdit {
            buffer_id: BufferId(5),
            ..
        }
    ));

    let save = CommandDispatcher::route_intent(
        CommandDispatchIntent::Save {
            buffer_id: BufferId(5),
        },
        active,
        devil_protocol::CorrelationId(2),
    )
    .expect("route save intent");
    assert_eq!(
        save,
        AppCommandRequest::Save {
            buffer_id: BufferId(5)
        }
    );

    let wrong_buffer = CommandDispatcher::route_intent(
        CommandDispatchIntent::Insert {
            buffer_id: BufferId(6),
            at: TextPosition::new(0, 0),
            text: "blocked".to_string(),
        },
        active,
        devil_protocol::CorrelationId(3),
    )
    .expect_err("wrong active buffer must be rejected before concrete ports");
    let _ = wrong_buffer;
}

#[test]
fn workspace_vfs_integration_command_execution_uses_mock_editor_and_workspace_ports() {
    let workspace_id = WorkspaceId(11);
    let file_id = FileId(7);
    let buffer_id = BufferId(5);
    let mut editor = MockEditorPort::default();
    let workspace = MockWorkspacePort {
        opened: Vec::new(),
        tree_requests: Vec::new(),
        tree: vec![mock_file_node(file_id, workspace_id, "mock.txt")],
    };
    let mut state = AppCommandExecutionState {
        workspace_id: Some(workspace_id),
        active_buffer_id: Some(buffer_id),
        active_file_id: Some(file_id),
    };

    let edit_request = AppCommandRequest::ApplyEdit {
        buffer_id,
        edit: TextEdit::insert(TextPosition::new(0, 0), "x"),
    };
    let edit_outcome =
        CommandExecutionService::execute(&edit_request, &mut editor, &workspace, &mut state)
            .expect("execute edit against mock ports")
            .expect("edit is fully handled by command service");
    assert!(matches!(edit_outcome, AppCommandOutcome::Edited(_)));
    assert_eq!(editor.applied.len(), 1);

    let explorer_outcome = CommandExecutionService::execute(
        &AppCommandRequest::RefreshExplorer,
        &mut editor,
        &workspace,
        &mut state,
    )
    .expect("execute explorer refresh against mock workspace port")
    .expect("refresh is fully handled by command service");
    match explorer_outcome {
        AppCommandOutcome::ExplorerRefreshed(projection) => {
            assert_eq!(projection.nodes.len(), 1);
            assert_eq!(projection.selection.expect("selection").file_id, file_id);
        }
        other => panic!("expected explorer refresh, got {other:?}"),
    }

    let save_outcome = CommandExecutionService::execute(
        &AppCommandRequest::Save { buffer_id },
        &mut editor,
        &workspace,
        &mut state,
    )
    .expect("save routing should not require concrete app services");
    assert!(save_outcome.is_none());
}

#[test]
fn workspace_vfs_integration_path_escape_is_denied_without_disk_mutation() {
    let root = create_root();
    let outside = root
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("devil-app-outside.txt");
    let _ = std::fs::remove_file(&outside);

    let mut app = AppComposition::new();
    app.open_workspace(
        &root,
        WorkspaceTrustState::Trusted,
        PrincipalId("trusted".to_string()),
    )
    .expect("open workspace");

    let open_err = app
        .open_file(outside.to_string_lossy())
        .expect_err("outside open should fail");

    assert!(!outside.exists());

    let _ = open_err;
    let _ = std::fs::remove_dir_all(&root);
}
