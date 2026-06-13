use std::sync::atomic::{AtomicU64, Ordering};

use legion_app::{
    AppCloseTabOutcome, AppCommandOutcome, AppComposition, AppSaveAllItemStatus, AppSaveAllStatus,
    AppSaveOutcome,
};
use legion_editor::{TextEdit, TextPosition};
use legion_memory::{MemoryCandidateRecord, MemoryConsentState, MemoryService};
use legion_protocol::{
    AgentRunId, CausalityId, CorrelationId, PrincipalId, ProtocolTextRange, TextCoordinate,
    ViewportScroll, ViewportSemanticTokenKind, WorkspaceTrustState,
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

    let _ = std::fs::remove_dir_all(&root);
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

    let _ = std::fs::remove_dir_all(&root);
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
                    || token.end_col <= rust_slashes
                    || token.start_col > rust_slashes
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
                    || token.end_col <= toml_hash
                    || token.start_col > toml_hash
            }),
        "TOML fallback must not mark # inside a string as a comment; got {:?}",
        toml_viewport.semantic_token_overlays
    );

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

    let _ = std::fs::remove_dir_all(&root);
}
