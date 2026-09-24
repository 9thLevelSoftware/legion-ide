//! Proposal-kind dispatch for the language tooling workflow.
//!
//! Moved verbatim out of `lib.rs` for the chokepoint budget (cross-cutting
//! rule 1). The enum and the three inherent `LanguageToolingWorkflow` methods
//! are the same bytes that lived in `lib.rs`; only the method visibility is
//! spelled `pub(crate)` because the callers now live in ancestor and sibling
//! modules. `LanguageProposalKind` is re-exported from the crate root so
//! `crate::LanguageProposalKind` keeps resolving.

use crate::{LanguageRequestInput, LanguageToolingWorkflow};
use legion_protocol::{
    LanguageToolingOperationKind, LanguageToolingOperationProjection, LanguageToolingProjection,
    LanguageToolingStatusKind, ProposalId, TimestampMillis,
};

#[derive(Debug, Clone, Copy)]
pub(crate) enum LanguageProposalKind {
    Formatting,
    Rename,
    OrganizeImports,
    CodeAction,
}

impl LanguageToolingWorkflow {
    pub(crate) fn record_proposal(
        &mut self,
        input: &LanguageRequestInput,
        kind: LanguageProposalKind,
        proposal_id: ProposalId,
        action_id: Option<&str>,
        message: String,
        operation_id: Option<String>,
    ) -> LanguageToolingProjection {
        let operation_kind = match kind {
            LanguageProposalKind::Formatting => LanguageToolingOperationKind::FormattingProposal,
            LanguageProposalKind::Rename => LanguageToolingOperationKind::RenameProposal,
            LanguageProposalKind::OrganizeImports => {
                LanguageToolingOperationKind::OrganizeImportsProposal
            }
            LanguageProposalKind::CodeAction => LanguageToolingOperationKind::CodeActionProposal,
        };
        self.projection.workspace_id = Some(input.workspace_id);
        self.projection.buffer_id = Some(input.buffer_id);
        self.projection.file_id = Some(input.metadata.identity.file_id);
        self.projection.status = LanguageToolingStatusKind::Ready;
        self.projection.status_message = message.clone();
        self.projection.generated_at = TimestampMillis::now();
        if matches!(kind, LanguageProposalKind::CodeAction)
            && let Some(action_id) = action_id
        {
            for quick_fix in &mut self.projection.quick_fixes {
                if quick_fix.action_id == action_id {
                    quick_fix.proposal_id = Some(proposal_id);
                }
            }
        }
        let has_explicit_operation_id = operation_id.is_some();
        let operation_id = operation_id.unwrap_or_else(|| self.next_operation_id(operation_kind));
        let row = LanguageToolingOperationProjection {
            operation_id: operation_id.clone(),
            kind: operation_kind,
            status: LanguageToolingStatusKind::Ready,
            request_id: if has_explicit_operation_id {
                None
            } else {
                Some(legion_protocol::LspRequestId(uuid::Uuid::now_v7()))
            },
            proposal_id: Some(proposal_id),
            message,
            correlation_id: Some(input.event_context.correlation_id),
            causality_id: Some(input.event_context.causality_id),
            generated_at: TimestampMillis::now(),
            schema_version: 1,
        };
        if let Some(existing) = self
            .projection
            .operations
            .iter_mut()
            .find(|operation| operation.operation_id == operation_id)
        {
            *existing = row;
        } else {
            self.push_operation(row);
        }
        self.projection()
    }

    pub(crate) fn record_proposal_failure(
        &mut self,
        input: &LanguageRequestInput,
        kind: LanguageProposalKind,
        message: String,
    ) -> LanguageToolingProjection {
        self.record_proposal_failure_with_operation_id(input, kind, message, None)
    }

    pub(crate) fn record_proposal_failure_with_operation_id(
        &mut self,
        input: &LanguageRequestInput,
        kind: LanguageProposalKind,
        message: String,
        operation_id: Option<String>,
    ) -> LanguageToolingProjection {
        let operation_kind = match kind {
            LanguageProposalKind::Formatting => LanguageToolingOperationKind::FormattingProposal,
            LanguageProposalKind::Rename => LanguageToolingOperationKind::RenameProposal,
            LanguageProposalKind::OrganizeImports => {
                LanguageToolingOperationKind::OrganizeImportsProposal
            }
            LanguageProposalKind::CodeAction => LanguageToolingOperationKind::CodeActionProposal,
        };
        self.projection.workspace_id = Some(input.workspace_id);
        self.projection.buffer_id = Some(input.buffer_id);
        self.projection.file_id = Some(input.metadata.identity.file_id);
        self.projection.status = LanguageToolingStatusKind::Failed;
        self.projection.status_message = message.clone();
        self.projection.generated_at = TimestampMillis::now();
        let has_explicit_operation_id = operation_id.is_some();
        let operation_id = operation_id.unwrap_or_else(|| self.next_operation_id(operation_kind));
        let row = LanguageToolingOperationProjection {
            operation_id: operation_id.clone(),
            kind: operation_kind,
            status: LanguageToolingStatusKind::Failed,
            request_id: if has_explicit_operation_id {
                None
            } else {
                Some(legion_protocol::LspRequestId(uuid::Uuid::now_v7()))
            },
            proposal_id: None,
            message,
            correlation_id: Some(input.event_context.correlation_id),
            causality_id: Some(input.event_context.causality_id),
            generated_at: TimestampMillis::now(),
            schema_version: 1,
        };
        if let Some(existing) = self
            .projection
            .operations
            .iter_mut()
            .find(|operation| operation.operation_id == operation_id)
        {
            *existing = row;
        } else {
            self.push_operation(row);
        }
        self.projection()
    }
}
