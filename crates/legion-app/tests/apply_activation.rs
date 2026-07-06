use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use legion_app::AppComposition;
use legion_protocol::{
    BatchProposalPayload, CanonicalPath, CapabilityId, CorrelationId, CreateFileProposal,
    PreviewSummary, PrincipalId, ProposalAffectedTarget, ProposalBatchAtomicity,
    ProposalBatchItem, ProposalBatchRollbackPolicy, ProposalDenialReason, ProposalId,
    ProposalLifecycleState, ProposalPayload, ProposalRequest,
    ProposalResponse, ProposalTargetCoverage, ProposalTargetCoverageKind, ProposalTargetKind,
    ProposalVersionPreconditions, StorageRepositoryRequest, StorageRepositoryResponse,
    TerminalCommandProposal, TimestampMillis, WorkspaceGeneration, WorkspaceOpened,
    WorkspaceProposal, WorkspaceTrustState,
};
use legion_security::BatchRuntimeApplyPolicy;
use uuid::Uuid;

static TEMP_ROOT_COUNTER: AtomicU64 = AtomicU64::new(0);

fn create_root() -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "legion-apply-activation-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |value| value.as_millis() as u64)
            + TEMP_ROOT_COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&root).expect("create temp root");
    root
}

fn open_trusted_workspace(root: &Path) -> (AppComposition, WorkspaceOpened) {
    let mut app = AppComposition::new();
    let opened = app
        .open_workspace(
            root,
            WorkspaceTrustState::Trusted,
            PrincipalId("test-principal".to_string()),
        )
        .expect("open trusted workspace");
    (app, opened)
}

fn open_untrusted_workspace(root: &Path) -> (AppComposition, WorkspaceOpened) {
    let mut app = AppComposition::new();
    let opened = app
        .open_workspace(
            root,
            WorkspaceTrustState::Untrusted,
            PrincipalId("test-principal".to_string()),
        )
        .expect("open untrusted workspace");
    (app, opened)
}

fn batch_create_proposal(
    root: &Path,
    workspace_id: legion_protocol::WorkspaceId,
    workspace_generation: WorkspaceGeneration,
) -> WorkspaceProposal {
    batch_create_proposal_with_capability(root, workspace_id, workspace_generation, "fs.write")
}

fn batch_create_proposal_with_capability(
    root: &Path,
    workspace_id: legion_protocol::WorkspaceId,
    workspace_generation: WorkspaceGeneration,
    envelope_capability: &str,
) -> WorkspaceProposal {
    let target_path = root.join("apply-activation.txt");
    let target_path = CanonicalPath(target_path.to_string_lossy().into_owned());
    let target_id = "target-create-apply-activation".to_string();

    WorkspaceProposal {
        proposal_id: ProposalId(42),
        principal: PrincipalId("test-principal".to_string()),
        capability: CapabilityId(envelope_capability.to_string()),
        correlation_id: CorrelationId(42),
        payload: ProposalPayload::Batch(BatchProposalPayload {
            batch_id: Uuid::from_u128(42),
            atomicity: ProposalBatchAtomicity::PrepareAllBeforeMutate,
            rollback_policy: ProposalBatchRollbackPolicy::NotRequired,
            target_coverage: ProposalTargetCoverage {
                coverage_kind: ProposalTargetCoverageKind::Complete,
                targets: vec![ProposalAffectedTarget {
                    target_id: target_id.clone(),
                    kind: ProposalTargetKind::PathOnly,
                    workspace_id: Some(workspace_id),
                    file_id: None,
                    buffer_id: None,
                    path: Some(target_path.clone()),
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
            items: vec![ProposalBatchItem {
                order: 0,
                item_id: target_id,
                payload: Box::new(ProposalPayload::CreateFile(CreateFileProposal {
                    path: target_path,
                    initial_content: Some("hello world".to_string()),
                })),
                target_ids: vec!["target-create-apply-activation".to_string()],
                required_capability: CapabilityId("fs.write".to_string()),
                rollback_step_ids: vec![],
            }],
            dependency_edges: vec![],
            rollback_steps: vec![],
            partial_failures: vec![],
            preview_warnings: vec![],
            schema_version: 1,
        }),
        preconditions: ProposalVersionPreconditions {
            file_version: None,
            buffer_version: None,
            snapshot_id: None,
            generation: None,
            file_content_version: None,
            workspace_generation: Some(workspace_generation),
            expected_fingerprint: None,
            expected_file_length: None,
            expected_modified_at: None,
        },
        preview: PreviewSummary {
            summary: "apply activation smoke test".to_string(),
            details: vec!["apply gate test".to_string()],
        },
        expires_at: None,
        created_at: TimestampMillis(1),
    }
}

/// Build a CreateFile proposal with the given `proposal_id`, `capability`, target path,
/// and workspace generation. CreateFile proposals require `fs.write` capability and
/// workspace generation preconditions to pass validation.
fn create_file_proposal(
    proposal_id: u64,
    target_path: CanonicalPath,
    workspace_generation: WorkspaceGeneration,
) -> WorkspaceProposal {
    WorkspaceProposal {
        proposal_id: ProposalId(proposal_id),
        principal: PrincipalId("test-principal".to_string()),
        capability: CapabilityId("fs.write".to_string()),
        correlation_id: CorrelationId(proposal_id),
        payload: ProposalPayload::CreateFile(CreateFileProposal {
            path: target_path,
            initial_content: None,
        }),
        preconditions: ProposalVersionPreconditions {
            file_version: None,
            buffer_version: None,
            snapshot_id: None,
            generation: None,
            file_content_version: None,
            workspace_generation: Some(workspace_generation),
            expected_fingerprint: None,
            expected_file_length: None,
            expected_modified_at: None,
        },
        preview: PreviewSummary {
            summary: "apply gate test".to_string(),
            details: vec![],
        },
        expires_at: None,
        created_at: TimestampMillis(proposal_id),
    }
}

/// Register lifecycle, validate, and preview a proposal — bringing it to the
/// `Previewed` state so `apply` can be attempted.
fn register_validate_preview(app: &mut AppComposition, proposal: &WorkspaceProposal) {
    assert!(
        matches!(
            app.register_proposal_lifecycle(proposal)
                .expect("register proposal lifecycle"),
            ProposalResponse::Created(_)
        ),
        "proposal should be created"
    );
    assert!(
        matches!(
            app.handle_proposal_request(ProposalRequest::Validate(proposal.clone()))
                .expect("validate proposal"),
            ProposalResponse::Validated(_)
        ),
        "proposal should be validated; proposal_id={:?}",
        proposal.proposal_id,
    );
    assert!(
        matches!(
            app.handle_proposal_request(ProposalRequest::Preview(proposal.clone()))
                .expect("preview proposal"),
            ProposalResponse::Previewed { .. }
        ),
        "proposal should be previewed"
    );
}

/// Read the audit lifecycle state for a proposal via the storage port.
fn audit_lifecycle_state(
    app: &AppComposition,
    proposal_id: ProposalId,
) -> Option<ProposalLifecycleState> {
    match app
        .storage_port()
        .handle(StorageRepositoryRequest::ReadProposalAuditRecord(
            proposal_id,
        ))
        .expect("read proposal audit record")
    {
        StorageRepositoryResponse::ProposalAuditRecord(Some(record)) => {
            Some(record.lifecycle_state)
        }
        StorageRepositoryResponse::ProposalAuditRecord(None) => None,
        other => panic!("unexpected storage response: {other:?}"),
    }
}

// ── Existing preflight / contract / journal tests ─────────────────────────────

#[test]
fn trusted_workspace_keeps_batch_runtime_apply_enabled() {
    let root = create_root();
    let (app, opened) = open_trusted_workspace(&root);
    let proposal = batch_create_proposal(&root, opened.workspace_id, opened.generation);

    let plan = app.preflight_batch_proposal(&proposal);
    let contract = app.plan_batch_execution_contract(&proposal);
    let journal = app.plan_batch_execution_journal(&proposal);
    let policy = BatchRuntimeApplyPolicy::default();

    assert!(plan.preflight_ok, "plan: {plan:?}");
    assert!(!plan.runtime_apply_disabled);
    assert!(contract.preflight.preflight_ok, "contract: {contract:?}");
    assert!(!contract.runtime_apply_disabled);
    assert!(!journal.mutation_allowed);
    assert!(!journal.runtime_apply_disabled);
    assert!(policy.allows_workspace_trust(Some(WorkspaceTrustState::Trusted)));

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn untrusted_workspace_disables_batch_runtime_apply() {
    let root = create_root();
    let (app, opened) = open_untrusted_workspace(&root);
    let proposal = batch_create_proposal(&root, opened.workspace_id, opened.generation);

    let plan = app.preflight_batch_proposal(&proposal);
    let contract = app.plan_batch_execution_contract(&proposal);
    let journal = app.plan_batch_execution_journal(&proposal);
    let policy = BatchRuntimeApplyPolicy::default();

    assert!(plan.preflight_ok, "plan: {plan:?}");
    assert!(plan.runtime_apply_disabled);
    assert!(plan.preview_warnings.iter().any(|warning| {
        warning.code == "proposal.batch_runtime_apply_requires_trusted_workspace"
    }));
    assert!(contract.preflight.preflight_ok, "contract: {contract:?}");
    assert!(contract.runtime_apply_disabled);
    assert!(journal.runtime_apply_disabled);
    assert!(policy.runtime_apply_disabled(Some(WorkspaceTrustState::Untrusted)));

    let _ = std::fs::remove_dir_all(&root);
}

// ── BatchRuntimeApplyPolicy commit/finalize unblocking tests ─────────────────

/// An explicitly-enabled `BatchRuntimeApplyPolicy` in a Trusted workspace unblocks
/// both `commit_blocked` and `finalize_blocked` on the execution contract.
#[test]
fn trusted_workspace_with_enabled_policy_unblocks_batch_commit_finalize() {
    let root = create_root();
    let (mut app, opened) = open_trusted_workspace(&root);
    let proposal = batch_create_proposal(&root, opened.workspace_id, opened.generation);

    // Enable the policy — default is fail-closed.
    app.set_batch_apply_policy_for_test(BatchRuntimeApplyPolicy {
        enabled: true,
        max_batch_size: 100,
    });

    let contract = app.plan_batch_execution_contract(&proposal);
    assert!(
        !contract.commit_blocked,
        "trusted + enabled policy should unblock commit; contract: {contract:?}"
    );
    assert!(
        !contract.finalize_blocked,
        "trusted + enabled policy should unblock finalize; contract: {contract:?}"
    );

    let _ = std::fs::remove_dir_all(&root);
}

/// Even with an explicitly-enabled policy, an Untrusted workspace keeps both
/// `commit_blocked` and `finalize_blocked` set to `true`.
#[test]
fn untrusted_workspace_keeps_batch_commit_finalize_blocked() {
    let root = create_root();
    let (mut app, opened) = open_untrusted_workspace(&root);
    let proposal = batch_create_proposal(&root, opened.workspace_id, opened.generation);

    // Even with policy enabled, trust must be Trusted.
    app.set_batch_apply_policy_for_test(BatchRuntimeApplyPolicy {
        enabled: true,
        max_batch_size: 100,
    });

    let contract = app.plan_batch_execution_contract(&proposal);
    assert!(
        contract.commit_blocked,
        "untrusted workspace must keep commit blocked; contract: {contract:?}"
    );
    assert!(
        contract.finalize_blocked,
        "untrusted workspace must keep finalize blocked; contract: {contract:?}"
    );

    let _ = std::fs::remove_dir_all(&root);
}

// ── ProposalApplyGate tests ───────────────────────────────────────────────────

/// A `fs.write` proposal in a Trusted workspace passes the apply gate and is Applied.
#[test]
fn trusted_workspace_proposal_passes_apply_gate() {
    let root = create_root();
    let (mut app, opened) = open_trusted_workspace(&root);

    let target = root.join("gate-pass.txt");
    let canonical = CanonicalPath(target.to_string_lossy().into_owned());
    let proposal = create_file_proposal(100, canonical, opened.generation);

    register_validate_preview(&mut app, &proposal);
    let response = app
        .handle_proposal_request(ProposalRequest::Apply(proposal.clone()))
        .expect("apply proposal");

    assert!(
        matches!(response, ProposalResponse::Applied(_)),
        "trusted workspace fs.write should pass apply gate and be applied; got {response:?}"
    );

    let _ = std::fs::remove_dir_all(&root);
}

/// A `fs.write` proposal in an Untrusted workspace is denied at the apply gate and
/// an audit row with `Denied` state is recorded.
#[test]
fn untrusted_workspace_proposal_is_denied_with_audit_row() {
    let root = create_root();
    let (mut app, opened) = open_untrusted_workspace(&root);

    let target = root.join("gate-deny.txt");
    let canonical = CanonicalPath(target.to_string_lossy().into_owned());
    let proposal = create_file_proposal(200, canonical, opened.generation);

    register_validate_preview(&mut app, &proposal);
    let response = app
        .handle_proposal_request(ProposalRequest::Apply(proposal.clone()))
        .expect("apply proposal");

    assert!(
        matches!(
            response,
            ProposalResponse::Denied {
                reason: ProposalDenialReason::PolicyDenied,
                ..
            }
        ),
        "untrusted workspace fs.write should be denied; got {response:?}"
    );

    // Verify audit row was written with Denied state.
    let audit_state = audit_lifecycle_state(&app, proposal.proposal_id);
    assert_eq!(
        audit_state,
        Some(ProposalLifecycleState::Denied),
        "denied proposal must have a Denied audit record"
    );

    let _ = std::fs::remove_dir_all(&root);
}

// ── Restricted-namespace denial tests ────────────────────────────────────────
//
// These tests use batch proposals because the batch validator does not check the
// envelope capability against any required value — making it possible to get a
// proposal with plugin.*/remote.*/collaboration.* capability into Previewed state.
// The apply gate then correctly denies it via DenyByDefaultBroker.

/// A proposal with a `plugin.*` capability is denied at the apply gate with an audit row.
#[test]
fn plugin_source_proposal_apply_is_denied_with_audit() {
    let root = create_root();
    let (mut app, opened) = open_trusted_workspace(&root);

    // plugin.fs is in the restricted namespace; DenyByDefaultBroker denies it.
    let proposal = batch_create_proposal_with_capability(
        &root,
        opened.workspace_id,
        opened.generation,
        "plugin.fs",
    );

    register_validate_preview(&mut app, &proposal);
    let response = app
        .handle_proposal_request(ProposalRequest::Apply(proposal.clone()))
        .expect("apply proposal");

    assert!(
        matches!(
            response,
            ProposalResponse::Denied {
                reason: ProposalDenialReason::PolicyDenied,
                ..
            }
        ),
        "plugin.fs capability should be denied by the apply gate; got {response:?}"
    );

    let audit_state = audit_lifecycle_state(&app, proposal.proposal_id);
    assert_eq!(
        audit_state,
        Some(ProposalLifecycleState::Denied),
        "denied plugin proposal must have a Denied audit record"
    );

    let _ = std::fs::remove_dir_all(&root);
}

/// A proposal with a `remote.*` capability is denied at the apply gate with an audit row.
#[test]
fn remote_source_proposal_apply_is_denied_with_audit() {
    let root = create_root();
    let (mut app, opened) = open_trusted_workspace(&root);

    let proposal = batch_create_proposal_with_capability(
        &root,
        opened.workspace_id,
        opened.generation,
        "remote.session.connect",
    );

    register_validate_preview(&mut app, &proposal);
    let response = app
        .handle_proposal_request(ProposalRequest::Apply(proposal.clone()))
        .expect("apply proposal");

    assert!(
        matches!(
            response,
            ProposalResponse::Denied {
                reason: ProposalDenialReason::PolicyDenied,
                ..
            }
        ),
        "remote.session.connect capability should be denied by the apply gate; got {response:?}"
    );

    let audit_state = audit_lifecycle_state(&app, proposal.proposal_id);
    assert_eq!(
        audit_state,
        Some(ProposalLifecycleState::Denied),
        "denied remote proposal must have a Denied audit record"
    );

    let _ = std::fs::remove_dir_all(&root);
}

/// A proposal with a `collaboration.*` capability is denied at the apply gate with an audit row.
#[test]
fn collaboration_source_proposal_apply_is_denied_with_audit() {
    let root = create_root();
    let (mut app, opened) = open_trusted_workspace(&root);

    let proposal = batch_create_proposal_with_capability(
        &root,
        opened.workspace_id,
        opened.generation,
        "collaboration.session.create",
    );

    register_validate_preview(&mut app, &proposal);
    let response = app
        .handle_proposal_request(ProposalRequest::Apply(proposal.clone()))
        .expect("apply proposal");

    assert!(
        matches!(
            response,
            ProposalResponse::Denied {
                reason: ProposalDenialReason::PolicyDenied,
                ..
            }
        ),
        "collaboration.session.create capability should be denied by the apply gate; got {response:?}"
    );

    let audit_state = audit_lifecycle_state(&app, proposal.proposal_id);
    assert_eq!(
        audit_state,
        Some(ProposalLifecycleState::Denied),
        "denied collaboration proposal must have a Denied audit record"
    );

    let _ = std::fs::remove_dir_all(&root);
}

/// `TerminalCommand` proposals are rejected at the validate stage as Unsupported,
/// and thus can never reach the apply path (the payload-level denial is defense in depth).
#[test]
fn terminal_command_proposal_apply_is_denied_with_audit() {
    let root = create_root();
    let mut app = AppComposition::new();
    app.open_workspace(
        &root,
        WorkspaceTrustState::Trusted,
        PrincipalId("test-principal".to_string()),
    )
    .expect("open trusted workspace");

    let proposal = WorkspaceProposal {
        proposal_id: ProposalId(600),
        principal: PrincipalId("test-principal".to_string()),
        capability: CapabilityId("editor.write".to_string()),
        correlation_id: CorrelationId(600),
        payload: ProposalPayload::TerminalCommand(TerminalCommandProposal {
            session_id: None,
            command: "echo hello".to_string(),
            cwd: None,
            env: Default::default(),
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
            summary: "terminal command denial test".to_string(),
            details: vec![],
        },
        expires_at: None,
        created_at: TimestampMillis(600),
    };

    // Register lifecycle.
    assert!(
        matches!(
            app.register_proposal_lifecycle(&proposal)
                .expect("register"),
            ProposalResponse::Created(_)
        ),
        "terminal command proposal should be created"
    );

    // Validate must not return Validated for TerminalCommand proposals — they are
    // either Rejected (Unsupported) when a working-directory is provided, or Denied
    // (PolicyDenied / target validation failure) when one is not.  Either outcome
    // prevents the proposal from reaching Previewed state and therefore apply.
    let validate_response = app
        .handle_proposal_request(ProposalRequest::Validate(proposal.clone()))
        .expect("validate");
    assert!(
        !matches!(validate_response, ProposalResponse::Validated(_)),
        "TerminalCommand should NOT be validated; got {validate_response:?}"
    );

    // After a non-Validated outcome the lifecycle state is Rejected or Denied —
    // apply from that state must also be blocked.
    let apply_response = app
        .handle_proposal_request(ProposalRequest::Apply(proposal.clone()))
        .expect("apply");
    assert!(
        !matches!(apply_response, ProposalResponse::Applied(_)),
        "TerminalCommand proposal must never reach Applied; got {apply_response:?}"
    );

    // Audit record exists — lifecycle event was recorded.
    let audit_state = audit_lifecycle_state(&app, proposal.proposal_id);
    assert!(
        audit_state.is_some(),
        "terminal-command proposal must have an audit record"
    );
    assert_ne!(
        audit_state,
        Some(ProposalLifecycleState::Applied),
        "terminal-command proposal audit must not show Applied"
    );

    let _ = std::fs::remove_dir_all(&root);
}
