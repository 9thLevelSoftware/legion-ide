//! Private lifecycle tests for worker diagnostic identity fencing.

use std::path::Path;

use legion_editor::{TextEdit, TextPosition};
use legion_protocol::{BufferId, PrincipalId, WorkspaceTrustState};
use serde_json::{Value, json};

fn params(path: &Path, version: u64, code: &str) -> Value {
    let path = path.to_string_lossy();
    json!({
        "uri": crate::canonical_path_to_uri(path.as_ref()),
        "version": version,
        "diagnostics": [{
            "range": {
                "start": {"line": 0, "character": 0},
                "end": {"line": 0, "character": 1}
            },
            "severity": 1,
            "code": code,
            "source": "diagnostic-lifecycle-test",
            "message": code
        }]
    })
}

fn version(app: &crate::AppComposition, buffer: BufferId) -> u64 {
    app.editor
        .current_snapshot(buffer)
        .expect("snapshot")
        .buffer_version
        .0
}

fn has_code(app: &crate::AppComposition, code: &str) -> bool {
    app.language_tooling_projection()
        .problems
        .iter()
        .any(|p| p.code_label.as_deref() == Some(code))
}

#[test]
fn diagnostic_batches_require_current_snapshot_and_reopened_buffer_identity() {
    let root = tempfile::tempdir().expect("workspace");
    let source = root.path().join("main.ts");
    std::fs::write(&source, "const value = 1;\n").expect("source");
    let mut app = crate::AppComposition::new();
    app.open_workspace(
        root.path(),
        WorkspaceTrustState::Trusted,
        PrincipalId("diagnostic-test".into()),
    )
    .expect("workspace");
    app.open_file(source.to_string_lossy()).expect("open");
    let original = app.active_buffer_id().expect("buffer");
    let initial = version(&app, original);
    app.ingest_lsp_diagnostic_batch(params(&source, initial, "CURRENT"), Some(original));
    assert!(has_code(&app, "CURRENT"));
    app.edit_active_buffer(TextEdit::insert(TextPosition::new(0, 0), "// edit\n"))
        .expect("edit");
    let edited = version(&app, original);
    assert!(edited > initial);
    app.ingest_lsp_diagnostic_batch(params(&source, initial, "STALE"), Some(original));
    assert!(!has_code(&app, "STALE"));
    app.ingest_lsp_diagnostic_batch(params(&source, edited, "EDITED"), Some(original));
    assert!(has_code(&app, "EDITED"));
    let save = app
        .save_active_buffer()
        .expect("save edited source before close");
    assert!(matches!(save, crate::AppSaveOutcome::Saved(_)));
    let close = app.close_tab(original).expect("close");
    assert!(matches!(
        close,
        crate::AppCloseTabOutcome::Closed { buffer_id } if buffer_id == original
    ));
    app.open_file(source.to_string_lossy()).expect("reopen");
    let reopened = app.active_buffer_id().expect("reopened buffer");
    assert_ne!(reopened, original);
    let reopened_version = version(&app, reopened);
    app.ingest_lsp_diagnostic_batch(
        params(&source, reopened_version, "OLD-BUFFER"),
        Some(original),
    );
    assert!(!has_code(&app, "OLD-BUFFER"));
    app.ingest_lsp_diagnostic_batch(
        params(&source, reopened_version, "REOPENED"),
        Some(reopened),
    );
    assert!(has_code(&app, "REOPENED"));
}
