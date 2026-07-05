use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use legion_desktop::{
    bridge::DesktopAction,
    search::DesktopSearchViewModel,
    view::DesktopProjectionViewModel,
    workflow::{DesktopLaunchConfig, DesktopRuntime, DesktopWorkflowOutcome},
};
use legion_protocol::{ProtocolTextRange, TextCoordinate, TimestampMillis};
use legion_ui::{
    SearchProjection, SearchScopeProjection, SearchStatusKindProjection, SearchStatusProjection,
};

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

fn coord(line: u32, character: u32, byte_offset: u64) -> TextCoordinate {
    TextCoordinate {
        line,
        character,
        byte_offset: Some(byte_offset),
        utf16_offset: Some(character as u64),
    }
}

fn range(start: u64, end: u64) -> ProtocolTextRange {
    ProtocolTextRange {
        start: coord(0, start as u32, start),
        end: coord(0, end as u32, end),
    }
}

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
            "legion_desktop_search_{}_{}_{}",
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
            && file_name.is_some_and(|name| name.starts_with("legion_desktop_search_"))
        {
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

#[test]
fn search_workflow_runs_active_file_search() {
    let workspace = TempWorkspace::new();
    let target = workspace.write("active.txt", "needle\nother needle\n");
    let mut runtime = open_runtime(workspace.path(), &target);

    assert_eq!(
        runtime
            .handle_action(DesktopAction::RunSearch {
                scope: SearchScopeProjection::ActiveFile,
                query: "needle".to_string(),
                limit: 10,
            })
            .expect("search should route through app authority"),
        DesktopWorkflowOutcome::SearchUpdated
    );

    let snapshot = runtime.projection_snapshot();
    assert_eq!(
        snapshot.search_projection.status.kind,
        SearchStatusKindProjection::Completed
    );
    assert_eq!(snapshot.search_projection.results.len(), 2);
    let model = DesktopSearchViewModel::from_projection(&snapshot.search_projection);
    assert!(model.header.contains("active file"));
    assert!(model.result_rows.iter().any(|row| row.contains("needle")));
}

#[test]
fn search_workflow_runs_workspace_search() {
    let workspace = TempWorkspace::new();
    let first = workspace.write("one.txt", "workspace needle\n");
    workspace.write("two.txt", "second needle\n");
    let mut runtime = open_runtime(workspace.path(), &first);

    runtime
        .handle_action(DesktopAction::RunSearch {
            scope: SearchScopeProjection::Workspace,
            query: "needle".to_string(),
            limit: 10,
        })
        .expect("workspace search should route through app authority");

    let snapshot = runtime.projection_snapshot();
    assert_eq!(snapshot.search_projection.results.len(), 2);
    assert!(
        DesktopSearchViewModel::from_projection(&snapshot.search_projection)
            .header
            .contains("workspace")
    );
}

#[test]
fn structural_search_workflow_runs_workspace_preview() {
    let workspace = TempWorkspace::new();
    let first = workspace.write("one.rs", "pub fn alpha() {}\n");
    let second = workspace.write("two.rs", "pub fn beta() {}\n");
    let mut runtime = open_runtime(workspace.path(), &first);

    assert_eq!(
        runtime
            .handle_action(DesktopAction::RunStructuralSearch {
                scope: SearchScopeProjection::Workspace,
                pattern: "fn $NAME ( )".to_string(),
                rewrite: Some("fn renamed_$NAME ( )".to_string()),
                limit: 10,
            })
            .expect("structural search should route through app authority"),
        DesktopWorkflowOutcome::StructuralSearchUpdated
    );

    let snapshot = runtime.projection_snapshot();
    let structural = &snapshot.structural_search_projection;
    assert_eq!(
        structural.status.kind,
        SearchStatusKindProjection::Completed
    );
    assert_eq!(structural.matches.len(), 2);
    assert_eq!(
        structural.rewrite_label.as_deref(),
        Some("fn renamed_$NAME ( )")
    );
    assert!(structural.proposal_id.is_some());
    assert!(
        structural
            .matches
            .iter()
            .any(|result| result.replacement_preview.as_deref() == Some("fn renamed_alpha ( )"))
    );
    let model = DesktopProjectionViewModel::from_snapshot(&snapshot);
    assert!(
        model
            .structural_search_rows
            .iter()
            .any(|row| row.contains("structural search: Completed matches=2"))
    );
    assert_eq!(
        fs::read_to_string(first).expect("first file should remain unchanged"),
        "pub fn alpha() {}\n"
    );
    assert_eq!(
        fs::read_to_string(second).expect("second file should remain unchanged"),
        "pub fn beta() {}\n"
    );
}

#[test]
fn search_workflow_projects_no_results_and_empty_query_errors() {
    let workspace = TempWorkspace::new();
    let target = workspace.write("none.txt", "text");
    let mut runtime = open_runtime(workspace.path(), &target);

    runtime
        .handle_action(DesktopAction::RunSearch {
            scope: SearchScopeProjection::ActiveFile,
            query: "missing".to_string(),
            limit: 10,
        })
        .expect("no-results search should route");
    assert_eq!(
        runtime.projection_snapshot().search_projection.status.kind,
        SearchStatusKindProjection::NoResults
    );

    runtime
        .handle_action(DesktopAction::RunSearch {
            scope: SearchScopeProjection::ActiveFile,
            query: " ".to_string(),
            limit: 10,
        })
        .expect("empty search should route");
    assert_eq!(
        runtime.projection_snapshot().search_projection.status.kind,
        SearchStatusKindProjection::ValidationError
    );
}

#[test]
fn search_workflow_cancels_current_query_by_id() {
    let workspace = TempWorkspace::new();
    let target = workspace.write("cancel.txt", "needle");
    let mut runtime = open_runtime(workspace.path(), &target);

    runtime
        .handle_action(DesktopAction::RunSearch {
            scope: SearchScopeProjection::ActiveFile,
            query: "needle".to_string(),
            limit: 10,
        })
        .expect("search should route");
    let query_id = runtime
        .projection_snapshot()
        .search_projection
        .query_id
        .expect("query id should be projected");

    runtime
        .handle_action(DesktopAction::CancelSearch { query_id })
        .expect("cancel should route");
    assert_eq!(
        runtime.projection_snapshot().search_projection.status.kind,
        SearchStatusKindProjection::Cancelled
    );
}

#[test]
fn search_workflow_displays_degraded_limited_projection() {
    let projection = SearchProjection {
        query_id: Some("search:test".to_string()),
        scope: SearchScopeProjection::ActiveFile,
        query_label: "needle".to_string(),
        status: SearchStatusProjection {
            kind: SearchStatusKindProjection::DegradedLimited,
            message: "Search was limited to degraded viewport content".to_string(),
        },
        results: vec![legion_ui::SearchResultProjection {
            query_id: "search:test".to_string(),
            scope: SearchScopeProjection::ActiveFile,
            workspace_id: None,
            buffer_id: None,
            file_id: None,
            file_path: None,
            line_number: 0,
            range: range(0, 6),
            snippet: "needle visible".to_string(),
            snippet_truncated: false,
            stale: false,
        }],
        result_limit: 10,
        omitted_result_count: 0,
        omitted_file_count: 0,
        diagnostics: vec!["limited to the visible viewport".to_string()],
        generated_at: TimestampMillis(1),
        schema_version: 1,
    };

    let model = DesktopSearchViewModel::from_projection(&projection);
    assert!(
        model
            .status_rows
            .iter()
            .any(|row| row.contains("DegradedLimited"))
    );
    assert!(
        model
            .diagnostic_rows
            .iter()
            .any(|row| row.contains("visible viewport"))
    );
}
