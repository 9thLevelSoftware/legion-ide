//! Smoke tests: verify the telemetry module is re-exported from the crate root
//! and that the consent-gated default-deny path is accessible externally.

use legion_ai::telemetry::{
    suggestion_telemetry_recorded_event, suggestion_telemetry_spool_record,
};
use legion_protocol::{
    AssistedAiOperationClass, AssistedAiProviderClass, AssistedAiProviderInvocationState,
    BufferVersion, CausalityId, CorrelationId, EventSequence, FileContentVersion, FileFingerprint,
    InlinePredictionAcceptanceId, InlinePredictionFreshness, InlinePredictionFreshnessState,
    InlinePredictionGhostText, InlinePredictionLatencyMetadata, InlinePredictionLifecycleAction,
    InlinePredictionLifecycleCommand, InlinePredictionProviderMetadata, InlinePredictionRequestId,
    InlinePredictionResult, InlinePredictionResultId, InlinePredictionResultState,
    InlinePredictionRetention, ProtocolTextRange, RedactionHint, SnapshotId, TextCoordinate,
    TimestampMillis, WorkspaceGeneration, WorkspaceId, validate_inline_prediction_result,
};
use legion_security::HostedTelemetryPolicy;
use uuid::Uuid;

fn sample_result() -> InlinePredictionResult {
    let metadata = InlinePredictionProviderMetadata {
        provider_id: "smoke-provider".to_string(),
        model_label: "smoke-model".to_string(),
        provider_class: AssistedAiProviderClass::Local,
        operation_class: AssistedAiOperationClass::InlinePrediction,
        invocation_state: AssistedAiProviderInvocationState::Completed,
        latency: InlinePredictionLatencyMetadata {
            queued_ms: 1,
            inference_ms: 5,
            total_ms: 6,
            timed_out: false,
        },
        health_labels: vec!["smoke".to_string()],
        cost_labels: vec!["local".to_string()],
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    };
    let coord = TextCoordinate {
        line: 0,
        character: 0,
        byte_offset: Some(0),
        utf16_offset: Some(0),
    };
    let result = InlinePredictionResult {
        result_id: InlinePredictionResultId("smoke:result:1".to_string()),
        request_id: InlinePredictionRequestId("smoke:req:1".to_string()),
        state: InlinePredictionResultState::Available,
        retention: InlinePredictionRetention::EphemeralDisplay,
        insert_range: ProtocolTextRange {
            start: coord,
            end: coord,
        },
        ghost_text: Some(InlinePredictionGhostText {
            text: "fn smoke() {}".to_string(),
            byte_len: 13,
            line_count: 1,
            text_fingerprint: FileFingerprint {
                algorithm: "smoke-v1".to_string(),
                value: "rust:0:13".to_string(),
            },
        }),
        fingerprint: legion_protocol::InlinePredictionFingerprintMetadata {
            snapshot_id: SnapshotId(1),
            buffer_version: BufferVersion(1),
            file_content_version: Some(FileContentVersion(1)),
            workspace_generation: WorkspaceGeneration(1),
            content_fingerprint: Some(FileFingerprint {
                algorithm: "sha256".to_string(),
                value: "content".to_string(),
            }),
            context_fingerprint: FileFingerprint {
                algorithm: "sha256".to_string(),
                value: "context".to_string(),
            },
            schema_version: 1,
        },
        freshness: InlinePredictionFreshness {
            state: InlinePredictionFreshnessState::Fresh,
            stale_reasons: Vec::new(),
            schema_version: 1,
        },
        provider: metadata,
        refusal: None,
        generated_at: TimestampMillis(1000),
        expires_at: None,
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    };
    validate_inline_prediction_result(&result).expect("valid smoke result");
    result
}

fn sample_lifecycle() -> InlinePredictionLifecycleCommand {
    InlinePredictionLifecycleCommand {
        request_id: InlinePredictionRequestId("smoke:req:1".to_string()),
        result_id: Some(InlinePredictionResultId("smoke:result:1".to_string())),
        action: InlinePredictionLifecycleAction::Accept,
        cancellation_token: None,
        dismissal_id: None,
        acceptance_id: Some(InlinePredictionAcceptanceId(
            Uuid::parse_str("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa").unwrap(),
        )),
        reason_labels: vec!["user.accepted".to_string()],
        requested_at: TimestampMillis(1001),
        correlation_id: CorrelationId(3),
        causality_id: CausalityId(Uuid::parse_str("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb").unwrap()),
        event_sequence: EventSequence(1),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    }
}

/// Verify the telemetry module API is accessible from `legion_ai::telemetry` and that
/// the consent gate blocks spool writes when `require_explicit_consent` is unmet.
#[test]
fn telemetry_module_is_accessible() {
    let policy_no_consent = HostedTelemetryPolicy {
        export_enabled: true,
        spool_write_enabled: true,
        require_explicit_consent: true,
        allowed_capabilities: Default::default(),
    };

    // Default-deny: require_explicit_consent=true and consent_current=false → None
    let blocked = suggestion_telemetry_spool_record(
        &sample_lifecycle(),
        &sample_result(),
        WorkspaceId(1),
        &policy_no_consent,
        false,
        false,
    );
    assert!(
        blocked.is_none(),
        "spool record must be blocked when consent is not current"
    );

    // Consent given: record is produced and recorded_event succeeds
    let policy_with_consent = HostedTelemetryPolicy {
        export_enabled: true,
        spool_write_enabled: true,
        require_explicit_consent: true,
        allowed_capabilities: Default::default(),
    };
    let record = suggestion_telemetry_spool_record(
        &sample_lifecycle(),
        &sample_result(),
        WorkspaceId(1),
        &policy_with_consent,
        true,
        false,
    )
    .expect("spool record is produced when consent is current");
    assert!(record.metadata_summary.contains("action=accept"));

    let envelope = suggestion_telemetry_recorded_event(
        &sample_lifecycle(),
        &sample_result(),
        WorkspaceId(1),
        &policy_with_consent,
        true,
        false,
    )
    .expect("recorded_event produces an envelope when consent is current");
    assert_eq!(envelope.event, "telemetry.spool_recorded");
    assert_eq!(envelope.redaction, RedactionHint::MetadataOnly);
}
