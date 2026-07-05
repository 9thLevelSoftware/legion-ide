//! App-side adapter that assembles `LspEditProposalConversionInput` from a
//! caller-supplied `WorkspaceEditProposalPayload` and request metadata (LANG.09).
//!
//! # Scope note
//! This module receives an *already-structured* `WorkspaceEditProposalPayload`
//! (byte-ranged `TextEdit`s, resolved `FileIdentity`, correct `WorkspaceEditSourceKind`).
//! Translating raw rust-analyzer WorkspaceEdit JSON (LSP line/character positions,
//! `uri` strings) into that struct requires document text for byte conversion and
//! workspace lookup for `FileIdentity` resolution; that work is deliberately
//! **out of scope here** and must be performed by the orchestrator layer before
//! calling this function.

use legion_protocol::{
    CapabilityId, LspEditProposalConversionInput, LspRequestCorrelation, PreviewSummary,
    PrincipalId, ProposalId, ProposalLifecycleState, ProposalPrivacyLabel,
    ProposalVersionPreconditions, TimestampMillis, WorkspaceEditProposalPayload,
};

/// Assembles a [`LspEditProposalConversionInput`] from an already-structured
/// `WorkspaceEditProposalPayload` and the surrounding proposal metadata.
///
/// The caller is responsible for having constructed `workspace_edit` with the
/// correct `source` variant (`LspRename`, `LspCodeAction`, etc.) and byte-
/// accurate `TextEdit` ranges. This function only assembles the envelope;
/// it does **not** parse or transform the edit content.
///
/// # Deferred work
/// Translating raw rust-analyzer WorkspaceEdit JSON (LSP `{line,character}`
/// positions and `uri`-based file references) into `WorkspaceEditProposalPayload`
/// requires document text (for line/character → byte offset conversion) and
/// workspace state (for `uri` → `FileIdentity` resolution). That translation is
/// deliberately out of this task's scope; the orchestrator will supply a
/// structured payload.
#[allow(clippy::too_many_arguments)]
pub fn workspace_edit_to_proposal_input(
    workspace_edit: WorkspaceEditProposalPayload,
    request: LspRequestCorrelation,
    proposal_id: ProposalId,
    principal: PrincipalId,
    capability: CapabilityId,
    preconditions: ProposalVersionPreconditions,
    lifecycle_state: ProposalLifecycleState,
    privacy_label: ProposalPrivacyLabel,
    preview: PreviewSummary,
    created_at: TimestampMillis,
    expires_at: Option<TimestampMillis>,
) -> LspEditProposalConversionInput {
    LspEditProposalConversionInput {
        proposal_id,
        principal,
        capability,
        request,
        workspace_edit,
        preconditions,
        lifecycle_state,
        privacy_label,
        preview,
        expires_at,
        created_at,
        diagnostics: Vec::new(),
        schema_version: 1,
    }
}
