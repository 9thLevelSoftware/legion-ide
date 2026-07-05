use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use legion_desktop::{
    bridge::DesktopAction,
    view::DesktopProjectionViewModel,
    workflow::{DesktopLaunchConfig, DesktopRuntime, DesktopWorkflowOutcome},
};
use legion_protocol::ViewportProjectionMode;
use legion_ui::SearchScopeProjection;

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
            "legion_desktop_large_file_guardrails_{}_{}_{}",
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
        if self.root.starts_with(&temp_root) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
}

fn open_runtime(root: &Path, initial_file: &Path) -> DesktopRuntime {
    DesktopRuntime::open(DesktopLaunchConfig::new(
        root.to_path_buf(),
        Some(initial_file.to_string_lossy().into_owned()),
    ))
    .expect("desktop runtime should open workspace and file")
}

fn write_large_file(root: &Path) -> PathBuf {
    let path = root.join("large.txt");
    let mut content = String::from("visible viewport line\n");
    while content.len() <= 5 * 1024 * 1024 + 2048 {
        content.push_str("bounded large file filler line\n");
    }
    content.push_str("\nHIDDEN_NEEDLE_AFTER_VIEWPORT\n");
    fs::write(&path, content).expect("large file should be written");
    path
}

#[test]
fn large_file_guardrails_degraded_rendering_uses_viewport() {
    let workspace = TempWorkspace::new();
    let large = write_large_file(workspace.path());
    let runtime = open_runtime(workspace.path(), &large);
    let snapshot = runtime.projection_snapshot();

    assert!(snapshot.active_buffer_projection.degraded);
    assert!(
        snapshot
            .active_buffer_projection
            .small_buffer_preview
            .is_none()
    );
    let viewport = snapshot
        .active_buffer_projection
        .viewport
        .as_ref()
        .expect("degraded viewport projection");
    assert_eq!(viewport.mode, ViewportProjectionMode::DegradedLargeFile);
    assert!(viewport.large_file_status.is_some());
    assert!(!viewport.line_slices.is_empty());

    let model = DesktopProjectionViewModel::from_snapshot(&snapshot);
    assert!(
        model
            .empty_or_degraded_flags
            .iter()
            .any(|flag| flag == "degraded")
    );
    assert!(!model.active_buffer_lines.is_empty());
    assert!(
        model
            .active_buffer_lines
            .iter()
            .all(|line| !line.contains("HIDDEN_NEEDLE_AFTER_VIEWPORT"))
    );
}

#[test]
fn large_file_guardrails_degraded_banner_names_capability_reduction() {
    let workspace = TempWorkspace::new();
    let large = write_large_file(workspace.path());
    let runtime = open_runtime(workspace.path(), &large);
    let snapshot = runtime.projection_snapshot();
    let model = DesktopProjectionViewModel::from_snapshot(&snapshot);

    assert!(
        model
            .large_file_banner_rows
            .iter()
            .any(|row| row.contains("large-file degraded"))
    );
    assert!(
        model
            .large_file_banner_rows
            .iter()
            .any(|row| row.contains("capability reduced"))
    );
    assert!(
        model
            .large_file_banner_rows
            .iter()
            .all(|row| !row.contains("HIDDEN_NEEDLE_AFTER_VIEWPORT"))
    );
}

#[test]
fn large_file_guardrails_search_is_bounded() {
    let workspace = TempWorkspace::new();
    let large = write_large_file(workspace.path());
    let mut runtime = open_runtime(workspace.path(), &large);

    assert_eq!(
        runtime
            .handle_action(DesktopAction::RunSearch {
                scope: SearchScopeProjection::ActiveFile,
                query: "HIDDEN_NEEDLE_AFTER_VIEWPORT".to_string(),
                limit: 10,
                case_sensitive: None,
                whole_word: None,
                use_regex: None,
            })
            .expect("bounded degraded search"),
        DesktopWorkflowOutcome::SearchUpdated
    );

    let snapshot = runtime.projection_snapshot();
    assert!(snapshot.active_buffer_projection.degraded);
    assert!(snapshot.search_projection.results.is_empty());
    assert!(
        snapshot
            .search_projection
            .status
            .message
            .contains("limited to degraded viewport")
    );
}
