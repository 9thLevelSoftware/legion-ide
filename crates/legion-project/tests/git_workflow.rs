use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use legion_project::{
    GitConflictChoice, GitDiffStrategy, GitHunkStage, GitInspectionBackend, GitSnapshotOptions,
    ProjectGitWorktreeKind, collect_git_snapshot, collect_git_snapshot_with_backend,
    commit_git_changes, git_forge_kind, git_pull_request_url, git_worktree_kind_for_path,
    resolve_git_conflict, stage_git_hunk, unstage_git_hunk, validate_git_commit_message,
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
            "legion_project_git_{}_{}_{}",
            std::process::id(),
            nanos,
            id
        ));
        fs::create_dir(&root).expect("temp git repo should be created");
        run_git(&root, ["init"]);
        run_git(&root, ["branch", "-M", "master"]);
        run_git(&root, ["config", "user.email", "legion@example.test"]);
        run_git(&root, ["config", "user.name", "Legion Test"]);
        run_git(&root, ["config", "core.autocrlf", "false"]);
        run_git(&root, ["config", "core.eol", "lf"]);
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
            && file_name.is_some_and(|name| name.starts_with("legion_project_git_"))
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

fn projected_path_matches(path: &str, expected: &Path) -> bool {
    Path::new(path)
        .canonicalize()
        .is_ok_and(|actual| actual == expected)
}

fn conflict_marker_text() -> String {
    format!(
        "{} ours\nfn current() {{}}\n{}\nfn incoming() {{}}\n{} theirs\n",
        "<".repeat(7),
        "=".repeat(7),
        ">".repeat(7)
    )
}

fn create_unmerged_state(repo: &TempGitRepo, files: &[(&str, &str)]) {
    for (path, _) in files {
        repo.write(path, "base version\n");
    }
    run_git(repo.path(), ["add", "."]);
    run_git(repo.path(), ["commit", "-m", "base"]);
    run_git(repo.path(), ["checkout", "-b", "feature"]);
    for (path, _) in files {
        repo.write(path, "incoming version\n");
    }
    run_git(repo.path(), ["add", "."]);
    run_git(repo.path(), ["commit", "-m", "feature"]);
    run_git(repo.path(), ["checkout", "master"]);
    for (path, _) in files {
        repo.write(path, "current version\n");
    }
    run_git(repo.path(), ["add", "."]);
    run_git(repo.path(), ["commit", "-m", "master"]);
    let _ = Command::new("git")
        .current_dir(repo.path())
        .args(["merge", "feature"])
        .output()
        .expect("git merge should run");
    for (path, content) in files {
        repo.write(path, content);
    }
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
            .any(|line| line.path == "src/lib.rs" && line.author == "Legion Test")
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

#[test]
fn git_snapshot_gix_backend_matches_cli_backend() {
    let repo = TempGitRepo::new();
    let source_path = repo.write(
        "src/lib.rs",
        "pub fn alpha() {\n    first();\n}\n\n\npub fn beta() {\n    second();\n}\n",
    );
    run_git(repo.path(), ["add", "."]);
    run_git(repo.path(), ["commit", "-m", "initial"]);

    repo.write(
        "src/lib.rs",
        "pub fn alpha() {\n    first_changed();\n}\n\n\npub fn beta() {\n    second_changed();\n}\n",
    );
    repo.write("src/conflict.rs", &conflict_marker_text());

    let options = GitSnapshotOptions {
        max_file_bytes_for_syntactic_diff: 1024 * 1024,
        max_hunks: 16,
        max_blame_lines: 16,
        max_commits: 8,
    };
    let cli = collect_git_snapshot_with_backend(
        repo.path(),
        Some(&source_path),
        options.clone(),
        GitInspectionBackend::Cli,
    )
    .expect("cli git snapshot should collect");
    let gix = collect_git_snapshot_with_backend(
        repo.path(),
        Some(&source_path),
        options,
        GitInspectionBackend::Gix,
    )
    .expect("gix git snapshot should collect");

    assert_eq!(cli.branch_label, gix.branch_label);
    assert_eq!(cli.head_short, gix.head_short);
    assert_eq!(cli.remote_url, gix.remote_url);
    assert_eq!(cli.remote_default_branch, gix.remote_default_branch);
    assert_eq!(cli.changed_files, gix.changed_files);
    assert_eq!(cli.hunks, gix.hunks);
    assert_eq!(cli.blame_lines, gix.blame_lines);
    assert_eq!(cli.commits, gix.commits);
    assert_eq!(cli.conflicts, gix.conflicts);
    assert_eq!(cli.worktrees, gix.worktrees);
}

#[test]
fn git_snapshot_projects_worktrees_and_orphan_prunable_entries() {
    let repo = TempGitRepo::new();
    repo.write("src/lib.rs", "pub fn alpha() {}\n");
    run_git(repo.path(), ["add", "."]);
    run_git(repo.path(), ["commit", "-m", "initial"]);

    let worktree_path = repo
        .path()
        .parent()
        .expect("repo parent")
        .join("legion-git-worktree");
    if worktree_path.exists() {
        std::fs::remove_dir_all(&worktree_path).expect("stale worktree path should be removable");
    }
    run_git(
        repo.path(),
        [
            "worktree",
            "add",
            worktree_path.to_str().expect("utf8"),
            "-b",
            "feature",
        ],
    );

    let options = GitSnapshotOptions {
        max_file_bytes_for_syntactic_diff: 1024 * 1024,
        max_hunks: 4,
        max_blame_lines: 4,
        max_commits: 4,
    };
    let snapshot = collect_git_snapshot(repo.path(), None, options.clone())
        .expect("git snapshot should collect");
    let repo_root = repo
        .path()
        .canonicalize()
        .expect("repo root should canonicalize");
    let worktree_root = worktree_path
        .canonicalize()
        .expect("worktree root should canonicalize");
    assert!(
        snapshot
            .worktrees
            .iter()
            .any(|worktree| projected_path_matches(&worktree.path, &repo_root))
    );
    assert!(
        snapshot
            .worktrees
            .iter()
            .any(|worktree| projected_path_matches(&worktree.path, &worktree_root))
    );
    assert!(
        snapshot
            .worktrees
            .iter()
            .any(|worktree| worktree.branch_label.as_deref() == Some("feature"))
    );

    std::fs::remove_dir_all(&worktree_path).expect("worktree directory should be removable");

    let prunable_snapshot = collect_git_snapshot(repo.path(), None, options)
        .expect("git refresh should collect after orphaning worktree");
    let orphan = prunable_snapshot
        .worktrees
        .iter()
        .find(|worktree| worktree.path.ends_with("legion-git-worktree"))
        .expect("orphaned worktree should still be projected before prune");
    assert!(
        orphan.prunable,
        "orphaned worktree should be flagged as prunable"
    );
}

#[test]
fn git_commit_validates_message_and_commits_staged_hunks() {
    let repo = TempGitRepo::new();
    let source_path = repo.write("src/lib.rs", "pub fn alpha() {\n    first();\n}\n");
    run_git(repo.path(), ["add", "."]);
    run_git(repo.path(), ["commit", "-m", "base"]);
    repo.write("src/lib.rs", "pub fn alpha() {\n    first_changed();\n}\n");

    let options = GitSnapshotOptions {
        max_file_bytes_for_syntactic_diff: 1024,
        max_hunks: 8,
        max_blame_lines: 8,
        max_commits: 8,
    };
    let snapshot = collect_git_snapshot(repo.path(), Some(&source_path), options)
        .expect("git refresh should succeed");
    let staged_hunk = snapshot
        .hunks
        .iter()
        .find(|hunk| hunk.path == "src/lib.rs" && hunk.stage == GitHunkStage::Unstaged)
        .expect("unstaged hunk should be projected")
        .clone();
    stage_git_hunk(repo.path(), &staged_hunk).expect("staging should succeed");

    let err = validate_git_commit_message("   \n\n").expect_err("blank commit should fail");
    assert!(err.to_string().contains("empty"));

    commit_git_changes(repo.path(), "feat: update alpha").expect("commit should succeed");
    let head = run_git(repo.path(), ["log", "-1", "--pretty=%s"]);
    assert_eq!(head.trim(), "feat: update alpha");
    let contents = fs::read_to_string(repo.path().join("src/lib.rs")).expect("file should read");
    assert!(contents.contains("first_changed"));
    assert!(
        run_git(repo.path(), ["diff", "--cached", "--", "src/lib.rs"])
            .trim()
            .is_empty()
    );
}

#[cfg(unix)]
#[test]
fn git_commit_disables_repository_controlled_hooks() {
    use std::os::unix::fs::PermissionsExt;

    let repo = TempGitRepo::new();
    repo.write("tracked.txt", "staged change\n");
    run_git(repo.path(), ["add", "tracked.txt"]);

    let hooks = repo.path().join("repository-hooks");
    fs::create_dir(&hooks).expect("hooks directory should be created");
    run_git(
        repo.path(),
        ["config", "core.hooksPath", "repository-hooks"],
    );
    for hook in [
        "pre-commit",
        "prepare-commit-msg",
        "commit-msg",
        "post-commit",
    ] {
        let hook_path = hooks.join(hook);
        fs::write(&hook_path, "#!/bin/sh\ntouch hook-ran\n").expect("hook should be written");
        let mut permissions = fs::metadata(&hook_path)
            .expect("hook metadata should be readable")
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&hook_path, permissions).expect("hook should be executable");
    }

    commit_git_changes(repo.path(), "feat: safe commit").expect("commit should succeed");

    assert!(
        !repo.path().join("hook-ran").exists(),
        "the IDE commit action must not execute repository-controlled hooks"
    );
    let head = run_git(repo.path(), ["log", "-1", "--pretty=%s"]);
    assert_eq!(head.trim(), "feat: safe commit");
}

#[test]
fn git_conflict_resolves_current_and_incoming() {
    let repo = TempGitRepo::new();
    let two_block_content = format!(
        "line1\n{current} current\nfn first_current() {{}}\n{sep}\nfn first_incoming() {{}}\n{incoming} incoming\nline2\n{current2} current2\nfn second_current() {{}}\n{sep2}\nfn second_incoming() {{}}\n{incoming2} incoming2\nline3\n",
        current = "<".repeat(7),
        sep = "=".repeat(7),
        incoming = ">".repeat(7),
        current2 = "<".repeat(7),
        sep2 = "=".repeat(7),
        incoming2 = ">".repeat(7),
    );
    create_unmerged_state(
        &repo,
        &[
            ("src/current.rs", &conflict_marker_text()),
            ("src/incoming.rs", &conflict_marker_text()),
            ("src/conflict.rs", &two_block_content),
        ],
    );

    let current_path = repo.path().join("src/current.rs");
    let incoming_path = repo.path().join("src/incoming.rs");

    resolve_git_conflict(
        repo.path(),
        "src/current.rs",
        GitConflictChoice::AcceptCurrent,
    )
    .expect("resolve current should succeed");
    let current_resolved = fs::read_to_string(&current_path).expect("should read");
    assert!(current_resolved.contains("fn current()"));
    assert!(!current_resolved.contains("fn incoming()"));
    assert!(!current_resolved.contains("<<<<<<<"));
    assert!(!current_resolved.contains("======="));
    assert!(!current_resolved.contains(">>>>>>>"));

    resolve_git_conflict(
        repo.path(),
        "src/incoming.rs",
        GitConflictChoice::AcceptIncoming,
    )
    .expect("resolve incoming should succeed");
    let incoming_resolved = fs::read_to_string(&incoming_path).expect("should read");
    assert!(incoming_resolved.contains("fn incoming()"));
    assert!(!incoming_resolved.contains("fn current()"));
    assert!(!incoming_resolved.contains("<<<<<<<"));
    assert!(!incoming_resolved.contains("======="));
    assert!(!incoming_resolved.contains(">>>>>>>"));

    resolve_git_conflict(
        repo.path(),
        "src/conflict.rs",
        GitConflictChoice::AcceptCurrent,
    )
    .expect("resolve multi-block current should succeed");
    let multi_resolved =
        fs::read_to_string(repo.path().join("src/conflict.rs")).expect("should read");
    assert!(multi_resolved.contains("fn first_current()"));
    assert!(multi_resolved.contains("fn second_current()"));
    assert!(!multi_resolved.contains("fn first_incoming()"));
    assert!(!multi_resolved.contains("fn second_incoming()"));
    assert!(!multi_resolved.contains("<<<<<<<"));
    assert!(!multi_resolved.contains("======="));
    assert!(!multi_resolved.contains(">>>>>>>"));
    assert!(multi_resolved.contains("line1\n"));
    assert!(multi_resolved.contains("line2\n"));
    assert!(multi_resolved.contains("line3\n"));
}

#[test]
fn git_conflict_refuses_outside_repo() {
    let repo = TempGitRepo::new();
    let outside = std::env::temp_dir().join("legion_project_git_outside.txt");
    fs::write(&outside, conflict_marker_text()).expect("write outside");
    let err = resolve_git_conflict(
        repo.path(),
        outside.to_string_lossy().as_ref(),
        GitConflictChoice::AcceptCurrent,
    )
    .expect_err("should fail for outside path");
    assert!(err.to_string().contains("outside"));
}

#[test]
fn git_conflict_rejects_malformed_markers() {
    let repo = TempGitRepo::new();
    let content = "<<<<<<< ours\nonly current\n>>>>>>> theirs\n";
    create_unmerged_state(&repo, &[("src/bad.rs", content)]);
    let err = resolve_git_conflict(repo.path(), "src/bad.rs", GitConflictChoice::AcceptCurrent)
        .expect_err("should fail for malformed markers");
    assert!(err.to_string().contains("malformed"));
}

#[test]
fn git_conflict_preserves_crlf_line_endings() {
    let repo = TempGitRepo::new();
    let crlf_content = format!(
        "header\r\n{current} ours\r\nfn current() {{}}\r\n{sep}\r\nfn incoming() {{}}\r\n{incoming} theirs\r\nfooter\r\n",
        current = "<".repeat(7),
        sep = "=".repeat(7),
        incoming = ">".repeat(7),
    );
    create_unmerged_state(
        &repo,
        &[
            ("src/crlf.rs", &crlf_content),
            ("src/crlf_incoming.rs", &crlf_content),
        ],
    );

    resolve_git_conflict(repo.path(), "src/crlf.rs", GitConflictChoice::AcceptCurrent)
        .expect("resolve current should succeed");
    let resolved = fs::read_to_string(repo.path().join("src/crlf.rs")).expect("should read");
    assert!(resolved.contains("fn current()"));
    assert!(!resolved.contains("fn incoming()"));
    assert!(!resolved.contains("<<<<<<<"));
    assert!(!resolved.contains("======="));
    assert!(!resolved.contains(">>>>>>>"));
    assert!(resolved.contains("header\r\n"));
    assert!(resolved.contains("footer\r\n"));
    assert!(
        !resolved.contains('\n') || resolved.contains("\r\n"),
        "CRLF line endings should be preserved"
    );

    resolve_git_conflict(
        repo.path(),
        "src/crlf_incoming.rs",
        GitConflictChoice::AcceptIncoming,
    )
    .expect("resolve incoming should succeed");
    let incoming_resolved =
        fs::read_to_string(repo.path().join("src/crlf_incoming.rs")).expect("should read");
    assert!(incoming_resolved.contains("fn incoming()"));
    assert!(!incoming_resolved.contains("fn current()"));
    assert!(incoming_resolved.contains("header\r\n"));
    assert!(incoming_resolved.contains("footer\r\n"));
}

#[test]
fn git_conflict_preserves_long_equal_lines_as_content() {
    let repo = TempGitRepo::new();
    let content = format!(
        "line1\n{current} ours\nfn current() {{}}\n====================\nfn incoming() {{}}\n{sep}\nfn incoming2() {{}}\n{incoming} theirs\nline2\n",
        current = "<".repeat(7),
        sep = "=".repeat(7),
        incoming = ">".repeat(7),
    );
    let content_incoming = format!(
        "line1\n{current} ours\nfn current() {{}}\n{sep}\nfn incoming2() {{}}\n====================\nfn incoming3() {{}}\n{incoming} theirs\nline2\n",
        current = "<".repeat(7),
        sep = "=".repeat(7),
        incoming = ">".repeat(7),
    );
    create_unmerged_state(
        &repo,
        &[
            ("src/sep_test.rs", &content),
            ("src/sep_test2.rs", &content_incoming),
        ],
    );

    resolve_git_conflict(
        repo.path(),
        "src/sep_test.rs",
        GitConflictChoice::AcceptCurrent,
    )
    .expect("resolve current should succeed");
    let resolved = fs::read_to_string(repo.path().join("src/sep_test.rs")).expect("should read");
    assert!(resolved.contains("fn current()"));
    assert!(
        resolved.contains("===================="),
        "long equal line should be preserved as content"
    );
    assert!(
        resolved.contains("fn incoming()"),
        "content before separator should be kept in current block"
    );
    assert!(!resolved.contains("fn incoming2()"));
    assert!(!resolved.contains("<<<<<<<"));
    assert!(
        !resolved
            .lines()
            .any(|l| l.trim_end_matches(['\r', '\n']) == "======="),
        "exact separator line should be removed"
    );
    assert!(!resolved.contains(">>>>>>>"));

    resolve_git_conflict(
        repo.path(),
        "src/sep_test2.rs",
        GitConflictChoice::AcceptIncoming,
    )
    .expect("resolve incoming should succeed");
    let incoming_resolved =
        fs::read_to_string(repo.path().join("src/sep_test2.rs")).expect("should read");
    assert!(incoming_resolved.contains("fn incoming2()"));
    assert!(
        incoming_resolved.contains("===================="),
        "long equal line should be preserved as content in incoming"
    );
    assert!(
        incoming_resolved.contains("fn incoming3()"),
        "content after separator should be kept in incoming block"
    );
    assert!(!incoming_resolved.contains("fn current()"));
    assert!(!incoming_resolved.contains("<<<<<<<"));
    assert!(
        !incoming_resolved
            .lines()
            .any(|l| l.trim_end_matches(['\r', '\n']) == "======="),
        "exact separator line should be removed"
    );
    assert!(!incoming_resolved.contains(">>>>>>>"));
}

#[test]
fn git_conflict_diff3_base_handling() {
    let repo = TempGitRepo::new();
    let diff3_content = format!(
        "line1\n{current} ours\nfn current() {{}}\n{base} base\nfn base() {{}}\n{sep}\nfn incoming() {{}}\n{incoming} theirs\nline2\n",
        current = "<".repeat(7),
        base = "|".repeat(7),
        sep = "=".repeat(7),
        incoming = ">".repeat(7),
    );
    create_unmerged_state(
        &repo,
        &[
            ("src/diff3_current.rs", &diff3_content),
            ("src/diff3_incoming.rs", &diff3_content),
        ],
    );

    let err = resolve_git_conflict(
        repo.path(),
        "src/diff3_current.rs",
        GitConflictChoice::AcceptCurrent,
    )
    .expect_err("accept current on diff3/base-marker blocks should fail closed");
    assert!(err.to_string().contains("base marker on current side"));
    let current_unchanged =
        fs::read_to_string(repo.path().join("src/diff3_current.rs")).expect("should read");
    assert_eq!(current_unchanged, diff3_content);

    resolve_git_conflict(
        repo.path(),
        "src/diff3_incoming.rs",
        GitConflictChoice::AcceptIncoming,
    )
    .expect("resolve incoming should succeed");
    let incoming_resolved =
        fs::read_to_string(repo.path().join("src/diff3_incoming.rs")).expect("should read");
    assert!(incoming_resolved.contains("fn incoming()"));
    assert!(
        !incoming_resolved.contains("fn base()"),
        "base text should be discarded"
    );
    assert!(
        !incoming_resolved.contains("|||||||"),
        "base marker should be discarded"
    );
    assert!(!incoming_resolved.contains("fn current()"));
    assert!(!incoming_resolved.contains("<<<<<<<"));
    assert!(!incoming_resolved.contains("======="));
    assert!(!incoming_resolved.contains(">>>>>>>"));
    assert!(incoming_resolved.contains("line1\n"));
    assert!(incoming_resolved.contains("line2\n"));
}

#[test]
fn git_conflict_rejects_marker_looking_content_before_actual_conflict() {
    let repo = TempGitRepo::new();
    let content = format!(
        "line1\n{literal} note\nunchanged docs\n{current} ours\nfn current() {{}}\n{sep}\nfn incoming() {{}}\n{end} theirs\nline2\n",
        literal = "<<<<<<<",
        current = "<<<<<<<",
        sep = "=======",
        end = ">>>>>>>"
    );
    create_unmerged_state(&repo, &[("src/lib.rs", &content)]);

    let err = resolve_git_conflict(
        repo.path(),
        Path::new("src/lib.rs"),
        GitConflictChoice::AcceptIncoming,
    )
    .expect_err("marker-looking content before a complete block must fail closed");

    assert!(
        err.to_string().contains("nested opening marker"),
        "unexpected error: {err:?}"
    );
    assert_eq!(
        std::fs::read_to_string(repo.path().join("src/lib.rs")).unwrap(),
        content
    );
}

#[test]
fn git_conflict_rejects_pipe_prefixed_current_side_as_ambiguous_base_marker() {
    let repo = TempGitRepo::new();
    let content = format!(
        "line1\n{current} ours\nfn current_before() {{}}\n{literal_base} notes\nfn current_after() {{}}\n{sep}\nfn incoming() {{}}\n{incoming} theirs\nline2\n",
        current = "<".repeat(7),
        literal_base = "|".repeat(7),
        sep = "=".repeat(7),
        incoming = ">".repeat(7),
    );
    create_unmerged_state(&repo, &[("src/current_base_marker.rs", &content)]);

    let err = resolve_git_conflict(
        repo.path(),
        "src/current_base_marker.rs",
        GitConflictChoice::AcceptCurrent,
    )
    .expect_err("pipe-prefixed current content should fail closed");
    assert!(err.to_string().contains("base marker on current side"));
    let unchanged =
        fs::read_to_string(repo.path().join("src/current_base_marker.rs")).expect("should read");
    assert_eq!(unchanged, content);
}

#[test]
fn git_conflict_rejects_complete_literal_marker_example_before_actual_conflict() {
    let repo = TempGitRepo::new();
    let literal = format!(
        "{current} example\nfn doc_current() {{}}\n{sep}\nfn doc_incoming() {{}}\n{incoming} example\n",
        current = "<".repeat(7),
        sep = "=".repeat(7),
        incoming = ">".repeat(7),
    );
    repo.write("src/lib.rs", &format!("intro\n{literal}value = base\n"));
    run_git(repo.path(), ["add", "."]);
    run_git(repo.path(), ["commit", "-m", "base"]);
    run_git(repo.path(), ["checkout", "-b", "feature"]);
    repo.write("src/lib.rs", &format!("intro\n{literal}value = incoming\n"));
    run_git(repo.path(), ["add", "."]);
    run_git(repo.path(), ["commit", "-m", "feature"]);
    run_git(repo.path(), ["checkout", "master"]);
    repo.write("src/lib.rs", &format!("intro\n{literal}value = current\n"));
    run_git(repo.path(), ["add", "."]);
    run_git(repo.path(), ["commit", "-m", "master"]);
    let merge = Command::new("git")
        .current_dir(repo.path())
        .args(["merge", "feature"])
        .output()
        .expect("git merge should run");
    assert!(!merge.status.success(), "merge should conflict");
    let conflicted = fs::read_to_string(repo.path().join("src/lib.rs")).expect("should read");
    assert!(conflicted.contains(&literal));
    assert!(conflicted.matches("<<<<<<<").count() >= 2);

    let err = resolve_git_conflict(repo.path(), "src/lib.rs", GitConflictChoice::AcceptIncoming)
        .expect_err("literal marker examples present in both stages should fail closed");

    assert!(err.to_string().contains("literal marker block"));
    let unchanged = fs::read_to_string(repo.path().join("src/lib.rs")).expect("should read");
    assert_eq!(unchanged, conflicted);
}

#[test]
fn git_conflict_rejects_ambiguous_separator_line_in_diff3_base_content() {
    let repo = TempGitRepo::new();
    let diff3_content = format!(
        "line1\n{current} ours\nfn current() {{}}\n{base} base\nfn base_before() {{}}\n{literal_sep}\nfn base_after() {{}}\n{sep}\nfn incoming() {{}}\n{incoming} theirs\nline2\n",
        current = "<".repeat(7),
        base = "|".repeat(7),
        literal_sep = "=".repeat(7),
        sep = "=".repeat(7),
        incoming = ">".repeat(7),
    );
    create_unmerged_state(&repo, &[("src/diff3_ambiguous_base.rs", &diff3_content)]);

    let err = resolve_git_conflict(
        repo.path(),
        "src/diff3_ambiguous_base.rs",
        GitConflictChoice::AcceptIncoming,
    )
    .expect_err("ambiguous diff3 separators should fail closed");
    assert!(err.to_string().contains("ambiguous conflict markers"));
    let unchanged =
        fs::read_to_string(repo.path().join("src/diff3_ambiguous_base.rs")).expect("should read");
    assert_eq!(unchanged, diff3_content);
}

#[test]
fn git_conflict_preserves_pipe_prefixed_incoming_after_separator() {
    let repo = TempGitRepo::new();
    let content = format!(
        "line1\n{current} ours\nfn current() {{}}\n{sep}\n||||||| notes\nfn incoming() {{}}\n{incoming} theirs\nline2\n",
        current = "<".repeat(7),
        sep = "=".repeat(7),
        incoming = ">".repeat(7),
    );
    create_unmerged_state(&repo, &[("src/pipe_incoming.rs", &content)]);

    resolve_git_conflict(
        repo.path(),
        "src/pipe_incoming.rs",
        GitConflictChoice::AcceptIncoming,
    )
    .expect("resolve incoming should succeed");
    let resolved =
        fs::read_to_string(repo.path().join("src/pipe_incoming.rs")).expect("should read");
    assert!(
        resolved.contains("||||||| notes"),
        "pipe-prefixed line after separator should be preserved in incoming block"
    );
    assert!(resolved.contains("fn incoming()"));
    assert!(!resolved.contains("fn current()"));
    assert!(!resolved.contains("<<<<<<<"));
    assert!(!resolved.contains("======="));
    assert!(!resolved.contains(">>>>>>>"));
    assert!(resolved.contains("line1\n"));
    assert!(resolved.contains("line2\n"));
}

#[test]
fn git_conflict_rejects_ambiguous_exact_separator_line_in_incoming_content() {
    let repo = TempGitRepo::new();
    let content = format!(
        "line1\n{current} ours\nfn current() {{}}\n{sep}\nfn incoming_before() {{}}\n{literal_sep}\nfn incoming_after() {{}}\n{incoming} theirs\nline2\n",
        current = "<".repeat(7),
        sep = "=".repeat(7),
        literal_sep = "=".repeat(7),
        incoming = ">".repeat(7),
    );
    create_unmerged_state(&repo, &[("src/exact_sep_incoming.rs", &content)]);

    let err = resolve_git_conflict(
        repo.path(),
        "src/exact_sep_incoming.rs",
        GitConflictChoice::AcceptIncoming,
    )
    .expect_err("ambiguous separators should fail closed");
    assert!(err.to_string().contains("ambiguous conflict markers"));
    let unchanged =
        fs::read_to_string(repo.path().join("src/exact_sep_incoming.rs")).expect("should read");
    assert_eq!(unchanged, content);
}

#[test]
fn git_conflict_rejects_ambiguous_exact_separator_line_in_current_content() {
    let repo = TempGitRepo::new();
    let content = format!(
        "line1\n{current} ours\nfn current_before() {{}}\n{literal_sep}\nfn current_after() {{}}\n{sep}\nfn incoming() {{}}\n{incoming} theirs\nline2\n",
        current = "<".repeat(7),
        literal_sep = "=".repeat(7),
        sep = "=".repeat(7),
        incoming = ">".repeat(7),
    );
    create_unmerged_state(&repo, &[("src/current_separator.rs", &content)]);

    let err = resolve_git_conflict(
        repo.path(),
        "src/current_separator.rs",
        GitConflictChoice::AcceptCurrent,
    )
    .expect_err("ambiguous separators should fail closed");
    assert!(err.to_string().contains("ambiguous conflict markers"));
    let unchanged =
        fs::read_to_string(repo.path().join("src/current_separator.rs")).expect("should read");
    assert_eq!(unchanged, content);
}

#[test]
fn git_conflict_rejects_ambiguous_end_marker_line_in_incoming_content() {
    let repo = TempGitRepo::new();
    let content = format!(
        "line1\n{current} ours\nfn current() {{}}\n{sep}\nfn incoming_before() {{}}\n{literal_end} notes\nfn incoming_after() {{}}\n{incoming} theirs\nline2\n",
        current = "<".repeat(7),
        sep = "=".repeat(7),
        literal_end = ">".repeat(7),
        incoming = ">".repeat(7),
    );
    create_unmerged_state(&repo, &[("src/end_marker_incoming.rs", &content)]);

    let err = resolve_git_conflict(
        repo.path(),
        "src/end_marker_incoming.rs",
        GitConflictChoice::AcceptIncoming,
    )
    .expect_err("ambiguous end-marker-looking lines should fail closed");
    assert!(err.to_string().contains("unbalanced block"));
    let unchanged =
        fs::read_to_string(repo.path().join("src/end_marker_incoming.rs")).expect("should read");
    assert_eq!(unchanged, content);
}

#[test]
fn git_conflict_rejects_no_conflict_block() {
    let repo = TempGitRepo::new();
    let original = "no conflict here\njust normal text\n";
    create_unmerged_state(&repo, &[("src/clean.rs", original)]);

    let err = resolve_git_conflict(
        repo.path(),
        "src/clean.rs",
        GitConflictChoice::AcceptCurrent,
    )
    .expect_err("should fail for file without conflict markers");
    assert!(err.to_string().contains("no conflict markers"));

    // file should remain unchanged
    let content = fs::read_to_string(repo.path().join("src/clean.rs")).expect("should read");
    assert_eq!(content, original, "file should not be modified");

    // file should not be staged by the failed resolution
    let status = run_git(repo.path(), ["status", "--porcelain", "--", "src/clean.rs"]);
    assert!(
        status.contains("U"),
        "file should remain unmerged after failed resolution: {}",
        status
    );
}

#[test]
fn git_conflict_resolves_from_subdirectory_root() {
    let repo = TempGitRepo::new();
    let subdir = repo.path().join("src");
    fs::create_dir(&subdir).expect("subdir should be created");
    let content = format!(
        "{current} ours\nfn current() {{}}\n{sep}\nfn incoming() {{}}\n{incoming} theirs\n",
        current = "<".repeat(7),
        sep = "=".repeat(7),
        incoming = ">".repeat(7),
    );
    create_unmerged_state(&repo, &[("src/subdir_conflict.rs", &content)]);

    resolve_git_conflict(
        &subdir,
        "src/subdir_conflict.rs",
        GitConflictChoice::AcceptCurrent,
    )
    .expect("resolve from subdirectory should succeed");
    let resolved =
        fs::read_to_string(repo.path().join("src/subdir_conflict.rs")).expect("should read");
    assert!(resolved.contains("fn current()"));
    assert!(!resolved.contains("fn incoming()"));
    assert!(!resolved.contains("<<<<<<<"));
    assert!(!resolved.contains("======="));
    assert!(!resolved.contains(">>>>>>>"));

    // verify git staged the file correctly from subdirectory root
    let status = run_git(
        repo.path(),
        ["status", "--porcelain", "--", "src/subdir_conflict.rs"],
    );
    assert!(
        status.starts_with("M ") || status.starts_with("A "),
        "file should be staged after resolution from subdirectory: {}",
        status
    );
}

#[test]
fn git_conflict_rejects_non_conflicted_marker_looking_file() {
    let repo = TempGitRepo::new();
    repo.write("src/clean.rs", "fn base() {}\n");
    run_git(repo.path(), ["add", "src/clean.rs"]);
    run_git(repo.path(), ["commit", "-m", "base"]);

    let marker_content = format!(
        "{} ours\nfn current() {{}}\n{}\nfn incoming() {{}}\n{} theirs\n",
        "<".repeat(7),
        "=".repeat(7),
        ">".repeat(7),
    );
    repo.write("src/clean.rs", &marker_content);

    let err = resolve_git_conflict(
        repo.path(),
        "src/clean.rs",
        GitConflictChoice::AcceptCurrent,
    )
    .expect_err("should fail for non-conflicted file");
    assert!(
        err.to_string()
            .contains("not in an unmerged conflict state"),
        "error should indicate unmerged status: {}",
        err
    );

    // File should remain unchanged
    let content = fs::read_to_string(repo.path().join("src/clean.rs")).expect("should read");
    assert_eq!(content, marker_content, "file should not be rewritten");

    // File should not be staged
    let status = run_git(repo.path(), ["status", "--porcelain", "--", "src/clean.rs"]);
    assert!(
        status.starts_with(" M") || status.starts_with("M "),
        "file should remain modified but not staged after failed resolution: {}",
        status
    );
}

#[test]
fn git_conflict_resolves_custom_marker_size() {
    let repo = TempGitRepo::new();
    let marker_len = 32;
    let current = "<".repeat(marker_len);
    let sep = "=".repeat(marker_len);
    let incoming = ">".repeat(marker_len);
    let content = format!(
        "line1\n{current} ours\nfn current() {{}}\n{sep}\nfn incoming() {{}}\n{incoming} theirs\nline2\n",
    );
    create_unmerged_state(
        &repo,
        &[("src/custom.rs", &content), ("src/custom2.rs", &content)],
    );

    resolve_git_conflict(
        repo.path(),
        "src/custom.rs",
        GitConflictChoice::AcceptCurrent,
    )
    .expect("resolve current should succeed with custom marker size");
    let resolved = fs::read_to_string(repo.path().join("src/custom.rs")).expect("should read");
    assert!(resolved.contains("fn current()"));
    assert!(!resolved.contains("fn incoming()"));
    assert!(!resolved.contains(&current));
    assert!(!resolved.contains(&sep));
    assert!(!resolved.contains(&incoming));
    assert!(resolved.contains("line1\n"));
    assert!(resolved.contains("line2\n"));

    resolve_git_conflict(
        repo.path(),
        "src/custom2.rs",
        GitConflictChoice::AcceptIncoming,
    )
    .expect("resolve incoming should succeed with custom marker size");
    let resolved2 = fs::read_to_string(repo.path().join("src/custom2.rs")).expect("should read");
    assert!(resolved2.contains("fn incoming()"));
    assert!(!resolved2.contains("fn current()"));
    assert!(!resolved2.contains(&current));
    assert!(!resolved2.contains(&sep));
    assert!(!resolved2.contains(&incoming));
    assert!(resolved2.contains("line1\n"));
    assert!(resolved2.contains("line2\n"));
}

#[test]
fn git_snapshot_projects_origin_remote_url() {
    let repo = TempGitRepo::new();
    repo.write(
        "src/lib.rs",
        "pub fn alpha() {}
",
    );
    run_git(repo.path(), ["add", "."]);
    run_git(repo.path(), ["commit", "-m", "initial"]);
    run_git(
        repo.path(),
        [
            "remote",
            "add",
            "origin",
            "git@github.com:legion/example-repo.git",
        ],
    );

    let snapshot = collect_git_snapshot(repo.path(), None, GitSnapshotOptions::default())
        .expect("snapshot should include remote metadata");
    let expected = run_git(repo.path(), ["remote", "get-url", "origin"]);
    assert_eq!(snapshot.remote_url.as_deref(), Some(expected.trim()));
}

#[test]
fn git_pull_request_url_builds_github_and_gitlab_links() {
    assert_eq!(
        git_forge_kind("git@github.com:legion/example-repo.git"),
        Some(legion_project::GitForgeKind::GitHub)
    );
    assert_eq!(
        git_pull_request_url(
            "git@github.com:legion/example-repo.git",
            "master",
            "feature/pr-flow"
        )
        .as_deref(),
        Some("https://github.com/legion/example-repo/compare/master...feature/pr-flow")
    );
    assert_eq!(
        git_forge_kind("https://gitlab.com/legion/example-repo.git"),
        Some(legion_project::GitForgeKind::GitLab)
    );
    assert_eq!(
        git_pull_request_url(
            "https://gitlab.com/legion/example-repo.git",
            "main",
            "feature/pr-flow"
        )
        .as_deref(),
        Some(
            "https://gitlab.com/legion/example-repo/-/merge_requests/new?merge_request[source_branch]=feature%2Fpr-flow&merge_request[target_branch]=main"
        )
    );
    assert_eq!(
        git_pull_request_url(
            "https://example.com/legion/example-repo.git",
            "main",
            "feature"
        ),
        None
    );
}

/// P2.F5.T3 — the SCM surface must be able to tell agent worktrees from the
/// user's own, so the classification is asserted directly on both separator
/// spellings and on the negative case.
#[test]
fn worktree_kind_distinguishes_agent_sandboxes_from_manual_worktrees() {
    // Delegated-task sandboxes are agent-owned.
    assert_eq!(
        git_worktree_kind_for_path(Path::new("/repo/target/delegated-tasks/task-abc123")),
        ProjectGitWorktreeKind::Agent
    );
    // Windows-spelled paths classify identically: a caller may hand us a
    // `PathBuf` it built itself rather than a porcelain-reported path.
    assert_eq!(
        git_worktree_kind_for_path(Path::new(r"C:\repo\target\delegated-tasks\task-abc123")),
        ProjectGitWorktreeKind::Agent
    );

    // Negative cases: none of these may be reported as agent worktrees, or the
    // user would see their own worktrees labelled as something they cannot manage.
    for manual in [
        "/repo",
        "/repo/target/debug",
        "/repo/../feature-worktree",
        "/repo/target/delegated-tasks",
        "/home/dev/tasks/task-abc123",
    ] {
        assert_eq!(
            git_worktree_kind_for_path(Path::new(manual)),
            ProjectGitWorktreeKind::Manual,
            "`{manual}` must classify as a manual worktree"
        );
    }
}

/// P2.F5.T3 — an agent worktree created inside a real repository is projected,
/// and projected as `Agent`, so it cannot be hidden from the user.
#[test]
fn git_snapshot_projects_agent_worktrees_as_agent_kind() {
    let repo = TempGitRepo::new();
    repo.write("src/lib.rs", "pub fn alpha() {}\n");
    run_git(repo.path(), ["add", "."]);
    run_git(repo.path(), ["commit", "-m", "initial"]);

    // Mirror the delegated-task sandbox layout that legion-agent creates.
    let agent_worktree = repo.path().join("target/delegated-tasks/task-visible");
    run_git(
        repo.path(),
        [
            "worktree",
            "add",
            agent_worktree.to_str().expect("utf8"),
            "-b",
            "agent-task",
        ],
    );

    let snapshot = collect_git_snapshot(repo.path(), None, GitSnapshotOptions::default())
        .expect("git snapshot should collect");

    let agent_rows: Vec<_> = snapshot
        .worktrees
        .iter()
        .filter(|worktree| worktree.kind == ProjectGitWorktreeKind::Agent)
        .collect();
    assert_eq!(
        agent_rows.len(),
        1,
        "exactly one agent worktree should be projected; got: {:?}",
        snapshot.worktrees
    );
    assert_eq!(agent_rows[0].branch_label.as_deref(), Some("agent-task"));

    // The repository root itself must not be swept up as an agent worktree.
    assert!(
        snapshot
            .worktrees
            .iter()
            .any(|worktree| worktree.kind == ProjectGitWorktreeKind::Manual),
        "the main worktree must still project as manual"
    );

    let _ = std::fs::remove_dir_all(&agent_worktree);
}

/// Staged hunks survive a working tree that exceeds the projection limit.
///
/// Unstaged hunks used to consume the whole `max_hunks` allowance before staged
/// hunks were requested at all, so a tree with more unstaged hunks than the
/// limit projected none of the index's work. No surface reading this projection
/// could show it, let alone unstage it, and a renderer cannot fix that by
/// partitioning what it receives: by then the staged hunks are already gone.
#[test]
fn staged_hunks_are_projected_when_unstaged_hunks_exceed_the_limit() {
    let repo = TempGitRepo::new();
    let root = repo.path();

    let mut seed = String::new();
    for line in 0..400 {
        seed.push_str(&format!("line {line}\n"));
    }
    repo.write("wide.txt", &seed);
    repo.write("staged.txt", "one\n");
    run_git(root, ["add", "."]);
    run_git(root, ["commit", "-m", "seed"]);

    repo.write("staged.txt", "one changed\n");
    run_git(root, ["add", "staged.txt"]);

    let mut edited = String::new();
    for line in 0..400 {
        if line % 4 == 0 && line > 0 {
            edited.push_str(&format!("line {line} CHANGED\n"));
        } else {
            edited.push_str(&format!("line {line}\n"));
        }
    }
    repo.write("wide.txt", &edited);

    let options = GitSnapshotOptions {
        max_file_bytes_for_syntactic_diff: 1024 * 1024,
        max_hunks: 8,
        max_blame_lines: 16,
        max_commits: 8,
    };
    let snapshot = collect_git_snapshot(root, None, options).expect("git snapshot should collect");

    let staged = snapshot
        .hunks
        .iter()
        .filter(|hunk| hunk.stage == GitHunkStage::Staged)
        .count();
    let unstaged = snapshot.hunks.len() - staged;
    assert!(
        unstaged > 0,
        "the fixture must produce unstaged hunks for this test to mean anything"
    );
    assert!(
        staged > 0,
        "staged hunks must reach the projection even when unstaged hunks alone would fill it; got {} hunks, all unstaged",
        snapshot.hunks.len()
    );
    assert!(
        snapshot.hunks.len() <= 8,
        "the limit must still hold, got {}",
        snapshot.hunks.len()
    );
}

/// A repository with exactly the allowance is not truncated.
///
/// `git_diff_hunks` stops at the limit it is given, so a vector of exactly
/// `max_hunks` is ambiguous by length alone. The first version of the flag read
/// that as truncation, so a caller asking for eight hunks from a repository with
/// exactly eight was told hunks had been omitted when none had — which makes a
/// public projection contract unreliable in the one direction that matters,
/// since a surface trusting it will stop showing exact counts it could show.
#[test]
fn a_snapshot_holding_every_hunk_is_not_marked_truncated() {
    let repo = TempGitRepo::new();
    let root = repo.path();

    let mut seed = String::new();
    for line in 0..40 {
        seed.push_str(&format!("line {line}\n"));
    }
    repo.write("exact.txt", &seed);
    run_git(root, ["add", "."]);
    run_git(root, ["commit", "-m", "seed"]);

    // Two widely separated edits: exactly two unstaged hunks, no staged ones.
    let mut edited = String::new();
    for line in 0..40 {
        if line == 5 || line == 30 {
            edited.push_str(&format!("line {line} CHANGED\n"));
        } else {
            edited.push_str(&format!("line {line}\n"));
        }
    }
    repo.write("exact.txt", &edited);

    let options = GitSnapshotOptions {
        max_file_bytes_for_syntactic_diff: 1024 * 1024,
        max_hunks: 2,
        max_blame_lines: 16,
        max_commits: 8,
    };
    let snapshot = collect_git_snapshot(root, None, options).expect("git snapshot should collect");

    assert_eq!(
        snapshot.hunks.len(),
        2,
        "the fixture must produce exactly the allowance"
    );
    assert!(
        !snapshot.hunks_truncated,
        "every hunk in the repository is present, so nothing was omitted"
    );
}

/// A merge resolved toward the current side still owes a commit.
///
/// The panel gates its Commit control on the snapshot. Resolving the last
/// conflict with **Use Current** can leave the index byte-identical to `HEAD`,
/// at which point porcelain status reports nothing at all -- while `MERGE_HEAD`
/// is still on disk and `git commit` would succeed and conclude the merge.
///
/// A snapshot that only counts changed files therefore tells the panel there is
/// nothing to commit in direct response to the panel's own conflict action, and
/// the repository is left mid-merge with no way to finish from that surface.
/// This is asserted against a real merge rather than a fixture because the
/// whole point is what git does with an empty index, not what a struct says.
#[test]
fn a_merge_resolved_to_the_current_side_still_reports_a_commit_to_make() {
    let repo = TempGitRepo::new();
    let root = repo.path();

    repo.write("shared.txt", "base\n");
    run_git(root, ["add", "."]);
    run_git(root, ["commit", "-m", "base"]);
    let base = run_git(root, ["rev-parse", "HEAD"]).trim().to_string();

    // One side changes the file.
    repo.write("shared.txt", "ours\n");
    run_git(root, ["add", "."]);
    run_git(root, ["commit", "-m", "ours"]);

    // The other side changes the same line, from the same base.
    run_git(root, ["checkout", "-b", "theirs", &base]);
    repo.write("shared.txt", "theirs\n");
    run_git(root, ["add", "."]);
    run_git(root, ["commit", "-m", "theirs"]);

    run_git(root, ["checkout", "-"]);
    // Conflicts, so this is expected to fail.
    let _ = std::process::Command::new("git")
        .current_dir(root)
        .args(["merge", "theirs"])
        .output()
        .expect("merge should run");

    let options = GitSnapshotOptions {
        max_file_bytes_for_syntactic_diff: 1024 * 1024,
        max_hunks: 16,
        max_blame_lines: 16,
        max_commits: 8,
    };

    let conflicted =
        collect_git_snapshot(root, None, options.clone()).expect("snapshot during conflict");
    assert!(
        conflicted.merge_awaiting_commit,
        "MERGE_HEAD exists during a conflicted merge"
    );

    // Resolve toward the current side, which is what **Use Current** does. The
    // index now matches HEAD exactly, because ours never changed.
    run_git(root, ["checkout", "--ours", "shared.txt"]);
    run_git(root, ["add", "shared.txt"]);

    let resolved =
        collect_git_snapshot(root, None, options.clone()).expect("snapshot after resolution");
    assert!(
        resolved
            .changed_files
            .iter()
            .all(|file| !file.status.starts_with('U') && !file.status.ends_with('U')),
        "no unmerged entries should remain, got {:?}",
        resolved
            .changed_files
            .iter()
            .map(|file| (&file.status, &file.path))
            .collect::<Vec<_>>()
    );
    // The case only bites when nothing is staged, so prove that first. If a
    // future git version or a changed fixture leaves an entry here, this test
    // is no longer exercising the empty-index path, and the assertion below
    // would pass for a reason unrelated to the fix.
    assert!(
        resolved
            .changed_files
            .iter()
            .all(|file| file.status.starts_with(' ') || file.status.starts_with('?')),
        "the fixture must leave nothing staged, or it is not testing the empty-index merge; \
         got {:?}",
        resolved
            .changed_files
            .iter()
            .map(|file| (&file.status, &file.path))
            .collect::<Vec<_>>()
    );
    assert!(
        resolved.merge_awaiting_commit,
        "the merge is unfinished and `git commit` would conclude it, but the snapshot says \
         there is nothing to commit. Changed files were {:?}",
        resolved
            .changed_files
            .iter()
            .map(|file| (&file.status, &file.path))
            .collect::<Vec<_>>()
    );

    // The claim above is only worth anything if git agrees, so ask it.
    let commit = std::process::Command::new("git")
        .current_dir(root)
        .args(["commit", "--no-edit"])
        .output()
        .expect("commit should run");
    assert!(
        commit.status.success(),
        "git refused the commit this snapshot said it would accept: {}",
        String::from_utf8_lossy(&commit.stderr)
    );

    let concluded = collect_git_snapshot(root, None, options).expect("snapshot after commit");
    assert!(
        !concluded.merge_awaiting_commit,
        "the merge is finished, so nothing is owed any more"
    );
}

/// A cherry-pick resolved to the current side owes nothing a commit can give.
///
/// The sibling of `a_merge_resolved_to_the_current_side_still_reports_a_commit_to_make`,
/// and the reason that flag is not simply "an operation is in progress".
///
/// With an index identical to `HEAD`, `git commit` **succeeds** for a merge and
/// **fails** for a cherry-pick — "The previous cherry-pick is now empty", exit
/// non-zero, `--allow-empty` required. An earlier version of the flag grouped
/// `CHERRY_PICK_HEAD` and `REVERT_HEAD` with `MERGE_HEAD` on the assumption they
/// behaved alike, which would have offered a Commit control whose only outcome
/// is an error.
///
/// This asserts both halves against real git, because the whole distinction is
/// a claim about what git does and nothing else can settle it.
#[test]
fn a_cherry_pick_resolved_to_the_current_side_is_not_committable() {
    let repo = TempGitRepo::new();
    let root = repo.path();

    repo.write("shared.txt", "base\n");
    run_git(root, ["add", "."]);
    run_git(root, ["commit", "-m", "base"]);
    let base = run_git(root, ["rev-parse", "HEAD"]).trim().to_string();

    repo.write("shared.txt", "ours\n");
    run_git(root, ["add", "."]);
    run_git(root, ["commit", "-m", "ours"]);

    run_git(root, ["checkout", "-b", "side", &base]);
    repo.write("shared.txt", "theirs\n");
    run_git(root, ["add", "."]);
    run_git(root, ["commit", "-m", "theirs"]);
    run_git(root, ["checkout", "-"]);

    // Expected to conflict.
    let _ = std::process::Command::new("git")
        .current_dir(root)
        .args(["cherry-pick", "side"])
        .output()
        .expect("cherry-pick should run");

    // Resolve toward the current side, leaving the index equal to HEAD.
    run_git(root, ["checkout", "--ours", "shared.txt"]);
    run_git(root, ["add", "shared.txt"]);

    let options = GitSnapshotOptions {
        max_file_bytes_for_syntactic_diff: 1024 * 1024,
        max_hunks: 16,
        max_blame_lines: 16,
        max_commits: 8,
    };
    let snapshot = collect_git_snapshot(root, None, options).expect("snapshot");

    assert!(
        !snapshot.merge_awaiting_commit,
        "a cherry-pick is not a merge awaiting a commit; offering Commit here produces a \
         button whose only outcome is an error"
    );

    // And git agrees, which is the whole point of the distinction.
    let commit = std::process::Command::new("git")
        .current_dir(root)
        .args(["commit", "--no-edit"])
        .output()
        .expect("commit should run");
    assert!(
        !commit.status.success(),
        "git accepted an empty cherry-pick commit, so this test's premise is wrong and the \
         merge-only restriction should be revisited: {}",
        String::from_utf8_lossy(&commit.stdout)
    );
}

/// A filename with spaces still projects its hunks against the right path.
///
/// Git appends metadata after a tab when a filename needs it, so a change to
/// `foo bar.txt` produces a `+++ b/foo bar.txt<TAB>` header. Keeping that
/// separator in the hunk's path made it disagree with the path porcelain status
/// reports, and the file then looked like it had no hunks at all.
///
/// That is not a cosmetic mismatch. A file believed hunkless gets a whole-path
/// Stage control *beside* its hunk controls, where one click stages every hunk
/// instead of the selected one — the exact outcome hunk-level staging exists to
/// prevent, reachable through a filename.
#[test]
fn a_filename_with_spaces_projects_hunks_under_its_real_path() {
    let repo = TempGitRepo::new();
    let root = repo.path();

    repo.write("foo bar.txt", "one\ntwo\nthree\n");
    run_git(root, ["add", "."]);
    run_git(root, ["commit", "-m", "seed"]);
    repo.write("foo bar.txt", "one\nCHANGED\nthree\n");

    let options = GitSnapshotOptions {
        max_file_bytes_for_syntactic_diff: 1024 * 1024,
        max_hunks: 16,
        max_blame_lines: 16,
        max_commits: 8,
    };
    let snapshot = collect_git_snapshot(root, None, options).expect("git snapshot");

    let hunk_paths: Vec<&str> = snapshot
        .hunks
        .iter()
        .map(|hunk| hunk.path.as_str())
        .collect();
    assert!(
        hunk_paths.contains(&"foo bar.txt"),
        "the hunk must be filed under the path status reports, got {hunk_paths:?}"
    );
    for path in &hunk_paths {
        assert!(
            !path.contains('\t'),
            "a diff header's trailing metadata leaked into the hunk path: {path:?}"
        );
    }

    // And the two halves of the projection agree, which is what the panel relies
    // on to decide a file is hunkless.
    let changed: Vec<&str> = snapshot
        .changed_files
        .iter()
        .map(|file| file.path.as_str())
        .collect();
    assert!(
        changed.contains(&"foo bar.txt"),
        "status should report the same path, got {changed:?}"
    );
}

/// A non-ASCII filename matches its own hunks.
///
/// With `core.quotePath` at its default, git renders non-ASCII bytes in a diff
/// header as C-style escapes inside quotes — `+++ "b/caf\303\251.txt"` — while
/// porcelain `-z` reports the raw bytes. The two then describe the same file
/// differently, so its hunks cannot be matched to its status row and it is
/// treated as hunkless: whole-path staging appears beside its own hunk controls,
/// where one click stages every hunk instead of the selected one.
///
/// Fixed by asking git not to quote rather than by teaching the parser to
/// unquote — one flag on every invocation, instead of a decoder that has to stay
/// correct for every escape git might emit.
#[test]
fn a_non_ascii_filename_matches_its_own_hunks() {
    let repo = TempGitRepo::new();
    let root = repo.path();

    let name = "caf\u{e9}.txt";
    repo.write(name, "one\ntwo\nthree\n");
    run_git(root, ["add", "."]);
    run_git(root, ["commit", "-m", "seed"]);
    repo.write(name, "one\nCHANGED\nthree\n");

    let options = GitSnapshotOptions {
        max_file_bytes_for_syntactic_diff: 1024 * 1024,
        max_hunks: 16,
        max_blame_lines: 16,
        max_commits: 8,
    };
    let snapshot = collect_git_snapshot(root, None, options).expect("git snapshot");

    let hunk_paths: Vec<&str> = snapshot
        .hunks
        .iter()
        .map(|hunk| hunk.path.as_str())
        .collect();
    let changed: Vec<&str> = snapshot
        .changed_files
        .iter()
        .map(|file| file.path.as_str())
        .collect();

    assert!(
        hunk_paths.contains(&name),
        "the hunk must be filed under the real filename, got {hunk_paths:?}"
    );
    assert!(
        changed.contains(&name),
        "status must report the same filename, got {changed:?}"
    );
    for path in &hunk_paths {
        assert!(
            !path.starts_with('"'),
            "a quoted path representation reached the projection: {path:?}"
        );
    }
}
