use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use legion_desktop::{
    bridge::DesktopAction,
    health::DesktopOperationalHealthSnapshot,
    view::DesktopProjectionViewModel,
    workflow::{DesktopLaunchConfig, DesktopRuntime, DesktopWorkflowOutcome},
};
use legion_protocol::TextCoordinate;
use legion_ui::SearchScopeProjection;

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

struct TempWorkspace {
    root: PathBuf,
}

impl TempWorkspace {
    fn new(prefix: &str) -> Self {
        let temp_root = std::env::temp_dir();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after epoch")
            .as_nanos();
        let id = TEMP_COUNTER.fetch_add(1, Ordering::SeqCst);
        let root = temp_root.join(format!("{prefix}_{}_{}_{}", std::process::id(), nanos, id));
        fs::create_dir(&root).expect("temp workspace should be created");
        Self { root }
    }

    fn path(&self) -> &Path {
        &self.root
    }

    fn write(&self, name: &str, content: &str) -> PathBuf {
        let path = self.root.join(name);
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

#[test]
fn operational_health_snapshot_and_view_rows_are_metadata_only() {
    let workspace = TempWorkspace::new("legion_desktop_operational_health");
    let file = workspace.write("main.rs", "fn main() { println!(\"SECRET_SOURCE_BODY\"); }");
    let mut runtime = DesktopRuntime::open(DesktopLaunchConfig::new(
        workspace.path().to_path_buf(),
        Some(file.to_string_lossy().into_owned()),
    ))
    .expect("runtime should open");

    assert_eq!(
        runtime
            .handle_action(DesktopAction::InsertText {
                text: "SECRET_PHASE7_DIRTY_BODY".to_string(),
                at: TextCoordinate {
                    line: 0,
                    character: 0,
                    byte_offset: Some(0),
                    utf16_offset: Some(0),
                },
            })
            .expect("edit should route through app authority"),
        DesktopWorkflowOutcome::Edited
    );
    runtime
        .handle_action(DesktopAction::RunSearch {
            scope: SearchScopeProjection::ActiveFile,
            query: "SECRET".to_string(),
            limit: 10,
        })
        .expect("search should route through app authority");
    runtime
        .handle_action(DesktopAction::TerminalLaunch {
            command_label: "SECRET_PHASE7_TERMINAL_PAYLOAD".to_string(),
        })
        .expect("terminal launch should route through app authority");
    runtime
        .handle_action(DesktopAction::StartAiProposal {
            instruction_label: "SECRET_PHASE7_PROMPT".to_string(),
        })
        .expect("proposal start should route through app authority");

    let snapshot = runtime.projection_snapshot();
    let health = DesktopOperationalHealthSnapshot::from_runtime(
        &snapshot,
        workspace.path().display().to_string(),
        runtime.last_outcome(),
        false,
        false,
    );
    let rows = health.rows();
    let joined = rows.join("\n");

    assert!(joined.contains("tabs: open=1 dirty=1"));
    assert!(joined.contains("search: status=Completed"));
    assert!(joined.contains("terminal: status=Denied"));
    assert!(joined.contains("proposals: rows="));
    assert!(joined.contains("unsupported: Remote production GUI: unsupported"));
    assert!(!joined.contains("SECRET_PHASE7_DIRTY_BODY"));
    assert!(!joined.contains("SECRET_PHASE7_TERMINAL_PAYLOAD"));
    assert!(!joined.contains("SECRET_PHASE7_PROMPT"));
    assert!(!joined.contains("SECRET_SOURCE_BODY"));
    assert!(!joined.contains("println!"));

    let model = DesktopProjectionViewModel::from_snapshot(&snapshot);
    let rendered_health = model.operational_health_rows.join("\n");
    assert!(rendered_health.contains("unsupported_surfaces:"));
    assert!(rendered_health.contains("terminal: status=Denied"));
    assert!(!rendered_health.contains("SECRET_PHASE7_DIRTY_BODY"));
    assert!(!rendered_health.contains("SECRET_PHASE7_TERMINAL_PAYLOAD"));
    assert!(!rendered_health.contains("SECRET_PHASE7_PROMPT"));
}

#[test]
fn operational_health_empty_projection_uses_safe_default_labels() {
    let snapshot = legion_ui::Shell::empty("Empty").projection_snapshot();
    let health = DesktopOperationalHealthSnapshot::from_projection(&snapshot);

    assert_eq!(health.open_tab_count, 0);
    assert_eq!(health.dirty_tab_count, 0);
    assert_eq!(health.last_outcome_label, "not_observed");
    assert_eq!(health.terminal_denial_label, "none");
    assert!(
        health
            .unsupported_surfaces
            .iter()
            .any(|surface| surface.contains("Autonomous apply: unsupported"))
    );
}
