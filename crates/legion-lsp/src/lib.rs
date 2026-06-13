//! Language-server runtime: JSON-RPC framing and supervised lifecycle scaffolding.

#![warn(missing_docs)]

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStderr, ChildStdin, ChildStdout, Command, Stdio};

use legion_protocol::{
    BufferVersion, EventId, EventSequence, FileFingerprint, FileId, LanguageCodeLensProjection,
    LanguageCompletionProjection, LanguageHoverProjection, LanguageId, LanguageInlayHintProjection,
    LanguageLocationProjection, LanguageOutlineSymbolProjection, LanguageProblemProjection,
    LspDiagnosticSummary, LspFormattingOptions, LspHealthState, LspLaunchDisposition,
    LspLaunchPolicyDecision, LspOperationContext, LspRequestId, LspRestartBackoffMetadata,
    LspResultStatus, LspSupervisionEvent, LspSupervisionEventKind, LspSupervisionLifecycleState,
    ProtocolDiagnosticSeverity, ProtocolTextRange, RedactionHint, SemanticFreshnessState,
    SemanticPrivacyScope, SnapshotId, TextCoordinate, Utf16Position, Utf16Range, WorkspaceId,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use thiserror::Error;
use uuid::Uuid;

/// Result type used by the LSP runtime crate.
pub type LspRuntimeResult<T> = Result<T, LspRuntimeError>;

/// LSP runtime errors.
#[derive(Debug, Error)]
pub enum LspRuntimeError {
    /// Process spawn failed before a usable language-server session existed.
    #[error("language-server spawn failed: {code}")]
    SpawnFailed {
        /// Metadata-only failure code.
        code: String,
    },
    /// JSON-RPC framing failed.
    #[error("malformed LSP frame: {message}")]
    MalformedFrame {
        /// Bounded diagnostic message.
        message: String,
    },
    /// JSON serialization or deserialization failed.
    #[error("LSP JSON serialization failed: {source}")]
    Json {
        /// serde_json source error.
        #[from]
        source: serde_json::Error,
    },
    /// A response arrived for an unknown or already-resolved JSON-RPC id.
    #[error("unknown LSP JSON-RPC response id {json_rpc_id}")]
    UnknownResponseId {
        /// JSON-RPC numeric identifier.
        json_rpc_id: u64,
    },
    /// A timeout was requested for an unknown or already-resolved request.
    #[error("unknown LSP request id {request_id:?}")]
    UnknownRequestId {
        /// Supervised LSP request identifier.
        request_id: LspRequestId,
    },
    /// The elapsed time has not exceeded the request timeout budget.
    #[error("LSP request {request_id:?} has not exceeded timeout budget")]
    TimeoutBudgetNotExceeded {
        /// Supervised LSP request identifier.
        request_id: LspRequestId,
    },
    /// The process-backed stdio session is not in a running state.
    #[error("LSP stdio session is not running")]
    SessionNotRunning,
    /// The child process exited unexpectedly before a usable stream was established.
    #[error("LSP stdio child process exited before becoming ready")]
    SessionSpawnedChildExited,
    /// I/O on the process-backed stdio session failed.
    #[error("LSP stdio I/O failed: {message}")]
    StdioIo {
        /// Bounded diagnostic message.
        message: String,
    },
    /// A framed payload from the server did not contain a valid JSON-RPC id.
    #[error("LSP stdio response missing id")]
    StdioResponseMissingId,
    /// The supervision policy denied launch before any process was spawned.
    /// Carries the supervision events observed during the policy
    /// evaluation so callers can assert that no raw source payloads
    /// leaked into the refusal event metadata.
    #[error("LSP stdio launch refused by supervision policy")]
    SupervisionRefused {
        /// Supervision events captured during the policy evaluation.
        events: Vec<LspSupervisionEvent>,
    },
}

/// Minimal JSON-RPC 2.0 envelope used by the LSP transport.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JsonRpcEnvelope {
    /// JSON-RPC protocol version.
    pub jsonrpc: String,
    /// Optional numeric id for requests and responses.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<u64>,
    /// Optional method for requests and notifications.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,
    /// Optional params payload.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
    /// Optional response result payload.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    /// Optional response error payload.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<Value>,
}

impl JsonRpcEnvelope {
    /// Builds a JSON-RPC request envelope.
    pub fn request(id: u64, method: impl Into<String>, params: Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id: Some(id),
            method: Some(method.into()),
            params: Some(params),
            result: None,
            error: None,
        }
    }

    /// Builds a JSON-RPC notification envelope.
    pub fn notification(method: impl Into<String>, params: Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id: None,
            method: Some(method.into()),
            params: Some(params),
            result: None,
            error: None,
        }
    }

    /// Builds a JSON-RPC response envelope.
    pub fn response(id: u64, result: Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id: Some(id),
            method: None,
            params: None,
            result: Some(result),
            error: None,
        }
    }

    /// Builds a JSON-RPC error response envelope.
    pub fn error_response(id: u64, code: i64, message: impl Into<String>) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id: Some(id),
            method: None,
            params: None,
            result: None,
            error: Some(json!({
                "code": code,
                "message": message.into(),
            })),
        }
    }
}

/// Content-Length based LSP frame encoder/decoder.
pub struct LspFramer;

impl LspFramer {
    /// Maximum payload size accepted by the one-frame decoder.
    pub const MAX_FRAME_PAYLOAD_BYTES: usize = 64 * 1024 * 1024;

    /// Encodes a JSON-RPC envelope into one LSP `Content-Length` frame.
    pub fn encode(envelope: &JsonRpcEnvelope) -> LspRuntimeResult<Vec<u8>> {
        let payload = serde_json::to_vec(envelope)?;
        let mut frame = format!("Content-Length: {}\r\n\r\n", payload.len()).into_bytes();
        frame.extend(payload);
        Ok(frame)
    }

    /// Decodes one complete LSP `Content-Length` frame into a JSON-RPC envelope.
    pub fn decode(frame: &[u8]) -> LspRuntimeResult<JsonRpcEnvelope> {
        let header_end = frame
            .windows(4)
            .position(|window| window == b"\r\n\r\n")
            .ok_or_else(|| LspRuntimeError::MalformedFrame {
                message: "missing header separator".to_string(),
            })?;
        let header = std::str::from_utf8(&frame[..header_end]).map_err(|err| {
            LspRuntimeError::MalformedFrame {
                message: format!("header was not UTF-8: {err}"),
            }
        })?;
        let length = header
            .lines()
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;
                name.eq_ignore_ascii_case("Content-Length").then_some(value)
            })
            .ok_or_else(|| LspRuntimeError::MalformedFrame {
                message: "missing Content-Length header".to_string(),
            })?
            .trim()
            .parse::<usize>()
            .map_err(|err| LspRuntimeError::MalformedFrame {
                message: format!("invalid Content-Length: {err}"),
            })?;
        if length > Self::MAX_FRAME_PAYLOAD_BYTES {
            return Err(LspRuntimeError::MalformedFrame {
                message: format!(
                    "Content-Length {length} exceeds max {}",
                    Self::MAX_FRAME_PAYLOAD_BYTES
                ),
            });
        }
        let payload_start = header_end + 4;
        let payload_end = payload_start.saturating_add(length);
        if frame.len() < payload_end {
            return Err(LspRuntimeError::MalformedFrame {
                message: "frame shorter than Content-Length".to_string(),
            });
        }
        Ok(serde_json::from_slice(&frame[payload_start..payload_end])?)
    }
}

/// Process launch configuration for one supervised language server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LspServerProcessConfig {
    /// Command to launch.
    pub command: String,
    /// Command arguments.
    pub args: Vec<String>,
    /// Optional working directory.
    pub cwd: Option<PathBuf>,
    /// Explicit environment entries to set.
    pub env: Vec<(String, String)>,
}

/// Binary-resolution metadata for one language-server adapter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LspServerBinarySource {
    /// Resolve the server from the system PATH.
    SystemPath {
        /// Binary name to look up in PATH.
        binary_name: String,
    },
    /// Resolve the server from a policy-gated downloaded artifact.
    DownloadedArtifact {
        /// Binary name used once the artifact is materialized.
        binary_name: String,
        /// Artifact URI or catalog entry.
        artifact_uri: String,
        /// Artifact checksum recorded by the supply-chain policy.
        checksum_sha256: String,
        /// Policy gate that authorizes the download path.
        policy_gate: String,
    },
}

/// One per-language adapter entry in the server registry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LanguageServerAdapterPlan {
    /// Stable language-server identifier.
    pub server_id: legion_protocol::LanguageServerId,
    /// Owning workspace.
    pub workspace_id: legion_protocol::WorkspaceId,
    /// Language served by this adapter.
    pub language_id: legion_protocol::LanguageId,
    /// Human-readable adapter name.
    pub display_name: String,
    /// Binary-resolution metadata.
    pub binary_source: LspServerBinarySource,
    /// Materialized process launch configuration.
    pub process: LspServerProcessConfig,
    /// Whether this adapter is the primary choice for the language.
    pub is_primary: bool,
}

impl LanguageServerAdapterPlan {
    /// Creates a system-path-backed adapter entry.
    pub fn system_path(
        server_id: legion_protocol::LanguageServerId,
        workspace_id: legion_protocol::WorkspaceId,
        language_id: legion_protocol::LanguageId,
        display_name: impl Into<String>,
        binary_name: impl Into<String>,
        args: Vec<String>,
        is_primary: bool,
    ) -> Self {
        let display_name = display_name.into();
        let binary_name = binary_name.into();
        Self {
            server_id,
            workspace_id,
            language_id,
            display_name,
            binary_source: LspServerBinarySource::SystemPath {
                binary_name: binary_name.clone(),
            },
            process: LspServerProcessConfig {
                command: binary_name,
                args,
                cwd: None,
                env: Vec::new(),
            },
            is_primary,
        }
    }

    /// Creates a policy-gated artifact-backed adapter entry.
    #[allow(clippy::too_many_arguments)]
    pub fn downloaded_artifact(
        server_id: legion_protocol::LanguageServerId,
        workspace_id: legion_protocol::WorkspaceId,
        language_id: legion_protocol::LanguageId,
        display_name: impl Into<String>,
        binary_name: impl Into<String>,
        artifact_uri: impl Into<String>,
        checksum_sha256: impl Into<String>,
        policy_gate: impl Into<String>,
        args: Vec<String>,
        is_primary: bool,
    ) -> Self {
        let display_name = display_name.into();
        let binary_name = binary_name.into();
        Self {
            server_id,
            workspace_id,
            language_id,
            display_name,
            binary_source: LspServerBinarySource::DownloadedArtifact {
                binary_name: binary_name.clone(),
                artifact_uri: artifact_uri.into(),
                checksum_sha256: checksum_sha256.into(),
                policy_gate: policy_gate.into(),
            },
            process: LspServerProcessConfig {
                command: binary_name,
                args,
                cwd: None,
                env: Vec::new(),
            },
            is_primary,
        }
    }

    /// Returns the materialized process configuration.
    pub fn process_config(&self) -> LspServerProcessConfig {
        self.process.clone()
    }
}

/// Binary-manifest entry describing a language-server adapter for one workspace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LspServerBinaryManifestEntry {
    /// Stable language-server identifier.
    pub server_id: legion_protocol::LanguageServerId,
    /// Owning workspace.
    pub workspace_id: legion_protocol::WorkspaceId,
    /// Language served by this adapter.
    pub language_id: legion_protocol::LanguageId,
    /// Human-readable adapter name.
    pub display_name: String,
    /// Binary-resolution metadata.
    pub binary_source: LspServerBinarySource,
    /// Optional workspace-level version pin recorded for the manifest audit.
    pub workspace_version_pin: Option<String>,
    /// Whether this adapter is the primary choice for the language.
    pub is_primary: bool,
}

/// Metadata-only manifest audit for the server-binary supply chain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LspServerBinaryManifest {
    /// Workspace covered by the manifest audit.
    pub workspace_id: legion_protocol::WorkspaceId,
    /// Language covered by the manifest audit.
    pub language_id: legion_protocol::LanguageId,
    /// Whether the audit ran under air-gap policy.
    pub air_gap: bool,
    /// Download attempts denied by the policy gate.
    pub denied_downloads: Vec<String>,
    /// Adapters retained for this workspace/language pair.
    pub entries: Vec<LspServerBinaryManifestEntry>,
}

/// Registry of per-language adapter plans.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LanguageServerAdapterRegistry {
    adapters_by_language: HashMap<legion_protocol::LanguageId, Vec<LanguageServerAdapterPlan>>,
}

impl LanguageServerAdapterRegistry {
    /// Creates an empty adapter registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers one adapter entry.
    pub fn register(&mut self, adapter: LanguageServerAdapterPlan) {
        let language_id = adapter.language_id.clone();
        let adapters = self.adapters_by_language.entry(language_id).or_default();
        adapters.push(adapter);
        adapters.sort_by_key(|entry| {
            (
                std::cmp::Reverse(entry.is_primary),
                entry.display_name.clone(),
                entry.server_id.0,
            )
        });
    }

    /// Returns all adapters for one language in registry order.
    pub fn adapters_for_language(
        &self,
        language_id: &legion_protocol::LanguageId,
    ) -> Vec<&LanguageServerAdapterPlan> {
        self.adapters_by_language
            .get(language_id)
            .map(|adapters| adapters.iter().collect())
            .unwrap_or_default()
    }

    /// Returns the launch configs for one workspace/language pair.
    pub fn process_configs_for_workspace_language(
        &self,
        workspace_id: legion_protocol::WorkspaceId,
        language_id: &legion_protocol::LanguageId,
    ) -> Vec<LspServerProcessConfig> {
        self.adapters_for_language(language_id)
            .into_iter()
            .filter(|adapter| adapter.workspace_id == workspace_id)
            .map(LanguageServerAdapterPlan::process_config)
            .collect()
    }

    /// Returns a metadata-only manifest audit for one workspace/language pair.
    pub fn binary_manifest_for_workspace_language(
        &self,
        workspace_id: legion_protocol::WorkspaceId,
        language_id: &legion_protocol::LanguageId,
        air_gap: bool,
    ) -> LspServerBinaryManifest {
        let mut denied_downloads = Vec::new();
        let entries = self
            .adapters_for_language(language_id)
            .into_iter()
            .filter(|adapter| adapter.workspace_id == workspace_id)
            .filter_map(|adapter| {
                let workspace_version_pin = Some(format!("workspace/{}", workspace_id.0));
                match &adapter.binary_source {
                    LspServerBinarySource::SystemPath { .. } => {
                        Some(LspServerBinaryManifestEntry {
                            server_id: adapter.server_id,
                            workspace_id: adapter.workspace_id,
                            language_id: adapter.language_id.clone(),
                            display_name: adapter.display_name.clone(),
                            binary_source: adapter.binary_source.clone(),
                            workspace_version_pin: None,
                            is_primary: adapter.is_primary,
                        })
                    }
                    LspServerBinarySource::DownloadedArtifact {
                        binary_name,
                        artifact_uri,
                        checksum_sha256,
                        policy_gate,
                    } => {
                        if air_gap {
                            denied_downloads.push(format!(
                                "{}:{} denied by {} ({})",
                                adapter.display_name, artifact_uri, policy_gate, checksum_sha256
                            ));
                            None
                        } else {
                            Some(LspServerBinaryManifestEntry {
                                server_id: adapter.server_id,
                                workspace_id: adapter.workspace_id,
                                language_id: adapter.language_id.clone(),
                                display_name: adapter.display_name.clone(),
                                binary_source: LspServerBinarySource::DownloadedArtifact {
                                    binary_name: binary_name.clone(),
                                    artifact_uri: artifact_uri.clone(),
                                    checksum_sha256: checksum_sha256.clone(),
                                    policy_gate: policy_gate.clone(),
                                },
                                workspace_version_pin,
                                is_primary: adapter.is_primary,
                            })
                        }
                    }
                }
            })
            .collect();

        LspServerBinaryManifest {
            workspace_id,
            language_id: language_id.clone(),
            air_gap,
            denied_downloads,
            entries,
        }
    }

    /// Returns the stable tier-2 adapter registry used by the smoke tests.
    pub fn tier_two() -> Self {
        let workspace_id = legion_protocol::WorkspaceId(1);
        let mut registry = Self::new();
        registry.register(LanguageServerAdapterPlan::system_path(
            legion_protocol::LanguageServerId(101),
            workspace_id,
            legion_protocol::LanguageId("rust".to_string()),
            "rust-analyzer",
            "rust-analyzer",
            Vec::new(),
            true,
        ));
        registry.register(LanguageServerAdapterPlan::system_path(
            legion_protocol::LanguageServerId(102),
            workspace_id,
            legion_protocol::LanguageId("typescript".to_string()),
            "typescript-language-server",
            "typescript-language-server",
            vec!["--stdio".to_string()],
            true,
        ));
        registry.register(LanguageServerAdapterPlan::system_path(
            legion_protocol::LanguageServerId(103),
            workspace_id,
            legion_protocol::LanguageId("typescript".to_string()),
            "tailwindcss-language-server",
            "tailwindcss-language-server",
            vec!["--stdio".to_string()],
            false,
        ));
        registry.register(LanguageServerAdapterPlan::downloaded_artifact(
            legion_protocol::LanguageServerId(104),
            workspace_id,
            legion_protocol::LanguageId("python".to_string()),
            "pyright",
            "pyright-langserver",
            "https://registry.example.invalid/pyright-1.1.400.tgz",
            "sha256:pyright-1.1.400",
            "policy://lsp-download/pyright",
            vec!["--stdio".to_string()],
            true,
        ));
        registry.register(LanguageServerAdapterPlan::system_path(
            legion_protocol::LanguageServerId(105),
            workspace_id,
            legion_protocol::LanguageId("go".to_string()),
            "gopls",
            "gopls",
            Vec::new(),
            true,
        ));
        registry
    }
}

/// Abstract handle for a launched language-server process.
pub trait LspProcessHandle: Send {
    /// Returns whether the process is still running.
    fn is_running(&mut self) -> bool;

    /// Terminates the process boundary.
    fn kill(&mut self);
}

/// Abstract launcher used by supervision tests and future platform-backed runtimes.
pub trait LspProcessLauncher {
    /// Spawns a language-server process and returns a supervised handle.
    fn spawn(
        &mut self,
        config: &LspServerProcessConfig,
    ) -> LspRuntimeResult<Box<dyn LspProcessHandle>>;
}

/// LSP supervisor configuration.
#[derive(Debug, Clone)]
pub struct LspSupervisorConfig {
    /// Launch policy produced by the protocol-layer trust/capability gate.
    pub launch_policy: LspLaunchPolicyDecision,
    /// Process configuration for the server if launch is allowed.
    pub process: LspServerProcessConfig,
    /// Initial restart backoff in milliseconds.
    pub initial_backoff_ms: u64,
    /// Maximum restart backoff in milliseconds.
    pub max_backoff_ms: u64,
    /// Maximum restart attempts before opening the circuit.
    pub max_restart_attempts: u32,
}

/// Supervised lifecycle owner for one language-server process boundary.
pub struct LspSupervisor {
    config: LspSupervisorConfig,
    lifecycle_state: LspSupervisionLifecycleState,
    health_state: LspHealthState,
    restart_attempts: u32,
    sequence: u64,
    process: Option<Box<dyn LspProcessHandle>>,
}

impl LspSupervisor {
    /// Creates a supervisor in the configured state.
    pub fn new(config: LspSupervisorConfig) -> Self {
        Self {
            config,
            lifecycle_state: LspSupervisionLifecycleState::Configured,
            health_state: LspHealthState::Unknown,
            restart_attempts: 0,
            sequence: 0,
            process: None,
        }
    }

    /// Current lifecycle state.
    pub fn lifecycle_state(&self) -> LspSupervisionLifecycleState {
        self.lifecycle_state
    }

    /// Current health state.
    pub fn health_state(&self) -> LspHealthState {
        self.health_state
    }

    /// Ensures the server is running or emits fail-closed supervision events.
    pub fn ensure_started(
        &mut self,
        launcher: &mut impl LspProcessLauncher,
    ) -> Vec<LspSupervisionEvent> {
        if !self.config.launch_policy.process_launch_allowed {
            self.lifecycle_state = match self.config.launch_policy.disposition {
                LspLaunchDisposition::RuntimeActivationDeferred => {
                    LspSupervisionLifecycleState::LaunchDeferred
                }
                _ => LspSupervisionLifecycleState::Disabled,
            };
            self.health_state = LspHealthState::Unavailable;
            return vec![self.event(LspSupervisionEventKind::LaunchRefused, None, None)];
        }

        if self.restart_attempts >= self.config.max_restart_attempts {
            self.lifecycle_state = LspSupervisionLifecycleState::CircuitOpen;
            self.health_state = LspHealthState::Unavailable;
            return vec![self.event(
                LspSupervisionEventKind::RestartBackoffUpdated,
                Some(self.restart_backoff(true, None)),
                None,
            )];
        }

        if let Some(process) = self.process.as_mut()
            && process.is_running()
        {
            self.lifecycle_state = LspSupervisionLifecycleState::Running;
            self.health_state = LspHealthState::Healthy;
            return Vec::new();
        }

        self.lifecycle_state = LspSupervisionLifecycleState::Starting;
        match launcher.spawn(&self.config.process) {
            Ok(process) => {
                self.process = Some(process);
                self.lifecycle_state = LspSupervisionLifecycleState::Running;
                self.health_state = LspHealthState::Healthy;
                self.restart_attempts = 0;
                vec![self.event(LspSupervisionEventKind::LifecycleChanged, None, None)]
            }
            Err(LspRuntimeError::SpawnFailed { code }) => {
                self.restart_attempts = self.restart_attempts.saturating_add(1);
                self.lifecycle_state = LspSupervisionLifecycleState::Failed;
                self.health_state = LspHealthState::Unavailable;
                vec![self.event(
                    LspSupervisionEventKind::RestartBackoffUpdated,
                    Some(self.restart_backoff(false, Some(code))),
                    None,
                )]
            }
            Err(err) => {
                self.restart_attempts = self.restart_attempts.saturating_add(1);
                self.lifecycle_state = LspSupervisionLifecycleState::Failed;
                self.health_state = LspHealthState::Unavailable;
                vec![self.event(
                    LspSupervisionEventKind::RestartBackoffUpdated,
                    Some(self.restart_backoff(false, Some(err.to_string()))),
                    None,
                )]
            }
        }
    }

    fn event(
        &mut self,
        kind: LspSupervisionEventKind,
        restart_backoff: Option<LspRestartBackoffMetadata>,
        diagnostics: Option<Vec<legion_protocol::ProtocolDiagnostic>>,
    ) -> LspSupervisionEvent {
        self.sequence = self.sequence.saturating_add(1);
        LspSupervisionEvent {
            event_id: EventId(Uuid::from_u128(0x1000 + u128::from(self.sequence))),
            sequence: EventSequence(self.sequence),
            kind,
            identity: self.config.launch_policy.identity.clone(),
            lifecycle_state: self.lifecycle_state,
            health_state: self.health_state,
            request: None,
            restart_backoff,
            capabilities: Vec::new(),
            diagnostic_summaries: Vec::new(),
            diagnostics: diagnostics.unwrap_or_default(),
            correlation_id: self.config.launch_policy.correlation_id,
            causality_id: self.config.launch_policy.causality_id,
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        }
    }

    fn restart_backoff(
        &self,
        circuit_breaker_open: bool,
        last_failure_code: Option<String>,
    ) -> LspRestartBackoffMetadata {
        let multiplier = if self.restart_attempts == 0 {
            1
        } else {
            1u64 << (self.restart_attempts.saturating_sub(1).min(63))
        };
        let next_backoff_ms = self
            .config
            .initial_backoff_ms
            .saturating_mul(multiplier)
            .min(self.config.max_backoff_ms);
        let last_failure_hash = last_failure_code.as_ref().map(|code| FileFingerprint {
            algorithm: "metadata-hash".to_string(),
            value: format!("failure-code:{}", stable_hash(code)),
        });
        LspRestartBackoffMetadata {
            restart_attempts: self.restart_attempts,
            max_restart_attempts: self.config.max_restart_attempts,
            next_backoff_ms,
            circuit_breaker_open,
            last_failure_code,
            last_failure_hash,
            schema_version: 1,
        }
    }
}

impl Drop for LspSupervisor {
    fn drop(&mut self) {
        if let Some(process) = self.process.as_mut() {
            process.kill();
        }
    }
}

/// Pending request metadata returned after preparing a JSON-RPC request.
#[derive(Debug, Clone)]
pub struct LspPendingRequest {
    /// Supervised request identifier.
    pub request_id: LspRequestId,
    /// Full operation context retained for later request-correlation event projection.
    pub context: LspOperationContext,
    /// Numeric JSON-RPC identifier.
    pub json_rpc_id: u64,
    /// Request method.
    pub method: String,
    /// Timeout budget in milliseconds.
    pub timeout_ms: u64,
    /// Encoded JSON-RPC envelope.
    pub envelope: JsonRpcEnvelope,
}

/// Correlated response metadata after matching a JSON-RPC response to an in-flight request.
#[derive(Debug, Clone)]
pub struct LspCorrelatedResponse {
    /// Supervised request identifier.
    pub request_id: LspRequestId,
    /// Full operation context retained from request preparation.
    pub context: LspOperationContext,
    /// Result freshness/status.
    pub status: LspResultStatus,
    /// Response result payload.
    pub result: Value,
    /// Optional JSON-RPC error payload.
    pub error: Option<Value>,
}

/// Cancellation metadata produced when a pending request is cancelled.
#[derive(Debug, Clone)]
pub struct LspCancelledRequest {
    /// Supervised request identifier.
    pub request_id: LspRequestId,
    /// Full operation context retained from request preparation.
    pub context: LspOperationContext,
    /// JSON-RPC id passed to the LSP `$/cancelRequest` notification.
    pub json_rpc_id: u64,
    /// Framed notification envelope to send to the language server.
    pub notification: JsonRpcEnvelope,
    /// Cancellation status returned to local callers.
    pub response: LspCorrelatedResponse,
}

/// Synchronous correlation state for JSON-RPC requests.
#[derive(Debug, Default)]
pub struct LspClient {
    next_json_rpc_id: u64,
    pending_by_json_rpc_id: HashMap<u64, LspPendingRequest>,
    json_rpc_id_by_request_id: HashMap<LspRequestId, u64>,
}

impl LspClient {
    /// Creates an empty client correlation state.
    pub fn new() -> Self {
        Self {
            next_json_rpc_id: 1,
            pending_by_json_rpc_id: HashMap::new(),
            json_rpc_id_by_request_id: HashMap::new(),
        }
    }

    /// Prepares a request and records its correlation metadata.
    pub fn prepare_request(
        &mut self,
        method: impl Into<String>,
        params: Value,
        context: LspOperationContext,
    ) -> LspPendingRequest {
        let json_rpc_id = self.next_json_rpc_id;
        self.next_json_rpc_id = self.next_json_rpc_id.saturating_add(1);
        let method = method.into();
        let envelope = JsonRpcEnvelope::request(json_rpc_id, method.clone(), params);
        let pending = LspPendingRequest {
            request_id: context.request_id,
            context: context.clone(),
            json_rpc_id,
            method,
            timeout_ms: context.timeout_ms,
            envelope,
        };
        self.pending_by_json_rpc_id
            .insert(json_rpc_id, pending.clone());
        self.json_rpc_id_by_request_id
            .insert(context.request_id, json_rpc_id);
        pending
    }

    /// Correlates a response envelope back to the supervised request that produced it.
    pub fn correlate_response(
        &mut self,
        response: JsonRpcEnvelope,
    ) -> LspRuntimeResult<LspCorrelatedResponse> {
        let json_rpc_id = response.id.ok_or_else(|| LspRuntimeError::MalformedFrame {
            message: "response missing id".to_string(),
        })?;
        let pending = self
            .pending_by_json_rpc_id
            .remove(&json_rpc_id)
            .ok_or(LspRuntimeError::UnknownResponseId { json_rpc_id })?;
        self.json_rpc_id_by_request_id.remove(&pending.request_id);
        let error = response.error;
        let status = if error.is_some() {
            LspResultStatus::Unavailable
        } else {
            LspResultStatus::Fresh
        };
        Ok(LspCorrelatedResponse {
            request_id: pending.request_id,
            context: pending.context,
            status,
            result: response.result.unwrap_or(Value::Null),
            error,
        })
    }

    /// Resolves a pending request as timed out after its timeout budget is exceeded.
    pub fn resolve_timeout(
        &mut self,
        request_id: LspRequestId,
        elapsed_ms: u64,
    ) -> LspRuntimeResult<LspCorrelatedResponse> {
        let json_rpc_id = *self
            .json_rpc_id_by_request_id
            .get(&request_id)
            .ok_or(LspRuntimeError::UnknownRequestId { request_id })?;
        let pending = self
            .pending_by_json_rpc_id
            .get(&json_rpc_id)
            .ok_or(LspRuntimeError::UnknownRequestId { request_id })?;
        if elapsed_ms <= pending.timeout_ms {
            return Err(LspRuntimeError::TimeoutBudgetNotExceeded { request_id });
        }
        let pending = self
            .pending_by_json_rpc_id
            .remove(&json_rpc_id)
            .expect("pending request existed after timeout lookup");
        self.json_rpc_id_by_request_id.remove(&request_id);
        Ok(LspCorrelatedResponse {
            request_id: pending.request_id,
            context: pending.context,
            status: LspResultStatus::Timeout,
            result: Value::Null,
            error: None,
        })
    }

    /// Cancels a pending request and returns the `$/cancelRequest` notification.
    pub fn cancel_request(
        &mut self,
        request_id: LspRequestId,
    ) -> LspRuntimeResult<LspCancelledRequest> {
        let json_rpc_id = *self
            .json_rpc_id_by_request_id
            .get(&request_id)
            .ok_or(LspRuntimeError::UnknownRequestId { request_id })?;
        let pending = self
            .pending_by_json_rpc_id
            .remove(&json_rpc_id)
            .ok_or(LspRuntimeError::UnknownRequestId { request_id })?;
        self.json_rpc_id_by_request_id.remove(&request_id);
        let response = LspCorrelatedResponse {
            request_id: pending.request_id,
            context: pending.context,
            status: LspResultStatus::Cancelled,
            result: Value::Null,
            error: None,
        };
        Ok(LspCancelledRequest {
            request_id,
            context: response.context.clone(),
            json_rpc_id,
            notification: JsonRpcEnvelope::notification(
                "$/cancelRequest",
                json!({"id": json_rpc_id}),
            ),
            response,
        })
    }
}

fn stable_hash(input: &str) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in input.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

// -----------------------------------------------------------------------------
// Document synchronization + diagnostic projection foundation (WS03.T2).
// -----------------------------------------------------------------------------

/// Metadata that identifies one text document for LSP synchronization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LspTextDocumentIdentity {
    /// LSP document URI.
    pub uri: String,
    /// Language identifier advertised to the server.
    pub language_id: LanguageId,
    /// Workspace scope.
    pub workspace_id: WorkspaceId,
    /// File scope.
    pub file_id: FileId,
    /// Snapshot represented by the synchronized text.
    pub snapshot_id: SnapshotId,
    /// Monotonic buffer version sent to the server.
    pub buffer_version: BufferVersion,
    /// Optional content hash for freshness checks.
    pub content_hash: Option<FileFingerprint>,
}

/// One LSP `textDocument/didChange` content-change entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LspTextDocumentChange {
    /// Optional UTF-16 range; absent for full-document changes.
    pub range: Option<Utf16Range>,
    /// Replacement text sent to the language server.
    pub text: String,
}

impl LspTextDocumentChange {
    /// Creates an incremental ranged change using LSP UTF-16 coordinates.
    pub fn ranged(range: Utf16Range, text: impl Into<String>) -> Self {
        Self {
            range: Some(range),
            text: text.into(),
        }
    }

    /// Creates a full-document replacement change.
    pub fn full_document(text: impl Into<String>) -> Self {
        Self {
            range: None,
            text: text.into(),
        }
    }
}

/// Builds a `textDocument/didOpen` notification envelope.
pub fn did_open_notification(document: &LspTextDocumentIdentity, text: &str) -> JsonRpcEnvelope {
    JsonRpcEnvelope::notification(
        "textDocument/didOpen",
        json!({
            "textDocument": {
                "uri": document.uri,
                "languageId": document.language_id.0,
                "version": document.buffer_version.0,
                "text": text,
            }
        }),
    )
}

/// Builds a `textDocument/didChange` notification envelope.
pub fn did_change_notification(
    document: &LspTextDocumentIdentity,
    changes: Vec<LspTextDocumentChange>,
) -> JsonRpcEnvelope {
    let content_changes = changes
        .into_iter()
        .map(|change| match change.range {
            Some(range) => json!({
                "range": utf16_range_to_lsp_json(range),
                "text": change.text,
            }),
            None => json!({"text": change.text}),
        })
        .collect::<Vec<_>>();
    JsonRpcEnvelope::notification(
        "textDocument/didChange",
        json!({
            "textDocument": {
                "uri": document.uri,
                "version": document.buffer_version.0,
            },
            "contentChanges": content_changes,
        }),
    )
}

/// Builds a JSON-RPC `textDocument/completion` request.
pub fn completion_request(
    id: u64,
    document: &LspTextDocumentIdentity,
    position: Utf16Position,
) -> JsonRpcEnvelope {
    JsonRpcEnvelope::request(
        id,
        "textDocument/completion",
        json!({
            "textDocument": {"uri": document.uri},
            "position": {"line": position.line, "character": position.character},
        }),
    )
}

/// Converts an LSP completion response payload into existing completion projections.
pub fn project_completion_response(
    response: &Value,
    limit: usize,
) -> Vec<LanguageCompletionProjection> {
    let Some(items) = completion_items(response) else {
        return Vec::new();
    };
    items
        .iter()
        .take(limit)
        .enumerate()
        .filter_map(|(index, item)| completion_projection_for_item(index, item))
        .collect()
}

fn completion_items(response: &Value) -> Option<&[Value]> {
    if let Some(items) = response.as_array() {
        return Some(items.as_slice());
    }
    response.get("items")?.as_array().map(Vec::as_slice)
}

fn completion_projection_for_item(
    index: usize,
    item: &Value,
) -> Option<LanguageCompletionProjection> {
    let label = item.get("label")?.as_str()?;
    let detail = item
        .get("detail")
        .and_then(Value::as_str)
        .map(|detail| bounded_lsp_label(detail, 160));
    let kind_label = item
        .get("kind")
        .and_then(Value::as_u64)
        .map(|kind| format!("lsp.completion.kind.{kind}"))
        .unwrap_or_else(|| "lsp.completion.kind.unknown".to_string());
    let label = bounded_lsp_label(label, 120);
    Some(LanguageCompletionProjection {
        completion_id: format!("lsp-completion-{index}-{:016x}", stable_hash(&label)),
        label,
        detail_label: detail,
        kind_label,
        score_basis_points: 10_000u16.saturating_sub((index as u16).saturating_mul(100)),
        degraded: item.get("insertText").is_none(),
        schema_version: 1,
    })
}

fn bounded_lsp_label(label: &str, max_bytes: usize) -> String {
    if label.len() <= max_bytes {
        return label.to_string();
    }
    let mut end = max_bytes.saturating_sub("…".len());
    while !label.is_char_boundary(end) && end > 0 {
        end -= 1;
    }
    format!("{}…", &label[..end])
}

/// Builds a JSON-RPC `textDocument/hover` request.
pub fn hover_request(
    id: u64,
    document: &LspTextDocumentIdentity,
    position: Utf16Position,
) -> JsonRpcEnvelope {
    JsonRpcEnvelope::request(
        id,
        "textDocument/hover",
        json!({
            "textDocument": {"uri": document.uri},
            "position": {"line": position.line, "character": position.character},
        }),
    )
}

/// Converts an LSP hover response payload into an existing hover projection row.
pub fn project_hover_response(
    response: &Value,
    file_id: Option<FileId>,
) -> Option<LanguageHoverProjection> {
    if response.is_null() {
        return None;
    }
    let contents = response.get("contents").unwrap_or(response);
    let summary = hover_contents_label(contents)?;
    let label = summary
        .lines()
        .next()
        .map(|line| bounded_lsp_label(line, 120))
        .unwrap_or_else(|| "lsp hover".to_string());
    let range = response.get("range").and_then(protocol_range_from_lsp_json);
    Some(LanguageHoverProjection {
        hover_id: format!("lsp-hover-{:016x}", stable_hash(&summary)),
        file_id,
        range,
        label,
        summary: bounded_lsp_label(&summary, 320),
        degraded: response.get("range").is_none(),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    })
}

fn hover_contents_label(contents: &Value) -> Option<String> {
    if let Some(value) = contents.as_str() {
        return Some(value.to_string());
    }
    if let Some(value) = contents.get("value").and_then(Value::as_str) {
        return Some(value.to_string());
    }
    if let Some(array) = contents.as_array() {
        let joined = array
            .iter()
            .filter_map(hover_contents_label)
            .collect::<Vec<_>>()
            .join("\n");
        if !joined.is_empty() {
            return Some(joined);
        }
    }
    None
}

/// Builds a JSON-RPC `textDocument/definition` request.
pub fn definition_request(
    id: u64,
    document: &LspTextDocumentIdentity,
    position: Utf16Position,
) -> JsonRpcEnvelope {
    JsonRpcEnvelope::request(
        id,
        "textDocument/definition",
        json!({
            "textDocument": {"uri": document.uri},
            "position": {"line": position.line, "character": position.character},
        }),
    )
}

/// Builds a JSON-RPC `textDocument/prepareRename` request.
pub fn prepare_rename_request(
    id: u64,
    document: &LspTextDocumentIdentity,
    position: Utf16Position,
) -> JsonRpcEnvelope {
    JsonRpcEnvelope::request(
        id,
        "textDocument/prepareRename",
        json!({
            "textDocument": {"uri": document.uri},
            "position": {"line": position.line, "character": position.character},
        }),
    )
}

/// Builds a JSON-RPC `textDocument/rename` request.
pub fn rename_request(
    id: u64,
    document: &LspTextDocumentIdentity,
    position: Utf16Position,
    new_name: impl Into<String>,
) -> JsonRpcEnvelope {
    JsonRpcEnvelope::request(
        id,
        "textDocument/rename",
        json!({
            "textDocument": {"uri": document.uri},
            "position": {"line": position.line, "character": position.character},
            "newName": new_name.into(),
        }),
    )
}

/// Builds a JSON-RPC `textDocument/declaration` request.
pub fn declaration_request(
    id: u64,
    document: &LspTextDocumentIdentity,
    position: Utf16Position,
) -> JsonRpcEnvelope {
    JsonRpcEnvelope::request(
        id,
        "textDocument/declaration",
        json!({
            "textDocument": {"uri": document.uri},
            "position": {"line": position.line, "character": position.character},
        }),
    )
}

/// Builds a JSON-RPC `textDocument/implementation` request.
pub fn implementation_request(
    id: u64,
    document: &LspTextDocumentIdentity,
    position: Utf16Position,
) -> JsonRpcEnvelope {
    JsonRpcEnvelope::request(
        id,
        "textDocument/implementation",
        json!({
            "textDocument": {"uri": document.uri},
            "position": {"line": position.line, "character": position.character},
        }),
    )
}

/// Builds a JSON-RPC `textDocument/typeDefinition` request.
pub fn type_definition_request(
    id: u64,
    document: &LspTextDocumentIdentity,
    position: Utf16Position,
) -> JsonRpcEnvelope {
    JsonRpcEnvelope::request(
        id,
        "textDocument/typeDefinition",
        json!({
            "textDocument": {"uri": document.uri},
            "position": {"line": position.line, "character": position.character},
        }),
    )
}

/// Builds a JSON-RPC `textDocument/formatting` request.
pub fn formatting_request(
    id: u64,
    document: &LspTextDocumentIdentity,
    options: &LspFormattingOptions,
) -> JsonRpcEnvelope {
    JsonRpcEnvelope::request(
        id,
        "textDocument/formatting",
        json!({
            "textDocument": {"uri": document.uri},
            "options": options,
        }),
    )
}

/// Builds a JSON-RPC `textDocument/rangeFormatting` request.
pub fn range_formatting_request(
    id: u64,
    document: &LspTextDocumentIdentity,
    range: Utf16Range,
    options: &LspFormattingOptions,
) -> JsonRpcEnvelope {
    JsonRpcEnvelope::request(
        id,
        "textDocument/rangeFormatting",
        json!({
            "textDocument": {"uri": document.uri},
            "range": utf16_range_to_lsp_json(range),
            "options": options,
        }),
    )
}

/// Builds a JSON-RPC `textDocument/references` request.
pub fn references_request(
    id: u64,
    document: &LspTextDocumentIdentity,
    position: Utf16Position,
    include_declaration: bool,
) -> JsonRpcEnvelope {
    JsonRpcEnvelope::request(
        id,
        "textDocument/references",
        json!({
            "textDocument": {"uri": document.uri},
            "position": {"line": position.line, "character": position.character},
            "context": {"includeDeclaration": include_declaration},
        }),
    )
}

/// Builds a JSON-RPC `textDocument/codeAction` request.
pub fn code_action_request(
    id: u64,
    document: &LspTextDocumentIdentity,
    range: Utf16Range,
    diagnostics: Vec<Value>,
    only: Option<Vec<String>>,
) -> JsonRpcEnvelope {
    let mut context = serde_json::Map::new();
    context.insert("diagnostics".to_string(), Value::Array(diagnostics));
    if let Some(only) = only.filter(|only| !only.is_empty()) {
        context.insert("only".to_string(), json!(only));
    }
    JsonRpcEnvelope::request(
        id,
        "textDocument/codeAction",
        json!({
            "textDocument": {"uri": document.uri},
            "range": utf16_range_to_lsp_json(range),
            "context": Value::Object(context),
        }),
    )
}

/// Builds a JSON-RPC `textDocument/codeAction` request for organize-imports actions.
pub fn organize_imports_request(
    id: u64,
    document: &LspTextDocumentIdentity,
    range: Utf16Range,
    diagnostics: Vec<Value>,
) -> JsonRpcEnvelope {
    code_action_request(
        id,
        document,
        range,
        diagnostics,
        Some(vec!["source.organizeImports".to_string()]),
    )
}

/// Builds a JSON-RPC `textDocument/signatureHelp` request.
pub fn signature_help_request(
    id: u64,
    document: &LspTextDocumentIdentity,
    position: Utf16Position,
) -> JsonRpcEnvelope {
    JsonRpcEnvelope::request(
        id,
        "textDocument/signatureHelp",
        json!({
            "textDocument": {"uri": document.uri},
            "position": {"line": position.line, "character": position.character},
        }),
    )
}

/// Builds a JSON-RPC `textDocument/documentSymbol` request.
pub fn document_symbol_request(id: u64, document: &LspTextDocumentIdentity) -> JsonRpcEnvelope {
    JsonRpcEnvelope::request(
        id,
        "textDocument/documentSymbol",
        json!({
            "textDocument": {"uri": document.uri},
        }),
    )
}

/// Converts LSP DocumentSymbol/SymbolInformation response shapes into outline rows.
pub fn project_document_symbol_response(
    response: &Value,
    limit: usize,
) -> Vec<LanguageOutlineSymbolProjection> {
    if limit == 0 {
        return Vec::new();
    }
    let Some(symbols) = response.as_array() else {
        return Vec::new();
    };
    let mut rows = Vec::new();
    for symbol in symbols {
        if append_document_symbol_row(&mut rows, symbol, 0, limit) {
            if let Some(last) = rows.last_mut() {
                last.children_omitted = true;
            }
            break;
        }
    }
    rows
}

fn append_document_symbol_row(
    rows: &mut Vec<LanguageOutlineSymbolProjection>,
    symbol: &Value,
    depth: u16,
    limit: usize,
) -> bool {
    if rows.len() >= limit {
        return true;
    }
    let Some(name) = symbol.get("name").and_then(Value::as_str) else {
        return false;
    };
    let label = bounded_lsp_label(name, 120);
    let kind_label = symbol
        .get("kind")
        .and_then(Value::as_u64)
        .map(|kind| format!("lsp.symbol.kind.{kind}"))
        .unwrap_or_else(|| "lsp.symbol.kind.unknown".to_string());
    let range = symbol
        .get("range")
        .or_else(|| {
            symbol
                .get("location")
                .and_then(|location| location.get("range"))
        })
        .and_then(protocol_range_from_lsp_json);
    let row_index = rows.len();
    rows.push(LanguageOutlineSymbolProjection {
        symbol_id: format!(
            "lsp-symbol-{depth}-{row_index}-{:016x}",
            stable_hash(&label)
        ),
        label,
        kind_label,
        range,
        depth,
        children_omitted: false,
        schema_version: 1,
    });

    let mut omitted = false;
    if let Some(children) = symbol.get("children").and_then(Value::as_array) {
        for child in children {
            if append_document_symbol_row(rows, child, depth.saturating_add(1), limit) {
                omitted = true;
                break;
            }
        }
    }
    rows[row_index].children_omitted = omitted;
    false
}

/// Builds a JSON-RPC `workspace/symbol` request.
pub fn workspace_symbol_request(id: u64, query: impl Into<String>) -> JsonRpcEnvelope {
    let query = bounded_lsp_label(&query.into(), 240);
    JsonRpcEnvelope::request(
        id,
        "workspace/symbol",
        json!({
            "query": query,
        }),
    )
}

/// Converts LSP workspace symbol response shapes into metadata-only location rows.
pub fn project_workspace_symbol_response(
    response: &Value,
    limit: usize,
) -> Vec<LanguageLocationProjection> {
    let Some(symbols) = response.as_array() else {
        return Vec::new();
    };
    symbols
        .iter()
        .take(limit)
        .enumerate()
        .filter_map(|(index, symbol)| workspace_symbol_location_projection(index, symbol))
        .collect()
}

fn workspace_symbol_location_projection(
    index: usize,
    symbol: &Value,
) -> Option<LanguageLocationProjection> {
    let name = symbol.get("name")?.as_str()?;
    let location = symbol.get("location")?;
    let range = location.get("range").and_then(protocol_range_from_lsp_json);
    let label = bounded_lsp_label(name, 120);
    let uri_hash = location
        .get("uri")
        .or_else(|| location.get("targetUri"))
        .and_then(Value::as_str)
        .map(stable_hash)
        .unwrap_or(0);
    Some(LanguageLocationProjection {
        location_id: format!(
            "lsp-workspace-symbol-{index}-{:016x}-{:016x}",
            stable_hash(&label),
            uri_hash
        ),
        file_id: None,
        path: None,
        range,
        label,
        degraded: location.get("range").is_none(),
        schema_version: 1,
    })
}

/// Builds a JSON-RPC `textDocument/inlayHint` request.
pub fn inlay_hint_request(
    id: u64,
    document: &LspTextDocumentIdentity,
    range: Utf16Range,
) -> JsonRpcEnvelope {
    JsonRpcEnvelope::request(
        id,
        "textDocument/inlayHint",
        json!({
            "textDocument": {"uri": document.uri},
            "range": utf16_range_to_lsp_json(range),
        }),
    )
}

/// Builds a JSON-RPC `textDocument/foldingRange` request.
pub fn folding_range_request(id: u64, document: &LspTextDocumentIdentity) -> JsonRpcEnvelope {
    JsonRpcEnvelope::request(
        id,
        "textDocument/foldingRange",
        json!({
            "textDocument": {"uri": document.uri},
        }),
    )
}

/// Builds a JSON-RPC `textDocument/semanticTokens/full` request.
pub fn semantic_tokens_full_request(
    id: u64,
    document: &LspTextDocumentIdentity,
) -> JsonRpcEnvelope {
    JsonRpcEnvelope::request(
        id,
        "textDocument/semanticTokens/full",
        json!({
            "textDocument": {"uri": document.uri},
        }),
    )
}

/// Converts LSP InlayHint response shapes into metadata-only inlay hint rows.
pub fn project_inlay_hint_response(
    response: &Value,
    source_label: &str,
    limit: usize,
) -> Vec<LanguageInlayHintProjection> {
    let Some(hints) = response.as_array() else {
        return Vec::new();
    };
    hints
        .iter()
        .take(limit)
        .enumerate()
        .filter_map(|(index, hint)| inlay_hint_projection_for_item(index, hint, source_label))
        .collect()
}

fn inlay_hint_projection_for_item(
    index: usize,
    hint: &Value,
    source_label: &str,
) -> Option<LanguageInlayHintProjection> {
    let position = hint
        .get("position")
        .and_then(protocol_coordinate_from_lsp_json)?;
    let label = inlay_hint_label(hint.get("label")?)?;
    let label = bounded_lsp_label(&label, 120);
    let kind_label = hint
        .get("kind")
        .and_then(Value::as_u64)
        .map(|kind| format!("lsp.inlay.kind.{kind}"))
        .unwrap_or_else(|| "lsp.inlay.kind.unknown".to_string());
    let range = hint
        .get("tooltip")
        .and_then(|tooltip| tooltip.get("range").and_then(protocol_range_from_lsp_json));
    Some(LanguageInlayHintProjection {
        hint_id: format!("lsp-inlay-{index}-{:016x}", stable_hash(&label)),
        label,
        kind_label,
        position,
        range,
        padding_left: hint
            .get("paddingLeft")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        padding_right: hint
            .get("paddingRight")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        source_label: bounded_lsp_label(source_label, 80),
        schema_version: 1,
    })
}

fn inlay_hint_label(value: &Value) -> Option<String> {
    if let Some(label) = value.as_str() {
        return Some(label.to_string());
    }
    let parts = value.as_array()?;
    let label = parts
        .iter()
        .filter_map(|part| {
            part.as_str().map(str::to_string).or_else(|| {
                part.get("value")
                    .and_then(Value::as_str)
                    .map(str::to_string)
            })
        })
        .collect::<Vec<_>>()
        .join("");
    if label.is_empty() { None } else { Some(label) }
}

/// Builds a JSON-RPC `textDocument/codeLens` request.
pub fn code_lens_request(id: u64, document: &LspTextDocumentIdentity) -> JsonRpcEnvelope {
    JsonRpcEnvelope::request(
        id,
        "textDocument/codeLens",
        json!({
            "textDocument": {"uri": document.uri},
        }),
    )
}

/// Converts LSP CodeLens response shapes into metadata-only code lens rows.
pub fn project_code_lens_response(
    response: &Value,
    source_label: &str,
    limit: usize,
) -> Vec<LanguageCodeLensProjection> {
    let Some(lenses) = response.as_array() else {
        return Vec::new();
    };
    lenses
        .iter()
        .take(limit)
        .enumerate()
        .filter_map(|(index, lens)| code_lens_projection_for_item(index, lens, source_label))
        .collect()
}

fn code_lens_projection_for_item(
    index: usize,
    lens: &Value,
    source_label: &str,
) -> Option<LanguageCodeLensProjection> {
    let range = lens.get("range").and_then(protocol_range_from_lsp_json);
    let command = lens.get("command");
    let title = command
        .and_then(|command| command.get("title"))
        .and_then(Value::as_str)
        .map(|title| bounded_lsp_label(title, 120))
        .unwrap_or_else(|| "lsp code lens".to_string());
    let command_label = command
        .and_then(|command| command.get("command"))
        .and_then(Value::as_str)
        .map(|command| bounded_lsp_label(command, 120))
        .unwrap_or_else(|| "lsp.codelens.unresolved".to_string());
    let data_kind = lens
        .get("data")
        .and_then(|data| data.get("kind"))
        .and_then(Value::as_str);
    let kind_label = data_kind
        .map(|kind| format!("lsp.codelens.{}", bounded_lsp_label(kind, 80)))
        .unwrap_or_else(|| {
            if command.is_some() {
                "lsp.codelens.command".to_string()
            } else {
                "lsp.codelens.unresolved".to_string()
            }
        });
    let data_label = code_lens_data_label(lens.get("data"));
    Some(LanguageCodeLensProjection {
        lens_id: format!("lsp-codelens-{index}-{:016x}", stable_hash(&title)),
        title,
        command_label,
        kind_label,
        range,
        data_label,
        source_label: bounded_lsp_label(source_label, 80),
        schema_version: 1,
    })
}

fn code_lens_data_label(data: Option<&Value>) -> Option<String> {
    let data = data?;
    let mut parts = Vec::new();
    if let Some(kind) = data.get("kind").and_then(Value::as_str) {
        parts.push(format!("kind={}", bounded_lsp_label(kind, 80)));
    }
    if let Some(count) = data.get("count").and_then(Value::as_u64) {
        parts.push(format!("count={count}"));
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join(","))
    }
}

/// Converts LSP Location/LocationLink response shapes into location projections.
pub fn project_location_response(
    response: &Value,
    limit: usize,
) -> Vec<LanguageLocationProjection> {
    let locations = if response.is_array() {
        response.as_array().map(Vec::as_slice)
    } else if response.is_object() && !response.is_null() {
        Some(std::slice::from_ref(response))
    } else {
        None
    };
    let Some(locations) = locations else {
        return Vec::new();
    };
    locations
        .iter()
        .take(limit)
        .enumerate()
        .filter_map(|(index, location)| location_projection_for_item(index, location))
        .collect()
}

fn location_projection_for_item(
    index: usize,
    location: &Value,
) -> Option<LanguageLocationProjection> {
    let uri = location
        .get("uri")
        .or_else(|| location.get("targetUri"))
        .and_then(Value::as_str)?;
    let range = location
        .get("targetSelectionRange")
        .or_else(|| location.get("targetRange"))
        .or_else(|| location.get("range"))
        .and_then(protocol_range_from_lsp_json);
    Some(LanguageLocationProjection {
        location_id: format!("lsp-location-{index}-{:016x}", stable_hash(uri)),
        file_id: None,
        path: None,
        range,
        label: format!("LSP location {index}"),
        degraded: range.is_none(),
        schema_version: 1,
    })
}

/// Context needed to project `publishDiagnostics` into Legion metadata rows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LspDiagnosticProjectionContext {
    /// Workspace scope.
    pub workspace_id: WorkspaceId,
    /// File receiving diagnostics.
    pub file_id: FileId,
    /// Snapshot described by diagnostics.
    pub snapshot_id: SnapshotId,
    /// Buffer version described by diagnostics.
    pub buffer_version: BufferVersion,
    /// Optional content hash used for freshness checks.
    pub content_hash: Option<FileFingerprint>,
    /// Privacy scope attached to emitted diagnostic metadata.
    pub privacy_scope: SemanticPrivacyScope,
    /// Whether diagnostic ranges may be surfaced in projection rows.
    pub disclose_ranges: bool,
}

/// Projected diagnostics and metadata-only summary for one `publishDiagnostics` payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LspProjectedDiagnostics {
    /// Problem rows suitable for existing language tooling projections.
    pub problems: Vec<LanguageProblemProjection>,
    /// Metadata-only aggregate summary for context manifests and supervision events.
    pub summary: LspDiagnosticSummary,
}

/// Converts an LSP `textDocument/publishDiagnostics` params object into Legion projections.
pub fn project_publish_diagnostics(
    params: &Value,
    context: LspDiagnosticProjectionContext,
) -> LspRuntimeResult<LspProjectedDiagnostics> {
    let diagnostics = params
        .get("diagnostics")
        .and_then(Value::as_array)
        .ok_or_else(|| LspRuntimeError::MalformedFrame {
            message: "publishDiagnostics payload missing diagnostics array".to_string(),
        })?;

    let mut problems = Vec::with_capacity(diagnostics.len());
    let mut ranges = Vec::new();
    let mut diagnostic_hashes = Vec::new();
    let mut source_hashes = Vec::new();
    let mut error_count = 0u32;
    let mut warning_count = 0u32;
    let mut information_count = 0u32;
    let mut hint_count = 0u32;

    for diagnostic in diagnostics {
        let severity = severity_from_lsp_value(diagnostic.get("severity"));
        match severity {
            ProtocolDiagnosticSeverity::Error => error_count = error_count.saturating_add(1),
            ProtocolDiagnosticSeverity::Warning => warning_count = warning_count.saturating_add(1),
            ProtocolDiagnosticSeverity::Info => {
                information_count = information_count.saturating_add(1);
            }
            ProtocolDiagnosticSeverity::Hint => {
                hint_count = hint_count.saturating_add(1);
            }
        }
        let range = diagnostic
            .get("range")
            .and_then(protocol_range_from_lsp_json);
        if let Some(range) = range {
            ranges.push(range);
        }
        let code_label = diagnostic_code_label(diagnostic);
        let source_label = diagnostic
            .get("source")
            .and_then(Value::as_str)
            .map(str::to_string);
        let message_hash_input = diagnostic
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or_default();
        diagnostic_hashes.push(metadata_fingerprint("lsp.diagnostic", message_hash_input));
        if let Some(source) = &source_label {
            source_hashes.push(metadata_fingerprint("lsp.source", source));
        }
        problems.push(LanguageProblemProjection {
            file_id: Some(context.file_id),
            path: None,
            range: range.filter(|_| context.disclose_ranges),
            severity,
            code_label,
            message: redacted_diagnostic_message(severity),
            source_label,
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        });
    }

    let summary = LspDiagnosticSummary {
        workspace_id: context.workspace_id,
        file_id: context.file_id,
        snapshot_id: context.snapshot_id,
        buffer_version: context.buffer_version,
        content_hash: context.content_hash,
        diagnostic_count: diagnostics.len() as u32,
        error_count,
        warning_count,
        information_count,
        hint_count,
        ranges: if context.disclose_ranges {
            ranges
        } else {
            Vec::new()
        },
        diagnostic_hashes,
        source_hashes,
        freshness: SemanticFreshnessState::Fresh,
        privacy_scope: context.privacy_scope,
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    };

    Ok(LspProjectedDiagnostics { problems, summary })
}

/// Builds a metadata-only warning row when LSP is unavailable and fallback indexing remains active.
pub fn lsp_unavailable_problem_projection(
    context: LspDiagnosticProjectionContext,
    reason: &str,
) -> LanguageProblemProjection {
    LanguageProblemProjection {
        file_id: Some(context.file_id),
        path: None,
        range: None,
        severity: ProtocolDiagnosticSeverity::Warning,
        code_label: Some(format!("lsp.unavailable:{}", stable_hash(reason))),
        message: "LSP unavailable; semantic/index fallback remains active".to_string(),
        source_label: Some("lsp".to_string()),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    }
}

fn utf16_range_to_lsp_json(range: Utf16Range) -> Value {
    json!({
        "start": {"line": range.start.line, "character": range.start.character},
        "end": {"line": range.end.line, "character": range.end.character},
    })
}

fn protocol_range_from_lsp_json(value: &Value) -> Option<ProtocolTextRange> {
    let start = value.get("start")?;
    let end = value.get("end")?;
    Some(ProtocolTextRange {
        start: protocol_coordinate_from_lsp_json(start)?,
        end: protocol_coordinate_from_lsp_json(end)?,
    })
}

fn protocol_coordinate_from_lsp_json(value: &Value) -> Option<TextCoordinate> {
    Some(TextCoordinate {
        line: value.get("line")?.as_u64()?.try_into().ok()?,
        character: value.get("character")?.as_u64()?.try_into().ok()?,
        byte_offset: None,
        utf16_offset: value.get("character")?.as_u64(),
    })
}

fn severity_from_lsp_value(value: Option<&Value>) -> ProtocolDiagnosticSeverity {
    match value.and_then(Value::as_u64) {
        Some(1) => ProtocolDiagnosticSeverity::Error,
        Some(2) => ProtocolDiagnosticSeverity::Warning,
        Some(3) => ProtocolDiagnosticSeverity::Info,
        Some(4) => ProtocolDiagnosticSeverity::Hint,
        _ => ProtocolDiagnosticSeverity::Info,
    }
}

fn diagnostic_code_label(diagnostic: &Value) -> Option<String> {
    let code = diagnostic.get("code")?;
    if let Some(label) = code.as_str() {
        Some(label.to_string())
    } else if let Some(number) = code.as_i64() {
        Some(number.to_string())
    } else {
        Some(format!("hash:{}", stable_hash(&code.to_string())))
    }
}

fn redacted_diagnostic_message(severity: ProtocolDiagnosticSeverity) -> String {
    match severity {
        ProtocolDiagnosticSeverity::Error => "LSP error diagnostic".to_string(),
        ProtocolDiagnosticSeverity::Warning => "LSP warning diagnostic".to_string(),
        ProtocolDiagnosticSeverity::Info => "LSP informational diagnostic".to_string(),
        ProtocolDiagnosticSeverity::Hint => "LSP hint diagnostic".to_string(),
    }
}

fn metadata_fingerprint(label: &str, input: &str) -> FileFingerprint {
    FileFingerprint {
        algorithm: label.to_string(),
        value: format!("{:016x}", stable_hash(input)),
    }
}

// -----------------------------------------------------------------------------
// Process-backed stdio transport (WS03.T1).
// -----------------------------------------------------------------------------

/// Owned handle to a launched `std::process::Child` running an LSP server
/// on piped stdio.
///
/// The handle implements [`LspProcessHandle`] so the existing
/// [`LspSupervisor`] can reuse it for metadata-only lifecycle bookkeeping,
/// and additionally exposes the buffered stdin/stdout pipes so the
/// higher-level [`LspStdioSession`] can read and write Content-Length
/// framed JSON-RPC messages. The stderr pipe is captured so the
/// supervisor/drain loop can read it independently; it is intentionally
/// metadata-only and never propagated into response payloads.
pub struct LspStdioProcess {
    child: Option<Child>,
    stdin: Option<ChildStdin>,
    stdout: Option<BufReader<ChildStdout>>,
    stderr: Option<ChildStderr>,
    killed: bool,
}

impl LspStdioProcess {
    /// Wraps an already-spawned child with captured pipes.
    pub fn new(child: Child) -> LspRuntimeResult<Self> {
        let mut child = child;
        let stdin = child.stdin.take().ok_or(LspRuntimeError::StdioIo {
            message: "child stdin unavailable".to_string(),
        })?;
        let stdout = child.stdout.take().ok_or(LspRuntimeError::StdioIo {
            message: "child stdout unavailable".to_string(),
        })?;
        let stderr = child.stderr.take();
        Ok(Self {
            child: Some(child),
            stdin: Some(stdin),
            stdout: Some(BufReader::new(stdout)),
            stderr,
            killed: false,
        })
    }

    /// Writes a single Content-Length framed envelope to the child's stdin.
    pub fn write_envelope(&mut self, envelope: &JsonRpcEnvelope) -> LspRuntimeResult<()> {
        let frame = LspFramer::encode(envelope)?;
        let stdin = self
            .stdin
            .as_mut()
            .ok_or(LspRuntimeError::SessionNotRunning)?;
        stdin
            .write_all(&frame)
            .map_err(|err| LspRuntimeError::StdioIo {
                message: format!("write frame: {err}"),
            })?;
        stdin.flush().map_err(|err| LspRuntimeError::StdioIo {
            message: format!("flush frame: {err}"),
        })?;
        Ok(())
    }

    /// Reads the next Content-Length framed JSON-RPC envelope from the
    /// child's stdout.
    ///
    /// Returns `Ok(None)` on clean EOF (peer closed the stream) so callers
    /// can distinguish a graceful shutdown from a framing error.
    pub fn read_envelope(&mut self) -> LspRuntimeResult<Option<JsonRpcEnvelope>> {
        let stdout = self
            .stdout
            .as_mut()
            .ok_or(LspRuntimeError::SessionNotRunning)?;
        let payload = match read_lsp_frame(stdout)? {
            Some(payload) => payload,
            None => return Ok(None),
        };
        Ok(Some(serde_json::from_slice(&payload)?))
    }

    /// Detaches the stderr handle so the supervisor/drain loop can read
    /// it independently. Returns `None` if stderr was not captured or
    /// already detached.
    pub fn take_stderr(&mut self) -> Option<ChildStderr> {
        self.stderr.take()
    }

    /// Polls the child without blocking. Returns whether the child is
    /// still running.
    fn child_is_running(&mut self) -> bool {
        self.child
            .as_mut()
            .is_some_and(|child| matches!(child.try_wait(), Ok(None)))
    }
}

impl LspProcessHandle for LspStdioProcess {
    fn is_running(&mut self) -> bool {
        self.child_is_running()
    }

    fn kill(&mut self) {
        self.killed = true;
        if let Some(child) = self.child.as_mut() {
            // Best-effort kill + reap so the test process does not
            // leave a zombie if it exits between kill and drop.
            let _ = child.kill();
            let _ = child.wait();
        }
        // Drop pipes so any blocked reader/writer unblocks.
        self.stdin.take();
        self.stdout.take();
        self.stderr.take();
    }
}

impl Drop for LspStdioProcess {
    fn drop(&mut self) {
        if !self.killed {
            self.kill();
        }
    }
}

/// Interface for launchers that can create a concrete stdio-backed
/// LSP process handle.
pub trait LspStdioSpawner {
    /// Spawns a stdio-backed language-server process.
    fn spawn_stdio(&mut self, config: &LspServerProcessConfig)
    -> LspRuntimeResult<LspStdioProcess>;
}

/// Launcher that spawns real `std::process::Child` processes with
/// piped stdio. Used by tests and by future platform-backed runtimes
/// that need a concrete launcher implementation.
#[derive(Debug, Default)]
pub struct LspStdioLauncher;

impl LspStdioLauncher {
    /// Creates a fresh launcher.
    pub fn new() -> Self {
        Self
    }

    /// Spawns a child process from the given config and returns the
    /// concrete [`LspStdioProcess`] handle so callers can drive the
    /// framed I/O directly without going through a trait object.
    pub fn spawn_stdio(
        &mut self,
        config: &LspServerProcessConfig,
    ) -> LspRuntimeResult<LspStdioProcess> {
        <Self as LspStdioSpawner>::spawn_stdio(self, config)
    }
}

impl LspStdioSpawner for LspStdioLauncher {
    fn spawn_stdio(
        &mut self,
        config: &LspServerProcessConfig,
    ) -> LspRuntimeResult<LspStdioProcess> {
        let child = spawn_stdio_child(config)?;
        LspStdioProcess::new(child)
    }
}

impl LspProcessLauncher for LspStdioLauncher {
    fn spawn(
        &mut self,
        config: &LspServerProcessConfig,
    ) -> LspRuntimeResult<Box<dyn LspProcessHandle>> {
        let process = <Self as LspStdioSpawner>::spawn_stdio(self, config)?;
        Ok(Box::new(process))
    }
}

/// Shared spawn helper used by both the inherent and trait paths.
fn spawn_stdio_child(config: &LspServerProcessConfig) -> LspRuntimeResult<Child> {
    let mut command = Command::new(&config.command);
    command
        .args(&config.args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(cwd) = &config.cwd {
        command.current_dir(cwd);
    }
    for (key, value) in &config.env {
        command.env(key, value);
    }
    command.spawn().map_err(|err| LspRuntimeError::SpawnFailed {
        code: format!("stdio.spawn_failed: {err}"),
    })
}

/// Metadata-only progress notification observed while reading LSP frames.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LspProgressNotification {
    /// Stable hash of the progress token.
    pub token_hash: FileFingerprint,
    /// Progress value kind (`begin`, `report`, `end`, or `unknown`).
    pub kind: String,
    /// Optional stable hash of the progress title/message.
    pub label_hash: Option<FileFingerprint>,
    /// Redaction hints for the progress metadata.
    pub redaction_hints: Vec<RedactionHint>,
    /// Progress metadata schema version.
    pub schema_version: u16,
}

/// Metadata-only `publishDiagnostics` notification observed while reading LSP frames.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LspDiagnosticNotificationMetadata {
    /// Stable hash of the diagnostic URI.
    pub uri_hash: FileFingerprint,
    /// Total diagnostic count.
    pub diagnostic_count: u32,
    /// Number of error diagnostics.
    pub error_count: u32,
    /// Number of warning diagnostics.
    pub warning_count: u32,
    /// Number of informational diagnostics.
    pub information_count: u32,
    /// Number of hint diagnostics.
    pub hint_count: u32,
    /// Stable hashes of diagnostic source labels.
    pub source_hashes: Vec<FileFingerprint>,
    /// Stable hashes of diagnostic messages/codes.
    pub diagnostic_hashes: Vec<FileFingerprint>,
    /// Redaction hints for the notification metadata.
    pub redaction_hints: Vec<RedactionHint>,
    /// Diagnostic-notification metadata schema version.
    pub schema_version: u16,
}

/// Process-backed stdio LSP session that combines an
/// [`LspStdioProcess`] handle with an [`LspClient`] correlation table.
///
/// The session is intentionally synchronous and small: WS03.T1 only
/// authorizes initialize/framing/correlation/process lifecycle. A future
/// WS03.T2+ runtime would build the asynchronous read pump on top of
/// this primitive.
///
/// The session drives the supervision policy through an embedded
/// [`LspSupervisor`] but bypasses the supervisor's process slot so the
/// session can own the live child for framed I/O. The supervisor
/// records the launch attempt and the resulting lifecycle/health
/// metadata, while the actual `kill` is performed by the session's
/// own `Drop` (and the supervisor's `Drop` is a no-op because the
/// process slot is `None` after the handoff).
pub struct LspStdioSession {
    process: LspStdioProcess,
    client: LspClient,
    supervisor: LspSupervisor,
    ready: bool,
    supervision_events: Vec<LspSupervisionEvent>,
    progress_notifications: Vec<LspProgressNotification>,
    diagnostic_notifications: Vec<LspDiagnosticNotificationMetadata>,
}

impl LspStdioSession {
    /// Launches the configured command through [`LspStdioLauncher`] and
    /// drives the supervision policy through the embedded
    /// [`LspSupervisor`]. The session never spawns a child if the
    /// policy denies supervision; in that case it returns
    /// [`LspRuntimeError::SessionNotRunning`] and the launcher
    /// remains untouched.
    pub fn start(
        config: LspSupervisorConfig,
        launcher: &mut impl LspStdioSpawner,
    ) -> LspRuntimeResult<Self> {
        // Step 1: ask the supervisor whether launch is allowed without
        // touching the concrete launcher. Policy-denied launches must
        // emit metadata and return before any process can spawn.
        if !config.launch_policy.process_launch_allowed {
            let mut supervisor = LspSupervisor::new(config.clone());
            let mut refusal_launcher = RefusalLauncher::default();
            let events = supervisor.ensure_started(&mut refusal_launcher);
            return Err(LspRuntimeError::SupervisionRefused { events });
        }

        // Step 2: launch a real child through the stdio launcher and
        // hand it to the session.
        let process = launcher.spawn_stdio(&config.process)?;

        // Step 3: record the already-authorized launch through the
        // metadata supervisor using a no-op process handle. The real
        // stdio process is owned solely by this session.
        let mut supervisor = LspSupervisor::new(config);
        let mut bookkeeping_launcher = BookkeepingLauncher;
        let events = supervisor.ensure_started(&mut bookkeeping_launcher);
        Ok(Self {
            process,
            client: LspClient::new(),
            supervisor,
            ready: false,
            supervision_events: events,
            progress_notifications: Vec::new(),
            diagnostic_notifications: Vec::new(),
        })
    }

    /// Returns the lifecycle state observed when the session was started.
    pub fn lifecycle_state(&self) -> LspSupervisionLifecycleState {
        self.supervisor.lifecycle_state()
    }

    /// Returns the supervision events recorded during the launch
    /// attempt. Tests use this to assert that the policy-deny path
    /// produces a [`LspSupervisionEventKind::LaunchRefused`] event
    /// without raw source payloads.
    pub fn supervision_events(&self) -> &[LspSupervisionEvent] {
        &self.supervision_events
    }

    /// Returns whether the session is still alive.
    pub fn is_running(&mut self) -> bool {
        self.process.is_running()
    }

    /// Sends a JSON-RPC request and returns its pending correlation metadata.
    pub fn send_request(
        &mut self,
        method: impl Into<String>,
        params: Value,
        context: LspOperationContext,
    ) -> LspRuntimeResult<LspPendingRequest> {
        let pending = self.client.prepare_request(method, params, context);
        self.process.write_envelope(&pending.envelope)?;
        Ok(pending)
    }

    /// Sends a JSON-RPC notification envelope.
    pub fn send_notification(
        &mut self,
        method: impl Into<String>,
        params: Value,
    ) -> LspRuntimeResult<()> {
        self.process
            .write_envelope(&JsonRpcEnvelope::notification(method, params))?;
        Ok(())
    }

    /// Blocks until a response for a previously sent request arrives.
    pub fn read_response_for(
        &mut self,
        pending: &LspPendingRequest,
    ) -> LspRuntimeResult<LspCorrelatedResponse> {
        self.read_until_correlated_response(pending.json_rpc_id, pending.request_id)
    }

    /// Cancels a pending request and writes the `$/cancelRequest` notification.
    pub fn cancel_request(
        &mut self,
        request_id: LspRequestId,
    ) -> LspRuntimeResult<LspCancelledRequest> {
        let cancelled = self.client.cancel_request(request_id)?;
        self.process.write_envelope(&cancelled.notification)?;
        Ok(cancelled)
    }

    /// Sends a JSON-RPC request and blocks until the correlated
    /// response arrives. Returns the correlated response metadata
    /// for the in-flight request.
    pub fn request(
        &mut self,
        method: impl Into<String>,
        params: Value,
        context: LspOperationContext,
    ) -> LspRuntimeResult<LspCorrelatedResponse> {
        let pending = self.send_request(method, params, context)?;
        self.read_response_for(&pending)
    }

    /// Sends the `initialize` request and returns the correlated
    /// `ServerCapabilities`-shaped result.
    ///
    /// The returned value's `status` is metadata-only; on success it
    /// will be [`LspResultStatus::Fresh`]. On JSON-RPC error responses
    /// it is [`LspResultStatus::Unavailable`].
    pub fn initialize(
        &mut self,
        params: Value,
        context: LspOperationContext,
    ) -> LspRuntimeResult<LspCorrelatedResponse> {
        let response = self.request("initialize", params, context)?;
        if response.status == LspResultStatus::Fresh {
            self.ready = true;
        }
        Ok(response)
    }

    /// Returns metadata-only progress notifications observed while reading frames.
    pub fn progress_notifications(&self) -> &[LspProgressNotification] {
        &self.progress_notifications
    }

    /// Returns metadata-only diagnostic notifications observed while reading frames.
    pub fn diagnostic_notifications(&self) -> &[LspDiagnosticNotificationMetadata] {
        &self.diagnostic_notifications
    }

    /// Returns whether the session has successfully completed an
    /// `initialize` exchange.
    pub fn is_ready(&self) -> bool {
        self.ready
    }

    /// Reads frames from the child until we see a response for
    /// `target_json_rpc_id`; intermediate notifications are
    /// discarded so the framing/buffering layer can be exercised
    /// without affecting correlation.
    fn read_until_correlated_response(
        &mut self,
        target_json_rpc_id: u64,
        expected_request_id: LspRequestId,
    ) -> LspRuntimeResult<LspCorrelatedResponse> {
        loop {
            let envelope = match self.process.read_envelope()? {
                Some(envelope) => envelope,
                None => {
                    return Err(LspRuntimeError::StdioIo {
                        message: "child closed stdout before response".to_string(),
                    });
                }
            };
            let Some(id) = envelope.id else {
                if envelope.method.as_deref() == Some("$/progress")
                    && let Some(progress) =
                        progress_notification_from_params(envelope.params.as_ref())
                {
                    self.progress_notifications.push(progress);
                } else if envelope.method.as_deref() == Some("textDocument/publishDiagnostics")
                    && let Some(diagnostics) =
                        diagnostic_notification_from_params(envelope.params.as_ref())
                {
                    self.diagnostic_notifications.push(diagnostics);
                }
                continue;
            };
            if id != target_json_rpc_id {
                // Skip notification-shaped or out-of-order responses.
                continue;
            }
            let correlated = self.client.correlate_response(envelope)?;
            debug_assert_eq!(correlated.request_id, expected_request_id);
            return Ok(correlated);
        }
    }
}

fn progress_notification_from_params(params: Option<&Value>) -> Option<LspProgressNotification> {
    let params = params?;
    let token = params.get("token")?.to_string();
    let value = params.get("value");
    let kind = value
        .and_then(|value| value.get("kind"))
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .to_string();
    let label = value.and_then(|value| {
        value
            .get("title")
            .or_else(|| value.get("message"))
            .and_then(Value::as_str)
    });
    Some(LspProgressNotification {
        token_hash: metadata_fingerprint("lsp.progress.token", &token),
        kind,
        label_hash: label.map(|label| metadata_fingerprint("lsp.progress.label", label)),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    })
}

fn diagnostic_notification_from_params(
    params: Option<&Value>,
) -> Option<LspDiagnosticNotificationMetadata> {
    let params = params?;
    let uri = params.get("uri")?.as_str()?;
    let diagnostics = params.get("diagnostics")?.as_array()?;
    let mut error_count = 0u32;
    let mut warning_count = 0u32;
    let mut information_count = 0u32;
    let mut hint_count = 0u32;
    let mut source_hashes = Vec::new();
    let mut diagnostic_hashes = Vec::new();
    for diagnostic in diagnostics {
        match severity_from_lsp_value(diagnostic.get("severity")) {
            ProtocolDiagnosticSeverity::Error => error_count = error_count.saturating_add(1),
            ProtocolDiagnosticSeverity::Warning => warning_count = warning_count.saturating_add(1),
            ProtocolDiagnosticSeverity::Info => {
                information_count = information_count.saturating_add(1);
            }
            ProtocolDiagnosticSeverity::Hint => hint_count = hint_count.saturating_add(1),
        }
        if let Some(source) = diagnostic.get("source").and_then(Value::as_str) {
            source_hashes.push(metadata_fingerprint("lsp.diagnostic.source", source));
        }
        let message = diagnostic
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let code = diagnostic
            .get("code")
            .map(Value::to_string)
            .unwrap_or_default();
        diagnostic_hashes.push(metadata_fingerprint(
            "lsp.diagnostic.notification",
            &format!("{code}:{message}"),
        ));
    }
    Some(LspDiagnosticNotificationMetadata {
        uri_hash: metadata_fingerprint("lsp.diagnostic.uri", uri),
        diagnostic_count: diagnostics.len() as u32,
        error_count,
        warning_count,
        information_count,
        hint_count,
        source_hashes,
        diagnostic_hashes,
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    })
}

impl Drop for LspStdioSession {
    fn drop(&mut self) {
        // The `LspStdioProcess`'s own `Drop` will kill the child. The
        // embedded `LspSupervisor` has an empty process slot so its
        // `Drop` is a no-op.
        let _ = &mut self.process;
    }
}

/// Launcher used by the policy-deny path: it refuses to spawn any
/// process so we can observe the supervision refusal without having
/// to clean up a real child.
#[derive(Debug, Default)]
struct RefusalLauncher {
    spawn_calls: usize,
}

impl LspProcessLauncher for RefusalLauncher {
    fn spawn(
        &mut self,
        _config: &LspServerProcessConfig,
    ) -> LspRuntimeResult<Box<dyn LspProcessHandle>> {
        self.spawn_calls += 1;
        Err(LspRuntimeError::SpawnFailed {
            code: "stdio.refusal_launcher.never_spawn".to_string(),
        })
    }
}

/// Launcher used by stdio sessions to record a successful launch in the
/// metadata-only supervisor without taking ownership of the real child.
#[derive(Debug, Default)]
struct BookkeepingLauncher;

impl LspProcessLauncher for BookkeepingLauncher {
    fn spawn(
        &mut self,
        _config: &LspServerProcessConfig,
    ) -> LspRuntimeResult<Box<dyn LspProcessHandle>> {
        Ok(Box::new(BookkeepingProcess { running: true }))
    }
}

#[derive(Debug)]
struct BookkeepingProcess {
    running: bool,
}

impl LspProcessHandle for BookkeepingProcess {
    fn is_running(&mut self) -> bool {
        self.running
    }

    fn kill(&mut self) {
        self.running = false;
    }
}

/// Reads one Content-Length framed LSP payload from a buffered reader.
fn read_lsp_frame<R: BufRead>(reader: &mut R) -> LspRuntimeResult<Option<Vec<u8>>> {
    let mut header = Vec::with_capacity(128);
    let mut byte = [0u8; 1];
    loop {
        match reader.read(&mut byte) {
            Ok(0) => {
                if header.is_empty() {
                    return Ok(None);
                }
                return Err(LspRuntimeError::MalformedFrame {
                    message: "unexpected EOF in header".to_string(),
                });
            }
            Ok(_) => {
                header.push(byte[0]);
                if header.ends_with(b"\r\n\r\n") {
                    break;
                }
                if header.len() > 16 * 1024 {
                    return Err(LspRuntimeError::MalformedFrame {
                        message: "header section too large".to_string(),
                    });
                }
            }
            Err(err) => {
                return Err(LspRuntimeError::StdioIo {
                    message: format!("read header: {err}"),
                });
            }
        }
    }
    let header_str = std::str::from_utf8(&header[..header.len() - 4]).map_err(|err| {
        LspRuntimeError::MalformedFrame {
            message: format!("header was not UTF-8: {err}"),
        }
    })?;
    let length: usize = header_str
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("Content-Length").then_some(value)
        })
        .ok_or_else(|| LspRuntimeError::MalformedFrame {
            message: "missing Content-Length header".to_string(),
        })?
        .trim()
        .parse()
        .map_err(|err| LspRuntimeError::MalformedFrame {
            message: format!("invalid Content-Length: {err}"),
        })?;
    if length > LspFramer::MAX_FRAME_PAYLOAD_BYTES {
        return Err(LspRuntimeError::MalformedFrame {
            message: format!(
                "Content-Length {length} exceeds max {}",
                LspFramer::MAX_FRAME_PAYLOAD_BYTES
            ),
        });
    }

    let mut payload = vec![0u8; length];
    reader.read_exact(&mut payload).map_err(|err| {
        if err.kind() == std::io::ErrorKind::UnexpectedEof {
            LspRuntimeError::MalformedFrame {
                message: "payload shorter than Content-Length".to_string(),
            }
        } else {
            LspRuntimeError::StdioIo {
                message: format!("read payload: {err}"),
            }
        }
    })?;
    Ok(Some(payload))
}
