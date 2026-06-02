use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use devil_project::{
    GitDiffStrategy, GitHunkStage, GitSnapshotOptions, collect_git_snapshot, stage_git_hunk,
    unstage_git_hunk,
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
            "devil_project_git_{}_{}_{}",
            std::process::id(),
            nanos,
            id
        ));
        fs::create_dir(&root).expect("temp git repo should be created");
        run_git(&root, ["init"]);
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
            && file_name.is_some_and(|name| name.starts_with("devil_project_git_"))
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
fn git_snapshot_projects_syntactic_diff_blame_graph_conflicts_and_hunk_staging() {
    let repo = TempGitRepo::new();
    let source_path = repo.write(
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

    let options = GitSnapshotOptions {
        max_file_bytes_for_syntactic_diff: 1024 * 1024,
        max_hunks: 16,
        max_blame_lines: 16,
        max_commits: 8,
    };
    let snapshot = collect_git_snapshot(repo.path(), Some(&source_path), options.clone())
        .expect("git snapshot should collect");

    assert_eq!(snapshot.branch_label.as_deref(), Some("master"));
    assert_eq!(snapshot.changed_files.len(), 2);
    let source_file = snapshot
        .changed_files
        .iter()
        .find(|file| file.path == "src/lib.rs")
        .expect("source file should be changed");
    assert_eq!(source_file.diff_strategy, GitDiffStrategy::Syntactic);
    assert_eq!(source_file.unstaged_hunk_count, 2);
    assert!(source_file.stageable);
    assert!(
        snapshot
            .blame_lines
            .iter()
            .any(|line| line.path == "src/lib.rs" && line.author == "Devil Test")
    );
    assert!(
        snapshot
            .commits
            .iter()
            .any(|commit| commit.summary == "initial" && commit.parent_count == 0)
    );
    assert!(
        snapshot
            .conflicts
            .iter()
            .any(|conflict| conflict.path == "src/conflict.rs" && conflict.marker_count == 3)
    );

    let first_hunk = snapshot
        .hunks
        .iter()
        .find(|hunk| hunk.path == "src/lib.rs" && hunk.stage == GitHunkStage::Unstaged)
        .expect("unstaged hunk should be projected")
        .clone();
    stage_git_hunk(repo.path(), &first_hunk).expect("hunk should stage");

    let cached = run_git(repo.path(), ["diff", "--cached", "--", "src/lib.rs"]);
    assert!(cached.contains("first_changed"));
    assert!(!cached.contains("second_changed"));

    let after_stage =
        collect_git_snapshot(repo.path(), Some(&source_path), options).expect("git refresh");
    assert!(
        after_stage
            .hunks
            .iter()
            .any(|hunk| hunk.path == "src/lib.rs" && hunk.stage == GitHunkStage::Staged)
    );
    assert!(
        after_stage
            .hunks
            .iter()
            .any(|hunk| hunk.path == "src/lib.rs" && hunk.stage == GitHunkStage::Unstaged)
    );

    let staged_hunk = after_stage
        .hunks
        .iter()
        .find(|hunk| hunk.path == "src/lib.rs" && hunk.stage == GitHunkStage::Staged)
        .expect("staged hunk should be projected")
        .clone();
    unstage_git_hunk(repo.path(), &staged_hunk).expect("hunk should unstage");
    let cached_after_unstage = run_git(repo.path(), ["diff", "--cached", "--", "src/lib.rs"]);
    assert!(cached_after_unstage.trim().is_empty());
}
