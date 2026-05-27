use std::sync::atomic::{AtomicU64, Ordering};

use devil_app::{AppCommandOutcome, AppComposition};
use devil_protocol::{PrincipalId, ProposalPayloadKind, TextCoordinate, WorkspaceTrustState};
use devil_ui::CommandDispatchIntent;

static TEMP_ROOT_COUNTER: AtomicU64 = AtomicU64::new(0);

fn create_root() -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!(
        "devil-language-tooling-{}-{}",
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
