//! `RustAnalyzerSession` — launch + handshake orchestrator (WS-LANG-01 LANG.03/04).
//!
//! Owns a live [`LspStdioSession`] and the [`LspServerHealthRecord`] that tracks
//! binary provenance, handshake status, and runtime health.

use legion_lsp::{DiscoveredBinary, LspStdioSession, LspStdioSpawner, LspSupervisorConfig};
use legion_protocol::{
    LanguageId, LanguageServerId, LspResultStatus, LspServerBinaryProvenance,
    LspServerHealthRecord, SnapshotId,
};

use super::RustAnalyzerDiscovery;

/// Bounded restart policy for a crashed server (design §8, LANG.10).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestartPolicy {
    /// Maximum restarts before giving up.
    pub max_restarts: u32,
    /// Base backoff in milliseconds, doubled per attempt.
    pub backoff_base_ms: u64,
}

impl RestartPolicy {
    /// Backoff duration for a zero-based attempt index.
    pub fn backoff_for_attempt(&self, attempt: u32) -> std::time::Duration {
        std::time::Duration::from_millis(self.backoff_base_ms << attempt.min(16))
    }

    /// Whether the restart budget is exhausted at `attempt`.
    pub fn is_exhausted(&self, attempt: u32) -> bool {
        attempt >= self.max_restarts
    }
}

/// Outcome of an LSP read request: the raw result plus the snapshot the
/// request was issued against and the freshness status.
#[derive(Debug, Clone)]
pub struct LspReadOutcome {
    /// Raw JSON result payload from the LSP response.
    pub result: serde_json::Value,
    /// The snapshot against which the request was issued.
    pub issued_snapshot: SnapshotId,
    /// Freshness status of the response.
    pub status: LspResultStatus,
}

/// Errors raised while launching or initializing the rust-analyzer session.
#[derive(Debug)]
pub enum LanguageSessionError {
    /// No binary could be discovered through any resolution source.
    Discovery,
    /// The process failed to launch or spawn.
    Launch(legion_lsp::LspRuntimeError),
    /// The `initialize` handshake failed.
    Handshake(legion_lsp::LspRuntimeError),
    /// A read request (completion/hover/etc.) failed.
    ReadRequest(legion_lsp::LspRuntimeError),
}

impl std::fmt::Display for LanguageSessionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LanguageSessionError::Discovery => write!(f, "rust-analyzer binary not found"),
            LanguageSessionError::Launch(e) => write!(f, "rust-analyzer launch failed: {e}"),
            LanguageSessionError::Handshake(e) => {
                write!(f, "rust-analyzer handshake failed: {e}")
            }
            LanguageSessionError::ReadRequest(e) => {
                write!(f, "rust-analyzer read request failed: {e}")
            }
        }
    }
}

impl std::error::Error for LanguageSessionError {}

/// Inputs for launching the rust-analyzer session.
pub struct RustAnalyzerLaunchConfig {
    /// Discovery inputs (resolution order: configured → project-local → PATH → bundled).
    pub discovery: RustAnalyzerDiscovery,
    /// Supervisor / process config (command, policy, backoff).
    pub supervisor: LspSupervisorConfig,
    /// Server identity written into the health record.
    pub server_id: LanguageServerId,
    /// Language identity written into the health record.
    pub language_id: LanguageId,
}

/// Owns a live rust-analyzer stdio session and its health record.
pub struct RustAnalyzerSession {
    session: LspStdioSession,
    health: LspServerHealthRecord,
}

impl RustAnalyzerSession {
    /// Resolves discovery for provenance, launches the stdio process, and seeds
    /// the health record with `init_status = Unavailable` until `initialize` is called.
    pub fn launch(
        config: RustAnalyzerLaunchConfig,
        launcher: &mut impl LspStdioSpawner,
    ) -> Result<Self, LanguageSessionError> {
        // Resolve provenance from discovery inputs (metadata-only — no path stored).
        let provenance: LspServerBinaryProvenance = match config.discovery.resolve() {
            DiscoveredBinary::Found { provenance, .. } => provenance,
            DiscoveredBinary::NotFound => return Err(LanguageSessionError::Discovery),
        };

        // Launch the stdio session through the caller-supplied launcher.
        let session = LspStdioSession::start(config.supervisor, launcher)
            .map_err(LanguageSessionError::Launch)?;

        // Seed the health record; init_status is Unavailable until initialize() succeeds.
        let health = LspServerHealthRecord {
            server_id: config.server_id,
            language_id: config.language_id,
            binary_provenance: provenance,
            binary_path_hash: None,
            artifact_hash: None,
            version: None,
            init_status: LspResultStatus::Unavailable,
            capabilities: Vec::new(),
            diagnostics_latency_ms: None,
            restart_count: 0,
            download_decision_id: None,
            schema_version: LspServerHealthRecord::schema_version(),
        };

        Ok(Self { session, health })
    }

    /// Sends the LSP `initialize` request and the `initialized` notification,
    /// then updates `health.init_status` from the correlated response.
    pub fn initialize(&mut self, root_uri: &str) -> Result<(), LanguageSessionError> {
        let params = serde_json::json!({
            "processId": std::process::id(),
            "rootUri": root_uri,
            "capabilities": {},
            "workspaceFolders": [{ "uri": root_uri, "name": "workspace" }],
        });

        let response = self
            .session
            .initialize(params, super::operation_context())
            .map_err(LanguageSessionError::Handshake)?;

        self.health.init_status = response.status;

        self.session
            .send_notification("initialized", serde_json::json!({}))
            .map_err(LanguageSessionError::Handshake)?;

        Ok(())
    }

    /// Borrows the health record for read-only projection.
    pub fn health(&self) -> &LspServerHealthRecord {
        &self.health
    }

    /// Sends `textDocument/didOpen` for a buffer.
    pub fn did_open(
        &mut self,
        uri: &str,
        language_id: &str,
        version: i64,
        text: &str,
    ) -> Result<(), LanguageSessionError> {
        let params = serde_json::json!({
            "textDocument": {
                "uri": uri,
                "languageId": language_id,
                "version": version,
                "text": text,
            }
        });
        self.session
            .send_notification("textDocument/didOpen", params)
            .map_err(LanguageSessionError::Handshake)
    }

    /// Returns diagnostics for the session, pumping until some arrive or the
    /// timeout elapses. Short-circuits if diagnostics are already buffered
    /// (e.g. emitted at/before initialize), so it never blocks needlessly.
    pub fn pump_diagnostics(
        &mut self,
        _uri: &str,
        timeout: std::time::Duration,
    ) -> Vec<legion_lsp::LspDiagnosticNotificationMetadata> {
        if self.session.diagnostic_notifications().is_empty() {
            let deadline = std::time::Instant::now() + timeout;
            let _ = self
                .session
                .pump_until(deadline, &mut |n| !n.diagnostics.is_empty());
        }
        self.session.diagnostic_notifications().to_vec()
    }

    /// Sends an LSP read request (e.g. `textDocument/completion`) and blocks
    /// for the correlated response.  Returns an [`LspReadOutcome`] carrying the
    /// raw JSON result, the snapshot the request was issued against, and the
    /// freshness status — allowing callers to gate ingestion via
    /// [`super::is_stale_response`] before projecting into buffer state.
    pub fn request_read(
        &mut self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<LspReadOutcome, LanguageSessionError> {
        let response = self
            .session
            .request(method.to_string(), params, super::operation_context())
            .map_err(LanguageSessionError::ReadRequest)?;
        Ok(LspReadOutcome {
            result: response.result,
            issued_snapshot: response.context.snapshot_id,
            status: response.status,
        })
    }

    /// Mutable access to the underlying stdio session (for later tasks: doc sync, restart).
    #[allow(dead_code)]
    pub(crate) fn session_mut(&mut self) -> &mut LspStdioSession {
        &mut self.session
    }

    /// Mutable access to the health record (for later tasks: restart counter, capability update).
    #[allow(dead_code)]
    pub(crate) fn health_mut(&mut self) -> &mut LspServerHealthRecord {
        &mut self.health
    }

    /// Records a crash, increments `restart_count`, and returns the backoff if
    /// a restart is still permitted (caller performs the relaunch).
    pub fn note_crash_and_should_restart(
        &mut self,
        policy: &RestartPolicy,
    ) -> Option<std::time::Duration> {
        let attempt = self.health.restart_count;
        if policy.is_exhausted(attempt) {
            self.health.init_status = legion_protocol::LspResultStatus::Unavailable;
            return None;
        }
        self.health.restart_count = attempt + 1;
        Some(policy.backoff_for_attempt(attempt))
    }
}
