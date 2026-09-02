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
    for name in [
        "ubuntu-33584539014.toml",
        "windows-33584539014.toml",
        "macos-33584539014.toml",
    ] {
        let text = fs::read_to_string(dir.join(name))
            .unwrap_or_else(|err| panic!("read {name}: {err}"));
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
