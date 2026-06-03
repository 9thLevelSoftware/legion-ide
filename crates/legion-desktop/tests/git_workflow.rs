use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use legion_desktop::{
    bridge::DesktopAction,
    view::DesktopProjectionViewModel,
    workflow::{DesktopLaunchConfig, DesktopRuntime, DesktopWorkflowOutcome},
};
use legion_ui::GitHunkStageProjection;

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
            "legion_desktop_git_{}_{}_{}",
            std::process::id(),
            nanos,
            id
        ));
        fs::create_dir(&root).expect("temp git repo should be created");
        run_git(&root, ["init"]);
        run_git(&root, ["branch", "-M", "master"]);
        run_git(&root, ["config", "user.email", "legion@example.test"]);
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
            && file_name.is_some_and(|name| name.starts_with("legion_desktop_git_"))
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

fn conflict_marker_text() -> String {
    format!(
        "{} ours\nfn current() {{}}\n{}\nfn incoming() {{}}\n{} theirs\n",
        "<".repeat(7),
        "=".repeat(7),
        ">".repeat(7)
    )
}

#[test]
fn desktop_git_workflow_projects_diff_blame_graph_and_hunk_actions() {
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
    repo.write("src/conflict.rs", &conflict_marker_text());

    let mut runtime = DesktopRuntime::open(DesktopLaunchConfig::new(
        repo.path().to_path_buf(),
        Some(source.to_string_lossy().into_owned()),
    ))
    .expect("desktop runtime should open git workspace");

    assert_eq!(
        runtime
            .handle_action(DesktopAction::RefreshGit)
            .expect("git refresh should route"),
        DesktopWorkflowOutcome::GitUpdated
    );
    let snapshot = runtime.projection_snapshot();
    assert_eq!(snapshot.git_projection.changed_files.len(), 2);
    assert!(
        snapshot
            .git_projection
            .conflicts
            .iter()
            .any(|conflict| conflict.path == "src/conflict.rs")
    );
    let model = DesktopProjectionViewModel::from_snapshot(&snapshot);
    assert!(
        model
            .git_rows
            .iter()
            .any(|row| row.contains("git file src/lib.rs"))
    );
    assert!(
        model
            .git_rows
            .iter()
            .any(|row| row.contains("git blame src/lib.rs"))
    );
    assert!(
        model
            .git_rows
            .iter()
            .any(|row| row.contains("git commit") && row.contains("initial"))
    );

    let hunk_id = snapshot
        .git_projection
        .hunks
        .iter()
        .find(|hunk| hunk.stage == GitHunkStageProjection::Unstaged)
        .expect("unstaged hunk should exist")
        .hunk_id
        .clone();
    assert_eq!(
        runtime
            .handle_action(DesktopAction::StageGitHunk { hunk_id })
            .expect("hunk stage should route"),
        DesktopWorkflowOutcome::GitUpdated
    );
    let cached = run_git(repo.path(), ["diff", "--cached", "--", "src/lib.rs"]);
    assert!(cached.contains("first_changed"));
    assert!(!cached.contains("second_changed"));
}

#[test]
fn desktop_git_workflow_resolves_conflicts_through_bridge_actions() {
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

    let mut runtime = DesktopRuntime::open(DesktopLaunchConfig::new(
        repo.path().to_path_buf(),
        Some(source.to_string_lossy().into_owned()),
    ))
    .expect("desktop runtime should open git workspace");

    assert_eq!(
        runtime
            .handle_action(DesktopAction::RefreshGit)
            .expect("git refresh should route"),
        DesktopWorkflowOutcome::GitUpdated
    );
    let snapshot = runtime.projection_snapshot();
    assert!(
        !snapshot.git_projection.conflicts.is_empty(),
        "conflicts should be present after merge"
    );
    assert!(
        snapshot
            .git_projection
            .conflicts
            .iter()
            .any(|c| c.path == "src/lib.rs"),
        "src/lib.rs should be conflicted"
    );

    assert_eq!(
        runtime
            .handle_action(DesktopAction::AcceptGitConflictCurrent {
                path: "src/lib.rs".to_string(),
            })
            .expect("accept current should route"),
        DesktopWorkflowOutcome::GitUpdated
    );

    let snapshot = runtime.projection_snapshot();
    assert!(
        !snapshot
            .git_projection
            .conflicts
            .iter()
            .any(|c| c.path == "src/lib.rs"),
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
