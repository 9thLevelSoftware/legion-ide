//! Deterministic, metadata-first remote development runtime harness.

#![warn(missing_docs)]

use std::collections::{HashMap, HashSet};

use devil_protocol::{
    CancellationTokenId, CanonicalPath, CapabilityDecision, CorrelationId, EventSequence,
    FileContentVersion, FileFingerprint, FileId, ProposalId, RedactionHint, RemoteAuditRecord,
    RemoteFilesystemOperation, RemoteFilesystemOperationKind, RemoteFilesystemSnapshot,
    RemoteNetworkHealthState, RemoteOfflineResumeManifest, RemoteOperationId,
    RemoteOperationLogCheckpoint, RemoteProcessDescriptor, RemotePtyDescriptor,
    RemoteSemanticQueryDescriptor, RemoteTransportEnvelope, RemoteTransportPayload,
    RemoteWorkspaceLifecycleState, RemoteWorkspaceSessionDescriptor, RemoteWorkspaceSessionId,
    RemoteWritePreconditions, RetentionLabel, SnapshotId, WorkspaceGeneration,
};
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
        causality_id: devil_protocol::CausalityId,
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
        causality_id: devil_protocol::CausalityId,
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
        entry.snapshot_id = preconditions.snapshot_id;
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
        let entry = RemoteFileEntry {
            file_id: FileId(stable_hash_u128(&operation.path.0) | 1),
            content,
            content_version: FileContentVersion(1),
            snapshot_id: preconditions.snapshot_id,
            fingerprint: metadata_fingerprint("remote-created-by-proposal"),
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
        let entry = self
            .files
            .remove(&operation.path.0)
            .expect("file checked above");
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
        descriptor: devil_protocol::RemoteLspDescriptor,
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
        algorithm: "devil-remote-fixture".to_string(),
        value: format!("len:{}:hash:{}", content.len(), stable_hash_u128(content)),
    }
}

fn stable_hash_u128(value: &str) -> u128 {
    let mut hash: u128 = 0xcbf2_9ce4_8422_2325;
    for byte in value.bytes() {
        hash ^= byte as u128;
        hash = hash.wrapping_mul(0x100_0000_01b3);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;
    use devil_protocol::{
        CapabilityId, CausalityId, LanguageId, LanguageServerId, LspRequestId, PrincipalId,
        RemoteAgentDescriptor, RemoteAuthorityDescriptor, RemoteOperationLogCheckpointId,
        SemanticQueryId, TerminalSessionId, TimestampMillis, WorkspaceId, WorkspaceTrustState,
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
                authority_id: devil_protocol::RemoteAuthorityId(7101),
                authority_label: "edge-authority:hash".to_string(),
                workspace_id: WorkspaceId(11),
                trust_state: WorkspaceTrustState::Trusted,
                principal_id: PrincipalId("principal-remote".to_string()),
                redaction_hints: vec![RedactionHint::MetadataOnly],
                schema_version: 1,
            },
            agent: RemoteAgentDescriptor {
                agent_id: devil_protocol::RemoteAgentId(7201),
                authority_id: devil_protocol::RemoteAuthorityId(7101),
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
            decision_id: devil_protocol::CapabilityDecisionId(1),
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
            buffer_version: Some(devil_protocol::BufferVersion(1)),
            snapshot_id: SnapshotId(66),
            correlation_id: CorrelationId(901),
            causality_id: causality_id(),
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

        let lsp = devil_protocol::RemoteLspDescriptor {
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
            version_vector: devil_protocol::CollaborationVersionVector { entries: vec![] },
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
}
