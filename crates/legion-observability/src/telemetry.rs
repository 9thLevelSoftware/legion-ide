//! Hosted telemetry helpers for metadata-only suggestion latency and acceptance samples.

use crate::{
    EventEnvelopeBuilder, EventSinkConfig, ObservabilityError,
    hosted_telemetry_spool_recorded_event, validate_envelope,
};
use legion_protocol::{
    CausalityId, CorrelationId, EventEnvelope, EventSequence, HostedTelemetryCategory,
    HostedTelemetrySpoolRecord, InlinePredictionAcceptanceId, InlinePredictionDismissalId,
    InlinePredictionLifecycleAction, InlinePredictionProviderMetadata, InlinePredictionResultId,
    PrincipalId, PrivacyClassification, RedactionHint, RetentionLabel, WorkbenchTelemetryConsent,
    WorkspaceId, validate_hosted_telemetry_spool_record,
};

/// Detail level requested for suggestion telemetry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SuggestionTelemetryDetailLevel {
    /// Emit only metadata-safe latency and lifecycle summary fields.
    MetadataOnly,
    /// Include richer metadata-safe provider and latency breakdown fields.
    Rich,
}

/// Build an audit event for the first-run crash-report consent choice.
pub fn crash_reporting_consent_recorded_event(
    consent: &WorkbenchTelemetryConsent,
    correlation_id: CorrelationId,
    causality_id: CausalityId,
    event_sequence: EventSequence,
    workspace_id: Option<WorkspaceId>,
    principal_id: Option<&PrincipalId>,
) -> Result<EventEnvelope, ObservabilityError> {
    let choice = if consent.crash_reports_enabled {
        "opt_in"
    } else {
        "opt_out"
    };
    let mut builder = EventEnvelopeBuilder::new(
        "telemetry.crash_reporting_consent_recorded",
        causality_id,
    )
    .retention(RetentionLabel::Audit)
    .redaction(RedactionHint::MetadataOnly)
    .correlation_id(correlation_id)
    .sequence(event_sequence)
    .metadata("choice", choice)
    .metadata("telemetry_enabled", consent.enabled)
    .metadata("crash_reports_enabled", consent.crash_reports_enabled)
    .metadata("raw_source_allowed", consent.raw_source_allowed)
    .metadata("consent_label", consent.consent_label.clone())
    .metadata(
        "metadata_summary",
        format!(
            "choice={choice} crash_reports_enabled={} telemetry_enabled={} consent_label={}",
            consent.crash_reports_enabled, consent.enabled, consent.consent_label
        ),
    );
    if let Some(workspace_id) = workspace_id {
        builder = builder.workspace_id(workspace_id);
    }
    if let Some(principal_id) = principal_id {
        builder = builder.principal_id(principal_id.clone());
    }
    let envelope = builder.build();
    validate_envelope(&envelope, EventSinkConfig::default())?;
    Ok(envelope)
}

/// Build a metadata-only hosted telemetry spool record for a suggestion latency or acceptance sample.
#[allow(clippy::too_many_arguments)]
pub fn suggestion_telemetry_spool_record(
    record_id: impl Into<String>,
    workspace_id: WorkspaceId,
    correlation_id: CorrelationId,
    causality_id: CausalityId,
    event_sequence: EventSequence,
    lifecycle_action: InlinePredictionLifecycleAction,
    request_id: impl Into<String>,
    result_id: Option<InlinePredictionResultId>,
    acceptance_id: Option<InlinePredictionAcceptanceId>,
    dismissal_id: Option<InlinePredictionDismissalId>,
    latency_ms: u32,
    provider: Option<&InlinePredictionProviderMetadata>,
    detail_level: SuggestionTelemetryDetailLevel,
) -> Result<HostedTelemetrySpoolRecord, ObservabilityError> {
    let request_id = request_id.into();
    let metadata_summary = suggestion_metadata_summary(
        &request_id,
        result_id.as_ref(),
        acceptance_id,
        dismissal_id,
        lifecycle_action,
        latency_ms,
        provider,
        detail_level,
    );

    let record = HostedTelemetrySpoolRecord {
        record_id: record_id.into(),
        workspace_id,
        category: HostedTelemetryCategory::NextEditPrediction,
        classification: PrivacyClassification::Metadata,
        metadata_summary,
        event_sequence,
        correlation_id,
        causality_id,
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    };
    validate_hosted_telemetry_spool_record(&record)
        .map_err(|_| ObservabilityError::InvalidPayload)?;
    Ok(record)
}

/// Build an envelope for a suggestion telemetry spool record.
pub fn suggestion_telemetry_recorded_event(
    record: &HostedTelemetrySpoolRecord,
) -> Result<EventEnvelope, ObservabilityError> {
    hosted_telemetry_spool_recorded_event(record)
}

#[allow(clippy::too_many_arguments)]
fn suggestion_metadata_summary(
    request_id: &str,
    result_id: Option<&InlinePredictionResultId>,
    acceptance_id: Option<InlinePredictionAcceptanceId>,
    dismissal_id: Option<InlinePredictionDismissalId>,
    lifecycle_action: InlinePredictionLifecycleAction,
    latency_ms: u32,
    provider: Option<&InlinePredictionProviderMetadata>,
    detail_level: SuggestionTelemetryDetailLevel,
) -> String {
    let mut fields = vec![
        format!("request_id={request_id}"),
        format!("action={}", lifecycle_action_label(lifecycle_action)),
        format!("latency_ms={latency_ms}"),
    ];
    if let Some(result_id) = result_id {
        fields.push(format!("result_id={}", result_id.0));
    }
    if let Some(acceptance_id) = acceptance_id {
        fields.push(format!("acceptance_id={}", acceptance_id.0));
    }
    if let Some(dismissal_id) = dismissal_id {
        fields.push(format!("dismissal_id={}", dismissal_id.0));
    }
    fields.push(format!(
        "accepted={}",
        matches!(lifecycle_action, InlinePredictionLifecycleAction::Accept)
    ));

    if matches!(detail_level, SuggestionTelemetryDetailLevel::Rich)
        && let Some(provider) = provider
    {
        fields.push(format!("provider_id={}", provider.provider_id));
        fields.push(format!("model_label={}", provider.model_label));
        fields.push(format!("provider_class={:?}", provider.provider_class));
        fields.push(format!("invocation_state={:?}", provider.invocation_state));
        fields.push(format!(
            "latency_breakdown=queued:{} inference:{} total:{} timed_out:{}",
            provider.latency.queued_ms,
            provider.latency.inference_ms,
            provider.latency.total_ms,
            provider.latency.timed_out,
        ));
        if !provider.health_labels.is_empty() {
            fields.push(format!(
                "health_labels={}",
                provider.health_labels.join(",")
            ));
        }
        if !provider.cost_labels.is_empty() {
            fields.push(format!("cost_labels={}", provider.cost_labels.join(",")));
        }
    }

    fields.push("payload_class=metadata_only".to_string());
    fields.join(" ")
}

fn lifecycle_action_label(action: InlinePredictionLifecycleAction) -> &'static str {
    match action {
        InlinePredictionLifecycleAction::Cancel => "cancel",
        InlinePredictionLifecycleAction::Dismiss => "dismiss",
        InlinePredictionLifecycleAction::Accept => "accept",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use legion_protocol::{
        AssistedAiProviderClass, AssistedAiProviderInvocationState, InlinePredictionLatencyMetadata,
    };
    use serde_json::json;
    use uuid::Uuid;

    fn provider_metadata() -> InlinePredictionProviderMetadata {
        InlinePredictionProviderMetadata {
            provider_id: "deterministic-inline".to_string(),
            model_label: "zeta2-style-deterministic".to_string(),
            provider_class: AssistedAiProviderClass::Local,
            operation_class: legion_protocol::AssistedAiOperationClass::InlinePrediction,
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
        }
    }

    #[test]
    fn suggestion_telemetry_defaults_to_metadata_only() {
        let record = suggestion_telemetry_spool_record(
            "inline:suggestion:1",
            WorkspaceId(7),
            CorrelationId(9),
            CausalityId(Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap()),
            EventSequence(11),
            InlinePredictionLifecycleAction::Accept,
            "inline:req:1",
            None,
            Some(InlinePredictionAcceptanceId(
                Uuid::parse_str("22222222-2222-2222-2222-222222222222").unwrap(),
            )),
            None,
            16,
            Some(&provider_metadata()),
            SuggestionTelemetryDetailLevel::MetadataOnly,
        )
        .expect("metadata-only record");

        assert_eq!(record.category, HostedTelemetryCategory::NextEditPrediction);
        assert_eq!(record.classification, PrivacyClassification::Metadata);
        assert_eq!(record.redaction_hints, vec![RedactionHint::MetadataOnly]);
        assert!(record.metadata_summary.contains("action=accept"));
        assert!(record.metadata_summary.contains("latency_ms=16"));
        assert!(record.metadata_summary.contains("accepted=true"));
        assert!(!record.metadata_summary.contains("provider_id="));
        assert!(!record.metadata_summary.contains("latency_breakdown="));
    }

    #[test]
    fn suggestion_telemetry_rich_fields_include_provider_metadata() {
        let record = suggestion_telemetry_spool_record(
            "inline:suggestion:2",
            WorkspaceId(7),
            CorrelationId(9),
            CausalityId(Uuid::parse_str("33333333-3333-3333-3333-333333333333").unwrap()),
            EventSequence(12),
            InlinePredictionLifecycleAction::Dismiss,
            "inline:req:2",
            Some(InlinePredictionResultId("inline:result:2".to_string())),
            None,
            Some(InlinePredictionDismissalId(
                Uuid::parse_str("44444444-4444-4444-4444-444444444444").unwrap(),
            )),
            27,
            Some(&provider_metadata()),
            SuggestionTelemetryDetailLevel::Rich,
        )
        .expect("rich record");

        assert!(
            record
                .metadata_summary
                .contains("result_id=inline:result:2")
        );
        assert!(
            record
                .metadata_summary
                .contains("dismissal_id=44444444-4444-4444-4444-444444444444")
        );
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
    }

    #[test]
    fn crash_reporting_consent_recorded_event_is_metadata_only() {
        let consent = WorkbenchTelemetryConsent {
            enabled: true,
            crash_reports_enabled: true,
            raw_source_allowed: false,
            consent_label: "crash-reports".to_string(),
            schema_version: 1,
        };

        let event = crash_reporting_consent_recorded_event(
            &consent,
            CorrelationId(22),
            CausalityId(Uuid::parse_str("66666666-6666-6666-6666-666666666666").unwrap()),
            EventSequence(18),
            Some(WorkspaceId(7)),
            Some(&PrincipalId("user:test".to_string())),
        )
        .expect("crash consent event");

        assert_eq!(event.event, "telemetry.crash_reporting_consent_recorded");
        assert_eq!(event.retention, RetentionLabel::Audit);
        assert_eq!(event.redaction, RedactionHint::MetadataOnly);
        assert_eq!(event.workspace_id, Some(WorkspaceId(7)));
        assert_eq!(
            event.principal_id,
            Some(PrincipalId("user:test".to_string()))
        );
        assert_eq!(event.payload["choice"], json!("opt_in"));
        assert_eq!(event.payload["crash_reports_enabled"], json!(true));
        assert_eq!(event.payload["consent_label"], json!("crash-reports"));
        assert_eq!(event.payload["metadata_only"], json!(true));
    }

    #[test]
    fn suggestion_telemetry_recorded_event_uses_metadata_only_envelope() {
        let record = suggestion_telemetry_spool_record(
            "inline:suggestion:3",
            WorkspaceId(7),
            CorrelationId(9),
            CausalityId(Uuid::parse_str("55555555-5555-5555-5555-555555555555").unwrap()),
            EventSequence(13),
            InlinePredictionLifecycleAction::Cancel,
            "inline:req:3",
            None,
            None,
            None,
            3,
            None,
            SuggestionTelemetryDetailLevel::MetadataOnly,
        )
        .expect("record");

        let event = suggestion_telemetry_recorded_event(&record).expect("event");
        assert_eq!(event.event, "telemetry.spool_recorded");
        assert_eq!(event.redaction, RedactionHint::MetadataOnly);
        assert_eq!(event.payload["payload_class"], json!("metadata_only"));
        assert_eq!(event.payload["record_id"], json!("inline:suggestion:3"));
    }
}
