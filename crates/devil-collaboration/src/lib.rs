//! Deterministic, metadata-first collaboration operation log runtime.

#![warn(missing_docs)]

use std::collections::{HashMap, HashSet};

use devil_protocol::{
    ByteRange, CollaborationAcknowledgement, CollaborationAcknowledgementStatus,
    CollaborationAuditRecord, CollaborationCausalGap, CollaborationDocumentBinding,
    CollaborationDocumentOperation, CollaborationDocumentOperationKind, CollaborationOperationId,
    CollaborationParticipant, CollaborationParticipantId, CollaborationPermission,
    CollaborationPresenceProjection, CollaborationReplayManifest, CollaborationSessionDescriptor,
    CollaborationSessionId, CollaborationSessionState, CollaborationTransportEnvelope,
    CollaborationTransportPayload, CollaborationVersionVectorEntry, CorrelationId, EventSequence,
    RedactionHint, RetentionLabel, TextRange,
};
use thiserror::Error;

/// Collaboration runtime validation or replay error.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CollaborationRuntimeError {
    /// Runtime feature flag is disabled.
    #[error("collaboration runtime is disabled")]
    RuntimeDisabled,
    /// Session descriptor is invalid.
    #[error("invalid collaboration session: {reason}")]
    InvalidSession {
        /// Validation reason.
        reason: String,
    },
    /// Participant descriptor is invalid or unauthorized.
    #[error("invalid collaboration participant: {reason}")]
    InvalidParticipant {
        /// Validation reason.
        reason: String,
    },
    /// Operation metadata or payload is invalid.
    #[error("invalid collaboration operation: {reason}")]
    InvalidOperation {
        /// Validation reason.
        reason: String,
    },
    /// Text operation could not apply cleanly.
    #[error("collaboration operation conflict: {reason}")]
    Conflict {
        /// Conflict reason.
        reason: String,
    },
    /// Session lifecycle state rejects the requested operation.
    #[error("collaboration session state rejected operation: {reason}")]
    InvalidSessionState {
        /// Rejection reason.
        reason: String,
    },
}

/// Runtime feature and resource limits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CollaborationRuntimeConfig {
    /// Whether operation application is enabled.
    pub runtime_enabled: bool,
    /// Maximum text payload bytes allowed in one operation.
    pub max_operation_text_bytes: usize,
    /// Maximum participants allowed in a session.
    pub max_participants: usize,
}

impl CollaborationRuntimeConfig {
    /// Returns a conservative enabled test/runtime configuration.
    pub fn enabled() -> Self {
        Self {
            runtime_enabled: true,
            ..Self::default()
        }
    }
}

impl Default for CollaborationRuntimeConfig {
    fn default() -> Self {
        Self {
            runtime_enabled: false,
            max_operation_text_bytes: 64 * 1024,
            max_participants: 32,
        }
    }
}

/// Result of submitting a collaboration operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CollaborationSubmitOutcome {
    /// Operation acknowledgement.
    pub acknowledgement: CollaborationAcknowledgement,
    /// Text after accepted replay, or unchanged text for rejected operations.
    pub document_text: String,
    /// Causal gap emitted for out-of-order participant sequences.
    pub causal_gap: Option<CollaborationCausalGap>,
}

/// In-memory deterministic operation-log session.
#[derive(Debug, Clone)]
pub struct CollaborationSessionRuntime {
    descriptor: CollaborationSessionDescriptor,
    participants: HashMap<CollaborationParticipantId, CollaborationParticipant>,
    config: CollaborationRuntimeConfig,
    initial_text: String,
    current_text: String,
    operations: Vec<CollaborationDocumentOperation>,
    acknowledgements: Vec<CollaborationAcknowledgement>,
    causal_gaps: Vec<CollaborationCausalGap>,
    presence: HashMap<CollaborationParticipantId, CollaborationPresenceProjection>,
    participant_sequences: HashMap<CollaborationParticipantId, u64>,
    operation_ids: HashSet<CollaborationOperationId>,
}

impl CollaborationSessionRuntime {
    /// Creates an enabled collaboration session runtime from protocol descriptors.
    pub fn new(
        descriptor: CollaborationSessionDescriptor,
        participants: Vec<CollaborationParticipant>,
        initial_text: impl Into<String>,
        config: CollaborationRuntimeConfig,
    ) -> Result<Self, CollaborationRuntimeError> {
        if descriptor.session_id.0 == 0
            || descriptor.workspace_id.0 == 0
            || descriptor.schema_version == 0
            || descriptor.document_bindings.is_empty()
        {
            return Err(CollaborationRuntimeError::InvalidSession {
                reason: "session id, workspace, schema, and document binding are required"
                    .to_string(),
            });
        }
        if !matches!(
            descriptor.state,
            CollaborationSessionState::Active | CollaborationSessionState::Degraded
        ) {
            return Err(CollaborationRuntimeError::InvalidSession {
                reason: "session must be active or degraded before accepting operations"
                    .to_string(),
            });
        }
        if participants.is_empty() || participants.len() > config.max_participants {
            return Err(CollaborationRuntimeError::InvalidParticipant {
                reason: "participant count is outside configured bounds".to_string(),
            });
        }

        let mut participant_map = HashMap::new();
        for participant in participants {
            if participant.session_id != descriptor.session_id
                || participant.participant_id.0 == 0
                || participant.principal_id.0.trim().is_empty()
                || participant.schema_version == 0
            {
                return Err(CollaborationRuntimeError::InvalidParticipant {
                    reason: "participant id, principal, session, and schema are required"
                        .to_string(),
                });
            }
            participant_map.insert(participant.participant_id, participant);
        }

        let initial_text = initial_text.into();
        Ok(Self {
            descriptor,
            participants: participant_map,
            config,
            current_text: initial_text.clone(),
            initial_text,
            operations: Vec::new(),
            acknowledgements: Vec::new(),
            causal_gaps: Vec::new(),
            presence: HashMap::new(),
            participant_sequences: HashMap::new(),
            operation_ids: HashSet::new(),
        })
    }

    /// Returns the collaboration session identifier.
    pub fn session_id(&self) -> CollaborationSessionId {
        self.descriptor.session_id
    }

    /// Returns the current session lifecycle state.
    pub fn session_state(&self) -> CollaborationSessionState {
        self.descriptor.state
    }

    /// Returns the current deterministic document text.
    pub fn document_text(&self) -> &str {
        &self.current_text
    }

    /// Returns accepted operations in current log order.
    pub fn operations(&self) -> &[CollaborationDocumentOperation] {
        &self.operations
    }

    /// Returns emitted acknowledgements.
    pub fn acknowledgements(&self) -> &[CollaborationAcknowledgement] {
        &self.acknowledgements
    }

    /// Returns detected causal gaps.
    pub fn causal_gaps(&self) -> &[CollaborationCausalGap] {
        &self.causal_gaps
    }

    /// Returns latest projected participant presence.
    pub fn presence(&self) -> Vec<CollaborationPresenceProjection> {
        let mut projections = self.presence.values().cloned().collect::<Vec<_>>();
        projections.sort_by_key(|projection| projection.participant_id.0);
        projections
    }

    /// Publishes metadata-only presence without mutating document text.
    pub fn publish_presence(
        &mut self,
        projection: CollaborationPresenceProjection,
    ) -> Result<(), CollaborationRuntimeError> {
        self.ensure_accepts_presence()?;
        if projection.session_id != self.descriptor.session_id || projection.schema_version == 0 {
            return Err(CollaborationRuntimeError::InvalidOperation {
                reason: "presence session and schema must match".to_string(),
            });
        }
        let participant = self.participant(projection.participant_id)?;
        if !participant
            .permissions
            .contains(&CollaborationPermission::PublishPresence)
        {
            return Err(CollaborationRuntimeError::InvalidParticipant {
                reason: "participant lacks presence publish permission".to_string(),
            });
        }

        self.presence.insert(projection.participant_id, projection);
        Ok(())
    }

    /// Submits a document operation, returning an explicit acknowledgement.
    pub fn submit_operation(
        &mut self,
        operation: CollaborationDocumentOperation,
    ) -> Result<CollaborationSubmitOutcome, CollaborationRuntimeError> {
        if !self.config.runtime_enabled {
            return Err(CollaborationRuntimeError::RuntimeDisabled);
        }
        self.ensure_accepts_operation()?;

        self.validate_operation_shape(&operation)?;

        if self.operation_ids.contains(&operation.operation_id) {
            let acknowledgement = self.acknowledge(
                &operation,
                CollaborationAcknowledgementStatus::Duplicate,
                Some("duplicate_operation"),
            );
            self.acknowledgements.push(acknowledgement.clone());
            return Ok(CollaborationSubmitOutcome {
                acknowledgement,
                document_text: self.current_text.clone(),
                causal_gap: None,
            });
        }

        let expected_sequence = self
            .participant_sequences
            .get(&operation.author_participant_id)
            .copied()
            .unwrap_or(0)
            .saturating_add(1);
        if operation.participant_sequence > expected_sequence {
            let gap = CollaborationCausalGap {
                session_id: operation.session_id,
                participant_id: operation.author_participant_id,
                expected_sequence,
                observed_sequence: operation.participant_sequence,
                reason_code: "participant_sequence_gap".to_string(),
            };
            let acknowledgement = self.acknowledge(
                &operation,
                CollaborationAcknowledgementStatus::GapDetected,
                Some("participant_sequence_gap"),
            );
            self.causal_gaps.push(gap.clone());
            self.acknowledgements.push(acknowledgement.clone());
            return Ok(CollaborationSubmitOutcome {
                acknowledgement,
                document_text: self.current_text.clone(),
                causal_gap: Some(gap),
            });
        }
        if operation.participant_sequence < expected_sequence {
            let acknowledgement = self.acknowledge(
                &operation,
                CollaborationAcknowledgementStatus::Stale,
                Some("stale_participant_sequence"),
            );
            self.acknowledgements.push(acknowledgement.clone());
            return Ok(CollaborationSubmitOutcome {
                acknowledgement,
                document_text: self.current_text.clone(),
                causal_gap: None,
            });
        }

        let previous_text = self.current_text.clone();
        self.operations.push(operation.clone());
        match replay_operations(&self.initial_text, &self.ordered_operations()) {
            Ok(text) => {
                self.current_text = text;
                self.operation_ids.insert(operation.operation_id);
                self.participant_sequences.insert(
                    operation.author_participant_id,
                    operation.participant_sequence,
                );
                let acknowledgement = self.acknowledge(
                    &operation,
                    CollaborationAcknowledgementStatus::Accepted,
                    None,
                );
                self.acknowledgements.push(acknowledgement.clone());
                Ok(CollaborationSubmitOutcome {
                    acknowledgement,
                    document_text: self.current_text.clone(),
                    causal_gap: None,
                })
            }
            Err(error) => {
                self.operations.pop();
                self.current_text = previous_text;
                let acknowledgement = self.acknowledge(
                    &operation,
                    CollaborationAcknowledgementStatus::ResyncRequired,
                    Some("operation_conflict"),
                );
                self.acknowledgements.push(acknowledgement.clone());
                if let CollaborationRuntimeError::Conflict { .. } = error {
                    Ok(CollaborationSubmitOutcome {
                        acknowledgement,
                        document_text: self.current_text.clone(),
                        causal_gap: None,
                    })
                } else {
                    Err(error)
                }
            }
        }
    }

    /// Builds a metadata-only replay manifest for accepted operation order.
    pub fn replay_manifest(
        &self,
        correlation_id: CorrelationId,
        event_sequence: EventSequence,
    ) -> CollaborationReplayManifest {
        CollaborationReplayManifest {
            session_id: self.descriptor.session_id,
            operation_ids: self
                .ordered_operations()
                .iter()
                .map(|operation| operation.operation_id)
                .collect(),
            participant_count: self.participants.len() as u32,
            acknowledgement_count: self.acknowledgements.len() as u32,
            causal_gap_count: self.causal_gaps.len() as u32,
            final_byte_count: self.current_text.len() as u64,
            correlation_id,
            causality_id: self
                .operations
                .last()
                .map(|operation| operation.preconditions.causality_id)
                .unwrap_or(self.descriptor.created_at_causality_fallback()),
            event_sequence,
            retention_label: RetentionLabel::Audit,
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        }
    }

    /// Builds a metadata-only audit record for the latest accepted state.
    pub fn audit_record(
        &self,
        operation_id: Option<CollaborationOperationId>,
        proposal_id: Option<devil_protocol::ProposalId>,
        event_sequence: EventSequence,
        correlation_id: CorrelationId,
    ) -> CollaborationAuditRecord {
        CollaborationAuditRecord {
            session_id: self.descriptor.session_id,
            operation_id,
            proposal_id,
            event_sequence,
            correlation_id,
            causality_id: self
                .operations
                .last()
                .map(|operation| operation.preconditions.causality_id)
                .unwrap_or(self.descriptor.created_at_causality_fallback()),
            retention_label: RetentionLabel::Audit,
            redaction_hints: vec![RedactionHint::MetadataOnly],
            metadata_summary: format!(
                "operations={}, participants={}, bytes={}, gaps={}",
                self.operations.len(),
                self.participants.len(),
                self.current_text.len(),
                self.causal_gaps.len()
            ),
            schema_version: 1,
        }
    }

    /// Accepts a transport envelope whose payload is a document operation.
    pub fn handle_transport_envelope(
        &mut self,
        envelope: CollaborationTransportEnvelope,
    ) -> Result<Option<CollaborationSubmitOutcome>, CollaborationRuntimeError> {
        if envelope.session_id != self.descriptor.session_id
            || envelope.sender_participant_id.0 == 0
            || envelope.correlation_id.0 == 0
            || envelope.causality_id.0.is_nil()
            || envelope.schema_version == 0
        {
            return Err(CollaborationRuntimeError::InvalidOperation {
                reason: "transport envelope metadata is invalid".to_string(),
            });
        }

        match envelope.payload {
            CollaborationTransportPayload::Operation(operation) => {
                Ok(Some(self.submit_operation(*operation)?))
            }
            CollaborationTransportPayload::Presence(projection) => {
                self.publish_presence(projection)?;
                Ok(None)
            }
            _ => Ok(None),
        }
    }

    /// Marks a participant as reconnecting and preserves document text.
    pub fn disconnect_participant(
        &mut self,
        participant_id: CollaborationParticipantId,
    ) -> Result<(), CollaborationRuntimeError> {
        self.participant(participant_id)?;
        self.descriptor.state = CollaborationSessionState::Reconnecting;
        let projection = self.presence.entry(participant_id).or_insert_with(|| {
            CollaborationPresenceProjection {
                session_id: self.descriptor.session_id,
                participant_id,
                cursor: None,
                selections: Vec::new(),
                activity_label: Some("reconnecting".to_string()),
                reconnecting: true,
                schema_version: 1,
            }
        });
        projection.reconnecting = true;
        projection.activity_label = Some("reconnecting".to_string());
        Ok(())
    }

    /// Begins reconnect metadata handling.
    pub fn begin_reconnect(
        &mut self,
        participant_id: CollaborationParticipantId,
    ) -> Result<CollaborationReplayManifest, CollaborationRuntimeError> {
        self.disconnect_participant(participant_id)?;
        Ok(self.replay_manifest(CorrelationId(1), EventSequence(1)))
    }

    /// Completes reconnect and returns the session to active operation processing.
    pub fn complete_reconnect(
        &mut self,
        participant_id: CollaborationParticipantId,
    ) -> Result<(), CollaborationRuntimeError> {
        self.participant(participant_id)?;
        if let Some(projection) = self.presence.get_mut(&participant_id) {
            projection.reconnecting = false;
            projection.activity_label = Some("active".to_string());
        }
        self.descriptor.state = CollaborationSessionState::Active;
        Ok(())
    }

    /// Removes a participant from the active session.
    pub fn leave_participant(
        &mut self,
        participant_id: CollaborationParticipantId,
    ) -> Result<(), CollaborationRuntimeError> {
        self.participant(participant_id)?;
        self.participants.remove(&participant_id);
        self.presence.remove(&participant_id);
        if self.participants.is_empty() {
            self.descriptor.state = CollaborationSessionState::Closing;
        }
        Ok(())
    }

    /// Starts fail-closed shutdown drain.
    pub fn begin_shutdown(&mut self) {
        self.descriptor.state = CollaborationSessionState::Closing;
    }

    /// Finishes shutdown and rejects future transport mutation.
    pub fn finish_shutdown(&mut self) {
        self.descriptor.state = CollaborationSessionState::Closed;
    }

    fn ensure_accepts_operation(&self) -> Result<(), CollaborationRuntimeError> {
        match self.descriptor.state {
            CollaborationSessionState::Active | CollaborationSessionState::Degraded => Ok(()),
            CollaborationSessionState::Reconnecting => {
                Err(CollaborationRuntimeError::InvalidSessionState {
                    reason: "reconnecting sessions require replay/resync before text operations"
                        .to_string(),
                })
            }
            CollaborationSessionState::Closing
            | CollaborationSessionState::Closed
            | CollaborationSessionState::Denied => {
                Err(CollaborationRuntimeError::InvalidSessionState {
                    reason: format!(
                        "session state {:?} rejects new operations",
                        self.descriptor.state
                    ),
                })
            }
            _ => Err(CollaborationRuntimeError::InvalidSessionState {
                reason: format!("session state {:?} is not active", self.descriptor.state),
            }),
        }
    }

    fn ensure_accepts_presence(&self) -> Result<(), CollaborationRuntimeError> {
        match self.descriptor.state {
            CollaborationSessionState::Active
            | CollaborationSessionState::Degraded
            | CollaborationSessionState::Reconnecting => Ok(()),
            CollaborationSessionState::Closing
            | CollaborationSessionState::Closed
            | CollaborationSessionState::Denied => {
                Err(CollaborationRuntimeError::InvalidSessionState {
                    reason: format!("session state {:?} rejects presence", self.descriptor.state),
                })
            }
            _ => Err(CollaborationRuntimeError::InvalidSessionState {
                reason: format!(
                    "session state {:?} is not publishable",
                    self.descriptor.state
                ),
            }),
        }
    }

    fn validate_operation_shape(
        &self,
        operation: &CollaborationDocumentOperation,
    ) -> Result<(), CollaborationRuntimeError> {
        if operation.session_id != self.descriptor.session_id
            || operation.operation_id.0 == 0
            || operation.author_participant_id.0 == 0
            || operation.participant_sequence == 0
            || operation.schema_version == 0
        {
            return Err(CollaborationRuntimeError::InvalidOperation {
                reason: "operation id, participant, sequence, session, and schema are required"
                    .to_string(),
            });
        }
        if !operation.preconditions.has_valid_identity_metadata() {
            return Err(CollaborationRuntimeError::InvalidOperation {
                reason: "identity, capability, correlation, and causality metadata are invalid"
                    .to_string(),
            });
        }
        self.participant(operation.author_participant_id)?;
        if !self
            .participant(operation.author_participant_id)?
            .permissions
            .contains(&CollaborationPermission::PublishOperation)
        {
            return Err(CollaborationRuntimeError::InvalidParticipant {
                reason: "participant lacks operation publish permission".to_string(),
            });
        }

        let binding = self.document_binding(operation)?;
        if operation.preconditions.workspace_id != binding.workspace_id
            || operation.preconditions.file_id != binding.file_id
            || operation.preconditions.buffer_id != binding.buffer_id
            || operation.preconditions.snapshot_id != binding.snapshot_id
            || operation.preconditions.buffer_version != binding.buffer_version
            || operation.preconditions.document_epoch != binding.document_epoch
        {
            return Err(CollaborationRuntimeError::InvalidOperation {
                reason: "operation preconditions do not match document binding".to_string(),
            });
        }

        if operation_text_bytes(&operation.kind) > self.config.max_operation_text_bytes {
            return Err(CollaborationRuntimeError::InvalidOperation {
                reason: "operation text payload exceeds configured bound".to_string(),
            });
        }
        if requires_range(&operation.kind) && operation.range.is_none() {
            return Err(CollaborationRuntimeError::InvalidOperation {
                reason: "text operation requires an affected range".to_string(),
            });
        }
        if let Some(range) = operation.range
            && !range.is_valid()
        {
            return Err(CollaborationRuntimeError::InvalidOperation {
                reason: "operation range is invalid".to_string(),
            });
        }
        Ok(())
    }

    fn participant(
        &self,
        participant_id: CollaborationParticipantId,
    ) -> Result<&CollaborationParticipant, CollaborationRuntimeError> {
        self.participants.get(&participant_id).ok_or_else(|| {
            CollaborationRuntimeError::InvalidParticipant {
                reason: "participant is not admitted to the session".to_string(),
            }
        })
    }

    fn document_binding(
        &self,
        operation: &CollaborationDocumentOperation,
    ) -> Result<&CollaborationDocumentBinding, CollaborationRuntimeError> {
        self.descriptor
            .document_bindings
            .iter()
            .find(|binding| {
                binding.workspace_id == operation.preconditions.workspace_id
                    && binding.file_id == operation.preconditions.file_id
                    && binding.buffer_id == operation.preconditions.buffer_id
            })
            .ok_or_else(|| CollaborationRuntimeError::InvalidOperation {
                reason: "document binding not found for operation".to_string(),
            })
    }

    fn acknowledge(
        &self,
        operation: &CollaborationDocumentOperation,
        status: CollaborationAcknowledgementStatus,
        reason_code: Option<&str>,
    ) -> CollaborationAcknowledgement {
        CollaborationAcknowledgement {
            session_id: operation.session_id,
            operation_id: operation.operation_id,
            participant_id: operation.author_participant_id,
            status,
            observed_vector: devil_protocol::CollaborationVersionVector {
                entries: self
                    .participant_sequences
                    .iter()
                    .map(
                        |(participant_id, sequence)| CollaborationVersionVectorEntry {
                            participant_id: *participant_id,
                            sequence: *sequence,
                        },
                    )
                    .collect(),
            },
            reason_code: reason_code.map(str::to_string),
            schema_version: 1,
        }
    }

    fn ordered_operations(&self) -> Vec<CollaborationDocumentOperation> {
        deterministic_order(&self.operations)
    }
}

trait DescriptorFallback {
    fn created_at_causality_fallback(&self) -> devil_protocol::CausalityId;
}

impl DescriptorFallback for CollaborationSessionDescriptor {
    fn created_at_causality_fallback(&self) -> devil_protocol::CausalityId {
        devil_protocol::CausalityId(uuid_like_fallback(self.session_id.0, self.created_at.0))
    }
}

fn uuid_like_fallback(session_id: u128, timestamp: u64) -> uuid::Uuid {
    let value = session_id ^ ((timestamp as u128) << 64) ^ 0xcccccccccccccccccccccccccccccccc;
    uuid::Uuid::from_u128(value.max(1))
}

fn operation_text_bytes(kind: &CollaborationDocumentOperationKind) -> usize {
    match kind {
        CollaborationDocumentOperationKind::Insert { text }
        | CollaborationDocumentOperationKind::Replace { text } => text.len(),
        CollaborationDocumentOperationKind::Delete
        | CollaborationDocumentOperationKind::CursorMove
        | CollaborationDocumentOperationKind::SelectionUpdate
        | CollaborationDocumentOperationKind::UndoCompensation
        | CollaborationDocumentOperationKind::NoopAcknowledgement
        | CollaborationDocumentOperationKind::ResyncRequest => 0,
    }
}

fn requires_range(kind: &CollaborationDocumentOperationKind) -> bool {
    matches!(
        kind,
        CollaborationDocumentOperationKind::Insert { .. }
            | CollaborationDocumentOperationKind::Delete
            | CollaborationDocumentOperationKind::Replace { .. }
    )
}

fn deterministic_order(
    operations: &[CollaborationDocumentOperation],
) -> Vec<CollaborationDocumentOperation> {
    let mut remaining = operations.to_vec();
    remaining.sort_by_key(operation_order_key);
    let mut ordered = Vec::with_capacity(remaining.len());

    while !remaining.is_empty() {
        let ready_index = remaining
            .iter()
            .position(|candidate| dependencies_satisfied(candidate, &remaining, &ordered))
            .unwrap_or(0);
        ordered.push(remaining.remove(ready_index));
    }

    ordered
}

fn dependencies_satisfied(
    candidate: &CollaborationDocumentOperation,
    remaining: &[CollaborationDocumentOperation],
    ordered: &[CollaborationDocumentOperation],
) -> bool {
    remaining.iter().all(|dependency| {
        if dependency.operation_id == candidate.operation_id {
            return true;
        }
        if !depends_on(candidate, dependency) {
            return true;
        }
        ordered
            .iter()
            .any(|ordered| ordered.operation_id == dependency.operation_id)
    })
}

fn depends_on(
    candidate: &CollaborationDocumentOperation,
    dependency: &CollaborationDocumentOperation,
) -> bool {
    if candidate.author_participant_id == dependency.author_participant_id {
        return candidate.participant_sequence > dependency.participant_sequence;
    }
    candidate
        .preconditions
        .base_vector
        .entries
        .iter()
        .any(|entry| {
            entry.participant_id == dependency.author_participant_id
                && entry.sequence >= dependency.participant_sequence
        })
}

fn operation_order_key(operation: &CollaborationDocumentOperation) -> (u64, u128, u128) {
    (
        operation.participant_sequence,
        operation.author_participant_id.0,
        operation.operation_id.0,
    )
}

fn replay_operations(
    initial_text: &str,
    operations: &[CollaborationDocumentOperation],
) -> Result<String, CollaborationRuntimeError> {
    let mut text = initial_text.to_string();
    for operation in operations {
        apply_operation(&mut text, operation)?;
    }
    Ok(text)
}

fn apply_operation(
    text: &mut String,
    operation: &CollaborationDocumentOperation,
) -> Result<(), CollaborationRuntimeError> {
    match &operation.kind {
        CollaborationDocumentOperationKind::Insert { text: inserted } => {
            let range = byte_range(operation.range)?;
            replace_range(text, ByteRange::new(range.start, range.start), inserted)
        }
        CollaborationDocumentOperationKind::Delete => {
            let range = byte_range(operation.range)?;
            replace_range(text, range, "")
        }
        CollaborationDocumentOperationKind::Replace { text: replacement } => {
            let range = byte_range(operation.range)?;
            replace_range(text, range, replacement)
        }
        CollaborationDocumentOperationKind::CursorMove
        | CollaborationDocumentOperationKind::SelectionUpdate
        | CollaborationDocumentOperationKind::UndoCompensation
        | CollaborationDocumentOperationKind::NoopAcknowledgement
        | CollaborationDocumentOperationKind::ResyncRequest => Ok(()),
    }
}

fn byte_range(range: Option<TextRange>) -> Result<ByteRange, CollaborationRuntimeError> {
    range.and_then(TextRange::as_byte_range).ok_or_else(|| {
        CollaborationRuntimeError::InvalidOperation {
            reason: "operation range must use byte coordinates".to_string(),
        }
    })
}

fn replace_range(
    text: &mut String,
    range: ByteRange,
    replacement: &str,
) -> Result<(), CollaborationRuntimeError> {
    let start = range.start as usize;
    let end = range.end as usize;
    if start > end
        || end > text.len()
        || !text.is_char_boundary(start)
        || !text.is_char_boundary(end)
    {
        return Err(CollaborationRuntimeError::Conflict {
            reason: "operation range is outside current document boundaries".to_string(),
        });
    }
    text.replace_range(start..end, replacement);
    Ok(())
}

#[cfg(test)]
mod tests {
    use devil_protocol::{
        BufferId, BufferVersion, CapabilityDecision, CapabilityDecisionId, CapabilityId,
        CausalityId, CollaborationDocumentEpoch, CollaborationOperationPreconditions,
        CollaborationParticipantRole, CollaborationVersionVector, FileFingerprint, FileId,
        PrincipalId, SnapshotId, TextCoordinate, TimestampMillis, WorkspaceId,
    };
    use uuid::Uuid;

    use super::*;

    fn causality_id(value: u128) -> CausalityId {
        CausalityId(Uuid::from_u128(value))
    }

    fn binding() -> CollaborationDocumentBinding {
        CollaborationDocumentBinding {
            workspace_id: WorkspaceId(11),
            file_id: FileId(33),
            buffer_id: BufferId(22),
            snapshot_id: SnapshotId(66),
            buffer_version: BufferVersion(1),
            document_epoch: CollaborationDocumentEpoch(3),
            content_fingerprint: Some(FileFingerprint {
                algorithm: "sha256".to_string(),
                value: "initial".to_string(),
            }),
            schema_version: 1,
        }
    }

    fn descriptor() -> CollaborationSessionDescriptor {
        CollaborationSessionDescriptor {
            session_id: CollaborationSessionId(1001),
            workspace_id: WorkspaceId(11),
            state: CollaborationSessionState::Active,
            created_by: PrincipalId("owner".to_string()),
            created_at: TimestampMillis(1700),
            document_bindings: vec![binding()],
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        }
    }

    fn participant(id: u128) -> CollaborationParticipant {
        CollaborationParticipant {
            session_id: CollaborationSessionId(1001),
            participant_id: CollaborationParticipantId(id),
            principal_id: PrincipalId(format!("participant-{id}")),
            role: CollaborationParticipantRole::Editor,
            permissions: vec![
                CollaborationPermission::PublishOperation,
                CollaborationPermission::PublishPresence,
            ],
            display_label: format!("p{id}"),
            schema_version: 1,
        }
    }

    fn capability() -> CapabilityDecision {
        CapabilityDecision {
            decision_id: CapabilityDecisionId(44),
            granted: true,
            capability: CapabilityId("collaboration.operation.publish".to_string()),
            reason: None,
        }
    }

    fn operation(
        operation_id: u128,
        participant_id: u128,
        participant_sequence: u64,
        kind: CollaborationDocumentOperationKind,
        range: Option<TextRange>,
        vector: Vec<CollaborationVersionVectorEntry>,
    ) -> CollaborationDocumentOperation {
        CollaborationDocumentOperation {
            session_id: CollaborationSessionId(1001),
            operation_id: CollaborationOperationId(operation_id),
            author_participant_id: CollaborationParticipantId(participant_id),
            participant_sequence,
            kind,
            range,
            preconditions: CollaborationOperationPreconditions {
                workspace_id: WorkspaceId(11),
                file_id: FileId(33),
                buffer_id: BufferId(22),
                snapshot_id: SnapshotId(66),
                buffer_version: BufferVersion(1),
                document_epoch: CollaborationDocumentEpoch(3),
                base_vector: CollaborationVersionVector { entries: vector },
                author_principal: PrincipalId(format!("participant-{participant_id}")),
                capability_decision: capability(),
                correlation_id: CorrelationId(900 + participant_sequence),
                causality_id: causality_id(operation_id),
                redaction_hints: vec![RedactionHint::MetadataOnly],
            },
            undo_group: None,
            occurred_at: TimestampMillis(1800 + participant_sequence),
            schema_version: 1,
        }
    }

    fn runtime(participant_count: u128, initial: &str) -> CollaborationSessionRuntime {
        let participants = (1..=participant_count).map(participant).collect::<Vec<_>>();
        CollaborationSessionRuntime::new(
            descriptor(),
            participants,
            initial,
            CollaborationRuntimeConfig::enabled(),
        )
        .expect("runtime should initialize")
    }

    #[test]
    fn default_runtime_config_is_fail_closed() {
        let mut runtime = CollaborationSessionRuntime::new(
            descriptor(),
            vec![participant(1)],
            "abc",
            CollaborationRuntimeConfig::default(),
        )
        .expect("descriptor is valid");
        let result = runtime.submit_operation(operation(
            1,
            1,
            1,
            CollaborationDocumentOperationKind::Insert {
                text: "x".to_string(),
            },
            Some(TextRange::byte(0, 0)),
            vec![],
        ));

        assert!(matches!(
            result,
            Err(CollaborationRuntimeError::RuntimeDisabled)
        ));
        assert_eq!(runtime.document_text(), "abc");
    }

    #[test]
    fn concurrent_insert_converges_for_two_three_and_five_participants() {
        for participant_count in [2_u128, 3, 5] {
            let operations = (1..=participant_count)
                .map(|id| {
                    operation(
                        1000 + id,
                        id,
                        1,
                        CollaborationDocumentOperationKind::Insert {
                            text: id.to_string(),
                        },
                        Some(TextRange::byte(0, 0)),
                        vec![],
                    )
                })
                .collect::<Vec<_>>();

            let mut forward = runtime(participant_count, "");
            for operation in operations.clone() {
                forward
                    .submit_operation(operation)
                    .expect("operation should apply");
            }

            let mut reverse = runtime(participant_count, "");
            for operation in operations.into_iter().rev() {
                reverse
                    .submit_operation(operation)
                    .expect("operation should apply");
            }

            assert_eq!(forward.document_text(), reverse.document_text());
        }
    }

    #[test]
    fn delete_replace_and_undo_compensation_are_deterministic_metadata_operations() {
        let mut runtime = runtime(2, "abcdef");
        runtime
            .submit_operation(operation(
                10,
                1,
                1,
                CollaborationDocumentOperationKind::Delete,
                Some(TextRange::byte(1, 3)),
                vec![],
            ))
            .expect("delete applies");
        runtime
            .submit_operation(operation(
                20,
                2,
                1,
                CollaborationDocumentOperationKind::Replace {
                    text: "XY".to_string(),
                },
                Some(TextRange::byte(2, 4)),
                vec![],
            ))
            .expect("replace applies");
        runtime
            .submit_operation(operation(
                30,
                1,
                2,
                CollaborationDocumentOperationKind::UndoCompensation,
                None,
                vec![],
            ))
            .expect("undo metadata applies");

        assert_eq!(runtime.document_text(), "adXY");
        assert_eq!(runtime.acknowledgements().len(), 3);
    }

    #[test]
    fn duplicate_gap_and_conflict_fail_closed_without_clobbering_text() {
        let mut runtime = runtime(1, "abc");
        let initial = operation(
            10,
            1,
            1,
            CollaborationDocumentOperationKind::Insert {
                text: "!".to_string(),
            },
            Some(TextRange::byte(3, 3)),
            vec![],
        );
        runtime
            .submit_operation(initial.clone())
            .expect("insert applies");

        let duplicate = runtime
            .submit_operation(initial)
            .expect("duplicate returns acknowledgement");
        assert_eq!(
            duplicate.acknowledgement.status,
            CollaborationAcknowledgementStatus::Duplicate
        );
        assert_eq!(runtime.document_text(), "abc!");

        let gap = runtime
            .submit_operation(operation(
                20,
                1,
                3,
                CollaborationDocumentOperationKind::Insert {
                    text: "?".to_string(),
                },
                Some(TextRange::byte(0, 0)),
                vec![],
            ))
            .expect("gap returns acknowledgement");
        assert_eq!(
            gap.acknowledgement.status,
            CollaborationAcknowledgementStatus::GapDetected
        );
        assert_eq!(runtime.document_text(), "abc!");

        let conflict = runtime
            .submit_operation(operation(
                30,
                1,
                2,
                CollaborationDocumentOperationKind::Delete,
                Some(TextRange::byte(40, 41)),
                vec![],
            ))
            .expect("conflict returns resync acknowledgement");
        assert_eq!(
            conflict.acknowledgement.status,
            CollaborationAcknowledgementStatus::ResyncRequired
        );
        assert_eq!(runtime.document_text(), "abc!");
    }

    #[test]
    fn presence_and_replay_manifest_are_metadata_only() {
        let mut runtime = runtime(1, "abc");
        runtime
            .publish_presence(CollaborationPresenceProjection {
                session_id: CollaborationSessionId(1001),
                participant_id: CollaborationParticipantId(1),
                cursor: Some(TextCoordinate {
                    line: 0,
                    character: 1,
                    byte_offset: Some(1),
                    utf16_offset: Some(1),
                }),
                selections: vec![],
                activity_label: Some("editing metadata-only range".to_string()),
                reconnecting: false,
                schema_version: 1,
            })
            .expect("presence applies");
        runtime
            .submit_operation(operation(
                10,
                1,
                1,
                CollaborationDocumentOperationKind::Insert {
                    text: "!".to_string(),
                },
                Some(TextRange::byte(3, 3)),
                vec![],
            ))
            .expect("insert applies");

        let manifest = runtime.replay_manifest(CorrelationId(77), EventSequence(2));
        let audit = runtime.audit_record(
            Some(CollaborationOperationId(10)),
            None,
            EventSequence(3),
            CorrelationId(78),
        );

        assert_eq!(runtime.presence().len(), 1);
        assert_eq!(manifest.operation_ids, vec![CollaborationOperationId(10)]);
        assert!(audit.metadata_summary.contains("operations=1"));
        assert!(!audit.metadata_summary.contains("abc!"));
    }

    #[test]
    fn disconnect_reconnect_and_shutdown_states_are_fail_closed() {
        let mut runtime = runtime(1, "abc");
        let participant = CollaborationParticipantId(1);

        runtime
            .disconnect_participant(participant)
            .expect("disconnect marks reconnecting");
        assert_eq!(
            runtime.session_state(),
            CollaborationSessionState::Reconnecting
        );
        assert!(runtime.presence()[0].reconnecting);

        let rejected = runtime
            .submit_operation(operation(
                10,
                1,
                1,
                CollaborationDocumentOperationKind::Insert {
                    text: "!".to_string(),
                },
                Some(TextRange::byte(3, 3)),
                vec![],
            ))
            .expect_err("reconnecting rejects text operations");
        assert!(matches!(
            rejected,
            CollaborationRuntimeError::InvalidSessionState { .. }
        ));

        let manifest = runtime
            .begin_reconnect(participant)
            .expect("reconnect returns replay metadata");
        assert_eq!(manifest.session_id, runtime.session_id());
        assert_eq!(manifest.final_byte_count, 3);
        runtime
            .complete_reconnect(participant)
            .expect("reconnect completes");
        assert_eq!(runtime.session_state(), CollaborationSessionState::Active);
        assert!(!runtime.presence()[0].reconnecting);

        runtime.begin_shutdown();
        assert_eq!(runtime.session_state(), CollaborationSessionState::Closing);
        runtime.finish_shutdown();
        assert_eq!(runtime.session_state(), CollaborationSessionState::Closed);
        let presence_rejected = runtime.publish_presence(CollaborationPresenceProjection {
            session_id: runtime.session_id(),
            participant_id: participant,
            cursor: None,
            selections: Vec::new(),
            activity_label: Some("closed".to_string()),
            reconnecting: false,
            schema_version: 1,
        });
        assert!(matches!(
            presence_rejected,
            Err(CollaborationRuntimeError::InvalidSessionState { .. })
        ));
    }
}
