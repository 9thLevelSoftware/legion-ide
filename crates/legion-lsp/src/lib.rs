//! Language-server runtime: JSON-RPC framing and supervised lifecycle scaffolding.

#![warn(missing_docs)]

/// LSP diagnostics projection module.
pub mod diagnostics;
/// LSP feature request builders and projection module.
pub mod features;
/// Pinned npm archive descriptors approved for downloaded language servers.
pub mod pinned_archives;

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::path::{Component, Path, PathBuf};
use std::process::{Child, ChildStderr, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use legion_protocol::{
    BufferVersion, CanonicalPath, EventId, EventSequence, FileFingerprint, FileId,
    LanguageCodeLensProjection, LanguageCompletionProjection, LanguageHoverProjection, LanguageId,
    LanguageInlayHintProjection, LanguageLocationProjection, LanguageOutlineSymbolProjection,
    LanguageProblemProjection, LspCallHierarchyIncomingCall, LspCallHierarchyItem,
    LspCallHierarchyOutgoingCall, LspDiagnosticSummary, LspFormattingOptions, LspHealthState,
    LspLaunchDisposition, LspLaunchPolicyDecision, LspOperationContext, LspPrepareRenameResult,
    LspRequestId, LspRestartBackoffMetadata, LspResultStatus, LspSupervisionEvent,
    LspSupervisionEventKind, LspSupervisionLifecycleState, ProtocolDiagnosticSeverity,
    ProtocolTextRange, RedactionHint, SemanticFreshnessState, SemanticPrivacyScope, SnapshotId,
    TextCoordinate, Utf16Position, Utf16Range, WorkspaceId,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use thiserror::Error;
use uuid::Uuid;

use pinned_archives::is_sha256_digest;
// Re-exported at the crate root so the extraction is transparent: every
// existing `legion_lsp::NAME` path keeps resolving after the move.
pub use pinned_archives::{
    LspPinnedArchive, TYPESCRIPT_COMPILER_ARCHIVE, TYPESCRIPT_LANGUAGE_SERVER_ARCHIVE,
    TYPESCRIPT_LANGUAGE_SERVER_POLICY_GATE, is_exact_pinned_version,
};

/// Result type used by the LSP runtime crate.
pub type LspRuntimeResult<T> = Result<T, LspRuntimeError>;

/// Maximum serialized parameter bytes accepted for an inbound
/// `workspace/applyEdit` request.
pub const MAX_APPLY_EDIT_PARAMS_BYTES: usize = 256 * 1024;

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
    /// The monotonic JSON-RPC id space was exhausted, so no further
    /// request can be correlated without reusing an id.
    #[error("LSP JSON-RPC id space exhausted")]
    JsonRpcIdExhausted,
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

/// A normalized Node.js semantic version supplied by the runtime approval
/// boundary. The app must obtain this by actually running its approved Node
/// executable with `--version`; this crate only compares the supplied value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct LspNodeVersion {
    /// Major version.
    pub major: u32,
    /// Minor version.
    pub minor: u32,
    /// Patch version.
    pub patch: u32,
}

impl LspNodeVersion {
    /// Parses one exact `node --version` line: `vMAJOR.MINOR.PATCH` or
    /// `MAJOR.MINOR.PATCH`, with one optional terminal LF or CRLF. Internal
    /// newlines, trailing text, and prerelease/build suffixes are rejected.
    pub fn parse(value: &str) -> Result<Self, LspDownloadedArtifactResolveError> {
        let value = value
            .strip_suffix("\r\n")
            .or_else(|| value.strip_suffix('\n'))
            .unwrap_or(value);
        let value = value.strip_prefix('v').unwrap_or(value);
        if value.is_empty() || value.contains('\r') || value.contains('\n') {
            return Err(invalid_runtime_version(value));
        }
        let mut parts = value.split('.');
        let parse_component = |component: Option<&str>| {
            let component = component.ok_or_else(|| invalid_runtime_version(value))?;
            if component.is_empty() || !component.bytes().all(|byte| byte.is_ascii_digit()) {
                return Err(invalid_runtime_version(value));
            }
            component
                .parse()
                .map_err(|_| invalid_runtime_version(value))
        };
        let version = Self {
            major: parse_component(parts.next())?,
            minor: parse_component(parts.next())?,
            patch: parse_component(parts.next())?,
        };
        if parts.next().is_some() {
            return Err(invalid_runtime_version(value));
        }
        Ok(version)
    }
}

/// Runtime required to execute a downloaded language-server artifact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LspArtifactRuntime {
    /// A Node.js executable must launch the package entrypoint.
    Node {
        /// Minimum compatible Node version.
        minimum_version: LspNodeVersion,
    },
}

/// Packaging metadata for a downloaded language-server artifact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LspDownloadedArtifactMetadata {
    /// Package name recorded by the package manifest.
    pub package_name: String,
    /// Pinned package version.
    pub version: String,
    /// Archive format (for example, `tar.gz`).
    pub archive_format: String,
    /// Relative package root inside the materialized artifact directory.
    pub package_root: PathBuf,
    /// Relative executable entrypoint below `package_root`.
    pub entrypoint: PathBuf,
    /// Runtime required to execute the entrypoint.
    pub runtime: LspArtifactRuntime,
}

/// Failure while resolving a materialized downloaded artifact into a process.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum LspDownloadedArtifactResolveError {
    /// A downloaded artifact has no materialized process configuration yet.
    #[error("downloaded artifact is not materialized")]
    ArtifactNotMaterialized,
    /// The adapter is not a downloaded artifact.
    #[error("adapter is not a downloaded artifact")]
    NotDownloadedArtifact,
    /// A required path was not absolute.
    #[error("{field} must be an absolute path")]
    RelativePath {
        /// Path role that must be absolute.
        field: &'static str,
    },
    /// A metadata path contains an unsafe component.
    #[error("{field} contains an unsafe path component")]
    UnsafePath {
        /// Metadata path role containing the unsafe component.
        field: &'static str,
    },
    /// A required path is absent.
    #[error("{field} does not exist: {path}")]
    MissingPath {
        /// Path role that is missing.
        field: &'static str,
        /// Missing filesystem path.
        path: PathBuf,
    },
    /// A required path is not the expected filesystem kind.
    #[error("{field} is not the expected filesystem entry: {path}")]
    WrongPathKind {
        /// Path role with the wrong kind.
        field: &'static str,
        /// Filesystem path with the wrong kind.
        path: PathBuf,
    },
    /// A runtime version string was malformed.
    #[error("invalid Node runtime version: {value}")]
    InvalidRuntimeVersion {
        /// Runtime version text that failed parsing.
        value: String,
    },
    /// The supplied runtime does not satisfy the descriptor minimum.
    #[error("Node runtime {observed:?} is older than required {required:?}")]
    RuntimeTooOld {
        /// Descriptor minimum.
        required: LspNodeVersion,
        /// Version observed by the app-owned approval boundary.
        observed: LspNodeVersion,
    },
    /// A path cannot be represented by the process configuration string API.
    #[error("{field} contains a non-UTF-8 path")]
    NonUtf8Path {
        /// Path role containing non-UTF-8 data.
        field: &'static str,
    },
    /// A canonical path uses an unsupported device namespace for Node.
    #[error("{field} uses an unsupported Windows device path namespace")]
    UnsupportedPathNamespace {
        /// Path role using the unsupported namespace.
        field: &'static str,
    },
    /// Catalog metadata omitted the package name.
    #[error("downloaded artifact package_name is empty")]
    EmptyPackageName,
    /// Catalog metadata pads the package name with surrounding whitespace.
    ///
    /// The name is compared byte for byte against the artifact descriptor by
    /// the app-owned startup authority, so a padded name that passed a
    /// trimming check here would fail there instead, far from its cause.
    #[error("downloaded artifact package_name {value:?} has surrounding whitespace")]
    PaddedPackageName {
        /// Package name text carrying leading or trailing whitespace.
        value: String,
    },
    /// Catalog metadata omitted the package version.
    #[error("downloaded artifact version is empty")]
    EmptyPackageVersion,
    /// Catalog metadata records a range or dist-tag instead of an exact pin.
    #[error("downloaded artifact version {value:?} is not an exact pinned release")]
    UnpinnedPackageVersion {
        /// Version text that is not an exact pinned release.
        value: String,
    },
    /// The catalog checksum is not a SHA-256 digest, so no materializer
    /// receipt can be verified against it.
    #[error("downloaded artifact catalog checksum is not a SHA-256 digest")]
    InvalidCatalogChecksum,
    /// The materializer receipt is not a SHA-256 digest.
    #[error("verified artifact checksum is not a SHA-256 digest")]
    InvalidChecksum,
    /// The materializer receipt does not match the catalog checksum.
    #[error("verified artifact checksum does not match catalog receipt")]
    ChecksumMismatch,
}

fn invalid_runtime_version(value: &str) -> LspDownloadedArtifactResolveError {
    LspDownloadedArtifactResolveError::InvalidRuntimeVersion {
        value: value.to_string(),
    }
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
        /// Pinned packaging and runtime metadata.
        metadata: Box<LspDownloadedArtifactMetadata>,
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
    /// Declared process configuration; downloaded entries require resolution
    /// after app-owned materialization before they can launch.
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

    /// Creates a package-backed adapter with explicit archive and runtime metadata.
    #[allow(clippy::too_many_arguments)]
    pub fn downloaded_package_artifact(
        server_id: legion_protocol::LanguageServerId,
        workspace_id: legion_protocol::WorkspaceId,
        language_id: legion_protocol::LanguageId,
        display_name: impl Into<String>,
        binary_name: impl Into<String>,
        artifact_uri: impl Into<String>,
        checksum_sha256: impl Into<String>,
        policy_gate: impl Into<String>,
        metadata: LspDownloadedArtifactMetadata,
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
                metadata: Box::new(metadata),
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

    /// Returns a launch-ready process configuration for system-path adapters.
    pub fn process_config(
        &self,
    ) -> Result<LspServerProcessConfig, LspDownloadedArtifactResolveError> {
        match &self.binary_source {
            LspServerBinarySource::DownloadedArtifact { .. } => {
                Err(LspDownloadedArtifactResolveError::ArtifactNotMaterialized)
            }
            LspServerBinarySource::SystemPath { .. } => Ok(self.process.clone()),
        }
    }

    /// Resolves a materialized package into a shell-free Node process config.
    ///
    /// The app-owned materializer must verify the archive and approve the Node
    /// executable first, then pass its observed `node --version` output and
    /// the verified artifact SHA-256 receipt here. Path and compatibility
    /// checks still run; the receipt binds this resolve to that materializer
    /// identity rather than trusting an arbitrary directory.
    pub fn resolve_downloaded_process(
        &self,
        artifact_root: &Path,
        approved_node: &Path,
        observed_node_version: &str,
        verified_artifact_sha256: &str,
    ) -> Result<LspServerProcessConfig, LspDownloadedArtifactResolveError> {
        let LspServerBinarySource::DownloadedArtifact {
            checksum_sha256,
            metadata,
            ..
        } = &self.binary_source
        else {
            return Err(LspDownloadedArtifactResolveError::NotDownloadedArtifact);
        };
        if metadata.package_name.trim().is_empty() {
            return Err(LspDownloadedArtifactResolveError::EmptyPackageName);
        }
        // Validate the bytes that are reported and compared, not a trimmed
        // copy of them. `startup_authority` compares `metadata.package_name`
        // and `metadata.version` byte for byte against the pinned descriptor,
        // so a check that silently normalizes here only relocates the failure
        // to a site that cannot explain it.
        if metadata.package_name != metadata.package_name.trim() {
            return Err(LspDownloadedArtifactResolveError::PaddedPackageName {
                value: metadata.package_name.clone(),
            });
        }
        if metadata.version.trim().is_empty() {
            return Err(LspDownloadedArtifactResolveError::EmptyPackageVersion);
        }
        // A catalog entry that names a range or a dist-tag names no particular
        // artifact: what it resolves to changes under the product's feet, so a
        // digest recorded beside it cannot mean anything. Reject the unpinned
        // catalog entry before comparing any materializer receipt against it.
        // A padded version is rejected here too: ` 6.0.0 ` is not the byte
        // string the descriptor literal contains.
        if !is_exact_pinned_version(&metadata.version) {
            return Err(LspDownloadedArtifactResolveError::UnpinnedPackageVersion {
                value: metadata.version.clone(),
            });
        }
        if !is_sha256_digest(checksum_sha256) {
            return Err(LspDownloadedArtifactResolveError::InvalidCatalogChecksum);
        }
        if !is_sha256_digest(verified_artifact_sha256) {
            return Err(LspDownloadedArtifactResolveError::InvalidChecksum);
        }
        if !checksum_sha256.eq_ignore_ascii_case(verified_artifact_sha256) {
            return Err(LspDownloadedArtifactResolveError::ChecksumMismatch);
        }
        if !artifact_root.is_absolute() {
            return Err(LspDownloadedArtifactResolveError::RelativePath {
                field: "artifact_root",
            });
        }
        if !approved_node.is_absolute() {
            return Err(LspDownloadedArtifactResolveError::RelativePath {
                field: "approved_node",
            });
        }
        validate_relative_artifact_path(&metadata.package_root, "package_root")?;
        validate_relative_artifact_path(&metadata.entrypoint, "entrypoint")?;
        let observed = LspNodeVersion::parse(observed_node_version)?;
        let LspArtifactRuntime::Node { minimum_version } = &metadata.runtime;
        if observed < *minimum_version {
            return Err(LspDownloadedArtifactResolveError::RuntimeTooOld {
                required: *minimum_version,
                observed,
            });
        }
        let root = artifact_root.canonicalize().map_err(|_| {
            LspDownloadedArtifactResolveError::MissingPath {
                field: "artifact_root",
                path: artifact_root.to_path_buf(),
            }
        })?;
        if !root.is_dir() {
            return Err(LspDownloadedArtifactResolveError::WrongPathKind {
                field: "artifact_root",
                path: artifact_root.to_path_buf(),
            });
        }
        let node = approved_node.canonicalize().map_err(|_| {
            LspDownloadedArtifactResolveError::MissingPath {
                field: "approved_node",
                path: approved_node.to_path_buf(),
            }
        })?;
        if !node.is_file() {
            return Err(LspDownloadedArtifactResolveError::WrongPathKind {
                field: "approved_node",
                path: approved_node.to_path_buf(),
            });
        }
        let package = root.join(&metadata.package_root);
        let entrypoint = package.join(&metadata.entrypoint);
        let package =
            package
                .canonicalize()
                .map_err(|_| LspDownloadedArtifactResolveError::MissingPath {
                    field: "package_root",
                    path: package,
                })?;
        if !package.starts_with(&root) || !package.is_dir() {
            return Err(LspDownloadedArtifactResolveError::WrongPathKind {
                field: "package_root",
                path: package,
            });
        }
        let entrypoint = entrypoint.canonicalize().map_err(|_| {
            LspDownloadedArtifactResolveError::MissingPath {
                field: "entrypoint",
                path: entrypoint,
            }
        })?;
        if !entrypoint.starts_with(&package) || !entrypoint.is_file() {
            return Err(LspDownloadedArtifactResolveError::WrongPathKind {
                field: "entrypoint",
                path: entrypoint,
            });
        }
        let command = node.into_os_string().into_string().map_err(|_| {
            LspDownloadedArtifactResolveError::NonUtf8Path {
                field: "approved_node",
            }
        })?;
        let entrypoint = node_compatible_path(&entrypoint, "entrypoint")?;
        let mut args = self.process.args.clone();
        args.insert(0, entrypoint);
        if !args.iter().any(|arg| arg == "--stdio") {
            args.push("--stdio".to_string());
        }
        Ok(LspServerProcessConfig {
            command,
            args,
            cwd: self.process.cwd.clone(),
            env: self.process.env.clone(),
        })
    }
}

/// Serialize a canonical Windows path in the form accepted by Node's module
/// loader. Rust may expose canonical paths with the `\\?\` prefix; retain
/// drive and UNC identity while rejecting arbitrary device namespaces.
pub fn node_compatible_path(
    path: &Path,
    field: &'static str,
) -> Result<String, LspDownloadedArtifactResolveError> {
    let value = path
        .to_str()
        .ok_or(LspDownloadedArtifactResolveError::NonUtf8Path { field })?;
    #[cfg(windows)]
    {
        if let Some(rest) = value.strip_prefix("\\\\?\\") {
            if rest.len() >= 2 && rest.as_bytes()[1] == b':' {
                return Ok(rest.to_string());
            }
            if let Some(unc) = rest.strip_prefix("UNC\\") {
                return Ok(format!("\\\\{unc}"));
            }
            return Err(LspDownloadedArtifactResolveError::UnsupportedPathNamespace { field });
        }
        if value.starts_with("\\\\.\\") {
            return Err(LspDownloadedArtifactResolveError::UnsupportedPathNamespace { field });
        }
    }
    Ok(value.to_string())
}

fn validate_relative_artifact_path(
    path: &Path,
    field: &'static str,
) -> Result<(), LspDownloadedArtifactResolveError> {
    if path.as_os_str().is_empty() || path.is_absolute() {
        return Err(LspDownloadedArtifactResolveError::UnsafePath { field });
    }
    if path.components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::CurDir | Component::RootDir | Component::Prefix(_)
        )
    }) {
        return Err(LspDownloadedArtifactResolveError::UnsafePath { field });
    }
    Ok(())
}

/// Resolution inputs for locating a rust-analyzer binary (design §5).
#[derive(Debug, Clone, Default)]
pub struct RustAnalyzerDiscovery {
    /// Explicit user/workspace-configured path.
    pub configured_path: Option<std::path::PathBuf>,
    /// Project-local vendored binary path.
    pub project_local_path: Option<std::path::PathBuf>,
    /// Application-bundled binary path.
    pub bundled_path: Option<std::path::PathBuf>,
    /// Raw `PATH` environment value to scan for `rust-analyzer`.
    pub path_env: Option<String>,
}

/// Outcome of binary discovery.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiscoveredBinary {
    /// A binary was resolved with the given provenance.
    Found {
        /// Resolved binary path.
        path: std::path::PathBuf,
        /// How it was resolved.
        provenance: legion_protocol::LspServerBinaryProvenance,
    },
    /// No binary could be resolved through any source.
    NotFound,
}

impl RustAnalyzerDiscovery {
    /// Resolves the binary in order: configured -> project-local -> PATH -> bundled.
    pub fn resolve(&self) -> DiscoveredBinary {
        use legion_protocol::LspServerBinaryProvenance as P;
        if let Some(p) = &self.configured_path {
            return DiscoveredBinary::Found {
                path: p.clone(),
                provenance: P::Configured,
            };
        }
        if let Some(p) = &self.project_local_path {
            return DiscoveredBinary::Found {
                path: p.clone(),
                provenance: P::ProjectLocal,
            };
        }
        if let Some(path_env) = &self.path_env {
            let exe = if cfg!(windows) {
                "rust-analyzer.exe"
            } else {
                "rust-analyzer"
            };
            for dir in std::env::split_paths(path_env) {
                let candidate = dir.join(exe);
                if candidate.is_file() {
                    return DiscoveredBinary::Found {
                        path: candidate,
                        provenance: P::SystemPath,
                    };
                }
            }
        }
        if let Some(p) = &self.bundled_path {
            return DiscoveredBinary::Found {
                path: p.clone(),
                provenance: P::Bundled,
            };
        }
        DiscoveredBinary::NotFound
    }

    /// Probes `<path> --version`, returning the trimmed stdout line if it runs.
    pub fn probe_version(path: &std::path::Path) -> Option<String> {
        let output = std::process::Command::new(path)
            .arg("--version")
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if text.is_empty() { None } else { Some(text) }
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

/// Errors raised while binding catalog adapters to an opened workspace.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LanguageServerRegistryError {
    /// Workspace identity zero is reserved and cannot own an adapter plan.
    InvalidWorkspaceId,
}

impl LanguageServerAdapterRegistry {
    /// Creates an empty adapter registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Rebinds the immutable catalog to one real opened workspace identity.
    ///
    /// The catalog is cloned and every plan receives `workspace_id`; the
    /// source registry is never mutated. Registration preserves the existing
    /// primary/name/server ordering. A zero workspace identity is rejected so
    /// callers cannot create fixture-like or fabricated bindings.
    pub fn for_workspace(
        &self,
        workspace_id: legion_protocol::WorkspaceId,
    ) -> Result<Self, LanguageServerRegistryError> {
        if workspace_id.0 == 0 {
            return Err(LanguageServerRegistryError::InvalidWorkspaceId);
        }
        let mut bound = Self::new();
        for adapters in self.adapters_by_language.values() {
            for adapter in adapters {
                let mut rebound = adapter.clone();
                rebound.workspace_id = workspace_id;
                bound.register(rebound);
            }
        }
        Ok(bound)
    }

    /// Returns the ordered adapter plans for a real workspace and language.
    pub fn adapters_for_workspace_language(
        &self,
        workspace_id: legion_protocol::WorkspaceId,
        language_id: &legion_protocol::LanguageId,
    ) -> Result<Vec<&LanguageServerAdapterPlan>, LanguageServerRegistryError> {
        if workspace_id.0 == 0 {
            return Err(LanguageServerRegistryError::InvalidWorkspaceId);
        }
        Ok(self
            .adapters_for_language(language_id)
            .into_iter()
            .filter(|adapter| adapter.workspace_id == workspace_id)
            .collect())
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
    ///
    /// This method materializes the requested adapter list as an all-or-error
    /// operation. If any selected entry is a downloaded artifact that has not
    /// been materialized, it returns `ArtifactNotMaterialized`; higher-level
    /// app code must select an adapter before resolution rather than treating
    /// this method as a fallback-selection policy.
    pub fn process_configs_for_workspace_language(
        &self,
        workspace_id: legion_protocol::WorkspaceId,
        language_id: &legion_protocol::LanguageId,
    ) -> Result<Vec<LspServerProcessConfig>, LspDownloadedArtifactResolveError> {
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
                        metadata,
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
                                    metadata: metadata.clone(),
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
    ///
    /// When the `CARGO_BIN_EXE_mock_lsp_server` environment variable is set
    /// (as it is during `cargo test`), the rust-analyzer entry uses the mock
    /// server binary so integration tests can exercise the product registry
    /// path without requiring a real rust-analyzer installation.
    pub fn tier_two() -> Self {
        let workspace_id = legion_protocol::WorkspaceId(1);
        let rust_command = std::env::var("CARGO_BIN_EXE_mock_lsp_server")
            .unwrap_or_else(|_| "rust-analyzer".to_string());
        let mut registry = Self::new();
        registry.register(LanguageServerAdapterPlan::system_path(
            legion_protocol::LanguageServerId(101),
            workspace_id,
            legion_protocol::LanguageId("rust".to_string()),
            "rust-analyzer",
            &rust_command,
            Vec::new(),
            true,
        ));
        // The approved TypeScript/JavaScript server is a pinned npm archive:
        // exact release, exact SHA-256, exact package root and entrypoint, and
        // the Node minimum that release declares. Its peer compiler archive is
        // pinned separately as `TYPESCRIPT_COMPILER_ARCHIVE`, because a
        // descriptor describes one archive and one entrypoint only.
        let typescript_family_server = |server_id: u64, language_id: &str, display_name: &str| {
            LanguageServerAdapterPlan::downloaded_package_artifact(
                legion_protocol::LanguageServerId(server_id),
                workspace_id,
                legion_protocol::LanguageId(language_id.to_string()),
                display_name,
                TYPESCRIPT_LANGUAGE_SERVER_ARCHIVE.package_name,
                TYPESCRIPT_LANGUAGE_SERVER_ARCHIVE.archive_url,
                TYPESCRIPT_LANGUAGE_SERVER_ARCHIVE.checksum_sha256,
                TYPESCRIPT_LANGUAGE_SERVER_POLICY_GATE,
                TYPESCRIPT_LANGUAGE_SERVER_ARCHIVE.metadata(),
                vec!["--stdio".to_string()],
                true,
            )
        };
        registry.register(typescript_family_server(
            102,
            "typescript",
            "typescript-language-server",
        ));
        // `tailwindcss-language-server` has no retained artifact and no
        // verified digest on this host, so it stays an unpinned PATH lookup
        // until an approved archive exists. It is the one documented
        // exception in the TypeScript family, and the registry contract test
        // names it explicitly rather than allowing it by a loose predicate.
        registry.register(LanguageServerAdapterPlan::system_path(
            legion_protocol::LanguageServerId(103),
            workspace_id,
            legion_protocol::LanguageId("typescript".to_string()),
            "tailwindcss-language-server",
            "tailwindcss-language-server",
            vec!["--stdio".to_string()],
            false,
        ));
        // The TypeScript language server also serves JavaScript/JSX/TSX. Keep
        // a distinct language identity so initialize/text-document language
        // IDs are advertised correctly while reusing the same pinned archive.
        registry.register(typescript_family_server(
            106,
            "javascript",
            "typescript-language-server (JavaScript)",
        ));
        registry.register(typescript_family_server(
            107,
            "javascriptreact",
            "typescript-language-server (JSX)",
        ));
        registry.register(typescript_family_server(
            108,
            "typescriptreact",
            "typescript-language-server (TSX)",
        ));
        registry.register(LanguageServerAdapterPlan::downloaded_package_artifact(
            legion_protocol::LanguageServerId(104),
            workspace_id,
            legion_protocol::LanguageId("python".to_string()),
            "pyright",
            "pyright-langserver",
            "https://registry.npmjs.org/pyright/-/pyright-1.1.400.tgz",
            "2ccba7af9c8b14bb81c8fa9bb558d8b5181b586ec4dfc448b78eb4209e7a429a",
            "policy://lsp-download/pyright",
            LspDownloadedArtifactMetadata {
                package_name: "pyright".to_string(),
                version: "1.1.400".to_string(),
                archive_format: "tar.gz".to_string(),
                package_root: PathBuf::from("package"),
                entrypoint: PathBuf::from("langserver.index.js"),
                runtime: LspArtifactRuntime::Node {
                    minimum_version: LspNodeVersion {
                        major: 14,
                        minor: 0,
                        patch: 0,
                    },
                },
            },
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

/// Bounded server-originated `workspace/applyEdit` request passed to an
/// explicitly installed application callback.
#[derive(Debug, Clone)]
pub struct LspApplyWorkspaceEditRequest {
    /// Original JSON-RPC request identifier, preserved for the response.
    pub json_rpc_id: u64,
    /// Bounded raw request parameters for app-owned proposal translation.
    pub params: Value,
    /// Active outgoing-request context, when the inbound request arrived while
    /// `read_response_for` was waiting for a response.
    pub context: Option<LspOperationContext>,
    /// Deadline inherited from the active worker request, if any. The app
    /// must use this same deadline for proposal authorization.
    pub deadline: Option<std::time::Instant>,
}

/// Transport-only result for an inbound `workspace/applyEdit` request.
///
/// The callback reports whether an app authority accepted the request. The
/// transport never applies the workspace edit itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LspApplyWorkspaceEditResponse {
    /// Whether the app authority accepted the edit for proposal processing.
    pub applied: bool,
    /// Bounded failure reason when `applied` is false.
    pub failure_reason: Option<String>,
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
    ) -> LspRuntimeResult<LspPendingRequest> {
        let json_rpc_id = self.next_json_rpc_id;
        // Use `checked_add` so id exhaustion is reported explicitly rather
        // than silently reusing `u64::MAX` and corrupting the correlation
        // tables on subsequent requests.
        self.next_json_rpc_id = self
            .next_json_rpc_id
            .checked_add(1)
            .ok_or(LspRuntimeError::JsonRpcIdExhausted)?;
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
        Ok(pending)
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

/// Builds a `textDocument/didClose` notification envelope.
pub fn did_close_notification(uri: &str) -> JsonRpcEnvelope {
    JsonRpcEnvelope::notification(
        "textDocument/didClose",
        json!({
            "textDocument": {
                "uri": uri,
            }
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
    // Extract LSP `insertText` only when it differs from the label and is
    // non-empty; identical values add no information.
    let insert_text_raw = item
        .get("insertText")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty() && *s != label.as_str());
    let insert_text = insert_text_raw.map(|s| bounded_lsp_label(s, 512));
    Some(LanguageCompletionProjection {
        completion_id: format!("lsp-completion-{index}-{:016x}", stable_hash(&label)),
        label,
        detail_label: detail,
        kind_label,
        score_basis_points: {
            // Compute the penalty in u32 so a large `index` cannot wrap when
            // cast to u16 before the multiply (which would let far-down
            // completions regain a top score).
            let penalty = (index as u32).saturating_mul(100);
            10_000u32.saturating_sub(penalty) as u16
        },
        degraded: item.get("insertText").is_none(),
        insert_text,
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

/// Builds a JSON-RPC `textDocument/prepareCallHierarchy` request.
pub fn prepare_call_hierarchy_request(
    id: u64,
    document: &LspTextDocumentIdentity,
    position: Utf16Position,
) -> JsonRpcEnvelope {
    JsonRpcEnvelope::request(
        id,
        "textDocument/prepareCallHierarchy",
        json!({
            "textDocument": {"uri": document.uri},
            "position": {"line": position.line, "character": position.character},
        }),
    )
}

/// Builds a JSON-RPC `callHierarchy/incomingCalls` request.
pub fn incoming_calls_request(id: u64, item: &LspCallHierarchyItem) -> JsonRpcEnvelope {
    JsonRpcEnvelope::request(
        id,
        "callHierarchy/incomingCalls",
        json!({
            "item": call_hierarchy_item_to_json(item),
        }),
    )
}

/// Builds a JSON-RPC `callHierarchy/outgoingCalls` request.
pub fn outgoing_calls_request(id: u64, item: &LspCallHierarchyItem) -> JsonRpcEnvelope {
    JsonRpcEnvelope::request(
        id,
        "callHierarchy/outgoingCalls",
        json!({
            "item": call_hierarchy_item_to_json(item),
        }),
    )
}

fn call_hierarchy_item_to_json(item: &LspCallHierarchyItem) -> Value {
    let mut obj = json!({
        "name": item.name,
        "kind": item.kind,
        "uri": item.uri,
        "range": {
            "start": {"line": item.range.start.line, "character": item.range.start.character},
            "end": {"line": item.range.end.line, "character": item.range.end.character},
        },
        "selectionRange": {
            "start": {"line": item.selection_range.start.line, "character": item.selection_range.start.character},
            "end": {"line": item.selection_range.end.line, "character": item.selection_range.end.character},
        },
    });
    if let Some(detail) = &item.detail {
        obj["detail"] = json!(detail);
    }
    if let Some(data) = &item.data {
        obj["data"] = data.clone();
    }
    obj
}

/// Converts an LSP `textDocument/prepareCallHierarchy` response into call hierarchy items.
///
/// Returns `None` when the response is `null` (call hierarchy not available at the position).
/// Returns `Some(vec![])` for an empty array response.
pub fn project_prepare_call_hierarchy_response(
    response: &Value,
) -> Option<Vec<LspCallHierarchyItem>> {
    if response.is_null() {
        return None;
    }
    let items = response.as_array()?;
    Some(
        items
            .iter()
            .filter_map(call_hierarchy_item_from_json)
            .collect(),
    )
}

/// Converts an LSP `callHierarchy/incomingCalls` response into incoming call projections.
pub fn project_incoming_calls_response(response: &Value) -> Vec<LspCallHierarchyIncomingCall> {
    let Some(calls) = response.as_array() else {
        return Vec::new();
    };
    calls
        .iter()
        .filter_map(|call| {
            let from = call.get("from").and_then(call_hierarchy_item_from_json)?;
            let from_ranges = call
                .get("fromRanges")
                .and_then(Value::as_array)
                .map(|ranges| {
                    ranges
                        .iter()
                        .filter_map(protocol_range_from_lsp_json)
                        .collect()
                })
                .unwrap_or_default();
            Some(LspCallHierarchyIncomingCall { from, from_ranges })
        })
        .collect()
}

/// Converts an LSP `callHierarchy/outgoingCalls` response into outgoing call projections.
pub fn project_outgoing_calls_response(response: &Value) -> Vec<LspCallHierarchyOutgoingCall> {
    let Some(calls) = response.as_array() else {
        return Vec::new();
    };
    calls
        .iter()
        .filter_map(|call| {
            let to = call.get("to").and_then(call_hierarchy_item_from_json)?;
            let from_ranges = call
                .get("fromRanges")
                .and_then(Value::as_array)
                .map(|ranges| {
                    ranges
                        .iter()
                        .filter_map(protocol_range_from_lsp_json)
                        .collect()
                })
                .unwrap_or_default();
            Some(LspCallHierarchyOutgoingCall { to, from_ranges })
        })
        .collect()
}

fn call_hierarchy_item_from_json(value: &Value) -> Option<LspCallHierarchyItem> {
    let name = value.get("name")?.as_str()?;
    let kind = value.get("kind")?.as_u64()?.try_into().ok()?;
    let uri = value.get("uri")?.as_str()?;
    let range = value.get("range").and_then(protocol_range_from_lsp_json)?;
    let selection_range = value
        .get("selectionRange")
        .and_then(protocol_range_from_lsp_json)?;
    let detail = value
        .get("detail")
        .and_then(Value::as_str)
        .map(|s| bounded_lsp_label(s, 160));
    let data = value.get("data").cloned();
    Some(LspCallHierarchyItem {
        name: bounded_lsp_label(name, 120),
        kind,
        uri: uri.to_string(),
        range,
        selection_range,
        detail,
        data,
    })
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

/// Builds a JSON-RPC `workspace/executeCommand` request.
///
/// Used to execute commands returned by code action responses. The `command`
/// string is the server-provided command identifier; `arguments` are opaque
/// values forwarded from the code action's `command.arguments` array.
pub fn execute_command_request(
    id: u64,
    command: impl Into<String>,
    arguments: Vec<Value>,
) -> JsonRpcEnvelope {
    JsonRpcEnvelope::request(
        id,
        "workspace/executeCommand",
        json!({
            "command": command.into(),
            "arguments": arguments,
        }),
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
    let uri = location
        .get("uri")
        .or_else(|| location.get("targetUri"))
        .and_then(Value::as_str);
    let uri_hash = uri.map(stable_hash).unwrap_or(0);
    let path = uri.and_then(path_from_file_uri);
    Some(LanguageLocationProjection {
        location_id: format!(
            "lsp-workspace-symbol-{index}-{:016x}-{:016x}",
            stable_hash(&label),
            uri_hash
        ),
        file_id: None,
        path,
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
    // A runnable lens is the one case where `command_label` must not be the LSP
    // command id: `ActivateLanguageCodeLens` hands that label to the terminal,
    // and "rust-analyzer.runSingle" is not something a shell can run. When the
    // lens carries runnable arguments, the label becomes the cargo invocation
    // they describe.
    let runnable = runnable_command_line(command);
    let command_label = runnable.clone().unwrap_or_else(|| {
        command
            .and_then(|command| command.get("command"))
            .and_then(Value::as_str)
            .map(|command| bounded_lsp_label(command, 120))
            .unwrap_or_else(|| "lsp.codelens.unresolved".to_string())
    });
    let data_kind = lens
        .get("data")
        .and_then(|data| data.get("kind"))
        .and_then(Value::as_str);
    let kind_label = if runnable.is_some() {
        // The marker `AppComposition::ActivateLanguageCodeLens` gates on before
        // it will launch anything.
        "lsp.codelens.runnable".to_string()
    } else {
        data_kind
            .map(|kind| format!("lsp.codelens.{}", bounded_lsp_label(kind, 80)))
            .unwrap_or_else(|| {
                if command.is_some() {
                    "lsp.codelens.command".to_string()
                } else {
                    "lsp.codelens.unresolved".to_string()
                }
            })
    };
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

/// The cargo command a rust-analyzer runnable lens describes, if it is one.
///
/// rust-analyzer publishes Run and Debug as code lenses whose command is
/// `rust-analyzer.runSingle` (or `debugSingle`) with a single argument carrying
/// `args: { cargoArgs, executableArgs }`. Those arrays are the actual
/// invocation; the command id is only a handle for a client that speaks
/// rust-analyzer's private protocol, and putting the handle in a field named
/// `command_label` makes the lens describe itself wrongly wherever it is shown
/// or recorded.
///
/// **Nothing executes this string today.** `ActivateLanguageCodeLens` and the
/// test explorer both hand it to `TerminalWorkflow::launch`, which spawns the
/// configured shell and uses the label only for display and audit. The value of
/// building it correctly is that the lens says what it is; the value of the
/// check below is that it stays safe on the day something does run it.
///
/// That check is a refusal, not an escape. Escaping shell metacharacters is a
/// game the defender loses eventually, and a legitimate cargo argument contains
/// none of them — so an element carrying a shell metacharacter or a control
/// character means the lens is not a runnable, and it falls through to the
/// ordinary path where the command id is displayed and nothing is claimed about
/// it. The right long-term shape is an argv vector executed without a shell at
/// all, which makes the question moot; until a caller exists to consume one,
/// refusing is what can be proven.
///
/// Returns `None` for any lens that is not a runnable, which is most of them.
fn runnable_command_line(command: Option<&Value>) -> Option<String> {
    let command = command?;
    let name = command.get("command").and_then(Value::as_str)?;
    if !name.starts_with("rust-analyzer.run") && !name.starts_with("rust-analyzer.debug") {
        return None;
    }
    let args = command
        .get("arguments")
        .and_then(Value::as_array)?
        .first()?
        .get("args")?;

    let cargo_args = string_list(args.get("cargoArgs"));
    if cargo_args.is_empty() {
        return None;
    }
    let executable_args = string_list(args.get("executableArgs"));

    let mut parts = vec!["cargo".to_string()];
    parts.extend(cargo_args);
    if !executable_args.is_empty() {
        parts.push("--".to_string());
        parts.extend(executable_args);
    }
    if parts.iter().any(|part| !is_plain_command_argument(part)) {
        return None;
    }
    Some(bounded_lsp_label(&parts.join(" "), 240))
}

/// Whether an argument is one a shell would pass through untouched.
///
/// Deliberately a whitelist of what a cargo argument actually contains rather
/// than a blacklist of what a shell reacts to: a blacklist is only as good as
/// its author's memory of every metacharacter, and this one has to hold against
/// a language server that may be hostile.
///
/// Rejects control characters (newline included, which is how one command
/// becomes two) and every shell metacharacter. Accepts what real cargo
/// arguments are made of: identifiers, paths, versions, feature lists,
/// `--flags`, and the `::` of a Rust test path.
fn is_plain_command_argument(argument: &str) -> bool {
    !argument.is_empty()
        && argument
            .chars()
            .all(|c| !c.is_control() && (c.is_ascii_alphanumeric() || "-_./:=+@,[]".contains(c)))
}

/// The string elements of a JSON array, bounded individually.
fn string_list(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .take(32)
                .map(|item| bounded_lsp_label(item, 80))
                .collect()
        })
        .unwrap_or_default()
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
    let path = path_from_file_uri(uri);
    let label = path
        .as_ref()
        .map(|p| {
            p.0.rsplit(['/', '\\'])
                .next()
                .filter(|s| !s.is_empty())
                .unwrap_or(p.0.as_str())
                .to_string()
        })
        .map(|name| format!("LSP location {index}: {name}"))
        .unwrap_or_else(|| format!("LSP location {index}"));
    let degraded = range.is_none() || path.is_none();
    Some(LanguageLocationProjection {
        location_id: format!("lsp-location-{index}-{:016x}", stable_hash(uri)),
        file_id: None,
        path,
        range,
        label,
        // Degraded when range *or* navigable path is missing (cross-file nav needs path).
        degraded,
        schema_version: 1,
    })
}

// ---------------------------------------------------------------------------
// Write-side response projections (format, prepareRename, code action).
//
// Write-side actions produce edits that feed into the proposal lifecycle,
// not display metadata. These projections bridge raw LSP JSON responses
// to structured types that the proposal pipeline can consume.
// ---------------------------------------------------------------------------

/// Converts an LSP formatting response (`TextEdit[]` or `null`) into a
/// proposal-ready workspace edit for a single document.
///
/// The caller supplies the document URI so the resulting edits can be
/// associated with the formatted file. Returns `None` when the response
/// is `null`, empty, or contains no parseable text edits.
pub fn project_formatting_response(
    response: &Value,
    uri: &str,
) -> Option<features::LspWorkspaceEditProposal> {
    let edits_array = response.as_array()?;
    if edits_array.is_empty() {
        return None;
    }
    let text_edits: Vec<features::LspTextEdit> = edits_array
        .iter()
        .filter_map(|edit| {
            let range = edit.get("range").and_then(protocol_range_from_lsp_json)?;
            let new_text = edit.get("newText")?.as_str()?.to_string();
            Some(features::LspTextEdit { range, new_text })
        })
        .collect();
    if text_edits.is_empty() {
        return None;
    }
    Some(features::LspWorkspaceEditProposal {
        label: "Format document".to_string(),
        file_edits: vec![features::LspFileEdit {
            uri: uri.to_string(),
            edits: text_edits,
        }],
    })
}

/// Converts an LSP `textDocument/prepareRename` response into a structured
/// prepare-rename result.
///
/// The LSP spec defines three response shapes:
/// - `Range` (the range that may be renamed)
/// - `{ range: Range, placeholder: string }` (range with placeholder text)
/// - `{ defaultBehavior: boolean }` (server defers to client word-boundary logic)
///
/// Returns `None` for `null` responses (rename not available at the position)
/// and for `{ defaultBehavior: false }`.
///
/// The `{ defaultBehavior: true }` shape cannot produce a valid range without
/// cursor context, so it returns `None`; callers should fall through to a
/// rename request using the word at the cursor position.
pub fn project_prepare_rename_response(response: &Value) -> Option<LspPrepareRenameResult> {
    if response.is_null() {
        return None;
    }
    // Shape: `{ range, placeholder }`.
    if let Some(range_json) = response.get("range") {
        let range = protocol_range_from_lsp_json(range_json)?;
        let placeholder = response
            .get("placeholder")
            .and_then(Value::as_str)
            .map(|s| bounded_lsp_label(s, 120));
        return Some(LspPrepareRenameResult {
            range,
            placeholder,
            allowed: true,
        });
    }
    // Shape: `{ defaultBehavior: bool }`.
    if response.get("defaultBehavior").is_some() {
        // Cannot produce a valid range without cursor context.
        return None;
    }
    // Shape: bare `Range` (start + end directly on the response object).
    let range = protocol_range_from_lsp_json(response)?;
    Some(LspPrepareRenameResult {
        range,
        placeholder: None,
        allowed: true,
    })
}

/// Metadata-only projection of a single LSP code-action candidate.
///
/// This is the write-side counterpart of read-side projection rows: it
/// surfaces enough metadata for the UI to display available actions without
/// including the full workspace edit or command payload. The full payloads
/// remain in the raw JSON for the app layer to translate through the
/// proposal pipeline when the user selects an action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LspCodeActionMetadata {
    /// Stable identifier for this action within the response.
    pub action_id: String,
    /// User-visible action title.
    pub title: String,
    /// Server-provided action kind (e.g., `"quickfix"`, `"refactor.extract"`).
    pub kind: Option<String>,
    /// Whether the server marked this as a preferred action.
    pub is_preferred: bool,
    /// Whether the action carries a workspace edit.
    pub has_edit: bool,
    /// Whether the action carries a command.
    pub has_command: bool,
    /// Disabled reason, if the action is disabled.
    pub disabled_reason: Option<String>,
    /// Schema version.
    pub schema_version: u16,
}

/// Converts an LSP `textDocument/codeAction` response into metadata-only
/// projections of available actions.
///
/// Each item in the response may be a `Command` (title + command + arguments)
/// or a `CodeAction` (title + kind + optional edit + optional command).
/// This projection extracts metadata only; the raw JSON is preserved for
/// the app layer to translate edits through the proposal pipeline.
pub fn project_code_action_response(response: &Value, limit: usize) -> Vec<LspCodeActionMetadata> {
    let Some(actions) = response.as_array() else {
        return Vec::new();
    };
    actions
        .iter()
        .take(limit)
        .enumerate()
        .filter_map(|(index, action)| code_action_metadata_for_item(index, action))
        .collect()
}

fn code_action_metadata_for_item(index: usize, action: &Value) -> Option<LspCodeActionMetadata> {
    let title = action.get("title")?.as_str()?;
    let title = bounded_lsp_label(title, 120);

    // Distinguish Command vs CodeAction: a Command has `command` as a string
    // at the top level; a CodeAction has `command` as an object.
    let is_command_shape = action.get("command").is_some_and(|c| c.is_string());

    if is_command_shape {
        // Pure Command shape: { title, command, arguments? }
        return Some(LspCodeActionMetadata {
            action_id: format!("lsp-action-{index}-{:016x}", stable_hash(&title)),
            title,
            kind: None,
            is_preferred: false,
            has_edit: false,
            has_command: true,
            disabled_reason: None,
            schema_version: 1,
        });
    }

    // CodeAction shape.
    let kind = action
        .get("kind")
        .and_then(Value::as_str)
        .map(|k| bounded_lsp_label(k, 80));
    let is_preferred = action
        .get("isPreferred")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let has_edit = action.get("edit").is_some();
    let has_command = action.get("command").is_some();
    let disabled_reason = action
        .get("disabled")
        .and_then(|d| d.get("reason"))
        .and_then(Value::as_str)
        .map(|r| bounded_lsp_label(r, 160));

    Some(LspCodeActionMetadata {
        action_id: format!("lsp-action-{index}-{:016x}", stable_hash(&title)),
        title,
        kind,
        is_preferred,
        has_edit,
        has_command,
        disabled_reason,
        schema_version: 1,
    })
}

/// Converts an LSP rename response (`WorkspaceEdit` or `null`) into a
/// proposal-ready workspace edit.
///
/// This is a thin delegation to
/// [`features::LspWorkspaceEditProposal::from_workspace_edit_json`] for
/// consistency with the other `project_*` response functions in this module.
pub fn project_rename_response(
    response: &Value,
    label: &str,
) -> Option<features::LspWorkspaceEditProposal> {
    if response.is_null() {
        return None;
    }
    features::LspWorkspaceEditProposal::from_workspace_edit_json(response, label.to_string())
}

/// Convert an LSP `file://` URI into a filesystem path for navigation.
///
/// Returns [`None`] for non-`file` schemes and for non-local authorities that
/// would be mis-mapped to local paths. Windows drive designators are normalized
/// via [`normalize_file_uri_drive`] before decoding. Percent-encoded octets
/// (`%20`, `%3A`) are decoded so desktop open/navigation can use the path.
///
/// UNC-style authorities (`file://server/share/path`) are preserved as
/// `//server/share/path` so Windows remote shares remain navigable.
pub fn path_from_file_uri(uri: &str) -> Option<CanonicalPath> {
    let normalized = normalize_file_uri_drive(uri);
    let decoded = percent_decode_uri_component(normalized.as_ref());
    let path = if let Some(rest) = decoded.strip_prefix("file:///") {
        // `file:///C:/...` (Windows) or `file:///home/...` (Unix absolute).
        if rest.len() >= 2
            && rest.as_bytes()[0].is_ascii_alphabetic()
            && rest.as_bytes().get(1) == Some(&b':')
        {
            rest.to_string()
        } else {
            format!("/{rest}")
        }
    } else {
        // `file://server/share/...` or `file://localhost/path`.
        let rest = decoded.strip_prefix("file://")?;
        let slash = rest.find('/')?;
        let authority = &rest[..slash];
        let path_part = &rest[slash..];
        if authority.is_empty()
            || authority.eq_ignore_ascii_case("localhost")
            || authority == "127.0.0.1"
            || authority == "[::1]"
        {
            // Local authority — treat as absolute local path.
            path_part.to_string()
        } else if authority.contains('.')
            || authority.contains(':')
            || authority.eq_ignore_ascii_case("http")
        {
            // Reject non-local / URL-like authorities rather than mis-map them.
            return None;
        } else {
            // UNC host (e.g. file://fileserver/share/src/lib.rs).
            format!("//{authority}{path_part}")
        }
    };
    if path.is_empty() || path == "/" {
        return None;
    }
    Some(CanonicalPath(path))
}

fn percent_decode_uri_component(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let (Some(hi), Some(lo)) = (hex_nibble(bytes[i + 1]), hex_nibble(bytes[i + 2]))
        {
            out.push((hi << 4) | lo);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
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

/// Canonicalize the hexadecimal casing of valid percent escapes in a file URI.
///
/// URI path spelling and literal percent characters remain unchanged; only a
/// `%` followed by two hexadecimal digits is rewritten to uppercase hex. Other
/// URI schemes are returned unchanged.
pub fn normalize_file_uri_percent_escapes(uri: &str) -> std::borrow::Cow<'_, str> {
    use std::borrow::Cow;
    if !uri.starts_with("file://") {
        return Cow::Borrowed(uri);
    }
    let bytes = uri.as_bytes();
    let mut normalized = None;
    for index in 0..bytes.len().saturating_sub(2) {
        if bytes[index] != b'%'
            || !bytes[index + 1].is_ascii_hexdigit()
            || !bytes[index + 2].is_ascii_hexdigit()
        {
            continue;
        }
        let output = normalized.get_or_insert_with(|| uri.to_string());
        output.replace_range(
            index + 1..index + 3,
            &uri[index + 1..index + 3].to_ascii_uppercase(),
        );
    }
    normalized.map_or(Cow::Borrowed(uri), Cow::Owned)
}

/// Normalize the Windows drive designator of a `file:///` URI after
/// canonicalizing percent-escape hex casing.
pub fn normalize_file_uri_drive(uri: &str) -> std::borrow::Cow<'_, str> {
    use std::borrow::Cow;
    let normalized = normalize_file_uri_percent_escapes(uri);
    let Some(rest) = normalized.strip_prefix("file:///") else {
        return normalized;
    };
    let bytes = rest.as_bytes();
    if bytes.is_empty() || !bytes[0].is_ascii_alphabetic() {
        return normalized;
    }
    let drive = bytes[0].to_ascii_lowercase() as char;
    // `X:` form — a path segment is only a drive designator with the colon.
    if bytes.len() >= 2 && bytes[1] == b':' {
        if bytes[0].is_ascii_uppercase() {
            return Cow::Owned(format!("file:///{drive}{}", &rest[1..]));
        }
        return normalized;
    }
    // `X%3A` form (percent-encoded colon, either hex case).
    if rest.len() >= 4 && rest[1..4].eq_ignore_ascii_case("%3a") {
        return Cow::Owned(format!("file:///{drive}:{}", &rest[4..]));
    }
    normalized
}

/// Returns the stable fingerprint used to key `publishDiagnostics` records by URI.
///
/// Callers outside this crate (e.g. `legion-app`) use this to filter
/// [`LspDiagnosticNotificationMetadata`] by document URI without storing the raw
/// URI string in the metadata record. The fingerprint matches the `uri_hash`
/// field produced by the LSP transport when recording a notification.
///
/// The URI's Windows drive designator is normalized before hashing (see
/// [`normalize_file_uri_drive`]) so the client's opened form and the
/// server's echoed form always produce the same fingerprint.
pub fn lsp_diagnostic_uri_fingerprint(uri: &str) -> FileFingerprint {
    metadata_fingerprint("lsp.diagnostic.uri", &normalize_file_uri_drive(uri))
}

// -----------------------------------------------------------------------------
// Process-backed stdio transport (WS03.T1).
// -----------------------------------------------------------------------------

/// Count cap on parsed envelopes waiting between the stdout reader and the
/// session. Memory is bounded separately by
/// [`STDOUT_READER_QUEUED_BYTE_BUDGET`].
pub const STDOUT_READER_QUEUE_CAP: usize = 8;

/// Maximum parsed payload bytes retained in the stdout mailbox.
///
/// One in-flight frame may exceed this when the mailbox is empty so a single
/// large response (up to [`LspFramer::MAX_FRAME_PAYLOAD_BYTES`]) can still
/// be delivered. Combined with the count cap this replaces an unbounded
/// `mpsc` that could retain every boxed 64 MiB envelope.
pub const STDOUT_READER_QUEUED_BYTE_BUDGET: usize = 2 * 1024 * 1024;

/// How long teardown waits for the stdout reader after dropping the mailbox
/// and signalling the process tree. A grandchild that still holds stdout
/// after a failed tree kill must not hang session reset forever.
const STDOUT_READER_JOIN_TIMEOUT: Duration = Duration::from_secs(2);

/// Message produced by the background stdout reader thread and consumed by
/// the session on the request-driving thread. Carrying parsed frames over a
/// channel decouples the (blocking) pipe read from the caller, so a fully
/// silent server can be bounded with [`Receiver::recv_timeout`] instead of
/// blocking a pipe read that `std` cannot time out.
enum StdoutReaderEvent {
    /// A successfully framed and parsed JSON-RPC envelope.
    Frame {
        envelope: Box<JsonRpcEnvelope>,
        payload_bytes: usize,
    },
    /// The peer closed stdout cleanly (clean EOF between frames).
    Eof,
    /// A framing or parse error terminated the reader.
    Err(Box<LspRuntimeError>),
}

/// Credits for parsed payload sitting in the stdout mailbox.
struct MailboxBudget {
    queued_bytes: Mutex<usize>,
    cv: Condvar,
}

impl MailboxBudget {
    fn new() -> Self {
        Self {
            queued_bytes: Mutex::new(0),
            cv: Condvar::new(),
        }
    }

    fn lock_queued_bytes(&self) -> std::sync::MutexGuard<'_, usize> {
        self.queued_bytes
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Reserve `n` bytes. An empty mailbox always accepts one frame so a
    /// single large response can still be delivered. Returns `false` when
    /// teardown has asked the reader to stop.
    fn acquire(&self, n: usize, stop: &AtomicBool) -> bool {
        let mut guard = self.lock_queued_bytes();
        loop {
            if stop.load(Ordering::Acquire) {
                return false;
            }
            if *guard == 0 || *guard + n <= STDOUT_READER_QUEUED_BYTE_BUDGET {
                *guard = guard.saturating_add(n);
                return true;
            }
            let (next, _) = self
                .cv
                .wait_timeout(guard, Duration::from_millis(50))
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            guard = next;
        }
    }

    fn release(&self, n: usize) {
        let mut guard = self.lock_queued_bytes();
        *guard = guard.saturating_sub(n);
        self.cv.notify_all();
    }

    fn wake(&self) {
        self.cv.notify_all();
    }
}

/// Background reader that owns the child's stdout pipe and forwards parsed
/// frames over a channel. The thread terminates after the first `Eof` or
/// `Err`, or once the receiver is dropped.
struct StdoutReader {
    rx: Receiver<StdoutReaderEvent>,
    handle: Option<JoinHandle<()>>,
}

/// How the stdout reader thread terminated (PKT-S3-WEDGE-R3).
///
/// The reader thread exits after forwarding its first terminal event. Before
/// this was recorded, a dead reader was indistinguishable from a silent
/// server: `try_recv_envelope` maps both `Disconnected` and `Eof` to `None`,
/// so a session whose reader died reported "no frames" forever — the GP-1 s3
/// wedge signature (total silence, empty stderr ring, writes still succeed).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LspReaderTerminal {
    /// The peer closed stdout cleanly (EOF between frames).
    Eof,
    /// A framing or JSON parse error terminated the reader. Carries the
    /// error's display message — error metadata only, never raw payload
    /// bytes.
    Error(String),
}

/// Point-in-time snapshot of the stdout reader thread's counters.
///
/// Written by the reader thread, read by post-mortem diagnostics (GP-1 s3)
/// to discriminate "server truly silent" (reader alive, zero frames) from
/// "our reader died" (terminal event recorded, child possibly still alive).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LspReaderStatsSnapshot {
    /// Frames successfully parsed and forwarded to the session channel.
    pub frames_forwarded: u64,
    /// Total payload bytes across forwarded frames (headers excluded).
    pub payload_bytes: u64,
    /// Terminal event that ended the reader thread, if it has ended.
    pub terminal: Option<LspReaderTerminal>,
    /// Teardown could not confirm that descendants holding stdout were killed.
    pub tree_kill_failed: bool,
}

/// Shared counters written by the reader thread and snapshot by the session.
/// Lives on [`LspStdioProcess`] (not [`StdoutReader`]) so the snapshot
/// survives `kill()` taking the reader down.
#[derive(Debug, Default)]
struct ReaderStatsShared {
    frames_forwarded: AtomicU64,
    payload_bytes: AtomicU64,
    terminal: Mutex<Option<LspReaderTerminal>>,
    tree_kill_failed: AtomicBool,
}

impl ReaderStatsShared {
    fn snapshot(&self) -> LspReaderStatsSnapshot {
        LspReaderStatsSnapshot {
            frames_forwarded: self.frames_forwarded.load(Ordering::Acquire),
            payload_bytes: self.payload_bytes.load(Ordering::Acquire),
            terminal: self.terminal.lock().map_or(None, |slot| slot.clone()),
            tree_kill_failed: self.tree_kill_failed.load(Ordering::Acquire),
        }
    }

    #[cfg_attr(not(windows), allow(dead_code))]
    fn record_tree_kill_failure(&self) {
        self.tree_kill_failed.store(true, Ordering::Release);
    }
}

/// Outcome of a deadline-bounded read on [`LspStdioProcess::read_envelope_until`].
pub enum LspReadOutcome {
    /// A framed envelope was read before the deadline.
    Envelope(JsonRpcEnvelope),
    /// The peer closed stdout cleanly before the deadline.
    Eof,
    /// The deadline elapsed before a frame arrived. The process is left
    /// running; the caller decides how to resolve the in-flight request.
    TimedOut,
}

/// Owned handle to a launched `std::process::Child` running an LSP server
/// on piped stdio.
///
/// The handle implements [`LspProcessHandle`] so the existing
/// [`LspSupervisor`] can reuse it for metadata-only lifecycle bookkeeping,
/// and additionally exposes the buffered stdin pipe plus a deadline-bounded
/// frame reader so the higher-level [`LspStdioSession`] can read and write
/// Content-Length framed JSON-RPC messages without a silent server blocking
/// the caller forever. The stdout pipe is read on a dedicated background
/// thread that forwards parsed frames over a channel; the session waits on
/// that channel with a deadline. The stderr pipe is captured so the
/// supervisor/drain loop can read it independently; it is intentionally
/// metadata-only and never propagated into response payloads.
pub struct LspStdioProcess {
    child: Option<Child>,
    stdin: Option<ChildStdin>,
    stderr: Option<ChildStderr>,
    reader: Option<StdoutReader>,
    reader_stats: Arc<ReaderStatsShared>,
    mailbox_budget: Arc<MailboxBudget>,
    reader_stop: Arc<AtomicBool>,
    /// `true` only when this handle spawned the child in its own process
    /// group via [`spawn_stdio_child`]. [`Self::new`] wraps an arbitrary
    /// `Child` and must not SIGKILL `-pid` as if it were a group leader.
    /// Windows stores that ownership on `windows_job` instead.
    #[cfg(unix)]
    owns_process_group: bool,
    #[cfg(windows)]
    windows_job: Option<WindowsStdioJob>,
    killed: bool,
}

impl LspStdioProcess {
    /// Wraps an already-spawned child with captured pipes.
    ///
    /// This constructor does **not** put the child in its own process group.
    /// Teardown therefore kills only the direct child on Unix. Prefer
    /// [`LspStdioLauncher`] when the handle should own the descendant tree.
    pub fn new(child: Child) -> LspRuntimeResult<Self> {
        Self::from_child(child, false)
    }

    fn from_supervised_child(child: Child) -> LspRuntimeResult<Self> {
        Self::from_child(child, true)
    }

    fn from_child(child: Child, owns_process_group: bool) -> LspRuntimeResult<Self> {
        let mut child = child;
        let stdin = child.stdin.take().ok_or(LspRuntimeError::StdioIo {
            message: "child stdin unavailable".to_string(),
        })?;
        let stdout = child.stdout.take().ok_or(LspRuntimeError::StdioIo {
            message: "child stdout unavailable".to_string(),
        })?;
        let stderr = child.stderr.take();
        let reader_stats = Arc::new(ReaderStatsShared::default());
        let mailbox_budget = Arc::new(MailboxBudget::new());
        let reader_stop = Arc::new(AtomicBool::new(false));
        let reader = StdoutReader::spawn(
            stdout,
            Arc::clone(&reader_stats),
            Arc::clone(&mailbox_budget),
            Arc::clone(&reader_stop),
        )?;
        #[cfg(windows)]
        let windows_job = if owns_process_group {
            assign_windows_stdio_job(&child)
        } else {
            None
        };
        Ok(Self {
            child: Some(child),
            stdin: Some(stdin),
            stderr,
            reader: Some(reader),
            reader_stats,
            mailbox_budget,
            reader_stop,
            #[cfg(unix)]
            owns_process_group,
            #[cfg(windows)]
            windows_job,
            killed: false,
        })
    }

    fn release_mailbox(&self, event: &StdoutReaderEvent) {
        if let StdoutReaderEvent::Frame { payload_bytes, .. } = event {
            self.mailbox_budget.release(*payload_bytes);
        }
    }

    /// Snapshot of the stdout reader thread's counters (frames forwarded,
    /// payload bytes, terminal event). Valid even after `kill()`.
    pub fn reader_stats(&self) -> LspReaderStatsSnapshot {
        self.reader_stats.snapshot()
    }

    /// Returns the child's exit status rendered as a string once it has
    /// terminated (`None` while it is still running or when the status
    /// cannot be collected). Post-mortem evidence: a panic (101), a signal,
    /// and a clean exit (0) point at different death modes
    /// (PKT-S3-WEDGE-R3).
    pub fn exit_status_string(&mut self) -> Option<String> {
        self.child
            .as_mut()
            .and_then(|child| match child.try_wait() {
                Ok(Some(status)) => Some(status.to_string()),
                _ => None,
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
    /// child's stdout, blocking until a frame, clean EOF, or a framing error.
    ///
    /// Returns `Ok(None)` on clean EOF (peer closed the stream) so callers
    /// can distinguish a graceful shutdown from a framing error.
    ///
    /// This blocks indefinitely on a silent server; prefer
    /// [`Self::read_envelope_until`] when an unresponsive server must be
    /// bounded by a deadline.
    pub fn read_envelope(&mut self) -> LspRuntimeResult<Option<JsonRpcEnvelope>> {
        let event = {
            let reader = self
                .reader
                .as_ref()
                .ok_or(LspRuntimeError::SessionNotRunning)?;
            match reader.rx.recv() {
                Ok(event) => event,
                // The reader thread ended without a terminal message we observed
                // (e.g. it was already drained); treat a closed channel as EOF.
                Err(_) => return Ok(None),
            }
        };
        self.release_mailbox(&event);
        match event {
            StdoutReaderEvent::Frame { envelope, .. } => Ok(Some(*envelope)),
            StdoutReaderEvent::Eof => Ok(None),
            StdoutReaderEvent::Err(err) => Err(*err),
        }
    }

    /// Reads the next framed JSON-RPC envelope, returning
    /// [`LspReadOutcome::TimedOut`] if `deadline` elapses before a frame
    /// arrives. Unlike [`Self::read_envelope`], this bounds the wait even
    /// when the server goes fully silent, because the actual blocking pipe
    /// read happens on a background thread and this only waits on a channel.
    pub fn read_envelope_until(&mut self, deadline: Instant) -> LspRuntimeResult<LspReadOutcome> {
        let remaining = deadline.saturating_duration_since(Instant::now());
        let event = {
            let reader = self
                .reader
                .as_ref()
                .ok_or(LspRuntimeError::SessionNotRunning)?;
            match reader.rx.recv_timeout(remaining) {
                Ok(event) => event,
                Err(RecvTimeoutError::Timeout) => return Ok(LspReadOutcome::TimedOut),
                // Reader thread ended; treat a closed channel as EOF.
                Err(RecvTimeoutError::Disconnected) => return Ok(LspReadOutcome::Eof),
            }
        };
        self.release_mailbox(&event);
        match event {
            StdoutReaderEvent::Frame { envelope, .. } => Ok(LspReadOutcome::Envelope(*envelope)),
            StdoutReaderEvent::Eof => Ok(LspReadOutcome::Eof),
            StdoutReaderEvent::Err(err) => Err(*err),
        }
    }

    /// Non-blocking check for a pending frame in the reader channel.
    ///
    /// Returns `Some(envelope)` if a frame is immediately available, or `None`
    /// if the channel is empty.  Never blocks.  Callers that need this for
    /// diagnostic notification draining (e.g. the session worker thread) should
    /// call this in a loop, breaking on `None`.
    pub fn try_recv_envelope(&mut self) -> Option<LspRuntimeResult<JsonRpcEnvelope>> {
        let event = {
            let reader = self.reader.as_ref()?;
            match reader.rx.try_recv() {
                Ok(event) => event,
                Err(mpsc::TryRecvError::Empty) | Err(mpsc::TryRecvError::Disconnected) => {
                    return None;
                }
            }
        };
        self.release_mailbox(&event);
        match event {
            StdoutReaderEvent::Frame { envelope, .. } => Some(Ok(*envelope)),
            StdoutReaderEvent::Eof => None,
            StdoutReaderEvent::Err(err) => Some(Err(*err)),
        }
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
        self.reader_stop.store(true, Ordering::Release);
        self.mailbox_budget.wake();
        if let Some(child) = self.child.as_mut() {
            // Kill the process group (Unix, when we created it) or the Job
            // Object / process tree (Windows) before joining the stdout
            // reader. A grandchild that inherited stdout keeps the pipe open
            // after `Child::kill`, so `read_lsp_frame` never returns.
            #[cfg(windows)]
            terminate_stdio_process_tree(child, self.windows_job.take(), &self.reader_stats);
            #[cfg(unix)]
            terminate_stdio_process_tree(child, self.owns_process_group);
        }
        self.stdin.take();
        self.stderr.take();
        // Drop the mailbox *before* joining. A reader parked on `send` after
        // the count cap is reached is only unblocked when `rx` is dropped.
        // Joining first deadlocks teardown.
        if let Some(reader) = self.reader.take() {
            let StdoutReader { rx, handle } = reader;
            drop(rx);
            if let Some(handle) = handle {
                join_stdout_reader(handle, &self.reader_stats);
            }
        }
    }
}

impl StdoutReader {
    /// Spawns a background thread that reads Content-Length framed JSON-RPC
    /// envelopes from `stdout` and forwards them over a channel until clean
    /// EOF or a framing/parse error. Counters and the terminal event are
    /// recorded in `stats` so a dead reader is observable after the fact
    /// (PKT-S3-WEDGE-R3).
    fn spawn(
        stdout: ChildStdout,
        stats: Arc<ReaderStatsShared>,
        budget: Arc<MailboxBudget>,
        stop: Arc<AtomicBool>,
    ) -> LspRuntimeResult<Self> {
        let (tx, rx) = mpsc::sync_channel(STDOUT_READER_QUEUE_CAP);
        let handle = std::thread::Builder::new()
            .name("legion-lsp-stdout-reader".to_string())
            .spawn(move || {
                let mut reader = BufReader::new(stdout);
                loop {
                    if stop.load(Ordering::Acquire) {
                        return;
                    }
                    let event = match read_lsp_frame(&mut reader) {
                        Ok(Some(payload)) => match serde_json::from_slice(&payload) {
                            Ok(envelope) => {
                                if !budget.acquire(payload.len(), &stop) {
                                    return;
                                }
                                stats.frames_forwarded.fetch_add(1, Ordering::AcqRel);
                                stats
                                    .payload_bytes
                                    .fetch_add(payload.len() as u64, Ordering::AcqRel);
                                StdoutReaderEvent::Frame {
                                    envelope: Box::new(envelope),
                                    payload_bytes: payload.len(),
                                }
                            }
                            Err(err) => StdoutReaderEvent::Err(Box::new(err.into())),
                        },
                        Ok(None) => StdoutReaderEvent::Eof,
                        Err(err) => StdoutReaderEvent::Err(Box::new(err)),
                    };
                    // Record the terminal event BEFORE forwarding it, so any
                    // consumer that observes the channel event also observes
                    // the stats.
                    let terminal_event = match &event {
                        StdoutReaderEvent::Frame { .. } => None,
                        StdoutReaderEvent::Eof => Some(LspReaderTerminal::Eof),
                        StdoutReaderEvent::Err(err) => {
                            Some(LspReaderTerminal::Error(err.to_string()))
                        }
                    };
                    let terminal = terminal_event.is_some();
                    if let Some(terminal_event) = terminal_event
                        && let Ok(mut slot) = stats.terminal.lock()
                    {
                        *slot = Some(terminal_event);
                    }
                    // Blocking send applies backpressure when the session is
                    // not draining. Dropping the receiver (session kill)
                    // unblocks this with an error so the thread can exit.
                    let payload_bytes = match &event {
                        StdoutReaderEvent::Frame { payload_bytes, .. } => Some(*payload_bytes),
                        _ => None,
                    };
                    if tx.send(event).is_err() || terminal {
                        if let Some(payload_bytes) = payload_bytes {
                            budget.release(payload_bytes);
                        }
                        return;
                    }
                }
            })
            .map_err(|err| LspRuntimeError::StdioIo {
                message: format!("spawn stdout reader thread: {err}"),
            })?;
        Ok(Self {
            rx,
            handle: Some(handle),
        })
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
        LspStdioProcess::from_supervised_child(child)
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
    // Put the child in its own process group so teardown can SIGKILL
    // descendants that inherited stdout (see `terminate_stdio_process_tree`).
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    command.spawn().map_err(|err| LspRuntimeError::SpawnFailed {
        code: format!("stdio.spawn_failed: {err}"),
    })
}

/// Forcibly terminates the stdio child and any descendants that still hold
/// the inherited stdout write end, then reaps the direct child.
#[cfg(unix)]
fn terminate_stdio_process_tree(child: &mut Child, owns_process_group: bool) {
    if owns_process_group {
        // Negative pid addresses the group this handle created in
        // `spawn_stdio_child`. Do not signal `-pid` for a `Child` wrapped
        // by [`LspStdioProcess::new`]; that process is not a group leader.
        let pid = child.id() as i32;
        let _ = nix::sys::signal::kill(
            nix::unistd::Pid::from_raw(-pid),
            nix::sys::signal::Signal::SIGKILL,
        );
    }
    let _ = child.kill();
    let _ = child.wait();
}

#[cfg(windows)]
fn terminate_stdio_process_tree(
    child: &mut Child,
    job: Option<WindowsStdioJob>,
    stats: &ReaderStatsShared,
) {
    let had_spawn_job = job.is_some();
    // Closing a KILL_ON_JOB_CLOSE job takes down every descendant that
    // inherited membership from the supervised spawn.
    drop(job);
    if !had_spawn_job && !terminate_windows_process_tree(child.id()) {
        // taskkill.exe missing, spawn denied, or non-zero exit: last-resort
        // Job Object covers the direct child only. Existing grandchildren
        // are not pulled in, so the tree kill stays unconfirmed.
        drop(assign_windows_stdio_job(child));
        stats.record_tree_kill_failure();
    }
    let _ = child.kill();
    let _ = child.wait();
}

/// Joins the stdout reader, giving up after [`STDOUT_READER_JOIN_TIMEOUT`]
/// so a failed tree-kill cannot hang session reset.
fn join_stdout_reader(handle: JoinHandle<()>, stats: &ReaderStatsShared) {
    let (done_tx, done_rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = handle.join();
        let _ = done_tx.send(());
    });
    if done_rx.recv_timeout(STDOUT_READER_JOIN_TIMEOUT).is_err()
        && stats.tree_kill_failed.load(Ordering::Acquire)
        && let Ok(mut slot) = stats.terminal.lock()
        && slot.is_none()
    {
        *slot = Some(LspReaderTerminal::Error(
            "stdout reader join timed out after unconfirmed process-tree kill".to_string(),
        ));
    }
}

#[cfg(windows)]
fn terminate_windows_process_tree(pid: u32) -> bool {
    let taskkill = std::env::var_os("SYSTEMROOT")
        .or_else(|| std::env::var_os("SystemRoot"))
        .map(|root| PathBuf::from(root).join("System32").join("taskkill.exe"));
    let Some(taskkill) = taskkill.filter(|path| path.is_file()) else {
        return false;
    };
    Command::new(taskkill)
        .args(["/PID", &pid.to_string(), "/T", "/F"])
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

#[cfg(windows)]
struct WindowsStdioJob(::windows::Win32::Foundation::HANDLE);

// Exclusive owner of the job-object handle. The raw HANDLE is !Send
// because it is a pointer newtype; this wrapper is Send so
// LspStdioProcess can satisfy LspProcessHandle: Send.
#[cfg(windows)]
unsafe impl Send for WindowsStdioJob {}

#[cfg(windows)]
impl Drop for WindowsStdioJob {
    fn drop(&mut self) {
        unsafe {
            let _ = ::windows::Win32::Foundation::CloseHandle(self.0);
        }
    }
}

#[cfg(windows)]
fn assign_windows_stdio_job(child: &Child) -> Option<WindowsStdioJob> {
    use std::os::windows::io::AsRawHandle;
    use windows::Win32::Foundation::{CloseHandle, HANDLE};
    use windows::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
        SetInformationJobObject,
    };
    use windows::core::PCWSTR;

    unsafe {
        let job = CreateJobObjectW(None, PCWSTR::null()).ok()?;
        let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        if SetInformationJobObject(
            job,
            JobObjectExtendedLimitInformation,
            std::ptr::from_ref(&limits).cast(),
            std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
        )
        .is_err()
        {
            let _ = CloseHandle(job);
            return None;
        }
        // windows 0.62 `HANDLE` is a `*mut c_void` newtype; `as _` also
        // covers the `isize` spelling used by some crate revisions.
        let process = HANDLE(child.as_raw_handle() as _);
        if AssignProcessToJobObject(job, process).is_err() {
            let _ = CloseHandle(job);
            return None;
        }
        Some(WindowsStdioJob(job))
    }
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

/// Result of a bounded notification pump.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PumpOutcome {
    /// The caller predicate returned true.
    PredicateMet,
    /// The deadline elapsed before the predicate was met.
    Deadline,
    /// The child closed its stdout before the predicate was met.
    Closed,
}

/// Notifications observed during a pump, borrowed for predicate evaluation.
#[derive(Debug, Default)]
pub struct PumpedNotifications {
    /// Diagnostic notifications observed so far.
    pub diagnostics: Vec<LspDiagnosticNotificationMetadata>,
    /// Progress notifications observed so far.
    pub progress: Vec<LspProgressNotification>,
}

/// Source of asynchronous LSP notifications. `BlockingPump` is the current
/// single-threaded implementation; a future reader-thread implementation can
/// replace it without changing callers (design §4, B-ready seam).
pub trait LspNotificationSource {
    /// Reads frames until `predicate` returns true, the deadline elapses, or
    /// the child closes stdout. Id-bearing frames are routed to correlation;
    /// notification frames are accumulated and surfaced to `predicate`.
    fn pump_until(
        &mut self,
        deadline: std::time::Instant,
        predicate: &mut dyn FnMut(&PumpedNotifications) -> bool,
    ) -> LspRuntimeResult<PumpOutcome>;
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
    /// Raw `publishDiagnostics` params, keyed by URI fingerprint.
    ///
    /// Updated any time a new `textDocument/publishDiagnostics` notification
    /// is received; allows callers that need the full JSON payload (e.g. to
    /// project it through `AppComposition::ingest_lsp_publish_diagnostics_for_buffer`)
    /// to retrieve the last raw params for a URI.
    /// [`Self::clear_diagnostics_for_uri`] removes the entry for the matching URI.
    raw_diagnostic_params: HashMap<FileFingerprint, serde_json::Value>,
    /// Correlated responses that arrived while we were waiting for a
    /// different request's response, keyed by JSON-RPC id. Drained by
    /// [`Self::read_response_for`] so an out-of-order response is never
    /// dropped and its request left stranded.
    response_stash: HashMap<u64, LspCorrelatedResponse>,
    /// Optional app-owned bridge for server-originated workspace edits.
    apply_edit_handler: Option<
        Box<dyn FnMut(LspApplyWorkspaceEditRequest) -> LspApplyWorkspaceEditResponse + Send>,
    >,
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
            raw_diagnostic_params: HashMap::new(),
            response_stash: HashMap::new(),
            apply_edit_handler: None,
        })
    }

    /// Installs the explicit app-owned handler for inbound `workspace/applyEdit`.
    ///
    /// The callback receives bounded metadata and parameters only; it is
    /// responsible for routing any edit through proposal authority. Without a
    /// handler, the transport returns an explicit negative result.
    pub fn set_apply_edit_handler<F>(&mut self, handler: F)
    where
        F: FnMut(LspApplyWorkspaceEditRequest) -> LspApplyWorkspaceEditResponse + Send + 'static,
    {
        self.apply_edit_handler = Some(Box::new(handler));
    }

    /// Removes the inbound `workspace/applyEdit` handler.
    pub fn clear_apply_edit_handler(&mut self) {
        self.apply_edit_handler = None;
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

    /// Snapshot of the stdout reader thread's counters (frames forwarded,
    /// payload bytes, terminal event). Post-mortem diagnostics use this to
    /// distinguish a genuinely silent server (reader alive, zero frames)
    /// from a dead reader (terminal event recorded, child possibly still
    /// alive) — the GP-1 s3 wedge discriminator (PKT-S3-WEDGE-R3).
    pub fn reader_stats(&self) -> LspReaderStatsSnapshot {
        self.process.reader_stats()
    }

    /// Returns the child's exit status as a string once it has terminated
    /// (`None` while running). See [`LspStdioProcess::exit_status_string`].
    pub fn exit_status_string(&mut self) -> Option<String> {
        self.process.exit_status_string()
    }

    /// Detaches the child process stderr handle so a supervisor/drain thread
    /// can read it independently.  Returns `None` if stderr was not captured
    /// or has already been detached.
    pub fn take_stderr(&mut self) -> Option<std::process::ChildStderr> {
        self.process.take_stderr()
    }

    /// Sends a JSON-RPC request and returns its pending correlation metadata.
    pub fn send_request(
        &mut self,
        method: impl Into<String>,
        params: Value,
        context: LspOperationContext,
    ) -> LspRuntimeResult<LspPendingRequest> {
        let pending = self.client.prepare_request(method, params, context)?;
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
        // A response for this request may have already been read (and
        // stashed) while we waited on an earlier request. Serve it from
        // the stash before reading new frames.
        if let Some(stashed) = self.response_stash.remove(&pending.json_rpc_id) {
            return Ok(stashed);
        }
        self.read_until_correlated_response(
            pending.json_rpc_id,
            pending.request_id,
            pending.timeout_ms,
            Some(&pending.context),
        )
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

    /// Removes all buffered diagnostic notifications whose URI fingerprint
    /// matches `uri_hash`, and discards the cached raw params for that URI.
    ///
    /// Call this before pumping for fresh diagnostics on a document that has
    /// just been updated via `didChange`: without a prior clear, the next pump
    /// may immediately return stale buffered data from before the edit.
    pub fn clear_diagnostics_for_uri(&mut self, uri_hash: FileFingerprint) {
        self.diagnostic_notifications
            .retain(|n| n.uri_hash != uri_hash);
        self.raw_diagnostic_params.remove(&uri_hash);
    }

    /// Returns and removes the most recently received raw `publishDiagnostics`
    /// params for the given URI fingerprint, if any.
    ///
    /// The returned `Value` is the full JSON-RPC params object as received from
    /// the language server.  It can be passed directly to
    /// `AppComposition::ingest_lsp_publish_diagnostics_for_buffer` to project
    /// LSP diagnostics through the app-owned `LanguageToolingProjection`.
    pub fn take_raw_diagnostic_params_for(
        &mut self,
        uri_hash: &FileFingerprint,
    ) -> Option<serde_json::Value> {
        self.raw_diagnostic_params.remove(uri_hash)
    }

    /// Non-blocking drain of raw `textDocument/publishDiagnostics` notification
    /// params from the reader channel.
    ///
    /// Drains all frames currently queued in the reader without blocking.  For
    /// each `publishDiagnostics` notification the raw params JSON is collected
    /// (to be projected by callers through `project_publish_diagnostics`).  All
    /// other frame types are routed through `record_notification` as normal.
    ///
    /// Safe to call at any time when no request is in flight.  Do NOT call
    /// while a `request` / `read_response_for` call is in progress on the same
    /// thread — that would steal frames from the correlation loop.
    pub fn try_drain_diagnostic_params(&mut self) -> Vec<serde_json::Value> {
        let mut raw_params = Vec::new();
        while let Some(Ok(envelope)) = self.process.try_recv_envelope() {
            // Server→client requests must be answered here too — the
            // per-frame drain is the only consumer running between explicit
            // requests, so an unanswered registration would otherwise sit
            // until the next blocking call (or forever).
            if let Ok(true) = self.answer_server_request(&envelope, None, None) {
                continue;
            }
            if envelope.method.as_deref() == Some("textDocument/publishDiagnostics")
                && let Some(params) = envelope.params.clone()
            {
                raw_params.push(params);
            }
            self.record_notification(&envelope, None);
        }
        raw_params
    }

    /// Returns whether the session has successfully completed an
    /// `initialize` exchange.
    pub fn is_ready(&self) -> bool {
        self.ready
    }

    /// Answers a server→client REQUEST (a frame carrying BOTH `id` and
    /// `method`), if the envelope is one. Returns `true` when it was.
    ///
    /// LSP requires the client to respond to every server request; a dropped
    /// request can stall server features that block on the reply (observed:
    /// rust-analyzer's `client/registerCapability` for client-side file
    /// watching). Capability (un)registration is acknowledged with a `null`
    /// result — the client accepts the registration and simply never emits
    /// the associated events. Any other server request receives the
    /// protocol-correct JSON-RPC MethodNotFound (-32601) error, signalling
    /// "unsupported" so the server can degrade instead of waiting.
    fn answer_server_request(
        &mut self,
        envelope: &JsonRpcEnvelope,
        context: Option<&LspOperationContext>,
        deadline: Option<std::time::Instant>,
    ) -> LspRuntimeResult<bool> {
        let (Some(id), Some(method)) = (envelope.id, envelope.method.as_deref()) else {
            return Ok(false);
        };
        if method == "workspace/applyEdit" {
            let response = match envelope.params.as_ref() {
                Some(params) => {
                    match Self::bounded_json_size(params, MAX_APPLY_EDIT_PARAMS_BYTES) {
                        Ok(_) => {
                            if !Self::apply_edit_params_have_edit(params) {
                                LspApplyWorkspaceEditResponse {
                                applied: false,
                                failure_reason: Some(
                                    "workspace/applyEdit parameters must contain an object edit"
                                        .to_string(),
                                ),
                            }
                            } else if let Some(handler) = self.apply_edit_handler.as_mut() {
                                handler(LspApplyWorkspaceEditRequest {
                                    json_rpc_id: id,
                                    params: params.clone(),
                                    context: context.cloned(),
                                    deadline,
                                })
                            } else {
                                LspApplyWorkspaceEditResponse {
                                    applied: false,
                                    failure_reason: Some(
                                        "workspace/applyEdit handler is not installed".to_string(),
                                    ),
                                }
                            }
                        }
                        Err(reason) => LspApplyWorkspaceEditResponse {
                            applied: false,
                            failure_reason: Some(reason.to_string()),
                        },
                    }
                }
                None => LspApplyWorkspaceEditResponse {
                    applied: false,
                    failure_reason: Some("workspace/applyEdit parameters are missing".to_string()),
                },
            };
            let result = json!({
                "applied": response.applied,
                "failureReason": response.failure_reason,
            });
            self.process
                .write_envelope(&JsonRpcEnvelope::response(id, result))?;
            return Ok(true);
        }
        let response = match method {
            "client/registerCapability" | "client/unregisterCapability" => JsonRpcEnvelope {
                jsonrpc: "2.0".to_string(),
                id: Some(id),
                method: None,
                params: None,
                result: Some(Value::Null),
                error: None,
            },
            _ => JsonRpcEnvelope {
                jsonrpc: "2.0".to_string(),
                id: Some(id),
                method: None,
                params: None,
                result: None,
                error: Some(serde_json::json!({
                    "code": -32601,
                    "message": "method not supported by this client",
                })),
            },
        };
        self.process.write_envelope(&response)?;
        Ok(true)
    }

    fn apply_edit_params_have_edit(params: &Value) -> bool {
        params
            .as_object()
            .and_then(|params| params.get("edit"))
            .is_some_and(Value::is_object)
    }

    fn bounded_json_size(value: &Value, limit: usize) -> Result<usize, &'static str> {
        struct LimitedWriter {
            used: usize,
            limit: usize,
            exceeded: bool,
        }

        impl Write for LimitedWriter {
            fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
                let Some(next) = self.used.checked_add(bytes.len()) else {
                    self.exceeded = true;
                    return Err(std::io::Error::other("JSON payload size overflow"));
                };
                if next > self.limit {
                    self.exceeded = true;
                    return Err(std::io::Error::other("JSON payload exceeds bound"));
                }
                self.used = next;
                Ok(bytes.len())
            }

            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }

        let mut writer = LimitedWriter {
            used: 0,
            limit,
            exceeded: false,
        };
        match serde_json::to_writer(&mut writer, value) {
            Ok(()) => Ok(writer.used),
            Err(_) if writer.exceeded => {
                Err("workspace/applyEdit parameters exceed bounded transport limit")
            }
            Err(_) => Err("workspace/applyEdit parameters are not serializable"),
        }
    }

    /// Routes a notification-shaped frame into the durable buffers and, when
    /// pumping, into the caller's accumulator. Returns true if the frame was a
    /// recognized notification.
    fn record_notification(
        &mut self,
        envelope: &JsonRpcEnvelope,
        acc: Option<&mut PumpedNotifications>,
    ) -> bool {
        match envelope.method.as_deref() {
            Some("$/progress") => {
                if let Some(p) = progress_notification_from_params(envelope.params.as_ref()) {
                    self.progress_notifications.push(p.clone());
                    if let Some(acc) = acc {
                        acc.progress.push(p);
                    }
                    return true;
                }
            }
            Some("textDocument/publishDiagnostics") => {
                if let Some(d) = diagnostic_notification_from_params(envelope.params.as_ref()) {
                    // Store the raw params alongside the metadata so callers can
                    // retrieve the full JSON payload for projection through
                    // AppComposition::ingest_lsp_publish_diagnostics_for_buffer.
                    if let Some(raw) = envelope.params.clone() {
                        self.raw_diagnostic_params.insert(d.uri_hash.clone(), raw);
                    }
                    self.diagnostic_notifications.push(d.clone());
                    if let Some(acc) = acc {
                        acc.diagnostics.push(d);
                    }
                    return true;
                }
            }
            _ => {}
        }
        false
    }

    /// Bounded pump for asynchronous notifications (design §4). Accumulated
    /// notifications also land in `self.diagnostic_notifications` /
    /// `self.progress_notifications` so existing accessors keep working.
    ///
    /// Each iteration uses [`LspStdioProcess::read_envelope_until`] with the
    /// remaining time budget so a server that is alive but silent cannot block
    /// the caller forever — `PumpOutcome::Deadline` is always returned before
    /// or at `deadline`, even when the server never writes another byte.
    pub fn pump_until(
        &mut self,
        deadline: std::time::Instant,
        predicate: &mut dyn FnMut(&PumpedNotifications) -> bool,
    ) -> LspRuntimeResult<PumpOutcome> {
        let mut acc = PumpedNotifications::default();
        loop {
            // Fast-path: check the deadline before issuing the read so that a
            // pre-expired deadline returns immediately without a syscall.
            if Instant::now() >= deadline {
                return Ok(PumpOutcome::Deadline);
            }
            // Deadline-bounded read: the actual blocking pipe read lives on a
            // background thread; this waits on the channel with a timeout, so
            // a silent-but-alive server cannot block the caller past `deadline`.
            let envelope = match self.process.read_envelope_until(deadline)? {
                LspReadOutcome::Envelope(envelope) => envelope,
                LspReadOutcome::TimedOut => return Ok(PumpOutcome::Deadline),
                LspReadOutcome::Eof => return Ok(PumpOutcome::Closed),
            };
            if envelope.id.is_some() {
                // Server→client requests must be answered even mid-pump
                // (rust-analyzer registers its client-side watcher while the
                // caller pumps for diagnostics). Other id-bearing frames are
                // out-of-band responses: callers must not pump with an
                // outstanding request, so those are skipped rather than
                // stashed.
                self.answer_server_request(&envelope, None, None)?;
                continue;
            }
            self.record_notification(&envelope, Some(&mut acc));
            if predicate(&acc) {
                return Ok(PumpOutcome::PredicateMet);
            }
        }
    }

    /// Reads frames from the child until we see a response for
    /// `target_json_rpc_id`. Intermediate `$/progress` and
    /// `textDocument/publishDiagnostics` notifications are recorded into the
    /// session buffers, while non-target responses are skipped, so the
    /// framing/buffering layer can be exercised without affecting correlation.
    fn read_until_correlated_response(
        &mut self,
        target_json_rpc_id: u64,
        expected_request_id: LspRequestId,
        timeout_ms: u64,
        context: Option<&LspOperationContext>,
    ) -> LspRuntimeResult<LspCorrelatedResponse> {
        let started = Instant::now();
        // A non-zero budget yields a hard deadline. The frame reader waits on
        // a channel fed by a background pipe-reader thread, so the deadline is
        // honored even when the server keeps emitting unrelated frames *or*
        // goes fully silent (the blocking pipe read lives off the caller's
        // thread). A zero budget means "wait indefinitely" and falls back to a
        // plain blocking read.
        let deadline = (timeout_ms > 0).then(|| started + Duration::from_millis(timeout_ms));
        loop {
            // Enforce the deadline before each read. This covers a server that
            // floods frames fast enough that `recv_timeout` always has a frame
            // buffered: the explicit check still bounds the loop.
            if let Some(deadline) = deadline
                && Instant::now() >= deadline
            {
                return self.resolve_request_timeout(
                    target_json_rpc_id,
                    expected_request_id,
                    started,
                    timeout_ms,
                );
            }
            let envelope = match deadline {
                Some(deadline) => match self.process.read_envelope_until(deadline)? {
                    LspReadOutcome::Envelope(envelope) => envelope,
                    LspReadOutcome::TimedOut => {
                        return self.resolve_request_timeout(
                            target_json_rpc_id,
                            expected_request_id,
                            started,
                            timeout_ms,
                        );
                    }
                    LspReadOutcome::Eof => {
                        return Err(LspRuntimeError::StdioIo {
                            message: "child closed stdout before response".to_string(),
                        });
                    }
                },
                None => match self.process.read_envelope()? {
                    Some(envelope) => envelope,
                    None => {
                        return Err(LspRuntimeError::StdioIo {
                            message: "child closed stdout before response".to_string(),
                        });
                    }
                },
            };
            let Some(id) = envelope.id else {
                self.record_notification(&envelope, None);
                continue;
            };
            // A frame with BOTH id and method is a server→client request —
            // answer it and keep waiting for the target response.
            if envelope.method.is_some() {
                self.answer_server_request(&envelope, context, deadline)?;
                continue;
            }
            if id != target_json_rpc_id {
                // A response for a *different* in-flight request. Correlate it
                // and stash it by JSON-RPC id so a later `read_response_for`
                // can retrieve it, rather than dropping it and stranding that
                // request in the correlation tables.
                match self.client.correlate_response(envelope) {
                    Ok(correlated) => {
                        self.response_stash.insert(id, correlated);
                    }
                    // Unknown / already-resolved id (e.g. a late response for a
                    // cancelled or timed-out request): nothing to correlate,
                    // skip it just like before.
                    Err(LspRuntimeError::UnknownResponseId { .. }) => {}
                    Err(err) => return Err(err),
                }
                continue;
            }
            let correlated = self.client.correlate_response(envelope)?;
            debug_assert_eq!(correlated.request_id, expected_request_id);
            return Ok(correlated);
        }
    }

    /// Resolves an in-flight request as timed out: best-effort emits a
    /// `$/cancelRequest` so the server can abandon the work, then drops the
    /// correlation entry and returns the [`LspResultStatus::Timeout`] response.
    ///
    /// The `$/cancelRequest` is written directly (rather than via
    /// [`LspClient::cancel_request`]) so the correlation tables stay intact for
    /// [`LspClient::resolve_timeout`], which requires the request to still be
    /// pending.
    fn resolve_request_timeout(
        &mut self,
        target_json_rpc_id: u64,
        expected_request_id: LspRequestId,
        started: Instant,
        timeout_ms: u64,
    ) -> LspRuntimeResult<LspCorrelatedResponse> {
        // Best-effort cancellation: ignore write failures (e.g. the server's
        // stdin is already gone) since we are tearing down the request anyway.
        let _ = self.process.write_envelope(&JsonRpcEnvelope::notification(
            "$/cancelRequest",
            json!({ "id": target_json_rpc_id }),
        ));
        // `resolve_timeout` requires the elapsed time to strictly exceed the
        // budget; clamp up by 1ms to cover the case where the deadline fired
        // at exactly the budget due to timer granularity.
        let elapsed_ms = u64::try_from(started.elapsed().as_millis())
            .unwrap_or(u64::MAX)
            .max(timeout_ms.saturating_add(1));
        self.client.resolve_timeout(expected_request_id, elapsed_ms)
    }
}

impl LspNotificationSource for LspStdioSession {
    fn pump_until(
        &mut self,
        deadline: std::time::Instant,
        predicate: &mut dyn FnMut(&PumpedNotifications) -> bool,
    ) -> LspRuntimeResult<PumpOutcome> {
        LspStdioSession::pump_until(self, deadline, predicate)
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
        // Route through the public fingerprint so recording and filtering
        // share the drive-designator normalization (PKT-S3-WEDGE-R3).
        uri_hash: lsp_diagnostic_uri_fingerprint(uri),
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

#[cfg(test)]
mod stdout_teardown_tests {
    use super::{
        LspReaderTerminal, ReaderStatsShared, STDOUT_READER_JOIN_TIMEOUT, join_stdout_reader,
    };
    use std::time::Duration;

    #[test]
    fn join_timeout_after_unconfirmed_tree_kill_records_terminal() {
        let stats = ReaderStatsShared::default();
        stats.record_tree_kill_failure();
        let handle = std::thread::spawn(|| {
            std::thread::sleep(STDOUT_READER_JOIN_TIMEOUT + Duration::from_secs(8));
        });
        join_stdout_reader(handle, &stats);
        let snapshot = stats.snapshot();
        assert!(
            snapshot.tree_kill_failed,
            "forced tree-kill failure must remain visible on the snapshot"
        );
        match snapshot.terminal {
            Some(LspReaderTerminal::Error(message)) => {
                assert!(
                    message.contains("unconfirmed process-tree kill"),
                    "join timeout after a failed tree kill must not look like a clean reader death, got {message}"
                );
            }
            other => panic!("expected unconfirmed-kill terminal error, got {other:?}"),
        }
    }
}
