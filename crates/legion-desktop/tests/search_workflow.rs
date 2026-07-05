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
                case_sensitive: None,
                whole_word: None,
                use_regex: None,
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
            case_sensitive: None,
            whole_word: None,
            use_regex: None,
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
            case_sensitive: None,
            whole_word: None,
            use_regex: None,
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
            case_sensitive: None,
            whole_word: None,
            use_regex: None,
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
            case_sensitive: None,
            whole_word: None,
            use_regex: None,
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

/// Case-sensitivity option: same corpus, uppercase query.
/// With case_sensitive=Some(true) only exact-case matches count;
/// with case_sensitive=Some(false) both cases match.
/// This test fails if the case_sensitive option is not threaded through
/// AppCommandRequest -> run_search -> search engine.
#[test]
fn search_options_case_sensitive_yields_different_result_counts() {
    let workspace = TempWorkspace::new();
    // Line 1: uppercase; Line 2: lowercase; query will be uppercase.
    let target = workspace.write("mixed.txt", "NEEDLE\nneedle\n");
    let mut runtime = open_runtime(workspace.path(), &target);

    // Case-sensitive: only the uppercase "NEEDLE" line matches.
    runtime
        .handle_action(DesktopAction::RunSearch {
            scope: SearchScopeProjection::ActiveFile,
            query: "NEEDLE".to_string(),
            limit: 10,
            case_sensitive: Some(true),
            whole_word: None,
            use_regex: None,
        })
        .expect("case-sensitive search should route through AppCommandRequest");
    let sensitive_count = runtime
        .projection_snapshot()
        .search_projection
        .results
        .len();

    // Case-insensitive: both "NEEDLE" and "needle" match.
    runtime
        .handle_action(DesktopAction::RunSearch {
            scope: SearchScopeProjection::ActiveFile,
            query: "NEEDLE".to_string(),
            limit: 10,
            case_sensitive: Some(false),
            whole_word: None,
            use_regex: None,
        })
        .expect("case-insensitive search should route through AppCommandRequest");
    let insensitive_count = runtime
        .projection_snapshot()
        .search_projection
        .results
        .len();

    assert_eq!(
        sensitive_count, 1,
        "case_sensitive=true should match only the uppercase line"
    );
    assert_eq!(
        insensitive_count, 2,
        "case_sensitive=false should match both lines"
    );
    assert!(
        insensitive_count > sensitive_count,
        "case-insensitive search should return more results than case-sensitive"
    );
}

/// Regex option: a regex pattern matches content that a literal pattern would not.
/// This test fails if use_regex is not threaded through AppCommandRequest -> run_search.
#[test]
fn search_options_use_regex_matches_pattern_literal_does_not() {
    let workspace = TempWorkspace::new();
    // "a." as a regex matches any 2-char sequence starting with 'a';
    // "a." as a literal would look for a literal dot after 'a', which is absent.
    let target = workspace.write("regex_corpus.txt", "ab ac ad\n");
    let mut runtime = open_runtime(workspace.path(), &target);

    // Literal "a." — no literal dot in file, no match.
    runtime
        .handle_action(DesktopAction::RunSearch {
            scope: SearchScopeProjection::ActiveFile,
            query: "a.".to_string(),
            limit: 10,
            case_sensitive: Some(false),
            whole_word: None,
            use_regex: Some(false),
        })
        .expect("literal search should route through AppCommandRequest");
    let literal_count = runtime
        .projection_snapshot()
        .search_projection
        .results
        .len();

    // Regex "a." — matches "ab", "ac", "ad" → multiple hits on the same line.
    runtime
        .handle_action(DesktopAction::RunSearch {
            scope: SearchScopeProjection::ActiveFile,
            query: "a.".to_string(),
            limit: 10,
            case_sensitive: Some(false),
            whole_word: None,
            use_regex: Some(true),
        })
        .expect("regex search should route through AppCommandRequest");
    let regex_snapshot = runtime.projection_snapshot();
    let regex_status = &regex_snapshot.search_projection.status;
    let regex_count = regex_snapshot.search_projection.results.len();

    assert_eq!(
        literal_count, 0,
        "literal 'a.' should not match when no literal dot is present"
    );
    assert_eq!(
        regex_status.kind,
        SearchStatusKindProjection::Completed,
        "regex search status should be Completed, not {:?}: {}",
        regex_status.kind,
        regex_status.message
    );
    assert!(
        regex_count > 0,
        "regex 'a.' should match 'ab', 'ac', 'ad' in the line"
    );
}

/// Invalid regex surfaces a ValidationError rather than panicking.
/// This test fails if the error is not propagated through the full
/// AppCommandRequest -> run_search -> parse_search_query path.
#[test]
fn search_options_invalid_regex_surfaces_validation_error() {
    let workspace = TempWorkspace::new();
    let target = workspace.write("any.txt", "some content\n");
    let mut runtime = open_runtime(workspace.path(), &target);

    runtime
        .handle_action(DesktopAction::RunSearch {
            scope: SearchScopeProjection::ActiveFile,
            query: "[invalid".to_string(), // unclosed bracket — invalid regex
            limit: 10,
            case_sensitive: None,
            whole_word: None,
            use_regex: Some(true),
        })
        .expect("invalid-regex search should not panic; error must be projection-level");

    let snapshot = runtime.projection_snapshot();
    assert_eq!(
        snapshot.search_projection.status.kind,
        SearchStatusKindProjection::ValidationError,
        "invalid regex should produce a ValidationError projection, got {:?}: {}",
        snapshot.search_projection.status.kind,
        snapshot.search_projection.status.message
    );
}

/// Whole-word option: "needle" should not match "needler" when whole_word=true.
/// This test fails if whole_word is not threaded through AppCommandRequest -> run_search.
#[test]
fn search_options_whole_word_restricts_partial_matches() {
    let workspace = TempWorkspace::new();
    // One standalone "needle" and one "needler" that contains "needle" as a prefix.
    let target = workspace.write("words.txt", "needle\nneedler\n");
    let mut runtime = open_runtime(workspace.path(), &target);

    // Without whole-word: "needle" matches both "needle" and "needler".
    runtime
        .handle_action(DesktopAction::RunSearch {
            scope: SearchScopeProjection::ActiveFile,
            query: "needle".to_string(),
            limit: 10,
            case_sensitive: Some(false),
            whole_word: Some(false),
            use_regex: None,
        })
        .expect("partial-match search should route through AppCommandRequest");
    let partial_count = runtime
        .projection_snapshot()
        .search_projection
        .results
        .len();

    // With whole-word: only the standalone "needle" on line 1 matches.
    runtime
        .handle_action(DesktopAction::RunSearch {
            scope: SearchScopeProjection::ActiveFile,
            query: "needle".to_string(),
            limit: 10,
            case_sensitive: Some(false),
            whole_word: Some(true),
            use_regex: None,
        })
        .expect("whole-word search should route through AppCommandRequest");
    let whole_word_count = runtime
        .projection_snapshot()
        .search_projection
        .results
        .len();

    assert!(
        partial_count > 0,
        "partial search should find 'needle' in both lines, got 0"
    );
    assert!(
        whole_word_count < partial_count,
        "whole_word search (count={whole_word_count}) should return fewer results than partial (count={partial_count})"
    );
    assert_eq!(
        whole_word_count, 1,
        "whole_word=true should match only the standalone 'needle' line"
    );
}

/// Desktop projection header tags reflect the active search options.
/// After dispatching RunSearch through the full pipeline
/// (DesktopAction -> AppCommandRequest -> SearchProjection -> DesktopSearchViewModel),
/// the header must contain the correct option tags.
/// This test fails if the option threading is severed at any layer.
#[test]
fn search_options_header_tags_reflect_active_options() {
    let workspace = TempWorkspace::new();
    let target = workspace.write("tag_corpus.txt", "hello world\n");
    let mut runtime = open_runtime(workspace.path(), &target);

    // Case-sensitive: should render [Cc] tag.
    runtime
        .handle_action(DesktopAction::RunSearch {
            scope: SearchScopeProjection::ActiveFile,
            query: "hello".to_string(),
            limit: 10,
            case_sensitive: Some(true),
            whole_word: None,
            use_regex: None,
        })
        .expect("search with case_sensitive should route");
    let header_cs =
        DesktopSearchViewModel::from_projection(&runtime.projection_snapshot().search_projection)
            .header;
    assert!(
        header_cs.contains("[Cc]"),
        "case_sensitive=true should render [Cc] in header, got: {header_cs:?}"
    );

    // Case-insensitive override: no case tag — clean header.
    runtime
        .handle_action(DesktopAction::RunSearch {
            scope: SearchScopeProjection::ActiveFile,
            query: "hello".to_string(),
            limit: 10,
            case_sensitive: Some(false),
            whole_word: None,
            use_regex: None,
        })
        .expect("search with case_sensitive=false should route");
    let header_ci =
        DesktopSearchViewModel::from_projection(&runtime.projection_snapshot().search_projection)
            .header;
    assert!(
        !header_ci.contains("[Cc]") && !header_ci.contains("[ci]"),
        "case_sensitive=false should produce no case tag, got: {header_ci:?}"
    );

    // Whole-word: should render [W] tag (case-insensitive so no [Cc]).
    runtime
        .handle_action(DesktopAction::RunSearch {
            scope: SearchScopeProjection::ActiveFile,
            query: "hello".to_string(),
            limit: 10,
            case_sensitive: Some(false),
            whole_word: Some(true),
            use_regex: None,
        })
        .expect("search with whole_word should route");
    let header_ww =
        DesktopSearchViewModel::from_projection(&runtime.projection_snapshot().search_projection)
            .header;
    assert!(
        header_ww.contains("[W]"),
        "whole_word=true should render [W] in header, got: {header_ww:?}"
    );
    assert!(
        !header_ww.contains("[Cc]"),
        "case_sensitive=false should suppress [Cc], got: {header_ww:?}"
    );

    // Regex: should render [.*] tag (case-insensitive so no [Cc]).
    runtime
        .handle_action(DesktopAction::RunSearch {
            scope: SearchScopeProjection::ActiveFile,
            query: "hel+o".to_string(),
            limit: 10,
            case_sensitive: Some(false),
            whole_word: None,
            use_regex: Some(true),
        })
        .expect("search with use_regex should route");
    let header_rx =
        DesktopSearchViewModel::from_projection(&runtime.projection_snapshot().search_projection)
            .header;
    assert!(
        header_rx.contains("[.*]"),
        "use_regex=true should render [.*] in header, got: {header_rx:?}"
    );
    assert!(
        !header_rx.contains("[Cc]"),
        "case_sensitive=false should suppress [Cc], got: {header_rx:?}"
    );

    // All non-default options together: [Cc][W][.*].
    runtime
        .handle_action(DesktopAction::RunSearch {
            scope: SearchScopeProjection::ActiveFile,
            query: "hel+o".to_string(),
            limit: 10,
            case_sensitive: Some(true),
            whole_word: Some(true),
            use_regex: Some(true),
        })
        .expect("search with all options should route");
    let header_all =
        DesktopSearchViewModel::from_projection(&runtime.projection_snapshot().search_projection)
            .header;
    assert!(
        header_all.contains("[Cc]"),
        "all-options header should contain [Cc], got: {header_all:?}"
    );
    assert!(
        header_all.contains("[W]"),
        "all-options header should contain [W], got: {header_all:?}"
    );
    assert!(
        header_all.contains("[.*]"),
        "all-options header should contain [.*], got: {header_all:?}"
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
        skipped_binary_count: 0,
        case_sensitive: true,
        whole_word: false,
        use_regex: false,
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
