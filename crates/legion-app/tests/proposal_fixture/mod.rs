//! Minimal fixture helpers for LSP edit → proposal routing tests (LANG.09).
//!
//! Shapes are copied from `legion-protocol/tests/dto_contracts.rs` helpers
//! (lines 115–537) and adapted for the app-level integration test.

use legion_protocol::{
    BufferId, BufferVersion, ByteRange, CancellationTokenId, CanonicalPath, CapabilityId,
    CausalityId, CorrelationId, EditBatch, FileContentVersion, FileFingerprint, FileId,
    FileIdentity, LanguageServerId, LspRequestCorrelation, LspRequestId, PreviewSummary,
    PrincipalId, ProposalAffectedTarget, ProposalPrivacyLabel, ProposalTargetCoverage,
    ProposalTargetCoverageKind, ProposalTargetKind, ProposalVersionPreconditions, RedactionHint,
    SemanticPrivacyScope, SnapshotId, TextEdit, TextRange, TimestampMillis,
    WorkspaceEditProposalPayload, WorkspaceEditSourceKind, WorkspaceFileOperation, WorkspaceId,
    WorkspaceTextEdit,
};
use uuid::Uuid;

fn lsp_request_id() -> LspRequestId {
    LspRequestId(Uuid::parse_str("12121212-1212-1212-1212-121212121212").unwrap())
}

fn cancellation_token_id() -> CancellationTokenId {
    CancellationTokenId(Uuid::parse_str("34343434-3434-3434-3434-343434343434").unwrap())
}

fn causality_id() -> CausalityId {
    CausalityId(Uuid::parse_str("cccccccc-cccc-cccc-cccc-cccccccccccc").unwrap())
}

pub fn fingerprint(value: &str) -> FileFingerprint {
    FileFingerprint {
        algorithm: "sha256".to_string(),
        value: value.to_string(),
    }
}

pub fn file_identity() -> FileIdentity {
    FileIdentity {
        file_id: FileId(33),
        workspace_id: WorkspaceId(11),
        canonical_path: CanonicalPath("C:/repo/src/main.rs".to_string()),
        content_version: FileContentVersion(44),
        content_hash: Some("sha256:file".to_string()),
    }
}

pub fn preconditions() -> ProposalVersionPreconditions {
    ProposalVersionPreconditions {
        // `file_version` is a legacy alias of `file_content_version`; both set equal.
        file_version: Some(FileContentVersion(44)),
        buffer_version: Some(BufferVersion(55)),
        snapshot_id: Some(SnapshotId(66)),
        generation: Some(legion_protocol::WorkspaceGeneration(77)),
        file_content_version: Some(FileContentVersion(44)),
        workspace_generation: Some(legion_protocol::WorkspaceGeneration(77)),
        expected_fingerprint: Some(fingerprint("expected")),
        expected_file_length: Some(1234),
        expected_modified_at: Some(TimestampMillis(9876)),
    }
}

pub fn batch_target_coverage() -> ProposalTargetCoverage {
    ProposalTargetCoverage {
        coverage_kind: ProposalTargetCoverageKind::Complete,
        targets: vec![ProposalAffectedTarget {
            target_id: "target-buffer-main".to_string(),
            kind: ProposalTargetKind::OpenBuffer,
            workspace_id: Some(WorkspaceId(11)),
            file_id: Some(FileId(33)),
            buffer_id: Some(BufferId(22)),
            path: Some(CanonicalPath("C:/repo/src/main.rs".to_string())),
            terminal_session_id: None,
            plugin_id: None,
            remote_authority: None,
            collaboration_session_id: None,
            byte_ranges: vec![ByteRange::new(10, 14)],
            redaction_hints: vec![RedactionHint::MetadataOnly],
        }],
        omitted_target_count: 0,
        redaction_hints: vec![RedactionHint::MetadataOnly],
    }
}

/// Capability required by an LSP rename edit.
pub const RENAME_CAPABILITY: &str = "language.rename";
/// Capability required by an LSP code action edit (distinct from rename).
pub const CODE_ACTION_CAPABILITY: &str = "language.code_action";

/// Build a minimal `WorkspaceEditProposalPayload` with the given source kind
/// and required capability. The caller supplies `required_capability` so the
/// payload and the conversion-input envelope can carry a capability accurate to
/// the edit source; the validator enforces `required_capability == capability`,
/// so each test must keep the two equal but distinct across edit kinds.
pub fn workspace_edit_payload(
    source: WorkspaceEditSourceKind,
    required_capability: &str,
) -> WorkspaceEditProposalPayload {
    WorkspaceEditProposalPayload {
        workspace_id: WorkspaceId(11),
        edit_id: Uuid::parse_str("78787878-7878-7878-7878-787878787878").unwrap(),
        title: "rename symbol".to_string(),
        source,
        target_coverage: batch_target_coverage(),
        file_edits: vec![WorkspaceTextEdit {
            file: file_identity(),
            buffer_id: Some(BufferId(22)),
            edits: EditBatch {
                edits: vec![TextEdit {
                    range: TextRange::byte(10, 14),
                    replacement: "renamed".to_string(),
                }],
            },
            preconditions: preconditions(),
        }],
        file_operations: vec![WorkspaceFileOperation::Rename {
            file: file_identity(),
            destination: CanonicalPath("C:/repo/src/main_renamed.rs".to_string()),
        }],
        required_capability: CapabilityId(required_capability.to_string()),
        diagnostics: vec![],
        schema_version: 1,
    }
}

/// Rename-source payload requiring the `language.rename` capability.
pub fn rename_payload() -> WorkspaceEditProposalPayload {
    workspace_edit_payload(WorkspaceEditSourceKind::LspRename, RENAME_CAPABILITY)
}

/// Code-action-source payload requiring the distinct `language.code_action`
/// capability so the code-action test independently exercises the invariant.
pub fn code_action_payload() -> WorkspaceEditProposalPayload {
    workspace_edit_payload(
        WorkspaceEditSourceKind::LspCodeAction,
        CODE_ACTION_CAPABILITY,
    )
}

/// LSP request correlation (mirrors dto_contracts.rs `lsp_request_correlation()`).
/// Note: `privacy_scope` must be `Workspace` (not `MetadataOnly`) to pass
/// `validate_lsp_edit_proposal_contract`.
pub fn correlation() -> LspRequestCorrelation {
    LspRequestCorrelation {
        request_id: lsp_request_id(),
        server_id: LanguageServerId(7),
        workspace_id: WorkspaceId(11),
        file_id: Some(FileId(33)),
        snapshot_id: Some(SnapshotId(66)),
        buffer_version: Some(BufferVersion(55)),
        correlation_id: CorrelationId(901),
        causality_id: causality_id(),
        cancellation_token: Some(cancellation_token_id()),
        privacy_scope: SemanticPrivacyScope::Workspace,
        issued_at: TimestampMillis(1100),
        schema_version: 1,
    }
}

pub fn privacy_label() -> ProposalPrivacyLabel {
    ProposalPrivacyLabel::WorkspaceMetadata
}

pub fn preview() -> PreviewSummary {
    PreviewSummary {
        summary: "proposal-mediated LSP edit".to_string(),
        details: vec!["metadata-only preview; full diff redacted".to_string()],
    }
}

pub fn created_at() -> TimestampMillis {
    TimestampMillis(1000)
}

pub fn principal() -> PrincipalId {
    PrincipalId("principal-1".to_string())
}

/// Envelope capability for the rename case (matches `rename_payload`).
pub fn rename_capability() -> CapabilityId {
    CapabilityId(RENAME_CAPABILITY.to_string())
}

/// Envelope capability for the code-action case (matches `code_action_payload`).
pub fn code_action_capability() -> CapabilityId {
    CapabilityId(CODE_ACTION_CAPABILITY.to_string())
}
