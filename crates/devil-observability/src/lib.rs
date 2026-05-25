//! Tracing, metrics, event log, and performance counters.

#![warn(missing_docs)]

use std::{
    collections::hash_map::DefaultHasher,
    fmt,
    hash::{Hash, Hasher},
    sync::{Arc, Mutex},
};

use devil_protocol::{
    AgentReplayManifest, AssistedAiAuditOutcomeCategory, AssistedAiAuditPrivacyDisposition,
    AssistedAiAuditRecord, AssistedAiAuditRedactionState, AssistedAiConsentBoundary,
    AssistedAiContractError, AssistedAiProjection, AssistedAiProposalPreviewSummary,
    AssistedAiProviderInvocationState, AssistedAiRequestContract, AssistedAiRequestDisposition,
    BufferId, CapabilityId, CausalityId, CorrelationId, DelegatedTaskAssistedAiAuditReference,
    DelegatedTaskAuditLinkageRecord, DelegatedTaskPlanContract, EventEnvelope, EventId,
    EventMetadataRecord, EventSequence, EventSeverity, EventSinkPort, EventSinkRequest,
    FileFingerprint, FileId, PermissionBudgetEvaluationDisposition, Phase4RuntimeAuditRecord,
    PrincipalId, ProposalAuditRecord, ProposalFailureReason, ProposalLifecycleState,
    ProposalLifecycleTransition, ProposalPayload, ProposalPayloadKind, ProposalPayloadSummary,
    ProposalPrivacyLabel, ProposalRejectionReason, ProposalRollbackReason, ProposalStaleReason,
    ProtocolDiagnostic, ProtocolError, ProtocolResult, RedactionHint, RetentionLabel,
    TextTransactionDescriptor, TimestampMillis, WorkspaceId, WorkspaceProposal,
    delegated_task_audit_linkage_record, validate_agent_replay_manifest,
    validate_assisted_ai_audit_record, validate_delegated_task_audit_linkage_record,
    validate_phase4_runtime_audit_record,
};
use serde_json::{Map, Value, json};
use thiserror::Error;
use uuid::Uuid;

/// Observability validation and redaction errors.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ObservabilityError {
    /// Event schema version is missing or invalid.
    #[error("event envelope schema_version must be non-zero")]
    InvalidSchemaVersion,
    /// Event name is empty.
    #[error("event envelope event name must be non-empty")]
    MissingEventName,
    /// Event payload was not an object after validation.
    #[error("event envelope payload must be a metadata object")]
    InvalidPayload,
    /// Event causality id is missing or nil.
    #[error("event envelope causality_id must be non-zero")]
    InvalidCausalityId,
    /// Event correlation id is missing or zero.
    #[error("event envelope correlation_id must be non-zero")]
    InvalidCorrelationId,
    /// Event sequence is missing or zero.
    #[error("event envelope sequence must be non-zero")]
    InvalidSequence,
    /// Event sink storage lock was poisoned.
    #[error("event sink storage lock poisoned")]
    StorageUnavailable,
}

/// Runtime configuration for validating and storing event envelopes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EventSinkConfig {
    /// Require a non-zero schema version on every envelope.
    pub require_schema_version: bool,
    /// Store metadata-only payloads even when no explicit redaction hint is present.
    pub metadata_only_by_default: bool,
}

impl Default for EventSinkConfig {
    fn default() -> Self {
        Self {
            require_schema_version: true,
            metadata_only_by_default: true,
        }
    }
}

/// In-memory event sink for tests and local replay drills.
#[derive(Debug, Clone)]
pub struct InMemoryEventSink {
    config: EventSinkConfig,
    events: Arc<Mutex<Vec<EventEnvelope>>>,
}

impl InMemoryEventSink {
    /// Construct an in-memory sink with default metadata-only retention.
    pub fn new() -> Self {
        Self::with_config(EventSinkConfig::default())
    }

    /// Construct an in-memory sink with explicit validation configuration.
    pub fn with_config(config: EventSinkConfig) -> Self {
        Self {
            config,
            events: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Validate, redact, and store an event sink request.
    pub fn try_emit(&self, request: EventSinkRequest) -> Result<(), ObservabilityError> {
        let mut envelope = request.envelope;
        validate_envelope(&envelope, self.config)?;

        if self.config.metadata_only_by_default || envelope.redaction != RedactionHint::None {
            envelope.payload = redact_payload(&envelope.payload, envelope.redaction);
        }

        self.events
            .lock()
            .map_err(|_| ObservabilityError::StorageUnavailable)?
            .push(envelope);
        Ok(())
    }

    /// Return a cloned snapshot of stored envelopes.
    pub fn events(&self) -> Result<Vec<EventEnvelope>, ObservabilityError> {
        Ok(self
            .events
            .lock()
            .map_err(|_| ObservabilityError::StorageUnavailable)?
            .clone())
    }
}

impl Default for InMemoryEventSink {
    fn default() -> Self {
        Self::new()
    }
}

impl EventSinkPort for InMemoryEventSink {
    fn emit(&self, request: EventSinkRequest) -> ProtocolResult<()> {
        self.try_emit(request).map_err(protocol_error)
    }
}

/// Event sink wrapper that actively redacts payloads before storage.
#[derive(Debug, Clone)]
pub struct RedactingEventSink {
    inner: InMemoryEventSink,
}

impl RedactingEventSink {
    /// Construct a redacting sink with metadata-only retention.
    pub fn new() -> Self {
        Self {
            inner: InMemoryEventSink::new(),
        }
    }

    /// Validate, redact, and store an event sink request.
    pub fn try_emit(&self, mut request: EventSinkRequest) -> Result<(), ObservabilityError> {
        if request.envelope.redaction == RedactionHint::None {
            request.envelope.redaction = RedactionHint::MetadataOnly;
        }
        self.inner.try_emit(request)
    }

    /// Return a cloned snapshot of redacted envelopes.
    pub fn events(&self) -> Result<Vec<EventEnvelope>, ObservabilityError> {
        self.inner.events()
    }
}

impl Default for RedactingEventSink {
    fn default() -> Self {
        Self::new()
    }
}

impl EventSinkPort for RedactingEventSink {
    fn emit(&self, request: EventSinkRequest) -> ProtocolResult<()> {
        self.try_emit(request).map_err(protocol_error)
    }
}

/// No-op event sink used when runtime observability wiring is intentionally deferred.
#[derive(Debug, Default, Clone)]
pub struct NoopEventSink;

impl EventSinkPort for NoopEventSink {
    fn emit(&self, _request: EventSinkRequest) -> ProtocolResult<()> {
        Ok(())
    }
}

/// Cloneable event-sink adapter for sharing one injected sink across focused services.
#[derive(Clone)]
pub struct SharedEventSink {
    inner: Arc<dyn EventSinkPort + Send + Sync>,
}

impl SharedEventSink {
    /// Wrap a concrete event sink in a shared adapter.
    pub fn new(sink: impl EventSinkPort + Send + Sync + 'static) -> Self {
        Self {
            inner: Arc::new(sink),
        }
    }

    /// Wrap an existing shared event-sink trait object.
    pub fn from_arc(inner: Arc<dyn EventSinkPort + Send + Sync>) -> Self {
        Self { inner }
    }
}

impl Default for SharedEventSink {
    fn default() -> Self {
        Self::new(NoopEventSink)
    }
}

impl fmt::Debug for SharedEventSink {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SharedEventSink").finish_non_exhaustive()
    }
}

impl EventSinkPort for SharedEventSink {
    fn emit(&self, request: EventSinkRequest) -> ProtocolResult<()> {
        self.inner.emit(request)
    }
}

/// Event metadata helper used by workspace write and editor transaction paths.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventMetadata {
    /// Event envelope schema version.
    pub schema_version: u16,
    /// Event name.
    pub event: String,
    /// Event severity.
    pub severity: EventSeverity,
    /// Retention label.
    pub retention: RetentionLabel,
    /// Redaction hint.
    pub redaction: RedactionHint,
    /// Causality chain id.
    pub causality_id: CausalityId,
    /// Optional parent event id.
    pub parent_event_id: Option<EventId>,
    /// Optional workspace id.
    pub workspace_id: Option<WorkspaceId>,
    /// Optional file id.
    pub file_id: Option<FileId>,
    /// Optional buffer id.
    pub buffer_id: Option<BufferId>,
    /// Correlation id.
    pub correlation_id: CorrelationId,
}

/// Envelope-ready event builder configured with metadata-only defaults.
#[derive(Debug, Clone)]
pub struct EventEnvelopeBuilder {
    schema_version: u16,
    event: String,
    severity: EventSeverity,
    retention: RetentionLabel,
    redaction: RedactionHint,
    causality_id: CausalityId,
    parent_event_id: Option<EventId>,
    workspace_id: Option<WorkspaceId>,
    file_id: Option<FileId>,
    buffer_id: Option<BufferId>,
    correlation_id: CorrelationId,
    principal_id: Option<PrincipalId>,
    sequence: EventSequence,
    payload: Map<String, Value>,
}

impl EventEnvelopeBuilder {
    /// Construct a builder for an event name and causality id.
    pub fn new(event: impl Into<String>, causality_id: CausalityId) -> Self {
        let mut payload = Map::new();
        payload.insert("metadata_only".to_string(), Value::Bool(true));
        Self {
            schema_version: 1,
            event: event.into(),
            severity: EventSeverity::Info,
            retention: RetentionLabel::Hot,
            redaction: RedactionHint::MetadataOnly,
            causality_id,
            parent_event_id: None,
            workspace_id: None,
            file_id: None,
            buffer_id: None,
            correlation_id: CorrelationId(1),
            principal_id: None,
            sequence: EventSequence(1),
            payload,
        }
    }

    /// Set severity.
    pub fn severity(mut self, severity: EventSeverity) -> Self {
        self.severity = severity;
        self
    }

    /// Set retention.
    pub fn retention(mut self, retention: RetentionLabel) -> Self {
        self.retention = retention;
        self
    }

    /// Set redaction hint.
    pub fn redaction(mut self, redaction: RedactionHint) -> Self {
        self.redaction = redaction;
        self
    }

    /// Set parent event id.
    pub fn parent_event_id(mut self, parent_event_id: Option<EventId>) -> Self {
        self.parent_event_id = parent_event_id;
        self
    }

    /// Set workspace id.
    pub fn workspace_id(mut self, workspace_id: WorkspaceId) -> Self {
        self.workspace_id = Some(workspace_id);
        self
    }

    /// Set file id.
    pub fn file_id(mut self, file_id: FileId) -> Self {
        self.file_id = Some(file_id);
        self.payload.insert("file_id".to_string(), json!(file_id.0));
        self
    }

    /// Set buffer id.
    pub fn buffer_id(mut self, buffer_id: BufferId) -> Self {
        self.buffer_id = Some(buffer_id);
        self.payload
            .insert("buffer_id".to_string(), json!(buffer_id.0));
        self
    }

    /// Set correlation id.
    pub fn correlation_id(mut self, correlation_id: CorrelationId) -> Self {
        self.correlation_id = correlation_id;
        self
    }

    /// Set principal id.
    pub fn principal_id(mut self, principal_id: PrincipalId) -> Self {
        self.principal_id = Some(principal_id);
        self
    }

    /// Set sequence.
    pub fn sequence(mut self, sequence: EventSequence) -> Self {
        self.sequence = sequence;
        self
    }

    /// Add metadata payload key.
    pub fn metadata(mut self, key: impl Into<String>, value: impl Into<Value>) -> Self {
        self.payload.insert(key.into(), value.into());
        self
    }

    /// Build the event envelope.
    pub fn build(self) -> EventEnvelope {
        EventEnvelope {
            schema_version: self.schema_version,
            event_id: EventId(Uuid::now_v7()),
            parent_event_id: self.parent_event_id,
            causality_id: self.causality_id,
            event: self.event,
            severity: self.severity,
            retention: self.retention,
            redaction: self.redaction,
            correlation_id: self.correlation_id,
            workspace_id: self.workspace_id,
            sequence: self.sequence,
            principal_id: self.principal_id,
            occurred_at: TimestampMillis::now(),
            payload: Value::Object(self.payload),
        }
    }
}

/// Build durable, metadata-only event metadata from a validated envelope.
pub fn event_metadata_record(envelope: &EventEnvelope) -> EventMetadataRecord {
    EventMetadataRecord {
        event_id: envelope.event_id,
        parent_event_id: envelope.parent_event_id,
        causality_id: envelope.causality_id,
        correlation_id: envelope.correlation_id,
        event: envelope.event.clone(),
        workspace_id: envelope.workspace_id,
        sequence: envelope.sequence,
        principal_id: envelope.principal_id.clone(),
        retention: envelope.retention,
        redaction: RedactionHint::MetadataOnly,
        occurred_at: envelope.occurred_at,
        schema_version: 1,
    }
}

/// Build a redacted proposal audit record without raw source text or raw paths.
pub fn proposal_audit_record(
    proposal: &WorkspaceProposal,
    transition: &ProposalLifecycleTransition,
) -> ProposalAuditRecord {
    ProposalAuditRecord {
        proposal_id: proposal.proposal_id,
        lifecycle_state: transition.lifecycle_state,
        timestamp: transition.timestamp,
        principal: transition.principal.clone(),
        capability: transition.capability.clone(),
        correlation_id: transition.correlation_id,
        causality_id: transition.causality_id,
        payload_summary: proposal_payload_summary(proposal),
        diagnostics: redacted_diagnostics(&transition.diagnostics),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    }
}

/// Build an envelope-ready metadata DTO proving that proposal audit storage completed.
pub fn proposal_audit_recorded_event(
    proposal: &WorkspaceProposal,
    transition: &ProposalLifecycleTransition,
    sequence: EventSequence,
) -> EventEnvelope {
    assert_core_ids(transition.causality_id, transition.correlation_id, sequence);
    let summary = proposal_payload_summary(proposal);
    let mut builder = EventEnvelopeBuilder::new("proposal.audit_recorded", transition.causality_id)
        .correlation_id(transition.correlation_id)
        .sequence(sequence)
        .principal_id(transition.principal.clone())
        .severity(EventSeverity::Info)
        .retention(RetentionLabel::Audit)
        .metadata("proposal_id", json!(proposal.proposal_id.0))
        .metadata(
            "lifecycle_state",
            format!("{:?}", transition.lifecycle_state),
        )
        .metadata("capability", transition.capability.0.clone())
        .metadata("payload_kind", format!("{:?}", summary.kind))
        .metadata("affected_file_count", summary.affected_files.len() as u64)
        .metadata("diagnostics", diagnostics_summary(&transition.diagnostics));

    if let Some(workspace_id) = proposal_workspace_id(proposal) {
        builder = builder.workspace_id(workspace_id);
    }
    for file_id in summary.affected_files.into_iter().take(1) {
        builder = builder.file_id(file_id);
    }

    builder.build()
}

/// Build a metadata-only assisted-AI audit record without provider invocation or mutation authority.
pub fn assisted_ai_audit_record(
    request: &AssistedAiRequestContract,
    projection: Option<&AssistedAiProjection>,
    preview: Option<&AssistedAiProposalPreviewSummary>,
    boundary: Option<&AssistedAiConsentBoundary>,
    outcome_category: AssistedAiAuditOutcomeCategory,
    event_sequence: EventSequence,
    schema_version: u16,
) -> Result<AssistedAiAuditRecord, AssistedAiContractError> {
    reject_forbidden_assisted_ai_metadata("request", request)?;
    if let Some(projection) = projection {
        reject_forbidden_assisted_ai_metadata("projection", projection)?;
    }
    if let Some(preview) = preview {
        reject_forbidden_assisted_ai_metadata("preview", preview)?;
    }

    let route = &request.route_decision;
    let refusal_error_category = route
        .refusal
        .as_ref()
        .map(|refusal| refusal.reason_code.clone())
        .or_else(|| {
            preview
                .and_then(|preview| preview.refusal.as_ref())
                .map(|refusal| refusal.reason_code.clone())
        });
    let privacy_disposition = assisted_ai_privacy_disposition(request, preview, boundary);
    let mut risk_labels = vec![request.proposal_intent.risk_label];
    let mut privacy_labels = vec![request.proposal_intent.privacy_label];
    if let Some(refusal) = route.refusal.as_ref() {
        risk_labels.push(refusal.risk_label);
    }
    if let Some(preview) = preview {
        risk_labels.push(preview.risk_label);
        privacy_labels.push(preview.privacy_label);
    }

    let record = AssistedAiAuditRecord {
        audit_id: format!("assist:audit:{}:{}", request.request_id, event_sequence.0),
        provider_capability_id: request.provider.provider_id.clone(),
        provider_capability_hash: metadata_fingerprint(
            "assisted-ai-provider",
            &serde_json::to_string(&request.provider).unwrap_or_default(),
        ),
        route_decision_id: format!("assist:route:{}", request.request_id),
        route_decision_hash: metadata_fingerprint(
            "assisted-ai-route",
            &serde_json::to_string(&request.route_decision).unwrap_or_default(),
        ),
        consent_disposition: boundary.map(|boundary| boundary.consent_state),
        budget_dispositions: assisted_ai_budget_dispositions(request, boundary),
        privacy_disposition,
        request_contract_id: request.request_id.clone(),
        request_contract_hash: metadata_fingerprint(
            "assisted-ai-request",
            &serde_json::to_string(request).unwrap_or_default(),
        ),
        projection_id: projection.map(|projection| projection.projection_id.clone()),
        projection_hash: projection.map(|projection| {
            metadata_fingerprint(
                "assisted-ai-projection",
                &serde_json::to_string(projection).unwrap_or_default(),
            )
        }),
        preview_id: preview.map(|preview| preview.preview_id.clone()),
        preview_hash: preview.map(|preview| {
            metadata_fingerprint(
                "assisted-ai-preview",
                &serde_json::to_string(preview).unwrap_or_default(),
            )
        }),
        proposal_id: preview.map(|preview| preview.proposal_id),
        outcome_category,
        refusal_error_category,
        correlation_id: request.correlation_id,
        causality_id: request.causality_id,
        event_sequence,
        risk_labels,
        privacy_labels,
        redaction_state: AssistedAiAuditRedactionState::MetadataOnly,
        runtime_invocation_state: AssistedAiProviderInvocationState::NotEncoded,
        runtime_activation_labels: vec![
            "provider.invocation.not_encoded".to_string(),
            "network.not_encoded".to_string(),
            "tool.disabled".to_string(),
            "agent.disabled".to_string(),
            "terminal.disabled".to_string(),
            "proposal.apply.not_encoded".to_string(),
        ],
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version,
    };
    validate_assisted_ai_audit_record(&record)?;
    Ok(record)
}

/// Build an envelope-ready event from a validated assisted-AI audit record.
pub fn assisted_ai_audit_recorded_event(
    record: &AssistedAiAuditRecord,
) -> Result<EventEnvelope, AssistedAiContractError> {
    validate_assisted_ai_audit_record(record)?;
    assert_core_ids(
        record.causality_id,
        record.correlation_id,
        record.event_sequence,
    );
    let mut builder = EventEnvelopeBuilder::new("assisted_ai.audit_recorded", record.causality_id)
        .correlation_id(record.correlation_id)
        .sequence(record.event_sequence)
        .severity(EventSeverity::Info)
        .retention(RetentionLabel::Audit)
        .metadata("audit_id", record.audit_id.clone())
        .metadata(
            "provider_capability_id",
            record.provider_capability_id.clone(),
        )
        .metadata(
            "provider_capability_hash",
            record.provider_capability_hash.value.clone(),
        )
        .metadata("route_decision_id", record.route_decision_id.clone())
        .metadata(
            "route_decision_hash",
            record.route_decision_hash.value.clone(),
        )
        .metadata("request_contract_id", record.request_contract_id.clone())
        .metadata(
            "request_contract_hash",
            record.request_contract_hash.value.clone(),
        )
        .metadata("outcome_category", format!("{:?}", record.outcome_category))
        .metadata(
            "runtime_invocation_state",
            format!("{:?}", record.runtime_invocation_state),
        )
        .metadata("risk_label_count", record.risk_labels.len() as u64)
        .metadata("privacy_label_count", record.privacy_labels.len() as u64)
        .metadata(
            "budget_disposition_count",
            record.budget_dispositions.len() as u64,
        )
        .metadata(
            "runtime_activation_label_count",
            record.runtime_activation_labels.len() as u64,
        );
    if let Some(proposal_id) = record.proposal_id {
        builder = builder.metadata("proposal_id", json!(proposal_id.0));
    }
    if let Some(preview_id) = &record.preview_id {
        builder = builder.metadata("preview_id", preview_id.clone());
    }
    if let Some(refusal) = &record.refusal_error_category {
        builder = builder.metadata("refusal_error_category", refusal.clone());
    }
    Ok(builder.build())
}

/// Build a metadata-only delegated-task readiness/audit linkage record without runtime activation.
pub fn delegated_task_readiness_audit_linkage_record(
    plan: &DelegatedTaskPlanContract,
    plan_hash: FileFingerprint,
    audit_references: Vec<DelegatedTaskAssistedAiAuditReference>,
    event_sequence: EventSequence,
    schema_version: u16,
) -> Result<DelegatedTaskAuditLinkageRecord, AssistedAiContractError> {
    delegated_task_audit_linkage_record(
        format!(
            "delegated-task:audit-linkage:{}:{}",
            plan.plan_id.0, event_sequence.0
        ),
        plan,
        plan_hash,
        audit_references,
        event_sequence,
        schema_version,
    )
}

/// Build an envelope-ready event from a validated delegated-task readiness/audit linkage record.
pub fn delegated_task_readiness_audit_linkage_recorded_event(
    record: &DelegatedTaskAuditLinkageRecord,
) -> Result<EventEnvelope, AssistedAiContractError> {
    validate_delegated_task_audit_linkage_record(record)?;
    assert_core_ids(
        record.causality_id,
        record.correlation_id,
        record.event_sequence,
    );
    Ok(EventEnvelopeBuilder::new(
        "delegated_task.readiness_audit_linkage_recorded",
        record.causality_id,
    )
    .correlation_id(record.correlation_id)
    .sequence(record.event_sequence)
    .severity(EventSeverity::Info)
    .retention(RetentionLabel::Audit)
    .metadata("linkage_id", record.linkage_id.clone())
    .metadata("plan_id", record.plan_id.0.clone())
    .metadata("plan_hash", record.plan_hash.value.clone())
    .metadata(
        "readiness_classification",
        format!("{:?}", record.readiness_classification),
    )
    .metadata("step_count", record.step_ids.len() as u64)
    .metadata(
        "proposal_preview_link_count",
        record.proposal_preview_links.len() as u64,
    )
    .metadata(
        "audit_reference_count",
        record.assisted_ai_audit_references.len() as u64,
    )
    .metadata("proposal_id_count", record.proposal_ids.len() as u64)
    .metadata("blocker_count", record.blockers.len() as u64)
    .metadata("refusal_count", record.refusals.len() as u64)
    .metadata(
        "runtime_activation",
        format!("{:?}", record.runtime_activation),
    )
    .build())
}

/// Build an envelope-ready event from a validated Phase 4 runtime audit record.
pub fn phase4_runtime_audit_recorded_event(
    record: &Phase4RuntimeAuditRecord,
) -> Result<EventEnvelope, AssistedAiContractError> {
    validate_phase4_runtime_audit_record(record)?;
    assert_core_ids(
        record.causality_id,
        record.correlation_id,
        record.event_sequence,
    );
    let mut builder =
        EventEnvelopeBuilder::new("phase4.runtime_audit_recorded", record.causality_id)
            .correlation_id(record.correlation_id)
            .sequence(record.event_sequence)
            .severity(EventSeverity::Info)
            .retention(RetentionLabel::Audit)
            .metadata("audit_id", record.audit_id.clone())
            .metadata("invocation_state", format!("{:?}", record.invocation_state))
            .metadata("outcome_label", record.outcome_label.clone())
            .metadata("label_count", record.labels.len() as u64);
    if let Some(run_id) = &record.run_id {
        builder = builder.metadata("run_id", run_id.0.clone());
    }
    if let Some(step_id) = &record.step_id {
        builder = builder.metadata("step_id", step_id.0.clone());
    }
    if let Some(provider_route_id) = &record.provider_route_id {
        builder = builder.metadata("provider_route_id", provider_route_id.clone());
    }
    Ok(builder.build())
}

/// Build an envelope-ready event from a validated agent replay manifest.
pub fn agent_replay_manifest_recorded_event(
    manifest: &AgentReplayManifest,
) -> Result<EventEnvelope, AssistedAiContractError> {
    validate_agent_replay_manifest(manifest)?;
    assert_core_ids(
        manifest.causality_id,
        manifest.correlation_id,
        manifest.event_sequence,
    );
    Ok(EventEnvelopeBuilder::new(
        "phase4.agent_replay_manifest_recorded",
        manifest.causality_id,
    )
    .correlation_id(manifest.correlation_id)
    .sequence(manifest.event_sequence)
    .severity(EventSeverity::Info)
    .retention(RetentionLabel::Audit)
    .metadata("run_id", manifest.run_id.0.clone())
    .metadata("transition_count", manifest.transitions.len() as u64)
    .metadata(
        "context_manifest_count",
        manifest.context_manifests.len() as u64,
    )
    .metadata(
        "provider_route_count",
        manifest.provider_route_ids.len() as u64,
    )
    .metadata("proposal_id_count", manifest.proposal_ids.len() as u64)
    .build())
}

/// Summarize a proposal payload using identifiers, hashes, counts, and byte lengths only.
pub fn proposal_payload_summary(proposal: &WorkspaceProposal) -> ProposalPayloadSummary {
    match &proposal.payload {
        ProposalPayload::TextEdit(payload) => ProposalPayloadSummary {
            kind: ProposalPayloadKind::TextEdit,
            affected_files: vec![payload.file_id],
            title: Some("text-edit".to_string()),
            byte_count: Some(
                payload
                    .edits
                    .edits
                    .iter()
                    .map(|edit| edit.replacement.len() as u64)
                    .sum(),
            ),
        },
        ProposalPayload::CreateFile(payload) => ProposalPayloadSummary {
            kind: ProposalPayloadKind::CreateFile,
            affected_files: Vec::new(),
            title: Some(format!("path_hash={}", metadata_hash(&payload.path.0))),
            byte_count: payload
                .initial_content
                .as_ref()
                .map(|text| text.len() as u64),
        },
        ProposalPayload::DeleteFile(payload) => ProposalPayloadSummary {
            kind: ProposalPayloadKind::DeleteFile,
            affected_files: vec![payload.file.file_id],
            title: Some("delete-file".to_string()),
            byte_count: None,
        },
        ProposalPayload::RenameFile(payload) => ProposalPayloadSummary {
            kind: ProposalPayloadKind::RenameFile,
            affected_files: vec![payload.file.file_id],
            title: Some(format!(
                "destination_path_hash={}",
                metadata_hash(&payload.destination.0)
            )),
            byte_count: None,
        },
        ProposalPayload::SaveFile(payload) => ProposalPayloadSummary {
            kind: ProposalPayloadKind::SaveFile,
            affected_files: vec![payload.file_id],
            title: Some("save-file".to_string()),
            byte_count: None,
        },
        ProposalPayload::FormatFile(payload) => ProposalPayloadSummary {
            kind: ProposalPayloadKind::FormatFile,
            affected_files: vec![payload.file.file_id],
            title: Some("format-file".to_string()),
            byte_count: None,
        },
        ProposalPayload::CodeAction(payload) => ProposalPayloadSummary {
            kind: ProposalPayloadKind::CodeAction,
            affected_files: vec![payload.file.file_id],
            title: Some(format!("title_hash={}", metadata_hash(&payload.title))),
            byte_count: Some(
                payload
                    .edits
                    .iter()
                    .map(|edit| edit.replacement.len() as u64)
                    .sum(),
            ),
        },
        ProposalPayload::TerminalCommand(payload) => ProposalPayloadSummary {
            kind: ProposalPayloadKind::TerminalCommand,
            affected_files: Vec::new(),
            title: Some(format!("command_hash={}", metadata_hash(&payload.command))),
            byte_count: None,
        },
        ProposalPayload::Batch(payload) => ProposalPayloadSummary {
            kind: ProposalPayloadKind::Batch,
            affected_files: payload
                .target_coverage
                .targets
                .iter()
                .filter_map(|target| target.file_id)
                .collect(),
            title: Some(format!(
                "batch_items={} atomicity={:?}",
                payload.items.len(),
                payload.atomicity
            )),
            byte_count: None,
        },
        ProposalPayload::WorkspaceEdit(payload) => ProposalPayloadSummary {
            kind: ProposalPayloadKind::WorkspaceEdit,
            affected_files: payload
                .target_coverage
                .targets
                .iter()
                .filter_map(|target| target.file_id)
                .collect(),
            title: Some(format!("source={:?}", payload.source)),
            byte_count: None,
        },
    }
}

/// Build an envelope-ready metadata DTO for a denied save.
pub fn save_denied_event(
    workspace_id: WorkspaceId,
    file_id: FileId,
    correlation_id: CorrelationId,
    causality_id: CausalityId,
    sequence: EventSequence,
    reason: impl Into<String>,
) -> EventEnvelope {
    assert_core_ids(causality_id, correlation_id, sequence);
    let reason = reason.into();
    EventEnvelopeBuilder::new("workspace.save_denied", causality_id)
        .workspace_id(workspace_id)
        .file_id(file_id)
        .correlation_id(correlation_id)
        .sequence(sequence)
        .severity(EventSeverity::Warning)
        .retention(RetentionLabel::Audit)
        .metadata("reason_hash", metadata_hash(&reason))
        .metadata("reason_len", reason.len() as u64)
        .build()
}

/// Build an envelope-ready metadata DTO for denied path escape attempts.
pub fn path_escape_denied_event(
    workspace_id: WorkspaceId,
    correlation_id: CorrelationId,
    causality_id: CausalityId,
    sequence: EventSequence,
    path: impl Into<String>,
) -> EventEnvelope {
    assert_core_ids(causality_id, correlation_id, sequence);
    let path = path.into();
    EventEnvelopeBuilder::new("workspace.path_escape_denied", causality_id)
        .workspace_id(workspace_id)
        .correlation_id(correlation_id)
        .sequence(sequence)
        .severity(EventSeverity::Warning)
        .retention(RetentionLabel::Audit)
        .metadata("path_hash", metadata_hash(&path))
        .metadata("path_len", path.len() as u64)
        .build()
}

/// Build an envelope-ready metadata DTO for editor transaction outcomes.
pub fn transaction_event(
    descriptor: &TextTransactionDescriptor,
    applied: bool,
    reason: Option<&str>,
    sequence: EventSequence,
) -> EventEnvelope {
    assert_core_ids(descriptor.causality_id, descriptor.correlation_id, sequence);
    let event = if applied {
        "editor.transaction_applied"
    } else {
        "editor.transaction_failed"
    };
    let mut builder = EventEnvelopeBuilder::new(event, descriptor.causality_id)
        .workspace_id(descriptor.workspace_id)
        .file_id(descriptor.file_id)
        .buffer_id(descriptor.buffer_id)
        .correlation_id(descriptor.correlation_id)
        .sequence(sequence)
        .severity(if applied {
            EventSeverity::Info
        } else {
            EventSeverity::Error
        })
        .retention(RetentionLabel::Warm)
        .metadata("transaction_id", descriptor.transaction_id.to_string())
        .metadata("schema_version", json!(descriptor.schema_version))
        .metadata("changed_ranges", json!(descriptor.changed_ranges.len()))
        .metadata("pre_snapshot_id", json!(descriptor.pre_snapshot_id.0))
        .metadata("post_snapshot_id", json!(descriptor.post_snapshot_id.0))
        .metadata("pre_buffer_version", json!(descriptor.pre_buffer_version.0))
        .metadata(
            "post_buffer_version",
            json!(descriptor.post_buffer_version.0),
        );

    if let Some(reason) = reason {
        builder = builder
            .metadata("reason_hash", metadata_hash(reason))
            .metadata("reason_len", reason.len() as u64);
    }

    builder.build()
}

/// Build an envelope-ready metadata DTO for watcher overflow or recovery.
pub fn watcher_recovery_event(
    workspace_id: WorkspaceId,
    correlation_id: CorrelationId,
    causality_id: CausalityId,
    sequence: EventSequence,
    recovered: bool,
) -> EventEnvelope {
    assert_core_ids(causality_id, correlation_id, sequence);
    EventEnvelopeBuilder::new(
        if recovered {
            "workspace.watcher_recovery"
        } else {
            "workspace.watcher_overflow"
        },
        causality_id,
    )
    .workspace_id(workspace_id)
    .correlation_id(correlation_id)
    .sequence(sequence)
    .severity(if recovered {
        EventSeverity::Info
    } else {
        EventSeverity::Warning
    })
    .retention(RetentionLabel::Warm)
    .metadata("recovered", recovered)
    .build()
}

/// Build a proposal-created lifecycle event.
pub fn proposal_created_event(
    proposal: &WorkspaceProposal,
    causality_id: CausalityId,
    sequence: EventSequence,
) -> EventEnvelope {
    proposal_lifecycle_event(
        "proposal.created",
        proposal,
        ProposalLifecycleState::Created,
        proposal.correlation_id,
        causality_id,
        sequence,
        EventSeverity::Info,
        None,
        &[],
    )
}

/// Build a proposal-validated lifecycle event.
pub fn proposal_validated_event(
    proposal: &WorkspaceProposal,
    transition: &ProposalLifecycleTransition,
    sequence: EventSequence,
) -> EventEnvelope {
    proposal_transition_event("proposal.validated", proposal, transition, sequence, None)
}

/// Build a proposal-previewed lifecycle event.
pub fn proposal_previewed_event(
    proposal: &WorkspaceProposal,
    transition: &ProposalLifecycleTransition,
    sequence: EventSequence,
) -> EventEnvelope {
    proposal_transition_event("proposal.previewed", proposal, transition, sequence, None)
}

/// Build a proposal-approved lifecycle event.
pub fn proposal_approved_event(
    proposal: &WorkspaceProposal,
    transition: &ProposalLifecycleTransition,
    sequence: EventSequence,
) -> EventEnvelope {
    proposal_transition_event("proposal.approved", proposal, transition, sequence, None)
}

/// Build a proposal-rejected lifecycle event.
pub fn proposal_rejected_event(
    proposal: &WorkspaceProposal,
    transition: &ProposalLifecycleTransition,
    reason: ProposalRejectionReason,
    sequence: EventSequence,
) -> EventEnvelope {
    proposal_transition_event(
        "proposal.rejected",
        proposal,
        transition,
        sequence,
        Some(format!("{reason:?}")),
    )
}

/// Build a proposal-applied lifecycle event.
pub fn proposal_applied_event(
    proposal: &WorkspaceProposal,
    transition: &ProposalLifecycleTransition,
    sequence: EventSequence,
) -> EventEnvelope {
    proposal_transition_event("proposal.applied", proposal, transition, sequence, None)
}

/// Build a proposal-failed lifecycle event.
pub fn proposal_failed_event(
    proposal: &WorkspaceProposal,
    transition: &ProposalLifecycleTransition,
    reason: ProposalFailureReason,
    sequence: EventSequence,
) -> EventEnvelope {
    proposal_transition_event(
        "proposal.failed",
        proposal,
        transition,
        sequence,
        Some(format!("{reason:?}")),
    )
}

/// Build a proposal-rolled-back lifecycle event.
pub fn proposal_rolled_back_event(
    proposal: &WorkspaceProposal,
    transition: &ProposalLifecycleTransition,
    reason: ProposalRollbackReason,
    sequence: EventSequence,
) -> EventEnvelope {
    proposal_transition_event(
        "proposal.rolled_back",
        proposal,
        transition,
        sequence,
        Some(format!("{reason:?}")),
    )
}

/// Build a stale-proposal rejection event.
pub fn stale_proposal_rejected_event(
    workspace_id: WorkspaceId,
    file_id: FileId,
    correlation_id: CorrelationId,
    causality_id: CausalityId,
    sequence: EventSequence,
    proposal_id: devil_protocol::ProposalId,
    reason: ProposalStaleReason,
) -> EventEnvelope {
    assert_core_ids(causality_id, correlation_id, sequence);
    EventEnvelopeBuilder::new("proposal.stale_rejected", causality_id)
        .workspace_id(workspace_id)
        .file_id(file_id)
        .correlation_id(correlation_id)
        .sequence(sequence)
        .severity(EventSeverity::Warning)
        .retention(RetentionLabel::Audit)
        .metadata("proposal_id", json!(proposal_id.0))
        .metadata("stale_reason", format!("{reason:?}"))
        .build()
}

/// Build a file-conflict-created event.
pub fn conflict_created_event(
    conflict: &devil_protocol::FileConflictState,
    correlation_id: CorrelationId,
    causality_id: CausalityId,
    sequence: EventSequence,
) -> EventEnvelope {
    assert_core_ids(causality_id, correlation_id, sequence);
    let context = &conflict.context;
    let mut builder = EventEnvelopeBuilder::new("workspace.conflict_created", causality_id)
        .workspace_id(context.workspace_id)
        .file_id(context.file_identity.file_id)
        .correlation_id(correlation_id)
        .sequence(sequence)
        .severity(EventSeverity::Warning)
        .retention(RetentionLabel::Audit)
        .metadata("state", format!("{:?}", conflict.state))
        .metadata("reason", format!("{:?}", context.reason))
        .metadata("buffer_version", json!(context.buffer_version.0))
        .metadata(
            "file_content_version",
            json!(context.file_content_version.0),
        )
        .metadata("snapshot_id", json!(context.snapshot_id.0))
        .metadata("diagnostics", diagnostics_summary(&conflict.diagnostics));
    builder = add_fingerprint_metadata(builder, "expected", context.expected_fingerprint.as_ref());
    builder = add_fingerprint_metadata(builder, "disk", context.disk_fingerprint.as_ref());
    builder.build()
}

/// Build a non-atomic fallback-attempted event.
pub fn fallback_attempted_event(
    workspace_id: WorkspaceId,
    file_id: FileId,
    correlation_id: CorrelationId,
    causality_id: CausalityId,
    sequence: EventSequence,
    policy: impl Into<String>,
) -> EventEnvelope {
    fallback_event(
        "workspace.fallback_attempted",
        EventSeverity::Warning,
        workspace_id,
        file_id,
        correlation_id,
        causality_id,
        sequence,
        policy,
    )
}

/// Build a non-atomic fallback-denied event.
pub fn fallback_denied_event(
    workspace_id: WorkspaceId,
    file_id: FileId,
    correlation_id: CorrelationId,
    causality_id: CausalityId,
    sequence: EventSequence,
    policy: impl Into<String>,
) -> EventEnvelope {
    fallback_event(
        "workspace.fallback_denied",
        EventSeverity::Warning,
        workspace_id,
        file_id,
        correlation_id,
        causality_id,
        sequence,
        policy,
    )
}

/// Build a non-atomic fallback-applied event.
pub fn fallback_applied_event(
    workspace_id: WorkspaceId,
    file_id: FileId,
    correlation_id: CorrelationId,
    causality_id: CausalityId,
    sequence: EventSequence,
    policy: impl Into<String>,
) -> EventEnvelope {
    fallback_event(
        "workspace.fallback_applied",
        EventSeverity::Info,
        workspace_id,
        file_id,
        correlation_id,
        causality_id,
        sequence,
        policy,
    )
}

/// Build an editor snapshot-retention degradation event.
#[allow(clippy::too_many_arguments)]
pub fn editor_retention_degradation_event(
    workspace_id: WorkspaceId,
    buffer_id: BufferId,
    correlation_id: CorrelationId,
    causality_id: CausalityId,
    sequence: EventSequence,
    retained_snapshot_count: usize,
    evicted_snapshot_count: usize,
    estimated_bytes: usize,
    reason: impl Into<String>,
) -> EventEnvelope {
    assert_core_ids(causality_id, correlation_id, sequence);
    let reason = reason.into();
    EventEnvelopeBuilder::new("editor.retention_degraded", causality_id)
        .workspace_id(workspace_id)
        .buffer_id(buffer_id)
        .correlation_id(correlation_id)
        .sequence(sequence)
        .severity(EventSeverity::Warning)
        .retention(RetentionLabel::Warm)
        .metadata("retained_snapshot_count", retained_snapshot_count as u64)
        .metadata("evicted_snapshot_count", evicted_snapshot_count as u64)
        .metadata("estimated_bytes", estimated_bytes as u64)
        .metadata("reason_hash", metadata_hash(&reason))
        .metadata("reason_len", reason.len() as u64)
        .build()
}

/// Build a security-denial event with path and reason redacted to metadata hashes.
#[allow(clippy::too_many_arguments)]
pub fn security_denial_event(
    workspace_id: WorkspaceId,
    file_id: Option<FileId>,
    principal_id: Option<PrincipalId>,
    capability: &CapabilityId,
    correlation_id: CorrelationId,
    causality_id: CausalityId,
    sequence: EventSequence,
    target_path: Option<&str>,
    reason: impl Into<String>,
) -> EventEnvelope {
    assert_core_ids(causality_id, correlation_id, sequence);
    let reason = reason.into();
    let mut builder = EventEnvelopeBuilder::new("security.denial", causality_id)
        .workspace_id(workspace_id)
        .correlation_id(correlation_id)
        .sequence(sequence)
        .severity(EventSeverity::Warning)
        .retention(RetentionLabel::Audit)
        .metadata("capability", capability.0.clone())
        .metadata("reason_hash", metadata_hash(&reason))
        .metadata("reason_len", reason.len() as u64);
    if let Some(file_id) = file_id {
        builder = builder.file_id(file_id);
    }
    if let Some(principal_id) = principal_id {
        builder = builder.principal_id(principal_id);
    }
    if let Some(path) = target_path {
        builder = builder
            .metadata("target_path_hash", metadata_hash(path))
            .metadata("target_path_len", path.len() as u64);
    }
    builder.build()
}

/// Build an open-file read-failure event with path and error text summarized.
pub fn open_file_read_failure_event(
    workspace_id: WorkspaceId,
    correlation_id: CorrelationId,
    causality_id: CausalityId,
    sequence: EventSequence,
    path: impl Into<String>,
    reason: impl Into<String>,
) -> EventEnvelope {
    assert_core_ids(causality_id, correlation_id, sequence);
    let path = path.into();
    let reason = reason.into();
    EventEnvelopeBuilder::new("workspace.open_read_failed", causality_id)
        .workspace_id(workspace_id)
        .correlation_id(correlation_id)
        .sequence(sequence)
        .severity(EventSeverity::Warning)
        .retention(RetentionLabel::Warm)
        .metadata("path_hash", metadata_hash(&path))
        .metadata("path_len", path.len() as u64)
        .metadata("reason_hash", metadata_hash(&reason))
        .metadata("reason_len", reason.len() as u64)
        .build()
}

fn proposal_transition_event(
    event: &'static str,
    proposal: &WorkspaceProposal,
    transition: &ProposalLifecycleTransition,
    sequence: EventSequence,
    reason: Option<String>,
) -> EventEnvelope {
    let severity = match transition.lifecycle_state {
        ProposalLifecycleState::Failed => EventSeverity::Error,
        ProposalLifecycleState::Denied
        | ProposalLifecycleState::Rejected
        | ProposalLifecycleState::RolledBack
        | ProposalLifecycleState::Stale
        | ProposalLifecycleState::Conflict => EventSeverity::Warning,
        _ => EventSeverity::Info,
    };
    proposal_lifecycle_event(
        event,
        proposal,
        transition.lifecycle_state,
        transition.correlation_id,
        transition.causality_id,
        sequence,
        severity,
        reason.as_deref(),
        &transition.diagnostics,
    )
}

#[allow(clippy::too_many_arguments)]
fn proposal_lifecycle_event(
    event: &'static str,
    proposal: &WorkspaceProposal,
    state: ProposalLifecycleState,
    correlation_id: CorrelationId,
    causality_id: CausalityId,
    sequence: EventSequence,
    severity: EventSeverity,
    reason: Option<&str>,
    diagnostics: &[ProtocolDiagnostic],
) -> EventEnvelope {
    assert_core_ids(causality_id, correlation_id, sequence);
    let summary = proposal_payload_summary(proposal);
    let mut builder = EventEnvelopeBuilder::new(event, causality_id)
        .correlation_id(correlation_id)
        .sequence(sequence)
        .principal_id(proposal.principal.clone())
        .severity(severity)
        .retention(RetentionLabel::Audit)
        .metadata("proposal_id", json!(proposal.proposal_id.0))
        .metadata("lifecycle_state", format!("{state:?}"))
        .metadata("capability", proposal.capability.0.clone())
        .metadata("payload_kind", format!("{:?}", summary.kind))
        .metadata("affected_file_count", summary.affected_files.len() as u64)
        .metadata("diagnostics", diagnostics_summary(diagnostics));

    if let Some(workspace_id) = proposal_workspace_id(proposal) {
        builder = builder.workspace_id(workspace_id);
    }
    for file_id in summary.affected_files.into_iter().take(1) {
        builder = builder.file_id(file_id);
    }
    if let Some(byte_count) = summary.byte_count {
        builder = builder.metadata("payload_byte_count", byte_count);
    }
    if let Some(title) = summary.title {
        builder = builder.metadata("title", title);
    }
    if let Some(reason) = reason {
        builder = builder.metadata("reason", reason.to_string());
    }

    builder.build()
}

fn proposal_workspace_id(proposal: &WorkspaceProposal) -> Option<WorkspaceId> {
    match &proposal.payload {
        ProposalPayload::DeleteFile(payload) => Some(payload.file.workspace_id),
        ProposalPayload::RenameFile(payload) => Some(payload.file.workspace_id),
        ProposalPayload::SaveFile(payload) => Some(payload.file.workspace_id),
        ProposalPayload::FormatFile(payload) => Some(payload.file.workspace_id),
        ProposalPayload::CodeAction(payload) => Some(payload.file.workspace_id),
        ProposalPayload::Batch(payload) => payload
            .target_coverage
            .targets
            .iter()
            .find_map(|target| target.workspace_id),
        ProposalPayload::WorkspaceEdit(payload) => Some(payload.workspace_id),
        ProposalPayload::TextEdit(_)
        | ProposalPayload::CreateFile(_)
        | ProposalPayload::TerminalCommand(_) => None,
    }
}

#[allow(clippy::too_many_arguments)]
fn fallback_event(
    event: &'static str,
    severity: EventSeverity,
    workspace_id: WorkspaceId,
    file_id: FileId,
    correlation_id: CorrelationId,
    causality_id: CausalityId,
    sequence: EventSequence,
    policy: impl Into<String>,
) -> EventEnvelope {
    assert_core_ids(causality_id, correlation_id, sequence);
    let policy = policy.into();
    EventEnvelopeBuilder::new(event, causality_id)
        .workspace_id(workspace_id)
        .file_id(file_id)
        .correlation_id(correlation_id)
        .sequence(sequence)
        .severity(severity)
        .retention(RetentionLabel::Audit)
        .metadata("policy_hash", metadata_hash(&policy))
        .metadata("policy_len", policy.len() as u64)
        .build()
}

fn add_fingerprint_metadata(
    mut builder: EventEnvelopeBuilder,
    prefix: &str,
    fingerprint: Option<&devil_protocol::FileFingerprint>,
) -> EventEnvelopeBuilder {
    if let Some(fingerprint) = fingerprint {
        builder = builder
            .metadata(
                format!("{prefix}_fingerprint_algorithm"),
                fingerprint.algorithm.clone(),
            )
            .metadata(
                format!("{prefix}_fingerprint_hash"),
                metadata_hash(&fingerprint.value),
            )
            .metadata(
                format!("{prefix}_fingerprint_len"),
                fingerprint.value.len() as u64,
            );
    }
    builder
}

fn metadata_fingerprint(algorithm: &str, value: &str) -> FileFingerprint {
    FileFingerprint {
        algorithm: algorithm.to_string(),
        value: metadata_hash(value),
    }
}

fn assisted_ai_budget_dispositions(
    request: &AssistedAiRequestContract,
    boundary: Option<&AssistedAiConsentBoundary>,
) -> Vec<PermissionBudgetEvaluationDisposition> {
    boundary.map_or_else(
        || {
            request
                .permission_budget_evaluations
                .iter()
                .map(|evaluation| evaluation.disposition)
                .collect()
        },
        |boundary| {
            boundary
                .budget_evaluations
                .iter()
                .map(|evaluation| evaluation.disposition)
                .collect()
        },
    )
}

fn assisted_ai_privacy_disposition(
    request: &AssistedAiRequestContract,
    preview: Option<&AssistedAiProposalPreviewSummary>,
    boundary: Option<&AssistedAiConsentBoundary>,
) -> AssistedAiAuditPrivacyDisposition {
    if boundary.is_some_and(|boundary| !boundary.privacy_scope_allowed)
        || request
            .route_decision
            .reasons
            .iter()
            .any(|reason| reason == "privacy.scope_denied")
    {
        return AssistedAiAuditPrivacyDisposition::Denied;
    }
    if preview
        .is_some_and(|preview| preview.privacy_label == ProposalPrivacyLabel::RedactedSensitive)
        || request.redaction_hints.contains(&RedactionHint::Full)
    {
        return AssistedAiAuditPrivacyDisposition::Redacted;
    }
    if request.route_decision.disposition == AssistedAiRequestDisposition::MetadataOnlyReady {
        AssistedAiAuditPrivacyDisposition::Allowed
    } else {
        AssistedAiAuditPrivacyDisposition::Unknown
    }
}

fn reject_forbidden_assisted_ai_metadata<T: serde::Serialize>(
    field: &str,
    value: &T,
) -> Result<(), AssistedAiContractError> {
    let serialized = serde_json::to_string(value).map_err(|_| {
        AssistedAiContractError::NonMetadataOnlyAuditRecord {
            field: field.to_string(),
            reason: "metadata.serialize_failed".to_string(),
        }
    })?;
    if contains_forbidden_assisted_ai_marker(&serialized) {
        return Err(AssistedAiContractError::NonMetadataOnlyAuditRecord {
            field: field.to_string(),
            reason: "forbidden.raw_or_payload_marker".to_string(),
        });
    }
    Ok(())
}

fn contains_forbidden_assisted_ai_marker(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    [
        "raw prompt",
        "source_body",
        "provider_payload",
        "provider request payload",
        "provider response payload",
        "chatcompletionrequest",
        "terminal output",
        "full diff",
        "reconstructed file",
        "model-generated prose",
        "network_request",
        "tool_call",
        "runtime_started",
        "fn main",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
}

fn redacted_diagnostics(diagnostics: &[ProtocolDiagnostic]) -> Vec<ProtocolDiagnostic> {
    diagnostics
        .iter()
        .map(|diagnostic| ProtocolDiagnostic {
            code: diagnostic.code.clone(),
            message: format!(
                "redacted:hash={};len={}",
                metadata_hash(&diagnostic.message),
                diagnostic.message.len()
            ),
            severity: diagnostic.severity,
            path: diagnostic.path.as_ref().map(|path| {
                devil_protocol::CanonicalPath(format!("hash:{}", metadata_hash(&path.0)))
            }),
            range: diagnostic.range,
        })
        .collect()
}

fn diagnostics_summary(diagnostics: &[ProtocolDiagnostic]) -> Value {
    json!({
        "count": diagnostics.len(),
        "codes": diagnostics.iter().map(|diagnostic| diagnostic.code.clone()).collect::<Vec<_>>(),
    })
}

fn metadata_hash(value: &str) -> String {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn assert_core_ids(
    causality_id: CausalityId,
    correlation_id: CorrelationId,
    sequence: EventSequence,
) {
    assert_ne!(causality_id.0, Uuid::nil(), "causality_id must be non-zero");
    assert_ne!(correlation_id.0, 0, "correlation_id must be non-zero");
    assert_ne!(sequence.0, 0, "sequence must be non-zero");
}

fn validate_envelope(
    envelope: &EventEnvelope,
    config: EventSinkConfig,
) -> Result<(), ObservabilityError> {
    if config.require_schema_version && envelope.schema_version == 0 {
        return Err(ObservabilityError::InvalidSchemaVersion);
    }
    if envelope.event.trim().is_empty() {
        return Err(ObservabilityError::MissingEventName);
    }
    if envelope.causality_id.0 == Uuid::nil() {
        return Err(ObservabilityError::InvalidCausalityId);
    }
    if envelope.correlation_id.0 == 0 {
        return Err(ObservabilityError::InvalidCorrelationId);
    }
    if envelope.sequence.0 == 0 {
        return Err(ObservabilityError::InvalidSequence);
    }
    validate_payload_shape(&envelope.payload)
}

fn validate_payload_shape(payload: &Value) -> Result<(), ObservabilityError> {
    match payload {
        Value::Object(_) => Ok(()),
        _ => Err(ObservabilityError::InvalidPayload),
    }
}

fn redact_payload(payload: &Value, hint: RedactionHint) -> Value {
    match hint {
        RedactionHint::Full => json!({"redacted": true, "metadata_only": true}),
        RedactionHint::MetadataOnly | RedactionHint::None => match payload {
            Value::Object(map) => {
                let mut redacted = Map::new();
                for (key, value) in map {
                    if is_sensitive_key(key) {
                        redacted.insert(key.clone(), Value::String("<redacted>".to_string()));
                    } else if is_metadata_value(value) {
                        redacted.insert(key.clone(), value.clone());
                    } else {
                        redacted.insert(key.clone(), Value::String("<metadata-only>".to_string()));
                    }
                }
                redacted.insert("metadata_only".to_string(), Value::Bool(true));
                Value::Object(redacted)
            }
            _ => json!({"redacted": true, "metadata_only": true}),
        },
    }
}

fn is_sensitive_key(key: &str) -> bool {
    let lower = key.to_ascii_lowercase();
    lower.contains("text")
        || lower.contains("source")
        || lower.contains("secret")
        || lower.contains("token")
        || lower.contains("password")
        || lower.contains("payload")
}

fn is_metadata_value(value: &Value) -> bool {
    matches!(
        value,
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_)
    )
}

fn protocol_error(error: ObservabilityError) -> ProtocolError {
    ProtocolError {
        code: "observability_error".to_string(),
        message: error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event_with_payload(payload: Value) -> EventEnvelope {
        EventEnvelopeBuilder::new("test.event", CausalityId(Uuid::now_v7()))
            .metadata("payload", payload)
            .build()
    }

    fn save_proposal() -> WorkspaceProposal {
        WorkspaceProposal {
            proposal_id: devil_protocol::ProposalId(42),
            principal: PrincipalId("tester".to_string()),
            capability: CapabilityId("fs.write".to_string()),
            correlation_id: CorrelationId(7),
            payload: ProposalPayload::SaveFile(devil_protocol::SaveFileProposal {
                file: devil_protocol::FileIdentity {
                    file_id: FileId(3),
                    workspace_id: WorkspaceId(1),
                    canonical_path: devil_protocol::CanonicalPath("/secret/source.rs".to_string()),
                    content_version: devil_protocol::FileContentVersion(9),
                    content_hash: Some("hash-only".to_string()),
                },
                buffer_id: BufferId(2),
                file_id: FileId(3),
                snapshot_id: devil_protocol::SnapshotId(4),
                buffer_version: devil_protocol::BufferVersion(5),
                file_content_version: devil_protocol::FileContentVersion(9),
                workspace_generation: devil_protocol::WorkspaceGeneration(6),
                expected_fingerprint: Some(devil_protocol::FileFingerprint {
                    algorithm: "test".to_string(),
                    value: "raw-fingerprint-value".to_string(),
                }),
                save_intent: devil_protocol::SaveIntent::Manual,
                conflict_policy: devil_protocol::SaveConflictPolicy::RejectIfChanged,
                trust_decision: devil_protocol::TrustDecisionContext {
                    workspace_trust_state: devil_protocol::WorkspaceTrustState::Trusted,
                    decision_id: None,
                    decided_at: Some(TimestampMillis(1)),
                },
                required_capability: CapabilityId("fs.write".to_string()),
                principal: PrincipalId("tester".to_string()),
                correlation_id: CorrelationId(7),
                diagnostics: Vec::new(),
            }),
            preconditions: devil_protocol::ProposalVersionPreconditions {
                file_version: Some(devil_protocol::FileContentVersion(9)),
                buffer_version: Some(devil_protocol::BufferVersion(5)),
                snapshot_id: Some(devil_protocol::SnapshotId(4)),
                generation: Some(devil_protocol::WorkspaceGeneration(6)),
                file_content_version: Some(devil_protocol::FileContentVersion(9)),
                workspace_generation: Some(devil_protocol::WorkspaceGeneration(6)),
                expected_fingerprint: None,
                expected_file_length: Some(12),
                expected_modified_at: None,
            },
            preview: devil_protocol::PreviewSummary {
                summary: "save source".to_string(),
                details: vec!["raw path /secret/source.rs".to_string()],
            },
            expires_at: None,
            created_at: TimestampMillis(1),
        }
    }

    fn transition(state: ProposalLifecycleState) -> ProposalLifecycleTransition {
        ProposalLifecycleTransition {
            proposal_id: devil_protocol::ProposalId(42),
            lifecycle_state: state,
            timestamp: TimestampMillis(2),
            principal: PrincipalId("tester".to_string()),
            capability: CapabilityId("fs.write".to_string()),
            correlation_id: CorrelationId(7),
            causality_id: CausalityId(Uuid::now_v7()),
            diagnostics: vec![ProtocolDiagnostic {
                code: "diag.code".to_string(),
                message: "raw path /secret/source.rs".to_string(),
                severity: devil_protocol::ProtocolDiagnosticSeverity::Warning,
                path: Some(devil_protocol::CanonicalPath(
                    "/secret/source.rs".to_string(),
                )),
                range: None,
            }],
        }
    }

    fn assisted_ai_audit_fixture() -> AssistedAiAuditRecord {
        AssistedAiAuditRecord {
            audit_id: "assist:audit:req-1:1".to_string(),
            provider_capability_id: "provider:local-redacted".to_string(),
            provider_capability_hash: FileFingerprint {
                algorithm: "hash".to_string(),
                value: "provider-hash".to_string(),
            },
            route_decision_id: "assist:route:req-1".to_string(),
            route_decision_hash: FileFingerprint {
                algorithm: "hash".to_string(),
                value: "route-hash".to_string(),
            },
            consent_disposition: Some(devil_protocol::AssistedAiConsentState::Granted),
            budget_dispositions: vec![PermissionBudgetEvaluationDisposition::Allowed],
            privacy_disposition: AssistedAiAuditPrivacyDisposition::Allowed,
            request_contract_id: "assist:req:1".to_string(),
            request_contract_hash: FileFingerprint {
                algorithm: "hash".to_string(),
                value: "request-hash".to_string(),
            },
            projection_id: Some("assisted-ai:p6-3".to_string()),
            projection_hash: Some(FileFingerprint {
                algorithm: "hash".to_string(),
                value: "projection-hash".to_string(),
            }),
            preview_id: Some("assist:preview:701".to_string()),
            preview_hash: Some(FileFingerprint {
                algorithm: "hash".to_string(),
                value: "preview-hash".to_string(),
            }),
            proposal_id: Some(devil_protocol::ProposalId(701)),
            outcome_category: AssistedAiAuditOutcomeCategory::ProposalPreviewReady,
            refusal_error_category: None,
            correlation_id: CorrelationId(901),
            causality_id: CausalityId(Uuid::now_v7()),
            event_sequence: EventSequence(77),
            risk_labels: vec![devil_protocol::ProposalRiskLabel::Medium],
            privacy_labels: vec![ProposalPrivacyLabel::WorkspaceMetadata],
            redaction_state: AssistedAiAuditRedactionState::MetadataOnly,
            runtime_invocation_state: AssistedAiProviderInvocationState::NotEncoded,
            runtime_activation_labels: vec![
                "provider.invocation.not_encoded".to_string(),
                "network.not_encoded".to_string(),
                "tool.disabled".to_string(),
                "agent.disabled".to_string(),
            ],
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        }
    }

    #[test]
    fn in_memory_sink_rejects_missing_schema_version() {
        let sink = InMemoryEventSink::new();
        let mut envelope = event_with_payload(json!({"ok": true}));
        envelope.schema_version = 0;

        let err = sink
            .try_emit(EventSinkRequest { envelope })
            .expect_err("schema version should be required");

        assert_eq!(err, ObservabilityError::InvalidSchemaVersion);
    }

    #[test]
    fn in_memory_sink_rejects_zero_core_identifiers() {
        let sink = InMemoryEventSink::new();
        let mut envelope = event_with_payload(json!({"ok": true}));
        envelope.causality_id = CausalityId(Uuid::nil());
        assert_eq!(
            sink.try_emit(EventSinkRequest {
                envelope: envelope.clone(),
            })
            .expect_err("nil causality should be rejected"),
            ObservabilityError::InvalidCausalityId
        );

        envelope.causality_id = CausalityId(Uuid::now_v7());
        envelope.correlation_id = CorrelationId(0);
        assert_eq!(
            sink.try_emit(EventSinkRequest {
                envelope: envelope.clone(),
            })
            .expect_err("zero correlation should be rejected"),
            ObservabilityError::InvalidCorrelationId
        );

        envelope.correlation_id = CorrelationId(1);
        envelope.sequence = EventSequence(0);
        assert_eq!(
            sink.try_emit(EventSinkRequest { envelope })
                .expect_err("zero sequence should be rejected"),
            ObservabilityError::InvalidSequence
        );
    }

    #[test]
    fn redacting_sink_removes_source_text_for_metadata_only_retention() {
        let sink = RedactingEventSink::new();
        let mut envelope = EventEnvelopeBuilder::new("test.event", CausalityId(Uuid::now_v7()))
            .metadata("source_text", "fn secret() {}")
            .metadata("line", json!(7))
            .metadata("summary", "transaction")
            .build();
        envelope.redaction = RedactionHint::MetadataOnly;

        sink.try_emit(EventSinkRequest { envelope })
            .expect("redacted event should store");

        let stored = sink.events().expect("stored events");
        let payload = &stored[0].payload;
        assert_eq!(payload["source_text"], "<redacted>");
        assert_eq!(payload["line"], 7);
        assert_eq!(payload["metadata_only"], true);
    }

    #[test]
    fn helpers_construct_envelope_ready_metadata_with_schema_and_causality() {
        let causality = CausalityId(Uuid::now_v7());
        let envelope = save_denied_event(
            WorkspaceId(10),
            FileId(11),
            CorrelationId(12),
            causality,
            EventSequence(13),
            "untrusted workspace",
        );

        assert_eq!(envelope.schema_version, 1);
        assert_eq!(envelope.causality_id, causality);
        assert_eq!(envelope.correlation_id, CorrelationId(12));
        assert_eq!(envelope.sequence, EventSequence(13));
        assert_eq!(envelope.workspace_id, Some(WorkspaceId(10)));
        assert_eq!(envelope.retention, RetentionLabel::Audit);
        assert_eq!(envelope.redaction, RedactionHint::MetadataOnly);
        assert_eq!(envelope.payload["reason_len"], 19);
    }

    #[test]
    fn transaction_and_watcher_helpers_cover_required_event_scenarios() {
        let transaction_id = Uuid::now_v7();
        let causality = CausalityId(Uuid::now_v7());
        let descriptor = TextTransactionDescriptor {
            workspace_id: WorkspaceId(1),
            buffer_id: BufferId(2),
            file_id: FileId(3),
            transaction_id,
            correlation_id: CorrelationId(4),
            source: devil_protocol::TransactionSource::User,
            pre_snapshot_id: devil_protocol::SnapshotId(5),
            post_snapshot_id: devil_protocol::SnapshotId(6),
            pre_buffer_version: devil_protocol::BufferVersion(1),
            post_buffer_version: devil_protocol::BufferVersion(2),
            changed_ranges: Vec::new(),
            causality_id: causality,
            parent_transaction_id: None,
            schema_version: 1,
            undo_group_id: None,
            occurred_at: TimestampMillis(7),
        };

        let applied = transaction_event(&descriptor, true, None, EventSequence(1));
        let failed = transaction_event(&descriptor, false, Some("invalid range"), EventSequence(2));
        let overflow = watcher_recovery_event(
            WorkspaceId(1),
            CorrelationId(4),
            causality,
            EventSequence(3),
            false,
        );
        let recovery = watcher_recovery_event(
            WorkspaceId(1),
            CorrelationId(4),
            causality,
            EventSequence(4),
            true,
        );
        let escape = path_escape_denied_event(
            WorkspaceId(1),
            CorrelationId(4),
            causality,
            EventSequence(5),
            "../secret.txt",
        );

        assert_eq!(applied.event, "editor.transaction_applied");
        assert_eq!(failed.event, "editor.transaction_failed");
        assert_eq!(overflow.event, "workspace.watcher_overflow");
        assert_eq!(recovery.event, "workspace.watcher_recovery");
        assert_eq!(escape.event, "workspace.path_escape_denied");
        assert_eq!(failed.payload["reason_len"], 13);
    }

    #[test]
    fn proposal_lifecycle_helpers_are_metadata_only_and_orderable() {
        let proposal = save_proposal();
        let created_causality = CausalityId(Uuid::now_v7());
        let created = proposal_created_event(&proposal, created_causality, EventSequence(1));
        let validated_transition = transition(ProposalLifecycleState::Validated);
        let validated =
            proposal_validated_event(&proposal, &validated_transition, EventSequence(2));
        let previewed_transition = transition(ProposalLifecycleState::Previewed);
        let previewed =
            proposal_previewed_event(&proposal, &previewed_transition, EventSequence(3));
        let applied_transition = transition(ProposalLifecycleState::Applied);
        let applied = proposal_applied_event(&proposal, &applied_transition, EventSequence(4));

        let events = [created, validated, previewed, applied];
        let names = events
            .iter()
            .map(|event| event.event.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            names,
            vec![
                "proposal.created",
                "proposal.validated",
                "proposal.previewed",
                "proposal.applied"
            ]
        );
        for (idx, event) in events.iter().enumerate() {
            assert_ne!(event.causality_id.0, Uuid::nil());
            assert_ne!(event.correlation_id.0, 0);
            assert_eq!(event.sequence.0, idx as u64 + 1);
            assert_eq!(event.redaction, RedactionHint::MetadataOnly);
            assert_ne!(event.payload.to_string(), "/secret/source.rs");
        }
    }

    #[test]
    fn proposal_failure_conflict_fallback_retention_and_security_helpers_redact_content() {
        let proposal = save_proposal();
        let failed_transition = transition(ProposalLifecycleState::Failed);
        let failed = proposal_failed_event(
            &proposal,
            &failed_transition,
            ProposalFailureReason::ApplyFailed,
            EventSequence(5),
        );
        let rejected_transition = transition(ProposalLifecycleState::Rejected);
        let rejected = proposal_rejected_event(
            &proposal,
            &rejected_transition,
            ProposalRejectionReason::ValidationFailed,
            EventSequence(6),
        );
        let rolled_back_transition = transition(ProposalLifecycleState::RolledBack);
        let rolled_back = proposal_rolled_back_event(
            &proposal,
            &rolled_back_transition,
            ProposalRollbackReason::ApplyFailed,
            EventSequence(7),
        );
        let stale = stale_proposal_rejected_event(
            WorkspaceId(1),
            FileId(3),
            CorrelationId(7),
            CausalityId(Uuid::now_v7()),
            EventSequence(8),
            proposal.proposal_id,
            ProposalStaleReason::FingerprintMismatch,
        );
        let denial = security_denial_event(
            WorkspaceId(1),
            Some(FileId(3)),
            Some(PrincipalId("tester".to_string())),
            &CapabilityId("fs.write".to_string()),
            CorrelationId(7),
            CausalityId(Uuid::now_v7()),
            EventSequence(9),
            Some("/secret/source.rs"),
            "denied because /secret/source.rs is blocked",
        );
        let fallback = fallback_denied_event(
            WorkspaceId(1),
            FileId(3),
            CorrelationId(7),
            CausalityId(Uuid::now_v7()),
            EventSequence(10),
            "non-atomic fallback disabled; raw path /secret/source.rs",
        );
        let retention = editor_retention_degradation_event(
            WorkspaceId(1),
            BufferId(2),
            CorrelationId(7),
            CausalityId(Uuid::now_v7()),
            EventSequence(11),
            4,
            2,
            512,
            "evicted undo snapshot with source text",
        );

        for event in [
            failed,
            rejected,
            rolled_back,
            stale,
            denial,
            fallback,
            retention,
        ] {
            assert_ne!(event.correlation_id.0, 0);
            assert_ne!(event.causality_id.0, Uuid::nil());
            assert_ne!(event.sequence.0, 0);
            assert_eq!(event.redaction, RedactionHint::MetadataOnly);
            assert!(!event.payload.to_string().contains("/secret/source.rs"));
            assert!(!event.payload.to_string().contains("source text"));
        }
    }

    #[test]
    fn proposal_audit_and_event_metadata_records_are_redacted() {
        let proposal = save_proposal();
        let transition = transition(ProposalLifecycleState::Applied);
        let audit = proposal_audit_record(&proposal, &transition);
        assert_eq!(audit.proposal_id, proposal.proposal_id);
        assert_eq!(audit.lifecycle_state, ProposalLifecycleState::Applied);
        assert_eq!(audit.redaction_hints, vec![RedactionHint::MetadataOnly]);
        assert!(!format!("{audit:?}").contains("/secret/source.rs"));

        let event = proposal_applied_event(&proposal, &transition, EventSequence(12));
        let metadata = event_metadata_record(&event);
        assert_eq!(metadata.event_id, event.event_id);
        assert_eq!(metadata.redaction, RedactionHint::MetadataOnly);
        assert_eq!(metadata.sequence, EventSequence(12));

        let audit_event = proposal_audit_recorded_event(&proposal, &transition, EventSequence(13));
        assert_eq!(audit_event.event, "proposal.audit_recorded");
        assert_eq!(audit_event.redaction, RedactionHint::MetadataOnly);
        assert_eq!(audit_event.sequence, EventSequence(13));
        assert_ne!(audit_event.correlation_id.0, 0);
        assert_ne!(audit_event.causality_id.0, Uuid::nil());
        assert!(
            !audit_event
                .payload
                .to_string()
                .contains("/secret/source.rs")
        );
    }

    #[test]
    fn assisted_ai_audit_event_is_metadata_only_and_no_invocation() {
        let record = assisted_ai_audit_fixture();
        let event = assisted_ai_audit_recorded_event(&record).expect("audit event validates");
        assert_eq!(event.event, "assisted_ai.audit_recorded");
        assert_eq!(event.correlation_id, CorrelationId(901));
        assert_eq!(event.sequence, EventSequence(77));
        assert_eq!(event.redaction, RedactionHint::MetadataOnly);
        assert_eq!(event.retention, RetentionLabel::Audit);
        assert_eq!(event.payload["proposal_id"], 701);
        assert_eq!(event.payload["runtime_invocation_state"], "NotEncoded");
        assert_eq!(event.payload["runtime_activation_label_count"], 4);

        let sink = InMemoryEventSink::new();
        sink.try_emit(EventSinkRequest { envelope: event })
            .expect("metadata-only assisted AI event stores");
        let stored = sink.events().expect("stored events");
        let serialized =
            serde_json::to_string(&stored).expect("serialize stored assisted AI event");
        assert!(serialized.contains("assisted_ai.audit_recorded"));
        assert!(serialized.contains("NotEncoded"));
        assert!(!serialized.contains("raw prompt"));
        assert!(!serialized.contains("source_body"));
        assert!(!serialized.contains("provider_payload"));
        assert!(!serialized.contains("terminal output"));
        assert!(!serialized.contains("ChatCompletionRequest"));
        assert!(!serialized.contains("network_request"));
        assert!(!serialized.contains("tool_call"));
        assert!(!serialized.contains("runtime_started"));
    }

    #[test]
    fn assisted_ai_audit_event_rejects_invalid_core_ids_and_raw_markers() {
        let mut zero_sequence = assisted_ai_audit_fixture();
        zero_sequence.event_sequence = EventSequence(0);
        assert!(matches!(
            assisted_ai_audit_recorded_event(&zero_sequence),
            Err(AssistedAiContractError::ZeroEventSequence)
        ));

        let mut raw_marker = assisted_ai_audit_fixture();
        raw_marker.refusal_error_category = Some("provider_payload raw prompt".to_string());
        assert!(matches!(
            assisted_ai_audit_recorded_event(&raw_marker),
            Err(AssistedAiContractError::NonMetadataOnlyAuditRecord { .. })
        ));
    }

    #[test]
    fn phase4_runtime_and_replay_events_are_metadata_only() {
        let run_id = devil_protocol::AgentRunId("phase4-run-observe".to_string());
        let audit = Phase4RuntimeAuditRecord {
            audit_id: "phase4-audit-observe".to_string(),
            run_id: Some(run_id.clone()),
            step_id: None,
            provider_route_id: Some("route-observe".to_string()),
            invocation_state: AssistedAiProviderInvocationState::Completed,
            outcome_label: "phase4.provider.completed".to_string(),
            labels: vec!["metadata-only".to_string()],
            correlation_id: CorrelationId(77),
            causality_id: CausalityId(Uuid::now_v7()),
            event_sequence: EventSequence(88),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        };
        let event = phase4_runtime_audit_recorded_event(&audit).expect("phase4 event");
        assert_eq!(event.event, "phase4.runtime_audit_recorded");
        assert_eq!(event.payload["invocation_state"], "Completed");

        let manifest = AgentReplayManifest {
            run_id,
            transitions: Vec::new(),
            context_manifests: Vec::new(),
            provider_route_ids: vec!["route-observe".to_string()],
            proposal_ids: vec![devil_protocol::ProposalId(9)],
            correlation_id: CorrelationId(77),
            causality_id: CausalityId(Uuid::now_v7()),
            event_sequence: EventSequence(89),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        };
        let replay_event = agent_replay_manifest_recorded_event(&manifest).expect("replay event");
        assert_eq!(replay_event.event, "phase4.agent_replay_manifest_recorded");
        assert_eq!(replay_event.payload["provider_route_count"], 1);

        let mut raw_marker = audit;
        raw_marker
            .labels
            .push("provider_payload raw prompt".to_string());
        assert!(matches!(
            phase4_runtime_audit_recorded_event(&raw_marker),
            Err(AssistedAiContractError::NonMetadataOnlyAuditRecord { .. })
        ));
    }
}
