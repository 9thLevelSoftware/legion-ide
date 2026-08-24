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
    sync::{
        Arc,
        atomic::{AtomicU64, AtomicUsize, Ordering},
    },
    time::{SystemTime, UNIX_EPOCH},
};

use legion_app::{AppCommandOutcome, AppComposition};
use legion_ui::CommandDispatchIntent;

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Host-shaped origin used to exercise the network path without a network.
const REWRITTEN_ORIGIN: &str = "https://git.legion.test/legion/example.git";
/// The host policy matches on for [`REWRITTEN_ORIGIN`].
const REWRITTEN_ORIGIN_HOST: &str = "git.legion.test";

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

    /// Point `origin` at a network host that git actually delivers to the local
    /// bare repository.
    ///
    /// `git remote get-url` reports the configured `https://…` URL, so policy
    /// classifies the remote as a non-loopback host and denies it by default,
    /// while `pushInsteadOf` rewrites the transport target to the bare repo. That
    /// makes it possible to prove a *granted* push physically lands without any
    /// network service, which a plain path remote cannot show because a path
    /// remote is never subject to the host checks in the first place.
    fn use_rewritten_network_origin(&self) {
        run_git(&self.work, &["remote", "add", "origin", REWRITTEN_ORIGIN]);
        run_git(
            &self.work,
            &[
                "config",
                &format!("url.{}.pushInsteadOf", self.bare.to_str().expect("utf8")),
                REWRITTEN_ORIGIN,
            ],
        );
        // Only `pushInsteadOf` — a plain `insteadOf` would also rewrite what
        // `git remote get-url` reports, which is exactly the value policy reads,
        // and the remote would classify as a local path instead of a host.
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
    let outcome = match app
        .dispatch_ui_intent(intent)
        .expect("git remote intent should dispatch")
    {
        AppCommandOutcome::GitUpdated(projection) => projection,
        other => panic!("expected a git projection, got {other:?}"),
    };
    app.drain_git_until_idle();
    app.shell_projection_snapshot("git-remote-policy")
        .map(|snapshot| snapshot.git_projection)
        .unwrap_or(outcome)
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
fn denied_remote_does_not_start_a_git_worker_job() {
    let repo = RemotePair::new();
    repo.use_network_origin();
    let calls = Arc::new(AtomicUsize::new(0));
    let runner_calls = Arc::clone(&calls);
    let runner: legion_app::GitInspectionRunner =
        Arc::new(move |generation, root, active_file, options| {
            runner_calls.fetch_add(1, Ordering::SeqCst);
            legion_project::collect_git_snapshot(root, active_file, options).map_err(|error| {
                legion_project::GitInspectionError::Parse(format!(
                    "generation {generation}: {error}"
                ))
            })
        });
    let mut app = AppComposition::new_with_git_runner_for_test(runner);
    app.open_workspace(
        &repo.work,
        legion_protocol::WorkspaceTrustState::Trusted,
        legion_protocol::PrincipalId("git-worker-denial-test".to_string()),
    )
    .expect("workspace should open");
    app.drain_git_until_idle();
    calls.store(0, Ordering::SeqCst);

    let projection = dispatch_git(
        &mut app,
        CommandDispatchIntent::PushGitRemote {
            remote: "origin".to_string(),
        },
    );
    assert!(
        !projection
            .remote_policy_audit
            .last()
            .expect("audit row")
            .allowed
    );
    assert_eq!(
        calls.load(Ordering::SeqCst),
        0,
        "denied push must not enqueue a worker snapshot"
    );
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

/// The full consent loop: denied by default, granted, push succeeds, and the
/// audit shows both decisions.
///
/// This is the test that proves the default-deny has a way out. Without the
/// grant path the first denial would be permanent, which is a missing feature
/// wearing a policy decision's clothes rather than a policy decision.
#[test]
fn a_denied_push_succeeds_after_the_user_grants_consent_for_the_host() {
    let repo = RemotePair::new();
    repo.use_rewritten_network_origin();
    let mut app = repo.open_app(legion_protocol::WorkspaceTrustState::Trusted);

    // 1. Denied by default: the host is non-loopback and the default policy is
    //    air-gapped with a localhost-only allowlist.
    let denied = dispatch_git(
        &mut app,
        CommandDispatchIntent::PushGitRemote {
            remote: "origin".to_string(),
        },
    );
    let denial = denied
        .remote_policy_audit
        .last()
        .expect("the denial must be recorded");
    assert!(!denial.allowed, "default policy should deny: {denial:?}");
    assert_eq!(denial.host.as_deref(), Some(REWRITTEN_ORIGIN_HOST));
    assert!(denial.detail.contains("air-gap"));
    assert!(
        !repo.bare_has_commits(),
        "the denied push must not have reached the remote"
    );

    // 2. The user grants consent for exactly that host.
    let granted = dispatch_git(
        &mut app,
        CommandDispatchIntent::GrantGitRemoteHost {
            host: REWRITTEN_ORIGIN_HOST.to_string(),
        },
    );
    let consent = granted
        .remote_policy_audit
        .last()
        .expect("the grant must be recorded");
    assert_eq!(consent.operation, "consent-grant");
    assert!(consent.allowed);
    assert!(consent.detail.contains("consent recorded"));

    // 3. The same push now succeeds and physically reaches the remote.
    let allowed = dispatch_git(
        &mut app,
        CommandDispatchIntent::PushGitRemote {
            remote: "origin".to_string(),
        },
    );
    let allow_row = allowed
        .remote_policy_audit
        .last()
        .expect("the allow must be recorded");
    assert_eq!(allow_row.operation, "push");
    assert!(allow_row.allowed, "consented push should be allowed");
    assert!(
        repo.bare_has_commits(),
        "the granted push must actually reach the remote"
    );

    // 4. The audit carries the whole story, in order, not just the latest verdict.
    let trail: Vec<(&str, bool)> = allowed
        .remote_policy_audit
        .iter()
        .map(|row| (row.operation.as_str(), row.allowed))
        .collect();
    assert_eq!(
        trail,
        vec![("push", false), ("consent-grant", true), ("push", true)],
        "the audit must show the denial, the grant, and the allow"
    );
}

/// Consent is only consent if it can be taken back.
#[test]
fn revoking_consent_restores_the_denial() {
    let repo = RemotePair::new();
    repo.use_rewritten_network_origin();
    let mut app = repo.open_app(legion_protocol::WorkspaceTrustState::Trusted);

    dispatch_git(
        &mut app,
        CommandDispatchIntent::GrantGitRemoteHost {
            host: REWRITTEN_ORIGIN_HOST.to_string(),
        },
    );
    dispatch_git(
        &mut app,
        CommandDispatchIntent::RevokeGitRemoteHost {
            host: REWRITTEN_ORIGIN_HOST.to_string(),
        },
    );

    let projection = dispatch_git(
        &mut app,
        CommandDispatchIntent::PushGitRemote {
            remote: "origin".to_string(),
        },
    );
    let row = projection
        .remote_policy_audit
        .last()
        .expect("push must record a row");
    assert!(!row.allowed, "revoked consent must deny again");
    assert!(
        !repo.bare_has_commits(),
        "a push denied after revocation must not reach the remote"
    );
}

/// An untrusted workspace cannot grant itself egress.
#[test]
fn consent_is_refused_in_an_untrusted_workspace() {
    let repo = RemotePair::new();
    repo.use_rewritten_network_origin();
    let mut app = repo.open_app(legion_protocol::WorkspaceTrustState::Untrusted);

    let error = app
        .dispatch_ui_intent(CommandDispatchIntent::GrantGitRemoteHost {
            host: REWRITTEN_ORIGIN_HOST.to_string(),
        })
        .expect_err("an untrusted workspace must not be able to grant consent");
    let rendered = format!("{error:?}");
    assert!(
        rendered.contains("untrusted"),
        "the refusal should name workspace trust; got: {rendered}"
    );
}

/// Consent for one host must not open a different host.
#[test]
fn consent_is_scoped_to_the_host_that_was_granted() {
    let repo = RemotePair::new();
    repo.use_network_origin();
    let mut app = repo.open_app(legion_protocol::WorkspaceTrustState::Trusted);

    dispatch_git(
        &mut app,
        CommandDispatchIntent::GrantGitRemoteHost {
            host: REWRITTEN_ORIGIN_HOST.to_string(),
        },
    );

    // origin still points at github.com, which was never granted.
    let projection = dispatch_git(
        &mut app,
        CommandDispatchIntent::PushGitRemote {
            remote: "origin".to_string(),
        },
    );
    let row = projection
        .remote_policy_audit
        .last()
        .expect("push must record a row");
    assert!(
        !row.allowed,
        "a grant for another host must not allow this one"
    );
    assert_eq!(row.host.as_deref(), Some("github.com"));
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
