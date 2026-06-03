use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use devil_project::{
    GitConflictChoice, GitDiffStrategy, GitHunkStage, GitSnapshotOptions, collect_git_snapshot,
    resolve_git_conflict, stage_git_hunk, unstage_git_hunk,
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
        run_git(&root, ["branch", "-M", "master"]);
        run_git(&root, ["config", "user.email", "devil@example.test"]);
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
    let outside = std::env::temp_dir().join("devil_project_git_outside.txt");
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
