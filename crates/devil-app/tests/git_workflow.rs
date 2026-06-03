use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use devil_app::{AppCommandOutcome, AppComposition};
use devil_editor::{TextEdit, TextPosition};
use devil_ui::{
    CommandDispatchIntent, GitConflictChoiceProjection, GitDiffStrategyProjection,
    GitHunkStageProjection, SearchStatusKindProjection,
};

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

struct TempGitRepo {
    root: PathBuf,
}

impl TempGitRepo {
    fn new() -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after epoch")
            .as_nanos();
        let id = TEMP_COUNTER.fetch_add(1, Ordering::SeqCst);
        let root = std::env::temp_dir().join(format!(
            "devil_app_git_{}_{}_{}",
            std::process::id(),
            nanos,
            id
        ));
        fs::create_dir(&root).expect("temp git repo should be created");
        run_git(&root, ["init"]);
        run_git(&root, ["branch", "-M", "master"]);
        run_git(&root, ["config", "user.email", "devil@example.test"]);
        run_git(&root, ["config", "user.name", "Legion Test"]);
        Self { root }
    }

    fn path(&self) -> &Path {
        &self.root
    }

    fn write(&self, relative: &str, content: &str) -> PathBuf {
        let path = self.root.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("parent directory should be created");
        }
        fs::write(&path, content).expect("file should be written");
        path
    }
}

impl Drop for TempGitRepo {
    fn drop(&mut self) {
        let temp_root = std::env::temp_dir();
        let file_name = self.root.file_name().and_then(|name| name.to_str());
        if self.root.starts_with(&temp_root)
            && file_name.is_some_and(|name| name.starts_with("devil_app_git_"))
        {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
}

fn run_git<const N: usize>(root: &Path, args: [&str; N]) -> String {
    let output = Command::new("git")
        .current_dir(root)
        .args(args)
        .output()
        .expect("git command should run");
    assert!(
        output.status.success(),
        "git {:?} failed\nstdout:\n{}\nstderr:\n{}",
        args,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).into_owned()
}

#[test]
fn git_workflow_refreshes_projection_and_stages_hunks_through_app_authority() {
    let repo = TempGitRepo::new();
    let source = repo.write(
        "src/lib.rs",
        "pub fn alpha() {\n    first();\n}\n\n\n\npub fn beta() {\n    second();\n}\n",
    );
    run_git(repo.path(), ["add", "."]);
    run_git(repo.path(), ["commit", "-m", "initial"]);
    repo.write(
        "src/lib.rs",
        "pub fn alpha() {\n    first_changed();\n}\n\n\n\npub fn beta() {\n    second_changed();\n}\n",
    );

    let mut app = AppComposition::new();
    app.open_workspace(
        repo.path(),
        devil_protocol::WorkspaceTrustState::Trusted,
        devil_protocol::PrincipalId("git-test".to_string()),
    )
    .expect("workspace should open");
    app.open_file(source.to_string_lossy())
        .expect("source should open");

    let projection = match app
        .dispatch_ui_intent(CommandDispatchIntent::RefreshGit)
        .expect("git refresh should dispatch")
    {
        AppCommandOutcome::GitUpdated(projection) => projection,
        other => panic!("expected git projection, got {other:?}"),
    };

    assert_eq!(projection.changed_files.len(), 1);
    assert_eq!(
        projection.changed_files[0].diff_strategy,
        GitDiffStrategyProjection::Syntactic
    );
    assert_eq!(projection.changed_files[0].unstaged_hunk_count, 2);
    assert!(
        projection
            .blame_lines
            .iter()
            .any(|line| line.author == "Legion Test")
    );
    assert!(
        projection
            .commits
            .iter()
            .any(|commit| commit.summary == "initial")
    );
    assert_eq!(
        app.shell_projection_snapshot("git")
            .expect("snapshot")
            .git_projection,
        projection
    );

    let hunk_id = projection
        .hunks
        .iter()
        .find(|hunk| hunk.stage == GitHunkStageProjection::Unstaged)
        .expect("unstaged hunk should exist")
        .hunk_id
        .clone();
    let staged = match app
        .dispatch_ui_intent(CommandDispatchIntent::StageGitHunk {
            hunk_id: hunk_id.clone(),
        })
        .expect("stage hunk should dispatch")
    {
        AppCommandOutcome::GitUpdated(projection) => projection,
        other => panic!("expected git projection, got {other:?}"),
    };
    assert!(
        staged
            .hunks
            .iter()
            .any(|hunk| hunk.stage == GitHunkStageProjection::Staged)
    );
    let cached = run_git(repo.path(), ["diff", "--cached", "--", "src/lib.rs"]);
    assert!(cached.contains("first_changed"));
    assert!(!cached.contains("second_changed"));

    let staged_hunk_id = staged
        .hunks
        .iter()
        .find(|hunk| hunk.stage == GitHunkStageProjection::Staged)
        .expect("staged hunk should exist")
        .hunk_id
        .clone();
    let unstaged = match app
        .dispatch_ui_intent(CommandDispatchIntent::UnstageGitHunk {
            hunk_id: staged_hunk_id,
        })
        .expect("unstage hunk should dispatch")
    {
        AppCommandOutcome::GitUpdated(projection) => projection,
        other => panic!("expected git projection, got {other:?}"),
    };
    assert!(
        unstaged
            .hunks
            .iter()
            .all(|hunk| hunk.stage == GitHunkStageProjection::Unstaged)
    );
    assert!(
        run_git(repo.path(), ["diff", "--cached", "--", "src/lib.rs"])
            .trim()
            .is_empty()
    );
    assert_eq!(
        app.shell_projection_snapshot("git")
            .expect("snapshot")
            .search_projection
            .status
            .kind,
        SearchStatusKindProjection::Idle
    );
}

#[test]
fn git_workflow_resolves_conflicts_through_app_authority() {
    let repo = TempGitRepo::new();
    let source = repo.write(
        "src/lib.rs",
        "pub fn alpha() {\n    original();\n}\n\npub fn beta() {\n    original_beta();\n}\n",
    );
    run_git(repo.path(), ["add", "."]);
    run_git(repo.path(), ["commit", "-m", "initial"]);

    run_git(repo.path(), ["checkout", "-b", "feature"]);
    repo.write(
        "src/lib.rs",
        "pub fn alpha() {\n    incoming_alpha();\n}\n\npub fn beta() {\n    incoming_beta();\n}\n",
    );
    run_git(repo.path(), ["add", "."]);
    run_git(repo.path(), ["commit", "-m", "feature"]);

    run_git(repo.path(), ["checkout", "master"]);
    repo.write(
        "src/lib.rs",
        "pub fn alpha() {\n    current_alpha();\n}\n\npub fn beta() {\n    current_beta();\n}\n",
    );
    run_git(repo.path(), ["add", "."]);
    run_git(repo.path(), ["commit", "-m", "master"]);

    let _ = Command::new("git")
        .current_dir(repo.path())
        .args(["merge", "feature"])
        .output()
        .expect("git merge command should run");

    let mut app = AppComposition::new();
    app.open_workspace(
        repo.path(),
        devil_protocol::WorkspaceTrustState::Trusted,
        devil_protocol::PrincipalId("git-conflict-test".to_string()),
    )
    .expect("workspace should open");
    app.open_file(source.to_string_lossy())
        .expect("source should open");

    let projection = match app
        .dispatch_ui_intent(CommandDispatchIntent::RefreshGit)
        .expect("git refresh should dispatch")
    {
        AppCommandOutcome::GitUpdated(projection) => projection,
        other => panic!("expected git projection, got {other:?}"),
    };

    assert!(
        !projection.conflicts.is_empty(),
        "conflicts should be present after merge"
    );
    let conflict = projection
        .conflicts
        .iter()
        .find(|c| c.path == "src/lib.rs")
        .expect("src/lib.rs conflict should exist");
    assert!(
        conflict.actions.iter().any(|a| a == "accept_current"),
        "accept_current action should be present"
    );
    assert!(
        conflict.actions.iter().any(|a| a == "accept_incoming"),
        "accept_incoming action should be present"
    );

    let resolved = match app
        .dispatch_ui_intent(CommandDispatchIntent::ResolveGitConflict {
            path: "src/lib.rs".to_string(),
            choice: GitConflictChoiceProjection::AcceptCurrent,
        })
        .expect("resolve conflict should dispatch")
    {
        AppCommandOutcome::GitUpdated(projection) => projection,
        other => panic!("expected git projection, got {other:?}"),
    };

    assert!(
        !resolved.conflicts.iter().any(|c| c.path == "src/lib.rs"),
        "src/lib.rs conflict should be resolved"
    );

    let content = fs::read_to_string(&source).expect("file should be readable");
    assert!(
        content.contains("current_alpha"),
        "resolved content should contain current_alpha"
    );
    assert!(
        content.contains("current_beta"),
        "resolved content should contain current_beta"
    );
    assert!(!content.contains("<<<<<<<"), "markers should be removed");
    assert!(!content.contains("======="), "markers should be removed");
    assert!(!content.contains(">>>>>>>"), "markers should be removed");

    let unmerged = run_git(repo.path(), ["diff", "--name-only", "--diff-filter=U"]);
    assert!(
        !unmerged.contains("src/lib.rs"),
        "src/lib.rs should no longer be in unmerged state after resolution"
    );
}

#[test]
fn git_workflow_resolves_conflict_and_syncs_open_buffer() {
    let repo = TempGitRepo::new();
    let source = repo.write(
        "src/lib.rs",
        "pub fn alpha() {\n    original();\n}\n\npub fn beta() {\n    original_beta();\n}\n",
    );
    run_git(repo.path(), ["add", "."]);
    run_git(repo.path(), ["commit", "-m", "initial"]);

    run_git(repo.path(), ["checkout", "-b", "feature"]);
    repo.write(
        "src/lib.rs",
        "pub fn alpha() {\n    incoming_alpha();\n}\n\npub fn beta() {\n    incoming_beta();\n}\n",
    );
    run_git(repo.path(), ["add", "."]);
    run_git(repo.path(), ["commit", "-m", "feature"]);

    run_git(repo.path(), ["checkout", "master"]);
    repo.write(
        "src/lib.rs",
        "pub fn alpha() {\n    current_alpha();\n}\n\npub fn beta() {\n    current_beta();\n}\n",
    );
    run_git(repo.path(), ["add", "."]);
    run_git(repo.path(), ["commit", "-m", "master"]);

    let _ = Command::new("git")
        .current_dir(repo.path())
        .args(["merge", "feature"])
        .output()
        .expect("git merge command should run");

    let mut app = AppComposition::new();
    app.open_workspace(
        repo.path(),
        devil_protocol::WorkspaceTrustState::Trusted,
        devil_protocol::PrincipalId("git-conflict-test".to_string()),
    )
    .expect("workspace should open");
    app.open_file(source.to_string_lossy())
        .expect("source should open");

    let projection = match app
        .dispatch_ui_intent(CommandDispatchIntent::RefreshGit)
        .expect("git refresh should dispatch")
    {
        AppCommandOutcome::GitUpdated(projection) => projection,
        other => panic!("expected git projection, got {other:?}"),
    };

    assert!(
        !projection.conflicts.is_empty(),
        "conflicts should be present after merge"
    );
    let conflict = projection
        .conflicts
        .iter()
        .find(|c| c.path == "src/lib.rs")
        .expect("src/lib.rs conflict should exist");
    assert!(
        conflict.actions.iter().any(|a| a == "accept_current"),
        "accept_current action should be present"
    );

    // Accept incoming side to verify the buffer is synchronized.
    let resolved = match app
        .dispatch_ui_intent(CommandDispatchIntent::ResolveGitConflict {
            path: "src/lib.rs".to_string(),
            choice: GitConflictChoiceProjection::AcceptIncoming,
        })
        .expect("resolve conflict should dispatch")
    {
        AppCommandOutcome::GitUpdated(projection) => projection,
        other => panic!("expected git projection, got {other:?}"),
    };

    assert!(
        !resolved.conflicts.iter().any(|c| c.path == "src/lib.rs"),
        "src/lib.rs conflict should be resolved"
    );

    // Active buffer projection should contain the accepted side and no markers.
    let snapshot = app
        .shell_projection_snapshot("test")
        .expect("snapshot should build");
    let active_text = snapshot
        .active_buffer_projection
        .small_buffer_preview
        .as_deref()
        .expect("active buffer should have small preview text");
    assert!(
        active_text.contains("incoming_alpha"),
        "active buffer should contain accepted incoming text, got:\n{active_text}"
    );
    assert!(
        active_text.contains("incoming_beta"),
        "active buffer should contain accepted incoming text, got:\n{active_text}"
    );
    assert!(
        !active_text.contains("<<<<<<<"),
        "active buffer should not contain conflict markers, got:\n{active_text}"
    );
    assert!(
        !active_text.contains("======="),
        "active buffer should not contain conflict markers, got:\n{active_text}"
    );
    assert!(
        !active_text.contains(">>>>>>>"),
        "active buffer should not contain conflict markers, got:\n{active_text}"
    );

    // Subsequent edit should not resurrect stale conflict-marker text.
    app.edit_active_buffer(TextEdit::insert(TextPosition::new(0, 0), "// header\n"))
        .expect("edit should apply");

    let after_edit = app
        .shell_projection_snapshot("test")
        .expect("snapshot should build");
    let after_text = after_edit
        .active_buffer_projection
        .small_buffer_preview
        .as_deref()
        .expect("active buffer should have text after edit");
    assert!(
        after_text.contains("// header"),
        "edit should be present, got:\n{after_text}"
    );
    assert!(
        !after_text.contains("<<<<<<<"),
        "edit should not resurrect conflict markers, got:\n{after_text}"
    );

    // Save should succeed without stale conflict metadata.
    let save_outcome = app.save_active_buffer().expect("save should dispatch");
    assert!(
        matches!(save_outcome, devil_app::AppSaveOutcome::Saved(_)),
        "save should succeed after conflict resolution, got {save_outcome:?}"
    );

    let disk = std::fs::read_to_string(&source).expect("file should be readable");
    assert!(
        disk.contains("// header"),
        "disk should contain the edit, got:\n{disk}"
    );
    assert!(
        !disk.contains("<<<<<<<"),
        "disk should not contain conflict markers after save, got:\n{disk}"
    );
}

#[test]
fn git_workflow_rejects_conflict_resolution_when_buffer_is_dirty() {
    let repo = TempGitRepo::new();
    let source = repo.write(
        "src/lib.rs",
        "pub fn alpha() {\n    original();\n}\n\npub fn beta() {\n    original_beta();\n}\n",
    );
    run_git(repo.path(), ["add", "."]);
    run_git(repo.path(), ["commit", "-m", "initial"]);

    run_git(repo.path(), ["checkout", "-b", "feature"]);
    repo.write(
        "src/lib.rs",
        "pub fn alpha() {\n    incoming_alpha();\n}\n\npub fn beta() {\n    incoming_beta();\n}\n",
    );
    run_git(repo.path(), ["add", "."]);
    run_git(repo.path(), ["commit", "-m", "feature"]);

    run_git(repo.path(), ["checkout", "master"]);
    repo.write(
        "src/lib.rs",
        "pub fn alpha() {\n    current_alpha();\n}\n\npub fn beta() {\n    current_beta();\n}\n",
    );
    run_git(repo.path(), ["add", "."]);
    run_git(repo.path(), ["commit", "-m", "master"]);

    let _ = Command::new("git")
        .current_dir(repo.path())
        .args(["merge", "feature"])
        .output()
        .expect("git merge command should run");

    let mut app = AppComposition::new();
    app.open_workspace(
        repo.path(),
        devil_protocol::WorkspaceTrustState::Trusted,
        devil_protocol::PrincipalId("git-conflict-test".to_string()),
    )
    .expect("workspace should open");
    app.open_file(source.to_string_lossy())
        .expect("source should open");

    let projection = match app
        .dispatch_ui_intent(CommandDispatchIntent::RefreshGit)
        .expect("git refresh should dispatch")
    {
        AppCommandOutcome::GitUpdated(projection) => projection,
        other => panic!("expected git projection, got {other:?}"),
    };

    assert!(
        !projection.conflicts.is_empty(),
        "conflicts should be present after merge"
    );

    // Make an unsaved edit to the active buffer.
    app.edit_active_buffer(TextEdit::insert(TextPosition::new(0, 0), "// dirty\n"))
        .expect("edit should apply");

    // Attempting to resolve the conflict while the buffer is dirty should fail.
    let err = app
        .dispatch_ui_intent(CommandDispatchIntent::ResolveGitConflict {
            path: "src/lib.rs".to_string(),
            choice: GitConflictChoiceProjection::AcceptCurrent,
        })
        .expect_err("resolve conflict should fail when buffer is dirty");
    assert!(
        err.to_string().contains("unsaved changes"),
        "error should mention unsaved changes: {err}"
    );

    // Dirty text should remain in the active buffer.
    let snapshot = app
        .shell_projection_snapshot("test")
        .expect("snapshot should build");
    let active_text = snapshot
        .active_buffer_projection
        .small_buffer_preview
        .as_deref()
        .expect("active buffer should have text");
    assert!(
        active_text.contains("// dirty"),
        "active buffer should still contain dirty edit, got:\n{active_text}"
    );
    assert!(
        active_text.contains("<<<<<<<"),
        "active buffer should still contain conflict markers, got:\n{active_text}"
    );

    // Disk should still contain conflict markers.
    let disk = fs::read_to_string(&source).expect("file should be readable");
    assert!(
        disk.contains("<<<<<<<"),
        "disk should still contain conflict markers after failed resolution"
    );
    assert!(
        disk.contains("======="),
        "disk should still contain conflict markers after failed resolution"
    );
    assert!(
        disk.contains(">>>>>>>"),
        "disk should still contain conflict markers after failed resolution"
    );

    // Git status should still report unmerged/conflicted.
    let status = run_git(repo.path(), ["status", "--porcelain", "--", "src/lib.rs"]);
    assert!(
        status.contains("U"),
        "git status should still show unmerged after failed resolution: {status}"
    );
}

#[test]
fn git_workflow_rejects_conflict_resolution_from_subdirectory_when_buffer_dirty() {
    let repo = TempGitRepo::new();
    let source = repo.write(
        "src/lib.rs",
        "pub fn alpha() {\n    original();\n}\n\npub fn beta() {\n    original_beta();\n}\n",
    );
    run_git(repo.path(), ["add", "."]);
    run_git(repo.path(), ["commit", "-m", "initial"]);

    run_git(repo.path(), ["checkout", "-b", "feature"]);
    repo.write(
        "src/lib.rs",
        "pub fn alpha() {\n    incoming_alpha();\n}\n\npub fn beta() {\n    incoming_beta();\n}\n",
    );
    run_git(repo.path(), ["add", "."]);
    run_git(repo.path(), ["commit", "-m", "feature"]);

    run_git(repo.path(), ["checkout", "master"]);
    repo.write(
        "src/lib.rs",
        "pub fn alpha() {\n    current_alpha();\n}\n\npub fn beta() {\n    current_beta();\n}\n",
    );
    run_git(repo.path(), ["add", "."]);
    run_git(repo.path(), ["commit", "-m", "master"]);

    let _ = Command::new("git")
        .current_dir(repo.path())
        .args(["merge", "feature"])
        .output()
        .expect("git merge command should run");

    // Open workspace from a subdirectory of the repo.
    let subdir = repo.path().join("src");
    let mut app = AppComposition::new();
    app.open_workspace(
        &subdir,
        devil_protocol::WorkspaceTrustState::Trusted,
        devil_protocol::PrincipalId("git-conflict-subdir-dirty-test".to_string()),
    )
    .expect("workspace should open from subdirectory");
    // Open the file using the absolute repo path.
    app.open_file(source.to_string_lossy())
        .expect("source should open");

    let projection = match app
        .dispatch_ui_intent(CommandDispatchIntent::RefreshGit)
        .expect("git refresh should dispatch")
    {
        AppCommandOutcome::GitUpdated(projection) => projection,
        other => panic!("expected git projection, got {other:?}"),
    };

    assert!(
        !projection.conflicts.is_empty(),
        "conflicts should be present after merge"
    );
    let conflict = projection
        .conflicts
        .iter()
        .find(|c| c.path == "src/lib.rs")
        .expect("src/lib.rs conflict should exist");
    assert!(
        conflict.actions.iter().any(|a| a == "accept_current"),
        "accept_current action should be present"
    );

    // Make an unsaved edit so the buffer is dirty.
    app.edit_active_buffer(TextEdit::insert(TextPosition::new(0, 0), "// dirty\n"))
        .expect("edit should apply");

    // Resolve with repo-relative path should reject because buffer is dirty.
    let err = app
        .dispatch_ui_intent(CommandDispatchIntent::ResolveGitConflict {
            path: "src/lib.rs".to_string(),
            choice: GitConflictChoiceProjection::AcceptCurrent,
        })
        .expect_err("resolve conflict should fail when buffer is dirty");
    assert!(
        err.to_string().contains("unsaved changes"),
        "error should mention unsaved changes: {err}"
    );

    // Dirty text should remain in the active buffer.
    let snapshot = app
        .shell_projection_snapshot("test")
        .expect("snapshot should build");
    let active_text = snapshot
        .active_buffer_projection
        .small_buffer_preview
        .as_deref()
        .expect("active buffer should have text");
    assert!(
        active_text.contains("// dirty"),
        "active buffer should still contain dirty edit, got:\n{active_text}"
    );
    assert!(
        active_text.contains("<<<<<<<"),
        "active buffer should still contain conflict markers, got:\n{active_text}"
    );

    // Disk should still contain conflict markers.
    let disk = fs::read_to_string(&source).expect("file should be readable");
    assert!(
        disk.contains("<<<<<<<"),
        "disk should still contain conflict markers after failed resolution"
    );
    assert!(
        disk.contains("======="),
        "disk should still contain conflict markers after failed resolution"
    );
    assert!(
        disk.contains(">>>>>>>"),
        "disk should still contain conflict markers after failed resolution"
    );

    // Git status should still report unmerged/conflicted.
    let status = run_git(repo.path(), ["status", "--porcelain", "--", "src/lib.rs"]);
    assert!(
        status.contains("U"),
        "git status should still show unmerged after failed resolution: {status}"
    );
}

#[test]
fn git_workflow_resolves_conflict_from_subdirectory_and_syncs_open_buffer() {
    let repo = TempGitRepo::new();
    let source = repo.write(
        "src/lib.rs",
        "pub fn alpha() {\n    original();\n}\n\npub fn beta() {\n    original_beta();\n}\n",
    );
    run_git(repo.path(), ["add", "."]);
    run_git(repo.path(), ["commit", "-m", "initial"]);

    run_git(repo.path(), ["checkout", "-b", "feature"]);
    repo.write(
        "src/lib.rs",
        "pub fn alpha() {\n    incoming_alpha();\n}\n\npub fn beta() {\n    incoming_beta();\n}\n",
    );
    run_git(repo.path(), ["add", "."]);
    run_git(repo.path(), ["commit", "-m", "feature"]);

    run_git(repo.path(), ["checkout", "master"]);
    repo.write(
        "src/lib.rs",
        "pub fn alpha() {\n    current_alpha();\n}\n\npub fn beta() {\n    current_beta();\n}\n",
    );
    run_git(repo.path(), ["add", "."]);
    run_git(repo.path(), ["commit", "-m", "master"]);

    let _ = Command::new("git")
        .current_dir(repo.path())
        .args(["merge", "feature"])
        .output()
        .expect("git merge command should run");

    // Open workspace from a subdirectory of the repo.
    let subdir = repo.path().join("src");
    let mut app = AppComposition::new();
    app.open_workspace(
        &subdir,
        devil_protocol::WorkspaceTrustState::Trusted,
        devil_protocol::PrincipalId("git-conflict-subdir-sync-test".to_string()),
    )
    .expect("workspace should open from subdirectory");
    // Open the file using the absolute repo path.
    app.open_file(source.to_string_lossy())
        .expect("source should open");

    let projection = match app
        .dispatch_ui_intent(CommandDispatchIntent::RefreshGit)
        .expect("git refresh should dispatch")
    {
        AppCommandOutcome::GitUpdated(projection) => projection,
        other => panic!("expected git projection, got {other:?}"),
    };

    assert!(
        !projection.conflicts.is_empty(),
        "conflicts should be present after merge"
    );
    let conflict = projection
        .conflicts
        .iter()
        .find(|c| c.path == "src/lib.rs")
        .expect("src/lib.rs conflict should exist");
    assert!(
        conflict.actions.iter().any(|a| a == "accept_current"),
        "accept_current action should be present"
    );

    // Resolve with repo-relative path should succeed and sync the open buffer.
    let resolved = match app
        .dispatch_ui_intent(CommandDispatchIntent::ResolveGitConflict {
            path: "src/lib.rs".to_string(),
            choice: GitConflictChoiceProjection::AcceptCurrent,
        })
        .expect("resolve conflict should dispatch")
    {
        AppCommandOutcome::GitUpdated(projection) => projection,
        other => panic!("expected git projection, got {other:?}"),
    };

    assert!(
        !resolved.conflicts.iter().any(|c| c.path == "src/lib.rs"),
        "src/lib.rs conflict should be resolved"
    );

    // Active buffer projection should contain the accepted side and no markers.
    let snapshot = app
        .shell_projection_snapshot("test")
        .expect("snapshot should build");
    let active_text = snapshot
        .active_buffer_projection
        .small_buffer_preview
        .as_deref()
        .expect("active buffer should have small preview text");
    assert!(
        active_text.contains("current_alpha"),
        "active buffer should contain accepted current text, got:\n{active_text}"
    );
    assert!(
        active_text.contains("current_beta"),
        "active buffer should contain accepted current text, got:\n{active_text}"
    );
    assert!(
        !active_text.contains("<<<<<<<"),
        "active buffer should not contain conflict markers, got:\n{active_text}"
    );
    assert!(
        !active_text.contains("======="),
        "active buffer should not contain conflict markers, got:\n{active_text}"
    );
    assert!(
        !active_text.contains(">>>>>>>"),
        "active buffer should not contain conflict markers, got:\n{active_text}"
    );

    // Disk should also reflect the resolution.
    let disk = fs::read_to_string(&source).expect("file should be readable");
    assert!(
        disk.contains("current_alpha"),
        "disk should contain resolved text, got:\n{disk}"
    );
    assert!(
        !disk.contains("<<<<<<<"),
        "disk should not contain conflict markers after resolution, got:\n{disk}"
    );

    // Subsequent edit should not resurrect stale conflict-marker text.
    app.edit_active_buffer(TextEdit::insert(TextPosition::new(0, 0), "// header\n"))
        .expect("edit should apply");

    let after_edit = app
        .shell_projection_snapshot("test")
        .expect("snapshot should build");
    let after_text = after_edit
        .active_buffer_projection
        .small_buffer_preview
        .as_deref()
        .expect("active buffer should have text after edit");
    assert!(
        after_text.contains("// header"),
        "edit should be present, got:\n{after_text}"
    );
    assert!(
        !after_text.contains("<<<<<<<"),
        "edit should not resurrect conflict markers, got:\n{after_text}"
    );

    // Save should succeed without stale conflict metadata.
    let save_outcome = app.save_active_buffer().expect("save should dispatch");
    assert!(
        matches!(save_outcome, devil_app::AppSaveOutcome::Saved(_)),
        "save should succeed after conflict resolution, got {save_outcome:?}"
    );

    let disk_after_save = std::fs::read_to_string(&source).expect("file should be readable");
    assert!(
        disk_after_save.contains("// header"),
        "disk should contain the edit, got:\n{disk_after_save}"
    );
    assert!(
        !disk_after_save.contains("<<<<<<<"),
        "disk should not contain conflict markers after save, got:\n{disk_after_save}"
    );
}
