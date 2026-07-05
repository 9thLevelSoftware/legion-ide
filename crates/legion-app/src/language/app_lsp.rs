//! Background LSP session lifecycle for `AppComposition` (WS-LANG-01 PKT-LSP-B T1).
//!
//! `LspSessionHandle` manages the startup/live/failed lifecycle of a
//! `RustAnalyzerSession` that runs on a background thread.  The design mirrors
//! `TerminalWorkflow::poll`: the frame path calls `drain()` via `try_recv` and
//! never blocks.

use std::{
    path::{Path, PathBuf},
    sync::mpsc,
    thread,
};

use legion_protocol::{LanguageId, LanguageServerId, LspResultStatus, LspServerHealthRecord};

use super::{
    LanguageSessionError, RustAnalyzerDiscovery, RustAnalyzerLaunchConfig, RustAnalyzerSession,
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

/// Internal lifecycle state.
enum LspSessionState {
    /// No startup attempted yet.
    Idle,
    /// Background thread has been spawned; waiting for the result.
    Starting { rx: mpsc::Receiver<LspStartResult> },
    /// Session is live and initialized.
    Live(RustAnalyzerSession),
    /// Launch was refused (untrusted, no binary, policy denied, etc.).
    Refused { reason: String },
    /// Session started but handshake or discovery failed.
    Failed { reason: String },
}

/// Manages the background startup and live-session lifecycle for one
/// `RustAnalyzerSession`.  All blocking work (discovery, process spawn, and
/// LSP `initialize` round-trip) happens on the background thread; the frame
/// path only calls `drain()` which is a non-blocking `try_recv`.
pub struct LspSessionHandle {
    state: LspSessionState,
    /// Workspace root passed at startup time, retained for diagnostics.
    pub workspace_root: Option<PathBuf>,
}

impl Default for LspSessionHandle {
    fn default() -> Self {
        Self::new()
    }
}

impl LspSessionHandle {
    /// Creates an idle handle.
    pub fn new() -> Self {
        Self {
            state: LspSessionState::Idle,
            workspace_root: None,
        }
    }

    /// Returns `true` if the handle is in the `Idle` state (no startup attempted).
    pub fn is_idle(&self) -> bool {
        matches!(self.state, LspSessionState::Idle)
    }

    /// Returns `true` if the session is live and initialized.
    pub fn is_live(&self) -> bool {
        matches!(self.state, LspSessionState::Live(_))
    }

    /// Returns `true` if startup was refused or the session failed.
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
            let result = startup_session(&root_path, &root_uri);
            // Ignore send failure (handle was dropped while starting).
            let _ = tx.send(result);
        });

        self.state = LspSessionState::Starting { rx };
    }

    /// Non-blocking drain — call once per frame tick.
    ///
    /// If Starting and a result is available, transitions to Live or Failed.
    /// Returns `true` when state changed (used to decide whether to refresh projection).
    pub fn drain(&mut self) -> bool {
        let LspSessionState::Starting { rx } = &self.state else {
            return false;
        };
        match rx.try_recv() {
            Ok(Ok(session)) => {
                self.state = LspSessionState::Live(session);
                true
            }
            Ok(Err(err)) => {
                self.state = LspSessionState::Failed {
                    reason: err.to_string(),
                };
                true
            }
            Err(mpsc::TryRecvError::Empty) => false,
            Err(mpsc::TryRecvError::Disconnected) => {
                self.state = LspSessionState::Failed {
                    reason: "startup thread disconnected without sending a result".to_string(),
                };
                true
            }
        }
    }

    /// Returns the current health record if the session is live (or a
    /// synthetic unavailable record if refused/failed).  Returns `None` when
    /// idle or starting.
    pub fn health_record(&self) -> Option<LspServerHealthRecord> {
        match &self.state {
            LspSessionState::Idle | LspSessionState::Starting { .. } => None,
            LspSessionState::Live(session) => Some(session.health().clone()),
            LspSessionState::Refused { .. } | LspSessionState::Failed { .. } => {
                Some(unavailable_health_record())
            }
        }
    }

    /// Returns a shared reference to the live session, or `None` otherwise.
    pub fn session(&self) -> Option<&RustAnalyzerSession> {
        match &self.state {
            LspSessionState::Live(session) => Some(session),
            _ => None,
        }
    }

    /// Returns an exclusive reference to the live session, or `None` otherwise.
    pub fn session_mut(&mut self) -> Option<&mut RustAnalyzerSession> {
        match &mut self.state {
            LspSessionState::Live(session) => Some(session),
            _ => None,
        }
    }

    /// Returns the human-readable reason for Refused or Failed states, or None.
    pub fn failure_reason(&self) -> Option<&str> {
        match &self.state {
            LspSessionState::Refused { reason } | LspSessionState::Failed { reason } => {
                Some(reason.as_str())
            }
            _ => None,
        }
    }
}

/// Runs full startup sequence: discovery → launch → initialize.
/// Called on the background thread only.
fn startup_session(
    workspace_root: &Path,
    root_uri: &str,
) -> Result<RustAnalyzerSession, LanguageSessionError> {
    let discovery = RustAnalyzerDiscovery {
        path_env: std::env::var("PATH").ok(),
        ..Default::default()
    };

    // Respect CARGO_BIN_EXE_mock_lsp_server for test environments.
    let resolved_discovery = if let Ok(mock_path) = std::env::var("CARGO_BIN_EXE_mock_lsp_server") {
        RustAnalyzerDiscovery {
            configured_path: Some(PathBuf::from(mock_path)),
            ..Default::default()
        }
    } else {
        discovery
    };

    let command = match resolved_discovery.resolve() {
        legion_lsp::DiscoveredBinary::Found { path, .. } => path.to_string_lossy().into_owned(),
        legion_lsp::DiscoveredBinary::NotFound => {
            return Err(LanguageSessionError::Discovery);
        }
    };

    let workspace_id = WorkspaceId(55);
    let server_id = LanguageServerId(101);
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
