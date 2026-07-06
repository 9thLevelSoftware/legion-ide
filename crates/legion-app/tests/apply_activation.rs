use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use legion_app::AppComposition;
use legion_protocol::{
    BatchProposalPayload, CanonicalPath, CapabilityId, CorrelationId, CreateFileProposal,
    FileTreeNode, PreviewSummary, PrincipalId, ProposalAffectedTarget, ProposalBatchAtomicity,
    ProposalBatchItem, ProposalBatchRollbackPolicy, ProposalDenialReason, ProposalId,
    ProposalLifecycleState, ProposalPayload, ProposalRequest, ProposalResponse,
    ProposalTargetCoverage, ProposalTargetCoverageKind, ProposalTargetKind,
    ProposalVersionPreconditions, RenameFileProposal, StorageRepositoryRequest,
    StorageRepositoryResponse, TerminalCommandProposal, TimestampMillis, WorkspaceGeneration,
    WorkspaceId, WorkspaceOpened, WorkspacePort, WorkspaceProposal, WorkspaceRequest,
    WorkspaceResponse, WorkspaceTrustState,
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

/// Read the workspace tree for a workspace.
fn workspace_tree(app: &AppComposition, workspace_id: WorkspaceId) -> Vec<FileTreeNode> {
    match app
        .workspace()
        .handle(WorkspaceRequest::ReadTree(workspace_id))
        .expect("read workspace tree")
    {
        WorkspaceResponse::Tree(tree) => tree,
        other => panic!("expected workspace tree, got {other:?}"),
    }
}

/// Find a node in the workspace tree by filename.
fn workspace_node_by_name(
    app: &AppComposition,
    workspace_id: WorkspaceId,
    name: &str,
) -> FileTreeNode {
    workspace_tree(app, workspace_id)
        .into_iter()
        .find(|node| node.name == name)
        .unwrap_or_else(|| panic!("workspace node '{name}' not found"))
}

/// Build a `ProposalVersionPreconditions` from a workspace tree node.
fn file_preconditions(
    node: &FileTreeNode,
    workspace_generation: WorkspaceGeneration,
) -> ProposalVersionPreconditions {
    let fingerprint = node
        .metadata
        .as_ref()
        .and_then(|m| m.fingerprint.clone())
        .expect("file node fingerprint");
    ProposalVersionPreconditions {
        file_version: Some(node.identity.content_version),
        buffer_version: None,
        snapshot_id: None,
        generation: Some(workspace_generation),
        file_content_version: Some(node.identity.content_version),
        workspace_generation: Some(workspace_generation),
        expected_fingerprint: Some(fingerprint),
        expected_file_length: node.metadata.as_ref().and_then(|m| m.size_bytes),
        expected_modified_at: None,
    }
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

// ── Finding 1: approve_and_apply_rename_proposal end-to-end test ─────────────

/// Full lifecycle for `approve_and_apply_rename_proposal`:
/// open workspace → create proposal in Previewed state → call the method →
/// assert Applied → verify the file was physically renamed on disk.
#[test]
fn approve_and_apply_rename_proposal_applies_and_renames_file() {
    let root = create_root();
    let source_path = root.join("rename-source.txt");
    let dest_path = root.join("rename-dest.txt");
    std::fs::write(&source_path, "rename-me").expect("seed source file");

    let (mut app, opened) = open_trusted_workspace(&root);

    // Get the workspace tree node for the source file so we have its identity.
    let node = workspace_node_by_name(&app, opened.workspace_id, "rename-source.txt");

    // Build a RenameFile proposal using the scanned file identity.
    let proposal = WorkspaceProposal {
        proposal_id: ProposalId(900),
        principal: PrincipalId("test-principal".to_string()),
        capability: CapabilityId("fs.write".to_string()),
        correlation_id: CorrelationId(900),
        payload: ProposalPayload::RenameFile(RenameFileProposal {
            file: node.identity.clone(),
            destination: CanonicalPath(dest_path.to_string_lossy().into_owned()),
        }),
        preconditions: file_preconditions(&node, opened.generation),
        preview: PreviewSummary {
            summary: "rename source to dest".to_string(),
            details: vec![],
        },
        expires_at: None,
        created_at: TimestampMillis(900),
    };

    // Bring the proposal to Previewed state via the normal lifecycle path.
    register_validate_preview(&mut app, &proposal);

    // Call the method under test.
    let response = app
        .approve_and_apply_rename_proposal(proposal.proposal_id)
        .expect("approve_and_apply_rename_proposal must not error");

    assert!(
        matches!(response, ProposalResponse::Applied(_)),
        "approve_and_apply_rename_proposal should return Applied; got {response:?}"
    );

    // Verify the rename actually happened on disk.
    assert!(
        !source_path.exists(),
        "source file must no longer exist after rename"
    );
    assert!(
        dest_path.exists(),
        "destination file must exist after rename"
    );
    assert_eq!(
        std::fs::read_to_string(&dest_path).expect("read dest"),
        "rename-me",
        "file content must be preserved after rename"
    );

    let _ = std::fs::remove_dir_all(&root);
}

// ── Finding 2: BatchRuntimeApplyPolicy production activation path test ────────

/// Opening a Trusted workspace must automatically enable the batch runtime apply
/// policy so that `plan_batch_execution_contract` returns `commit_blocked: false`
/// and `finalize_blocked: false` without requiring a separate test-only
/// `set_batch_apply_policy_for_test` call.
#[test]
fn open_trusted_workspace_enables_batch_policy_for_production() {
    let root = create_root();
    let (app, opened) = open_trusted_workspace(&root);
    let proposal = batch_create_proposal(&root, opened.workspace_id, opened.generation);

    let contract = app.plan_batch_execution_contract(&proposal);

    assert!(
        !contract.commit_blocked,
        "trusted workspace open must unblock commit without manual policy override; \
         contract: {contract:?}"
    );
    assert!(
        !contract.finalize_blocked,
        "trusted workspace open must unblock finalize without manual policy override; \
         contract: {contract:?}"
    );

    let _ = std::fs::remove_dir_all(&root);
}

// ── Finding 3: TerminalCommand defense-in-depth arm test ─────────────────────

/// The `TerminalCommand` arm in `apply_workspace_proposal` is defense-in-depth:
/// validate normally rejects it before the proposal can reach `Previewed`.
/// This test bypasses validate by force-setting the lifecycle state and confirms
/// the payload-level deny arm fires correctly.
#[test]
fn terminal_command_defense_in_depth_arm_denied_from_previewed_state() {
    let root = create_root();
    let (mut app, _opened) = open_trusted_workspace(&root);

    // Use `editor.write` so the ProposalApplyGate passes (doesn't start with
    // "terminal." / "plugin." / "remote." / "collaboration." or "fs.").
    let proposal = WorkspaceProposal {
        proposal_id: ProposalId(701),
        principal: PrincipalId("test-principal".to_string()),
        capability: CapabilityId("editor.write".to_string()),
        correlation_id: CorrelationId(701),
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
            summary: "defense-in-depth terminal test".to_string(),
            details: vec![],
        },
        expires_at: None,
        created_at: TimestampMillis(701),
    };

    // Register lifecycle context and set Created state.
    assert!(
        matches!(
            app.register_proposal_lifecycle(&proposal)
                .expect("register"),
            ProposalResponse::Created(_)
        ),
        "proposal must be created"
    );

    // Bypass validate (which would reject TerminalCommand) by forcing the
    // lifecycle state directly to Previewed.
    app.force_proposal_lifecycle_state_for_test(
        proposal.proposal_id,
        ProposalLifecycleState::Previewed,
    );

    // Now apply — must hit the defense-in-depth deny arm, not Applied.
    let response = app
        .handle_proposal_request(ProposalRequest::Apply(proposal.clone()))
        .expect("apply should not error");

    assert!(
        matches!(
            response,
            ProposalResponse::Denied {
                reason: ProposalDenialReason::PolicyDenied,
                ..
            }
        ),
        "TerminalCommand bypassing validate must be denied at payload level; got {response:?}"
    );

    // Audit record must reflect denial, not apply.
    let audit_state = audit_lifecycle_state(&app, proposal.proposal_id);
    assert_eq!(
        audit_state,
        Some(ProposalLifecycleState::Denied),
        "defense-in-depth terminal denial must produce Denied audit record"
    );

    let _ = std::fs::remove_dir_all(&root);
}
