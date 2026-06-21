//! Language-tooling orchestration extracted from `lib.rs` (design §10).
//!
//! This module provides the capability-gated decision layer for LSP tooling
//! lifecycle operations such as rust-analyzer binary acquisition and session
//! launch/handshake.

mod download;
pub use download::{
    DownloadDecision, RustAnalyzerDownloadRequest, evaluate_rust_analyzer_download,
    verify_downloaded_artifact,
};

mod session;
pub use session::{LanguageSessionError, LspReadOutcome, RustAnalyzerLaunchConfig, RustAnalyzerSession};

// Re-export discovery types consumed by tests and callers.
pub use legion_lsp::{DiscoveredBinary, RustAnalyzerDiscovery};

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
pub fn is_stale_response(
    issued: legion_protocol::SnapshotId,
    current: legion_protocol::SnapshotId,
) -> bool {
    issued != current
}

/// Builds a deterministic baseline [`LspOperationContext`] for handshake-phase
/// LSP calls (e.g. `initialize`).  All fields are fixed metadata-only values
/// that unambiguously identify a "session bootstrap" operation.
///
/// [`LspOperationContext`]: legion_protocol::LspOperationContext
pub(crate) fn operation_context() -> legion_protocol::LspOperationContext {
    use legion_protocol::*;
    LspOperationContext {
        request_id: LspRequestId(uuid::Uuid::from_u128(1)),
        workspace_id: WorkspaceId(55),
        // Bootstrap/handshake context: no document is open yet, so document-scoped ids are 0.
        file_id: FileId(0),
        buffer_id: BufferId(0),
        snapshot_id: SnapshotId(0),
        buffer_version: BufferVersion(0),
        language_id: LanguageId("rust".to_string()),
        correlation_id: CorrelationId(1u64),
        causality_id: CausalityId(uuid::Uuid::from_u128(1001)),
        timeout_ms: 5000,
        cancellation_token: CancellationTokenId(uuid::Uuid::from_u128(2001)),
        content_hash: None,
        privacy_scope: SemanticPrivacyScope::Workspace,
        schema_version: 1,
    }
}
