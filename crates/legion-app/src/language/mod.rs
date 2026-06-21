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
pub use session::{LanguageSessionError, RustAnalyzerLaunchConfig, RustAnalyzerSession};

// Re-export discovery types consumed by tests and callers.
pub use legion_lsp::{DiscoveredBinary, RustAnalyzerDiscovery};

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
        file_id: FileId(11),
        buffer_id: BufferId(12),
        snapshot_id: SnapshotId(13),
        buffer_version: BufferVersion(14),
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
