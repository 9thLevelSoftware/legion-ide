//! Language-server launch + handshake orchestration (WS-LANG-01 LANG.03/04).
//!
//! Owns a live [`LspStdioSession`] and the [`LspServerHealthRecord`] that tracks
//! binary provenance, handshake status, and runtime health.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use legion_lsp::{
    DiscoveredBinary, LspNodeVersion, LspStdioSession, LspStdioSpawner, LspSupervisorConfig,
};
use legion_protocol::{
    CapabilityDecisionId, FileFingerprint, LanguageId, LanguageServerId, LspCapabilitySummary,
    LspResultStatus, LspServerBinaryProvenance, LspServerHealthRecord, SnapshotId,
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

/// Errors raised while launching or initializing a language-server session.
#[derive(Debug)]
pub enum LanguageSessionError {
    /// No binary could be discovered through any resolution source.
    Discovery,
    /// Supplied launch metadata does not match the supervisor policy identity
    /// or is incomplete. This is checked before the launcher is touched.
    InvalidConfiguration(String),
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
            LanguageSessionError::Discovery => write!(f, "language-server binary not found"),
            LanguageSessionError::InvalidConfiguration(e) => {
                write!(f, "language-server launch configuration is invalid: {e}")
            }
            LanguageSessionError::Launch(e) => write!(f, "language-server launch failed: {e}"),
            LanguageSessionError::Handshake(e) => {
                write!(f, "language-server handshake failed: {e}")
            }
            LanguageSessionError::ReadRequest(e) => {
                write!(f, "language-server read request failed: {e}")
            }
            LanguageSessionError::Unavailable => {
                write!(
                    f,
                    "language-server session is not initialized or is in backoff"
                )
            }
        }
    }
}

impl std::error::Error for LanguageSessionError {}

/// Validated metadata and supervisor inputs for launching any language server.
pub struct LanguageServerLaunchConfig {
    /// Supervisor / process config. Its policy identity is authoritative.
    pub supervisor: LspSupervisorConfig,
    /// Server identity, which must match the supervisor policy identity.
    pub server_id: LanguageServerId,
    /// Language identity, which must match the supervisor policy identity.
    pub language_id: LanguageId,
    /// Provenance of the approved binary.
    pub binary_provenance: LspServerBinaryProvenance,
    /// Optional hash of the approved artifact.
    pub artifact_hash: Option<FileFingerprint>,
    /// Additional verified artifact identities required by the launch, such
    /// as the compiler paired with an offline TypeScript server bundle.
    pub artifact_dependencies: Vec<FileFingerprint>,
    /// Optional version of the approved language server package.
    pub version: Option<String>,
    /// Observed Node runtime version used to launch a downloaded server.
    pub node_runtime_version: Option<LspNodeVersion>,
    /// Optional decision authorizing a downloaded artifact.
    pub download_decision_id: Option<CapabilityDecisionId>,
}

/// Inputs for launching the rust-analyzer compatibility adapter.
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

/// Owns a live language-neutral stdio session and its health record.
pub struct LanguageServerSession {
    session: LspStdioSession,
    health: LspServerHealthRecord,
    /// Bounded exact command identifiers advertised by executeCommandProvider.
    execute_command_ids: Vec<String>,
    /// Shared ring buffer populated by the background stderr drain thread
    /// spawned in `startup_session()`.  Callers clone the `Arc` to extract
    /// the ring for projection (PKT-LSP-C T4).
    pub(crate) stderr_ring: Arc<Mutex<VecDeque<String>>>,
}

/// Compatibility name retained for existing Rust-analyzer callers.
pub type RustAnalyzerSession = LanguageServerSession;

/// Capability keys the read side gates requests on.
///
/// One list, because this was two: the parser recorded three keys while
/// `issue_lsp_read` checked nine, so five requests could never be sent. Adding
/// a gated request means adding its key here, and
/// `tests/lsp_capability_gating.rs` fails when the two drift.
const GATED_READ_CAPABILITIES: &[&str] = &[
    "hoverProvider",
    "definitionProvider",
    "completionProvider",
    "referencesProvider",
    "documentSymbolProvider",
    "inlayHintProvider",
    "codeLensProvider",
    "callHierarchyProvider",
];

/// Edit-producing provider capabilities admitted only when initialize
/// explicitly advertises them. Object-valued options count as supported;
/// null, false, and absence remain unsupported.
const GATED_WRITE_CAPABILITIES: &[&str] = &[
    "renameProvider",
    "documentFormattingProvider",
    "codeActionProvider",
    "executeCommandProvider",
];

const EXECUTE_COMMAND_ID_LIMIT: usize = 128;
const EXECUTE_COMMAND_ID_MAX_BYTES: usize = 256;

impl LanguageServerSession {
    /// Launches a server from supplied, already-approved metadata.
    pub fn launch_configured(
        config: LanguageServerLaunchConfig,
        launcher: &mut impl LspStdioSpawner,
    ) -> Result<Self, LanguageSessionError> {
        let policy_identity = &config.supervisor.launch_policy.identity;
        if config.server_id.0 == 0 {
            return Err(LanguageSessionError::InvalidConfiguration(
                "server_id must be nonzero".to_string(),
            ));
        }
        if config.language_id.0.trim().is_empty() {
            return Err(LanguageSessionError::InvalidConfiguration(
                "language_id must be nonempty".to_string(),
            ));
        }
        if policy_identity.workspace_id.0 == 0 {
            return Err(LanguageSessionError::InvalidConfiguration(
                "identity workspace_id must be nonzero".to_string(),
            ));
        }
        let Some(root_id) = policy_identity.root_id else {
            return Err(LanguageSessionError::InvalidConfiguration(
                "identity root_id is required".to_string(),
            ));
        };
        if root_id.0 == 0 {
            return Err(LanguageSessionError::InvalidConfiguration(
                "identity root_id must be nonzero".to_string(),
            ));
        }
        let policy = &config.supervisor.launch_policy;
        let posture = &policy.posture;
        if posture.workspace_id.0 == 0 {
            return Err(LanguageSessionError::InvalidConfiguration(
                "posture workspace_id must be nonzero".to_string(),
            ));
        }
        if policy_identity.workspace_id != posture.workspace_id {
            return Err(LanguageSessionError::InvalidConfiguration(
                "identity and posture workspace_id differ".to_string(),
            ));
        }
        if posture.required_capability.0 != "lsp.launch" {
            return Err(LanguageSessionError::InvalidConfiguration(
                "launch posture must authorize lsp.launch".to_string(),
            ));
        }
        if posture.decision_id.is_none_or(|decision| decision.0 == 0) {
            return Err(LanguageSessionError::InvalidConfiguration(
                "lsp.launch decision_id must be nonzero".to_string(),
            ));
        }
        // Re-run the protocol policy derivation instead of approximating its
        // lifecycle with a boolean. In particular, a trusted/privacy-allowed
        // posture with runtime activation deferred is `RuntimeActivationDeferred`,
        // while an untrusted or privacy-denied posture has a different refusal
        // disposition and reason code.
        let expected_policy = legion_protocol::LspLaunchPolicyDecision::evaluate(
            policy_identity.clone(),
            posture.clone(),
            policy.runtime_activation_accepted,
            policy.correlation_id,
            policy.causality_id,
            policy.diagnostics.clone(),
            policy.schema_version,
        );
        if policy.disposition != expected_policy.disposition
            || policy.process_launch_allowed != expected_policy.process_launch_allowed
            || policy.reason_code != expected_policy.reason_code
        {
            return Err(LanguageSessionError::InvalidConfiguration(
                "launch policy disposition, posture, and allowed bit disagree".to_string(),
            ));
        }
        if config.supervisor.launch_policy.correlation_id.0 == 0 {
            return Err(LanguageSessionError::InvalidConfiguration(
                "launch policy correlation_id must be nonzero".to_string(),
            ));
        }
        if config.supervisor.launch_policy.causality_id.0.is_nil() {
            return Err(LanguageSessionError::InvalidConfiguration(
                "launch policy causality_id must be nonzero".to_string(),
            ));
        }
        if policy_identity.server_id != config.server_id {
            return Err(LanguageSessionError::InvalidConfiguration(
                "server_id does not match supervisor policy identity".to_string(),
            ));
        }
        if policy_identity.language_id != config.language_id {
            return Err(LanguageSessionError::InvalidConfiguration(
                "language_id does not match supervisor policy identity".to_string(),
            ));
        }
        if config
            .download_decision_id
            .is_some_and(|decision| decision.0 == 0)
        {
            return Err(LanguageSessionError::InvalidConfiguration(
                "download_decision_id must be nonzero".to_string(),
            ));
        }
        if config
            .version
            .as_ref()
            .is_some_and(|version| version.trim().is_empty())
        {
            return Err(LanguageSessionError::InvalidConfiguration(
                "version must be nonempty when supplied".to_string(),
            ));
        }
        if !matches!(
            config.binary_provenance,
            LspServerBinaryProvenance::Downloaded
        ) && config.node_runtime_version.is_some()
        {
            return Err(LanguageSessionError::InvalidConfiguration(
                "Node runtime version requires downloaded provenance".to_string(),
            ));
        }
        match config.binary_provenance {
            LspServerBinaryProvenance::Downloaded => {
                let Some(hash) = config.artifact_hash.as_ref() else {
                    return Err(LanguageSessionError::InvalidConfiguration(
                        "downloaded provenance requires an artifact hash".to_string(),
                    ));
                };
                if !is_sha256_fingerprint(hash) {
                    return Err(LanguageSessionError::InvalidConfiguration(
                        "downloaded artifact hash must be a 64-digit sha256 fingerprint"
                            .to_string(),
                    ));
                }
                if config
                    .artifact_dependencies
                    .iter()
                    .any(|dependency| !is_sha256_fingerprint(dependency))
                {
                    return Err(LanguageSessionError::InvalidConfiguration(
                        "downloaded artifact dependency must be a 64-digit sha256 fingerprint"
                            .to_string(),
                    ));
                }
            }
            _ if config.artifact_hash.is_some()
                || config.download_decision_id.is_some()
                || !config.artifact_dependencies.is_empty() =>
            {
                return Err(LanguageSessionError::InvalidConfiguration(
                    "artifact hash and download decision require downloaded provenance".to_string(),
                ));
            }
            _ => {}
        }

        // LspStdioSession performs the mandatory supervisor policy gate before
        // invoking the supplied launcher. This config is metadata, not an
        // approval receipt, and never grants process launch by itself.
        let session = LspStdioSession::start(config.supervisor, launcher)
            .map_err(LanguageSessionError::Launch)?;
        let health = LspServerHealthRecord {
            server_id: config.server_id,
            language_id: config.language_id,
            binary_provenance: config.binary_provenance,
            binary_path_hash: None,
            artifact_hash: config.artifact_hash,
            version: config.version,
            init_status: LspResultStatus::Unavailable,
            capabilities: Vec::new(),
            diagnostics_latency_ms: None,
            restart_count: 0,
            download_decision_id: config.download_decision_id,
            schema_version: LspServerHealthRecord::schema_version(),
        };
        Ok(Self {
            session,
            health,
            execute_command_ids: Vec::new(),
            stderr_ring: Arc::new(Mutex::new(VecDeque::new())),
        })
    }

    /// Resolves Rust-analyzer discovery, then delegates to the generic path.
    pub fn launch(
        config: RustAnalyzerLaunchConfig,
        launcher: &mut impl LspStdioSpawner,
    ) -> Result<Self, LanguageSessionError> {
        // Resolve provenance from discovery inputs (metadata-only — no path stored).
        let provenance: LspServerBinaryProvenance = match config.discovery.resolve() {
            DiscoveredBinary::Found { provenance, .. } => provenance,
            DiscoveredBinary::NotFound => return Err(LanguageSessionError::Discovery),
        };

        Self::launch_configured(
            LanguageServerLaunchConfig {
                supervisor: config.supervisor,
                server_id: config.server_id,
                language_id: config.language_id,
                binary_provenance: provenance,
                artifact_hash: None,
                artifact_dependencies: Vec::new(),
                version: None,
                node_runtime_version: None,
                download_decision_id: None,
            },
            launcher,
        )
    }

    /// Sends the LSP `initialize` request and the `initialized` notification,
    /// then updates `health.init_status` from the correlated response.
    pub fn initialize(&mut self, root_uri: &str) -> Result<(), LanguageSessionError> {
        self.initialize_with_options(root_uri, None, None)
    }

    /// [`initialize`] with explicit `initializationOptions` and client
    /// `capabilities`.
    ///
    /// `initialization_options`, when `Some`, is serialized as the LSP
    /// `initializationOptions` field (rust-analyzer reads its config from
    /// it — e.g. `{"files": {"watcher": "client"}}` disables the server-side
    /// `notify` file watcher, whose failure on temp workspace paths wedges
    /// RA's analysis loop; captured verbatim by the stderr ring during the
    /// GP-1 s3 investigation: "notify error: Input watch path is neither a
    /// file nor a directory").
    ///
    /// `client_capabilities`, when `Some`, is **merged over** the defaults
    /// from [`default_client_capabilities`] (e.g. advertising
    /// `workspace.didChangeWatchedFiles.dynamicRegistration` so a
    /// client-watcher config is honoured). Merging rather than replacing is
    /// deliberate: a caller adding one unrelated capability must not silently
    /// drop the pull-diagnostics advertisement, without which a conforming
    /// server may withhold `diagnosticProvider` and strand the client on the
    /// push-only path this default exists to avoid.
    pub fn initialize_with_options(
        &mut self,
        root_uri: &str,
        initialization_options: Option<serde_json::Value>,
        client_capabilities: Option<serde_json::Value>,
    ) -> Result<(), LanguageSessionError> {
        let params = build_initialize_params(root_uri, initialization_options, client_capabilities);

        let response = self
            .session
            .initialize(params, super::operation_context())
            .map_err(LanguageSessionError::Handshake)?;

        self.health.init_status = response.status;
        self.execute_command_ids.clear();

        // Parse capability summaries from the initialize result body.
        // Only populate when the handshake succeeded; an error result has no capabilities.
        if self.health.init_status == LspResultStatus::Fresh
            && let Some(caps) = response
                .result
                .get("capabilities")
                .and_then(|v| v.as_object())
        {
            // Every capability the read side gates on, from one list, because
            // two lists in two files drifted: `issue_lsp_read` refused to send
            // references, document symbols, inlay hints and code lenses for as
            // long as this loop named only three keys, and the gate cannot be
            // opened by a capability nobody recorded.
            //
            // Nothing caught it. The tests for those requests inject a health
            // record directly, so they asserted that a request fires when the
            // capability is present — true — and never that it was present.
            // In use it looked like a working feature, because a refused gate
            // falls back to the lexical index and the panel still fills.
            //
            // `tests/lsp_capability_gating.rs` now drives the real handshake
            // against a mock advertising all of them and fails if this list
            // falls behind again.
            for &cap_name in GATED_READ_CAPABILITIES {
                // `boolean | XOptions` per LSP: rust-analyzer sends an object
                // for several of these, and `as_bool()` on an object is `None`
                // — which read as "unsupported" and closed the gate on a server
                // that had just said yes.
                let supported = caps
                    .get(cap_name)
                    .map(|v| !v.is_null() && v.as_bool() != Some(false))
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
            for &cap_name in GATED_WRITE_CAPABILITIES {
                let supported = caps
                    .get(cap_name)
                    .map(|v| !v.is_null() && v.as_bool() != Some(false))
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
            self.execute_command_ids = parse_execute_command_ids(caps);
            let resolve_supported = caps
                .get("codeActionProvider")
                .and_then(|value| value.get("resolveProvider"))
                .map(|value| !value.is_null() && value.as_bool() != Some(false))
                .unwrap_or(false);
            self.health.capabilities.push(LspCapabilitySummary {
                capability: "codeActionResolveProvider".to_string(),
                supported: resolve_supported,
                dynamic_registration: false,
                option_hash: None,
                redaction_hints: Vec::new(),
                schema_version: 1,
            });
            // `diagnosticProvider` (LSP 3.17 pull diagnostics) is an object
            // capability, not a bool: present-and-not-false means supported.
            // rust-analyzer >= 1.96-era serves NATIVE diagnostics (type
            // errors, etc.) only through the pull channel; push
            // (`publishDiagnostics`) carries flycheck/cargo output, which
            // only refreshes on save. Callers use
            // [`supports_pull_diagnostics`] to pick the pull path.
            let pull_supported = caps
                .get("diagnosticProvider")
                .map(|v| !v.is_null() && v.as_bool() != Some(false))
                .unwrap_or(false);
            self.health.capabilities.push(LspCapabilitySummary {
                capability: "diagnosticProvider".to_string(),
                supported: pull_supported,
                dynamic_registration: false,
                option_hash: None,
                redaction_hints: Vec::new(),
                schema_version: 1,
            });
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

    /// Snapshot of the stdout reader thread's counters (frames forwarded,
    /// payload bytes, terminal event).  Discriminates "server truly silent"
    /// from "our reader thread died" in wedge post-mortems and drives the
    /// worker-loop transport-death detection (PKT-S3-WEDGE-R3).
    pub fn reader_stats(&self) -> legion_lsp::LspReaderStatsSnapshot {
        self.session.reader_stats()
    }

    /// Whether the language-server child process is still running.
    ///
    /// A dead reader with a live child means the transport died while the
    /// server survived; a dead child explains a clean-EOF reader terminal.
    pub fn is_running(&mut self) -> bool {
        self.session.is_running()
    }

    /// The child's exit status as a string once it has terminated (`None`
    /// while running).  Post-mortem evidence: a panic (101), a signal, and a
    /// clean exit (0) point at different death modes (PKT-S3-WEDGE-R3).
    pub fn exit_status_string(&mut self) -> Option<String> {
        self.session.exit_status_string()
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

    /// Whether the server advertised LSP 3.17 pull diagnostics
    /// (`diagnosticProvider`) in its initialize result.
    pub fn supports_pull_diagnostics(&self) -> bool {
        self.health
            .capabilities
            .iter()
            .any(|c| c.capability == "diagnosticProvider" && c.supported)
    }

    /// Returns whether the current initialize response advertised this exact
    /// execute-command identifier. The list is replaced on every initialize
    /// and bounded to keep server metadata from becoming an unbounded cache.
    pub fn supports_execute_command(&self, command_id: &str) -> bool {
        !command_id.is_empty()
            && command_id.len() <= EXECUTE_COMMAND_ID_MAX_BYTES
            && self
                .execute_command_ids
                .binary_search_by(|candidate| candidate.as_str().cmp(command_id))
                .is_ok()
    }

    /// Returns the bounded exact command identifiers from the current
    /// initialize response for app/session capability projection.
    pub(crate) fn execute_command_ids(&self) -> &[String] {
        &self.execute_command_ids
    }

    /// Issues a `textDocument/diagnostic` pull request (LSP 3.17) for `uri`
    /// and parses the document diagnostic report.
    ///
    /// No `previousResultId` is sent, so a conforming server always answers
    /// with a **full** report rather than `unchanged`. The request is ordered
    /// after any prior `didChange` on the same connection, so the report
    /// reflects the latest synced content.
    ///
    /// Returns [`LanguageSessionError::Unavailable`] if the session is not
    /// live. Transport/timeout failures surface as
    /// [`LanguageSessionError::ReadRequest`]; callers polling in a loop
    /// should treat those as retryable.
    pub fn pull_diagnostics_with_context(
        &mut self,
        uri: &str,
        operation_context: legion_protocol::LspOperationContext,
    ) -> Result<PulledDiagnostics, LanguageSessionError> {
        if self.health.init_status != legion_protocol::LspResultStatus::Fresh {
            return Err(LanguageSessionError::Unavailable);
        }
        let params = serde_json::json!({
            "textDocument": { "uri": uri },
        });
        let response = self
            .session
            .request(
                "textDocument/diagnostic".to_string(),
                params,
                operation_context,
            )
            .map_err(LanguageSessionError::ReadRequest)?;
        Ok(parse_pull_diagnostics_result(&response.result, uri))
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
    pub fn request_read_with_context(
        &mut self,
        method: &str,
        params: serde_json::Value,
        snapshot_id: SnapshotId,
        operation_context: Option<legion_protocol::LspOperationContext>,
    ) -> Result<LspReadOutcome, LanguageSessionError> {
        if self.health.init_status != legion_protocol::LspResultStatus::Fresh {
            return Err(LanguageSessionError::Unavailable);
        }
        let Some(ctx) = operation_context else {
            return Err(LanguageSessionError::InvalidConfiguration(
                "document LSP requests require an app-admitted operation context".to_string(),
            ));
        };
        if ctx.snapshot_id != snapshot_id {
            return Err(LanguageSessionError::InvalidConfiguration(
                "document LSP context snapshot does not match request snapshot".to_string(),
            ));
        }
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

    /// Sends `textDocument/didClose` for a buffer.
    pub fn did_close(&mut self, uri: &str) -> Result<(), LanguageSessionError> {
        if self.health.init_status != legion_protocol::LspResultStatus::Fresh {
            return Err(LanguageSessionError::Unavailable);
        }
        let params = serde_json::json!({
            "textDocument": {
                "uri": uri,
            }
        });
        self.session
            .send_notification("textDocument/didClose", params)
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
        std::thread::spawn(move || super::app_lsp::drain_stderr_reader(stderr, ring));
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

fn parse_execute_command_ids(
    capabilities: &serde_json::Map<String, serde_json::Value>,
) -> Vec<String> {
    let mut command_ids = Vec::new();
    let Some(commands) = capabilities
        .get("executeCommandProvider")
        .and_then(|value| value.get("commands"))
        .and_then(serde_json::Value::as_array)
    else {
        return command_ids;
    };
    for command in commands.iter().take(EXECUTE_COMMAND_ID_LIMIT) {
        let Some(command) = command.as_str() else {
            continue;
        };
        if command.is_empty() || command.len() > EXECUTE_COMMAND_ID_MAX_BYTES {
            continue;
        }
        command_ids.push(command.to_string());
    }
    command_ids.sort_unstable();
    command_ids.dedup();
    command_ids
}

fn is_sha256_fingerprint(fingerprint: &FileFingerprint) -> bool {
    fingerprint.algorithm.eq_ignore_ascii_case("sha256")
        && fingerprint.value.len() == 64
        && fingerprint
            .value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
}

/// Builds the LSP `initialize` request params.
/// Parsed summary of one `textDocument/diagnostic` (pull) response.
///
/// Counts only plus a synthesized `publishDiagnostics`-shaped params object,
/// so pull results can be routed through the exact same ingestion/projection
/// path as pushed notifications
/// (`AppComposition::ingest_lsp_publish_diagnostics_for_buffer`).
#[derive(Debug, Clone, PartialEq)]
pub struct PulledDiagnostics {
    /// True when the server answered with a `kind: "full"` report.
    pub kind_full: bool,
    /// Items with LSP severity Error (1) — or no severity, which the LSP
    /// spec tells clients to interpret as an error.
    pub error_count: usize,
    /// All items in the report.
    pub total_count: usize,
    /// `{"uri": .., "diagnostics": [..]}` synthesized from a full report;
    /// `None` for `unchanged`/malformed responses.
    pub publish_params: Option<serde_json::Value>,
}

/// Parses a `textDocument/diagnostic` result into [`PulledDiagnostics`].
///
/// Pure so the report taxonomy is unit-testable: full reports (with/without
/// errors), `unchanged` reports, null results, and shape surprises all
/// resolve deterministically without a live server.
fn parse_pull_diagnostics_result(result: &serde_json::Value, uri: &str) -> PulledDiagnostics {
    let kind_full = result.get("kind").and_then(|k| k.as_str()) == Some("full");
    if !kind_full {
        return PulledDiagnostics {
            kind_full: false,
            error_count: 0,
            total_count: 0,
            publish_params: None,
        };
    }
    let items: Vec<serde_json::Value> = result
        .get("items")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let error_count = items
        .iter()
        .filter(|item| match item.get("severity").and_then(|s| s.as_u64()) {
            Some(severity) => severity == 1,
            // LSP: "when absent, it is up to the client to interpret" — the
            // conservative reading for an error pump is "error".
            None => true,
        })
        .count();
    let total_count = items.len();
    let publish_params = Some(serde_json::json!({
        "uri": uri,
        "diagnostics": items,
    }));
    PulledDiagnostics {
        kind_full: true,
        error_count,
        total_count,
        publish_params,
    }
}

/// Default client capabilities advertised in `initialize`.
///
/// Advertises LSP 3.17 pull diagnostics (`textDocument.diagnostic`).
/// rust-analyzer 1.96+ serves native diagnostics (type errors, borrow errors)
/// only via the pull channel; push `publishDiagnostics` carries
/// flycheck/cargo output, which refreshes on save. Without this
/// advertisement a client that never saves sees no semantic errors at all
/// (GP-1 s3 wedge, reproduced 2026-08-15 on rust-analyzer 1.97.1 across all
/// three hosted OSes and locally).
fn default_client_capabilities() -> serde_json::Value {
    serde_json::json!({
        "textDocument": {
            "publishDiagnostics": {},
            "diagnostic": {
                "dynamicRegistration": false,
                "relatedDocumentSupport": false,
            },
        },
    })
}

///
/// Pure so the wire shape is unit-testable: `initializationOptions` is
/// emitted only when provided (the LSP spec makes it optional), and the
/// client `capabilities` object defaults to [`default_client_capabilities`]
/// (pull-diagnostics advertisement) when not provided.
fn build_initialize_params(
    root_uri: &str,
    initialization_options: Option<serde_json::Value>,
    client_capabilities: Option<serde_json::Value>,
) -> serde_json::Value {
    let mut capabilities = default_client_capabilities();
    if let Some(overrides) = client_capabilities {
        merge_json(&mut capabilities, overrides);
    }
    let mut params = serde_json::json!({
        "processId": std::process::id(),
        "rootUri": root_uri,
        "capabilities": capabilities,
        "workspaceFolders": [{ "uri": root_uri, "name": "workspace" }],
    });
    if let Some(options) = initialization_options {
        params["initializationOptions"] = options;
    }
    params
}

/// Recursively merge `overlay` into `base`; non-object values at the same key
/// replace, so a caller can still override a specific default outright.
fn merge_json(base: &mut serde_json::Value, overlay: serde_json::Value) {
    match (base, overlay) {
        (serde_json::Value::Object(base), serde_json::Value::Object(overlay)) => {
            for (key, value) in overlay {
                match base.get_mut(&key) {
                    Some(existing) => merge_json(existing, value),
                    None => {
                        base.insert(key, value);
                    }
                }
            }
        }
        (base, overlay) => *base = overlay,
    }
}

#[cfg(test)]
mod initialize_params_tests {
    use super::build_initialize_params;

    #[test]
    fn default_params_advertise_pull_diagnostics_and_no_initialization_options() {
        let params = build_initialize_params("file:///tmp/ws", None, None);
        assert_eq!(params["rootUri"], "file:///tmp/ws");
        // Pull diagnostics (LSP 3.17) must be advertised by default —
        // rust-analyzer 1.96+ serves native diagnostics only via pull.
        assert!(
            params["capabilities"]["textDocument"]["diagnostic"].is_object(),
            "default capabilities must advertise textDocument.diagnostic"
        );
        assert!(
            params.get("initializationOptions").is_none(),
            "initializationOptions must be omitted (not null) when unset"
        );
        assert_eq!(params["workspaceFolders"][0]["uri"], "file:///tmp/ws");
    }

    #[test]
    fn pull_result_full_report_counts_errors_and_synthesizes_publish_params() {
        let result = serde_json::json!({
            "kind": "full",
            "resultId": "r1",
            "items": [
                {"range": {"start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 1}}, "severity": 1, "message": "boom"},
                {"range": {"start": {"line": 1, "character": 0}, "end": {"line": 1, "character": 1}}, "severity": 2, "message": "warn"},
                {"range": {"start": {"line": 2, "character": 0}, "end": {"line": 2, "character": 1}}, "message": "no severity — reads as error"},
            ],
        });
        let pulled = super::parse_pull_diagnostics_result(&result, "file:///tmp/a.rs");
        assert!(pulled.kind_full);
        assert_eq!(pulled.error_count, 2);
        assert_eq!(pulled.total_count, 3);
        let params = pulled
            .publish_params
            .expect("full report synthesizes params");
        assert_eq!(params["uri"], "file:///tmp/a.rs");
        assert_eq!(params["diagnostics"].as_array().unwrap().len(), 3);
    }

    #[test]
    fn pull_result_clean_full_report_has_zero_errors_with_params() {
        let result = serde_json::json!({"kind": "full", "resultId": "r2", "items": []});
        let pulled = super::parse_pull_diagnostics_result(&result, "file:///tmp/a.rs");
        assert!(pulled.kind_full);
        assert_eq!(pulled.error_count, 0);
        assert_eq!(pulled.total_count, 0);
        assert!(pulled.publish_params.is_some());
    }

    #[test]
    fn pull_result_unchanged_and_malformed_are_not_full() {
        for result in [
            serde_json::json!({"kind": "unchanged", "resultId": "r1"}),
            serde_json::json!(null),
            serde_json::json!({"unexpected": true}),
        ] {
            let pulled = super::parse_pull_diagnostics_result(&result, "file:///tmp/a.rs");
            assert!(!pulled.kind_full);
            assert_eq!(pulled.error_count, 0);
            assert!(pulled.publish_params.is_none());
        }
    }

    #[test]
    fn initialization_options_are_serialized_verbatim() {
        let options = serde_json::json!({"files": {"watcher": "client"}});
        let params = build_initialize_params("file:///tmp/ws", Some(options.clone()), None);
        assert_eq!(params["initializationOptions"], options);
    }

    #[test]
    fn client_capabilities_merge_over_the_defaults() {
        // GP-1 passes an override carrying only a watcher capability. Merging
        // (not replacing) is what keeps pull diagnostics advertised there.
        let caps = serde_json::json!({
            "workspace": {"didChangeWatchedFiles": {"dynamicRegistration": true}}
        });
        let params = build_initialize_params("file:///tmp/ws", None, Some(caps.clone()));
        assert_eq!(
            params["capabilities"]["workspace"]["didChangeWatchedFiles"]["dynamicRegistration"],
            true,
            "caller capability is present"
        );
        assert!(
            params["capabilities"]["textDocument"]["diagnostic"].is_object(),
            "an unrelated override must not drop the pull-diagnostics default"
        );
    }

    #[test]
    fn a_caller_can_still_override_a_specific_default() {
        let caps = serde_json::json!({
            "textDocument": {"diagnostic": {"dynamicRegistration": true}}
        });
        let params = build_initialize_params("file:///tmp/ws", None, Some(caps));
        assert_eq!(
            params["capabilities"]["textDocument"]["diagnostic"]["dynamicRegistration"], true,
            "explicit overrides still win over the default value"
        );
    }

    #[test]
    fn merge_replaces_non_object_values() {
        let caps = serde_json::json!({"textDocument": {"publishDiagnostics": false}});
        let params = build_initialize_params("file:///tmp/ws", None, Some(caps));
        assert_eq!(
            params["capabilities"]["textDocument"]["publishDiagnostics"], false,
            "a scalar overlay replaces the default object outright"
        );
    }
}

#[cfg(test)]
mod configured_launch_tests {
    use super::*;
    use legion_lsp::{LspServerProcessConfig, LspStdioLauncher, LspStdioProcess};
    use legion_protocol::{
        CapabilityId, CausalityId, CorrelationId, LspConfiguredServerIdentity,
        LspLaunchPolicyDecision, LspWorkspaceTrustPosture, RedactionHint, SemanticPrivacyScope,
        WorkspaceId, WorkspaceRootId, WorkspaceTrustState,
    };

    struct MustNotSpawn;

    impl LspStdioSpawner for MustNotSpawn {
        fn spawn_stdio(
            &mut self,
            _config: &LspServerProcessConfig,
        ) -> legion_lsp::LspRuntimeResult<LspStdioProcess> {
            panic!("invalid configuration reached process spawn")
        }
    }

    struct FailsToSpawn;

    impl LspStdioSpawner for FailsToSpawn {
        fn spawn_stdio(
            &mut self,
            _config: &LspServerProcessConfig,
        ) -> legion_lsp::LspRuntimeResult<LspStdioProcess> {
            Err(legion_lsp::LspRuntimeError::SpawnFailed {
                code: "test.no_process".to_string(),
            })
        }
    }

    struct RecordingSpawner {
        inner: LspStdioLauncher,
        command: Option<String>,
        args: Vec<String>,
    }

    impl LspStdioSpawner for RecordingSpawner {
        fn spawn_stdio(
            &mut self,
            config: &LspServerProcessConfig,
        ) -> legion_lsp::LspRuntimeResult<LspStdioProcess> {
            self.command = Some(config.command.clone());
            self.args = config.args.clone();
            self.inner.spawn_stdio(config)
        }
    }

    fn config(server_id: LanguageServerId, language_id: &str) -> LanguageServerLaunchConfig {
        let identity = LspConfiguredServerIdentity {
            server_id: LanguageServerId(7),
            workspace_id: WorkspaceId(55),
            root_id: Some(WorkspaceRootId(5)),
            language_id: LanguageId("python".to_string()),
            display_name: "pyright".to_string(),
            command_hash: FileFingerprint {
                algorithm: "test".to_string(),
                value: "command".to_string(),
            },
            args_hash: None,
            env_hash: None,
            cwd_hash: None,
            settings_hash: None,
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        };
        let policy = LspLaunchPolicyDecision::evaluate(
            identity,
            LspWorkspaceTrustPosture {
                workspace_id: WorkspaceId(55),
                workspace_trust_state: WorkspaceTrustState::Trusted,
                privacy_scope: SemanticPrivacyScope::Workspace,
                privacy_scope_allowed: true,
                required_capability: CapabilityId("lsp.launch".to_string()),
                // Process-spawn approval is distinct from any download
                // decision carried by the launch metadata below.
                decision_id: Some(CapabilityDecisionId(2)),
                diagnostics: Vec::new(),
                schema_version: 1,
            },
            true,
            CorrelationId(1),
            CausalityId(uuid::Uuid::from_u128(1)),
            Vec::new(),
            1,
        );
        LanguageServerLaunchConfig {
            supervisor: LspSupervisorConfig {
                launch_policy: policy,
                process: LspServerProcessConfig {
                    command: "pyright-langserver".to_string(),
                    args: vec!["--stdio".to_string()],
                    cwd: None,
                    env: Vec::new(),
                },
                initial_backoff_ms: 1,
                max_backoff_ms: 2,
                max_restart_attempts: 1,
            },
            server_id,
            language_id: LanguageId(language_id.to_string()),
            binary_provenance: LspServerBinaryProvenance::Downloaded,
            artifact_hash: Some(FileFingerprint {
                algorithm: "sha256".to_string(),
                value: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
                    .to_string(),
            }),
            artifact_dependencies: Vec::new(),
            version: Some("1.2.3".to_string()),
            node_runtime_version: None,
            // Local verified artifact import has no network/download decision;
            // the separate lsp.launch decision is carried by posture.
            download_decision_id: None,
        }
    }

    #[test]
    fn generic_config_rejects_identity_mismatch_before_spawn() {
        let mut launcher = MustNotSpawn;
        let result = LanguageServerSession::launch_configured(
            config(LanguageServerId(8), "python"),
            &mut launcher,
        );
        assert!(matches!(
            result,
            Err(LanguageSessionError::InvalidConfiguration(_))
        ));
    }

    #[test]
    fn generic_config_rejects_empty_language_before_spawn() {
        let mut launcher = MustNotSpawn;
        let result = LanguageServerSession::launch_configured(
            config(LanguageServerId(7), " "),
            &mut launcher,
        );
        assert!(matches!(
            result,
            Err(LanguageSessionError::InvalidConfiguration(_))
        ));
    }

    #[test]
    fn generic_config_rejects_malformed_artifact_dependency_before_spawn() {
        let mut invalid = config(LanguageServerId(7), "python");
        invalid.artifact_dependencies.push(FileFingerprint {
            algorithm: "sha256".to_string(),
            value: "not-a-digest".to_string(),
        });
        let mut launcher = MustNotSpawn;
        assert!(matches!(
            LanguageServerSession::launch_configured(invalid, &mut launcher),
            Err(LanguageSessionError::InvalidConfiguration(reason))
                if reason.contains("artifact dependency")
        ));
    }

    #[test]
    fn generic_config_rejects_zero_or_cross_workspace_metadata_before_spawn() {
        let mut cases = Vec::new();
        let mut zero_identity_workspace = config(LanguageServerId(7), "python");
        zero_identity_workspace
            .supervisor
            .launch_policy
            .identity
            .workspace_id = WorkspaceId(0);
        cases.push(zero_identity_workspace);
        let mut zero_root = config(LanguageServerId(7), "python");
        zero_root.supervisor.launch_policy.identity.root_id = Some(WorkspaceRootId(0));
        cases.push(zero_root);
        let mut mismatch = config(LanguageServerId(7), "python");
        mismatch.supervisor.launch_policy.posture.workspace_id = WorkspaceId(56);
        cases.push(mismatch);

        for invalid in cases {
            let mut launcher = MustNotSpawn;
            assert!(matches!(
                LanguageServerSession::launch_configured(invalid, &mut launcher),
                Err(LanguageSessionError::InvalidConfiguration(_))
            ));
        }
    }

    #[test]
    fn generic_config_rejects_missing_or_wrong_process_spawn_decision_before_spawn() {
        let mut missing = config(LanguageServerId(7), "python");
        missing.supervisor.launch_policy.posture.decision_id = None;
        let mut wrong_capability = config(LanguageServerId(7), "python");
        wrong_capability
            .supervisor
            .launch_policy
            .posture
            .required_capability = CapabilityId("network.fetch".to_string());

        for invalid in [missing, wrong_capability] {
            let mut launcher = MustNotSpawn;
            assert!(matches!(
                LanguageServerSession::launch_configured(invalid, &mut launcher),
                Err(LanguageSessionError::InvalidConfiguration(_))
            ));
        }
    }

    #[test]
    fn generic_config_rejects_contradictory_policy_before_spawn() {
        let mut invalid = config(LanguageServerId(7), "python");
        invalid.supervisor.launch_policy.disposition =
            legion_protocol::LspLaunchDisposition::DisabledCapabilityDenied;
        let mut launcher = MustNotSpawn;
        assert!(matches!(
            LanguageServerSession::launch_configured(invalid, &mut launcher),
            Err(LanguageSessionError::InvalidConfiguration(_))
        ));
    }

    #[test]
    fn generic_config_keeps_denied_policy_fail_closed_without_spawning() {
        let mut denied = config(LanguageServerId(7), "python");
        denied
            .supervisor
            .launch_policy
            .posture
            .workspace_trust_state = legion_protocol::WorkspaceTrustState::Untrusted;
        let policy = &denied.supervisor.launch_policy;
        let identity = policy.identity.clone();
        let posture = policy.posture.clone();
        let runtime_activation_accepted = policy.runtime_activation_accepted;
        let correlation_id = policy.correlation_id;
        let causality_id = policy.causality_id;
        let diagnostics = policy.diagnostics.clone();
        let schema_version = policy.schema_version;
        denied.supervisor.launch_policy = legion_protocol::LspLaunchPolicyDecision::evaluate(
            identity,
            posture,
            runtime_activation_accepted,
            correlation_id,
            causality_id,
            diagnostics,
            schema_version,
        );
        let mut launcher = MustNotSpawn;
        assert!(matches!(
            LanguageServerSession::launch_configured(denied, &mut launcher),
            Err(LanguageSessionError::Launch(
                legion_lsp::LspRuntimeError::SupervisionRefused { .. }
            ))
        ));
    }

    #[test]
    fn generic_config_rejects_forged_privacy_denial_disposition() {
        let mut forged = config(LanguageServerId(7), "python");
        forged.supervisor.launch_policy.runtime_activation_accepted = false;
        forged.supervisor.launch_policy.disposition =
            legion_protocol::LspLaunchDisposition::DisabledPrivacyDenied;
        forged.supervisor.launch_policy.reason_code =
            "lsp.supervision.disabled.privacy_denied".to_string();
        forged.supervisor.launch_policy.process_launch_allowed = false;
        let mut launcher = MustNotSpawn;
        assert!(matches!(
            LanguageServerSession::launch_configured(forged, &mut launcher),
            Err(LanguageSessionError::InvalidConfiguration(_))
        ));
    }

    #[test]
    fn evaluated_runtime_deferred_policy_reaches_supervisor_without_spawning() {
        let mut deferred = config(LanguageServerId(7), "python");
        deferred
            .supervisor
            .launch_policy
            .runtime_activation_accepted = false;
        deferred.supervisor.launch_policy.disposition =
            legion_protocol::LspLaunchDisposition::RuntimeActivationDeferred;
        deferred.supervisor.launch_policy.reason_code =
            "lsp.supervision.runtime_deferred".to_string();
        deferred.supervisor.launch_policy.process_launch_allowed = false;
        let mut launcher = MustNotSpawn;
        assert!(matches!(
            LanguageServerSession::launch_configured(deferred, &mut launcher),
            Err(LanguageSessionError::Launch(
                legion_lsp::LspRuntimeError::SupervisionRefused { .. }
            ))
        ));
    }

    #[test]
    fn generic_config_requires_download_metadata_consistency() {
        let mut missing_hash = config(LanguageServerId(7), "python");
        missing_hash.artifact_hash = None;
        let mut invalid_hash = config(LanguageServerId(7), "python");
        invalid_hash.artifact_hash = Some(FileFingerprint {
            algorithm: "sha256".to_string(),
            value: "artifact".to_string(),
        });
        let mut local_with_download_metadata = config(LanguageServerId(7), "python");
        local_with_download_metadata.binary_provenance = LspServerBinaryProvenance::Configured;
        local_with_download_metadata.download_decision_id = Some(CapabilityDecisionId(9));
        let mut local_offline = config(LanguageServerId(7), "python");
        local_offline.binary_provenance = LspServerBinaryProvenance::Configured;
        local_offline.artifact_hash = None;
        local_offline.download_decision_id = None;

        for invalid in [missing_hash, invalid_hash, local_with_download_metadata] {
            let mut launcher = MustNotSpawn;
            assert!(matches!(
                LanguageServerSession::launch_configured(invalid, &mut launcher),
                Err(LanguageSessionError::InvalidConfiguration(_))
            ));
        }

        // A verified local/offline artifact has no network download decision;
        // lsp.launch approval remains in the posture decision above.
        let mut launcher = FailsToSpawn;
        let result = LanguageServerSession::launch_configured(local_offline, &mut launcher);
        assert!(matches!(result, Err(LanguageSessionError::Launch(_))));
    }

    #[test]
    fn generic_python_launch_preserves_metadata_args_and_read_route() {
        let current_exe = std::env::current_exe().expect("test executable path");
        let profile_dir = current_exe
            .parent()
            .and_then(std::path::Path::parent)
            .expect("target profile directory");
        let mock_name = if cfg!(windows) {
            "mock_lsp_server.exe"
        } else {
            "mock_lsp_server"
        };
        let mock_path = profile_dir.join(mock_name);
        assert!(
            mock_path.is_file(),
            "mock_lsp_server not found at {}; run `cargo build -p legion-lsp --bin mock_lsp_server` first",
            mock_path.display()
        );

        let mut config = config(LanguageServerId(7), "python");
        config.supervisor.process.command = mock_path.to_string_lossy().into_owned();
        config.supervisor.process.args = vec!["--stdio".to_string(), "--python".to_string()];
        let expected_artifact = config.artifact_hash.clone();
        let expected_version = config.version.clone();
        let expected_decision = config.download_decision_id;
        let expected_command = config.supervisor.process.command.clone();
        let mut launcher = RecordingSpawner {
            inner: LspStdioLauncher::new(),
            command: None,
            args: Vec::new(),
        };
        let mut session = LanguageServerSession::launch_configured(config, &mut launcher)
            .expect("generic configured launch should succeed");
        assert_eq!(launcher.command.as_deref(), Some(expected_command.as_str()));
        assert_eq!(launcher.args, vec!["--stdio", "--python"]);
        assert_eq!(session.health().server_id, LanguageServerId(7));
        assert_eq!(
            session.health().language_id,
            LanguageId("python".to_string())
        );
        assert_eq!(
            session.health().binary_provenance,
            LspServerBinaryProvenance::Downloaded
        );
        assert_eq!(session.health().artifact_hash, expected_artifact);
        assert_eq!(session.health().version, expected_version);
        assert_eq!(session.health().download_decision_id, expected_decision);

        session.initialize("file:///workspace").expect("initialize");
        let outcome = session
            .request_read_with_context(
                "textDocument/completion",
                serde_json::json!({"textDocument": {"uri": "file:///workspace/main.py"}}),
                SnapshotId(11),
                Some(super::super::operation_context_for_snapshot(SnapshotId(11))),
            )
            .expect("completion request should route through generic session");
        assert_eq!(outcome.issued_snapshot, SnapshotId(11));
        assert_eq!(outcome.result["items"][0]["label"], "mockCompletion");
    }
}

#[cfg(test)]
mod execute_command_capability_tests {
    use super::parse_execute_command_ids;

    #[test]
    fn parses_exact_commands_skips_malformed_and_enforces_bound() {
        let mut commands = vec![
            serde_json::json!("rust-analyzer.reload"),
            serde_json::json!("rust-analyzer.organizeImports"),
            serde_json::json!(null),
            serde_json::json!(42),
            serde_json::json!(""),
            serde_json::json!("x".repeat(257)),
        ];
        commands.extend((0..130).map(|index| serde_json::json!(format!("test.command.{index}"))));
        let capabilities = serde_json::json!({
            "executeCommandProvider": { "commands": commands }
        });
        let ids = parse_execute_command_ids(capabilities.as_object().expect("object"));
        // The parser bounds the wire array it examines at 128 entries.  The
        // malformed records occupy four of those slots, so only 124 valid
        // identifiers are retained from this deliberately adversarial input.
        assert_eq!(ids.len(), 124);
        assert!(ids.iter().any(|id| id == "rust-analyzer.reload"));
        assert!(ids.iter().any(|id| id == "rust-analyzer.organizeImports"));
        assert!(!ids.iter().any(|id| id.is_empty()));
        assert!(!ids.iter().any(|id| id.len() > 256));
        assert!(!ids.iter().any(|id| id == "unknown.command"));
    }

    #[test]
    fn absent_or_malformed_provider_has_no_exact_commands() {
        for capabilities in [
            serde_json::json!({}),
            serde_json::json!({ "executeCommandProvider": false }),
            serde_json::json!({ "executeCommandProvider": { "commands": [null, 7] } }),
        ] {
            assert!(
                parse_execute_command_ids(capabilities.as_object().expect("object")).is_empty()
            );
        }
    }
}

#[cfg(test)]
mod stderr_drain_tests {
    use super::super::app_lsp::drain_stderr_reader;
    use std::collections::VecDeque;
    use std::sync::{Arc, Mutex};

    #[test]
    fn shared_stderr_drain_handles_utf8_boundary_without_panicking() {
        let mut input = vec![b'x'; 511];
        input.extend_from_slice("€\n".as_bytes());
        let ring = Arc::new(Mutex::new(VecDeque::new()));
        drain_stderr_reader(&input[..], ring.clone());

        let lines = ring.lock().expect("stderr ring lock");
        assert_eq!(lines.len(), 1);
        assert!(lines[0].ends_with('…'));
        assert!(lines[0].len() <= 512);
    }
}
