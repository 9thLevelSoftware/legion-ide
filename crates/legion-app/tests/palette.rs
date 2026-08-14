use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use legion_app::{AppCommandOutcome, AppComposition};
use legion_protocol::{PrincipalId, TextCoordinate, WorkspaceTrustState};
use legion_ui::{
    CommandDispatchIntent, PaletteConfirmationProjection, PaletteMode, PaletteResultKind,
    SearchScopeProjection, SearchStatusKindProjection, ShellLayoutProjection,
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

fn run_git(root: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .current_dir(root)
        .args(args)
        .output()
        .expect("git command should run");
    assert!(
        output.status.success(),
        "git {args:?} failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).into_owned()
}

#[test]
fn palette_direct_destructive_dispatch_waits_for_app_owned_confirmation() {
    let workspace = TempWorkspace::new();
    let mut app = open_app(workspace.path(), None);
    app.dispatch_ui_intent(CommandDispatchIntent::SetZoomPercent { zoom_percent: 135 })
        .expect("custom zoom should be applied before reset is requested");
    app.dispatch_ui_intent(CommandDispatchIntent::OpenPalette {
        mode: PaletteMode::Command,
        query: ">preferences reset settings".to_string(),
        scope: SearchScopeProjection::Workspace,
    })
    .expect("reset command should be searchable");

    let outcome = app
        .dispatch_ui_intent(CommandDispatchIntent::DispatchPaletteSelection)
        .expect("initial destructive dispatch should request confirmation");

    let AppCommandOutcome::PaletteUpdated(palette) = outcome else {
        panic!("destructive dispatch must remain in the palette until confirmed");
    };
    assert!(
        palette.open,
        "the palette must remain open for confirmation"
    );
    let pending = palette
        .pending_confirmation
        .expect("the app must project the pending confirmation");
    assert_eq!(pending.command_id, "command:preferences-settings-reset");
    assert!(pending.operands.is_empty());
    let settings = app
        .shell_projection_snapshot("palette")
        .expect("settings should remain projectable")
        .settings_projection;
    assert_eq!(
        settings.zoom_percent, 135,
        "a direct dispatch must not bypass app-owned confirmation"
    );
}

fn request_reset_confirmation(app: &mut AppComposition) -> PaletteConfirmationProjection {
    app.dispatch_ui_intent(CommandDispatchIntent::OpenPalette {
        mode: PaletteMode::Command,
        query: ">preferences reset settings".to_string(),
        scope: SearchScopeProjection::Workspace,
    })
    .expect("reset command should be searchable");
    let outcome = app
        .dispatch_ui_intent(CommandDispatchIntent::DispatchPaletteSelection)
        .expect("reset dispatch should request confirmation");
    let AppCommandOutcome::PaletteUpdated(palette) = outcome else {
        panic!("reset dispatch must not execute before confirmation");
    };
    palette
        .pending_confirmation
        .expect("app should project a pending confirmation")
}

#[test]
fn palette_stale_confirmation_token_cannot_mutate_settings() {
    let workspace = TempWorkspace::new();
    let mut app = open_app(workspace.path(), None);
    app.dispatch_ui_intent(CommandDispatchIntent::SetZoomPercent { zoom_percent: 135 })
        .expect("custom zoom should be applied");
    let pending = request_reset_confirmation(&mut app);
    app.dispatch_ui_intent(CommandDispatchIntent::UpdatePaletteQuery {
        query: ">save all".to_string(),
    })
    .expect("changing the query should invalidate the confirmation");

    app.dispatch_ui_intent(CommandDispatchIntent::ConfirmPaletteSelection {
        token: pending.token,
        command_id: pending.command_id,
        operands: pending.operands,
    })
    .expect("stale confirmation should fail closed");

    let snapshot = app
        .shell_projection_snapshot("stale confirmation")
        .expect("projection should build");
    assert_eq!(snapshot.settings_projection.zoom_percent, 135);
    assert!(snapshot.palette_projection.pending_confirmation.is_none());
}

#[test]
fn palette_mismatched_command_or_operands_cannot_mutate_settings() {
    let workspace = TempWorkspace::new();
    let mut app = open_app(workspace.path(), None);
    app.dispatch_ui_intent(CommandDispatchIntent::SetZoomPercent { zoom_percent: 135 })
        .expect("custom zoom should be applied");
    let pending = request_reset_confirmation(&mut app);

    for (command_id, operands) in [
        ("command:git-delete-branch".to_string(), Vec::new()),
        (pending.command_id.clone(), vec!["unexpected".to_string()]),
    ] {
        app.dispatch_ui_intent(CommandDispatchIntent::ConfirmPaletteSelection {
            token: pending.token,
            command_id,
            operands,
        })
        .expect("mismatched confirmation should fail closed");
        let snapshot = app
            .shell_projection_snapshot("mismatched confirmation")
            .expect("projection should build");
        assert_eq!(snapshot.settings_projection.zoom_percent, 135);
        assert_eq!(
            snapshot.palette_projection.pending_confirmation.as_ref(),
            Some(&pending),
            "a mismatched request must not consume the valid pending confirmation"
        );
    }
}

#[test]
fn palette_confirmation_cancellation_clears_pending_without_mutation() {
    let workspace = TempWorkspace::new();
    let mut app = open_app(workspace.path(), None);
    app.dispatch_ui_intent(CommandDispatchIntent::SetZoomPercent { zoom_percent: 135 })
        .expect("custom zoom should be applied");
    let pending = request_reset_confirmation(&mut app);

    app.dispatch_ui_intent(CommandDispatchIntent::CancelPaletteConfirmation {
        token: pending.token,
    })
    .expect("cancellation should dispatch");

    let snapshot = app
        .shell_projection_snapshot("cancelled confirmation")
        .expect("projection should build");
    assert_eq!(snapshot.settings_projection.zoom_percent, 135);
    assert!(snapshot.palette_projection.pending_confirmation.is_none());
}

#[test]
fn palette_matching_confirmation_executes_once_and_consumes_token() {
    let workspace = TempWorkspace::new();
    let mut app = open_app(workspace.path(), None);
    app.dispatch_ui_intent(CommandDispatchIntent::SetZoomPercent { zoom_percent: 135 })
        .expect("custom zoom should be applied");
    let pending = request_reset_confirmation(&mut app);
    let confirmation = CommandDispatchIntent::ConfirmPaletteSelection {
        token: pending.token,
        command_id: pending.command_id,
        operands: pending.operands,
    };

    let outcome = app
        .dispatch_ui_intent(confirmation.clone())
        .expect("matching confirmation should execute");
    assert!(matches!(outcome, AppCommandOutcome::SettingsUpdated(_)));
    let snapshot = app
        .shell_projection_snapshot("confirmed reset")
        .expect("projection should build");
    assert_eq!(snapshot.settings_projection.zoom_percent, 100);
    assert!(!snapshot.palette_projection.open);
    assert!(snapshot.palette_projection.pending_confirmation.is_none());

    app.dispatch_ui_intent(CommandDispatchIntent::SetZoomPercent { zoom_percent: 140 })
        .expect("zoom should remain mutable after reset");
    app.dispatch_ui_intent(confirmation)
        .expect("replayed confirmation should fail closed");
    let snapshot = app
        .shell_projection_snapshot("replayed confirmation")
        .expect("projection should build");
    assert_eq!(snapshot.settings_projection.zoom_percent, 140);
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
fn command_palette_ranks_available_commands_before_unavailable_commands() {
    let workspace = TempWorkspace::new();
    let mut app = open_app(workspace.path(), None);

    app.dispatch_ui_intent(CommandDispatchIntent::OpenPalette {
        mode: PaletteMode::Command,
        query: ">save".to_string(),
        scope: SearchScopeProjection::Workspace,
    })
    .expect("command palette should open without an active tab");

    let palette = app
        .shell_projection_snapshot("palette")
        .expect("projection should build")
        .palette_projection;
    let save_all = palette
        .results
        .iter()
        .position(|result| result.title == "Save All")
        .expect("available Save All command should remain searchable");
    let save_active = palette
        .results
        .iter()
        .position(|result| result.title == "Save Active Buffer")
        .expect("unavailable Save Active Buffer command should remain searchable");

    assert!(palette.results[save_all].disabled_reason.is_none());
    assert!(palette.results[save_active].disabled_reason.is_some());
    assert!(
        save_all < save_active,
        "available commands must rank before unavailable commands: {:?}",
        palette.results
    );
    assert_eq!(palette.selected_index, save_all);
}

#[test]
fn command_palette_never_selects_or_dispatches_an_unavailable_only_result() {
    let workspace = TempWorkspace::new();
    let mut app = open_app(workspace.path(), None);

    app.dispatch_ui_intent(CommandDispatchIntent::OpenPalette {
        mode: PaletteMode::Command,
        query: ">save active buffer".to_string(),
        scope: SearchScopeProjection::Workspace,
    })
    .expect("unavailable commands should remain searchable");
    let palette = app
        .shell_projection_snapshot("palette")
        .expect("projection should build")
        .palette_projection;
    assert_eq!(palette.results.len(), 1);
    assert_eq!(
        palette.results[0].disabled_reason.as_deref(),
        Some("Open a tab first")
    );
    assert_eq!(
        palette.selected_index,
        palette.results.len(),
        "an unavailable-only result set must have no default selection"
    );

    app.dispatch_ui_intent(CommandDispatchIntent::MovePaletteSelection { delta: 1 })
        .expect("selection movement should remain a no-op");
    let outcome = app
        .dispatch_ui_intent(CommandDispatchIntent::DispatchPaletteSelection)
        .expect("dispatching no available selection should be a no-op");
    let AppCommandOutcome::PaletteUpdated(palette) = outcome else {
        panic!("unavailable dispatch must leave the palette open");
    };
    assert!(palette.open);
    assert_eq!(palette.selected_index, palette.results.len());
}

#[test]
fn argument_dependent_git_commands_require_explicit_operands() {
    let workspace = TempWorkspace::new();
    let mut app = open_app(workspace.path(), None);

    for (query, title, reason) in [
        (
            ">git switch branch",
            "Git: Switch Branch",
            "Enter a branch name",
        ),
        (
            ">git create branch",
            "Git: Create Branch",
            "Enter a branch name",
        ),
        (
            ">git delete branch",
            "Git: Delete Branch",
            "Enter a branch name",
        ),
        (
            ">git remove worktree",
            "Git: Remove Worktree",
            "Enter a worktree path",
        ),
        (
            ">git new worktree",
            "Git: New Worktree",
            "Enter a branch and worktree path",
        ),
        (
            ">git commit",
            "Git: Commit Staged Changes",
            "Enter a commit message",
        ),
    ] {
        app.dispatch_ui_intent(CommandDispatchIntent::OpenPalette {
            mode: PaletteMode::Command,
            query: query.to_string(),
            scope: SearchScopeProjection::Workspace,
        })
        .expect("command palette should open");

        let palette = app
            .shell_projection_snapshot("git operands")
            .expect("projection should build")
            .palette_projection;
        let result = palette
            .results
            .iter()
            .find(|result| result.title == title)
            .unwrap_or_else(|| panic!("{title} should remain searchable for `{query}`"));
        assert_eq!(result.disabled_reason.as_deref(), Some(reason), "{query}");
        assert_ne!(palette.selected_index, 0, "{query} must not be selected");
    }
}

#[test]
fn palette_git_stash_allows_omitting_the_optional_message() {
    let workspace = TempWorkspace::new();
    let source = workspace.write("src/lib.rs", "pub fn value() -> u8 { 1 }\n");
    run_git(workspace.path(), &["init"]);
    run_git(
        workspace.path(),
        &["config", "user.email", "palette@example.test"],
    );
    run_git(workspace.path(), &["config", "user.name", "Palette Test"]);
    run_git(workspace.path(), &["add", "."]);
    run_git(workspace.path(), &["commit", "-m", "initial"]);
    let mut app = open_app(workspace.path(), Some(&source));

    workspace.write("src/lib.rs", "pub fn value() -> u8 { 2 }\n");
    app.dispatch_ui_intent(CommandDispatchIntent::OpenPalette {
        mode: PaletteMode::Command,
        query: ">git stash".to_string(),
        scope: SearchScopeProjection::Workspace,
    })
    .expect("stash command should open");

    let palette = app
        .shell_projection_snapshot("optional stash message")
        .expect("projection should build")
        .palette_projection;
    let stash = palette
        .results
        .iter()
        .find(|result| result.title == "Git: Stash Changes")
        .expect("stash command should remain searchable");
    assert_eq!(stash.disabled_reason, None);
    assert_eq!(
        palette
            .results
            .get(palette.selected_index)
            .map(|result| &result.id),
        Some(&stash.id),
        "bare git stash should be the available selection"
    );
    assert!(matches!(
        app.dispatch_ui_intent(CommandDispatchIntent::DispatchPaletteSelection)
            .expect("stash command should dispatch"),
        AppCommandOutcome::GitUpdated(_)
    ));

    let stash_subject = run_git(workspace.path(), &["stash", "list", "--format=%gs"]);
    assert!(
        stash_subject.trim_start().starts_with("WIP on "),
        "omitting the message should use Git's default stash subject: {stash_subject:?}"
    );
}

#[test]
fn argument_dependent_git_commands_project_only_resolved_operands() {
    let workspace = TempWorkspace::new();
    let mut app = open_app(workspace.path(), None);

    for (query, title, detail) in [
        (
            ">git switch branch feature/palette",
            "Git: Switch Branch",
            "Switch to branch ‘feature/palette’",
        ),
        (
            ">git create branch feature/new",
            "Git: Create Branch",
            "Create and switch to branch ‘feature/new’",
        ),
        (
            ">git delete branch feature/old",
            "Git: Delete Branch",
            "Delete branch ‘feature/old’",
        ),
        (
            ">git remove worktree worktrees/old copy",
            "Git: Remove Worktree",
            "Remove worktree ‘worktrees/old copy’",
        ),
        (
            ">git new worktree feature/wt worktrees/new copy",
            "Git: New Worktree",
            "Create worktree ‘worktrees/new copy’ from branch ‘feature/wt’",
        ),
        (
            ">git commit fix(parser): preserve café exactly",
            "Git: Commit Staged Changes",
            "Commit staged changes as ‘fix(parser): preserve café exactly’",
        ),
        (
            ">git stash WIP: preserve → exactly",
            "Git: Stash Changes",
            "Stash changes as ‘WIP: preserve → exactly’",
        ),
        (
            ">Git: Switch Branch feature/title-switch",
            "Git: Switch Branch",
            "Switch to branch ‘feature/title-switch’",
        ),
        (
            ">Git: Create Branch feature/title-create",
            "Git: Create Branch",
            "Create and switch to branch ‘feature/title-create’",
        ),
        (
            ">Git: Delete Branch feature/title-delete",
            "Git: Delete Branch",
            "Delete branch ‘feature/title-delete’",
        ),
        (
            ">Git: Remove Worktree worktrees/title copy",
            "Git: Remove Worktree",
            "Remove worktree ‘worktrees/title copy’",
        ),
        (
            ">Git: New Worktree feature/title-wt worktrees/title new",
            "Git: New Worktree",
            "Create worktree ‘worktrees/title new’ from branch ‘feature/title-wt’",
        ),
        (
            ">Git: Commit Staged Changes fix: title spelling",
            "Git: Commit Staged Changes",
            "Commit staged changes as ‘fix: title spelling’",
        ),
        (
            ">Git: Stash Changes WIP title spelling",
            "Git: Stash Changes",
            "Stash changes as ‘WIP title spelling’",
        ),
    ] {
        app.dispatch_ui_intent(CommandDispatchIntent::OpenPalette {
            mode: PaletteMode::Command,
            query: query.to_string(),
            scope: SearchScopeProjection::Workspace,
        })
        .expect("command palette should open");

        let palette = app
            .shell_projection_snapshot("git operands")
            .expect("projection should build")
            .palette_projection;
        let result = palette
            .results
            .iter()
            .find(|result| result.title == title)
            .unwrap_or_else(|| panic!("{title} should match `{query}`"));
        assert_eq!(result.disabled_reason, None, "{query}");
        assert_eq!(result.detail.as_deref(), Some(detail), "{query}");
        assert_eq!(
            palette
                .results
                .get(palette.selected_index)
                .map(|row| &row.id),
            Some(&result.id),
            "the resolved command should be the available selection"
        );
    }
}

#[test]
fn palette_git_commit_and_stash_use_only_exact_parsed_messages() {
    let workspace = TempWorkspace::new();
    let source = workspace.write("src/lib.rs", "pub fn value() -> u8 { 1 }\n");
    run_git(workspace.path(), &["init"]);
    run_git(
        workspace.path(),
        &["config", "user.email", "palette@example.test"],
    );
    run_git(workspace.path(), &["config", "user.name", "Palette Test"]);
    run_git(workspace.path(), &["add", "."]);
    run_git(workspace.path(), &["commit", "-m", "initial"]);
    let mut app = open_app(workspace.path(), Some(&source));

    workspace.write("src/lib.rs", "pub fn value() -> u8 { 2 }\n");
    run_git(workspace.path(), &["add", "src/lib.rs"]);
    app.dispatch_ui_intent(CommandDispatchIntent::OpenPalette {
        mode: PaletteMode::Command,
        query: ">git commit fix(parser): preserve café exactly".to_string(),
        scope: SearchScopeProjection::Workspace,
    })
    .expect("commit command should open");
    assert!(matches!(
        app.dispatch_ui_intent(CommandDispatchIntent::DispatchPaletteSelection)
            .expect("commit command should dispatch"),
        AppCommandOutcome::GitUpdated(_)
    ));
    assert_eq!(
        run_git(workspace.path(), &["log", "-1", "--pretty=%s"]).trim(),
        "fix(parser): preserve café exactly"
    );

    workspace.write("src/lib.rs", "pub fn value() -> u8 { 3 }\n");
    app.dispatch_ui_intent(CommandDispatchIntent::OpenPalette {
        mode: PaletteMode::Command,
        query: ">git stash WIP: preserve → exactly".to_string(),
        scope: SearchScopeProjection::Workspace,
    })
    .expect("stash command should open");
    assert!(matches!(
        app.dispatch_ui_intent(CommandDispatchIntent::DispatchPaletteSelection)
            .expect("stash command should dispatch"),
        AppCommandOutcome::GitUpdated(_)
    ));
    let stash_subject = run_git(workspace.path(), &["stash", "list", "--format=%gs"]);
    assert!(
        stash_subject
            .trim_end()
            .ends_with("WIP: preserve → exactly"),
        "stash subject must contain only the parsed message: {stash_subject:?}"
    );
    assert!(!stash_subject.contains("git stash"));
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
        PendingConfirmation,
        /// Command that returns `AppCommandOutcome::Noop` (e.g. LSP lifecycle).
        Noop,
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
            expected_outcome: ExpectedOutcome::PendingConfirmation,
            dirty_before_save: false,
        },
        // PKT-LSP-C T1: lazy session start / restart palette commands.
        CommandCase {
            query: ">language server start",
            expected_title: "Language Server: Start",
            expected_outcome: ExpectedOutcome::Noop,
            dirty_before_save: false,
        },
        CommandCase {
            query: ">language server restart",
            expected_title: "Language Server: Restart",
            expected_outcome: ExpectedOutcome::Noop,
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
            ExpectedOutcome::PendingConfirmation => match outcome {
                AppCommandOutcome::PaletteUpdated(projection) => {
                    assert!(projection.open);
                    assert!(projection.pending_confirmation.is_some());
                }
                other => panic!("expected pending confirmation, got {other:?}"),
            },
            ExpectedOutcome::Noop => {
                assert!(
                    matches!(outcome, AppCommandOutcome::Noop),
                    "expected Noop outcome, got {outcome:?}"
                )
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
