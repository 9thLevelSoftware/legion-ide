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
fn windowed_gui_3os_green_reports_set_window_created() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../plans/evidence/production/WS-P0/windowed-gui-3os");
    let mut found = 0;
    for entry in fs::read_dir(&dir).unwrap_or_else(|err| panic!("read {}: {err}", dir.display())) {
        let entry = entry.expect("dir entry");
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if !name.ends_with(".toml") {
            continue;
        }
        found += 1;
        let text = fs::read_to_string(entry.path())
            .unwrap_or_else(|err| panic!("read {name}: {err}"));
        let name = name.as_ref();
        assert!(
            text.contains("window_created = true"),
            "{name} must record a native window"
        );
        assert!(
            text.contains("status = \"passed\""),
            "{name} must be a passed open/edit/save"
        );
        assert!(
            text.contains("not_beta_smoke = true"),
            "{name} must not be --beta-smoke"
        );
        assert!(
            text.contains("not_golden_path_5 = true"),
            "{name} must not be golden-path-5"
        );
    }
    assert!(
        found >= 6,
        "expected clock-run reports for at least two 3-OS dispatches, found {found}"
    );
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
