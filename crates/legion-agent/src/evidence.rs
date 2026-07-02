use super::*;
use legion_ai::redaction::redact_model_bound_output;
use legion_debug::{
    EvidenceProjectionError, debug_adapter_audit_evidence as debug_adapter_audit_artifact,
    debug_adapter_audit_summary as debug_adapter_audit_text,
    test_run_summary_evidence as test_run_summary_artifact, test_run_summary_text,
};
use sha2::{Digest, Sha256};

fn artifact_to_evidence_record(
    worker_id: &LegionWorkflowWorkerId,
    kind: LegionEvidenceKind,
    source: LegionEvidenceSource,
    artifact: legion_protocol::EvidenceArtifact,
    redacted_payload_summary: String,
) -> LegionEvidenceRecord {
    LegionEvidenceRecord {
        evidence_id: format!("legion-evidence:{}:{}", worker_id.0, artifact.artifact_id),
        kind,
        source,
        payload_hash: artifact.summary_hash,
        redacted_payload_summary,
        command_label: artifact.command_label,
        exit_status: artifact.passed.then_some(0).or(Some(1)),
        privacy_scope: LegionEvidencePrivacyScope::WorkspaceMetadata,
        generated_at: artifact.generated_at,
        redaction_hints: artifact.redaction_hints,
        schema_version: artifact.schema_version,
    }
}

fn map_projection_error(error: EvidenceProjectionError) -> AgentError {
    AgentError::InvalidLegionWorkflow(error.to_string())
}

fn fingerprint(value: impl AsRef<[u8]>) -> FileFingerprint {
    let digest = Sha256::digest(value.as_ref());
    FileFingerprint {
        algorithm: "sha256".to_string(),
        value: format!("{:x}", digest),
    }
}

/// Convert an external log into metadata-only evidence for a worker.
pub fn external_log_evidence_record(
    worker_id: &LegionWorkflowWorkerId,
    log_label: &str,
    log_text: &str,
    generated_at: TimestampMillis,
) -> Result<LegionEvidenceRecord, AgentError> {
    let payload_hash = fingerprint(log_text.as_bytes());
    let summary = redact_model_bound_output(
        &format!(
            "external log label={} bytes={} hash={}",
            log_label,
            log_text.len(),
            payload_hash.value
        ),
        160,
    )
    .redacted_text;
    Ok(LegionEvidenceRecord {
        evidence_id: format!("legion-evidence:{}:external-log:{}", worker_id.0, log_label),
        kind: LegionEvidenceKind::CommandRun,
        source: LegionEvidenceSource::LocalTool,
        payload_hash,
        redacted_payload_summary: summary,
        command_label: Some(log_label.to_string()),
        exit_status: None,
        privacy_scope: LegionEvidencePrivacyScope::WorkspaceMetadata,
        generated_at,
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    })
}

/// Convert a debug adapter audit record into metadata-only evidence for a worker.
pub fn debug_adapter_audit_evidence_record(
    worker_id: &LegionWorkflowWorkerId,
    audit: &legion_protocol::DebugAdapterAuditRecord,
    generated_at: TimestampMillis,
) -> Result<LegionEvidenceRecord, AgentError> {
    let summary = debug_adapter_audit_text(audit).map_err(map_projection_error)?;
    let summary = redact_model_bound_output(&summary, 160).redacted_text;
    let artifact =
        debug_adapter_audit_artifact(audit, generated_at).map_err(map_projection_error)?;
    Ok(artifact_to_evidence_record(
        worker_id,
        LegionEvidenceKind::CommandRun,
        LegionEvidenceSource::LocalTool,
        artifact,
        summary,
    ))
}

/// Convert a test-run summary into metadata-only evidence for a worker.
pub fn test_run_summary_evidence_record(
    worker_id: &LegionWorkflowWorkerId,
    summary: &legion_protocol::TestRunSummary,
    generated_at: TimestampMillis,
) -> Result<LegionEvidenceRecord, AgentError> {
    let summary_text = test_run_summary_text(summary).map_err(map_projection_error)?;
    let summary_text = redact_model_bound_output(&summary_text, 160).redacted_text;
    let artifact =
        test_run_summary_artifact(summary, generated_at).map_err(map_projection_error)?;
    Ok(artifact_to_evidence_record(
        worker_id,
        LegionEvidenceKind::CommandRun,
        LegionEvidenceSource::LocalCommand,
        artifact,
        summary_text,
    ))
}

impl LegionWorkflowCoordinator {
    /// Records metadata-only debug adapter evidence for a worker.
    pub fn record_debug_adapter_audit_evidence(
        &mut self,
        worker_id: &LegionWorkflowWorkerId,
        audit: &legion_protocol::DebugAdapterAuditRecord,
        generated_at: TimestampMillis,
    ) -> Result<LegionWorkflowCoordinatorOutput, AgentError> {
        self.find_worker(worker_id)?;
        let evidence = debug_adapter_audit_evidence_record(worker_id, audit, generated_at)?;
        self.evidence_records.push(evidence.clone());
        Ok(LegionWorkflowCoordinatorOutput::EvidenceReady(Box::new(
            evidence,
        )))
    }

    /// Records metadata-only test-run evidence for a worker.
    pub fn record_test_run_summary_evidence(
        &mut self,
        worker_id: &LegionWorkflowWorkerId,
        summary: &legion_protocol::TestRunSummary,
        generated_at: TimestampMillis,
    ) -> Result<LegionWorkflowCoordinatorOutput, AgentError> {
        self.find_worker(worker_id)?;
        let evidence = test_run_summary_evidence_record(worker_id, summary, generated_at)?;
        self.evidence_records.push(evidence.clone());
        Ok(LegionWorkflowCoordinatorOutput::EvidenceReady(Box::new(
            evidence,
        )))
    }

    /// Records metadata-only external log evidence for a worker.
    pub fn record_external_log_evidence(
        &mut self,
        worker_id: &LegionWorkflowWorkerId,
        log_label: &str,
        log_text: &str,
        generated_at: TimestampMillis,
    ) -> Result<LegionWorkflowCoordinatorOutput, AgentError> {
        self.find_worker(worker_id)?;
        let evidence = external_log_evidence_record(worker_id, log_label, log_text, generated_at)?;
        self.evidence_records.push(evidence.clone());
        Ok(LegionWorkflowCoordinatorOutput::EvidenceReady(Box::new(
            evidence,
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use legion_protocol::{
        CausalityId, CommandRiskLabel, ContextManifestItemCount, DebugAdapterAuditRecord,
        DebugSessionId, DebugSessionState, DelegatedTaskAffectedTargetSummary,
        DelegatedTaskOperationClass, DelegatedTaskPlanId, FileFingerprint,
        LegionWorkflowDependency, LegionWorkflowDependencyId, LegionWorkflowDependencyState,
        LegionWorkflowMergeApproval, LegionWorkflowSession, LegionWorkflowSessionId,
        LegionWorkflowSignOff, LegionWorkflowSignOffId, LegionWorkflowSignOffState,
        LegionWorkflowState, LegionWorkflowVerificationGate, LegionWorkflowVerificationGateId,
        LegionWorkflowVerificationGateState, LegionWorkflowWorkerRole, LegionWorkflowWorkerState,
        PrincipalId, PrivacyClassification, ProposalId, ProposalPrivacyLabel, ProposalRiskLabel,
        ProposalTargetKind, RedactionHint, TestControllerId, TestRunId, TestRunState,
        TestRunSummary, TimestampMillis, WorkspaceId,
    };
    use uuid::Uuid;

    fn causality(value: u128) -> CausalityId {
        CausalityId(Uuid::from_u128(value))
    }

    fn workflow_hash(value: &str) -> FileFingerprint {
        FileFingerprint {
            algorithm: "sha256".to_string(),
            value: value.to_string(),
        }
    }

    fn workflow_ref(id: &str) -> legion_protocol::AssistedAiTrustProjectionReference {
        legion_protocol::AssistedAiTrustProjectionReference {
            reference_id: id.to_string(),
            kind: legion_protocol::AssistedAiTrustProjectionKind::AssistedAiProjection,
            projection_hash: workflow_hash(id),
            schema_version: 1,
        }
    }

    fn workflow_target(label: &str) -> DelegatedTaskAffectedTargetSummary {
        DelegatedTaskAffectedTargetSummary {
            target_id: format!("target:{label}"),
            kind: ProposalTargetKind::MetadataOnly,
            workspace_id: Some(WorkspaceId(1)),
            file_id: None,
            buffer_id: None,
            ranges: Vec::new(),
            hashes: vec![workflow_hash(label)],
            counts: vec![ContextManifestItemCount {
                label: "target-count".to_string(),
                count: 1,
            }],
            labels: vec![label.to_string()],
            risk_label: ProposalRiskLabel::Medium,
            privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        }
    }

    fn workflow_worker(
        id: &str,
        backend: LegionWorkflowModelBackend,
        target_label: &str,
    ) -> LegionWorkflowWorkerAssignment {
        LegionWorkflowWorkerAssignment {
            worker_id: LegionWorkflowWorkerId(id.to_string()),
            role: LegionWorkflowWorkerRole::Implementer,
            state: if backend == LegionWorkflowModelBackend::ProviderBacked {
                LegionWorkflowWorkerState::ProviderRouteRequired
            } else {
                LegionWorkflowWorkerState::Ready
            },
            model_backend: backend,
            display_safe_model_label: format!("{id}:metadata"),
            allowed_command_classes: vec![DelegatedTaskOperationClass::DraftProposalMetadata],
            linked_delegated_plan_id: Some(DelegatedTaskPlanId(format!("plan:{id}"))),
            assisted_ai_route: (backend == LegionWorkflowModelBackend::ProviderBacked)
                .then(|| workflow_ref(&format!("route:{id}"))),
            affected_targets: vec![workflow_target(target_label)],
            risk_labels: vec![CommandRiskLabel::Review],
            privacy_labels: vec![PrivacyClassification::Metadata],
            correlation_id: legion_protocol::CorrelationId(901),
            causality_id: causality(901),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        }
    }

    fn workflow_session() -> LegionWorkflowSession {
        LegionWorkflowSession {
            session_id: LegionWorkflowSessionId("session:legion:agent".to_string()),
            directive_artifact_id: Some("artifact:directive:agent".to_string()),
            spec_artifact_id: Some("artifact:spec:agent".to_string()),
            task_graph_artifact_id: Some("artifact:task-graph:agent".to_string()),
            product_mode: legion_protocol::ProductMode::LegionWorkflows,
            worker_assignments: vec![
                workflow_worker(
                    "worker:local",
                    LegionWorkflowModelBackend::Local,
                    "crates/legion-agent/src/lib.rs",
                ),
                workflow_worker(
                    "worker:provider",
                    LegionWorkflowModelBackend::ProviderBacked,
                    "crates/legion-agent/tests/review.rs",
                ),
            ],
            dependency_edges: vec![LegionWorkflowDependency {
                dependency_id: LegionWorkflowDependencyId("dependency:local-provider".to_string()),
                predecessor_worker_id: LegionWorkflowWorkerId("worker:local".to_string()),
                successor_worker_id: LegionWorkflowWorkerId("worker:provider".to_string()),
                state: LegionWorkflowDependencyState::Pending,
                label: "local before provider".to_string(),
                schema_version: 1,
            }],
            conflict_summaries: Vec::new(),
            verification_gates: vec![LegionWorkflowVerificationGate {
                gate_id: LegionWorkflowVerificationGateId("verification:agent".to_string()),
                state: LegionWorkflowVerificationGateState::Passed,
                label: "agent tests".to_string(),
                evidence_artifact_id: Some("artifact:evidence:agent".to_string()),
                command_class_label: "cargo-test".to_string(),
                redaction_hints: vec![RedactionHint::MetadataOnly],
                schema_version: 1,
            }],
            sign_off_records: vec![LegionWorkflowSignOff {
                sign_off_id: LegionWorkflowSignOffId("signoff:agent".to_string()),
                state: LegionWorkflowSignOffState::SignedOff,
                required_role: LegionWorkflowWorkerRole::Reviewer,
                reviewer_principal_id: Some(PrincipalId("principal:reviewer".to_string())),
                label: "review sign-off".to_string(),
                redaction_hints: vec![RedactionHint::MetadataOnly],
                schema_version: 1,
            }],
            proposal_ids: vec![ProposalId(1303)],
            merge_approval: Some(LegionWorkflowMergeApproval {
                approval_artifact_id: Some("artifact:approval:agent".to_string()),
                approval_granted: true,
                rollback_available: true,
                audit_persisted_before_success: true,
                main_workspace_dirty_conflict: false,
                proposal_preconditions_stale: false,
                labels: vec!["approval-gated".to_string()],
                redaction_hints: vec![RedactionHint::MetadataOnly],
                schema_version: 1,
            }),
            lifecycle_state: LegionWorkflowState::Executing,
            generated_at: TimestampMillis(1303),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
            correlation_id: legion_protocol::CorrelationId(901),
            causality_id: causality(902),
        }
    }

    #[test]
    fn evidence_helpers_track_counts_without_raw_logs() {
        let mut coordinator =
            LegionWorkflowCoordinator::new(workflow_session()).expect("valid workflow");
        let worker_id = LegionWorkflowWorkerId("worker:local".to_string());
        let audit = DebugAdapterAuditRecord {
            session_id: DebugSessionId("debug-session-1".to_string()),
            state: DebugSessionState::Exited,
            adapter_type: "lldb".to_string(),
            event_sequence: legion_protocol::EventSequence(17),
            correlation_id: legion_protocol::CorrelationId(9),
            causality_id: causality(17),
            metadata_summary:
                "debug adapter exited cleanly Authorization: Bearer sk-test-1234567890 and "
                    .repeat(4),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        };
        let test_run = TestRunSummary {
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

        let debug_output = coordinator
            .record_debug_adapter_audit_evidence(&worker_id, &audit, TimestampMillis(42))
            .expect("debug evidence");
        let test_output = coordinator
            .record_test_run_summary_evidence(&worker_id, &test_run, TimestampMillis(99))
            .expect("test evidence");

        assert!(matches!(
            debug_output,
            LegionWorkflowCoordinatorOutput::EvidenceReady(_)
        ));
        assert!(matches!(
            test_output,
            LegionWorkflowCoordinatorOutput::EvidenceReady(_)
        ));
        assert_eq!(coordinator.evidence_records_for_worker(&worker_id).len(), 2);
        assert!(
            coordinator.evidence_records()[0]
                .redacted_payload_summary
                .contains("debug session=")
        );
        assert!(
            coordinator.evidence_records()[0]
                .redacted_payload_summary
                .len()
                <= 160
        );
        assert!(
            !coordinator.evidence_records()[0]
                .redacted_payload_summary
                .contains("Authorization: Bearer")
        );
        assert!(
            coordinator.evidence_records()[1]
                .redacted_payload_summary
                .contains("test run=")
        );
        assert!(
            coordinator.evidence_records()[1]
                .redacted_payload_summary
                .len()
                <= 160
        );
    }

    #[test]
    fn debug_evidence_refuses_stack_traces() {
        let audit = DebugAdapterAuditRecord {
            session_id: DebugSessionId("debug-session-2".to_string()),
            state: DebugSessionState::Failed,
            adapter_type: "lldb".to_string(),
            event_sequence: legion_protocol::EventSequence(18),
            correlation_id: legion_protocol::CorrelationId(10),
            causality_id: causality(18),
            metadata_summary: "Traceback (most recent call last): raw stack trace".to_string(),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        };

        let error = debug_adapter_audit_evidence_record(
            &LegionWorkflowWorkerId("worker:local".to_string()),
            &audit,
            TimestampMillis(42),
        )
        .expect_err("stack trace must be rejected");

        assert!(matches!(error, AgentError::InvalidLegionWorkflow(_)));
    }

    #[test]
    fn test_evidence_tracks_counts_without_raw_logs() {
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

        let evidence = test_run_summary_evidence_record(
            &LegionWorkflowWorkerId("worker:local".to_string()),
            &summary,
            TimestampMillis(99),
        )
        .expect("metadata-only test evidence");

        assert_eq!(evidence.command_label.as_deref(), Some("cargo test"));
        assert_eq!(evidence.kind, LegionEvidenceKind::CommandRun);
        assert_eq!(evidence.source, LegionEvidenceSource::LocalCommand);
        assert!(!evidence.redacted_payload_summary.contains("stack trace"));
        assert_eq!(evidence.exit_status, Some(1));
    }

    #[test]
    fn external_log_evidence_is_metadata_only() {
        let evidence = external_log_evidence_record(
            &LegionWorkflowWorkerId("worker:external".to_string()),
            "external-agent.log",
            "Traceback (most recent call last): raw external log body",
            TimestampMillis(123),
        )
        .expect("external log evidence");

        assert_eq!(evidence.kind, LegionEvidenceKind::CommandRun);
        assert_eq!(evidence.source, LegionEvidenceSource::LocalTool);
        assert_eq!(
            evidence.command_label.as_deref(),
            Some("external-agent.log")
        );
        assert_eq!(evidence.payload_hash.algorithm, "sha256");
        assert!(evidence.redacted_payload_summary.contains("external"));
        assert!(
            !evidence
                .redacted_payload_summary
                .contains("raw external log body")
        );
    }
}
