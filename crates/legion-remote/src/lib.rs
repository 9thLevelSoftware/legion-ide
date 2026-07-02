//! Deterministic, metadata-first remote development runtime harness.

#![warn(missing_docs)]

use std::collections::{HashMap, HashSet};

use legion_protocol::{
    AssistedAiContractError, CancellationTokenId, CanonicalPath, CapabilityDecision, CorrelationId,
    EventSequence, FileContentVersion, FileFingerprint, FileId, LegionCloudLaneProposalResponse,
    LegionCloudLaneTaskEvent, LegionCloudLaneTaskId, LegionCloudLaneTaskRequest,
    LegionCloudLaneTaskStatus, LegionEvidenceRecord, PrincipalId, ProposalId, RedactionHint,
    RemoteAgentDescriptor, RemoteAgentId, RemoteAuditRecord, RemoteAuthorityDescriptor,
    RemoteAuthorityId, RemoteCapabilityKind, RemoteFilesystemOperation,
    RemoteFilesystemOperationKind, RemoteFilesystemSnapshot, RemoteNetworkHealthState,
    RemoteOfflineResumeManifest, RemoteOperationId, RemoteOperationLogCheckpoint,
    RemoteProcessDescriptor, RemotePtyDescriptor, RemoteSemanticQueryDescriptor,
    RemoteTransportEnvelope, RemoteTransportPayload, RemoteWorkspaceLifecycleState,
    RemoteWorkspaceSessionDescriptor, RemoteWorkspaceSessionId, RemoteWritePreconditions,
    RetentionLabel, SnapshotId, TimestampMillis, WorkspaceGeneration, WorkspaceId,
    WorkspaceTrustState, validate_legion_cloud_lane_proposal_response,
    validate_legion_cloud_lane_task_event, validate_legion_cloud_lane_task_request,
    validate_legion_cloud_lane_task_status, validate_legion_evidence_record,
};
use serde_json::Value;
use thiserror::Error;
use uuid::Uuid;

/// Remote runtime validation, policy, or deterministic harness error.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum RemoteRuntimeError {
    /// Runtime feature flag is disabled.
    #[error("remote runtime is disabled")]
    RuntimeDisabled,
    /// Session descriptor is invalid.
    #[error("invalid remote session: {reason}")]
    InvalidSession {
        /// Validation reason.
        reason: String,
    },
    /// Session was not found in the runtime registry.
    #[error("remote session is missing: {session_id:?}")]
    SessionMissing {
        /// Missing session identifier.
        session_id: RemoteWorkspaceSessionId,
    },
    /// Transport envelope metadata is invalid.
    #[error("invalid remote transport envelope: {reason}")]
    InvalidEnvelope {
        /// Validation reason.
        reason: String,
    },
    /// Capability, trust, proposal, or feature policy denied the request.
    #[error("remote policy denied request: {reason}")]
    PolicyDenied {
        /// Denial reason.
        reason: String,
    },
    /// Operation metadata or preconditions are invalid.
    #[error("invalid remote operation: {reason}")]
    InvalidOperation {
        /// Validation reason.
        reason: String,
    },
    /// Operation exceeded configured resource bounds.
    #[error("remote operation exceeded bounds: {reason}")]
    LimitExceeded {
        /// Limit reason.
        reason: String,
    },
    /// Transport or network layer failed.
    #[error("transport error: {reason}")]
    Transport {
        /// Failure reason.
        reason: String,
    },
    /// Body serialization or deserialization failed.
    #[error("serialization error: {reason}")]
    Serialization {
        /// Failure reason.
        reason: String,
    },
    /// HTTP response indicated an error.
    #[error("HTTP response error: status={status}, reason={reason}")]
    HttpResponse {
        /// HTTP status code.
        status: u16,
        /// Failure reason.
        reason: String,
    },
}

/// Runtime feature and deterministic fixture limits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteRuntimeConfig {
    /// Whether remote sessions may be created.
    pub runtime_enabled: bool,
    /// Whether remote filesystem read/list/stat/write fixture behavior is enabled.
    pub filesystem_enabled: bool,
    /// Whether remote process and PTY fixture behavior is enabled.
    pub execution_enabled: bool,
    /// Whether remote LSP fixture behavior is enabled.
    pub lsp_enabled: bool,
    /// Whether remote semantic-query fixture behavior is enabled.
    pub semantic_query_enabled: bool,
    /// Whether offline resume fixture behavior is enabled.
    pub offline_resume_enabled: bool,
    /// Maximum active sessions in a runtime registry.
    pub max_sessions: usize,
    /// Maximum bytes stored by one deterministic remote fixture file.
    pub max_file_bytes: usize,
    /// Maximum bounded output bytes for process or PTY descriptors.
    pub max_output_bytes: u64,
}

impl RemoteRuntimeConfig {
    /// Returns a conservative enabled deterministic configuration for Phase 7 validation.
    pub fn enabled() -> Self {
        Self {
            runtime_enabled: true,
            filesystem_enabled: true,
            execution_enabled: true,
            lsp_enabled: true,
            semantic_query_enabled: true,
            offline_resume_enabled: true,
            ..Self::default()
        }
    }
}

impl Default for RemoteRuntimeConfig {
    fn default() -> Self {
        Self {
            runtime_enabled: false,
            filesystem_enabled: false,
            execution_enabled: false,
            lsp_enabled: false,
            semantic_query_enabled: false,
            offline_resume_enabled: false,
            max_sessions: 16,
            max_file_bytes: 4 * 1024 * 1024,
            max_output_bytes: 256 * 1024,
        }
    }
}

/// Deterministic Legion Cloud Lane client limits and feature switches.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LegionCloudLaneClientConfig {
    /// Whether cloud-lane operations may call the configured transport.
    pub runtime_enabled: bool,
    /// Maximum cost in cents accepted before transport submission.
    pub max_cost_cents: u32,
    /// Maximum metadata-declared upload bytes accepted before transport submission.
    pub max_upload_bytes: u64,
}

impl LegionCloudLaneClientConfig {
    /// Build an enabled config with explicit task cost and upload caps.
    pub fn enabled(max_cost_cents: u32, max_upload_bytes: u64) -> Self {
        Self {
            runtime_enabled: true,
            max_cost_cents,
            max_upload_bytes,
        }
    }

    /// Build the conservative deterministic config used by crate-level tests.
    pub fn enabled_for_tests() -> Self {
        Self::enabled(75, 32 * 1024)
    }
}

/// Transport boundary used by Legion Cloud Lane clients.
pub trait LegionCloudLaneTransport {
    /// Submit a metadata-only task request to the cloud-lane control plane.
    fn submit_task(
        &mut self,
        request: &LegionCloudLaneTaskRequest,
    ) -> Result<LegionCloudLaneTaskStatus, RemoteRuntimeError>;

    /// Stream currently available metadata-only task events.
    fn stream_task_events(
        &mut self,
        task_id: &LegionCloudLaneTaskId,
    ) -> Result<Vec<LegionCloudLaneTaskEvent>, RemoteRuntimeError>;

    /// Cancel a task through its scoped cancellation token.
    fn cancel_task(
        &mut self,
        task_id: &LegionCloudLaneTaskId,
        cancellation_token: CancellationTokenId,
        reason_label: &str,
    ) -> Result<LegionCloudLaneTaskStatus, RemoteRuntimeError>;

    /// Fetch metadata for a cloud-produced proposal.
    fn fetch_task_proposal(
        &mut self,
        task_id: &LegionCloudLaneTaskId,
    ) -> Result<LegionCloudLaneProposalResponse, RemoteRuntimeError>;

    /// Fetch metadata-only evidence records for a cloud task.
    fn fetch_task_evidence(
        &mut self,
        task_id: &LegionCloudLaneTaskId,
    ) -> Result<Vec<LegionEvidenceRecord>, RemoteRuntimeError>;
}

/// Policy-validating client wrapper for a Legion Cloud Lane transport.
#[derive(Debug, Clone)]
pub struct LegionCloudLaneClient<T> {
    transport: T,
    config: LegionCloudLaneClientConfig,
}

impl<T> LegionCloudLaneClient<T> {
    /// Create a client from an explicit transport and policy config.
    pub fn new(transport: T, config: LegionCloudLaneClientConfig) -> Self {
        Self { transport, config }
    }

    /// Inspect the underlying deterministic transport.
    pub fn transport(&self) -> &T {
        &self.transport
    }

    /// Mutably inspect the underlying deterministic transport.
    pub fn transport_mut(&mut self) -> &mut T {
        &mut self.transport
    }
}

impl<T: LegionCloudLaneTransport> LegionCloudLaneClient<T> {
    /// Validate and submit a cloud-lane task request.
    pub fn submit_task(
        &mut self,
        request: LegionCloudLaneTaskRequest,
    ) -> Result<LegionCloudLaneTaskStatus, RemoteRuntimeError> {
        self.ensure_enabled()?;
        validate_cloud_task_request_limits(&request, &self.config)?;
        let status = self.transport.submit_task(&request)?;
        validate_legion_cloud_lane_task_status(&status).map_err(cloud_contract_error)?;
        validate_cloud_task_response_id("submit status", &request.task_id, &status.task_id)?;
        Ok(status)
    }

    /// Fetch current task event metadata.
    pub fn stream_task_events(
        &mut self,
        task_id: &LegionCloudLaneTaskId,
    ) -> Result<Vec<LegionCloudLaneTaskEvent>, RemoteRuntimeError> {
        self.ensure_enabled()?;
        validate_cloud_task_id(task_id)?;
        let events = self.transport.stream_task_events(task_id)?;
        for event in &events {
            validate_legion_cloud_lane_task_event(event).map_err(cloud_contract_error)?;
            validate_cloud_task_response_id("event", task_id, &event.task_id)?;
        }
        Ok(events)
    }

    /// Cancel a task with a non-nil cancellation token and display-safe reason label.
    pub fn cancel_task(
        &mut self,
        task_id: &LegionCloudLaneTaskId,
        cancellation_token: CancellationTokenId,
        reason_label: &str,
    ) -> Result<LegionCloudLaneTaskStatus, RemoteRuntimeError> {
        self.ensure_enabled()?;
        validate_cloud_task_id(task_id)?;
        validate_cancellation_token(cancellation_token)?;
        if reason_label.trim().is_empty() {
            return Err(RemoteRuntimeError::InvalidOperation {
                reason: "cloud lane cancellation reason must be non-empty".to_string(),
            });
        }
        let status = self
            .transport
            .cancel_task(task_id, cancellation_token, reason_label)?;
        validate_legion_cloud_lane_task_status(&status).map_err(cloud_contract_error)?;
        validate_cloud_task_response_id("cancel status", task_id, &status.task_id)?;
        Ok(status)
    }

    /// Fetch metadata for the proposal produced by a task.
    pub fn fetch_task_proposal(
        &mut self,
        task_id: &LegionCloudLaneTaskId,
    ) -> Result<LegionCloudLaneProposalResponse, RemoteRuntimeError> {
        self.ensure_enabled()?;
        validate_cloud_task_id(task_id)?;
        let response = self.transport.fetch_task_proposal(task_id)?;
        validate_legion_cloud_lane_proposal_response(&response).map_err(cloud_contract_error)?;
        validate_cloud_task_response_id("proposal response", task_id, &response.task_id)?;
        Ok(response)
    }

    /// Fetch metadata-only evidence records for a task.
    pub fn fetch_task_evidence(
        &mut self,
        task_id: &LegionCloudLaneTaskId,
    ) -> Result<Vec<LegionEvidenceRecord>, RemoteRuntimeError> {
        self.ensure_enabled()?;
        validate_cloud_task_id(task_id)?;
        let evidence = self.transport.fetch_task_evidence(task_id)?;
        for record in &evidence {
            validate_legion_evidence_record(record).map_err(cloud_contract_error)?;
        }
        Ok(evidence)
    }

    fn ensure_enabled(&self) -> Result<(), RemoteRuntimeError> {
        if self.config.runtime_enabled {
            Ok(())
        } else {
            Err(RemoteRuntimeError::RuntimeDisabled)
        }
    }
}

fn validate_cloud_task_request_limits(
    request: &LegionCloudLaneTaskRequest,
    config: &LegionCloudLaneClientConfig,
) -> Result<(), RemoteRuntimeError> {
    validate_legion_cloud_lane_task_request(request).map_err(cloud_contract_error)?;
    if config.max_cost_cents == 0 || request.budget.estimated_cost_cents > config.max_cost_cents {
        return Err(RemoteRuntimeError::LimitExceeded {
            reason: "cloud lane estimated cost exceeds configured cost cap".to_string(),
        });
    }
    if config.max_upload_bytes == 0
        || request.upload_manifest.total_upload_bytes > config.max_upload_bytes
    {
        return Err(RemoteRuntimeError::LimitExceeded {
            reason: "cloud lane upload bytes exceed configured upload cap".to_string(),
        });
    }
    Ok(())
}

fn validate_cloud_task_id(task_id: &LegionCloudLaneTaskId) -> Result<(), RemoteRuntimeError> {
    if task_id.0.trim().is_empty() {
        return Err(RemoteRuntimeError::InvalidOperation {
            reason: "cloud lane task id must be non-empty".to_string(),
        });
    }
    if task_id
        .0
        .chars()
        .any(|c| c.is_control() || matches!(c, '/' | '\\' | '?' | '#'))
    {
        return Err(RemoteRuntimeError::InvalidOperation {
            reason: "cloud lane task id must not contain path, query, or control characters"
                .to_string(),
        });
    }
    Ok(())
}

/// Percent-encodes a string for safe inclusion as a single URL path segment.
///
/// Characters permitted in an RFC 3986 path segment (`pchar`: unreserved,
/// sub-delims, `:` and `@`) are passed through verbatim; everything else
/// (including `/`, `?`, `#`, and control bytes) is percent-encoded so a task id
/// cannot alter the request path or query.
fn encode_path_segment(segment: &str) -> String {
    let mut encoded = String::with_capacity(segment.len());
    for byte in segment.bytes() {
        match byte {
            // unreserved
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~'
            // sub-delims
            | b'!' | b'$' | b'&' | b'\'' | b'(' | b')' | b'*' | b'+' | b',' | b';' | b'='
            // additional pchar
            | b':' | b'@' => {
                encoded.push(byte as char);
            }
            _ => encoded.push_str(&format!("%{byte:02X}")),
        }
    }
    encoded
}

fn validate_cloud_task_response_id(
    response_kind: &str,
    expected: &LegionCloudLaneTaskId,
    actual: &LegionCloudLaneTaskId,
) -> Result<(), RemoteRuntimeError> {
    if actual != expected {
        return Err(RemoteRuntimeError::InvalidOperation {
            reason: format!(
                "cloud lane {response_kind} task id {} does not match requested task id {}",
                actual.0, expected.0
            ),
        });
    }
    Ok(())
}

fn cloud_contract_error(error: AssistedAiContractError) -> RemoteRuntimeError {
    RemoteRuntimeError::InvalidOperation {
        reason: cloud_contract_reason(&error),
    }
}

fn cloud_contract_reason(error: &AssistedAiContractError) -> String {
    match error {
        AssistedAiContractError::NonMetadataOnlyAuditRecord { field, reason }
            if field == "legion.cloud.upload_manifest.contains_forbidden_material"
                && reason == "forbidden_material" =>
        {
            "cloud upload manifest contains forbidden material".to_string()
        }
        _ => format!("invalid Legion Cloud Lane metadata: {error}"),
    }
}

/// Remote connection kind accepted by the policy-gated session planner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemoteConnectionKind {
    /// SSH authority backed by a headless remote agent.
    Ssh,
    /// Devcontainer authority backed by a headless remote agent.
    Devcontainer,
}

/// Policy-validated remote connection request metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteConnectionSpec {
    /// Session identifier assigned by app-owned composition.
    pub session_id: RemoteWorkspaceSessionId,
    /// Authority identifier assigned by app-owned composition.
    pub authority_id: RemoteAuthorityId,
    /// Agent identifier assigned by app-owned composition.
    pub agent_id: RemoteAgentId,
    /// Local workspace projection identifier.
    pub workspace_id: WorkspaceId,
    /// Principal requesting the session.
    pub principal_id: PrincipalId,
    /// Redacted authority label, for example `ssh:user@host` or `devcontainer:name`.
    pub authority_label: String,
    /// Redacted workspace root label.
    pub workspace_root_label: String,
    /// Credential reference label or hash.
    pub credential_reference_label: String,
    /// Remote agent version label.
    pub agent_version: String,
    /// Trust state observed before activation.
    pub trust_state: WorkspaceTrustState,
    /// Capability kinds granted by policy for this session.
    pub granted_capabilities: Vec<RemoteCapabilityKind>,
}

/// Parsed, metadata-only devcontainer configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteDevcontainerConfig {
    /// Display-safe devcontainer name.
    pub name_label: String,
    /// Display-safe image or Dockerfile label.
    pub image_label: String,
    /// Display-safe remote user label.
    pub remote_user_label: Option<String>,
    /// Display-safe workspace folder label.
    pub workspace_folder_label: Option<String>,
    /// Number of declared features.
    pub feature_count: usize,
    /// Number of declared mounts.
    pub mount_count: usize,
}

/// Policy-gated remote connection plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteConnectionPlan {
    /// Connection kind.
    pub kind: RemoteConnectionKind,
    /// Session descriptor accepted by `RemoteSessionRuntime`.
    pub descriptor: RemoteWorkspaceSessionDescriptor,
    /// Display-safe workspace root label.
    pub workspace_root_label: String,
    /// Credential reference label or hash.
    pub credential_reference_label: String,
    /// Parsed devcontainer metadata, when applicable.
    pub devcontainer: Option<RemoteDevcontainerConfig>,
}

/// Build an active SSH remote session plan without spawning ambient processes.
pub fn plan_ssh_session(
    spec: RemoteConnectionSpec,
) -> Result<RemoteConnectionPlan, RemoteRuntimeError> {
    validate_connection_spec(&spec)?;
    Ok(RemoteConnectionPlan {
        kind: RemoteConnectionKind::Ssh,
        descriptor: descriptor_from_connection_spec(&spec),
        workspace_root_label: spec.workspace_root_label,
        credential_reference_label: spec.credential_reference_label,
        devcontainer: None,
    })
}

/// Parse `devcontainer.json` and build an active devcontainer remote session plan.
pub fn plan_devcontainer_session_from_json(
    spec: RemoteConnectionSpec,
    devcontainer_json: &str,
) -> Result<RemoteConnectionPlan, RemoteRuntimeError> {
    validate_connection_spec(&spec)?;
    let value = serde_json::from_str::<Value>(devcontainer_json).map_err(|error| {
        RemoteRuntimeError::InvalidSession {
            reason: format!("invalid devcontainer.json: {error}"),
        }
    })?;
    let devcontainer = parse_devcontainer_config(&value)?;
    Ok(RemoteConnectionPlan {
        kind: RemoteConnectionKind::Devcontainer,
        descriptor: descriptor_from_connection_spec(&spec),
        workspace_root_label: spec.workspace_root_label,
        credential_reference_label: spec.credential_reference_label,
        devcontainer: Some(devcontainer),
    })
}

/// Returns the conservative remote capabilities granted by connection planners.
pub fn default_remote_capabilities() -> Vec<RemoteCapabilityKind> {
    vec![
        RemoteCapabilityKind::Connect,
        RemoteCapabilityKind::FilesystemRead,
        RemoteCapabilityKind::FilesystemWrite,
        RemoteCapabilityKind::TerminalAccess,
        RemoteCapabilityKind::LspLaunch,
        RemoteCapabilityKind::AuditExport,
        RemoteCapabilityKind::OfflineResume,
    ]
}

fn validate_connection_spec(spec: &RemoteConnectionSpec) -> Result<(), RemoteRuntimeError> {
    if spec.session_id.0 == 0 || spec.authority_id.0 == 0 || spec.agent_id.0 == 0 {
        return Err(RemoteRuntimeError::InvalidSession {
            reason: "remote identifiers must be non-zero".to_string(),
        });
    }
    if spec.authority_label.trim().is_empty()
        || spec.workspace_root_label.trim().is_empty()
        || spec.credential_reference_label.trim().is_empty()
        || spec.agent_version.trim().is_empty()
        || spec.principal_id.0.trim().is_empty()
    {
        return Err(RemoteRuntimeError::InvalidSession {
            reason: "connection labels and principal must be non-empty".to_string(),
        });
    }
    if spec.trust_state != WorkspaceTrustState::Trusted {
        return Err(RemoteRuntimeError::PolicyDenied {
            reason: "remote connection requires a trusted workspace".to_string(),
        });
    }
    if !spec
        .granted_capabilities
        .contains(&RemoteCapabilityKind::Connect)
    {
        return Err(RemoteRuntimeError::PolicyDenied {
            reason: "remote connection requires Connect capability".to_string(),
        });
    }
    Ok(())
}

fn descriptor_from_connection_spec(
    spec: &RemoteConnectionSpec,
) -> RemoteWorkspaceSessionDescriptor {
    RemoteWorkspaceSessionDescriptor {
        session_id: spec.session_id,
        authority: RemoteAuthorityDescriptor {
            authority_id: spec.authority_id,
            authority_label: spec.authority_label.clone(),
            workspace_id: spec.workspace_id,
            trust_state: spec.trust_state.clone(),
            principal_id: spec.principal_id.clone(),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        },
        agent: RemoteAgentDescriptor {
            agent_id: spec.agent_id,
            authority_id: spec.authority_id,
            agent_version: spec.agent_version.clone(),
            runtime_enabled: true,
            schema_version: 1,
        },
        state: RemoteWorkspaceLifecycleState::Active,
        granted_capabilities: spec.granted_capabilities.clone(),
        created_at: TimestampMillis::now(),
        last_heartbeat_at: Some(TimestampMillis::now()),
        schema_version: 1,
    }
}

fn parse_devcontainer_config(
    value: &Value,
) -> Result<RemoteDevcontainerConfig, RemoteRuntimeError> {
    let object = value
        .as_object()
        .ok_or_else(|| RemoteRuntimeError::InvalidSession {
            reason: "devcontainer.json root must be an object".to_string(),
        })?;
    let name_label = object
        .get("name")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("devcontainer")
        .to_string();
    let image_label = object
        .get("image")
        .and_then(Value::as_str)
        .or_else(|| {
            object
                .get("build")
                .and_then(|build| build.get("dockerfile"))
                .and_then(Value::as_str)
        })
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| RemoteRuntimeError::InvalidSession {
            reason: "devcontainer.json requires image or build.dockerfile".to_string(),
        })?
        .to_string();
    let remote_user_label = object
        .get("remoteUser")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string);
    let workspace_folder_label = object
        .get("workspaceFolder")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string);
    let feature_count = object
        .get("features")
        .and_then(Value::as_object)
        .map(|features| features.len())
        .unwrap_or(0);
    let mount_count = object
        .get("mounts")
        .and_then(Value::as_array)
        .map(|mounts| mounts.len())
        .unwrap_or(0);
    Ok(RemoteDevcontainerConfig {
        name_label,
        image_label,
        remote_user_label,
        workspace_folder_label,
        feature_count,
        mount_count,
    })
}

/// Remote operation disposition emitted by the deterministic runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemoteOperationDisposition {
    /// Operation was accepted.
    Accepted,
    /// Operation was a duplicate and was ignored.
    Duplicate,
    /// Operation was denied by trust, capability, proposal, or feature policy.
    Denied,
    /// Operation was stale against version, fingerprint, generation, or checkpoint metadata.
    Stale,
    /// Operation conflicted with current remote fixture state.
    Conflict,
    /// Operation exposed a causal or checkpoint gap.
    GapDetected,
    /// Operation is intentionally unsupported by the accepted Phase 7 slice.
    Unsupported,
    /// Operation was metadata-only and did not change fixture state.
    Noop,
}

/// Result of handling one remote operation envelope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteOperationOutcome {
    /// Operation identifier.
    pub operation_id: RemoteOperationId,
    /// Operation disposition.
    pub disposition: RemoteOperationDisposition,
    /// Stable reason code for denied, stale, conflict, gap, or unsupported outcomes.
    pub reason_code: Option<String>,
    /// Metadata-only filesystem snapshot when produced.
    pub snapshot: Option<RemoteFilesystemSnapshot>,
    /// Metadata-only operation checkpoint when produced.
    pub checkpoint: Option<RemoteOperationLogCheckpoint>,
}

impl RemoteOperationOutcome {
    fn new(
        operation_id: RemoteOperationId,
        disposition: RemoteOperationDisposition,
        reason_code: Option<&str>,
    ) -> Self {
        Self {
            operation_id,
            disposition,
            reason_code: reason_code.map(str::to_string),
            snapshot: None,
            checkpoint: None,
        }
    }

    fn with_snapshot(mut self, snapshot: RemoteFilesystemSnapshot) -> Self {
        self.snapshot = Some(snapshot);
        self
    }

    fn with_checkpoint(mut self, checkpoint: RemoteOperationLogCheckpoint) -> Self {
        self.checkpoint = Some(checkpoint);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RemoteFileEntry {
    file_id: FileId,
    content: String,
    content_version: FileContentVersion,
    snapshot_id: SnapshotId,
    fingerprint: FileFingerprint,
}

/// In-memory deterministic remote workspace session.
#[derive(Debug, Clone)]
pub struct RemoteSessionRuntime {
    descriptor: RemoteWorkspaceSessionDescriptor,
    config: RemoteRuntimeConfig,
    workspace_generation: WorkspaceGeneration,
    files: HashMap<String, RemoteFileEntry>,
    seen_operations: HashSet<RemoteOperationId>,
    checkpoints: Vec<RemoteOperationLogCheckpoint>,
    network_health: RemoteNetworkHealthState,
}

impl RemoteSessionRuntime {
    /// Creates an enabled remote session runtime from protocol descriptors.
    pub fn new(
        descriptor: RemoteWorkspaceSessionDescriptor,
        workspace_generation: WorkspaceGeneration,
        config: RemoteRuntimeConfig,
    ) -> Result<Self, RemoteRuntimeError> {
        validate_session_descriptor(&descriptor)?;
        if !config.runtime_enabled {
            return Err(RemoteRuntimeError::RuntimeDisabled);
        }
        if !descriptor.activation_is_policy_ready() {
            return Err(RemoteRuntimeError::PolicyDenied {
                reason: "remote session must be trusted, active, principal-scoped, and explicitly enabled"
                    .to_string(),
            });
        }
        if workspace_generation.0 == 0 {
            return Err(RemoteRuntimeError::InvalidSession {
                reason: "workspace generation must be non-zero".to_string(),
            });
        }

        Ok(Self {
            descriptor,
            config,
            workspace_generation,
            files: HashMap::new(),
            seen_operations: HashSet::new(),
            checkpoints: Vec::new(),
            network_health: RemoteNetworkHealthState::Healthy,
        })
    }

    /// Returns the remote session identifier.
    pub fn session_id(&self) -> RemoteWorkspaceSessionId {
        self.descriptor.session_id
    }

    /// Returns the current session descriptor.
    pub fn descriptor(&self) -> &RemoteWorkspaceSessionDescriptor {
        &self.descriptor
    }

    /// Returns the current lifecycle state.
    pub fn state(&self) -> RemoteWorkspaceLifecycleState {
        self.descriptor.state
    }

    /// Returns the current network health state.
    pub fn network_health(&self) -> RemoteNetworkHealthState {
        self.network_health
    }

    /// Returns recorded operation-log checkpoints.
    pub fn checkpoints(&self) -> &[RemoteOperationLogCheckpoint] {
        &self.checkpoints
    }

    /// Seeds an ephemeral deterministic remote fixture file.
    pub fn seed_file(
        &mut self,
        path: CanonicalPath,
        file_id: FileId,
        content: impl Into<String>,
    ) -> Result<RemoteFilesystemSnapshot, RemoteRuntimeError> {
        let content = content.into();
        if content.len() > self.config.max_file_bytes {
            return Err(RemoteRuntimeError::LimitExceeded {
                reason: "remote fixture file exceeds configured size limit".to_string(),
            });
        }
        let snapshot_id = SnapshotId(stable_hash_u128(&path.0) | 1);
        let fingerprint = metadata_fingerprint(&content);
        let entry = RemoteFileEntry {
            file_id,
            content,
            content_version: FileContentVersion(1),
            snapshot_id,
            fingerprint,
        };
        self.files.insert(path.0.clone(), entry);
        Ok(self.snapshot_for_path(&path))
    }

    /// Returns current ephemeral fixture text for tests and remote-side assertions.
    pub fn fixture_file_text(&self, path: &CanonicalPath) -> Option<&str> {
        self.files.get(&path.0).map(|entry| entry.content.as_str())
    }

    /// Handles a validated remote transport envelope.
    pub fn handle_transport_envelope(
        &mut self,
        envelope: RemoteTransportEnvelope,
    ) -> Result<RemoteOperationOutcome, RemoteRuntimeError> {
        self.validate_envelope(&envelope)?;
        if !self.seen_operations.insert(envelope.operation_id) {
            return Ok(RemoteOperationOutcome::new(
                envelope.operation_id,
                RemoteOperationDisposition::Duplicate,
                Some("duplicate_operation"),
            ));
        }

        match envelope.payload {
            RemoteTransportPayload::Session(descriptor) => {
                self.handle_session_descriptor(envelope.operation_id, descriptor)
            }
            RemoteTransportPayload::FilesystemSnapshot(snapshot) => {
                self.handle_filesystem_snapshot(envelope.operation_id, snapshot)
            }
            RemoteTransportPayload::FilesystemOperation(operation) => {
                self.handle_filesystem_operation(envelope.operation_id, operation)
            }
            RemoteTransportPayload::Process(descriptor) => {
                self.handle_process_descriptor(envelope.operation_id, descriptor)
            }
            RemoteTransportPayload::Pty(descriptor) => {
                self.handle_pty_descriptor(envelope.operation_id, descriptor)
            }
            RemoteTransportPayload::Lsp(descriptor) => {
                self.handle_lsp_descriptor(envelope.operation_id, descriptor)
            }
            RemoteTransportPayload::SemanticQuery(descriptor) => {
                self.handle_semantic_query_descriptor(envelope.operation_id, descriptor)
            }
            RemoteTransportPayload::OperationLogCheckpoint(checkpoint) => {
                self.handle_checkpoint(envelope.operation_id, checkpoint)
            }
            RemoteTransportPayload::OfflineResume(manifest) => {
                self.handle_offline_resume(envelope.operation_id, manifest)
            }
            RemoteTransportPayload::Audit(record) => {
                self.handle_audit(envelope.operation_id, record)
            }
        }
    }

    /// Marks the remote session as reconnecting and preserves fixture state.
    pub fn begin_reconnect(&mut self) {
        self.descriptor.state = RemoteWorkspaceLifecycleState::Reconnecting;
        self.network_health = RemoteNetworkHealthState::Disconnected;
    }

    /// Completes reconnect after identity, cache, and version preconditions are externally validated.
    pub fn complete_reconnect(&mut self) -> Result<(), RemoteRuntimeError> {
        if !matches!(
            self.descriptor.state,
            RemoteWorkspaceLifecycleState::Reconnecting
        ) {
            return Err(RemoteRuntimeError::InvalidSession {
                reason: "session must be reconnecting before reconnect completion".to_string(),
            });
        }
        self.descriptor.state = RemoteWorkspaceLifecycleState::Active;
        self.network_health = RemoteNetworkHealthState::Healthy;
        Ok(())
    }

    /// Marks the session offline and preserves fixture state for explicit resume.
    pub fn mark_offline(&mut self) {
        self.descriptor.state = RemoteWorkspaceLifecycleState::Offline;
        self.network_health = RemoteNetworkHealthState::Offline;
    }

    /// Builds a metadata-only offline resume manifest.
    pub fn offline_resume_manifest(
        &self,
        correlation_id: CorrelationId,
        causality_id: legion_protocol::CausalityId,
        event_sequence: EventSequence,
    ) -> RemoteOfflineResumeManifest {
        RemoteOfflineResumeManifest {
            session_id: self.session_id(),
            checkpoints: self
                .checkpoints
                .iter()
                .map(|checkpoint| checkpoint.checkpoint_id)
                .collect(),
            workspace_generation: self.workspace_generation,
            snapshot_id: SnapshotId(self.files.len() as u128 + 1),
            correlation_id,
            causality_id,
            event_sequence,
            schema_version: 1,
        }
    }

    /// Builds a metadata-only remote audit record for the latest session state.
    pub fn audit_record(
        &self,
        operation_id: Option<RemoteOperationId>,
        proposal_id: Option<ProposalId>,
        event_sequence: EventSequence,
        correlation_id: CorrelationId,
        causality_id: legion_protocol::CausalityId,
    ) -> RemoteAuditRecord {
        RemoteAuditRecord {
            session_id: self.session_id(),
            operation_id,
            proposal_id,
            event_sequence,
            correlation_id,
            causality_id,
            retention_label: RetentionLabel::Audit,
            redaction_hints: vec![RedactionHint::MetadataOnly],
            metadata_summary: format!(
                "state={:?} files={} checkpoints={} network={:?}",
                self.descriptor.state,
                self.files.len(),
                self.checkpoints.len(),
                self.network_health
            ),
            schema_version: 1,
        }
    }

    fn validate_envelope(
        &self,
        envelope: &RemoteTransportEnvelope,
    ) -> Result<(), RemoteRuntimeError> {
        if envelope.session_id != self.session_id() || envelope.operation_id.0 == 0 {
            return Err(RemoteRuntimeError::InvalidEnvelope {
                reason: "envelope session and operation id must match active session".to_string(),
            });
        }
        if envelope.schema_version == 0 || !envelope.has_valid_event_identity() {
            return Err(RemoteRuntimeError::InvalidEnvelope {
                reason: "schema, principal, correlation, causality, and sequence are required"
                    .to_string(),
            });
        }
        if envelope.redaction_hints.contains(&RedactionHint::None) {
            return Err(RemoteRuntimeError::InvalidEnvelope {
                reason: "remote envelopes must not request raw retention".to_string(),
            });
        }
        Ok(())
    }

    fn handle_session_descriptor(
        &mut self,
        operation_id: RemoteOperationId,
        descriptor: RemoteWorkspaceSessionDescriptor,
    ) -> Result<RemoteOperationOutcome, RemoteRuntimeError> {
        validate_session_descriptor(&descriptor)?;
        if descriptor.session_id != self.session_id() {
            return Ok(RemoteOperationOutcome::new(
                operation_id,
                RemoteOperationDisposition::Conflict,
                Some("session_id_mismatch"),
            ));
        }
        self.descriptor = descriptor;
        Ok(RemoteOperationOutcome::new(
            operation_id,
            RemoteOperationDisposition::Accepted,
            None,
        ))
    }

    fn handle_filesystem_snapshot(
        &self,
        operation_id: RemoteOperationId,
        snapshot: RemoteFilesystemSnapshot,
    ) -> Result<RemoteOperationOutcome, RemoteRuntimeError> {
        if snapshot.session_id != self.session_id()
            || snapshot.workspace_id != self.descriptor.authority.workspace_id
            || snapshot.schema_version == 0
            || snapshot.snapshot_id.0 == 0
        {
            return Err(RemoteRuntimeError::InvalidOperation {
                reason: "filesystem snapshot identity is invalid".to_string(),
            });
        }
        Ok(
            RemoteOperationOutcome::new(operation_id, RemoteOperationDisposition::Noop, None)
                .with_snapshot(snapshot),
        )
    }

    fn handle_filesystem_operation(
        &mut self,
        operation_id: RemoteOperationId,
        operation: RemoteFilesystemOperation,
    ) -> Result<RemoteOperationOutcome, RemoteRuntimeError> {
        if !self.config.filesystem_enabled {
            return Ok(RemoteOperationOutcome::new(
                operation_id,
                RemoteOperationDisposition::Denied,
                Some("remote_filesystem_disabled"),
            ));
        }
        if operation.session_id != self.session_id()
            || operation.operation_id.0 == 0
            || operation.operation_id != operation_id
            || operation.schema_version == 0
        {
            return Err(RemoteRuntimeError::InvalidOperation {
                reason: "filesystem operation session, operation id, and schema are required"
                    .to_string(),
            });
        }

        match operation.kind {
            RemoteFilesystemOperationKind::Read
            | RemoteFilesystemOperationKind::List
            | RemoteFilesystemOperationKind::Stat => Ok(RemoteOperationOutcome::new(
                operation_id,
                RemoteOperationDisposition::Accepted,
                None,
            )
            .with_snapshot(self.snapshot_for_path(&operation.path))),
            RemoteFilesystemOperationKind::Write => self.write_file(operation_id, operation),
            RemoteFilesystemOperationKind::Create => self.create_file(operation_id, operation),
            RemoteFilesystemOperationKind::Delete => self.delete_file(operation_id, operation),
            RemoteFilesystemOperationKind::Rename => self.rename_file(operation_id, operation),
        }
    }

    fn write_file(
        &mut self,
        operation_id: RemoteOperationId,
        operation: RemoteFilesystemOperation,
    ) -> Result<RemoteOperationOutcome, RemoteRuntimeError> {
        let Some(preconditions) = operation.write_preconditions.as_ref() else {
            return Ok(denied(operation_id, "remote_write_missing_preconditions"));
        };
        if let Some(outcome) = self.validate_mutation_gate(operation_id, &operation, preconditions)
        {
            return Ok(outcome);
        }
        let Some(existing) = self.files.get(&operation.path.0) else {
            return Ok(conflict(operation_id, "remote_file_missing"));
        };
        if preconditions.file_content_version != existing.content_version {
            return Ok(stale(operation_id, "remote_file_content_version_mismatch"));
        }
        if preconditions.snapshot_id != existing.snapshot_id {
            return Ok(stale(operation_id, "remote_snapshot_mismatch"));
        }
        if preconditions.expected_fingerprint.as_ref() != Some(&existing.fingerprint) {
            return Ok(stale(operation_id, "remote_fingerprint_mismatch"));
        }

        let proposal_id = operation
            .proposal_id
            .expect("mutation gate checked proposal id");
        let replacement = format!(
            "remote-proposal:{}:operation:{}",
            proposal_id.0, operation_id.0
        );
        if replacement.len() > self.config.max_file_bytes {
            return Err(RemoteRuntimeError::LimitExceeded {
                reason: "remote write payload exceeds configured limit".to_string(),
            });
        }
        let entry = self
            .files
            .get_mut(&operation.path.0)
            .expect("file checked above");
        entry.content = replacement;
        entry.content_version = FileContentVersion(entry.content_version.0.saturating_add(1));
        entry.snapshot_id =
            derived_snapshot_id(&operation.path.0, entry.content_version, operation_id);
        entry.fingerprint = metadata_fingerprint(&entry.content);
        Ok(
            RemoteOperationOutcome::new(operation_id, RemoteOperationDisposition::Accepted, None)
                .with_snapshot(self.snapshot_for_path(&operation.path)),
        )
    }

    fn create_file(
        &mut self,
        operation_id: RemoteOperationId,
        operation: RemoteFilesystemOperation,
    ) -> Result<RemoteOperationOutcome, RemoteRuntimeError> {
        let Some(preconditions) = operation.write_preconditions.as_ref() else {
            return Ok(denied(operation_id, "remote_create_missing_preconditions"));
        };
        if let Some(outcome) = self.validate_mutation_gate(operation_id, &operation, preconditions)
        {
            return Ok(outcome);
        }
        if self.files.contains_key(&operation.path.0) {
            return Ok(conflict(operation_id, "remote_file_already_exists"));
        }
        let proposal_id = operation
            .proposal_id
            .expect("mutation gate checked proposal id");
        let content = format!("remote-created-by-proposal:{}", proposal_id.0);
        let content_version = FileContentVersion(1);
        let fingerprint = metadata_fingerprint(&content);
        let entry = RemoteFileEntry {
            file_id: FileId(stable_hash_u128(&operation.path.0) | 1),
            content,
            content_version,
            snapshot_id: derived_snapshot_id(&operation.path.0, content_version, operation_id),
            fingerprint,
        };
        self.files.insert(operation.path.0.clone(), entry);
        Ok(
            RemoteOperationOutcome::new(operation_id, RemoteOperationDisposition::Accepted, None)
                .with_snapshot(self.snapshot_for_path(&operation.path)),
        )
    }

    fn delete_file(
        &mut self,
        operation_id: RemoteOperationId,
        operation: RemoteFilesystemOperation,
    ) -> Result<RemoteOperationOutcome, RemoteRuntimeError> {
        let Some(preconditions) = operation.write_preconditions.as_ref() else {
            return Ok(denied(operation_id, "remote_delete_missing_preconditions"));
        };
        if let Some(outcome) = self.validate_mutation_gate(operation_id, &operation, preconditions)
        {
            return Ok(outcome);
        }
        let Some(existing) = self.files.get(&operation.path.0) else {
            return Ok(conflict(operation_id, "remote_file_missing"));
        };
        if preconditions.file_content_version != existing.content_version {
            return Ok(stale(operation_id, "remote_file_content_version_mismatch"));
        }
        if preconditions.snapshot_id != existing.snapshot_id {
            return Ok(stale(operation_id, "remote_snapshot_mismatch"));
        }
        if preconditions.expected_fingerprint.as_ref() != Some(&existing.fingerprint) {
            return Ok(stale(operation_id, "remote_fingerprint_mismatch"));
        }
        self.files.remove(&operation.path.0);
        Ok(RemoteOperationOutcome::new(
            operation_id,
            RemoteOperationDisposition::Accepted,
            None,
        ))
    }

    fn rename_file(
        &mut self,
        operation_id: RemoteOperationId,
        operation: RemoteFilesystemOperation,
    ) -> Result<RemoteOperationOutcome, RemoteRuntimeError> {
        let Some(preconditions) = operation.write_preconditions.as_ref() else {
            return Ok(denied(operation_id, "remote_rename_missing_preconditions"));
        };
        if let Some(outcome) = self.validate_mutation_gate(operation_id, &operation, preconditions)
        {
            return Ok(outcome);
        }
        let Some(destination) = operation.destination.clone() else {
            return Ok(conflict(operation_id, "remote_rename_destination_missing"));
        };
        if self.files.contains_key(&destination.0) {
            return Ok(conflict(operation_id, "remote_destination_already_exists"));
        }
        let Some(existing) = self.files.get(&operation.path.0) else {
            return Ok(conflict(operation_id, "remote_file_missing"));
        };
        if preconditions.file_content_version != existing.content_version {
            return Ok(stale(operation_id, "remote_file_content_version_mismatch"));
        }
        if preconditions.snapshot_id != existing.snapshot_id {
            return Ok(stale(operation_id, "remote_snapshot_mismatch"));
        }
        if preconditions.expected_fingerprint.as_ref() != Some(&existing.fingerprint) {
            return Ok(stale(operation_id, "remote_fingerprint_mismatch"));
        }
        let mut entry = self
            .files
            .remove(&operation.path.0)
            .expect("file checked above");
        entry.snapshot_id =
            derived_snapshot_id(&destination.0, entry.content_version, operation_id);
        self.files.insert(destination.0.clone(), entry);
        Ok(
            RemoteOperationOutcome::new(operation_id, RemoteOperationDisposition::Accepted, None)
                .with_snapshot(self.snapshot_for_path(&destination)),
        )
    }

    fn validate_mutation_gate(
        &self,
        operation_id: RemoteOperationId,
        operation: &RemoteFilesystemOperation,
        preconditions: &RemoteWritePreconditions,
    ) -> Option<RemoteOperationOutcome> {
        if operation.proposal_id.is_none() {
            return Some(denied(operation_id, "remote_mutation_requires_proposal"));
        }
        if !preconditions.has_required_write_guards() {
            return Some(denied(operation_id, "remote_write_guard_invalid"));
        }
        if validate_capability(&preconditions.capability_decision, "remote.fs.write").is_err() {
            return Some(denied(operation_id, "remote_write_capability_denied"));
        }
        if preconditions.principal_id != self.descriptor.authority.principal_id {
            return Some(denied(operation_id, "remote_write_principal_mismatch"));
        }
        if preconditions.workspace_generation != self.workspace_generation {
            return Some(stale(operation_id, "remote_workspace_generation_mismatch"));
        }
        None
    }

    fn handle_process_descriptor(
        &self,
        operation_id: RemoteOperationId,
        descriptor: RemoteProcessDescriptor,
    ) -> Result<RemoteOperationOutcome, RemoteRuntimeError> {
        if !self.config.execution_enabled {
            return Ok(denied(operation_id, "remote_execution_disabled"));
        }
        validate_capability(&descriptor.capability_decision, "remote.process.launch")?;
        validate_cancellation_token(descriptor.cancellation_token_id)?;
        if descriptor.session_id != self.session_id()
            || descriptor.operation_id.0 == 0
            || descriptor.operation_id != operation_id
            || descriptor.command_label.trim().is_empty()
            || descriptor.schema_version == 0
        {
            return Err(RemoteRuntimeError::InvalidOperation {
                reason: "process descriptor identity is invalid".to_string(),
            });
        }
        if descriptor.output_byte_limit == 0
            || descriptor.output_byte_limit > self.config.max_output_bytes
        {
            return Ok(denied(operation_id, "remote_process_output_limit_denied"));
        }
        Ok(RemoteOperationOutcome::new(
            operation_id,
            RemoteOperationDisposition::Accepted,
            None,
        ))
    }

    fn handle_pty_descriptor(
        &self,
        operation_id: RemoteOperationId,
        descriptor: RemotePtyDescriptor,
    ) -> Result<RemoteOperationOutcome, RemoteRuntimeError> {
        if !self.config.execution_enabled {
            return Ok(denied(operation_id, "remote_pty_disabled"));
        }
        validate_capability(&descriptor.capability_decision, "remote.pty.input")?;
        if descriptor.session_id != self.session_id()
            || descriptor.terminal_session_id.0 == 0
            || descriptor.columns == 0
            || descriptor.rows == 0
            || descriptor.schema_version == 0
        {
            return Err(RemoteRuntimeError::InvalidOperation {
                reason: "PTY descriptor identity is invalid".to_string(),
            });
        }
        if descriptor.transcript_byte_limit == 0
            || descriptor.transcript_byte_limit > self.config.max_output_bytes
        {
            return Ok(denied(operation_id, "remote_pty_transcript_limit_denied"));
        }
        Ok(RemoteOperationOutcome::new(
            operation_id,
            RemoteOperationDisposition::Accepted,
            None,
        ))
    }

    fn handle_lsp_descriptor(
        &self,
        operation_id: RemoteOperationId,
        descriptor: legion_protocol::RemoteLspDescriptor,
    ) -> Result<RemoteOperationOutcome, RemoteRuntimeError> {
        if !self.config.lsp_enabled {
            return Ok(denied(operation_id, "remote_lsp_disabled"));
        }
        validate_capability(&descriptor.capability_decision, "remote.lsp.launch")?;
        validate_cancellation_token(descriptor.cancellation_token_id)?;
        if descriptor.session_id != self.session_id()
            || descriptor.language_server_id.0 == 0
            || descriptor.request_id.0.is_nil()
            || descriptor.language_id.0.trim().is_empty()
            || descriptor.schema_version == 0
        {
            return Err(RemoteRuntimeError::InvalidOperation {
                reason: "LSP descriptor identity is invalid".to_string(),
            });
        }
        Ok(RemoteOperationOutcome::new(
            operation_id,
            RemoteOperationDisposition::Accepted,
            None,
        ))
    }

    fn handle_semantic_query_descriptor(
        &self,
        operation_id: RemoteOperationId,
        descriptor: RemoteSemanticQueryDescriptor,
    ) -> Result<RemoteOperationOutcome, RemoteRuntimeError> {
        if !self.config.semantic_query_enabled {
            return Ok(denied(operation_id, "remote_semantic_query_disabled"));
        }
        validate_capability(&descriptor.capability_decision, "remote.semantic.query")?;
        if descriptor.session_id != self.session_id()
            || descriptor.query_id.0.is_nil()
            || descriptor.purpose.trim().is_empty()
            || descriptor.max_results == 0
            || descriptor.schema_version == 0
            || descriptor.redaction_hints.contains(&RedactionHint::None)
        {
            return Err(RemoteRuntimeError::InvalidOperation {
                reason: "semantic query descriptor identity or redaction metadata is invalid"
                    .to_string(),
            });
        }
        Ok(RemoteOperationOutcome::new(
            operation_id,
            RemoteOperationDisposition::Accepted,
            None,
        ))
    }

    fn handle_checkpoint(
        &mut self,
        operation_id: RemoteOperationId,
        checkpoint: RemoteOperationLogCheckpoint,
    ) -> Result<RemoteOperationOutcome, RemoteRuntimeError> {
        if checkpoint.session_id != self.session_id()
            || checkpoint.checkpoint_id.0 == 0
            || checkpoint.last_operation_id.0 == 0
            || checkpoint.event_sequence.0 == 0
            || checkpoint.schema_version == 0
        {
            return Err(RemoteRuntimeError::InvalidOperation {
                reason: "operation checkpoint identity is invalid".to_string(),
            });
        }
        self.network_health = checkpoint.network_health;
        self.checkpoints.push(checkpoint.clone());
        Ok(
            RemoteOperationOutcome::new(operation_id, RemoteOperationDisposition::Accepted, None)
                .with_checkpoint(checkpoint),
        )
    }

    fn handle_offline_resume(
        &self,
        operation_id: RemoteOperationId,
        manifest: RemoteOfflineResumeManifest,
    ) -> Result<RemoteOperationOutcome, RemoteRuntimeError> {
        if !self.config.offline_resume_enabled {
            return Ok(denied(operation_id, "remote_offline_resume_disabled"));
        }
        if manifest.session_id != self.session_id()
            || manifest.workspace_generation.0 == 0
            || manifest.snapshot_id.0 == 0
            || manifest.correlation_id.0 == 0
            || manifest.causality_id.0.is_nil()
            || manifest.event_sequence.0 == 0
            || manifest.schema_version == 0
        {
            return Err(RemoteRuntimeError::InvalidOperation {
                reason: "offline resume manifest identity is invalid".to_string(),
            });
        }
        if manifest.workspace_generation != self.workspace_generation {
            return Ok(stale(operation_id, "remote_resume_generation_mismatch"));
        }
        let known = self
            .checkpoints
            .iter()
            .map(|checkpoint| checkpoint.checkpoint_id)
            .collect::<HashSet<_>>();
        if manifest
            .checkpoints
            .iter()
            .any(|checkpoint| !known.contains(checkpoint))
        {
            return Ok(RemoteOperationOutcome::new(
                operation_id,
                RemoteOperationDisposition::GapDetected,
                Some("remote_resume_checkpoint_gap"),
            ));
        }
        Ok(RemoteOperationOutcome::new(
            operation_id,
            RemoteOperationDisposition::Accepted,
            None,
        ))
    }

    fn handle_audit(
        &self,
        operation_id: RemoteOperationId,
        record: RemoteAuditRecord,
    ) -> Result<RemoteOperationOutcome, RemoteRuntimeError> {
        if record.session_id != self.session_id() || !record.is_metadata_only_valid() {
            return Err(RemoteRuntimeError::InvalidOperation {
                reason: "remote audit record is not metadata-only valid".to_string(),
            });
        }
        Ok(RemoteOperationOutcome::new(
            operation_id,
            RemoteOperationDisposition::Noop,
            None,
        ))
    }

    fn snapshot_for_path(&self, path: &CanonicalPath) -> RemoteFilesystemSnapshot {
        let entry = self.files.get(&path.0);
        RemoteFilesystemSnapshot {
            session_id: self.session_id(),
            workspace_id: self.descriptor.authority.workspace_id,
            workspace_generation: self.workspace_generation,
            snapshot_id: entry.map_or(SnapshotId(1), |entry| entry.snapshot_id),
            file_id: entry.map(|entry| entry.file_id),
            file_content_version: entry.map(|entry| entry.content_version),
            fingerprint: entry.map(|entry| entry.fingerprint.clone()),
            byte_len: entry.map(|entry| entry.content.len() as u64).or(Some(0)),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        }
    }
}

/// Registry for deterministic remote workspace sessions.
#[derive(Debug, Clone)]
pub struct RemoteDevelopmentRuntime {
    config: RemoteRuntimeConfig,
    sessions: HashMap<RemoteWorkspaceSessionId, RemoteSessionRuntime>,
}

impl RemoteDevelopmentRuntime {
    /// Creates a remote runtime registry with explicit configuration.
    pub fn new(config: RemoteRuntimeConfig) -> Self {
        Self {
            config,
            sessions: HashMap::new(),
        }
    }

    /// Creates and stores a remote session.
    pub fn create_session(
        &mut self,
        descriptor: RemoteWorkspaceSessionDescriptor,
        workspace_generation: WorkspaceGeneration,
    ) -> Result<(), RemoteRuntimeError> {
        if self.sessions.len() >= self.config.max_sessions {
            return Err(RemoteRuntimeError::LimitExceeded {
                reason: "remote session count exceeds configured limit".to_string(),
            });
        }
        let session = RemoteSessionRuntime::new(
            descriptor.clone(),
            workspace_generation,
            self.config.clone(),
        )?;
        self.sessions.insert(descriptor.session_id, session);
        Ok(())
    }

    /// Returns a session by identifier.
    pub fn session(
        &self,
        session_id: RemoteWorkspaceSessionId,
    ) -> Result<&RemoteSessionRuntime, RemoteRuntimeError> {
        self.sessions
            .get(&session_id)
            .ok_or(RemoteRuntimeError::SessionMissing { session_id })
    }

    /// Returns a mutable session by identifier.
    pub fn session_mut(
        &mut self,
        session_id: RemoteWorkspaceSessionId,
    ) -> Result<&mut RemoteSessionRuntime, RemoteRuntimeError> {
        self.sessions
            .get_mut(&session_id)
            .ok_or(RemoteRuntimeError::SessionMissing { session_id })
    }

    /// Handles a remote transport envelope by dispatching to the owning session.
    pub fn handle_transport_envelope(
        &mut self,
        envelope: RemoteTransportEnvelope,
    ) -> Result<RemoteOperationOutcome, RemoteRuntimeError> {
        self.session_mut(envelope.session_id)?
            .handle_transport_envelope(envelope)
    }

    /// Returns current session descriptors in deterministic order.
    pub fn session_descriptors(&self) -> Vec<RemoteWorkspaceSessionDescriptor> {
        let mut descriptors = self
            .sessions
            .values()
            .map(|session| session.descriptor().clone())
            .collect::<Vec<_>>();
        descriptors.sort_by_key(|descriptor| descriptor.session_id.0);
        descriptors
    }
}

impl Default for RemoteDevelopmentRuntime {
    fn default() -> Self {
        Self::new(RemoteRuntimeConfig::default())
    }
}

fn validate_session_descriptor(
    descriptor: &RemoteWorkspaceSessionDescriptor,
) -> Result<(), RemoteRuntimeError> {
    if descriptor.session_id.0 == 0
        || descriptor.authority.authority_id.0 == 0
        || descriptor.authority.workspace_id.0 == 0
        || descriptor.authority.principal_id.0.trim().is_empty()
        || descriptor.agent.agent_id.0 == 0
        || descriptor.agent.authority_id != descriptor.authority.authority_id
        || descriptor.schema_version == 0
        || descriptor.authority.schema_version == 0
        || descriptor.agent.schema_version == 0
    {
        return Err(RemoteRuntimeError::InvalidSession {
            reason:
                "session, authority, agent, workspace, principal, and schema metadata are required"
                    .to_string(),
        });
    }
    if descriptor
        .authority
        .redaction_hints
        .contains(&RedactionHint::None)
    {
        return Err(RemoteRuntimeError::InvalidSession {
            reason: "remote authority metadata must not request raw retention".to_string(),
        });
    }
    Ok(())
}

fn validate_capability(
    decision: &CapabilityDecision,
    expected: &str,
) -> Result<(), RemoteRuntimeError> {
    if decision.decision_id.0 == 0 || !decision.granted || decision.capability.0 != expected {
        return Err(RemoteRuntimeError::PolicyDenied {
            reason: format!("capability {expected} was not granted"),
        });
    }
    Ok(())
}

fn validate_cancellation_token(token: CancellationTokenId) -> Result<(), RemoteRuntimeError> {
    if token.0 == Uuid::nil() {
        return Err(RemoteRuntimeError::InvalidOperation {
            reason: "cancellation token must be non-nil".to_string(),
        });
    }
    Ok(())
}

fn denied(operation_id: RemoteOperationId, reason: &str) -> RemoteOperationOutcome {
    RemoteOperationOutcome::new(
        operation_id,
        RemoteOperationDisposition::Denied,
        Some(reason),
    )
}

fn stale(operation_id: RemoteOperationId, reason: &str) -> RemoteOperationOutcome {
    RemoteOperationOutcome::new(
        operation_id,
        RemoteOperationDisposition::Stale,
        Some(reason),
    )
}

fn conflict(operation_id: RemoteOperationId, reason: &str) -> RemoteOperationOutcome {
    RemoteOperationOutcome::new(
        operation_id,
        RemoteOperationDisposition::Conflict,
        Some(reason),
    )
}

fn metadata_fingerprint(content: &str) -> FileFingerprint {
    FileFingerprint {
        algorithm: "legion-remote-fixture".to_string(),
        value: format!("len:{}:hash:{}", content.len(), stable_hash_u128(content)),
    }
}

/// Derives a fresh post-mutation snapshot id bound to the path, resulting content
/// version, and accepting operation so each accepted mutation advances snapshot state.
fn derived_snapshot_id(
    path: &str,
    content_version: FileContentVersion,
    operation_id: RemoteOperationId,
) -> SnapshotId {
    let seed = format!("{path}:{}:{}", content_version.0, operation_id.0);
    SnapshotId(stable_hash_u128(&seed) | 1)
}

fn stable_hash_u128(value: &str) -> u128 {
    let mut hash: u128 = 0xcbf2_9ce4_8422_2325;
    for byte in value.bytes() {
        hash ^= byte as u128;
        hash = hash.wrapping_mul(0x100_0000_01b3);
    }
    hash
}

/// Configuration for HTTP Legion Cloud Lane transport.
///
/// The `Debug` implementation intentionally redacts the auth token value.
#[derive(Clone)]
pub struct HttpLegionCloudLaneTransportConfig {
    /// Base URL for the cloud control plane (e.g., `https://cloud.example.invalid`).
    pub base_url: String,
    /// Request timeout.
    pub timeout: std::time::Duration,
    /// Display-safe client identity label used for observability and correlation.
    pub client_identity_label: String,
    /// Optional auth token as `(label, value)`. The value is redacted in logs.
    pub auth_token: Option<(String, String)>,
}

impl std::fmt::Debug for HttpLegionCloudLaneTransportConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut builder = f.debug_struct("HttpLegionCloudLaneTransportConfig");
        builder
            .field("base_url", &self.base_url)
            .field("timeout", &self.timeout)
            .field("client_identity_label", &self.client_identity_label)
            .field(
                "auth_token",
                &self
                    .auth_token
                    .as_ref()
                    .map(|(label, _)| format!("{label}:<redacted>")),
            )
            .finish()
    }
}

/// Production HTTP JSON transport for the Legion Cloud Lane.
pub struct HttpLegionCloudLaneTransport {
    client: reqwest::blocking::Client,
    config: HttpLegionCloudLaneTransportConfig,
}

impl std::fmt::Debug for HttpLegionCloudLaneTransport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HttpLegionCloudLaneTransport")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

fn ensure_rustls_provider() {
    use std::sync::Once;
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        rustls::crypto::ring::default_provider()
            .install_default()
            .expect("rustls ring provider install");
    });
}

impl HttpLegionCloudLaneTransport {
    /// Construct a transport from explicit configuration.
    pub fn new(config: HttpLegionCloudLaneTransportConfig) -> Result<Self, RemoteRuntimeError> {
        ensure_rustls_provider();
        let client = reqwest::blocking::Client::builder()
            .timeout(config.timeout)
            .build()
            .map_err(|err| RemoteRuntimeError::Transport {
                reason: format!("failed to build HTTP client: {err}"),
            })?;
        Ok(Self { client, config })
    }

    fn url(&self, path: &str) -> String {
        format!("{}{path}", self.config.base_url.trim_end_matches('/'))
    }

    fn common_headers(&self) -> Result<reqwest::header::HeaderMap, RemoteRuntimeError> {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            reqwest::header::CONTENT_TYPE,
            reqwest::header::HeaderValue::from_static("application/json"),
        );
        if let Some((label, value)) = &self.config.auth_token {
            let auth_value = format!("{label} {value}");
            let header_value =
                reqwest::header::HeaderValue::from_str(&auth_value).map_err(|err| {
                    RemoteRuntimeError::InvalidOperation {
                        reason: format!("configured authorization header value is invalid: {err}"),
                    }
                })?;
            headers.insert(reqwest::header::AUTHORIZATION, header_value);
        }
        let identity_value = reqwest::header::HeaderValue::from_str(
            &self.config.client_identity_label,
        )
        .map_err(|err| RemoteRuntimeError::InvalidOperation {
            reason: format!("configured client identity header value is invalid: {err}"),
        })?;
        headers.insert("X-Legion-Client-Identity", identity_value);
        Ok(headers)
    }

    fn send_with_body(
        &self,
        method: reqwest::Method,
        url: &str,
        body: Option<&serde_json::Value>,
    ) -> Result<reqwest::blocking::Response, RemoteRuntimeError> {
        let mut request = self.client.request(method, url);
        request = request.headers(self.common_headers()?);
        if let Some(body) = body {
            request = request.json(body);
        }
        let response = request
            .send()
            .map_err(|err| RemoteRuntimeError::Transport {
                reason: format!("HTTP request failed: {err}"),
            })?;
        Ok(response)
    }

    fn expect_ok(
        response: reqwest::blocking::Response,
    ) -> Result<reqwest::blocking::Response, RemoteRuntimeError> {
        let status = response.status().as_u16();
        if !response.status().is_success() {
            let reason = response
                .text()
                .unwrap_or_else(|_| "unable to read error body".to_string());
            return Err(RemoteRuntimeError::HttpResponse { status, reason });
        }
        Ok(response)
    }

    fn parse_json<T: serde::de::DeserializeOwned>(
        response: reqwest::blocking::Response,
    ) -> Result<T, RemoteRuntimeError> {
        response
            .json()
            .map_err(|err| RemoteRuntimeError::Serialization {
                reason: format!("failed to deserialize response body: {err}"),
            })
    }
}

impl LegionCloudLaneTransport for HttpLegionCloudLaneTransport {
    fn submit_task(
        &mut self,
        request: &LegionCloudLaneTaskRequest,
    ) -> Result<LegionCloudLaneTaskStatus, RemoteRuntimeError> {
        let body =
            serde_json::to_value(request).map_err(|err| RemoteRuntimeError::Serialization {
                reason: format!("failed to serialize request body: {err}"),
            })?;
        let response = self.send_with_body(
            reqwest::Method::POST,
            &self.url("/v1/cloud/tasks"),
            Some(&body),
        )?;
        let response = Self::expect_ok(response)?;
        Self::parse_json(response)
    }

    fn stream_task_events(
        &mut self,
        task_id: &LegionCloudLaneTaskId,
    ) -> Result<Vec<LegionCloudLaneTaskEvent>, RemoteRuntimeError> {
        let url = self.url(&format!(
            "/v1/cloud/tasks/{}/events",
            encode_path_segment(&task_id.0)
        ));
        let response = self.send_with_body(reqwest::Method::GET, &url, None)?;
        let response = Self::expect_ok(response)?;
        Self::parse_json(response)
    }

    fn cancel_task(
        &mut self,
        task_id: &LegionCloudLaneTaskId,
        cancellation_token: CancellationTokenId,
        reason_label: &str,
    ) -> Result<LegionCloudLaneTaskStatus, RemoteRuntimeError> {
        let body = serde_json::json!({
            "cancellation_token": cancellation_token.0,
            "reason_label": reason_label,
        });
        let url = self.url(&format!(
            "/v1/cloud/tasks/{}/cancel",
            encode_path_segment(&task_id.0)
        ));
        let response = self.send_with_body(reqwest::Method::POST, &url, Some(&body))?;
        let response = Self::expect_ok(response)?;
        Self::parse_json(response)
    }

    fn fetch_task_proposal(
        &mut self,
        task_id: &LegionCloudLaneTaskId,
    ) -> Result<LegionCloudLaneProposalResponse, RemoteRuntimeError> {
        let url = self.url(&format!(
            "/v1/cloud/tasks/{}/proposal",
            encode_path_segment(&task_id.0)
        ));
        let response = self.send_with_body(reqwest::Method::GET, &url, None)?;
        let response = Self::expect_ok(response)?;
        Self::parse_json(response)
    }

    fn fetch_task_evidence(
        &mut self,
        task_id: &LegionCloudLaneTaskId,
    ) -> Result<Vec<LegionEvidenceRecord>, RemoteRuntimeError> {
        let url = self.url(&format!(
            "/v1/cloud/tasks/{}/evidence",
            encode_path_segment(&task_id.0)
        ));
        let response = self.send_with_body(reqwest::Method::GET, &url, None)?;
        let response = Self::expect_ok(response)?;
        Self::parse_json(response)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use legion_protocol::{
        AssistedAiProviderClass, CapabilityDecisionId, CapabilityId, CausalityId, LanguageId,
        LanguageServerId, LegionCloudLaneBudget, LegionCloudLaneSecretScanStatus,
        LegionCloudLaneTaskId, LegionCloudLaneTaskRequest, LegionCloudLaneTaskState,
        LegionCloudLaneUploadManifest, LegionEvidenceKind, LegionEvidencePrivacyScope,
        LegionEvidenceRecord, LegionEvidenceSource, LegionModelCapability,
        LegionProviderLocalityPreference, LegionProviderPrivacyPolicy, LegionProviderRouteHealth,
        LegionProviderRouteMetadata, LegionTaskContextRef, LegionTaskContextRefKind,
        LegionTaskFileScope, LegionTaskOutputContract, LegionTaskPacket, LegionTaskPacketId,
        LegionTaskPolicy, LegionTaskValidationPlan, LegionWorkerResult, LegionWorkerResultKind,
        LspRequestId, PrincipalId, RemoteAgentDescriptor, RemoteAuthorityDescriptor,
        RemoteOperationLogCheckpointId, SemanticQueryId, TerminalSessionId, TimestampMillis,
        WorkspaceId, WorkspaceTrustState,
    };

    fn causality_id() -> CausalityId {
        CausalityId(Uuid::parse_str("cccccccc-cccc-cccc-cccc-cccccccccccc").unwrap())
    }

    fn session_descriptor(
        state: RemoteWorkspaceLifecycleState,
    ) -> RemoteWorkspaceSessionDescriptor {
        RemoteWorkspaceSessionDescriptor {
            session_id: RemoteWorkspaceSessionId(7001),
            authority: RemoteAuthorityDescriptor {
                authority_id: legion_protocol::RemoteAuthorityId(7101),
                authority_label: "edge-authority:hash".to_string(),
                workspace_id: WorkspaceId(11),
                trust_state: WorkspaceTrustState::Trusted,
                principal_id: PrincipalId("principal-remote".to_string()),
                redaction_hints: vec![RedactionHint::MetadataOnly],
                schema_version: 1,
            },
            agent: RemoteAgentDescriptor {
                agent_id: legion_protocol::RemoteAgentId(7201),
                authority_id: legion_protocol::RemoteAuthorityId(7101),
                agent_version: "test-agent/1".to_string(),
                runtime_enabled: true,
                schema_version: 1,
            },
            state,
            granted_capabilities: vec![],
            created_at: TimestampMillis(1700),
            last_heartbeat_at: Some(TimestampMillis(1800)),
            schema_version: 1,
        }
    }

    fn capability(name: &str) -> CapabilityDecision {
        CapabilityDecision {
            decision_id: legion_protocol::CapabilityDecisionId(1),
            granted: true,
            capability: CapabilityId(name.to_string()),
            reason: None,
        }
    }

    fn write_preconditions(fingerprint: FileFingerprint) -> RemoteWritePreconditions {
        RemoteWritePreconditions {
            capability_decision: capability("remote.fs.write"),
            principal_id: PrincipalId("principal-remote".to_string()),
            expected_fingerprint: Some(fingerprint),
            file_content_version: FileContentVersion(1),
            workspace_generation: WorkspaceGeneration(1),
            buffer_version: Some(legion_protocol::BufferVersion(1)),
            snapshot_id: SnapshotId(66),
            correlation_id: CorrelationId(901),
            causality_id: causality_id(),
        }
    }

    #[derive(Debug, Default)]
    struct RecordingCloudLaneTransport {
        calls: Vec<&'static str>,
    }

    impl LegionCloudLaneTransport for RecordingCloudLaneTransport {
        fn submit_task(
            &mut self,
            request: &LegionCloudLaneTaskRequest,
        ) -> Result<LegionCloudLaneTaskStatus, RemoteRuntimeError> {
            self.calls.push("submit");
            Ok(LegionCloudLaneTaskStatus {
                task_id: request.task_id.clone(),
                state: LegionCloudLaneTaskState::Submitted,
                status_label: "submitted".to_string(),
                estimated_cost_cents: request.budget.estimated_cost_cents,
                billed_cost_cents: 0,
                queue_position: Some(1),
                event_sequence: EventSequence(1),
                generated_at: TimestampMillis(1700),
                redaction_hints: vec![RedactionHint::MetadataOnly],
                schema_version: 1,
            })
        }

        fn stream_task_events(
            &mut self,
            task_id: &LegionCloudLaneTaskId,
        ) -> Result<Vec<LegionCloudLaneTaskEvent>, RemoteRuntimeError> {
            self.calls.push("events");
            Ok(vec![LegionCloudLaneTaskEvent {
                task_id: task_id.clone(),
                event_id: "event:queued".to_string(),
                state: LegionCloudLaneTaskState::Queued,
                event_label: "queued for validation lane".to_string(),
                event_sequence: EventSequence(2),
                generated_at: TimestampMillis(1710),
                redaction_hints: vec![RedactionHint::MetadataOnly],
                schema_version: 1,
            }])
        }

        fn cancel_task(
            &mut self,
            task_id: &LegionCloudLaneTaskId,
            _cancellation_token: CancellationTokenId,
            reason_label: &str,
        ) -> Result<LegionCloudLaneTaskStatus, RemoteRuntimeError> {
            self.calls.push("cancel");
            Ok(LegionCloudLaneTaskStatus {
                task_id: task_id.clone(),
                state: LegionCloudLaneTaskState::Cancelled,
                status_label: reason_label.to_string(),
                estimated_cost_cents: 0,
                billed_cost_cents: 0,
                queue_position: None,
                event_sequence: EventSequence(3),
                generated_at: TimestampMillis(1720),
                redaction_hints: vec![RedactionHint::MetadataOnly],
                schema_version: 1,
            })
        }

        fn fetch_task_proposal(
            &mut self,
            task_id: &LegionCloudLaneTaskId,
        ) -> Result<LegionCloudLaneProposalResponse, RemoteRuntimeError> {
            self.calls.push("proposal");
            Ok(LegionCloudLaneProposalResponse {
                task_id: task_id.clone(),
                proposal_id: Some(ProposalId(9001)),
                worker_result: Some(cloud_worker_result(task_id)),
                redaction_hints: vec![RedactionHint::MetadataOnly],
                schema_version: 1,
            })
        }

        fn fetch_task_evidence(
            &mut self,
            task_id: &LegionCloudLaneTaskId,
        ) -> Result<Vec<LegionEvidenceRecord>, RemoteRuntimeError> {
            self.calls.push("evidence");
            Ok(vec![cloud_evidence(task_id)])
        }
    }

    #[derive(Debug, Default)]
    struct MismatchedCloudLaneTransport;

    impl LegionCloudLaneTransport for MismatchedCloudLaneTransport {
        fn submit_task(
            &mut self,
            request: &LegionCloudLaneTaskRequest,
        ) -> Result<LegionCloudLaneTaskStatus, RemoteRuntimeError> {
            Ok(LegionCloudLaneTaskStatus {
                task_id: mismatched_cloud_task_id(),
                state: LegionCloudLaneTaskState::Submitted,
                status_label: "submitted".to_string(),
                estimated_cost_cents: request.budget.estimated_cost_cents,
                billed_cost_cents: 0,
                queue_position: Some(1),
                event_sequence: EventSequence(1),
                generated_at: TimestampMillis(1700),
                redaction_hints: vec![RedactionHint::MetadataOnly],
                schema_version: 1,
            })
        }

        fn stream_task_events(
            &mut self,
            _task_id: &LegionCloudLaneTaskId,
        ) -> Result<Vec<LegionCloudLaneTaskEvent>, RemoteRuntimeError> {
            Ok(vec![LegionCloudLaneTaskEvent {
                task_id: mismatched_cloud_task_id(),
                event_id: "event:queued".to_string(),
                state: LegionCloudLaneTaskState::Queued,
                event_label: "queued for validation lane".to_string(),
                event_sequence: EventSequence(2),
                generated_at: TimestampMillis(1710),
                redaction_hints: vec![RedactionHint::MetadataOnly],
                schema_version: 1,
            }])
        }

        fn cancel_task(
            &mut self,
            _task_id: &LegionCloudLaneTaskId,
            _cancellation_token: CancellationTokenId,
            reason_label: &str,
        ) -> Result<LegionCloudLaneTaskStatus, RemoteRuntimeError> {
            Ok(LegionCloudLaneTaskStatus {
                task_id: mismatched_cloud_task_id(),
                state: LegionCloudLaneTaskState::Cancelled,
                status_label: reason_label.to_string(),
                estimated_cost_cents: 0,
                billed_cost_cents: 0,
                queue_position: None,
                event_sequence: EventSequence(3),
                generated_at: TimestampMillis(1720),
                redaction_hints: vec![RedactionHint::MetadataOnly],
                schema_version: 1,
            })
        }

        fn fetch_task_proposal(
            &mut self,
            _task_id: &LegionCloudLaneTaskId,
        ) -> Result<LegionCloudLaneProposalResponse, RemoteRuntimeError> {
            let task_id = mismatched_cloud_task_id();
            Ok(LegionCloudLaneProposalResponse {
                task_id: task_id.clone(),
                proposal_id: Some(ProposalId(9001)),
                worker_result: Some(cloud_worker_result(&task_id)),
                redaction_hints: vec![RedactionHint::MetadataOnly],
                schema_version: 1,
            })
        }

        fn fetch_task_evidence(
            &mut self,
            task_id: &LegionCloudLaneTaskId,
        ) -> Result<Vec<LegionEvidenceRecord>, RemoteRuntimeError> {
            Ok(vec![cloud_evidence(task_id)])
        }
    }

    fn cloud_packet() -> LegionTaskPacket {
        LegionTaskPacket {
            packet_id: LegionTaskPacketId("cloud-packet:remote:1".to_string()),
            workspace_id: WorkspaceId(11),
            objective_summary_hash: FileFingerprint {
                algorithm: "sha256".to_string(),
                value: "objective".to_string(),
            },
            allowed_files: vec![LegionTaskFileScope {
                scope_id: "allowed:src-lib".to_string(),
                path: CanonicalPath("/workspace/src/lib.rs".to_string()),
                fingerprint: None,
                redaction_hints: vec![RedactionHint::MetadataOnly],
                schema_version: 1,
            }],
            forbidden_files: vec![LegionTaskFileScope {
                scope_id: "forbidden:env".to_string(),
                path: CanonicalPath("/workspace/.env".to_string()),
                fingerprint: None,
                redaction_hints: vec![RedactionHint::MetadataOnly],
                schema_version: 1,
            }],
            context_snippet_refs: vec![LegionTaskContextRef {
                reference_id: "snippet:1".to_string(),
                kind: LegionTaskContextRefKind::ContextSnippet,
                payload_hash: metadata_fingerprint("snippet"),
                redacted_summary: "redacted snippet".to_string(),
                redaction_hints: vec![RedactionHint::MetadataOnly],
                schema_version: 1,
            }],
            full_file_refs: Vec::new(),
            command_output_refs: Vec::new(),
            output_contract: LegionTaskOutputContract {
                expected_result_kind: LegionWorkerResultKind::PatchProposal,
                proposal_only: true,
                direct_mutation_allowed: false,
                required_evidence_kinds: vec![LegionEvidenceKind::CommandRun],
                redaction_hints: vec![RedactionHint::MetadataOnly],
                schema_version: 1,
            },
            validation_plan: LegionTaskValidationPlan {
                required_commands: vec!["cargo test -p legion-remote --all-targets".to_string()],
                success_criteria: vec!["remote cloud lane test passes".to_string()],
                stop_conditions: vec!["policy denied".to_string()],
                redaction_hints: vec![RedactionHint::MetadataOnly],
                schema_version: 1,
            },
            policy: LegionTaskPolicy {
                locality_preference: LegionProviderLocalityPreference::RemoteAllowed,
                privacy_policy: LegionProviderPrivacyPolicy::MetadataOnly,
                cost_budget_cents: Some(75),
                latency_budget_ms: Some(30_000),
                allow_network: true,
                allow_direct_workspace_mutation: false,
                redaction_hints: vec![RedactionHint::MetadataOnly],
                schema_version: 1,
            },
            correlation_id: CorrelationId(901),
            causality_id: causality_id(),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        }
    }

    fn cloud_request() -> LegionCloudLaneTaskRequest {
        LegionCloudLaneTaskRequest {
            task_id: LegionCloudLaneTaskId("cloud-task:remote:1".to_string()),
            lane_id: "cloud-lane:validation".to_string(),
            control_plane_endpoint_id: "endpoint:legion-cloud:test".to_string(),
            task_packet: cloud_packet(),
            upload_manifest: LegionCloudLaneUploadManifest {
                manifest_id: "upload:remote:1".to_string(),
                allowed_files: vec![LegionTaskFileScope {
                    scope_id: "upload:src".to_string(),
                    path: CanonicalPath("/workspace/src/lib.rs".to_string()),
                    fingerprint: None,
                    redaction_hints: vec![RedactionHint::MetadataOnly],
                    schema_version: 1,
                }],
                forbidden_files: vec![LegionTaskFileScope {
                    scope_id: "upload:forbidden-env".to_string(),
                    path: CanonicalPath("/workspace/.env".to_string()),
                    fingerprint: None,
                    redaction_hints: vec![RedactionHint::MetadataOnly],
                    schema_version: 1,
                }],
                total_upload_bytes: 16_384,
                scope_visible_to_user: true,
                contains_forbidden_material: false,
                secret_scan_status: LegionCloudLaneSecretScanStatus::Passed,
                redaction_hints: vec![RedactionHint::MetadataOnly],
                schema_version: 1,
            },
            budget: LegionCloudLaneBudget {
                max_cost_cents: 75,
                estimated_cost_cents: 50,
                max_queue_depth: 2,
                current_queue_depth: 1,
                usage_metering_label: "meter:remote:unit".to_string(),
                hard_cap_enforced: true,
                redaction_hints: vec![RedactionHint::MetadataOnly],
                schema_version: 1,
            },
            capability_decision: CapabilityDecision {
                decision_id: CapabilityDecisionId(700),
                granted: true,
                capability: CapabilityId("cloud.lane.submit".to_string()),
                reason: Some("allowed".to_string()),
            },
            cancellation_token: CancellationTokenId(
                Uuid::parse_str("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa").unwrap(),
            ),
            correlation_id: CorrelationId(901),
            causality_id: causality_id(),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        }
    }

    fn cloud_evidence(task_id: &LegionCloudLaneTaskId) -> LegionEvidenceRecord {
        LegionEvidenceRecord {
            evidence_id: format!("evidence:{}", task_id.0),
            kind: LegionEvidenceKind::CommandRun,
            source: LegionEvidenceSource::ProviderMetadata,
            payload_hash: metadata_fingerprint("cloud-evidence"),
            redacted_payload_summary: "cloud validation evidence metadata".to_string(),
            command_label: Some("cargo test".to_string()),
            exit_status: Some(0),
            privacy_scope: LegionEvidencePrivacyScope::WorkspaceMetadata,
            generated_at: TimestampMillis(1730),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        }
    }

    fn mismatched_cloud_task_id() -> LegionCloudLaneTaskId {
        LegionCloudLaneTaskId("cloud-task:remote:other".to_string())
    }

    fn cloud_worker_result(task_id: &LegionCloudLaneTaskId) -> LegionWorkerResult {
        LegionWorkerResult {
            result_id: format!("worker-result:{}", task_id.0),
            packet_id: cloud_packet().packet_id,
            result_kind: LegionWorkerResultKind::PatchProposal,
            patch_proposal: Some(ProposalId(9001)),
            documentation_proposal: None,
            analysis_summary: Some("cloud worker returned proposal metadata".to_string()),
            test_plan_summary: Some("cloud validation lane ran configured tests".to_string()),
            blocked_reason: None,
            invalid_reason: None,
            evidence_records: vec![cloud_evidence(task_id)],
            provider_route: Some(LegionProviderRouteMetadata {
                route_id: "cloud-route:validation".to_string(),
                locality_preference: LegionProviderLocalityPreference::RemoteAllowed,
                cost_budget_cents: Some(75),
                latency_budget_ms: Some(30_000),
                privacy_policy: LegionProviderPrivacyPolicy::MetadataOnly,
                model_capability: LegionModelCapability::CodePatch,
                provider_class: AssistedAiProviderClass::HostedRemote,
                route_health: LegionProviderRouteHealth::Healthy,
                labels: vec!["cloud-lane".to_string()],
                redaction_hints: vec![RedactionHint::MetadataOnly],
                schema_version: 1,
            }),
            correlation_id: CorrelationId(901),
            causality_id: causality_id(),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        }
    }

    fn envelope(
        operation_id: RemoteOperationId,
        payload: RemoteTransportPayload,
    ) -> RemoteTransportEnvelope {
        RemoteTransportEnvelope {
            session_id: RemoteWorkspaceSessionId(7001),
            operation_id,
            correlation_id: CorrelationId(901),
            causality_id: causality_id(),
            event_sequence: EventSequence(operation_id.0 as u64),
            principal_id: PrincipalId("principal-remote".to_string()),
            payload,
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        }
    }

    fn runtime() -> RemoteSessionRuntime {
        RemoteSessionRuntime::new(
            session_descriptor(RemoteWorkspaceLifecycleState::Active),
            WorkspaceGeneration(1),
            RemoteRuntimeConfig::enabled(),
        )
        .expect("remote runtime should start")
    }

    fn connection_spec(
        session_id: u128,
        authority_label: &str,
        credential_reference_label: &str,
    ) -> RemoteConnectionSpec {
        RemoteConnectionSpec {
            session_id: RemoteWorkspaceSessionId(session_id),
            authority_id: legion_protocol::RemoteAuthorityId(session_id + 1),
            agent_id: legion_protocol::RemoteAgentId(session_id + 2),
            workspace_id: WorkspaceId(11),
            principal_id: PrincipalId("principal-remote".to_string()),
            authority_label: authority_label.to_string(),
            workspace_root_label: "/workspace/project".to_string(),
            credential_reference_label: credential_reference_label.to_string(),
            agent_version: "legion-remote-agent:metadata".to_string(),
            trust_state: WorkspaceTrustState::Trusted,
            granted_capabilities: default_remote_capabilities(),
        }
    }

    #[test]
    fn ssh_connection_plan_activates_remote_runtime() {
        let plan = plan_ssh_session(connection_spec(
            9101,
            "ssh:principal@example.invalid",
            "credential:ssh-key:test",
        ))
        .expect("ssh plan should be accepted");

        assert_eq!(plan.kind, RemoteConnectionKind::Ssh);
        assert!(plan.descriptor.activation_is_policy_ready());
        let runtime = RemoteSessionRuntime::new(
            plan.descriptor,
            WorkspaceGeneration(1),
            RemoteRuntimeConfig::enabled(),
        )
        .expect("planned ssh descriptor should activate runtime");
        assert_eq!(runtime.state(), RemoteWorkspaceLifecycleState::Active);
    }

    #[test]
    fn devcontainer_connection_plan_parses_config_and_activates_runtime() {
        let plan = plan_devcontainer_session_from_json(
            connection_spec(9201, "devcontainer:test", "credential:docker-context:test"),
            r#"{
                "name": "Rust",
                "image": "mcr.microsoft.com/devcontainers/rust:latest",
                "remoteUser": "vscode",
                "workspaceFolder": "/workspaces/legion",
                "features": {
                    "ghcr.io/devcontainers/features/rust:1": {}
                },
                "mounts": ["source=cache,target=/cache,type=volume"]
            }"#,
        )
        .expect("devcontainer plan should be accepted");

        assert_eq!(plan.kind, RemoteConnectionKind::Devcontainer);
        let devcontainer = plan.devcontainer.expect("devcontainer metadata");
        assert_eq!(devcontainer.name_label, "Rust");
        assert_eq!(devcontainer.feature_count, 1);
        assert_eq!(devcontainer.mount_count, 1);
        let runtime = RemoteSessionRuntime::new(
            plan.descriptor,
            WorkspaceGeneration(1),
            RemoteRuntimeConfig::enabled(),
        )
        .expect("planned devcontainer descriptor should activate runtime");
        assert_eq!(runtime.state(), RemoteWorkspaceLifecycleState::Active);
    }

    #[test]
    fn devcontainer_connection_plan_fails_closed_without_image_or_dockerfile() {
        let error = plan_devcontainer_session_from_json(
            connection_spec(9301, "devcontainer:test", "credential:docker-context:test"),
            r#"{ "name": "missing image" }"#,
        )
        .expect_err("devcontainer without image/dockerfile must fail closed");

        assert!(matches!(
            error,
            RemoteRuntimeError::InvalidSession { reason }
                if reason.contains("image or build.dockerfile")
        ));
    }

    #[test]
    fn remote_runtime_is_default_off() {
        let error = RemoteSessionRuntime::new(
            session_descriptor(RemoteWorkspaceLifecycleState::Active),
            WorkspaceGeneration(1),
            RemoteRuntimeConfig::default(),
        )
        .expect_err("default runtime must be disabled");

        assert!(matches!(error, RemoteRuntimeError::RuntimeDisabled));
    }

    #[test]
    fn remote_session_requires_trusted_active_enabled_descriptor() {
        let inactive = RemoteSessionRuntime::new(
            session_descriptor(RemoteWorkspaceLifecycleState::Degraded),
            WorkspaceGeneration(1),
            RemoteRuntimeConfig::enabled(),
        )
        .expect_err("degraded session is not activation-ready");
        assert!(matches!(inactive, RemoteRuntimeError::PolicyDenied { .. }));

        let mut untrusted = session_descriptor(RemoteWorkspaceLifecycleState::Active);
        untrusted.authority.trust_state = WorkspaceTrustState::Untrusted;
        let error = RemoteSessionRuntime::new(
            untrusted,
            WorkspaceGeneration(1),
            RemoteRuntimeConfig::enabled(),
        )
        .expect_err("untrusted session must deny");
        assert!(matches!(error, RemoteRuntimeError::PolicyDenied { .. }));
    }

    #[test]
    fn remote_filesystem_write_requires_proposal_and_preconditions() {
        let mut runtime = runtime();
        let path = CanonicalPath("/workspace/src/main.rs".to_string());
        let snapshot = runtime
            .seed_file(path.clone(), FileId(33), "fn main() {}")
            .expect("fixture seed should work");
        let mut preconditions = write_preconditions(snapshot.fingerprint.clone().unwrap());
        preconditions.snapshot_id = snapshot.snapshot_id;
        let mut operation = RemoteFilesystemOperation {
            session_id: runtime.session_id(),
            operation_id: RemoteOperationId(8001),
            kind: RemoteFilesystemOperationKind::Write,
            path: path.clone(),
            destination: None,
            write_preconditions: Some(preconditions),
            proposal_id: None,
            schema_version: 1,
        };

        let denied = runtime
            .handle_transport_envelope(envelope(
                RemoteOperationId(8001),
                RemoteTransportPayload::FilesystemOperation(operation.clone()),
            ))
            .expect("denial should be explicit");
        assert_eq!(denied.disposition, RemoteOperationDisposition::Denied);
        assert_eq!(
            runtime.fixture_file_text(&path),
            Some("fn main() {}"),
            "denied remote write must not mutate fixture text"
        );

        operation.operation_id = RemoteOperationId(8002);
        operation.proposal_id = Some(ProposalId(700));
        let accepted = runtime
            .handle_transport_envelope(envelope(
                RemoteOperationId(8002),
                RemoteTransportPayload::FilesystemOperation(operation),
            ))
            .expect("proposal-mediated remote write should be accepted");
        assert_eq!(accepted.disposition, RemoteOperationDisposition::Accepted);
        assert_eq!(
            runtime.fixture_file_text(&path),
            Some("remote-proposal:700:operation:8002")
        );
    }

    #[test]
    fn remote_filesystem_stale_fingerprint_preserves_fixture_text() {
        let mut runtime = runtime();
        let path = CanonicalPath("/workspace/src/lib.rs".to_string());
        runtime
            .seed_file(path.clone(), FileId(34), "pub fn value() {}")
            .expect("fixture seed should work");
        let operation = RemoteFilesystemOperation {
            session_id: runtime.session_id(),
            operation_id: RemoteOperationId(8101),
            kind: RemoteFilesystemOperationKind::Write,
            path: path.clone(),
            destination: None,
            write_preconditions: Some(write_preconditions(metadata_fingerprint("old"))),
            proposal_id: Some(ProposalId(701)),
            schema_version: 1,
        };

        let stale = runtime
            .handle_transport_envelope(envelope(
                RemoteOperationId(8101),
                RemoteTransportPayload::FilesystemOperation(operation),
            ))
            .expect("stale should be explicit");

        assert_eq!(stale.disposition, RemoteOperationDisposition::Stale);
        assert_eq!(runtime.fixture_file_text(&path), Some("pub fn value() {}"));
    }

    #[test]
    fn remote_execution_surfaces_are_policy_gated_and_bounded() {
        let mut runtime = runtime();
        let process = RemoteProcessDescriptor {
            session_id: runtime.session_id(),
            operation_id: RemoteOperationId(8201),
            command_label: "cargo-check".to_string(),
            cwd: Some(CanonicalPath("/workspace".to_string())),
            capability_decision: capability("remote.process.launch"),
            cancellation_token_id: CancellationTokenId(
                Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap(),
            ),
            output_byte_limit: 1024,
            schema_version: 1,
        };
        let accepted = runtime
            .handle_transport_envelope(envelope(
                RemoteOperationId(8201),
                RemoteTransportPayload::Process(process),
            ))
            .expect("process descriptor should be accepted");
        assert_eq!(accepted.disposition, RemoteOperationDisposition::Accepted);

        let pty = RemotePtyDescriptor {
            session_id: runtime.session_id(),
            terminal_session_id: TerminalSessionId(44),
            columns: 120,
            rows: 30,
            transcript_byte_limit: 1024,
            capability_decision: capability("remote.pty.input"),
            schema_version: 1,
        };
        let accepted = runtime
            .handle_transport_envelope(envelope(
                RemoteOperationId(8202),
                RemoteTransportPayload::Pty(pty),
            ))
            .expect("PTY descriptor should be accepted");
        assert_eq!(accepted.disposition, RemoteOperationDisposition::Accepted);

        let lsp = legion_protocol::RemoteLspDescriptor {
            session_id: runtime.session_id(),
            language_server_id: LanguageServerId(55),
            request_id: LspRequestId(
                Uuid::parse_str("22222222-2222-2222-2222-222222222222").unwrap(),
            ),
            language_id: LanguageId("rust".to_string()),
            capability_decision: capability("remote.lsp.launch"),
            cancellation_token_id: CancellationTokenId(
                Uuid::parse_str("33333333-3333-3333-3333-333333333333").unwrap(),
            ),
            schema_version: 1,
        };
        let accepted = runtime
            .handle_transport_envelope(envelope(
                RemoteOperationId(8203),
                RemoteTransportPayload::Lsp(lsp),
            ))
            .expect("LSP descriptor should be accepted");
        assert_eq!(accepted.disposition, RemoteOperationDisposition::Accepted);

        let semantic = RemoteSemanticQueryDescriptor {
            session_id: runtime.session_id(),
            query_id: SemanticQueryId(
                Uuid::parse_str("44444444-4444-4444-4444-444444444444").unwrap(),
            ),
            purpose: "symbol-search".to_string(),
            max_results: 8,
            capability_decision: capability("remote.semantic.query"),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        };
        let accepted = runtime
            .handle_transport_envelope(envelope(
                RemoteOperationId(8204),
                RemoteTransportPayload::SemanticQuery(semantic),
            ))
            .expect("semantic query descriptor should be accepted");
        assert_eq!(accepted.disposition, RemoteOperationDisposition::Accepted);
    }

    #[test]
    fn remote_reconnect_and_offline_resume_are_explicit() {
        let mut runtime = runtime();
        let checkpoint = RemoteOperationLogCheckpoint {
            checkpoint_id: RemoteOperationLogCheckpointId(9001),
            session_id: runtime.session_id(),
            last_operation_id: RemoteOperationId(8204),
            version_vector: legion_protocol::CollaborationVersionVector { entries: vec![] },
            network_health: RemoteNetworkHealthState::Degraded,
            event_sequence: EventSequence(99),
            schema_version: 1,
        };
        let accepted = runtime
            .handle_transport_envelope(envelope(
                RemoteOperationId(8301),
                RemoteTransportPayload::OperationLogCheckpoint(checkpoint),
            ))
            .expect("checkpoint should be accepted");
        assert_eq!(accepted.disposition, RemoteOperationDisposition::Accepted);

        runtime.begin_reconnect();
        assert_eq!(runtime.state(), RemoteWorkspaceLifecycleState::Reconnecting);
        runtime.complete_reconnect().expect("reconnect completes");
        assert_eq!(runtime.network_health(), RemoteNetworkHealthState::Healthy);

        runtime.mark_offline();
        let manifest =
            runtime.offline_resume_manifest(CorrelationId(901), causality_id(), EventSequence(100));
        let accepted = runtime
            .handle_transport_envelope(envelope(
                RemoteOperationId(8302),
                RemoteTransportPayload::OfflineResume(manifest),
            ))
            .expect("offline resume should be accepted");
        assert_eq!(accepted.disposition, RemoteOperationDisposition::Accepted);
    }

    #[test]
    fn remote_audit_records_are_metadata_only_valid() {
        let runtime = runtime();
        let audit = runtime.audit_record(
            Some(RemoteOperationId(8401)),
            Some(ProposalId(700)),
            EventSequence(7),
            CorrelationId(901),
            causality_id(),
        );

        assert!(audit.is_metadata_only_valid());
        assert!(audit.metadata_summary.contains("files=0"));
        assert!(!audit.metadata_summary.contains("raw_source"));
        assert!(!audit.metadata_summary.contains("raw_transcript"));
        assert!(!audit.metadata_summary.contains("process_output"));
    }

    #[test]
    fn cloud_lane_client_submits_streams_cancels_and_fetches_metadata() {
        let mut client = LegionCloudLaneClient::new(
            RecordingCloudLaneTransport::default(),
            LegionCloudLaneClientConfig::enabled_for_tests(),
        );
        let request = cloud_request();

        let status = client
            .submit_task(request.clone())
            .expect("submit cloud task");
        assert_eq!(status.state, LegionCloudLaneTaskState::Submitted);

        let events = client
            .stream_task_events(&request.task_id)
            .expect("stream events");
        assert_eq!(events[0].state, LegionCloudLaneTaskState::Queued);

        let cancelled = client
            .cancel_task(
                &request.task_id,
                request.cancellation_token,
                "user cancelled cloud task",
            )
            .expect("cancel cloud task");
        assert_eq!(cancelled.state, LegionCloudLaneTaskState::Cancelled);

        let proposal = client
            .fetch_task_proposal(&request.task_id)
            .expect("fetch cloud proposal");
        assert_eq!(proposal.proposal_id, Some(ProposalId(9001)));

        let evidence = client
            .fetch_task_evidence(&request.task_id)
            .expect("fetch cloud evidence");
        assert_eq!(evidence.len(), 1);
        assert_eq!(
            client.transport().calls,
            vec!["submit", "events", "cancel", "proposal", "evidence"]
        );
    }

    #[test]
    fn cloud_lane_client_rejects_mismatched_response_task_ids() {
        let request = cloud_request();
        let mut client = LegionCloudLaneClient::new(
            MismatchedCloudLaneTransport,
            LegionCloudLaneClientConfig::enabled_for_tests(),
        );

        let error = client
            .submit_task(request.clone())
            .expect_err("submit status must match requested task id");
        assert!(
            error
                .to_string()
                .contains("does not match requested task id")
        );

        let error = client
            .stream_task_events(&request.task_id)
            .expect_err("events must match requested task id");
        assert!(
            error
                .to_string()
                .contains("does not match requested task id")
        );

        let error = client
            .cancel_task(
                &request.task_id,
                request.cancellation_token,
                "user cancelled cloud task",
            )
            .expect_err("cancel status must match requested task id");
        assert!(
            error
                .to_string()
                .contains("does not match requested task id")
        );

        let error = client
            .fetch_task_proposal(&request.task_id)
            .expect_err("proposal response must match requested task id");
        assert!(
            error
                .to_string()
                .contains("does not match requested task id")
        );
    }

    #[test]
    fn cloud_lane_client_rejects_forbidden_upload_scope_before_transport() {
        let mut client = LegionCloudLaneClient::new(
            RecordingCloudLaneTransport::default(),
            LegionCloudLaneClientConfig::enabled_for_tests(),
        );
        let mut request = cloud_request();
        request.upload_manifest.contains_forbidden_material = true;

        let error = client
            .submit_task(request)
            .expect_err("forbidden upload must fail closed");
        assert!(
            error
                .to_string()
                .contains("cloud upload manifest contains forbidden material")
        );
        assert!(client.transport().calls.is_empty());
    }
}
