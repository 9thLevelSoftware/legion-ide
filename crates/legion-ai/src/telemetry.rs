//! AI-side telemetry helpers for suggestion latency and acceptance.

use legion_observability::telemetry::{
    SuggestionTelemetryDetailLevel, suggestion_telemetry_spool_record as build_spool_record,
};
use legion_protocol::{
    HostedTelemetrySpoolRecord, InlinePredictionLifecycleCommand, InlinePredictionResult,
    WorkspaceId, validate_inline_prediction_lifecycle_command, validate_inline_prediction_result,
};
use legion_security::HostedTelemetryPolicy;

/// Build a metadata-only hosted telemetry spool record for an inline prediction suggestion.
///
/// Returns `None` when the policy or consent gate blocks suggestion telemetry.
pub fn suggestion_telemetry_spool_record(
    lifecycle: &InlinePredictionLifecycleCommand,
    result: &InlinePredictionResult,
    workspace_id: WorkspaceId,
    policy: &HostedTelemetryPolicy,
    consent_current: bool,
    rich_fields_opt_in: bool,
) -> Option<HostedTelemetrySpoolRecord> {
    validate_inline_prediction_lifecycle_command(lifecycle).ok()?;
    validate_inline_prediction_result(result).ok()?;

    if !policy.spool_write_enabled {
        return None;
    }
    if policy.require_explicit_consent && !consent_current {
        return None;
    }

    let detail_level = if rich_fields_opt_in && consent_current && policy.export_enabled {
        SuggestionTelemetryDetailLevel::Rich
    } else {
        SuggestionTelemetryDetailLevel::MetadataOnly
    };

    build_spool_record(
        format!(
            "inline:suggestion:{}:{}",
            lifecycle.request_id.0, lifecycle.event_sequence.0
        ),
        workspace_id,
        lifecycle.correlation_id,
        lifecycle.causality_id,
        lifecycle.event_sequence,
        lifecycle.action,
        lifecycle.request_id.0.clone(),
        lifecycle.result_id.clone(),
        lifecycle.acceptance_id,
        lifecycle.dismissal_id,
        result.provider.latency.total_ms,
        Some(&result.provider),
        detail_level,
    )
    .ok()
}

/// Suggestion telemetry helper that also tolerates a missing policy gate.
pub fn suggestion_telemetry_recorded_event(
    lifecycle: &InlinePredictionLifecycleCommand,
    result: &InlinePredictionResult,
    workspace_id: WorkspaceId,
    policy: &HostedTelemetryPolicy,
    consent_current: bool,
    rich_fields_opt_in: bool,
) -> Option<legion_protocol::EventEnvelope> {
    let record = suggestion_telemetry_spool_record(
        lifecycle,
        result,
        workspace_id,
        policy,
        consent_current,
        rich_fields_opt_in,
    )?;
    legion_observability::telemetry::suggestion_telemetry_recorded_event(&record).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use legion_protocol::{
        AssistedAiOperationClass, AssistedAiProviderClass, AssistedAiProviderInvocationState,
        BufferVersion, CausalityId, CorrelationId, EventSequence, FileContentVersion,
        FileFingerprint, InlinePredictionAcceptanceId, InlinePredictionFreshness,
        InlinePredictionFreshnessState, InlinePredictionGhostText, InlinePredictionLatencyMetadata,
        InlinePredictionLifecycleAction, InlinePredictionProviderMetadata,
        InlinePredictionRequestId, InlinePredictionResultId, InlinePredictionResultState,
        InlinePredictionRetention, ProtocolTextRange, RedactionHint, SnapshotId, TimestampMillis,
        WorkspaceGeneration, validate_inline_prediction_result,
    };
    use uuid::Uuid;

    fn sample_result() -> InlinePredictionResult {
        let metadata = InlinePredictionProviderMetadata {
            provider_id: "deterministic-inline".to_string(),
            model_label: "zeta2-style-deterministic".to_string(),
            provider_class: AssistedAiProviderClass::Local,
            operation_class: AssistedAiOperationClass::InlinePrediction,
            invocation_state: AssistedAiProviderInvocationState::Completed,
            latency: InlinePredictionLatencyMetadata {
                queued_ms: 4,
                inference_ms: 12,
                total_ms: 16,
                timed_out: false,
            },
            health_labels: vec!["deterministic".to_string()],
            cost_labels: vec!["local".to_string()],
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        };
        let result = InlinePredictionResult {
            result_id: InlinePredictionResultId("inline:result:1".to_string()),
            request_id: InlinePredictionRequestId("inline:req:1".to_string()),
            state: InlinePredictionResultState::Available,
            retention: InlinePredictionRetention::EphemeralDisplay,
            insert_range: ProtocolTextRange {
                start: legion_protocol::TextCoordinate {
                    line: 3,
                    character: 5,
                    byte_offset: Some(42),
                    utf16_offset: Some(42),
                },
                end: legion_protocol::TextCoordinate {
                    line: 3,
                    character: 5,
                    byte_offset: Some(42),
                    utf16_offset: Some(42),
                },
            },
            ghost_text: Some(InlinePredictionGhostText {
                text: "fn answer() -> u32 { 42 }".to_string(),
                byte_len: 25,
                line_count: 1,
                text_fingerprint: FileFingerprint {
                    algorithm: "deterministic-inline-v1".to_string(),
                    value: "rust:3:25".to_string(),
                },
            }),
            fingerprint: legion_protocol::InlinePredictionFingerprintMetadata {
                snapshot_id: SnapshotId(22),
                buffer_version: BufferVersion(11),
                file_content_version: Some(FileContentVersion(33)),
                workspace_generation: WorkspaceGeneration(44),
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
            generated_at: TimestampMillis(2000),
            expires_at: None,
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        };
        validate_inline_prediction_result(&result).expect("valid result");
        result
    }

    fn sample_lifecycle(
        action: InlinePredictionLifecycleAction,
    ) -> InlinePredictionLifecycleCommand {
        InlinePredictionLifecycleCommand {
            request_id: InlinePredictionRequestId("inline:req:1".to_string()),
            result_id: Some(InlinePredictionResultId("inline:result:1".to_string())),
            action,
            cancellation_token: Some(legion_protocol::CancellationTokenId(
                Uuid::parse_str("66666666-6666-6666-6666-666666666666").unwrap(),
            )),
            dismissal_id: Some(legion_protocol::InlinePredictionDismissalId(
                Uuid::parse_str("77777777-7777-7777-7777-777777777777").unwrap(),
            )),
            acceptance_id: Some(InlinePredictionAcceptanceId(
                Uuid::parse_str("88888888-8888-8888-8888-888888888888").unwrap(),
            )),
            reason_labels: vec!["user.accepted".to_string()],
            requested_at: TimestampMillis(2001),
            correlation_id: CorrelationId(7),
            causality_id: CausalityId(
                Uuid::parse_str("99999999-9999-9999-9999-999999999999").unwrap(),
            ),
            event_sequence: EventSequence(14),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        }
    }

    #[test]
    fn suggestion_telemetry_is_blocked_without_current_consent() {
        let policy = HostedTelemetryPolicy {
            export_enabled: true,
            spool_write_enabled: true,
            require_explicit_consent: true,
            allowed_capabilities: Default::default(),
        };

        let record = suggestion_telemetry_spool_record(
            &sample_lifecycle(InlinePredictionLifecycleAction::Accept),
            &sample_result(),
            WorkspaceId(5),
            &policy,
            false,
            false,
        );

        assert!(record.is_none());
    }

    #[test]
    fn suggestion_telemetry_defaults_to_metadata_only() {
        let policy = HostedTelemetryPolicy {
            export_enabled: true,
            spool_write_enabled: true,
            require_explicit_consent: true,
            allowed_capabilities: Default::default(),
        };

        let record = suggestion_telemetry_spool_record(
            &sample_lifecycle(InlinePredictionLifecycleAction::Accept),
            &sample_result(),
            WorkspaceId(5),
            &policy,
            true,
            false,
        )
        .expect("metadata-only record");

        assert_eq!(
            record.category,
            legion_protocol::HostedTelemetryCategory::NextEditPrediction
        );
        assert!(record.metadata_summary.contains("action=accept"));
        assert!(record.metadata_summary.contains("latency_ms=16"));
        assert!(record.metadata_summary.contains("accepted=true"));
        assert!(!record.metadata_summary.contains("provider_id="));
    }

    #[test]
    fn suggestion_telemetry_rich_fields_are_opt_in() {
        let policy = HostedTelemetryPolicy {
            export_enabled: true,
            spool_write_enabled: true,
            require_explicit_consent: true,
            allowed_capabilities: Default::default(),
        };

        let record = suggestion_telemetry_spool_record(
            &sample_lifecycle(InlinePredictionLifecycleAction::Dismiss),
            &sample_result(),
            WorkspaceId(5),
            &policy,
            true,
            true,
        )
        .expect("rich record");

        assert!(
            record
                .metadata_summary
                .contains("provider_id=deterministic-inline")
        );
        assert!(
            record
                .metadata_summary
                .contains("latency_breakdown=queued:4 inference:12 total:16 timed_out:false")
        );
        assert!(
            record
                .metadata_summary
                .contains("health_labels=deterministic")
        );
    }
}
