use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use devil_app::{AppCommandOutcome, AppComposition};
use devil_ui::{
    CommandDispatchIntent, GitDiffStrategyProjection, GitHunkStageProjection,
    SearchStatusKindProjection,
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
        run_git(&root, ["config", "user.name", "Devil Test"]);
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
            .any(|line| line.author == "Devil Test")
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
