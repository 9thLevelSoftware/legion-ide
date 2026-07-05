use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use legion_desktop::{
    bridge::DesktopAction,
    workflow::{DesktopLaunchConfig, DesktopRuntime},
};
use serde_json::json;

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

struct TempWorkspace {
    root: PathBuf,
}

impl TempWorkspace {
    fn new() -> Self {
        let temp_root = std::env::temp_dir();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after epoch")
            .as_nanos();
        let id = TEMP_COUNTER.fetch_add(1, Ordering::SeqCst);
        let root = temp_root.join(format!(
            "legion_desktop_diagnostics_harness_{}_{}_{}",
            std::process::id(),
            nanos,
            id
        ));
        fs::create_dir(&root).expect("temp workspace should be created");
        Self { root }
    }

    fn path(&self) -> &Path {
        &self.root
    }

    fn write(&self, name: &str, content: &str) -> PathBuf {
        let path = self.root.join(name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("temp file parent should be created");
        }
        fs::write(&path, content).expect("temp file should be written");
        path
    }
}

impl Drop for TempWorkspace {
    fn drop(&mut self) {
        if self.root.starts_with(std::env::temp_dir()) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
}

fn open_runtime(root: &Path) -> DesktopRuntime {
    DesktopRuntime::open(DesktopLaunchConfig::new(root.to_path_buf(), None))
        .expect("desktop runtime should open workspace")
}

#[test]
fn desktop_runtime_projects_publish_diagnostics_and_clears_them_again() {
    let workspace = TempWorkspace::new();
    let file = workspace.write("src/main.rs", "fn main() {}\n");
    let mut runtime = open_runtime(workspace.path());

    runtime
        .handle_action(DesktopAction::OpenPathText(
            file.to_string_lossy().into_owned(),
        ))
        .expect("opening a workspace file should route through desktop harness");

    let buffer_id = runtime
        .projection_snapshot()
        .active_buffer_projection
        .buffer_id
        .expect("opening a file should activate a buffer");

    let diagnostics = json!({
        "uri": format!("file://{}", file.display()),
        "diagnostics": [
            {
                "range": {
                    "start": {"line": 0, "character": 3},
                    "end": {"line": 0, "character": 7}
                },
                "severity": 1,
                "source": "rustc",
                "message": "expected diagnostic text"
            }
        ]
    });

    runtime
        .ingest_lsp_publish_diagnostics_for_buffer(buffer_id, &diagnostics, true, None)
        .expect("desktop harness should project publishDiagnostics into the app snapshot");

    let snapshot = runtime.projection_snapshot();
    let file_id = snapshot
        .active_buffer_projection
        .file_id
        .expect("active file id");
    assert!(
        snapshot
            .language_tooling_projection
            .problems
            .iter()
            .any(|problem| problem.file_id == Some(file_id)
                && problem.source_label.as_deref() == Some("rustc")),
        "projected diagnostics should reach the problems panel source"
    );

    let cleared = json!({
        "uri": format!("file://{}", file.display()),
        "diagnostics": []
    });

    runtime
        .ingest_lsp_publish_diagnostics_for_buffer(buffer_id, &cleared, true, None)
        .expect("empty publishDiagnostics should clear the projected problem rows");

    assert!(
        runtime
            .projection_snapshot()
            .language_tooling_projection
            .problems
            .iter()
            .all(|problem| problem.file_id != Some(file_id)
                || problem.source_label.as_deref() != Some("rustc")),
        "cleared diagnostics should remove the problem row source used by the problems panel"
    );
}
