/// Task 2: Commit message and author validation.
///
/// Non-empty message + author config are hard errors; non-conventional-commits
/// prefix is a warn-only advisory that still allows the commit to proceed.
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use legion_app::{AppCommandOutcome, AppComposition};
use legion_ui::CommandDispatchIntent;

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
    fn new_with_author() -> Self {
        assert!(git_available(), "git binary required");
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        let id = TEMP_COUNTER.fetch_add(1, Ordering::SeqCst);
        let root = std::env::temp_dir().join(format!(
            "legion_commit_val_{}_{}_{}",
            std::process::id(),
            nanos,
            id
        ));
        fs::create_dir(&root).expect("temp dir");
        run_git(&root, ["init"]);
        run_git(&root, ["branch", "-M", "master"]);
        run_git(&root, ["config", "user.email", "val@test.example"]);
        run_git(&root, ["config", "user.name", "Val Test"]);
        Self { root }
    }

    fn new_without_author() -> Self {
        assert!(git_available(), "git binary required");
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        let id = TEMP_COUNTER.fetch_add(1, Ordering::SeqCst);
        let root = std::env::temp_dir().join(format!(
            "legion_commit_noauth_{}_{}_{}",
            std::process::id(),
            nanos,
            id
        ));
        fs::create_dir(&root).expect("temp dir");
        run_git(&root, ["init"]);
        run_git(&root, ["branch", "-M", "master"]);
        // Do NOT configure user.name or user.email so validation picks that up.
        // Also unset any global config by overriding with empty values.
        run_git(&root, ["config", "user.name", ""]);
        run_git(&root, ["config", "user.email", ""]);
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
        fs::write(&path, content).expect("write");
        path
    }
}

impl Drop for TempGitRepo {
    fn drop(&mut self) {
        let tmp = std::env::temp_dir();
        let file_name = self.root.file_name().and_then(|n| n.to_str());
        if self.root.starts_with(&tmp)
            && file_name.is_some_and(|n| {
                n.starts_with("legion_commit_val_") || n.starts_with("legion_commit_noauth_")
            })
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
    // Some git invocations (like config with empty values) may return non-zero.
    // We allow them here intentionally.
    let _ = status;
}

fn setup_staged_change(repo: &TempGitRepo) {
    // Create and commit an initial file, then stage a change.
    repo.write("src/lib.rs", "pub fn hello() {}\n");
    let _ = Command::new("git")
        .current_dir(repo.path())
        .args(["add", "."])
        .status();
    let _ = Command::new("git")
        .current_dir(repo.path())
        .args(["commit", "-m", "initial"])
        .status();
    repo.write("src/lib.rs", "pub fn hello() { println!(\"hi\"); }\n");
    let _ = Command::new("git")
        .current_dir(repo.path())
        .args(["add", "."])
        .status();
}

// ─── Direct validation unit tests (legion-project level) ──────────────────────

#[test]
fn commit_validation_empty_message_is_hard_error() {
    let repo = TempGitRepo::new_with_author();
    let result = legion_project::validate_commit_with_author(repo.path(), "");
    assert!(
        !result.errors.is_empty(),
        "empty message should be a hard error"
    );
    assert!(
        result.errors.iter().any(|e| e.contains("empty")),
        "error should mention 'empty', got: {:?}",
        result.errors
    );
}

#[test]
fn commit_validation_blank_message_is_hard_error() {
    let repo = TempGitRepo::new_with_author();
    let result = legion_project::validate_commit_with_author(repo.path(), "   \n   ");
    assert!(
        !result.errors.is_empty(),
        "blank message should be a hard error"
    );
}

#[test]
fn commit_validation_missing_author_is_hard_error() {
    let repo = TempGitRepo::new_without_author();
    let result = legion_project::validate_commit_with_author(repo.path(), "feat: add thing");
    // Should error on missing author (name or email)
    assert!(
        !result.errors.is_empty(),
        "missing git author should produce hard errors; got warnings={:?} errors={:?}",
        result.warnings,
        result.errors,
    );
}

#[test]
fn commit_validation_non_cc_prefix_is_warning_only() {
    let repo = TempGitRepo::new_with_author();
    let result = legion_project::validate_commit_with_author(repo.path(), "add new feature");
    // No hard errors (author is configured), but should have a CC warning.
    assert!(
        result.errors.is_empty(),
        "non-CC subject with configured author should have no hard errors; errors={:?}",
        result.errors
    );
    assert!(
        !result.warnings.is_empty(),
        "non-CC subject should produce a warning"
    );
}

#[test]
fn commit_validation_cc_prefix_is_warning_free() {
    let repo = TempGitRepo::new_with_author();
    for prefix in &["feat", "fix", "refactor", "test", "docs", "build", "chore"] {
        let msg = format!("{}: do something", prefix);
        let result = legion_project::validate_commit_with_author(repo.path(), &msg);
        assert!(
            result.errors.is_empty(),
            "CC prefix '{}' should have no hard errors; errors={:?}",
            prefix,
            result.errors
        );
        assert!(
            result.warnings.is_empty(),
            "CC prefix '{}' should have no warnings; warnings={:?}",
            prefix,
            result.warnings
        );
    }
}

// ─── App-level: commit projection contains validation warnings ─────────────────

#[test]
fn commit_validation_warnings_surfaced_in_git_projection() {
    let repo = TempGitRepo::new_with_author();
    setup_staged_change(&repo);

    let mut app = AppComposition::new();
    app.open_workspace(
        repo.path(),
        legion_protocol::WorkspaceTrustState::Trusted,
        legion_protocol::PrincipalId("val-test".to_string()),
    )
    .expect("workspace open");

    // Validate a message with no CC prefix through the intent.
    let projection = match app
        .dispatch_ui_intent(CommandDispatchIntent::ValidateGitCommitMessage {
            message: "add thing without prefix".to_string(),
        })
        .expect("validate commit should dispatch")
    {
        AppCommandOutcome::GitUpdated(p) => p,
        other => panic!("expected GitUpdated, got {other:?}"),
    };

    assert!(
        !projection.commit_validation_warnings.is_empty(),
        "non-CC message should produce warnings in projection"
    );
}

#[test]
fn commit_validation_cc_message_clears_warnings() {
    let repo = TempGitRepo::new_with_author();
    setup_staged_change(&repo);

    let mut app = AppComposition::new();
    app.open_workspace(
        repo.path(),
        legion_protocol::WorkspaceTrustState::Trusted,
        legion_protocol::PrincipalId("val-test".to_string()),
    )
    .expect("workspace open");

    let projection = match app
        .dispatch_ui_intent(CommandDispatchIntent::ValidateGitCommitMessage {
            message: "feat: add thing".to_string(),
        })
        .expect("validate commit should dispatch")
    {
        AppCommandOutcome::GitUpdated(p) => p,
        other => panic!("expected GitUpdated, got {other:?}"),
    };

    assert!(
        projection.commit_validation_warnings.is_empty(),
        "CC message should clear all validation warnings"
    );
}

#[test]
fn commit_with_empty_message_is_blocked_by_app() {
    let repo = TempGitRepo::new_with_author();
    setup_staged_change(&repo);

    let mut app = AppComposition::new();
    app.open_workspace(
        repo.path(),
        legion_protocol::WorkspaceTrustState::Trusted,
        legion_protocol::PrincipalId("val-test".to_string()),
    )
    .expect("workspace open");
    app.dispatch_ui_intent(CommandDispatchIntent::RefreshGit)
        .expect("refresh");

    let outcome = app.dispatch_ui_intent(CommandDispatchIntent::CommitGitChanges {
        message: "".to_string(),
    });
    assert!(
        outcome.is_err(),
        "committing with empty message should return an error"
    );
}
