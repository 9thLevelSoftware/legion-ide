//! GAP-01.2: hosted windowed GUI must not export an empty `WGPU_BACKEND`.
//!
//! wgpu's `Backends::from_env` treats a *set* variable as the full mask.
//! `WGPU_BACKEND=` is zero backends, then
//! `Failed to create surface for any enabled backend: {}` on Windows/macOS.

use std::fs;
use std::path::PathBuf;

fn workflow_text() -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../.github/workflows/legion-windowed-gui.yml");
    fs::read_to_string(&path).unwrap_or_else(|err| panic!("read {}: {err}", path.display()))
}

#[test]
fn windowed_gui_workflow_does_not_export_empty_wgpu_backend() {
    let text = workflow_text();
    assert!(
        !text.contains("|| ''"),
        "do not use `&& 'gl' || ''` for WGPU_BACKEND; an empty export disables every backend"
    );
    assert!(text.contains("export WGPU_BACKEND=gl"));
    assert!(text.contains("export WGPU_BACKEND=dx12"));
    assert!(text.contains("export WGPU_BACKEND=metal"));
    assert!(
        !text.contains("WGPU_ADAPTER_NAME"),
        "adapter-name filters must not hide WARP/Metal after backends are enabled"
    );
}
