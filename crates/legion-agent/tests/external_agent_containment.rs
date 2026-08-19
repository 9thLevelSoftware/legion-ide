//! Containment tests for a Legion-governed external agent run (P6.F4.T2/T3).
//!
//! The stop conditions for this work are both escapes:
//!
//! * the external agent reading outside its assigned scope, and
//! * an external edit landing in the main workspace without a proposal.
//!
//! So the tests here are refusals. The one happy-path test present exists to
//! show the refusals are not achieved by refusing everything.

use std::fs;
use std::path::{Path, PathBuf};

use legion_agent::{
    ExternalAgentFilesystemAccess, ExternalAgentLog, ExternalAgentScope, ExternalAgentSession,
    ExternalEditBatchInput, ExternalWorktreeEdit, external_edits_to_proposals,
    external_logs_to_evidence_records, resolve_lease_relative_read,
};
use legion_protocol::{
    CapabilityId, CausalityId, CorrelationId, LegionToolKind, LegionWorkflowWorkerId, PrincipalId,
    ProposalId, ProposalPayload, TimestampMillis, WorkspaceId,
};

/// A main workspace with a leased worktree beside it, plus a file outside both
/// that the agent must never reach.
struct GovernedRun {
    root: PathBuf,
    main_workspace: PathBuf,
    lease: PathBuf,
    outside_secret: PathBuf,
}

impl GovernedRun {
    fn new(label: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "legion-external-{label}-{}-{:?}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let main_workspace = root.join("workspace");
        let lease = root.join("leases").join("task-external");
        let outside_secret = root.join("outside").join("secret.txt");

        fs::create_dir_all(main_workspace.join("src")).expect("main workspace");
        fs::write(main_workspace.join("src/private.rs"), "workspace secret\n")
            .expect("workspace file");
        fs::create_dir_all(lease.join("src")).expect("lease");
        fs::write(lease.join("src/lib.rs"), "pub fn scoped() {}\n").expect("lease file");
        fs::write(
            lease.join(".git"),
            "gitdir: ../../workspace/.git/worktrees/x\n",
        )
        .expect("lease git link");
        fs::create_dir_all(outside_secret.parent().expect("parent")).expect("outside dir");
        fs::write(&outside_secret, "do not read me\n").expect("outside file");

        Self {
            root,
            main_workspace,
            lease,
            outside_secret,
        }
    }

    fn scope(&self) -> ExternalAgentScope {
        ExternalAgentScope::new(
            &self.lease,
            &self.main_workspace,
            vec![LegionToolKind::Read, LegionToolKind::EditAsProposal],
        )
        .expect("lease is not the main workspace")
    }

    fn session(&self) -> ExternalAgentSession {
        ExternalAgentSession::begin(self.scope(), ExternalAgentFilesystemAccess::HostBrokered)
            .expect("host-brokered agent is admissible")
    }
}

impl Drop for GovernedRun {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[cfg(unix)]
fn make_symlink_dir(target: &Path, link: &Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(target, link)
}

/// Windows only grants `symlink_dir` with `SeCreateSymbolicLinkPrivilege`
/// (Developer Mode or elevation), which most CI and developer hosts do not
/// have — so a bare `symlink_dir` attempt makes this test silently vacuous on
/// exactly the platform it most needs to run. A directory junction is the same
/// class of reparse point for containment purposes (`canonicalize` resolves it
/// through `GetFinalPathNameByHandle` just as it does a symlink) and needs no
/// privilege, so it is the fallback rather than a skip.
#[cfg(windows)]
fn make_symlink_dir(target: &Path, link: &Path) -> std::io::Result<()> {
    if std::os::windows::fs::symlink_dir(target, link).is_ok() {
        return Ok(());
    }
    let status = std::process::Command::new("cmd")
        .arg("/C")
        .arg("mklink")
        .arg("/J")
        .arg(link)
        .arg(target)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()?;
    if status.success() {
        Ok(())
    } else {
        Err(std::io::Error::other("mklink /J failed"))
    }
}

fn batch_input() -> ExternalEditBatchInput {
    ExternalEditBatchInput {
        workspace_id: WorkspaceId(1),
        principal: PrincipalId("principal:external-agent".to_string()),
        capability: CapabilityId("fs.write".to_string()),
        correlation_id: CorrelationId(7),
        causality_id: CausalityId(uuid::Uuid::now_v7()),
        first_proposal_id: ProposalId(100),
        created_at: TimestampMillis(1_700_000_000_000),
    }
}

// ---------------------------------------------------------------------------
// P6.F4.T2 — "Stop if the external agent can read outside the assigned scope."
// ---------------------------------------------------------------------------

#[test]
fn a_read_of_an_absolute_path_outside_the_lease_is_refused() {
    let run = GovernedRun::new("abs-outside");
    let mut session = run.session();

    let error = session
        .authorize_read(&run.outside_secret)
        .expect_err("a path outside the lease must be refused");

    assert!(
        error.to_string().contains("external agent access denied"),
        "unexpected error: {error}"
    );
}

/// The main workspace is the specific outside the lease exists to exclude.
#[test]
fn a_read_of_a_main_workspace_file_is_refused() {
    let run = GovernedRun::new("main-workspace-read");
    let mut session = run.session();

    let error = session
        .authorize_read(&run.main_workspace.join("src/private.rs"))
        .expect_err("the main workspace is outside the lease");

    assert!(error.to_string().contains("access denied"));
}

/// The traversal case. This is caught only by the lease-relative resolution:
/// the delegated-task scope check compares path components lexically, so
/// `<lease>/../../outside/secret.txt` starts with `<lease>` and passes it.
#[test]
fn a_relative_traversal_out_of_the_lease_is_refused() {
    let run = GovernedRun::new("traversal");
    let mut session = run.session();

    let error = session
        .authorize_read(Path::new("../../outside/secret.txt"))
        .expect_err("traversal out of the lease must be refused");

    assert!(error.to_string().contains("access denied"));
}

/// A symlink created inside the lease that points outside it. Its spelling is
/// entirely in-lease, so only symlink-following resolution catches it.
#[test]
fn a_read_through_an_in_lease_symlink_pointing_outside_is_refused() {
    let run = GovernedRun::new("symlink-escape");
    let link = run.lease.join("escape");
    if make_symlink_dir(run.outside_secret.parent().expect("parent"), &link).is_err() {
        eprintln!("skipping: symlink creation not permitted on this host");
        return;
    }

    let mut session = run.session();
    let error = session
        .authorize_read(&link.join("secret.txt"))
        .expect_err("a symlink out of the lease must be refused");

    assert!(error.to_string().contains("access denied"));
}

/// `.git` in a leased worktree is a link file naming the main repository's git
/// directory. It is genuinely inside the lease, so the boundary check allows
/// it; only the forbidden-path list refuses it.
#[test]
fn a_read_of_the_lease_git_link_is_refused() {
    let run = GovernedRun::new("git-link");
    let mut session = run.session();

    let error = session
        .authorize_read(Path::new(".git"))
        .expect_err("the lease git link discloses the main repository");

    assert!(
        error.to_string().contains("forbidden-path"),
        "expected a forbidden-path denial, got: {error}"
    );
}

/// Leasing the main workspace itself would make every main-workspace read
/// "in scope". Refused at scope construction, before a session can exist.
#[test]
fn a_lease_that_is_the_main_workspace_is_refused() {
    let run = GovernedRun::new("lease-is-workspace");

    let error = ExternalAgentScope::new(
        &run.main_workspace,
        &run.main_workspace,
        vec![LegionToolKind::Read],
    )
    .expect_err("the main workspace must never be leased to an external agent");

    assert!(error.to_string().contains("identical to workspace root"));
}

/// Leasing an ancestor of the main workspace puts the workspace inside the
/// lease, which has the same effect by a different route.
#[test]
fn a_lease_that_contains_the_main_workspace_is_refused() {
    let run = GovernedRun::new("lease-contains-workspace");

    let error = ExternalAgentScope::new(&run.root, &run.main_workspace, vec![LegionToolKind::Read])
        .expect_err("a lease that contains the workspace exposes it");

    assert!(error.to_string().contains("ancestor of workspace root"));
}

/// An agent holding real file descriptors is not contained by a decision layer
/// it never consults. No sandbox backend confines reads today, so this refuses
/// on every platform.
#[test]
fn a_direct_filesystem_agent_without_os_read_enforcement_is_refused() {
    let run = GovernedRun::new("direct-process");

    let error = ExternalAgentSession::begin(
        run.scope(),
        ExternalAgentFilesystemAccess::DirectProcess {
            os_read_enforced: false,
        },
    )
    .expect_err("an unconfined direct-process agent must not start");

    assert!(error.to_string().contains("launch refused"));
    assert!(error.to_string().contains("does not confine its reads"));
}

/// Tool allowlist: a scope that grants reads does not thereby grant writes.
#[test]
fn a_write_from_a_read_only_scope_is_refused() {
    let run = GovernedRun::new("read-only-scope");
    let scope =
        ExternalAgentScope::new(&run.lease, &run.main_workspace, vec![LegionToolKind::Read])
            .expect("scope");
    let mut session =
        ExternalAgentSession::begin(scope, ExternalAgentFilesystemAccess::HostBrokered)
            .expect("session");

    let error = session
        .authorize_write(Path::new("src/lib.rs"))
        .expect_err("a read-only scope must not authorize writes");

    assert!(
        error
            .to_string()
            .contains("not allowed by the selected scope")
    );
}

/// A refusal that leaves no trace is a refusal nobody can review.
#[test]
fn every_refused_request_leaves_an_audit_row() {
    let run = GovernedRun::new("audit");
    let mut session = run.session();

    let _ = session.authorize_read(&run.outside_secret);
    let _ = session.authorize_read(Path::new("../../outside/secret.txt"));
    let _ = session.authorize_read(Path::new(".git"));

    let log = session.access_log();
    assert_eq!(log.len(), 3, "every decision is audited, not just the last");
    assert!(
        log.iter().all(|record| !record.allowed),
        "all three requests were refusals"
    );
    assert!(
        log.iter().all(|record| !record.reason.trim().is_empty()),
        "a refusal must record why"
    );
}

/// Non-vacuity for lease-relative resolution: the process working directory is
/// the crate directory here, not the lease. Resolving the request against the
/// process CWD — which is what `validate_containment` alone does — would place
/// `src/lib.rs` outside the lease and reject a legitimate read.
#[test]
fn an_in_lease_relative_read_resolves_against_the_lease_not_the_process_cwd() {
    let run = GovernedRun::new("lease-relative");
    let cwd = std::env::current_dir().expect("cwd");
    assert!(
        !cwd.starts_with(&run.lease),
        "this test is only meaningful when the process CWD is outside the lease"
    );

    let resolved = resolve_lease_relative_read(&run.lease, Path::new("src/lib.rs"))
        .expect("an in-lease relative path resolves");

    assert_eq!(resolved, PathBuf::from("src").join("lib.rs"));
}

/// The agent completes a scoped task: read what it was given, write inside the
/// lease, and leave with proposals rather than mutations.
#[test]
fn a_scoped_external_agent_run_reads_writes_and_proposes_without_bypassing_policy() {
    let run = GovernedRun::new("happy-path");
    let mut session = run.session();

    let read = session
        .authorize_read(Path::new("src/lib.rs"))
        .expect("an in-lease read is allowed");
    assert_eq!(read, PathBuf::from("src").join("lib.rs"));

    let edits = vec![
        ExternalWorktreeEdit {
            lease_relative_path: PathBuf::from("src/lib.rs"),
            content: "pub fn scoped() { /* patched */ }\n".to_string(),
        },
        ExternalWorktreeEdit {
            lease_relative_path: PathBuf::from("src/added.rs"),
            content: "pub fn added() {}\n".to_string(),
        },
    ];

    let proposals =
        external_edits_to_proposals(&mut session, &edits, &batch_input()).expect("proposals");

    assert_eq!(
        proposals.len(),
        edits.len(),
        "every external edit becomes exactly one proposal"
    );
    for proposal in &proposals {
        assert!(
            matches!(proposal.payload, ProposalPayload::WorkspaceEdit(_)),
            "an external edit leaves the lease only as a workspace-edit proposal"
        );
    }
    assert!(
        session
            .access_log()
            .iter()
            .filter(|record| record.allowed)
            .count()
            >= 3,
        "the read and both edit authorizations are audited"
    );
}

// ---------------------------------------------------------------------------
// P6.F4.T3 — every external edit becomes a proposal; every log a row
// ---------------------------------------------------------------------------

/// One escaping edit must not leave the other two converted. A partial result
/// is a batch where some edits are reviewable and the rest are unaccounted for.
#[test]
fn one_out_of_lease_edit_aborts_the_whole_batch() {
    let run = GovernedRun::new("batch-abort");
    let mut session = run.session();

    let edits = vec![
        ExternalWorktreeEdit {
            lease_relative_path: PathBuf::from("src/lib.rs"),
            content: "a\n".to_string(),
        },
        ExternalWorktreeEdit {
            lease_relative_path: PathBuf::from("../../outside/secret.txt"),
            content: "b\n".to_string(),
        },
        ExternalWorktreeEdit {
            lease_relative_path: PathBuf::from("src/added.rs"),
            content: "c\n".to_string(),
        },
    ];

    let error = external_edits_to_proposals(&mut session, &edits, &batch_input())
        .expect_err("an out-of-lease edit rejects the batch");

    assert!(error.to_string().contains("access denied"));
}

/// Two edits to one path would produce two proposals for one file; whichever
/// applied second would silently discard the reviewed content of the first.
#[test]
fn two_edits_to_the_same_path_are_refused() {
    let run = GovernedRun::new("dup-edit");
    let mut session = run.session();

    let edits = vec![
        ExternalWorktreeEdit {
            lease_relative_path: PathBuf::from("src/lib.rs"),
            content: "first\n".to_string(),
        },
        ExternalWorktreeEdit {
            lease_relative_path: PathBuf::from("src/lib.rs"),
            content: "second\n".to_string(),
        },
    ];

    let error = external_edits_to_proposals(&mut session, &edits, &batch_input())
        .expect_err("a repeated path rejects the batch");

    assert!(error.to_string().contains("more than once"));
}

#[test]
fn every_external_log_becomes_an_evidence_row() {
    let worker = LegionWorkflowWorkerId("worker:external".to_string());
    let logs = vec![
        ExternalAgentLog {
            label: "stdout".to_string(),
            text: "planning\n".to_string(),
        },
        ExternalAgentLog {
            label: "stderr".to_string(),
            text: "warning\n".to_string(),
        },
        ExternalAgentLog {
            label: "tool-calls".to_string(),
            text: "read src/lib.rs\n".to_string(),
        },
    ];

    let records = external_logs_to_evidence_records(&worker, &logs, TimestampMillis(1))
        .expect("every log converts");

    assert_eq!(records.len(), logs.len());
    let ids: Vec<&str> = records
        .iter()
        .map(|record| record.evidence_id.as_str())
        .collect();
    for (index, id) in ids.iter().enumerate() {
        assert!(
            !ids[index + 1..].contains(id),
            "evidence ids must be distinct or rows collapse"
        );
    }
}

/// Two logs sharing a label produce two rows with one evidence id. A consumer
/// keying by id sees one log where there were two.
#[test]
fn two_logs_sharing_a_label_are_refused() {
    let worker = LegionWorkflowWorkerId("worker:external".to_string());
    let logs = vec![
        ExternalAgentLog {
            label: "stdout".to_string(),
            text: "first\n".to_string(),
        },
        ExternalAgentLog {
            label: "stdout".to_string(),
            text: "second\n".to_string(),
        },
    ];

    let error = external_logs_to_evidence_records(&worker, &logs, TimestampMillis(1))
        .expect_err("colliding evidence ids must be refused");

    assert!(error.to_string().contains("more than once"));
}

#[test]
fn an_unlabelled_log_is_refused() {
    let worker = LegionWorkflowWorkerId("worker:external".to_string());
    let logs = vec![ExternalAgentLog {
        label: "   ".to_string(),
        text: "orphan\n".to_string(),
    }];

    let error = external_logs_to_evidence_records(&worker, &logs, TimestampMillis(1))
        .expect_err("an unlabelled log cannot be traced");

    assert!(error.to_string().contains("non-empty label"));
}

/// Evidence rows are metadata-only: the log's bytes stay in the lease.
#[test]
fn log_text_never_reaches_the_evidence_row_verbatim() {
    let worker = LegionWorkflowWorkerId("worker:external".to_string());
    let marker = "MARKER-9f3a2b-do-not-store";
    let logs = vec![ExternalAgentLog {
        label: "stdout".to_string(),
        text: format!("agent said {marker}\n"),
    }];

    let records = external_logs_to_evidence_records(&worker, &logs, TimestampMillis(1))
        .expect("log converts");

    assert_eq!(records.len(), 1);
    assert!(
        !records[0].redacted_payload_summary.contains(marker),
        "raw log text must not reach the evidence summary: {}",
        records[0].redacted_payload_summary
    );
}
