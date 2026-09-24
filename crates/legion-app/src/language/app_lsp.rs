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
    collections::{HashMap, VecDeque},
    io::Read,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

/// Maximum number of redacted stderr lines retained in the ring buffer.
const STDERR_RING_CAPACITY: usize = 100;
/// Maximum byte length of a single retained stderr line (longer lines are
/// truncated with `…` to prevent unbounded growth of the ring elements).
const STDERR_LINE_MAX_LEN: usize = 512;

use legion_protocol::{
    BufferId, FileId, LanguageId, LanguageServerId, LspResultStatus, LspServerHealthRecord,
    LspSessionLifecycleKind, LspSessionLogProjection, LspSessionStatusProjection, SnapshotId,
    WorkspaceId,
};

use super::{
    LanguageServerLaunchConfig, LanguageServerSession, LanguageSessionError, LspReadOutcome,
};
#[cfg(any(test, feature = "test-helpers"))]
use super::{RustAnalyzerDiscovery, RustAnalyzerLaunchConfig, RustAnalyzerSession};
use legion_lsp::LspStdioLauncher;
#[cfg(any(test, feature = "test-helpers"))]
use legion_lsp::{LspServerProcessConfig, LspSupervisorConfig};
use legion_protocol::{CapabilityDecisionId, FileFingerprint, LspServerBinaryProvenance};
#[cfg(any(test, feature = "test-helpers"))]
use legion_protocol::{
    CapabilityId, CausalityId, CorrelationId, LspConfiguredServerIdentity, LspLaunchPolicyDecision,
    LspWorkspaceTrustPosture, RedactionHint, SemanticPrivacyScope, WorkspaceRootId,
    WorkspaceTrustState,
};
#[cfg(any(test, feature = "test-helpers"))]
use uuid::Uuid;

/// Result type delivered from the background startup thread.
pub type LspStartResult = Result<LanguageServerSession, LanguageSessionError>;

type LanguageServerPreparation = dyn Fn(Arc<AtomicBool>) -> Result<LanguageServerStartConfig, LanguageSessionError>
    + Send
    + Sync
    + 'static;
type TransportDeathSignal = Arc<Mutex<Option<String>>>;

/// Fully selected and approved inputs for one language-server startup.
///
/// The app owns selection, trust, capability approval, and artifact
/// validation.  This worker-facing descriptor only carries the resulting
/// launch metadata and adapter initialization inputs; it performs no PATH
/// discovery, broker calls, materialization, or runtime probing.
pub struct LanguageServerStartConfig {
    /// Workspace root associated with the identity and initialize request.
    pub workspace_root: PathBuf,
    /// File URI sent in the LSP `initialize` request.
    pub root_uri: String,
    /// Complete, already-approved generic session launch configuration.
    pub launch_config: LanguageServerLaunchConfig,
    /// Adapter-specific LSP `initializationOptions`.
    pub initialization_options: Option<serde_json::Value>,
    /// Optional adapter/client capability additions.
    pub client_capabilities: Option<serde_json::Value>,
}

/// Selected server metadata retained for unavailable health projections while
/// a prepared launch is refused, backing off, or fails.  The app supplies this
/// before preparation; the handle never invents a Rust/default identity for a
/// generic language path.
#[derive(Debug, Clone)]
pub struct LspSelectedServerMetadata {
    /// Selected language-server identity.
    pub server_id: LanguageServerId,
    /// Selected language identity.
    pub language_id: LanguageId,
    /// Approved binary provenance.
    pub binary_provenance: LspServerBinaryProvenance,
    /// Verified artifact fingerprint, when downloaded.
    pub artifact_hash: Option<FileFingerprint>,
    /// Observed server/runtime version.
    pub version: Option<String>,
    /// Decision authorizing a downloaded artifact, when applicable.
    pub download_decision_id: Option<CapabilityDecisionId>,
}

impl From<&LanguageServerLaunchConfig> for LspSelectedServerMetadata {
    fn from(config: &LanguageServerLaunchConfig) -> Self {
        Self {
            server_id: config.server_id,
            language_id: config.language_id.clone(),
            binary_provenance: config.binary_provenance,
            artifact_hash: config.artifact_hash.clone(),
            version: config.version.clone(),
            download_decision_id: config.download_decision_id,
        }
    }
}

impl Clone for LanguageServerStartConfig {
    fn clone(&self) -> Self {
        Self {
            workspace_root: self.workspace_root.clone(),
            root_uri: self.root_uri.clone(),
            launch_config: clone_launch_config(&self.launch_config),
            initialization_options: self.initialization_options.clone(),
            client_capabilities: self.client_capabilities.clone(),
        }
    }
}

fn clone_launch_config(config: &LanguageServerLaunchConfig) -> LanguageServerLaunchConfig {
    LanguageServerLaunchConfig {
        supervisor: config.supervisor.clone(),
        server_id: config.server_id,
        language_id: config.language_id.clone(),
        binary_provenance: config.binary_provenance,
        artifact_hash: config.artifact_hash.clone(),
        artifact_dependencies: config.artifact_dependencies.clone(),
        version: config.version.clone(),
        node_runtime_version: config.node_runtime_version,
        download_decision_id: config.download_decision_id,
    }
}

/// Tag carried with a worker request so the drain side can route the result.
#[derive(Debug, Clone)]
pub struct LspRequestTag {
    /// Buffer the request was issued against.
    pub buffer_id: BufferId,
    /// What kind of read is this.
    pub kind: LspReadKind,
    /// Snapshot when the request was issued (stale-gate).
    pub snapshot_id: SnapshotId,
    /// Opaque write-side operation correlation, when this request must produce a proposal.
    pub operation_id: Option<String>,
    /// Authoritative document operation context captured at app admission.
    /// `None` is retained only for unit-fixture compatibility; product reads
    /// and writes populate it before crossing to the worker.
    pub operation_context: Option<legion_protocol::LspOperationContext>,
}

/// Bounded metadata retained while an accepted write-side LSP request is in flight.
#[derive(Debug, Clone)]
pub(crate) struct PendingLspWriteOperation {
    /// Opaque operation identifier reused in the projected terminal result.
    pub(crate) operation_id: String,
    /// Existing projection operation category.
    pub(crate) operation_kind: crate::LanguageToolingOperationKind,
    /// Workspace and file identity retained for terminal status projection.
    pub(crate) workspace_id: WorkspaceId,
    pub(crate) file_id: FileId,
    /// Buffer and snapshot the request was admitted against.
    pub(crate) buffer_id: BufferId,
    pub(crate) snapshot_id: SnapshotId,
    /// Event context reused when the resulting proposal is recorded.
    pub(crate) event_context: crate::EventContext,
}

/// Discriminator for routing worker read results.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LspReadKind {
    /// Completion request (`textDocument/completion`).
    Completion,
    /// Hover request (`textDocument/hover`).
    Hover,
    /// Go-to-definition request (`textDocument/definition`).
    Definition,
    /// References request (`textDocument/references`).
    References,
    /// Document-symbol request (`textDocument/documentSymbol`).
    ///
    /// Named `Outline` rather than `DocumentSymbol` to match
    /// `LanguageReadKind::Outline`, which is what the projection calls it.
    Outline,
    /// Inlay-hint request (`textDocument/inlayHint`).
    InlayHints,
    /// Call-hierarchy prepare (`textDocument/prepareCallHierarchy`).
    ///
    /// Carries the direction the caller ultimately wants, because the response
    /// to this request is what makes the follow-up possible and the follow-up
    /// is what the user actually asked for.
    CallHierarchyPrepare,
    /// Callers (`callHierarchy/incomingCalls`).
    IncomingCalls,
    /// Callees (`callHierarchy/outgoingCalls`).
    OutgoingCalls,
    /// Code-lens request (`textDocument/codeLens`).
    CodeLens,
    /// Formatting request (`textDocument/formatting`).
    Formatting,
    /// Code-action request (`textDocument/codeAction`).
    CodeAction {
        /// Whether the request was narrowed to `source.organizeImports`.
        ///
        /// Organize-imports is a code action with a kind rather than a request
        /// of its own, so the two share a wire call and are told apart here.
        organize_imports: bool,
    },
    /// Resolve one response-scoped code-action candidate.
    CodeActionResolve {
        /// Response identity.
        response_id: String,
        /// Candidate identity.
        action_id: String,
    },
    /// Execute an explicitly selected server command from a code action.
    CodeActionExecuteCommand {
        /// Response identity.
        response_id: String,
        /// Candidate identity.
        action_id: String,
    },
    /// Rename request (`textDocument/rename`).
    ///
    /// Carries the replacement identifier so the drain-side handler can
    /// create the proposal title without re-reading the request params.
    Rename {
        /// The new identifier the user requested.
        new_name: String,
    },
}

/// Message sent from the frame path to the worker thread.
pub enum LspWorkerRequest {
    /// Issue a blocking LSP read request on the worker thread.
    RequestRead {
        /// LSP method name.
        method: String,
        /// JSON-RPC parameters.
        params: serde_json::Value,
        /// App-side result routing metadata.
        tag: LspRequestTag,
    },
    /// Fire-and-forget: send a `textDocument/didChange` notification.
    DidChange {
        /// Canonical `file://` document identity.
        uri: String,
        /// Monotonic document version after the edit.
        version: i64,
        /// Full post-edit document text.
        text: String,
        /// App-admitted context for diagnostics pull after this sync.
        operation_context: Option<legion_protocol::LspOperationContext>,
    },
    /// Deferred didChange whose text producer runs only after queue admission.
    DidChangeDeferred {
        /// Canonical document identity.
        uri: String,
        /// Version captured before the producer runs.
        version: i64,
        /// One-shot text payload supplied after admission.
        text_rx: mpsc::Receiver<Option<String>>,
        /// App-admitted context retained through deferred text production.
        operation_context: Option<legion_protocol::LspOperationContext>,
    },
    /// Fire-and-forget: send a `textDocument/didOpen` notification.
    DidOpen {
        /// Canonical `file://` document identity.
        uri: String,
        /// Language identifier.
        language_id: String,
        /// Initial document version.
        version: i64,
        /// Full document text at open time.
        text: String,
        /// Authoritative editor buffer identity for lifecycle fencing.
        buffer_id: BufferId,
        /// App-admitted context for diagnostics pull after this sync.
        operation_context: Option<legion_protocol::LspOperationContext>,
    },
    /// Deferred didOpen whose text producer runs only after queue admission.
    DidOpenDeferred {
        /// Canonical document identity.
        uri: String,
        /// LSP language identifier.
        language_id: String,
        /// Version captured before the producer runs.
        version: i64,
        /// One-shot text payload supplied after admission.
        text_rx: mpsc::Receiver<Option<String>>,
        /// Authoritative editor buffer identity.
        buffer_id: BufferId,
        /// App-admitted context retained through deferred text production.
        operation_context: Option<legion_protocol::LspOperationContext>,
    },
    /// Internal marker for an admitted deferred request whose producer
    /// cancelled before supplying text.
    DeferredCancelled,
    /// Fire-and-forget: send a `textDocument/didClose` notification.
    DidClose {
        /// Canonical `file://` document identity.
        uri: String,
    },
}

/// Message sent from the worker thread back to the frame path.
pub enum LspWorkerResult {
    /// Server-originated `workspace/applyEdit` request awaiting app authority.
    ApplyEditRequested {
        /// Bounded request captured by the transport.
        request: legion_lsp::LspApplyWorkspaceEditRequest,
        /// Direct bounded reply channel waking the worker callback.
        reply: mpsc::SyncSender<legion_lsp::LspApplyWorkspaceEditResponse>,
        /// Shared deadline arbitration state used by the app claim path.
        decision: crate::language::ApplyEditDecision,
    },
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
        /// Buffer identity captured when the worker observed the document.
        captured_buffer_id: Option<BufferId>,
    },
    /// The session transport died: the stdout reader thread recorded a
    /// terminal event (server closed stdout, or a framing/parse error killed
    /// the reader while the server may still be alive).  Sent exactly once;
    /// the worker thread exits after sending.  Intercepted by
    /// [`LspSessionHandle::try_drain_results`], which routes it through the
    /// restart circuit breaker — it never reaches the frame-path result
    /// dispatch (PKT-S3-WEDGE-R3).
    TransportDead {
        /// Redacted, bounded, metadata-only description of the terminal
        /// reader event.
        reason: String,
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
    /// Reliable bounded terminal signal independent of the result queue.  A
    /// full result queue must never lose a transport-death lifecycle event.
    transport_dead: TransportDeathSignal,
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
    /// App-owned preparation retained for bounded automatic restart.  Each
    /// invocation must revalidate trust, approval, and artifact/runtime
    /// identity before returning a fresh descriptor.
    preparation: Option<Arc<LanguageServerPreparation>>,
    /// Cancellation for the currently queued startup/preparation generation.
    startup_cancel: Option<Arc<AtomicBool>>,
    /// Selected metadata for unavailable health projections.
    selected_metadata: Option<LspSelectedServerMetadata>,
    execute_command_ids: Vec<String>,
    /// Shared bounded, redacted stderr retained across startup failures and
    /// automatic retries for diagnosis.
    stderr_ring: Arc<Mutex<VecDeque<String>>>,
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
            preparation: None,
            startup_cancel: None,
            selected_metadata: None,
            execute_command_ids: Vec::new(),
            stderr_ring: Arc::new(Mutex::new(VecDeque::new())),
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

    /// Selected adapter identity for the current lifecycle, when a start has
    /// selected one.
    pub fn selected_server_id(&self) -> Option<LanguageServerId> {
        self.selected_metadata
            .as_ref()
            .map(|metadata| metadata.server_id)
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
    #[cfg(any(test, feature = "test-helpers"))]
    pub fn start_for_workspace(&mut self, workspace_root: &Path, trusted: bool) {
        self.start_for_workspace_with_server_path(workspace_root, trusted, None);
    }

    /// Like [`start_for_workspace`] but lets callers inject an explicit server
    /// binary path rather than relying on PATH-based discovery.  Intended for
    /// tests that want to point at a mock binary without mutating the process
    /// environment (which is unsound in multi-threaded test processes).
    #[cfg(any(test, feature = "test-helpers"))]
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

        // This compatibility route performs the legacy Rust-analyzer
        // discovery.  Generic callers must use
        // `start_for_workspace_with_config`, which has no discovery fallback.
        self.preparation = None;
        self.startup_cancel = None;
        self.selected_metadata = Some(LspSelectedServerMetadata {
            server_id: LanguageServerId(1),
            language_id: LanguageId("rust".to_string()),
            binary_provenance: LspServerBinaryProvenance::SystemPath,
            artifact_hash: None,
            version: None,
            download_decision_id: None,
        });

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
        let stderr_ring = Arc::clone(&self.stderr_ring);
        let (tx, rx) = mpsc::channel();

        thread::spawn(move || {
            let result =
                startup_session(&root_path, &root_uri, configured_server_path, stderr_ring);
            // Ignore send failure (handle was dropped while starting).
            let _ = tx.send(result);
        });

        self.state = LspSessionState::Starting { rx };
    }

    /// Starts a selected language server from an app-provided, already
    /// validated descriptor.  The launch and initialize work is performed on
    /// the existing bounded background startup path; this method never blocks
    /// and is a no-op while another startup or live worker exists.
    pub fn start_for_workspace_with_config(&mut self, config: LanguageServerStartConfig) {
        if !self.is_idle() {
            return;
        }

        let config = Arc::new(config);
        self.workspace_root = Some(config.workspace_root.clone());
        self.selected_metadata = Some(LspSelectedServerMetadata::from(&config.launch_config));
        self.preparation = None;
        let cancel = self.new_start_generation();
        self.spawn_configured_start(config, cancel);
    }

    /// Starts after preparing a fresh, app-approved descriptor on the
    /// background startup thread.  Preparation must remain bounded and must
    /// not touch frame-path state.  This one-shot form intentionally disables
    /// automatic restart after a crash because approval cannot be reused.
    pub fn start_preparing<F>(
        &mut self,
        workspace_root: PathBuf,
        metadata: LspSelectedServerMetadata,
        preparation: F,
    ) where
        F: FnOnce(Arc<AtomicBool>) -> Result<LanguageServerStartConfig, LanguageSessionError>
            + Send
            + 'static,
    {
        if !self.is_idle() {
            return;
        }
        self.workspace_root = Some(workspace_root);
        self.selected_metadata = Some(metadata);
        self.preparation = None;
        let cancel = self.new_start_generation();
        self.spawn_preparation_start(
            self.workspace_root.clone().expect("set above"),
            cancel,
            preparation,
        );
    }

    /// Starts after app-owned preparation and retains the repeatable
    /// preparation factory for bounded automatic restart.  Every retry invokes
    /// the factory again; no launch config or approval receipt is cloned.
    pub fn start_preparing_with_factory<F>(
        &mut self,
        workspace_root: PathBuf,
        metadata: LspSelectedServerMetadata,
        preparation: F,
    ) where
        F: Fn(Arc<AtomicBool>) -> Result<LanguageServerStartConfig, LanguageSessionError>
            + Send
            + Sync
            + 'static,
    {
        if !self.is_idle() {
            return;
        }
        let preparation: Arc<LanguageServerPreparation> = Arc::new(preparation);
        self.workspace_root = Some(workspace_root);
        self.selected_metadata = Some(metadata);
        self.preparation = Some(Arc::clone(&preparation));
        let cancel = self.new_start_generation();
        self.spawn_preparation_start(
            self.workspace_root.clone().expect("set above"),
            cancel,
            move |cancel| preparation(cancel),
        );
    }

    /// Explicitly restarts with a fresh app-provided descriptor.
    ///
    /// Callers must revalidate trust, approval, artifact/runtime identity, and
    /// correlation/causality before constructing `config`; the handle never
    /// silently reuses the previous descriptor for this user-triggered path.
    pub fn restart_for_workspace_with_config(&mut self, config: LanguageServerStartConfig) {
        self.state = LspSessionState::Idle;
        self.execute_command_ids.clear();
        self.restart_count = 0;
        self.cancel_pending_start();
        self.preparation = None;
        self.start_for_workspace_with_config(config);
    }

    /// Explicitly restarts after a fresh one-shot app preparation.
    pub fn restart_for_workspace_preparing<F>(
        &mut self,
        workspace_root: PathBuf,
        metadata: LspSelectedServerMetadata,
        preparation: F,
    ) where
        F: FnOnce(Arc<AtomicBool>) -> Result<LanguageServerStartConfig, LanguageSessionError>
            + Send
            + 'static,
    {
        self.state = LspSessionState::Idle;
        self.execute_command_ids.clear();
        self.restart_count = 0;
        self.cancel_pending_start();
        self.preparation = None;
        self.start_preparing(workspace_root, metadata, preparation);
    }

    /// Cancel any pending startup and drop the live worker for an app
    /// workspace transition or explicit toolchain clear.
    pub fn reset_to_idle(&mut self) {
        self.cancel_pending_start();
        self.preparation = None;
        self.state = LspSessionState::Idle;
        self.execute_command_ids.clear();
        self.restart_count = 0;
        self.selected_metadata = None;
        self.workspace_root = None;
    }

    fn new_start_generation(&mut self) -> Arc<AtomicBool> {
        self.cancel_pending_start();
        let cancel = Arc::new(AtomicBool::new(false));
        self.startup_cancel = Some(Arc::clone(&cancel));
        cancel
    }

    fn cancel_pending_start(&mut self) {
        if let Some(cancel) = self.startup_cancel.take() {
            cancel.store(true, Ordering::Release);
        }
    }

    fn spawn_configured_start(
        &mut self,
        config: Arc<LanguageServerStartConfig>,
        cancel: Arc<AtomicBool>,
    ) {
        let (tx, rx) = mpsc::channel();
        let stderr_ring = Arc::clone(&self.stderr_ring);
        thread::spawn(move || {
            let result = if cancel.load(Ordering::Acquire) {
                Err(LanguageSessionError::Unavailable)
            } else {
                startup_configured_session(config, cancel, stderr_ring.clone())
            };
            let _ = tx.send(result);
        });
        self.state = LspSessionState::Starting { rx };
    }

    fn spawn_preparation_start<F>(
        &mut self,
        expected_root: PathBuf,
        cancel: Arc<AtomicBool>,
        preparation: F,
    ) where
        F: FnOnce(Arc<AtomicBool>) -> Result<LanguageServerStartConfig, LanguageSessionError>
            + Send
            + 'static,
    {
        let (tx, rx) = mpsc::channel();
        let stderr_ring = Arc::clone(&self.stderr_ring);
        thread::spawn(move || {
            let result = if cancel.load(Ordering::Acquire) {
                Err(LanguageSessionError::Unavailable)
            } else {
                preparation(Arc::clone(&cancel)).and_then(|config| {
                    if cancel.load(Ordering::Acquire) {
                        return Err(LanguageSessionError::Unavailable);
                    }
                    if !same_workspace_root(&config.workspace_root, &expected_root) {
                        return Err(LanguageSessionError::InvalidConfiguration(
                            "prepared workspace root does not match requested root".to_string(),
                        ));
                    }
                    startup_configured_session(Arc::new(config), cancel, stderr_ring)
                })
            };
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
    #[cfg(any(test, feature = "test-helpers"))]
    pub fn restart_for_workspace(&mut self, workspace_root: &Path, trusted: bool) {
        self.state = LspSessionState::Idle;
        self.execute_command_ids.clear();
        self.restart_count = 0;
        self.cancel_pending_start();
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
            let LspSessionState::BackingOff {
                earliest_retry_ms, ..
            } = &self.state
            else {
                unreachable!()
            };
            let now = now_unix_ms();
            if now >= *earliest_retry_ms {
                // Timer fired — reset to Idle and re-start.
                self.state = LspSessionState::Idle;
                if let Some(preparation) = self.preparation.clone() {
                    // Automatic retries are bounded by this handle's circuit
                    // breaker and must obtain a fresh app-approved config.
                    let Some(root) = self.workspace_root.clone() else {
                        self.state = LspSessionState::Failed {
                            reason: "automatic restart has no workspace root".to_string(),
                        };
                        return true;
                    };
                    let cancel = self.new_start_generation();
                    self.spawn_preparation_start(root, cancel.clone(), move |cancel| {
                        preparation(cancel)
                    });
                } else {
                    self.state = LspSessionState::Failed {
                        reason: "automatic restart requires fresh app preparation".to_string(),
                    };
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
                self.execute_command_ids = session.execute_command_ids().to_vec();
                let worker = spawn_session_worker(session);
                self.state = LspSessionState::Live(Box::new(LspWorkerHandle {
                    health,
                    request_tx: worker.0,
                    result_rx: worker.1,
                    transport_dead: worker.2,
                }));
                true
            }
            Ok(Err(err)) => {
                if matches!(err, LanguageSessionError::InvalidConfiguration(_)) {
                    self.state = LspSessionState::Refused {
                        reason: err.to_string(),
                    };
                } else if matches!(err, LanguageSessionError::Unavailable)
                    && self
                        .startup_cancel
                        .as_ref()
                        .is_some_and(|cancel| cancel.load(Ordering::Acquire))
                {
                    // A cancelled generation must not feed the restart
                    // circuit breaker or resurrect a stale worker.
                    self.state = LspSessionState::Idle;
                } else {
                    self.transition_failure(err.to_string());
                }
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
        let mut transport_death: Option<String> = None;
        let mut worker_disconnected = false;
        loop {
            match worker.result_rx.try_recv() {
                // Intercepted here: the state transition is the handle's job,
                // and the worker exits right after sending this, so the
                // Disconnected arm below must not overwrite the specific
                // reason with the generic one (PKT-S3-WEDGE-R3).
                Ok(LspWorkerResult::TransportDead { reason }) => {
                    transport_death = Some(reason);
                }
                Ok(result) => results.push(result),
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    worker_disconnected = true;
                    break;
                }
            }
        }
        if transport_death.is_none() {
            transport_death = worker
                .transport_dead
                .lock()
                .ok()
                .and_then(|mut signal| signal.take());
        }
        if let Some(reason) = transport_death {
            // Route through the restart circuit breaker (PKT-LSP-C T3):
            // BackingOff with auto-retry while budget remains, Failed after.
            self.transition_failure(truncate_reason(&format!("LSP transport died: {reason}")));
        } else if worker_disconnected {
            // Worker thread exited without reporting; treat it as a bounded
            // transport failure so the normal restart circuit breaker applies.
            self.transition_failure("LSP worker thread exited unexpectedly".to_string());
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

    pub(crate) fn supports_execute_command(&self, command_id: &str) -> bool {
        !command_id.is_empty()
            && self
                .execute_command_ids
                .iter()
                .any(|candidate| candidate == command_id)
    }

    #[cfg(test)]
    pub(crate) fn set_execute_command_ids_for_test(&mut self, command_ids: Vec<String>) {
        self.execute_command_ids = command_ids;
    }

    /// Send a fire-and-forget `textDocument/didChange` notification.
    ///
    /// Returns `false` if the session is not Live.  Errors are silently dropped;
    /// the session can restart and re-sync independently.
    pub fn send_did_change(&mut self, uri: String, version: i64, text: String) -> bool {
        self.send_did_change_with_context(uri, version, text, None)
    }

    /// Sends didChange with the app-admitted operation context used for pull diagnostics.
    pub fn send_did_change_with_context(
        &mut self,
        uri: String,
        version: i64,
        text: String,
        operation_context: Option<legion_protocol::LspOperationContext>,
    ) -> bool {
        let LspSessionState::Live(worker) = &mut self.state else {
            return false;
        };
        let request = LspWorkerRequest::DidChange {
            uri,
            version,
            text,
            operation_context,
        };
        worker.request_tx.try_send(request).is_ok()
    }

    /// Reserve the cap-one request slot before invoking the text producer.
    /// The worker receives the deferred payload after admission; `None` is a
    /// cancellation and does not terminate the worker.
    pub fn send_did_change_deferred<F>(
        &mut self,
        uri: String,
        version: i64,
        produce_text: F,
    ) -> bool
    where
        F: FnOnce() -> Option<String>,
    {
        self.send_did_change_deferred_with_context(uri, version, produce_text, None)
    }

    /// Sends deferred didChange while retaining its app-admitted context.
    pub fn send_did_change_deferred_with_context<F>(
        &mut self,
        uri: String,
        version: i64,
        produce_text: F,
        operation_context: Option<legion_protocol::LspOperationContext>,
    ) -> bool
    where
        F: FnOnce() -> Option<String>,
    {
        let LspSessionState::Live(worker) = &mut self.state else {
            return false;
        };
        let (text_tx, text_rx) = mpsc::sync_channel(1);
        if worker
            .request_tx
            .try_send(LspWorkerRequest::DidChangeDeferred {
                uri,
                version,
                text_rx,
                operation_context,
            })
            .is_err()
        {
            return false;
        }
        let Some(payload) = produce_text() else {
            let _ = text_tx.send(None);
            return false;
        };
        text_tx.send(Some(payload)).is_ok()
    }

    /// Send a fire-and-forget `textDocument/didOpen` notification.
    pub fn send_did_open(
        &mut self,
        uri: String,
        language_id: String,
        version: i64,
        text: String,
        buffer_id: BufferId,
    ) -> bool {
        self.send_did_open_with_context(uri, language_id, version, text, buffer_id, None)
    }

    /// Sends didOpen with the app-admitted operation context used for pull diagnostics.
    pub fn send_did_open_with_context(
        &mut self,
        uri: String,
        language_id: String,
        version: i64,
        text: String,
        buffer_id: BufferId,
        operation_context: Option<legion_protocol::LspOperationContext>,
    ) -> bool {
        let LspSessionState::Live(worker) = &mut self.state else {
            return false;
        };
        let request = LspWorkerRequest::DidOpen {
            uri,
            language_id,
            version,
            text,
            buffer_id,
            operation_context,
        };
        worker.request_tx.try_send(request).is_ok()
    }

    /// Reserve the cap-one request slot before invoking the text producer for
    /// a didOpen notification.
    pub fn send_did_open_deferred<F>(
        &mut self,
        uri: String,
        language_id: String,
        version: i64,
        buffer_id: BufferId,
        produce_text: F,
    ) -> bool
    where
        F: FnOnce() -> Option<String>,
    {
        self.send_did_open_deferred_with_context(
            uri,
            language_id,
            version,
            buffer_id,
            produce_text,
            None,
        )
    }

    /// Sends deferred didOpen while retaining its app-admitted context.
    pub fn send_did_open_deferred_with_context<F>(
        &mut self,
        uri: String,
        language_id: String,
        version: i64,
        buffer_id: BufferId,
        produce_text: F,
        operation_context: Option<legion_protocol::LspOperationContext>,
    ) -> bool
    where
        F: FnOnce() -> Option<String>,
    {
        let LspSessionState::Live(worker) = &mut self.state else {
            return false;
        };
        let (text_tx, text_rx) = mpsc::sync_channel(1);
        if worker
            .request_tx
            .try_send(LspWorkerRequest::DidOpenDeferred {
                uri,
                language_id,
                version,
                text_rx,
                buffer_id,
                operation_context,
            })
            .is_err()
        {
            return false;
        }
        let Some(payload) = produce_text() else {
            let _ = text_tx.send(None);
            return false;
        };
        text_tx.send(Some(payload)).is_ok()
    }

    /// Send a fire-and-forget `textDocument/didClose` notification.
    pub fn send_did_close(&mut self, uri: String) -> bool {
        let LspSessionState::Live(worker) = &mut self.state else {
            return false;
        };
        let request = LspWorkerRequest::DidClose { uri };
        worker.request_tx.try_send(request).is_ok()
    }

    /// Returns the current health record if the session is live (or a
    /// synthetic unavailable record if refused/failed/backing-off).  Returns
    /// `None` when idle or starting.
    pub fn health_record(&self) -> Option<LspServerHealthRecord> {
        match &self.state {
            LspSessionState::Idle | LspSessionState::Starting { .. } => None,
            LspSessionState::Live(worker) => Some(worker.health.clone()),
            LspSessionState::BackingOff { restart_count, .. } => {
                unavailable_health_record_for(self.selected_metadata.as_ref(), *restart_count)
            }
            LspSessionState::Refused { .. } | LspSessionState::Failed { .. } => {
                unavailable_health_record_for(self.selected_metadata.as_ref(), self.restart_count)
            }
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

    /// Returns the redacted stderr ring-buffer projection whenever startup or
    /// the live session has captured at least one line; `None` otherwise
    /// (PKT-LSP-C T4).  The handle-owned ring survives startup failures and
    /// bounded automatic retries so diagnostics remain available in those
    /// states too.
    ///
    /// The lines are copies of the redacted strings stored in the ring buffer
    /// at the time of the call; they are metadata-only (all file paths have
    /// been replaced with `[REDACTED]` by the drain thread).
    pub fn stderr_log_projection(&self) -> Option<LspSessionLogProjection> {
        let guard = self.stderr_ring.lock().ok()?;
        if guard.is_empty() {
            return None;
        }
        Some(LspSessionLogProjection {
            lines: guard.iter().cloned().collect(),
            schema_version: 1,
        })
    }
}

/// Resolve one server `workspace/applyEdit` callback against the app decision
/// and its bounded reply channel.
fn await_apply_edit_response<F>(
    reply_rx: mpsc::Receiver<legion_lsp::LspApplyWorkspaceEditResponse>,
    decision: crate::language::ApplyEditDecision,
    deadline: std::time::Instant,
    before_claimed_wait: F,
) -> legion_lsp::LspApplyWorkspaceEditResponse
where
    F: FnOnce(),
{
    loop {
        let remaining = deadline.saturating_duration_since(std::time::Instant::now());
        if remaining.is_zero() {
            match decision.on_deadline() {
                crate::language::DeadlineDecision::Expired => {
                    return legion_lsp::LspApplyWorkspaceEditResponse {
                        applied: false,
                        failure_reason: Some("app applyEdit authority timed out".to_string()),
                    };
                }
                crate::language::DeadlineDecision::Claimed => {
                    before_claimed_wait();
                    let result = decision.wait_for_result();
                    return legion_lsp::LspApplyWorkspaceEditResponse {
                        applied: result.applied,
                        failure_reason: result.failure_reason,
                    };
                }
                crate::language::DeadlineDecision::Finished(result) => {
                    return legion_lsp::LspApplyWorkspaceEditResponse {
                        applied: result.applied,
                        failure_reason: result.failure_reason,
                    };
                }
            }
        }
        match reply_rx.recv_timeout(remaining.min(Duration::from_millis(50))) {
            Ok(reply) => {
                if let crate::language::DeadlineDecision::Finished(result) = decision.on_deadline()
                {
                    break legion_lsp::LspApplyWorkspaceEditResponse {
                        applied: result.applied,
                        failure_reason: result.failure_reason,
                    };
                }
                break reply;
            }
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                break legion_lsp::LspApplyWorkspaceEditResponse {
                    applied: false,
                    failure_reason: Some("app applyEdit authority disconnected".to_string()),
                };
            }
        }
    }
}

#[cfg(test)]
mod apply_edit_callback_tests {
    use super::*;
    use std::sync::{Arc, Barrier};

    #[test]
    fn claimed_at_deadline_waits_for_actual_success() {
        let decision = crate::language::ApplyEditDecision::new();
        let claim = decision
            .claim(std::time::Instant::now() + Duration::from_secs(1))
            .expect("claim");
        let (reply_tx, reply_rx) = mpsc::sync_channel(1);
        drop(reply_tx);
        let entered = Arc::new(Barrier::new(2));
        let worker_entered = Arc::clone(&entered);
        let worker = thread::spawn(move || {
            await_apply_edit_response(reply_rx, decision, std::time::Instant::now(), move || {
                worker_entered.wait();
            })
        });
        entered.wait();
        assert!(claim.finish(crate::language::ApplyEditDecisionResult::applied()));
        let response = worker.join().expect("callback worker");
        assert!(response.applied);
        assert!(response.failure_reason.is_none());
    }

    #[test]
    fn timeout_wins_and_later_claim_is_forbidden() {
        let decision = crate::language::ApplyEditDecision::new();
        let (reply_tx, reply_rx) = mpsc::sync_channel(1);
        drop(reply_tx);
        let response = await_apply_edit_response(
            reply_rx,
            decision.clone(),
            std::time::Instant::now() - Duration::from_secs(1),
            || {},
        );
        assert!(!response.applied);
        assert!(
            response
                .failure_reason
                .as_deref()
                .is_some_and(|reason| reason.contains("timed out"))
        );
        assert!(
            decision
                .claim(std::time::Instant::now() + Duration::from_secs(1))
                .is_err()
        );
    }

    #[test]
    fn completed_result_at_deadline_is_returned() {
        let decision = crate::language::ApplyEditDecision::new();
        let claim = decision
            .claim(std::time::Instant::now() + Duration::from_secs(1))
            .expect("claim");
        assert!(claim.finish(crate::language::ApplyEditDecisionResult::applied()));
        let (reply_tx, reply_rx) = mpsc::sync_channel(1);
        drop(reply_tx);
        let response =
            await_apply_edit_response(reply_rx, decision, std::time::Instant::now(), || {});
        assert!(response.applied);
        assert!(response.failure_reason.is_none());
    }
}

/// Spawns the session worker thread.  Returns `(request_tx, result_rx,
/// transport_dead)`; the terminal signal is separate from the bounded result
/// queue so a full queue cannot lose lifecycle failure information.
///
/// The channel capacities are intentionally small:
///   - `request_tx`: capacity 1 — one in-flight LSP request at a time;
///     subsequent sends are dropped and retried on the next keystroke.
///   - `result_rx`: capacity 16 — allow batching of diagnostic notifications.
fn spawn_session_worker(
    mut session: LanguageServerSession,
) -> (
    mpsc::SyncSender<LspWorkerRequest>,
    mpsc::Receiver<LspWorkerResult>,
    TransportDeathSignal,
) {
    let (request_tx, request_rx) = mpsc::sync_channel::<LspWorkerRequest>(1);
    let (result_tx, result_rx) = mpsc::sync_channel::<LspWorkerResult>(16);
    let transport_dead: TransportDeathSignal = Arc::new(Mutex::new(None));
    let worker_transport_dead = Arc::clone(&transport_dead);
    let apply_result_tx = result_tx.clone();
    session
        .session_mut()
        .set_apply_edit_handler(move |request| {
            let (reply_tx, reply_rx) = mpsc::sync_channel(1);
            let decision = crate::language::ApplyEditDecision::new();
            let deadline = request
                .deadline
                .unwrap_or_else(|| std::time::Instant::now() + Duration::from_secs(120));
            if apply_result_tx
                .try_send(LspWorkerResult::ApplyEditRequested {
                    request,
                    reply: reply_tx,
                    decision: decision.clone(),
                })
                .is_err()
            {
                return legion_lsp::LspApplyWorkspaceEditResponse {
                    applied: false,
                    failure_reason: Some("app applyEdit queue is full".to_string()),
                };
            }
            await_apply_edit_response(reply_rx, decision, deadline, || {})
        });

    thread::spawn(move || {
        run_session_worker(&mut session, request_rx, result_tx, worker_transport_dead);
    });

    (request_tx, result_rx, transport_dead)
}

/// Worker thread main loop.
///
/// - Waits up to `NOTIFICATION_POLL_INTERVAL` for an incoming request.
/// - If a request arrives, executes it (blocking LSP call).
/// - On timeout (no request), drains any buffered `publishDiagnostics`
///   notifications from the reader channel and forwards them to the frame path.
fn resolve_deferred_request(request: LspWorkerRequest) -> LspWorkerRequest {
    match request {
        LspWorkerRequest::DidChangeDeferred {
            uri,
            version,
            text_rx,
            operation_context,
        } => text_rx
            .recv()
            .ok()
            .flatten()
            .map(|text| LspWorkerRequest::DidChange {
                uri,
                version,
                text,
                operation_context,
            })
            .unwrap_or(LspWorkerRequest::DeferredCancelled),
        LspWorkerRequest::DidOpenDeferred {
            uri,
            language_id,
            version,
            text_rx,
            buffer_id,
            operation_context,
        } => text_rx
            .recv()
            .ok()
            .flatten()
            .map(|text| LspWorkerRequest::DidOpen {
                uri,
                language_id,
                version,
                text,
                buffer_id,
                operation_context,
            })
            .unwrap_or(LspWorkerRequest::DeferredCancelled),
        request => request,
    }
}

fn run_session_worker(
    session: &mut LanguageServerSession,
    request_rx: mpsc::Receiver<LspWorkerRequest>,
    result_tx: mpsc::SyncSender<LspWorkerResult>,
    transport_dead: TransportDeathSignal,
) {
    const NOTIFICATION_POLL_INTERVAL: Duration = Duration::from_millis(50);
    const MAX_PENDING_PULL_DIAGNOSTICS: usize = 32;
    let mut pending_pull_diagnostics =
        HashMap::<String, (serde_json::Value, Option<BufferId>)>::new();
    let mut pending_push_diagnostics =
        HashMap::<String, (serde_json::Value, Option<BufferId>)>::new();
    let mut document_buffers = HashMap::<String, BufferId>::new();

    'worker: loop {
        flush_pending_diagnostics(&result_tx, &mut pending_pull_diagnostics);
        flush_pending_diagnostics(&result_tx, &mut pending_push_diagnostics);
        match request_rx
            .recv_timeout(NOTIFICATION_POLL_INTERVAL)
            .map(resolve_deferred_request)
        {
            Ok(LspWorkerRequest::DeferredCancelled) => continue,
            Ok(LspWorkerRequest::DidChangeDeferred { .. })
            | Ok(LspWorkerRequest::DidOpenDeferred { .. }) => {
                unreachable!("deferred requests are resolved before worker dispatch")
            }
            Ok(LspWorkerRequest::RequestRead {
                method,
                params,
                tag,
            }) => {
                let outcome = session.request_read_with_context(
                    &method,
                    params,
                    tag.snapshot_id,
                    tag.operation_context.clone(),
                );
                // Best-effort send; if the result channel is full the result
                // is dropped.  The caller will retry on next keystroke.
                let _ = result_tx.try_send(LspWorkerResult::ReadResult { outcome, tag });
            }
            Ok(LspWorkerRequest::DidChange {
                uri,
                version,
                text,
                operation_context,
            }) => {
                let _ = session.did_change(&uri, version, &text);
                let Some(normalized_uri) = crate::normalize_lsp_document_uri(&uri) else {
                    continue;
                };
                if !request_pull_diagnostics(
                    session,
                    &result_tx,
                    &normalized_uri,
                    &mut pending_pull_diagnostics,
                    document_buffers.get(&normalized_uri).copied(),
                    MAX_PENDING_PULL_DIAGNOSTICS,
                    operation_context,
                ) {
                    break 'worker;
                }
            }
            Ok(LspWorkerRequest::DidOpen {
                uri,
                language_id,
                version,
                text,
                buffer_id,
                operation_context,
            }) => {
                let _ = session.did_open(&uri, &language_id, version, &text);
                let Some(normalized_uri) = crate::normalize_lsp_document_uri(&uri) else {
                    break 'worker;
                };
                document_buffers.insert(normalized_uri.clone(), buffer_id);
                if !request_pull_diagnostics(
                    session,
                    &result_tx,
                    &normalized_uri,
                    &mut pending_pull_diagnostics,
                    document_buffers.get(&normalized_uri).copied(),
                    MAX_PENDING_PULL_DIAGNOSTICS,
                    operation_context,
                ) {
                    break 'worker;
                }
            }
            Ok(LspWorkerRequest::DidClose { uri }) => {
                let _ = session.did_close(&uri);
                if let Some(normalized_uri) = crate::normalize_lsp_document_uri(&uri) {
                    document_buffers.remove(&normalized_uri);
                }
                if let Some(normalized_uri) = crate::normalize_lsp_document_uri(&uri) {
                    pending_pull_diagnostics.remove(&normalized_uri);
                    pending_push_diagnostics.remove(&normalized_uri);
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                // No requests pending.  Drain any buffered diagnostic
                // notifications that arrived since the last check.
                for raw_params in session.try_drain_diagnostic_params() {
                    let normalized_uri = raw_params
                        .get("uri")
                        .and_then(serde_json::Value::as_str)
                        .and_then(crate::normalize_lsp_document_uri);
                    let captured_buffer_id = normalized_uri
                        .as_ref()
                        .and_then(|uri| document_buffers.get(uri).copied());
                    if !queue_latest_diagnostics(
                        &result_tx,
                        &mut pending_push_diagnostics,
                        raw_params,
                        captured_buffer_id,
                        MAX_PENDING_PULL_DIAGNOSTICS,
                    ) {
                        break 'worker;
                    }
                }
                // Transport-death detection (PKT-S3-WEDGE-R3): once the
                // stdout reader thread records a terminal event, this session
                // can never surface another frame — before this check, a
                // crashed or reader-killed server left the session projecting
                // Live forever while draining nothing.  Report once and exit;
                // the frame path routes the report through the restart
                // circuit breaker.
                if let Some(terminal) = session.reader_stats().terminal {
                    let reason = match terminal {
                        legion_lsp::LspReaderTerminal::Eof => {
                            "server closed stdout (process exit or crash)".to_string()
                        }
                        legion_lsp::LspReaderTerminal::Error(message) => {
                            format!(
                                "stdout reader terminated: {}",
                                super::redact_lsp_stderr_line(&message)
                            )
                        }
                    };
                    let bounded_reason = truncate_reason(&reason);
                    if let Ok(mut terminal) = transport_dead.lock()
                        && terminal.is_none()
                    {
                        *terminal = Some(bounded_reason.clone());
                    }
                    let _ = result_tx.try_send(LspWorkerResult::TransportDead {
                        reason: bounded_reason,
                    });
                    break;
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

/// Request native diagnostics after a document sync when the server advertises
/// LSP 3.17 pull diagnostics. Servers without that capability continue to use
/// their `publishDiagnostics` notifications, which are drained by the idle
/// branch above. The request runs on the session worker, so a slow server never
/// blocks the frame thread; a full result queue simply coalesces with the next
/// document update.
fn request_pull_diagnostics(
    session: &mut LanguageServerSession,
    result_tx: &mpsc::SyncSender<LspWorkerResult>,
    uri: &str,
    pending: &mut HashMap<String, (serde_json::Value, Option<BufferId>)>,
    captured_buffer_id: Option<BufferId>,
    pending_limit: usize,
    operation_context: Option<legion_protocol::LspOperationContext>,
) -> bool {
    if !session.supports_pull_diagnostics() {
        return true;
    }
    let Some(operation_context) = operation_context else {
        return true;
    };
    let Ok(report) = session.pull_diagnostics_with_context(uri, operation_context) else {
        return true;
    };
    if let Some(raw_params) = report.publish_params {
        return queue_latest_diagnostics(
            result_tx,
            pending,
            raw_params,
            captured_buffer_id,
            pending_limit,
        );
    }
    true
}

/// Flushes the bounded latest-report-per-document slot without blocking the
/// worker. A full result queue leaves the remaining entries for the next
/// worker iteration, after the frame path has drained older results.
fn flush_pending_diagnostics(
    result_tx: &mpsc::SyncSender<LspWorkerResult>,
    pending: &mut HashMap<String, (serde_json::Value, Option<BufferId>)>,
) {
    let keys = pending.keys().cloned().collect::<Vec<_>>();
    for uri in keys {
        let Some((raw_params, captured_buffer_id)) = pending.get(&uri).cloned() else {
            continue;
        };
        if result_tx
            .try_send(LspWorkerResult::DiagnosticBatch {
                raw_params,
                captured_buffer_id,
            })
            .is_err()
        {
            break;
        }
        pending.remove(&uri);
    }
}

fn queue_latest_diagnostics(
    result_tx: &mpsc::SyncSender<LspWorkerResult>,
    pending: &mut HashMap<String, (serde_json::Value, Option<BufferId>)>,
    raw_params: serde_json::Value,
    captured_buffer_id: Option<BufferId>,
    pending_limit: usize,
) -> bool {
    if captured_buffer_id.is_none() {
        return true;
    }
    let Some(uri) = raw_params
        .get("uri")
        .and_then(serde_json::Value::as_str)
        .and_then(crate::normalize_lsp_document_uri)
    else {
        return true;
    };
    let item = LspWorkerResult::DiagnosticBatch {
        raw_params: raw_params.clone(),
        captured_buffer_id,
    };
    match result_tx.try_send(item) {
        Ok(()) => {
            pending.remove(&uri);
            true
        }
        Err(mpsc::TrySendError::Disconnected(_)) => false,
        Err(mpsc::TrySendError::Full(item)) => {
            if pending.contains_key(&uri) || pending.len() < pending_limit {
                pending.insert(uri, (raw_params, captured_buffer_id));
                true
            } else {
                // This is the worker thread, never the frame path. Preserve
                // delivery for a new URI by applying bounded backpressure
                // until the frame drains a result or teardown drops the
                // receiver. The latter releases `send` with an error.
                result_tx.send(item).is_ok()
            }
        }
    }
}

/// Background drain thread for LSP server stderr (PKT-LSP-C T4).
///
/// Reads `stderr` line by line, redacts each line via
/// `redact_lsp_stderr_line`, truncates lines exceeding
/// `STDERR_LINE_MAX_LEN`, and appends to the shared `ring` buffer.
/// When the ring is at capacity the oldest line is evicted (FIFO).
/// The thread exits when the child process closes its stderr pipe.
fn drain_stderr(stderr: std::process::ChildStderr, ring: Arc<Mutex<VecDeque<String>>>) {
    drain_stderr_reader(stderr, ring);
}

/// Drains an arbitrary stderr reader without allowing an unterminated line to
/// grow in memory.  Bytes after the retained prefix are consumed until the
/// next newline so subsequent lines remain aligned.
pub(super) fn drain_stderr_reader<R: Read>(mut reader: R, ring: Arc<Mutex<VecDeque<String>>>) {
    let mut read_buf = [0_u8; 4096];
    let mut line = Vec::with_capacity(STDERR_LINE_MAX_LEN);
    let mut truncated = false;

    loop {
        let read = match reader.read(&mut read_buf) {
            Ok(0) => break,
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => break,
            Ok(read) => read,
        };
        for &byte in &read_buf[..read] {
            if byte == b'\n' {
                // Match `BufRead::lines()` for CRLF while retaining a
                // standalone CR in a partial EOF line.
                let raw_line = if !truncated {
                    line.strip_suffix(b"\r").unwrap_or(&line)
                } else {
                    &line
                };
                append_stderr_line(
                    raw_line,
                    truncated || raw_line.len() > STDERR_LINE_MAX_LEN,
                    &ring,
                );
                line.clear();
                truncated = false;
                continue;
            }
            // Keep one possible CRLF byte beyond the retained prefix so a
            // line exactly at the limit is not marked truncated.
            if line.len() < STDERR_LINE_MAX_LEN + 1 {
                line.push(byte);
            } else {
                truncated = true;
            }
        }
    }

    if !line.is_empty() || truncated {
        // A final CR without a following LF is part of the partial line.
        append_stderr_line(&line, truncated || line.len() > STDERR_LINE_MAX_LEN, &ring);
    }
}

fn append_stderr_line(raw_line: &[u8], truncated: bool, ring: &Arc<Mutex<VecDeque<String>>>) {
    // This happens before UTF-8 conversion and cannot panic on invalid server
    // output.
    let rendered = render_stderr_line(raw_line, truncated);
    let redacted = super::redact_lsp_stderr_line(&rendered);
    // Redaction markers can expand many short path tokens, so enforce the
    // retained-line byte cap again after redaction.
    let redacted = if redacted.len() > STDERR_LINE_MAX_LEN {
        render_stderr_line(redacted.as_bytes(), true)
    } else {
        redacted
    };
    if let Ok(mut guard) = ring.lock() {
        if guard.len() >= STDERR_RING_CAPACITY {
            guard.pop_front();
        }
        guard.push_back(redacted);
    }
}

/// True when both paths name the same directory after filesystem
/// canonicalize. macOS `/var` vs `/private/var` and Windows `\\?\` prefixes
/// must not refuse an otherwise valid language-server start.
fn same_workspace_root(left: &Path, right: &Path) -> bool {
    match (std::fs::canonicalize(left), std::fs::canonicalize(right)) {
        (Ok(left), Ok(right)) => left == right,
        _ => left == right,
    }
}

fn render_stderr_line(raw_line: &[u8], truncated: bool) -> String {
    let lossy = String::from_utf8_lossy(raw_line);
    if !truncated && lossy.len() <= STDERR_LINE_MAX_LEN {
        return lossy.into_owned();
    }

    let marker = "…";
    let prefix_limit = STDERR_LINE_MAX_LEN.saturating_sub(marker.len());
    let mut rendered = String::with_capacity(STDERR_LINE_MAX_LEN);
    for ch in lossy.chars() {
        if rendered.len() + ch.len_utf8() > prefix_limit {
            break;
        }
        rendered.push(ch);
    }
    rendered.push_str(marker);
    rendered
}

/// Runs full startup sequence: discovery → launch → initialize.
/// Called on the background startup thread only.
///
/// `configured_server_path` overrides PATH-based discovery when `Some`.
/// Tests pass the mock binary path this way rather than mutating the process
/// environment (which races in parallel test execution).
#[cfg(any(test, feature = "test-helpers"))]
fn startup_session(
    workspace_root: &Path,
    root_uri: &str,
    configured_server_path: Option<PathBuf>,
    stderr_ring: Arc<Mutex<VecDeque<String>>>,
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
        required_capability: CapabilityId("lsp.launch".to_string()),
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
    if let Some(stderr) = session.take_stderr() {
        thread::spawn(move || {
            drain_stderr(stderr, stderr_ring);
        });
    }
    // Pass `files.watcher: "client"` so rust-analyzer does not start its own
    // notify file-watcher on the workspace root.  The notify watcher fails on
    // temp-path workspaces with a "Input watch path is neither a file nor a
    // directory" error that wedges RA's analysis loop (M8 PKT-S3-WEDGE-R3).
    // The product session already sends `didOpen`/`didChange`/`didClose`, so
    // external file-change notifications are the only thing lost — an
    // acceptable trade-off for private beta.
    session.initialize_with_options(
        root_uri,
        Some(serde_json::json!({"files": {"watcher": "client"}})),
        None,
    )?;

    Ok(session)
}

/// Runs generic configured startup on the background startup thread.
///
/// The descriptor is supplied by the app authority.  In particular, this
/// function intentionally does not discover a binary, consult a broker, or
/// manufacture identity/approval metadata.
fn startup_configured_session(
    config: Arc<LanguageServerStartConfig>,
    cancel: Arc<AtomicBool>,
    stderr_ring: Arc<Mutex<VecDeque<String>>>,
) -> Result<LanguageServerSession, LanguageSessionError> {
    if cancel.load(Ordering::Acquire) {
        return Err(LanguageSessionError::Unavailable);
    }
    let canonical_root = std::fs::canonicalize(&config.workspace_root).map_err(|error| {
        LanguageSessionError::InvalidConfiguration(format!(
            "workspace root cannot be canonicalized: {error}"
        ))
    })?;
    if !canonical_root.is_dir() {
        return Err(LanguageSessionError::InvalidConfiguration(
            "workspace root is not a directory".to_string(),
        ));
    }
    // Compare after filesystem canonicalize. macOS `/var` vs `/private/var`
    // and Windows `\\?\` prefixes produce different URI spellings for the
    // same directory; a string compare of those spellings refused Live.
    let incoming_root = crate::uri_to_canonical_path(&config.root_uri);
    let incoming_canonical = std::fs::canonicalize(&incoming_root).map_err(|error| {
        LanguageSessionError::InvalidConfiguration(format!(
            "root_uri cannot be canonicalized: {error}"
        ))
    })?;
    if incoming_canonical != canonical_root {
        return Err(LanguageSessionError::InvalidConfiguration(
            "root_uri does not match the configured workspace root".to_string(),
        ));
    }
    let root_uri = config.root_uri.clone();
    let initialization_options = config.initialization_options.clone();
    let client_capabilities = config.client_capabilities.clone();
    let mut launcher = LspStdioLauncher::new();
    let mut session = LanguageServerSession::launch_configured(
        clone_launch_config(&config.launch_config),
        &mut launcher,
    )?;
    if let Some(stderr) = session.take_stderr() {
        thread::spawn(move || {
            drain_stderr(stderr, stderr_ring);
        });
    }
    if cancel.load(Ordering::Acquire) {
        return Err(LanguageSessionError::Unavailable);
    }
    session.initialize_with_options(&root_uri, initialization_options, client_capabilities)?;

    if cancel.load(Ordering::Acquire) {
        return Err(LanguageSessionError::Unavailable);
    }

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
            transport_dead: Arc::new(Mutex::new(None)),
        }));
    }

    /// Test-only live session setup that returns the production request queue
    /// receiver so callers can inspect fire-and-forget requests.
    #[cfg(any(test, feature = "test-helpers"))]
    pub fn set_live_with_request_receiver_for_test(
        &mut self,
        health: LspServerHealthRecord,
    ) -> mpsc::Receiver<LspWorkerRequest> {
        let (request_rx, result_tx) = self.set_live_with_request_and_result_sender_for_test(health);
        // Preserve the historical receiver-only helper's behavior while
        // keeping the result channel connected for the lifetime of the test
        // session.  New tests should retain the returned sender explicitly.
        std::mem::forget(result_tx);
        request_rx
    }

    /// Test-only live setup that exposes both bounded channels.  The result
    /// sender must be retained by the harness so `drain` does not interpret a
    /// dropped test channel as worker termination.
    #[cfg(any(test, feature = "test-helpers"))]
    pub fn set_live_with_request_and_result_sender_for_test(
        &mut self,
        health: LspServerHealthRecord,
    ) -> (
        mpsc::Receiver<LspWorkerRequest>,
        mpsc::SyncSender<LspWorkerResult>,
    ) {
        // Match the production worker queue so tests exercise cap-one
        // admission and backpressure rather than an oversized fake queue.
        let (request_tx, request_rx) = mpsc::sync_channel::<LspWorkerRequest>(1);
        let (result_tx, result_rx) = mpsc::sync_channel::<LspWorkerResult>(16);
        self.state = LspSessionState::Live(Box::new(LspWorkerHandle {
            health,
            request_tx,
            result_rx,
            transport_dead: Arc::new(Mutex::new(None)),
        }));
        (request_rx, result_tx)
    }

    /// Test-only: directly inject lines (already-redacted) into the stderr ring
    /// buffer of a `Live` session handle.  A no-op when the handle is not Live.
    /// Used by T4 tests to exercise `stderr_log_projection()` without a real
    /// child process (PKT-LSP-C T4).
    #[cfg(any(test, feature = "test-helpers"))]
    pub fn inject_stderr_ring_for_test(&mut self, lines: Vec<String>) {
        let Ok(mut guard) = self.stderr_ring.lock() else {
            return;
        };
        for line in lines {
            if guard.len() >= STDERR_RING_CAPACITY {
                guard.pop_front();
            }
            guard.push_back(line);
        }
    }

    /// Test-only: signal a transport death through the production sideband
    /// while the bounded result queue may be full.
    #[cfg(any(test, feature = "test-helpers"))]
    pub fn signal_transport_dead_for_test(&mut self, reason: impl Into<String>) {
        let LspSessionState::Live(worker) = &self.state else {
            return;
        };
        if let Ok(mut signal) = worker.transport_dead.lock() {
            *signal = Some(reason.into());
        }
    }

    /// Test-only: like [`Self::set_live_health_for_test`] but returns the
    /// result-channel sender so a test can inject `LspWorkerResult`s and
    /// observe how the frame path routes them (PKT-S3-WEDGE-R3).
    #[cfg(any(test, feature = "test-helpers"))]
    pub fn set_live_with_result_sender_for_test(
        &mut self,
        health: LspServerHealthRecord,
    ) -> mpsc::SyncSender<LspWorkerResult> {
        let (request_tx, request_rx) = mpsc::sync_channel::<LspWorkerRequest>(64);
        let (result_tx, result_rx) = mpsc::sync_channel::<LspWorkerResult>(16);
        std::mem::forget(request_rx);
        self.state = LspSessionState::Live(Box::new(LspWorkerHandle {
            health,
            request_tx,
            result_rx,
            transport_dead: Arc::new(Mutex::new(None)),
        }));
        result_tx
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

impl Drop for LspSessionHandle {
    fn drop(&mut self) {
        self.cancel_pending_start();
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
fn unavailable_health_record_for(
    metadata: Option<&LspSelectedServerMetadata>,
    restart_count: u32,
) -> Option<LspServerHealthRecord> {
    let metadata = metadata?;
    Some(LspServerHealthRecord {
        server_id: metadata.server_id,
        language_id: metadata.language_id.clone(),
        binary_provenance: metadata.binary_provenance,
        binary_path_hash: None,
        artifact_hash: metadata.artifact_hash.clone(),
        version: metadata.version.clone(),
        init_status: LspResultStatus::Unavailable,
        capabilities: Vec::new(),
        diagnostics_latency_ms: None,
        restart_count,
        download_decision_id: metadata.download_decision_id,
        schema_version: LspServerHealthRecord::schema_version(),
    })
}

/// Legacy compatibility health record for test-only injected handles.
#[cfg(test)]
fn unavailable_health_record() -> LspServerHealthRecord {
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
    crate::canonical_path_to_uri(&path.to_string_lossy())
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
        let root = std::env::temp_dir().join(format!("legion-lsp-t3-breaker-{nanos}"));
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
        assert!(
            !handle.is_backing_off(),
            "must not be backing off after explicit restart"
        );

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

#[cfg(test)]
mod generic_startup_tests {
    use super::*;
    use legion_protocol::{
        CapabilityDecisionId, CapabilityId, CausalityId, CorrelationId, FileFingerprint,
        LspConfiguredServerIdentity, LspLaunchPolicyDecision, LspServerBinaryProvenance,
        LspSessionLifecycleKind, LspWorkspaceTrustPosture, RedactionHint, SemanticPrivacyScope,
        WorkspaceId, WorkspaceRootId, WorkspaceTrustState,
    };
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use uuid::Uuid;

    fn wait_for_backoff(handle: &mut LspSessionHandle) {
        for _ in 0..100 {
            handle.drain();
            if handle.is_backing_off() {
                return;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        panic!("startup did not reach BackingOff within bounded test window");
    }

    fn wait_for_refused_or_failed(handle: &mut LspSessionHandle) {
        for _ in 0..100 {
            handle.drain();
            if handle.is_refused_or_failed() {
                return;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        panic!("startup did not reach Refused/Failed within bounded test window");
    }

    fn wait_for_flag(flag: &AtomicBool, what: &str) {
        for _ in 0..200 {
            if flag.load(Ordering::Acquire) {
                return;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        panic!("{what} was not observed within bounded test window");
    }

    fn configured_python_start() -> Option<LanguageServerStartConfig> {
        let executable = std::env::current_exe().ok()?;
        let profile_dir = executable.parent()?.parent()?;
        let binary = profile_dir.join(if cfg!(windows) {
            "mock_lsp_server.exe"
        } else {
            "mock_lsp_server"
        });
        if !binary.is_file() {
            return None;
        }
        let workspace_root = profile_dir.to_path_buf();
        let command = binary.to_string_lossy().into_owned();
        let identity = LspConfiguredServerIdentity {
            server_id: LanguageServerId(901),
            workspace_id: WorkspaceId(902),
            root_id: Some(WorkspaceRootId(903)),
            language_id: LanguageId("python".to_string()),
            display_name: "mock-pyright".to_string(),
            command_hash: FileFingerprint {
                algorithm: "test".to_string(),
                value: "mock-python".to_string(),
            },
            args_hash: None,
            env_hash: None,
            cwd_hash: None,
            settings_hash: None,
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        };
        let posture = LspWorkspaceTrustPosture {
            workspace_id: WorkspaceId(902),
            workspace_trust_state: WorkspaceTrustState::Trusted,
            privacy_scope: SemanticPrivacyScope::Workspace,
            privacy_scope_allowed: true,
            required_capability: CapabilityId("lsp.launch".to_string()),
            decision_id: Some(CapabilityDecisionId(904)),
            diagnostics: Vec::new(),
            schema_version: 1,
        };
        let policy = LspLaunchPolicyDecision::evaluate(
            identity,
            posture,
            true,
            CorrelationId(905),
            CausalityId(Uuid::from_u128(906)),
            Vec::new(),
            1,
        );
        let root_uri = path_to_file_uri(&workspace_root);
        Some(LanguageServerStartConfig {
            workspace_root,
            root_uri,
            launch_config: LanguageServerLaunchConfig {
                supervisor: LspSupervisorConfig {
                    launch_policy: policy,
                    process: LspServerProcessConfig {
                        command,
                        args: vec!["--stdio".to_string(), "--python".to_string()],
                        cwd: None,
                        env: Vec::new(),
                    },
                    initial_backoff_ms: 10,
                    max_backoff_ms: 100,
                    max_restart_attempts: 1,
                },
                server_id: LanguageServerId(901),
                language_id: LanguageId("python".to_string()),
                binary_provenance: LspServerBinaryProvenance::Downloaded,
                artifact_hash: Some(FileFingerprint {
                    algorithm: "sha256".to_string(),
                    value: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
                        .to_string(),
                }),
                artifact_dependencies: Vec::new(),
                version: Some("1.1.400".to_string()),
                node_runtime_version: None,
                download_decision_id: None,
            },
            initialization_options: None,
            client_capabilities: None,
        })
    }

    fn test_metadata() -> LspSelectedServerMetadata {
        LspSelectedServerMetadata {
            server_id: LanguageServerId(901),
            language_id: LanguageId("python".to_string()),
            binary_provenance: LspServerBinaryProvenance::Downloaded,
            artifact_hash: None,
            version: Some("1.1.400".to_string()),
            download_decision_id: Some(CapabilityDecisionId(904)),
        }
    }

    #[test]
    fn configured_non_rust_mock_launch_reaches_live_with_python_health() {
        let Some(config) = configured_python_start() else {
            eprintln!("skip: mock_lsp_server binary not found");
            return;
        };
        let mut handle = LspSessionHandle::new();
        handle.start_for_workspace_with_config(config);
        for _ in 0..250 {
            handle.drain();
            if handle.is_live() {
                let health = handle.health_record().expect("live health");
                assert_eq!(health.language_id, LanguageId("python".to_string()));
                assert_eq!(health.server_id, LanguageServerId(901));
                return;
            }
            if handle.is_refused_or_failed() {
                panic!(
                    "configured Python mock launch failed: {:?}",
                    handle.session_status_projection()
                );
            }
            std::thread::sleep(Duration::from_millis(4));
        }
        panic!("configured Python mock launch did not become live");
    }

    #[test]
    fn cancelled_deferred_payload_keeps_mock_worker_alive_for_next_read() {
        // This is a required worker contract test. Build the fixture first
        // with `cargo build -p legion-lsp --bin mock_lsp_server`.
        let config = configured_python_start()
            .expect("mock_lsp_server is required; build it with cargo build -p legion-lsp --bin mock_lsp_server");
        let stderr_ring = Arc::new(Mutex::new(VecDeque::new()));
        let session = startup_configured_session(
            Arc::new(config),
            Arc::new(AtomicBool::new(false)),
            stderr_ring,
        )
        .expect("mock session initializes");
        let (request_tx, request_rx) = mpsc::sync_channel::<LspWorkerRequest>(1);
        let (result_tx, result_rx) = mpsc::sync_channel::<LspWorkerResult>(16);
        let transport_dead = Arc::new(Mutex::new(None));
        let worker_transport_dead = Arc::clone(&transport_dead);
        let worker = thread::spawn(move || {
            let mut session = session;
            run_session_worker(&mut session, request_rx, result_tx, worker_transport_dead);
        });

        let (payload_tx, payload_rx) = mpsc::sync_channel(1);
        let operation_context = crate::language::operation_context_for_snapshot(SnapshotId(1));
        request_tx
            .send(LspWorkerRequest::DidChangeDeferred {
                uri: "file:///tmp/cancel.rs".to_string(),
                version: 1,
                text_rx: payload_rx,
                operation_context: Some(operation_context.clone()),
            })
            .expect("enqueue deferred request");
        payload_tx.send(None).expect("deliver cancellation");
        request_tx
            .send(LspWorkerRequest::RequestRead {
                method: "textDocument/hover".to_string(),
                params: serde_json::json!({
                    "textDocument": {"uri": "file:///tmp/cancel.rs"},
                    "position": {"line": 0, "character": 0}
                }),
                tag: LspRequestTag {
                    buffer_id: BufferId(1),
                    kind: LspReadKind::Hover,
                    snapshot_id: SnapshotId(1),
                    operation_id: None,
                    operation_context: Some(operation_context),
                },
            })
            .expect("enqueue read after cancellation");

        let result = result_rx
            .recv_timeout(Duration::from_secs(10))
            .expect("mock worker must answer the subsequent read");
        assert!(matches!(
            result,
            LspWorkerResult::ReadResult { outcome: Ok(_), .. }
        ));
        drop(request_tx);
        worker
            .join()
            .expect("mock worker exits after channel close");
    }

    #[test]
    fn configured_handshake_failure_retains_early_stderr() {
        let Some(mut config) = configured_python_start() else {
            eprintln!("skip: mock_lsp_server binary not found");
            return;
        };
        config
            .launch_config
            .supervisor
            .process
            .env
            .push(("MOCK_LSP_FAIL_INITIALIZE".to_string(), "1".to_string()));

        let mut handle = LspSessionHandle::new();
        handle.start_for_workspace_with_config(config);
        for _ in 0..2_000 {
            handle.drain();
            if handle.is_refused_or_failed() {
                break;
            }
            std::thread::sleep(Duration::from_millis(4));
        }
        assert!(
            handle.is_refused_or_failed(),
            "handshake fixture should exhaust bounded startup retries: {:?}",
            handle.session_status_projection()
        );

        for _ in 0..100 {
            handle.drain();
            if let Some(log) = handle.stderr_log_projection() {
                assert!(
                    log.lines
                        .iter()
                        .any(|line| line.contains("initialize fixture failure"))
                );
                return;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        panic!(
            "handshake failure lost early stderr: status={:?}",
            handle.session_status_projection()
        );
    }

    #[test]
    fn replacing_or_dropping_start_cancels_late_preparation() {
        let started = Arc::new(AtomicBool::new(false));
        let cancelled = Arc::new(AtomicBool::new(false));
        let observed_started = Arc::clone(&started);
        let observed_cancelled = Arc::clone(&cancelled);
        let mut handle = LspSessionHandle::new();
        handle.start_preparing(
            PathBuf::from("C:/legion-old"),
            test_metadata(),
            move |cancel| {
                observed_started.store(true, Ordering::Release);
                while !cancel.load(Ordering::Acquire) {
                    std::thread::sleep(Duration::from_millis(1));
                }
                observed_cancelled.store(true, Ordering::Release);
                Err(LanguageSessionError::Unavailable)
            },
        );
        wait_for_flag(&started, "original preparation start");

        handle.restart_for_workspace_preparing(
            PathBuf::from("C:/legion-new"),
            test_metadata(),
            |_cancel| {
                Err(LanguageSessionError::InvalidConfiguration(
                    "replacement denied in test".to_string(),
                ))
            },
        );
        wait_for_refused_or_failed(&mut handle);
        // Replacement can refuse immediately; the superseded prepare thread
        // may still be inside its 1 ms cancel poll. Wait for it the same way
        // the drop half already waits — loaded macOS CI was losing that race.
        wait_for_flag(&cancelled, "superseded preparation cancel");

        let dropped = Arc::new(AtomicBool::new(false));
        let observed_dropped = Arc::clone(&dropped);
        let (entered_tx, entered_rx) = mpsc::sync_channel(1);
        let mut pending = LspSessionHandle::new();
        pending.start_preparing(
            PathBuf::from("C:/legion-drop"),
            test_metadata(),
            move |cancel| {
                entered_tx.send(()).expect("drop test receiver is alive");
                while !cancel.load(Ordering::Acquire) {
                    std::thread::sleep(Duration::from_millis(1));
                }
                observed_dropped.store(true, Ordering::Release);
                Err(LanguageSessionError::Unavailable)
            },
        );
        assert!(
            entered_rx.recv_timeout(Duration::from_secs(1)).is_ok(),
            "drop test preparation did not start"
        );
        drop(pending);
        wait_for_flag(&dropped, "dropped preparation cancel");
    }

    #[test]
    fn configured_preparation_failure_does_not_attempt_a_launch() {
        let calls = Arc::new(AtomicUsize::new(0));
        let observed_calls = Arc::clone(&calls);
        let mut handle = LspSessionHandle::new();
        handle.start_preparing(
            PathBuf::from("C:/legion-test-python"),
            test_metadata(),
            move |_cancel| {
                observed_calls.fetch_add(1, Ordering::SeqCst);
                Err(LanguageSessionError::InvalidConfiguration(
                    "test approval denied".to_string(),
                ))
            },
        );

        wait_for_refused_or_failed(&mut handle);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(
            handle.session_status_projection().lifecycle,
            LspSessionLifecycleKind::Refused
        );
    }

    #[test]
    fn automatic_retry_invokes_fresh_preparation_each_time() {
        let calls = Arc::new(AtomicUsize::new(0));
        let observed_calls = Arc::clone(&calls);
        let mut handle = LspSessionHandle::new();
        handle.start_preparing_with_factory(
            PathBuf::from("C:/legion-test-python"),
            test_metadata(),
            move |_cancel| {
                observed_calls.fetch_add(1, Ordering::SeqCst);
                Err(LanguageSessionError::Unavailable)
            },
        );

        wait_for_backoff(&mut handle);
        assert_eq!(calls.load(Ordering::SeqCst), 1);

        handle.set_backing_off_for_test(1, true);
        assert!(handle.drain());
        wait_for_backoff(&mut handle);
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn failed_python_preparation_preserves_selected_health_metadata() {
        let mut handle = LspSessionHandle::new();
        handle.start_preparing(
            PathBuf::from("C:/legion-python"),
            test_metadata(),
            |_cancel| Err(LanguageSessionError::Unavailable),
        );

        wait_for_backoff(&mut handle);
        let health = handle.health_record().expect("selected metadata health");
        assert_eq!(health.server_id, LanguageServerId(901));
        assert_eq!(health.language_id, LanguageId("python".to_string()));
        assert_eq!(
            health.binary_provenance,
            LspServerBinaryProvenance::Downloaded
        );
        assert_eq!(health.version.as_deref(), Some("1.1.400"));
        assert_eq!(health.download_decision_id, Some(CapabilityDecisionId(904)));
        assert_eq!(health.restart_count, 1);
    }

    #[test]
    fn one_shot_preparation_refuses_automatic_retry_without_factory() {
        let mut handle = LspSessionHandle::new();
        handle.start_preparing(
            PathBuf::from("C:/legion-test-python"),
            test_metadata(),
            |_cancel| Err(LanguageSessionError::Unavailable),
        );

        wait_for_backoff(&mut handle);
        handle.set_backing_off_for_test(1, true);
        assert!(handle.drain());
        assert!(handle.is_refused_or_failed());
        assert_eq!(
            handle.session_status_projection().failure_reason.as_deref(),
            Some("automatic restart requires fresh app preparation")
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PKT-LSP-C T4: stderr ring buffer as redacted projection — TDD tests
// ─────────────────────────────────────────────────────────────────────────────
#[cfg(test)]
mod stderr_tests {
    use super::*;
    use legion_protocol::LspServerBinaryProvenance;
    use std::io::{self, Cursor, Read};
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn make_live_handle() -> LspSessionHandle {
        let mut handle = LspSessionHandle::new();
        let health = LspServerHealthRecord {
            server_id: LanguageServerId(1),
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
        };
        handle.set_live_health_for_test(health);
        handle
    }

    #[test]
    fn deferred_document_send_does_not_produce_when_offline() {
        let mut handle = LspSessionHandle::new();
        let calls = AtomicUsize::new(0);
        assert!(
            !handle.send_did_change_deferred("file:///tmp/test.rs".to_string(), 1, || {
                calls.fetch_add(1, Ordering::SeqCst);
                Some("text".to_string())
            },)
        );
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn deferred_document_send_reserves_before_text_production() {
        let mut handle = LspSessionHandle::new();
        let health = LspServerHealthRecord {
            server_id: LanguageServerId(1),
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
        };
        let (request_rx, _result_tx) =
            handle.set_live_with_request_and_result_sender_for_test(health);
        assert!(handle.send_did_change("file:///tmp/full.rs".to_string(), 1, "x".to_string()));
        let calls = AtomicUsize::new(0);
        assert!(
            !handle.send_did_change_deferred("file:///tmp/full.rs".to_string(), 2, || {
                calls.fetch_add(1, Ordering::SeqCst);
                Some("new".to_string())
            },)
        );
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        let _ = request_rx
            .recv()
            .expect("the first request occupies the slot");
    }

    #[test]
    fn deferred_document_send_produces_once_after_admission() {
        let mut handle = LspSessionHandle::new();
        let health = LspServerHealthRecord {
            server_id: LanguageServerId(1),
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
        };
        let (request_rx, _result_tx) =
            handle.set_live_with_request_and_result_sender_for_test(health);
        let calls = AtomicUsize::new(0);
        assert!(
            handle.send_did_change_deferred("file:///tmp/admitted.rs".to_string(), 1, || {
                calls.fetch_add(1, Ordering::SeqCst);
                Some("accepted".to_string())
            },)
        );
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        let request = request_rx.recv().expect("admitted request");
        assert!(matches!(
            resolve_deferred_request(request),
            LspWorkerRequest::DidChange { text, version: 1, .. } if text == "accepted"
        ));
    }

    #[test]
    fn deferred_none_payload_does_not_disconnect_request_channel() {
        let mut handle = LspSessionHandle::new();
        let health = LspServerHealthRecord {
            server_id: LanguageServerId(1),
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
        };
        let (request_rx, _result_tx) =
            handle.set_live_with_request_and_result_sender_for_test(health);
        assert!(!handle.send_did_change_deferred("file:///tmp/cancel.rs".to_string(), 1, || None,));
        let _ = request_rx
            .recv()
            .expect("cancelled request remains observable");
        assert!(handle.send_did_change("file:///tmp/next.rs".to_string(), 2, "ok".to_string()));
        let _ = request_rx.recv().expect("next request remains admissible");
    }

    // ── T4-1: Projection is None when session is Idle ────────────────────────

    #[test]
    fn t4_stderr_projection_none_when_idle() {
        let handle = LspSessionHandle::new();
        assert!(
            handle.stderr_log_projection().is_none(),
            "idle handle must not project stderr"
        );
    }

    #[test]
    fn t4_stderr_projection_survives_handshake_failure_state() {
        let mut handle = LspSessionHandle::new();
        handle.state = LspSessionState::Failed {
            reason: "language-server handshake failed".to_string(),
        };
        drain_stderr_reader(
            Cursor::new(b"mock handshake diagnostic\n"),
            Arc::clone(&handle.stderr_ring),
        );

        let projection = handle
            .stderr_log_projection()
            .expect("startup stderr must remain available after handshake failure");
        assert_eq!(projection.lines, vec!["mock handshake diagnostic"]);
    }

    // ── T4-2: Projection is None when ring is empty ──────────────────────────

    #[test]
    fn t4_stderr_projection_none_when_ring_empty() {
        let handle = make_live_handle();
        assert!(
            handle.stderr_log_projection().is_none(),
            "empty ring must yield None projection"
        );
    }

    // ── T4-3: Ring cap is respected ──────────────────────────────────────────

    /// Inject 110 lines into a cap-100 ring; the projection must contain
    /// exactly 100 lines (oldest evicted).
    #[test]
    fn t4_ring_buffer_cap_respected() {
        let mut handle = make_live_handle();
        let lines: Vec<String> = (0..110).map(|i| format!("line {i}")).collect();
        handle.inject_stderr_ring_for_test(lines);

        let proj = handle
            .stderr_log_projection()
            .expect("projection must be Some after injecting lines");
        assert_eq!(
            proj.lines.len(),
            STDERR_RING_CAPACITY,
            "ring must be capped at STDERR_RING_CAPACITY"
        );
        // The oldest lines (0..10) should have been evicted; the newest
        // (10..110) should remain.
        assert_eq!(proj.lines[0], "line 10", "oldest surviving line is line 10");
        assert_eq!(
            proj.lines[STDERR_RING_CAPACITY - 1],
            "line 109",
            "newest line is line 109"
        );
    }

    // ── T4-4: Lines injected are returned by projection ──────────────────────

    #[test]
    fn t4_injected_lines_appear_in_projection() {
        let mut handle = make_live_handle();
        handle.inject_stderr_ring_for_test(vec![
            "[REDACTED]".to_string(),
            "ERROR: initialization failed".to_string(),
        ]);

        let proj = handle
            .stderr_log_projection()
            .expect("projection must be Some");
        assert_eq!(proj.lines.len(), 2);
        assert_eq!(proj.lines[0], "[REDACTED]");
        assert_eq!(proj.lines[1], "ERROR: initialization failed");
    }

    // ── T4-5: No raw secret path reaches the projection ──────────────────────

    /// Simulate the drain thread's redaction: run `redact_lsp_stderr_line` on
    /// a sentinel-containing line, inject the result, and confirm the sentinel
    /// never appears in the projection.
    #[test]
    fn t4_no_raw_secret_in_projection() {
        let sentinel = "/very/secret/workspace/src/main.rs";
        let raw_line = format!("rust-analyzer analysing {sentinel}");

        // The drain thread calls `redact_lsp_stderr_line` before storing.
        let redacted = super::super::redact_lsp_stderr_line(&raw_line);

        let mut handle = make_live_handle();
        handle.inject_stderr_ring_for_test(vec![redacted]);

        let proj = handle
            .stderr_log_projection()
            .expect("projection must be Some");

        for line in &proj.lines {
            assert!(
                !line.contains(sentinel),
                "sentinel must not appear in projection line: {line}"
            );
            assert!(
                !line.contains("secret"),
                "any fragment of the sentinel path must not appear: {line}"
            );
        }
        // At least one line must contain the redaction marker.
        assert!(
            proj.lines.iter().any(|l| l.contains("[REDACTED]")),
            "at least one line must carry the [REDACTED] marker"
        );
    }

    #[test]
    fn t4_drain_handles_crlf_invalid_utf8_and_eof_partial_lines() {
        let input = b"first\r\ninvalid \xff byte\npartial\r";
        let ring = Arc::new(Mutex::new(VecDeque::new()));
        drain_stderr_reader(&input[..], ring.clone());

        assert_eq!(
            ring.lock()
                .unwrap()
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            vec!["first", "invalid � byte", "partial\r"]
        );
    }

    #[test]
    fn t4_drain_truncates_at_utf8_seam_without_panicking() {
        let mut input = vec![b'x'; STDERR_LINE_MAX_LEN - 1];
        input.extend_from_slice("€\n".as_bytes());
        let ring = Arc::new(Mutex::new(VecDeque::new()));
        drain_stderr_reader(&input[..], ring.clone());

        let lines = ring.lock().unwrap();
        assert_eq!(lines.len(), 1);
        assert!(lines[0].ends_with('…'));
        assert!(lines[0].len() <= STDERR_LINE_MAX_LEN);
    }

    struct UndelimitedReader {
        remaining: usize,
        max_requested: usize,
    }

    impl Read for UndelimitedReader {
        fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
            self.max_requested = self.max_requested.max(buffer.len());
            let count = self.remaining.min(buffer.len());
            buffer[..count].fill(b'x');
            self.remaining -= count;
            Ok(count)
        }
    }

    #[test]
    fn t4_drain_bounds_undelimited_input_and_retains_one_line() {
        let mut reader = UndelimitedReader {
            remaining: STDERR_LINE_MAX_LEN * 1024,
            max_requested: 0,
        };
        let ring = Arc::new(Mutex::new(VecDeque::new()));
        drain_stderr_reader(&mut reader, ring.clone());

        let lines = ring.lock().unwrap();
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].len(), STDERR_LINE_MAX_LEN);
        assert!(lines[0].ends_with('…'));
        assert!(reader.max_requested <= 4096);
    }

    #[test]
    fn t4_drain_preserves_redaction_and_ring_eviction() {
        let mut input = b"diagnostic /known/secret/workspace\n".to_vec();
        for index in 0..(STDERR_RING_CAPACITY + 1) {
            input.extend_from_slice(format!("line {index}\n").as_bytes());
        }
        let ring = Arc::new(Mutex::new(VecDeque::new()));
        drain_stderr_reader(&input[..], ring.clone());

        let lines = ring.lock().unwrap();
        assert_eq!(lines.len(), STDERR_RING_CAPACITY);
        assert!(!lines.iter().any(|line| line.contains("known")));
        assert_eq!(lines.front().map(String::as_str), Some("line 1"));
        assert_eq!(lines.back().map(String::as_str), Some("line 100"));
    }

    #[test]
    fn t4_drain_caps_redaction_expansion_at_utf8_boundary() {
        let input = "/a ".repeat(STDERR_LINE_MAX_LEN);
        let ring = Arc::new(Mutex::new(VecDeque::new()));
        drain_stderr_reader(input.as_bytes(), ring.clone());

        let lines = ring.lock().unwrap();
        assert_eq!(lines.len(), 1);
        assert!(lines[0].len() <= STDERR_LINE_MAX_LEN);
        assert!(std::str::from_utf8(lines[0].as_bytes()).is_ok());
        assert!(!lines[0].contains("/a"));
        assert!(lines[0].contains("[REDACTED]"));
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PKT-S3-WEDGE-R3: transport-death detection — TDD tests
//
// Before this packet, an RA process that crashed (stdout EOF) or whose one
// bad frame killed the client reader thread was INVISIBLE to the product
// session outside an in-flight request: the worker loop kept polling
// `try_drain_diagnostic_params()` (empty forever) and the session projected
// Live indefinitely — the product-facing twin of the GP-1 s3 wedge.
// ─────────────────────────────────────────────────────────────────────────────
#[cfg(test)]
mod transport_death_tests {
    use super::*;
    use legion_protocol::LspSessionLifecycleKind;

    /// Builds a RustAnalyzerSession around a child that is NOT an LSP server
    /// (`rustc --version`): it prints non-framed bytes and exits, so the
    /// stdout reader thread deterministically records a terminal event.
    fn session_on_non_lsp_child() -> RustAnalyzerSession {
        let identity = LspConfiguredServerIdentity {
            server_id: legion_protocol::LanguageServerId(1),
            workspace_id: WorkspaceId(1),
            root_id: Some(WorkspaceRootId(1)),
            language_id: LanguageId("rust".to_string()),
            display_name: "rustc-as-fake-ls".to_string(),
            command_hash: FileFingerprint {
                algorithm: "test".to_string(),
                value: "cmd:rustc".to_string(),
            },
            args_hash: None,
            env_hash: None,
            cwd_hash: None,
            settings_hash: None,
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        };
        let posture = LspWorkspaceTrustPosture {
            workspace_id: WorkspaceId(1),
            workspace_trust_state: WorkspaceTrustState::Trusted,
            privacy_scope: SemanticPrivacyScope::Workspace,
            privacy_scope_allowed: true,
            required_capability: CapabilityId("lsp.launch".to_string()),
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
                command: "rustc".to_string(),
                args: vec!["--version".to_string()],
                cwd: None,
                env: Vec::new(),
            },
            initial_backoff_ms: 25,
            max_backoff_ms: 100,
            max_restart_attempts: 1,
        };
        let config = RustAnalyzerLaunchConfig {
            discovery: RustAnalyzerDiscovery {
                configured_path: Some(PathBuf::from("rustc")),
                ..Default::default()
            },
            supervisor,
            server_id: legion_protocol::LanguageServerId(1),
            language_id: LanguageId("rust".to_string()),
        };
        let mut launcher = LspStdioLauncher::new();
        RustAnalyzerSession::launch(config, &mut launcher).expect("launch rustc as fake LS")
    }

    /// The worker loop must detect a terminal reader event during its idle
    /// poll, report it ONCE as `LspWorkerResult::TransportDead`, and exit.
    #[test]
    fn transport_death_worker_reports_and_exits() {
        let mut session = session_on_non_lsp_child();
        let (request_tx, request_rx) = mpsc::sync_channel::<LspWorkerRequest>(1);
        let (result_tx, result_rx) = mpsc::sync_channel::<LspWorkerResult>(16);

        let transport_dead = Arc::new(Mutex::new(None));
        let worker_transport_dead = Arc::clone(&transport_dead);
        let worker = thread::spawn(move || {
            run_session_worker(&mut session, request_rx, result_tx, worker_transport_dead);
        });

        // The worker must report transport death within a bounded window
        // (rustc exits immediately; the idle poll runs every 50 ms).
        let result = result_rx
            .recv_timeout(Duration::from_secs(10))
            .expect("worker must report transport death instead of polling forever");
        match result {
            LspWorkerResult::TransportDead { reason } => {
                assert!(
                    !reason.is_empty(),
                    "transport-death reason must describe the terminal event"
                );
            }
            LspWorkerResult::ReadResult { .. } => panic!("unexpected ReadResult"),
            LspWorkerResult::ApplyEditRequested { .. } => {
                panic!("unexpected ApplyEditRequested")
            }
            LspWorkerResult::DiagnosticBatch { .. } => panic!("unexpected DiagnosticBatch"),
        }

        // The worker thread must exit after reporting (no eternal idle loop).
        worker
            .join()
            .expect("worker thread must exit after transport death");
        assert!(
            transport_dead
                .lock()
                .expect("transport signal lock")
                .is_some(),
            "transport death must remain observable independently of result queue"
        );
        // Keep the request channel alive until after the join so the exit is
        // attributable to transport death, not channel disconnect.
        drop(request_tx);
    }

    /// The frame path must route `TransportDead` through the restart circuit
    /// breaker (BackingOff with a bounded reason), not leave the session Live
    /// and not hard-Fail while restart budget remains.
    #[test]
    fn transport_death_routes_through_circuit_breaker() {
        let mut handle = LspSessionHandle::new();
        let result_tx = handle.set_live_with_result_sender_for_test(unavailable_health_record());
        result_tx
            .send(LspWorkerResult::TransportDead {
                reason: "reader terminated: test-injected".to_string(),
            })
            .expect("inject transport death");

        let results = handle.try_drain_results();
        assert!(
            results.is_empty(),
            "TransportDead must be intercepted by the handle, not surfaced as a worker result"
        );

        let status = handle.session_status_projection();
        assert_eq!(
            status.lifecycle,
            LspSessionLifecycleKind::BackingOff,
            "transport death must enter the restart circuit breaker"
        );
        assert_eq!(status.restart_count, 1);
        let reason = status
            .failure_reason
            .expect("breaker entry must carry the transport-death reason");
        assert!(
            reason.contains("test-injected"),
            "reason must carry the terminal event description, got: {reason}"
        );
    }

    #[test]
    fn full_result_queue_cannot_drop_transport_death_lifecycle() {
        let mut handle = LspSessionHandle::new();
        let result_tx = handle.set_live_with_result_sender_for_test(unavailable_health_record());
        for _ in 0..16 {
            result_tx
                .try_send(LspWorkerResult::DiagnosticBatch {
                    raw_params: serde_json::json!({"diagnostics": []}),
                    captured_buffer_id: None,
                })
                .expect("test result queue should accept its bounded capacity");
        }
        handle.signal_transport_dead_for_test("reader terminated while result queue full");

        let results = handle.try_drain_results();
        assert_eq!(results.len(), 16, "queued diagnostics should still drain");
        let status = handle.session_status_projection();
        assert_eq!(status.lifecycle, LspSessionLifecycleKind::BackingOff);
        assert_eq!(status.restart_count, 1);
        assert!(
            status
                .failure_reason
                .expect("sideband transport reason")
                .contains("result queue full")
        );
    }

    #[test]
    fn push_diagnostics_queue_retries_latest_report_after_full_queue() {
        let (result_tx, result_rx) = mpsc::sync_channel(1);
        result_tx
            .send(LspWorkerResult::DiagnosticBatch {
                raw_params: serde_json::json!({"uri": "file:///tmp/other.ts"}),
                captured_buffer_id: None,
            })
            .expect("fill bounded result queue");
        let mut pending = HashMap::new();
        queue_latest_diagnostics(
            &result_tx,
            &mut pending,
            serde_json::json!({"uri": "file:///tmp/a.ts", "version": 1}),
            Some(BufferId(1)),
            32,
        );
        queue_latest_diagnostics(
            &result_tx,
            &mut pending,
            serde_json::json!({"uri": "file:///tmp/a.ts", "version": 2}),
            Some(BufferId(1)),
            32,
        );
        let normalized_a = crate::normalize_lsp_document_uri("file:///tmp/a.ts").unwrap();
        assert_eq!(pending[&normalized_a].0["version"], 2);
        let _ = result_rx.recv().expect("drain old result");
        flush_pending_diagnostics(&result_tx, &mut pending);
        let delivered = result_rx.recv().expect("deliver latest retry");
        match delivered {
            LspWorkerResult::DiagnosticBatch { raw_params, .. } => {
                assert_eq!(raw_params["version"], 2);
            }
            _ => panic!("expected diagnostic batch"),
        }
        assert!(pending.is_empty());
    }

    #[test]
    fn push_diagnostics_pending_slot_is_bounded_for_distinct_uris() {
        let (result_tx, _result_rx) = mpsc::sync_channel(1);
        result_tx
            .send(LspWorkerResult::DiagnosticBatch {
                raw_params: serde_json::json!({"uri": "file:///tmp/occupied.ts"}),
                captured_buffer_id: None,
            })
            .expect("fill bounded result queue");
        let mut pending = HashMap::new();
        for index in 0..32 {
            queue_latest_diagnostics(
                &result_tx,
                &mut pending,
                serde_json::json!({"uri": format!("file:///tmp/{index}.ts")}),
                Some(BufferId(1)),
                32,
            );
        }
        assert_eq!(pending.len(), 32);
        let normalized_last = crate::normalize_lsp_document_uri("file:///tmp/31.ts").unwrap();
        assert!(pending.contains_key(&normalized_last));
    }

    #[test]
    fn saturated_distinct_diagnostic_uri_applies_worker_backpressure() {
        let (result_tx, result_rx) = mpsc::sync_channel(1);
        result_tx
            .send(LspWorkerResult::DiagnosticBatch {
                raw_params: serde_json::json!({"uri": "file:///tmp/occupied.ts"}),
                captured_buffer_id: None,
            })
            .expect("fill bounded result queue");
        let mut pending = HashMap::new();
        for index in 0..32 {
            queue_latest_diagnostics(
                &result_tx,
                &mut pending,
                serde_json::json!({"uri": format!("file:///tmp/{index}.ts")}),
                Some(BufferId(1)),
                32,
            );
        }
        let worker_tx = result_tx.clone();
        let (completion_tx, completion_rx) = mpsc::sync_channel(1);
        let worker = thread::spawn(move || {
            let completed = queue_latest_diagnostics(
                &worker_tx,
                &mut pending,
                serde_json::json!({"uri": "file:///tmp/32.ts"}),
                Some(BufferId(1)),
                32,
            );
            completion_tx.send(completed).expect("report completion");
        });
        assert!(
            completion_rx
                .recv_timeout(Duration::from_millis(20))
                .is_err(),
            "saturated worker must remain blocked until the result queue drains"
        );
        let _ = result_rx.recv().expect("drain occupied result");
        let delivered = result_rx.recv().expect("backpressured result delivered");
        assert!(matches!(
            delivered,
            LspWorkerResult::DiagnosticBatch { raw_params, .. }
                if raw_params["uri"] == "file:///tmp/32.ts"
        ));
        assert!(completion_rx.recv().expect("worker send completes"));
        worker.join().expect("worker exits");

        let (result_tx, result_rx) = mpsc::sync_channel(1);
        result_tx
            .send(LspWorkerResult::DiagnosticBatch {
                raw_params: serde_json::json!({"uri": "file:///tmp/occupied.ts"}),
                captured_buffer_id: None,
            })
            .expect("fill bounded result queue");
        let mut pending = HashMap::new();
        for index in 0..32 {
            pending.insert(
                format!("file:///tmp/{index}.ts"),
                (serde_json::json!({}), None),
            );
        }
        let worker_tx = result_tx.clone();
        let (completion_tx, completion_rx) = mpsc::sync_channel(1);
        let worker = thread::spawn(move || {
            let completed = queue_latest_diagnostics(
                &worker_tx,
                &mut pending,
                serde_json::json!({"uri": "file:///tmp/disconnected.ts"}),
                Some(BufferId(1)),
                32,
            );
            completion_tx.send(completed).expect("report completion");
        });
        drop(result_rx);
        assert!(
            !completion_rx
                .recv_timeout(Duration::from_secs(1))
                .expect("blocked worker must be released by receiver drop")
        );
        worker.join().expect("worker exits after receiver drop");
    }

    #[test]
    fn direct_delivery_removes_pending_report_before_flush() {
        let (result_tx, result_rx) = mpsc::sync_channel(1);
        result_tx
            .send(LspWorkerResult::DiagnosticBatch {
                raw_params: serde_json::json!({"uri": "file:///tmp/occupied.ts"}),
                captured_buffer_id: None,
            })
            .expect("fill bounded result queue");
        let mut pending = HashMap::new();
        queue_latest_diagnostics(
            &result_tx,
            &mut pending,
            serde_json::json!({"uri": "file:///tmp/a.ts", "version": 1}),
            Some(BufferId(1)),
            32,
        );
        let _ = result_rx.recv().expect("drain occupied result");
        assert!(queue_latest_diagnostics(
            &result_tx,
            &mut pending,
            serde_json::json!({"uri": "file:///tmp/a.ts", "version": 2}),
            Some(BufferId(1)),
            32,
        ));
        flush_pending_diagnostics(&result_tx, &mut pending);
        let delivered = result_rx.recv().expect("receive direct latest report");
        assert!(matches!(
            delivered,
            LspWorkerResult::DiagnosticBatch { raw_params, .. }
                if raw_params["version"] == 2
        ));
        assert!(pending.is_empty());
        assert!(result_rx.try_recv().is_err(), "old pending report replayed");
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn diagnostic_queue_normalizes_localhost_uri_identity() {
        let (result_tx, _result_rx) = mpsc::sync_channel(1);
        result_tx
            .send(LspWorkerResult::DiagnosticBatch {
                raw_params: serde_json::json!({"uri": "file:///tmp/occupied.ts"}),
                captured_buffer_id: None,
            })
            .expect("fill bounded result queue");
        let mut pending = HashMap::new();
        queue_latest_diagnostics(
            &result_tx,
            &mut pending,
            serde_json::json!({"uri": "file://localhost/tmp/source%20file.ts"}),
            Some(BufferId(1)),
            32,
        );
        assert!(pending.contains_key("file:///tmp/source%20file.ts"));
    }
}
