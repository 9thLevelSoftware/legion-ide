use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use legion_desktop::{
    bridge::DesktopAction,
    workflow::{DesktopLaunchConfig, DesktopRuntime, DesktopWorkflowOutcome},
};
use legion_protocol::TextCoordinate;
use legion_ui::{DockMode, SearchScopeProjection};

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
fn diagnostics_export_path_parses_from_launch_args() {
    let config = DesktopLaunchConfig::from_args([
        "--workspace".into(),
        ".".into(),
        "--diagnostics-export".into(),
        "target/gui-phase6-diagnostics.md".into(),
    ])
    .expect("diagnostics args should parse");

    assert_eq!(
        config.diagnostics_export,
        Some(PathBuf::from("target/gui-phase6-diagnostics.md"))
    );
}

#[test]
fn diagnostics_export_writes_metadata_only_runtime_status() {
    let workspace = TempWorkspace::new("legion_desktop_diagnostics_export");
    let file = workspace.write("main.txt", "hello");
    let diagnostics_path = workspace.path().join("diagnostics.md");
    let session_path = workspace.path().join("session.json");
    let mut runtime = DesktopRuntime::open(
        DesktopLaunchConfig::new(
            workspace.path().to_path_buf(),
            Some(file.to_string_lossy().into_owned()),
        )
        .with_session_state(session_path)
        .with_diagnostics_export(diagnostics_path.clone()),
    )
    .expect("runtime should open");

    assert!(diagnostics_path.exists());
    let initial = fs::read_to_string(&diagnostics_path).expect("diagnostics should be readable");
    assert!(initial.contains("open_tab_count: 1"));
    assert!(initial.contains("session_state_configured: true"));
    assert!(initial.contains("## Operational Health"));
    assert!(initial.contains("diagnostics_export_configured: true"));
    assert!(initial.contains("unsupported_surfaces:"));
    assert!(initial.contains("clipboard_smoke: adapter-path passed"));
    assert!(initial.contains("ime_smoke: adapter-path passed"));
    assert!(initial.contains("file_dialog_smoke: adapter-path passed"));
    assert!(initial.contains("accessibility_projection_node_count:"));
    assert!(!initial.contains("clipboard_smoke: failed"));
    assert!(!initial.contains("hello"));

    assert_eq!(
        runtime
            .handle_action(DesktopAction::InsertText {
                text: "SECRET_DIRTY_BODY".to_string(),
                at: TextCoordinate {
                    line: 0,
                    character: 5,
                    byte_offset: Some(5),
                    utf16_offset: Some(5),
                },
            })
            .expect("edit should be routed through app authority"),
        DesktopWorkflowOutcome::Edited
    );
    runtime
        .handle_action(DesktopAction::RunSearch {
            scope: SearchScopeProjection::ActiveFile,
            query: "SECRET_PHASE7_QUERY".to_string(),
            limit: 10,
            case_sensitive: None,
            whole_word: None,
            use_regex: None,
        })
        .expect("search should be routed through app authority");
    runtime
        .handle_action(DesktopAction::TerminalLaunch {
            command_label: "SECRET_PHASE7_TERMINAL_PAYLOAD".to_string(),
        })
        .expect("terminal launch should be routed through app authority");
    assert_eq!(
        runtime
            .handle_action(DesktopAction::SetProductMode {
                mode: DockMode::Assist
            })
            .expect("assist mode should be routed through app authority"),
        DesktopWorkflowOutcome::ProductModeChanged {
            mode: DockMode::Assist
        }
    );
    runtime
        .handle_action(DesktopAction::StartAiProposal {
            instruction_label: "SECRET_PHASE7_PROMPT".to_string(),
        })
        .expect("proposal start should be routed through app authority");

    let updated = fs::read_to_string(&diagnostics_path).expect("diagnostics should be readable");
    assert!(updated.contains("dirty_tab_count: 1"));
    assert!(updated.contains("last_outcome: AssistedAiUpdated"));
    assert!(updated.contains("## Operational Health"));
    assert!(updated.contains("search_status: NoResults"));
    assert!(updated.contains("terminal_status: Running"));
    assert!(updated.contains("terminal_denial: none"));
    assert!(updated.contains("unsupported_surfaces:"));
    assert!(updated.contains("Autonomous apply: unsupported"));
    assert!(!updated.contains("SECRET_DIRTY_BODY"));
    assert!(!updated.contains("SECRET_PHASE7_QUERY"));
    assert!(!updated.contains("SECRET_PHASE7_TERMINAL_PAYLOAD"));
    assert!(!updated.contains("SECRET_PHASE7_PROMPT"));
    assert!(!updated.contains("small_buffer_preview"));
    assert!(!updated.contains("source_body"));
}
