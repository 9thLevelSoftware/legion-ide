use std::sync::atomic::{AtomicU64, Ordering};

use legion_app::{
    AppCloseTabOutcome, AppCommandOutcome, AppComposition, AppSaveAllItemStatus, AppSaveAllStatus,
    AppSaveOutcome,
};
use legion_editor::{TextEdit, TextPosition};
use legion_protocol::{
    PrincipalId, ProtocolTextRange, TextCoordinate, ViewportScroll, WorkspaceTrustState,
};
use legion_ui::{CommandDispatchIntent, ShellLayoutProjection};

static TEMP_ROOT_COUNTER: AtomicU64 = AtomicU64::new(0);

fn create_root() -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!(
        "legion-app-daily-editing-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |value| value.as_millis() as u64)
            + TEMP_ROOT_COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&root).expect("create temp root");
    root
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
    assert_eq!(viewport.cursor.character, 3);
    assert_eq!(viewport.selections.len(), 1);
    assert_eq!(viewport.scroll.left_column, 2);

    let _ = std::fs::remove_dir_all(&root);
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

    let _ = std::fs::remove_dir_all(&root);
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

    let _ = std::fs::remove_dir_all(&root);
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

    let _ = std::fs::remove_dir_all(&root);
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

    let _ = std::fs::remove_dir_all(&root);
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

    let _ = std::fs::remove_dir_all(&root);
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

    let _ = std::fs::remove_dir_all(&root);
}
