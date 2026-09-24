use std::sync::atomic::{AtomicU64, Ordering};

use legion_app::{
    AppCloseTabOutcome, AppCommandOutcome, AppComposition, AppSaveAllItemStatus, AppSaveAllStatus,
    AppSaveOutcome,
};
use legion_editor::{TextEdit, TextPosition};
use legion_memory::{MemoryCandidateRecord, MemoryConsentState, MemoryService};
use legion_observability::{InMemoryEventSink, SharedEventSink};
use legion_protocol::{
    AgentRunId, BufferVersion, CaretAffinity, CausalityId, CorrelationId, PrincipalId,
    ProtocolTextRange, SnapshotId, TextCoordinate, ViewportScroll, ViewportSemanticTokenKind,
    WorkspaceTrustState,
};
use legion_storage::HotExitStore;
use legion_ui::{CommandDispatchIntent, ShellLayoutProjection};

static TEMP_ROOT_COUNTER: AtomicU64 = AtomicU64::new(0);

fn recv_did_change(
    rx: &std::sync::mpsc::Receiver<legion_app::language::LspWorkerRequest>,
) -> (String, i64, String) {
    match rx
        .recv_timeout(std::time::Duration::from_secs(1))
        .expect("didChange must be queued")
    {
        legion_app::language::LspWorkerRequest::DidChange {
            uri, version, text, ..
        } => (uri, version, text),
        legion_app::language::LspWorkerRequest::DidChangeDeferred {
            uri,
            version,
            text_rx,
            ..
        } => {
            let text = text_rx
                .recv_timeout(std::time::Duration::from_secs(1))
                .expect("deferred didChange text")
                .expect("deferred didChange produced text");
            (uri, version, text)
        }
        _ => panic!("expected didChange request"),
    }
}

/// Drop-guarded temporary workspace root. The directory is removed on drop with a
/// prefix/location check so a panic before the end of a test never leaks the temp dir.
struct TempWorkspace {
    root: std::path::PathBuf,
}

impl std::ops::Deref for TempWorkspace {
    type Target = std::path::Path;

    fn deref(&self) -> &std::path::Path {
        &self.root
    }
}

impl Drop for TempWorkspace {
    fn drop(&mut self) {
        let temp_root = std::env::temp_dir();
        let file_name = self.root.file_name().and_then(|name| name.to_str());
        if self.root.starts_with(&temp_root)
            && file_name.is_some_and(|name| name.starts_with("legion-app-daily-editing-"))
        {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }
}

fn create_root() -> TempWorkspace {
    let root = std::env::temp_dir().join(format!(
        "legion-app-daily-editing-{}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |value| value.as_millis() as u64),
        TEMP_ROOT_COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir(&root).expect("create temp root");
    TempWorkspace { root }
}

fn text_coordinate(line: u32, character: u32) -> TextCoordinate {
    TextCoordinate {
        line,
        character,
        byte_offset: None,
        utf16_offset: None,
    }
}

fn trusted_app(root: &std::path::Path) -> AppComposition {
    let mut app = AppComposition::new();
    app.open_workspace(
        root,
        WorkspaceTrustState::Trusted,
        PrincipalId("daily-editing".to_string()),
    )
    .expect("open workspace");
    app
}

fn fresh_lsp_health() -> legion_protocol::LspServerHealthRecord {
    legion_protocol::LspServerHealthRecord {
        server_id: legion_protocol::LanguageServerId(1),
        language_id: legion_protocol::LanguageId("rust".to_string()),
        binary_provenance: legion_protocol::LspServerBinaryProvenance::Configured,
        binary_path_hash: None,
        artifact_hash: None,
        version: None,
        init_status: legion_protocol::LspResultStatus::Fresh,
        capabilities: Vec::new(),
        diagnostics_latency_ms: None,
        restart_count: 0,
        download_decision_id: None,
        schema_version: 1,
    }
}

#[test]
fn daily_editing_contracts_tabs_switch_active_buffer() {
    let root = create_root();
    let first = root.join("first.txt");
    let second = root.join("second.txt");
    std::fs::write(&first, "first\n").expect("seed first");
    std::fs::write(&second, "second\n").expect("seed second");

    let mut app = trusted_app(&root);
    app.open_file(first.to_string_lossy()).expect("open first");
    let first_buffer = app.active_buffer_id().expect("first buffer");
    app.open_file(second.to_string_lossy())
        .expect("open second");
    let second_buffer = app.active_buffer_id().expect("second buffer");

    let snapshot = app.shell_projection_snapshot("daily").expect("snapshot");
    assert_eq!(snapshot.daily_editing_projection.tabs.tabs.len(), 2);
    assert_eq!(
        snapshot.daily_editing_projection.tabs.active_buffer_id,
        Some(second_buffer)
    );

    let outcome = app
        .dispatch_ui_intent(CommandDispatchIntent::SwitchTab {
            buffer_id: first_buffer,
        })
        .expect("switch tab");
    assert!(matches!(outcome, AppCommandOutcome::TabSwitched(buffer) if buffer == first_buffer));
    assert_eq!(app.active_buffer_id(), Some(first_buffer));
    assert_eq!(
        app.active_buffer_projection(&ShellLayoutProjection::plain("daily"))
            .expect("active projection")
            .small_buffer_text(),
        Some("first\n")
    );

    app.dispatch_ui_intent(CommandDispatchIntent::SetCursor {
        buffer_id: first_buffer,
        cursor: text_coordinate(0, 3),
    })
    .expect("set cursor");
    app.dispatch_ui_intent(CommandDispatchIntent::SetSelection {
        buffer_id: first_buffer,
        range: ProtocolTextRange {
            start: text_coordinate(0, 0),
            end: text_coordinate(0, 5),
        },
    })
    .expect("set selection");
    app.dispatch_ui_intent(CommandDispatchIntent::SetViewportScroll {
        buffer_id: first_buffer,
        scroll: ViewportScroll {
            top_line: 0,
            left_column: 2,
        },
    })
    .expect("set scroll");

    let projected = app
        .active_buffer_projection(&ShellLayoutProjection::plain("daily"))
        .expect("active projection after cursor");
    let viewport = projected.viewport.expect("viewport");
    // SetSelection is the legacy forward-range compatibility view; its head
    // is the range end under the directed-caret authority.
    assert_eq!(viewport.cursor.character, 5);
    assert_eq!(viewport.selections.len(), 1);
    assert_eq!(viewport.scroll.left_column, 2);
}

#[test]
fn daily_editing_visual_cursor_route_is_stale_and_atomic() {
    let root = create_root();
    let first = root.join("first.txt");
    let second = root.join("second.txt");
    std::fs::write(&first, "wrapped\n").expect("seed first");
    std::fs::write(&second, "other\n").expect("seed second");
    let mut app = trusted_app(&root);
    app.open_file(first.to_string_lossy()).expect("open first");
    let first_buffer = app.active_buffer_id().expect("first buffer");
    app.open_file(second.to_string_lossy())
        .expect("open second");
    let second_buffer = app.active_buffer_id().expect("second buffer");
    app.dispatch_ui_intent(CommandDispatchIntent::SwitchTab {
        buffer_id: first_buffer,
    })
    .expect("activate first");
    let viewport = app
        .active_buffer_projection(&ShellLayoutProjection::plain("daily"))
        .expect("active projection")
        .viewport
        .expect("viewport");
    let before_text = app
        .active_buffer_projection(&ShellLayoutProjection::plain("daily"))
        .expect("active projection")
        .small_buffer_text()
        .expect("small text")
        .to_string();
    let before_version = viewport.buffer_version;
    let before_dirty = app.editor().is_dirty(first_buffer).expect("dirty state");
    let before_transactions = app.editor().transaction_log().len();
    let head = text_coordinate(0, 3);
    let visual = |buffer_id, expected_snapshot_id, expected_buffer_version, cursor, affinity| {
        CommandDispatchIntent::SetVisualCursor {
            buffer_id,
            expected_snapshot_id,
            expected_buffer_version,
            cursor,
            affinity,
        }
    };
    app.dispatch_ui_intent(visual(
        first_buffer,
        viewport.snapshot_id,
        viewport.buffer_version,
        head,
        CaretAffinity::Downstream,
    ))
    .expect("valid visual placement");
    let placed = app
        .active_buffer_projection(&ShellLayoutProjection::plain("daily"))
        .expect("projection after placement")
        .viewport
        .expect("viewport after placement");
    assert_eq!(placed.cursors[0].line, head.line);
    assert_eq!(placed.cursors[0].character, head.character);
    assert_eq!(placed.cursor_affinities, vec![CaretAffinity::Downstream]);
    assert_eq!(
        app.active_buffer_projection(&ShellLayoutProjection::plain("daily"))
            .expect("projection after placement")
            .small_buffer_text(),
        Some(before_text.as_str())
    );
    assert_eq!(placed.buffer_version, before_version);

    let selection = CommandDispatchIntent::SetVisualDirectedSelection {
        buffer_id: first_buffer,
        expected_snapshot_id: placed.snapshot_id,
        expected_buffer_version: placed.buffer_version,
        anchor: text_coordinate(0, 1),
        head: text_coordinate(0, 4),
        head_affinity: CaretAffinity::Downstream,
    };
    app.dispatch_ui_intent(selection)
        .expect("valid visual selection placement");
    let selected = app
        .active_buffer_projection(&ShellLayoutProjection::plain("daily"))
        .expect("projection after selection")
        .viewport
        .expect("viewport after selection");
    assert_eq!(selected.selections.len(), 1);
    assert_eq!(selected.selections[0].start.line, 0);
    assert_eq!(selected.selections[0].start.character, 1);
    assert_eq!(selected.selections[0].end.line, 0);
    assert_eq!(selected.selections[0].end.character, 4);
    assert_eq!(selected.selections[0].start.byte_offset, Some(1));
    assert_eq!(selected.selections[0].end.byte_offset, Some(4));
    assert_eq!(selected.cursor_affinities, vec![CaretAffinity::Downstream]);
    assert_eq!(selected.buffer_version, before_version);
    assert_eq!(
        app.editor().is_dirty(first_buffer).expect("dirty state"),
        before_dirty
    );
    assert_eq!(app.editor().transaction_log().len(), before_transactions);

    let stale = app.dispatch_ui_intent(visual(
        first_buffer,
        SnapshotId(viewport.snapshot_id.0.saturating_add(1)),
        viewport.buffer_version,
        text_coordinate(0, 1),
        CaretAffinity::Upstream,
    ));
    assert!(stale.is_err(), "stale snapshot must reject");
    let stale_version = app.dispatch_ui_intent(visual(
        first_buffer,
        selected.snapshot_id,
        BufferVersion(selected.buffer_version.0.saturating_add(1)),
        text_coordinate(0, 1),
        CaretAffinity::Upstream,
    ));
    assert!(stale_version.is_err(), "stale buffer version must reject");
    let inactive = app.dispatch_ui_intent(visual(
        second_buffer,
        viewport.snapshot_id,
        viewport.buffer_version,
        text_coordinate(0, 1),
        CaretAffinity::Upstream,
    ));
    assert!(inactive.is_err(), "inactive buffer must reject");
    let invalid = app.dispatch_ui_intent(visual(
        first_buffer,
        viewport.snapshot_id,
        viewport.buffer_version,
        text_coordinate(0, 99),
        CaretAffinity::Upstream,
    ));
    assert!(invalid.is_err(), "invalid endpoint must reject");
    let invalid_anchor =
        app.dispatch_ui_intent(CommandDispatchIntent::SetVisualDirectedSelection {
            buffer_id: first_buffer,
            expected_snapshot_id: selected.snapshot_id,
            expected_buffer_version: selected.buffer_version,
            anchor: text_coordinate(0, 99),
            head: text_coordinate(0, 2),
            head_affinity: CaretAffinity::Upstream,
        });
    assert!(
        invalid_anchor.is_err(),
        "invalid anchor must reject atomically"
    );
    let unchanged = app
        .active_buffer_projection(&ShellLayoutProjection::plain("daily"))
        .expect("projection after rejects")
        .viewport
        .expect("viewport after rejects");
    assert_eq!(unchanged.cursors[0].line, head.line);
    assert_eq!(unchanged.cursors[0].character, 4);
    assert_eq!(unchanged.cursor_affinities, vec![CaretAffinity::Downstream]);
    assert_eq!(unchanged.selections.len(), 1);
    assert_eq!(unchanged.selections[0].start.byte_offset, Some(1));
    assert_eq!(unchanged.selections[0].end.byte_offset, Some(4));
    assert_eq!(unchanged.buffer_version, before_version);
    assert_eq!(
        app.editor().is_dirty(first_buffer).expect("dirty state"),
        before_dirty
    );
    assert_eq!(app.editor().transaction_log().len(), before_transactions);
    assert_eq!(
        app.active_buffer_projection(&ShellLayoutProjection::plain("daily"))
            .expect("projection after rejects")
            .small_buffer_text(),
        Some(before_text.as_str())
    );
}

#[test]
fn daily_editing_contracts_native_directed_replacement_uses_correlated_app_authority() {
    let root = create_root();
    let file = root.join("replace.txt");
    std::fs::write(&file, "abcd").expect("seed file");
    let mut app = trusted_app(&root);
    app.open_file(file.to_string_lossy()).expect("open file");
    let buffer_id = app.active_buffer_id().expect("active buffer");
    app.dispatch_ui_intent(CommandDispatchIntent::SetDirectedSelection {
        buffer_id,
        anchor: text_coordinate(0, 1),
        head: text_coordinate(0, 3),
    })
    .expect("set directed selection");
    app.dispatch_ui_intent(CommandDispatchIntent::ReplaceDirectedCarets {
        buffer_id,
        text: "X".to_string(),
    })
    .expect("replace directed carets");
    let projection = app
        .active_buffer_projection(&ShellLayoutProjection::plain("daily"))
        .expect("projection");
    assert_eq!(projection.small_buffer_text(), Some("aXd"));
    assert!(projection.viewport.expect("viewport").selections.is_empty());
    let transaction = app
        .editor()
        .transaction_log()
        .last()
        .expect("replacement transaction");
    assert!(transaction.correlation_id.is_some());
}

#[test]
fn daily_editing_contracts_replacement_effects_match_apply_edit_and_queue_did_change() {
    fn run_edit(
        root: &std::path::Path,
        file_name: &str,
    ) -> (
        String,
        legion_protocol::TextTransactionDescriptor,
        legion_protocol::EventEnvelope,
        (String, i64, String),
        bool,
    ) {
        let file = root.join(file_name);
        std::fs::write(&file, "abcd").expect("seed file");
        let sink = InMemoryEventSink::new();
        let mut app = AppComposition::with_event_sink(SharedEventSink::new(sink.clone()));
        app.open_workspace(
            root,
            WorkspaceTrustState::Trusted,
            PrincipalId("daily-editing-effects".to_string()),
        )
        .expect("open workspace");
        app.open_file(file.to_string_lossy()).expect("open file");
        let buffer_id = app.active_buffer_id().expect("active buffer");
        let request_rx = app.set_lsp_request_receiver_for_test(fresh_lsp_health());
        let outcome = app
            .dispatch_ui_intent(CommandDispatchIntent::Replace {
                buffer_id,
                range: legion_protocol::ProtocolTextRange {
                    start: text_coordinate(0, 1),
                    end: text_coordinate(0, 3),
                },
                replacement: "X".to_string(),
            })
            .expect("edit");
        let descriptor = match outcome {
            AppCommandOutcome::Edited(descriptor) => descriptor,
            other => panic!("expected edited outcome, got {other:?}"),
        };
        let request = recv_did_change(&request_rx);
        let canonical_file = std::fs::canonicalize(&file).expect("canonical file path");
        let normalized_file = canonical_file.to_string_lossy().replace('\\', "/");
        let normalized_file = normalized_file
            .strip_prefix("//?/UNC/")
            .or_else(|| normalized_file.strip_prefix("//?/"))
            .unwrap_or(&normalized_file);
        let normalized_file = if normalized_file.starts_with("server/") {
            format!("//{normalized_file}")
        } else {
            normalized_file.to_string()
        };
        let expected_uri = if normalized_file.starts_with('/') {
            format!("file://{normalized_file}")
        } else {
            format!("file:///{normalized_file}")
        };
        let (uri, version, text) = &request;
        assert_eq!(uri, &expected_uri, "didChange URI mismatch");
        assert_eq!(
            *version, descriptor.post_buffer_version.0 as i64,
            "didChange version mismatch"
        );
        assert_eq!(text, "aXd", "didChange text mismatch");
        let event = sink
            .events()
            .expect("event snapshot")
            .into_iter()
            .find(|event| event.event == "editor.transaction_applied")
            .expect("replacement transaction event");
        let text = app
            .editor()
            .text(buffer_id)
            .expect("buffer text")
            .to_string();
        let dirty = app.editor().is_dirty(buffer_id).expect("dirty state");
        assert_eq!(event.correlation_id, descriptor.correlation_id);
        assert_ne!(event.correlation_id.0, 0);
        assert_ne!(event.causality_id.0, uuid::Uuid::nil());
        assert_eq!(
            event.payload["post_buffer_version"],
            serde_json::json!(descriptor.post_buffer_version.0)
        );
        assert!(dirty, "an applied edit must leave the buffer dirty");
        assert_eq!(descriptor.changed_ranges.len(), 1);
        (text, descriptor, event, request, dirty)
    }

    let root = create_root();
    std::fs::write(root.join("directed.txt"), "abcd").expect("seed directed file");
    let file = root.join("directed.txt");
    let sink = InMemoryEventSink::new();
    let mut directed = AppComposition::with_event_sink(SharedEventSink::new(sink.clone()));
    directed
        .open_workspace(
            &*root,
            WorkspaceTrustState::Trusted,
            PrincipalId("daily-editing-effects".to_string()),
        )
        .expect("open workspace");
    directed
        .open_file(file.to_string_lossy())
        .expect("open file");
    let buffer_id = directed.active_buffer_id().expect("active buffer");
    let request_rx = directed.set_lsp_request_receiver_for_test(fresh_lsp_health());
    directed
        .dispatch_ui_intent(CommandDispatchIntent::SetDirectedSelection {
            buffer_id,
            anchor: text_coordinate(0, 1),
            head: text_coordinate(0, 3),
        })
        .expect("directed selection");
    let directed_descriptor = match directed
        .dispatch_ui_intent(CommandDispatchIntent::ReplaceDirectedCarets {
            buffer_id,
            text: "X".to_string(),
        })
        .expect("directed replacement")
    {
        AppCommandOutcome::Edited(descriptor) => descriptor,
        other => panic!("expected edited outcome, got {other:?}"),
    };
    let directed_request = recv_did_change(&request_rx);
    let directed_event = sink
        .events()
        .expect("directed event snapshot")
        .into_iter()
        .find(|event| event.event == "editor.transaction_applied")
        .expect("directed transaction event");
    let directed_text = directed.editor().text(buffer_id).expect("text").to_string();
    let directed_dirty = directed.editor().is_dirty(buffer_id).expect("dirty");
    let canonical_directed_file = std::fs::canonicalize(&file).expect("canonical directed path");
    let normalized_directed_file = canonical_directed_file.to_string_lossy().replace('\\', "/");
    let normalized_directed_file = normalized_directed_file
        .strip_prefix("//?/UNC/")
        .or_else(|| normalized_directed_file.strip_prefix("//?/"))
        .unwrap_or(&normalized_directed_file)
        .to_string();
    let directed_expected_uri = if normalized_directed_file.starts_with('/') {
        format!("file://{normalized_directed_file}")
    } else {
        format!("file:///{normalized_directed_file}")
    };
    assert_eq!(directed_text, "aXd");
    assert_ne!(directed_event.correlation_id.0, 0);
    assert_ne!(directed_event.causality_id.0, uuid::Uuid::nil());

    let ordinary = run_edit(&root, "ordinary.txt");
    assert_eq!(directed_text, ordinary.0);
    assert_eq!(directed_dirty, ordinary.4);
    assert_eq!(
        directed_descriptor.post_buffer_version,
        ordinary.1.post_buffer_version
    );
    assert_eq!(
        directed_descriptor.changed_ranges,
        ordinary.1.changed_ranges
    );
    assert_eq!(
        directed_event.correlation_id,
        directed_descriptor.correlation_id
    );
    assert_eq!(
        directed_event.causality_id,
        directed_descriptor.causality_id
    );
    assert_eq!(directed_request.0, directed_expected_uri);
    assert_eq!(
        directed_request.1,
        directed_descriptor.post_buffer_version.0 as i64
    );
    assert_eq!(directed_request.2, "aXd");
    assert!(ordinary.3.0.ends_with("/ordinary.txt"));
    assert_eq!(ordinary.3.1, ordinary.1.post_buffer_version.0 as i64);
    assert_eq!(ordinary.3.2, "aXd");
}

#[test]
fn daily_editing_contracts_native_delete_routes_all_carets_and_did_change() {
    fn run(root: &std::path::Path, name: &str, backward: bool) -> String {
        let file = root.join(name);
        std::fs::write(&file, "a😀b\ncde\n").expect("seed file");
        let sink = InMemoryEventSink::new();
        let mut app = AppComposition::with_event_sink(SharedEventSink::new(sink.clone()));
        app.open_workspace(
            root,
            WorkspaceTrustState::Trusted,
            PrincipalId("daily-delete".to_string()),
        )
        .expect("open workspace");
        app.open_file(file.to_string_lossy()).expect("open file");
        let buffer_id = app.active_buffer_id().expect("active buffer");
        app.dispatch_ui_intent(CommandDispatchIntent::SetCursor {
            buffer_id,
            cursor: text_coordinate(0, 1),
        })
        .expect("set primary caret");
        app.dispatch_ui_intent(CommandDispatchIntent::AddCursorBelow { buffer_id })
            .expect("add second caret");
        let request_rx = app.set_lsp_request_receiver_for_test(fresh_lsp_health());
        let before_transactions = app.editor().transaction_log().len();
        let outcome = app
            .dispatch_ui_intent(CommandDispatchIntent::DeleteDirectedCarets {
                buffer_id,
                backward,
            })
            .expect("native delete");
        let descriptor = match outcome {
            AppCommandOutcome::Edited(descriptor) => descriptor,
            other => panic!("expected edited outcome, got {other:?}"),
        };
        assert_eq!(
            app.editor().transaction_log().len(),
            before_transactions + 1
        );
        assert!(app.editor().is_dirty(buffer_id).expect("dirty state"));
        assert_ne!(descriptor.correlation_id.0, 0);
        let event = sink
            .events()
            .expect("event snapshot")
            .into_iter()
            .find(|event| event.event == "editor.transaction_applied")
            .expect("transaction event");
        assert_eq!(event.correlation_id, descriptor.correlation_id);
        let (_uri, version, text) = recv_did_change(&request_rx);
        assert_eq!(version, descriptor.post_buffer_version.0 as i64);
        assert_eq!(
            text,
            app.editor().text(buffer_id).expect("text").to_string()
        );
        text
    }

    let root = create_root();
    assert_eq!(run(&root, "backward.txt", true), "😀b\nde\n");
    assert_eq!(run(&root, "forward.txt", false), "ab\nce\n");
}

#[test]
fn daily_editing_contracts_native_delete_noop_has_no_effects() {
    let root = create_root();
    let file = root.join("noop.txt");
    std::fs::write(&file, "abc\ndef\n").expect("seed file");
    let sink = InMemoryEventSink::new();
    let mut app = AppComposition::with_event_sink(SharedEventSink::new(sink.clone()));
    app.open_workspace(
        &*root,
        WorkspaceTrustState::Trusted,
        PrincipalId("daily-delete-noop".to_string()),
    )
    .expect("open workspace");
    app.open_file(file.to_string_lossy()).expect("open file");
    let buffer_id = app.active_buffer_id().expect("active buffer");
    app.dispatch_ui_intent(CommandDispatchIntent::SetCursor {
        buffer_id,
        cursor: text_coordinate(0, 0),
    })
    .expect("set primary caret");
    let before_viewport = app
        .active_buffer_projection(&ShellLayoutProjection::plain("daily"))
        .expect("before projection")
        .viewport
        .expect("before viewport");
    let before_carets = app.editor().cursors(buffer_id).expect("before carets");
    let request_rx = app.set_lsp_request_receiver_for_test(fresh_lsp_health());
    let before_transactions = app.editor().transaction_log().len();
    let outcome = app
        .dispatch_ui_intent(CommandDispatchIntent::DeleteDirectedCarets {
            buffer_id,
            backward: true,
        })
        .expect("noop native delete");
    assert!(matches!(outcome, AppCommandOutcome::Noop));
    assert_eq!(app.editor().transaction_log().len(), before_transactions);
    assert!(!app.editor().is_dirty(buffer_id).expect("dirty state"));
    let after_viewport = app
        .active_buffer_projection(&ShellLayoutProjection::plain("daily"))
        .expect("after projection")
        .viewport
        .expect("after viewport");
    assert_eq!(
        after_viewport.buffer_version,
        before_viewport.buffer_version
    );
    assert_eq!(after_viewport.snapshot_id, before_viewport.snapshot_id);
    assert_eq!(
        app.editor().cursors(buffer_id).expect("after carets"),
        before_carets
    );
    assert!(
        sink.events()
            .expect("event snapshot")
            .into_iter()
            .all(|event| event.event != "editor.transaction_applied")
    );
    assert!(
        request_rx
            .recv_timeout(std::time::Duration::from_millis(100))
            .is_err()
    );
}

#[test]
fn daily_editing_contracts_horizontal_movement_is_caret_only() {
    let root = create_root();
    let file = root.join("horizontal.txt");
    std::fs::write(&file, "a\u{301}😀b\ncd\n").expect("seed file");
    let sink = InMemoryEventSink::new();
    let mut app = AppComposition::with_event_sink(SharedEventSink::new(sink.clone()));
    app.open_workspace(
        &*root,
        WorkspaceTrustState::Trusted,
        PrincipalId("daily-horizontal".to_string()),
    )
    .expect("open workspace");
    app.open_file(file.to_string_lossy()).expect("open file");
    let buffer_id = app.active_buffer_id().expect("active buffer");
    app.dispatch_ui_intent(CommandDispatchIntent::SetCursor {
        buffer_id,
        cursor: text_coordinate(0, 3),
    })
    .expect("set initial caret");
    app.dispatch_ui_intent(CommandDispatchIntent::AddCursorBelow { buffer_id })
        .expect("add second caret");
    let before_viewport = app
        .active_buffer_projection(&ShellLayoutProjection::plain("daily"))
        .expect("before projection")
        .viewport
        .expect("before viewport");
    let before_carets = app
        .editor()
        .directed_carets(buffer_id)
        .expect("before directed carets");
    assert_eq!(
        before_carets,
        vec![
            legion_editor::DirectedCaret::new(legion_editor::TextPosition::new(0, 3), None),
            legion_editor::DirectedCaret::new(legion_editor::TextPosition::new(1, 2), None),
        ]
    );
    let before_transactions = app.editor().transaction_log().len();
    let request_rx = app.set_lsp_request_receiver_for_test(fresh_lsp_health());

    let outcome = app
        .dispatch_ui_intent(CommandDispatchIntent::MoveHorizontally {
            buffer_id,
            left: true,
            extend: true,
        })
        .expect("horizontal movement");
    assert!(matches!(outcome, AppCommandOutcome::CursorSet(id) if id == buffer_id));
    assert_eq!(app.editor().transaction_log().len(), before_transactions);
    assert!(!app.editor().is_dirty(buffer_id).expect("dirty state"));
    assert_eq!(
        app.editor().buffer_version(buffer_id).expect("version"),
        before_viewport.buffer_version
    );
    let after_carets = app
        .editor()
        .directed_carets(buffer_id)
        .expect("after directed carets");
    assert_eq!(
        after_carets,
        vec![
            legion_editor::DirectedCaret::new(
                legion_editor::TextPosition::new(0, 0),
                Some(legion_editor::TextPosition::new(0, 3)),
            ),
            legion_editor::DirectedCaret::new(
                legion_editor::TextPosition::new(1, 1),
                Some(legion_editor::TextPosition::new(1, 2)),
            ),
        ]
    );
    let after_viewport = app
        .active_buffer_projection(&ShellLayoutProjection::plain("daily"))
        .expect("after projection")
        .viewport
        .expect("after viewport");
    assert_eq!(after_viewport.snapshot_id, before_viewport.snapshot_id);
    assert_eq!(
        after_viewport.buffer_version,
        before_viewport.buffer_version
    );
    assert_eq!(
        app.editor().text(buffer_id).expect("text").to_string(),
        "a\u{301}😀b\ncd\n"
    );
    assert!(
        sink.events()
            .expect("event snapshot")
            .into_iter()
            .all(|event| event.event != "editor.transaction_applied")
    );
    assert!(
        request_rx
            .recv_timeout(std::time::Duration::from_millis(100))
            .is_err()
    );
}

#[test]
fn daily_editing_contracts_generic_delete_preserves_explicit_range_with_multiple_carets() {
    let root = create_root();
    let file = root.join("range.txt");
    std::fs::write(&file, "abc\ndef\n").expect("seed file");
    let mut app = trusted_app(&root);
    app.open_file(file.to_string_lossy()).expect("open file");
    let buffer_id = app.active_buffer_id().expect("active buffer");
    app.dispatch_ui_intent(CommandDispatchIntent::SetCursor {
        buffer_id,
        cursor: text_coordinate(0, 0),
    })
    .expect("set primary caret");
    app.dispatch_ui_intent(CommandDispatchIntent::AddCursorBelow { buffer_id })
        .expect("add second caret");
    let before_carets = app.editor().cursors(buffer_id).expect("before carets");
    assert_eq!(before_carets.len(), 2);
    assert_eq!(
        before_carets[0].position,
        legion_editor::TextPosition::new(0, 0)
    );
    assert_eq!(
        before_carets[1].position,
        legion_editor::TextPosition::new(1, 0)
    );
    app.dispatch_ui_intent(CommandDispatchIntent::Delete {
        buffer_id,
        range: ProtocolTextRange {
            start: text_coordinate(0, 0),
            end: text_coordinate(0, 1),
        },
    })
    .expect("explicit range delete");
    assert_eq!(
        app.editor().text(buffer_id).expect("text").to_string(),
        "bc\ndef\n"
    );
    let carets = app.editor().cursors(buffer_id).expect("carets");
    assert_eq!(carets.len(), 2);
    assert_eq!(carets[0].position, legion_editor::TextPosition::new(0, 0));
    assert_eq!(carets[1].position, legion_editor::TextPosition::new(1, 0));
    app.dispatch_ui_intent(CommandDispatchIntent::Undo { buffer_id })
        .expect("undo explicit range delete");
    assert_eq!(
        app.editor().text(buffer_id).expect("undo text").to_string(),
        "abc\ndef\n"
    );
    let undo_carets = app.editor().cursors(buffer_id).expect("undo carets");
    assert_eq!(undo_carets.len(), 2);
    assert_eq!(
        undo_carets[0].position,
        legion_editor::TextPosition::new(0, 0)
    );
    assert_eq!(
        undo_carets[1].position,
        legion_editor::TextPosition::new(1, 0)
    );
    app.dispatch_ui_intent(CommandDispatchIntent::Redo { buffer_id })
        .expect("redo explicit range delete");
    assert_eq!(
        app.editor().text(buffer_id).expect("redo text").to_string(),
        "bc\ndef\n"
    );
    let redo_carets = app.editor().cursors(buffer_id).expect("redo carets");
    assert_eq!(redo_carets.len(), 2);
    assert_eq!(
        redo_carets[0].position,
        legion_editor::TextPosition::new(0, 0)
    );
    assert_eq!(
        redo_carets[1].position,
        legion_editor::TextPosition::new(1, 0)
    );
}

#[test]
fn daily_editing_contracts_directed_carets_survive_tab_switch_per_buffer() {
    let root = create_root();
    let first = root.join("first.txt");
    let second = root.join("second.txt");
    std::fs::write(&first, "first\nlong").expect("seed first");
    std::fs::write(&second, "second\nother").expect("seed second");
    let mut app = trusted_app(&root);
    app.open_file(first.to_string_lossy()).expect("open first");
    let first_buffer = app.active_buffer_id().expect("first buffer");
    app.open_file(second.to_string_lossy())
        .expect("open second");
    let second_buffer = app.active_buffer_id().expect("second buffer");
    app.dispatch_ui_intent(CommandDispatchIntent::SetDirectedSelection {
        buffer_id: second_buffer,
        anchor: text_coordinate(0, 0),
        head: text_coordinate(0, 3),
    })
    .expect("second selection");
    app.dispatch_ui_intent(CommandDispatchIntent::SwitchTab {
        buffer_id: first_buffer,
    })
    .expect("switch first");
    app.dispatch_ui_intent(CommandDispatchIntent::SetDirectedSelection {
        buffer_id: first_buffer,
        anchor: text_coordinate(0, 0),
        head: text_coordinate(0, 2),
    })
    .expect("first selection");
    app.dispatch_ui_intent(CommandDispatchIntent::SwitchTab {
        buffer_id: second_buffer,
    })
    .expect("switch second");
    let second = app
        .active_buffer_projection(&ShellLayoutProjection::plain("daily"))
        .expect("second projection")
        .viewport
        .expect("second viewport");
    assert_eq!(second.selections[0].start.character, 0);
    assert_eq!(second.selections[0].end.character, 3);
    app.dispatch_ui_intent(CommandDispatchIntent::SwitchTab {
        buffer_id: first_buffer,
    })
    .expect("switch first again");
    let first = app
        .active_buffer_projection(&ShellLayoutProjection::plain("daily"))
        .expect("first projection")
        .viewport
        .expect("first viewport");
    assert_eq!(first.selections[0].start.character, 0);
    assert_eq!(first.selections[0].end.character, 2);
}

#[test]
fn daily_editing_contracts_active_projection_emits_visible_syntax_overlays() {
    let root = create_root();
    let cases = [
        (
            "src/lib.rs",
            "pub fn answer() -> u32 {\n    42\n}\n",
            ViewportSemanticTokenKind::Keyword,
        ),
        (
            "Cargo.toml",
            "[package]\nname = \"legion-ide\"\n",
            ViewportSemanticTokenKind::String,
        ),
        (
            "README.md",
            "# Legion IDE\n\n```rust\nfn main() {}\n```\n",
            ViewportSemanticTokenKind::Keyword,
        ),
    ];

    let mut app = trusted_app(&root);
    for (relative_path, source, expected_kind) in cases {
        let path = root.join(relative_path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("create parent");
        }
        std::fs::write(&path, source).expect("seed source");

        app.open_file(path.to_string_lossy())
            .expect("open highlighted file");

        let projection = app
            .active_buffer_projection(&ShellLayoutProjection::plain("syntax"))
            .expect("active projection");
        let viewport = projection.viewport.expect("viewport projection");

        assert!(
            viewport
                .semantic_token_overlays
                .iter()
                .any(|token| token.kind == expected_kind),
            "expected {expected_kind:?} overlay for {relative_path}; got {:?}",
            viewport.semantic_token_overlays
        );
        assert!(
            viewport.semantic_token_overlays.iter().all(|token| viewport
                .line_slices
                .iter()
                .any(|line| line.line_number == token.line_number)),
            "syntax overlays must be bounded to visible viewport lines"
        );
    }
}

#[test]
fn daily_editing_contracts_scrolled_projection_seeds_stateful_syntax_overlays() {
    let root = create_root();
    let rust_path = root.join("src/stateful.rs");
    let markdown_path = root.join("README.md");
    std::fs::create_dir_all(rust_path.parent().expect("rust parent")).expect("create src");
    std::fs::write(
        &rust_path,
        "fn main() {\n    /*\n     * visible comment\n     */\n}\n",
    )
    .expect("seed rust source");
    std::fs::write(&markdown_path, "# Notes\n\n```rust\nfn visible() {}\n```\n")
        .expect("seed markdown source");

    let mut app = trusted_app(&root);

    app.open_file(rust_path.to_string_lossy())
        .expect("open rust source");
    let rust_buffer = app.active_buffer_id().expect("rust buffer");
    app.dispatch_ui_intent(CommandDispatchIntent::SetViewportScroll {
        buffer_id: rust_buffer,
        scroll: ViewportScroll {
            top_line: 2,
            left_column: 0,
        },
    })
    .expect("scroll rust viewport");
    let rust_projection = app
        .active_buffer_projection(&ShellLayoutProjection::plain("syntax"))
        .expect("rust projection");
    let rust_viewport = rust_projection.viewport.expect("rust viewport");
    assert!(
        rust_viewport.semantic_token_overlays.iter().any(|token| {
            token.line_number == 2 && token.kind == ViewportSemanticTokenKind::Comment
        }),
        "scrolled block comment line should retain comment highlighting; got {:?}",
        rust_viewport.semantic_token_overlays
    );

    app.open_file(markdown_path.to_string_lossy())
        .expect("open markdown source");
    let markdown_buffer = app.active_buffer_id().expect("markdown buffer");
    app.dispatch_ui_intent(CommandDispatchIntent::SetViewportScroll {
        buffer_id: markdown_buffer,
        scroll: ViewportScroll {
            top_line: 3,
            left_column: 0,
        },
    })
    .expect("scroll markdown viewport");
    let markdown_projection = app
        .active_buffer_projection(&ShellLayoutProjection::plain("syntax"))
        .expect("markdown projection");
    let markdown_viewport = markdown_projection.viewport.expect("markdown viewport");
    assert!(
        markdown_viewport
            .semantic_token_overlays
            .iter()
            .any(|token| {
                token.line_number == 3 && token.kind == ViewportSemanticTokenKind::Keyword
            }),
        "scrolled fenced Rust code should retain keyword highlighting; got {:?}",
        markdown_viewport.semantic_token_overlays
    );
}

#[test]
fn daily_editing_contracts_fallback_comment_detection_ignores_delimiters_inside_strings() {
    let root = create_root();
    let rust_path = root.join("src/string_delimiter.rs");
    let toml_path = root.join("Cargo.toml");
    std::fs::create_dir_all(rust_path.parent().expect("rust parent")).expect("create src");
    let rust_source = "pub const URL: &str = \"https://github.com\";\n";
    let toml_source = "name = \"value # nested\"\n";
    std::fs::write(&rust_path, rust_source).expect("seed rust source");
    std::fs::write(&toml_path, toml_source).expect("seed toml source");

    let mut app = trusted_app(&root);

    app.open_file(rust_path.to_string_lossy())
        .expect("open rust source");
    let rust_viewport = app
        .active_buffer_projection(&ShellLayoutProjection::plain("syntax"))
        .expect("rust projection")
        .viewport
        .expect("rust viewport");
    let rust_slashes = rust_source.find("//").expect("rust delimiter") as u32;
    assert!(
        rust_viewport
            .semantic_token_overlays
            .iter()
            .filter(|token| token.line_number == 0)
            .all(|token| {
                token.kind != ViewportSemanticTokenKind::Comment
                    || !(token.start_col < rust_slashes + 2 && token.end_col > rust_slashes)
            }),
        "Rust fallback must not mark // inside a string as a comment; got {:?}",
        rust_viewport.semantic_token_overlays
    );

    app.open_file(toml_path.to_string_lossy())
        .expect("open toml source");
    let toml_viewport = app
        .active_buffer_projection(&ShellLayoutProjection::plain("syntax"))
        .expect("toml projection")
        .viewport
        .expect("toml viewport");
    let toml_hash = toml_source.find('#').expect("toml delimiter") as u32;
    assert!(
        toml_viewport
            .semantic_token_overlays
            .iter()
            .filter(|token| token.line_number == 0)
            .all(|token| {
                token.kind != ViewportSemanticTokenKind::Comment
                    || !(token.start_col <= toml_hash && token.end_col > toml_hash)
            }),
        "TOML fallback must not mark # inside a string as a comment; got {:?}",
        toml_viewport.semantic_token_overlays
    );
}

#[test]
fn daily_editing_contracts_save_all_preserves_rejected_dirty_buffers() {
    let root = create_root();
    let clean = root.join("clean.txt");
    let conflicted = root.join("conflicted.txt");
    std::fs::write(&clean, "clean").expect("seed clean");
    std::fs::write(&conflicted, "conflicted").expect("seed conflicted");

    let mut app = trusted_app(&root);
    app.open_file(clean.to_string_lossy()).expect("open clean");
    let clean_buffer = app.active_buffer_id().expect("clean buffer");
    app.edit_active_buffer(TextEdit::insert(TextPosition::new(0, 5), "!"))
        .expect("edit clean");
    app.open_file(conflicted.to_string_lossy())
        .expect("open conflicted");
    let conflicted_buffer = app.active_buffer_id().expect("conflicted buffer");
    app.edit_active_buffer(TextEdit::insert(TextPosition::new(0, 10), "!"))
        .expect("edit conflicted");

    std::fs::write(&conflicted, "external").expect("external overwrite");

    let outcome = app.save_all().expect("save all");
    assert_eq!(outcome.results.len(), 2);
    assert_eq!(outcome.saved_count, 1);
    assert_eq!(outcome.rejected_count, 1);
    assert!(outcome.results.iter().any(|item| {
        item.buffer_id == clean_buffer && matches!(item.outcome, Some(AppSaveOutcome::Saved(_)))
    }));
    assert!(outcome.results.iter().any(|item| {
        item.buffer_id == conflicted_buffer
            && matches!(item.outcome, Some(AppSaveOutcome::Rejected(_)))
    }));
    assert_eq!(
        std::fs::read_to_string(&clean).expect("read clean"),
        "clean!"
    );
    assert_eq!(
        app.editor().text(conflicted_buffer).expect("dirty text"),
        "conflicted!"
    );
    assert!(
        app.editor()
            .is_dirty(conflicted_buffer)
            .expect("dirty preserved")
    );
}

#[test]
fn daily_editing_save_all_saves_all_dirty_buffers_in_tab_order() {
    let root = create_root();
    let first = root.join("ordered-first.txt");
    let second = root.join("ordered-second.txt");
    std::fs::write(&first, "first").expect("seed first");
    std::fs::write(&second, "second").expect("seed second");

    let mut app = trusted_app(&root);
    app.open_file(first.to_string_lossy()).expect("open first");
    let first_buffer = app.active_buffer_id().expect("first buffer");
    app.edit_active_buffer(TextEdit::insert(TextPosition::new(0, 5), "!"))
        .expect("edit first");
    app.open_file(second.to_string_lossy())
        .expect("open second");
    let second_buffer = app.active_buffer_id().expect("second buffer");
    app.edit_active_buffer(TextEdit::insert(TextPosition::new(0, 6), "!"))
        .expect("edit second");

    let outcome = app.save_all().expect("save all");
    assert_eq!(outcome.status, AppSaveAllStatus::Saved);
    assert_eq!(outcome.saved_count, 2);
    assert_eq!(outcome.rejected_count, 0);
    assert_eq!(
        outcome
            .results
            .iter()
            .map(|item| item.buffer_id)
            .collect::<Vec<_>>(),
        vec![first_buffer, second_buffer]
    );
    for item in &outcome.results {
        assert_eq!(item.status, AppSaveAllItemStatus::Saved);
        assert!(matches!(item.outcome, Some(AppSaveOutcome::Saved(_))));
        assert!(item.rejection_metadata.is_none());
        assert!(!item.final_dirty);
        assert!(item.file_id.is_some());
        assert!(item.file_path.is_some());
    }
    assert_eq!(
        std::fs::read_to_string(&first).expect("read first"),
        "first!"
    );
    assert_eq!(
        std::fs::read_to_string(&second).expect("read second"),
        "second!"
    );
}

#[test]
fn daily_editing_save_all_reports_mixed_conflict_metadata_and_dirty_state() {
    let root = create_root();
    let clean = root.join("mixed-clean.txt");
    let conflicted = root.join("mixed-conflicted.txt");
    std::fs::write(&clean, "clean").expect("seed clean");
    std::fs::write(&conflicted, "conflicted").expect("seed conflicted");

    let mut app = trusted_app(&root);
    app.open_file(clean.to_string_lossy()).expect("open clean");
    let clean_buffer = app.active_buffer_id().expect("clean buffer");
    app.edit_active_buffer(TextEdit::insert(TextPosition::new(0, 5), "!"))
        .expect("edit clean");
    app.open_file(conflicted.to_string_lossy())
        .expect("open conflicted");
    let conflicted_buffer = app.active_buffer_id().expect("conflicted buffer");
    app.edit_active_buffer(TextEdit::insert(TextPosition::new(0, 10), "!"))
        .expect("edit conflicted");
    std::fs::write(&conflicted, "external").expect("external overwrite");

    let outcome = app.save_all().expect("save all");
    assert_eq!(outcome.status, AppSaveAllStatus::Partial);
    assert_eq!(outcome.saved_count, 1);
    assert_eq!(outcome.rejected_count, 1);

    let clean_item = outcome
        .results
        .iter()
        .find(|item| item.buffer_id == clean_buffer)
        .expect("clean save item");
    assert_eq!(clean_item.status, AppSaveAllItemStatus::Saved);
    assert!(!clean_item.final_dirty);

    let rejected_item = outcome
        .results
        .iter()
        .find(|item| item.buffer_id == conflicted_buffer)
        .expect("rejected save item");
    assert_eq!(rejected_item.status, AppSaveAllItemStatus::Rejected);
    assert!(matches!(
        rejected_item.outcome,
        Some(AppSaveOutcome::Rejected(_))
    ));
    assert!(rejected_item.final_dirty);
    let metadata = rejected_item
        .rejection_metadata
        .as_ref()
        .expect("rejection metadata");
    assert!(matches!(
        metadata.response_kind.as_str(),
        "Conflict" | "Stale" | "Denied" | "Rejected" | "Failed"
    ));
    assert!(metadata.proposal_id.is_some());

    assert_eq!(
        app.editor().text(conflicted_buffer).expect("dirty text"),
        "conflicted!"
    );
    assert_eq!(
        std::fs::read_to_string(&conflicted).expect("external content"),
        "external"
    );
}

#[test]
fn daily_editing_save_all_no_open_buffers_returns_noop_outcome() {
    let root = create_root();
    let mut app = trusted_app(&root);

    let outcome = app.save_all().expect("save all no-op");
    assert_eq!(outcome.status, AppSaveAllStatus::Noop);
    assert!(outcome.results.is_empty());
    assert_eq!(outcome.saved_count, 0);
    assert_eq!(outcome.rejected_count, 0);
}

#[test]
fn daily_editing_contracts_close_dirty_requires_prompt() {
    let root = create_root();
    let clean = root.join("clean-close.txt");
    let dirty = root.join("dirty-close.txt");
    std::fs::write(&clean, "clean").expect("seed clean");
    std::fs::write(&dirty, "dirty").expect("seed dirty");

    let mut app = trusted_app(&root);
    app.open_file(clean.to_string_lossy()).expect("open clean");
    let clean_buffer = app.active_buffer_id().expect("clean buffer");
    app.open_file(dirty.to_string_lossy()).expect("open dirty");
    let dirty_buffer = app.active_buffer_id().expect("dirty buffer");

    let close_clean = app.close_tab(clean_buffer).expect("close clean");
    assert!(matches!(
        close_clean,
        AppCloseTabOutcome::Closed { buffer_id } if buffer_id == clean_buffer
    ));

    app.edit_active_buffer(TextEdit::insert(TextPosition::new(0, 5), "!"))
        .expect("edit dirty");
    let close_dirty = app.close_tab(dirty_buffer).expect("close dirty");
    assert!(matches!(
        close_dirty,
        AppCloseTabOutcome::CloseDirtyPrompt { buffer_id, .. } if buffer_id == dirty_buffer
    ));
    assert_eq!(app.active_buffer_id(), Some(dirty_buffer));
    assert_eq!(
        app.editor().text(dirty_buffer).expect("dirty text"),
        "dirty!"
    );
    assert!(app.editor().is_dirty(dirty_buffer).expect("dirty"));
    assert!(
        app.shell_projection_snapshot("daily")
            .expect("snapshot")
            .daily_editing_projection
            .close_dirty_prompt
            .is_some()
    );
}

#[test]
fn daily_editing_contracts_session_record_is_metadata_only() {
    let root = create_root();
    let target = root.join("session.txt");
    std::fs::write(&target, "seed").expect("seed target");
    let dirty_body = "SECRET_DIRTY_BODY";

    let mut app = trusted_app(&root);
    app.open_file(target.to_string_lossy())
        .expect("open target");
    app.edit_active_buffer(TextEdit::insert(TextPosition::new(0, 4), dirty_body))
        .expect("edit target");

    let record = app
        .capture_workspace_session_record()
        .expect("capture session");
    assert_eq!(record.open_tabs.len(), 1);
    assert_eq!(record.dirty_indicators.len(), 1);
    assert!(record.dirty_indicators[0].dirty);

    let serialized_shape = format!("{record:?}");
    assert!(!serialized_shape.contains(dirty_body));
    assert!(!serialized_shape.contains("seedSECRET"));
    assert!(
        app.shell_projection_snapshot("daily")
            .expect("snapshot")
            .daily_editing_projection
            .session_record
            .is_some()
    );
}

#[test]
fn daily_editing_contracts_hot_exit_restores_dirty_body_without_writing_disk() {
    let root = create_root();
    let target = root.join("session.txt");
    std::fs::write(&target, "seed").expect("seed target");
    let dirty_body = "SECRET_DIRTY_BODY";

    let mut app = trusted_app(&root);
    app.open_file(target.to_string_lossy())
        .expect("open target");
    app.edit_active_buffer(TextEdit::insert(TextPosition::new(0, 4), dirty_body))
        .expect("edit target");
    let record = app
        .capture_workspace_session_record()
        .expect("capture session");
    let snapshots = app.capture_hot_exit_snapshots().expect("capture hot-exit");
    assert_eq!(snapshots.len(), 1);
    assert!(snapshots[0].body.contains(dirty_body));
    let dir = root.join("unsaved");
    HotExitStore::save(&dir, &snapshots).expect("save hot-exit");
    let serialized_shape = format!("{record:?}");
    assert!(!serialized_shape.contains(dirty_body));

    drop(app);
    let mut restored = trusted_app(&root);
    restored
        .restore_workspace_session_record(&record)
        .expect("restore tabs");
    let loaded = HotExitStore::load(&dir).expect("load hot-exit");
    let count = restored
        .restore_hot_exit_snapshots(&loaded)
        .expect("restore hot-exit");
    assert_eq!(count, 1);
    let snapshot = restored
        .shell_projection_snapshot("daily")
        .expect("snapshot");
    let text = snapshot
        .active_buffer_projection
        .small_buffer_text()
        .expect("text");
    assert!(text.contains(dirty_body), "restored text was {text:?}");
    let disk = std::fs::read_to_string(&target).expect("disk");
    assert_eq!(disk, "seed");
}

#[test]
fn daily_editing_contracts_hot_exit_conflicts_when_disk_fingerprint_changed() {
    let root = create_root();
    let target = root.join("session.txt");
    std::fs::write(&target, "seed").expect("seed target");
    let dirty_body = "SECRET_DIRTY_BODY";

    let mut app = trusted_app(&root);
    app.open_file(target.to_string_lossy())
        .expect("open target");
    app.edit_active_buffer(TextEdit::insert(TextPosition::new(0, 4), dirty_body))
        .expect("edit target");
    let record = app
        .capture_workspace_session_record()
        .expect("capture session");
    let snapshots = app.capture_hot_exit_snapshots().expect("capture hot-exit");
    assert!(snapshots[0].disk_fingerprint.is_some());
    let dir = root.join("unsaved");
    HotExitStore::save(&dir, &snapshots).expect("save hot-exit");

    drop(app);
    std::fs::write(&target, "external-edit").expect("external overwrite");
    let mut restored = trusted_app(&root);
    restored
        .restore_workspace_session_record(&record)
        .expect("restore tabs");
    let loaded = HotExitStore::load(&dir).expect("load hot-exit");
    let count = restored
        .restore_hot_exit_snapshots(&loaded)
        .expect("restore hot-exit");
    assert_eq!(count, 1);
    let snapshot = restored
        .shell_projection_snapshot("daily")
        .expect("snapshot");
    let text = snapshot
        .active_buffer_projection
        .small_buffer_text()
        .expect("text");
    assert!(
        text.contains(dirty_body),
        "fingerprint mismatch must restore dirty body as unsaved, got {text:?}"
    );
    let recapture = restored
        .capture_hot_exit_snapshots()
        .expect("recapture hot-exit");
    assert_eq!(recapture.len(), 1);
    assert!(recapture[0].body.contains(dirty_body));
    let outcome = restored.save_active_buffer().expect("save");
    assert!(
        matches!(outcome, AppSaveOutcome::Rejected(_)),
        "save must conflict against the external edit, got {outcome:?}"
    );
    let disk = std::fs::read_to_string(&target).expect("disk");
    assert_eq!(disk, "external-edit");
}

#[test]
fn daily_editing_contracts_session_record_restores_memory_snapshot() {
    let root = create_root();
    let mut app = trusted_app(&root);

    let mut memory_service = MemoryService::new();
    memory_service
        .retain(MemoryCandidateRecord {
            candidate_id: "memory-candidate-restore".to_string(),
            run_id: Some(AgentRunId("memory-run-restore".to_string())),
            consent: MemoryConsentState::ProjectLongTerm,
            labels: vec!["memory.metadata_only".to_string()],
            correlation_id: CorrelationId(77),
            causality_id: CausalityId(uuid::Uuid::from_u128(77)),
            event_sequence: legion_protocol::EventSequence(77),
        })
        .expect("retain memory candidate");
    let memory_snapshot_json =
        serde_json::to_string(&memory_service.snapshot()).expect("serialize memory snapshot");

    let mut record = app
        .capture_workspace_session_record()
        .expect("capture session record");
    record.memory_snapshot_json = Some(memory_snapshot_json.clone());

    app.restore_workspace_session_record(&record)
        .expect("restore session record");

    let restored = app
        .capture_workspace_session_record()
        .expect("capture restored record");
    assert_eq!(
        restored.memory_snapshot_json.as_deref(),
        Some(memory_snapshot_json.as_str())
    );
}

#[test]
fn minimap_toggle_persists_through_settings() {
    let root = create_root();
    let file = root.join("minimap_test.rs");
    std::fs::write(&file, "fn main() {}\n").expect("seed file");

    let mut app = trusted_app(&root);
    app.open_file(file.to_string_lossy()).expect("open file");

    // Default: minimap_visible is false.
    let snapshot = app.shell_projection_snapshot("daily").expect("snapshot");
    assert!(
        !snapshot.settings_projection.editor.minimap_visible,
        "minimap should be hidden by default"
    );

    // Enable minimap.
    let outcome = app
        .dispatch_ui_intent(CommandDispatchIntent::SetMinimapVisible { visible: true })
        .expect("set minimap visible");
    assert!(
        matches!(outcome, AppCommandOutcome::SettingsUpdated(_)),
        "expected SettingsUpdated outcome"
    );

    let snapshot = app
        .shell_projection_snapshot("daily")
        .expect("snapshot after enable");
    assert!(
        snapshot.settings_projection.editor.minimap_visible,
        "minimap should be visible after toggle on"
    );

    // Disable minimap.
    let outcome = app
        .dispatch_ui_intent(CommandDispatchIntent::SetMinimapVisible { visible: false })
        .expect("set minimap hidden");
    assert!(
        matches!(outcome, AppCommandOutcome::SettingsUpdated(_)),
        "expected SettingsUpdated outcome"
    );

    let snapshot = app
        .shell_projection_snapshot("daily")
        .expect("snapshot after disable");
    assert!(
        !snapshot.settings_projection.editor.minimap_visible,
        "minimap should be hidden after toggle off"
    );
}
