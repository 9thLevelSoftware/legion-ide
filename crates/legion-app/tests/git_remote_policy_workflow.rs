//! P2.F5.T2 / P2.F5.T4 — push, fetch, and pull run only after a policy decision,
//! and every attempt leaves an audit row the SCM surface can render.
//!
//! The critical property is that a denial actually *stops* the operation. Each
//! deny test therefore points the remote at a real local bare repository that a
//! push would certainly succeed against, then asserts the bare repository is
//! still empty afterwards. Without that check a test could pass simply because
//! the network was unavailable.

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

/// Returns true if a working `git` binary is available on PATH. Checked once.
fn git_available() -> bool {
    use std::sync::OnceLock;
    static AVAILABLE: OnceLock<bool> = OnceLock::new();
    *AVAILABLE.get_or_init(|| {
        Command::new("git")
            .arg("--version")
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    })
}

fn unique_temp_path(prefix: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after epoch")
        .as_nanos();
    let id = TEMP_COUNTER.fetch_add(1, Ordering::SeqCst);
    std::env::temp_dir().join(format!("{prefix}_{}_{nanos}_{id}", std::process::id()))
}

/// A work repository plus a local bare repository configured as `origin`.
struct RemotePair {
    work: PathBuf,
    bare: PathBuf,
}

impl RemotePair {
    /// Build a repository with one commit and `origin` pointing at a local bare repo.
    fn new() -> Self {
        assert!(
            git_available(),
            "git binary is not available on PATH; install git to run the \
             git_remote_policy_workflow integration tests"
        );
        let work = unique_temp_path("legion_app_remote_work");
        let bare = unique_temp_path("legion_app_remote_bare");
        fs::create_dir(&work).expect("work repo directory should be created");
        fs::create_dir(&bare).expect("bare repo directory should be created");

        run_git(&bare, &["init", "--bare"]);
        run_git(&work, &["init"]);
        run_git(&work, &["branch", "-M", "master"]);
        run_git(&work, &["config", "user.email", "legion@example.test"]);
        run_git(&work, &["config", "user.name", "Legion Test"]);
        fs::write(work.join("lib.rs"), "pub fn alpha() {}\n").expect("source should be written");
        run_git(&work, &["add", "."]);
        run_git(&work, &["commit", "-m", "initial"]);

        Self { work, bare }
    }

    /// Point `origin` at the local bare repository.
    fn use_local_origin(&self) {
        run_git(
            &self.work,
            &[
                "remote",
                "add",
                "origin",
                self.bare.to_str().expect("bare path should be utf8"),
            ],
        );
    }

    /// Point `origin` at a network host that the default air-gapped policy denies.
    fn use_network_origin(&self) {
        run_git(
            &self.work,
            &[
                "remote",
                "add",
                "origin",
                "git@github.com:legion/example.git",
            ],
        );
    }

    /// Whether the bare repository has received any commit on `master`.
    ///
    /// This is the check that distinguishes "policy stopped the push" from
    /// "the push ran and happened to fail".
    fn bare_has_commits(&self) -> bool {
        Command::new("git")
            .current_dir(&self.bare)
            .args(["log", "--oneline", "master"])
            .output()
            .map(|output| output.status.success() && !output.stdout.is_empty())
            .unwrap_or(false)
    }

    fn open_app(&self, trust: legion_protocol::WorkspaceTrustState) -> AppComposition {
        let mut app = AppComposition::new();
        app.open_workspace(
            &self.work,
            trust,
            legion_protocol::PrincipalId("git-remote-policy-test".to_string()),
        )
        .expect("workspace should open");
        app.open_file(self.work.join("lib.rs").to_string_lossy())
            .expect("source should open");
        // The remote verbs need a branch label, which only exists after a refresh.
        app.dispatch_ui_intent(CommandDispatchIntent::RefreshGit)
            .expect("git refresh should dispatch");
        app
    }
}

impl Drop for RemotePair {
    fn drop(&mut self) {
        for path in [&self.work, &self.bare] {
            let is_temp_child = path.starts_with(std::env::temp_dir())
                && path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with("legion_app_remote_"));
            if is_temp_child {
                let _ = fs::remove_dir_all(path);
            }
        }
    }
}

fn run_git(root: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .current_dir(root)
        .args(args)
        .output()
        .expect("git command should run");
    assert!(
        output.status.success(),
        "git {args:?} failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn dispatch_git(
    app: &mut AppComposition,
    intent: CommandDispatchIntent,
) -> legion_ui::GitProjection {
    match app
        .dispatch_ui_intent(intent)
        .expect("git remote intent should dispatch")
    {
        AppCommandOutcome::GitUpdated(projection) => projection,
        other => panic!("expected a git projection, got {other:?}"),
    }
}

#[test]
fn push_to_a_local_remote_is_allowed_and_records_an_allow_row() {
    let repo = RemotePair::new();
    repo.use_local_origin();
    let mut app = repo.open_app(legion_protocol::WorkspaceTrustState::Trusted);

    let projection = dispatch_git(
        &mut app,
        CommandDispatchIntent::PushGitRemote {
            remote: "origin".to_string(),
        },
    );

    let row = projection
        .remote_policy_audit
        .last()
        .expect("push must record a policy row");
    assert!(row.allowed, "local-path push should be allowed: {row:?}");
    assert_eq!(row.operation, "push");
    assert_eq!(row.remote, "origin");
    assert_eq!(row.target, "local-path");
    // The allow decision must have been acted on, not merely recorded.
    assert!(
        repo.bare_has_commits(),
        "an allowed push must actually reach the remote"
    );
}

#[test]
fn push_from_an_untrusted_workspace_is_denied_and_never_reaches_the_remote() {
    let repo = RemotePair::new();
    repo.use_local_origin();
    let mut app = repo.open_app(legion_protocol::WorkspaceTrustState::Untrusted);

    let projection = dispatch_git(
        &mut app,
        CommandDispatchIntent::PushGitRemote {
            remote: "origin".to_string(),
        },
    );

    let row = projection
        .remote_policy_audit
        .last()
        .expect("a denied push must still record a policy row");
    assert!(!row.allowed);
    assert!(
        row.detail.contains("trusted workspace"),
        "the row must name the reason; got: {}",
        row.detail
    );
    // This is the assertion that proves the denial had teeth: the same remote
    // accepted the push in the trusted test above.
    assert!(
        !repo.bare_has_commits(),
        "a denied push must not reach the remote"
    );
}

#[test]
fn push_to_a_network_remote_is_denied_by_the_air_gapped_default_policy() {
    let repo = RemotePair::new();
    repo.use_network_origin();
    let mut app = repo.open_app(legion_protocol::WorkspaceTrustState::Trusted);

    let projection = dispatch_git(
        &mut app,
        CommandDispatchIntent::PushGitRemote {
            remote: "origin".to_string(),
        },
    );

    let row = projection
        .remote_policy_audit
        .last()
        .expect("a denied push must still record a policy row");
    assert!(!row.allowed);
    assert_eq!(row.target, "ssh://github.com");
    assert!(row.detail.contains("air-gap"), "got: {}", row.detail);
}

#[test]
fn fetch_and_pull_are_reachable_as_intents_and_are_policy_gated() {
    // Before this task `fetch_git_remote` and `pull_git_remote` existed in the
    // engine with no intent, no request, and no caller. This test exists to keep
    // them wired: it fails if either intent stops routing.
    let repo = RemotePair::new();
    repo.use_network_origin();
    let mut app = repo.open_app(legion_protocol::WorkspaceTrustState::Trusted);

    for (intent, expected_label) in [
        (
            CommandDispatchIntent::FetchGitRemote {
                remote: "origin".to_string(),
            },
            "fetch",
        ),
        (
            CommandDispatchIntent::PullGitRemote {
                remote: "origin".to_string(),
            },
            "pull",
        ),
    ] {
        let projection = dispatch_git(&mut app, intent);
        let row = projection
            .remote_policy_audit
            .last()
            .expect("every remote verb must record a policy row");
        assert_eq!(row.operation, expected_label);
        assert!(
            !row.allowed,
            "air-gapped policy should deny {expected_label}"
        );
    }
}

#[test]
fn fetch_from_a_local_remote_is_allowed_and_runs() {
    let repo = RemotePair::new();
    repo.use_local_origin();
    let mut app = repo.open_app(legion_protocol::WorkspaceTrustState::Trusted);

    // Publish a commit so the bare repo has something to fetch back.
    dispatch_git(
        &mut app,
        CommandDispatchIntent::PushGitRemote {
            remote: "origin".to_string(),
        },
    );
    let projection = dispatch_git(
        &mut app,
        CommandDispatchIntent::FetchGitRemote {
            remote: "origin".to_string(),
        },
    );

    let row = projection
        .remote_policy_audit
        .last()
        .expect("fetch must record a policy row");
    assert!(row.allowed, "local-path fetch should be allowed: {row:?}");
    assert_eq!(row.operation, "fetch");
    // A fetch that actually ran leaves a remote-tracking ref behind.
    let refs = run_git(&repo.work, &["for-each-ref", "refs/remotes/origin"]);
    assert!(
        refs.contains("refs/remotes/origin/master"),
        "an allowed fetch must create the remote-tracking ref; got: {refs}"
    );
}

#[test]
fn the_audit_trail_survives_a_projection_refresh() {
    // `refresh_git_projection` rebuilds the projection from git, which would
    // erase a denial the user has not read yet.
    let repo = RemotePair::new();
    repo.use_network_origin();
    let mut app = repo.open_app(legion_protocol::WorkspaceTrustState::Trusted);

    dispatch_git(
        &mut app,
        CommandDispatchIntent::PushGitRemote {
            remote: "origin".to_string(),
        },
    );
    let refreshed = dispatch_git(&mut app, CommandDispatchIntent::RefreshGit);

    assert_eq!(
        refreshed.remote_policy_audit.len(),
        1,
        "the denial row must survive the refresh"
    );
    assert!(!refreshed.remote_policy_audit[0].allowed);
}
