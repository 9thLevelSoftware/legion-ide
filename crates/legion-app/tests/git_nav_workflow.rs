/// Task 1: Diff-review keyboard navigation.
///
/// Tests that hunk and file navigation intents move the focused hunk in the
/// git projection; the actual keybinding routing lives in the desktop adapter.
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use legion_app::{AppCommandOutcome, AppComposition};
use legion_ui::{CommandDispatchIntent, GitHunkStageProjection};

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

struct TempGitRepo {
    root: PathBuf,
}

fn git_available() -> bool {
    use std::sync::OnceLock;
    static AVAILABLE: OnceLock<bool> = OnceLock::new();
    *AVAILABLE.get_or_init(|| {
        Command::new("git")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    })
}

impl TempGitRepo {
    fn new() -> Self {
        assert!(git_available(), "git binary required for navigation tests");
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        let id = TEMP_COUNTER.fetch_add(1, Ordering::SeqCst);
        let root = std::env::temp_dir().join(format!(
            "legion_git_nav_{}_{}_{}",
            std::process::id(),
            nanos,
            id
        ));
        fs::create_dir(&root).expect("temp dir");
        run_git(&root, ["init"]);
        run_git(&root, ["branch", "-M", "master"]);
        run_git(&root, ["config", "user.email", "nav@test.example"]);
        run_git(&root, ["config", "user.name", "Nav Test"]);
        Self { root }
    }

    fn path(&self) -> &Path {
        &self.root
    }

    fn write(&self, relative: &str, content: &str) -> PathBuf {
        let path = self.root.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("parent dir");
        }
        fs::write(&path, content).expect("write file");
        path
    }
}

impl Drop for TempGitRepo {
    fn drop(&mut self) {
        let temp_root = std::env::temp_dir();
        let file_name = self.root.file_name().and_then(|n| n.to_str());
        if self.root.starts_with(&temp_root)
            && file_name.is_some_and(|n| n.starts_with("legion_git_nav_"))
        {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
}

fn run_git<const N: usize>(root: &Path, args: [&str; N]) {
    let status = Command::new("git")
        .current_dir(root)
        .args(args)
        .status()
        .expect("git command");
    assert!(status.success(), "git {:?} failed", args);
}

fn open_app_with_hunks(repo: &TempGitRepo) -> (AppComposition, PathBuf, PathBuf) {
    // Create two files with changes so we have multiple hunks across multiple files.
    let src_a = repo.write("src/a.rs", "pub fn a1() { 1 }\n\n\n\npub fn a2() { 2 }\n");
    let src_b = repo.write("src/b.rs", "pub fn b1() { 1 }\n\n\n\npub fn b2() { 2 }\n");
    run_git(repo.path(), ["add", "."]);
    run_git(repo.path(), ["commit", "-m", "initial"]);

    // Modify both files so each has at least one hunk.
    repo.write("src/a.rs", "pub fn a1() { 10 }\n\n\n\npub fn a2() { 20 }\n");
    repo.write("src/b.rs", "pub fn b1() { 10 }\n\n\n\npub fn b2() { 20 }\n");

    let mut app = AppComposition::new();
    app.open_workspace(
        repo.path(),
        legion_protocol::WorkspaceTrustState::Trusted,
        legion_protocol::PrincipalId("nav-test".to_string()),
    )
    .expect("workspace should open");
    app.open_file(src_a.to_string_lossy()).expect("open a.rs");

    // Refresh so hunks are populated.
    app.dispatch_ui_intent(CommandDispatchIntent::RefreshGit)
        .expect("git refresh");

    (app, src_a, src_b)
}

#[test]
fn git_nav_next_hunk_sets_focused_hunk_id() {
    let repo = TempGitRepo::new();
    let (mut app, _, _) = open_app_with_hunks(&repo);

    // Initially no hunk is focused.
    let snapshot = app.shell_projection_snapshot("nav").expect("snapshot");
    assert!(
        snapshot.git_projection.focused_hunk_id.is_none(),
        "focused_hunk_id should start as None"
    );

    // Navigate to next hunk.
    let projection = match app
        .dispatch_ui_intent(CommandDispatchIntent::GitNavNextHunk)
        .expect("next hunk should dispatch")
    {
        AppCommandOutcome::GitUpdated(p) => p,
        other => panic!("expected GitUpdated, got {other:?}"),
    };

    assert!(
        projection.focused_hunk_id.is_some(),
        "focused_hunk_id should be set after GitNavNextHunk"
    );
    // The focused hunk id must correspond to a real hunk.
    let focused = projection.focused_hunk_id.as_deref().unwrap();
    assert!(
        projection.hunks.iter().any(|h| h.hunk_id == focused),
        "focused hunk id should match a projected hunk"
    );
}

#[test]
fn git_nav_prev_hunk_wraps_to_last_when_no_focus() {
    let repo = TempGitRepo::new();
    let (mut app, _, _) = open_app_with_hunks(&repo);

    // Navigate backward from no focus — should land on the last hunk.
    let projection = match app
        .dispatch_ui_intent(CommandDispatchIntent::GitNavPrevHunk)
        .expect("prev hunk should dispatch")
    {
        AppCommandOutcome::GitUpdated(p) => p,
        other => panic!("expected GitUpdated, got {other:?}"),
    };

    assert!(
        projection.focused_hunk_id.is_some(),
        "focused_hunk_id should be set after GitNavPrevHunk"
    );
    let focused = projection.focused_hunk_id.as_deref().unwrap();
    // Should be the last hunk in the list.
    let last_hunk_id = projection
        .hunks
        .last()
        .map(|h| h.hunk_id.as_str())
        .expect("at least one hunk");
    assert_eq!(
        focused, last_hunk_id,
        "prev from no focus should land on last hunk"
    );
}

#[test]
fn git_nav_next_hunk_advances_through_hunks() {
    let repo = TempGitRepo::new();
    let (mut app, _, _) = open_app_with_hunks(&repo);

    // Navigate to first hunk.
    let p1 = match app
        .dispatch_ui_intent(CommandDispatchIntent::GitNavNextHunk)
        .expect("next hunk 1")
    {
        AppCommandOutcome::GitUpdated(p) => p,
        other => panic!("{other:?}"),
    };
    let first = p1.focused_hunk_id.clone().expect("first hunk");

    // Navigate to second.
    let p2 = match app
        .dispatch_ui_intent(CommandDispatchIntent::GitNavNextHunk)
        .expect("next hunk 2")
    {
        AppCommandOutcome::GitUpdated(p) => p,
        other => panic!("{other:?}"),
    };
    let second = p2.focused_hunk_id.clone().expect("second hunk");

    // If there are multiple hunks, they should differ; if only one, they should be equal (wrapped).
    if p1.hunks.len() > 1 {
        assert_ne!(first, second, "advancing should move to a different hunk");
    }
}

#[test]
fn git_nav_next_file_jumps_across_files() {
    let repo = TempGitRepo::new();
    let (mut app, _, _) = open_app_with_hunks(&repo);

    // Seed: focus on the first hunk.
    let p1 = match app
        .dispatch_ui_intent(CommandDispatchIntent::GitNavNextHunk)
        .expect("seed next hunk")
    {
        AppCommandOutcome::GitUpdated(p) => p,
        other => panic!("{other:?}"),
    };
    let first_file = p1
        .focused_hunk_id
        .as_deref()
        .and_then(|id| p1.hunks.iter().find(|h| h.hunk_id == id))
        .map(|h| h.path.clone())
        .expect("first hunk should have a path");

    // Navigate to next file.
    let p2 = match app
        .dispatch_ui_intent(CommandDispatchIntent::GitNavNextFile)
        .expect("next file")
    {
        AppCommandOutcome::GitUpdated(p) => p,
        other => panic!("{other:?}"),
    };

    let unique_files: std::collections::HashSet<&str> =
        p2.hunks.iter().map(|h| h.path.as_str()).collect();
    if unique_files.len() > 1 {
        // Should have moved to a different file.
        let new_file = p2
            .focused_hunk_id
            .as_deref()
            .and_then(|id| p2.hunks.iter().find(|h| h.hunk_id == id))
            .map(|h| h.path.as_str())
            .expect("should have focused hunk");
        assert_ne!(
            new_file,
            first_file.as_str(),
            "next-file nav should move to different file"
        );
    }
}

#[test]
fn git_nav_snapshot_reflects_focused_hunk() {
    let repo = TempGitRepo::new();
    let (mut app, _, _) = open_app_with_hunks(&repo);

    app.dispatch_ui_intent(CommandDispatchIntent::GitNavNextHunk)
        .expect("next hunk");

    let snapshot = app.shell_projection_snapshot("nav").expect("snapshot");
    assert!(
        snapshot.git_projection.focused_hunk_id.is_some(),
        "shell snapshot git_projection should reflect focused hunk"
    );
}
