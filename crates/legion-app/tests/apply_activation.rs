use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use legion_app::AppComposition;
use legion_protocol::{
    BatchProposalPayload, CanonicalPath, CapabilityId, CorrelationId, PreviewSummary, PrincipalId,
    ProposalAffectedTarget, ProposalBatchAtomicity, ProposalBatchItem, ProposalBatchRollbackPolicy,
    ProposalId, ProposalPayload, ProposalTargetCoverage, ProposalTargetCoverageKind,
    ProposalTargetKind, ProposalVersionPreconditions, TimestampMillis, WorkspaceGeneration,
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

#[allow(dead_code)]
fn open_workspace(
    trust: WorkspaceTrustState,
) -> (AppComposition, legion_protocol::WorkspaceOpened) {
    let root = create_root();
    let mut app = AppComposition::new();
    let opened = app
        .open_workspace(
            &root,
            trust,
            PrincipalId("principal-apply-activation".to_string()),
        )
        .expect("open workspace");
    (app, opened)
}

fn batch_create_proposal(
    root: &Path,
    workspace_id: legion_protocol::WorkspaceId,
    workspace_generation: WorkspaceGeneration,
) -> WorkspaceProposal {
    let target_path = root.join("apply-activation.txt");
    let target_path = CanonicalPath(target_path.to_string_lossy().into_owned());
    let target_id = "target-create-apply-activation".to_string();

    WorkspaceProposal {
        proposal_id: ProposalId(42),
        principal: PrincipalId("principal-apply-activation".to_string()),
        capability: CapabilityId("fs.write".to_string()),
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
                payload: Box::new(ProposalPayload::CreateFile(
                    legion_protocol::CreateFileProposal {
                        path: target_path,
                        initial_content: Some("hello world".to_string()),
                    },
                )),
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
            details: vec!["trusted workspaces keep batch runtime apply enabled".to_string()],
        },
        expires_at: None,
        created_at: TimestampMillis(1),
    }
}

#[test]
fn trusted_workspace_keeps_batch_runtime_apply_enabled() {
    let root = create_root();
    let mut app = AppComposition::new();
    let opened = app
        .open_workspace(
            &root,
            WorkspaceTrustState::Trusted,
            PrincipalId("trusted-principal".to_string()),
        )
        .expect("open trusted workspace");
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
    let mut app = AppComposition::new();
    let opened = app
        .open_workspace(
            &root,
            WorkspaceTrustState::Untrusted,
            PrincipalId("untrusted-principal".to_string()),
        )
        .expect("open untrusted workspace");
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
