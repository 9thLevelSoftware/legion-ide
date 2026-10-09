//! Exact small-buffer accessibility metadata through the public app boundary.

use legion_app::AppComposition;
use legion_protocol::{PrincipalId, WorkspaceTrustState};
use legion_ui::ui::EditorAccessibilityCoverage;

#[test]
fn active_document_metadata_names_exact_editable_small_snapshot() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("note.txt");
    std::fs::write(&path, "a\u{301}🦀\r\n").unwrap();
    let mut app = AppComposition::new();
    app.open_workspace(
        root.path(),
        WorkspaceTrustState::Untrusted,
        PrincipalId("accessibility".into()),
    )
    .unwrap();
    app.open_file(path.to_string_lossy()).unwrap();
    let active = app
        .shell_projection_snapshot("a11y")
        .unwrap()
        .active_buffer_projection;
    let metadata = active
        .accessibility
        .expect("app publishes document metadata");
    let viewport = active.viewport.unwrap();
    assert_eq!(Some(metadata.buffer_id), active.buffer_id);
    assert_eq!(metadata.snapshot_id, viewport.snapshot_id);
    assert_eq!(metadata.buffer_version, viewport.buffer_version);
    assert_eq!(metadata.byte_len, 9);
    assert!(
        metadata.editable,
        "workspace trust affects saving, not buffer editability"
    );
    assert_eq!(
        metadata.coverage,
        EditorAccessibilityCoverage::CompleteSmallBuffer
    );
}
