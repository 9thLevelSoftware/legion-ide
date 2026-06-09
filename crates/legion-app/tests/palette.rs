use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use legion_app::{AppCommandOutcome, AppComposition};
use legion_protocol::PrincipalId;
use legion_protocol::WorkspaceTrustState;
use legion_ui::{
    CommandDispatchIntent, PaletteMode, PaletteResultKind, SearchScopeProjection,
    SearchStatusKindProjection,
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
