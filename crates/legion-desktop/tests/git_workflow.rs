use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use legion_desktop::{
    bridge::{DesktopAction, DesktopAppRequest, DesktopBridgeOutput, DesktopCommandBridge},
    view::DesktopProjectionViewModel,
    workflow::{DesktopLaunchConfig, DesktopRuntime, DesktopWorkflowOutcome},
};
use legion_ui::{GitHunkStageProjection, GitRemotePolicyProjection, Shell};

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
    runtime.drain_git_until_idle();
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
    assert!(
        model
            .git_rows
            .iter()
            .any(|row| row.contains("git worktree"))
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
    runtime.drain_git_until_idle();
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
    runtime.drain_git_until_idle();

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

#[test]
fn desktop_git_workflow_pushes_current_branch_to_origin() {
    let repo = TempGitRepo::new();
    let remote_root = repo
        .path()
        .parent()
        .expect("repo parent should exist")
        .join("legion-desktop-git-remote.git");
    if remote_root.exists() {
        fs::remove_dir_all(&remote_root).expect("stale remote should be removable");
    }
    fs::create_dir_all(&remote_root).expect("remote root should be creatable");
    run_git(remote_root.as_path(), ["init", "--bare"]);

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
            remote_root.to_str().expect("utf8"),
        ],
    );

    let mut runtime = DesktopRuntime::open(DesktopLaunchConfig::new(
        repo.path().to_path_buf(),
        Some(
            repo.path()
                .join("src/lib.rs")
                .to_string_lossy()
                .into_owned(),
        ),
    ))
    .expect("desktop runtime should open git workspace");
    runtime
        .handle_action(DesktopAction::RefreshGit)
        .expect("refresh should route");
    runtime.drain_git_until_idle();

    assert_eq!(
        runtime
            .handle_action(DesktopAction::PushGitRemote)
            .expect("push should route"),
        DesktopWorkflowOutcome::GitUpdated
    );
    runtime.drain_git_until_idle();

    let pushed = run_git(&remote_root, ["log", "--oneline", "--all"]);
    assert!(
        pushed.contains("initial"),
        "remote should receive the pushed commit"
    );

    fs::remove_dir_all(&remote_root).expect("remote should be removable");
}

#[test]
fn desktop_git_workflow_translates_open_pr_url_from_remote_metadata() {
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

    let mut runtime = DesktopRuntime::open(DesktopLaunchConfig::new(
        repo.path().to_path_buf(),
        Some(
            repo.path()
                .join("src/lib.rs")
                .to_string_lossy()
                .into_owned(),
        ),
    ))
    .expect("desktop runtime should open git workspace");
    runtime
        .handle_action(DesktopAction::RefreshGit)
        .expect("refresh should route");
    runtime.drain_git_until_idle();
    let snapshot = runtime.projection_snapshot();
    let bridge = DesktopCommandBridge::new();

    assert_eq!(
        bridge.translate(DesktopAction::OpenGitPullRequestUrl, &snapshot),
        DesktopBridgeOutput::AppRequest(DesktopAppRequest::OpenExternalUrl {
            url: "https://github.com/legion/example-repo/compare/master...master".to_string(),
        })
    );
}

/// M-2: commit_validation_warnings and commit_validation_errors appear in git_rows.
///
/// The desktop `git_rows` renderer must surface both advisory warnings and
/// hard blockers from the git projection so the commit panel can display them.
#[test]
fn desktop_git_rows_includes_commit_validation_warnings_and_errors() {
    // Build a minimal snapshot using the Shell::empty helper and inject
    // commit validation state directly into the git_projection field.
    let mut snapshot = Shell::empty("git-validation-test").projection_snapshot();

    snapshot.git_projection.commit_validation_warnings =
        vec!["non-CC prefix: advisory only".to_string()];
    snapshot.git_projection.commit_validation_errors =
        vec!["git user.name is not configured".to_string()];

    let model = DesktopProjectionViewModel::from_snapshot(&snapshot);

    assert!(
        model
            .git_rows
            .iter()
            .any(|row| row.starts_with("git commit-warning:")),
        "git_rows must include a 'git commit-warning:' row when commit_validation_warnings is set; \
         got: {:?}",
        model.git_rows
    );
    assert!(
        model
            .git_rows
            .iter()
            .any(|row| row.starts_with("git commit-error:")),
        "git_rows must include a 'git commit-error:' row when commit_validation_errors is set; \
         got: {:?}",
        model.git_rows
    );
}

/// P2.F5.T1 — the gutter's diff data must be re-read, not captured once.
///
/// The stop condition for this task is "diff/blame data is read once and never
/// refreshed". A single refresh cannot distinguish a live projection from a
/// cached one, so this drives two refreshes across a second edit and asserts the
/// projection moved.
#[test]
fn desktop_git_refresh_reflects_edits_made_after_the_first_refresh() {
    let repo = TempGitRepo::new();
    let source = repo.write("src/lib.rs", "pub fn alpha() {\n    first();\n}\n");
    repo.write("src/other.rs", "pub fn gamma() {}\n");
    run_git(repo.path(), ["add", "."]);
    run_git(repo.path(), ["commit", "-m", "initial"]);
    repo.write("src/lib.rs", "pub fn alpha() {\n    first_changed();\n}\n");

    let mut runtime = DesktopRuntime::open(DesktopLaunchConfig::new(
        repo.path().to_path_buf(),
        Some(source.to_string_lossy().into_owned()),
    ))
    .expect("desktop runtime should open git workspace");

    runtime
        .handle_action(DesktopAction::RefreshGit)
        .expect("first git refresh should route");
    runtime.drain_git_until_idle();
    let first = runtime.projection_snapshot().git_projection;
    assert_eq!(
        first.changed_files.len(),
        1,
        "only src/lib.rs is modified at this point; got {:?}",
        first.changed_files
    );

    // Change the working tree behind the projection's back.
    repo.write("src/other.rs", "pub fn gamma() {\n    added();\n}\n");

    runtime
        .handle_action(DesktopAction::RefreshGit)
        .expect("second git refresh should route");
    runtime.drain_git_until_idle();
    let second = runtime.projection_snapshot().git_projection;

    assert_eq!(
        second.changed_files.len(),
        2,
        "the second refresh must observe the new edit; got {:?}",
        second.changed_files
    );
    assert!(
        second
            .changed_files
            .iter()
            .any(|file| file.path == "src/other.rs"),
        "src/other.rs must appear after the second refresh"
    );
    assert_ne!(
        first.hunks.len(),
        second.hunks.len(),
        "hunk set must change with the working tree, not stay pinned to the first read"
    );
}

/// P2.F5.T4 — a denied network operation is rendered distinctly in the SCM rows.
#[test]
fn desktop_git_rows_renders_remote_policy_verdicts() {
    let mut snapshot = Shell::empty("git-remote-policy-test").projection_snapshot();
    snapshot.git_projection.remote_policy_audit = vec![
        GitRemotePolicyProjection {
            operation: "fetch".to_string(),
            remote: "origin".to_string(),
            target: "local-path".to_string(),
            host: None,
            allowed: true,
            detail: "git fetch remote=origin target=local-path class=Network decision=allow"
                .to_string(),
        },
        GitRemotePolicyProjection {
            operation: "push".to_string(),
            remote: "origin".to_string(),
            target: "ssh://github.com".to_string(),
            host: Some("github.com".to_string()),
            allowed: false,
            detail: "git push remote=origin target=ssh://github.com class=Network \
                     decision=deny (air-gap mode denies non-loopback git push to `github.com`)"
                .to_string(),
        },
    ];

    let model = DesktopProjectionViewModel::from_snapshot(&snapshot);

    assert!(
        model
            .git_rows
            .iter()
            .any(|row| row.starts_with("git policy allowed:")),
        "an allowed verdict must be rendered; got: {:?}",
        model.git_rows
    );
    let denied = model
        .git_rows
        .iter()
        .find(|row| row.starts_with("git policy DENIED:"))
        .expect("a denied verdict must be rendered distinctly");
    assert!(
        denied.contains("air-gap"),
        "the denial reason must reach the user; got: {denied}"
    );
}

/// P2.F5.T4 — the grant is reachable from the UI and targets exactly the host
/// that was denied, so a user never has to retype it (or mistype it into
/// consenting to a different host).
#[test]
fn desktop_bridge_grants_consent_for_the_denied_host() {
    let mut snapshot = Shell::empty("git-consent-test").projection_snapshot();
    let bridge = DesktopCommandBridge::new();

    // With no denial on record there is nothing to grant.
    assert_eq!(
        bridge.translate(DesktopAction::GrantDeniedGitRemoteHost, &snapshot),
        DesktopBridgeOutput::Error(
            legion_desktop::bridge::DesktopBridgeError::MissingDeniedGitRemoteHost
        )
    );

    // An allowed row is not a reason to ask for consent either.
    snapshot.git_projection.remote_policy_audit = vec![GitRemotePolicyProjection {
        operation: "fetch".to_string(),
        remote: "origin".to_string(),
        target: "local-path".to_string(),
        host: None,
        allowed: true,
        detail: String::new(),
    }];
    assert_eq!(
        bridge.translate(DesktopAction::GrantDeniedGitRemoteHost, &snapshot),
        DesktopBridgeOutput::Error(
            legion_desktop::bridge::DesktopBridgeError::MissingDeniedGitRemoteHost
        )
    );

    // Once a denial names a host, the grant targets that host.
    snapshot
        .git_projection
        .remote_policy_audit
        .push(GitRemotePolicyProjection {
            operation: "push".to_string(),
            remote: "origin".to_string(),
            target: "ssh://github.com".to_string(),
            host: Some("github.com".to_string()),
            allowed: false,
            detail: "decision=deny (air-gap …)".to_string(),
        });
    assert_eq!(
        bridge.translate(DesktopAction::GrantDeniedGitRemoteHost, &snapshot),
        DesktopBridgeOutput::Intent(legion_ui::CommandDispatchIntent::GrantGitRemoteHost {
            host: "github.com".to_string(),
        })
    );
}

/// P2.F5.T2 — fetch and pull reach the app layer as intents.
///
/// These verbs existed in the git engine with no caller at all before this task.
#[test]
fn desktop_bridge_translates_fetch_and_pull_actions() {
    let snapshot = Shell::empty("git-remote-verbs-test").projection_snapshot();
    let bridge = DesktopCommandBridge::new();

    assert_eq!(
        bridge.translate(DesktopAction::FetchGitRemote, &snapshot),
        DesktopBridgeOutput::Intent(legion_ui::CommandDispatchIntent::FetchGitRemote {
            remote: "origin".to_string(),
        })
    );
    assert_eq!(
        bridge.translate(DesktopAction::PullGitRemote, &snapshot),
        DesktopBridgeOutput::Intent(legion_ui::CommandDispatchIntent::PullGitRemote {
            remote: "origin".to_string(),
        })
    );
}
