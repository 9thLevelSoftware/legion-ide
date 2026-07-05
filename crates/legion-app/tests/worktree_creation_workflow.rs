/// Task 4: Branch/worktree creation UI.
///
/// "Git: New Branch" already exists as "git-create-branch".
/// This test covers the new "Git: New Worktree" palette command.
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
    fn new() -> Self {
        assert!(git_available(), "git required for worktree tests");
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let id = TEMP_COUNTER.fetch_add(1, Ordering::SeqCst);
        let root =
            std::env::temp_dir().join(format!("legion_wt_{}_{}_{}", std::process::id(), nanos, id));
        fs::create_dir(&root).expect("temp dir");
        run_git(&root, ["init"]);
        run_git(&root, ["branch", "-M", "master"]);
        run_git(&root, ["config", "user.email", "wt@test.example"]);
        run_git(&root, ["config", "user.name", "WT Test"]);
        Self { root }
    }

    fn path(&self) -> &Path {
        &self.root
    }

    fn write(&self, relative: &str, content: &str) -> PathBuf {
        let path = self.root.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("parent");
        }
        fs::write(&path, content).expect("write");
        path
    }
}

impl Drop for TempGitRepo {
    fn drop(&mut self) {
        let tmp = std::env::temp_dir();
        if self.root.starts_with(&tmp)
            && self
                .root
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("legion_wt_"))
        {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
}

fn run_git<const N: usize>(root: &Path, args: [&str; N]) {
    let output = Command::new("git")
        .current_dir(root)
        .args(args)
        .output()
        .expect("git command");
    assert!(
        output.status.success(),
        "git {:?} failed: {}",
        args,
        String::from_utf8_lossy(&output.stderr)
    );
}

fn make_initial_commit(repo: &TempGitRepo) {
    repo.write("README.md", "# test\n");
    run_git(repo.path(), ["add", "."]);
    run_git(repo.path(), ["commit", "-m", "initial"]);
}

#[test]
fn create_git_branch_via_intent_succeeds() {
    let repo = TempGitRepo::new();
    make_initial_commit(&repo);

    let mut app = AppComposition::new();
    app.open_workspace(
        repo.path(),
        legion_protocol::WorkspaceTrustState::Trusted,
        legion_protocol::PrincipalId("wt-test".to_string()),
    )
    .expect("workspace open");

    // Create branch via existing CreateGitBranch intent.
    let result = app.dispatch_ui_intent(CommandDispatchIntent::CreateGitBranch {
        branch: "feature/new-branch".to_string(),
    });
    assert!(
        result.is_ok(),
        "creating a branch should succeed: {:?}",
        result.err()
    );
}

#[test]
fn create_git_worktree_via_intent_creates_directory() {
    let repo = TempGitRepo::new();
    make_initial_commit(&repo);

    // Create a feature branch first.
    run_git(repo.path(), ["branch", "feature/wt-test"]);

    let worktree_path = std::env::temp_dir().join(format!("legion_wt_dir_{}", std::process::id()));

    let mut app = AppComposition::new();
    app.open_workspace(
        repo.path(),
        legion_protocol::WorkspaceTrustState::Trusted,
        legion_protocol::PrincipalId("wt-test".to_string()),
    )
    .expect("workspace open");

    let result = app.dispatch_ui_intent(CommandDispatchIntent::CreateGitWorktree {
        branch: "feature/wt-test".to_string(),
        worktree_path: worktree_path.to_string_lossy().to_string(),
    });

    assert!(
        result.is_ok(),
        "creating a worktree should succeed: {:?}",
        result.err()
    );

    // The worktree directory should now exist.
    assert!(
        worktree_path.exists(),
        "worktree directory should exist after CreateGitWorktree: {:?}",
        worktree_path
    );

    // Cleanup.
    let _ = Command::new("git")
        .current_dir(repo.path())
        .args([
            "worktree",
            "remove",
            "--force",
            &worktree_path.to_string_lossy(),
        ])
        .status();
    let _ = fs::remove_dir_all(&worktree_path);
}

// ─── I-4: create_git_worktree input validation ────────────────────────────────

/// I-4: Reject paths that contain `..` traversal components.
#[test]
fn create_git_worktree_rejects_dotdot_traversal() {
    let repo = TempGitRepo::new();
    make_initial_commit(&repo);
    run_git(repo.path(), ["branch", "feature/wt-reject1"]);

    let result = legion_project::create_git_worktree(
        repo.path(),
        "feature/wt-reject1",
        "../../../evil_escape",
    );
    assert!(
        matches!(
            result,
            Err(legion_project::GitInspectionError::InvalidInput(_))
        ),
        ".. traversal must be rejected with InvalidInput; got {result:?}"
    );
}

/// I-4: Reject absolute paths that fall outside the workspace parent directory.
#[test]
fn create_git_worktree_rejects_absolute_outside_parent() {
    let repo = TempGitRepo::new();
    make_initial_commit(&repo);
    run_git(repo.path(), ["branch", "feature/wt-reject2"]);

    // Build a path provably outside the workspace parent by going two levels up
    // from the repo root (grandparent > parent = allowed_parent).
    let outside_path = repo
        .path()
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace grandparent must exist")
        .join("legion_outside_test_reject");

    let result =
        legion_project::create_git_worktree(repo.path(), "feature/wt-reject2", &outside_path);
    assert!(
        matches!(
            result,
            Err(legion_project::GitInspectionError::InvalidInput(_))
        ),
        "absolute path outside workspace parent must be rejected; got {result:?}"
    );
}

/// I-4: Reject paths that already exist on disk.
#[test]
fn create_git_worktree_rejects_existing_path() {
    let repo = TempGitRepo::new();
    make_initial_commit(&repo);
    run_git(repo.path(), ["branch", "feature/wt-reject3"]);

    // Use the repo root itself — it already exists on disk.
    let result =
        legion_project::create_git_worktree(repo.path(), "feature/wt-reject3", repo.path());
    assert!(
        matches!(
            result,
            Err(legion_project::GitInspectionError::InvalidInput(_))
        ),
        "already-existing path must be rejected with InvalidInput; got {result:?}"
    );
}

// ─── Path-canonicalization regression tests ───────────────────────────────────

/// Windows regression: open the workspace via a directory junction and confirm
/// that `CreateGitWorktree` still validates containment correctly when the repo
/// root was accessed through an alias path.
///
/// Uses `mklink /J` (no elevation required).  Skipped gracefully if junction
/// creation fails (FAT volume, or restricted policy).
#[cfg(windows)]
#[test]
fn create_git_worktree_via_junction_alias_passes_containment() {
    let repo = TempGitRepo::new();
    make_initial_commit(&repo);
    run_git(repo.path(), ["branch", "feature/junc-wt"]);

    // Build a junction that points at the parent of the repo (the temp dir).
    let alias = std::env::temp_dir().join(format!(
        "legion_wt_junc_{}_{}_{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
        TEMP_COUNTER.fetch_add(1, Ordering::SeqCst)
    ));

    struct JuncGuard(PathBuf);
    impl Drop for JuncGuard {
        fn drop(&mut self) {
            let _ = Command::new("cmd")
                .args(["/C", "rmdir", &self.0.to_string_lossy()])
                .status();
        }
    }

    // Junction points to the parent dir so the repo lives at alias/<repo_name>.
    let parent = repo.path().parent().expect("repo parent");
    let junc_status = Command::new("cmd")
        .args([
            "/C",
            "mklink",
            "/J",
            &alias.to_string_lossy(),
            &parent.to_string_lossy(),
        ])
        .status();
    let junc_ok = junc_status.map(|s| s.success()).unwrap_or(false);
    if !junc_ok || !alias.exists() {
        eprintln!("SKIP: junction creation failed");
        return;
    }
    let _guard = JuncGuard(alias.clone());

    // Open workspace via the junction alias.
    let repo_name = repo
        .path()
        .file_name()
        .expect("repo dir name")
        .to_string_lossy()
        .to_string();
    let alias_repo = alias.join(&repo_name);
    let mut app = AppComposition::new();
    app.open_workspace(
        &alias_repo,
        legion_protocol::WorkspaceTrustState::Trusted,
        legion_protocol::PrincipalId("junc-wt-test".to_string()),
    )
    .expect("workspace open via junction");

    // The worktree target lives next to the (aliased) repo dir — inside the alias parent.
    let worktree_target = alias.join(format!("legion_wt_junc_target_{}", std::process::id()));

    let result = app.dispatch_ui_intent(CommandDispatchIntent::CreateGitWorktree {
        branch: "feature/junc-wt".to_string(),
        worktree_path: worktree_target.to_string_lossy().to_string(),
    });

    // Cleanup regardless of outcome.
    let _ = Command::new("git")
        .current_dir(&alias_repo)
        .args([
            "worktree",
            "remove",
            "--force",
            &worktree_target.to_string_lossy(),
        ])
        .status();
    let _ = fs::remove_dir_all(&worktree_target);

    assert!(
        result.is_ok(),
        "worktree creation via junction alias must succeed: {:?}",
        result.err()
    );
}

/// Unix regression: open workspace via a symlink and confirm containment
/// validation works when both the repo root and worktree path are accessed
/// through symlinks.
#[cfg(not(windows))]
#[test]
fn create_git_worktree_via_symlink_passes_containment() {
    let repo = TempGitRepo::new();
    make_initial_commit(&repo);
    run_git(repo.path(), ["branch", "feature/sym-wt"]);

    // Create a symlink that points to the parent dir of the repo.
    let alias = std::env::temp_dir().join(format!(
        "legion_wt_sym_{}_{}_{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
        TEMP_COUNTER.fetch_add(1, Ordering::SeqCst)
    ));

    struct SymGuard(PathBuf);
    impl Drop for SymGuard {
        fn drop(&mut self) {
            let _ = fs::remove_file(&self.0);
        }
    }

    let parent = repo.path().parent().expect("repo parent");
    if std::os::unix::fs::symlink(parent, &alias).is_err() {
        eprintln!("SKIP: symlink creation failed");
        return;
    }
    let _guard = SymGuard(alias.clone());

    let repo_name = repo
        .path()
        .file_name()
        .expect("repo dir name")
        .to_string_lossy()
        .to_string();
    let alias_repo = alias.join(&repo_name);
    let mut app = AppComposition::new();
    app.open_workspace(
        &alias_repo,
        legion_protocol::WorkspaceTrustState::Trusted,
        legion_protocol::PrincipalId("sym-wt-test".to_string()),
    )
    .expect("workspace open via symlink");

    let worktree_target = alias.join(format!("legion_wt_sym_target_{}", std::process::id()));

    let result = app.dispatch_ui_intent(CommandDispatchIntent::CreateGitWorktree {
        branch: "feature/sym-wt".to_string(),
        worktree_path: worktree_target.to_string_lossy().to_string(),
    });

    // Cleanup regardless.
    let _ = Command::new("git")
        .current_dir(&alias_repo)
        .args([
            "worktree",
            "remove",
            "--force",
            &worktree_target.to_string_lossy(),
        ])
        .status();
    let _ = fs::remove_dir_all(&worktree_target);

    assert!(
        result.is_ok(),
        "worktree creation via symlinked workspace must succeed: {:?}",
        result.err()
    );
}

#[test]
fn git_new_worktree_palette_command_exists() {
    // Verify that "Git: New Worktree" appears in the palette projection.
    let mut app = AppComposition::new();
    // Palette works without a workspace for command listing.
    let result = app
        .dispatch_ui_intent(CommandDispatchIntent::OpenPalette {
            mode: legion_ui::PaletteMode::Command,
            query: "Git: New Worktree".to_string(),
            scope: legion_ui::SearchScopeProjection::Workspace,
        })
        .expect("open palette");

    let palette = match result {
        AppCommandOutcome::PaletteUpdated(p) => p,
        other => panic!("expected PaletteUpdated, got {other:?}"),
    };

    assert!(
        palette
            .results
            .iter()
            .any(|r| r.title.contains("Worktree") || r.id.contains("worktree")),
        "palette should have a 'Git: New Worktree' command; results: {:?}",
        palette.results.iter().map(|r| &r.title).collect::<Vec<_>>()
    );
}
