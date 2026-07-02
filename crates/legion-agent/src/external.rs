use legion_protocol::{
    AssistedAiContractError, CapabilityId, CausalityId, CorrelationId, PreviewSummary, PrincipalId,
    ProposalId, ProposalPayload, ProposalTargetCoverageKind, ProposalVersionPreconditions,
    TimestampMillis, WorkspaceEditProposalPayload, WorkspaceProposal, WorkspaceTextEdit,
};
use uuid::Uuid;

use super::AgentError;

fn invalid_metadata(reason: impl Into<String>) -> AgentError {
    AgentError::InvalidMetadata(AssistedAiContractError::InvalidProposalMetadata {
        reason: reason.into(),
    })
}

fn preview_summary(payload: &WorkspaceEditProposalPayload) -> PreviewSummary {
    let mut details = vec![format!("source={:?}", payload.source)];
    details.push(format!("file_edits={}", payload.file_edits.len()));
    details.push(format!("file_operations={}", payload.file_operations.len()));
    details.push(format!(
        "target_coverage={:?}",
        payload.target_coverage.coverage_kind
    ));
    PreviewSummary {
        summary: payload.title.clone(),
        details,
    }
}

fn complete_preconditions(edit: &WorkspaceTextEdit) -> bool {
    edit.preconditions.file_version.is_some()
        && edit.preconditions.buffer_version.is_some()
        && edit.preconditions.snapshot_id.is_some()
        && edit.preconditions.generation.is_some()
        && edit.preconditions.file_content_version.is_some()
        && edit.preconditions.workspace_generation.is_some()
        && edit.preconditions.expected_fingerprint.is_some()
}

fn validate_workspace_edit_conversion(
    proposal_id: ProposalId,
    principal: &PrincipalId,
    capability: &CapabilityId,
    correlation_id: CorrelationId,
    causality_id: CausalityId,
    payload: &WorkspaceEditProposalPayload,
    preconditions: &ProposalVersionPreconditions,
) -> Result<(), AgentError> {
    let _ = principal;
    if proposal_id.0 == 0 {
        return Err(invalid_metadata(
            "external proposal requires a non-zero proposal id",
        ));
    }
    if correlation_id.0 == 0 {
        return Err(AgentError::InvalidMetadata(
            AssistedAiContractError::ZeroCorrelationId,
        ));
    }
    if causality_id.0 == Uuid::nil() {
        return Err(AgentError::InvalidMetadata(
            AssistedAiContractError::NilCausalityId,
        ));
    }
    if payload.title.trim().is_empty() {
        return Err(invalid_metadata("external proposal requires a title"));
    }
    if payload.schema_version == 0 {
        return Err(invalid_metadata(
            "external proposal payload schema_version must be non-zero",
        ));
    }
    if payload.required_capability != *capability {
        return Err(invalid_metadata(
            "external proposal capability does not match payload capability",
        ));
    }
    if payload.target_coverage.coverage_kind != ProposalTargetCoverageKind::Complete
        || payload.target_coverage.omitted_target_count != 0
    {
        return Err(invalid_metadata(
            "external proposal requires complete target coverage without omissions",
        ));
    }
    if payload.file_edits.is_empty() && payload.file_operations.is_empty() {
        return Err(invalid_metadata(
            "external proposal requires at least one file edit or file operation",
        ));
    }
    if !payload.file_edits.is_empty()
        && payload
            .file_edits
            .iter()
            .any(|edit| !complete_preconditions(edit))
    {
        return Err(invalid_metadata(
            "external proposal file edits require version and fingerprint preconditions",
        ));
    }
    if preconditions.file_version.is_none()
        && preconditions.buffer_version.is_none()
        && preconditions.snapshot_id.is_none()
        && preconditions.generation.is_none()
        && preconditions.file_content_version.is_none()
        && preconditions.workspace_generation.is_none()
        && preconditions.expected_fingerprint.is_none()
        && !payload.file_edits.is_empty()
    {
        return Err(invalid_metadata(
            "external proposal requires proposal preconditions for file edits",
        ));
    }
    Ok(())
}

/// Input for converting an external edit into a proposal envelope.
#[derive(Debug, Clone)]
pub struct ExternalWorkspaceEditProposalInput {
    /// Stable proposal identifier.
    pub proposal_id: ProposalId,
    /// Principal responsible for the proposal.
    pub principal: PrincipalId,
    /// Capability required before mutation authority may apply the proposal.
    pub capability: CapabilityId,
    /// Audit correlation identifier.
    pub correlation_id: CorrelationId,
    /// Audit causality identifier.
    pub causality_id: CausalityId,
    /// Proposal-ready workspace edit payload.
    pub payload: WorkspaceEditProposalPayload,
    /// Version preconditions copied into the proposal envelope.
    pub preconditions: ProposalVersionPreconditions,
    /// Proposal expiration timestamp.
    pub expires_at: Option<TimestampMillis>,
    /// Proposal creation timestamp.
    pub created_at: TimestampMillis,
}

/// Convert an external edit into a proposal envelope without mutation.
pub fn external_workspace_edit_proposal(
    input: ExternalWorkspaceEditProposalInput,
) -> Result<WorkspaceProposal, AgentError> {
    validate_workspace_edit_conversion(
        input.proposal_id,
        &input.principal,
        &input.capability,
        input.correlation_id,
        input.causality_id,
        &input.payload,
        &input.preconditions,
    )?;

    let ExternalWorkspaceEditProposalInput {
        proposal_id,
        principal,
        capability,
        correlation_id,
        payload,
        preconditions,
        expires_at,
        created_at,
        ..
    } = input;

    Ok(WorkspaceProposal {
        proposal_id,
        principal,
        capability,
        correlation_id,
        payload: ProposalPayload::WorkspaceEdit(payload.clone()),
        preconditions,
        preview: preview_summary(&payload),
        expires_at,
        created_at,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use legion_protocol::{
        CanonicalPath, ProposalAffectedTarget, ProposalTargetCoverage, ProposalTargetKind,
        RedactionHint, WorkspaceEditSourceKind, WorkspaceFileOperation,
    };

    fn proposal_target(path: &str) -> ProposalAffectedTarget {
        ProposalAffectedTarget {
            target_id: format!("target:{path}"),
            kind: ProposalTargetKind::PathOnly,
            workspace_id: None,
            file_id: None,
            buffer_id: None,
            path: Some(CanonicalPath(path.to_string())),
            terminal_session_id: None,
            plugin_id: None,
            remote_authority: None,
            collaboration_session_id: None,
            byte_ranges: vec![],
            redaction_hints: vec![RedactionHint::MetadataOnly],
        }
    }

    #[test]
    fn external_workspace_edit_proposal_builds_preview_without_mutation() {
        let input = ExternalWorkspaceEditProposalInput {
            proposal_id: ProposalId(42),
            principal: PrincipalId("principal:external".to_string()),
            capability: CapabilityId("fs.write".to_string()),
            correlation_id: CorrelationId(17),
            causality_id: CausalityId(Uuid::now_v7()),
            payload: WorkspaceEditProposalPayload {
                workspace_id: legion_protocol::WorkspaceId(9),
                edit_id: Uuid::now_v7(),
                title: "Apply external workspace change".to_string(),
                source: WorkspaceEditSourceKind::User,
                target_coverage: ProposalTargetCoverage {
                    coverage_kind: ProposalTargetCoverageKind::Complete,
                    targets: vec![proposal_target("src/external.rs")],
                    omitted_target_count: 0,
                    redaction_hints: vec![RedactionHint::MetadataOnly],
                },
                file_edits: vec![],
                file_operations: vec![WorkspaceFileOperation::Create {
                    path: CanonicalPath("src/external.rs".to_string()),
                    initial_content_hash: None,
                }],
                required_capability: CapabilityId("fs.write".to_string()),
                diagnostics: vec![],
                schema_version: 1,
            },
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
            expires_at: None,
            created_at: TimestampMillis(99),
        };

        let proposal = external_workspace_edit_proposal(input).expect("proposal envelope");

        assert_eq!(proposal.proposal_id, ProposalId(42));
        assert_eq!(proposal.correlation_id, CorrelationId(17));
        assert_eq!(proposal.preview.summary, "Apply external workspace change");
        assert_eq!(proposal.preview.details[0], "source=User");
        assert!(matches!(
            proposal.payload,
            ProposalPayload::WorkspaceEdit(_)
        ));
    }

    #[test]
    fn external_workspace_edit_proposal_rejects_missing_payload() {
        let input = ExternalWorkspaceEditProposalInput {
            proposal_id: ProposalId(43),
            principal: PrincipalId("principal:external".to_string()),
            capability: CapabilityId("fs.write".to_string()),
            correlation_id: CorrelationId(17),
            causality_id: CausalityId(Uuid::now_v7()),
            payload: WorkspaceEditProposalPayload {
                workspace_id: legion_protocol::WorkspaceId(9),
                edit_id: Uuid::now_v7(),
                title: "Empty proposal".to_string(),
                source: WorkspaceEditSourceKind::User,
                target_coverage: ProposalTargetCoverage {
                    coverage_kind: ProposalTargetCoverageKind::Complete,
                    targets: vec![proposal_target("src/external.rs")],
                    omitted_target_count: 0,
                    redaction_hints: vec![RedactionHint::MetadataOnly],
                },
                file_edits: vec![],
                file_operations: vec![],
                required_capability: CapabilityId("fs.write".to_string()),
                diagnostics: vec![],
                schema_version: 1,
            },
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
            expires_at: None,
            created_at: TimestampMillis(99),
        };

        let error = external_workspace_edit_proposal(input).expect_err("invalid proposal");
        assert!(
            error
                .to_string()
                .contains("requires at least one file edit or file operation")
        );
    }
}
