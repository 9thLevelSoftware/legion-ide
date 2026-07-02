//! Metadata-only crash summary helpers for minidump capture and symbol upload gating.

use legion_protocol::{
    CausalityId, CorrelationId, EventEnvelope, EventSequence, PrincipalId, RedactionHint,
    RetentionLabel, VsCodeExtensionCrashReport, WorkbenchTelemetryConsent, WorkspaceId,
};

use crate::{EventEnvelopeBuilder, EventSinkConfig, ObservabilityError, validate_envelope};

/// Build a metadata-only crash summary envelope.
///
/// The envelope records that a minidump was captured and only marks the crash as
/// symbolicated when crash-report consent is enabled. When consent is disabled, any
/// requested upload target is suppressed from the payload.
#[allow(clippy::too_many_arguments)]
pub fn crash_summary_recorded_event(
    report: &VsCodeExtensionCrashReport,
    consent: &WorkbenchTelemetryConsent,
    correlation_id: CorrelationId,
    causality_id: CausalityId,
    event_sequence: EventSequence,
    workspace_id: Option<WorkspaceId>,
    principal_id: Option<&PrincipalId>,
    symbol_upload_target: Option<&str>,
) -> Result<EventEnvelope, ObservabilityError> {
    let symbolicated = consent.crash_reports_enabled;
    let symbol_upload_state = if symbolicated && symbol_upload_target.is_some() {
        "queued"
    } else {
        "skipped"
    };

    let mut builder = EventEnvelopeBuilder::new("telemetry.crash_summary_recorded", causality_id)
        .retention(RetentionLabel::Audit)
        .redaction(RedactionHint::MetadataOnly)
        .correlation_id(correlation_id)
        .sequence(event_sequence)
        .metadata("category", "CrashSummary")
        .metadata("crash_id", report.crash_id.clone())
        .metadata("runtime", format!("{:?}", report.runtime))
        .metadata("exit_code", report.exit_code)
        .metadata("signal", report.signal.clone())
        .metadata("metadata_summary", report.metadata_summary.clone())
        .metadata("telemetry_enabled", consent.enabled)
        .metadata("crash_reports_enabled", consent.crash_reports_enabled)
        .metadata("consent_label", consent.consent_label.clone())
        .metadata("symbolicated", symbolicated)
        .metadata("symbol_upload_state", symbol_upload_state)
        .metadata("minidump_captured", true)
        .metadata("payload_class", "metadata_only");

    if let Some(workspace_id) = workspace_id {
        builder = builder.workspace_id(workspace_id);
    }
    if let Some(principal_id) = principal_id {
        builder = builder.principal_id(principal_id.clone());
    }
    if symbolicated && let Some(symbol_upload_target) = symbol_upload_target {
        builder = builder.metadata("symbol_upload_target", symbol_upload_target);
    }

    let envelope = builder.build();
    validate_envelope(&envelope, EventSinkConfig::default())?;
    Ok(envelope)
}

#[cfg(test)]
mod tests {
    use super::*;
    use legion_protocol::{
        CausalityId, CorrelationId, EventSequence, VsCodeExtensionHostRuntime, VsCodeExtensionId,
    };
    use uuid::Uuid;

    fn crash_report() -> VsCodeExtensionCrashReport {
        VsCodeExtensionCrashReport {
            extension_id: VsCodeExtensionId("legion.rust-tools".to_string()),
            runtime: VsCodeExtensionHostRuntime::NodeSidecar,
            crash_id: "crash-001".to_string(),
            exit_code: Some(137),
            signal: Some("SIGKILL".to_string()),
            metadata_summary: "extension host terminated after watchdog timeout".to_string(),
            correlation_id: CorrelationId(7),
            causality_id: CausalityId(
                Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap(),
            ),
            sequence: EventSequence(3),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        }
    }

    #[test]
    fn crash_summary_suppresses_symbol_upload_without_consent() {
        let report = crash_report();
        let consent = WorkbenchTelemetryConsent {
            enabled: false,
            crash_reports_enabled: false,
            raw_source_allowed: false,
            consent_label: "local-only".to_string(),
            schema_version: 1,
        };

        let event = crash_summary_recorded_event(
            &report,
            &consent,
            CorrelationId(22),
            CausalityId(Uuid::parse_str("22222222-2222-2222-2222-222222222222").unwrap()),
            EventSequence(18),
            Some(WorkspaceId(7)),
            Some(&PrincipalId("user:test".to_string())),
            Some("s3://symbols/legion"),
        )
        .expect("crash summary event");

        assert_eq!(event.event, "telemetry.crash_summary_recorded");
        assert_eq!(event.retention, RetentionLabel::Audit);
        assert_eq!(event.redaction, RedactionHint::MetadataOnly);
        assert_eq!(event.payload["category"], serde_json::json!("CrashSummary"));
        assert_eq!(event.payload["symbolicated"], serde_json::json!(false));
        assert_eq!(
            event.payload["symbol_upload_state"],
            serde_json::json!("skipped")
        );
        assert_eq!(event.payload["minidump_captured"], serde_json::json!(true));
        assert!(
            !event
                .payload
                .as_object()
                .unwrap()
                .contains_key("symbol_upload_target")
        );
    }

    #[test]
    fn crash_summary_includes_symbol_upload_target_when_consented() {
        let report = crash_report();
        let consent = WorkbenchTelemetryConsent {
            enabled: true,
            crash_reports_enabled: true,
            raw_source_allowed: false,
            consent_label: "crash-reports".to_string(),
            schema_version: 1,
        };

        let event = crash_summary_recorded_event(
            &report,
            &consent,
            CorrelationId(23),
            CausalityId(Uuid::parse_str("33333333-3333-3333-3333-333333333333").unwrap()),
            EventSequence(19),
            Some(WorkspaceId(7)),
            Some(&PrincipalId("user:test".to_string())),
            Some("s3://symbols/legion"),
        )
        .expect("crash summary event");

        assert_eq!(event.payload["symbolicated"], serde_json::json!(true));
        assert_eq!(
            event.payload["symbol_upload_state"],
            serde_json::json!("queued")
        );
        assert_eq!(
            event.payload["symbol_upload_target"],
            serde_json::json!("s3://symbols/legion")
        );
        assert_eq!(
            event.payload["consent_label"],
            serde_json::json!("crash-reports")
        );
        assert_eq!(event.payload["crash_id"], serde_json::json!("crash-001"));
    }
}
