//! Keyboard navigation smoke test for the desktop adapter.
//!
//! This regression ensures the product-mode switch can be activated without a
//! pointer by tabbing to the first pill and pressing Enter.

use std::{
    fs,
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use legion_desktop::{
    bridge::DesktopAction,
    workflow::{DesktopEframeApp, DesktopLaunchConfig, DesktopRuntime, DesktopWorkflowOutcome},
};
use legion_ui::DockMode;

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

struct TempWorkspace {
    root: std::path::PathBuf,
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
            "legion_desktop_keyboard_nav_{}_{}_{}",
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
}

impl Drop for TempWorkspace {
    fn drop(&mut self) {
        let temp_root = std::env::temp_dir();
        let file_name = self.root.file_name().and_then(|name| name.to_str());
        if self.root.starts_with(&temp_root)
            && file_name.is_some_and(|name| name.starts_with("legion_desktop_keyboard_nav_"))
        {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
}

fn open_runtime(root: &Path) -> DesktopRuntime {
    DesktopRuntime::open(DesktopLaunchConfig::new(root.to_path_buf(), None))
        .expect("desktop runtime should open workspace")
}

#[test]
fn product_mode_switch_accepts_keyboard_activation() {
    let workspace = TempWorkspace::new();
    let mut runtime = open_runtime(workspace.path());
    runtime
        .handle_action(DesktopAction::SetProductMode {
            mode: DockMode::Assist,
        })
        .expect("switching to Assist should succeed");
    let mut app = DesktopEframeApp::new(runtime);

    assert_eq!(app.runtime_snapshot().product_mode, DockMode::Assist);

    let input = egui::RawInput {
        focused: true,
        modifiers: egui::Modifiers {
            command: true,
            alt: true,
            ..egui::Modifiers::default()
        },
        events: vec![egui::Event::Key {
            key: egui::Key::M,
            physical_key: Some(egui::Key::M),
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers {
                command: true,
                alt: true,
                ..egui::Modifiers::default()
            },
        }],
        ..egui::RawInput::default()
    };
    let _ = app.run_headless_input(input);

    assert_eq!(
        app.runtime_snapshot().product_mode,
        DockMode::Manual,
        "keyboard activation should select the Manual product mode"
    );
}

// ─── T4: Problems panel keyboard navigation ───────────────────────────────────

/// `ProblemNext` moves the focused index forward and wraps around.
#[test]
fn t4_problem_next_increments_selection() {
    let workspace = TempWorkspace::new();
    let file = workspace.root.join("main.rs");
    std::fs::write(&file, "fn main() {}\n").expect("write file");
    let mut runtime = open_runtime(workspace.path());

    // ProblemNext on a runtime with no problems is a no-op (no crash).
    let outcome = runtime
        .handle_action(DesktopAction::ProblemNext)
        .expect("ProblemNext must not error");
    assert_eq!(outcome, DesktopWorkflowOutcome::Noop);
    assert_eq!(runtime.problems_selected_index_for_test(), 0);

    // Open the file through the app so a buffer is created.
    let src_file = file.to_string_lossy().to_string();
    runtime
        .app_mut_for_test()
        .open_file(file.to_string_lossy())
        .expect("open_file must succeed");
    let uri = format!(
        "file:///{}",
        src_file.replace('\\', "/").trim_start_matches('/')
    );
    let buffer_id = runtime
        .app_mut_for_test()
        .active_buffer_id()
        .expect("active buffer must exist after open_file");
    let params = serde_json::json!({
        "uri": uri,
        "diagnostics": [
            {
                "range": { "start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 1} },
                "severity": 1, "message": "error 1"
            },
            {
                "range": { "start": {"line": 1, "character": 0}, "end": {"line": 1, "character": 1} },
                "severity": 2, "message": "warning 2"
            }
        ]
    });
    runtime
        .app_mut_for_test()
        .ingest_lsp_publish_diagnostics_for_buffer(buffer_id, &params, false, None)
        .expect("inject diagnostics");

    // ProblemNext moves from index 0 → 1.
    runtime
        .handle_action(DesktopAction::ProblemNext)
        .expect("ProblemNext");
    assert_eq!(runtime.problems_selected_index_for_test(), 1);

    // ProblemNext wraps 1 → 0.
    runtime
        .handle_action(DesktopAction::ProblemNext)
        .expect("ProblemNext wraps");
    assert_eq!(runtime.problems_selected_index_for_test(), 0);
}

/// `ProblemPrev` moves the focused index backward and wraps around.
#[test]
fn t4_problem_prev_decrements_selection() {
    let workspace = TempWorkspace::new();
    let file = workspace.root.join("lib.rs");
    std::fs::write(&file, "pub fn f() {}\n").expect("write file");
    let mut runtime = open_runtime(workspace.path());

    // Open the file through the app so a buffer is created.
    runtime
        .app_mut_for_test()
        .open_file(file.to_string_lossy())
        .expect("open_file must succeed");
    let src_file = file.to_string_lossy().to_string();
    let uri = format!(
        "file:///{}",
        src_file.replace('\\', "/").trim_start_matches('/')
    );
    let buffer_id = runtime
        .app_mut_for_test()
        .active_buffer_id()
        .expect("active buffer");
    let params = serde_json::json!({
        "uri": uri,
        "diagnostics": [
            { "range": { "start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 1} },
              "severity": 1, "message": "e1" },
            { "range": { "start": {"line": 1, "character": 0}, "end": {"line": 1, "character": 1} },
              "severity": 1, "message": "e2" }
        ]
    });
    runtime
        .app_mut_for_test()
        .ingest_lsp_publish_diagnostics_for_buffer(buffer_id, &params, false, None)
        .expect("inject");

    // Start at 0; ProblemPrev wraps to 1 (last item).
    runtime
        .handle_action(DesktopAction::ProblemPrev)
        .expect("ProblemPrev");
    assert_eq!(runtime.problems_selected_index_for_test(), 1);

    // ProblemPrev again → 0.
    runtime
        .handle_action(DesktopAction::ProblemPrev)
        .expect("ProblemPrev again");
    assert_eq!(runtime.problems_selected_index_for_test(), 0);
}

/// `ProblemActivate` with no problems is a Noop (guard condition).
#[test]
fn t4_problem_activate_with_no_problems_is_noop() {
    let workspace = TempWorkspace::new();
    let mut runtime = open_runtime(workspace.path());
    let outcome = runtime
        .handle_action(DesktopAction::ProblemActivate)
        .expect("ProblemActivate must not error");
    assert_eq!(outcome, DesktopWorkflowOutcome::Noop);
}
