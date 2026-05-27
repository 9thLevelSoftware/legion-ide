use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use devil_app::{AppCommandOutcome, AppComposition};
use devil_protocol::{PrincipalId, WorkspaceTrustState};
use devil_ui::{CommandDispatchIntent, SearchScopeProjection, SearchStatusKindProjection};

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

struct TempWorkspace {
    root: PathBuf,
}

impl TempWorkspace {
    fn new() -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after epoch")
            .as_nanos();
        let id = TEMP_COUNTER.fetch_add(1, Ordering::SeqCst);
        let root = std::env::temp_dir().join(format!(
            "devil_app_daily_search_{}_{}_{}",
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
        fs::write(&path, content).expect("temp file should be written");
        path
    }
}

impl Drop for TempWorkspace {
    fn drop(&mut self) {
        let temp_root = std::env::temp_dir();
        let file_name = self.root.file_name().and_then(|name| name.to_str());
        if self.root.starts_with(&temp_root)
            && file_name.is_some_and(|name| name.starts_with("devil_app_daily_search_"))
        {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
}

fn open_app(root: &Path, file: Option<&Path>) -> AppComposition {
    let mut app = AppComposition::new();
    app.open_workspace(
        root,
        WorkspaceTrustState::Trusted,
        PrincipalId("search-test".to_string()),
    )
    .expect("workspace should open");
    if let Some(file) = file {
        app.open_file(file.to_string_lossy())
            .expect("file should open");
    }
    app
}

fn run_search(
    app: &mut AppComposition,
    scope: SearchScopeProjection,
    query: &str,
    limit: usize,
) -> devil_ui::SearchProjection {
    match app
        .dispatch_ui_intent(CommandDispatchIntent::RunSearch {
            scope,
            query: query.to_string(),
            limit,
        })
        .expect("search intent should dispatch")
    {
        AppCommandOutcome::SearchUpdated(projection) => projection,
        other => panic!("expected search outcome, got {other:?}"),
    }
}

#[test]
fn daily_editing_search_active_file_returns_bounded_rows() {
    let workspace = TempWorkspace::new();
    let target = workspace.write("active.txt", "alpha\nbeta alpha\n");
    let mut app = open_app(workspace.path(), Some(&target));

    let projection = run_search(&mut app, SearchScopeProjection::ActiveFile, "alpha", 10);

    assert_eq!(
        projection.status.kind,
        SearchStatusKindProjection::Completed
    );
    assert_eq!(projection.results.len(), 2);
    assert_eq!(projection.results[0].line_number, 0);
    assert_eq!(projection.results[1].line_number, 1);
    assert_eq!(projection.results[0].snippet, "alpha");
    assert_eq!(
        app.shell_projection_snapshot("search")
            .expect("projection snapshot")
            .search_projection,
        projection
    );
}

#[test]
fn daily_editing_search_workspace_scans_authorized_files() {
    let workspace = TempWorkspace::new();
    let first = workspace.write("one.txt", "needle one\n");
    let second = workspace.write("two.txt", "prefix needle two\n");
    let mut app = open_app(workspace.path(), Some(&first));

    let projection = run_search(&mut app, SearchScopeProjection::Workspace, "needle", 10);

    assert_eq!(
        projection.status.kind,
        SearchStatusKindProjection::Completed
    );
    assert_eq!(projection.results.len(), 2);
    assert!(projection.results.iter().any(|row| {
        row.file_path
            .as_ref()
            .is_some_and(|path| path.0.ends_with("one.txt"))
    }));
    assert!(projection.results.iter().any(|row| {
        row.file_path
            .as_ref()
            .is_some_and(|path| path.0.ends_with("two.txt"))
    }));
    assert_eq!(fs::read_to_string(first).expect("first"), "needle one\n");
    assert_eq!(
        fs::read_to_string(second).expect("second"),
        "prefix needle two\n"
    );
}

#[test]
fn daily_editing_search_empty_query_is_validation_error() {
    let workspace = TempWorkspace::new();
    let target = workspace.write("empty.txt", "anything");
    let mut app = open_app(workspace.path(), Some(&target));

    let projection = run_search(&mut app, SearchScopeProjection::ActiveFile, "   ", 10);

    assert_eq!(
        projection.status.kind,
        SearchStatusKindProjection::ValidationError
    );
    assert!(projection.results.is_empty());
    assert_eq!(projection.query_label, "");
}

#[test]
fn daily_editing_search_limit_tracks_omitted_results() {
    let workspace = TempWorkspace::new();
    let target = workspace.write("limit.txt", "hit hit hit hit\n");
    let mut app = open_app(workspace.path(), Some(&target));

    let projection = run_search(&mut app, SearchScopeProjection::ActiveFile, "hit", 2);

    assert_eq!(projection.results.len(), 2);
    assert_eq!(projection.omitted_result_count, 2);
    assert_eq!(projection.result_limit, 2);
}

#[test]
fn daily_editing_search_workspace_skips_oversized_files() {
    let workspace = TempWorkspace::new();
    let oversized = format!("needle{}", "x".repeat(256 * 1024 + 1));
    workspace.write("oversized.txt", &oversized);
    let mut app = open_app(workspace.path(), None);

    let projection = run_search(&mut app, SearchScopeProjection::Workspace, "needle", 10);

    assert_eq!(
        projection.status.kind,
        SearchStatusKindProjection::NoResults
    );
    assert_eq!(projection.results.len(), 0);
    assert_eq!(projection.omitted_file_count, 1);
    assert!(
        projection
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.contains("exceeds the workspace search bound"))
    );
}

#[test]
fn daily_editing_search_degraded_active_file_uses_limited_viewport() {
    let workspace = TempWorkspace::new();
    let mut text = String::from("needle visible\n");
    text.push_str(&"x".repeat(5 * 1024 * 1024 + 1));
    let target = workspace.write("huge.txt", &text);
    let mut app = open_app(workspace.path(), Some(&target));

    let projection = run_search(&mut app, SearchScopeProjection::ActiveFile, "needle", 10);

    assert_eq!(
        projection.status.kind,
        SearchStatusKindProjection::DegradedLimited
    );
    assert_eq!(projection.results.len(), 1);
    assert!(
        projection
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.contains("limited to the visible viewport"))
    );
    let snapshot = app
        .shell_projection_snapshot("search")
        .expect("shell projection");
    assert!(snapshot.active_buffer_projection.degraded);
    assert!(
        snapshot
            .active_buffer_projection
            .small_buffer_preview
            .is_none()
    );
}
