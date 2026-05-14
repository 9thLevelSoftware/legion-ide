use std::{
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
};

static TEMP_ROOT_COUNTER: AtomicU64 = AtomicU64::new(0);

use devil_app::{
    AppCommandExecutionState, AppCommandOutcome, AppCommandRequest, AppComposition,
    AppEditorCommandPort, AppSaveOutcome, AppWorkspaceCommandPort, CommandDispatcher,
    CommandExecutionService, OpenFileIntent,
};
use devil_editor::{TextEdit, TextPosition};
use devil_observability::{InMemoryEventSink, SharedEventSink};
use devil_project::OpenedFileText;
use devil_protocol::{
    BufferId, BufferVersion, CanonicalPath, CausalityId, ChangedTextRange, EventEnvelope,
    FileConflictLifecycleState, FileContentVersion, FileId, FileIdentity, FileMetadata,
    FileTreeNode, PrincipalId, SnapshotId, TextTransactionDescriptor, TimestampMillis,
    TransactionSource, WorkspaceId, WorkspaceTrustState,
};
use devil_ui::CommandDispatchIntent;
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
    assert!(matches!(save_err, AppSaveOutcome::Rejected(_)));

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
