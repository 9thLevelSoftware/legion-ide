use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use legion_app::{AppCommandOutcome, AppComposition};
use legion_protocol::{PrincipalId, TextCoordinate, WorkspaceTrustState};
use legion_ui::{
    CommandDispatchIntent, PaletteMode, PaletteResultKind, SearchScopeProjection,
    SearchStatusKindProjection, ShellLayoutProjection,
};

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
            "legion_app_palette_{}_{}_{}",
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
            fs::create_dir_all(parent).expect("temp parent should be created");
        }
        fs::write(&path, content).expect("temp file should be written");
        path
    }
}

impl Drop for TempWorkspace {
    fn drop(&mut self) {
        let temp_root = std::env::temp_dir();
        let file_name = self.root.file_name().and_then(|name| name.to_str());
        if self.root.starts_with(&temp_root)
            && file_name.is_some_and(|name| name.starts_with("legion_app_palette_"))
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
        PrincipalId("palette-test".to_string()),
    )
    .expect("workspace should open");
    if let Some(file) = file {
        app.open_file(file.to_string_lossy())
            .expect("file should open");
    }
    app
}

fn projected_path_eq(actual: Option<&str>, expected: &Path) -> bool {
    let Some(actual) = actual else {
        return false;
    };
    let Ok(actual) = Path::new(actual).canonicalize() else {
        return false;
    };
    let Ok(expected) = expected.canonicalize() else {
        return false;
    };
    actual == expected
}

#[test]
fn palette_file_mode_ranks_workspace_file_results() {
    let workspace = TempWorkspace::new();
    workspace.write("src/alpha_widget.rs", "fn alpha_widget() {}\n");
    workspace.write("docs/alpha-notes.md", "# Alpha\n");
    workspace.write("src/beta.rs", "fn beta() {}\n");
    let mut app = open_app(workspace.path(), None);

    app.dispatch_ui_intent(CommandDispatchIntent::OpenPalette {
        mode: PaletteMode::File,
        query: "alpha".to_string(),
        scope: SearchScopeProjection::ActiveFile,
    })
    .expect("palette open should dispatch");

    let palette = app
        .shell_projection_snapshot("palette")
        .expect("projection should build")
        .palette_projection;

    assert!(palette.open);
    assert_eq!(palette.mode, PaletteMode::File);
    assert_eq!(palette.query, "alpha");
    assert_eq!(palette.selected_index, 0);
    assert!(palette.results.len() >= 2);
    assert!(
        palette
            .results
            .iter()
            .all(|result| result.kind == PaletteResultKind::File)
    );
    assert!(palette.results[0].title.contains("alpha"));
    assert!(!palette.results[0].match_indices.is_empty());
}

#[test]
fn palette_file_mode_frecency_boosts_recently_focused_file() {
    let workspace = TempWorkspace::new();
    let first = workspace.write("src/alpha_widget.rs", "fn alpha_widget() {}\n");
    let second = workspace.write("src/beta_widget.rs", "fn beta_widget() {}\n");
    let mut app = open_app(workspace.path(), None);

    app.open_file(first.to_string_lossy())
        .expect("open first file");
    let first_buffer = app.active_buffer_id().expect("first buffer");
    app.open_file(second.to_string_lossy())
        .expect("open second file");
    let _second_buffer = app.active_buffer_id().expect("second buffer");
    app.dispatch_ui_intent(CommandDispatchIntent::SwitchTab {
        buffer_id: first_buffer,
    })
    .expect("switch back to first file");
    assert_eq!(app.active_buffer_id(), Some(first_buffer));

    app.dispatch_ui_intent(CommandDispatchIntent::OpenPalette {
        mode: PaletteMode::File,
        query: String::new(),
        scope: SearchScopeProjection::Workspace,
    })
    .expect("palette open should dispatch");

    let palette = app
        .shell_projection_snapshot("palette")
        .expect("projection should build")
        .palette_projection;

    assert_eq!(palette.mode, PaletteMode::File);
    assert_eq!(palette.results[0].kind, PaletteResultKind::File);
    assert!(projected_path_eq(
        palette.results[0].path.as_deref(),
        &first
    ));
    assert!(
        palette
            .results
            .iter()
            .any(|result| projected_path_eq(result.path.as_deref(), &second))
    );
}

#[test]
fn palette_symbol_mode_opens_symbol_location() {
    let workspace = TempWorkspace::new();
    let source = workspace.write("src/lib.rs", "fn alpha_widget() {}\nfn beta_widget() {}\n");
    let mut app = open_app(workspace.path(), Some(&source));
    let buffer_id = app.active_buffer_id().expect("source buffer");

    app.dispatch_ui_intent(CommandDispatchIntent::OpenPalette {
        mode: PaletteMode::Symbol,
        query: "alpha_widget".to_string(),
        scope: SearchScopeProjection::Workspace,
    })
    .expect("symbol palette should open");

    let palette = app
        .shell_projection_snapshot("palette")
        .expect("projection should build")
        .palette_projection;

    assert_eq!(palette.mode, PaletteMode::Symbol);
    assert_eq!(palette.results[0].kind, PaletteResultKind::Symbol);
    assert!(projected_path_eq(
        palette.results[0].path.as_deref(),
        &source
    ));
    assert!(palette.results[0].position.is_some());

    let outcome = app
        .dispatch_ui_intent(CommandDispatchIntent::DispatchPaletteSelection)
        .expect("symbol selection should dispatch");
    assert!(matches!(outcome, AppCommandOutcome::Opened(_)));
    assert_eq!(app.active_buffer_id(), Some(buffer_id));

    let projected = app
        .active_buffer_projection(&ShellLayoutProjection::plain("palette"))
        .expect("active projection after symbol jump");
    let viewport = projected.viewport.expect("viewport");
    assert_eq!(viewport.cursor.line, 0);
    assert_eq!(viewport.cursor.character, 3);
}

#[test]
fn palette_recent_buffers_mode_switches_to_recent_tab() {
    let workspace = TempWorkspace::new();
    let first = workspace.write("src/first.rs", "fn first() {}\n");
    let second = workspace.write("src/second.rs", "fn second() {}\n");
    let mut app = open_app(workspace.path(), None);

    app.open_file(first.to_string_lossy())
        .expect("open first file");
    let first_buffer = app.active_buffer_id().expect("first buffer");
    app.open_file(second.to_string_lossy())
        .expect("open second file");
    let _second_buffer = app.active_buffer_id().expect("second buffer");
    app.dispatch_ui_intent(CommandDispatchIntent::SwitchTab {
        buffer_id: first_buffer,
    })
    .expect("switch back to first file");

    app.dispatch_ui_intent(CommandDispatchIntent::OpenPalette {
        mode: PaletteMode::RecentBuffers,
        query: String::new(),
        scope: SearchScopeProjection::Workspace,
    })
    .expect("recent palette should open");

    let palette = app
        .shell_projection_snapshot("palette")
        .expect("projection should build")
        .palette_projection;

    assert_eq!(palette.mode, PaletteMode::RecentBuffers);
    assert_eq!(palette.results[0].kind, PaletteResultKind::RecentBuffers);
    assert!(projected_path_eq(
        palette.results[0].path.as_deref(),
        &first
    ));
    assert_eq!(palette.results[0].buffer_id, Some(first_buffer));

    let outcome = app
        .dispatch_ui_intent(CommandDispatchIntent::DispatchPaletteSelection)
        .expect("recent selection should dispatch");
    assert!(matches!(outcome, AppCommandOutcome::TabSwitched(buffer) if buffer == first_buffer));
    assert_eq!(app.active_buffer_id(), Some(first_buffer));
    assert!(
        palette
            .results
            .iter()
            .any(|result| result.buffer_id == Some(_second_buffer))
    );
}

#[test]
fn palette_selection_movement_is_clamped_to_projected_results() {
    let workspace = TempWorkspace::new();
    workspace.write("alpha.txt", "alpha\n");
    workspace.write("beta.txt", "beta\n");
    let mut app = open_app(workspace.path(), None);

    app.dispatch_ui_intent(CommandDispatchIntent::OpenPalette {
        mode: PaletteMode::File,
        query: String::new(),
        scope: SearchScopeProjection::ActiveFile,
    })
    .expect("palette open should dispatch");
    app.dispatch_ui_intent(CommandDispatchIntent::MovePaletteSelection { delta: 99 })
        .expect("palette movement should dispatch");
    let palette = app
        .shell_projection_snapshot("palette")
        .expect("projection should build")
        .palette_projection;
    assert_eq!(palette.selected_index, palette.results.len() - 1);

    app.dispatch_ui_intent(CommandDispatchIntent::MovePaletteSelection { delta: -99 })
        .expect("palette movement should dispatch");
    let palette = app
        .shell_projection_snapshot("palette")
        .expect("projection should build")
        .palette_projection;
    assert_eq!(palette.selected_index, 0);
}

#[test]
fn palette_dispatches_file_search_structural_and_command_results() {
    let workspace = TempWorkspace::new();
    let target = workspace.write("src/main.rs", "fn main() {\n    let needle = 1;\n}\n");
    let mut app = open_app(workspace.path(), Some(&target));

    app.dispatch_ui_intent(CommandDispatchIntent::OpenPalette {
        mode: PaletteMode::File,
        query: "main.rs".to_string(),
        scope: SearchScopeProjection::ActiveFile,
    })
    .expect("file palette should open");
    assert!(matches!(
        app.dispatch_ui_intent(CommandDispatchIntent::DispatchPaletteSelection)
            .expect("file selection should dispatch"),
        AppCommandOutcome::Opened(_)
    ));

    app.dispatch_ui_intent(CommandDispatchIntent::OpenPalette {
        mode: PaletteMode::Search,
        query: "/needle".to_string(),
        scope: SearchScopeProjection::ActiveFile,
    })
    .expect("search palette should open");
    let search = match app
        .dispatch_ui_intent(CommandDispatchIntent::DispatchPaletteSelection)
        .expect("search selection should dispatch")
    {
        AppCommandOutcome::SearchUpdated(projection) => projection,
        other => panic!("expected search update, got {other:?}"),
    };
    assert_eq!(search.status.kind, SearchStatusKindProjection::Completed);
    assert_eq!(search.query_label, "needle");

    app.dispatch_ui_intent(CommandDispatchIntent::OpenPalette {
        mode: PaletteMode::StructuralSearch,
        query: "#fn $NAME".to_string(),
        scope: SearchScopeProjection::ActiveFile,
    })
    .expect("structural palette should open");
    let structural = match app
        .dispatch_ui_intent(CommandDispatchIntent::DispatchPaletteSelection)
        .expect("structural selection should dispatch")
    {
        AppCommandOutcome::StructuralSearchUpdated(projection) => projection,
        other => panic!("expected structural search update, got {other:?}"),
    };
    assert_eq!(
        structural.status.kind,
        SearchStatusKindProjection::Completed
    );
    assert_eq!(structural.pattern_label, "fn $NAME");

    app.dispatch_ui_intent(CommandDispatchIntent::OpenPalette {
        mode: PaletteMode::Command,
        query: ">refresh explorer".to_string(),
        scope: SearchScopeProjection::ActiveFile,
    })
    .expect("command palette should open");
    assert!(matches!(
        app.dispatch_ui_intent(CommandDispatchIntent::DispatchPaletteSelection)
            .expect("command selection should dispatch"),
        AppCommandOutcome::ExplorerRefreshed(_)
    ));
}

#[test]
fn palette_command_mode_covers_registered_command_catalog() {
    enum ExpectedOutcome {
        Save,
        SaveAll,
        TabClosed,
        ExplorerRefreshed,
        GitUpdated,
        PaletteClosed,
        SettingsUpdated,
    }

    struct CommandCase {
        query: &'static str,
        expected_title: &'static str,
        expected_outcome: ExpectedOutcome,
        dirty_before_save: bool,
    }

    let cases = [
        CommandCase {
            query: ">save all",
            expected_title: "Save All",
            expected_outcome: ExpectedOutcome::SaveAll,
            dirty_before_save: false,
        },
        CommandCase {
            query: ">save active buffer",
            expected_title: "Save Active Buffer",
            expected_outcome: ExpectedOutcome::Save,
            dirty_before_save: true,
        },
        CommandCase {
            query: ">close active tab",
            expected_title: "Close Active Tab",
            expected_outcome: ExpectedOutcome::TabClosed,
            dirty_before_save: false,
        },
        CommandCase {
            query: ">reveal active file",
            expected_title: "Reveal Active File in Explorer",
            expected_outcome: ExpectedOutcome::ExplorerRefreshed,
            dirty_before_save: false,
        },
        CommandCase {
            query: ">refresh explorer",
            expected_title: "Refresh Explorer",
            expected_outcome: ExpectedOutcome::ExplorerRefreshed,
            dirty_before_save: false,
        },
        CommandCase {
            query: ">refresh git",
            expected_title: "Refresh Git",
            expected_outcome: ExpectedOutcome::GitUpdated,
            dirty_before_save: false,
        },
        CommandCase {
            query: ">close command palette",
            expected_title: "Close Command Palette",
            expected_outcome: ExpectedOutcome::PaletteClosed,
            dirty_before_save: false,
        },
        CommandCase {
            query: ">preferences open settings",
            expected_title: "Preferences: Open Settings",
            expected_outcome: ExpectedOutcome::SettingsUpdated,
            dirty_before_save: false,
        },
        CommandCase {
            query: ">preferences theme dark",
            expected_title: "Preferences: Theme Dark",
            expected_outcome: ExpectedOutcome::SettingsUpdated,
            dirty_before_save: false,
        },
        CommandCase {
            query: ">preferences theme light",
            expected_title: "Preferences: Theme Light",
            expected_outcome: ExpectedOutcome::SettingsUpdated,
            dirty_before_save: false,
        },
        CommandCase {
            query: ">preferences theme system",
            expected_title: "Preferences: Theme System",
            expected_outcome: ExpectedOutcome::SettingsUpdated,
            dirty_before_save: false,
        },
        CommandCase {
            query: ">preferences reset zoom",
            expected_title: "Preferences: Reset Zoom",
            expected_outcome: ExpectedOutcome::SettingsUpdated,
            dirty_before_save: false,
        },
        CommandCase {
            query: ">preferences reset settings",
            expected_title: "Preferences: Reset Settings",
            expected_outcome: ExpectedOutcome::SettingsUpdated,
            dirty_before_save: false,
        },
    ];

    let workspace = TempWorkspace::new();
    let source = workspace.write("src/main.rs", "fn main() {}\n");
    let mut resolved_cases = 0;

    for case in &cases {
        let mut app = open_app(workspace.path(), Some(&source));
        let initial_buffer_id = app.active_buffer_id().expect("active buffer");
        if case.dirty_before_save {
            app.dispatch_ui_intent(CommandDispatchIntent::Insert {
                buffer_id: initial_buffer_id,
                at: TextCoordinate {
                    line: 0,
                    character: 0,
                    byte_offset: None,
                    utf16_offset: None,
                },
                text: "// dirty\n".to_string(),
            })
            .expect("dirty insert should dispatch");
        }

        app.dispatch_ui_intent(CommandDispatchIntent::OpenPalette {
            mode: PaletteMode::Command,
            query: case.query.to_string(),
            scope: SearchScopeProjection::ActiveFile,
        })
        .expect("command palette should open");

        let palette = app
            .shell_projection_snapshot("palette")
            .expect("projection should build")
            .palette_projection;

        assert_eq!(palette.results[0].title, case.expected_title);
        assert_eq!(palette.results[0].kind, PaletteResultKind::Command);

        let outcome = app
            .dispatch_ui_intent(CommandDispatchIntent::DispatchPaletteSelection)
            .expect("command selection should dispatch");

        match &case.expected_outcome {
            ExpectedOutcome::Save => assert!(matches!(outcome, AppCommandOutcome::Save(_))),
            ExpectedOutcome::SaveAll => {
                assert!(matches!(outcome, AppCommandOutcome::SaveAll(_)))
            }
            ExpectedOutcome::TabClosed => {
                assert!(matches!(outcome, AppCommandOutcome::TabClose(_)))
            }
            ExpectedOutcome::ExplorerRefreshed => {
                assert!(matches!(outcome, AppCommandOutcome::ExplorerRefreshed(_)))
            }
            ExpectedOutcome::GitUpdated => {
                assert!(matches!(outcome, AppCommandOutcome::GitUpdated(_)))
            }
            ExpectedOutcome::PaletteClosed => match outcome {
                AppCommandOutcome::PaletteUpdated(projection) => {
                    assert!(!projection.open);
                }
                other => panic!("expected palette update, got {other:?}"),
            },
            ExpectedOutcome::SettingsUpdated => {
                assert!(matches!(outcome, AppCommandOutcome::SettingsUpdated(_)))
            }
        }

        resolved_cases += 1;
    }

    // Every listed case must resolve. The denominator is the actual case count, not a
    // magic number decoupled from the table.
    let coverage_percent = (resolved_cases as f32 / cases.len() as f32) * 100.0;
    assert!(
        (coverage_percent - 100.0).abs() < f32::EPSILON,
        "command coverage report: {resolved_cases}/{} cases resolved ({coverage_percent:.1}%)",
        cases.len()
    );

    // Guard against catalog drift: derive the registered command catalog from a live palette
    // projection and assert every registered command is either exercised above or explicitly
    // allowlisted (git mutations need a real repository / query argument and are covered by the
    // git_workflow integration tests). A new command added without a case or allowlist entry
    // fails here with a catalog-vs-cases diff.
    let mut catalog_app = open_app(workspace.path(), Some(&source));
    catalog_app
        .dispatch_ui_intent(CommandDispatchIntent::OpenPalette {
            mode: PaletteMode::Command,
            query: ">".to_string(),
            scope: SearchScopeProjection::ActiveFile,
        })
        .expect("command palette should open for catalog enumeration");
    let catalog_titles: std::collections::BTreeSet<String> = catalog_app
        .shell_projection_snapshot("palette")
        .expect("projection should build")
        .palette_projection
        .results
        .iter()
        .filter(|result| result.kind == PaletteResultKind::Command)
        .map(|result| result.title.clone())
        .collect();
    assert!(
        !catalog_titles.is_empty(),
        "registered command catalog should not be empty"
    );

    let case_titles: std::collections::BTreeSet<String> = cases
        .iter()
        .map(|case| case.expected_title.to_string())
        .collect();

    let allowlisted: std::collections::BTreeSet<String> = [
        "Git: Switch Branch",
        "Git: Create Branch",
        "Git: Delete Branch",
        "Git: Stash Changes",
        "Git: Prune Worktrees",
        "Git: Remove Worktree",
        "Git: Commit Staged Changes",
        // These commands require an open git workspace; covered by worktree/local-history tests.
        "Git: Export Worktree Evidence",
        "Git: Local History",
        "Git: New Worktree",
    ]
    .into_iter()
    .map(str::to_string)
    .collect();

    let stale_cases: Vec<&String> = case_titles.difference(&catalog_titles).collect();
    assert!(
        stale_cases.is_empty(),
        "test cases reference commands missing from the catalog (stale/renamed): {stale_cases:?}"
    );

    let covered: std::collections::BTreeSet<String> =
        case_titles.union(&allowlisted).cloned().collect();
    let uncovered: Vec<&String> = catalog_titles.difference(&covered).collect();
    assert!(
        uncovered.is_empty(),
        "registered commands missing a test case (add a CommandCase or allowlist entry): \
         {uncovered:?}"
    );
}
