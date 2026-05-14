//! Tracing, metrics, event log, and performance counters.

#![warn(missing_docs)]

use std::sync::{Arc, Mutex};

use devil_protocol::{
    BufferId, CausalityId, CorrelationId, EventEnvelope, EventId, EventSequence, EventSeverity,
    EventSinkPort, EventSinkRequest, FileId, PrincipalId, ProtocolError, ProtocolResult,
    RedactionHint, RetentionLabel, TextTransactionDescriptor, TimestampMillis, WorkspaceId,
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
            correlation_id: CorrelationId(0),
            principal_id: None,
            sequence: EventSequence(0),
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

/// Build an envelope-ready metadata DTO for a denied save.
pub fn save_denied_event(
    workspace_id: WorkspaceId,
    file_id: FileId,
    causality_id: CausalityId,
    reason: impl Into<String>,
) -> EventEnvelope {
    EventEnvelopeBuilder::new("workspace.save_denied", causality_id)
        .workspace_id(workspace_id)
        .file_id(file_id)
        .severity(EventSeverity::Warning)
        .retention(RetentionLabel::Audit)
        .metadata("reason", reason.into())
        .build()
}

/// Build an envelope-ready metadata DTO for denied path escape attempts.
pub fn path_escape_denied_event(
    workspace_id: WorkspaceId,
    causality_id: CausalityId,
    path: impl Into<String>,
) -> EventEnvelope {
    EventEnvelopeBuilder::new("workspace.path_escape_denied", causality_id)
        .workspace_id(workspace_id)
        .severity(EventSeverity::Warning)
        .retention(RetentionLabel::Audit)
        .metadata("path", path.into())
        .build()
}

/// Build an envelope-ready metadata DTO for editor transaction outcomes.
pub fn transaction_event(
    descriptor: &TextTransactionDescriptor,
    applied: bool,
    reason: Option<&str>,
) -> EventEnvelope {
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
        .severity(if applied {
            EventSeverity::Info
        } else {
            EventSeverity::Error
        })
        .retention(RetentionLabel::Warm)
        .metadata("transaction_id", descriptor.transaction_id.to_string())
        .metadata("schema_version", json!(descriptor.schema_version))
        .metadata("changed_ranges", json!(descriptor.changed_ranges.len()));

    if let Some(reason) = reason {
        builder = builder.metadata("reason", reason.to_string());
    }

    builder.build()
}

/// Build an envelope-ready metadata DTO for watcher overflow or recovery.
pub fn watcher_recovery_event(
    workspace_id: WorkspaceId,
    causality_id: CausalityId,
    recovered: bool,
) -> EventEnvelope {
    EventEnvelopeBuilder::new(
        if recovered {
            "workspace.watcher_recovery"
        } else {
            "workspace.watcher_overflow"
        },
        causality_id,
    )
    .workspace_id(workspace_id)
    .severity(if recovered {
        EventSeverity::Info
    } else {
        EventSeverity::Warning
    })
    .retention(RetentionLabel::Warm)
    .metadata("recovered", recovered)
    .build()
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
            causality,
            "untrusted workspace",
        );

        assert_eq!(envelope.schema_version, 1);
        assert_eq!(envelope.causality_id, causality);
        assert_eq!(envelope.workspace_id, Some(WorkspaceId(10)));
        assert_eq!(envelope.retention, RetentionLabel::Audit);
        assert_eq!(envelope.redaction, RedactionHint::MetadataOnly);
        assert_eq!(envelope.payload["reason"], "untrusted workspace");
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

        let applied = transaction_event(&descriptor, true, None);
        let failed = transaction_event(&descriptor, false, Some("invalid range"));
        let overflow = watcher_recovery_event(WorkspaceId(1), causality, false);
        let recovery = watcher_recovery_event(WorkspaceId(1), causality, true);
        let escape = path_escape_denied_event(WorkspaceId(1), causality, "../secret.txt");

        assert_eq!(applied.event, "editor.transaction_applied");
        assert_eq!(failed.event, "editor.transaction_failed");
        assert_eq!(overflow.event, "workspace.watcher_overflow");
        assert_eq!(recovery.event, "workspace.watcher_recovery");
        assert_eq!(escape.event, "workspace.path_escape_denied");
        assert_eq!(failed.payload["reason"], "invalid range");
    }
}
