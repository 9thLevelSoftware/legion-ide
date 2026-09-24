//! Language-tooling orchestration extracted from `lib.rs` (design §10).
//!
//! This module provides the capability-gated decision layer for LSP tooling
//! lifecycle operations such as rust-analyzer binary acquisition and session
//! launch/handshake.

mod download;
mod runtime;
pub use runtime::{
    ApprovedNodeRuntime, NODE_RUNTIME_FINGERPRINT_MAX_BYTES, NODE_RUNTIME_PROBE_CAPABILITY,
    NODE_RUNTIME_PROBE_STREAM_LIMIT, NODE_RUNTIME_PROBE_TIMEOUT, NodeRuntimeApprovalError,
    NodeRuntimeApprovalRequest, approve_node_runtime,
};
mod formatter_approval;
pub use formatter_approval::{
    ApprovedFormatterExecutable, FORMATTER_FINGERPRINT_MAX_BYTES, FORMATTER_PROBE_ARG_MAX_BYTES,
    FORMATTER_PROBE_CAPABILITY, FORMATTER_PROBE_MAX_ARGS, FORMATTER_PROBE_STREAM_LIMIT,
    FORMATTER_PROBE_TIMEOUT, FORMATTER_UNPROBEABLE_REASON_MAX_BYTES, FormatterApprovalError,
    FormatterApprovalRequest, FormatterProbe, FormatterProbeOutcome, approve_formatter_executable,
};
mod external_formatter;
pub use external_formatter::{
    EXTERNAL_FORMATTER_BROKER_PREREQUISITE, EXTERNAL_FORMATTER_DETAIL_TAG,
    EXTERNAL_FORMATTER_UNPROBEABLE_REASON, ExternalFormatterBounds, ExternalFormatterError,
    ExternalFormatterRun, ExternalFormattingAdmission, PYTHON_FORMATTER_EXPECTED_EXIT_CODE,
    PYTHON_FORMATTER_MAX_DOCUMENT_BYTES, PYTHON_FORMATTER_STDERR_LIMIT,
    PYTHON_FORMATTER_STDIN_ARGS, PYTHON_FORMATTER_STDOUT_LIMIT, PYTHON_FORMATTER_TIMEOUT,
    run_external_formatter,
};
mod startup_authority;
pub use startup_authority::{LanguageStartupAuthority, LanguageStartupContext};
mod typescript_bundle;
pub use typescript_bundle::TypeScriptBundleDescriptor;
mod materialize;
pub use download::{
    DownloadDecision, RustAnalyzerDownloadRequest, evaluate_rust_analyzer_download,
    verify_downloaded_artifact,
};
pub use materialize::{
    ArtifactDescriptor, ArtifactSource, CancellationToken, LanguageArtifactMaterializer,
    MaterializeError, MaterializeEvent, MaterializeHandle, MaterializeProgress, MaterializeRequest,
    MaterializedArtifact,
};

mod session;
pub use session::{
    LanguageServerLaunchConfig, LanguageServerSession, LanguageSessionError, LspReadOutcome,
    RestartPolicy, RustAnalyzerLaunchConfig, RustAnalyzerSession,
};

mod local_proposals;
mod proposal;
pub use proposal::workspace_edit_to_proposal_input;
pub(crate) mod proposal_kinds;
pub(crate) mod toolchain_settings;

mod redaction;
pub use redaction::{StderrSummary, redact_lsp_stderr, redact_lsp_stderr_line};

mod translate;
pub use translate::{
    DocumentResolver, ResolvedDocument, TranslationError, lsp_position_to_byte_offset,
    translate_workspace_edit, uri_to_canonical_path,
};

mod code_action_commands;
pub(crate) use code_action_commands::PendingLspCommandContext;
mod code_action_diagnostics;
mod code_actions;
#[cfg(test)]
#[path = "formatting_dispatch_tests.rs"]
mod formatting_dispatch_tests;
#[cfg(test)]
#[path = "server_apply_edit_tests.rs"]
mod server_apply_edit_tests;
mod server_apply_edits;
#[cfg(test)]
#[path = "typescript_organize_tests.rs"]
mod typescript_organize_tests;
pub(crate) use server_apply_edits::ServerApplyEditAuthority;
mod apply_edit_decision;
pub(crate) use apply_edit_decision::{
    ApplyEditClaim, ApplyEditDecision, ApplyEditDecisionResult, DeadlineDecision,
};
mod lsp_reads;
pub(crate) use code_action_commands::{
    CodeActionCommand, CodeActionCommandSidecars, extract_command,
};
pub(crate) use code_action_diagnostics::{CodeActionDiagnostics, DiagnosticIdentity};
pub(crate) use code_actions::{CodeActionAuthority, CodeActionIdentity, bounded_code_action_size};
pub(crate) use lsp_reads::DeferredLspWrite;

mod problem_rows;
pub(crate) use problem_rows::{
    language_projection_for_new_identity, language_quick_fixes_prioritizing,
};

mod call_hierarchy;
pub use call_hierarchy::{
    CALL_HIERARCHY_ROW_CAP, PendingCallHierarchy, call_params, call_row_label, first_item,
    prepare_params, rows_from_incoming, rows_from_outgoing, status_message,
};

mod app_lsp;
#[cfg(any(test, feature = "test-helpers"))]
pub use app_lsp::LspWorkerRequest;
pub(crate) use app_lsp::PendingLspWriteOperation;
pub use app_lsp::{
    LanguageServerStartConfig, LspReadKind, LspRequestTag, LspSelectedServerMetadata,
    LspSessionHandle, LspWorkerResult,
};

// Re-export discovery types consumed by tests and callers.
pub use legion_lsp::{DiscoveredBinary, RustAnalyzerDiscovery};

use legion_protocol::SnapshotId;

/// Returns `true` when a response issued against `issued` is stale relative to
/// the buffer's `current` snapshot (LANG.07).
///
/// A response is considered stale whenever the snapshot it was issued against
/// differs from the snapshot the buffer is currently at — regardless of
/// direction. Callers should discard stale responses rather than projecting
/// them into buffer state.
///
/// # Deferred call-site adoption
/// The legacy `ingest_lsp_*_response_for_buffer` methods in `lib.rs` are NOT
/// yet gated by this function (their signatures would need to change and there
/// are ~8 such methods with 25k-line call-site context). They operate on
/// mock/deterministic-fed data. Broad adoption is deliberately deferred; the
/// real read path goes through [`RustAnalyzerSession::request_read`], which
/// surfaces `issued_snapshot` in [`LspReadOutcome`] so callers can apply this
/// gate directly.
pub fn is_stale_response(issued: SnapshotId, current: SnapshotId) -> bool {
    issued != current
}

/// Builds a deterministic baseline [`LspOperationContext`] for handshake-phase
/// LSP calls (e.g. `initialize`).  All fields are fixed metadata-only values
/// that unambiguously identify a "session bootstrap" operation.
///
/// [`LspOperationContext`]: legion_protocol::LspOperationContext
pub(crate) fn operation_context() -> legion_protocol::LspOperationContext {
    operation_context_for_snapshot(SnapshotId(0))
}

/// Builds an [`LspOperationContext`] for a read request at a specific buffer
/// snapshot.  `snapshot_id` is threaded into `issued_snapshot` in the
/// returned [`LspReadOutcome`], enabling the `is_stale_response` gate.
///
/// [`LspOperationContext`]: legion_protocol::LspOperationContext
pub(crate) fn operation_context_for_snapshot(
    snapshot_id: SnapshotId,
) -> legion_protocol::LspOperationContext {
    use legion_protocol::*;
    LspOperationContext {
        request_id: LspRequestId(uuid::Uuid::now_v7()),
        workspace_id: WorkspaceId(55),
        file_id: FileId(0),
        buffer_id: BufferId(0),
        snapshot_id,
        buffer_version: BufferVersion(0),
        language_id: LanguageId("rust".to_string()),
        correlation_id: CorrelationId(1u64),
        causality_id: CausalityId(uuid::Uuid::now_v7()),
        timeout_ms: 5000,
        cancellation_token: CancellationTokenId(uuid::Uuid::now_v7()),
        content_hash: None,
        privacy_scope: SemanticPrivacyScope::Workspace,
        schema_version: 1,
    }
}
