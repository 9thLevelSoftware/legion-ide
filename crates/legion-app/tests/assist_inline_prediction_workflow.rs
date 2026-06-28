use std::sync::atomic::{AtomicU64, Ordering};

use legion_app::{AppCommandOutcome, AppComposition, AppCompositionError, AppProductMode};
use legion_editor::{TextEdit, TextPosition};
use legion_protocol::{PrincipalId, TextCoordinate, TransactionSource, WorkspaceTrustState};
use legion_ui::CommandDispatchIntent;

static TEMP_ROOT_COUNTER: AtomicU64 = AtomicU64::new(0);

fn create_root() -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!(
        "legion-app-assist-inline-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |value| value.as_millis() as u64)
            + TEMP_ROOT_COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&root).expect("create temp root");
    root
}

fn open_rust_file(app: &mut AppComposition, root: &std::path::Path) -> legion_protocol::BufferId {
    let target = root.join("main.rs");
    std::fs::write(&target, "fn main() {}\n").expect("seed rust file");
    app.open_workspace(
        root,
        WorkspaceTrustState::Trusted,
        PrincipalId("assist-inline-test".to_string()),
    )
    .expect("open workspace");
    app.open_file(target.to_string_lossy())
        .expect("open rust file");
    app.shell_projection_snapshot("active")
        .expect("shell snapshot")
        .active_buffer_projection
        .buffer_id
        .expect("active buffer id")
}

fn cursor_at_line_end() -> TextCoordinate {
    TextCoordinate {
        line: 0,
        character: 12,
        byte_offset: Some(12),
        utf16_offset: Some(12),
    }
}

fn inline_projection(outcome: AppCommandOutcome) -> legion_ui::AssistInlinePredictionProjection {
    match outcome {
        AppCommandOutcome::AssistInlinePredictionUpdated(projection) => projection,
        other => panic!("expected inline prediction outcome, got {other:?}"),
    }
}

#[test]
fn manual_mode_rejects_assist_inline_prediction_request() {
    let root = create_root();
    let mut app = AppComposition::new();
    let buffer_id = open_rust_file(&mut app, &root);

    let error = app
        .dispatch_ui_intent(CommandDispatchIntent::RequestAssistInlinePrediction {
            buffer_id,
            position: cursor_at_line_end(),
        })
        .expect_err("manual mode rejects inline prediction");
    assert!(matches!(
        error,
        AppCompositionError::AiRuntime(message)
            if message.contains("requires Assist, Delegate, or Automate")
    ));

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn assist_inline_prediction_request_projects_and_accepts_bounded_ghost_text() {
    let root = create_root();
    let mut app = AppComposition::new();
    app.set_product_mode(AppProductMode::Assist);
    let buffer_id = open_rust_file(&mut app, &root);

    let projection = inline_projection(
        app.dispatch_ui_intent(CommandDispatchIntent::RequestAssistInlinePrediction {
            buffer_id,
            position: cursor_at_line_end(),
        })
        .expect("request inline prediction"),
    );
    let active = projection
        .active_prediction
        .as_ref()
        .expect("active ghost prediction");
    assert_eq!(active.buffer_id, Some(buffer_id));
    assert_eq!(
        active.status,
        legion_ui::AssistInlinePredictionStatusProjection::Ready
    );
    assert!(active.ghost_text_label.contains("next edit line 1"));

    let accepted = inline_projection(
        app.dispatch_ui_intent(CommandDispatchIntent::AcceptAssistInlinePrediction {
            buffer_id,
            prediction_id: Some(active.prediction_id.clone()),
        })
        .expect("accept inline prediction"),
    );
    assert!(accepted.active_prediction.is_none());
    assert!(accepted.rows.iter().any(|row| {
        row.prediction_id == active.prediction_id
            && row.status == legion_ui::AssistInlinePredictionStatusProjection::Accepted
    }));
    assert_eq!(
        app.editor().text(buffer_id).expect("editor text"),
        "fn main() {} // next edit line 1\n"
    );
    assert_eq!(app.editor().undo_len(buffer_id).expect("undo len"), 1);
    let tx_log = app.editor().transaction_log();
    assert_eq!(tx_log.len(), 1);
    assert!(matches!(tx_log[0].source, TransactionSource::User));
    assert!(tx_log[0].undo_group_id.is_some());

    let undo = app
        .dispatch_ui_intent(CommandDispatchIntent::Undo { buffer_id })
        .expect("undo inline accept");
    assert!(matches!(undo, AppCommandOutcome::Edited(_)));
    assert_eq!(
        app.editor().text(buffer_id).expect("undo text"),
        "fn main() {}\n"
    );
    assert_eq!(app.editor().redo_len(buffer_id).expect("redo len"), 1);
    assert_eq!(app.editor().undo_len(buffer_id).expect("undo len"), 0);

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn assist_inline_prediction_accept_marks_stale_without_mutating_after_buffer_change() {
    let root = create_root();
    let mut app = AppComposition::new();
    app.set_product_mode(AppProductMode::Assist);
    let buffer_id = open_rust_file(&mut app, &root);

    let projection = inline_projection(
        app.dispatch_ui_intent(CommandDispatchIntent::RequestAssistInlinePrediction {
            buffer_id,
            position: cursor_at_line_end(),
        })
        .expect("request inline prediction"),
    );
    let active = projection
        .active_prediction
        .as_ref()
        .expect("active prediction");
    let prediction_id = active.prediction_id.clone();
    let snapshot_id = active.snapshot_id.expect("request snapshot id");
    let buffer_version = active.buffer_version.expect("request buffer version");

    let current = app
        .editor()
        .current_snapshot(buffer_id)
        .expect("current snapshot");
    assert_eq!(current.snapshot_id, snapshot_id);
    assert_eq!(current.buffer_version, buffer_version);

    app.edit_active_buffer(TextEdit::insert(TextPosition::new(0, 0), "// changed\n"))
        .expect("first intervening edit");
    app.edit_active_buffer(TextEdit::insert(
        TextPosition::new(1, 0),
        "let typed = 1;\n",
    ))
    .expect("second intervening edit");

    let stale = inline_projection(
        app.dispatch_ui_intent(CommandDispatchIntent::AcceptAssistInlinePrediction {
            buffer_id,
            prediction_id: Some(prediction_id.clone()),
        })
        .expect("stale accept updates projection"),
    );

    assert!(stale.active_prediction.is_none());
    let stale_row = stale
        .rows
        .iter()
        .find(|row| row.prediction_id == prediction_id)
        .expect("stale row retained as metadata");
    assert_eq!(
        stale_row.status,
        legion_ui::AssistInlinePredictionStatusProjection::Stale
    );
    assert_eq!(stale_row.snapshot_id, Some(snapshot_id));
    assert_eq!(stale_row.buffer_version, Some(buffer_version));
    assert!(stale_row.stale);
    let text = app.editor().text(buffer_id).expect("editor text");
    assert!(text.starts_with("// changed\nlet typed = 1;\n"));
    assert!(!text.contains("next edit line 1"));

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn assist_inline_prediction_projection_hides_ghost_text_when_buffer_becomes_stale() {
    let root = create_root();
    let mut app = AppComposition::new();
    app.set_product_mode(AppProductMode::Assist);
    let buffer_id = open_rust_file(&mut app, &root);

    let projection = inline_projection(
        app.dispatch_ui_intent(CommandDispatchIntent::RequestAssistInlinePrediction {
            buffer_id,
            position: cursor_at_line_end(),
        })
        .expect("request inline prediction"),
    );
    let prediction_id = projection
        .active_prediction
        .as_ref()
        .expect("active prediction")
        .prediction_id
        .clone();

    app.edit_active_buffer(TextEdit::insert(TextPosition::new(0, 0), "// changed\n"))
        .expect("intervening edit");
    let projected = app
        .shell_projection_snapshot("stale")
        .expect("snapshot after intervening edit")
        .assist_inline_prediction_projection;

    assert!(projected.active_prediction.is_none());
    let stale_row = projected
        .rows
        .iter()
        .find(|row| row.prediction_id == prediction_id)
        .expect("stale row retained as metadata");
    assert_eq!(
        stale_row.status,
        legion_ui::AssistInlinePredictionStatusProjection::Stale
    );
    assert_eq!(stale_row.ghost_text_label, "");

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn switching_back_to_manual_clears_assist_inline_prediction_projection() {
    let root = create_root();
    let mut app = AppComposition::new();
    app.set_product_mode(AppProductMode::Assist);
    let buffer_id = open_rust_file(&mut app, &root);

    let assist_projection = inline_projection(
        app.dispatch_ui_intent(CommandDispatchIntent::RequestAssistInlinePrediction {
            buffer_id,
            position: cursor_at_line_end(),
        })
        .expect("request inline prediction"),
    );
    assert!(assist_projection.active_prediction.is_some());

    app.set_product_mode(AppProductMode::Manual);
    let manual_projection = app
        .shell_projection_snapshot("manual")
        .expect("manual snapshot")
        .assist_inline_prediction_projection;

    assert!(manual_projection.active_prediction.is_none());
    assert!(manual_projection.rows.is_empty());
    assert!(!manual_projection.request_in_flight);

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn accepted_assist_inline_prediction_cannot_be_dismissed_again() {
    let root = create_root();
    let mut app = AppComposition::new();
    app.set_product_mode(AppProductMode::Assist);
    let buffer_id = open_rust_file(&mut app, &root);

    let projection = inline_projection(
        app.dispatch_ui_intent(CommandDispatchIntent::RequestAssistInlinePrediction {
            buffer_id,
            position: cursor_at_line_end(),
        })
        .expect("request inline prediction"),
    );
    let prediction_id = projection
        .active_prediction
        .as_ref()
        .expect("active prediction")
        .prediction_id
        .clone();

    inline_projection(
        app.dispatch_ui_intent(CommandDispatchIntent::AcceptAssistInlinePrediction {
            buffer_id,
            prediction_id: Some(prediction_id.clone()),
        })
        .expect("accept inline prediction"),
    );

    let error = app
        .dispatch_ui_intent(CommandDispatchIntent::DismissAssistInlinePrediction {
            buffer_id,
            prediction_id: Some(prediction_id),
        })
        .expect_err("terminal prediction state cannot transition again");
    assert!(matches!(
        error,
        AppCompositionError::AiRuntime(message)
            if message.contains("must be available")
    ));

    let _ = std::fs::remove_dir_all(root);
}
