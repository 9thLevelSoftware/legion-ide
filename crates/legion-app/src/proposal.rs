use std::collections::HashSet;

use legion_protocol::{
    ProposalPayload, ProposalTargetCoverage, ProposalTargetCoverageKind, WorkspaceProposal,
};
use legion_security::ProposalAutoApprovalPolicy;

/// Returns the risk label that the proposal coordinator uses for deterministic routing.
pub fn proposal_risk_label(
    payload: &ProposalPayload,
    target_coverage: &ProposalTargetCoverage,
) -> legion_protocol::ProposalRiskLabel {
    if target_coverage.coverage_kind != ProposalTargetCoverageKind::Complete
        || target_coverage.omitted_target_count > 0
    {
        return legion_protocol::ProposalRiskLabel::Unknown;
    }

    match payload {
        ProposalPayload::TerminalCommand(_) | ProposalPayload::DeleteFile(_) => {
            legion_protocol::ProposalRiskLabel::High
        }
        ProposalPayload::Batch(_)
        | ProposalPayload::WorkspaceEdit(_)
        | ProposalPayload::RenameFile(_)
        | ProposalPayload::CodeAction(_) => legion_protocol::ProposalRiskLabel::Medium,
        ProposalPayload::TextEdit(_)
        | ProposalPayload::CreateFile(_)
        | ProposalPayload::SaveFile(_)
        | ProposalPayload::FormatFile(_) => legion_protocol::ProposalRiskLabel::Low,
    }
}

/// Returns true when an opt-in policy may auto-approve this proposal.
pub fn proposal_auto_approval_allowed(
    policy: &ProposalAutoApprovalPolicy,
    payload: &ProposalPayload,
    target_coverage: &ProposalTargetCoverage,
) -> bool {
    if !policy.enabled {
        return false;
    }

    if proposal_risk_label(payload, target_coverage) != legion_protocol::ProposalRiskLabel::Low {
        return false;
    }

    let risk_rule_ids = proposal_risk_rule_ids_from_coverage(target_coverage);
    !risk_rule_ids.is_empty() && policy.allows_rule_ids(&risk_rule_ids)
}

/// Derives the stable deterministic rule ids that should be cited for a proposal coverage.
pub fn proposal_risk_rule_ids_from_coverage(
    target_coverage: &ProposalTargetCoverage,
) -> Vec<String> {
    if target_coverage.coverage_kind != ProposalTargetCoverageKind::Complete
        || target_coverage.omitted_target_count > 0
    {
        return Vec::new();
    }

    proposal_risk_rule_ids_from_complete_coverage()
}

/// Returns a filtered batch proposal that keeps only items whose affected targets were accepted.
///
/// The returned proposal preserves the original proposal metadata but narrows the batch payload so
/// the normal proposal apply pipeline can execute only the accepted hunks.
pub fn filtered_batch_proposal_for_accepted_targets(
    proposal: &WorkspaceProposal,
    accepted_target_ids: &HashSet<String>,
) -> Option<WorkspaceProposal> {
    let ProposalPayload::Batch(batch) = &proposal.payload else {
        return None;
    };

    if accepted_target_ids.is_empty() {
        return None;
    }

    let filtered_items = batch
        .items
        .iter()
        .filter(|item| {
            !item.target_ids.is_empty()
                && item
                    .target_ids
                    .iter()
                    .all(|target_id| accepted_target_ids.contains(target_id))
        })
        .cloned()
        .collect::<Vec<_>>();
    if filtered_items.is_empty() {
        return None;
    }

    let retained_item_ids = filtered_items
        .iter()
        .map(|item| item.item_id.clone())
        .collect::<HashSet<_>>();
    let retained_target_ids = filtered_items
        .iter()
        .flat_map(|item| item.target_ids.iter().cloned())
        .collect::<HashSet<_>>();

    let mut filtered_batch = batch.clone();
    filtered_batch.items = filtered_items;
    filtered_batch.target_coverage.targets = batch
        .target_coverage
        .targets
        .iter()
        .filter(|target| retained_target_ids.contains(&target.target_id))
        .cloned()
        .collect();
    filtered_batch.target_coverage.coverage_kind = ProposalTargetCoverageKind::Complete;
    filtered_batch.target_coverage.omitted_target_count = 0;
    filtered_batch.dependency_edges = batch
        .dependency_edges
        .iter()
        .filter(|edge| {
            retained_item_ids.contains(&edge.prerequisite_item_id)
                && retained_item_ids.contains(&edge.dependent_item_id)
        })
        .cloned()
        .collect();
    filtered_batch.rollback_steps = batch
        .rollback_steps
        .iter()
        .filter(|step| retained_item_ids.contains(&step.item_id))
        .cloned()
        .collect();
    filtered_batch.partial_failures = batch
        .partial_failures
        .iter()
        .filter(|failure| retained_item_ids.contains(&failure.item_id))
        .cloned()
        .collect();
    filtered_batch.preview_warnings = batch
        .preview_warnings
        .iter()
        .filter(|warning| {
            warning
                .target_id
                .as_ref()
                .is_none_or(|target_id| retained_target_ids.contains(target_id))
        })
        .cloned()
        .collect();

    let mut filtered_proposal = proposal.clone();
    filtered_proposal.payload = ProposalPayload::Batch(filtered_batch);
    Some(filtered_proposal)
}

fn proposal_risk_rule_ids_from_complete_coverage() -> Vec<String> {
    legion_protocol::risk::RiskRuleId::all()
        .iter()
        .map(|rule_id| rule_id.stable_id().to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use legion_protocol::{
        BatchProposalPayload, CanonicalPath, CapabilityId, CreateFileProposal, PreviewSummary,
        PrincipalId, ProposalAffectedTarget, ProposalBatchAtomicity, ProposalBatchItem,
        ProposalBatchRollbackPolicy, ProposalId, ProposalPayload, ProposalRollbackStep,
        ProposalTargetCoverage, ProposalTargetCoverageKind, ProposalTargetKind,
        ProposalVersionPreconditions, WorkspaceProposal,
    };

    #[test]
    fn auto_approval_requires_low_risk_and_matching_rule_ids() {
        let policy = ProposalAutoApprovalPolicy {
            enabled: true,
            allowed_rule_ids: proposal_risk_rule_ids_from_complete_coverage(),
        };
        let coverage = ProposalTargetCoverage {
            coverage_kind: ProposalTargetCoverageKind::Complete,
            targets: vec![],
            omitted_target_count: 0,
            redaction_hints: vec![],
        };
        let payload = ProposalPayload::TextEdit(legion_protocol::TextEditProposal {
            file_id: legion_protocol::FileId(1),
            edits: legion_protocol::EditBatch { edits: vec![] },
        });

        assert!(proposal_auto_approval_allowed(&policy, &payload, &coverage));
    }

    #[test]
    fn auto_approval_rejects_non_low_risk_payloads() {
        let policy = ProposalAutoApprovalPolicy {
            enabled: true,
            allowed_rule_ids: proposal_risk_rule_ids_from_complete_coverage(),
        };
        let coverage = ProposalTargetCoverage {
            coverage_kind: ProposalTargetCoverageKind::Complete,
            targets: vec![],
            omitted_target_count: 0,
            redaction_hints: vec![],
        };
        let payload = ProposalPayload::DeleteFile(legion_protocol::DeleteFileProposal {
            file: legion_protocol::FileIdentity {
                file_id: legion_protocol::FileId(2),
                workspace_id: legion_protocol::WorkspaceId(1),
                canonical_path: legion_protocol::CanonicalPath("/tmp/delete.txt".to_string()),
                content_version: legion_protocol::FileContentVersion(1),
                content_hash: None,
            },
        });

        assert!(!proposal_auto_approval_allowed(
            &policy, &payload, &coverage
        ));
    }

    #[test]
    fn filtered_batch_proposal_keeps_only_accepted_items_and_metadata() {
        let target_keep = ProposalAffectedTarget {
            target_id: "target-keep".to_string(),
            kind: ProposalTargetKind::PathOnly,
            workspace_id: Some(legion_protocol::WorkspaceId(7)),
            file_id: None,
            buffer_id: None,
            path: Some(CanonicalPath("/tmp/keep.txt".to_string())),
            terminal_session_id: None,
            plugin_id: None,
            remote_authority: None,
            collaboration_session_id: None,
            byte_ranges: vec![],
            redaction_hints: vec![],
        };
        let target_drop = ProposalAffectedTarget {
            target_id: "target-drop".to_string(),
            ..target_keep.clone()
        };
        let batch_item_keep = ProposalBatchItem {
            order: 0,
            item_id: "item-keep".to_string(),
            payload: Box::new(ProposalPayload::CreateFile(CreateFileProposal {
                path: CanonicalPath("/tmp/keep.txt".to_string()),
                initial_content: Some("keep".to_string()),
            })),
            target_ids: vec![target_keep.target_id.clone()],
            required_capability: CapabilityId("fs.write".to_string()),
            rollback_step_ids: vec!["rollback-keep".to_string()],
        };
        let batch_item_drop = ProposalBatchItem {
            order: 1,
            item_id: "item-drop".to_string(),
            payload: Box::new(ProposalPayload::CreateFile(CreateFileProposal {
                path: CanonicalPath("/tmp/drop.txt".to_string()),
                initial_content: Some("drop".to_string()),
            })),
            target_ids: vec![target_drop.target_id.clone()],
            required_capability: CapabilityId("fs.write".to_string()),
            rollback_step_ids: vec!["rollback-drop".to_string()],
        };
        let proposal = WorkspaceProposal {
            proposal_id: ProposalId(77),
            principal: PrincipalId("principal".to_string()),
            capability: CapabilityId("fs.write".to_string()),
            correlation_id: legion_protocol::CorrelationId(77),
            payload: ProposalPayload::Batch(BatchProposalPayload {
                batch_id: uuid::Uuid::from_u128(77),
                atomicity: ProposalBatchAtomicity::PrepareAllBeforeMutate,
                rollback_policy: ProposalBatchRollbackPolicy::NotRequired,
                target_coverage: ProposalTargetCoverage {
                    coverage_kind: ProposalTargetCoverageKind::Complete,
                    targets: vec![target_keep.clone(), target_drop.clone()],
                    omitted_target_count: 0,
                    redaction_hints: vec![],
                },
                items: vec![batch_item_keep.clone(), batch_item_drop.clone()],
                dependency_edges: vec![legion_protocol::ProposalBatchDependency {
                    prerequisite_item_id: batch_item_keep.item_id.clone(),
                    dependent_item_id: batch_item_drop.item_id.clone(),
                    kind: legion_protocol::ProposalBatchDependencyKind::RequiresValidation,
                }],
                rollback_steps: vec![ProposalRollbackStep {
                    order: 0,
                    step_id: "rollback-keep".to_string(),
                    item_id: batch_item_keep.item_id.clone(),
                    target_id: target_keep.target_id.clone(),
                    action: legion_protocol::ProposalRollbackAction::DeleteCreatedFile,
                    expected_preconditions: ProposalVersionPreconditions {
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
                    diagnostics: vec![],
                }],
                partial_failures: vec![legion_protocol::ProposalPartialFailureRecord {
                    item_id: batch_item_drop.item_id.clone(),
                    target_id: target_drop.target_id.clone(),
                    reason: legion_protocol::ProposalFailureReason::ApplyFailed,
                    disposition:
                        legion_protocol::ProposalPartialFailureDisposition::FailedBeforeMutation,
                    diagnostics: vec![],
                }],
                preview_warnings: vec![
                    legion_protocol::ProposalPreviewWarning {
                        code: "keep-target-warning".to_string(),
                        kind: legion_protocol::ProposalPreviewWarningKind::AtomicityUnavailable,
                        message: "keep target warning".to_string(),
                        target_id: Some(target_keep.target_id.clone()),
                        redaction_hints: vec![],
                    },
                    legion_protocol::ProposalPreviewWarning {
                        code: "drop-target-warning".to_string(),
                        kind: legion_protocol::ProposalPreviewWarningKind::AtomicityUnavailable,
                        message: "drop target warning".to_string(),
                        target_id: Some(target_drop.target_id.clone()),
                        redaction_hints: vec![],
                    },
                ],
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
                summary: "filter batch by accepted target ids".to_string(),
                details: vec![],
            },
            expires_at: None,
            created_at: legion_protocol::TimestampMillis(1),
        };
        let accepted = HashSet::from([target_keep.target_id.clone()]);

        let filtered = filtered_batch_proposal_for_accepted_targets(&proposal, &accepted)
            .expect("filtered batch proposal should exist");
        let ProposalPayload::Batch(batch) = filtered.payload else {
            panic!("expected batch payload");
        };
        assert_eq!(batch.items.len(), 1);
        assert_eq!(batch.items[0].item_id, batch_item_keep.item_id);
        assert_eq!(batch.target_coverage.targets.len(), 1);
        assert_eq!(
            batch.target_coverage.targets[0].target_id,
            target_keep.target_id
        );
        assert!(batch.dependency_edges.is_empty());
        assert_eq!(batch.rollback_steps.len(), 1);
        assert_eq!(batch.rollback_steps[0].item_id, batch_item_keep.item_id);
        assert_eq!(batch.partial_failures.len(), 0);
        assert_eq!(batch.preview_warnings.len(), 1);
        assert_eq!(
            batch.preview_warnings[0].target_id.as_deref(),
            Some(target_keep.target_id.as_str())
        );
    }
}
