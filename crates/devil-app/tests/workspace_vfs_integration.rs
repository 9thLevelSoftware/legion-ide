use std::{
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
};

static TEMP_ROOT_COUNTER: AtomicU64 = AtomicU64::new(0);
const FULL_CACHE_BUDGET_BYTES: usize = 5 * 1024 * 1024;

use devil_app::{
    AppCommandExecutionState, AppCommandOutcome, AppCommandRequest, AppComposition,
    AppEditorCommandPort, AppSaveOutcome, AppWorkspaceCommandPort, BatchExecutionJournalItemState,
    BatchExecutionJournalStageState, BatchExecutionStage, BatchPlanningSemantics,
    BatchPreflightRoute, BatchRollbackContractStatus, CommandDispatcher, CommandExecutionService,
    OpenFileIntent,
};
use devil_editor::{TextEdit, TextPosition};
use devil_observability::{InMemoryEventSink, SharedEventSink};
use devil_project::OpenedFileText;
use devil_protocol::{
    BatchProposalPayload, BufferId, BufferVersion, CanonicalPath, CapabilityId, CausalityId,
    ChangedTextRange, CorrelationId, EditBatch, EventEnvelope, FileConflictLifecycleState,
    FileContentVersion, FileId, FileIdentity, FileMetadata, FileTreeNode, PreviewSummary,
    PrincipalId, ProposalAffectedTarget, ProposalAuditRecord, ProposalBatchAtomicity,
    ProposalBatchDependency, ProposalBatchDependencyKind, ProposalBatchItem,
    ProposalBatchRollbackPolicy, ProposalDenialReason, ProposalId, ProposalLifecycleAction,
    ProposalLifecycleCommand, ProposalLifecycleCommandReason, ProposalLifecycleState,
    ProposalPayload, ProposalRejectionReason, ProposalRequest, ProposalResponse,
    ProposalRollbackAction, ProposalRollbackReason, ProposalRollbackStep, ProposalStaleReason,
    ProposalTargetCoverage, ProposalTargetCoverageKind, ProposalTargetKind,
    ProposalVersionPreconditions, RedactionHint, SaveConflictPolicy, SaveFileProposal, SaveIntent,
    SnapshotId, StorageRepositoryRequest, StorageRepositoryResponse, TextCoordinate, TextOffset,
    TextRange, TextTransactionDescriptor, TimestampMillis, TransactionSource, TrustDecisionContext,
    ViewportProjectionMode, WorkspaceGeneration, WorkspaceId, WorkspacePort, WorkspaceProposal,
    WorkspaceRequest, WorkspaceResponse, WorkspaceTrustState,
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

fn ui_text_coordinate(line: u32, character: u32) -> TextCoordinate {
    TextCoordinate {
        line,
        character,
        byte_offset: None,
        utf16_offset: None,
    }
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

fn audit_record(app: &AppComposition, proposal_id: ProposalId) -> ProposalAuditRecord {
    match app
        .storage_port()
        .handle(StorageRepositoryRequest::ReadProposalAuditRecord(
            proposal_id,
        ))
        .expect("read proposal audit record")
    {
        StorageRepositoryResponse::ProposalAuditRecord(Some(record)) => record,
        other => panic!("expected proposal audit record, got {other:?}"),
    }
}

fn assert_metadata_only_audit(record: &ProposalAuditRecord) {
    assert_ne!(record.correlation_id.0, 0);
    assert_ne!(record.causality_id.0, Uuid::nil());
    assert_ne!(record.schema_version, 0);
    assert_eq!(record.redaction_hints, vec![RedactionHint::MetadataOnly]);
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

fn preflight_target(
    target_id: &str,
    workspace_id: Option<WorkspaceId>,
    file_id: Option<FileId>,
    path: &Path,
    kind: ProposalTargetKind,
) -> ProposalAffectedTarget {
    ProposalAffectedTarget {
        target_id: target_id.to_string(),
        kind,
        workspace_id,
        file_id,
        buffer_id: None,
        path: Some(CanonicalPath(path.to_string_lossy().into_owned())),
        terminal_session_id: None,
        plugin_id: None,
        remote_authority: None,
        collaboration_session_id: None,
        byte_ranges: Vec::new(),
        redaction_hints: Vec::new(),
    }
}

fn workspace_edit_path_target(target_id: String, path: &Path) -> ProposalAffectedTarget {
    ProposalAffectedTarget {
        target_id,
        kind: ProposalTargetKind::PathOnly,
        workspace_id: None,
        file_id: None,
        buffer_id: None,
        path: Some(CanonicalPath(path.to_string_lossy().into_owned())),
        terminal_session_id: None,
        plugin_id: None,
        remote_authority: None,
        collaboration_session_id: None,
        byte_ranges: Vec::new(),
        redaction_hints: Vec::new(),
    }
}

fn save_payload_for_open_buffer(
    file: FileIdentity,
    editor_file_id: FileId,
    buffer_id: BufferId,
    snapshot_id: SnapshotId,
    buffer_version: BufferVersion,
    workspace_generation: WorkspaceGeneration,
    expected_fingerprint: devil_protocol::FileFingerprint,
) -> ProposalPayload {
    ProposalPayload::SaveFile(SaveFileProposal {
        file_id: editor_file_id,
        file: file.clone(),
        buffer_id,
        snapshot_id,
        buffer_version,
        file_content_version: file.content_version,
        workspace_generation,
        expected_fingerprint: Some(expected_fingerprint),
        save_intent: SaveIntent::ExternalCommand,
        conflict_policy: SaveConflictPolicy::RejectIfChanged,
        trust_decision: TrustDecisionContext {
            workspace_trust_state: WorkspaceTrustState::Trusted,
            decision_id: None,
            decided_at: Some(TimestampMillis(1)),
        },
        required_capability: CapabilityId("fs.write".to_string()),
        principal: PrincipalId("trusted".to_string()),
        correlation_id: CorrelationId(42),
        diagnostics: Vec::new(),
    })
}

fn batch_payload_for_test(
    atomicity: ProposalBatchAtomicity,
    rollback_policy: ProposalBatchRollbackPolicy,
    targets: Vec<ProposalAffectedTarget>,
    items: Vec<ProposalBatchItem>,
) -> BatchProposalPayload {
    BatchProposalPayload {
        batch_id: Uuid::parse_str("77777777-7777-7777-7777-777777777777").unwrap(),
        atomicity,
        rollback_policy,
        target_coverage: ProposalTargetCoverage {
            coverage_kind: ProposalTargetCoverageKind::Complete,
            targets,
            omitted_target_count: 0,
            redaction_hints: Vec::new(),
        },
        items,
        dependency_edges: Vec::new(),
        rollback_steps: Vec::new(),
        partial_failures: Vec::new(),
        preview_warnings: Vec::new(),
        schema_version: 1,
    }
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
    assert_eq!(
        app.editor().text(buffer_id).expect("dirty text preserved"),
        "seed!"
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
            "proposal.audit_recorded",
            "editor.transaction_applied",
            "proposal.created",
            "proposal.validated",
            "proposal.previewed",
            "proposal.applied",
            "proposal.audit_recorded",
        ],
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn workspace_vfs_integration_large_file_projection_omits_full_source_text() {
    let root = create_root();
    let target = root.join("large.txt");
    let text = deterministic_large_text(FULL_CACHE_BUDGET_BYTES + 128 * 1024);
    let text_len = text.len();
    std::fs::write(&target, &text).expect("large file");

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
    let status = viewport
        .large_file_status
        .as_ref()
        .expect("large-file status");
    assert_eq!(status.byte_len as usize, text_len);
    assert!(status.message.contains("viewport payloads are chunked"));
    assert!(!viewport.line_slices.is_empty());
    assert!(viewport.line_slices.len() <= 24);
    assert!(
        viewport
            .line_slices
            .iter()
            .all(|slice| slice.visible_text.len() < text_len)
    );
    assert!(payload_bytes < FULL_CACHE_BUDGET_BYTES / 32);
    assert!(payload_bytes < text_len / 32);
    assert!(viewport.decoration_spans.is_empty());
    assert!(viewport.fold_ranges.is_empty());
    assert!(viewport.semantic_token_overlays.is_empty());

    let shell_snapshot = app
        .shell_projection_snapshot("large shell")
        .expect("shell projection snapshot");
    let shell_active = &shell_snapshot.active_buffer_projection;
    let shell_viewport = shell_active
        .viewport
        .as_ref()
        .expect("shell viewport projection");
    let shell_payload_bytes = shell_viewport
        .line_slices
        .iter()
        .map(|slice| slice.visible_text.len())
        .sum::<usize>();

    assert!(shell_active.degraded);
    assert!(shell_active.small_buffer_text().is_none());
    assert_eq!(
        shell_viewport.mode,
        ViewportProjectionMode::DegradedLargeFile
    );
    assert!(shell_viewport.large_file_status.is_some());
    assert!(shell_payload_bytes < text_len / 32);
    assert!(
        shell_viewport
            .line_slices
            .iter()
            .all(|slice| slice.visible_text.len() < text_len)
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn workspace_vfs_integration_small_buffer_projection_keeps_bounded_preview() {
    let root = create_root();
    let target = root.join("small.txt");
    std::fs::write(&target, "seed\nsmall\n").expect("small file");

    let (mut app, _sink) = app_with_events();
    app.open_workspace(
        &root,
        WorkspaceTrustState::Trusted,
        PrincipalId("trusted".to_string()),
    )
    .expect("open workspace");
    app.open_file(target.to_string_lossy())
        .expect("open small target file");

    let projection = app
        .active_buffer_projection(&ShellLayoutProjection::plain("small"))
        .expect("active projection");
    let viewport = projection.viewport.as_ref().expect("viewport projection");

    assert!(!projection.degraded);
    assert_eq!(projection.small_buffer_text(), Some("seed\nsmall\n"));
    assert_eq!(viewport.mode, ViewportProjectionMode::Normal);
    assert!(viewport.large_file_status.is_none());

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
    let other = root.join("untouched.txt");
    std::fs::write(&target, "seed").expect("seed file");
    std::fs::write(&other, "untouched").expect("seed untouched file");

    let (mut app, sink) = app_with_events();
    let opened = app
        .open_workspace(
            &root,
            WorkspaceTrustState::Trusted,
            PrincipalId("trusted".to_string()),
        )
        .expect("open workspace");
    app.open_file(other.to_string_lossy())
        .expect("open untouched file");
    let other_buffer_id = app.active_buffer_id().expect("other buffer id");
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
    assert_eq!(
        app.editor()
            .text(other_buffer_id)
            .expect("other buffer text"),
        "untouched"
    );
    assert!(app.editor().is_dirty(buffer_id).expect("dirty state"));
    assert_eq!(app.editor().undo_len(buffer_id).expect("undo len"), 1);

    let events = sink.events().expect("captured proposal apply events");
    assert_events_include_order(
        &events,
        &[
            "proposal.created",
            "proposal.validated",
            "proposal.previewed",
            "editor.transaction_applied",
            "proposal.applied",
            "proposal.audit_recorded",
        ],
    );
    let audit = audit_record(&app, ProposalId(701));
    assert_eq!(audit.lifecycle_state, ProposalLifecycleState::Applied);
    assert_metadata_only_audit(&audit);

    let undo = app
        .dispatch_ui_intent(CommandDispatchIntent::Undo { buffer_id })
        .expect("undo proposal edit");
    assert!(matches!(undo, AppCommandOutcome::Edited(_)));
    assert_eq!(app.editor().text(buffer_id).expect("undo text"), "seed");
    assert_eq!(
        app.editor()
            .text(other_buffer_id)
            .expect("other buffer remains unchanged"),
        "untouched"
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn workspace_vfs_integration_rollback_lifecycle_emits_audit_before_success() {
    let root = create_root();
    let target = root.join("rollback-audit.txt");
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
    let node = workspace_node_by_name(&app, opened.workspace_id, "rollback-audit.txt");
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
    assert!(matches!(
        app.handle_proposal_request(ProposalRequest::Apply(proposal.clone()))
            .expect("apply text edit proposal"),
        ProposalResponse::Applied(_)
    ));

    let rollback = ProposalLifecycleCommand {
        proposal_id: proposal.proposal_id,
        action: ProposalLifecycleAction::Rollback,
        principal: proposal.principal.clone(),
        capability: proposal.capability.clone(),
        correlation_id: proposal.correlation_id,
        causality_id: CausalityId(Uuid::now_v7()),
        reason: Some(ProposalLifecycleCommandReason::Rollback(
            ProposalRollbackReason::UserRequested,
        )),
        diagnostics: Vec::new(),
        requested_at: TimestampMillis(2),
        schema_version: 1,
    };
    let response = app
        .handle_proposal_request(ProposalRequest::Rollback(rollback))
        .expect("rollback proposal lifecycle");

    assert!(matches!(response, ProposalResponse::RolledBack { .. }));
    let audit = audit_record(&app, ProposalId(702));
    assert_eq!(audit.lifecycle_state, ProposalLifecycleState::RolledBack);
    assert_metadata_only_audit(&audit);
    let events = sink.events().expect("captured rollback audit events");
    assert_events_include_order(
        &events,
        &[
            "proposal.applied",
            "proposal.audit_recorded",
            "proposal.rolled_back",
            "proposal.audit_recorded",
        ],
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn workspace_vfs_integration_rollback_audit_failure_records_failed_lifecycle() {
    let root = create_root();
    let target = root.join("rollback-audit-failure.txt");
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
    let node = workspace_node_by_name(&app, opened.workspace_id, "rollback-audit-failure.txt");
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
        ProposalId(742),
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
    let proposal_id = proposal.proposal_id;

    register_validate_preview(&mut app, &proposal);
    assert!(matches!(
        app.handle_proposal_request(ProposalRequest::Apply(proposal.clone()))
            .expect("apply text edit proposal"),
        ProposalResponse::Applied(_)
    ));
    app.fail_next_proposal_audit_write_for_test();
    let rollback = ProposalLifecycleCommand {
        proposal_id,
        action: ProposalLifecycleAction::Rollback,
        principal: proposal.principal.clone(),
        capability: proposal.capability.clone(),
        correlation_id: proposal.correlation_id,
        causality_id: CausalityId(Uuid::now_v7()),
        reason: Some(ProposalLifecycleCommandReason::Rollback(
            ProposalRollbackReason::UserRequested,
        )),
        diagnostics: Vec::new(),
        requested_at: TimestampMillis(2),
        schema_version: 1,
    };
    let response = app
        .handle_proposal_request(ProposalRequest::Rollback(rollback))
        .expect("rollback proposal lifecycle with audit failure");

    assert!(matches!(
        response,
        ProposalResponse::Failed {
            reason: devil_protocol::ProposalFailureReason::StorageFailed,
            ..
        }
    ));
    let shell = app
        .shell_projection_snapshot("rollback audit failure ledger")
        .expect("shell projection after rollback audit failure");
    let row = shell
        .proposal_ledger_projection
        .rows
        .iter()
        .find(|row| row.proposal_id == proposal_id)
        .expect("proposal ledger row after rollback audit failure");
    assert_eq!(row.lifecycle.state, ProposalLifecycleState::Failed);

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn workspace_vfs_integration_open_buffer_audit_failure_fails_closed_and_rolls_back() {
    let root = create_root();
    let target = root.join("audit-failure.txt");
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
    let node = workspace_node_by_name(&app, opened.workspace_id, "audit-failure.txt");
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
        ProposalId(703),
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
    let proposal_id = proposal.proposal_id;

    register_validate_preview(&mut app, &proposal);
    app.fail_next_proposal_audit_write_for_test();
    let response = app
        .handle_proposal_request(ProposalRequest::Apply(proposal))
        .expect("apply text edit proposal with audit failure");

    assert!(matches!(
        response,
        ProposalResponse::Failed {
            reason: devil_protocol::ProposalFailureReason::StorageFailed,
            ..
        }
    ));
    let shell = app
        .shell_projection_snapshot("text edit audit failure ledger")
        .expect("shell projection after text edit audit failure");
    let row = shell
        .proposal_ledger_projection
        .rows
        .iter()
        .find(|row| row.proposal_id == proposal_id)
        .expect("proposal ledger row after text edit audit failure");
    assert_eq!(row.lifecycle.state, ProposalLifecycleState::Failed);
    assert_eq!(app.editor().text(buffer_id).expect("buffer text"), "seed");
    assert_eq!(app.editor().undo_len(buffer_id).expect("undo len"), 0);
    let audit_response = app
        .storage_port()
        .handle(StorageRepositoryRequest::ReadProposalAuditRecord(
            ProposalId(703),
        ))
        .expect("read audit record after failed apply");
    if let StorageRepositoryResponse::ProposalAuditRecord(Some(record)) = audit_response {
        assert_ne!(record.lifecycle_state, ProposalLifecycleState::Applied);
    }
    let events = sink.events().expect("captured audit failure events");
    assert!(!event_names(&events).contains(&"proposal.audit_recorded"));

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn workspace_vfs_integration_closed_file_audit_failure_fails_closed_and_rolls_back() {
    let root = create_root();
    let create_target = root.join("audit-create.txt");
    let delete_target = root.join("audit-delete.txt");
    let rename_source = root.join("audit-rename.txt");
    let rename_destination = root.join("audit-renamed.txt");
    std::fs::write(&delete_target, "delete seed").expect("seed delete file");
    std::fs::write(&rename_source, "rename seed").expect("seed rename file");

    let (mut app, sink) = app_with_events();
    let opened = app
        .open_workspace(
            &root,
            WorkspaceTrustState::Trusted,
            PrincipalId("trusted".to_string()),
        )
        .expect("open workspace");

    let create = proposal_envelope_with(
        ProposalId(704),
        "fs.write",
        ProposalPayload::CreateFile(devil_protocol::CreateFileProposal {
            path: CanonicalPath(create_target.to_string_lossy().into_owned()),
            initial_content: Some("created content".to_string()),
        }),
        workspace_preconditions(opened.generation),
    );
    register_validate_preview(&mut app, &create);
    app.fail_next_proposal_audit_write_for_test();
    let create_response = app
        .handle_proposal_request(ProposalRequest::Apply(create))
        .expect("apply create proposal with audit failure");
    assert!(matches!(
        create_response,
        ProposalResponse::Failed {
            reason: devil_protocol::ProposalFailureReason::StorageFailed,
            ..
        }
    ));
    assert!(!create_target.exists());
    assert!(
        workspace_tree(&app, opened.workspace_id)
            .into_iter()
            .all(|node| node.name != "audit-create.txt")
    );

    let delete_node = workspace_node_by_name(&app, opened.workspace_id, "audit-delete.txt");
    let delete = proposal_envelope_with(
        ProposalId(705),
        "fs.write",
        ProposalPayload::DeleteFile(devil_protocol::DeleteFileProposal {
            file: delete_node.identity.clone(),
        }),
        file_preconditions(&delete_node, opened.generation),
    );
    register_validate_preview(&mut app, &delete);
    app.fail_next_proposal_audit_write_for_test();
    let delete_response = app
        .handle_proposal_request(ProposalRequest::Apply(delete))
        .expect("apply delete proposal with audit failure");
    assert!(matches!(
        delete_response,
        ProposalResponse::Failed {
            reason: devil_protocol::ProposalFailureReason::StorageFailed,
            ..
        }
    ));
    assert_eq!(
        std::fs::read_to_string(&delete_target).expect("read rolled-back delete file"),
        "delete seed"
    );
    assert!(
        workspace_tree(&app, opened.workspace_id)
            .into_iter()
            .any(|node| node.name == "audit-delete.txt")
    );

    let rename_node = workspace_node_by_name(&app, opened.workspace_id, "audit-rename.txt");
    let rename = proposal_envelope_with(
        ProposalId(706),
        "fs.write",
        ProposalPayload::RenameFile(devil_protocol::RenameFileProposal {
            file: rename_node.identity.clone(),
            destination: CanonicalPath(rename_destination.to_string_lossy().into_owned()),
        }),
        file_preconditions(&rename_node, opened.generation),
    );
    register_validate_preview(&mut app, &rename);
    app.fail_next_proposal_audit_write_for_test();
    let rename_response = app
        .handle_proposal_request(ProposalRequest::Apply(rename))
        .expect("apply rename proposal with audit failure");
    assert!(matches!(
        rename_response,
        ProposalResponse::Failed {
            reason: devil_protocol::ProposalFailureReason::StorageFailed,
            ..
        }
    ));
    assert_eq!(
        std::fs::read_to_string(&rename_source).expect("read rolled-back rename source"),
        "rename seed"
    );
    assert!(!rename_destination.exists());
    let tree = workspace_tree(&app, opened.workspace_id);
    assert!(tree.iter().any(|node| node.name == "audit-rename.txt"));
    assert!(tree.iter().all(|node| node.name != "audit-renamed.txt"));

    let events = sink.events().expect("captured audit failure events");
    assert!(!event_names(&events).contains(&"proposal.audit_recorded"));

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn workspace_vfs_integration_audit_records_redact_sensitive_text_edit_payloads() {
    let root = create_root();
    let target = root.join("redaction-audit.txt");
    std::fs::write(&target, "seed").expect("seed file");

    let (mut app, _sink) = app_with_events();
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
    let node = workspace_node_by_name(&app, opened.workspace_id, "redaction-audit.txt");
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
    let secret = "FULL_SOURCE_PROMPT_SECRET_TERMINAL_PROVIDER_PAYLOAD";
    let proposal = proposal_envelope_with(
        ProposalId(704),
        "editor.write",
        ProposalPayload::TextEdit(devil_protocol::TextEditProposal {
            file_id,
            edits: EditBatch {
                edits: vec![devil_protocol::TextEdit {
                    range: TextRange::new(TextOffset::byte(0), TextOffset::byte(4)),
                    replacement: secret.to_string(),
                }],
            },
        }),
        preconditions,
    );

    register_validate_preview(&mut app, &proposal);
    assert!(matches!(
        app.handle_proposal_request(ProposalRequest::Apply(proposal))
            .expect("apply redaction proposal"),
        ProposalResponse::Applied(_)
    ));
    let audit = audit_record(&app, ProposalId(704));
    assert_metadata_only_audit(&audit);
    let debug = format!("{audit:?}");
    assert!(!debug.contains(secret));
    assert!(!debug.contains("FULL_SOURCE"));
    assert!(!debug.contains("PROMPT_SECRET"));
    assert!(!debug.contains("TERMINAL_PROVIDER_PAYLOAD"));

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
fn workspace_vfs_integration_stale_text_edit_snapshot_does_not_apply() {
    let root = create_root();
    let target = root.join("stale-snapshot-edit.txt");
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
    let node = workspace_node_by_name(&app, opened.workspace_id, "stale-snapshot-edit.txt");
    let current_snapshot = app
        .editor()
        .current_snapshot(buffer_id)
        .expect("current snapshot")
        .clone();
    let mut preconditions = file_preconditions(&node, opened.generation);
    preconditions.buffer_version = Some(current_snapshot.buffer_version);
    preconditions.snapshot_id = Some(SnapshotId(current_snapshot.snapshot_id.0.saturating_add(1)));
    let proposal = proposal_envelope_with(
        ProposalId(703),
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
        .expect("apply stale snapshot text edit proposal");

    let ProposalResponse::Stale { stale, .. } = response else {
        panic!("expected stale text edit response, got {response:?}");
    };
    assert_eq!(stale.reason, ProposalStaleReason::SnapshotMismatch);
    assert_eq!(app.editor().text(buffer_id).expect("buffer text"), "seed");
    assert_eq!(app.editor().undo_len(buffer_id).expect("undo len"), 0);

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn workspace_vfs_integration_text_edit_apply_requires_valid_lifecycle() {
    let root = create_root();
    let target = root.join("lifecycle-edit.txt");
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
    let node = workspace_node_by_name(&app, opened.workspace_id, "lifecycle-edit.txt");
    let current_snapshot = app
        .editor()
        .current_snapshot(buffer_id)
        .expect("current snapshot")
        .clone();
    let mut preconditions = file_preconditions(&node, opened.generation);
    preconditions.buffer_version = Some(current_snapshot.buffer_version);
    preconditions.snapshot_id = Some(current_snapshot.snapshot_id);
    let proposal = proposal_envelope_with(
        ProposalId(704),
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

    assert!(matches!(
        app.register_proposal_lifecycle(&proposal)
            .expect("register proposal lifecycle"),
        ProposalResponse::Created(_)
    ));
    let response = app
        .handle_proposal_request(ProposalRequest::Apply(proposal))
        .expect("apply created text edit proposal");

    let ProposalResponse::Rejected { transition, reason } = response else {
        panic!("expected lifecycle rejection, got {response:?}");
    };
    assert_eq!(reason, ProposalRejectionReason::ValidationFailed);
    assert!(
        transition
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "proposal.invalid_lifecycle_transition")
    );
    assert_eq!(app.editor().text(buffer_id).expect("buffer text"), "seed");
    assert_eq!(app.editor().undo_len(buffer_id).expect("undo len"), 0);

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

    let (mut app, sink) = app_with_events();
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

    let events = sink.events().expect("captured closed-file apply events");
    assert_events_include_order(
        &events,
        &[
            "proposal.applied",
            "proposal.audit_recorded",
            "proposal.applied",
            "proposal.audit_recorded",
            "proposal.applied",
            "proposal.audit_recorded",
        ],
    );
    for proposal_id in [ProposalId(710), ProposalId(711), ProposalId(712)] {
        let audit = audit_record(&app, proposal_id);
        assert_eq!(audit.lifecycle_state, ProposalLifecycleState::Applied);
        assert_metadata_only_audit(&audit);
    }

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
fn workspace_vfs_integration_registered_save_apply_routes_through_workspace_actor() {
    let root = create_root();
    let target = root.join("generic-save.txt");
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
    app.edit_active_buffer(TextEdit::insert(TextPosition::new(0, 4), "!"))
        .expect("dirty buffer before generic save");
    let snapshot = app
        .editor()
        .current_snapshot(buffer_id)
        .expect("current snapshot")
        .clone();
    let node = workspace_node_by_name(&app, opened.workspace_id, "generic-save.txt");
    let mut preconditions = file_preconditions(&node, opened.generation);
    preconditions.buffer_version = Some(snapshot.buffer_version);
    preconditions.snapshot_id = Some(snapshot.snapshot_id);
    let fingerprint = preconditions
        .expected_fingerprint
        .clone()
        .expect("expected fingerprint");
    let proposal = proposal_envelope_with(
        ProposalId(740),
        "fs.write",
        save_payload_for_open_buffer(
            node.identity.clone(),
            file_id,
            buffer_id,
            snapshot.snapshot_id,
            snapshot.buffer_version,
            opened.generation,
            fingerprint,
        ),
        preconditions,
    );

    register_validate_preview(&mut app, &proposal);
    let response = app
        .handle_proposal_request(ProposalRequest::Apply(proposal))
        .expect("apply generic save proposal");

    assert!(
        matches!(response, ProposalResponse::Applied(_)),
        "expected applied generic save response, got {response:?}"
    );
    assert_eq!(
        std::fs::read_to_string(&target).expect("read saved content"),
        "seed!"
    );
    assert!(!app.editor().is_dirty(buffer_id).expect("dirty cleared"));
    let events = sink.events().expect("captured generic save events");
    assert_events_include_order(
        &events,
        &[
            "editor.transaction_applied",
            "proposal.created",
            "proposal.validated",
            "proposal.previewed",
            "proposal.applied",
            "proposal.audit_recorded",
        ],
    );
    let audit = audit_record(&app, ProposalId(740));
    assert_eq!(audit.lifecycle_state, ProposalLifecycleState::Applied);
    assert_metadata_only_audit(&audit);

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn workspace_vfs_integration_shell_projection_lists_live_registered_proposals() {
    let root = create_root();
    let target = root.join("projection-save.txt");
    std::fs::write(&target, "seed").expect("seed file");

    let (mut app, _sink) = app_with_events();
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
    app.edit_active_buffer(TextEdit::insert(TextPosition::new(0, 4), "!"))
        .expect("dirty buffer before generic save");
    let snapshot = app
        .editor()
        .current_snapshot(buffer_id)
        .expect("current snapshot")
        .clone();
    let node = workspace_node_by_name(&app, opened.workspace_id, "projection-save.txt");
    let mut preconditions = file_preconditions(&node, opened.generation);
    preconditions.buffer_version = Some(snapshot.buffer_version);
    preconditions.snapshot_id = Some(snapshot.snapshot_id);
    let fingerprint = preconditions
        .expected_fingerprint
        .clone()
        .expect("expected fingerprint");
    let proposal = proposal_envelope_with(
        ProposalId(745),
        "fs.write",
        save_payload_for_open_buffer(
            node.identity.clone(),
            file_id,
            buffer_id,
            snapshot.snapshot_id,
            snapshot.buffer_version,
            opened.generation,
            fingerprint,
        ),
        preconditions,
    );

    register_validate_preview(&mut app, &proposal);

    let shell = app
        .shell_projection_snapshot("proposal ledger")
        .expect("shell projection");
    let ledger = shell.proposal_ledger_projection;
    assert_eq!(ledger.selected_proposal_id, Some(proposal.proposal_id));
    let row = ledger
        .rows
        .iter()
        .find(|row| row.proposal_id == proposal.proposal_id)
        .expect("proposal ledger row");
    assert_eq!(row.lifecycle.state, ProposalLifecycleState::Previewed);
    assert_eq!(row.workspace_id, Some(opened.workspace_id));
    assert_eq!(
        row.payload_kind,
        devil_protocol::ProposalPayloadKind::SaveFile
    );
    assert!(row.diff_summary.full_source_redacted);
    assert!(row.redaction_hints.contains(&RedactionHint::MetadataOnly));

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn workspace_vfs_integration_registered_save_audit_failure_fails_closed_and_rolls_back() {
    let root = create_root();
    let target = root.join("generic-save-audit-failure.txt");
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
    app.edit_active_buffer(TextEdit::insert(TextPosition::new(0, 4), "!"))
        .expect("dirty buffer before generic save");
    let snapshot = app
        .editor()
        .current_snapshot(buffer_id)
        .expect("current snapshot")
        .clone();
    let node = workspace_node_by_name(&app, opened.workspace_id, "generic-save-audit-failure.txt");
    let mut preconditions = file_preconditions(&node, opened.generation);
    preconditions.buffer_version = Some(snapshot.buffer_version);
    preconditions.snapshot_id = Some(snapshot.snapshot_id);
    let fingerprint = preconditions
        .expected_fingerprint
        .clone()
        .expect("expected fingerprint");
    let proposal = proposal_envelope_with(
        ProposalId(741),
        "fs.write",
        save_payload_for_open_buffer(
            node.identity.clone(),
            file_id,
            buffer_id,
            snapshot.snapshot_id,
            snapshot.buffer_version,
            opened.generation,
            fingerprint,
        ),
        preconditions,
    );
    let proposal_id = proposal.proposal_id;

    register_validate_preview(&mut app, &proposal);
    app.fail_next_proposal_audit_write_for_test();
    let response = app
        .handle_proposal_request(ProposalRequest::Apply(proposal))
        .expect("apply generic save proposal with audit failure");

    assert!(matches!(
        response,
        ProposalResponse::Failed {
            reason: devil_protocol::ProposalFailureReason::StorageFailed,
            ..
        }
    ));
    let shell = app
        .shell_projection_snapshot("audit failure ledger")
        .expect("shell projection after audit failure");
    let row = shell
        .proposal_ledger_projection
        .rows
        .iter()
        .find(|row| row.proposal_id == proposal_id)
        .expect("proposal ledger row after audit failure");
    assert_eq!(row.lifecycle.state, ProposalLifecycleState::Failed);
    assert_eq!(
        std::fs::read_to_string(&target).expect("read rolled-back saved content"),
        "seed"
    );
    assert_eq!(
        app.editor()
            .text(buffer_id)
            .expect("dirty editor text preserved"),
        "seed!"
    );
    assert!(app.editor().is_dirty(buffer_id).expect("dirty preserved"));
    assert_eq!(
        app.editor()
            .buffer_save_state(buffer_id)
            .expect("save state"),
        FileConflictLifecycleState::SaveFailed
    );

    let events = sink
        .events()
        .expect("captured generic save audit failure events");
    assert!(!event_names(&events).contains(&"proposal.audit_recorded"));

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn workspace_vfs_integration_stale_registered_save_preserves_dirty_buffer_and_disk() {
    let root = create_root();
    let target = root.join("generic-save-stale.txt");
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
    app.edit_active_buffer(TextEdit::insert(TextPosition::new(0, 4), "!"))
        .expect("dirty buffer before proposal");
    let snapshot = app
        .editor()
        .current_snapshot(buffer_id)
        .expect("current snapshot")
        .clone();
    let node = workspace_node_by_name(&app, opened.workspace_id, "generic-save-stale.txt");
    let mut preconditions = file_preconditions(&node, opened.generation);
    preconditions.buffer_version = Some(snapshot.buffer_version);
    preconditions.snapshot_id = Some(snapshot.snapshot_id);
    let fingerprint = preconditions
        .expected_fingerprint
        .clone()
        .expect("expected fingerprint");
    let proposal = proposal_envelope_with(
        ProposalId(741),
        "fs.write",
        save_payload_for_open_buffer(
            node.identity.clone(),
            file_id,
            buffer_id,
            snapshot.snapshot_id,
            snapshot.buffer_version,
            opened.generation,
            fingerprint,
        ),
        preconditions,
    );

    register_validate_preview(&mut app, &proposal);
    app.edit_active_buffer(TextEdit::insert(TextPosition::new(0, 5), "?"))
        .expect("make save proposal stale");
    let response = app
        .handle_proposal_request(ProposalRequest::Apply(proposal))
        .expect("apply stale generic save proposal");

    let ProposalResponse::Stale { stale, .. } = response else {
        panic!("expected stale save apply response, got {response:?}");
    };
    assert_eq!(stale.reason, ProposalStaleReason::BufferVersionMismatch);
    assert_eq!(
        std::fs::read_to_string(&target).expect("disk content preserved"),
        "seed"
    );
    assert_eq!(app.editor().text(buffer_id).expect("dirty text"), "seed!?");
    assert!(app.editor().is_dirty(buffer_id).expect("dirty preserved"));

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn workspace_vfs_integration_conflicted_registered_save_preserves_dirty_buffer_and_disk() {
    let root = create_root();
    let target = root.join("generic-save-conflict.txt");
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
    app.edit_active_buffer(TextEdit::insert(TextPosition::new(0, 4), "!"))
        .expect("dirty buffer before proposal");
    let snapshot = app
        .editor()
        .current_snapshot(buffer_id)
        .expect("current snapshot")
        .clone();
    let node = workspace_node_by_name(&app, opened.workspace_id, "generic-save-conflict.txt");
    let mut preconditions = file_preconditions(&node, opened.generation);
    preconditions.buffer_version = Some(snapshot.buffer_version);
    preconditions.snapshot_id = Some(snapshot.snapshot_id);
    let fingerprint = preconditions
        .expected_fingerprint
        .clone()
        .expect("expected fingerprint");
    let proposal = proposal_envelope_with(
        ProposalId(746),
        "fs.write",
        save_payload_for_open_buffer(
            node.identity.clone(),
            file_id,
            buffer_id,
            snapshot.snapshot_id,
            snapshot.buffer_version,
            opened.generation,
            fingerprint,
        ),
        preconditions,
    );

    register_validate_preview(&mut app, &proposal);
    std::fs::write(&target, "external").expect("external overwrite before apply");
    let response = app
        .handle_proposal_request(ProposalRequest::Apply(proposal))
        .expect("apply conflicted generic save proposal");

    assert!(matches!(
        response,
        ProposalResponse::Conflict { .. } | ProposalResponse::Stale { .. }
    ));
    assert_eq!(
        std::fs::read_to_string(&target).expect("disk content preserved"),
        "external"
    );
    assert_eq!(app.editor().text(buffer_id).expect("dirty text"), "seed!");
    assert!(app.editor().is_dirty(buffer_id).expect("dirty preserved"));

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn workspace_vfs_integration_closed_file_apply_rejections_do_not_mutate_disk_or_dirty_buffers() {
    let root = create_root();
    let dirty = root.join("dirty.txt");
    let stale_target = root.join("stale-delete.txt");
    let conflict_target = root.join("conflict-delete.txt");
    let escape = root.parent().expect("root parent").join(format!(
        "devil-escape-{}.txt",
        TEMP_ROOT_COUNTER.load(Ordering::Relaxed)
    ));
    std::fs::write(&dirty, "dirty").expect("seed dirty file");
    std::fs::write(&stale_target, "stale").expect("seed stale target");
    std::fs::write(&conflict_target, "conflict").expect("seed conflict target");

    let mut app = AppComposition::new();
    let opened = app
        .open_workspace(
            &root,
            WorkspaceTrustState::Trusted,
            PrincipalId("trusted".to_string()),
        )
        .expect("open workspace");
    app.open_file(dirty.to_string_lossy())
        .expect("open dirty file");
    let dirty_buffer = app.active_buffer_id().expect("dirty buffer");
    app.edit_active_buffer(TextEdit::insert(TextPosition::new(0, 5), "!"))
        .expect("make unrelated dirty buffer");

    let stale_node = workspace_node_by_name(&app, opened.workspace_id, "stale-delete.txt");
    let stale_delete = proposal_envelope_with(
        ProposalId(742),
        "fs.write",
        ProposalPayload::DeleteFile(devil_protocol::DeleteFileProposal {
            file: stale_node.identity.clone(),
        }),
        file_preconditions(&stale_node, WorkspaceGeneration(opened.generation.0 + 1)),
    );
    register_validate_preview(&mut app, &stale_delete);
    let stale_response = app
        .handle_proposal_request(ProposalRequest::Apply(stale_delete))
        .expect("apply stale delete proposal");
    assert!(matches!(stale_response, ProposalResponse::Stale { .. }));
    assert!(stale_target.exists());

    let conflict_node = workspace_node_by_name(&app, opened.workspace_id, "conflict-delete.txt");
    let conflict_delete = proposal_envelope_with(
        ProposalId(743),
        "fs.write",
        ProposalPayload::DeleteFile(devil_protocol::DeleteFileProposal {
            file: conflict_node.identity.clone(),
        }),
        file_preconditions(&conflict_node, opened.generation),
    );
    register_validate_preview(&mut app, &conflict_delete);
    std::fs::write(&conflict_target, "external").expect("external overwrite");
    let conflict_response = app
        .handle_proposal_request(ProposalRequest::Apply(conflict_delete))
        .expect("apply conflicted delete proposal");
    assert!(matches!(conflict_response, ProposalResponse::Stale { .. }));
    assert_eq!(
        std::fs::read_to_string(&conflict_target).expect("conflict disk preserved"),
        "external"
    );

    let escape_create = proposal_envelope_with(
        ProposalId(744),
        "fs.write",
        ProposalPayload::CreateFile(devil_protocol::CreateFileProposal {
            path: CanonicalPath(escape.to_string_lossy().into_owned()),
            initial_content: Some("escape".to_string()),
        }),
        workspace_preconditions(opened.generation),
    );
    register_validate_preview(&mut app, &escape_create);
    let escape_response = app
        .handle_proposal_request(ProposalRequest::Apply(escape_create))
        .expect("apply path escape create proposal");
    assert!(matches!(escape_response, ProposalResponse::Denied { .. }));
    assert!(!escape.exists());
    assert_eq!(
        app.editor()
            .text(dirty_buffer)
            .expect("dirty buffer preserved"),
        "dirty!"
    );
    assert!(app.editor().is_dirty(dirty_buffer).expect("dirty retained"));

    let _ = std::fs::remove_dir_all(&root);
    let _ = std::fs::remove_file(&escape);
}

#[test]
fn workspace_vfs_integration_untrusted_closed_file_create_is_denied_without_dirty_buffer_loss() {
    let root = create_root();
    let dirty = root.join("dirty-untrusted.txt");
    let create_target = root.join("blocked-create.txt");
    std::fs::write(&dirty, "dirty").expect("seed dirty file");

    let mut app = AppComposition::new();
    let opened = app
        .open_workspace(
            &root,
            WorkspaceTrustState::Untrusted,
            PrincipalId("untrusted".to_string()),
        )
        .expect("open untrusted workspace");
    app.open_file(dirty.to_string_lossy())
        .expect("open dirty file");
    let dirty_buffer = app.active_buffer_id().expect("dirty buffer");
    app.edit_active_buffer(TextEdit::insert(TextPosition::new(0, 5), "!"))
        .expect("make unrelated dirty buffer");

    let proposal = proposal_envelope_with(
        ProposalId(747),
        "fs.write",
        ProposalPayload::CreateFile(devil_protocol::CreateFileProposal {
            path: CanonicalPath(create_target.to_string_lossy().into_owned()),
            initial_content: Some("blocked".to_string()),
        }),
        workspace_preconditions(opened.generation),
    );

    register_validate_preview(&mut app, &proposal);
    let response = app
        .handle_proposal_request(ProposalRequest::Apply(proposal))
        .expect("apply untrusted create proposal");

    assert!(matches!(response, ProposalResponse::Denied { .. }));
    assert!(!create_target.exists());
    assert_eq!(
        app.editor()
            .text(dirty_buffer)
            .expect("dirty buffer preserved"),
        "dirty!"
    );
    assert!(app.editor().is_dirty(dirty_buffer).expect("dirty retained"));

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn workspace_vfs_integration_single_file_workspace_edit_create_applies_closed_file_only() {
    let root = create_root();
    let target = root.join("workspace-edit-created.txt");

    let mut app = AppComposition::new();
    let opened = app
        .open_workspace(
            &root,
            WorkspaceTrustState::Trusted,
            PrincipalId("trusted".to_string()),
        )
        .expect("open workspace");
    let target_path = CanonicalPath(target.to_string_lossy().into_owned());
    let proposal = proposal_envelope_with(
        ProposalId(745),
        "fs.write",
        ProposalPayload::WorkspaceEdit(devil_protocol::WorkspaceEditProposalPayload {
            workspace_id: opened.workspace_id,
            edit_id: Uuid::now_v7(),
            title: "create empty file".to_string(),
            source: devil_protocol::WorkspaceEditSourceKind::User,
            target_coverage: ProposalTargetCoverage {
                coverage_kind: ProposalTargetCoverageKind::Complete,
                targets: vec![workspace_edit_path_target(
                    format!("workspace-edit:create:path:{}", target_path.0),
                    &target,
                )],
                omitted_target_count: 0,
                redaction_hints: Vec::new(),
            },
            file_edits: Vec::new(),
            file_operations: vec![devil_protocol::WorkspaceFileOperation::Create {
                path: target_path,
                initial_content_hash: None,
            }],
            required_capability: CapabilityId("fs.write".to_string()),
            diagnostics: Vec::new(),
            schema_version: 1,
        }),
        workspace_preconditions(opened.generation),
    );

    register_validate_preview(&mut app, &proposal);
    let response = app
        .handle_proposal_request(ProposalRequest::Apply(proposal))
        .expect("apply workspace edit create proposal");

    assert!(matches!(response, ProposalResponse::Applied(_)));
    assert_eq!(
        std::fs::read_to_string(&target).expect("workspace edit created file"),
        ""
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn workspace_vfs_integration_batch_preflight_plans_supported_routes_without_side_effects() {
    let root = create_root();
    let open_path = root.join("open.txt");
    let delete_path = root.join("delete.txt");
    let rename_path = root.join("rename.txt");
    let rename_destination = root.join("renamed.txt");
    let create_path = root.join("created.txt");
    std::fs::write(&open_path, "seed").expect("seed open");
    std::fs::write(&delete_path, "seed").expect("seed delete");
    std::fs::write(&rename_path, "seed").expect("seed rename");

    let mut app = AppComposition::new();
    let opened = app
        .open_workspace(
            &root,
            WorkspaceTrustState::Trusted,
            PrincipalId("trusted".to_string()),
        )
        .expect("open workspace");
    let open_file_id = app
        .open_file(open_path.to_string_lossy())
        .expect("open file");
    let buffer_id = app.active_buffer_id().expect("active buffer");
    let snapshot = app
        .editor()
        .current_snapshot(buffer_id)
        .expect("current snapshot")
        .clone();
    let delete_node = workspace_node_by_name(&app, opened.workspace_id, "delete.txt");
    let rename_node = workspace_node_by_name(&app, opened.workspace_id, "rename.txt");
    let mut preconditions = file_preconditions(&delete_node, opened.generation);
    preconditions.buffer_version = Some(snapshot.buffer_version);
    preconditions.snapshot_id = Some(snapshot.snapshot_id);

    let targets = vec![
        preflight_target(
            "target-create",
            Some(opened.workspace_id),
            None,
            &create_path,
            ProposalTargetKind::PathOnly,
        ),
        preflight_target(
            "target-delete",
            Some(opened.workspace_id),
            Some(delete_node.identity.file_id),
            &delete_path,
            ProposalTargetKind::ClosedFile,
        ),
        preflight_target(
            "target-rename",
            Some(opened.workspace_id),
            Some(rename_node.identity.file_id),
            &rename_path,
            ProposalTargetKind::ClosedFile,
        ),
        preflight_target(
            "target-text",
            Some(opened.workspace_id),
            Some(open_file_id),
            &open_path,
            ProposalTargetKind::OpenBuffer,
        ),
    ];
    let batch = batch_payload_for_test(
        ProposalBatchAtomicity::PrepareAllBeforeMutate,
        ProposalBatchRollbackPolicy::NotRequired,
        targets,
        vec![
            ProposalBatchItem {
                order: 3,
                item_id: "text".to_string(),
                payload: Box::new(text_edit_payload(open_file_id, 0, 4)),
                target_ids: vec!["target-text".to_string()],
                required_capability: CapabilityId("editor.write".to_string()),
                rollback_step_ids: Vec::new(),
            },
            ProposalBatchItem {
                order: 0,
                item_id: "create".to_string(),
                payload: Box::new(ProposalPayload::CreateFile(
                    devil_protocol::CreateFileProposal {
                        path: CanonicalPath(create_path.to_string_lossy().into_owned()),
                        initial_content: Some("new".to_string()),
                    },
                )),
                target_ids: vec!["target-create".to_string()],
                required_capability: CapabilityId("fs.write".to_string()),
                rollback_step_ids: Vec::new(),
            },
            ProposalBatchItem {
                order: 1,
                item_id: "delete".to_string(),
                payload: Box::new(ProposalPayload::DeleteFile(
                    devil_protocol::DeleteFileProposal {
                        file: delete_node.identity.clone(),
                    },
                )),
                target_ids: vec!["target-delete".to_string()],
                required_capability: CapabilityId("fs.write".to_string()),
                rollback_step_ids: Vec::new(),
            },
            ProposalBatchItem {
                order: 2,
                item_id: "rename".to_string(),
                payload: Box::new(ProposalPayload::RenameFile(
                    devil_protocol::RenameFileProposal {
                        file: rename_node.identity.clone(),
                        destination: CanonicalPath(
                            rename_destination.to_string_lossy().into_owned(),
                        ),
                    },
                )),
                target_ids: vec!["target-rename".to_string()],
                required_capability: CapabilityId("fs.write".to_string()),
                rollback_step_ids: Vec::new(),
            },
        ],
    );
    let proposal = proposal_envelope_with(
        ProposalId(731),
        "fs.write",
        ProposalPayload::Batch(batch),
        preconditions,
    );

    let plan = app.preflight_batch_proposal(&proposal);

    assert!(
        plan.diagnostics.is_empty(),
        "diagnostics: {:?}",
        plan.diagnostics
    );
    assert!(plan.runtime_apply_disabled);
    assert_eq!(
        plan.items
            .iter()
            .map(|item| item.item_id.as_str())
            .collect::<Vec<_>>(),
        vec!["create", "delete", "rename", "text"]
    );
    assert_eq!(plan.items[0].route, BatchPreflightRoute::CreateFile);
    assert_eq!(plan.items[3].route, BatchPreflightRoute::TextEdit);
    assert!(plan.items.iter().all(|item| item.supported));
    assert!(!create_path.exists());
    assert!(delete_path.exists());
    assert!(rename_path.exists());
    assert!(!rename_destination.exists());
    assert_eq!(app.editor().text(buffer_id).expect("buffer text"), "seed");

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn workspace_vfs_integration_batch_preflight_rejects_unresolved_parent_traversal_without_mutation()
{
    let root = create_root();
    let inside_path = root.join("new").join("deep").join("file.txt");
    let outside_file_name = format!(
        "devil-outside-{}.txt",
        TEMP_ROOT_COUNTER.load(Ordering::Relaxed)
    );
    let outside_normalized = root.parent().expect("root parent").join(&outside_file_name);
    let outside_path = root
        .join("new")
        .join("..")
        .join("..")
        .join(&outside_file_name);

    let mut app = AppComposition::new();
    let opened = app
        .open_workspace(
            &root,
            WorkspaceTrustState::Trusted,
            PrincipalId("trusted".to_string()),
        )
        .expect("open workspace");
    let batch = batch_payload_for_test(
        ProposalBatchAtomicity::PrepareAllBeforeMutate,
        ProposalBatchRollbackPolicy::NotRequired,
        vec![
            preflight_target(
                "target-inside",
                Some(opened.workspace_id),
                None,
                &inside_path,
                ProposalTargetKind::PathOnly,
            ),
            preflight_target(
                "target-outside",
                Some(opened.workspace_id),
                None,
                &outside_path,
                ProposalTargetKind::PathOnly,
            ),
        ],
        vec![
            ProposalBatchItem {
                order: 0,
                item_id: "inside".to_string(),
                payload: Box::new(ProposalPayload::CreateFile(
                    devil_protocol::CreateFileProposal {
                        path: CanonicalPath(inside_path.to_string_lossy().into_owned()),
                        initial_content: Some("inside".to_string()),
                    },
                )),
                target_ids: vec!["target-inside".to_string()],
                required_capability: CapabilityId("fs.write".to_string()),
                rollback_step_ids: Vec::new(),
            },
            ProposalBatchItem {
                order: 1,
                item_id: "outside".to_string(),
                payload: Box::new(ProposalPayload::CreateFile(
                    devil_protocol::CreateFileProposal {
                        path: CanonicalPath(outside_path.to_string_lossy().into_owned()),
                        initial_content: Some("outside".to_string()),
                    },
                )),
                target_ids: vec!["target-outside".to_string()],
                required_capability: CapabilityId("fs.write".to_string()),
                rollback_step_ids: Vec::new(),
            },
        ],
    );
    let proposal = proposal_envelope_with(
        ProposalId(732),
        "fs.write",
        ProposalPayload::Batch(batch),
        workspace_preconditions(opened.generation),
    );

    let plan = app.preflight_batch_proposal(&proposal);

    assert!(!plan.preflight_ok);
    assert!(
        plan.items
            .iter()
            .any(|item| item.item_id == "inside" && item.preflight_ok)
    );
    assert!(plan.items.iter().any(|item| {
        item.item_id == "outside"
            && item
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "proposal.path_outside_workspace")
    }));
    assert!(!inside_path.exists());
    assert!(!outside_normalized.exists());

    let _ = std::fs::remove_dir_all(&root);
    let _ = std::fs::remove_file(&outside_normalized);
}

#[test]
fn workspace_vfs_integration_batch_preflight_rejects_invalid_envelope_and_item_capability() {
    let root = create_root();
    let target = root.join("created.txt");

    let mut app = AppComposition::new();
    let opened = app
        .open_workspace(
            &root,
            WorkspaceTrustState::Trusted,
            PrincipalId("trusted".to_string()),
        )
        .expect("open workspace");
    let batch = batch_payload_for_test(
        ProposalBatchAtomicity::PrepareAllBeforeMutate,
        ProposalBatchRollbackPolicy::NotRequired,
        vec![preflight_target(
            "target-create",
            Some(opened.workspace_id),
            None,
            &target,
            ProposalTargetKind::PathOnly,
        )],
        vec![ProposalBatchItem {
            order: 0,
            item_id: "create".to_string(),
            payload: Box::new(ProposalPayload::CreateFile(
                devil_protocol::CreateFileProposal {
                    path: CanonicalPath(target.to_string_lossy().into_owned()),
                    initial_content: Some("new".to_string()),
                },
            )),
            target_ids: vec!["target-create".to_string()],
            required_capability: CapabilityId("editor.write".to_string()),
            rollback_step_ids: Vec::new(),
        }],
    );
    let mut proposal = proposal_envelope_with(
        ProposalId(733),
        "",
        ProposalPayload::Batch(batch),
        workspace_preconditions(opened.generation),
    );
    proposal.principal = PrincipalId("   ".to_string());
    proposal.correlation_id = CorrelationId(0);
    proposal.preview.summary.clear();

    let plan = app.preflight_batch_proposal(&proposal);

    assert!(!plan.preflight_ok);
    for expected in [
        "proposal.missing_principal",
        "proposal.missing_capability",
        "proposal.zero_correlation_id",
        "proposal.missing_preview",
    ] {
        assert!(
            plan.diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == expected),
            "missing diagnostic {expected}: {:?}",
            plan.diagnostics
        );
    }
    assert!(plan.items.iter().any(|item| {
        item.diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "proposal.invalid_batch_item_capability")
    }));
    assert!(!target.exists());

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn workspace_vfs_integration_batch_preflight_uses_non_active_buffer_metadata_for_text_edit() {
    let root = create_root();
    let first_path = root.join("first.txt");
    let second_path = root.join("second.txt");
    std::fs::write(&first_path, "first").expect("seed first");
    std::fs::write(&second_path, "second").expect("seed second");

    let mut app = AppComposition::new();
    let opened = app
        .open_workspace(
            &root,
            WorkspaceTrustState::Trusted,
            PrincipalId("trusted".to_string()),
        )
        .expect("open workspace");
    let first_file_id = app
        .open_file(first_path.to_string_lossy())
        .expect("open first file");
    let first_buffer_id = app.active_buffer_id().expect("first buffer");
    let first_snapshot = app
        .editor()
        .current_snapshot(first_buffer_id)
        .expect("first snapshot")
        .clone();
    let first_node = workspace_node_by_name(&app, opened.workspace_id, "first.txt");
    let mut preconditions = file_preconditions(&first_node, opened.generation);
    preconditions.buffer_version = Some(first_snapshot.buffer_version);
    preconditions.snapshot_id = Some(first_snapshot.snapshot_id);

    app.open_file(second_path.to_string_lossy())
        .expect("open second file");
    assert_ne!(app.active_buffer_id(), Some(first_buffer_id));

    let batch = batch_payload_for_test(
        ProposalBatchAtomicity::PrepareAllBeforeMutate,
        ProposalBatchRollbackPolicy::NotRequired,
        vec![preflight_target(
            "target-first",
            Some(opened.workspace_id),
            Some(first_file_id),
            &first_path,
            ProposalTargetKind::OpenBuffer,
        )],
        vec![ProposalBatchItem {
            order: 0,
            item_id: "edit-first".to_string(),
            payload: Box::new(ProposalPayload::TextEdit(
                devil_protocol::TextEditProposal {
                    file_id: first_file_id,
                    edits: EditBatch {
                        edits: vec![devil_protocol::TextEdit {
                            range: TextRange::new(TextOffset::byte(0), TextOffset::byte(5)),
                            replacement: "FIRST".to_string(),
                        }],
                    },
                },
            )),
            target_ids: vec!["target-first".to_string()],
            required_capability: CapabilityId("editor.write".to_string()),
            rollback_step_ids: Vec::new(),
        }],
    );
    let proposal = proposal_envelope_with(
        ProposalId(734),
        "editor.write",
        ProposalPayload::Batch(batch),
        preconditions,
    );

    let plan = app.preflight_batch_proposal(&proposal);

    assert!(plan.preflight_ok, "plan diagnostics: {:?}", plan);
    assert_eq!(
        app.editor().text(first_buffer_id).expect("first text"),
        "first"
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn workspace_vfs_integration_batch_preflight_rejects_dependency_errors_without_mutation() {
    let app = AppComposition::new();
    let target_path = Path::new("C:/repo/new.txt");
    let targets = vec![preflight_target(
        "target-create",
        Some(WorkspaceId(11)),
        None,
        target_path,
        ProposalTargetKind::PathOnly,
    )];
    let item = ProposalBatchItem {
        order: 0,
        item_id: "create".to_string(),
        payload: Box::new(ProposalPayload::CreateFile(
            devil_protocol::CreateFileProposal {
                path: CanonicalPath(target_path.to_string_lossy().into_owned()),
                initial_content: None,
            },
        )),
        target_ids: vec!["target-create".to_string()],
        required_capability: CapabilityId("fs.write".to_string()),
        rollback_step_ids: Vec::new(),
    };
    let mut unknown = batch_payload_for_test(
        ProposalBatchAtomicity::OrderedNonAtomic,
        ProposalBatchRollbackPolicy::NotSupported,
        targets.clone(),
        vec![item.clone()],
    );
    unknown.dependency_edges.push(ProposalBatchDependency {
        prerequisite_item_id: "missing".to_string(),
        dependent_item_id: "create".to_string(),
        kind: ProposalBatchDependencyKind::RequiresValidation,
    });
    let unknown_plan =
        app.preflight_batch_proposal(&proposal_envelope(ProposalPayload::Batch(unknown)));
    assert!(!unknown_plan.preflight_ok);
    assert!(
        unknown_plan
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "proposal.unknown_batch_dependency")
    );

    let mut cycle = batch_payload_for_test(
        ProposalBatchAtomicity::OrderedNonAtomic,
        ProposalBatchRollbackPolicy::NotSupported,
        targets,
        vec![
            item,
            ProposalBatchItem {
                order: 1,
                item_id: "second".to_string(),
                payload: Box::new(ProposalPayload::CreateFile(
                    devil_protocol::CreateFileProposal {
                        path: CanonicalPath("C:/repo/second.txt".to_string()),
                        initial_content: None,
                    },
                )),
                target_ids: vec!["target-create".to_string()],
                required_capability: CapabilityId("fs.write".to_string()),
                rollback_step_ids: Vec::new(),
            },
        ],
    );
    cycle.dependency_edges = vec![
        ProposalBatchDependency {
            prerequisite_item_id: "create".to_string(),
            dependent_item_id: "second".to_string(),
            kind: ProposalBatchDependencyKind::RequiresValidation,
        },
        ProposalBatchDependency {
            prerequisite_item_id: "second".to_string(),
            dependent_item_id: "create".to_string(),
            kind: ProposalBatchDependencyKind::RequiresValidation,
        },
    ];
    let cycle_plan =
        app.preflight_batch_proposal(&proposal_envelope(ProposalPayload::Batch(cycle)));
    assert!(!cycle_plan.preflight_ok);
    assert!(
        cycle_plan
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "proposal.batch_dependency_cycle")
    );
    assert!(cycle_plan.partial_failures.iter().all(|failure| matches!(
        failure.disposition,
        devil_protocol::ProposalPartialFailureDisposition::FailedBeforeMutation
    )));
}

#[test]
fn workspace_vfs_integration_batch_preflight_rejects_missing_and_unknown_targets() {
    let app = AppComposition::new();
    let mut batch = batch_payload_for_test(
        ProposalBatchAtomicity::OrderedNonAtomic,
        ProposalBatchRollbackPolicy::NotSupported,
        vec![preflight_target(
            "known",
            Some(WorkspaceId(11)),
            None,
            Path::new("C:/repo/new.txt"),
            ProposalTargetKind::PathOnly,
        )],
        vec![
            ProposalBatchItem {
                order: 0,
                item_id: "missing-targets".to_string(),
                payload: Box::new(ProposalPayload::CreateFile(
                    devil_protocol::CreateFileProposal {
                        path: CanonicalPath("C:/repo/new.txt".to_string()),
                        initial_content: None,
                    },
                )),
                target_ids: Vec::new(),
                required_capability: CapabilityId("fs.write".to_string()),
                rollback_step_ids: Vec::new(),
            },
            ProposalBatchItem {
                order: 1,
                item_id: "unknown-target".to_string(),
                payload: Box::new(ProposalPayload::CreateFile(
                    devil_protocol::CreateFileProposal {
                        path: CanonicalPath("C:/repo/other.txt".to_string()),
                        initial_content: None,
                    },
                )),
                target_ids: vec!["unknown".to_string()],
                required_capability: CapabilityId("fs.write".to_string()),
                rollback_step_ids: Vec::new(),
            },
        ],
    );
    batch.target_coverage.omitted_target_count = 0;
    let plan = app.preflight_batch_proposal(&proposal_envelope(ProposalPayload::Batch(batch)));

    assert!(!plan.preflight_ok);
    assert!(plan.items.iter().any(|item| {
        item.diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "proposal.missing_batch_item_targets")
    }));
    assert!(plan.items.iter().any(|item| {
        item.diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "proposal.unknown_batch_target")
    }));
    assert!(!plan.partial_failures.is_empty());
}

#[test]
fn workspace_vfs_integration_batch_preflight_rejects_unproven_rollback_boundaries() {
    let app = AppComposition::new();
    let target_path = Path::new("C:/repo/new.txt");
    let targets = vec![preflight_target(
        "target-create",
        Some(WorkspaceId(11)),
        None,
        target_path,
        ProposalTargetKind::PathOnly,
    )];
    let item = ProposalBatchItem {
        order: 0,
        item_id: "create".to_string(),
        payload: Box::new(ProposalPayload::CreateFile(
            devil_protocol::CreateFileProposal {
                path: CanonicalPath(target_path.to_string_lossy().into_owned()),
                initial_content: None,
            },
        )),
        target_ids: vec!["target-create".to_string()],
        required_capability: CapabilityId("fs.write".to_string()),
        rollback_step_ids: Vec::new(),
    };
    let all_or_nothing = batch_payload_for_test(
        ProposalBatchAtomicity::AllOrNothing,
        ProposalBatchRollbackPolicy::Required,
        targets.clone(),
        vec![item.clone()],
    );
    let plan =
        app.preflight_batch_proposal(&proposal_envelope(ProposalPayload::Batch(all_or_nothing)));
    assert!(!plan.preflight_ok);
    assert!(
        plan.diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "proposal.missing_rollback_proof")
    );

    let not_supported_strong = batch_payload_for_test(
        ProposalBatchAtomicity::PrepareAllBeforeMutate,
        ProposalBatchRollbackPolicy::NotSupported,
        targets,
        vec![ProposalBatchItem {
            rollback_step_ids: vec!["rollback-create".to_string()],
            ..item
        }],
    );
    let plan = app.preflight_batch_proposal(&proposal_envelope(ProposalPayload::Batch(
        not_supported_strong,
    )));
    assert!(!plan.preflight_ok);
    assert!(
        plan.diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "proposal.unsupported_rollback_policy")
    );

    let mut proven = batch_payload_for_test(
        ProposalBatchAtomicity::AllOrNothing,
        ProposalBatchRollbackPolicy::Required,
        vec![preflight_target(
            "target-create",
            Some(WorkspaceId(11)),
            None,
            target_path,
            ProposalTargetKind::PathOnly,
        )],
        vec![ProposalBatchItem {
            order: 0,
            item_id: "create".to_string(),
            payload: Box::new(ProposalPayload::CreateFile(
                devil_protocol::CreateFileProposal {
                    path: CanonicalPath(target_path.to_string_lossy().into_owned()),
                    initial_content: None,
                },
            )),
            target_ids: vec!["target-create".to_string()],
            required_capability: CapabilityId("fs.write".to_string()),
            rollback_step_ids: vec!["rollback-create".to_string()],
        }],
    );
    proven.rollback_steps = vec![ProposalRollbackStep {
        order: 0,
        step_id: "rollback-create".to_string(),
        item_id: "create".to_string(),
        target_id: "target-create".to_string(),
        action: ProposalRollbackAction::DeleteCreatedFile,
        expected_preconditions: empty_preconditions(),
        diagnostics: Vec::new(),
    }];
    let plan = app.preflight_batch_proposal(&proposal_envelope(ProposalPayload::Batch(proven)));
    assert!(plan.runtime_apply_disabled);
}

#[test]
fn workspace_vfs_integration_batch_execution_contract_reports_audit_and_commit_barriers() {
    let root = create_root();
    let target = root.join("contract-created.txt");

    let mut app = AppComposition::new();
    let opened = app
        .open_workspace(
            &root,
            WorkspaceTrustState::Trusted,
            PrincipalId("trusted".to_string()),
        )
        .expect("open workspace");
    let batch = batch_payload_for_test(
        ProposalBatchAtomicity::PrepareAllBeforeMutate,
        ProposalBatchRollbackPolicy::NotRequired,
        vec![preflight_target(
            "target-create",
            Some(opened.workspace_id),
            None,
            &target,
            ProposalTargetKind::PathOnly,
        )],
        vec![ProposalBatchItem {
            order: 0,
            item_id: "create".to_string(),
            payload: Box::new(ProposalPayload::CreateFile(
                devil_protocol::CreateFileProposal {
                    path: CanonicalPath(target.to_string_lossy().into_owned()),
                    initial_content: Some("contract".to_string()),
                },
            )),
            target_ids: vec!["target-create".to_string()],
            required_capability: CapabilityId("fs.write".to_string()),
            rollback_step_ids: Vec::new(),
        }],
    );
    let proposal = proposal_envelope_with(
        ProposalId(736),
        "fs.write",
        ProposalPayload::Batch(batch),
        workspace_preconditions(opened.generation),
    );

    let contract = app.plan_batch_execution_contract(&proposal);
    let journal = app.plan_batch_execution_journal(&proposal);

    assert!(contract.preflight.preflight_ok, "contract: {contract:?}");
    assert!(contract.runtime_apply_disabled);
    assert!(contract.audit_before_success_required);
    assert!(contract.commit_blocked);
    assert!(contract.finalize_blocked);
    assert_eq!(contract.proposal_id, ProposalId(736));
    assert_eq!(
        contract.batch_id,
        Some(Uuid::parse_str("77777777-7777-7777-7777-777777777777").unwrap())
    );
    assert_eq!(contract.items.len(), 1);
    assert!(contract.items[0].preflight_ok);
    assert_eq!(contract.items[0].route, BatchPreflightRoute::CreateFile);
    assert!(
        contract
            .preview_warnings
            .iter()
            .any(|warning| warning.code == "proposal.batch_contract_not_runtime_execution")
    );
    assert_eq!(
        contract
            .stages
            .iter()
            .map(|stage| stage.stage)
            .collect::<Vec<_>>(),
        vec![
            BatchExecutionStage::Prepare,
            BatchExecutionStage::Preflight,
            BatchExecutionStage::Mutate,
            BatchExecutionStage::Commit,
            BatchExecutionStage::Audit,
            BatchExecutionStage::Finalize,
            BatchExecutionStage::Rollback,
        ]
    );
    for required_blocked_stage in [
        BatchExecutionStage::Mutate,
        BatchExecutionStage::Commit,
        BatchExecutionStage::Audit,
        BatchExecutionStage::Finalize,
        BatchExecutionStage::Rollback,
    ] {
        let stage = contract
            .stages
            .iter()
            .find(|stage| stage.stage == required_blocked_stage)
            .expect("stage present");
        assert!(stage.required);
        assert!(stage.blocked);
        assert!(!stage.diagnostics.is_empty());
    }
    assert!(!journal.mutation_allowed);
    assert!(journal.runtime_apply_disabled);
    assert!(journal.audit_before_success_required);
    assert_eq!(journal.items.len(), 1);
    assert_eq!(
        journal.items[0].state,
        BatchExecutionJournalItemState::RuntimeMutationDisabled
    );
    assert_eq!(journal.items[0].route, BatchPreflightRoute::CreateFile);
    assert_eq!(
        journal
            .stages
            .iter()
            .find(|stage| stage.stage == BatchExecutionStage::Mutate)
            .expect("mutate stage present")
            .state,
        BatchExecutionJournalStageState::Blocked
    );
    assert!(!target.exists());

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn workspace_vfs_integration_batch_contract_rejects_mismatched_rollback_action() {
    let root = create_root();
    let target = root.join("rollback-created.txt");

    let mut app = AppComposition::new();
    let opened = app
        .open_workspace(
            &root,
            WorkspaceTrustState::Trusted,
            PrincipalId("trusted".to_string()),
        )
        .expect("open workspace");
    let mut batch = batch_payload_for_test(
        ProposalBatchAtomicity::AllOrNothing,
        ProposalBatchRollbackPolicy::Required,
        vec![preflight_target(
            "target-create",
            Some(opened.workspace_id),
            None,
            &target,
            ProposalTargetKind::PathOnly,
        )],
        vec![ProposalBatchItem {
            order: 0,
            item_id: "create".to_string(),
            payload: Box::new(ProposalPayload::CreateFile(
                devil_protocol::CreateFileProposal {
                    path: CanonicalPath(target.to_string_lossy().into_owned()),
                    initial_content: Some("rollback".to_string()),
                },
            )),
            target_ids: vec!["target-create".to_string()],
            required_capability: CapabilityId("fs.write".to_string()),
            rollback_step_ids: vec!["rollback-create".to_string()],
        }],
    );
    batch.rollback_steps = vec![ProposalRollbackStep {
        order: 0,
        step_id: "rollback-create".to_string(),
        item_id: "create".to_string(),
        target_id: "target-create".to_string(),
        action: ProposalRollbackAction::RestoreFileSnapshot,
        expected_preconditions: empty_preconditions(),
        diagnostics: Vec::new(),
    }];
    let proposal = proposal_envelope_with(
        ProposalId(737),
        "fs.write",
        ProposalPayload::Batch(batch),
        workspace_preconditions(opened.generation),
    );

    let contract = app.plan_batch_execution_contract(&proposal);

    assert!(!contract.preflight.preflight_ok);
    assert!(!contract.items[0].exact_rollback_proof);
    assert!(
        contract
            .preflight
            .diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.code == "proposal.unresolved_rollback_step" })
    );
    assert!(
        contract.items[0]
            .diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.code == "proposal.unresolved_rollback_step" })
    );
    assert!(!target.exists());

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn workspace_vfs_integration_batch_contract_records_dependency_blocked_partial_failures() {
    let root = create_root();
    let outside = root.join("..").join("contract-outside.txt");
    let outside_normalized = root
        .parent()
        .expect("root parent")
        .join("contract-outside.txt");
    let dependent = root.join("dependent.txt");

    let mut app = AppComposition::new();
    let opened = app
        .open_workspace(
            &root,
            WorkspaceTrustState::Trusted,
            PrincipalId("trusted".to_string()),
        )
        .expect("open workspace");
    let mut batch = batch_payload_for_test(
        ProposalBatchAtomicity::PrepareAllBeforeMutate,
        ProposalBatchRollbackPolicy::NotRequired,
        vec![
            preflight_target(
                "target-outside",
                Some(opened.workspace_id),
                None,
                &outside,
                ProposalTargetKind::PathOnly,
            ),
            preflight_target(
                "target-dependent",
                Some(opened.workspace_id),
                None,
                &dependent,
                ProposalTargetKind::PathOnly,
            ),
        ],
        vec![
            ProposalBatchItem {
                order: 0,
                item_id: "outside".to_string(),
                payload: Box::new(ProposalPayload::CreateFile(
                    devil_protocol::CreateFileProposal {
                        path: CanonicalPath(outside.to_string_lossy().into_owned()),
                        initial_content: Some("outside".to_string()),
                    },
                )),
                target_ids: vec!["target-outside".to_string()],
                required_capability: CapabilityId("fs.write".to_string()),
                rollback_step_ids: Vec::new(),
            },
            ProposalBatchItem {
                order: 1,
                item_id: "dependent".to_string(),
                payload: Box::new(ProposalPayload::CreateFile(
                    devil_protocol::CreateFileProposal {
                        path: CanonicalPath(dependent.to_string_lossy().into_owned()),
                        initial_content: Some("dependent".to_string()),
                    },
                )),
                target_ids: vec!["target-dependent".to_string()],
                required_capability: CapabilityId("fs.write".to_string()),
                rollback_step_ids: Vec::new(),
            },
        ],
    );
    batch.dependency_edges.push(ProposalBatchDependency {
        prerequisite_item_id: "outside".to_string(),
        dependent_item_id: "dependent".to_string(),
        kind: ProposalBatchDependencyKind::RequiresValidation,
    });
    let proposal = proposal_envelope_with(
        ProposalId(738),
        "fs.write",
        ProposalPayload::Batch(batch),
        workspace_preconditions(opened.generation),
    );

    let contract = app.plan_batch_execution_contract(&proposal);
    let journal = app.plan_batch_execution_journal(&proposal);

    assert!(!contract.preflight.preflight_ok);
    assert_eq!(
        contract
            .partial_failures
            .iter()
            .map(|failure| (failure.item_id.as_str(), failure.disposition))
            .collect::<Vec<_>>(),
        vec![
            (
                "outside",
                devil_protocol::ProposalPartialFailureDisposition::FailedBeforeMutation,
            ),
            (
                "dependent",
                devil_protocol::ProposalPartialFailureDisposition::NotStarted,
            ),
        ]
    );
    assert_eq!(
        contract.items[1].partial_failure_disposition,
        Some(devil_protocol::ProposalPartialFailureDisposition::NotStarted)
    );
    assert_eq!(
        journal
            .items
            .iter()
            .map(|item| (item.item_id.as_str(), item.state))
            .collect::<Vec<_>>(),
        vec![
            ("outside", BatchExecutionJournalItemState::PreflightRejected,),
            (
                "dependent",
                BatchExecutionJournalItemState::DependencyBlocked,
            ),
        ]
    );
    assert!(!journal.mutation_allowed);
    assert!(!outside_normalized.exists());
    assert!(!dependent.exists());

    let _ = std::fs::remove_dir_all(&root);
    let _ = std::fs::remove_file(&outside_normalized);
}

#[test]
fn workspace_vfs_integration_batch_contract_models_atomic_best_effort_and_dry_run() {
    let root = create_root();
    let atomic_target = root.join("atomic-created.txt");
    let best_effort_target = root.join("best-effort-created.txt");
    let dry_run_target = root.join("dry-run-created.txt");

    let mut app = AppComposition::new();
    let opened = app
        .open_workspace(
            &root,
            WorkspaceTrustState::Trusted,
            PrincipalId("trusted".to_string()),
        )
        .expect("open workspace");

    let mut atomic_batch = batch_payload_for_test(
        ProposalBatchAtomicity::AllOrNothing,
        ProposalBatchRollbackPolicy::Required,
        vec![preflight_target(
            "target-atomic",
            Some(opened.workspace_id),
            None,
            &atomic_target,
            ProposalTargetKind::PathOnly,
        )],
        vec![ProposalBatchItem {
            order: 0,
            item_id: "atomic-create".to_string(),
            payload: Box::new(ProposalPayload::CreateFile(
                devil_protocol::CreateFileProposal {
                    path: CanonicalPath(atomic_target.to_string_lossy().into_owned()),
                    initial_content: Some("atomic".to_string()),
                },
            )),
            target_ids: vec!["target-atomic".to_string()],
            required_capability: CapabilityId("fs.write".to_string()),
            rollback_step_ids: vec!["rollback-atomic".to_string()],
        }],
    );
    atomic_batch.rollback_steps = vec![ProposalRollbackStep {
        order: 0,
        step_id: "rollback-atomic".to_string(),
        item_id: "atomic-create".to_string(),
        target_id: "target-atomic".to_string(),
        action: ProposalRollbackAction::DeleteCreatedFile,
        expected_preconditions: empty_preconditions(),
        diagnostics: Vec::new(),
    }];
    let atomic_contract = app.plan_batch_execution_contract(&proposal_envelope_with(
        ProposalId(739),
        "fs.write",
        ProposalPayload::Batch(atomic_batch),
        workspace_preconditions(opened.generation),
    ));
    assert!(
        atomic_contract.preflight.preflight_ok,
        "{atomic_contract:?}"
    );
    assert_eq!(
        atomic_contract.planning_semantics,
        Some(BatchPlanningSemantics::Atomic)
    );
    assert_eq!(
        atomic_contract
            .rollback_contract
            .as_ref()
            .expect("rollback contract")
            .status,
        BatchRollbackContractStatus::Exact
    );
    assert!(atomic_contract.items[0].exact_rollback_proof);

    let mut best_effort_batch = batch_payload_for_test(
        ProposalBatchAtomicity::OrderedNonAtomic,
        ProposalBatchRollbackPolicy::BestEffort,
        vec![preflight_target(
            "target-best-effort",
            Some(opened.workspace_id),
            None,
            &best_effort_target,
            ProposalTargetKind::PathOnly,
        )],
        vec![ProposalBatchItem {
            order: 0,
            item_id: "best-effort-create".to_string(),
            payload: Box::new(ProposalPayload::CreateFile(
                devil_protocol::CreateFileProposal {
                    path: CanonicalPath(best_effort_target.to_string_lossy().into_owned()),
                    initial_content: Some("best-effort".to_string()),
                },
            )),
            target_ids: vec!["target-best-effort".to_string()],
            required_capability: CapabilityId("fs.write".to_string()),
            rollback_step_ids: vec!["rollback-best-effort".to_string()],
        }],
    );
    best_effort_batch.rollback_steps = vec![ProposalRollbackStep {
        order: 0,
        step_id: "rollback-best-effort".to_string(),
        item_id: "best-effort-create".to_string(),
        target_id: "target-best-effort".to_string(),
        action: ProposalRollbackAction::RestoreFileSnapshot,
        expected_preconditions: empty_preconditions(),
        diagnostics: Vec::new(),
    }];
    let best_effort_contract = app.plan_batch_execution_contract(&proposal_envelope_with(
        ProposalId(740),
        "fs.write",
        ProposalPayload::Batch(best_effort_batch),
        workspace_preconditions(opened.generation),
    ));
    assert!(
        best_effort_contract.preflight.preflight_ok,
        "{best_effort_contract:?}"
    );
    assert_eq!(
        best_effort_contract.planning_semantics,
        Some(BatchPlanningSemantics::BestEffort)
    );
    let best_effort_rollback = best_effort_contract
        .rollback_contract
        .as_ref()
        .expect("best-effort rollback contract");
    assert_eq!(
        best_effort_rollback.status,
        BatchRollbackContractStatus::BestEffort
    );
    assert!(!best_effort_rollback.steps[0].exact);
    assert!(
        best_effort_rollback.steps[0]
            .diagnostics
            .iter()
            .any(|diagnostic| {
                diagnostic.code == "proposal.unresolved_rollback_step"
                    && diagnostic.path.is_none()
                    && diagnostic.range.is_none()
            })
    );

    let dry_run_batch = batch_payload_for_test(
        ProposalBatchAtomicity::PrepareAllBeforeMutate,
        ProposalBatchRollbackPolicy::NotRequired,
        vec![preflight_target(
            "target-dry-run",
            Some(opened.workspace_id),
            None,
            &dry_run_target,
            ProposalTargetKind::PathOnly,
        )],
        vec![ProposalBatchItem {
            order: 0,
            item_id: "dry-run-create".to_string(),
            payload: Box::new(ProposalPayload::CreateFile(
                devil_protocol::CreateFileProposal {
                    path: CanonicalPath(dry_run_target.to_string_lossy().into_owned()),
                    initial_content: Some("dry-run".to_string()),
                },
            )),
            target_ids: vec!["target-dry-run".to_string()],
            required_capability: CapabilityId("fs.write".to_string()),
            rollback_step_ids: Vec::new(),
        }],
    );
    let dry_run_contract = app.plan_batch_execution_contract(&proposal_envelope_with(
        ProposalId(741),
        "fs.write",
        ProposalPayload::Batch(dry_run_batch),
        workspace_preconditions(opened.generation),
    ));
    assert!(
        dry_run_contract.preflight.preflight_ok,
        "{dry_run_contract:?}"
    );
    assert_eq!(
        dry_run_contract.planning_semantics,
        Some(BatchPlanningSemantics::DryRun)
    );
    assert_eq!(
        dry_run_contract
            .rollback_contract
            .as_ref()
            .expect("dry-run rollback contract")
            .status,
        BatchRollbackContractStatus::NotRequired
    );
    assert!(!atomic_target.exists());
    assert!(!best_effort_target.exists());
    assert!(!dry_run_target.exists());

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn workspace_vfs_integration_batch_preflight_rejects_unsupported_mixed_and_duplicate_targets() {
    let root = create_root();
    let target = root.join("duplicate-target.txt");

    let mut app = AppComposition::new();
    let opened = app
        .open_workspace(
            &root,
            WorkspaceTrustState::Trusted,
            PrincipalId("trusted".to_string()),
        )
        .expect("open workspace");

    let unsupported_mixed = batch_payload_for_test(
        ProposalBatchAtomicity::OrderedNonAtomic,
        ProposalBatchRollbackPolicy::NotSupported,
        vec![
            preflight_target(
                "target-create",
                Some(opened.workspace_id),
                None,
                &target,
                ProposalTargetKind::PathOnly,
            ),
            ProposalAffectedTarget {
                target_id: "target-terminal".to_string(),
                kind: ProposalTargetKind::TerminalSession,
                workspace_id: None,
                file_id: None,
                buffer_id: None,
                path: Some(CanonicalPath(root.to_string_lossy().into_owned())),
                terminal_session_id: None,
                plugin_id: None,
                remote_authority: None,
                collaboration_session_id: None,
                byte_ranges: Vec::new(),
                redaction_hints: Vec::new(),
            },
        ],
        vec![
            ProposalBatchItem {
                order: 0,
                item_id: "create".to_string(),
                payload: Box::new(ProposalPayload::CreateFile(
                    devil_protocol::CreateFileProposal {
                        path: CanonicalPath(target.to_string_lossy().into_owned()),
                        initial_content: Some("create".to_string()),
                    },
                )),
                target_ids: vec!["target-create".to_string()],
                required_capability: CapabilityId("fs.write".to_string()),
                rollback_step_ids: Vec::new(),
            },
            ProposalBatchItem {
                order: 1,
                item_id: "terminal".to_string(),
                payload: Box::new(ProposalPayload::TerminalCommand(
                    devil_protocol::TerminalCommandProposal {
                        session_id: None,
                        command: "echo blocked".to_string(),
                        cwd: Some(CanonicalPath(root.to_string_lossy().into_owned())),
                        env: std::collections::HashMap::new(),
                    },
                )),
                target_ids: vec!["target-terminal".to_string()],
                required_capability: CapabilityId("terminal.execute".to_string()),
                rollback_step_ids: Vec::new(),
            },
        ],
    );
    let unsupported_plan = app.preflight_batch_proposal(&proposal_envelope_with(
        ProposalId(742),
        "fs.write",
        ProposalPayload::Batch(unsupported_mixed),
        workspace_preconditions(opened.generation),
    ));
    assert!(!unsupported_plan.preflight_ok);
    assert!(
        unsupported_plan
            .diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.code == "proposal.unsupported_batch_target_kind" })
    );
    assert!(unsupported_plan.items.iter().any(|item| {
        item.route == BatchPreflightRoute::Terminal && !item.supported && !item.preflight_ok
    }));

    let duplicate_targets = batch_payload_for_test(
        ProposalBatchAtomicity::OrderedNonAtomic,
        ProposalBatchRollbackPolicy::NotSupported,
        vec![
            preflight_target(
                "target-a",
                Some(opened.workspace_id),
                None,
                &target,
                ProposalTargetKind::PathOnly,
            ),
            preflight_target(
                "target-b",
                Some(opened.workspace_id),
                None,
                &target,
                ProposalTargetKind::PathOnly,
            ),
        ],
        vec![ProposalBatchItem {
            order: 0,
            item_id: "create".to_string(),
            payload: Box::new(ProposalPayload::CreateFile(
                devil_protocol::CreateFileProposal {
                    path: CanonicalPath(target.to_string_lossy().into_owned()),
                    initial_content: Some("duplicate".to_string()),
                },
            )),
            target_ids: vec!["target-a".to_string(), "target-b".to_string()],
            required_capability: CapabilityId("fs.write".to_string()),
            rollback_step_ids: Vec::new(),
        }],
    );
    let duplicate_plan = app.preflight_batch_proposal(&proposal_envelope_with(
        ProposalId(743),
        "fs.write",
        ProposalPayload::Batch(duplicate_targets),
        workspace_preconditions(opened.generation),
    ));
    assert!(!duplicate_plan.preflight_ok);
    assert!(
        duplicate_plan
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "proposal.duplicate_target")
    );
    assert!(!target.exists());

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn workspace_vfs_integration_batch_apply_cannot_self_approve_from_created_state() {
    let root = create_root();
    let target = root.join("self-approval-created.txt");

    let mut app = AppComposition::new();
    let opened = app
        .open_workspace(
            &root,
            WorkspaceTrustState::Trusted,
            PrincipalId("trusted".to_string()),
        )
        .expect("open workspace");
    let batch = batch_payload_for_test(
        ProposalBatchAtomicity::PrepareAllBeforeMutate,
        ProposalBatchRollbackPolicy::NotRequired,
        vec![preflight_target(
            "target-create",
            Some(opened.workspace_id),
            None,
            &target,
            ProposalTargetKind::PathOnly,
        )],
        vec![ProposalBatchItem {
            order: 0,
            item_id: "create".to_string(),
            payload: Box::new(ProposalPayload::CreateFile(
                devil_protocol::CreateFileProposal {
                    path: CanonicalPath(target.to_string_lossy().into_owned()),
                    initial_content: Some("blocked".to_string()),
                },
            )),
            target_ids: vec!["target-create".to_string()],
            required_capability: CapabilityId("fs.write".to_string()),
            rollback_step_ids: Vec::new(),
        }],
    );
    let proposal = proposal_envelope_with(
        ProposalId(744),
        "fs.write",
        ProposalPayload::Batch(batch),
        workspace_preconditions(opened.generation),
    );

    assert!(matches!(
        app.register_proposal_lifecycle(&proposal)
            .expect("register lifecycle"),
        ProposalResponse::Created(_)
    ));
    let response = app
        .handle_proposal_request(ProposalRequest::Apply(proposal))
        .expect("apply from created state");

    assert!(matches!(
        response,
        ProposalResponse::Rejected {
            reason: ProposalRejectionReason::ValidationFailed,
            ..
        }
    ));
    if let ProposalResponse::Rejected { transition, .. } = response {
        assert!(
            transition
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "proposal.invalid_lifecycle_transition")
        );
    }
    assert!(!target.exists());

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn workspace_vfs_integration_batch_apply_remains_fail_closed_after_preview() {
    let root = create_root();
    let target = root.join("batch-created.txt");

    let mut app = AppComposition::new();
    let opened = app
        .open_workspace(
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
        workspace_preconditions(opened.generation),
    );

    let contract = app.plan_batch_execution_contract(&proposal);
    assert!(contract.preflight.preflight_ok, "contract: {contract:?}");
    assert!(contract.runtime_apply_disabled);
    assert!(contract.audit_before_success_required);
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
            at: ui_text_coordinate(0, 0),
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
            at: ui_text_coordinate(0, 0),
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
            at: ui_text_coordinate(0, 0),
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
