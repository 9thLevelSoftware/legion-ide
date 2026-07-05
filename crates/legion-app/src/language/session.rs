//! `RustAnalyzerSession` — launch + handshake orchestrator (WS-LANG-01 LANG.03/04).
//!
//! Owns a live [`LspStdioSession`] and the [`LspServerHealthRecord`] that tracks
//! binary provenance, handshake status, and runtime health.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use legion_lsp::{DiscoveredBinary, LspStdioSession, LspStdioSpawner, LspSupervisorConfig};
use legion_protocol::{
    LanguageId, LanguageServerId, LspCapabilitySummary, LspResultStatus, LspServerBinaryProvenance,
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
    /// The server is not in an initialized/live state (e.g. post-crash backoff
    /// or budget exhausted). Callers must not send requests to a non-live session.
    Unavailable,
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
            LanguageSessionError::Unavailable => {
                write!(
                    f,
                    "rust-analyzer session is not initialized or is in backoff"
                )
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
    /// Shared ring buffer populated by the background stderr drain thread
    /// spawned in `startup_session()`.  Callers clone the `Arc` to extract
    /// the ring for projection (PKT-LSP-C T4).
    pub(crate) stderr_ring: Arc<Mutex<VecDeque<String>>>,
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

        Ok(Self {
            session,
            health,
            stderr_ring: Arc::new(Mutex::new(VecDeque::new())),
        })
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

        // Parse capability summaries from the initialize result body.
        // Only populate when the handshake succeeded; an error result has no capabilities.
        if self.health.init_status == LspResultStatus::Fresh
            && let Some(caps) = response
                .result
                .get("capabilities")
                .and_then(|v| v.as_object())
        {
            // Track the capability keys we care about for read-side gating.
            for cap_name in ["hoverProvider", "definitionProvider", "completionProvider"] {
                let supported = caps
                    .get(cap_name)
                    .map(|v| v.as_bool().unwrap_or(false))
                    .unwrap_or(false);
                self.health.capabilities.push(LspCapabilitySummary {
                    capability: cap_name.to_string(),
                    supported,
                    dynamic_registration: false,
                    option_hash: None,
                    redaction_hints: Vec::new(),
                    schema_version: 1,
                });
            }
        }

        self.session
            .send_notification("initialized", serde_json::json!({}))
            .map_err(LanguageSessionError::Handshake)?;

        Ok(())
    }

    /// Borrows the health record for read-only projection.
    pub fn health(&self) -> &LspServerHealthRecord {
        &self.health
    }

    /// Snapshot of the buffered diagnostic-notification metadata.
    ///
    /// Read-only post-mortem introspection for smokes and tests (same class
    /// as [`health`]): when a diagnostics pump times out, the buffer shows
    /// whether the server published anything at all during the wait —
    /// distinguishing "server silent" from "notifications arrived but the
    /// predicate never matched". Metadata-only (hashes and counts).
    pub fn buffered_diagnostic_notifications(
        &self,
    ) -> Vec<legion_lsp::LspDiagnosticNotificationMetadata> {
        self.session.diagnostic_notifications().to_vec()
    }

    /// Sends `textDocument/didOpen` for a buffer.
    ///
    /// Returns [`LanguageSessionError::Unavailable`] immediately if the session
    /// is not in an initialized/live state. No write is made to the transport
    /// in that case.
    pub fn did_open(
        &mut self,
        uri: &str,
        language_id: &str,
        version: i64,
        text: &str,
    ) -> Result<(), LanguageSessionError> {
        if self.health.init_status != legion_protocol::LspResultStatus::Fresh {
            return Err(LanguageSessionError::Unavailable);
        }
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

    /// Sends `textDocument/didChange` with a full-document replacement.
    ///
    /// Returns and removes the most recently received raw `publishDiagnostics`
    /// params for `uri`, if any.
    ///
    /// The returned value can be passed to
    /// `AppComposition::ingest_lsp_publish_diagnostics_for_buffer` to project
    /// the diagnostics through the app-owned `LanguageToolingProjection`.
    pub fn take_last_diagnostic_params_for(&mut self, uri: &str) -> Option<serde_json::Value> {
        let expected_hash = legion_lsp::lsp_diagnostic_uri_fingerprint(uri);
        self.session.take_raw_diagnostic_params_for(&expected_hash)
    }

    /// Pumps until a diagnostic notification for `uri` arrives with
    /// `error_count == 0` (all errors cleared), or the timeout elapses.
    ///
    /// Call this after a fix [`did_change`] to confirm the language server has
    /// acknowledged the repaired source.
    ///
    /// Returns `true` if a clean (0-error) notification was received before the
    /// deadline, or `false` if the deadline elapsed.
    pub fn pump_until_diagnostics_clear(
        &mut self,
        uri: &str,
        timeout: std::time::Duration,
    ) -> bool {
        let expected_hash = legion_lsp::lsp_diagnostic_uri_fingerprint(uri);
        let deadline = std::time::Instant::now() + timeout;
        // Do NOT pre-clear the notification buffer before pumping.
        //
        // rust-analyzer sometimes sends an immediate "clearing ack" notification
        // (publishDiagnostics with an empty diagnostics array) right after it
        // processes a textDocument/didChange, before re-analysing the new content.
        // This ack can arrive in the background reader's MPSC channel before
        // pump_until_diagnostics_clear is even called.  Pre-clearing the buffer
        // is unnecessary: pump_until reads fresh frames from the channel regardless
        // of what is already buffered.
        //
        // pump_until_has_error_for always calls clear_diagnostics_for_uri at its
        // start, so after it returns the buffer contains only notifications received
        // during that pump.  The last such notification is the error notification that
        // matched its predicate — so buffered_clean is false and the pump starts fresh.
        let outcome = self.session.pump_until(deadline, &mut |n| {
            n.diagnostics
                .iter()
                .any(|d| d.uri_hash == expected_hash && d.error_count == 0)
        });
        // Check the most recent buffered notification for this URI as a secondary
        // success signal: a clean notification received during the pump will be the
        // last entry in the buffer when PredicateMet is returned.
        let buffered_clean = self
            .session
            .diagnostic_notifications()
            .iter()
            .rev()
            .find(|n| n.uri_hash == expected_hash)
            .is_some_and(|n| n.error_count == 0);
        matches!(outcome, Ok(legion_lsp::PumpOutcome::PredicateMet)) || buffered_clean
    }

    /// Pumps until a diagnostic notification for `uri` arrives with at least one
    /// error (`error_count > 0`), or the timeout elapses.
    ///
    /// Call this after a [`did_change`] that introduces a compile error.
    /// Unlike [`pump_diagnostics`], this predicate skips "clear" notifications
    /// (e.g. the acknowledgement batch rust-analyzer sends immediately after a
    /// `didChange` before it has re-analysed the new content) and only returns
    /// `true` once a notification with real errors has been observed.
    ///
    /// Internally drains any pre-existing buffered notifications for `uri`
    /// before starting the pump so stale data does not trigger an early return.
    ///
    /// Returns `true` if an error notification was received before the deadline.
    pub fn pump_until_has_error_for(&mut self, uri: &str, timeout: std::time::Duration) -> bool {
        let expected_hash = legion_lsp::lsp_diagnostic_uri_fingerprint(uri);
        let deadline = std::time::Instant::now() + timeout;
        // Drain stale buffered notifications (e.g. the pre-error clean batch) so we
        // only match fresh ones produced after the erroneous did_change.
        self.session
            .clear_diagnostics_for_uri(expected_hash.clone());
        let outcome = self.session.pump_until(deadline, &mut |n| {
            n.diagnostics
                .iter()
                .any(|d| d.uri_hash == expected_hash && d.error_count > 0)
        });
        // Honour the accumulated buffer as a secondary check: if pump_until
        // returned Deadline but an error notification arrived anyway (e.g. just
        // before the deadline), count it as a pass.
        let buffered_error = self
            .session
            .diagnostic_notifications()
            .iter()
            .filter(|n| n.uri_hash == expected_hash)
            .any(|n| n.error_count > 0);
        matches!(outcome, Ok(legion_lsp::PumpOutcome::PredicateMet)) || buffered_error
    }

    /// Returns diagnostics for `uri`, pumping until some arrive or the timeout
    /// elapses. Short-circuits if diagnostics for that specific URI are already
    /// buffered (e.g. emitted at/before initialize), so it never blocks
    /// needlessly. Diagnostics for other URIs are not returned.
    pub fn pump_diagnostics(
        &mut self,
        uri: &str,
        timeout: std::time::Duration,
    ) -> Vec<legion_lsp::LspDiagnosticNotificationMetadata> {
        // Compute the expected URI fingerprint once so we can use it in both
        // the short-circuit check and the pump predicate without storing the
        // raw URI string.
        let expected_hash = legion_lsp::lsp_diagnostic_uri_fingerprint(uri);
        let has_buffered = self
            .session
            .diagnostic_notifications()
            .iter()
            .any(|n| n.uri_hash == expected_hash);
        if !has_buffered {
            let deadline = std::time::Instant::now() + timeout;
            let _ = self.session.pump_until(deadline, &mut |n| {
                n.diagnostics.iter().any(|d| d.uri_hash == expected_hash)
            });
        }
        self.session
            .diagnostic_notifications()
            .iter()
            .filter(|n| n.uri_hash == expected_hash)
            .cloned()
            .collect()
    }

    /// Sends an LSP read request (e.g. `textDocument/completion`) and blocks
    /// for the correlated response.  Returns an [`LspReadOutcome`] carrying the
    /// raw JSON result, the snapshot the request was issued against, and the
    /// freshness status — allowing callers to gate ingestion via
    /// [`super::is_stale_response`] before projecting into buffer state.
    ///
    /// `snapshot_id` is the buffer's current snapshot at the time the request
    /// is issued.  It is threaded through the `LspOperationContext` and
    /// surfaces in `LspReadOutcome::issued_snapshot`, enabling the
    /// `is_stale_response` gate at the drain/ingest point (D1 fix).
    ///
    /// Returns [`LanguageSessionError::Unavailable`] immediately if the session
    /// is not in an initialized/live state (e.g. post-crash backoff). No write
    /// is made to the transport in that case.
    pub fn request_read(
        &mut self,
        method: &str,
        params: serde_json::Value,
        snapshot_id: SnapshotId,
    ) -> Result<LspReadOutcome, LanguageSessionError> {
        if self.health.init_status != legion_protocol::LspResultStatus::Fresh {
            return Err(LanguageSessionError::Unavailable);
        }
        let ctx = super::operation_context_for_snapshot(snapshot_id);
        let response = self
            .session
            .request(method.to_string(), params, ctx)
            .map_err(LanguageSessionError::ReadRequest)?;
        Ok(LspReadOutcome {
            result: response.result,
            issued_snapshot: response.context.snapshot_id,
            status: response.status,
        })
    }

    /// Sends `textDocument/didChange` for a buffer (full-text sync, v1).
    ///
    /// Full-text sync is acceptable for v1 per the brief; incremental sync
    /// can be added later when performance requires it.
    ///
    /// Returns [`LanguageSessionError::Unavailable`] immediately if the session
    /// is not in an initialized/live state (no write reaches the transport).
    /// Per the LSP spec, `version` must be strictly greater than the version
    /// used in the matching `did_open` (or prior `did_change`) call.
    pub fn did_change(
        &mut self,
        uri: &str,
        version: i64,
        text: &str,
    ) -> Result<(), LanguageSessionError> {
        if self.health.init_status != legion_protocol::LspResultStatus::Fresh {
            return Err(LanguageSessionError::Unavailable);
        }
        let params = serde_json::json!({
            "textDocument": {
                "uri": uri,
                "version": version,
            },
            "contentChanges": [{ "text": text }],
        });
        self.session
            .send_notification("textDocument/didChange", params)
            .map_err(LanguageSessionError::Handshake)
    }

    /// Non-blocking drain of raw `publishDiagnostics` notification params.
    ///
    /// Delegates to [`LspStdioSession::try_drain_diagnostic_params`].  Safe to
    /// call only when no request is in flight (i.e. the session worker thread
    /// should call this in the idle/timeout branch of its recv loop).
    pub fn try_drain_diagnostic_params(&mut self) -> Vec<serde_json::Value> {
        self.session.try_drain_diagnostic_params()
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

    /// Detaches the child process stderr handle so a background drain thread
    /// can read it independently (PKT-LSP-C T4).  Returns `None` if stderr
    /// was not captured or has already been detached.
    pub fn take_stderr(&mut self) -> Option<std::process::ChildStderr> {
        self.session.take_stderr()
    }

    /// Spawns the background stderr drain thread for ring-buffer projection
    /// (PKT-LSP-C T4 / Controller C).
    ///
    /// Call this once after [`initialize`] returns to start draining child
    /// process stderr into the shared [`stderr_ring`].  The thread exits
    /// automatically when the child process closes its stderr pipe.
    ///
    /// Safe to call multiple times: if stderr has already been taken (or was
    /// never captured), the method is a silent no-op.
    pub fn start_stderr_drain(&mut self) {
        let Some(stderr) = self.take_stderr() else {
            return;
        };
        let ring = self.stderr_ring.clone();
        std::thread::spawn(move || {
            use std::io::BufRead;
            /// Maximum byte length of a retained stderr line.
            const LINE_MAX_LEN: usize = 512;
            /// Maximum number of lines retained in the ring buffer.
            const RING_CAPACITY: usize = 100;
            let reader = std::io::BufReader::new(stderr);
            for raw in reader.lines() {
                let Ok(raw_line) = raw else { break };
                let truncated: String = if raw_line.len() > LINE_MAX_LEN {
                    format!("{}…", &raw_line[..LINE_MAX_LEN])
                } else {
                    raw_line
                };
                let redacted = super::redact_lsp_stderr_line(&truncated);
                if let Ok(mut guard) = ring.lock() {
                    if guard.len() >= RING_CAPACITY {
                        guard.pop_front();
                    }
                    guard.push_back(redacted);
                }
            }
        });
    }

    /// Returns a clone of the shared stderr ring-buffer `Arc` so callers can
    /// store it in a worker handle and later read it for projection
    /// (PKT-LSP-C T4).
    pub fn stderr_ring(&self) -> Arc<Mutex<VecDeque<String>>> {
        self.stderr_ring.clone()
    }

    /// Records a crash, increments `restart_count`, and returns the backoff if
    /// a restart is still permitted (caller performs the relaunch).
    ///
    /// `init_status` is set to [`LspResultStatus::Unavailable`] immediately,
    /// regardless of whether restart budget remains. This prevents callers from
    /// treating the session as live during the backoff window. After a successful
    /// re-initialize, `initialize()` restores `init_status` to `Fresh`.
    pub fn note_crash_and_should_restart(
        &mut self,
        policy: &RestartPolicy,
    ) -> Option<std::time::Duration> {
        // Mark unavailable immediately so callers observe the degraded state
        // during the backoff window (Finding 3: init_status must not stay Fresh
        // while the process is crashed/restarting).
        self.health.init_status = legion_protocol::LspResultStatus::Unavailable;
        let attempt = self.health.restart_count;
        if policy.is_exhausted(attempt) {
            return None;
        }
        self.health.restart_count = attempt + 1;
        Some(policy.backoff_for_attempt(attempt))
    }
}
