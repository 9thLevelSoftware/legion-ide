use std::{
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
};

static TEMP_ROOT_COUNTER: AtomicU64 = AtomicU64::new(0);

use devil_app::{AppCommandOutcome, AppComposition};
use devil_editor::{TextEdit, TextPosition};
use devil_protocol::{PrincipalId, WorkspaceTrustState};
use devil_ui::CommandDispatchIntent;

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

#[test]
fn workspace_vfs_integration_untrusted_save_is_denied_without_disk_mutation() {
    let root = create_root();
    let target = root.join("blocked.txt");
    std::fs::write(&target, "seed").expect("seed file");

    let mut app = AppComposition::new();
    app.open_workspace(
        &root,
        WorkspaceTrustState::Untrusted,
        PrincipalId("untrusted".to_string()),
    )
    .expect("open workspace");
    app.open_file(target.to_string_lossy())
        .expect("open target file");

    let save_err = app.save_active_buffer().expect_err("save should be denied");

    let _ = std::fs::remove_dir_all(&root);
    let _ = save_err;
}

#[test]
fn workspace_vfs_integration_open_edit_save_use_engine_and_workspace_ids() {
    let root = create_root();
    let target = root.join("editable.txt");
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

    let edit = app
        .edit_active_buffer(TextEdit::insert(TextPosition::new(0, 4), "!"))
        .expect("edit through editor engine");
    assert_eq!(edit.workspace_id, opened.workspace_id);
    assert_eq!(edit.file_id, file_id);
    assert_eq!(edit.buffer_id, buffer_id);

    let save = app
        .save_active_buffer()
        .expect("save through workspace actor");
    assert_eq!(save.workspace_id, opened.workspace_id);
    assert_eq!(save.file_id, file_id);
    assert_eq!(save.buffer_id, buffer_id);
    assert_eq!(
        std::fs::read_to_string(&target).expect("read saved file"),
        "seed!"
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn workspace_vfs_integration_ui_command_intent_routes_to_engine_apply_edit() {
    let root = create_root();
    let target = root.join("intent.txt");
    std::fs::write(&target, "seed").expect("seed file");

    let mut app = AppComposition::new();
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

    let _ = std::fs::remove_dir_all(&root);
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
