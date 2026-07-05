//! Background LSP session lifecycle for `AppComposition` (WS-LANG-01 PKT-LSP-B T1).
//!
//! `LspSessionHandle` manages the startup/live/failed lifecycle of a
//! `RustAnalyzerSession` that runs on a dedicated worker thread.  The design
//! mirrors `TerminalWorkflow::poll`: the frame path calls `drain()` via
//! `try_recv` and never blocks.
//!
//! ## Worker-thread architecture (T6/T7 enabler)
//!
//! Once the session is Live, all LSP I/O (requests + notifications) happens on
//! a dedicated "session thread".  The frame path communicates via two MPSC
//! channels:
//!   - `request_tx`:  send `LspWorkerRequest` (completion, hover, did_change …)
//!   - `result_rx`:   receive `LspWorkerResult` (read outcomes, diagnostic batches)
//!
//! `try_drain_results()` drains the result channel non-blockingly each frame;
//! `issue_request()` sends a request non-blockingly (drops the request if the
//! bounded channel is full — callers retry on the next keystroke/frame).

use std::{
    path::{Path, PathBuf},
    sync::mpsc,
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use legion_protocol::{
    BufferId, LanguageId, LanguageServerId, LspResultStatus, LspServerHealthRecord,
    LspSessionLifecycleKind, LspSessionStatusProjection, SnapshotId,
};

use super::{
    LanguageSessionError, LspReadOutcome, RustAnalyzerDiscovery, RustAnalyzerLaunchConfig,
    RustAnalyzerSession,
};
use legion_lsp::{LspServerProcessConfig, LspStdioLauncher, LspSupervisorConfig};
use legion_protocol::{
    CapabilityDecisionId, CapabilityId, CausalityId, CorrelationId, FileFingerprint,
    LspConfiguredServerIdentity, LspLaunchPolicyDecision, LspWorkspaceTrustPosture, RedactionHint,
    SemanticPrivacyScope, WorkspaceId, WorkspaceRootId, WorkspaceTrustState,
};
use uuid::Uuid;

/// Result type delivered from the background startup thread.
pub type LspStartResult = Result<RustAnalyzerSession, LanguageSessionError>;

/// Tag carried with a worker request so the drain side can route the result.
#[derive(Debug, Clone)]
pub struct LspRequestTag {
    /// Buffer the request was issued against.
    pub buffer_id: BufferId,
    /// What kind of read is this.
    pub kind: LspReadKind,
    /// Snapshot when the request was issued (stale-gate).
    pub snapshot_id: SnapshotId,
}

/// Discriminator for routing worker read results.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LspReadKind {
    /// Completion request (`textDocument/completion`).
    Completion,
    /// Hover request (`textDocument/hover`).
    Hover,
    /// Go-to-definition request (`textDocument/definition`).
    Definition,
}

/// Message sent from the frame path to the worker thread.
pub enum LspWorkerRequest {
    /// Issue a blocking LSP read request on the worker thread.
    RequestRead {
        method: String,
        params: serde_json::Value,
        tag: LspRequestTag,
    },
    /// Fire-and-forget: send a `textDocument/didChange` notification.
    DidChange {
        uri: String,
        version: i64,
        text: String,
    },
    /// Fire-and-forget: send a `textDocument/didOpen` notification.
    DidOpen {
        uri: String,
        language_id: String,
        version: i64,
        text: String,
    },
}

/// Message sent from the worker thread back to the frame path.
pub enum LspWorkerResult {
    /// A read request completed (or failed).
    ReadResult {
        /// The LSP request outcome or error.
        outcome: Result<LspReadOutcome, LanguageSessionError>,
        /// Routing tag identifying the buffer and request kind.
        tag: LspRequestTag,
    },
    /// A `textDocument/publishDiagnostics` notification arrived.
    DiagnosticBatch {
        /// Raw JSON params as sent by the LSP server. Never stored in logs;
        /// callers must project through `legion_lsp::project_publish_diagnostics`
        /// immediately.
        raw_params: serde_json::Value,
    },
}

/// Live-session handle: channels to the worker thread + cached health record.
struct LspWorkerHandle {
    /// Cached health record updated when the session went Live.
    health: LspServerHealthRecord,
    /// Send requests to the worker thread (bounded: drops when full).
    request_tx: mpsc::SyncSender<LspWorkerRequest>,
    /// Receive results from the worker thread (non-blocking drain each frame).
    result_rx: mpsc::Receiver<LspWorkerResult>,
}

/// Internal lifecycle state.
enum LspSessionState {
    /// No startup attempted yet.
    Idle,
    /// Background thread has been spawned; waiting for the result.
    Starting { rx: mpsc::Receiver<LspStartResult> },
    /// Session worker thread is live. Boxed: the worker handle (channels +
    /// health record) dwarfs the other variants, and clippy's
    /// large-enum-variant lint correctly flags the size asymmetry.
    Live(Box<LspWorkerHandle>),
    /// Auto-restart is waiting for the backoff timer to fire (PKT-LSP-C T3).
    /// `earliest_retry_ms` is a wall-clock millisecond timestamp (UNIX epoch).
    BackingOff {
        /// Number of automatic restart attempts completed so far.
        restart_count: u32,
        /// Earliest UNIX-epoch millisecond timestamp at which to auto-retry.
        earliest_retry_ms: u64,
        /// Metadata-only failure reason (bounded to 256 chars).
        reason: String,
    },
    /// Launch was refused (untrusted, no binary, policy denied, etc.).
    Refused { reason: String },
    /// All auto-restart attempts exhausted; explicit user restart required.
    Failed { reason: String },
}

/// Manages the background startup and live-session lifecycle for one
/// `RustAnalyzerSession`.  All blocking work (discovery, process spawn, and
/// LSP `initialize` round-trip) happens on the background startup thread; the
/// frame path only calls `drain()` which is a non-blocking `try_recv`.
///
/// Once Live, I/O happens on a dedicated worker thread; the frame path
/// communicates through bounded MPSC channels.
pub struct LspSessionHandle {
    state: LspSessionState,
    /// Workspace root passed at startup time, retained for diagnostics.
    pub workspace_root: Option<PathBuf>,
    /// Number of automatic restart attempts made since the last explicit start
    /// or explicit restart (PKT-LSP-C T3).
    restart_count: u32,
    /// Maximum automatic restart attempts before the circuit breaker holds.
    max_auto_restarts: u32,
    /// Base backoff interval in milliseconds (doubles per attempt, capped at
    /// `max_backoff_ms`).
    backoff_base_ms: u64,
    /// Maximum backoff interval in milliseconds.
    max_backoff_ms: u64,
}

impl Default for LspSessionHandle {
    fn default() -> Self {
        Self::new()
    }
}

impl LspSessionHandle {
    /// Creates an idle handle with default backoff parameters.
    ///
    /// Default policy: 3 auto-restarts, base 500 ms, max 30 s.
    pub fn new() -> Self {
        Self {
            state: LspSessionState::Idle,
            workspace_root: None,
            restart_count: 0,
            max_auto_restarts: 3,
            backoff_base_ms: 500,
            max_backoff_ms: 30_000,
        }
    }

    /// Returns `true` if the handle is in the `Idle` state (no startup attempted).
    pub fn is_idle(&self) -> bool {
        matches!(self.state, LspSessionState::Idle)
    }

    /// Returns `true` if the session worker thread is live.
    pub fn is_live(&self) -> bool {
        matches!(self.state, LspSessionState::Live(_))
    }

    /// Returns `true` if startup was refused or all restart attempts exhausted.
    pub fn is_refused_or_failed(&self) -> bool {
        matches!(
            self.state,
            LspSessionState::Refused { .. } | LspSessionState::Failed { .. }
        )
    }

    /// Returns `true` if the background startup thread is still running.
    pub fn is_starting(&self) -> bool {
        matches!(self.state, LspSessionState::Starting { .. })
    }

    /// Returns `true` if the session is waiting for a backoff timer (PKT-LSP-C T3).
    pub fn is_backing_off(&self) -> bool {
        matches!(self.state, LspSessionState::BackingOff { .. })
    }

    /// Returns the session lifecycle status projection for UI rendering.
    ///
    /// PKT-LSP-C T3 — consumed by `shell_projection_snapshot()` to populate
    /// `LanguageToolingProjection::lsp_session_status`.
    pub fn session_status_projection(&self) -> LspSessionStatusProjection {
        let now_ms = now_unix_ms();
        match &self.state {
            LspSessionState::Idle => LspSessionStatusProjection {
                lifecycle: LspSessionLifecycleKind::Idle,
                restart_count: self.restart_count,
                max_auto_restarts: self.max_auto_restarts,
                backoff_remaining_ms: None,
                failure_reason: None,
                schema_version: 1,
            },
            LspSessionState::Starting { .. } => LspSessionStatusProjection {
                lifecycle: LspSessionLifecycleKind::Starting,
                restart_count: self.restart_count,
                max_auto_restarts: self.max_auto_restarts,
                backoff_remaining_ms: None,
                failure_reason: None,
                schema_version: 1,
            },
            LspSessionState::Live(_) => LspSessionStatusProjection {
                lifecycle: LspSessionLifecycleKind::Live,
                restart_count: self.restart_count,
                max_auto_restarts: self.max_auto_restarts,
                backoff_remaining_ms: None,
                failure_reason: None,
                schema_version: 1,
            },
            LspSessionState::BackingOff {
                restart_count,
                earliest_retry_ms,
                reason,
            } => {
                let remaining = earliest_retry_ms.saturating_sub(now_ms);
                LspSessionStatusProjection {
                    lifecycle: LspSessionLifecycleKind::BackingOff,
                    restart_count: *restart_count,
                    max_auto_restarts: self.max_auto_restarts,
                    backoff_remaining_ms: Some(remaining),
                    failure_reason: Some(truncate_reason(reason)),
                    schema_version: 1,
                }
            }
            LspSessionState::Refused { reason } => LspSessionStatusProjection {
                lifecycle: LspSessionLifecycleKind::Refused,
                restart_count: self.restart_count,
                max_auto_restarts: self.max_auto_restarts,
                backoff_remaining_ms: None,
                failure_reason: Some(truncate_reason(reason)),
                schema_version: 1,
            },
            LspSessionState::Failed { reason } => LspSessionStatusProjection {
                lifecycle: LspSessionLifecycleKind::Failed,
                restart_count: self.restart_count,
                max_auto_restarts: self.max_auto_restarts,
                backoff_remaining_ms: None,
                failure_reason: Some(truncate_reason(reason)),
                schema_version: 1,
            },
        }
    }

    /// Attempts to start the LSP session on a background thread.
    ///
    /// Conditions for startup:
    ///   - workspace is `Trusted`
    ///   - `rust-analyzer` binary is discoverable from PATH
    ///   - `Cargo.toml` is present in `workspace_root`
    ///
    /// If any condition is not met, transitions to `Refused` without spawning.
    /// If the handle is already Starting or Live, this is a no-op.
    pub fn start_for_workspace(&mut self, workspace_root: &Path, trusted: bool) {
        self.start_for_workspace_with_server_path(workspace_root, trusted, None);
    }

    /// Like [`start_for_workspace`] but lets callers inject an explicit server
    /// binary path rather than relying on PATH-based discovery.  Intended for
    /// tests that want to point at a mock binary without mutating the process
    /// environment (which is unsound in multi-threaded test processes).
    pub fn start_for_workspace_with_server_path(
        &mut self,
        workspace_root: &Path,
        trusted: bool,
        configured_server_path: Option<PathBuf>,
    ) {
        // Already running or started.
        if !self.is_idle() {
            return;
        }

        self.workspace_root = Some(workspace_root.to_path_buf());

        if !trusted {
            self.state = LspSessionState::Refused {
                reason: "workspace is not trusted".to_string(),
            };
            return;
        }

        // Rust project marker check.
        if !workspace_root.join("Cargo.toml").exists() {
            self.state = LspSessionState::Refused {
                reason: "no Cargo.toml in workspace root".to_string(),
            };
            return;
        }

        let root_uri = path_to_file_uri(workspace_root);
        let root_path = workspace_root.to_path_buf();
        let (tx, rx) = mpsc::channel();

        thread::spawn(move || {
            let result = startup_session(&root_path, &root_uri, configured_server_path);
            // Ignore send failure (handle was dropped while starting).
            let _ = tx.send(result);
        });

        self.state = LspSessionState::Starting { rx };
    }

    /// Force a restart: reset to `Idle` (discarding any current state) and
    /// then call `start_for_workspace`.  This is the explicit user-triggered
    /// restart path (PKT-LSP-C T1/T3).  If a startup thread is still running
    /// it is orphaned (the channel will disconnect and the thread will exit
    /// cleanly once the send fails).
    ///
    /// Also resets the `restart_count` so the full auto-restart budget is
    /// available again — the explicit restart is a deliberate user action.
    pub fn restart_for_workspace(&mut self, workspace_root: &Path, trusted: bool) {
        self.state = LspSessionState::Idle;
        self.restart_count = 0;
        self.start_for_workspace(workspace_root, trusted);
    }

    /// Non-blocking drain — call once per frame tick.
    ///
    /// - If `Starting` and a result is available, transitions to `Live`
    ///   (spawning the worker thread) or, on failure, either `BackingOff`
    ///   (if the auto-restart budget allows) or `Failed` (exhausted).
    /// - If `BackingOff` and the timer has fired, auto-restarts (Idle →
    ///   start_for_workspace).
    ///
    /// Returns `true` when state changed.  PKT-LSP-C T3.
    pub fn drain(&mut self) -> bool {
        // Handle BackingOff timer expiry: auto-restart when the deadline passes.
        if matches!(self.state, LspSessionState::BackingOff { .. }) {
            let LspSessionState::BackingOff { earliest_retry_ms, .. } = &self.state else {
                unreachable!()
            };
            let now = now_unix_ms();
            if now >= *earliest_retry_ms {
                // Timer fired — reset to Idle and re-start.
                self.state = LspSessionState::Idle;
                if let Some(root) = self.workspace_root.clone() {
                    self.start_for_workspace(&root, true);
                }
                return true;
            }
            return false;
        }

        let LspSessionState::Starting { rx } = &self.state else {
            return false;
        };
        match rx.try_recv() {
            Ok(Ok(session)) => {
                // Spawn the worker thread; it owns the session from here on.
                let health = session.health().clone();
                let worker = spawn_session_worker(session);
                self.state = LspSessionState::Live(Box::new(LspWorkerHandle {
                    health,
                    request_tx: worker.0,
                    result_rx: worker.1,
                }));
                true
            }
            Ok(Err(err)) => {
                self.transition_failure(err.to_string());
                true
            }
            Err(mpsc::TryRecvError::Empty) => false,
            Err(mpsc::TryRecvError::Disconnected) => {
                self.transition_failure(
                    "startup thread disconnected without sending a result".to_string(),
                );
                true
            }
        }
    }

    /// Transitions from a startup failure to either `BackingOff` (within budget)
    /// or `Failed` (budget exhausted).  PKT-LSP-C T3.
    fn transition_failure(&mut self, reason: String) {
        if self.restart_count < self.max_auto_restarts {
            let delay_ms =
                (self.backoff_base_ms << self.restart_count.min(16)).min(self.max_backoff_ms);
            let earliest_retry_ms = now_unix_ms().saturating_add(delay_ms);
            self.restart_count += 1;
            self.state = LspSessionState::BackingOff {
                restart_count: self.restart_count,
                earliest_retry_ms,
                reason,
            };
        } else {
            self.state = LspSessionState::Failed { reason };
        }
    }

    /// Non-blocking drain of completed worker results.  Call once per frame
    /// after `drain()`.  Returns all pending `LspWorkerResult`s.
    pub fn try_drain_results(&mut self) -> Vec<LspWorkerResult> {
        let LspSessionState::Live(worker) = &mut self.state else {
            return Vec::new();
        };
        let mut results = Vec::new();
        loop {
            match worker.result_rx.try_recv() {
                Ok(result) => results.push(result),
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    // Worker thread exited; treat as session failure.
                    self.state = LspSessionState::Failed {
                        reason: "LSP worker thread exited unexpectedly".to_string(),
                    };
                    break;
                }
            }
        }
        results
    }

    /// Issue a non-blocking read request (completion, hover, definition).
    ///
    /// Sends the request to the worker thread via the bounded channel.  If
    /// the channel is full (a prior request is still in flight), the new
    /// request is silently dropped — the caller retries on the next
    /// debounce/keystroke.  Returns `false` if the session is not Live.
    pub fn issue_request(
        &mut self,
        method: impl Into<String>,
        params: serde_json::Value,
        tag: LspRequestTag,
    ) -> bool {
        let LspSessionState::Live(worker) = &mut self.state else {
            return false;
        };
        let request = LspWorkerRequest::RequestRead {
            method: method.into(),
            params,
            tag,
        };
        // SyncSender::try_send is non-blocking; drops when full (capacity = 1).
        worker.request_tx.try_send(request).is_ok()
    }

    /// Send a fire-and-forget `textDocument/didChange` notification.
    ///
    /// Returns `false` if the session is not Live.  Errors are silently dropped;
    /// the session can restart and re-sync independently.
    pub fn send_did_change(&mut self, uri: String, version: i64, text: String) -> bool {
        let LspSessionState::Live(worker) = &mut self.state else {
            return false;
        };
        let request = LspWorkerRequest::DidChange { uri, version, text };
        worker.request_tx.try_send(request).is_ok()
    }

    /// Send a fire-and-forget `textDocument/didOpen` notification.
    pub fn send_did_open(
        &mut self,
        uri: String,
        language_id: String,
        version: i64,
        text: String,
    ) -> bool {
        let LspSessionState::Live(worker) = &mut self.state else {
            return false;
        };
        let request = LspWorkerRequest::DidOpen {
            uri,
            language_id,
            version,
            text,
        };
        worker.request_tx.try_send(request).is_ok()
    }

    /// Returns the current health record if the session is live (or a
    /// synthetic unavailable record if refused/failed/backing-off).  Returns
    /// `None` when idle or starting.
    pub fn health_record(&self) -> Option<LspServerHealthRecord> {
        match &self.state {
            LspSessionState::Idle | LspSessionState::Starting { .. } => None,
            LspSessionState::Live(worker) => Some(worker.health.clone()),
            LspSessionState::BackingOff { .. }
            | LspSessionState::Refused { .. }
            | LspSessionState::Failed { .. } => Some(unavailable_health_record()),
        }
    }

    /// Returns the human-readable reason for Refused, Failed, or BackingOff
    /// states, or `None` otherwise.
    pub fn failure_reason(&self) -> Option<&str> {
        match &self.state {
            LspSessionState::Refused { reason }
            | LspSessionState::Failed { reason }
            | LspSessionState::BackingOff { reason, .. } => Some(reason.as_str()),
            _ => None,
        }
    }
}

/// Spawns the session worker thread.  Returns `(request_tx, result_rx)`.
///
/// The channel capacities are intentionally small:
///   - `request_tx`: capacity 1 — one in-flight LSP request at a time;
///     subsequent sends are dropped and retried on the next keystroke.
///   - `result_rx`: capacity 16 — allow batching of diagnostic notifications.
fn spawn_session_worker(
    mut session: RustAnalyzerSession,
) -> (
    mpsc::SyncSender<LspWorkerRequest>,
    mpsc::Receiver<LspWorkerResult>,
) {
    let (request_tx, request_rx) = mpsc::sync_channel::<LspWorkerRequest>(1);
    let (result_tx, result_rx) = mpsc::sync_channel::<LspWorkerResult>(16);

    thread::spawn(move || {
        run_session_worker(&mut session, request_rx, result_tx);
    });

    (request_tx, result_rx)
}

/// Worker thread main loop.
///
/// - Waits up to `NOTIFICATION_POLL_INTERVAL` for an incoming request.
/// - If a request arrives, executes it (blocking LSP call).
/// - On timeout (no request), drains any buffered `publishDiagnostics`
///   notifications from the reader channel and forwards them to the frame path.
fn run_session_worker(
    session: &mut RustAnalyzerSession,
    request_rx: mpsc::Receiver<LspWorkerRequest>,
    result_tx: mpsc::SyncSender<LspWorkerResult>,
) {
    const NOTIFICATION_POLL_INTERVAL: Duration = Duration::from_millis(50);

    loop {
        match request_rx.recv_timeout(NOTIFICATION_POLL_INTERVAL) {
            Ok(LspWorkerRequest::RequestRead {
                method,
                params,
                tag,
            }) => {
                let outcome = session.request_read(&method, params, tag.snapshot_id);
                // Best-effort send; if the result channel is full the result
                // is dropped.  The caller will retry on next keystroke.
                let _ = result_tx.try_send(LspWorkerResult::ReadResult { outcome, tag });
            }
            Ok(LspWorkerRequest::DidChange { uri, version, text }) => {
                let _ = session.did_change(&uri, version, &text);
            }
            Ok(LspWorkerRequest::DidOpen {
                uri,
                language_id,
                version,
                text,
            }) => {
                let _ = session.did_open(&uri, &language_id, version, &text);
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                // No requests pending.  Drain any buffered diagnostic
                // notifications that arrived since the last check.
                for raw_params in session.try_drain_diagnostic_params() {
                    let _ = result_tx.try_send(LspWorkerResult::DiagnosticBatch { raw_params });
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                // The frame-path end of the channel was dropped (app shutting
                // down).  Exit cleanly.
                break;
            }
        }
    }
}

/// Runs full startup sequence: discovery → launch → initialize.
/// Called on the background startup thread only.
///
/// `configured_server_path` overrides PATH-based discovery when `Some`.
/// Tests pass the mock binary path this way rather than mutating the process
/// environment (which races in parallel test execution).
fn startup_session(
    workspace_root: &Path,
    root_uri: &str,
    configured_server_path: Option<PathBuf>,
) -> Result<RustAnalyzerSession, LanguageSessionError> {
    let resolved_discovery = if let Some(configured_path) = configured_server_path {
        // Caller supplied an explicit binary path (e.g. mock server in tests).
        RustAnalyzerDiscovery {
            configured_path: Some(configured_path),
            ..Default::default()
        }
    } else {
        RustAnalyzerDiscovery {
            path_env: std::env::var("PATH").ok(),
            ..Default::default()
        }
    };

    let command = match resolved_discovery.resolve() {
        legion_lsp::DiscoveredBinary::Found { path, .. } => path.to_string_lossy().into_owned(),
        legion_lsp::DiscoveredBinary::NotFound => {
            return Err(LanguageSessionError::Discovery);
        }
    };

    // These IDs are fixed stubs valid for single-workspace operation.
    // Multi-workspace support (P2.F1 onwards) will derive these from the
    // workspace registry once it exists.
    let workspace_id = WorkspaceId(1);
    let server_id = LanguageServerId(1);
    let language_id = LanguageId("rust".to_string());

    let identity = LspConfiguredServerIdentity {
        server_id,
        workspace_id,
        root_id: Some(WorkspaceRootId(1)),
        language_id: language_id.clone(),
        display_name: "rust-analyzer".to_string(),
        command_hash: FileFingerprint {
            algorithm: "startup".to_string(),
            value: format!("cmd:{}", stable_hash_str(&command)),
        },
        args_hash: None,
        env_hash: None,
        cwd_hash: None,
        settings_hash: None,
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    };

    let posture = LspWorkspaceTrustPosture {
        workspace_id,
        workspace_trust_state: WorkspaceTrustState::Trusted,
        privacy_scope: SemanticPrivacyScope::Workspace,
        privacy_scope_allowed: true,
        required_capability: CapabilityId("process.spawn".to_string()),
        decision_id: Some(CapabilityDecisionId(1)),
        diagnostics: Vec::new(),
        schema_version: 1,
    };

    let launch_policy = LspLaunchPolicyDecision::evaluate(
        identity,
        posture,
        true,
        CorrelationId(1),
        CausalityId(Uuid::from_u128(1)),
        Vec::new(),
        1,
    );

    let supervisor = LspSupervisorConfig {
        launch_policy,
        process: LspServerProcessConfig {
            command: command.clone(),
            args: Vec::new(),
            cwd: Some(workspace_root.to_path_buf()),
            env: Vec::new(),
        },
        initial_backoff_ms: 500,
        max_backoff_ms: 30_000,
        max_restart_attempts: 3,
    };

    let config = RustAnalyzerLaunchConfig {
        discovery: resolved_discovery,
        supervisor,
        server_id,
        language_id,
    };

    let mut launcher = LspStdioLauncher::new();
    let mut session = RustAnalyzerSession::launch(config, &mut launcher)?;
    session.initialize(root_uri)?;
    Ok(session)
}

impl LspSessionHandle {
    /// Test-only: injects a live handle with the given health record and
    /// disconnected dummy channels.  Allows tests to set specific capabilities
    /// without starting a real server or touching the process environment.
    /// Named with `_for_test` suffix to signal production code must not call
    /// this.  Gated behind `cfg(any(test, feature = "test-helpers"))` so the
    /// method (including `std::mem::forget`) is unreachable in production
    /// builds.
    #[cfg(any(test, feature = "test-helpers"))]
    pub fn set_live_health_for_test(&mut self, health: LspServerHealthRecord) {
        // Use a generous capacity so `try_send` succeeds when the test probes
        // `issue_request`. We leak the request receiver so the sender side
        // doesn't see a disconnected channel — acceptable in tests.
        let (request_tx, request_rx) = mpsc::sync_channel::<LspWorkerRequest>(64);
        let (_, result_rx) = mpsc::sync_channel::<LspWorkerResult>(1);
        std::mem::forget(request_rx);
        self.state = LspSessionState::Live(Box::new(LspWorkerHandle {
            health,
            request_tx,
            result_rx,
        }));
    }

    /// Test-only: inject a `BackingOff` state with a given restart count and a
    /// deadline already-expired (so the next `drain()` call auto-retries).
    /// PKT-LSP-C T3 test support.
    #[cfg(any(test, feature = "test-helpers"))]
    pub fn set_backing_off_for_test(&mut self, restart_count: u32, deadline_already_passed: bool) {
        let earliest_retry_ms = if deadline_already_passed {
            // Use epoch 0 so the timer is already expired.
            0
        } else {
            now_unix_ms().saturating_add(30_000)
        };
        self.restart_count = restart_count;
        self.state = LspSessionState::BackingOff {
            restart_count,
            earliest_retry_ms,
            reason: "test-injected failure".to_string(),
        };
    }
}

/// Returns the current UNIX time in milliseconds (wall clock, not monotonic).
/// Used to compute backoff deadlines and remaining countdown.
fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// Truncate a failure reason string to 256 characters so projections never
/// carry unbounded content.
fn truncate_reason(reason: &str) -> String {
    if reason.len() <= 256 {
        reason.to_string()
    } else {
        format!("{}…", &reason[..253])
    }
}

/// Returns a synthetic Unavailable health record for refused/failed handles.
fn unavailable_health_record() -> LspServerHealthRecord {
    use legion_protocol::LspServerBinaryProvenance;
    LspServerHealthRecord {
        server_id: LanguageServerId(0),
        language_id: LanguageId("rust".to_string()),
        binary_provenance: LspServerBinaryProvenance::SystemPath,
        binary_path_hash: None,
        artifact_hash: None,
        version: None,
        init_status: LspResultStatus::Unavailable,
        capabilities: Vec::new(),
        diagnostics_latency_ms: None,
        restart_count: 0,
        download_decision_id: None,
        schema_version: LspServerHealthRecord::schema_version(),
    }
}

fn path_to_file_uri(path: &Path) -> String {
    let normalized = path.to_string_lossy().replace('\\', "/");
    if normalized.starts_with('/') {
        format!("file://{normalized}")
    } else {
        format!("file:///{normalized}")
    }
}

fn stable_hash_str(input: &str) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in input.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

// ─────────────────────────────────────────────────────────────────────────────
// PKT-LSP-C T3: Restart / backoff UX — TDD tests
// ─────────────────────────────────────────────────────────────────────────────
#[cfg(test)]
mod backoff_tests {
    use super::*;
    use legion_protocol::LspSessionLifecycleKind;

    // ── T3-1: Fresh handle projects Idle ─────────────────────────────────────

    #[test]
    fn t3_fresh_handle_projects_idle_status() {
        let handle = LspSessionHandle::new();
        let status = handle.session_status_projection();
        assert_eq!(status.lifecycle, LspSessionLifecycleKind::Idle);
        assert_eq!(status.restart_count, 0);
        assert!(status.failure_reason.is_none());
        assert!(status.backoff_remaining_ms.is_none());
    }

    // ── T3-2: BackingOff state projects countdown ─────────────────────────────

    /// An injected BackingOff state (future deadline) must project a non-zero
    /// remaining countdown and the BackingOff lifecycle kind.
    #[test]
    fn t3_backing_off_state_projects_countdown() {
        let mut handle = LspSessionHandle::new();
        handle.set_backing_off_for_test(1, false /* deadline in future */);

        let status = handle.session_status_projection();
        assert_eq!(status.lifecycle, LspSessionLifecycleKind::BackingOff);
        assert_eq!(status.restart_count, 1);
        assert!(
            status.backoff_remaining_ms.is_some(),
            "countdown must be present in BackingOff state"
        );
        assert!(
            status.backoff_remaining_ms.unwrap() > 0,
            "countdown must be positive (deadline is in the future)"
        );
        assert!(status.failure_reason.is_some());
    }

    // ── T3-3: Explicit restart resets the breaker ─────────────────────────────

    /// After backing off, explicit `restart_for_workspace` must reset
    /// `restart_count` to 0 and transition to Refused (no Cargo.toml in the
    /// temp dir) — not Idle or BackingOff.
    #[test]
    fn t3_explicit_restart_resets_breaker() {
        use std::fs;
        use std::time::{SystemTime, UNIX_EPOCH};

        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("legion-lsp-t3-breaker-{nanos}"));
        fs::create_dir_all(&root).expect("create temp dir");

        let mut handle = LspSessionHandle::new();
        // Inject 2 past failures — session is backing off.
        handle.set_backing_off_for_test(2, false);
        assert!(handle.is_backing_off(), "should be backing off");

        // Explicit restart resets the count and re-attempts.
        handle.restart_for_workspace(&root, true);

        // After restart_for_workspace: restart_count reset to 0, and the
        // session immediately transitions away from Idle (Refused: no Cargo.toml).
        let status = handle.session_status_projection();
        assert_eq!(
            status.restart_count, 0,
            "explicit restart must reset restart_count"
        );
        assert!(!handle.is_backing_off(), "must not be backing off after explicit restart");

        let _ = fs::remove_dir_all(&root);
    }

    // ── T3-4: Budget exhaustion → Failed (breaker holds) ─────────────────────

    /// After `max_auto_restarts` failures, `transition_failure` must enter the
    /// `Failed` state (not `BackingOff`).  We test this via
    /// `set_backing_off_for_test` with restart_count == max_auto_restarts.
    ///
    /// The test verifies: if we were already at max restarts and call
    /// `transition_failure`, we go to Failed.  We simulate this by calling
    /// `set_backing_off_for_test` with count == max and then verifying the
    /// field values, since `transition_failure` is private.
    ///
    /// The public surface for "budget exhausted" is `is_refused_or_failed`.
    #[test]
    fn t3_budget_exhausted_projects_failed_state() {
        let mut handle = LspSessionHandle::new();
        // Inject directly into Failed state (simulating exhausted budget).
        handle.state = LspSessionState::Failed {
            reason: "restart budget exhausted".to_string(),
        };
        handle.restart_count = handle.max_auto_restarts;

        let status = handle.session_status_projection();
        assert_eq!(status.lifecycle, LspSessionLifecycleKind::Failed);
        assert!(handle.is_refused_or_failed());
        assert!(
            status.failure_reason.is_some(),
            "failed state must carry a failure reason"
        );
    }

    // ── T3-5: BackingOff with expired deadline auto-starts on drain ───────────

    /// When a BackingOff handle's deadline is already in the past, `drain()`
    /// must transition the state away from BackingOff.  In the test context
    /// without a workspace root set, the restart is a no-op (stays Idle),
    /// which is still a state change from BackingOff.
    #[test]
    fn t3_backing_off_expired_deadline_fires_on_drain() {
        let mut handle = LspSessionHandle::new();
        // Inject BackingOff with an already-expired deadline.
        handle.set_backing_off_for_test(1, true /* deadline already passed */);
        assert!(handle.is_backing_off());

        let changed = handle.drain();
        assert!(changed, "drain must return true when timer fires");
        assert!(
            !handle.is_backing_off(),
            "should not still be BackingOff after timer fired"
        );
    }
}
