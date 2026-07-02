use legion_protocol::{
    DebugAdapterAuditRecord, DebugSessionState, EvidenceArtifact, FileFingerprint, RedactionHint,
    TestRunState, TestRunSummary, TimestampMillis, VerificationRunState,
};
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum EvidenceProjectionError {
    #[error("metadata-only evidence rejected raw stack-trace markers in {field}")]
    RawStackTrace { field: &'static str },
}

fn fingerprint(value: impl Into<String>) -> FileFingerprint {
    FileFingerprint {
        algorithm: "sha256".to_string(),
        value: value.into(),
    }
}

fn contains_stack_trace_markers(text: &str) -> bool {
    let lowered = text.to_ascii_lowercase();
    lowered.contains("traceback")
        || lowered.contains("stack backtrace")
        || lowered.contains("backtrace")
}

fn ensure_redacted(field: &'static str, value: &str) -> Result<(), EvidenceProjectionError> {
    if contains_stack_trace_markers(value) {
        return Err(EvidenceProjectionError::RawStackTrace { field });
    }
    Ok(())
}

pub fn debug_adapter_audit_summary(
    audit: &DebugAdapterAuditRecord,
) -> Result<String, EvidenceProjectionError> {
    ensure_redacted(
        "debug_adapter_audit.metadata_summary",
        &audit.metadata_summary,
    )?;
    Ok(format!(
        "debug session={} state={:?} adapter={} event_sequence={} correlation={} causality={}",
        audit.session_id.0,
        audit.state,
        audit.adapter_type,
        audit.event_sequence.0,
        audit.correlation_id.0,
        audit.causality_id.0,
    ))
}

pub fn test_run_summary_text(summary: &TestRunSummary) -> Result<String, EvidenceProjectionError> {
    Ok(format!(
        "test run={} controller={} state={:?} passed={} failed={} skipped={} errored={} duration_ms={}",
        summary.run_id.0,
        summary.controller_id.0,
        summary.state,
        summary.passed,
        summary.failed,
        summary.skipped,
        summary.errored,
        summary.duration_ms,
    ))
}

fn debug_run_state(state: DebugSessionState) -> VerificationRunState {
    match state {
        DebugSessionState::Exited => VerificationRunState::Passed,
        DebugSessionState::Failed => VerificationRunState::Failed,
        DebugSessionState::Configured
        | DebugSessionState::Launching
        | DebugSessionState::Running => VerificationRunState::Running,
        DebugSessionState::Paused => VerificationRunState::Blocked,
    }
}

fn test_run_passed(summary: &TestRunSummary) -> bool {
    summary.state == TestRunState::Passed && summary.failed == 0 && summary.errored == 0
}

pub fn debug_adapter_audit_evidence(
    audit: &DebugAdapterAuditRecord,
    generated_at: TimestampMillis,
) -> Result<EvidenceArtifact, EvidenceProjectionError> {
    let summary = debug_adapter_audit_summary(audit)?;
    Ok(EvidenceArtifact {
        artifact_id: format!("artifact:evidence:debug:{}", audit.session_id.0),
        session_id: Some(format!("debug-session:{}", audit.session_id.0)),
        command_label: Some(format!("debug-adapter:{}", audit.adapter_type)),
        run_state: debug_run_state(audit.state),
        summary_hash: fingerprint(format!("debug-audit:{}", summary)),
        passed: audit.state == DebugSessionState::Exited,
        failure_labels: if audit.state == DebugSessionState::Failed {
            vec![format!("debug-session.failed:{}", audit.adapter_type)]
        } else {
            Vec::new()
        },
        screenshot_hashes: Vec::new(),
        log_hashes: vec![fingerprint(format!("debug-audit-log:{}", summary))],
        retention_policy_label: "metadata-only".to_string(),
        raw_payload_retained: false,
        generated_at,
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    })
}

pub fn test_run_summary_evidence(
    summary: &TestRunSummary,
    generated_at: TimestampMillis,
) -> Result<EvidenceArtifact, EvidenceProjectionError> {
    let summary_text = test_run_summary_text(summary)?;
    let passed = test_run_passed(summary);
    Ok(EvidenceArtifact {
        artifact_id: format!("artifact:evidence:test-run:{}", summary.run_id.0),
        session_id: Some(format!("test-controller:{}", summary.controller_id.0)),
        command_label: Some("cargo test".to_string()),
        run_state: match summary.state {
            TestRunState::Passed => VerificationRunState::Passed,
            TestRunState::Failed => VerificationRunState::Failed,
            TestRunState::Skipped => VerificationRunState::Blocked,
            TestRunState::Errored => VerificationRunState::Failed,
            TestRunState::NotRun | TestRunState::Queued | TestRunState::Running => {
                VerificationRunState::Running
            }
        },
        summary_hash: fingerprint(format!("test-run:{}", summary_text)),
        passed,
        failure_labels: if passed {
            Vec::new()
        } else {
            vec![
                format!("test-run.state:{:?}", summary.state),
                format!("test-run.counts:{}-{}", summary.failed, summary.errored),
            ]
        },
        screenshot_hashes: Vec::new(),
        log_hashes: vec![fingerprint(format!("test-run-log:{}", summary_text))],
        retention_policy_label: "metadata-only".to_string(),
        raw_payload_retained: false,
        generated_at,
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use legion_protocol::{
        CausalityId, CorrelationId, DebugSessionId, TestControllerId, TestRunId,
    };
    use uuid::Uuid;

    fn causality(value: u128) -> CausalityId {
        CausalityId(Uuid::from_u128(value))
    }

    #[test]
    fn debug_adapter_audit_evidence_is_metadata_only() {
        let audit = DebugAdapterAuditRecord {
            session_id: DebugSessionId("debug-session-1".to_string()),
            state: DebugSessionState::Exited,
            adapter_type: "lldb".to_string(),
            event_sequence: legion_protocol::EventSequence(17),
            correlation_id: CorrelationId(9),
            causality_id: causality(17),
            metadata_summary: "debug adapter exited cleanly".to_string(),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        };

        let evidence = debug_adapter_audit_evidence(&audit, TimestampMillis(42))
            .expect("metadata-only debug evidence");

        assert_eq!(
            evidence.command_label.as_deref(),
            Some("debug-adapter:lldb")
        );
        assert!(evidence.passed);
        assert_eq!(evidence.run_state, VerificationRunState::Passed);
        assert!(evidence.summary_hash.value.contains("debug-audit:"));
        assert!(!evidence.summary_hash.value.contains("stack trace"));
    }

    #[test]
    fn debug_adapter_audit_evidence_refuses_stack_traces() {
        let audit = DebugAdapterAuditRecord {
            session_id: DebugSessionId("debug-session-2".to_string()),
            state: DebugSessionState::Failed,
            adapter_type: "lldb".to_string(),
            event_sequence: legion_protocol::EventSequence(18),
            correlation_id: CorrelationId(10),
            causality_id: causality(18),
            metadata_summary: "Traceback (most recent call last): raw stack trace".to_string(),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        };

        let error = debug_adapter_audit_summary(&audit).expect_err("stack trace must be rejected");
        assert_eq!(
            error,
            EvidenceProjectionError::RawStackTrace {
                field: "debug_adapter_audit.metadata_summary"
            }
        );
    }

    #[test]
    fn test_run_summary_evidence_tracks_counts_without_raw_logs() {
        let summary = TestRunSummary {
            run_id: TestRunId("run-1".to_string()),
            controller_id: TestControllerId("controller-1".to_string()),
            state: TestRunState::Failed,
            passed: 4,
            failed: 2,
            skipped: 1,
            errored: 1,
            duration_ms: 250,
            schema_version: 1,
        };

        let evidence = test_run_summary_evidence(&summary, TimestampMillis(99))
            .expect("metadata-only test evidence");

        assert_eq!(evidence.command_label.as_deref(), Some("cargo test"));
        assert!(!evidence.passed);
        assert_eq!(evidence.run_state, VerificationRunState::Failed);
        assert_eq!(evidence.failure_labels.len(), 2);
        assert!(evidence.log_hashes[0].value.contains("test-run-log:"));
    }
}
